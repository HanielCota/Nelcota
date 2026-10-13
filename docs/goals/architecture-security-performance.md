# G02: Organize and harden Nelcota under realistic load

Status: completed; two execution passes and two corrective reviews are recorded
in [the execution report](execution-report.md).

## Objective

Organize the entire project around clear capability ownership, apply SOLID where
it improves real boundaries, fix reproduced security and concurrency defects,
and demonstrate performance and recovery under load, stress and sustained use.
Preserve PostgreSQL RLS, the single Rust backend binary and existing contracts.

## Scope and organization

Cover the Rust crates, the embedded Svelte/TypeScript panel, both SDKs, SQL
migrations, deployment/backup workflows and supporting test/benchmark tooling.
Rust uses crates, modules, structs, enums and traits. Organize TypeScript state,
components and helpers by feature and shared responsibility.

Use [architecture.md](../architecture.md) as the ownership map: server composes
capabilities; core supplies shared primitives; auth owns authentication; API owns
REST/query/catalog behavior; storage owns file operations; webhooks owns delivery;
admin adapts panel workflows; CLI owns host lifecycle and operational commands.
Keep library modules independent of the server assembly and avoid dependency
cycles between capabilities or a catch-all shared module. Internal references
to types exposed by a Rust module facade are not a capability dependency cycle.

| Principle | Application |
|---|---|
| Single responsibility | Give each operation and module one coherent reason to change; keep HTTP adaptation separate from transaction and cleanup ownership. |
| Open/closed | Extend real variation through existing bounded interfaces, such as mail or storage backends, without broad unrelated edits. |
| Substitution | Test interchangeable implementations against the same documented behavior and failure contract. |
| Interface segregation | Expose the operations each consumer needs and keep implementation details private. |
| Dependency inversion | Supply dependencies explicitly at capability boundaries; use traits when implementations vary and concrete functions/types elsewhere. |

A move or abstraction needs a concrete ownership, reuse or testability benefit.
File length and class count are not architecture success metrics.

## Milestones

1. **Inventory and baseline.** Map dependencies, public interfaces, misplaced
   responsibilities, duplicated behavior and existing security/resource limits.
   Record actual findings with reproduction steps. Capture a release-build
   performance baseline before refactoring affected paths.
2. **Organization by capability.** Correct misplaced modules and tangled
   responsibilities in small changes. Preserve behavior and public facades,
   use existing wire contracts and keep validation, transactions, cancellation
   and cleanup under one operation owner.
3. **Security and correctness.** Reproduce and fix boundary failures, with tests
   proving isolation and atomicity. Review the areas listed below and resolve
   relevant scanner findings with evidence, including documented false positives.
4. **Measured optimization.** Fix observed repeated work, unbounded allocations,
   pool starvation, oversized results and expensive queries. Compare the same
   fixtures before and after; measure authenticated RLS paths as well as owner
   and service paths, with fresh and updated database statistics.
5. **Load, stress and recovery.** Add reproducible scenarios to existing benchmark
   tooling, measure resource behavior and verify correctness during overload,
   interruption and recovery.
6. **Regression gates and documentation.** Keep fast relevant checks on PRs;
   schedule expensive load/soak work separately. Record architecture decisions,
   operating limits, run instructions and remaining limitations.

## Security and concurrency scope

- Authentication/session/link races, key and secret handling, OAuth/PKCE,
  redirects, rate limits and lifecycle cleanup, coordinated with G01.
- RLS and database grants, role/claims reset, atomic writes, generated metadata,
  and absence of unintended privilege escalation or cross-user data access.
- Input boundaries: null and non-finite numbers, SQL parameters and identifiers,
  unsafe HTML/rendering, header injection, file paths, signed URLs and payload
  limits. Preserve guard clauses and the project's no-else convention.
- Webhook SSRF, DNS/redirect handling, signatures, persistent leases, retries
  and fencing between workers. Preserve documented at-least-once semantics;
  retries must retain event identity rather than claim exactly-once delivery.
- Storage upload/overwrite/delete overlap, unpublished object cleanup, quotas
  and concurrent SQL/API writers, for both disk and real S3.
- Frontend/SDK duplicate requests, refresh coordination, stale response ordering,
  cancellation, retries after committed mutations and secret-safe persistence.
- Backup/restore consistency, interrupted deployment/upgrades and additive
  migrations. Check dependencies through the existing audit/deny and native
  preset tools without suppressing security checks to make CI green.

## Workload matrix

| Scenario | What to exercise |
|---|---|
| Auth | Hashing/login, verification, refresh rotation/replay and revocation; clients from multiple application instances share the database. |
| REST/RLS | Indexed reads, filters, writes, RPC, pagination and large datasets with both visible and hidden rows. |
| Panel | Concurrent previews, counts, SQL executions, exports with slow/unpolled consumers and cancellation while ordinary requests continue. |
| Storage | Wide/nested directories, Unicode prefixes, collisions, upload conflicts, quotas and multipart interruption on disk/S3. |
| Webhooks | Bursts, slow/failing destinations, retries, restarts, lease expiry and correct signed event identity. |
| Mixed traffic | Auth, REST, panel, storage and webhook activity together, including pool saturation and post-overload recovery. |

Use seeded disposable data, runtime-generated fixture secrets and controlled
provider/mail/webhook destinations. Run load generators against test environments
with explicit CPU, RAM, connection, request-rate and storage limits.

## Measurement and stress protocol

- Reuse [bench](../../bench/README.md) and the native k6 container approach.
  Record Git SHA, release flags, runtime/tool versions, hardware, topology,
  profile, dataset, RLS policy, load shape and configured budgets.
- Measure p50/p95/p99, achieved throughput, unexpected errors, expected
  429/503 responses, RSS/CPU, PostgreSQL connections/locks, query plans and
  pending work. Report auth hashing separately from indexed REST reads.
- Warm the environment and compare medians from at least three equivalent
  steady-load runs. Agree and record the operating envelope before optimization;
  the default refactor gate permits at most a 10% regression in p95 or throughput
  on the same scenario. Investigate noisy/inconclusive results rather than
  silently changing the gate.
- Increase load through measured capacity into overload, add sudden bursts and
  run a soak for at least 30 minutes near the documented sustainable capacity.
  Repeat equivalent load cycles to check for retained-work or memory growth.
- During controlled failures, exercise database locks/unavailability, client
  disconnects, slow consumers, provider failures and worker/server restart.
  Verify rollback, isolation, idempotent credential consumption and cleanup.
- Require recovery within each documented resource deadline plus its cleanup
  grace. Check that normal requests resume and connections, admission slots and
  pending work return to their expected steady state.
- Respect AGENTS.md: low-priority local execution, bounded test threads and no
  simultaneous Rust/Vitest/audit runs. Large stress/soak campaigns belong on
  resource-limited dedicated environments; preserve diagnostics and tear down
  fixtures created by each run.

## Completion criteria

- [x] Dependency and ownership maps describe the final organization, with
  explicit public interfaces and no new cycles between capabilities or
  unexplained shared layers.
- [x] Refactors preserve external HTTP/JWT/SDK contracts, existing data and
  migration history, with compatibility and upgrade evidence where affected.
- [x] Each confirmed defect has a reproducer, fix and regression covering the
  relevant security, concurrency or resource boundary.
- [x] Nominal load has no unexpected failures or correctness violations;
  deliberate overload produces documented bounded responses.
- [x] Connections, admission, buffered results and background work stay within
  configured budgets; load cycles show no unbounded accumulation or OOM.
- [x] Before/after measurements meet the declared regression envelope and
  document the sustainable capacity for each representative workload.
- [x] Stress, spike, mixed-traffic and 30-minute soak reports demonstrate
  recovery, cleanup and preserved data/access invariants.
- [x] Relevant formatting, Clippy, Rust/S3/upgrade, frontend/SDK, native lint and
  dependency checks pass; heavy benchmark jobs are reproducible separately.
- [x] Documentation records the module map, decisions, validated operating
  limits, measured results and any explicitly unresolved limitations.

## Execution record

Track each milestone's scope, evidence, resulting changes and checks. Code,
tests and generated panel assets follow CONTRIBUTING.md commit/PR conventions.
Distinguish planned work from completed checks and measured results.
