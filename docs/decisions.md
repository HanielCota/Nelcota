# Decision log

Decisions made while building Nelcota, with the why. Newest at the end.

## Milestone 1

**D1. Apache-2.0 license.** Chosen by the maintainer (2026-10-06).

**D2. Name and binary.** The project is called Nelcota; the single binary is
`nelcota` (crate `nelcota-server`). In Milestone 4 it gains subcommands
(`nelcota init`, `nelcota up`...); without arguments it starts the server.

**D3. The `admin` and `cli` crates only arrive in Milestones 4/5.** Avoids
empty crates.

**D4. Integration tests in `crates/server/tests/`.** The workspace is virtual
(there is no package at the root), and Cargo integration tests must belong to a
package. `server` exposes the `Router` in a lib so the tests use the same code
as production.

**D5. A single URL.** `NELCOTA_DATABASE_URL` points to a role that owns the
schema (the `postgres` superuser in Docker) and is used only for migrations and
to set the `authenticator` password. The API uses the same host/database,
swapping user/password for `authenticator` / `NELCOTA_AUTHENTICATOR_PASSWORD`.

**D6. The `authenticator` password stays out of migrations.** Migrations hold
no secrets. At startup the server computes the SCRAM-SHA-256 verifier on the
client side and runs `ALTER ROLE ... PASSWORD 'SCRAM-SHA-256$...'`: the plain
password never travels over the wire nor shows up in Postgres' statement log.

**D7. `set_config('role', $1, true)` instead of `SET LOCAL ROLE x`.** Both are
equivalent (`SET LOCAL ROLE` is sugar for `set_config('role', ..., true)`), but
`set_config` takes a parameter, sets role and claims in a single round trip and
avoids any SQL built as a string. The role value comes from a closed `enum`.

**D8. Pool with `RecyclingMethod::Fast`.** Role and claims are
transaction-scoped and die on COMMIT/ROLLBACK (including a drop without commit,
which sends ROLLBACK). No `DISCARD ALL` is needed on each checkout. Covered by
the pool-leak and rollback tests in `crates/server/tests/rls.rs`.

**D9. Internal migrations in `nelcota.schema_migrations`.** A schema of its
own, without GRANTs to the API roles, so it does not show up in `public`
(which the automatic API exposes). An advisory lock serializes bootstrap
across instances.

**D10. The example table stays out of migrations.** `examples/todos.sql` is
applied by the tests and optionally in dev. Nelcota's migrations create no
business tables in anyone's database.

**D11. Extensions in the `extensions` schema.** `pgcrypto` and
`pg_stat_statements` stay out of `public`. `pg_stat_statements` requires
`shared_preload_libraries=pg_stat_statements` (already in the dev compose and
in the tests).

**D12. Provisional HS256 JWT** (superseded by D20). Milestone 1 uses HS256
with a shared secret (≥ 32 characters) behind the `JwtVerifier` trait.
Milestone 2 adds EdDSA + JWKS as the default, and HS256 stays as a simple mode.

**D13. Claim rules.** `role` is required and must be `anon`, `authenticated`
or `service_role`. `authenticated` requires a uuid `sub` (so `auth.uid()` never
fails the cast). `aud` is not verified yet (it joins the contract in Milestone
2). 30 s leeway on `exp`.

**D14. No token = `anon`; invalid token = 401.** An `Authorization` header
that is present but invalid (expired, wrong signature, scheme other than
`Bearer`, unknown role) never silently becomes `anon`.

**D15. 401 vs 403 on missing permission.** `insufficient_privilege` (42501)
becomes 401 for `anon` (needs to sign in) and 403 for the other roles, as in
PostgREST.

**D16. `rsa` ignored in `cargo audit`/`cargo deny` (RUSTSEC-2023-0071).** It
comes through `jsonwebtoken`'s `rust_crypto` backend. The Marvin Attack affects
operations with the RSA **private** key; Nelcota never signs with RSA. The
`aws_lc_rs` backend would avoid the dependency, but requires a C/CMake
toolchain in the build, which hurts the "simple static binary". Revisit if
RS256 is ever used for signing.

**D17. Libraries outside the prompt's table.**
- `uuid`: validates the `sub` claim and is Postgres' native type for ids.
- `postgres-protocol`: already a transitive dependency of `tokio-postgres`;
  used to compute the SCRAM verifier (D6).
- `http-body-util` (dev only): reads response bodies in tests.

**D18. Postgres without TLS on the internal network.** The database is only
reachable over Docker's internal network (or localhost in dev). TLS to the
database waits for a remote-database scenario.

**D19. Edition 2024, MSRV 1.88.** The current `testcontainers-modules` already
requires 1.88.

## Milestone 2

**D20. EdDSA (Ed25519) as the default; HS256 as legacy.** The private key comes
from `NELCOTA_JWT_PRIVATE_KEY` (PKCS#8 PEM or the one-line base64, which fits
in a `.env`). With an EdDSA key **and** an HS256 secret configured, the server
signs with EdDSA and accepts both when verifying (zero-downtime migration). The
`kid` is the RFC 7638 thumbprint.

**D21. Internal `nelcota_auth` role for the `auth` schema.** Auth handlers run
with `SET LOCAL ROLE nelcota_auth` (a fixed string). No JWT can assume it (the
`Role` enum does not contain it), and the API roles have no GRANT on `auth.*`.
Discarded alternative: `SECURITY DEFINER` functions, which would spread logic
into PL/pgSQL with no real gain.

**D22. 32-byte opaque refresh token, stored only as SHA-256.** SHA-256 (not
argon2) because the token already has 256 bits of entropy; lookup is through a
unique index, with no comparison in the application.

**D23. Reuse detection revokes the session (family), not the user.** Other
sessions stay valid. No grace window for now (documented).

**D24. Logout revokes the session; the access JWT is valid until `exp`.**
Access tokens are stateless and short (15 min by default). Checking revocation
on every request would cost an extra database round trip.

**D25. Argon2id with RustCrypto's default parameters (m=19 MiB, t=2, p=1).**
Runs in `spawn_blocking`, with at most `min(cores, 4)` concurrent hashes (peak
< 80 MiB). Logging in with an unknown email verifies against a dummy hash, so
the response time does not reveal whether the account exists.

**D26. In-memory rate limit, fixed 1-minute window** (the window became a GCRA in D69), per IP (signup/token)
and per email (login). One binary per install makes Redis unnecessary. The IP
comes from `X-Forwarded-For` (rightmost entry) only with
`NELCOTA_TRUST_PROXY=true`.

**D27. Signing up with a duplicate email answers 409.** Without email
confirmation in the MVP, hiding the account's existence at signup would bring
no real protection (login still does not reveal it).

**D28. New libraries.** `argon2` (it was in the table); `sha2`, `base64` and
`getrandom` were already transitive dependencies (`jsonwebtoken`/RustCrypto)
and are used directly at the same versions, without growing the tree. The ISC
license is allowed in `cargo deny` (it comes from `simple_asn1`, used to read
PEM).

## Milestone 3

**D29. Dynamic SQL with our own builder, no `sea-query`.** Every URL value
becomes a **text** parameter, and Postgres converts it to the column type
(`$1::text::<catalog type>`); inserts and updates use
`json_populate_recordset`/`json_populate_record`, and RPC uses `json_to_record`.
That way Postgres validates types as it would a literal, and the application
does not need to map types. `sea-query` binds each value to a Rust type (and
`sea-query-postgres` would require that mapping), besides not expressing these
patterns well. The builder is ~400 lines with three auditable rules:
identifiers only from the catalog and always quoted; cast types only from the
catalog; values always parameters. Unit and integration tests cover injection
on both sides.

**D30. OpenAPI built with `serde_json`, no `utoipa`.** `utoipa` generates the
specification at compile time from Rust types; ours comes from the catalog at
run time. The document is filtered by the request role's privileges (like
PostgREST's `follow-privileges` mode).

**D31. Catalog reload through an event trigger + `LISTEN nelcota`.** The
trigger runs on `ddl_command_end` and `sql_drop` (including GRANT/REVOKE). The
listener batches bursts (100 ms), reconnects with backoff and reloads on every
reconnect. Without a superuser, the migration proceeds without the trigger and
reloading is manual (`NOTIFY`).

**D32. PATCH/DELETE require a filter.** Unlike PostgREST (which allows it and
lets RLS limit it), we refuse with 400. The cost is an explicit filter when
the intent really is to hit everything.

**D33. Batches with different keys respect DEFAULT.** Rows are grouped by
column set and each group becomes an INSERT (CTEs in one statement). In
PostgREST, without `Prefer: missing=default`, missing columns become NULL; we
find DEFAULT the least surprising behaviour.

**D34. `statement_timeout` on the `authenticator` role.** `ALTER ROLE anon SET
...` has no effect with `SET ROLE` (Postgres only applies a role's settings at
login). Bootstrap applies `NELCOTA_STATEMENT_TIMEOUT_SECS` (default 10 s) to
`authenticator`, and the error becomes 504.

**D35. Postgres' default kept for `EXECUTE` on functions (PUBLIC).** Changing
the default privileges would hide plain Postgres behaviour; the documentation
and the panel warn about it.

**D36. New libraries.** `form_urlencoded`, `futures-util` and `bytes` were
already transitive dependencies (axum/tokio-postgres); tower-http's
`compression-gzip` adds `flate2`.

## Milestone 4

**D37. One binary, two roles.** `nelcota` without arguments (or `serve`)
starts the server; the subcommands operate the install. On a host,
`migrate`/`types`/`token` are forwarded to the `app` container (`docker compose
exec`), which is what can reach Postgres on an `internal: true` network; with
`NELCOTA_DATABASE_URL` in the environment, they run directly.

**D38. Postgres profiles become `-c key=value` in the compose file.** The
`deploy/postgres/profiles/*.conf` files are the source; `init` turns them into
arguments of the `postgres` command. Using `config_file` would replace the
image's whole `postgresql.conf` (and change the default `pg_hba.conf`
directory).

**D39. Backups with `pg_dump -Fc`; S3 uploads with the `amazon/aws-cli`
image.** Nothing to install on the host and no home-grown S3 signing (SigV4).
Restore in a single transaction (`--single-transaction --exit-on-error`). PITR
with WAL-G is pending (daily dumps: an RPO of up to 24 h).

**D40. Upgrade with rollback = previous image + restore of the pre-upgrade
backup.** The new version may have applied internal migrations the previous
one does not know (refinery refuses to start with an "unknown" migration).
That is why the rollback also restores the database. The acceptance test
covers this path.

**D41. `scratch` image with a musl binary; built-in healthcheck.** No shell or
curl in the image: `nelcota healthcheck` performs `GET /health` over plain TCP.

**D42. User migrations in refinery's format (`V<n>__<name>.sql`)**, in
`nelcota.user_migrations`, separate from the internal ones
(`nelcota.schema_migrations`). CRLF is normalized before the checksum (a
Windows checkout does not diverge).

**D43. `install.sh` installs Docker with the official script if it is
missing** (as root and without `NELCOTA_SKIP_DOCKER=1`): that is what makes
the "3 commands" possible on a fresh VPS. The binary's SHA-256 checksum is
always verified.

**D44. `init` does not enable the firewall without `--firewall`.** Enabling
`ufw` remotely with the wrong SSH port locks the owner out of the machine. By
default it only recommends it.

## Milestone 5

**D45. Panel with server-side HTML + ~2 KB of our own JS, no HTMX**
(superseded by D50). Pages are ordinary forms; the only interactive part (the
SQL editor) fits in a few lines of `fetch`. CSP `script-src 'self'` without
`unsafe-inline`. Assets embedded with `rust-embed`.

**D46. Panel login separate from end users.** Email + argon2id hash in the
`.env` (generated by `init`; the password is shown once). In-memory sessions
(restarting the server signs the admin out), `HttpOnly; SameSite=Strict`
cookie (`Secure` behind the proxy), login rate limit and an
`Origin`/`Sec-Fetch-Site` check on every POST (CSRF).

**D47. The panel uses the admin connection.** The admin owns the database, as
in `psql`. Table writes reuse the API's SQL builder (catalog identifiers,
parameterized values). The SQL editor opens a **new** connection per run:
`BEGIN` without `COMMIT` or `SET ROLE` do not contaminate the pool. The SQL
text does not go to the log.

**D48. Panel values keep Postgres' text** (`RawValue`), without going through
`f64`: `numeric(30,10)` shows and is edited with every digit.

## Panel design

**D49 (superseded by D50).** A panel with a database-console look (dark by
default, emerald accent), inspired by Supabase but with its own name, brand
and icons. Inter + JetBrains Mono typography, embedded in the binary (~88 KB,
OFL, see `crates/admin/assets/FONTS.md`): no Google Fonts, to keep the CSP at
`'self'` and work offline. Light mode via `prefers-color-scheme`. Hand-drawn
SVG icons (no icon package). On the overview, the row count is exact for
tables under 10 thousand estimated rows and uses the planner's estimate (`≈`)
for large ones.

## Panel in Svelte

**D50. Panel in Svelte 5 + shadcn-svelte + Tailwind v4 + CodeMirror 6**,
built with Vite (the maintainer's choice, 2026-10-06). The goal is the
interactivity server-side HTML did not deliver well: inline editing in the
grid, side sheets, multiple selection, column sorting, a SQL editor with
syntax highlighting and autocomplete for the database's tables and columns. An
SPA with Vite, **no SvelteKit**: it injects inline script into the page, which
would break the CSP `script-src 'self'`. Our own routing (~60 lines, history
API).

**D51. The build (`crates/admin/ui/dist`) is versioned in git** and embedded
with `rust-embed`. That way `cargo build`, `cargo install` and the Dockerfile
still need no Node: only whoever works on the panel needs Node. CI's
`admin-ui` job runs `npm ci`, `npm audit`, `svelte-check`, `vite build` and
fails if the versioned `dist` is out of date.

**D52. CSP: `style-src` now accepts `'unsafe-inline'`; `script-src` stays at
`'self'` only.** Svelte transitions, CodeMirror and menu positioning
(floating-ui) inject styles at run time. Inline style runs no code; inline
scripts remain forbidden (there is a test).

**D53. The panel becomes a JSON API at `/admin/api/*`.** Same session, same
origin check (CSRF) and the same SQL builder as the REST API. Values travel as
Postgres' exact text (`RawValue`). Batch deletes in a single transaction. The
SPA is served at any `/admin/*` (fallback to `index.html`). Hashed assets get
immutable caching; `index.html`, `no-cache`.

**D54. CodeMirror loaded on demand.** The main bundle stays around 130 KB
gzip; the SQL editor (~145 KB gzip) is only downloaded when its page opens.
The Inter and JetBrains Mono fonts come from `@fontsource-variable` (OFL),
served by the binary itself.

**D55. A panel without ornaments.** Removed the generic template patterns:
stat cards with icons, gradients, a glowing login, a logo in a colored square,
pills and dots on every status, avatars with initials, skeletons, colored
toasts, an icon before every title, subtitles explaining the obvious and the
"Postgres 17" pill (which was hardcoded, not read from the database).
Criterion: color only for problems (no RLS, RLS without policies, error,
destructive action); short texts. Inter/JetBrains Mono swapped for IBM Plex
Sans/Mono, less ubiquitous and with the feel of a technical tool. Security
warnings stay, as plain text.

## Several projects per host

**D56. One host, N isolated projects.** A shared Caddy (the only one to
publish 80/443) and, per project, an app and a Postgres on an exclusive
internal network. A single project is a host with one project: there are not
two models to maintain. The registry (`nelcota-host.json`) is the source of
truth; the Caddyfile and the public list (`shared/projects.json`) are
generated from it.

**D57. One Postgres per project.** Postgres roles apply to the whole server;
in a shared Postgres, projects would share `anon`, `authenticated`,
`service_role` and the `authenticator` password. The cost (~50 MB of RAM per
project) buys physical isolation, per-project backup/restore and leaving, and
independent upgrades.

**D58. Own domain or a subdomain of the base domain.** `init api.shop.com`
derives the name "shop"; `init --project blog` uses `blog.<base-domain>`.
Locally: `<project>.localhost` (browsers resolve `*.localhost` to the
machine).

**D59. Single sign-on through an SSO handoff, with a per-project login
option.** Cookies do not cross domains, so the origin panel issues an HS256
JWT (host's shared secret, `aud` = target project, 60 s, single-use `jti`)
that travels in the URL fragment and is traded for a session at the target.
In per-project mode there is no shared secret and the handoff does not exist.
Only `panel_login.rs` writes admin credentials into the `.env` files;
switching modes regenerates and restarts the apps.

**D60. The project list is mounted as a folder and reread on every request.**
The app receives `shared/` read-only (a folder bind, not a file bind, so the
CLI's atomic rename is visible). New projects show up in the switcher without
restarting anyone. Each project's state comes from `GET /health` over the
`nelcota_edge` network, which now includes the version.

**D61. Removal always with a final backup** in `archive/` before deleting
containers and volumes; the site leaves Caddy with `caddy reload` (without
taking the others down).

## Identity and account in the panel

**D62. The mascot is the panel's identity, the only exception to D55.** It
comes from the NelcotaScreenShare atlas (`src/assets/mascot/`), cropped with
the poses anchored at the feet and right arm so the figure does not jump when
the expression changes. It is not decoration because it reacts to the login
screen's state: it waves on open, follows the pointer with its eyes, closes
its eyes while the password is typed and looks sad after a refused login. The
eyes are a vector layer over the PNG's (only the pupil moves); with
`prefers-reduced-motion` the gaze jumps straight to the target.

**D63. Full-height sidebar.** Project at the top (same height and border as
the topbar), pages in the middle, account at the bottom; the topbar covers
only the content area. Pinned, it takes 240px from `xl` up; collapsed, it
becomes an icon rail that expands over the content. "Projects" left the menu:
the project switcher already leads there.

**D64. The admin photo lives in the project's database, in the `nelcota`
schema** (migration V4, `nelcota.admin_avatar`). The admin login only exists
in environment variables, so there was nowhere to keep data about them; the
internal schema has no USAGE for any API role. The photo is per project (each
one has its own Postgres). The browser center-crops and scales it to 256px in
WebP; the server only accepts PNG, JPEG or WebP recognized by their leading
bytes, up to 256 KB. The URL carries the version (`?v=`), so the image can be
cached.

**D65. `nelcota-core` recompiles when `migrations/` changes.**
`embed_migrations!` reads the folder without telling the compiler; a
`build.rs` with `rerun-if-changed` avoids incremental builds with the old
list.

**D66. Password recovery by email, with the project's SMTP.** Nelcota runs no
mail server: `NELCOTA_SMTP_URL`, `NELCOTA_SMTP_FROM` and
`NELCOTA_PASSWORD_RECOVERY_URL` (all three or none; half-configured, the
server does not start). The link goes to the app's page with the token in the
fragment; tokens live in `auth.one_time_tokens` (migration V5) only as
SHA-256, are valid for 1 hour and once, with at most one email per minute per
account. Sending happens in the background so the response time does not
reveal whether the account exists. Using the link changes the password, ends
the sessions and marks the email as confirmed. Sent with `lettre` over rustls
+ ring, with Mozilla's roots embedded (`webpki-roots`): the `scratch` image has
no system certificates and the musl build stays free of OpenSSL. This brought
two permissive licenses into `deny.toml`: CDLA-Permissive-2.0 (the root list,
which is data) and 0BSD (`quoted_printable`).

**D67. Panel DDL becomes a migration, registered as already applied.** Every
DDL the panel applies goes through `apply.rs` and is recorded in
`nelcota.panel_changes` (migration V6), in the same transaction. "Generate
migration" bundles the pending ones into a file and inserts the matching row
into `nelcota.user_migrations` with refinery's checksum
(`Migration::unapplied(..).checksum()`): `migrate` does not reapply the file
and, with `abort_divergent`, refuses an edited copy. The generated text is
stored (`nelcota.panel_migrations`) so it downloads again byte for byte. The
version goes past the highest number in the database, in the folder and among
earlier exports; the folder comes from `NELCOTA_MIGRATIONS_DIR` or from the
`/migrations` mount in the container. The SQL editor is left out: it can run
anything, including DML, and separating schema changes would require parsing
the SQL.

**D68. English codebase, translated panel.** Code, identifiers, comments,
tests, server messages (REST, auth and admin API errors), CLI output, the
recovery email, the README and these docs are English. Only the panel
interface is translated: Portuguese (pt-BR) and English, through typed
per-area catalogs (`crates/admin/ui/src/lib/i18n/messages`) where the English
object is typed against the Portuguese one, so a missing key fails the type
check. The panel picks the browser language on the first visit and remembers
the choice made in the account menu. Admin API errors carry `code` and
`params` (`{"error", "code", "params"}`), so the panel shows them in the
chosen language and falls back to the server's English text (raw Postgres
errors, for instance). Migrations V1–V6 keep their Portuguese file names and
comments: refinery checksums name and content, and editing them would make
every existing install refuse to start.

**D69. Libraries instead of four hand-written pieces.** A review of what
the code reimplemented kept the deliberate ones (the SQL builder of D29, the
OpenAPI of D30, the PostgREST-style parser, TypeScript generation, CSV,
secret generation, image sniffing) and replaced four:
- **Rate limit:** `governor`'s keyed GCRA instead of a fixed window. The budget
  refills continuously, so a client can no longer spend almost twice the limit
  across the turn of a minute, and `Retry-After` is the real wait. Still in
  memory per process; only the `std` and `dashmap` features.
- **Session cookie:** the `cookie` crate parses the `Cookie` header and builds
  `Set-Cookie` in one place, instead of `split(';')` and `format!`. No signing
  or encryption features: the value is an opaque token.
- **Health probe:** `hyper-util`'s client instead of a hand-written HTTP/1.1
  request and response split; a chunked body no longer breaks it. hyper was
  already in the tree through axum.
- **Identifier quoting:** `postgres_protocol::escape::escape_identifier`, already
  a dependency. String literals in generated DDL keep the hand-written `''`
  doubling: `escape_literal` would emit ` E'...'` for backslashes and change the
  text of panel-generated migrations.

**D70. `or=`/`and=` as PostgREST logic trees.** The request's filters are
a tree (`Condition`): leaves are the existing filters and groups join their
items with OR/AND, optionally negated; the top level stays ANDed, so existing
URLs mean the same. Each group renders as a parenthesized expression, so
precedence never depends on its surroundings, and leaves keep the D29 rules
(catalog columns, quoted identifiers, values as parameters). Depth is capped at
8 and a tree at 100 filters, which bounds the recursive parser on hostile
input. As in PostgREST, `or`/`and` are reserved query keys.

### Known pending items

- Relation embedding and upsert wait until after the MVP.
- PITR with WAL-G (or pgBackRest) archiving WAL to S3.
- Install without Docker (systemd): the binary no longer depends on Docker;
  `init` still has to generate the units.
- The release workflow (musl binaries + image on GHCR) is written, but only
  runs once the repository is on GitHub; `install.sh` depends on it.
