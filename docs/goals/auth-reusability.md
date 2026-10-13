# G01: Make Nelcota authentication reusable

Status: completed; two execution passes and two corrective reviews are recorded
in [the execution report](execution-report.md).

## Objective

Allow another Rust application using PostgreSQL to integrate Nelcota
authentication without depending on the panel, storage, REST API, webhooks,
CLI or server crate. Define a stable public contract and prove reuse in a
consumer outside this Cargo workspace, while preserving existing Nelcota
behavior, data and security boundaries.

## Starting point

[nelcota-auth](../../crates/auth/Cargo.toml) is already a capability crate.
Its public API includes the router, AuthState, AuthSettings, JWT verification,
passwords, mail delivery and OAuth providers. Its direct internal dependency
is nelcota-core; it does not directly depend on the panel or storage.

The remaining reuse questions include shared Claims/Role/error contracts,
database roles and initialization, central bootstrap embedding all migrations,
workspace/path dependencies and the inherited publish=false setting.
The supported starting stack is Rust, Axum and PostgreSQL; portability to
other languages or databases is outside this goal.

## Deliverables

1. Inventory the auth dependency graph, configuration, public types, background
   tasks, database objects and transaction ownership. Identify actual coupling
   with evidence before changing interfaces.
2. Document the supported consumer API: construction, routes, configuration,
   JWT/JWKS, session lifecycle, errors, startup, pruning, shutdown and resource
   limits. Keep types returned by public operations reachable to consumers.
3. Resolve the minimum shared-contract and bootstrap dependencies demonstrated
   by the external consumer. Existing concrete functions remain appropriate;
   use interchangeable adapters where implementations vary, such as mail,
   token verification and provider transport.
4. Define an idempotent auth initialization and upgrade path for a fresh
   consumer database that needs only auth and its required shared SQL objects.
   Preserve the central migration history and existing Nelcota installations;
   do not change released migration names, contents or checksums.
5. Add a minimal runnable consumer outside the workspace. Document how it builds
   and starts using the intended dependency set, generates development keys,
   initializes disposable PostgreSQL, exercises authentication and shuts down.
   Inspect its dependency tree to prove the excluded capability crates are absent.
6. Define versioning and compatibility for the public Rust API, HTTP/JWT
   contracts and database lifecycle. Check that the intended distribution can
   include required assets and dependency versions. Record packaging limitations
   explicitly; a path-only workspace example does not prove publishable packaging.
7. Document integration, configuration defaults, optional capabilities, upgrade
   steps and troubleshooting. Preserve the current protocols consumed by both
   SDKs and the embedded panel.

## Security and concurrency validation

Use a disposable PostgreSQL instance and controlled mail/provider fixtures.
Cover:

- Signup with confirmation enabled and disabled, login, token refresh, logout,
  revocation, recovery, confirmation, magic links and supported OAuth/PKCE flows.
- Invalid credentials, expired tokens, wrong issuer/audience where enforced,
  unexpected algorithms/keys, malformed claims and redirect allowlist checks.
- Recovery/link replay, refresh replay and competing refreshes from separate
  application instances sharing one database. At most one consumption of a
  single-use credential may succeed; revocation must not resurrect sessions.
- User A attempting to access or mutate user B's state; privilege boundaries
  for anon, authenticated, service and internal database roles. Authentication
  must continue to leave authorization to PostgreSQL RLS.
- Role/claims cleanup after errors and cancellation, database rollback, email
  failure after commit, interrupted requests, bounded background work and clean
  shutdown. Check that token validation failures cannot silently become anon.
- Secret handling: runtime-generated fixture secrets, redacted logs, hashes
  instead of plaintext stored credentials, and documented key management.

Measure hashing, login, refresh and verification separately. Record latency,
memory, pool/admission usage and behavior under rate limits; preserve configured
password-hashing strength when measuring or optimizing.

## Completion criteria

- [x] A fresh external consumer builds and runs with the intended auth dependency
  set, without the excluded Nelcota capability crates.
- [x] Its database setup installs only the required auth/shared SQL objects,
  is idempotent and has a tested upgrade/compatibility story.
- [x] Existing Nelcota users, password hashes, tokens/sessions, roles and
  authentication interfaces remain compatible, or any required change has an
  explicit migration and compatibility test.
- [x] Public API, distribution constraints and versioning policy are documented;
  compilation tests exercise the consumer-facing API.
- [x] Real-PostgreSQL integration tests prove the security and concurrency
  boundaries above; mocks alone are insufficient for transaction or RLS claims.
- [x] Relevant Rust/SDK/panel checks and upgrade tests pass for affected behavior.
- [x] Performance evidence shows that reuse preserves resource budgets and the
  agreed baseline envelope defined in G02.
- [x] An integration guide and working example let another developer reproduce
  setup, usage, failures and shutdown without reading Nelcota's server assembly.

## Execution boundaries

Keep implementation in this repository while proving the contract. Use
incremental changes with regressions alongside fixes, following existing
module ownership and guard-clause conventions. Repository extraction and
package publication can follow after the consumer and compatibility evidence
justify an independent release lifecycle.

Save dependency diagrams, test results, packaging checks and before/after
measurements with the final implementation review.
