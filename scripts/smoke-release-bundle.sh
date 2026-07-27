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
test -x "$UNPACK/vela"
test -z "$(find "$UNPACK" -type l -print -quit)"
test "$("$UNPACK/vela" --version)" = "vela $EXPECTED_VERSION"

if [ "$REQUIRE_PLATFORM_SIGNATURE" = true ]; then
  test "$(uname -s)" = Darwin
  codesign --verify --strict --verbose=2 "$UNPACK/vela"
  spctl --assess --type execute --verbose=2 "$UNPACK/vela"
fi

install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
HOME="$ROOT/home" "$PREFIX/bin/vela" --version

# Exercise an exact in-place upgrade, then uninstall only installed product bytes.
install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
rm "$PREFIX/bin/vela"
test ! -e "$PREFIX/bin/vela"

echo "release bundle smoke passed: $(basename "$ARCHIVE") ($EXPECTED_VERSION)"
