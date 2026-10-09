# @nelcota/client

TypeScript client for [Nelcota](https://github.com/HanielCota/Nelcota): the
REST API under RLS, auth and storage.

- **Thin.** Every call is one HTTP request. There is no hidden cache, and
  Postgres (GRANTs and RLS) decides what a call may do.
- **No runtime dependencies.** It uses `fetch`, Web Crypto, `AbortSignal`,
  Web Locks and `BroadcastChannel`, and runs in browsers, Node 22+, Deno,
  Bun and edge runtimes. ESM only, side-effect free, with subpath exports
  (`/rest`, `/auth`, `/storage`).
- **Typed from your schema** by `nelcota types`, down to the embeds in a
  `select` string.
- **Safe by construction.** Input never becomes query syntax, storage paths
  cannot climb out of their bucket, and a `service_role` token is refused in
  a browser.

```sh
npm install @nelcota/client
nelcota types -o src/database.ts
```

```ts
import { createClient } from '@nelcota/client';
import type { Database } from './database';

export const nelcota = createClient<Database>('https://api.example.com');
```

## Results and errors

Every call resolves to `{ data, error }` and never throws for HTTP or
network failures. `error` is a `NelcotaError`:

- `status`: the HTTP status, or `0` when no response arrived.
- `code`: the server's code untouched (`user_already_exists`,
  `invalid_grant`, `db_error`, `rate_limited`...), or a client code:
  `network_error`, `timeout`, `aborted`, `invalid_response`, `not_single`,
  `session_missing`, `pkce_missing`.
- `retryAfter`: seconds to wait, after a 429 or 503.

A programming mistake (an invalid column name, a `..` in a file path, a
`service_role` token in a browser) throws a `NelcotaUsageError` before
anything is sent.

```ts
const { data, error } = await nelcota.from('notes').select('id,body');
if (error) console.error(error.code, error.message);
```

## Tables, views and functions

```ts
// Read: columns, *, embeds in either direction, filters, order, paging.
const { data, count } = await nelcota
  .from('orders')
  .select('id,total,customer:customers(name),items(product,qty)', { count: 'exact' })
  .eq('status', 'paid')
  .gte('items.qty', 2)                                  // filters the embedded rows
  .order('created_at', { ascending: false })
  .order('qty', { referencedTable: 'items' })
  .range(0, 19);
// data: { id: number; total: number; customer: { name: string } | null;
//         items: { product: string; qty: number }[] }[]

// One row: an error (not_single) unless exactly one matches.
const { data: order } = await nelcota.from('orders').select().eq('id', 10).single();

// Only the count (a HEAD request, no rows).
const { count: open } = await nelcota.from('orders').select('*', { count: 'exact', head: true }).eq('status', 'open');

// Groups: conditions are built with typed calls, never strings.
await nelcota.from('products').select().or((c) => [
  c.eq('status', 'featured'),
  c.and([c.gt('price', 100), c.not(c.is('archived', true))]),
]);

// Writes return nothing unless you ask for the rows with select().
await nelcota.from('notes').insert({ body: 'hi' });
const { data: created } = await nelcota.from('notes').insert({ body: 'hi' }).select('id').single();
await nelcota.from('notes').update({ body: 'edited' }).eq('id', 1);
await nelcota.from('notes').delete().in('id', [1, 2, 3]);
await nelcota.from('products').upsert(rows, { onConflict: 'slug' });      // or ignoreDuplicates: true

// Functions run with the caller's role.
const { data: total } = await nelcota.rpc('cart_total', { cart_id: 7 });
```

Filters: `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `like`, `ilike`, `in`,
`is` (`null`, `true`, `false`, `'unknown'`), `not(column, op, value)`,
`match({...})`, `or(...)` and `and(...)`. A column path such as
`orders.total` filters an embed. Builders are immutable, so a base query can
be reused.

`like`/`ilike` read `*` (or `%`) as "any text". Wrap text a person typed in
`escapeLike` so `%` and `_` match literally:

```ts
import { escapeLike } from '@nelcota/client';
nelcota.from('products').select().ilike('name', `*${escapeLike(search)}*`);
```

`update` and `delete` need at least one filter; the server refuses them
otherwise. `count: 'exact'` scans every matching row, so ask for it only
when you show it.

## Auth

```ts
await nelcota.auth.signUp({ email, password, data: { name: 'Ana' } });  // session is null while the email awaits confirmation
await nelcota.auth.signInWithPassword({ email, password });
await nelcota.auth.signOut();

const { data: session } = await nelcota.auth.getSession();   // refreshed first when about to expire
const { data: user } = await nelcota.auth.getUser();         // the server's view, now

const stop = nelcota.auth.onChange((event, session) => {
  // 'signed_in' | 'signed_out' | 'refreshed' | 'user_updated', also from other tabs
});
```

**Sessions.** In browsers the session is kept in `localStorage`; elsewhere
it is kept in memory. Pass `auth: { storage }` to change that, for example
with a cookie adapter for server-side rendering. Anything with
`getItem`/`setItem`/`removeItem` works, sync or async.

`localStorage` can be read by any script on the page, so the usual XSS
hygiene applies (a Content Security Policy, no untrusted HTML). The session
holds a short-lived access token (15 minutes by default) and a refresh
token that rotates on every use.

**Refresh.** The access token is refreshed in the background before it
expires, and again right before a request when needed.

The server ends a session whose refresh token is used twice, so refreshes
are coordinated:

- One refresh at a time in a process, however many requests are waiting.
- Across tabs, a Web Lock serializes refreshes, and a tab that waited takes
  the session the other tab got.

If a refresh fails for a transient reason, the old token is still sent. The
request then fails with 401 instead of silently running as a visitor.

**OAuth (Google, GitHub)** uses PKCE, with the verifier kept in this
browser:

```ts
// Login page: navigates to the provider.
await nelcota.auth.signInWithOAuth({ provider: 'github', redirectTo: 'https://app.example.com/auth/callback' });

// On the callback page: reads ?code= (or ?error=), cleans the address bar, signs in.
const { data, error } = await nelcota.auth.handleRedirect();
```

`redirectTo` must be listed in the server's `NELCOTA_OAUTH_REDIRECT_URLS`.

**Email links** arrive as `https://your.app/page#type=...&token=...`.
`readEmailLink()` takes the token and removes it from the address bar and
history:

```ts
const link = nelcota.auth.readEmailLink();
if (link?.type === 'magiclink' || link?.type === 'signup') await nelcota.auth.verifyEmailLink(link);
if (link?.type === 'recovery') await nelcota.auth.resetPassword({ token: link.token, password: newPassword });

await nelcota.auth.sendMagicLink(email);
await nelcota.auth.requestPasswordReset(email);   // same answer whether or not the account exists
await nelcota.auth.resendConfirmation(email);
```

**On a server**, forward the caller's token instead of keeping a session:

```ts
const nelcota = createClient<Database>(url, { accessToken: () => request.headers.authorization?.slice(7) ?? null });
```

A `service_role` token bypasses RLS. Use it only on a trusted server; in a
browser the client refuses it.

## Storage

```ts
const files = nelcota.storage.from('avatars');

await files.upload(`${user.id}/me.png`, file);                       // 409 object_exists if taken
await files.upload(`${user.id}/me.png`, file, { upsert: true });     // replace
const { data: blob } = await files.download(`${user.id}/me.png`);
const { data: listing } = await files.list({ prefix: `${user.id}/` });
await files.remove(`${user.id}/me.png`);

const { data: url } = await files.createSignedUrl('reports/q1.pdf', 3600);  // anyone with the URL, for an hour
files.publicUrl('logo.png');                                                 // public buckets, no request
```

- **Uploads** send the bytes as they are (no multipart). A `ReadableStream`
  is streamed without buffering where the runtime supports it (Node, Deno,
  Bun; browsers over HTTP/2).
- **`open(name, { range, ifNoneMatch, download })`** returns the raw
  `Response`, for streaming `response.body`, resuming with ranges (206) or
  revalidating with ETags (304).
- **Names** are normalized to NFC and encoded segment by segment. Empty,
  `.` and `..` segments, control characters and backslashes are refused.
- **Buckets** are managed with `createBucket`, `updateBucket`,
  `deleteBucket`, `getBucket` and `listBuckets`, usually as `service_role`.

Who may read or write which file is up to the policies on
`storage.objects`; see the server's
[storage docs](https://github.com/HanielCota/Nelcota/blob/main/docs/storage.md).

## Options

```ts
createClient<Database>(url, {
  auth: { storage, storageKey, autoRefresh, refreshMargin },
  accessToken,           // () => token | null, replaces the session
  fetch,                 // custom fetch (tests, tracing)
  headers,               // extra headers (browsers only send what CORS allows)
  timeout: 30_000,       // ms per attempt for API calls; uploads/downloads have none
  retries: 2,            // reads only, on 429/503 and network errors, honouring Retry-After
});
```

Every builder takes `.abortSignal(signal)` and `.timeout(ms)`; the other
calls take `{ signal, timeout }`.

## Compatibility

| @nelcota/client | Nelcota server |
|---|---|
| 0.1.x | 0.3.0 or newer (exposed CORS headers, `Relationships` in `nelcota types`) |

Older servers work too, except that browsers cannot read `Retry-After`, and
embeds are typed loosely without `Relationships`.

## Development

```sh
npm ci
npm run check && npm test && npm run test:types   # types, unit and type-level tests
npm run build && npm run size                     # dist/ and the size budget per entry
cargo build -p nelcota-server
npm run test:contract -- --browser                # against the real binary (Docker), plus Chromium
```

The contract run starts Postgres and Mailpit in containers and the server
from `target/debug`, applies `test/contract/fixture.sql` and generates the
types with `nelcota types`. It then runs the contract tests (including a
fuzz of the query encoding) and, with `--browser`, a Chromium page on
another origin.
