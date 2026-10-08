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
| `nelcota-cli`  | host with N projects (`init`, `up`, `projects`, `remove`, `migrate`, `backup`, `upgrade`, `panel-login`...). Modules: `host` (registry), `naming`, `scaffold`, `caddy`, `registry`, `panel_login`, `lifecycle`, `backup`, `upgrade` |
| `nelcota-server` | the `nelcota` binary: dispatches the CLI or starts the server (trace, timeout, CORS, gzip) |

Everything compiles into a single binary (~19 MB, static with musl in the
Docker image).

The query compiler separates the validated request representation (`query/mod.rs`),
URL parsing (`query/parse.rs`) and SQL generation (`query/sql.rs`), behind the
existing public interface. Panel read/write handlers are organized by feature in
`admin/src/api/`; DDL validation and execution remain separate modules.

Authentication's HTTP handlers adapt requests to `accounts` and `sessions`.
`db` owns auth-role transactions, `request` owns HTTP metadata, and recovery
uses the shared session interface directly. The panel's root assembles routes;
`auth`, `middleware`, `error`, `assets` and `state` own their respective behavior.

Storage's `upload` module owns streamed writes and their metadata commit,
`operations` owns reads/listing/deletion, and `signing` owns signed-file grants.
Their interface takes upload options and a byte stream and returns typed data.
The `http` adapter handles headers, response status, URLs and file serving for
both public routes and the panel. Policy checks still happen before bytes are
consumed, and failed writes still clean up their unpublished versions.

The CLI separates project lifecycle and version upgrades from backup workflows.
`backup/workflow` coordinates capture, retention and restore; `backup/remote`
owns S3 transport. Database and immutable file capture retain one owner so
their snapshot consistency remains intact.

Panel wire DTOs live in `admin/src/contracts.rs` and the existing structure/DDL
types. Their generated TypeScript and JSON Schemas are versioned with the UI;
standalone validators reject incompatible responses in the client. Row values
remain strings or NULL so decimal and bigint values retain precision.

The SQL editor streams messages over dedicated connections with a 30-second
statement timeout, four concurrent runs, 1000 retained rows per result, an 8 MiB
retained-value budget and 32 displayed results. It drains the entire batch after
reaching a display limit, preserving transaction semantics. Dropping the HTTP
future sends a Postgres cancellation request and closes its dedicated connection.
Ordinary API pool connections stay separate.

DDL commits once and reports `applied` plus `catalog_pending`. An introspection
failure after COMMIT does not turn a successful mutation into an error. Catalog
reloads are serialized, and one worker retries failed refreshes with backoff;
the client refreshes its view without replaying the change.

`RemoteResource` owns loading, errors, cancellation and result ordering in table,
user and storage pages. Storage's upload queue owns progress, partial failures
and conflict retries, capturing the bucket and folder before navigation changes.
These modules have unit tests; browser flows and real-Postgres regressions run in CI.
`TableRows` owns row selection, loading and mutation reconciliation, including
captured primary keys and navigation during writes. The table page owns its
navigation, dialogs and presentation; its HTTP adapter supplies row operations.

Recovery operations own link issuance and atomic consumption, password updates,
session revocation and post-commit email delivery. HTTP handlers only extract
request metadata, apply public endpoint rate limits and adapt their results.
Migration operations return typed records and commit version allocation,
checksums and change tracking together; their `files` module owns names,
rendering and directory discovery, while the root handles HTTP downloads.

The REST root composes routes. Its `http` adapter handles URLs, preferences,
status codes and headers; `operations` compiles and executes CRUD/RPC under
the caller's role, returning PostgreSQL-generated JSON and range metadata.
Catalog internals separate `model`, `introspection`, `refresh` and `listener`
without adding concepts to the public `CatalogHandle` interface.

The SQL executor owns connection lifetime, concurrency, result budgets and
cancellation behind `execute`; the route handles HTTP status and serialization.
The panel's `SqlExecution` owns request state and timing, and `StorageBrowser`
owns folder paging and refreshes after uploads/deletions. Both accept adapters
and discard stale responses when cancelled or navigated away from.

Native deployment separates machine/package provisioning, systemd units,
PostgreSQL configuration and atomic binary replacement. Caddy owns its own
configuration writes. PITR separates private configuration from process adapters
and restore sequencing; restore still coordinates stopping services, restoring,
waiting for promotion, taking a backup on the new timeline and starting the app.

## A request's flow

1. `Auth` (extractor) turns the `Authorization` header into `Claims`.
2. The operation takes a connection from the pool (always as `authenticator`).
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
