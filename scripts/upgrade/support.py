"""Bounded local receiver and checksum-pinned release downloads."""

import hashlib
import hmac
import json
import socket
import threading
import time
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def ident(value):
    return '"' + value.replace('"', '""') + '"'


def quote(value):
    return "'" + str(value).replace("'", "''") + "'"


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def release_binary(config, release, cache):
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / (config["asset"] + "-" + release["version"])
    if not path.exists():
        url = (
            "https://github.com/"
            + config["repository"]
            + "/releases/download/"
            + release["tag"]
            + "/"
            + config["asset"]
        )
        temporary = path.with_name(path.name + ".download-" + uuid.uuid4().hex)
        try:
            with (
                urllib.request.urlopen(url, timeout=30) as response,
                temporary.open("wb") as target,
            ):
                size = 0
                deadline = time.monotonic() + 120
                while chunk := response.read(65536):
                    size += len(chunk)
                    if size > 128 * 1024 * 1024 or time.monotonic() > deadline:
                        raise RuntimeError("Release download exceeded its budget")
                    target.write(chunk)
            if digest(temporary) != release["sha256"]:
                raise RuntimeError("Published binary checksum mismatch: " + release["tag"])
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    if digest(path) != release["sha256"]:
        raise RuntimeError("Cached binary checksum mismatch: " + str(path))
    path.chmod(0o755)
    return path.resolve()


class Receiver(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), ReceiverHandler)
        self.secret = None
        self.status = 204
        self.requests = []
        self.errors = []
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.shutdown()
        self.server_close()
        self.thread.join(timeout=5)


class ReceiverHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        self.connection.settimeout(5)
        try:
            length = int(self.headers.get("Content-Length", "-1"))
            assert 0 <= length <= 262144, "Unbounded webhook body"
            body = self.rfile.read(length)
            assert len(body) == length, "Incomplete webhook body"
            event = self.headers["x-nelcota-event-id"]
            timestamp = self.headers["x-nelcota-timestamp"]
            expected = (
                "v1="
                + hmac.new(
                    self.server.secret.encode(),
                    (event + "." + timestamp + ".").encode() + body,
                    hashlib.sha256,
                ).hexdigest()
            )
            assert hmac.compare_digest(expected, self.headers["x-nelcota-signature"])
            assert abs(time.time() - int(timestamp)) < 300
            payload = json.loads(body)
            assert payload["id"] == event
            with self.server.lock:
                self.server.requests.append(
                    {
                        "event_id": event,
                        "delivery_id": self.headers["x-nelcota-delivery-id"],
                        "payload": payload,
                        "raw_hash": hashlib.sha256(body).hexdigest(),
                    }
                )
                status = self.server.status
        except (AssertionError, OSError, TypeError, ValueError, KeyError) as error:
            with self.server.lock:
                self.server.errors.append(repr(error))
            status = 400
        self.send_response(status)
        self.send_header("Content-Length", "0")
        self.end_headers()
