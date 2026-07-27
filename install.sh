#!/usr/bin/env bash
set -euo pipefail

REPO="vela-science/vela"
PREFIX="${VELA_INSTALL_PREFIX:-/usr/local}"
BINDIR="${VELA_INSTALL_BINDIR:-$PREFIX/bin}"
ACTION="${1:-install}"

case "$ACTION" in
  install|upgrade) ;;
  uninstall)
    for binary in vela; do
      if [ -e "$BINDIR/$binary" ]; then
        if [ -w "$BINDIR/$binary" ]; then rm -f "$BINDIR/$binary"; else sudo rm -f "$BINDIR/$binary"; fi
      fi
    done
    echo "Removed Vela. Frontier data was preserved."
    exit 0
    ;;
  -h|--help|help)
    echo "usage: install.sh [install|upgrade|uninstall]"
    exit 0
    ;;
  *) echo "usage: install.sh [install|upgrade|uninstall]" >&2; exit 2 ;;
esac

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
  darwin-arm64|darwin-aarch64) ASSET="vela-macos-aarch64.zip" ;;
  linux-x86_64) ASSET="vela-linux-x86_64.tar.gz" ;;
  darwin-x86_64)
    echo "No prebuilt bundle for Intel macOS (x86_64). Build Vela from source:" >&2
    echo "  cargo install --locked --path crates/vela-cli" >&2
    exit 1
    ;;
  *) echo "Unsupported: ${OS}-${ARCH}" >&2; exit 1 ;;
esac

TAG="${VELA_VERSION:-}"
if [ -z "$TAG" ]; then
  TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
fi
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
SUM_URL="${URL}.sha256"
TRUST_URL="${URL}.trust.json"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Installing Vela ${TAG} for ${OS}/${ARCH}..."
command -v gh >/dev/null 2>&1 || {
  echo "ERROR: GitHub CLI is required to verify build provenance (https://cli.github.com/)." >&2
  exit 1
}
curl -fsSL "$URL" -o "$TMP/$ASSET"
curl -fsSL "$SUM_URL" -o "$TMP/$ASSET.sha256" || {
  echo "ERROR: checksum missing for ${ASSET}; refusing an unverified install." >&2
  exit 1
}
curl -fsSL "$TRUST_URL" -o "$TMP/$ASSET.trust.json" || {
  echo "ERROR: release trust metadata missing for ${ASSET}; refusing installation." >&2
  exit 1
}
curl -fsSL "${TRUST_URL}.sha256" -o "$TMP/$ASSET.trust.json.sha256" || {
  echo "ERROR: release trust-metadata checksum missing for ${ASSET}; refusing installation." >&2
  exit 1
}
(
  cd "$TMP"
  shasum -a 256 -c "$ASSET.sha256"
  shasum -a 256 -c "$ASSET.trust.json.sha256"
)
if [ -n "${VELA_EXPECTED_SHA256:-}" ]; then
  OBSERVED_SHA256=$(shasum -a 256 "$TMP/$ASSET" | awk '{print $1}')
  if [ "$OBSERVED_SHA256" != "$VELA_EXPECTED_SHA256" ]; then
    echo "ERROR: ${ASSET} differs from the ecosystem-lock SHA-256; refusing installation." >&2
    exit 1
  fi
fi
gh attestation verify "$TMP/$ASSET" \
  --repo "$REPO" \
  --signer-workflow "$REPO/.github/workflows/release.yml" \
  --source-ref "refs/tags/$TAG" >/dev/null
gh attestation verify "$TMP/$ASSET.trust.json" \
  --repo "$REPO" \
  --signer-workflow "$REPO/.github/workflows/release.yml" \
  --source-ref "refs/tags/$TAG" >/dev/null
grep -Fq '"schema":"vela.release-trust.v1"' "$TMP/$ASSET.trust.json"
grep -Fq "\"artifact\":\"$ASSET\"" "$TMP/$ASSET.trust.json"
grep -Fq '"artifact_class":"portable"' "$TMP/$ASSET.trust.json"
PLATFORM_SIGNATURE=$(sed -n 's/.*"platform_signature":"\([^"]*\)".*/\1/p' "$TMP/$ASSET.trust.json")
test -n "$PLATFORM_SIGNATURE" || {
  echo "ERROR: release trust metadata omits platform_signature." >&2
  exit 1
}
mkdir -p "$TMP/unpack"
if [ "$OS" = "darwin" ]; then
  ditto -x -k "$TMP/$ASSET" "$TMP/unpack"
else
  tar -C "$TMP/unpack" -xzf "$TMP/$ASSET"
fi
test -f "$TMP/unpack/vela"
chmod +x "$TMP/unpack/vela"

if [ "$OS" = "darwin" ]; then
  if [ "$PLATFORM_SIGNATURE" = "developer_id_notarized" ]; then
    codesign --verify --strict --verbose=2 "$TMP/unpack/vela"
    spctl --assess --type execute --verbose=2 "$TMP/unpack/vela"
  elif [ "$PLATFORM_SIGNATURE" = "absent" ]; then
    echo "Note: this is a GitHub-attested portable build without Apple Developer ID notarization."
  else
    echo "ERROR: unsupported macOS platform-signature tier: $PLATFORM_SIGNATURE" >&2
    exit 1
  fi
fi

mkdir -p "$BINDIR" 2>/dev/null || true
if [[ -w "$BINDIR" ]]; then
  install "$TMP/unpack/vela" "$BINDIR/vela"
else
  sudo install "$TMP/unpack/vela" "$BINDIR/vela"
fi

echo "Installed vela to $BINDIR"
"$BINDIR/vela" --version

if [[ ":$PATH:" != *":$BINDIR:"* ]]; then
  echo
  echo "Note: $BINDIR is not on PATH. Add it before running Vela."
fi

echo
echo "Quick start in a reviewed frontier clone:"
echo "  1) verify:  vela check . --strict --json"
echo "  2) inspect: vela next . --json"
echo "  3) start:   vela start <target> --frontier . --as agent:<you> --json"
echo "  4) submit:  vela submit <attempt> --frontier . --claim <claim> --json"
echo "Producer guide: https://github.com/vela-science/vela/blob/${TAG}/docs/PRODUCER_QUICKSTART.md"
