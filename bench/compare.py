"""Compare equivalent native k6 campaign medians without hiding failed runs."""

import argparse
import json
import math
from pathlib import Path
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("baseline", type=Path)
parser.add_argument("candidate", type=Path)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument(
    "--workloads",
    type=lambda value: value.split(","),
    default=["read", "rls", "panel", "storage", "login", "refresh", "mixed"],
)
args = parser.parse_args()
rows = []
failed = []
fixture_fields = [
    "cpu_affinity",
    "app_cpus",
    "app_memory_mb",
    "pg_cpus",
    "pg_memory_mb",
    "generator_memory_mb",
    "generator_go_memory_limit_mib",
    "pg_data",
    "max_wal_mb",
    "max_fixture_disk_mb",
    "pool",
    "max_connections",
    "k6",
    "storage_backend",
    "steady_rate",
    "postgres_version",
    "images",
    "dataset",
]
metadata = [
    json.loads((directory / "metadata.json").read_text())
    for directory in [args.baseline, args.candidate]
]
for field in ["steady_duration", "workloads", "s3", "schema", "metrics_backend", "metrics_memory_mb", "metrics_cpus", "offered_rate_unit", "nominal_slos_ms", "auth_rate_limit_per_minute", "autovacuum", "webhook_history_maintenance"]:
    if any(field in value for value in metadata):
        fixture_fields.append(field)
fixture_differences = [
    field
    for field in fixture_fields
    if any(field not in value for value in metadata)
    or metadata[0].get(field) != metadata[1].get(field)
]


def valid_metrics(metrics):
    try:
        values = [
            metrics["http_req_duration"][key] for key in ["p(50)", "p(95)", "p(99)"]
        ]
        rate = metrics["http_reqs"]["rate"]
        count = metrics["http_reqs"]["count"]
        if not all(
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and math.isfinite(value)
            for value in [*values, rate, count]
        ):
            return False
        return (
            metrics["unexpected"]["value"] == 0
            and metrics["checks"]["fails"] == 0
            and 0 <= values[0] <= values[1] <= values[2]
            and values[1] > 0
            and rate > 0
            and count > 0
        )
    except (KeyError, TypeError):
        return False


for workload in args.workloads:
    measurements = []
    for directory in [args.baseline, args.candidate]:
        values = [
            json.loads((directory / f"{workload}-{i}.json").read_text())["metrics"]
            for i in range(1, 4)
        ]
        valid = [metrics for metrics in values if valid_metrics(metrics)]
        measurement = {
            "successful_runs": len(valid),
            "failures": len(values) - len(valid),
        }
        if valid:
            for name, key in [
                ("p50_ms", "p(50)"),
                ("p95_ms", "p(95)"),
                ("p99_ms", "p(99)"),
            ]:
                measurement[name] = statistics.median(
                    metrics["http_req_duration"][key] for metrics in valid
                )
            measurement["rps"] = statistics.median(
                metrics["http_reqs"]["rate"] for metrics in valid
            )
            measurement["duration_seconds"] = statistics.median(
                metrics["http_reqs"]["count"] / metrics["http_reqs"]["rate"]
                for metrics in valid
            )
        measurements.append(measurement)
    baseline, candidate = measurements
    comparable = (
        baseline["successful_runs"] == 3
        and candidate["successful_runs"] == 3
        and not fixture_differences
    )
    if (
        comparable
        and abs(baseline["duration_seconds"] - candidate["duration_seconds"]) > 1
    ):
        comparable = False
    passed = comparable
    row = {
        "workload": workload,
        "baseline": baseline,
        "candidate": candidate,
        "comparable": comparable,
    }
    if comparable:
        row["p95_ratio"] = candidate["p95_ms"] / baseline["p95_ms"]
        row["rps_ratio"] = candidate["rps"] / baseline["rps"]
        passed = row["p95_ratio"] <= 1.10 and row["rps_ratio"] >= 0.90
    row["gate_passed"] = passed
    if not passed:
        failed.append(workload)
    rows.append(row)
report = {
    "p95_limit": 1.10,
    "rps_minimum": 0.90,
    "fixture_differences": fixture_differences,
    "rows": rows,
    "failed": failed,
}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
for row in rows:
    print(
        row["workload"], "comparable:", row["comparable"], "passed:", row["gate_passed"]
    )
if failed:
    raise SystemExit("Regression envelope not met: " + ", ".join(failed))
