#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="${1:?usage: .github/release/smoke-bundle.sh <archive> <version>}"
EXPECTED_VERSION="${2:?usage: .github/release/smoke-bundle.sh <archive> <version>}"

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

install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
HOME="$ROOT/home" "$PREFIX/bin/vela" --version

# Exercise an exact in-place upgrade, then uninstall only installed product bytes.
install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
rm "$PREFIX/bin/vela"
test ! -e "$PREFIX/bin/vela"

echo "release bundle smoke passed: $(basename "$ARCHIVE") ($EXPECTED_VERSION)"
