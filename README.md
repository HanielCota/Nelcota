# Nelcota

Open source BaaS, simple and lock-in free: **plain Postgres + one Rust binary +
a 5-minute deploy on a VPS.**

- **Postgres is the product.** Schema, RLS policies and functions are plain SQL
  and keep working without Nelcota.
- **Authorization belongs to RLS.** The API only validates the JWT and assumes
  its role inside a transaction. It never decides permissions on its own.
- **One binary** with an automatic REST API, auth (EdDSA JWT + JWKS,
  argon2id, refresh with rotation, password recovery, email confirmation
  and magic link by email, sign-in with Google or GitHub), file storage
  under RLS (disk or S3), an admin panel and a deploy CLI.
- **Open standards:** JWT/JWKS, PHC argon2id, OAuth 2.0 + PKCE, plain SQL, OpenAPI,
  S3-compatible. You can leave and take everything with you
  ([leaving.md](docs/leaving.md)).

## Deploy (fresh VPS → HTTPS)

```sh
curl -fsSL https://nelcota.com/install | sh
nelcota init api.yourdomain.com
nelcota up
```

**Several projects on the same VPS**, each one isolated (Postgres, users, keys,
backups), with its own domain or a subdomain and a single sign-on for every
panel:

```sh
nelcota init api.shop.com                              # own domain
nelcota init --project blog --base-domain example.com  # subdomain: blog.example.com
nelcota up && nelcota projects
```

Details in [docs/deploy.md](docs/deploy.md). Backup and restore in
[docs/backup.md](docs/backup.md), including point-in-time recovery
(`nelcota -p shop pitr enable`: WAL archived to S3, restore to any second).
A VPS that holds a single project can skip Docker: `nelcota init <domain>
--runtime systemd` installs Postgres and Caddy from apt and runs the app as a
systemd unit.

## Usage in 1 minute

```sql
-- migrations/V1__notes.sql  →  nelcota migrate
CREATE TABLE public.notes (
    id    bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner uuid NOT NULL DEFAULT auth.uid(),
    body  text NOT NULL
);
ALTER TABLE public.notes ENABLE ROW LEVEL SECURITY;
CREATE POLICY owner ON public.notes FOR ALL TO authenticated
    USING (owner = auth.uid()) WITH CHECK (owner = auth.uid());
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notes TO authenticated;
```

```sh
# sign up → JWT
curl -X POST https://api.yourdomain.com/auth/v1/signup \
  -H 'content-type: application/json' -d '{"email":"ana@x.com","password":"strong-password-123"}'

# CRUD under RLS
curl -X POST https://api.yourdomain.com/rest/v1/notes -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -H 'prefer: return=representation' -d '{"body":"hi"}'
curl "https://api.yourdomain.com/rest/v1/notes?body=ilike.*hi*&order=id.desc" -H "authorization: Bearer $TOKEN"
```

Files under the same policies: a row in `storage.objects` per file, bytes on
disk or in any S3 bucket ([storage.md](docs/storage.md)).

```sh
curl -X POST https://api.yourdomain.com/storage/v1/object/avatars/$USER_ID/me.png   -H "authorization: Bearer $TOKEN" -H 'content-type: image/png' --data-binary @me.png
```

Frontend types: `nelcota types -o database.ts`. OpenAPI at `/rest/v1/`.
TypeScript client: [`@nelcota/client`](sdk/typescript/README.md), typed from
those types, with sessions, OAuth and storage.
Panel at `/admin/`: table editor with inline editing, SQL editor with
autocomplete, users, RLS policies and migrations generated from panel changes.
The panel is available in Portuguese and English (picked from the browser,
switchable in the account menu).

## Development

Requirements: stable Rust and Docker.

```sh
cargo run -- dev            # Postgres 17 in a container + server at http://127.0.0.1:8000
cargo test                  # integration tests start a real Postgres 17 (testcontainers)
cargo clippy --all-targets -- -D warnings && cargo fmt --all --check
cargo audit && cargo deny check
./scripts/acceptance.sh --local   # init + up + HTTPS + migrate + backup/restore + rollback

# panel (Svelte 5 + shadcn-svelte + Tailwind v4 + CodeMirror)
cd crates/admin/ui && npm install && npm run dev
```

The panel build (`crates/admin/ui/dist`) is versioned: compiling the binary
does not need Node.

`dev` prints the panel URL and generated admin login; credentials persist in
`.nelcota/dev.env`. Existing development environments are upgraded automatically.
For hot reload, run the panel command in a second terminal and open
`http://127.0.0.1:5173/admin/`. Vite proxies the API to `http://127.0.0.1:8000`;
set `NELCOTA_API_URL` to use another server address.

After changing panel wire types, run `npm run contracts` in `crates/admin/ui`.
Rust generates TypeScript and JSON Schema; Node compiles browser validators
without runtime `eval`. `cargo test` detects stale contracts. Run `npm run check`,
`npm test`, `npx playwright install chromium`, `npm run test:e2e` and
`npm run build` for panel changes. Browser regressions are also checked in CI.

The tests prove, against a real Postgres, that a user cannot read or change
another user's data (with every verb), that invalid, expired or unknown-role
JWTs get 401, that role and claims do not leak between pooled requests, that
malicious identifiers and values do not become SQL, and that reusing a refresh
token kills the session.

## Documentation

- [REST API](docs/api.md): filters, writes, RPC, OpenAPI, errors
- [Storage](docs/storage.md): buckets, policies, uploads, signed URLs
- [JWT and roles](docs/jwt-and-roles.md): the token contract and the JWT → RLS flow
- [Auth schema](docs/schema-auth.md): tables, password format, endpoints
- [Panel](docs/panel.md)
- [Deploy](docs/deploy.md) · [Backup](docs/backup.md) · [Leaving Nelcota](docs/leaving.md)
- [Architecture](docs/architecture.md) · [Decisions](docs/decisions.md) · [Benchmark](bench/README.md)

> **Warning:** A JWT with `role: service_role` bypasses all RLS. Never expose it
> in the frontend.

## Out of the MVP

Realtime, Edge Functions, MFA, image transformations,
multi-tenant. The architecture leaves room for them.

## License

[Apache-2.0](LICENSE)
