#!/usr/bin/env bash
# Deploy acceptance test: "clean machine → 3 commands → /health over HTTPS".
#
# VM mode (the real test, on a fresh Linux VPS with DNS pointed at it):
#   ACCEPT_DOMAIN=api.example.com ./scripts/acceptance.sh
#   → curl install | sh ; nelcota init $ACCEPT_DOMAIN --yes ; nelcota up ; measure.
#
# Local mode (simulation; Docker and ports 80/443 free):
#   ./scripts/acceptance.sh --local
#   → local host with TWO projects (shop.localhost and blog.localhost), using the
#     local binary and image (NELCOTA_BIN, NELCOTA_IMAGE:NELCOTA_TEST_VERSION).
#     Exercises: HTTPS, migrate, signup, RLS, types, backup/restore, upgrade
#     rollback, isolation between projects, single sign-on (SSO) between panels,
#     switching to per-project login and project removal.
set -euo pipefail

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
fail() { printf '\nFAILED: %s\n' "$*" >&2; exit 1; }
now() { date +%s; }

if [ "${1:-}" != "--local" ]; then
  : "${ACCEPT_DOMAIN:?set ACCEPT_DOMAIN or use --local}"
  start=$(now)
  curl -fsSL "${NELCOTA_INSTALL_URL:-https://nelcota.com/install}" | sh
  mkdir -p /opt/nelcota && cd /opt/nelcota
  nelcota init "$ACCEPT_DOMAIN" --yes
  nelcota up
  until curl -fsS "https://$ACCEPT_DOMAIN/health" >/dev/null; do sleep 1; done
  echo "From zero to HTTPS in $(( $(now) - start ))s"
  exit 0
fi

BIN="${NELCOTA_BIN:-target/release/nelcota}"
IMAGE="${NELCOTA_IMAGE:-nelcota}"
VERSION="${NELCOTA_TEST_VERSION:-dev}"
[ -x "$BIN" ] || [ -x "$BIN.exe" ] || fail "binary not found: $BIN (cargo build --release)"

WORK="$(mktemp -d)"
ROOT="$WORK"
command -v cygpath >/dev/null 2>&1 && ROOT="$(cygpath -w "$WORK")"
native() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
nelcota() { "$BIN" -C "$ROOT" "$@"; }
cleanup() { nelcota down --all --volumes >/dev/null 2>&1 || true; rm -rf "$WORK"; }
trap cleanup EXIT

# *.localhost points at this machine; --resolve avoids depending on the resolver.
CURL=(curl -ksS --fail-with-body --resolve shop.localhost:443:127.0.0.1 --resolve blog.localhost:443:127.0.0.1)
SHOP="https://shop.localhost"
BLOG="https://blog.localhost"
code_of() { curl -ks --resolve shop.localhost:443:127.0.0.1 --resolve blog.localhost:443:127.0.0.1 -o /dev/null -w '%{http_code}' "$@" || true; }

say "1-3. init (2 projects) + up (timed)"
start=$(now)
nelcota init --local --project shop --yes --image "$IMAGE" --version "$VERSION" | tee "$WORK/init-shop.log"
nelcota init --local --project blog --yes --image "$IMAGE" --version "$VERSION" | tee "$WORK/init-blog.log"
nelcota up
until "${CURL[@]}" "$SHOP/health" >/dev/null 2>&1 && "${CURL[@]}" "$BLOG/health" >/dev/null 2>&1; do sleep 1; done
elapsed=$(( $(now) - start ))
echo "Two projects with HTTPS in ${elapsed}s"

say "Projects"
nelcota projects | tee "$WORK/projects.txt"
grep -q "shop.localhost" "$WORK/projects.txt" || fail "projects without shop"
grep -q "blog.localhost" "$WORK/projects.txt" || fail "projects without blog"

say "SQL migration on shop only"
cat > "$WORK/projects/shop/migrations/V1__notes.sql" <<'SQL'
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
nelcota -p shop migrate
nelcota -p shop migrate   # idempotent

say "Signup + API under RLS (via Caddy/HTTPS)"
session=$("${CURL[@]}" -X POST "$SHOP/auth/v1/signup" -H 'content-type: application/json' \
  -d '{"email":"acceptance@example.com","password":"strong-password-123"}')
token=$(printf '%s' "$session" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
[ -n "$token" ] || fail "signup returned no access_token: $session"
auth=(-H "authorization: Bearer $token" -H 'content-type: application/json')
for _ in $(seq 1 20); do
  "${CURL[@]}" "${auth[@]}" "$SHOP/rest/v1/notes" >/dev/null 2>&1 && break; sleep 0.5
done
"${CURL[@]}" "${auth[@]}" -X POST "$SHOP/rest/v1/notes" -d '{"body":"before the backup"}' >/dev/null
"${CURL[@]}" "${auth[@]}" "$SHOP/rest/v1/notes" | grep -q "before the backup" || fail "note not found"
[ "$(code_of "$SHOP/rest/v1/notes")" = "401" ] || fail "anon should get 401"
echo "ok: the user reads their own note; anon gets 401"

say "Isolation between projects"
[ "$(code_of -H "authorization: Bearer $token" "$BLOG/rest/v1/")" = "401" ] || fail "a shop JWT must not work on blog"
[ "$(code_of "$BLOG/rest/v1/notes")" = "404" ] || fail "the shop table must not exist on blog"
echo "ok: separate keys, users and tables"

say "Single sign-on between panels (SSO)"
admin_email=$(grep -m1 -o 'email: .*' "$WORK/init-shop.log" | awk '{print $2}' | tr -d '\r')
admin_pass=$(grep -m1 -o 'password: .*' "$WORK/init-shop.log" | awk '{print $2}' | tr -d '\r')
[ -n "$admin_pass" ] || fail "the host password did not show up in init"
if grep -q 'password:' "$WORK/init-blog.log"; then fail "the 2nd project should not generate a new password with single sign-on"; fi
login_json="{\"email\":\"$admin_email\",\"password\":\"$admin_pass\"}"
jar="$WORK/shop.cookies"
[ "$(code_of -c "$jar" -X POST "$SHOP/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "200" ] \
  || fail "login on the shop panel failed"
"${CURL[@]}" -b "$jar" "$SHOP/admin/api/overview" | grep -q '"notes"' || fail "the shop panel does not list notes"
"${CURL[@]}" -b "$jar" "$SHOP/admin/api/projects" | grep -q '"blog"' || fail "the switcher does not list blog"
handoff=$("${CURL[@]}" -b "$jar" -X POST "$SHOP/admin/api/sso/handoff" \
  -H 'content-type: application/json' -d '{"project":"blog"}')
sso_token=$(printf '%s' "$handoff" | sed -n 's/.*#sso=\([^"]*\)".*/\1/p')
[ -n "$sso_token" ] || fail "handoff without token: $handoff"
blog_jar="$WORK/blog.cookies"
[ "$(code_of -c "$blog_jar" -X POST "$BLOG/admin/api/sso" -H 'content-type: application/json' -d "{\"token\":\"$sso_token\"}")" = "200" ] \
  || fail "handoff refused on blog"
"${CURL[@]}" -b "$blog_jar" "$BLOG/admin/api/session" | grep -q '"project":"blog"' || fail "blog session not created"
[ "$(code_of -X POST "$BLOG/admin/api/sso" -H 'content-type: application/json' -d "{\"token\":\"$sso_token\"}")" = "401" ] \
  || fail "handoff token reused"
echo "ok: signing in to shop opened blog; single-use token"

say "Per-project login and back to single sign-on"
nelcota panel-login per-project
[ "$(code_of -X POST "$SHOP/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "401" ] \
  || fail "the host password must not work with per-project login"
nelcota panel-login shared
[ "$(code_of -X POST "$SHOP/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "200" ] \
  || fail "the host password should work again"
echo "ok: login modes switched and applied"

say "Shop TypeScript types"
nelcota -p shop types -o "$(native "$WORK/database.ts")"
grep -q "notes" "$WORK/database.ts" || fail "types without the notes table"

say "Backup → write → restore (shop)"
nelcota -p shop backup --keep 3
dump=$(ls "$WORK"/projects/shop/backups/*.dump | tail -1)
"${CURL[@]}" "${auth[@]}" -X POST "$SHOP/rest/v1/notes" -d '{"body":"after the backup"}' >/dev/null
nelcota -p shop restore "$(native "$dump")" --yes
body=$("${CURL[@]}" "${auth[@]}" "$SHOP/rest/v1/notes")
echo "$body" | grep -q "before the backup" || fail "restore lost data from the backup"
if echo "$body" | grep -q "after the backup"; then fail "restore did not go back to the backup state"; fi
echo "ok: backup state restored"

say "Upgrade to a missing version → automatic rollback (shop)"
if nelcota -p shop upgrade --version does-not-exist-999; then fail "upgrade should have failed"; fi
grep -q "NELCOTA_VERSION=$VERSION" "$WORK/projects/shop/.env" || fail "version did not go back to $VERSION"
"${CURL[@]}" "${auth[@]}" "$SHOP/rest/v1/notes" | grep -q "before the backup" || fail "data lost in the rollback"
"${CURL[@]}" "$BLOG/health" >/dev/null || fail "blog must not be affected by the shop upgrade"
echo "ok: rollback kept version and data; blog untouched"

say "Remove blog"
nelcota -p blog remove --yes
ls "$WORK"/archive/*.dump >/dev/null 2>&1 || fail "remove without a final backup in archive/"
[ "$(code_of "$BLOG/health")" != "200" ] || fail "blog still answers after remove"
"${CURL[@]}" "$SHOP/health" >/dev/null || fail "shop went down when blog was removed"
if nelcota projects | grep -q "blog"; then fail "blog is still listed"; fi
echo "ok: blog removed (backup in archive/), shop up"

say "Idle memory"
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}' | grep -E 'NAME|nelcota-(shop|edge)' || true

say "ACCEPTED: two projects with HTTPS in ${elapsed}s"
