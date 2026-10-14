# G04: Three complete project reviews with corrective feedback

Status: three review passes completed; final-source CI is tracked in PR #85
and the delivery report. The starting revision is
`bb9e6ef986687e505af5c8e809a77a53c380ebc9`.

## Objective

Review the project in three distinct passes, reproduce and fix confirmed
defects, improve the review criteria from the findings, and leave a report
that separates observed results from remaining operational acceptance work.
Execution is autonomous; use the existing architecture and reasonable defaults
without waiting for routine implementation choices.

## Scope

Cover all capabilities: authentication, REST/RLS/catalog, storage, webhooks,
admin transport and Svelte panel, both SDKs, core configuration/database
lifecycle, server composition, CLI deployment/backup/recovery, migrations,
release/install tooling, dependency policy, CI and documented operating limits.
Preserve the single native Rust backend, PostgreSQL authorization, immutable
released migrations and the public HTTP/SDK contracts.

## Review protocol

| Pass | Primary questions | Feedback into the next pass |
|---|---|---|
| 1: Trust and correctness | Can untrusted inputs cross authorization, SQL, filesystem, network, rendering or credential boundaries? Are null/non-finite values, malformed data and failure responses handled correctly? | Turn each finding into an invariant and identify adjacent callers/backends that need the same check. |
| 2: Concurrency and resources | Who owns transactions, leases, cancellation, timers, refreshes and cleanup? Can stale responses, retries or partial persistence duplicate work, revive revoked state or retain unbounded resources? | Exercise interruption and contention, inspect sibling operations, and add the missing lifecycle/ownership criteria. |
| 3: Independent reassessment | Does the final implementation satisfy both earlier passes under failure? Are compatibility, packaging, upgrades, recovery, observability and release gates consistent with the documented behavior? | Revisit every changed boundary from the caller's perspective, fix new findings and repeat affected checks before closing the report. |

Every pass revisits the full capability matrix; the primary question changes
the review lens rather than excluding other areas. Record the actual files,
invariants and checks inspected. Three reviews do not mean running an unchanged
linter three times, and scanner output is not a proof of runtime safety.

## Correction and verification rules

- Confirm a suspected defect with a failing regression or concrete runtime
  evidence before claiming it was fixed. Keep a finding ledger with severity,
  reproduction, correction, relevant tests and residual limits.
- Extend criteria when a defect reveals a missing assumption. Check all
  applicable sibling operations and adapters, including disk/S3, browser/server
  SDKs and native/container host layouts.
- Make focused changes in the owning package. Preserve early returns and
  validate input at its boundary; avoid speculative abstractions or unrelated
  style rewrites.
- Use disposable fixtures and bounded stress scenarios. Respect AGENTS.md:
  low priority, four Cargo jobs, two test threads, and no simultaneous
  Rust/Vitest/audit runs. Reuse the shared test Postgres and preserve unrelated
  services. Run broader checks when the scope or new evidence requires them.
- Keep fixes and regression tests together in Conventional Commits; regenerate
  embedded panel assets in a separate build commit. Verify the final source
  revision and final CI rather than inheriting green checks from an older SHA.
- Do not publish a release, enable a production controller, merge a PR or delete
  remote branches as a side effect of this review. These are separate operating
  actions from the requested review and report.

## Completion criteria

- [x] Pass 1 covers the capability matrix and records confirmed findings.
- [x] Pass 2 covers the matrix again and applies explicit feedback from pass 1.
- [x] Pass 3 reassesses the final changes and expands the criteria again.
- [x] Confirmed defects are corrected and relevant regressions pass.
- [x] Architecture, migrations, dependency policy and generated assets have
  validation evidence; final-source CI is recorded with its SHA in the PR/report.
- [x] The report records all three passes, corrections, actual validation,
  unchanged findings that were not defects, and unrun operational acceptance.
- [x] A Portuguese report is prepared for the requester, without a pending
  clarification or a claim of exhaustive proof that no future bug is possible.

The execution record is maintained in
[triple-review-execution.md](triple-review-execution.md).
