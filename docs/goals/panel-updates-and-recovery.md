# G03: Controlled panel updates with recoverable deployment

Status: implementation integrated; local validation is recorded in the
[execution report](panel-updates-execution.md). Production enablement remains
blocked until the exact-binary VPS acceptance gates pass.

Planning baseline: `e8b03471124921921fe23443b133d009dfdac0cf` (PR #84).
The baseline passed 17 PR checks and 16 post-merge checks. Those results
validate the existing project, not the update feature proposed here.

## Objective and operating envelope

Let an authorized operator discover an official release, review its impact,
start an update and follow recovery through the interface. Preserve accepted
data, credentials, project isolation and an independent SSH recovery path.
Keep the Rust backend as one distributed binary, with separate process roles
where privileges and availability require them. Production needs no Node.

The first version updates one Nelcota project at a time on a Linux host using
the existing Docker or systemd runtime. Database engine, Caddy, OS, SDK and
controller upgrades have separate lifecycles. Checking a release does not
authorize installation. Scheduled or unattended installation is outside this
goal's first version. Downgrades, custom images and incomparable versions are
blocked in the panel unless a later reviewed recovery policy supports them.
The initial host budget allows one active update, with bounded admission and
an explicit busy result for conflicting requests. Discovery and monitoring
remain available. Increasing deployment concurrency requires new evidence.

Zero risk is not a completion criterion. Recovery must be demonstrated inside
a recorded operating envelope. Root compromise, loss of the host and its only
backup, inaccessible decryption keys and irreversible external effects cannot
be solved by a rollback button. Off-host recovery is a separate tested gate.

## Existing foundations and gaps

| Foundation | Work required before exposing updates |
|---|---|
| `crates/cli/src/upgrade.rs` owns prepare, maintenance, capture, activation, health and rollback. | Extract a reusable typed workflow without duplicating policy; add durable operation state and reconciliation. |
| Backups pair a database dump with checked disk-file versions. | Protect active snapshots from retention, validate configuration recovery and define supported S3/PITR restoration explicitly. |
| Maintenance survives a failed upgrade through a disk marker. | Coordinate the marker with the journal, CLI operations and proxy changes; reconcile both after restart. |
| Caddy currently returns 503 for the entire paused project site. | Keep a separately authenticated management interface reachable while the app is stopped. |
| Panel sessions are in memory and disappear on app restart. | Give the controller its own authentication lifecycle and restricted job monitoring; existing app cookies cannot be the only recovery credential. |
| systemd currently deploys the running CLI executable. | Select a verified candidate explicitly; an older controller must not accidentally redeploy itself as the target application. |
| The release workflow emits SHA-256 files and signed build provenance. | Verify provenance at installation and publish a signed readiness manifest only after all required artifacts and gates succeed. |
| CLI commands already provide an SSH recovery entry point. | Make them share deployment locks and recovery state with the controller. Preserve legacy installs until explicit enrollment. |

These are integration gaps and design constraints. They are not evidence that
all existing CLI operations are defective.

## Safety invariants

1. Read-only discovery cannot start, stop, install or restore anything.
2. Deployment authority is granted by host policy and scoped to a project.
   End-user JWTs, `service_role`, project database ownership and ordinary panel
   sessions do not independently grant host deployment privileges.
3. Every installation target has an immutable identity: version, source commit,
   architecture, size, checksum and verified binary/image digest. A mutable
   tag or a displayed version string alone is insufficient.
4. Download, verification and compatibility checks finish before maintenance.
   Required previous artifacts are cached before traffic is paused.
5. Maintenance is confirmed at the proxy before stopping the old app. Every
   known writer and background worker is quiesced before capturing the backup.
6. A complete, verified backup of all affected state is durable before activation.
   Incomplete captures never become restore candidates.
7. Only one mutating deployment operation can own a project. Locks also cover
   conflicting CLI actions and snapshot retention; shared proxy changes have
   their own host-wide serialization.
8. Closing a tab, disconnecting a stream or losing an HTTP response cannot
   abandon or repeat an accepted operation.
9. Restart recovery checks actual runtime, database, files and proxy state.
   Journal entries alone do not prove a side effect completed.
10. Public traffic and external-effect workers resume only after validation
    and a durable commit decision. A failed or uncertain recovery keeps the
    maintenance gate closed and preserves its evidence.
11. Rollback during an update restores the matching application and data
    snapshot. Reverting a completed update after later writes is a new recovery
    operation requiring a new backup and explicit data-loss/compatibility review.
12. Logs, progress, URLs and history never reveal credentials, tokens, signing
    keys, signed download URLs or raw command arguments.

## Ownership and architecture

The control-plane decision is recorded in D101 of [the decision log](../decisions.md). Its default design is
a management origin separate from project applications, reached from the panel
and served through the existing Caddy. The management page and its status API
remain available throughout app maintenance. Installation enrolls this origin,
TLS, operator credentials and project permissions explicitly.

Use separate native process roles: an unprivileged HTTP/UI controller and a
restricted host worker. The worker has no public network listener. A bounded
local IPC channel uses OS peer identity and independently enforced host policy.
Privileged decisions cannot rely only on the HTTP adapter's assertions.
The ADR must specify credential ownership, fresh authentication, revocation,
socket permissions and the threat of a compromised project or adapter.
OS peer UID alone does not identify a project: Docker applications can share
the same UID. Bind enrollment, transport access and requested project to
host-owned policy; never authorize a sibling project from a caller-supplied ID.

| Owner | Responsibility |
|---|---|
| Shared deployment capability, extracted when needed | Typed plans, state transitions, protocol contracts and invariants; no HTTP or arbitrary shell commands. |
| CLI runtime adapters | Existing project registry, Docker/systemd operations, proxy maintenance, backup and restoration. |
| Native controller and worker | Host policy, verified releases, durable jobs, bounded execution and recovery. |
| Admin HTTP adapters | Discovery and authorized handoff to management; no Docker socket, root execution or arbitrary host paths. |
| `crates/admin/ui/src/lib/features/updates/` | Version presentation, review, confirmation, progress, history and recovery guidance. |
| Server/binary assembly | Compose process roles and capability interfaces. |
| Release workflow | Publish compatible, verified artifacts and a final readiness manifest. |

Keep dependency direction explicit in the architecture ownership checker.
Avoid adding operational dependencies to auth-only consumers or a generic
shared module. Update contract generation, English/Portuguese catalogs and
versioned panel assets through their existing owners.

The first controller installation is an operator action through SSH using a
verified release. A running controller does not replace itself during an app
job. Targets requiring a newer controller/protocol are blocked with an upgrade
instruction; a separate tested controller-upgrade procedure requires no active
jobs and a readable previous journal format.

## Release and authorization contracts

- Publish a versioned manifest after both supported Linux architectures and
  required OCI images exist. Bind source revision, actual package version,
  platform digests, sizes, migration compatibility, minimum controller version
  and release channel to verified provenance from the approved repository and
  workflow identity. Partial release publication is not an installable update.
- Pin artifacts by digest. Reject changed assets under an already approved
  identity, drafts, prereleases on the stable channel, unknown schema versions,
  malformed SemVer and incompatible OS/architecture/runtime combinations.
- Use a maintained native provenance verifier and reviewed trust roots. Define
  certificate/identity verification, trust rotation, revocation and offline
  behavior. SHA-256 validates bytes but does not authenticate the publisher.
  Provenance also does not prove the source is safe: promotion requires the
  exact reviewed revision, green CI and staging evidence. A release/package
  version mismatch is rejected. Invalid host time and uncertain trust state
  block installation instead of bypassing expiration checks.
- Bound metadata size, artifact size, decompression, redirect count, network
  deadlines and retries. Use TLS verification and approved download destinations;
  user-supplied URLs cannot direct the worker to internal services or files.
- Cache discovery per host, coalesce concurrent checks and observe upstream rate
  limits. Never install from stale metadata whose trust checks have expired.
- Create a short-lived plan tied to the current project/runtime identity,
  configuration revision, target digest and backup policy. Revalidate immediately
  before acceptance and again before activation; configuration drift invalidates it.
- Require fresh operator authentication and host-granted deployment permission.
  Use explicit origin/CSRF protection, rate limits and single-use authorization
  tied to the plan. Existing project SSO keys cannot mint deployment authority.
- Persist an idempotency key and payload fingerprint before acknowledging a job.
  Repeating the same request returns the same job; changing its target under the
  same key is rejected. Job IDs are not authorization credentials.
- Monitoring credentials authorize only the bound job/project, have finite
  lifetimes and avoid URLs and browser localStorage. Reauthentication can renew
  monitoring without starting another update. Revocation blocks new operations;
  an already accepted safety-critical recovery continues under host policy.

## Durable workflow and recovery

Use explicit states such as `planned`, `preparing`, `prepared`, `pausing`,
`capturing`, `backup_verified`, `activating`, `validating`, `committing`,
`succeeded`, `recovering`, `recovered`, `failed_before_change` and
`needs_operator`. Define allowed transitions and required evidence for each.
Persist intent before side effects and record verified completion afterwards.
Use atomic replacement, file and directory synchronization, private ownership,
bounded records and a versioned journal schema. Filesystem crash behavior must
be tested; atomic rename alone is not proof of power-loss durability.

1. Resolve and verify artifacts, current/previous identities and compatibility.
2. Check disk/inode capacity, memory pressure, service access, backup policy,
   configuration drift and the ability to maintain an independent control route.
3. Accept and persist the job; acquire project ownership and serialize any
   shared registry/proxy mutation using a documented lock order.
4. Confirm maintenance at public ingress, drain old requests and stop known
   writers. Candidate startup uses a validation mode with background/external
   effects paused, including webhook dispatch and scheduled destructive work.
5. Capture the database, referenced file versions and required private
   configuration together. Verify completeness and integrity, preserve keys and
   credentials, and protect the snapshot from cron cleanup. Meet the configured
   encrypted off-host backup policy before activation.
6. Activate the pinned candidate using atomic/durable native replacement or an
   immutable OCI digest. Preserve the verified previous runtime and configuration.
7. Validate actual runtime identity, migration history, database access, role
   safety, storage availability and required HTTP behavior. Production probes
   are bounded and avoid irreversible notifications or mutations of user data.
8. Durably commit the accepted version, then reopen traffic and resume workers
   in a defined order. If reopening the proxy fails, retain/reestablish maintenance
   and report the unresolved state; HTTP availability alone is not success.
9. On pre-commit activation/validation failure, keep maintenance, restore the
   matching database/files/configuration and previous runtime, then validate it.
   Record the irreversible previous-version resume decision before restarting
   background effects; finish recovery after health/proxy verification. If that fails, preserve all
   artifacts, keep maintenance and require an explicit recovery procedure.

On startup, reconcile every nonterminal job before accepting a conflicting
operation. A deadline, expired lease or missing worker is not permission for a
second process to mutate the same project: terminate/reconcile the old owner
and its child processes before transferring ownership. Unknown or corrupted
state never triggers a guessed restore or automatic traffic reopening.

The durable commit decision is the rollback boundary. If it exists, a crash
before recording `succeeded` cannot justify restoring the pre-update snapshot:
traffic may already have accepted new writes. Reconcile the committed candidate
or enter incident recovery. If observed proxy state conflicts with the journal,
close maintenance and investigate instead of assuming no writes occurred.
Ordinary `up`, proxy reload and boot recovery must respect unresolved journal
state; they cannot reopen traffic based only on the shallow health endpoint.

Define the supported local filesystems and lock/durability assumptions. Unproven
network/shared filesystems are outside the first release's journal envelope.
Document recovery RPO/RTO from measured local and off-host restores, including
backup-key custody. A local backup protects update rollback; an off-host backup
is required by the production policy for loss of the whole disk/host.

All destructive steps have bounded child-process handling, controlled
environment variables and fixed executable/argument construction. Browser
input cannot choose shell text, executable paths, service names or restore
paths. Reject traversal, symlink escapes, non-finite numbers and out-of-range
limits at every boundary. Preserve current guard-clause conventions.
Use monotonic time for running deadlines and sequence numbers for event order;
wall-clock changes cannot extend execution or revive consumed approvals.
Keep secrets out of subprocess arguments and public errors as well as logs.

Cancellation is available before maintenance when stopping preparation is safe.
After mutation begins, the interface offers controlled recovery, not a generic
kill button. Losing the browser does not cancel the job.

## Data, storage and external effects

- Test the supported database migration path before promoting a release. Include
  legacy supported versions and both empty and populated databases. Released
  root migrations remain immutable; manual downgrades never imply automatic
  reversal of arbitrary SQL migrations.
- A host/project with uncoordinated direct database writers or another active
  application instance cannot claim a consistent snapshot. Either enroll and
  quiesce those writers or block the panel operation with a precise reason.
- Disk storage uses immutable file versions and a paired manifest. Verify bytes,
  ownership and reference consistency after restoration; cleanup cannot remove
  active/historical snapshot contents.
- S3 storage needs a demonstrated object-version/snapshot restoration contract,
  lifecycle-retention protection and tested permissions. Block file-affecting
  rollback where that contract is unavailable. A PostgreSQL dump alone does not
  restore overwritten or deleted remote object bytes.
- Document the supported PITR/WAL recovery path and retention interactions.
  Restore tests verify that the resumed cluster archives correctly; unexplained
  timeline or history mismatches require operator recovery.
- Preserve JWT signing keys, database credentials, administrator configuration,
  storage settings and migrations. Private configuration backup is encrypted
  where exported; journal/history contain references, not secret values.
- Verify the database dump as well as the file manifest; record its digest,
  completion and matching snapshot identity. Keep previous formats readable
  where supported, but do not certify an old/unpaired dump as a rollback bundle.
- Pause external-effect workers until commit. Messages, emails and webhooks
  already delivered cannot be undone by restoring the database. Preserve their
  event identities and documented retry semantics during recovery.
- A failed healthcheck after traffic has reopened uses a separate incident
  procedure. Do not silently restore an older snapshot over newly accepted data.

## Interface and operational behavior

The project panel shows current identity, release availability, verification
time and a trusted management entry point. Release descriptions are untrusted
content: sanitize rendering, restrict links and avoid remote HTML/images.
The management page provides plan review, backup requirements, expected
maintenance, authorization, live stages, success/recovery outcomes and history.
It remains accessible when the project app is stopped or its login resets.

Use sequenced resumable events or bounded polling with backoff, one subscription
per active view and bounded server buffers. Reconnecting or opening two tabs
shows the same job. Ignore stale events from other jobs/projects. Show stage
progress honestly; do not invent percentages or completion times.

Errors have stable codes, safe descriptions and an actionable next step. Keep
full private diagnostics separate from public progress. Record operator,
project, plan/target identity, state transitions, timestamps, backup references
and validation results in a bounded audit history. Retention never removes
evidence required by an active or unresolved operation.

## Delivery phases and exit gates

| Phase | Deliverable | Exit gate |
|---|---|---|
| 0. Baseline and design | Inventory, threat model, supported runtime/storage envelope, ADR, permissions and recovery invariants. | Every privileged action, known writer, backup dependency and trust root has an owner and a testable contract. |
| 1. Release readiness | Verified manifest, immutable identities, compatibility rules and cached discovery. | Corrupted, forged, incomplete, stale and incompatible releases are rejected without modifying a project. |
| 2. Deployment kernel | Reusable CLI workflow, journal, locks, reconciliation and validation mode. | CLI nominal behavior remains compatible; every interrupted phase ends in a verified state or closed maintenance. |
| 3. Host enrollment | Unprivileged management/controller interface, restricted worker, IPC and scoped authorization. | A project account/compromised project cannot obtain arbitrary host execution or authority over another project. |
| 4. Read-only UI | Availability, changelog, compatibility and agent status in both languages. | Repeated checks are bounded, sanitized and incapable of initiating deployment. |
| 5. Controlled update UI | Plan review, fresh authentication, idempotent start, independent progress and history. | Multiple clicks/tabs, disconnects and restarts preserve one operation and correct authorization. |
| 6. Recovery campaign | Docker/systemd interruption, storage, compatibility and backup-restore fixtures. | Recovery proofs cover actual data/file bytes, isolation, keys and maintenance ordering. |
| 7. Staging and resource limits | Real disposable VPS tests for Linux amd64/arm64, TLS/DNS, restart and resource stress. | Published artifacts and the final commit meet correctness and recorded operating budgets on the supported matrix. |
| 8. Opt-in production rollout | Versioned documentation, operator enrollment, recovery runbook and gradual enablement. | All release gates pass; local/remote backups and SSH recovery have been exercised. |

Each phase is a coherent reviewable change. Keep code and its regression tests
together; generated panel assets have a separate commit. Update the final PR
scope when a phase changes. Runtime installation remains disabled by default
until its gates pass. Older/unmanaged installations continue their current CLI
workflow; missing capabilities produce an explanation rather than a partial setup.

## Required failure and abuse matrix

| Scenario | Required result and evidence |
|---|---|
| Anonymous caller, end-user JWT, `service_role`, missing deployment role, revoked/expired operator session | No job creation and no host changes; audit without secrets. |
| Cross-origin request, replayed approval, wrong project/job, stale plan | Reject before mutation; isolation tests cover both shared and per-project panel login. |
| Shell/path/header injection, symlink swap, oversized metadata, malicious release Markdown/URLs | No arbitrary execution, traversal, SSRF or XSS; bounded resource use. |
| Changed checksum/digest, wrong provenance identity, malformed/partial release, mutable tag, unsupported architecture/protocol | No maintenance or activation; existing runtime stays usable. |
| Double-click, request retry after lost response, two tabs, two workers, concurrent CLI upgrade/restore/remove/migrate | One accepted operation; conflicting work receives a stable busy result. |
| Concurrent projects or shared Caddy reload | No registry/proxy lost update or effect on another project; global resource budgets hold. |
| Download failure, rate limit, TLS/DNS failure or interruption during preparation | Bounded retry/cleanup; no project downtime or changed data. |
| Low disk/inodes, disk filling mid-backup, quota, ENOSPC, OOM or permission failure | No unverified activation; preserve recoverable artifacts and explain the blocked phase. |
| Partial/corrupt backup, missing remote object version, inaccessible key or unavailable backup provider | Activation is blocked under the declared policy; never substitute an unrelated backup. |
| Failed migration, unhealthy candidate or runtime identity mismatch | Matching previous runtime and data restore before public traffic; no accepted pre-commit writes lost. |
| Crash/SIGKILL at every transition and side-effect boundary | Restart reconciliation cannot duplicate destructive steps or silently open maintenance. |
| Abrupt VM power loss during journal/config/binary/proxy writes | Durable recovery proof on supported filesystems; unknown state stays closed. |
| Journal corruption, unsupported journal/protocol schema, changed filesystem assumptions or wall-clock jumps | Reject unsafe enrollment/installation and reconcile existing work without guessed state or revived approvals. |
| Proxy reload failure, stale health response or successful DB check with broken auth/storage | No false success or premature reopening; required validation probes fail the job. |
| Failure during restore, missing previous image/binary or mixed schema/files | Closed maintenance, retained backups and actionable SSH recovery; no retry that erases evidence. |
| Worker starts dispatching webhook/email or another writer changes data during capture/validation | Test proves effects remain paused or enrollment/update is blocked. |
| Tab close, network loss, SSE reconnect, management reload or app-session reset | Job continues once, progress resumes with proper authorization and no secret-bearing URLs. |
| Retention races, long history, slow readers and repeated checking under load | Active backups/jobs remain protected; bounded memory, disk growth, event queues and upstream requests. |
| Revert requested after a completed update has accepted new writes | New recovery plan and backup; no silent rollback to a stale snapshot. |

## Validation and release evidence

- Fast tests cover transition rules, request validation, authorization,
  idempotency, journal parsing, IPC bounds, trust policy and release rendering.
  Real integration tests prove locks, fs/process behavior and runtime adapters.
- Use disposable PostgreSQL 17, real Docker/systemd fixtures, local disk and a
  controlled S3-compatible backend. A mock suite alone cannot certify recovery.
- Upgrade a populated legacy fixture and a current fixture. Compare exact
  numeric/text rows, schema/history, RLS boundaries, sessions/JWKS, file bytes
  and ownership before/after successful updates and failed activation.
- Browser tests cover English/Portuguese, keyboard use, multiple tabs, stale
  events, reconnection and progress while the app actually stops. Reuse real
  Chromium, Firefox and WebKit coverage where browser behavior differs.
- Measure request/event counts, memory, CPU, disk amplification and normal API
  latency with discovery/monitoring active. Compare at least three equivalent
  runs; retain the existing 10% regression envelope on an unchanged fixture.
  Run a bounded 30-minute staging soak with repeated checks and update/recovery
  cycles; archive fixture, hardware, versions, digests and sanitized outcomes.
- Run two execution/validation passes and two corrective review passes against
  the final implementation. The second pass includes hostile interruption and
  concurrent operations, not just a repeat of the successful path.
- Respect AGENTS.md: related local suites at low priority, bounded workers,
  and Rust/Vitest/audit runs one at a time. Expensive crash/soak campaigns run
  on isolated resource-limited hosts. Cleanup removes only owned fixtures.
- Required CI includes existing format, Clippy, architecture, Rust, S3, upgrade,
  panel, SDK, native lint and dependency gates plus the new relevant regressions.
  Investigate newly introduced high/critical findings; do not silence scanners
  or relax resource gates to obtain a green result.
- Record final source SHA, signed release identity, manifest/protocol versions,
  target digests, supported upgrade paths, backup/restore evidence, phase fault
  results, resource budgets, downtime measurements and explicit exclusions.

## Completion checklist

- [x] Architecture and threat model define every privilege and availability boundary (D101 and operating guide).
- [ ] Release discovery is read-only, bounded and verifies readiness/provenance.
- [ ] Shared CLI/controller workflow has durable ownership and crash reconciliation.
- [x] Host enrollment and scoped authorization remain independent of project sessions (including real IPC UID/scope tests).
- [ ] Candidate verification, backup and validation precede traffic reopening.
- [ ] Supported disk/S3/PITR recovery paths have actual restore evidence.
- [ ] External-effect workers and other writers obey the maintenance contract.
- [x] Progress survives app restarts, reconnection and duplicate requests in local controller/browser tests.
- [ ] Corrupt/unknown state and failed recovery retain closed maintenance and artifacts.
- [ ] Docker/systemd and supported CPU architectures pass the documented matrix.
- [ ] Resource/performance gates and both corrective review passes succeed.
- [ ] A disposable VPS validates published artifacts, TLS, power-loss recovery and SSH rescue.
- [ ] Documentation, translations, generated assets and final-commit CI are current.
- [x] Opt-in enrollment, controller upgrade and post-commit incident procedures are documented.

## Execution record template

For each phase record: source SHA, scope, changed ownership/contracts, threat
or failure reproduced, correction, checks and fixtures used, observed outcome,
runtime/resource limits and remaining blocked gates. Separate proposals from
implemented behavior and published releases from source-only validation.

References: [deployment](../deploy.md), [backup](../backup.md),
[architecture](../architecture.md), [decisions](../decisions.md),
[release handoff](../onboarding/release.md),
[operational quality](architecture-security-performance.md),
[contribution rules](../../CONTRIBUTING.md).
