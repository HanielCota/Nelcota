"""Runtime diagnostics cannot expand traffic, expose configuration or hide cleanup failure."""
import base64
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stdout
import gzip
import io
import json
from pathlib import Path
import secrets
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bench"))
import provider_runtime as runtime


class RuntimeBoundaries(unittest.TestCase):
    def client(self, stopped=None):
        if stopped is None:
            stopped = threading.Event()
        return runtime.Client("http://127.0.0.1:19000", secrets.token_hex(16), secrets.token_hex(32),
                              "owned-fixture", stopped)

    def test_profiles_change_only_one_worker_setting_and_reject_unknown_cases(self):
        self.assertEqual(runtime.overrides("default"), {})
        self.assertEqual(runtime.overrides("runtime-workers"), {"RUSTFS_RUNTIME_WORKER_THREADS": "4"})
        self.assertEqual(runtime.overrides("fsync-workers"), {"RUSTFS_RUNTIME_FSYNC_BLOCKING_THREADS": "4"})
        changed = runtime.overrides("runtime-workers")
        changed["RUSTFS_RUNTIME_WORKER_THREADS"] = "1000"
        self.assertEqual(runtime.overrides("runtime-workers")["RUSTFS_RUNTIME_WORKER_THREADS"], "4")
        with self.assertRaisesRegex(RuntimeError, "Unknown bounded"):
            runtime.overrides("unlimited")

    def test_concurrent_requests_cannot_overrun_the_shared_budget(self):
        client = self.client()
        client.attempted = runtime.PROFILE["maximum_signed_requests_per_case"] - 1
        def attempt(_):
            try:
                return client.send({}) == b"ok"
            except RuntimeError:
                return False
        with patch.object(runtime.S3Inventory, "send", return_value=b"ok") as send:
            with ThreadPoolExecutor(max_workers=4) as workers:
                results = list(workers.map(attempt, range(16)))
        self.assertEqual(sum(results), 1)
        self.assertEqual(send.call_count, 1)
        self.assertEqual(client.completed, 1)
        self.assertEqual(client.attempted, runtime.PROFILE["maximum_signed_requests_per_case"])

    def test_guard_stops_new_traffic_and_http_errors_release_the_response(self):
        stopped = threading.Event()
        client = self.client(stopped)
        stopped.set()
        with patch.object(runtime.S3Inventory, "send") as send:
            with self.assertRaisesRegex(RuntimeError, "stopped by a guard"):
                client.send({})
            send.assert_not_called()
        stopped.clear()
        body = io.BytesIO(b"private diagnostic body")
        error = urllib.error.HTTPError("http://127.0.0.1:19000", 403, "denied", {}, body)
        with patch.object(runtime.S3Inventory, "send", side_effect=error):
            with self.assertRaises(urllib.error.HTTPError):
                client.send({})
        self.assertTrue(body.closed)

    def test_environment_projection_retains_no_credentials_or_nonfinite_values(self):
        value = runtime.runtime_environment(["RUSTFS_SECRET_KEY=" + secrets.token_hex(32),
            "RUSTFS_RUNTIME_WORKER_THREADS=4", "RUSTFS_RUNTIME_FSYNC_BLOCKING_THREADS="])
        self.assertEqual(value, {"RUSTFS_RUNTIME_WORKER_THREADS": 4, "RUSTFS_RUNTIME_FSYNC_BLOCKING_THREADS": None})
        for invalid in ["NaN", "inf", "-1", "1025", "true"]:
            with self.subTest(invalid=invalid):
                with self.assertRaisesRegex(RuntimeError, "Invalid runtime"):
                    runtime.runtime_environment(["RUSTFS_RUNTIME_WORKER_THREADS=" + invalid])

    def test_wrong_object_bytes_stop_other_workers_before_new_mutations(self):
        stopped = threading.Event()
        client = self.client(stopped)
        with patch.object(client, "send", side_effect=[b"", b"different"]) as send:
            with self.assertRaisesRegex(RuntimeError, "different object bytes"):
                runtime.transfer(client, b"expected", stopped)
        self.assertTrue(stopped.is_set())
        self.assertEqual(send.call_count, 2)
        with patch.object(runtime.S3Inventory, "send") as transport:
            with self.assertRaisesRegex(RuntimeError, "stopped by a guard"):
                client.send({}, method="PUT", key="bench/" + secrets.token_hex(16))
            transport.assert_not_called()

    def test_objects_with_the_same_seed_have_distinct_verified_bytes(self):
        class Objects:
            def __init__(self):
                self.objects, self.published = {}, []
            def send(self, parameters, method="GET", key=None, body=None):
                if method == "PUT":
                    self.objects[key] = body
                    self.published.append(body)
                    return b""
                if method == "GET":
                    return self.objects[key]
                if method == "DELETE":
                    del self.objects[key]
                    return b""
                raise AssertionError("Unexpected fixture method")
        objects, stopped = Objects(), threading.Event()
        for _ in range(2):
            self.assertTrue(runtime.transfer(objects, b"same fixture seed", stopped))
        self.assertFalse(stopped.is_set())
        self.assertEqual(len(set(objects.published)), 2)
        self.assertTrue(all(len(body) == runtime.PROFILE["payload_bytes"] for body in objects.published))
        self.assertEqual(objects.objects, {})

    def test_cleanup_failure_preserves_a_failed_receipt_and_releases_the_heartbeat(self):
        class Watcher:
            def poll(self):
                return None
            def wait(self, timeout):
                return 0
        before = {thread.ident for thread in threading.enumerate()}
        with tempfile.TemporaryDirectory() as root:
            with patch.object(runtime.subprocess, "Popen", return_value=Watcher()), \
                    patch.object(runtime, "host_reserves", return_value="host_disk_reserve"), \
                    patch.object(runtime, "cleanup", side_effect=RuntimeError("must stay private")):
                result = runtime.case("default", "owned-runtime-fixture", Path(root), 1, time.monotonic() + 60, 0)
            self.assertEqual(result["status"], "failed")
            self.assertFalse(result["cleanup_confirmed"])
            self.assertIn("fixture_cleanup_RuntimeError", result["cleanup_errors"])
            self.assertNotIn("must stay private", json.dumps(result))
            self.assertTrue((Path(root) / "default/result.json").is_file())
        self.assertEqual({thread.ident for thread in threading.enumerate()}, before)


class RuntimePublication(unittest.TestCase):
    def report(self):
        return {"identity": "native-provider-runtime-123-1", "source": secrets.token_hex(20),
                "status": "failed", "certifies_g05": False, "cleanup_confirmed": False, "cases": []}

    def test_unknown_configuration_and_incomplete_passing_cases_cannot_be_published(self):
        report = self.report()
        with self.assertRaisesRegex(RuntimeError, "Unknown provider runtime public"):
            runtime.public_payload({**report, "private_environment": "must stay private"})
        with self.assertRaisesRegex(RuntimeError, "Unknown provider runtime case"):
            runtime.public_payload({**report, "cases": [{"name": "default", "private_environment": "must stay private"}]})
        with self.assertRaisesRegex(RuntimeError, "lacks all independent"):
            runtime.public_payload({**report, "status": "passed", "cleanup_confirmed": True})

    def test_failed_public_report_remains_readable_without_losing_its_verdict(self):
        report = self.report()
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / "public"
            with redirect_stdout(output):
                runtime.publish(directory, report)
            path = directory / "provider-runtime.json"
            self.assertEqual(path.stat().st_mode & 0o777, 0o644)
            self.assertEqual(json.loads(path.read_text()), report)
        notices = [json.loads(line.split("::", 2)[2]) for line in output.getvalue().splitlines()
                   if line.startswith("::notice title=Provider runtime evidence::")]
        raw = gzip.decompress(base64.b64decode("".join(value["payload"] for value in notices)))
        self.assertEqual(json.loads(raw), report)


if __name__ == "__main__":
    unittest.main()
