#!/usr/bin/env bash
# Acceptance test for hosts without Docker (`nelcota init --runtime systemd`).
#
#   NELCOTA_LINUX_BIN=path/to/nelcota-x86_64-linux-musl ./scripts/acceptance-systemd.sh
#
# Docker is only the test bench: a Debian 12 container with systemd as PID 1
# plays the VPS. Inside it: init (apt installs Postgres 17, pgBackRest and
# Caddy), a refused second project, up, HTTPS through Caddy, migrate, signup
# and RLS, types and token, backup → write → restore, upgrade (binary swap),
# logs, PITR against an S3 server with TLS (SeaweedFS), and remove.
set -euo pipefail
# Git Bash: never rewrite container paths.
export MSYS_NO_PATHCONV=1
say() { printf '
[1m== %s[0m
' "$*"; }
fail() { printf '
FAILED: %s
' "$*" >&2; exit 1; }

LINUX_BIN="${NELCOTA_LINUX_BIN:?set NELCOTA_LINUX_BIN to a Linux (musl) build of nelcota}"
[ -f "$LINUX_BIN" ] || fail "binary not found: $LINUX_BIN"
command -v cygpath >/dev/null 2>&1 && LINUX_BIN="$(cygpath -m "$LINUX_BIN")"

cleanup() {
  docker rm -f nelcota-vps native-s3 >/dev/null 2>&1 || true
  docker network rm nelcota-native-s3 >/dev/null 2>&1 || true
}
trap cleanup EXIT

say "Debian 12 with systemd"
docker build -q -t nelcota-test-vps:debian12 - >/dev/null <<'DOCKERFILE'
FROM debian:12
ENV container=docker
RUN apt-get update -q && apt-get install -y -q systemd systemd-sysv curl ca-certificates procps  && rm -f /lib/systemd/system/multi-user.target.wants/getty* && apt-get clean
STOPSIGNAL SIGRTMIN+3
CMD ["/lib/systemd/systemd"]
DOCKERFILE
cleanup
docker run -d --name nelcota-vps --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw   --tmpfs /run --tmpfs /run/lock nelcota-test-vps:debian12 >/dev/null
until docker exec nelcota-vps systemctl is-system-running 2>/dev/null | grep -qE "running|degraded"; do sleep 1; done

vps() { docker exec -i nelcota-vps "$@"; }
n() { vps nelcota -C /opt/nelcota "$@"; }
curl_() { vps curl -ksS --fail-with-body --resolve shop.localhost:443:127.0.0.1 "$@"; }
sql() { vps runuser -u postgres -- psql -tAc "$1"; }

say "binary"
docker cp "$LINUX_BIN" nelcota-vps:/usr/local/bin/nelcota
vps chmod 755 /usr/local/bin/nelcota
vps nelcota --version

say "init --runtime systemd (installs Postgres 17, pgBackRest, Caddy)"
vps mkdir -p /opt/nelcota
n init --local --project shop --runtime systemd --yes
vps test -f /etc/systemd/system/nelcota-shop.service || fail "no unit"
vps test ! -f /opt/nelcota/projects/shop/docker-compose.yml || fail "compose file on a systemd host"
vps grep -q "reverse_proxy 127.0.0.1:8000" /etc/caddy/Caddyfile || fail "Caddyfile"

say "a second project is refused"
if n init --local --project blog --yes; then fail "second project accepted"; fi

say "up"
n up
curl_ https://shop.localhost/health; echo
n status

say "migrate + signup + RLS through Caddy"
vps sh -c 'cat > /opt/nelcota/projects/shop/migrations/V1__notes.sql' <<'SQL'
CREATE TABLE public.notes (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    owner uuid NOT NULL DEFAULT auth.uid(),
    body text NOT NULL
);
ALTER TABLE public.notes ENABLE ROW LEVEL SECURITY;
CREATE POLICY owner ON public.notes FOR ALL TO authenticated
    USING (owner = auth.uid()) WITH CHECK (owner = auth.uid());
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notes TO authenticated;
SQL
n -p shop migrate
n -p shop migrate
token=$(curl_ -X POST https://shop.localhost/auth/v1/signup -H 'content-type: application/json' \
  -d '{"email":"ana@x.com","password":"strong-password-123"}' | sed -E 's/.*"access_token":"([^"]+)".*/\1/')
[ -n "$token" ] || fail "no token"
curl_ -X POST https://shop.localhost/rest/v1/notes -H "authorization: Bearer $token" \
  -H 'content-type: application/json' -d '{"body":"before the backup"}' >/dev/null
curl_ https://shop.localhost/rest/v1/notes -H "authorization: Bearer $token" | grep -q "before the backup" || fail "note"
code=$(vps curl -ks -o /dev/null -w '%{http_code}' --resolve shop.localhost:443:127.0.0.1 https://shop.localhost/rest/v1/notes)
[ "$code" = "401" ] || fail "anon got $code, expected 401"
types=$(n -p shop types); echo "$types" | grep -q "notes" || fail "types"
tok=$(n -p shop token service-role 2>/dev/null); echo "$tok" | grep -q '\.' || fail "token"

say "backup → write → restore"
n -p shop backup --keep 3
dump=$(vps sh -c 'ls /opt/nelcota/projects/shop/backups/*.dump | tail -1')
curl_ -X POST https://shop.localhost/rest/v1/notes -H "authorization: Bearer $token" \
  -H 'content-type: application/json' -d '{"body":"after the backup"}' >/dev/null
n -p shop restore "$dump" --yes
body=$(curl_ https://shop.localhost/rest/v1/notes -H "authorization: Bearer $token")
echo "$body" | grep -q "before the backup" || fail "restore lost data"
if echo "$body" | grep -q "after the backup"; then fail "restore did not go back"; fi

say "upgrade (same version: swaps the binary, keeps the previous one)"
n -p shop upgrade
vps test -f /usr/local/lib/nelcota/nelcota.previous || fail "no previous binary"
curl_ https://shop.localhost/health; echo

say "logs"
n -p shop logs app | tail -3

say "PITR against S3 over TLS"
docker network create nelcota-native-s3 >/dev/null 2>&1 || true
docker rm -f native-s3 >/dev/null 2>&1 || true
docker run -d --name native-s3 --network nelcota-native-s3 --network-alias s3 --entrypoint sh chrislusf/seaweedfs -c '
  apk add -q openssl >/dev/null 2>&1 || true
  mkdir -p /certs && cd /certs
  openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=s3 -keyout private.key -out public.crt 2>/dev/null
  echo "{\"identities\":[{\"name\":\"a\",\"credentials\":[{\"accessKey\":\"k\",\"secretKey\":\"secret123\"}],\"actions\":[\"Admin\",\"Read\",\"Write\",\"List\",\"Tagging\"]}]}" > s3.json
  exec weed server -ip.bind=0.0.0.0 -dir=/data -s3 -s3.port=9000 -s3.cert.file=/certs/public.crt -s3.key.file=/certs/private.key -s3.config=/certs/s3.json' >/dev/null
docker network connect nelcota-native-s3 nelcota-vps 2>/dev/null || true
sleep 10
docker run --rm --network nelcota-native-s3 -e AWS_ACCESS_KEY_ID=k -e AWS_SECRET_ACCESS_KEY=secret123 \
  -e AWS_DEFAULT_REGION=us-east-1 amazon/aws-cli --endpoint-url https://s3:9000 --no-verify-ssl s3 mb s3://backups 2>&1 | grep -v Warn
vps sh -c 'printf "NELCOTA_BACKUP_S3_ENDPOINT=https://s3:9000\nNELCOTA_BACKUP_S3_BUCKET=backups\nNELCOTA_BACKUP_S3_ACCESS_KEY=k\nNELCOTA_BACKUP_S3_SECRET_KEY=secret123\nNELCOTA_BACKUP_S3_VERIFY_TLS=false\nNELCOTA_BACKUP_S3_URI_STYLE=path\n" >> /opt/nelcota/host.env'
sql "CREATE TABLE marks (id serial PRIMARY KEY, label text); INSERT INTO marks (label) VALUES ('before enable');" >/dev/null
n -p shop pitr enable
vps stat -c '%U %a' /etc/pgbackrest/pgbackrest.conf
sql "INSERT INTO marks (label) VALUES ('kept')" >/dev/null
sleep 2; T=$(sql "SELECT now()"); sleep 2
sql "INSERT INTO marks (label) VALUES ('discarded'); SELECT pg_switch_wal();" >/dev/null
out=$(n -p shop backup); echo "$out" | tail -3
echo "$out" | grep -q "PITR base backup" || fail "no PITR base backup"
n -p shop pitr restore --time "$T" --yes
labels=$(sql "SELECT string_agg(label, ',' ORDER BY id) FROM marks")
[ "$labels" = "before enable,kept" ] || fail "PITR restore got '$labels'"
curl_ https://shop.localhost/health; echo
n -p shop pitr disable

say "remove"
n -p shop remove --yes
vps test ! -f /etc/systemd/system/nelcota-shop.service || fail "unit left behind"
if vps pg_lsclusters --no-header | grep -q "17 *main"; then fail "cluster left behind"; fi

say "ACCEPTED: systemd host"
