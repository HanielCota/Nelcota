# G03 execution record

Date: 2026-10-13. Baseline: `e8b03471124921921fe23443b133d009dfdac0cf`.
Implementation branch: `feat/controlled-panel-updates`. This record describes
local implementation evidence; merge status and final-commit CI are tracked in
the associated pull request. It does not establish a published release or enabled
production host. See [the operating guide](../panel-updates.md).

## Delivered implementation

- `nelcota-deployment` owns bounded release/protocol types, private durable
  receipts, host leases, idempotency, monotonic sequences and crash reconciliation.
- Native CLI roles enroll the host, serve management HTTP, verify official
  artifacts, execute jobs and recover them through SSH. Policy and mutable state
  must be root-owned; operator scope binds to project incarnation identities.
- The unprivileged controller and restricted IPC worker use distinct process
  roles and a management origin independent of project maintenance. Project/SSO/
  service-role authentication cannot authorize updates. Start and recovery
  require fresh operator-password confirmation.
- Runtime adapters preserve exact previous binary/image/configuration identities,
  paired database/file snapshots and checked offsite readback. Candidate routes
  and external-effect workers stay gated until a durable commit decision. Both
  new-version commit and restarting a restored version are irreversible data
  boundaries; neither permits a later automatic stale-snapshot restore.
- The panel provides operator login, read-only project discovery, release check,
  plans/blockers, confirmation, sequence-based progress, history, reconnect and
  controlled recovery in Portuguese and English. The rebuilt assets are embedded.
- Release publication adds signed readiness and downloadable attestation bundles
  only after both binaries, the OCI image and exact-source main CI succeed.
  The first controller release must be 0.5.0 or later; workspace release version
  and release publication remain separate from this implementation.

## Two corrective reviews

The first review covered authority, repeated requests, concurrency, state and
recovery. Corrections include rechecking policy/session after asynchronous
reauthentication/preflight, separating project incarnation from its name,
retaining accepted jobs omitted by a stale poll, prioritizing incomplete jobs
when wall time moves backwards, reading the authoritative receipt after failed
persistence, and forbidding rollback after either irreversible resume decision.

The second review covered filesystem trust, runtime identity and activation.
Corrections include root ownership of the complete deployment-state inventory,
canonical compose/native-unit enrollment, bounded probes and measured disk
reserve, confirmation that old database connections are gone, exact running
native PID/image identity, publicly readable proxy/registry files despite the
worker umask, native gates outside private host metadata, candidate-specific gate
proof and application-route readiness before reopening the proxy. Privileged
backup reads/restores now use directory handles and refuse bucket/object symlink
traversal; restore no longer recursively chowns application-controlled paths.
Corrupt deployment state is reported as unavailable rather than a login failure.

## Local evidence

The targeted checks executed on Linux amd64 with Rust 1.99.0, Postgres 17,
Chromium and the repository's low-priority/resource limits. Detailed local logs
are retained under `.tools/reports/updates/`; they are disposable evidence, not
production acceptance reports.

| Check | Result and practical limit |
|---|---|
| Deployment domain | 13 tests passed, including 16 concurrent callers, idempotency, incomplete-stage reconciliation, corrupt/missing receipts, persistence failure, both irreversible boundaries and backwards wall time |
| CLI library | 62 tests passed before the additional real-IPC test; the final updates subset passed 9 tests including that test |
| Privileged worker transport | Actual root worker in an isolated, network-disabled Docker container passed; another UID with socket-group access was rejected, the approved UID authenticated, project scope held and installation stayed blocked without validation evidence |
| Backup boundary | 4 tests passed, including real Postgres historical-file restore/corruption/retention and source/destination symlink escape rejection |
| HTTP controller | 3 tests passed: origin/Host/CSRF, project/bearer/duplicate-cookie/injected-input rejection and secure cookie without token disclosure |
| Application traffic gate | 2 tests passed: direct API/session/WebSocket traffic blocked before commit and atomic readiness cannot bypass missing candidate proof |
| Configuration | 12 tests passed, including missing/invalid deployment identity and relative gate rejection |
| Frontend capability parsing | 4 tests passed: null/NaN/unsafe/unknown-state rejection, stale progress, missing accepted job and canonical controller origins |
| Browser flows | 3 Chromium tests passed: separate operator login, lost start response without automatic mutation retry, same-key explicit retry/reload and committed incident recovery |
| Native controller smoke | Built Rust binary served embedded management HTML/assets and IPC status; session isolation, CSRF and security headers passed; the IPC status backend in this smoke was a fixture |
| Build/type/lint | Rust build and Clippy with denied warnings; Svelte/TypeScript checks; frontend production build; boundary/migration-byte check; Oxlint base preset with zero findings |
| Release construction | 2 Python tests passed with synthetic CI/artifact fixtures; exact-source/latest-main gating, hashes, both architectures, image digest, migration identity and malformed/partial receipt rejection |

The initial backup regression could not initialize Postgres because the local
workspace filesystem was full. Diagnostics showed `No space left on device`.
Only ignored compilation caches/examples were removed; the regression passed on
retry. Incremental compilation is disabled in the local cloud environment helper
to prevent the same growth. The pre-existing shared test Postgres was preserved;
new disposable fixtures were removed. Frontend build retains the pre-existing
large reference-document chunk warning; it does not prevent the build.

## Gates that remain closed

These local results do not exercise the entire privileged deployment workflow
against newly published, attested 0.5.0+ candidates. No real VPS was supplied or
enrolled, no production policy was enabled and no public release was created.
The associated pull request tracks final-commit CI separately from these local
results.

Actual Docker and systemd upgrades/rollback, aarch64 validation, abrupt VM power
loss, real release provenance/downloads, independent-origin TLS/SSH rescue and a
full off-host database/files/configuration restore remain acceptance gates. V1
rejects project S3 storage, PITR, custom images/configuration/database/storage
layouts and unsupported enrollment assumptions. The application/controller
upgrade paths remain separate. Never mark an unrun gate as passed or disable
verification to make a host eligible.

Run the matrix with the exact installed controller and published candidate,
record binary/source/release identity and restore RPO/RTO, retain diagnostics and
create the root-owned validation report only from that evidence. Installation
has no HTTP bypass and remains disabled until all required report checks pass.
