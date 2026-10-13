# Ownership and corrected findings

Baseline: `a7b77634361961023341eba540bb8dfc5e5ef6e3`, release build with the
repository profile and Rust 1.99. The executable was copied and SHA-256 recorded
before changing Rust source. Benchmark metadata also records binary digests,
image/runtime versions, dirty source status and resource budgets.

## Final capability graph

| Owner | Internal runtime dependencies | Public responsibility |
|---|---|---|
| core | none | Claims, errors, config, pools, transaction context and canonical migration lifecycle. |
| auth | core | Authentication router/state, keys/verifier, passwords, links/OAuth, sessions and pruning. |
| API | core, auth | Catalog, query/SQL construction and REST/RPC under request RLS. |
| storage | core, auth | Object operation ownership, disk/S3 adapters, metadata, quotas, signing and cleanup. |
| webhooks | core | Configuration, durable capture/queue, bounded delivery, signatures and leases. |
| admin | core, auth, API, storage, webhooks | Panel transport/session adaptation and bounded admin operations. |
| CLI | core, auth, API | Deployment, backup/restore, migration and host lifecycle commands. |
| server | all backend capabilities | Composition, configuration and process lifecycle. |
| Rust SDK | no backend capability | HTTP/query/auth/storage client contracts and coordinated token rotation. |
| TypeScript SDK | no backend capability | Browser/server client contracts, cancellation and refresh coordination. |

`scripts/check-boundaries.py` checks the intended direct dependency sets and
the packaged migration mirror. No reverse dependency on server is introduced.
The external archive consumer independently checks that excluded capabilities
are absent from its transitive graph.

The panel keeps feature state/helpers with their features; shared transport
errors live in `api-error.ts`. This removes the API/i18n import cycle while
preserving the existing `ApiError` re-export. Dialog content imports its overlay
directly, removing its import through its own barrel. UI translations and
wire contracts retain their existing owners.

## Confirmed findings and corrections

| Finding | Evidence | Correction |
|---|---|---|
| Service-token lifetime multiplication/addition could overflow. | Failing key unit regression with `u64::MAX`. | Checked arithmetic, positive lifetime and startup/session lifetime validation. |
| Direct auth reuse accepted a short HS256 secret. | Failing key regression with runtime-generated 31-byte input. | The key factory enforces the same byte minimum as the full server. |
| Privileged/inheriting built-in roles could expose credentials. | Isolated PostgreSQL proves that INHERIT TRUE overrides NOINHERIT and grants anon access to auth token hashes. | Validate flags and membership inheritance on all five shared/auth roles and existing storage/webhook roles; fail startup without silently downgrading them. |
| Cancelled bootstrap left a backend waiting for the advisory lock, including after its runtime stopped. | Real-PostgreSQL cancellation and stopped-runtime regressions; the latter failed before correction. | Own/abort the driver, send bounded cancellation and enable per-bootstrap one-second client connection checks without limiting migration duration. |
| Hash concurrency did not bound execution threads/allocator arenas. | A two-slot pool used four execution threads across repeated admitted work. | Fixed worker threads, bounded submission and cancellation-owned permits; preserve Argon2id strength. |
| Fast webhook destinations waited for every 500-ms poll. | Only 37 of 64 events delivered within five seconds. | Refill on completion, coalesce ready completions and retain idle/error backoff. |
| Old benchmark disabled certificate verification globally and used a floating runner image. | `bench/read.js` and `bench/run.sh` baseline. | TLS verification by default and a pinned native k6 digest; insecure TLS needs an explicit development opt-in. |
| Panel error and dialog imports formed cycles. | Four paired `import/no-cycle` diagnostics. | Move the shared error contract and import the overlay implementation directly; gate imports on PRs. |
| Normal refresh made two consecutive database state-update round trips. | Quiet-host refresh p95 exceeded the unchanged 10% gate; 54 auth/concurrency/SDK regressions cover resulting behavior. | Combine token retirement/session timestamp updates in one CTE within the same already-locked transaction. |
| Promotion reported success before transaction commit. | Failure/rollback inspection of the promotion transaction. | Emit migration success only after COMMIT. |
| Compile-only archive checks inferred runtime work from an inherited DB URL. | An unusable inherited URL caused the old compile-only command to launch the consumer and fail. | Start runtime fixtures/consumer only with --run; the same compile-only reproduction now passes. |
| Ownership checks missed dependency aliases and target-specific entries. | Temporary manifest regressions with a hidden reverse dependency. | Resolve package/workspace names and target runtime dependencies; require explicit ownership for every capability. |
| Failed/incomparable baselines could certify performance. | Zero valid baseline samples still returned gate_passed=true. | Require three valid samples on both sides, finite metrics and matching fixture/duration evidence; fail incomparable reports. |
| Docker timeout errors could reveal credential arguments or bypass cleanup. | A TimeoutExpired mock includes the plaintext fixture password in its default representation. | Suppress argument-bearing exceptions, preserve stderr diagnostics and keep teardown independent of diagnostic capture. |

Auth-only setup additionally required scoped initialization, archived SQL assets,
an explicit promotion path and owned pruning. These are reuse deliverables,
rather than claims that existing full-database initialization was corrupt.
Promotion's lower gaps commit together, including its history records, so
interrupted promotion cannot leave an unsupported lower-gap history.

## Scanner triage

The original optional full preset scan had 1,175 errors and 31 warnings. Most
are advisory/style findings: 764 brace preferences, 196 non-null assertions
(many in fixtures), function/parameter complexity, deliberate serial loops,
and test/script console output. The security group has 31 console findings
in fixtures/tools/examples, plus the control-character regex used to neutralize
spreadsheet formulas. Production panel/SDK source does not log those secrets.
Removing that regex would weaken sanitization. Structural Ed25519 PKCS#8 bytes
are ASN.1 framing, not a hard-coded private key. The webhook's published
independent HMAC vector deliberately uses a known fixture key; recovery test
passwords also belong to disposable fixtures. Such CodeQL cryptographic-value
findings require test/known-vector triage, rather than treating those values as
production credentials. New reusable-auth DB fixtures share a process-local,
runtime-generated authenticator password because roles are cluster-global.

Rules remain enabled. The final optional native scan has 1,169 errors and
31 warnings, with zero execution failures. Base/import gates pass; confirmed
runtime import cycles were corrected. Optional broad presets remain advisory and are not
represented as zero findings or converted into blanket suppressions. Deliberate
serial operations preserve request ordering, transaction scope and backpressure;
they are not blindly parallelized in response to `no-await-in-loop`.

Existing real-PostgreSQL/HTTP/S3 regressions cover SQL injection, role/claims
cleanup, RLS isolation, token/link/OAuth races, SSRF, header/path checks, storage
overlap/quotas, pending multipart cleanup, bounded exports, retries/fencing and
SDK/panel cancellation. New checks complement these boundaries without adding
generic repositories or unrelated layers. Full results belong in the execution
record rather than assuming a green linter proves those runtime properties.
