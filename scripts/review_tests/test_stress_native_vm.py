"""Safety boundaries for the separately dispatched disposable native VM."""

import json
import hashlib
import base64
import gzip
import http.client
from pathlib import Path
import sys
import socket
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bench"))
import native_preflight
import native_vm
import native_evidence
import native_contracts
import native_guest


class NativeVMBoundaries(unittest.TestCase):
    def test_native_journal_categories_never_publish_log_payloads(self):
        value = native_guest.journal_categories("Failed at step EXEC: Permission denied private-token\npassword authentication failed postgres://private-user:private-password@localhost/db")
        self.assertEqual(value["exec_failure"], 1)
        self.assertEqual(value["permission_failure"], 1)
        self.assertEqual(value["database_authentication_failure"], 1)
        self.assertNotIn("private", str(value))

    def test_environment_and_symlink_diagnostics_publish_metadata_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "environment"
            path.write_bytes(b"private-password-and-token")
            path.chmod(0o600)
            value = native_guest.diagnostic_file(path, environment=True)
            self.assertEqual(value["size_bytes"], 26)
            self.assertEqual(value["mode"], 0o600)
            self.assertNotIn("sha256", value)
            self.assertNotIn("private", str(value))
            self.assertTrue(value["UTF8_valid"])
            self.assertFalse(value["contains_NUL"])
            self.assertTrue(value["content_shape_available"])
            link = Path(temporary) / "binary-link"
            link.symlink_to(path)
            linked = native_guest.diagnostic_file(link, binary=True)
            self.assertTrue(linked["symlink"])
            self.assertFalse(linked["regular"])
            self.assertNotIn("sha256", linked)

    def test_native_exec_status_is_retained_without_reading_a_process(self):
        result = SimpleNamespace(stdout=b"MainPID=0\nActiveState=failed\nExecMainCode=1\nExecMainStatus=203\nStatusErrno=13\n")
        with patch.object(native_guest, "command", return_value=result):
            value = native_guest.unit_state("nelcota-stress.service")
        self.assertEqual((value["ExecMainCode"], value["ExecMainStatus"], value["StatusErrno"]), (1, 203, 13))

    def test_configuration_fault_restores_owned_original_even_after_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "config"
            path.write_bytes(b"private fixture configuration")
            path.chmod(0o600)
            with self.assertRaisesRegex(ValueError, "fixture interruption"):
                with native_contracts.configuration_fault(path):
                    self.assertTrue(path.is_dir())
                    raise ValueError("fixture interruption")
            self.assertEqual(path.read_bytes(), b"private fixture configuration")
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)

    def test_preexisting_configuration_directory_is_never_removed_as_our_fault(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "foreign"
            path.mkdir()
            with self.assertRaisesRegex(RuntimeError, "preexisting"):
                with native_contracts.configuration_fault(path):
                    self.fail("must not inject into a foreign directory")
            self.assertTrue(path.is_dir())

    def test_migration_census_consumes_actual_psql_bytes(self):
        with patch("native_contracts.sql", side_effect=[b"t\n", b"t\n", b"1\n"]):
            self.assertEqual(native_contracts.migration_census(), {
                "fixture_table_created": True, "migration_journal_present": True, "applied_fixture_migrations": 1})

    def test_absent_migration_tables_do_not_invent_a_count(self):
        with patch("native_contracts.sql", side_effect=[b"f\n", b"f\n"]) as read:
            self.assertEqual(native_contracts.migration_census(), {
                "fixture_table_created": False, "migration_journal_present": False, "applied_fixture_migrations": None})
            self.assertEqual(read.call_count, 2)

    def test_invalid_sql_booleans_and_counts_fail_closed(self):
        for value in [b"", b"true", b"NaN", b"1"]:
            with self.subTest(value=value), patch("native_contracts.sql", return_value=value):
                with self.assertRaisesRegex(RuntimeError, "invalid SQL boolean"):
                    native_contracts.migration_census()
        for value in [b"", b"NaN", b"-1", b"1001", b"1" * 17]:
            with self.subTest(value=value), patch("native_contracts.sql", side_effect=[b"t", b"t", value]):
                with self.assertRaises(RuntimeError):
                    native_contracts.migration_census()

    def signup_response(self, status, account):
        reader, writer = socket.socketpair()
        content = json.dumps(account).encode()
        response = None
        try:
            writer.sendall(("HTTP/1.1 " + str(status) + " Fixture\r\nContent-Length: " + str(len(content)) + "\r\n\r\n").encode() + content)
            writer.shutdown(socket.SHUT_WR)
            response = http.client.HTTPResponse(reader)
            response.begin()
            connection = Mock()
            connection.getresponse.return_value = response
            api = native_vm.Api.__new__(native_vm.Api)
            api.guest = SimpleNamespace(https_port=8443, assert_safe=Mock())
            api.context = None
            api.attempts = api.received_bytes = api.sent_bytes = 0
            with patch("native_vm.Connection", return_value=connection):
                return native_vm.signup(api, "alice@example.invalid", "fixture-input")
        finally:
            if response is not None:
                response.close()
            reader.close()
            writer.close()

    def test_documented_201_signup_is_accepted(self):
        account = {"access_token": "fixture-token", "user": {"email": "alice@example.invalid"}}
        self.assertEqual(self.signup_response(201, account), "fixture-token")

    def test_200_cannot_substitute_for_successful_creation(self):
        with self.assertRaisesRegex(RuntimeError, "status 200"):
            self.signup_response(200, {"access_token": "fixture-token", "user": {"email": "alice@example.invalid"}})

    def test_signup_requires_matching_identity_and_a_token(self):
        for account in [{"user": {"email": "alice@example.invalid"}},
                        {"access_token": "fixture-token", "user": {"email": "bob@example.invalid"}}]:
            with self.subTest(account=account):
                with self.assertRaises(RuntimeError):
                    self.signup_response(201, account)

    def test_large_report_survives_annotation_limit_without_loss(self):
        report = {"identity": "native-vm-owned", "source": "1" * 40, "certifies_g05": False,
                  "status": "failed", "cleanup_confirmed": True,
                  "checks": ['quotes" percent% newline\n unicode\u00e7 backslash\\' * 300]}
        notices = native_evidence.notices(report)
        fragments = []
        for line in notices:
            self.assertLessEqual(len(line.encode()), 4000)
            decoded = line.split("::", 2)[2].replace("%0A", "\n").replace("%0D", "\r").replace("%25", "%")
            value = json.loads(decoded)
            if "payload" in value:
                fragments.append(value)
        self.assertEqual(len(fragments), fragments[0]["parts"])
        content = gzip.decompress(base64.b64decode("".join(value["payload"] for value in fragments), validate=True)).decode()
        self.assertEqual(hashlib.sha256(content.encode()).hexdigest(), fragments[0]["sha256"])
        self.assertEqual(json.loads(content), report)
        self.assertLessEqual(len(notices), 10)

    def test_multiple_fragments_stay_within_ten_annotations_per_step(self):
        report = {"identity": "native-vm-owned", "source": "1" * 40, "certifies_g05": False,
                  "status": "failed", "cleanup_confirmed": True,
                  "checks": [hashlib.sha256(str(i).encode()).hexdigest() for i in range(200)]}
        notices = native_evidence.notices(report)
        self.assertGreater(len(notices), 2)
        self.assertLessEqual(len(notices), 10)
        report["checks"] = [hashlib.sha256(str(i).encode()).hexdigest() for i in range(800)]
        with self.assertRaisesRegex(ValueError, "annotation count"):
            native_evidence.notices(report)

    def test_unknown_fields_and_nonfinite_values_are_never_published(self):
        report = {"identity": "native-vm-owned", "source": "1" * 40, "certifies_g05": False,
                  "status": "failed", "cleanup_confirmed": True}
        for fields in [{"private_log": "must never publish"}, {"elapsed_seconds_including_preparation": float("nan")}]:
            with self.subTest(fields=fields):
                with self.assertRaises(ValueError):
                    native_evidence.notices({**report, **fields})

    def test_self_hosted_inventory_is_rejected_before_reading_capacity(self):
        with patch.dict("os.environ", {"GITHUB_ACTIONS": "true", "RUNNER_ENVIRONMENT": "self-hosted"}, clear=True):
            with patch("native_preflight.os.statvfs") as filesystem:
                with self.assertRaisesRegex(RuntimeError, "unknown or self-hosted"):
                    native_preflight.inventory()
                filesystem.assert_not_called()

    def test_service_namespace_rejects_foreign_names_and_command_fragments(self):
        for identity in ["postgresql", "../native-vm-owned", "native-vm-", "native-vm-a\n", "native-vm-a;stop"]:
            with self.subTest(identity=identity):
                with self.assertRaises(RuntimeError):
                    native_vm.unit(identity)

    def test_foreign_service_is_never_stopped_or_reset_in_any_state(self):
        for state in ["active", "inactive", "failed"]:
            with self.subTest(state=state), tempfile.TemporaryDirectory() as temporary:
                with patch("native_vm.unit_fields", return_value={"ActiveState": state, "LoadState": "loaded", "Description": "foreign"}):
                    with patch("native_vm.command") as execute:
                        with self.assertRaisesRegex(RuntimeError, "foreign"):
                            native_vm.stop_owned("native-vm-owned")
                        guest = native_vm.Guest("native-vm-owned", Path(temporary))
                        with self.assertRaisesRegex(RuntimeError, "foreign"):
                            guest.start()
                        execute.assert_not_called()

    def test_power_cut_requires_owned_description_and_waits_for_actual_stop(self):
        states = [{"ActiveState": "active", "Description": "native-vm-owned", "MainPID": "99"},
                  {"ActiveState": "failed", "Description": "native-vm-owned", "MainPID": "0"}]
        with patch("native_vm.unit_fields", side_effect=states):
            with patch("native_vm.command") as execute:
                result = native_vm.stop_owned("native-vm-owned", power_cut=True)
                self.assertEqual(result["MainPID"], "0")
                execute.assert_called_once_with(["systemctl", "kill", "--signal=SIGKILL", "--kill-whom=all", "native-vm-owned.service"])

    def test_expired_sampler_stops_vm_and_controller_with_preserved_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "public").mkdir()
            owner = {"identity": "native-vm-owned", "output": str(root), "pid": 1234,
                     "process_identity": "9876", "started_monotonic": time.monotonic()}
            (root / "ownership.json").write_text(json.dumps(owner))
            (root / "heartbeat.json").write_text(json.dumps({"monotonic": time.monotonic() - 31}))
            with patch("native_vm.process_identity", return_value="9876"):
                with patch("native_vm.stop_owned", return_value={"ActiveState": "inactive", "MainPID": "0"}) as stop_vm:
                    with patch("native_vm.stop_owner") as stop_controller:
                        native_vm.watchdog(root)
                        stop_vm.assert_called_once_with("native-vm-owned")
                        stop_controller.assert_called_once_with(owner)
            receipt = json.loads((root / "public/emergency-stop.json").read_text())
            self.assertEqual(receipt["reason"], "resource_observation_gap")

    def test_independent_emergency_receipt_prevents_restarting_load(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "public").mkdir()
            (root / "public/emergency-stop.json").write_text("{}")
            guest = native_vm.Guest("native-vm-owned", root)
            with patch("native_vm.command") as execute:
                with self.assertRaisesRegex(RuntimeError, "Independent"):
                    guest.start()
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
