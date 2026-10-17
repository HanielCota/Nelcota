"""Check boot-time fencing after a marker-only failed native restore."""
import math
import http.client
import time

from native_pitr_failure import PG_UNIT, UNIT, fenced
from native_guest import require

PROFILE = {"minimum_observation_seconds": 20, "maximum_observation_seconds": 50,
           "minimum_sample_gap_seconds": 1, "maximum_samples": 22,
           "maximum_guest_resource_request_seconds": 12,
           "maximum_authenticated_HTTP_seconds": 5,
           "scope": "Owned guest RAM loss after marker-only restore failure; no physical host, real PITR or corruption claim"}


def snapshot(resources):
    units = resources.get("units", {})
    result = {}
    for role, name in [("app", UNIT), ("postgres", PG_UNIT)]:
        value = units.get(name, {})
        require(value.get("LoadState") in {"loaded", "masked"} and type(value.get("MainPID")) is int
                and value["MainPID"] >= 0, "Missing post-boot native unit state")
        result[role] = {"MainPID": value["MainPID"], "ActiveState": value.get("ActiveState")}
    fenced(result)
    return result


def verdict(samples, elapsed):
    require(isinstance(samples, list) and 0 < len(samples) <= PROFILE["maximum_samples"]
            and type(elapsed) in {int, float} and math.isfinite(elapsed) and elapsed >= 0,
            "Missing bounded post-boot observation")
    for row in samples:
        require(isinstance(row, dict) and set(row) == {"units", "authenticated_API_reopened"}
                and type(row["authenticated_API_reopened"]) is bool, "Invalid post-boot observation")
        if (not fenced(row["units"]) or row["authenticated_API_reopened"]
                or any(unit["ActiveState"] != "inactive" for unit in row["units"].values())):
            return False
    return len(samples) >= 2 and PROFILE["minimum_observation_seconds"] <= elapsed <= PROFILE["maximum_observation_seconds"]


def observe(guest, api, token):
    began = time.monotonic()
    samples = []
    while time.monotonic() - began < PROFILE["minimum_observation_seconds"]:
        guest.assert_safe()
        require(len(samples) < PROFILE["maximum_samples"], "Post-boot sample ceiling reached")
        units = snapshot(guest.action("resources", timeout=PROFILE["maximum_guest_resource_request_seconds"]))
        reopened = False
        try:
            api.request("GET", "/rest/v1/native_probe?select=id", token=token)
            reopened = True
        except (OSError, RuntimeError, http.client.HTTPException):
            # A safety failure cannot be mistaken for successful fencing.
            guest.assert_safe()
        samples.append({"units": units, "authenticated_API_reopened": reopened})
        if not fenced(units) or reopened:
            break
        time.sleep(PROFILE["minimum_sample_gap_seconds"])
    elapsed = time.monotonic() - began
    return {"profile": PROFILE, "samples": samples, "elapsed_seconds": elapsed,
            "unknown_state_kept_stopped_after_boot": verdict(samples, elapsed)}
