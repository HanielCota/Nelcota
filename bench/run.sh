#!/usr/bin/env bash
# Benchmark reproduzível da leitura simples, contra uma instalação do `nelcota init`.
#   ./bench/run.sh [diretório-do-projeto]
# Variáveis: VUS (32), DURATION (30s), BASE_URL (http://app:8000, sem o Caddy).
set -euo pipefail
PROJECT="${1:-.}"
HERE="$(cd "$(dirname "$0")" && pwd)"
compose() { docker compose --project-directory "$PROJECT" -f "$PROJECT/docker-compose.yml" "$@"; }

echo "== Preparando dados (100 mil linhas)"
compose exec -T postgres psql -q -U postgres -v ON_ERROR_STOP=1 < "$HERE/setup.sql"
compose exec -T postgres psql -q -U postgres -c "NOTIFY nelcota, 'reload schema'"
sleep 1

echo "== k6: ${VUS:-32} usuários virtuais por ${DURATION:-30s}"
docker run --rm -i --network nelcota_web \
  -e VUS="${VUS:-32}" -e DURATION="${DURATION:-30s}" -e BASE_URL="${BASE_URL:-http://app:8000}" \
  grafana/k6:latest run --quiet - < "$HERE/leitura.js"

echo "== Memória"
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}\t{{.CPUPerc}}' | grep -E 'NAME|nelcota'
