#!/usr/bin/env bash
#
# Reviewer Agent batching benchmark.
#
# Synthesizes a frontier with N pending `finding.add` proposals and
# runs `vela review-pending` twice — once per-proposal (batch_size=1,
# the v0.28 default) and once batched (batch_size=N) — printing
# wall-clock duration for each.
#
# This is the source for the perf numbers cited in CHANGELOG for
# v0.29.3+. Re-run after any reviewer-prompt change so the table
# stays honest.
#
# Usage:
#   ./scripts/bench-reviewer-batching.sh [N]   # default N=8

set -euo pipefail

N="${1:-8}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VELA="${VELA:-./target/release/vela}"
WORK="$(mktemp -d -t vela-reviewer-bench-XXXX)"

if [[ ! -x "$VELA" ]]; then
  echo "building vela…" >&2
  cargo build --release --bin vela >/dev/null
fi

echo "== Reviewer batching benchmark · N=$N =="
echo "workdir: $WORK"
echo

# Source frontier (small fixture with real findings)
SRC="examples/paper-folder/expected/frontier.json"
test -s "$SRC" || { echo "missing $SRC"; exit 1; }

# Build a synthetic set of pending proposals — one per finding,
# truncated to N. We treat each proposal as a "newly extracted"
# finding-add candidate that the reviewer must score.
SYNTH_PROPOSALS=$(jq --argjson n "$N" '
  [ .findings[0:$n] | to_entries[] | {
      schema: "vela.proposal.v0.1",
      id: ("vp_bench_" + (.key | tostring) + "_" + (.value.id | sub("^vf_"; ""))),
      kind: "finding.add",
      target: { type: "finding", id: ("vf_synth_" + (.key | tostring) + "_" + (.value.id | sub("^vf_"; ""))) },
      actor: { id: "agent:literature-scout", type: "agent" },
      created_at: "2026-04-26T00:00:00Z",
      reason: "synthetic benchmark proposal",
      status: "pending_review",
      payload: {
        finding: {
          assertion: { text: .value.assertion.text, type: (.value.assertion.type // "claim") },
          provenance: { authors: [{ id: "agent:literature-scout", role: "agent" }] }
        }
      },
      source_refs: [],
      caveats: []
    } ]' "$SRC")

# Stitch synthetic proposals into a fresh frontier
prepare_frontier() {
  local out="$1"
  jq --argjson p "$SYNTH_PROPOSALS" '
    .proposals = $p
    | del(.proposals_count)
  ' "$SRC" > "$out"
}

run_pass() {
  local label="$1"
  local batch="$2"
  local frontier="$WORK/frontier_${label}.json"
  prepare_frontier "$frontier"
  local pending=$(jq '.proposals | length' "$frontier")
  echo "  -- pass: $label · batch_size=$batch · pending=$pending"
  local start=$(date +%s)
  "$VELA" review-pending --frontier "$frontier" --batch-size "$batch" --json \
    > "$WORK/out_${label}.json" 2> "$WORK/err_${label}.log"
  local end=$(date +%s)
  local elapsed=$((end - start))
  local scored=$(jq -r '.scored // 0' "$WORK/out_${label}.json")
  echo "     elapsed=${elapsed}s · scored=${scored}"
  echo "$elapsed" > "$WORK/elapsed_${label}"
  echo "$scored"  > "$WORK/scored_${label}"
}

echo "== Pass 1: per-proposal mode (v0.28 baseline) =="
run_pass "single" 1

echo
echo "== Pass 2: batched mode (v0.29.3+) =="
run_pass "batched" "$N"

E1=$(cat "$WORK/elapsed_single")
E2=$(cat "$WORK/elapsed_batched")
S1=$(cat "$WORK/scored_single")
S2=$(cat "$WORK/scored_batched")

echo
echo "== Result =="
echo "  per-proposal (batch_size=1):  ${E1}s · scored ${S1}/${N}"
echo "  batched      (batch_size=${N}): ${E2}s · scored ${S2}/${N}"
if [[ $E2 -gt 0 ]]; then
  RATIO=$(awk "BEGIN { printf \"%.1f\", $E1 / $E2 }")
  echo "  speedup: ${RATIO}× (per-proposal / batched)"
fi
echo
echo "logs in: $WORK"
