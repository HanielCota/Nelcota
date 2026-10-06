#!/bin/sh
# Instalador do nelcota: baixa o binário, confere o checksum e instala.
#
#   curl -fsSL https://nelcota.dev/install | sh
#
# Variáveis opcionais:
#   NELCOTA_VERSION=0.1.0     versão (padrão: a mais recente)
#   NELCOTA_REPO=dono/repo    repositório no GitHub (padrão: nelcota/nelcota)
#   PREFIX=/usr/local/bin     onde instalar
#   NELCOTA_SKIP_DOCKER=1     não instala o Docker se ele faltar
#
# Prefere não rodar `curl | sh`? Baixe o binário e o .sha256 da página de
# releases e confira com `sha256sum -c`. Este script faz exatamente isso.
set -eu

REPO="${NELCOTA_REPO:-nelcota/nelcota}"
VERSION="${NELCOTA_VERSION:-latest}"
PREFIX="${PREFIX:-/usr/local/bin}"

say() { printf '%s\n' "→ $*"; }
die() { printf '%s\n' "erro: $*" >&2; exit 1; }

[ "$(uname -s)" = "Linux" ] || die "o instalador é para Linux (no macOS/Windows use 'cargo install --git https://github.com/$REPO nelcota-server')"

case "$(uname -m)" in
  x86_64|amd64) TARGET="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) TARGET="aarch64-unknown-linux-musl" ;;
  *) die "arquitetura não suportada: $(uname -m)" ;;
esac

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  command -v sudo >/dev/null 2>&1 && SUDO="sudo" || die "rode como root (ou instale o sudo)"
fi

if [ "$VERSION" = "latest" ]; then
  BASE="https://github.com/$REPO/releases/latest/download"
else
  BASE="https://github.com/$REPO/releases/download/v$VERSION"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
BIN="nelcota-$TARGET"

say "Baixando $BIN ($VERSION)"
curl -fsSL "$BASE/$BIN" -o "$TMP/$BIN"
curl -fsSL "$BASE/$BIN.sha256" -o "$TMP/$BIN.sha256"

say "Conferindo o checksum (SHA-256)"
(cd "$TMP" && sha256sum -c "$BIN.sha256") >/dev/null || die "checksum não confere; abortado"

$SUDO install -m 0755 "$TMP/$BIN" "$PREFIX/nelcota"
say "nelcota instalado em $PREFIX/nelcota ($("$PREFIX/nelcota" --version))"

if ! command -v docker >/dev/null 2>&1; then
  if [ "${NELCOTA_SKIP_DOCKER:-0}" = "1" ]; then
    say "Docker não encontrado (NELCOTA_SKIP_DOCKER=1): instale antes do 'nelcota init'"
  else
    say "Docker não encontrado: instalando com o script oficial (get.docker.com)"
    curl -fsSL https://get.docker.com | $SUDO sh
  fi
fi

cat <<'MSG'

Próximos passos:
  nelcota init api.seudominio.com    # aponte o DNS (registro A) para esta máquina antes
  nelcota up
MSG
