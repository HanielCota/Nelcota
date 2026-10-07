# The `auth` schema

A public, stable contract. Every table is ordinary SQL: you can read, export
and migrate it without Nelcota (see [leaving.md](leaving.md)).

The tables are **not** exposed by the REST API. The server reaches them with
the internal role `nelcota_auth`, which no JWT can assume. `anon`,
`authenticated` and `service_role` have no GRANT on them (there is a test for
that).

## `auth.users`

| Column | Type | Notes |
|---|---|---|
| `id` | `uuid` PK | `gen_random_uuid()`; it is the JWT `sub` and `auth.uid()` |
| `email` | `text` unique | always lowercase, ≤ 254 characters |
| `encrypted_password` | `text` | **argon2id PHC string** (see below); `NULL` = no password |
| `email_confirmed_at` | `timestamptz` | set when the person uses a recovery link (proves they receive the email); confirmation at signup is still out of the MVP |
| `raw_user_meta_data` | `jsonb` | the signup `data` field; returned as `user_metadata` |
| `created_at`, `updated_at` | `timestamptz` | |
| `last_sign_in_at` | `timestamptz` | updated on every login |

### Password format

```
$argon2id$v=19$m=19456,t=2,p=1$<salt base64>$<hash base64>
```

It is the standard PHC format. The parameters (19 MiB, 2 iterations,
parallelism 1) follow the OWASP recommendation. It works directly with any
argon2 library: Python `argon2-cffi` (`PasswordHasher().verify(hash,
password)`), Node `argon2` (`argon2.verify(hash, password)`), Go
`alexedwards/argon2id`, PHP `password_verify()`, Keycloak/Authentik via import.

## `auth.sessions`

One row per login.

| Column | Type | Notes |
|---|---|---|
| `id` | `uuid` PK | goes into the JWT as the `session_id` claim |
| `user_id` | `uuid` → `auth.users` | `ON DELETE CASCADE` |
| `created_at`, `refreshed_at` | `timestamptz` | |
| `revoked_at` | `timestamptz` | logout or detected refresh token reuse |
| `user_agent`, `ip` | `text`, `inet` | informative |

## `auth.refresh_tokens`

| Column | Type | Notes |
|---|---|---|
| `id` | `bigint` PK | |
| `session_id` | `uuid` → `auth.sessions` | the token's "family" |
| `token_hash` | `bytea` unique | **SHA-256** of the opaque token; the token itself is never stored |
| `revoked` | `boolean` | `true` once used (rotation) |
| `created_at`, `expires_at` | `timestamptz` | default validity: 30 days |

### Rotation and reuse detection

1. `POST /auth/v1/token?grant_type=refresh_token` with token R1.
2. R1 valid → marked `revoked = true`; R2 is issued in the same session.
3. If R1 is used **again** (someone has a copy), the whole session is revoked:
   R2, R3... stop working. The user's other sessions continue.

Two simultaneous refreshes with the same token (two tabs) also trigger the
detection. The client should serialize refreshes. A grace window may come
later, if needed.

## `auth.one_time_tokens`

Links sent by email (today, only password recovery).

| Column | Type | Notes |
|---|---|---|
| `id` | `bigint` PK | |
| `user_id` | `uuid` → `auth.users` | `ON DELETE CASCADE` |
| `kind` | `text` | `recovery` |
| `token_hash` | `bytea` unique | **SHA-256** of the token; the token itself is never stored |
| `created_at`, `expires_at` | `timestamptz` | validity: 1 hour |
| `used_at` | `timestamptz` | set on use; the link does not work again |

## Endpoints

| Method and route | Body | Response |
|---|---|---|
| `POST /auth/v1/signup` | `{email, password, data?}` | 201 + session |
| `POST /auth/v1/token?grant_type=password` | `{email, password}` | 200 + session |
| `POST /auth/v1/token?grant_type=refresh_token` | `{refresh_token}` | 200 + session |
| `POST /auth/v1/logout` | Bearer | 204 (revokes the session) |
| `GET /auth/v1/user` | Bearer | user data |
| `GET /auth/v1/.well-known/jwks.json` | - | public JWKS |
| `POST /auth/v1/recover` | `{email}` | 200 `{}` (whether or not the account exists) |
| `POST /auth/v1/verify` | `{type: "recovery", token, password}` | 200 + session |

Session:

```json
{
  "access_token": "<JWT>",
  "token_type": "bearer",
  "expires_in": 900,
  "expires_at": 1767225600,
  "refresh_token": "<opaque>",
  "user": { "id": "...", "email": "...", "user_metadata": {}, "created_at": "...", "last_sign_in_at": "...", "email_confirmed_at": null }
}
```

Errors: `422 validation_failed` (email or password outside the rules), `409
user_already_exists`, `400 invalid_grant` (invalid credentials or refresh, with
the same message for an unknown email and a wrong password), `429
rate_limited` with `Retry-After`, `403 signup_disabled`.

Recovery: `403 recovery_disabled` (project without SMTP), `400 invalid_grant`
(invalid, expired or already used link), `400 unsupported_type`.

Password: 8 to 256 characters. Rate limit:
`NELCOTA_AUTH_RATE_LIMIT_PER_MINUTE` per IP (default 30) and the same limit
per email on login.

## Password recovery

Off until the project configures SMTP (Nelcota has no mail server of its own;
use your provider's: Postmark, SES, Resend...). In the project's `.env`, all
three together:

```sh
NELCOTA_SMTP_URL=smtps://user:password@smtp.example.com:465   # or smtp://...:587?tls=required
NELCOTA_SMTP_FROM=Shop <no-reply@shop.com>
NELCOTA_PASSWORD_RECOVERY_URL=https://app.shop.com/new-password
```

A half-configured setup stops the server from starting (better than finding
out on the first "forgot my password").

1. The app calls `POST /auth/v1/recover {email}`. The answer is always
   `200 {}`, so it does not reveal who has an account. If the account exists,
   an email arrives (in English, subject "Reset your password") with the link
   `https://app.shop.com/new-password#type=recovery&token=...`.
2. That app page reads the token from the fragment (`location.hash`), asks for
   the new password and calls `POST /auth/v1/verify {type: "recovery", token,
   password}`.
3. The response is a session (same format as login): the person is signed in.

Guarantees:

- The token travels in the URL fragment, which the browser sends to no server:
  it does not show up in logs or in the `Referer`.
- It is valid for 1 hour and only once. Asking again voids the previous link.
- At most one email per minute for the same account, on top of the per-IP
  rate limit: the endpoint cannot be used to flood someone's inbox.
- The email goes out in the background: response time does not reveal whether
  the account exists, and an SMTP failure goes to the log instead of becoming
  an error.
- Changing the password ends every session of the account (refresh tokens stop
  working immediately; access JWTs already issued stay valid until they
  expire).
