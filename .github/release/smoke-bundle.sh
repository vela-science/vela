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
agent_started=false
trust_pin_path=""
cleanup() {
  case "$trust_pin_path" in
    */.vela/trust/authorities/vfr_*.json)
      rm -f -- "$trust_pin_path"
      ;;
  esac
  if [[ "$agent_started" == true ]]; then
    ssh-agent -k >/dev/null 2>&1 || true
  fi
  rm -rf "$ROOT"
}
trap cleanup EXIT
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

# A version-only smoke allowed a stale binary with a current version string to
# ship. Exercise the current public profile contract from the staged artifact
# so release bytes must agree with the source tree's actual product boundary.
for executable in ssh-agent ssh-add ssh-keygen; do
  command -v "$executable" >/dev/null 2>&1
done
python_bin=python3
command -v "$python_bin" >/dev/null 2>&1 || python_bin=python
ssh-keygen -q -t ed25519 -N '' -C 'Vela release smoke' -f "$ROOT/authority"
eval "$(ssh-agent -s)" >/dev/null
agent_started=true
ssh-add "$ROOT/authority" >/dev/null
AUTHORITY_FINGERPRINT="$(ssh-keygen -lf "$ROOT/authority.pub" -E sha256 | awk '{print $2}')"
FRONTIER="$ROOT/frontier"
"$UNPACK/vela" init "$FRONTIER" \
  --name "Release smoke" \
  --scope "Does this bundle read the current Frontier profile?" \
  --key "$AUTHORITY_FINGERPRINT" \
  --json > "$ROOT/init.json"
trust_pin_path="$("$python_bin" -c 'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["authority"]["local_trust"]["anchor_path"])' "$ROOT/init.json")"
test -f "$FRONTIER/frontier.toml"
test ! -e "$FRONTIER/frontier.yaml"
"$UNPACK/vela" status "$FRONTIER" --json > "$ROOT/status.json"
"$python_bin" - "$ROOT/init.json" "$ROOT/status.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    initialized = json.load(source)
with open(sys.argv[2], encoding="utf-8") as source:
    status = json.load(source)

assert initialized["schema"] == "vela.frontier-init.v3"
assert initialized["authority"]["state"] == "initialized"
assert initialized["scientific_object_count"] == 0
assert initialized["repository"]["repository_root"].startswith("sha256:")
assert initialized["next_action"].startswith("vela submit ")
assert status["schema"] == "vela.status.v3"
assert status["integrity"] == {
    "replay": "verified",
    "strict": "pass",
    "blocker_count": 0,
    "blockers_by_code": {},
}
assert status["counts"]["claims"] == 0
assert status["actions"]["work"]["mode"] == "direct_submission"
PY

install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
HOME="$ROOT/home" "$PREFIX/bin/vela" --version

# Exercise an exact in-place upgrade, then uninstall only installed product bytes.
install -m 0755 "$UNPACK/vela" "$PREFIX/bin/vela"
rm "$PREFIX/bin/vela"
test ! -e "$PREFIX/bin/vela"

echo "release bundle smoke passed: $(basename "$ARCHIVE") ($EXPECTED_VERSION)"
