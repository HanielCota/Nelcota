# nelcota-core

Shared HTTP errors, claims, configuration and PostgreSQL transaction primitives
for Nelcota capability crates. Request role and JWT claims are transaction-local:
commit, rollback and cancellation do not authorize the next pool borrower.

`db::bootstrap` installs the full canonical schema. Auth consumers should call
`nelcota_auth::bootstrap` instead. Bootstrap requires a PostgreSQL administrator;
request pools connect as `authenticator`, never as the administrator.

The packaged `migrations/` directory mirrors the repository's canonical SQL
byte for byte. Run `python3 scripts/check-boundaries.py` before packaging.
Released migrations and their refinery checksums must remain unchanged.

No Node runtime is required. This crate uses PostgreSQL 17 and Axum 0.8.
See the repository integration guide for packaging and compatibility guarantees.
