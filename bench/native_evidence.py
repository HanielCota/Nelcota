#!/usr/bin/env python3
"""Publish bounded, lossless Actions notices for an already-public VM report."""

import argparse
import hashlib
import json
from pathlib import Path
import re

FIELDS = {"identity", "source", "certifies_g05", "checks", "image", "image_checksum_manifest",
          "native_init", "tls_ca_sha256", "status", "failure", "guest_diagnostics",
          "diagnostic_failure_type", "elapsed_seconds_including_preparation", "resource_samples",
          "cleanup_confirmed", "cleanup_scope", "safety_guard_failure", "backup",
          "restore_elapsed_seconds", "completed_effective_load_seconds", "power_cut_unit_state",
          "guest_power_cut_recovery_seconds", "power_cut_reconciliation", "idle_resources", "native_remove"}


def notices(report):
    if not isinstance(report, dict) or set(report) - FIELDS:
        raise ValueError("Unknown native report fields; refusing publication")
    if report.get("certifies_g05") is not False or report.get("status") not in {"passed", "failed"}:
        raise ValueError("Invalid native report verdict")
    if not isinstance(report.get("cleanup_confirmed"), bool):
        raise ValueError("Missing native cleanup receipt")
    if not isinstance(report.get("source"), str) or re.fullmatch(r"[0-9a-f]{40}", report["source"]) is None:
        raise ValueError("Missing immutable native report source")
    text = json.dumps(report, separators=(",", ":"), allow_nan=False)
    if len(text.encode()) > 64 * 1024:
        raise ValueError("Native report exceeds the notice boundary")
    digest = hashlib.sha256(text.encode()).hexdigest()
    fragments = [text[index:index + 900] for index in range(0, len(text), 900)]
    output = []
    summary = {key: report.get(key) for key in ["identity", "source", "status", "failure", "cleanup_confirmed",
                                               "resource_samples", "elapsed_seconds_including_preparation"]}
    summary["report_sha256"] = digest
    output.append(("Native VM summary", summary))
    for index, fragment in enumerate(fragments, 1):
        output.append(("Native VM evidence", {"part": index, "parts": len(fragments), "sha256": digest, "payload": fragment}))
    values = []
    for title, value in output:
        encoded = json.dumps(value, separators=(",", ":"), allow_nan=False)
        encoded = encoded.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        line = "::notice title=" + title + "::" + encoded
        if len(line.encode()) > 4000:
            raise ValueError("Native notice exceeds the Actions annotation limit")
        values.append(line)
    return values


def publish(report):
    for line in notices(report):
        print(line, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    if args.report.is_symlink() or not args.report.is_file() or args.report.stat().st_size > 128 * 1024:
        raise SystemExit("Native public report is missing or exceeds its file boundary")
    publish(json.loads(args.report.read_text()))
