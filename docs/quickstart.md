# Quickstart: local development

From nothing to a typed client reading a table under RLS, on your machine.
Deploying to a VPS is covered in [deploy.md](deploy.md).

You need **Docker** (the development Postgres runs in a container) and, for
the client, Node 22+ or Rust.

## 1. Install the binary

On Linux:

```sh
curl -fsSL https://nelcota.com/install | NELCOTA_SKIP_DOCKER=1 sh   # keep your own Docker
```

On macOS or Windows, build it with Cargo (it installs the `nelcota` command):

```sh
cargo install --git https://github.com/HanielCota/Nelcota nelcota-server
```

Working on Nelcota itself? Run `cargo run -- dev` from a clone instead of
`nelcota dev` below.

## 2. Start the development environment

```sh
mkdir notes-app && cd notes-app
nelcota dev
```

`nelcota dev` starts Postgres 17 in a container (`nelcota-dev-postgres-54322`,
data in a Docker volume), applies Nelcota's internal migrations and serves the
API in the foreground. It prints where everything is:

```
  API:      http://127.0.0.1:8000/rest/v1/
  Auth:     http://127.0.0.1:8000/auth/v1/
  Panel:    http://127.0.0.1:8000/admin/
  Login:    admin@localhost  (ADMIN_PASSWORD in .nelcota/dev.env)
  Postgres: postgres://postgres:***@127.0.0.1:54322/postgres  (POSTGRES_PASSWORD in .nelcota/dev.env)

  Migrations: nelcota migrate   ·   Types: nelcota types -o database.ts
  service_role token (bypasses RLS): nelcota token service-role --days 30
```

Generated secrets (database password, signing key, panel login) stay in
`.nelcota/dev.env`, which is created readable by you only and ignored by git.
Running `nelcota dev` again reuses them and the same database. Use
`--listen 127.0.0.1:9000` or `--db-port 54323` if the default ports are taken.

Email confirmation is off in development (no SMTP is configured), so a
sign-up returns a session right away.

## 3. Write a migration

Leave `nelcota dev` running and open a second terminal in the same folder.
Migrations are plain SQL files named `V<n>__<name>.sql` in `migrations/`:

```sql
-- migrations/V1__notes.sql
CREATE TABLE public.notes (
    id         bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner      uuid        NOT NULL DEFAULT auth.uid(),
    body       text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE public.notes ENABLE ROW LEVEL SECURITY;
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notes TO authenticated;
CREATE POLICY notes_owner ON public.notes FOR ALL TO authenticated
    USING (owner = auth.uid()) WITH CHECK (owner = auth.uid());
```

Three things make a table usable from the API: the **GRANT** (which roles may
touch it at all), **RLS enabled** and a **policy** (which rows each user sees).
The SDK examples use a slightly larger version with a storage bucket:
[examples/notes.sql](../examples/notes.sql).

## 4. Apply it

```sh
nelcota migrate
```

It applies the files in `migrations/` that are not applied yet, in version
order, and records them in `nelcota.user_migrations`. An applied file
must not change afterwards: write a new `V2__...` instead. The API notices the
new table by itself (no restart).

You can also try SQL first in the panel's SQL editor
(`http://127.0.0.1:8000/admin/`), then turn it into a migration.

## 5. Generate types

```sh
nelcota types -o src/database.ts                 # TypeScript
nelcota types --lang rust -o src/database.rs     # Rust
```

Run it again after every migration: the types describe the schema as it is
now.

## 6. Use it from the client

```sh
npm install @nelcota/client
```

```ts
import { createClient } from '@nelcota/client';
import type { Database } from './database';

const nelcota = createClient<Database>('http://127.0.0.1:8000');

const { error } = await nelcota.auth.signUp({ email: 'ana@example.com', password: 'a-long-password' });
if (error) throw error;

await nelcota.from('notes').insert({ body: 'My first note' }).throwOnError();
const { data: notes } = await nelcota
  .from('notes')
  .select('id,body,created_at')
  .order('id', { ascending: false })
  .throwOnError();
console.log(notes); // only Ana's notes: RLS filters the rest
```

The Rust client works the same way; see [nelcota-client](../sdk/rust/README.md).

Or with `curl` and `jq`:

```sh
TOKEN=$(curl -s -X POST http://127.0.0.1:8000/auth/v1/token?grant_type=password \
  -H 'content-type: application/json' \
  -d '{"email":"ana@example.com","password":"a-long-password"}' | jq -r .access_token)
curl "http://127.0.0.1:8000/rest/v1/notes?order=id.desc" -H "authorization: Bearer $TOKEN"
```

## Next

- Something returns `[]`, 401 or 403? See [troubleshooting.md](troubleshooting.md).
- Query syntax, writes and errors: [api.md](api.md). Files: [storage.md](storage.md).
- SDKs: [@nelcota/client](../sdk/typescript/README.md) (JavaScript/TypeScript)
  and [nelcota-client](../sdk/rust/README.md) (Rust).
- Going live: [deploy.md](deploy.md).
