"""Regressions for falsely accepted performance/ownership evidence."""

from pathlib import Path
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
COMPARE = ROOT / "bench/compare.py"
CHECKER = ROOT / "scripts/check-boundaries.py"


class Comparison(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.baseline, self.candidate = self.root / "baseline", self.root / "candidate"
        metadata = {
            "cpu_affinity": [0, 1, 2],
            "app_cpus": 1,
            "app_memory_mb": 128,
            "pg_cpus": 1,
            "pg_memory_mb": 384,
            "generator_memory_mb": 512,
            "generator_go_memory_limit_mib": 448,
            "pg_data": "disk",
            "max_wal_mb": 64,
            "max_fixture_disk_mb": 512,
            "pool": 8,
            "max_connections": 40,
            "k6": "pinned fixture digest",
            "storage_backend": "disk",
            "steady_rate": 0,
            "postgres_version": "PostgreSQL 17 fixture",
            "images": {},
            "dataset": {"products": 100000, "private": 100000, "objects": 10000},
        }
        for directory in [self.baseline, self.candidate]:
            directory.mkdir()
            (directory / "metadata.json").write_text(json.dumps(metadata))
            for iteration in range(1, 4):
                (directory / f"read-{iteration}.json").write_text(
                    json.dumps(
                        {
                            "metrics": {
                                "unexpected": {"value": 0},
                                "checks": {"fails": 0},
                                "http_req_duration": {
                                    "p(50)": 1,
                                    "p(95)": 2,
                                    "p(99)": 3,
                                },
                                "http_reqs": {"rate": 100, "count": 3000},
                            }
                        }
                    )
                )

    def compare(self):
        output = self.root / "report.json"
        result = subprocess.run(
            [
                sys.executable,
                str(COMPARE),
                str(self.baseline),
                str(self.candidate),
                "--workloads",
                "read",
                "--output",
                str(output),
            ],
            capture_output=True,
            text=True,
        )
        return result.returncode, json.loads(output.read_text())

    def change_sample(self, directory, iteration, update):
        path = directory / f"read-{iteration}.json"
        value = json.loads(path.read_text())
        update(value["metrics"])
        path.write_text(json.dumps(value))

    def test_equivalent_three_valid_runs_pass(self):
        code, report = self.compare()
        self.assertEqual(code, 0)
        self.assertTrue(report["rows"][0]["gate_passed"])

    def useful_fixture(self,candidate_rate):
        for directory in [self.baseline,self.candidate]:
            path=directory/'metadata.json'
            metadata=json.loads(path.read_text());metadata['schema']=2
            path.write_text(json.dumps(metadata))
            (directory/'completion.json').write_text(json.dumps({'status':'passed','cleanup_confirmed':True}))
            (directory/'cleanup.json').write_text(json.dumps({'containers':[],'volumes':[],'directory_removed':True}))
            rate=100
            if directory==self.candidate:
                rate=candidate_rate
            for index in range(1,4):
                self.change_sample(directory,index,lambda metrics:metrics.update(successful_operations={'rate':rate,'count':rate*30}))

    def test_rejected_requests_cannot_mask_twenty_percent_loss_of_useful_work(self):
        self.useful_fixture(80)
        code,report=self.compare()
        self.assertNotEqual(code,0)
        self.assertTrue(report['useful_throughput_verified'])
        self.assertAlmostEqual(report['rows'][0]['rps_ratio'],.8)

    def test_missing_useful_counter_cannot_fall_back_to_http_requests(self):
        self.useful_fixture(100)
        self.change_sample(self.candidate,1,lambda metrics:metrics.pop('successful_operations'))
        code,report=self.compare()
        self.assertNotEqual(code,0)
        self.assertFalse(report['rows'][0]['comparable'])

    def test_later_campaign_failure_cannot_be_hidden_by_good_steady_samples(self):
        self.useful_fixture(100)
        (self.baseline/'completion.json').write_text(json.dumps({'status':'failed','cleanup_confirmed':True}))
        code,report=self.compare()
        self.assertNotEqual(code,0)
        self.assertFalse(report['rows'][0]['comparable'])

    def test_completed_equivalent_useful_protocol_passes(self):
        self.useful_fixture(100)
        code,report=self.compare()
        self.assertEqual(code,0)
        self.assertTrue(report['rows'][0]['gate_passed'])

    def test_failed_baseline_cannot_certify_candidate(self):
        self.change_sample(
            self.baseline, 1, lambda value: value["unexpected"].update(value=1)
        )
        code, report = self.compare()
        self.assertNotEqual(code, 0)
        self.assertFalse(report["rows"][0]["comparable"])
        self.assertFalse(report["rows"][0]["gate_passed"])

    def test_different_backend_or_resource_budget_is_incomparable(self):
        for field, replacement in [
            ("storage_backend", "s3"),
            ("generator_memory_mb", 256),
        ]:
            path = self.candidate / "metadata.json"
            metadata = json.loads((self.baseline / "metadata.json").read_text())
            metadata[field] = replacement
            path.write_text(json.dumps(metadata))
            code, report = self.compare()
            self.assertNotEqual(code, 0)
            self.assertIn(field, report["fixture_differences"])

    def test_nonfinite_latency_cannot_pass_the_gate(self):
        self.change_sample(
            self.candidate,
            2,
            lambda value: value["http_req_duration"].update({"p(95)": float("nan")}),
        )
        code, report = self.compare()
        self.assertNotEqual(code, 0)
        self.assertFalse(report["rows"][0]["gate_passed"])

    def test_different_metric_protocol_cannot_certify_candidate(self):
        path = self.candidate / 'metadata.json'
        metadata = json.loads(path.read_text())
        metadata['metrics_backend'] = 'different estimator'
        path.write_text(json.dumps(metadata))
        code, report = self.compare()
        self.assertNotEqual(code, 0)
        self.assertIn('metrics_backend', report['fixture_differences'])

    def test_different_nominal_duration_is_incomparable(self):
        for iteration in range(1, 4):
            self.change_sample(
                self.candidate,
                iteration,
                lambda value: value["http_reqs"].update(count=6000),
            )
        code, report = self.compare()
        self.assertNotEqual(code, 0)
        self.assertFalse(report["rows"][0]["comparable"])


class Ownership(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("goal_boundaries", CHECKER)
        self.checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.checker)
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.checker.ROOT = self.root
        (self.root / "Cargo.toml").write_text("[workspace.dependencies]\n")
        for capability, dependencies in self.checker.EXPECTED.items():
            path = self.root / "crates" / capability
            path.mkdir(parents=True)
            (path / "Cargo.toml").write_text(
                "[dependencies]\n"
                + "".join(
                    f'nelcota-{dependency} = "0.4.0"\n' for dependency in dependencies
                )
            )
        (self.root / "migrations").mkdir()
        (self.root / "crates/core/migrations").mkdir()

    def test_renamed_dependency_cannot_hide_reverse_boundary(self):
        path = self.root / "crates/core/Cargo.toml"
        path.write_text(
            path.read_text()
            + 'hidden = { package = "nelcota-auth", version = "0.4.0" }\n'
        )
        with self.assertRaisesRegex(SystemExit, "core"):
            self.checker.check()

    def test_target_specific_dependency_cannot_hide_reverse_boundary(self):
        path = self.root / "crates/core/Cargo.toml"
        path.write_text(
            path.read_text()
            + "[target.'cfg(windows)'.dependencies]\nnelcota-auth = \"0.4.0\"\n"
        )
        with self.assertRaisesRegex(SystemExit, "core"):
            self.checker.check()

    def test_workspace_alias_is_checked_by_package_name(self):
        (self.root / "Cargo.toml").write_text(
            '[workspace.dependencies]\nhidden = { package = "nelcota-auth", version = "0.4.0" }\n'
        )
        path = self.root / "crates/core/Cargo.toml"
        path.write_text(path.read_text() + "hidden.workspace = true\n")
        with self.assertRaisesRegex(SystemExit, "core"):
            self.checker.check()


class DiagnosticSafety(unittest.TestCase):
    def campaign_module(self):
        spec = importlib.util.spec_from_file_location(
            "goal_campaign", ROOT / "bench/campaign.py"
        )
        campaign = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(campaign)
        return campaign

    def test_missing_resource_samples_cannot_certify_campaign(self):
        campaign = self.campaign_module()
        self.assertTrue(campaign.resource_evidence_violations([], 50))
        self.assertTrue(campaign.resource_evidence_violations([{"elapsed": 1}], 50))

    def test_resource_observation_gap_cannot_certify_campaign(self):
        campaign = self.campaign_module()
        self.assertTrue(
            campaign.resource_evidence_violations([{"elapsed": 1}, {"elapsed": 32}], 35)
        )
        self.assertEqual(
            campaign.resource_evidence_violations(
                [{"elapsed": 1}, {"elapsed": 15}], 20
            ),
            [],
        )

    def test_early_stopped_sampler_cannot_certify_unobserved_tail(self):
        campaign = self.campaign_module()
        self.assertTrue(
            campaign.resource_evidence_violations([{"elapsed": 1}, {"elapsed": 15}], 46)
        )

    def test_docker_timeout_does_not_expose_credential_arguments(self):
        campaign = self.campaign_module()
        timeout = subprocess.TimeoutExpired(
            ["docker", "run", "-e", "PASSWORD=fixture-secret"], 45
        )
        with mock.patch.object(campaign.subprocess, "run", side_effect=timeout):
            with self.assertRaisesRegex(
                RuntimeError, "Docker operation timed out"
            ) as raised:
                campaign.docker("run", "-e", "PASSWORD=fixture-secret")
        self.assertNotIn("fixture-secret", str(raised.exception))
        self.assertTrue(raised.exception.__suppress_context__)


if __name__ == "__main__":
    unittest.main()
