"""Immediate restore fencing must not substitute for post-boot evidence."""
import copy
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "bench"))
import native_pitr_reboot as reboot


class NativePitrBootControl(unittest.TestCase):
    def stopped(self):
        return {role: {"MainPID": 0, "ActiveState": "inactive"} for role in ["app", "postgres"]}

    def samples(self):
        return [{"units": self.stopped(), "authenticated_API_reopened": False} for _ in range(2)]

    def test_complete_stopped_window_passes_and_single_or_short_observation_cannot(self):
        self.assertTrue(reboot.verdict(self.samples(), 20))
        self.assertFalse(reboot.verdict(self.samples()[:1], 20))
        self.assertFalse(reboot.verdict(self.samples(), 0.1))
        self.assertFalse(reboot.verdict(self.samples(), 51))

    def test_running_failed_or_activating_service_after_boot_cannot_certify(self):
        for role in ["app", "postgres"]:
            for state, pid in [("active", 123), ("activating", 0), ("failed", 0)]:
                values = self.samples()
                values[1]["units"][role] = {"MainPID": pid, "ActiveState": state}
                with self.subTest(role=role, state=state):
                    self.assertFalse(reboot.verdict(values, 20))

    def test_authenticated_response_cannot_be_hidden_by_inactive_PID_snapshot(self):
        values = self.samples(); values[1]["authenticated_API_reopened"] = True
        self.assertFalse(reboot.verdict(values, 20))

    def test_missing_nonfinite_boolean_or_unbounded_observations_are_rejected(self):
        for values, elapsed in [([], 20), (None, 20), (self.samples() * 12, 20),
                                (self.samples(), float("nan")), (self.samples(), None), (self.samples(), True)]:
            with self.subTest(elapsed=elapsed), self.assertRaises(RuntimeError):
                reboot.verdict(values, elapsed)
        values = self.samples(); values[0]["units"]["app"]["MainPID"] = True
        with self.assertRaises(RuntimeError):
            reboot.verdict(values, 20)

    def test_resource_projection_requires_exact_roles_and_omits_private_fields(self):
        point = {"LoadState": "loaded", "MainPID": 0, "ActiveState": "inactive", "private": "never publish"}
        resources = {"units": {unit: copy.deepcopy(point) for unit in [reboot.UNIT, reboot.PG_UNIT]}}
        actual = reboot.snapshot(resources)
        self.assertEqual(actual, self.stopped())
        self.assertNotIn("private", json.dumps(actual))
        resources["units"][reboot.PG_UNIT]["LoadState"] = "masked"
        self.assertEqual(reboot.snapshot(resources), self.stopped())
        resources["units"].pop(reboot.PG_UNIT)
        with self.assertRaises(RuntimeError):
            reboot.snapshot(resources)

    def test_safety_guard_failure_is_never_swallowed_as_a_fenced_connection(self):
        guest = SimpleNamespace(assert_safe=Mock(side_effect=RuntimeError("critical guard")), action=Mock())
        api = SimpleNamespace(request=Mock())
        with self.assertRaisesRegex(RuntimeError, "critical guard"):
            reboot.observe(guest, api, object())
        guest.action.assert_not_called(); api.request.assert_not_called()


if __name__ == "__main__":
    unittest.main()
