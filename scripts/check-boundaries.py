"""Check capability ownership and exact packaged migration assets (Python 3.11+)."""

from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    "core": set(),
    "deployment": set(),
    "auth": {"core"},
    "api": {"core", "auth"},
    "storage": {"core", "auth"},
    "webhooks": {"core"},
    "admin": {"deployment", "core", "auth", "api", "storage", "webhooks"},
    "cli": {"core", "auth", "api", "deployment"},
    "server": {"core", "auth", "api", "storage", "webhooks", "admin", "cli"},
}


def check():
    errors = []
    workspace = (
        tomllib.loads((ROOT / "Cargo.toml").read_text())
        .get("workspace", {})
        .get("dependencies", {})
    )
    declared = {path.parent.name for path in (ROOT / "crates").glob("*/Cargo.toml")}
    if declared != set(EXPECTED):
        errors.append("Capability manifests must have an explicit ownership contract")
    for capability, expected in EXPECTED.items():
        manifest = tomllib.loads(
            (ROOT / "crates" / capability / "Cargo.toml").read_text()
        )
        sections = [manifest.get("dependencies", {})]
        sections.extend(
            target.get("dependencies", {})
            for target in manifest.get("target", {}).values()
        )
        actual = set()
        for section in sections:
            for name, specification in section.items():
                if isinstance(specification, dict) and specification.get("workspace"):
                    specification = workspace.get(name, specification)
                package = name
                if isinstance(specification, dict):
                    package = specification.get("package", name)
                if package.startswith("nelcota-"):
                    actual.add(package.removeprefix("nelcota-"))
        if actual != expected:
            errors.append(
                f"{capability}: expected {sorted(expected)}, found {sorted(actual)}"
            )
    canonical = {
        path.name: path.read_bytes() for path in (ROOT / "migrations").glob("*.sql")
    }
    packaged = {
        path.name: path.read_bytes()
        for path in (ROOT / "crates/core/migrations").glob("*.sql")
    }
    if canonical != packaged:
        errors.append(
            "Packaged migrations differ from canonical migrations; copy new SQL without editing released files"
        )
    if errors:
        raise SystemExit("\n".join(errors))
    print("Capability dependencies and packaged migration bytes match their contracts")


if __name__ == "__main__":
    check()
