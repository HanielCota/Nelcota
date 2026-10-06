#!/usr/bin/env bash
# Teste de aceitação do deploy: "máquina limpa → 3 comandos → /health por HTTPS".
#
# Modo VM (o teste de verdade, numa VPS Linux zerada com DNS apontado):
#   ACCEPT_DOMAIN=api.exemplo.com ./scripts/acceptance.sh
#   → curl install | sh ; nelcota init $ACCEPT_DOMAIN --yes ; nelcota up ; mede.
#
# Modo local (simulação, roda em qualquer máquina com Docker e as portas 80/443 livres):
#   ./scripts/acceptance.sh --local
#   → usa o binário e a imagem locais (NELCOTA_BIN, NELCOTA_IMAGE:NELCOTA_TEST_VERSION),
#     HTTPS em https://localhost com certificado interno do Caddy. Além do tempo,
#     exercita migrate, signup, API sob RLS, types, backup/restore e o rollback
#     automático do upgrade.
set -euo pipefail

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
fail() { printf '\nFALHOU: %s\n' "$*" >&2; exit 1; }
now() { date +%s; }

if [ "${1:-}" != "--local" ]; then
  : "${ACCEPT_DOMAIN:?defina ACCEPT_DOMAIN ou use --local}"
  start=$(now)
  curl -fsSL "${NELCOTA_INSTALL_URL:-https://nelcota.dev/install}" | sh
  mkdir -p ~/nelcota && cd ~/nelcota
  nelcota init "$ACCEPT_DOMAIN" --yes
  nelcota up
  until curl -fsS "https://$ACCEPT_DOMAIN/health" >/dev/null; do sleep 1; done
  echo "Do zero a HTTPS em $(( $(now) - start ))s"
  exit 0
fi

BIN="${NELCOTA_BIN:-target/release/nelcota}"
IMAGE="${NELCOTA_IMAGE:-nelcota}"
VERSION="${NELCOTA_TEST_VERSION:-dev}"
[ -x "$BIN" ] || [ -x "$BIN.exe" ] || fail "binário não encontrado: $BIN (cargo build --release)"

WORK="$(mktemp -d)"
DIR="$WORK"
command -v cygpath >/dev/null 2>&1 && DIR="$(cygpath -w "$WORK")"
nelcota() { "$BIN" -C "$DIR" "$@"; }
cleanup() { nelcota down --volumes >/dev/null 2>&1 || true; rm -rf "$WORK"; }
trap cleanup EXIT

CURL=(curl -ksS --fail-with-body)
BASE="https://localhost"

say "1-3. init --local + up (cronometrado)"
start=$(now)
nelcota init --local --yes --image "$IMAGE" --version "$VERSION" | tee "$WORK/init.log"
nelcota up
until "${CURL[@]}" "$BASE/health" >/dev/null 2>&1; do sleep 1; done
elapsed=$(( $(now) - start ))
echo "HTTPS respondendo em ${elapsed}s (sem contar download de imagens já em cache)"

say "Status"
nelcota status

say "Migração SQL do usuário"
cat > "$WORK/migrations/V1__notas.sql" <<'SQL'
CREATE TABLE public.notas (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    dono uuid NOT NULL DEFAULT auth.uid(),
    texto text NOT NULL
);
ALTER TABLE public.notas ENABLE ROW LEVEL SECURITY;
CREATE POLICY dono ON public.notas FOR ALL TO authenticated
    USING (dono = auth.uid()) WITH CHECK (dono = auth.uid());
GRANT SELECT, INSERT, UPDATE, DELETE ON public.notas TO authenticated;
SQL
nelcota migrate
nelcota migrate   # idempotente

say "Signup + API sob RLS (via Caddy/HTTPS)"
session=$("${CURL[@]}" -X POST "$BASE/auth/v1/signup" -H 'content-type: application/json' \
  -d '{"email":"aceite@exemplo.com","password":"senha-forte-123"}')
token=$(printf '%s' "$session" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
[ -n "$token" ] || fail "signup não devolveu access_token: $session"
auth=(-H "authorization: Bearer $token" -H 'content-type: application/json')
for _ in $(seq 1 20); do
  "${CURL[@]}" "${auth[@]}" "$BASE/rest/v1/notas" >/dev/null 2>&1 && break; sleep 0.5
done
"${CURL[@]}" "${auth[@]}" -X POST "$BASE/rest/v1/notas" -d '{"texto":"antes do backup"}' >/dev/null
"${CURL[@]}" "${auth[@]}" "$BASE/rest/v1/notas" | grep -q "antes do backup" || fail "nota não encontrada"
anon=$(curl -ks -o /dev/null -w '%{http_code}' "$BASE/rest/v1/notas")
[ "$anon" = "401" ] || fail "anon deveria receber 401, recebeu $anon"
echo "ok: usuário lê a própria nota; anon recebe 401"

say "Painel (login separado, via HTTPS)"
admin_email=$(grep -o 'Admin do painel: .*' "$WORK/init.log" | awk '{print $4}')
admin_pass=$(grep -o 'Senha: .*' "$WORK/init.log" | awk '{print $2}' | tr -d '\r')
jar="$WORK/cookies.txt"
code=$(curl -ks -o /dev/null -w '%{http_code}' -c "$jar" -X POST "$BASE/admin/api/login" \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$admin_email\",\"password\":\"$admin_pass\"}")
[ "$code" = "200" ] || fail "login no painel falhou (HTTP $code)"
"${CURL[@]}" -b "$jar" "$BASE/admin/api/overview" | grep -q '"notas"' || fail "painel não lista a tabela notas"
"${CURL[@]}" "$BASE/admin/" | grep -q '<div id="app">' || fail "SPA do painel não foi servida"
echo "ok: login no painel, tabela notas listada e SPA servida"

say "Tipos TypeScript"
nelcota types -o "$DIR/database.ts"
grep -q "notas" "$WORK/database.ts" || fail "types sem a tabela notas"

say "Backup → escrita → restore"
nelcota backup --keep 3
dump=$(ls "$WORK"/backups/nelcota-*.dump | tail -1)
"${CURL[@]}" "${auth[@]}" -X POST "$BASE/rest/v1/notas" -d '{"texto":"depois do backup"}' >/dev/null
dump_native="$dump"; command -v cygpath >/dev/null 2>&1 && dump_native="$(cygpath -w "$dump")"
nelcota restore "$dump_native" --yes
body=$("${CURL[@]}" "${auth[@]}" "$BASE/rest/v1/notas")
echo "$body" | grep -q "antes do backup" || fail "restore perdeu dados do backup"
echo "$body" | grep -q "depois do backup" && fail "restore não voltou ao estado do backup"
echo "ok: estado do backup restaurado (sessão do usuário continua válida)"

say "Upgrade para versão inexistente → rollback automático"
if nelcota upgrade --version nao-existe-999; then fail "upgrade deveria ter falhado"; fi
grep -q "NELCOTA_VERSION=$VERSION" "$WORK/.env" || fail "versão não voltou para $VERSION"
"${CURL[@]}" "$BASE/health" >/dev/null || fail "app fora do ar após rollback"
"${CURL[@]}" "${auth[@]}" "$BASE/rest/v1/notas" | grep -q "antes do backup" || fail "dados perdidos no rollback"
echo "ok: rollback manteve versão e dados"

say "Memória em repouso"
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}' | grep -E 'NAME|nelcota' || true

say "ACEITO: HTTPS em ${elapsed}s"
