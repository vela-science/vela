#!/usr/bin/env bash
#
# Install Vela, and verify it without needing the provider it came from.
#
# `docs/CONTINUITY.md` requires the system to be installable and verifiable when
# GitHub is not reachable. This script used to make that impossible on its own
# terms: it resolved the version through `api.github.com`, hard-required `gh`,
# and verified through `gh attestation verify --signer-workflow`, which is
# GitHub's OIDC provenance service. Every one of those is the provider.
#
# There are now two verification paths, and the script says which one it used.
#
#   1. The signed release manifest. `scripts/release.sh` mints
#      `<asset>.release-manifest.json` binding release identity, commit, tree,
#      toolchain, asset digests and SBOM digests, and signs it under the
#      `vela-release` namespace with the distribution identity in
#      `allowed_signers`. Verification is `ssh-keygen -Y verify` plus a
#      checksum comparison — OpenSSH and coreutils, no API, no `gh`, and it
#      works against a mirror or a directory on a USB stick.
#
#   2. GitHub attestation, when the manifest is absent. Releases published
#      before the manifest existed have no way to offer path 1, and silently
#      downgrading them to "checksum only" would be a weaker install wearing
#      the same output. So they still verify the old way, and say so.
#
# What is deliberately NOT here: a path that installs an unverified binary.
# `<asset>.sha256` sits beside `<asset>` on the same host, so on its own it
# proves transport integrity and nothing about who built the bytes.
#
# environment:
#   VELA_VERSION           tag to install; skips the api.github.com lookup
#   VELA_RELEASE_BASE_URL  where to fetch release files from — a mirror, or a
#                          `file://` directory holding a retained release
#   VELA_ALLOWED_SIGNERS   path to an out-of-band allowed_signers file, so the
#                          trust root need not come from the same host as the
#                          bytes it verifies
#   VELA_EXPECTED_SHA256   archive digest from an ecosystem lock
#   VELA_INSTALL_PREFIX    install prefix (default /usr/local)
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
    echo "No prebuilt bundle for Intel macOS (x86_64). From an exact tagged source checkout, run:" >&2
    echo "  cargo install --locked --path crates/vela-cli" >&2
    exit 1
    ;;
  *) echo "Unsupported: ${OS}-${ARCH}" >&2; exit 1 ;;
esac

TAG="${VELA_VERSION:-}"
if [ -z "$TAG" ]; then
  if [ -n "${VELA_RELEASE_BASE_URL:-}" ]; then
    # A mirror has no releases API to ask, and guessing "latest" against bytes
    # somebody else arranged is how you install a version nobody chose.
    echo "ERROR: set VELA_VERSION when VELA_RELEASE_BASE_URL is set." >&2
    exit 1
  fi
  TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
fi
BASE_URL="${VELA_RELEASE_BASE_URL:-https://github.com/${REPO}/releases/download/${TAG}}"
URL="${BASE_URL}/${ASSET}"
SUM_URL="${URL}.sha256"
MANIFEST_NAME="${ASSET}.release-manifest.json"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

digest_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

echo "Installing Vela ${TAG} for ${OS}/${ARCH} from ${BASE_URL}..."
curl -fsSL "$URL" -o "$TMP/$ASSET"
curl -fsSL "$SUM_URL" -o "$TMP/$ASSET.sha256" || {
  echo "ERROR: checksum missing for ${ASSET}; refusing an unverified install." >&2
  exit 1
}
(
  cd "$TMP"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$ASSET.sha256"
  else
    shasum -a 256 -c "$ASSET.sha256"
  fi
)
if [ -n "${VELA_EXPECTED_SHA256:-}" ]; then
  OBSERVED_SHA256="$(digest_of "$TMP/$ASSET")"
  if [ "$OBSERVED_SHA256" != "$VELA_EXPECTED_SHA256" ]; then
    echo "ERROR: ${ASSET} differs from the ecosystem-lock SHA-256; refusing installation." >&2
    exit 1
  fi
fi

# The distribution identity, as published in `allowed_signers` at the tag this
# script ships with. Inline so the script carries its own trust root: fetching
# the verifier from the host that served the bytes would verify nothing. Supply
# VELA_ALLOWED_SIGNERS to pin it out of band instead.
read -r -d '' EMBEDDED_ALLOWED_SIGNERS <<'SIGNERS' || true
release@vela.space namespaces="vela-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDZ+fwqljQBzprznZmeAY3KwHVrKOaW+/z5GflVwilG5 Vela release distribution
SIGNERS

VERIFIED_BY=""
SIGNED_MANIFEST=""
if curl -fsSL "${BASE_URL}/${MANIFEST_NAME}" -o "$TMP/$MANIFEST_NAME" 2>/dev/null \
  && curl -fsSL "${BASE_URL}/${MANIFEST_NAME}.sig" -o "$TMP/$MANIFEST_NAME.sig" 2>/dev/null; then
  SIGNED_MANIFEST="yes"
fi

if [ -n "$SIGNED_MANIFEST" ]; then
  command -v ssh-keygen >/dev/null 2>&1 || {
    echo "ERROR: ssh-keygen is required to verify the signed release manifest." >&2
    exit 1
  }
  if [ -n "${VELA_ALLOWED_SIGNERS:-}" ]; then
    SIGNERS_FILE="$VELA_ALLOWED_SIGNERS"
  else
    SIGNERS_FILE="$TMP/allowed_signers"
    printf '%s\n' "$EMBEDDED_ALLOWED_SIGNERS" > "$SIGNERS_FILE"
  fi
  # A signature that is present and wrong is tampering, and there is no falling
  # back from it. Only a wholly absent signature is a state the pipeline itself
  # produces, and that case is handled below.
  ssh-keygen -Y verify -f "$SIGNERS_FILE" -I release@vela.space \
    -n vela-release -s "$TMP/$MANIFEST_NAME.sig" < "$TMP/$MANIFEST_NAME" >/dev/null || {
    echo "ERROR: the release manifest signature did not verify against the distribution identity." >&2
    exit 1
  }
  # A verified manifest is only as good as the tie from it back to these bytes.
  # Without this the signature would attest to a document that merely mentions
  # an asset by name.
  #
  # `sort_keys=True` puts "sha256" directly after "name" inside each asset
  # object, and the closing quote keeps `<asset>` from matching
  # `<asset>.spdx.json`. Fixed-string, so the dots in the filename stay dots.
  # `release_manifest.py` emits `sha256:<hex>`, so the prefix is optional here
  # and the result is checked below rather than trusted: a `sed` that does not
  # match passes the whole line through, which would fail as a digest mismatch
  # and send someone hunting for tampering that never happened.
  MANIFEST_SHA256=$(grep -F -A3 "\"name\": \"${ASSET}\"" "$TMP/$MANIFEST_NAME" \
    | grep '"sha256"' | head -1 | sed -E 's/.*"sha256": "(sha256:)?([0-9a-f]{64})".*/\2/')
  case "$MANIFEST_SHA256" in
    *[!0-9a-f]* | "")
      echo "ERROR: could not read a SHA-256 for ${ASSET} out of ${MANIFEST_NAME}." >&2
      echo "       The manifest verified, so this is a format change rather than tampering." >&2
      exit 1
      ;;
  esac
  if [ "${#MANIFEST_SHA256}" -ne 64 ]; then
    echo "ERROR: the digest for ${ASSET} in ${MANIFEST_NAME} is not 64 hex characters." >&2
    exit 1
  fi
  if [ "$(digest_of "$TMP/$ASSET")" != "$MANIFEST_SHA256" ]; then
    echo "ERROR: ${ASSET} does not match the digest in the signed release manifest." >&2
    exit 1
  fi
  VERIFIED_BY="signed release manifest (provider-independent)"
elif [ -n "${VELA_REQUIRE_SIGNED_MANIFEST:-}" ]; then
  echo "ERROR: VELA_REQUIRE_SIGNED_MANIFEST is set and ${TAG} has no signed manifest." >&2
  exit 1
elif command -v gh >/dev/null 2>&1; then
  # Falling back is not a downgrade to nothing: `gh attestation verify` is a
  # real check against GitHub's provenance, so stripping the `.sig` buys an
  # attacker a different verification rather than none.
  #
  # This branch has to exist because the pipeline produces the state it handles.
  # `release.yml` requires the manifest before publishing and deliberately does
  # not sign it — putting the distribution key in Actions would re-couple the
  # artifact to the provider — so a manifest is published, then signed out of
  # band by an operator. Making a manifest without a signature fatal would have
  # broken every install on the next tag, on every platform, which is strictly
  # worse than the coupling it was trying to remove.
  if [ -f "$TMP/$MANIFEST_NAME" ]; then
    echo "Note: ${TAG} publishes a release manifest with no signature beside it, so it"
    echo "      proves nothing on its own and was ignored. Verifying through GitHub instead."
  fi
  gh attestation verify "$TMP/$ASSET" \
    --repo "$REPO" \
    --signer-workflow "$REPO/.github/workflows/release.yml" \
    --source-ref "refs/tags/$TAG" >/dev/null
  VERIFIED_BY="GitHub attestation (requires GitHub; no signed manifest for this release)"
else
  echo "ERROR: ${TAG} publishes no signed release manifest, and GitHub CLI is not installed," >&2
  echo "       so this build's provenance cannot be checked. Install a release that carries" >&2
  echo "       ${MANIFEST_NAME} and ${MANIFEST_NAME}.sig, or install gh" >&2
  echo "       (https://cli.github.com/). A checksum served beside the archive is not" >&2
  echo "       provenance; refusing." >&2
  exit 1
fi

mkdir -p "$TMP/unpack"
if [ "$OS" = "darwin" ]; then
  ditto -x -k "$TMP/$ASSET" "$TMP/unpack"
else
  tar -C "$TMP/unpack" -xzf "$TMP/$ASSET"
fi
test -f "$TMP/unpack/vela"
chmod +x "$TMP/unpack/vela"

echo "Verified by: ${VERIFIED_BY}"

if [ "$OS" = "darwin" ]; then
  echo "Note: this is a portable build without Apple Developer ID notarization."
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
echo "Quick start in a reviewed repository clone:"
echo "  1) verify:  vela replay . --json"
echo "  2) inspect: vela next . --json"
echo "  3) brief:   vela start <target> --repo . --json  # optional, write-free"
echo "  4) submit:  vela submit --repo . --claim <claim> --type <type> --replayability <class> --artifact <path>:<kind> --caveat <limit> --as agent:<you> --json"
echo "Producer guide: https://github.com/vela-science/vela/blob/${TAG}/docs/PRODUCER_QUICKSTART.md"
