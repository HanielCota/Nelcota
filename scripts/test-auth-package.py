"""Compile and run an external consumer from packaged auth/core archives."""

import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import argparse
from contextlib import contextmanager
import json
import base64
import secrets
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--run",
    action="store_true",
    help="Create disposable PostgreSQL and exercise the archive consumer",
)
arguments = parser.parse_args()
if not __debug__:
    parser.error("Assertions must be enabled; do not use Python -O")

ROOT = Path(__file__).resolve().parents[1]


@contextmanager
def runtime_fixture(environment):
    if not arguments.run:
        yield environment
        return
    name = "nelcota-auth-package-" + secrets.token_hex(6)
    password = secrets.token_urlsafe(32)
    created = False
    try:
        try:
            creation = subprocess.run(
                [
                    "docker",
                    "run",
                    "-d",
                    "--name",
                    name,
                    "-e",
                    "POSTGRES_PASSWORD=" + password,
                    "-p",
                    "127.0.0.1::5432",
                    "--tmpfs",
                    "/var/lib/postgresql/data",
                    "postgres:17-alpine",
                ],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                timeout=45,
            )
        except subprocess.TimeoutExpired:
            raise RuntimeError("Disposable PostgreSQL creation timed out") from None
        created = creation.returncode == 0
        if creation.returncode:
            # CalledProcessError would include Docker's plaintext password argument.
            raise RuntimeError("Disposable PostgreSQL creation failed")
        inspected = json.loads(subprocess.check_output(["docker", "inspect", name]))[0]
        port = inspected["NetworkSettings"]["Ports"]["5432/tcp"][0]["HostPort"]
        for _ in range(100):
            if (
                subprocess.run(
                    [
                        "docker",
                        "exec",
                        name,
                        "pg_isready",
                        "-h",
                        "127.0.0.1",
                        "-U",
                        "postgres",
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                ).returncode
                == 0
            ):
                break
            time.sleep(0.1)
        import socket

        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            bind = "127.0.0.1:" + str(probe.getsockname()[1])
        environment.update(
            {
                "AUTH_DATABASE_URL": f"postgres://postgres:{password}@127.0.0.1:{port}/postgres",
                "AUTH_DATABASE_PASSWORD": secrets.token_urlsafe(32),
                "AUTH_PRIVATE_KEY": base64.b64encode(
                    subprocess.check_output(
                        [
                            "openssl",
                            "genpkey",
                            "-algorithm",
                            "Ed25519",
                            "-outform",
                            "DER",
                        ]
                    )
                ).decode(),
                "AUTH_BIND": bind,
            }
        )
        yield environment
        objects = subprocess.check_output(
            [
                "docker",
                "exec",
                name,
                "psql",
                "-Atq",
                "-U",
                "postgres",
                "-c",
                "SELECT count(*) FROM pg_tables WHERE schemaname IN ('storage','webhooks') OR (schemaname='nelcota' AND tablename<>'schema_migrations');",
            ]
        ).strip()
        assert objects == b"0", "Archive consumer installed unrelated capabilities"
        active = subprocess.check_output(
            [
                "docker",
                "exec",
                name,
                "psql",
                "-Atq",
                "-U",
                "postgres",
                "-c",
                "SELECT count(*) FROM pg_stat_activity WHERE usename='authenticator';",
            ]
        ).strip()
        assert active == b"0", "Consumer shutdown retained request connections"
    finally:
        cleanup = subprocess.run(
            ["docker", "rm", "-fv", name],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=45,
        )
        if created and cleanup.returncode:
            raise RuntimeError("Disposable PostgreSQL cleanup failed")


with tempfile.TemporaryDirectory(prefix="nelcota-auth-package-") as temporary:
    directory = Path(temporary)
    packages = {}
    for name in ["nelcota-core", "nelcota-auth"]:
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        version = manifest["workspace"]["package"]["version"]
        archive = ROOT / "target/package" / f"{name}-{version}.crate"
        if not archive.is_file():
            raise SystemExit(
                "Run cargo package -p nelcota-core -p nelcota-auth --allow-dirty --no-verify first"
            )
        with tarfile.open(archive) as source:
            source.extractall(directory, filter="data")
        packages[name] = directory / f"{name}-{version}"
    consumer = directory / "consumer"
    shutil.copytree(
        ROOT / "examples/auth-consumer",
        consumer,
        ignore=shutil.ignore_patterns("target", "Cargo.lock"),
    )
    manifest = consumer / "Cargo.toml"
    text = manifest.read_text().replace(
        'path = "../../crates/auth"',
        'path = "../' + packages["nelcota-auth"].name + '"',
    )
    text += (
        '\n[patch.crates-io]\nnelcota-core = { path = "../'
        + packages["nelcota-core"].name
        + '" }\n'
    )
    manifest.write_text(text)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(ROOT / "target/auth-consumer")
    command = ["cargo", "build", "--offline", "--manifest-path", str(manifest)]
    subprocess.run(command, env=environment, check=True)
    graph = subprocess.run(
        [
            "cargo",
            "tree",
            "--offline",
            "--manifest-path",
            str(manifest),
            "--prefix",
            "none",
        ],
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    for excluded in [
        "nelcota-admin",
        "nelcota-storage",
        "nelcota-api",
        "nelcota-webhooks",
        "nelcota-cli",
        "nelcota-server",
    ]:
        if any(line.startswith(excluded + " ") for line in graph.splitlines()):
            raise SystemExit("Unexpected consumer dependency: " + excluded)
    print("Archive consumer builds without excluded capabilities")
    with runtime_fixture(environment) as environment:
        if arguments.run:
            # Runtime credentials and PostgreSQL must be disposable. Ctrl-C uses
            # the example's real graceful shutdown; no production DB is inferred.
            import signal
            import time
            import json
            import urllib.request
            import urllib.error
            import secrets

            bind = environment.get("AUTH_BIND", "127.0.0.1:8080")
            process = subprocess.Popen(
                [str(ROOT / "target/auth-consumer/debug/nelcota-auth-consumer")],
                env=environment,
            )
            try:
                base = "http://" + bind
                for _ in range(100):
                    if process.poll() is not None:
                        raise RuntimeError("Archive consumer exited during startup")
                    try:
                        with urllib.request.urlopen(
                            base + "/auth/v1/.well-known/jwks.json", timeout=1
                        ) as response:
                            assert json.load(response)["keys"]
                        break
                    except urllib.error.URLError:
                        time.sleep(0.1)
                password = secrets.token_urlsafe(32)
                body = json.dumps(
                    {
                        "email": secrets.token_hex(8) + "@example.com",
                        "password": password,
                    }
                ).encode()
                request = urllib.request.Request(
                    base + "/auth/v1/signup",
                    data=body,
                    headers={"content-type": "application/json"},
                )
                with urllib.request.urlopen(request, timeout=10) as response:
                    assert response.status == 201
                    session = json.load(response)
                request = urllib.request.Request(
                    base + "/auth/v1/token?grant_type=refresh_token",
                    data=json.dumps(
                        {"refresh_token": session["refresh_token"]}
                    ).encode(),
                    headers={"content-type": "application/json"},
                )
                with urllib.request.urlopen(request, timeout=10) as response:
                    assert json.load(response)["user"]["id"] == session["user"]["id"]
                print("Archive consumer signup, rotation and JWKS work on PostgreSQL")
            finally:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    raise RuntimeError("Consumer did not shut down within its budget")
            assert process.returncode == 0
