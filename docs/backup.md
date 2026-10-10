# Backup and restore

## Backup

```sh
nelcota -p shop backup            # pg_dump -Fc → projects/shop/backups/nelcota-shop-<date>.dump
                                  # (+ s3://<bucket>/shop/ when host.env has a bucket)
nelcota -p shop backup --upload   # requires the upload: fails if host.env has no bucket
nelcota backup --all --keep 7     # every project; keeps the 7 most recent local dumps (--keep ≥ 1)
```

`backup` uploads on its own whenever the S3 variables are in `host.env`, so
adding a bucket after `init` needs no other change. `--upload` forces it and
fails without one.

The first `init` (as root) installs a daily cron job at 03:00
(`/etc/cron.d/nelcota-backup`):

```
0 3 * * * root '/usr/local/bin/nelcota' -C '/opt/nelcota' backup --all --keep 7 >> /var/log/nelcota-backup.log 2>&1
```

A broken project does not stop the others from being backed up; the command
lists the failures and exits non-zero. `nelcota status` shows the age of each
project's last backup, and `nelcota doctor` reports a missing cron job or a
backup older than 26 hours.

With disk storage, every dump has a sibling `<dump>.files/` directory containing
the immutable object versions and a SHA-256 manifest. The dump and file list
share one exported Postgres snapshot. A SHARE lock on `storage.objects` prevents
metadata changes during capture; reads continue, but file writes may wait.

The upload publishes that directory to `s3://<bucket>/<project>/<dump>.files/`
before uploading the dump. Later replacements and deletions cannot change an
older snapshot. Keep each dump together with its file directory; `--keep` prunes
both locally. Configure remote lifecycle rules for both members of the pair.

On Unix, dumps, snapshot files, manifests and environment files are created
with mode **0600**, before writing data. Backup and archive directories use
**0700**. Copies retain the private policy rather than the source's mode;
downloads run with umask 077 and their bundles are sealed afterward. Existing
environment files are restricted before overwriting them. Windows keeps the
filesystem's ACL policy; Unix modes are not simulated there.

The dump uses `pg_dump`'s custom format: it includes schema, data, RLS
policies, functions and the `auth` schema (users with their argon2id hashes).
Cluster roles (`anon`, `authenticated`...) are not in the dump, but Nelcota's
migrations recreate them on any new install.

### S3-compatible

Any service with an S3 API: AWS S3, Backblaze B2, Cloudflare R2, Wasabi,
MinIO. Variables in `host.env` (they apply to every project):

```sh
NELCOTA_BACKUP_S3_ENDPOINT=https://s3.us-west-002.backblazeb2.com
NELCOTA_BACKUP_S3_BUCKET=my-backups
NELCOTA_BACKUP_S3_ACCESS_KEY=...
NELCOTA_BACKUP_S3_SECRET_KEY=...
NELCOTA_BACKUP_S3_REGION=us-west-002
```

Uploads use the official `amazon/aws-cli` image (nothing to install on the
host). Configure whatever retention (lifecycle) you want on the bucket.

> **Keep `host.env` and each project's `.env` somewhere safe, apart from the
> bucket.** They are not in the backup (they hold secrets). Without the
> project's `.env` a restore still works, but the JWT key changes and every
> user has to sign in again (passwords keep working).

## Restore

```sh
nelcota -p shop restore projects/shop/backups/nelcota-shop-20261006T030000Z.dump
```

The restore asks for confirmation (`--yes` skips it), puts the project's
Caddy site in maintenance (HTTP 503 with `Retry-After`), stops the app and
takes a **safety backup** of the current state, printing its path. Then it
runs `pg_restore --clean --if-exists` in **a single transaction** (if anything
fails, the database stays as it was), starts the app and reopens traffic once
it is healthy. If the app does not become healthy, the site stays in
maintenance and the error shows how to go back:

```sh
nelcota -p shop restore projects/shop/backups/<safety dump> --yes
```

A restore also takes over a maintenance gate left closed by a failed upgrade
or restore, since it is how such a project recovers.

Use `--files` to restore the matching disk snapshot. It uses the local sibling
directory when present, otherwise fetches that dump's snapshot from the backup
bucket. The manifest, file sizes and hashes are checked before stopping the app.
The immutable versions are copied before restoring the database, so a file-copy
failure leaves the database untouched. Existing unrelated versions remain for
the normal orphan collector. Backups made with the former shared storage mirror
have no historical file manifest; they remain usable for database-only restores.

### Restore on a new VPS

```sh
curl -fsSL https://nelcota.com/install | sh
mkdir -p /opt/nelcota && cd /opt/nelcota
nelcota init api.shop.com --yes
cp /safe/place/shop.env projects/shop/.env   # optional: keeps the JWT key
nelcota up
aws s3 cp s3://my-backups/shop/nelcota-shop-....dump projects/shop/backups/ --endpoint-url ...
nelcota -p shop restore projects/shop/backups/nelcota-shop-....dump --yes
```

The acceptance test (`scripts/acceptance.sh --local`) takes a backup, writes
more data, restores and checks that the state went back to the backup's.

When a project is removed (`nelcota -p <name> remove`), a final backup is kept
in `archive/`, in the host folder. If its Postgres is stopped, `remove` starts
it for the backup and refuses to delete anything when it cannot;
`--no-backup` removes without one.
Disk snapshots are archived alongside the final dump.

## PITR (point-in-time recovery)

Daily dumps lose up to 24 h of writes in the worst case. With PITR on,
Postgres sends every WAL segment to the S3-compatible storage as it fills (and
at least once a minute), next to periodic base backups, so the project can go
back to **any second** the archive covers: just before a bad migration or a
`DELETE` without a `WHERE`.

```sh
nelcota -p shop pitr enable                                    # archiving + first full backup
nelcota -p shop pitr status                                    # backups and the range covered
nelcota -p shop pitr restore --time "2026-10-07 14:30:00+00"   # back to that moment
nelcota -p shop pitr restore                                   # up to the last archived write
nelcota -p shop pitr disable                                   # stops archiving (keeps what is in S3)
```

- It uses [pgBackRest](https://pgbackrest.org/), installed in the project's
  Postgres image (`postgres/Dockerfile`: the official `postgres:17-alpine` plus
  the Alpine package, so the data directory is unchanged). Everything goes to
  `s3://<bucket>/<project>/pitr/`, beside the dumps.
- It needs the S3 settings in `host.env` (the same ones the backup upload uses).
  pgBackRest only talks to S3 over HTTPS. For your own MinIO/SeaweedFS with a
  self-signed certificate, add `NELCOTA_BACKUP_S3_VERIFY_TLS=false`; for
  storage that needs path-style URLs, `NELCOTA_BACKUP_S3_URI_STYLE=path`.
- `enable` writes `pgbackrest.env` in the project folder (mode 600, it holds
  the S3 keys). After changing the S3 settings in `host.env`, run `enable`
  again to regenerate it.
- The daily `backup --all` also takes a base backup for projects with PITR:
  full on Sundays, differential on the other days. Two full backups are kept,
  so the window reaches back one to two weeks. Fewer WAL segments to replay
  means a faster restore.
- `restore` stops the project, rewrites the data directory from the closest
  base backup, replays the WAL up to the target and starts everything again.
  It discards every write after the target, so it asks for confirmation
  (`--yes` skips it). Then it takes a new full backup, because the restored
  database continues on a new timeline that older backups cannot serve.
- A target the archive does not cover is refused by pgBackRest before any
  change, and the project starts again as it was.
- Projects created before PITR are updated by `enable`: Postgres switches to
  the image built from `postgres/Dockerfile` and gets an `egress` network to
  reach S3 (the `internal` network has no way out). If `docker-compose.yml`
  was edited by hand, `enable` prints the changes to make.

The acceptance test (`scripts/acceptance-pitr.sh`) runs the whole cycle
against a local S3 server with TLS.
