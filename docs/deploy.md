# Deploy

**Fresh VPS → HTTPS in 3 commands.**

```sh
curl -fsSL https://nelcota.dev/install | sh     # binary (with checksum) + Docker if missing
mkdir -p /opt/nelcota && cd /opt/nelcota
nelcota init api.yourdomain.com                 # creates the host and the first project
nelcota up                                      # starts projects + Caddy, waits for the healthcheck
```

First create a DNS **A** record for the domain pointing at the VPS IP. Caddy
issues the certificate on its own on the first request.

Without `curl | sh`: download `nelcota-<architecture>` and
`nelcota-<architecture>.sha256` from the releases page, run `sha256sum -c` and
copy it to `/usr/local/bin`. The [script](../scripts/install.sh) does just
that.

## Host and projects

A VPS can hold **several projects**, each one isolated: its own Postgres,
users, JWT keys, panel and backups. A single project is simply a host with one
project.

```
Internet ──80/443──▶ Caddy (shared)
                        ├── api.shop.com ──▶ app-shop ──▶ shop's Postgres   (its own internal network)
                        └── blog.example.com ──▶ app-blog ──▶ blog's Postgres
```

```
/opt/nelcota/                   (the host folder; -C <folder> or NELCOTA_ROOT)
├── nelcota-host.json           registry: login mode, base domain, projects (no secrets)
├── host.env                    host secrets (0600): admin, SSO secret, S3
├── shared/projects.json        public list (name + URL), mounted into the apps for the switcher
├── caddy/                      shared Caddy; the Caddyfile is generated from the registry
├── archive/                    final backups of removed projects
└── projects/<name>/            docker-compose.yml, .env (0600), migrations/, backups/
```

- Only Caddy publishes ports. Each project's Postgres sits on its own
  `internal: true` network; apps talk to Caddy over the `nelcota_edge` network.
  Postgres also joins an `egress` network of its own, outbound only, to
  archive WAL to S3 when PITR is on.
- One Postgres per project, not a shared one: Postgres roles (`anon`,
  `authenticated`, `authenticator`...) apply to the whole server, and separate
  servers keep the physical isolation and a per-project `pg_dump` for leaving.

### Own domain or subdomain

```sh
nelcota init api.shop.com                                # own domain → project "shop"
nelcota init api.shop.com --project sales                # explicit name
nelcota init --project blog --base-domain example.com    # subdomain: blog.example.com
nelcota init --project panel                             # uses the base domain already saved on the host
nelcota init --local --project shop                      # local: https://shop.localhost
```

The project name comes from the domain (`api.shop.com` → `shop`), or from
`--project`. It shows up in the panel's project switcher and in the commands'
`-p`.

## Commands

With a single project, `-p` is optional. With several, commands that act on a
project ask for `-p <name>`.

| Command | What it does |
|---|---|
| `nelcota projects` | lists projects, state and version |
| `nelcota up` | starts every project and Caddy (`-p` for just one) |
| `nelcota status` | project state |
| `nelcota -p shop logs -f [app]` | logs |
| `nelcota -p shop down` / `down --all` | stops (`--volumes` DELETES the data) |
| `nelcota -p shop migrate` | applies `projects/shop/migrations/V<n>__<name>.sql` |
| `nelcota -p shop types -o database.ts` | TypeScript types of the schema |
| `nelcota -p shop token service-role` | service JWT (**bypasses RLS**) |
| `nelcota -p shop backup [--upload]` / `backup --all` | dump into `backups/` (and to S3) |
| `nelcota -p shop restore <file>` | restores a dump |
| `nelcota -p shop pitr enable` / `status` / `restore --time ...` | point-in-time recovery with WAL archived to S3 ([backup.md](backup.md#pitr-point-in-time-recovery)) |
| `nelcota -p shop upgrade` / `upgrade --all` | upgrades with a backup and automatic rollback |
| `nelcota -p shop remove` | final backup in `archive/`, removes containers, data and the Caddy site |
| `nelcota panel-login shared` / `per-project` | panel login: single or per project |
| `nelcota admin-password` | new panel password (`-p` with per-project login) |

On a host, `migrate`, `types` and `token` run inside the project's `app`
container, which is what can reach Postgres. Outside a host, they use
`NELCOTA_DATABASE_URL`.

## Panel login

- **Single** (default): one email and password for every panel on the host.
  Signing in to one panel and switching projects with the switcher does not ask
  for the password again.
- **Per project**: each panel has its own credentials; the switcher just takes
  you to the other panel, which asks for its own login.

```sh
nelcota panel-login per-project     # generates and shows one password per project
nelcota panel-login shared          # back to single login (host password)
```

Security details in [panel.md](panel.md).

## What the first `init` does

1. **Checks** Docker and the compose plugin, free ports 80/443, RAM and the
   domain's DNS.
2. **Generates** the host secrets: admin credentials (password shown **once**;
   `host.env` keeps only the argon2id hash) and the single sign-on secret.
3. **Backup:** asks for the S3-compatible destination (or use `--s3-*`) and,
   as root, installs the daily cron job for every project (03:00, keeps 7).
4. **Machine:** with `--firewall`, configures `ufw` (SSH, 80 and 443).

Each later `init` only creates the project: its own secrets (Postgres,
`authenticator`, Ed25519 key), a Postgres profile from the RAM split between
projects, and the Caddy site (reloaded without taking the others down).

| VPS | Small projects (estimate) |
|---|---|
| 1 GB | 2–3 |
| 2 GB | 5–8 |
| 4 GB | 12–20 |

## Migrations

Plain SQL in `projects/<name>/migrations/`, named `V1__create_tables.sql`,
`V2__indexes.sql`... Each file runs in a transaction. Tracking lives in
`nelcota.user_migrations` (version, name, date, checksum): if an already
applied file is edited, `migrate` refuses. After applying, the API reloads the
catalog on its own.

### Changes made in the panel

Tables, columns and policies created in the panel go straight to the project's
database, but they do not exist in any other environment (staging, another
machine, a fresh database) until they become a file. The panel records each
one and lists the pending ones under **Migrations**; "Generate migration"
downloads `V<n>__<name>.sql` with them and already registers it as applied in
this database, with the same checksum `migrate` computes. Put the file in
`migrations/` and commit it: nothing runs again here, and `migrate` applies it
in the other environments.

The chosen number goes past everything in the database and in the folder (the
container reads `./migrations`, mounted at `/migrations`), so a file not yet
applied is never overtaken. Outside the container, point to the folder with
`NELCOTA_MIGRATIONS_DIR`. What you run in the SQL editor is not recorded: copy
that SQL into a migration by hand.

## Upgrade

`nelcota -p shop upgrade` takes a backup, changes `NELCOTA_VERSION` in the
project's `.env`, pulls the image and waits for the healthcheck. If the new
version does not become healthy, it goes back to the previous version **and**
restores the backup (the new version may have migrated the schema). Other
projects are not touched; `upgrade --all` upgrades one at a time.

## Local development

```sh
nelcota dev        # Postgres 17 in a container (127.0.0.1:54322) + server on :8000
```

To test the full deploy locally (several projects, Caddy, HTTPS with an
internal certificate): `nelcota init --local --project shop` and open
`https://shop.localhost/admin/`.

## Acceptance test

```sh
ACCEPT_DOMAIN=api.example.com ./scripts/acceptance.sh   # real VM
./scripts/acceptance.sh --local                          # local simulation
```

Last local run (2026-10-06, Windows 11 + Docker Desktop, cached images): **two
projects with HTTPS in 28 s**. Also passing: migrate (idempotent), signup and
RLS through Caddy, isolation between projects (one project's JWT is not valid
on the other; one project's tables do not exist on the other), single sign-on
between panels with a single-use token, switching to per-project login and
back, `types`, backup → write → restore, automatic upgrade rollback without
affecting the other project, and project removal with a backup in `archive/`.

VM mode has not been run yet: it depends on the published repository and the
first release.
