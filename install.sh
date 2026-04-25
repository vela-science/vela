#!/usr/bin/env bash
set -euo pipefail

REPO="vela-science/vela"
BINARY="vela"
PREFIX="${VELA_INSTALL_PREFIX:-/usr/local}"
BINDIR="${VELA_INSTALL_BINDIR:-$PREFIX/bin}"

# Detect OS and arch
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
  darwin-arm64|darwin-aarch64) NAME="vela-macos-aarch64" ;;
  darwin-x86_64) NAME="vela-macos-x86_64" ;;
  linux-x86_64)  NAME="vela-linux-x86_64" ;;
  *) echo "Unsupported: ${OS}-${ARCH}"; exit 1 ;;
esac

# Get latest release
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
URL="https://github.com/${REPO}/releases/download/${LATEST}/${NAME}"
SUM_URL="${URL}.sha256"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Installing vela ${LATEST} for ${OS}/${ARCH}..."
curl -fsSL "$URL" -o "$TMP/$BINARY"

if curl -fsSL "$SUM_URL" -o "$TMP/$BINARY.sha256"; then
  (
    cd "$TMP"
    shasum -a 256 -c "$BINARY.sha256"
  )
else
  echo "Checksum file not found for ${NAME}; continuing without checksum verification."
fi

chmod +x "$TMP/$BINARY"
mkdir -p "$BINDIR" 2>/dev/null || true
if [[ -w "$BINDIR" ]]; then
  install "$TMP/$BINARY" "$BINDIR/$BINARY"
else
  sudo install "$TMP/$BINARY" "$BINDIR/$BINARY"
fi

echo "Installed vela to $BINDIR/$BINARY"
"$BINDIR/$BINARY" --version

if ! command -v "$BINDIR/$BINARY" >/dev/null 2>&1 && [[ ":$PATH:" != *":$BINDIR:"* ]]; then
  echo
  echo "Note: $BINDIR is not on PATH. Add it before running vela directly."
fi

echo
echo "Quick start (v0 frontier workflow):"
echo "  1) compile: vela compile \"blood brain barrier Alzheimer\" -n 10 --output frontier.json"
echo "  2) check:   vela check frontier.json"
echo "  3) proof:   vela proof frontier.json --out proof-packet"
echo "  4) serve:   vela serve frontier.json"
echo "  5) bench:   vela bench frontiers/bbb-alzheimer.json --gold benchmarks/gold-50.json"
echo
echo "Then use search/export for your review workflow:"
echo "  - vela search \"LRP1 RAGE amyloid\" --source frontier.json"
echo "  - vela export frontier.json --format packet --output proof-packet"
