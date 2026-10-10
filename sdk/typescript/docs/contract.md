# Client contract

`@nelcota/client` 0.1.x targets the Nelcota HTTP protocol implemented by this
repository. The package has its own version; server compatibility is determined
by the endpoints and features below, not by matching package version numbers.

## Interface

| Module | Methods | HTTP contract |
|---|---|---|
| REST | `from().select()`, filters, order, limit, range, single, maybeSingle | `GET/HEAD /rest/v1/{table}`; encoded query; optional `Prefer: count=exact`; `Content-Range` |
| REST writes | insert, update, delete, upsert, optional select | `POST/PATCH/DELETE /rest/v1/{table}`; JSON body; `Prefer: return=minimal/representation`; upsert resolution and `on_conflict` |
| RPC | `rpc(name, args)` | `POST /rest/v1/rpc/{name}`; named JSON arguments; caller's role |
| Auth | signUp | `POST /auth/v1/signup`; session or `{user}` awaiting confirmation |
| Auth | password login, refresh, exchangeCode | `POST /auth/v1/token?grant_type=password/refresh_token/pkce` |
| Auth | getUser, signOut | `GET /auth/v1/user`, `POST /auth/v1/logout` |
| Auth links | recover, magic link, resend, verify/reset | `POST /auth/v1/recover`, `/magiclink`, `/resend`, `/verify` |
| OAuth | signInWithOAuth, handleRedirect | `GET /auth/v1/authorize`; S256 PKCE; callback `?code=` exchanged once |
| Storage | upload, replace, open/download, remove | `POST/PUT/GET/DELETE /storage/v1/object/{bucket}/{path}`; raw bytes |
| Storage | list, signed URL, public URL | `POST /storage/v1/object/list/{bucket}`, `/object/sign/{bucket}/{path}`; `/object/public/{bucket}/{path}` |
| Buckets | list, get, create, update, delete | `/storage/v1/bucket` and `/storage/v1/bucket/{id}` |

`schema` selects the generated TypeScript schema; the server's configured
exposed schema determines the HTTP resource. It does not switch database schemas
at runtime. RLS and GRANTs always remain server/Postgres decisions.

## Results and failures

- Auth and storage return `{data,error}`. REST adds `status` and `count`; RPC
  adds `status`. Writes without `.select()` and HEAD responses return null.
  HEAD is typed as null, including chained selects. `single`/`maybeSingle`
  require a row representation and refuse minimal writes or HEAD before HTTP.
  A requested row representation that is empty or not a JSON array returns
  `invalid_response`; it never succeeds with null or another shape.
- HTTP failures preserve server `code`, `message` and status. Network, abort,
  timeout and interrupted buffered bodies return status 0 and client codes.
- Invalid JSON returns `invalid_response` with the response's HTTP status.
- Invalid developer configuration and unsafe identifiers/paths throw
  `NelcotaUsageError`. Errors thrown by user-supplied token/storage/fetch
  configuration outside the HTTP operation remain the caller's responsibility.
- GET and HEAD retry network failures, body-read failures, 429 and 503; mutations,
  auth token exchange, storage listing and RPC are not automatically retried.
- Timeout covers each attempt and its buffered body, excluding session refresh
  and the wait between retries. Retry-After up to 30 seconds is respected;
  longer delays return the error immediately with `retryAfter` for the caller.
- `storage.open()` hands the raw response to the caller. Subsequent stream
  errors must be handled by the caller; `download()` buffers into a Blob and
  returns body failures in its result.

## Sessions and lifecycle

Each client owns its session adapter. The default is localStorage in browsers
and a separate in-memory adapter elsewhere. A custom adapter can be sync or
async. Instantiate a server client per request or supply a per-request token.
The default storage key includes the host and base path, so projects under
different paths cannot overwrite each other's sessions in a shared adapter.

Refresh calls on one auth client share a promise. Clients sharing an adapter
and storage key are serialized with Web Locks, with a module-local queue as a
fallback. Only Web Locks coordinate separate browser tabs. If a browser lacks
Web Locks, use per-tab sessionStorage or an adapter with external coordination;
the module-local queue does not guarantee cross-tab rotation safety.

Auth methods that perform HTTP accept `{signal,timeout}` as the last argument.
Concurrent refresh callers share the first caller's timeout. Signals cancel
each caller's wait independently; rotation continues and persists its result
even when a waiter cancels. An already aborted caller does not start rotation.
REST/storage signals also cover waiting for the token source or session
refresh, without aborting an operation shared by other callers. `signOut()` waits
for a pending refresh and clears locally even when server revocation fails.
`dispose()` stops timers, closes channels and clears listeners; it neither
revokes the session nor aborts requests already in flight.

An explicit `accessToken` controls REST/storage requests and disables automatic
session refresh in the combined client. Auth methods remain available and may
store a session when called explicitly.

## Bucket settings

Creation accepts `BucketSettings` with optional fields. Updating uses
`BucketUpdateSettings`, requires `public`, `file_size_limit` and
`allowed_mime_types`, and replaces the entire settings object with PUT.
Use explicit null to clear a limit. Missing fields throw `NelcotaUsageError`
before any HTTP request, including in JavaScript consumers.

## Release acceptance

Run `check`, unit/type tests, `build`, `size`, `test:package`, runtime smoke tests
and `test:contract -- --browser`. CI tests Node 22/24/26, Bun, Deno and
Chromium/Firefox/WebKit. Edge hosts may consume the Web API implementation but
are not included in this compatibility guarantee until tested on that host.
