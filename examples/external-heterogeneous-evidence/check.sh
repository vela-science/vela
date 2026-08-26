#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXPECTED="$ROOT/expected.json"
BUNDLE="$ROOT/heterogeneous-evidence.git.bundle"
VELA="${VELA_BIN:-vela}"

for executable in git jq python3; do
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

WORK="$(mktemp -d "${TMPDIR:-/tmp}/vela-external-evidence.XXXXXX")"
ANCHOR_TO_REMOVE=""
cleanup() {
  if test -n "$ANCHOR_TO_REMOVE" && test -f "$ANCHOR_TO_REMOVE"; then
    rm -- "$ANCHOR_TO_REMOVE"
    echo "removed fixture trust pin: $ANCHOR_TO_REMOVE"
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

python3 "$ROOT/analysis.py" "$ROOT/observations.csv" --check "$ROOT/result.json"

git clone -q -b "$(jq -er .branches.valid.branch "$EXPECTED")" \
  "$BUNDLE" "$WORK/valid"
"$VELA" authority trust pin "$WORK/valid" \
  --record-root "$(jq -er .authority.sequence_one_record_root "$EXPECTED")" \
  --json > "$WORK/pin.json"
if test "$(jq -er .operation "$WORK/pin.json")" = installed; then
  ANCHOR_TO_REMOVE="$(jq -er .authority_trust_anchor_path "$WORK/pin.json")"
fi
"$VELA" replay "$WORK/valid" --json > "$WORK/replay.json"
"$VELA" status "$WORK/valid" --json > "$WORK/status.json"
"$VELA" why "$WORK/valid" "$(jq -er .objects.claim_id "$EXPECTED")" \
  --json > "$WORK/why.json"

test "$(git -C "$WORK/valid" rev-parse HEAD)" = \
  "$(jq -er .branches.valid.git_commit "$EXPECTED")"
test "$(git -C "$WORK/valid" rev-parse 'HEAD^{tree}')" = \
  "$(jq -er .branches.valid.git_tree "$EXPECTED")"
test -z "$(git -C "$WORK/valid" status --short)"
test "$(jq -er .repository_root "$WORK/replay.json")" = \
  "$(jq -er .branches.valid.repository_root "$EXPECTED")"
test "$(jq -er .counts.accepted_claims "$WORK/replay.json")" = 1
test "$(jq -er .counts.pending_claims "$WORK/replay.json")" = 0
test "$(jq -er .counts.submissions "$WORK/replay.json")" = 1
test "$(jq -er .counts.verifications "$WORK/replay.json")" = 2
test "$(jq -er .integrity.strict "$WORK/status.json")" = pass
test "$(jq -er .standing "$WORK/why.json")" = accepted
test "$(jq -er .proposal_status "$WORK/why.json")" = accepted
test "$(jq -er '.chain.verification_records | length' "$WORK/why.json")" = 2
STANDING="$(jq -j -cS .accepted_claims "$WORK/valid/.vela/repository.json" | digest_stream)"
test "$STANDING" = \
  "$(jq -er .branches.valid.accepted_set_fixture_commitment "$EXPECTED")"

git clone -q -b "$(jq -er .branches.incomplete_review.branch "$EXPECTED")" \
  "$BUNDLE" "$WORK/incomplete"
"$VELA" review inbox "$WORK/incomplete" --json > "$WORK/incomplete-inbox.json"
test "$(jq -er .repository_root "$WORK/incomplete-inbox.json")" = \
  "$(jq -er .branches.incomplete_review.repository_root "$EXPECTED")"
test "$(jq -er '.entries[0].readiness.protocol_gate' "$WORK/incomplete-inbox.json")" = blocked
test "$(jq -er '.entries[0].verification_records | length' "$WORK/incomplete-inbox.json")" = 1
test "$(jq -er '.entries[0].readiness.blockers[0].subject' "$WORK/incomplete-inbox.json")" = \
  evidence_scope_review

git clone -q -b "$(jq -er .branches.missing_artifact.branch "$EXPECTED")" \
  "$BUNDLE" "$WORK/missing"
set +e
"$VELA" replay "$WORK/missing" --json > "$WORK/missing-replay.json"
MISSING_EXIT=$?
set -e
test "$MISSING_EXIT" = "$(jq -er .branches.missing_artifact.replay_exit "$EXPECTED")"
test "$(jq -er .ok "$WORK/missing-replay.json")" = false
test "$(jq -er .error.message "$WORK/missing-replay.json")" = \
  "$(jq -er .branches.missing_artifact.error_message "$EXPECTED")"

echo "external heterogeneous evidence fixture: ok"
