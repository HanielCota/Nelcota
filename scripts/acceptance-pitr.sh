#!/usr/bin/env bash
# Point-in-time recovery acceptance test (local; Docker and ports 80/443 free).
#
#   cargo build --release && ./scripts/acceptance-pitr.sh
#
# A local host with one project archives WAL with pgBackRest to an S3 server
# (SeaweedFS with a self-signed certificate, joined to the project's egress
# network), then: enable → writes around a moment → backup → restore to that
# moment → archiving on the new timeline → a refused restore leaves the
# project up → restore to the latest write → disable.
# Uses the local binary and image (NELCOTA_BIN, NELCOTA_IMAGE:NELCOTA_TEST_VERSION).
set -euo pipefail
# Git Bash: never rewrite container paths (/data, /certs...).
export MSYS_NO_PATHCONV=1
say() { printf '
[1m== %s[0m
' "$*"; }
fail() { printf '
FAILED: %s
' "$*" >&2; exit 1; }

BIN="${NELCOTA_BIN:-target/release/nelcota}"
IMAGE="${NELCOTA_IMAGE:-nelcota}"
VERSION="${NELCOTA_TEST_VERSION:-dev}"
[ -x "$BIN" ] || [ -x "$BIN.exe" ] || fail "binary not found: $BIN (cargo build --release)"
WORK="$(mktemp -d)"
command -v cygpath >/dev/null 2>&1 && WORK="$(cygpath -m "$WORK")"
ROOT="$WORK"
nelcota() { "$BIN" -C "$ROOT" "$@"; }
sql() { docker compose --project-directory "$ROOT/projects/shop" exec -T postgres psql -U postgres -tAc "$1"; }
cleanup() {
  nelcota down --all --volumes >/dev/null 2>&1 || true
  docker rm -f pitr-s3 >/dev/null 2>&1 || true
  docker volume rm pitr-s3-certs >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

say "S3 (SeaweedFS) with a self-signed certificate"
mkdir -p "$WORK/certs"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj "/CN=s3"   -addext "subjectAltName=DNS:s3"   -keyout "$WORK/certs/private.key" -out "$WORK/certs/public.crt" 2>/dev/null
cat > "$WORK/certs/s3.json" <<'JSON'
{"identities":[{"name":"admin","credentials":[{"accessKey":"pitrkey","secretKey":"pitrsecret123"}],"actions":["Admin","Read","Write","List","Tagging"]}]}
JSON
docker volume create pitr-s3-certs >/dev/null
docker run --rm -v pitr-s3-certs:/c -v "$WORK/certs:/src:ro" alpine sh -c 'cp /src/* /c/'
docker run -d --name pitr-s3 -v pitr-s3-certs:/certs:ro chrislusf/seaweedfs   server -ip.bind=0.0.0.0 -dir=/data -s3 -s3.port=9000 -s3.cert.file=/certs/public.crt   -s3.key.file=/certs/private.key -s3.config=/certs/s3.json >/dev/null

say "Host + project with S3"
nelcota init --local --project shop --yes --image "$IMAGE" --version "$VERSION" \
  --s3-endpoint https://s3:9000 --s3-bucket backups \
  --s3-access-key pitrkey --s3-secret-key pitrsecret123 >/dev/null
printf 'NELCOTA_BACKUP_S3_VERIFY_TLS=false\nNELCOTA_BACKUP_S3_URI_STYLE=path\n' >> "$WORK/host.env"
grep -q "build: ./postgres" "$WORK/projects/shop/docker-compose.yml" || fail "template without the built image"
nelcota up >/dev/null
docker network connect --alias s3 nelcota-shop_egress pitr-s3
docker run --rm --network nelcota-shop_egress -e AWS_ACCESS_KEY_ID=pitrkey   -e AWS_SECRET_ACCESS_KEY=pitrsecret123 -e AWS_DEFAULT_REGION=us-east-1 amazon/aws-cli   --endpoint-url https://s3:9000 --no-verify-ssl s3 mb s3://backups 2>&1 | grep -v Warning

sql "CREATE TABLE marks (id serial PRIMARY KEY, label text NOT NULL); INSERT INTO marks (label) VALUES ('before enable');"

say "pitr enable"
nelcota -p shop pitr enable

say "Writes around a moment"
sql "INSERT INTO marks (label) VALUES ('kept')" >/dev/null
sleep 2
T=$(sql "SELECT now()")
sleep 2
sql "INSERT INTO marks (label) VALUES ('discarded'); SELECT pg_switch_wal();" >/dev/null
echo "target: $T"

say "backup (dump + PITR base backup)"
nelcota -p shop backup | tee "$WORK/backup.log"
grep -q "PITR base backup" "$WORK/backup.log" || fail "backup did not take a PITR base backup"

say "pitr status"
nelcota -p shop pitr status | tee "$WORK/status.log"
grep -q "full backup" "$WORK/status.log" || fail "status without the full backup"

say "pitr restore --time"
nelcota -p shop pitr restore --time "$T" --yes
labels=$(sql "SELECT string_agg(label, ',' ORDER BY id) FROM marks")
echo "rows after restore: $labels"
[ "$labels" = "before enable,kept" ] || fail "expected 'before enable,kept', got '$labels'"

say "archiving continues on the new timeline"
sql "INSERT INTO marks (label) VALUES ('after restore'); SELECT pg_switch_wal();" >/dev/null
docker compose --project-directory "$ROOT/projects/shop" exec -T -u postgres postgres pgbackrest --stanza=main check

say "restore to a moment the archive does not reach fails cleanly"
if nelcota -p shop pitr restore --time "2099-01-01 00:00:00+00" --yes; then
  fail "restore to the future succeeded"
fi
nelcota -p shop pitr restore --yes
labels=$(sql "SELECT string_agg(label, ',' ORDER BY id) FROM marks")
echo "rows after restoring the latest state: $labels"
[ "$labels" = "before enable,kept,after restore" ] || fail "latest restore got '$labels'"

say "pitr disable"
nelcota -p shop pitr disable
[ "$(sql "SHOW archive_mode")" = "off" ] || fail "archive_mode still on"
[ ! -f "$WORK/projects/shop/pgbackrest.env" ] || fail "pgbackrest.env still there"

say "PITR OK"
