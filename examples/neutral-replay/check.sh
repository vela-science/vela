#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXPECTED="$ROOT/expected.json"
BUNDLE="$ROOT/neutral-replay.git.bundle"
VELA="${VELA_BIN:-vela}"

for executable in git jq; do
  command -v "$executable" >/dev/null 2>&1 || {
    echo "ERROR: $executable is required" >&2
    exit 2
  }
done
command -v "$VELA" >/dev/null 2>&1 || {
  echo "ERROR: Vela binary not found: $VELA" >&2
  exit 2
}

digest_stream() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print "sha256:" $1}'
  else
    shasum -a 256 | awk '{print "sha256:" $1}'
  fi
}

digest_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print "sha256:" $1}'
  else
    shasum -a 256 "$1" | awk '{print "sha256:" $1}'
  fi
}

test "$(digest_file "$BUNDLE")" = "$(jq -er .bundle_root "$EXPECTED")"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/vela-neutral-replay.XXXXXX")"
ANCHOR_TO_REMOVE=""
cleanup() {
  if test -n "$ANCHOR_TO_REMOVE" && test -f "$ANCHOR_TO_REMOVE"; then
    rm -- "$ANCHOR_TO_REMOVE"
    echo "removed fixture trust pin: $ANCHOR_TO_REMOVE"
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

git clone -q -b "$(jq -er .valid.branch "$EXPECTED")" "$BUNDLE" "$WORK/valid"
"$VELA" authority trust pin "$WORK/valid" \
  --record-root "$(jq -er .valid.sequence_one_record_root "$EXPECTED")" \
  --json > "$WORK/pin.json"
if test "$(jq -er .operation "$WORK/pin.json")" = installed; then
  ANCHOR_TO_REMOVE="$(jq -er .authority_trust_anchor_path "$WORK/pin.json")"
fi
"$VELA" replay "$WORK/valid" --json > "$WORK/valid-replay.json"
"$VELA" status "$WORK/valid" --json > "$WORK/valid-status.json"

test "$(git -C "$WORK/valid" rev-parse HEAD)" = "$(jq -er .valid.git_commit "$EXPECTED")"
test "$(git -C "$WORK/valid" rev-parse 'HEAD^{tree}')" = "$(jq -er .valid.git_tree "$EXPECTED")"
test -z "$(git -C "$WORK/valid" status --short)"
test "$(jq -er .repository_root "$WORK/valid-replay.json")" = \
  "$(jq -er .valid.repository_root "$EXPECTED")"
test "$(jq -er .counts.accepted_claims "$WORK/valid-replay.json")" = \
  "$(jq -er .valid.accepted_claim_count "$EXPECTED")"
test "$(jq -er .counts.pending_claims "$WORK/valid-replay.json")" = \
  "$(jq -er .valid.pending_claim_count "$EXPECTED")"

STANDING="$(jq -j -cS .accepted_claims "$WORK/valid/.vela/repository.json" | digest_stream)"
test "$STANDING" = "$(jq -er .valid.accepted_standing_fixture_commitment "$EXPECTED")"

git clone -q -b "$(jq -er .corrupt_artifact.branch "$EXPECTED")" \
  "$BUNDLE" "$WORK/corrupt"
set +e
"$VELA" replay "$WORK/corrupt" --json > "$WORK/corrupt-replay.json"
CORRUPT_EXIT=$?
set -e
test "$CORRUPT_EXIT" = "$(jq -er .corrupt_artifact.replay_exit "$EXPECTED")"
test "$(jq -er .ok "$WORK/corrupt-replay.json")" = false
test "$(jq -er .error.message "$WORK/corrupt-replay.json")" = \
  "$(jq -er .corrupt_artifact.error_message "$EXPECTED")"

echo "neutral replay fixture: ok"
