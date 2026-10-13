# Project goals

Both goals are implemented and validated through two execution passes and two
corrective reviews. See the [execution report](execution-report.md) and its
versioned measurements, compatibility checks and operating limits.

| Goal | Outcome |
|---|---|
| [G01: Reusable authentication](auth-reusability.md) | An independent Rust consumer integrates Nelcota authentication through a stable public API and a documented database lifecycle. |
| [G02: Architecture and operational quality](architecture-security-performance.md) | Clear module ownership, tested security boundaries and reproducible performance, load and stress evidence across the project. |

G02 established the inventory and retained release baseline; G01's archive
consumer proves the auth boundary. The ownership map and regression gates keep
the resulting capability interfaces and immutable migration assets reviewable.

Both goals preserve the [architecture](../architecture.md) and existing
[decisions](../decisions.md): PostgreSQL owns authorization through RLS, the
backend ships as one Rust binary and needs no Node runtime. The existing
frontend/SDK toolchain remains available for builds and tests.
Use [AGENTS.md](../../AGENTS.md) and [CONTRIBUTING.md](../../CONTRIBUTING.md)
for execution limits, migration rules and contribution practices.

Record the starting commit, deliverables, validation evidence and limits when
extending these completed goals. Keep findings distinct from
proposals; each confirmed defect needs a reproducer and an appropriate fix.
Publishing packages or opening another repository is a separate release step
from proving reuse in a consumer outside the workspace.
