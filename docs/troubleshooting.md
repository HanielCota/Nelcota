# Troubleshooting

The usual surprises, what causes them and how to fix them. Errors are
`{"code", "message"}` JSON; the SDKs expose them as `error.code` /
`Error::code()`. Newer servers also send `sqlstate`, `details`, `hint` and
`constraint` for database errors.

## A query returns `[]`, but the rows exist

Row-level security filtered them. RLS never fails a read: rows the policies
do not allow simply are not there for this caller. Check:

- **Who is asking.** Without a session the request runs as `anon`; with one,
  as `authenticated` with `auth.uid()` = the user's id. A row owned by
  another user is invisible, as intended.
- **The policy.** `USING (owner = auth.uid())` only matches rows whose
  `owner` was set. Rows inserted from the SQL editor or a migration have the
  `owner` you gave them (or `NULL`, since `auth.uid()` is empty there).
- **Policies for the role.** A policy `TO authenticated` does not apply to
  `anon`. With RLS enabled and no policy for a role, that role sees nothing.

The same goes for `update` and `delete`: changing zero rows is a success, not
an error. To be sure a row changed, ask for it back
(`.update({...}).eq('id', id).select('id')`) and check the result.

A write that violates a policy's `WITH CHECK` *is* an error: 401/403
`db_error` with `new row violates row-level security policy` (`42501`).

## 401 or 403 with a valid token

The role has no **GRANT** on the table, so Postgres refuses before RLS is
even consulted: `db_error`, `permission denied for table notes (42501)`.
Visitors (`anon`) get 401 (signing in may help); signed-in users get 403.

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notes TO authenticated;
GRANT SELECT ON public.notes TO anon;   -- only if visitors may read it
```

Functions called through `rpc` need `GRANT EXECUTE`. Storage needs policies on
`storage.objects` ([storage.md](storage.md)).

A 401 `invalid_token` is different: the token is malformed, expired or
signed by another key (a token from another project, or from before
`nelcota dev` was reset). Sign in again; the SDKs refresh tokens on their
own while the refresh token is valid.

## `email_not_confirmed` on sign-in

The server has `NELCOTA_EMAIL_CONFIRMATION_URL` set, so a new account must
open the link sent by email before it can sign in with a password. Sign-up
answers with the user and no session while it waits. Send the link again
with `resendConfirmation(email)` (`resend_confirmation` in Rust). Locally,
`nelcota dev` leaves confirmation off.

## `redirect_not_allowed` when starting an OAuth sign-in

`redirectTo` must match one of the comma-separated
`NELCOTA_OAUTH_REDIRECT_URLS`: same scheme, host and port, and a path equal
to or under the listed one. `http://localhost:5173/auth/callback` and
`http://127.0.0.1:5173/auth/callback` are different origins. Restart the
server after changing the variable.

## Types do not match the database

`nelcota types` writes a snapshot of the schema. After a migration, run it
again (`nelcota types -o src/database.ts`, or `--lang rust`), otherwise new
columns are unknown to the compiler and removed ones still compile, failing
only at run time with `invalid_query` or `db_error`. Running it in CI and
failing on a diff catches stale files.

The server itself reloads its catalog on every DDL. On a managed Postgres
without the event trigger, reload it by hand: `NOTIFY nelcota, 'reload schema';`.

## `service_role` refused in the browser

`@nelcota/client` throws `NelcotaUsageError: A service_role token bypasses
RLS and must never reach a browser` when it sees such a token in a page. That
token skips every policy, so anyone who opens the developer tools would own
the database. Use it only on a server (for example from
`nelcota token service-role --days 30`); in the browser, sign users in and
let RLS decide.

## Only 1000 rows come back

Each read returns at most `NELCOTA_MAX_ROWS` rows (1000 by default), even
without a `limit` and even if the `limit` is larger. Page through the rest:

- TypeScript: `range(from, to)` or `limit` + `offset`; the result's `range`
  and `count` (with `count: 'exact'`) tell whether more rows exist, and
  `pages(size)` walks every row.
- Rust: `range(start, end)` or `limit` + `offset`, with `count_exact()`.
- HTTP: `Content-Range: 0-999/*` (or `/total` with `Prefer: count=exact`).

Order by a unique column when paging. Raising the cap makes every large read
cost more memory; responses also have an 8 MiB budget
(`413 response_too_large`).

## 429 `rate_limited`

Auth endpoints (sign-up, sign-in and refresh, email links, OAuth) allow
`NELCOTA_AUTH_RATE_LIMIT_PER_MINUTE` requests per minute per client IP (30 by
default); password sign-in is also limited per email. Many clients behind one
NAT, or a test suite signing in repeatedly, share that budget. The response carries `Retry-After`; the SDKs expose it as
`error.retryAfter` (seconds) / `Error::retry_after()`. Behind a reverse proxy,
set `NELCOTA_TRUST_PROXY=true` only if the proxy sets `X-Forwarded-For`;
otherwise every visitor shares the proxy's IP and its budget.

A 503 `unavailable` means the database could not be reached or is
overloaded; it may also carry `Retry-After`. The SDKs retry reads on 429 and
503 by themselves; writes are never retried automatically.
