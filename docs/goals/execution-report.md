# Goal execution and validation

Status: completed. Both goals received two execution passes and two final
reviews with corrections. Two original 30-minute soaks and an additional
30-minute soak of the corrected refresh implementation completed successfully.

Baseline commit: `a7b77634361961023341eba540bb8dfc5e5ef6e3`.
Work branch: `feat/auth-reuse-and-quality-goals`.

## Deliverables

Authentication now has a public constructor, scoped database bootstrap, packaged
SQL assets, owned pruning and an external Rust consumer. The consumer builds
from actual auth/core `.crate` archives, without server, CLI, REST, panel,
storage or webhook dependencies. Auth-only databases promote to full Nelcota
without changing released migration bytes, names, checksums, users or sessions.
See the [integration guide](../auth-integration.md).

Password hashing uses fixed worker threads and cancellation-owned admission;
webhook scheduling refills completed slots immediately. Argon2id remains
m=19,456 KiB/t=2/p=1. The panel shares its transport error type without an
API/i18n cycle and imports dialog overlay directly. Ownership and runtime
interfaces are checked without adding generic repositories or unrelated layers.
See the [ownership map and findings](ownership-and-findings.md).

The review additionally rejects inherited privileges on all protected built-in
roles, detects disconnected bootstrap clients even after runtime shutdown,
emits migration success logs after COMMIT, prevents inherited configuration
from activating compile-only tests and rejects incomparable benchmark evidence.

## Validation matrix

| Check | Evidence |
|---|---|
| Rust workspace, execution 1 | 482 passed, zero failed; two S3-only tests ignored in the ordinary run. |
| Rust workspace, execution 2 | 488 passed, zero failed; the same two S3-only tests separately exercised. |
| Auth reuse after review corrections | Nine real-PostgreSQL regressions passed, including runtime shutdown, inherited credential access, checksum failure, promotion rollback and independent-instance revocation. |
| Real S3 | Both tests passed twice, including interrupted/timed-out multipart cleanup. |
| Panel | 390 unit tests and all 61 Chromium cases passed twice; Svelte/type checks have zero errors/warnings. |
| TypeScript SDK | 173 unit and 15 type tests passed twice. Builds, package checks and real PostgreSQL/Mailpit/Chromium contracts passed, including two-tab refresh/sign-out and the demo. |
| Rust SDK | Package verification and an external archive consumer passed. |
| Upgrade | Published 0.3.1 to the checkout passed twice, preserving rows, exact numerics, sequences, RLS, roles, passwords/sessions, disk bytes, webhook retry/restart/rollback and pre-upgrade backup restoration. |
| Auth distribution | External archive compilation, signup/rotation/JWKS, scoped SQL and graceful shutdown passed. Compile-only also passes with an unusable inherited DB URL and starts no consumer process. |
| Dependencies | Cargo audit: 467 dependencies, no vulnerabilities. cargo-deny 0.20.2: advisories/bans/licenses/sources passed. Both npm audits: zero vulnerabilities. |
| Fast gates | Rustfmt, Clippy with warnings denied, migration/boundary checks, native base/import checks and 22 Python/preset regressions passed. |

An earlier browser attempt edited live source during execution and failed;
affected cases and then two complete stable-source suites passed. Failed
attempts remain in local diagnostics. Optional Oxlint advisories total
**1,169 errors/31 warnings**, with zero execution failures. Base and import
checks have zero findings. Confirmed import cycles are corrected; broad
advisory rules remain enabled and triaged. A green gate does not prove the
absence of all possible future defects.

The isolated crypto benchmark measured hashing median/p95 **15.58/21.04 ms**
and EdDSA verification **35.10/48.42 microseconds**, excluding HTTP/SQL latency
and preserving configured password strength.

## Release measurement protocol

The baseline executable was saved before source changes. Reference and candidate
use the repository release profile, Rust 1.99 and the same pinned native k6
image. Fixtures contain 100k products, 100k RLS rows with half hidden, 10k object
metadata entries and bounded real file uploads/downloads/overwrites. The 10k
entries are listing metadata, rather than 10k large physical blobs. Fresh and
analyzed statistics have separate runs and owner/service/RLS query plans.

| Component | Budget |
|---|---|
| App | One CPU, 128 MiB, two password workers. |
| PostgreSQL | One CPU, 384 MiB, 40 connections, 64 MiB WAL target. |
| Request pool | Eight connections; admin/webhook pools retain separate existing limits. |
| Native k6 | One CPU, 512 MiB; Go memory target 448 MiB. |
| Database disk | Owned disposable volume; monitored 512-MiB fixture budget. |
| S3 fixture when selected | RustFS, one additional CPU, 512 MiB. |

The app, database and generator use distinct CPU affinity slots; S3 uses a
fourth slot. Metadata records binary hashes, starting Git commit/dirty state,
images, PostgreSQL version, dataset and budgets. Results are local to this
host, exclude Caddy/public TLS and do not establish production/cloud or
large-object network capacity.

The unchanged gate compares medians of **three valid 30-second nominal runs**:
p95 may increase at most 10%, and throughput may decrease at most 10%.
Execution 1 passed all seven scenarios:

| Scenario | Baseline p95 ms | Candidate p95 ms | Baseline req/s | Candidate req/s |
|---|---:|---:|---:|---:|
| Indexed read | 10.84 | 11.21 | 2,095.34 | 1,964.75 |
| Authenticated RLS | 12.76 | 13.37 | 1,668.79 | 1,651.74 |
| Panel | 29.04 | 28.19 | 700.27 | 721.84 |
| Disk storage | 56.57 | 58.39 | 653.79 | 637.89 |
| Password login | 312.65 | 324.52 | 55.49 | 56.08 |
| Refresh | 29.16 | 30.11 | 738.91 | 708.86 |
| Mixed | 73.75 | 73.71 | 790.55 | 750.17 |

Full p50/p95/p99, throughput and ratios are in
[comparison-pass-1.json](results/comparison-pass-1.json).
Mixed p99 was 330.80 ms versus 237.96 ms in this nominal comparison despite
passing the declared p95/throughput gate. This is a measured tail-latency cost,
not a claim that every percentile improved.

Execution 2 initially exceeded the gate for refresh and mixed traffic while
browser validation ran on other CPUs. The failed comparison is preserved;
CPU affinity alone does not eliminate host-wide contention. A sequential,
quiet-host repetition passed mixed traffic but refresh p95 remained 12.38%
above its equivalent baseline. Normal rotation performed two consecutive state
updates as separate round trips. The review combines them into one CTE in the
same locked transaction; all 54 affected auth/concurrency/Rust-SDK regressions
passed. A fresh complete reference/candidate comparison, another full soak and
the real S3 comparison completed. The gate has not been relaxed.

## Corrected-release comparison

The final release combines the normal refresh state updates while retaining
row locks, one transaction, retry grace and the existing HTTP response. The
fresh complete comparison passed read, panel, disk storage, login, refresh and
mixed traffic. Full-matrix RLS initially failed (p95 +14.35%, throughput -12.60%);
its request implementation did not change and both plans use the same primary
index, filter 20 rows and execute in approximately 0.06 ms. PostgreSQL saturated
its CPU in both runs. An independent isolated three-run RLS pair passed. This
supports the measured isolated envelope; preserve the full-matrix variation
and repeat capacity planning on a dedicated host.

| Scenario | Candidate / baseline p95 | Candidate / baseline throughput | Evidence |
|---|---:|---:|---|
| Read | 1.007 | 0.982 | Full campaign |
| RLS | 0.953 | 1.050 | Isolated matched pair |
| Panel | 1.004 | 1.004 | Full campaign |
| Disk storage | 1.014 | 0.966 | Full campaign |
| Login | 1.029 | 1.024 | Full campaign |
| Refresh | 1.052 | 1.022 | Full campaign |
| Mixed | 0.930 | 1.032 | Full campaign |

[Corrected-release comparison](results/comparison-corrected-release.json)
identifies each source protocol;
[the failed full-matrix attempt](results/comparison-full-attempt.json) remains
available. Each reported comparison has three valid 30-second reference and
candidate samples and uses the unchanged 10% gate. The RLS row is explicitly
an isolated measurement rather than an assertion that its full-matrix attempt
passed.

Before opening the PR, two additional isolated RLS pairs used execution order
baseline/candidate/candidate/baseline with the same saved release binaries,
fixtures, resource budgets and three analyzed 30-second samples per campaign.
Both passed the unchanged gate: candidate/baseline p95 ratios were **0.970** and
**0.920**, with throughput ratios **0.991** and **1.074**. Each plan used
`bench_private_pkey`, returned 20 rows, filtered 20 rows and executed in
0.079–0.085 ms. Nominal response/ownership checks passed; both candidates
drained the webhook queue without OOM, idle transactions or waiting locks.
The old baseline retained 582/712 webhooks after its recovery deadline; those
failures are recorded separately from the successful nominal RLS samples.
[Counterbalanced RLS evidence](results/comparison-rls-pre-pr.json) contains all
twelve nominal samples, both comparisons, binary/fixture metadata and final
invariants. These measurements did not reproduce a consistent RLS regression
and reduce execution-order bias. They do not prove the precise cause of the
historical full-matrix variation; its failed result remains preserved.

The real RustFS storage pair passed: baseline/candidate p95
79.79/79.89 ms (ratio 1.001), throughput 631.27/651.42 req/s (ratio 1.032).
Listing, downloads and overwrites use the same 10k-entry metadata fixture,
small real byte payloads and one-CPU/512-MiB object service on its fourth CPU.
[The S3 comparison](results/comparison-s3.json) and
[invariants](results/s3-invariants.json) retain image/trust-bundle hashes,
post-load byte checks, signatures, queue recovery and container state. The old
scheduler again failed its queue recovery; the corrected candidate drained it.
This is a short S3 workload comparison, alongside two real multipart-failure
regressions, rather than a 30-minute S3 soak.

## Stress, soak and recovery

Both campaigns exercise 16/32/64/128-VU mixed stress, four-VU recovery, a server
restart and a **1,800-real-second** constant-arrival soak at 80% of each
campaign's measured mixed capacity. Traffic includes reads, RLS, panel,
storage, hashing, refresh and signed webhook-producing writes. Nominal
responses and RLS/signature checks must pass; deliberate overload permits
bounded 429/503/504 responses. Webhooks must drain within 30 seconds.

| Evidence | Execution 1 | Execution 2 |
|---|---:|---:|
| Measured soak duration | 1,800.021 s | 1,800.005 s |
| Requested / achieved rate | 600 / 599.83 req/s | 459 / 458.92 req/s |
| Completed HTTP requests | 1,079,703 | 826,057 |
| Soak p50 / p95 / p99 ms | 4.44 / 45.30 / 134.07 | 2.31 / 26.28 / 52.98 |
| Dropped generator iterations | 304 (0.028%) | 147 (0.018%) |
| Unexpected responses / failed checks | 0 / 0 | 0 / 0 |
| Peak app RSS, whole campaign | 45.42 MiB | 46.53 MiB |
| Last 20-minute app RSS range | 43.68–44.53 MiB | 41.57–42.50 MiB |
| Peak database working set / disk | 203.6 / 249 MiB | 188.1 / 220 MiB |
| Peak connections / queued webhooks | 16 / 21 | 16 / 15 |
| Final idle transactions / waiting locks / queued webhooks | 0 / 0 / 0 | 0 / 0 / 0 |
| App / database OOM | None / none | None / none |

[Execution 1](results/execution-pass-1.json) and
[execution 2](results/execution-pass-2.json) include stress/recovery timings and
final invariants. There were 205/207 resource samples, maximum gaps of
13.8/13.7 seconds and no sampler errors. Transient locks/transactions during
requests are distinguished from retained work after traffic stops. These
measurements do not guarantee constant memory beyond the observed duration.

Validated mixed rates are 459–600 req/s under these fixtures and budgets;
the second rate was calibrated under a busier host. Stress raised queueing
latency while preserving bounds; first-pass 128-VU p99 was 3.49 seconds and
ordinary four-VU traffic recovered before/after restart. Dedicated real-service
regressions also cover database locks/unavailability, unpolled exports,
provider/mail failures, cancellation, multipart interruption and lease fencing.

The long campaigns used the measured binary recorded in their JSON summaries.
Subsequent review changes include bootstrap validation/diagnostics, benchmark
evidence/cleanup and the combined refresh state update. The corrected release
therefore receives another complete measured campaign and 30-minute soak;
archive consumers and 54 affected regressions also exercise it.

The corrected release additionally completed another 1,800.017-second soak:
1,065,313 HTTP requests at 591.83 req/s, zero unexpected responses/check failures,
2,094 dropped iterations (0.196%), and p50/p95/p99 of 5.47/95.84/302.63 ms.
Peak RSS was 45.75 MiB; the final queue, idle transactions and waiting locks
were zero. All 129 bounded fixture objects passed post-load byte verification.
[Corrected-release soak](results/corrected-release-soak.json) records its
binary hash, capacity, stress, resources and container invariants. One resource
sample encountered the intentionally removed app during restart; the largest
sample gap was 25.5 seconds. This gap is recorded rather than described as zero
sampler errors. The higher sustained-load tail latency is retained alongside
the passing correctness/resource bounds.

## Reviews, failed attempts and limits

First final review: confirmed and corrected inherited credential privileges,
a bootstrap backend retained after runtime shutdown, pre-COMMIT success logs,
compile-only runtime activation, renamed/target-specific dependency escapes
and an incomplete baseline reported as passing. New regressions distinguish
successful, failed and incomparable evidence, reject non-finite metrics and
check resource/duration equivalence. Harness output reuse is rejected;
arrival delivery must reach 99%, final object bytes/container state are checked,
logs retain stderr and diagnostic failure cannot prevent container removal.
Second final review rechecked public/distribution interfaces, transaction and
worker ownership, immutable SQL mirrors, CI configuration and evidence limits.
It additionally rejects Python -O in archive checks so disabled assertions
cannot certify integration behavior. The actual optimized-mode command is
rejected before any fixture is created. The S3 startup fixture initially lacked CA certificates. It now supplies the
system trust bundle without disabling TLS verification and records its SHA-256
for equivalence. S3 reference/candidate measurements and affected script checks
passed. Both reviews are complete.

The old binary intermittently exceeded 128 MiB during hashing stress. Its
scheduler left **5,950 webhooks** after the equivalent 30-second recovery
budget. The fixed worker pool bounds execution threads without weakening
Argon2id; the corrected delivery scheduler drained both long-run queues.

Two earlier harness failures interrupted soaks and are excluded: PostgreSQL
RAM was exhausted by data/WAL growth in tmpfs, and k6 was killed under a
256-MiB generator quota. Owned disk storage, bounded response-body retention
and a 512-MiB generator corrected the harness. App/database CPU/RAM budgets
remain unchanged; equivalent reference measurements were repeated. Earlier
failed/noisy comparisons are not presented as passing evidence.

Auth/core packages travel together; publication and repository extraction are
separate release steps. The demonstrated connector uses private PostgreSQL
with `NoTls`; managed-database TLS integration is outside this example. Existing
issuer/audience and stateless access-token policies are documented in the
integration guide. Rate limits remain per process. Real S3 failure regressions
and short workload comparisons do not constitute a 30-minute S3 soak.

[Functional validation summary](results/validation.json) records counts and
scope. PR checks now cover capability/migration boundaries, archive integration,
native base/import gates and preset/benchmark-evidence regressions. Heavy stress
remains a separately reproducible manual job. The included auth host, immutable
SQL packaging, compatibility policy and operating limits complete G01/G02;
future releases should repeat measurements on their deployment hardware.

Raw attempts are retained in ignored `.tools/reports/goals/`; sanitized results
are versioned here. The manual stress workflow uploads diagnostics on a
self-hosted Linux runner labeled `nelcota-benchmark`. No runner was provisioned,
package published, repository extracted or external service deployed.

PR review found CodeQL alert 144 in the benchmark's temporary credential JSON.
The harness now keeps that fixture in memory and supplies it through the Docker
child environment, without writing/mounting a credential file or including its
value in generator command arguments. Three five-second mixed samples plus
warmup, recovery and restart passed on the unchanged release binary: no failed
checks, queue residue, idle transactions, waiting locks or OOM; all bounded
fixture objects passed byte verification. This is a fixture transport regression
check, rather than a replacement for the recorded 30-second comparisons/soaks.
The pre-PR review also bounds SDK failure annotations to preserve their final
error and ignores local standalone-consumer build output.
