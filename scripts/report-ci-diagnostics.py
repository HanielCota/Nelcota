#!/usr/bin/env python3
"""Read bounded public build/lint diagnostics on a disposable Actions runner."""

import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

MAX_BYTES = 8 * 1024 ** 2
PANIC = re.compile(r"^thread '([A-Za-z0-9_:]{1,200})'(?: \([0-9]{1,12}\))? panicked at ((?:crates|sdk)/[A-Za-z0-9_./-]+\.rs):([0-9]{1,8}):([0-9]{1,6}):$")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


class BoundedRedirect(urllib.request.HTTPRedirectHandler):
    def __init__(self):
        self.redirects = 0

    def redirect_request(self, request, response, code, message, headers, target):
        self.redirects += 1
        parsed = urllib.parse.urlsplit(target)
        host = parsed.hostname or ""
        allowed = host in {"api.github.com", "github.com"} or host.endswith((".githubusercontent.com", ".blob.core.windows.net"))
        require(self.redirects <= 3 and parsed.scheme == "https" and parsed.port in {None, 443}
                and parsed.username is None and parsed.password is None and allowed,
                "Refusing an untrusted diagnostic redirect")
        redirected = super().redirect_request(request, response, code, message, headers, target)
        require(redirected is not None, "Unsupported diagnostic redirect")
        # A signed download URL authenticates itself. Never forward the API token.
        redirected.remove_header("Authorization")
        return redirected


def api(path, *, logs=False):
    require(re.fullmatch(r"actions/jobs/[0-9]{1,20}(?:/logs)?", path) is not None, "Unexpected diagnostic API path")
    token = os.environ.get("GH_TOKEN", "")
    require(0 < len(token) <= 512 and token.isascii() and not any(char.isspace() for char in token), "Missing diagnostic API token")
    request = urllib.request.Request("https://api.github.com/repos/Nelcota/Nelcota/" + path,
                                     headers={"User-Agent": "nelcota-bounded-ci-diagnostics", "Accept": "application/vnd.github+json"})
    request.add_unredirected_header("Authorization", "Bearer " + token)
    opener = urllib.request.build_opener(BoundedRedirect())
    began = time.monotonic()
    content = bytearray()
    try:
        with opener.open(request, timeout=15) as response:
            while True:
                require(time.monotonic() - began <= 45, "Diagnostic exceeded its duration budget")
                chunk = response.read(min(65536, MAX_BYTES + 1 - len(content)))
                if not chunk:
                    break
                content.extend(chunk)
                require(len(content) <= MAX_BYTES, "Diagnostic exceeded its eight-MiB file budget")
    except urllib.error.HTTPError as error:
        code = error.code
        error.close()
        raise RuntimeError("GitHub diagnostic download failed: HTTP " + str(code) + "; URLs and raw body remain private") from None
    except (OSError, urllib.error.URLError):
        raise RuntimeError("GitHub diagnostic transport failed; URLs and raw errors remain private") from None
    if logs:
        return content.decode(errors="replace")
    return json.loads(content)


def panic_locations(content):
    findings = []
    current = None
    for raw in content.splitlines():
        line = re.sub(r"\x1b\[[0-9;]*[mK]", "", raw)
        line = re.sub(r"^[0-9T:.+-]+Z\s+", "", line).strip()
        match = PANIC.fullmatch(line)
        if match is not None and len(match[2]) <= 240 and ".." not in match[2].split("/"):
            current = {"test": match[1], "file": match[2], "line": int(match[3]), "column": int(match[4])}
            if len(findings) < 16:
                findings.append(current)
            continue
        if current is None:
            continue
        categories = {"Text file busy": "exec_text_file_busy", "native application runs a different executable": "running_executable_identity_mismatch",
                      "No such file or directory": "missing_process_or_file", "called `Result::unwrap_err()` on an `Ok` value": "unexpected_success"}
        for marker, category in categories.items():
            if marker in line[:512]:
                current["category"] = category
        current = None
    return findings


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
        allowed = {"Run cargo clippy --all-targets -- -D warnings", "Run npm run check", "versioned dist is up to date", "Run set -o pipefail"}
        require(failures and all(value in allowed for value in failures), "Refusing unrelated runtime or deployment logs")
        content = api("actions/jobs/" + identity + "/logs", logs=True)
        headers = []
        for raw in content.splitlines():
            if "Run set -o pipefail" in failures:
                break
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
                  "panic_locations": panic_locations(content),
                  "scope": "Selected public compiler/lint/guide diagnostics and Rust panic source locations/categories only; no assertion payloads, runtime logs or test-result certification"}
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
