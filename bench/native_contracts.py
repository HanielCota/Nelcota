"""Exercise documented native cwd and machine ownership only inside the guest."""

import hashlib
from pathlib import Path
import subprocess
import time

from native_guest import ROOT, PROJECT, UNIT, cli, command, install_limits, prepare, require, sql, unit_state, versions


def environment_is_absolute():
    path = Path("/etc/systemd/system") / UNIT
    require(path.is_file() and path.stat().st_size <= 65536, "Missing bounded native unit")
    values = [line.removeprefix("EnvironmentFile=") for line in path.read_text().splitlines()
              if line.startswith("EnvironmentFile=")]
    require(len(values) == 1, "Native unit has ambiguous environment files")
    return Path(values[0].strip('"')).is_absolute()


def default_root():
    require(not ROOT.exists(), "Default-root probe requires a fresh guest root")
    install_limits()
    ROOT.mkdir(mode=0o700)
    initial = command(["env", "-u", "NELCOTA_ROOT", "/usr/local/bin/nelcota", "init", "--local", "--project", PROJECT,
                       "--runtime", "systemd", "--profile", "1gb", "--yes"], cwd=ROOT, timeout=650, check=False)
    result = {"case": "documented init from the application directory without -C or NELCOTA_ROOT",
              "init_exit_code": initial.returncode, "environment_file_absolute": environment_is_absolute(),
              "versions": versions()}
    verify = command(["systemd-analyze", "verify", "/etc/systemd/system/" + UNIT], check=False, timeout=30)
    result["systemd_verify_exit_code"] = verify.returncode
    try:
        up = cli("up", timeout=30, check=False)
        result["up_exit_code"] = up.returncode
        result["up_timed_out"] = False
    except subprocess.TimeoutExpired:
        result["up_exit_code"] = None
        result["up_timed_out"] = True
    time.sleep(3)
    result["app_unit"] = unit_state(UNIT)
    return result


def host_root():
    first = prepare()
    other = Path("/opt/g05-native-other")
    require(not other.exists(), "Second host probe requires an unused owned root")
    sql("CREATE TABLE public.g05_host_sentinel (id integer PRIMARY KEY); INSERT INTO public.g05_host_sentinel VALUES (1);\n")
    # Credential verifiers stay only in private controller memory. Only whether
    # they changed is published, never their bytes or a derivative fingerprint.
    before = sql("SELECT rolpassword FROM pg_authid WHERE rolname='postgres';\n")
    caddy = Path("/etc/caddy/Caddyfile")
    before_caddy = hashlib.sha256(caddy.read_bytes()).digest()
    second = command(["/usr/local/bin/nelcota", "-C", str(other), "init", "--local", "--project", "other",
                      "--runtime", "systemd", "--profile", "1gb", "--skip-checks", "--yes"], timeout=650, check=False)
    after = sql("SELECT rolpassword FROM pg_authid WHERE rolname='postgres';\n")
    count = int(sql("SELECT count(*) FROM public.g05_host_sentinel;\n"))
    return {"case": "second native init in another owned directory with network checks skipped",
            "first_native_init": first,
            "second_init_exit_code": second.returncode,
            "first_database_password_changed": before != after,
            "first_caddy_configuration_changed": before_caddy != hashlib.sha256(caddy.read_bytes()).digest(),
            "first_sentinel_rows": count,
            "second_host_created": (other / "nelcota-host.json").is_file(),
            "second_unit_created": Path("/etc/systemd/system/nelcota-other.service").exists(),
            "first_app_after": unit_state(UNIT),
            "scope": "Both roots, the database, credentials and sentinels are generated disposable VM fixtures"}


def run(case):
    if case == "default-root":
        return default_root()
    require(case == "host-root", "Unknown native installation contract")
    return host_root()
