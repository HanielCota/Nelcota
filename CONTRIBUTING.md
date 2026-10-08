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

## Pull requests

- One logical change per commit, with tests in the same commit as the code
  they cover. Messages follow [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat(storage): ...`, `fix(cli): ...`), imperative, with a body saying why.
- Code, comments, docs and commit messages are in English. Only the panel's
  interface is translated (pt-BR and en, through the i18n catalogs).
- Security-sensitive code (auth, RLS, SQL building, storage) needs a test that
  proves the boundary holds, e.g. that one user cannot reach another's data.
- Migrations in `migrations/` are never edited once released: add a new one.
- CI must pass: fmt, clippy, tests (including S3), cargo-deny, audit and the
  panel checks.

## Reporting security issues

Privately, as described in [SECURITY.md](SECURITY.md).
