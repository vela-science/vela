#!/bin/sh
set -eu

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
FRONTIER="${1:-/Users/williamblair/personal/formal-conjectures-frontier}"
VELA="${VELA_BIN:-/Users/williamblair/.canopus/bin/vela-0.940.9-4813da26}"
RUN="${CANOPUS_RUN:-/Users/williamblair/.canopus/runs/formal-conjectures-frontier/2026-07-28-formal-505-repair-4/run/run.json}"
CANOPUS_CLI="${CANOPUS_CLI:-$HOME/.canopus/bin/canopus-formal-505-d0e05094.js}"
ACTOR="verifier:formal-erdos-505-replay-v1"

EXPECTED_HEAD="aca6076afe184cec8f0a5ecdca25815b0d4111ef"
EXPECTED_VELA_VERSION="vela 0.940.9"
EXPECTED_VELA_SHA256="b4b85550aed52134ad2e21a3b1a163390ca1f16673811274b55b3b0f2089ed9c"
EXPECTED_VERIFIER_SHA256="a6ac3ec91e6307fc9919fb180379ec7c0beab709a71f3057dff04ca62028542e"
EXPECTED_REPORT_SHA256="ca35b7bb266abb68aeab9705a3f5386af4001b61a018861c6667f4cdf977e795"
EXPECTED_RECORD_SHA256="e7989f006365cfdfa517665eb9fe168acf4e06724fa89db69920eeb30c231b25"

if [ "$(git -C "$FRONTIER" rev-parse HEAD)" != "$EXPECTED_HEAD" ]; then
  echo "Formal Frontier head changed; do not import this Verification." >&2
  exit 1
fi
if [ -z "${SSH_AUTH_SOCK:-}" ] || [ ! -S "$SSH_AUTH_SOCK" ]; then
  echo "SSH_AUTH_SOCK must name the loaded repository-authority agent socket." >&2
  exit 1
fi
if [ "$("$VELA" --version)" != "$EXPECTED_VELA_VERSION" ]; then
  echo "Vela version changed; do not import this Verification." >&2
  exit 1
fi

check_sha256() {
  expected="$1"
  path="$2"
  label="$3"
  observed="$(shasum -a 256 "$path" | awk '{print $1}')"
  if [ "$observed" != "$expected" ]; then
    echo "$label changed; do not import this Verification." >&2
    exit 1
  fi
}

check_sha256 "$EXPECTED_VELA_SHA256" "$VELA" "Vela binary"
check_sha256 "$EXPECTED_VERIFIER_SHA256" "$ROOT/paper/artifacts/formal-505/verify_replay.py" "Verifier"
check_sha256 "$EXPECTED_REPORT_SHA256" "$ROOT/paper/artifacts/formal-505/report.v1.json" "Verifier report"
check_sha256 "$EXPECTED_RECORD_SHA256" "$ROOT/paper/artifacts/formal-505/verification.v1.json" "Signed Verification"

replay="$(mktemp "${TMPDIR:-/tmp}/vela-formal-505.XXXXXX")"
trap 'rm -f "$replay"' EXIT HUP INT TERM
python3 "$ROOT/paper/artifacts/formal-505/verify_replay.py" \
  --canopus-cli "$CANOPUS_CLI" \
  --run "$RUN" \
  --output "$replay" >/dev/null
if ! cmp -s "$ROOT/paper/artifacts/formal-505/report.v1.json" "$replay"; then
  echo "Fresh verifier replay differs from the retained report; do not import." >&2
  exit 1
fi

"$VELA" check "$FRONTIER" --strict --json >/dev/null
"$VELA" verification import \
  "$FRONTIER" \
  "$ROOT/paper/artifacts/formal-505/verification.v1.json" \
  --as "$ACTOR" \
  --json
