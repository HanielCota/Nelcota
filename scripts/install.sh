#!/bin/sh
# nelcota installer: downloads the binary, checks the checksum and installs it.
#
#   curl -fsSL https://nelcota.dev/install | sh
#
# Optional variables:
#   NELCOTA_VERSION=0.1.0     version (default: the latest)
#   NELCOTA_REPO=owner/repo   GitHub repository (default: nelcota/nelcota)
#   PREFIX=/usr/local/bin     where to install
#   NELCOTA_SKIP_DOCKER=1     do not install Docker if it is missing
#
# Rather not run `curl | sh`? Download the binary and the .sha256 from the
# releases page and check them with `sha256sum -c`. This script does exactly that.
set -eu

REPO="${NELCOTA_REPO:-nelcota/nelcota}"
VERSION="${NELCOTA_VERSION:-latest}"
PREFIX="${PREFIX:-/usr/local/bin}"

say() { printf '%s\n' "→ $*"; }
die() { printf '%s\n' "error: $*" >&2; exit 1; }

[ "$(uname -s)" = "Linux" ] || die "the installer is for Linux (on macOS/Windows use 'cargo install --git https://github.com/$REPO nelcota-server')"

case "$(uname -m)" in
  x86_64|amd64) TARGET="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) TARGET="aarch64-unknown-linux-musl" ;;
  *) die "unsupported architecture: $(uname -m)" ;;
esac

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  command -v sudo >/dev/null 2>&1 && SUDO="sudo" || die "run as root (or install sudo)"
fi

if [ "$VERSION" = "latest" ]; then
  BASE="https://github.com/$REPO/releases/latest/download"
else
  BASE="https://github.com/$REPO/releases/download/v$VERSION"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
BIN="nelcota-$TARGET"

say "Downloading $BIN ($VERSION)"
curl -fsSL "$BASE/$BIN" -o "$TMP/$BIN"
curl -fsSL "$BASE/$BIN.sha256" -o "$TMP/$BIN.sha256"

say "Checking the checksum (SHA-256)"
(cd "$TMP" && sha256sum -c "$BIN.sha256") >/dev/null || die "checksum does not match; aborted"

$SUDO install -m 0755 "$TMP/$BIN" "$PREFIX/nelcota"
say "nelcota installed at $PREFIX/nelcota ($("$PREFIX/nelcota" --version))"

if ! command -v docker >/dev/null 2>&1; then
  if [ "${NELCOTA_SKIP_DOCKER:-0}" = "1" ]; then
    say "Docker not found (NELCOTA_SKIP_DOCKER=1): install it before 'nelcota init'"
  else
    say "Docker not found: installing it with the official script (get.docker.com)"
    curl -fsSL https://get.docker.com | $SUDO sh
  fi
fi

cat <<'MSG'

Next steps:
  nelcota init api.yourdomain.com    # point DNS (A record) at this machine first
  nelcota up
MSG
