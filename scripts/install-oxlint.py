#!/usr/bin/env python3
"""Install the checksum-pinned native Oxlint binary inside this checkout."""

import hashlib
import io
import json
import os
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def target_name():
    machine = platform.machine().lower()
    machine = {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    system = platform.system()
    if system == "Darwin":
        return f"{machine}-apple-darwin"
    if system == "Windows":
        return f"{machine}-pc-windows-msvc"
    if system == "Linux":
        musl = platform.libc_ver()[0] == "musl" or any(Path("/lib").glob("ld-musl-*.so.1"))
        libc = "gnu"
        if musl:
            libc = "musl"
        return f"{machine}-unknown-linux-{libc}"
    raise ValueError(f"Unsupported platform: {system} {machine}")


def binary_bytes(archive, asset):
    # Read just the binary; never extract archive paths into the filesystem.
    expected = {"oxlint", "oxlint.exe", f"oxlint-{asset['target']}", f"oxlint-{asset['target']}.exe"}
    if asset["name"].endswith(".zip"):
        with zipfile.ZipFile(io.BytesIO(archive)) as contents:
            matches = [entry for entry in contents.infolist() if not entry.is_dir() and Path(entry.filename).name in expected]
            if len(matches) != 1:
                raise ValueError("Release archive must contain exactly one Oxlint binary")
            if matches[0].file_size > 100 * 1024 * 1024:
                raise ValueError("Release binary exceeds the expected size limit")
            return contents.read(matches[0])
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as contents:
        matches = [entry for entry in contents.getmembers() if entry.isfile() and Path(entry.name).name in expected]
        if len(matches) != 1:
            raise ValueError("Release archive must contain exactly one Oxlint binary")
        if matches[0].size > 100 * 1024 * 1024:
            raise ValueError("Release binary exceeds the expected size limit")
        return contents.extractfile(matches[0]).read()


def main():
    lock = json.loads((ROOT / "presets/oxlint.lock.json").read_text())
    target = target_name()
    asset = lock["assets"].get(target)
    if asset is None:
        raise ValueError(f"No pinned binary for {target}; supported targets: {', '.join(lock['assets'])}")
    binary_name = {"Windows": "oxlint.exe"}.get(platform.system(), "oxlint")
    destination = ROOT / ".tools/oxlint" / binary_name
    expected_version = f"Version: {lock['version']}"
    if destination.is_file():
        result = subprocess.run([str(destination), "--version"], capture_output=True, text=True, timeout=10)
        if result.returncode == 0 and result.stdout.strip() == expected_version:
            print(f"Oxlint {lock['version']} already installed at {destination.relative_to(ROOT)}")
            return
    request = urllib.request.Request(asset["url"], headers={"User-Agent": "Nelcota review-tool installer"})
    with urllib.request.urlopen(request, timeout=30) as response:
        archive = response.read(asset["size"] + 1)
    if len(archive) != asset["size"]:
        raise ValueError("Release archive size does not match the pinned asset")
    if hashlib.sha256(archive).hexdigest() != asset["sha256"]:
        raise ValueError("Release archive checksum does not match; nothing installed")
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix="oxlint-", suffix={"nt": ".exe"}.get(os.name, ""), dir=destination.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(binary_bytes(archive, asset))
        os.chmod(temporary, 0o755)
        result = subprocess.run([temporary, "--version"], capture_output=True, text=True, timeout=10)
        if result.returncode != 0 or result.stdout.strip() != expected_version:
            raise ValueError("Downloaded binary does not report the pinned version")
        os.replace(temporary, destination)
    finally:
        Path(temporary).unlink(missing_ok=True)
    print(f"Installed native Oxlint {lock['version']} at {destination.relative_to(ROOT)} (SHA-256 verified)")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f"Oxlint installation failed: {error}", file=sys.stderr)
        raise SystemExit(2)
