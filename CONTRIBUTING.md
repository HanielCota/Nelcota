# Contributing

Thanks for helping. Nelcota stays small on purpose: Postgres is the product,
authorization belongs to RLS, and everything ships as one binary. Read
[docs/decisions.md](docs/decisions.md) before proposing a larger change; an
issue first saves both of us time when a change touches one of those
decisions.

## Setup

Stable Rust and Docker (the integration tests start a real Postgres 17).

```sh
cargo run -- dev                      # Postgres in a container + server at :8000
cargo test                            # unit and integration tests
cargo clippy --all-targets -- -D warnings && cargo fmt --all --check
cargo deny check && cargo audit
cd crates/admin/ui && npm ci && npm run check && npm test   # panel
```

The panel build (`crates/admin/ui/dist`) is committed: after changing the
panel, run `npm run build` and commit `dist` in its own commit.

Without Docker, server integration tests can use an existing **disposable**
Postgres 17 cluster with `pg_stat_statements` preloaded. Set
`NELCOTA_TEST_DATABASE_URL` to its admin connection URL; each test creates an
isolated database and initializes cluster-wide roles. Never point this variable
at a development or production database. Docker-specific CLI and S3 tests still
require their respective services.

The upgrade regression requires Python 3.11+, Docker and Linux amd64. It
downloads checksum-pinned published binaries and creates one disposable
Postgres 17 cluster per run, because cluster-wide roles must start clean:

```sh
cargo build --locked -p nelcota-server
nice -n 19 ionice -c3 python3 scripts/test-upgrade.py  # 0.3.1 → this checkout
nice -n 19 ionice -c3 python3 scripts/test-upgrade.py --published-target  # 0.3.1 → 0.4.0
```

Use `sg docker -c 'nice -n 19 ionice -c3 python3 scripts/test-upgrade.py'` if
the current login has not picked up Docker group membership. The test compares
existing rows, exact numeric values, sequences, schema objects, RLS and disk
files, then exercises auth continuity, signed webhooks, rollback, retries,
restart and pre-upgrade backup restore. It removes only its own container;
diagnostics stay in `target/upgrade-tests/`. CI runs both scenarios, including
the current checkout so a published-release check cannot hide a PR regression.

## Pull requests

- One logical change per commit, with tests in the same commit as the code
  they cover. Messages follow [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat(storage): ...`, `fix(cli): ...`), imperative, with a body saying why.
- Code, comments, docs and commit messages are in English. Only the panel's
  interface is translated (pt-BR and en, through the i18n catalogs).
- Prefer guard clauses and early returns to nested `if`/`else` control flow;
  use `match` or `switch` for distinct cases. Rust `let ... else`, conditional
  value expressions and Svelte rendering branches remain appropriate.
- Security-sensitive code (auth, RLS, SQL building, storage) needs a test that
  proves the boundary holds, e.g. that one user cannot reach another's data.
- Migrations in `migrations/` are never edited once released: add a new one.
- CI must pass: fmt, clippy, tests (including S3), cargo-deny, audit and the
  panel checks.

## Reporting security issues

Privately, as described in [SECURITY.md](SECURITY.md).
