#!/usr/bin/env bash
#
# Concise Vela v0 demo:
#   correction -> canonical event -> history -> stale proof -> refreshed proof
#
# This script works on a temporary copy of frontiers/bbb-alzheimer.json.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$ROOT/target/release/vela"
SOURCE="$ROOT/frontiers/bbb-alzheimer.json"
WORKDIR="${VELA_V0_DEMO_WORKDIR:-/tmp/vela-v0-state-proof-demo}"
FRONTIER="$WORKDIR/frontier.json"
BEFORE_PACKET="$WORKDIR/proof-before"
AFTER_PACKET="$WORKDIR/proof-after"
SUMMARY="$WORKDIR/summary.json"

fail() {
  printf '[FAIL] %s\n' "$1" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

need_cmd jq

if [[ ! -x "$BINARY" ]]; then
  fail "Release binary missing. Run: cargo build --release -p vela-protocol"
fi

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cp "$SOURCE" "$FRONTIER"

printf 'Vela v0 state-proof demo\n'
printf 'workdir: %s\n\n' "$WORKDIR"

printf '1. Inspect bounded frontier state\n'
"$BINARY" stats "$FRONTIER"

printf '\n2. Validate current frontier\n'
"$BINARY" check "$FRONTIER" --strict --json > "$WORKDIR/check-before.json"
jq -r '"status: " + .summary.status + " (" + (.summary.checked_findings|tostring) + " findings)"' "$WORKDIR/check-before.json"

printf '\n3. Export and validate a proof packet without mutating the frontier\n'
"$BINARY" proof "$FRONTIER" --out "$BEFORE_PACKET" >/dev/null
"$BINARY" packet validate "$BEFORE_PACKET"

printf '\n4. Apply a reviewed correction as a proposal-backed state transition\n'
FINDING_ID=$("$BINARY" search "LRP1" --source "$FRONTIER" --limit 1 --json | jq -r '.results[0].id')
[[ -n "$FINDING_ID" && "$FINDING_ID" != "null" ]] || fail "Could not select a finding"
"$BINARY" review "$FRONTIER" "$FINDING_ID" \
  --status contested \
  --reason "Demo correction: preserve scope before reuse as agent context." \
  --reviewer "reviewer:demo" \
  --apply \
  --json > "$WORKDIR/review-apply.json"
jq -r '"proposal: " + .proposal_id + " / event: " + .applied_event_id' "$WORKDIR/review-apply.json"

printf '\n5. Replayable history now includes the correction\n'
"$BINARY" history "$FRONTIER" "$FINDING_ID"

printf '\n6. The old recorded proof is now stale\n'
set +e
"$BINARY" check "$FRONTIER" --strict --json > "$WORKDIR/check-stale.json"
STALE_RC=$?
set -e
[[ "$STALE_RC" -ne 0 ]] || fail "Expected strict check to fail while proof is stale"
jq -e '.proof_state.latest_packet.status == "stale"' "$WORKDIR/check-stale.json" >/dev/null \
  || fail "Expected stale proof state"
jq -r '"proof status: " + .proof_state.latest_packet.status' "$WORKDIR/check-stale.json"

printf '\n7. Refresh proof on the temporary frontier copy\n'
"$BINARY" proof "$FRONTIER" --out "$AFTER_PACKET" --record-proof-state >/dev/null
"$BINARY" packet validate "$AFTER_PACKET"
"$BINARY" check "$FRONTIER" --strict --json > "$WORKDIR/check-after.json"
jq -r '"status: " + .summary.status + " / proof: " + .proof_state.latest_packet.status' "$WORKDIR/check-after.json"

jq -n \
  --arg workdir "$WORKDIR" \
  --arg finding_id "$FINDING_ID" \
  --arg before_packet "$BEFORE_PACKET" \
  --arg after_packet "$AFTER_PACKET" \
  --slurpfile before "$WORKDIR/check-before.json" \
  --slurpfile stale "$WORKDIR/check-stale.json" \
  --slurpfile after "$WORKDIR/check-after.json" \
  '{
    ok: true,
    workdir: $workdir,
    finding_id: $finding_id,
    before: {
      packet: $before_packet,
      check_status: $before[0].summary.status
    },
    correction: {
      stale_proof_status: $stale[0].proof_state.latest_packet.status
    },
    after: {
      packet: $after_packet,
      check_status: $after[0].summary.status,
      proof_status: $after[0].proof_state.latest_packet.status
    }
  }' > "$SUMMARY"

printf '\nSummary written to: %s\n' "$SUMMARY"
printf 'Demo complete.\n'
