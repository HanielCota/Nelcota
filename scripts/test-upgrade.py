#!/usr/bin/env python3
"""Run the 0.3.1 upgrade regression against a checkout or the 0.4.0 release."""

import argparse
import json
import platform
import re
import signal
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

from upgrade.scenario import UpgradeScenario
from upgrade.support import release_binary

REPO = Path(__file__).resolve().parent.parent


def interrupt(signum, frame):
    raise KeyboardInterrupt("Upgrade test interrupted")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--published-target",
        action="store_true",
        help="test the checksum-pinned v0.4.0 release instead of the checkout",
    )
    parser.add_argument(
        "--to-binary", type=Path, help="checkout binary (default: target/debug/nelcota)"
    )
    parser.add_argument("--cache-dir", type=Path, default=REPO / "target/upgrade-binaries")
    parser.add_argument("--artifacts-dir", type=Path, default=REPO / "target/upgrade-tests")
    args = parser.parse_args()
    if args.published_target and args.to_binary:
        parser.error("--published-target and --to-binary are mutually exclusive")
    if not __debug__:
        parser.error("assertions must be enabled (do not use Python -O)")
    if sys.platform != "linux" or platform.machine() not in {"x86_64", "amd64"}:
        parser.error("published baseline requires Linux amd64")
    signal.signal(signal.SIGTERM, interrupt)
    config = json.loads((REPO / "scripts/upgrade/releases.json").read_text())
    args.artifacts_dir.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="upgrade-", dir=args.artifacts_dir.resolve()))
    scenario = None
    report = {"success": False, "checks": []}
    status = 1
    try:
        subprocess.run(
            ["docker", "info"],
            check=True,
            timeout=30,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        old = release_binary(config, config["baseline"], args.cache_dir)
        target = (args.to_binary or REPO / "target/debug/nelcota").resolve()
        version = tomllib.loads((REPO / "Cargo.toml").read_text())["workspace"]["package"][
            "version"
        ]
        migrations = sorted(
            int(re.match(r"V(\d+)__", path.name)[1])
            for path in (REPO / "migrations").glob("V*__*.sql")
        )
        if args.published_target:
            target = release_binary(config, config["target"], args.cache_dir)
            version = config["target"]["version"]
            migrations = list(range(1, 14))
        for binary, expected in [(old, "0.3.1"), (target, version)]:
            if not binary.is_file():
                raise RuntimeError(
                    "Build the checkout first: cargo build --locked -p nelcota-server"
                )
            actual = subprocess.check_output(
                [str(binary), "--version"], text=True, timeout=15
            ).strip()
            if actual != "nelcota " + expected:
                raise RuntimeError(f"Expected {expected}, got {actual}")
        scenario = UpgradeScenario(root, old, target, version, migrations)
        report = scenario.report
        report["target_kind"] = "checkout"
        if args.published_target:
            report["target_kind"] = "published"
        scenario.prepare(config["postgres_image"])
        scenario.run()
        status = 0
    except BaseException as error:
        report["success"] = False
        report["error"] = repr(error)
        print("FAIL: " + str(error), file=sys.stderr)
        raise
    finally:
        if scenario:
            try:
                scenario.close()
            except RuntimeError as error:
                status = 1
                print("FAIL: " + str(error), file=sys.stderr)
        (root / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print("Upgrade report: " + str(root / "report.json"), flush=True)
    return status


if __name__ == "__main__":
    sys.exit(main())
