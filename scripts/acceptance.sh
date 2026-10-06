#!/usr/bin/env bash
# Teste de aceitação do deploy: "máquina limpa → 3 comandos → /health por HTTPS".
#
# Modo VM (o teste de verdade, numa VPS Linux zerada com DNS apontado):
#   ACCEPT_DOMAIN=api.exemplo.com ./scripts/acceptance.sh
#   → curl install | sh ; nelcota init $ACCEPT_DOMAIN --yes ; nelcota up ; mede.
#
# Modo local (simulação; Docker e portas 80/443 livres):
#   ./scripts/acceptance.sh --local
#   → host local com DOIS projetos (loja.localhost e blog.localhost), usando o
#     binário e a imagem locais (NELCOTA_BIN, NELCOTA_IMAGE:NELCOTA_TEST_VERSION).
#     Exercita: HTTPS, migrate, signup, RLS, types, backup/restore, rollback do
#     upgrade, isolamento entre projetos, login único (SSO) entre painéis, troca
#     para login por projeto e remoção de projeto.
set -euo pipefail

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
fail() { printf '\nFALHOU: %s\n' "$*" >&2; exit 1; }
now() { date +%s; }

if [ "${1:-}" != "--local" ]; then
  : "${ACCEPT_DOMAIN:?defina ACCEPT_DOMAIN ou use --local}"
  start=$(now)
  curl -fsSL "${NELCOTA_INSTALL_URL:-https://nelcota.dev/install}" | sh
  mkdir -p /opt/nelcota && cd /opt/nelcota
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
ROOT="$WORK"
command -v cygpath >/dev/null 2>&1 && ROOT="$(cygpath -w "$WORK")"
native() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
nelcota() { "$BIN" -C "$ROOT" "$@"; }
cleanup() { nelcota down --all --volumes >/dev/null 2>&1 || true; rm -rf "$WORK"; }
trap cleanup EXIT

# *.localhost aponta para a própria máquina; --resolve evita depender do resolvedor.
CURL=(curl -ksS --fail-with-body --resolve loja.localhost:443:127.0.0.1 --resolve blog.localhost:443:127.0.0.1)
LOJA="https://loja.localhost"
BLOG="https://blog.localhost"
code_of() { curl -ks --resolve loja.localhost:443:127.0.0.1 --resolve blog.localhost:443:127.0.0.1 -o /dev/null -w '%{http_code}' "$@" || true; }

say "1-3. init (2 projetos) + up (cronometrado)"
start=$(now)
nelcota init --local --project loja --yes --image "$IMAGE" --version "$VERSION" | tee "$WORK/init-loja.log"
nelcota init --local --project blog --yes --image "$IMAGE" --version "$VERSION" | tee "$WORK/init-blog.log"
nelcota up
until "${CURL[@]}" "$LOJA/health" >/dev/null 2>&1 && "${CURL[@]}" "$BLOG/health" >/dev/null 2>&1; do sleep 1; done
elapsed=$(( $(now) - start ))
echo "Dois projetos com HTTPS em ${elapsed}s"

say "Projetos"
nelcota projects | tee "$WORK/projects.txt"
grep -q "loja.localhost" "$WORK/projects.txt" || fail "projects sem loja"
grep -q "blog.localhost" "$WORK/projects.txt" || fail "projects sem blog"

say "Migração SQL só na loja"
cat > "$WORK/projects/loja/migrations/V1__notas.sql" <<'SQL'
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
nelcota -p loja migrate
nelcota -p loja migrate   # idempotente

say "Signup + API sob RLS (via Caddy/HTTPS)"
session=$("${CURL[@]}" -X POST "$LOJA/auth/v1/signup" -H 'content-type: application/json' \
  -d '{"email":"aceite@exemplo.com","password":"senha-forte-123"}')
token=$(printf '%s' "$session" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)
[ -n "$token" ] || fail "signup não devolveu access_token: $session"
auth=(-H "authorization: Bearer $token" -H 'content-type: application/json')
for _ in $(seq 1 20); do
  "${CURL[@]}" "${auth[@]}" "$LOJA/rest/v1/notas" >/dev/null 2>&1 && break; sleep 0.5
done
"${CURL[@]}" "${auth[@]}" -X POST "$LOJA/rest/v1/notas" -d '{"texto":"antes do backup"}' >/dev/null
"${CURL[@]}" "${auth[@]}" "$LOJA/rest/v1/notas" | grep -q "antes do backup" || fail "nota não encontrada"
[ "$(code_of "$LOJA/rest/v1/notas")" = "401" ] || fail "anon deveria receber 401"
echo "ok: usuário lê a própria nota; anon recebe 401"

say "Isolamento entre projetos"
[ "$(code_of -H "authorization: Bearer $token" "$BLOG/rest/v1/")" = "401" ] || fail "JWT da loja não pode valer no blog"
[ "$(code_of "$BLOG/rest/v1/notas")" = "404" ] || fail "a tabela da loja não pode existir no blog"
echo "ok: chaves, usuários e tabelas separados"

say "Login único entre painéis (SSO)"
admin_email=$(grep -m1 -o 'email: .*' "$WORK/init-loja.log" | awk '{print $2}' | tr -d '\r')
admin_pass=$(grep -m1 -o 'senha: .*' "$WORK/init-loja.log" | awk '{print $2}' | tr -d '\r')
[ -n "$admin_pass" ] || fail "senha do host não apareceu no init"
if grep -q 'senha:' "$WORK/init-blog.log"; then fail "o 2º projeto não deveria gerar senha nova no login único"; fi
login_json="{\"email\":\"$admin_email\",\"password\":\"$admin_pass\"}"
jar="$WORK/loja.cookies"
[ "$(code_of -c "$jar" -X POST "$LOJA/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "200" ] \
  || fail "login no painel da loja falhou"
"${CURL[@]}" -b "$jar" "$LOJA/admin/api/overview" | grep -q '"notas"' || fail "painel da loja não lista notas"
"${CURL[@]}" -b "$jar" "$LOJA/admin/api/projects" | grep -q '"blog"' || fail "seletor não lista o blog"
handoff=$("${CURL[@]}" -b "$jar" -X POST "$LOJA/admin/api/sso/handoff" \
  -H 'content-type: application/json' -d '{"project":"blog"}')
sso_token=$(printf '%s' "$handoff" | sed -n 's/.*#sso=\([^"]*\)".*/\1/p')
[ -n "$sso_token" ] || fail "handoff sem token: $handoff"
blog_jar="$WORK/blog.cookies"
[ "$(code_of -c "$blog_jar" -X POST "$BLOG/admin/api/sso" -H 'content-type: application/json' -d "{\"token\":\"$sso_token\"}")" = "200" ] \
  || fail "handoff recusado no blog"
"${CURL[@]}" -b "$blog_jar" "$BLOG/admin/api/session" | grep -q '"project":"blog"' || fail "sessão do blog não criada"
[ "$(code_of -X POST "$BLOG/admin/api/sso" -H 'content-type: application/json' -d "{\"token\":\"$sso_token\"}")" = "401" ] \
  || fail "token de handoff reutilizado"
echo "ok: login na loja abriu o blog; token de uso único"

say "Login por projeto e volta ao login único"
nelcota panel-login per-project
[ "$(code_of -X POST "$LOJA/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "401" ] \
  || fail "senha do host não pode valer no login por projeto"
nelcota panel-login shared
[ "$(code_of -X POST "$LOJA/admin/api/login" -H 'content-type: application/json' -d "$login_json")" = "200" ] \
  || fail "senha do host deveria voltar a valer"
echo "ok: modos de login trocados e aplicados"

say "Tipos TypeScript da loja"
nelcota -p loja types -o "$(native "$WORK/database.ts")"
grep -q "notas" "$WORK/database.ts" || fail "types sem a tabela notas"

say "Backup → escrita → restore (loja)"
nelcota -p loja backup --keep 3
dump=$(ls "$WORK"/projects/loja/backups/*.dump | tail -1)
"${CURL[@]}" "${auth[@]}" -X POST "$LOJA/rest/v1/notas" -d '{"texto":"depois do backup"}' >/dev/null
nelcota -p loja restore "$(native "$dump")" --yes
body=$("${CURL[@]}" "${auth[@]}" "$LOJA/rest/v1/notas")
echo "$body" | grep -q "antes do backup" || fail "restore perdeu dados do backup"
if echo "$body" | grep -q "depois do backup"; then fail "restore não voltou ao estado do backup"; fi
echo "ok: estado do backup restaurado"

say "Upgrade para versão inexistente → rollback automático (loja)"
if nelcota -p loja upgrade --version nao-existe-999; then fail "upgrade deveria ter falhado"; fi
grep -q "NELCOTA_VERSION=$VERSION" "$WORK/projects/loja/.env" || fail "versão não voltou para $VERSION"
"${CURL[@]}" "${auth[@]}" "$LOJA/rest/v1/notas" | grep -q "antes do backup" || fail "dados perdidos no rollback"
"${CURL[@]}" "$BLOG/health" >/dev/null || fail "o blog não pode ser afetado pelo upgrade da loja"
echo "ok: rollback manteve versão e dados; blog intacto"

say "Remover o blog"
nelcota -p blog remove --yes
ls "$WORK"/archive/*.dump >/dev/null 2>&1 || fail "remove sem backup final em archive/"
[ "$(code_of "$BLOG/health")" != "200" ] || fail "blog continua respondendo depois do remove"
"${CURL[@]}" "$LOJA/health" >/dev/null || fail "a loja caiu ao remover o blog"
if nelcota projects | grep -q "blog"; then fail "blog continua listado"; fi
echo "ok: blog removido (backup em archive/), loja no ar"

say "Memória em repouso"
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}' | grep -E 'NAME|nelcota-(loja|edge)' || true

say "ACEITO: dois projetos com HTTPS em ${elapsed}s"
