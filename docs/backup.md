# Backup and restore

## Backup

```sh
nelcota -p shop backup            # pg_dump -Fc → projects/shop/backups/nelcota-shop-<date>.dump
nelcota -p shop backup --upload   # and uploads it to the configured S3-compatible storage (s3://<bucket>/shop/)
nelcota backup --all --keep 7     # every project; keeps the 7 most recent local dumps
```

The first `init` (as root) installs a daily cron job at 03:00
(`/etc/cron.d/nelcota-backup`) running `backup --all`, plus `--upload` when S3
is configured. A broken project does not stop the others from being backed up.

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

The restore stops the app, runs `pg_restore --clean --if-exists` in **a single
transaction** (if anything fails, the database stays as it was) and starts the
app again.

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
in `archive/`, in the host folder.

## PITR (point-in-time recovery)

Daily dumps lose up to 24 h of writes in the worst case. To recover up to the
last second, the planned evolution is to archive the WAL continuously with
**WAL-G** (or pgBackRest) to the same S3. `init` does not set this up yet; it
is listed as pending in [decisions.md](decisions.md).
