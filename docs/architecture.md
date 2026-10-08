# Architecture

```
client ──HTTPS──▶ Caddy ──▶ nelcota (single binary) ──▶ Postgres 17
                              │ axum: /health, /rest/v1/*, /storage/v1/*
                              │ auth: validates JWT → Claims
                              │ core: pool (authenticator) + transaction with role/claims
```

## Crates

| Crate | Responsibility |
|---|---|
| `nelcota-core` | config (`figment`), HTTP errors, `Claims`/`Role`, pool, migrations, `begin_request` |
| `nelcota-auth` | `JwtVerifier` trait, EdDSA/HS256 keys and JWKS, `Auth` extractor, signup/login/refresh/logout, password recovery, argon2id, rate limit |
| `nelcota-api`  | catalog introspection, SQL builder, CRUD/RPC, OpenAPI, TS types |
| `nelcota-storage` | files: buckets and objects under RLS (`storage` schema), bytes on disk or S3 (`object_store`), signed URLs, orphan collector |
| `nelcota-admin` | panel at `/admin`: JSON API (`/admin/api`) + embedded Svelte SPA (`ui/dist`) |
| `nelcota-cli`  | host with N projects (`init`, `up`, `projects`, `remove`, `migrate`, `backup`, `upgrade`, `panel-login`...). Modules: `host` (registry), `naming`, `scaffold`, `caddy`, `registry`, `panel_login`, `ops` |
| `nelcota-server` | the `nelcota` binary: dispatches the CLI or starts the server (trace, timeout, CORS, gzip) |

Everything compiles into a single binary (~19 MB, static with musl in the
Docker image).

## A request's flow

1. `Auth` (extractor) turns the `Authorization` header into `Claims`.
2. The handler takes a connection from the pool (always as `authenticator`).
3. `db::begin_request` opens the transaction and sets role + claims.
4. The query runs under RLS; Postgres builds the JSON (`json_agg`) and the API
   just passes the bytes through.
5. COMMIT. Postgres errors are translated by `ApiError::from_db`.

`db::begin_request` is the **only** path to run SQL on behalf of a user.

## Startup

1. Loads the config (`NELCOTA_*` + optional `nelcota.toml`).
2. `db::bootstrap` (admin connection): advisory lock → migrations in
   `migrations/` (control table `nelcota.schema_migrations`) → `authenticator`
   password (SCRAM verifier computed locally).
3. Creates the API pool and starts the HTTP server. Shuts down cleanly on
   SIGTERM/Ctrl+C.

## Routes

| Prefix | Served by | Authorization |
|---|---|---|
| `/health` | server | public |
| `/rest/v1/*` | api | JWT → role → GRANTs + RLS |
| `/auth/v1/*` | auth | public (signup/login/recovery) or the user's JWT |
| `/storage/v1/*` | storage | JWT → role → RLS on `storage.objects`; public buckets and signed URLs without a token |
| `/admin/*` | admin | panel login (session), admin connection |

## Internal migrations

Plain SQL in `migrations/V<n>__<name>.sql`, applied by `refinery` and recorded
in `nelcota.schema_migrations` (version, name, date, checksum). They can be
applied by hand with `psql` if Nelcota no longer exists. The early ones
(V1–V6) keep their original Portuguese names and comments: refinery checksums
name and content, so editing them would break existing installs (D68).
