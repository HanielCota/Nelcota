"""Safety boundaries for the separately dispatched disposable native VM."""

import json
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bench"))
import native_preflight
import native_vm


class NativeVMBoundaries(unittest.TestCase):
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

    def test_foreign_active_service_is_never_stopped(self):
        with patch("native_vm.unit_fields", return_value={"ActiveState": "active", "Description": "foreign"}):
            with patch("native_vm.command") as execute:
                with self.assertRaisesRegex(RuntimeError, "foreign"):
                    native_vm.stop_owned("native-vm-owned")
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
