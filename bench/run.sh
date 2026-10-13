#!/usr/bin/env bash
# Reproducible benchmark of the simple read, against a `nelcota init` install.
#   ./bench/run.sh [project-directory]
# Variables: VUS (32), DURATION (30s), BASE_URL (http://app:8000, without Caddy).
set -euo pipefail
PROJECT="${1:-.}"
HERE="$(cd "$(dirname "$0")" && pwd)"
compose() { docker compose --project-directory "$PROJECT" -f "$PROJECT/docker-compose.yml" "$@"; }

echo "== Preparing data (100k rows)"
compose exec -T postgres psql -q -U postgres -v ON_ERROR_STOP=1 < "$HERE/setup.sql"
compose exec -T postgres psql -q -U postgres -c "NOTIFY nelcota, 'reload schema'"
sleep 1

echo "== k6: ${VUS:-32} virtual users for ${DURATION:-30s}"
docker run --rm -i --network nelcota_web \
  -e VUS="${VUS:-32}" -e DURATION="${DURATION:-30s}" -e BASE_URL="${BASE_URL:-http://app:8000}" \
  -e INSECURE_TLS="${INSECURE_TLS:-false}" \
  grafana/k6:1.6.1@sha256:a5ad6bc089a08d77c3ec49f3db8c6fa7a148e4073efcac44c675dbaf3568d8e1 run --quiet - < "$HERE/read.js"

echo "== Memory"
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}\t{{.CPUPerc}}' | grep -E 'NAME|nelcota'
