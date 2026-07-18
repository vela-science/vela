#!/usr/bin/env bash
set -euo pipefail

REPO="vela-science/vela"
PREFIX="${VELA_INSTALL_PREFIX:-/usr/local}"
BINDIR="${VELA_INSTALL_BINDIR:-$PREFIX/bin}"
POLICYDIR="${VELA_POLKIT_POLICY_DIR:-/usr/share/polkit-1/actions}"
ACTION="${1:-install}"

case "$ACTION" in
  install|upgrade) ;;
  uninstall)
    for binary in vela vela-signer; do
      if [ -e "$BINDIR/$binary" ]; then
        if [ -w "$BINDIR/$binary" ]; then rm -f "$BINDIR/$binary"; else sudo rm -f "$BINDIR/$binary"; fi
      fi
    done
    if [ -e "$POLICYDIR/science.vela.signer.policy" ]; then
      if [ -w "$POLICYDIR/science.vela.signer.policy" ]; then
        rm -f "$POLICYDIR/science.vela.signer.policy"
      else
        sudo rm -f "$POLICYDIR/science.vela.signer.policy"
      fi
    fi
    echo "Removed Vela binaries and platform integration. Frontier and identity data were preserved."
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
    echo "No prebuilt bundle for Intel macOS (x86_64). Build both binaries from source:" >&2
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
(
  cd "$TMP"
  shasum -a 256 -c "$ASSET.sha256"
)
gh attestation verify "$TMP/$ASSET" \
  --repo "$REPO" \
  --signer-workflow "$REPO/.github/workflows/release.yml" \
  --source-ref "refs/tags/$TAG" >/dev/null
mkdir -p "$TMP/unpack"
if [ "$OS" = "darwin" ]; then
  ditto -x -k "$TMP/$ASSET" "$TMP/unpack"
else
  tar -C "$TMP/unpack" -xzf "$TMP/$ASSET"
fi
for binary in vela vela-signer; do
  test -f "$TMP/unpack/$binary"
  chmod +x "$TMP/unpack/$binary"
done

if [ "$OS" = "darwin" ]; then
  for binary in vela vela-signer; do
    codesign --verify --strict --verbose=2 "$TMP/unpack/$binary"
    spctl --assess --type execute --verbose=2 "$TMP/unpack/$binary"
  done
fi

mkdir -p "$BINDIR" 2>/dev/null || true
for binary in vela vela-signer; do
  if [[ -w "$BINDIR" ]]; then
    install "$TMP/unpack/$binary" "$BINDIR/$binary"
  else
    sudo install "$TMP/unpack/$binary" "$BINDIR/$binary"
  fi
done

if [ "$OS" = "linux" ]; then
  POLICY="$TMP/unpack/share/polkit-1/actions/science.vela.signer.policy"
  test -f "$POLICY"
  if [[ -w "$POLICYDIR" ]]; then
    mkdir -p "$POLICYDIR"
    install -m 0644 "$POLICY" "$POLICYDIR/science.vela.signer.policy"
  else
    sudo mkdir -p "$POLICYDIR"
    sudo install -m 0644 "$POLICY" "$POLICYDIR/science.vela.signer.policy"
  fi
fi

echo "Installed vela and vela-signer to $BINDIR"
"$BINDIR/vela" --version
"$BINDIR/vela-signer" --version

if [[ ":$PATH:" != *":$BINDIR:"* ]]; then
  echo
  echo "Note: $BINDIR is not on PATH. Add it before running Vela."
fi

echo
echo "Quick start in a reviewed frontier clone:"
echo "  1) verify:  vela check . --strict --json"
echo "  2) inspect: vela next . --json"
echo "  3) claim:   vela work <target> --frontier . --as agent:<you> --json"
echo "Producer guide: https://github.com/vela-science/vela/blob/${TAG}/docs/PRODUCER_QUICKSTART.md"
