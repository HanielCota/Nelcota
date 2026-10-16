#!/usr/bin/env python3
"""Read bounded public build/lint diagnostics on a disposable Actions runner."""

import json
import os
import re
import resource
import subprocess
import sys
import tempfile


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def limited_output():
    resource.setrlimit(resource.RLIMIT_FSIZE, (8 * 1024 ** 2, 8 * 1024 ** 2))


def api(path, *, logs=False):
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        result = subprocess.run(["gh", "api", "--hostname", "github.com", "repos/Nelcota/Nelcota/" + path],
                                stdout=output, stderr=errors, timeout=45, preexec_fn=limited_output)
        if result.returncode != 0:
            errors.seek(0)
            text = errors.read(16384).decode(errors="replace")
            codes = re.findall(r"HTTP(?:/[0-9.]+)?\s+([1-5][0-9]{2})", text)
            raise RuntimeError("GitHub diagnostic download failed: exit " + str(result.returncode) +
                               "; HTTP codes " + ",".join(codes[:3]) + "; URLs and raw stderr remain private")
        require(output.tell() <= 8 * 1024 ** 2, "Diagnostic exceeded its eight-MiB file budget")
        output.seek(0)
        content = output.read()
    if logs:
        return content.decode(errors="replace")
    return json.loads(content)


def main():
    require(os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted",
            "CI diagnostic download requires the disposable Actions runner")
    require(len(sys.argv) == 2, "Expected bounded job IDs")
    identifiers = sys.argv[1].split(",")
    require(1 <= len(identifiers) <= 3 and all(re.fullmatch(r"[0-9]{1,20}", value) for value in identifiers),
            "Expected up to three numeric job IDs")
    for identity in identifiers:
        metadata = api("actions/jobs/" + identity)
        require(metadata.get("name") in {"check", "admin-ui"} and metadata.get("conclusion") == "failure",
                "Only failed public Rust/panel CI jobs can be inspected")
        failures = [step["name"] for step in metadata.get("steps", []) if step.get("conclusion") == "failure"]
        allowed = {"Run cargo clippy --all-targets -- -D warnings", "Run npm run check", "versioned dist is up to date"}
        require(failures and all(value in allowed for value in failures), "Refusing unrelated runtime or deployment logs")
        content = api("actions/jobs/" + identity + "/logs", logs=True)
        headers = []
        for raw in content.splitlines():
            line = re.sub(r"\x1b\[[0-9;]*[mK]", "", raw)
            line = re.sub(r"^[0-9T:.+-]+Z\s+", "", line).strip()
            selected = bool(re.match(r"^(error(?:\[E[0-9]+\])?|warning):", line))
            selected = selected or bool(re.match(r"^--> (crates|scripts)/[^\s]+:[0-9]+:[0-9]+$", line))
            selected = selected or "https://rust-lang.github.io/rust-clippy/" in line
            selected = selected or line.startswith("Guide translations need updating")
            if line.startswith("docs/") and ": Missing translation:" in line:
                line = line.split(": Missing translation:", 1)[0] + ": Missing translation"
                selected = True
            if selected and len(headers) < 10:
                headers.append(line[:240])
        report = {"job_id": identity, "source": metadata.get("head_sha"), "failed_steps": failures,
                  "headers": headers, "raw_log_bytes": len(content.encode()),
                  "scope": "Selected public compiler/lint/guide diagnostics only; no runtime logs or test-result certification"}
        message = json.dumps(report, separators=(",", ":"), allow_nan=False)
        message = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        require(len(message.encode()) <= 3900, "Diagnostic annotation exceeded its byte budget")
        print("::notice title=Bounded CI diagnostics::" + message, flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        failure = {"status": "failed", "failure_type": type(error).__name__}
        if isinstance(error, RuntimeError):
            failure["reason"] = str(error)[:512]
        print("::notice title=Bounded CI diagnostic failure::" + json.dumps(failure, separators=(",", ":")), flush=True)
        raise SystemExit(1)
