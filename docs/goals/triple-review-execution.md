# G04 execution record

Baseline: `bb9e6ef986687e505af5c8e809a77a53c380ebc9`, PR #85.
The baseline CI has 18 successful checks, including real PostgreSQL/S3,
upgrades, panel browser tests, SDK runtimes and 100 refresh-contention rounds
each on Chromium, Firefox and WebKit. This is baseline evidence, not validation
of changes made during G04.

Local tools: Rust 1.99.0, Node 24.19.0, Python 3.12.14. Reviews and tests run
serially at low priority where required by AGENTS.md.

## Pass 1: Trust and correctness

Reviewed signature/role verification, OAuth redirect and single-use flow
boundaries, request role/claim isolation, SQL parameter/identifier generation,
query/collection limits, storage names/grants/inert response headers, webhook
connected-address validation, panel CSRF/rendering, SDK session persistence,
root-owned deployment policy, bounded IPC, release provenance, backup symlink
handling, server composition, packaged migrations and release/install scripts.

Four confirmed findings:

1. A failed native logout publication could restore a foreign-owned old session.
2. An accepted native sign-in could be replaced with that foreign old session
   when publication failed.
3. The update panel buffered controller/proxy JSON without a byte budget.
4. A non-JSON 401 lost its HTTP status and left the update panel in reconnect
   mode instead of offering login.

Two SDK regressions failed before correction, then passed with committed receipt
revisions and a pre-authentication snapshot. Native read/write failures retain
local state while the exact prior receipt remains current; a genuinely newer
committed receipt still supersedes it. The panel regressions failed before
correction, then passed with 1 MiB success and 64 KiB error budgets and preserved
HTTP status. Actual stream cleanup uses the shared byte-budget reader.

Checks: SDK auth suites 73 passed; controller/body suites 10 passed;
real PostgreSQL hardening 7, re-audit 3 and RLS 10 passed. Capability/migration
contracts, native base lint and import-cycle preset passed. Auth/root size
budgets were increased by 250 B after measuring the revision fix at roughly
180 B gzip; REST/storage budgets remain unchanged.

Feedback into pass 2: test foreign *old versus new* revisions, repeated equal
values, interrupted owners, HTTP body cancellation, auth-transition polling,
late completion and admission cleanup. A lock/owner identity alone does not
prove which state is newer, and a decode exception must not erase HTTP meaning.

## Pass 2: Concurrency and resources

Revisited session/link/credential serialization, SDK caller cancellation and
background refresh, panel generation ownership/query deduplication/upload
ordering, catalog notification coalescing/reconciliation, export admission,
multipart cleanup permits, storage overlap/quota ownership, webhook leases/
retry fencing and host-wide deployment/recovery locks.

Two further confirmed findings:

5. An older update status poll retained a shared busy flag across logout/login,
   preventing the new session from loading and scheduling its next poll.
6. The Rust SDK applied an uncapped refresh margin, rotating a still-fresh
   short-lived token or a token with an oversized configured margin.

The real Chromium logout/login regression failed before correction. Polling is
now owned by its session generation; login/logout cancel older reads and an old
completion cannot release a newer poll. All four update browser cases passed,
including lost start responses and committed incident recovery.

The Rust SDK HTTP regression failed before correction; read and background
margins now share a half-lifetime cap. The auth suite passed eight tests, with
20 fresh-session reads in each of two lifetime/margin configurations producing
zero rotation requests. No password or JWT strength was reduced.

SDK receipt feedback scenarios passed: old versus new foreign receipts,
identical-value republication, interrupted reservations, legacy receipt upgrade,
accepted-but-unsaved sign-in and newer logout. Auth suites: 77 passed.

Feedback into pass 3: check operation ownership in both success and finally
paths; apply the same margin rule to on-demand and timer work; test body aborts
and undeclared lengths, interrupted sign-in recovery, package compatibility,
and fresh final-source browser checks. Avoid adding a timeout around an unknown
database commit and then deleting bytes that might have been published.

## Pass 3: Independent reassessment

Reassessed the final changes from callers and interrupted operations, and
revisited compatibility, role/claim boundaries, finite serialization, native
and custom persistence, response/body cancellation, timer ownership, transport
resource limits, backup/recovery, packaged migration bytes, release gates,
native lint, dependency audits and documentation consistency.

Two additional findings came from extending the native-storage visibility
criterion to adjacent credential storage:

7. Native PKCE read/consumption could use an older verifier or consume it again
   from a stale localStorage view, despite keyed Web Locks.
8. An already cancelled OAuth exchange consumed its verifier before HTTP;
   cancellation after claiming the code could discard the accepted response.

Three PKCE boundary regressions failed before the fix with faithfully separate
lock names. Native verifier publication/consumption now uses a separate journal
key and an atomic tombstone. An already cancelled caller does not claim the
verifier. Once claiming starts, exchange owns its configured HTTP lifetime;
cancellation ends the caller's wait and an accepted response still persists.
Custom adapters retain their storage API and keyed-lock serialization. A fourth
regression proves persistence after cancellation during the actual exchange.

Final local SDK checks passed: 197 TypeScript unit tests, 15 type tests,
all entry-point package/runtime checks, 31 real-server contract tests and 100
real Chromium tab-contention rounds. Rust SDK: 31 unit/integration tests and
10 doctests, targeted Clippy and seven real-server HTTP cases passed. The same
fresh-session regression also exercises the opt-in background timer.

The native PKCE protocol also passed across two real Chromium tabs with a
controlled HTTP exchange: an independently hashed challenge matches the
published verifier, a consumed verifier cannot be used a second time, and
pre-cancellation preserves the next exchange. This exercises actual browser
storage/crypto/locking; the HTTP response is a fixture and is not live provider
acceptance. The same browser gate runs on Firefox/WebKit in final CI.

Panel checks passed: 35 lifetime/cache/transport tests, 41 guide/controller
tests, Svelte/TypeScript checks, five real Chromium update cases and production
build. The added online/expired-poll case confirms that an old 401 cannot replace
a newly authenticated session. Server performance/lifetime cases: nine passed;
deployment domain: 15; CLI updates: 12 (plus the isolated worker subprocesses).
Tooling/release/installer regression suite: 42 passed. Initial security and
concurrency suites contributed 48 real PostgreSQL tests in the earlier passes.

Base lint, import cycles, dynamic-execution scan and packaged migration contracts
passed. Both npm audits reported zero vulnerabilities. Final gzip closures are
16,789 B root and 10,967 B auth, roughly 360 B above the G04 baseline. Budgets
are 16,900/11,050 B; REST/storage retain their original budgets and no runtime
dependency was added.

Optional scanner triage retains 73 advisory bug-preset diagnostics, one
intentional control-character regex finding and 34 console warnings confined
to tests/tools/examples. These include deliberate cleanup isolation, Promise
void handlers, thenable query builders and invalid-input fixtures; they are not
73 confirmed runtime bugs. Rules were not suppressed to produce a green scan.
Gitleaks/Semgrep and the typed Oxlint companion remain unavailable/disabled as
documented in the preset inventory. The GitHub code-scanning alerts API returned
HTTP 403; successful CodeQL workflow execution does not prove zero open alerts.

No additional confirmed runtime defect remained after this pass. Final remote
CI is checked on the published revision and recorded in PR #85 and the delivery
report, separately from baseline CI or these local results.

## Capability coverage across all three passes

The baseline inventory contains 768 source/test/tooling files and 94,123 lines.
Coverage combines inspection of critical paths, broad static checks and runtime
regressions; it does not claim a line-by-line proof of every possible behavior.

| Capability | Pass 1 boundary | Pass 2 ownership/resource | Pass 3 reassessment/evidence |
|---|---|---|---|
| Core/auth | Closed JWT roles, keys, redirects, pooled claims | Session/link locks, hashing admission, pruning | RLS/hardening/reaudit, cancellation, serializer and migration contracts |
| REST/catalog | Parameters, identifiers, nested query/result caps | Serialized reconciliation and coalesced notifications | Real HTTP/SDK, finite serialization and bounded collections |
| Disk/S3 storage | Paths, grants, inert file headers, private roots | Quotas, version overlap, multipart cleanup permits | PostgreSQL operation/concurrency tests; final CI includes real S3 |
| Webhooks | Connected-address SSRF, redirects, signatures | Persistent leases, retries, fencing, bounded DNS/HTTP | 13 real-service cases, preserved at-least-once semantics |
| Admin backend | Cookies, CSRF, trusted SQL/role selection | Export admission, cancellation and dedicated connections | Nine performance/lifetime cases and final panel/server CI |
| Svelte panel | Safe rendering, response shapes/status/byte caps | Cache deduplication, navigation/write/poll generations | Five update browser cases, lifecycle tests and guide/build checks |
| TypeScript SDK | Shape/finite guards, native persistence | Revision ordering, dirty state, refresh/caller ownership | PKCE negative cases, 197 units, package/types/runtime and real contracts |
| Rust SDK | Transport/serde/secret-safe session handling | Shared rotation, cancellation, half-lifetime margins | Full SDK tests/doctests, Clippy and real-server HTTP |
| Deployment/CLI | Root policy, IPC scopes, provenance, symlinks | Host lease, monotonic receipts and irreversible resume decisions | 15 domain/12 CLI update tests and isolated worker transport |
| Server/migrations | Assembly, CORS, request limits, role bootstrap | Background and connection lifetime ownership | Immutable migration mirror, capability graph and final workspace CI |
| Tooling/releases | Pinned actions/artifacts, installer checksums | Idempotent fixtures, cleanup and bounded benchmark evidence | 42 tooling regressions; final packaging/upgrade/supply-chain gates |

## Finding ledger

| ID | Confirmed behavior | Correction | Status |
|---|---|---|---|
| R01 | Failed native logout reloads another owner's older session | Compare committed revisions and preserve unsaved local logout | Corrected; before/after regression |
| R02 | Failed sign-in publication loses the accepted new session | Capture the pre-HTTP receipt and retain local state until superseded | Corrected; before/after regression |
| R03 | Update responses can be buffered without a byte limit | Shared 1 MiB success/64 KiB error reader with stream cleanup | Corrected; before/after plus undeclared-length/abort regressions |
| R04 | HTML 401 becomes a decode exception and hides login | Preserve HTTP status through malformed/error body handling | Corrected; unit and real-browser regression |
| R05 | An older poll blocks a new authenticated session | Generation-owned polls and cancellation at login/logout | Corrected; failing real-browser reproducer and two ordering cases |
| R06 | Rust SDK rotates still-fresh short-lived tokens | Shared half-lifetime margin for foreground and background | Corrected; real HTTP counts remain zero for fresh reads/timers |
| R07 | PKCE uses stale/reused native verifier state | Journal verifier publication and atomic one-time consumption | Corrected; stale-view and duplicate-code regressions |
| R08 | Cancelled OAuth consumes its verifier or loses accepted exchange | Guard before claim; preserve operation ownership after claim | Corrected; cancellation before/during exchange regressions |

## Final validation and limitations

The associated PR records final-source CI and its workflow URL. Baseline
success is never substituted for final-source verification. Generated embedded
assets are committed separately and rebuilt for a clean repeat.

Production VPS enrollment, abrupt power loss, off-host restore, actual
Docker/systemd activation with newly published attested releases and long soak
acceptance for the update controller remain separate G03 gates. G04 did not
publish, deploy or certify that operating matrix. No new throughput or p95
benchmark claim is made from unit/runtime correctness tests. The prior G02
release campaign remains historical evidence tied to its own revision.

Open-alert API access and optional unavailable scanners are reported explicitly;
they do not turn an unrun check into a pass. The requested Portuguese delivery
report contains the published SHA, final CI outcome and remaining limits.
