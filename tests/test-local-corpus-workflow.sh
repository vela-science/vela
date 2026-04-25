#!/usr/bin/env bash
#
# Local paper-folder workflow smoke test.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VELA="${VELA:-./target/release/vela}"
WORKDIR="${VELA_LOCAL_CORPUS_WORKDIR:-/tmp/vela-local-corpus-workflow}"

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"

if [[ ! -x "$VELA" ]]; then
  cargo build --release -p vela-protocol
fi

echo "== Local corpus compile =="
GOOGLE_API_KEY= OPENROUTER_API_KEY= GROQ_API_KEY= ANTHROPIC_API_KEY= \
  "$VELA" compile examples/paper-folder/papers --output "$WORKDIR/frontier.json"

test -s "$WORKDIR/frontier.json"
test -s "$WORKDIR/compile-report.json"
test -s "$WORKDIR/quality-table.json"
test -s "$WORKDIR/frontier-quality.md"
jq -e '
  . as $root |
  (.sources | length) >= 4 and
  all(.sources[]; (.content_hash | startswith("sha256:"))) and
  (.evidence_atoms | length) >= (.findings | length) and
  (.condition_records | length) >= (.findings | length) and
  all(.evidence_atoms[]; .source_id and .finding_id and .evidence_type and .supports_or_challenges) and
  all(.evidence_atoms[]; .source_id as $sid | any($root.sources[]; .id == $sid)) and
  all(.evidence_atoms[]; .condition_refs[] as $cid | ($cid | startswith("finding:")) or any($root.condition_records[]; .id == $cid))
' "$WORKDIR/frontier.json" >/dev/null

jq -e '
  .summary.accepted == 4 and
  .summary.errors == 0 and
  .source_coverage.csv == 1 and
  .source_coverage.text == 1 and
  .source_coverage.jats == 1 and
  .source_coverage.pdf == 1 and
  .extraction_modes.offline_text == 1 and
  .extraction_modes.offline_jats == 1 and
  .extraction_modes.offline_pdf == 1
' "$WORKDIR/compile-report.json" >/dev/null

jq -e '
  .findings | length >= 6 and
  all(.[]; .source_span_status and .provenance_complete != null and .recommended_review_action)
' "$WORKDIR/quality-table.json" >/dev/null
rg -q '^# Frontier quality' "$WORKDIR/frontier-quality.md"

echo "== Bad input diagnostics =="
GOOGLE_API_KEY= OPENROUTER_API_KEY= GROQ_API_KEY= ANTHROPIC_API_KEY= \
  "$VELA" compile examples/paper-folder/bad-input --output "$WORKDIR/bad-frontier.json" \
  > "$WORKDIR/bad-compile.txt"
test -s "$WORKDIR/bad-frontier.json"
test -s "$WORKDIR/compile-report.json"
jq -e '
  .summary.skipped == 1 and
  .summary.errors == 1 and
  any(.sources[]; .source_type == "unsupported" and .status == "skipped") and
  any(.sources[]; .source_type == "csv" and .status == "error" and (.error | contains("CSV line"))) and
  any(.sources[]; .source_type == "pdf" and (.warnings | join(" ") | contains("low-text")))
' "$WORKDIR/compile-report.json" >/dev/null

echo "== Local corpus check/normalize/proof =="
"$VELA" check "$WORKDIR/frontier.json" --strict --json > "$WORKDIR/check.json"
jq -e '
  .ok == true and
  (.diagnostics | type == "array") and
  .source_registry.count >= 4 and
  .evidence_atoms.count >= .summary.checked_findings and
  .conditions.count >= .summary.checked_findings and
  .source_registry.missing_hash_count == 0
' "$WORKDIR/check.json" >/dev/null

"$VELA" normalize "$WORKDIR/frontier.json" --out "$WORKDIR/frontier.normalized.json" --json \
  > "$WORKDIR/normalize.json"
jq -e '.ok == true' "$WORKDIR/normalize.json" >/dev/null

echo "== Correctable frontier state loop =="
STATE_FRONTIER="$WORKDIR/frontier.state.json"
cp "$WORKDIR/frontier.normalized.json" "$STATE_FRONTIER"
FINDING_ID="$(jq -r '.findings[0].id' "$STATE_FRONTIER")"
export FINDING_ID
"$VELA" review "$STATE_FRONTIER" "$FINDING_ID" \
  --status contested \
  --reason "Fixture review: evidence requires manual confirmation." \
  --reviewer "reviewer:test" \
  --apply \
  --json > "$WORKDIR/review-event.json"
jq -e '.ok == true and .command == "review" and .finding_id == env.FINDING_ID' \
  "$WORKDIR/review-event.json" >/dev/null
"$VELA" caveat "$STATE_FRONTIER" "$FINDING_ID" \
  --text "Fixture caveat: deterministic extraction only." \
  --author "reviewer:test" \
  --apply \
  --json > "$WORKDIR/caveat-event.json"
jq -e '.ok == true and .command == "caveat"' "$WORKDIR/caveat-event.json" >/dev/null
"$VELA" revise "$STATE_FRONTIER" "$FINDING_ID" \
  --confidence 0.42 \
  --reason "Fixture revision after manual review." \
  --reviewer "reviewer:test" \
  --apply \
  --json > "$WORKDIR/revise-event.json"
jq -e '.ok == true and .command == "revise"' "$WORKDIR/revise-event.json" >/dev/null
"$VELA" history "$STATE_FRONTIER" "$FINDING_ID" --json > "$WORKDIR/history.json"
jq -e '
  .ok == true and
  (.events | length) >= 3 and
  (.review_events | length) == 0 and
  (.confidence_updates | length) == 0 and
  (.finding.annotations | length) >= 1
' "$WORKDIR/history.json" >/dev/null

"$VELA" proof "$STATE_FRONTIER" --out "$WORKDIR/proof" --json \
  > "$WORKDIR/proof.json"
jq -e '.ok == true' "$WORKDIR/proof.json" >/dev/null

"$VELA" packet validate "$WORKDIR/proof" > "$WORKDIR/packet-validate.txt"
rg -q 'status: ok' "$WORKDIR/packet-validate.txt"
jq -e 'type == "array" and length >= 4 and all(.[]; .id and .locator and .finding_ids)' \
  "$WORKDIR/proof/sources/source-registry.json" >/dev/null
jq -e 'type == "array" and length >= 1 and all(.[]; .id and .source_id and .finding_id)' \
  "$WORKDIR/proof/evidence/evidence-atoms.json" >/dev/null
jq -e '.schema == "vela.source-evidence-map.v0" and (.sources | type == "object")' \
  "$WORKDIR/proof/evidence/source-evidence-map.json" >/dev/null
jq -e 'type == "array" and length >= 1 and all(.[]; .id and .finding_id and .translation_scope and .comparator_status)' \
  "$WORKDIR/proof/conditions/condition-records.json" >/dev/null
jq -e '.schema == "vela.condition-matrix.v0" and (.conditions | type == "array")' \
  "$WORKDIR/proof/conditions/condition-matrix.json" >/dev/null
jq -e '.schema == "vela.state-transitions.v1" and .source == "canonical_events" and (.transitions | length) >= 3' \
  "$WORKDIR/proof/state-transitions.json" >/dev/null
jq -e 'type == "array" and length >= 3' "$WORKDIR/proof/events/events.json" >/dev/null
jq -e '.ok == true and (.status == "ok" or .status == "no_events")' \
  "$WORKDIR/proof/events/replay-report.json" >/dev/null

echo "== Agent tool check =="
"$VELA" serve "$WORKDIR/frontier.normalized.json" --check-tools --json > "$WORKDIR/tool-check.json"
jq -e '
  .ok == true and
  .summary.failed == 0 and
  (.checks | length >= 8) and
  all(.checks[]; .has_data == true and .has_markdown == true and .has_signals == true and .has_caveats == true)
' \
  "$WORKDIR/tool-check.json" >/dev/null
jq -e '
  .tool_calls and
  any(.tool_calls[]; .tool == "get_finding" and .arguments.id == "vf_c4bf737129fe5c50") and
  .final_answer_shape.must_cite_finding_ids == true
' examples/paper-folder/expected/mcp-transcript.json >/dev/null

echo "== Example benchmark fixture =="
"$VELA" bench --suite benchmarks/suites/example-paper-folder.json --json \
  > "$WORKDIR/example-bench.json"
jq -e '.ok == true and (.tasks | length) == 4' "$WORKDIR/example-bench.json" >/dev/null
python3 benchmarks/validate-benchmark-fixtures.py --suite benchmarks/suites/example-paper-folder.json

echo "Local corpus workflow passed: $WORKDIR"
