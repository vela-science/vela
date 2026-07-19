#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="${1:?usage: smoke-release-bundle.sh <archive> <version> <require-platform-signature>}"
EXPECTED_VERSION="${2:?usage: smoke-release-bundle.sh <archive> <version> <require-platform-signature>}"
REQUIRE_PLATFORM_SIGNATURE="${3:?usage: smoke-release-bundle.sh <archive> <version> <require-platform-signature>}"

case "$REQUIRE_PLATFORM_SIGNATURE" in
  true|false) ;;
  *) echo "require-platform-signature must be true or false" >&2; exit 2 ;;
esac

test -f "$ARCHIVE"
test -f "$ARCHIVE.sha256"
test -f "$ARCHIVE.spdx.json"
test -f "$ARCHIVE.spdx.json.sha256"
(
  cd "$(dirname "$ARCHIVE")"
  shasum -a 256 -c "$(basename "$ARCHIVE").sha256"
  shasum -a 256 -c "$(basename "$ARCHIVE").spdx.json.sha256"
)

ROOT="$(mktemp -d)"
trap 'rm -rf "$ROOT"' EXIT
UNPACK="$ROOT/unpack"
PREFIX="$ROOT/prefix"
mkdir -p "$UNPACK" "$PREFIX/bin" "$ROOT/home"

case "$ARCHIVE" in
  *.zip) ditto -x -k "$ARCHIVE" "$UNPACK" ;;
  *.tar.gz) tar -C "$UNPACK" -xzf "$ARCHIVE" ;;
  *) echo "unsupported release archive: $ARCHIVE" >&2; exit 2 ;;
esac

test -f "$UNPACK/vela"
test -f "$UNPACK/vela-signer"
test -x "$UNPACK/vela"
test -x "$UNPACK/vela-signer"
test -z "$(find "$UNPACK" -type l -print -quit)"
test "$("$UNPACK/vela" --version)" = "vela $EXPECTED_VERSION"
test "$("$UNPACK/vela-signer" --version)" = "vela-signer $EXPECTED_VERSION"

if [ "$REQUIRE_PLATFORM_SIGNATURE" = true ]; then
  test "$(uname -s)" = Darwin
  codesign --verify --strict --verbose=2 "$UNPACK/vela" "$UNPACK/vela-signer"
  spctl --assess --type execute --verbose=2 "$UNPACK/vela"
  spctl --assess --type execute --verbose=2 "$UNPACK/vela-signer"
fi

if [ "$(uname -s)" = Linux ]; then
  POLICY="share/polkit-1/actions/science.vela.signer.policy"
  test -f "$UNPACK/$POLICY"
  mkdir -p "$PREFIX/$(dirname "$POLICY")"
  install -m 0644 "$UNPACK/$POLICY" "$PREFIX/$POLICY"
fi

for binary in vela vela-signer; do
  install -m 0755 "$UNPACK/$binary" "$PREFIX/bin/$binary"
done
HOME="$ROOT/home" "$PREFIX/bin/vela" --version
HOME="$ROOT/home" "$PREFIX/bin/vela-signer" --version

# Exercise an exact in-place upgrade, then uninstall only installed product bytes.
for binary in vela vela-signer; do
  install -m 0755 "$UNPACK/$binary" "$PREFIX/bin/$binary"
  rm "$PREFIX/bin/$binary"
  test ! -e "$PREFIX/bin/$binary"
done
if [ "$(uname -s)" = Linux ]; then
  rm "$PREFIX/$POLICY"
  test ! -e "$PREFIX/$POLICY"
fi

echo "release bundle smoke passed: $(basename "$ARCHIVE") ($EXPECTED_VERSION)"
