#!/usr/bin/env bash
#
# First-frontier smoke path for a new user.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

VELA="${VELA:-./target/release/vela}"
WORKDIR="${VELA_FIRST_FRONTIER_WORKDIR:-/tmp/vela-first-frontier}"

if [[ ! -x "$VELA" ]]; then
  cargo build --release -p vela-protocol
fi

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

echo "== Compile local paper folder =="
GOOGLE_API_KEY= OPENROUTER_API_KEY= GROQ_API_KEY= ANTHROPIC_API_KEY= \
  "$VELA" compile examples/paper-folder/papers --output "$WORKDIR/frontier.json"

echo "== Inspect generated diagnostics =="
test -s "$WORKDIR/compile-report.json"
test -s "$WORKDIR/quality-table.json"
test -s "$WORKDIR/frontier-quality.md"

echo "== Check and normalize =="
"$VELA" check "$WORKDIR/frontier.json" --strict --json > "$WORKDIR/check.json"
"$VELA" normalize "$WORKDIR/frontier.json" --out "$WORKDIR/frontier.normalized.json" --json \
  > "$WORKDIR/normalize.json"
FINDING_ID="$(jq -r '.findings[0].id' "$WORKDIR/frontier.json")"
"$VELA" review "$WORKDIR/frontier.normalized.json" "$FINDING_ID" \
  --status contested \
  --reason "Example review: verify deterministic extraction before reuse." \
  --reviewer "reviewer:example" \
  --apply \
  --json > "$WORKDIR/review-event.json"
"$VELA" caveat "$WORKDIR/frontier.normalized.json" "$FINDING_ID" \
  --text "Example caveat: this finding comes from the tiny fixture corpus." \
  --author "reviewer:example" \
  --apply \
  --json > "$WORKDIR/caveat-event.json"
"$VELA" history "$WORKDIR/frontier.normalized.json" "$FINDING_ID" --json > "$WORKDIR/history.json"

echo "== Proof and tool contract =="
"$VELA" proof "$WORKDIR/frontier.normalized.json" --out "$WORKDIR/proof-packet" --json \
  > "$WORKDIR/proof.json"
"$VELA" serve "$WORKDIR/frontier.normalized.json" --check-tools --json \
  > "$WORKDIR/tool-check.json"

echo "First frontier outputs: $WORKDIR"
