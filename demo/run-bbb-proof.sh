#!/usr/bin/env bash
#
# Vela BBB Proof Workflow
#
# Runs a canonical BBB/Alzheimer proof loop against frontiers/bbb-alzheimer.json:
#   - benchmark suite
#   - frontier check
#   - frontier stats
#   - search / tensions / gaps
#   - HTTP + MCP-compatible tool checks
#   - proof packet export + validation
#
# Usage:
#   ./demo/run-bbb-proof.sh
#
# Env:
#   BBB_PROOF_PORT       Port for temporary HTTP server (default: 3848)
#   BBB_PROOF_WORKDIR    Override output directory for logs + packet
#
set -euo pipefail

VELA_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$VELA_ROOT/target/release/vela"
FRONTIER_SOURCE="$VELA_ROOT/frontiers/bbb-alzheimer.json"
BENCHMARK_SUITE="$VELA_ROOT/benchmarks/suites/bbb-core.json"
PORT="${BBB_PROOF_PORT:-3848}"
WORKDIR="${BBB_PROOF_WORKDIR:-$VELA_ROOT/demo/bbb-proof-run-$(date -u +%Y%m%dT%H%M%SZ)}"
WORK_FRONTIER="$WORKDIR/frontier.state.json"
BASE="http://127.0.0.1:${PORT}"
HTTP_BODY="$WORKDIR/http-body.json"
LOG="$WORKDIR/run.log"
CHECK_REPORT="$WORKDIR/check-report.json"
BENCHMARK_REPORT="$WORKDIR/benchmark-report.json"
SUMMARY_JSON="$WORKDIR/summary.json"
PACKET_DIR="$WORKDIR/bbb-alzheimer-proof-packet"
SERVER_PID=""

log() {
  printf '%s\n' "$1" | tee -a "$LOG"
}

fail() {
  log "[FAIL] $1"
  exit 1
}

assert_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "Missing required command: $1"
  fi
}

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

wait_for_server() {
  for i in $(seq 1 60); do
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}/api/stats" || echo 000)
    if [[ "$code" == "200" ]]; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}

http_get() {
  local url="$1"
  HTTP_CODE=$(curl -s -o "$HTTP_BODY" -w "%{http_code}" "$url")
}

http_post() {
  local url="$1"
  local payload="$2"
  HTTP_CODE=$(curl -s -H 'Content-Type: application/json' -X POST -d "$payload" -o "$HTTP_BODY" -w "%{http_code}" "$url")
}

assert_http_ok() {
  local expected="$1"
  local description="$2"
  if [[ "$HTTP_CODE" != "$expected" ]]; then
    fail "$description (HTTP $HTTP_CODE, expected $expected)"
  fi
}

assert_json_has() {
  local expr="$1"
  local description="$2"
  if ! jq -e "$expr" "$HTTP_BODY" >/dev/null 2>&1; then
    fail "$description"
  fi
}

assert_body_contains() {
  local needle="$1"
  local description="$2"
  if ! grep -Fq "$needle" "$HTTP_BODY" ; then
    fail "$description"
  fi
}

validate_proof_trace() {
  local root="$1"
  local trace="$root/proof-trace.json"
  if [[ ! -f "$trace" ]]; then
    fail "proof trace missing: $trace"
  fi

  log "Validating proof trace: $trace"
  if ! jq -e '
    (type == "object" or type == "array")
    and (
      (type == "object" and length > 0)
      or (type == "array" and length > 0)
    )
  ' "$trace" >/dev/null; then
    fail "proof trace exists but is not a non-empty JSON object or array: $trace"
  fi
  log "[PASS] proof trace JSON is present and non-empty"
}

extract_finding_id() {
  local needle="$1"
  local result
  result=$(jq -r '.result' "$HTTP_BODY" | grep -Eo 'vf_[0-9a-f]{15,}') || true
  if [[ -z "$result" ]]; then
    result=$(jq -r 'if has("result") then .result else empty end' "$HTTP_BODY" | grep -Eo 'vf_[0-9a-f]{15,}' || true)
  fi
  if [[ -z "$result" ]]; then
    fail "$needle returned no finding id"
  fi
  echo "$result" | head -n 1
}

mkdir -p "$WORKDIR"
: > "$LOG"

log "BBB proof workflow: BBB Alzheimer proof smoke"
log "Working dir: $WORKDIR"

assert_cmd jq
assert_cmd curl

if [[ ! -x "$BINARY" ]]; then
  fail "Binary not found at $BINARY. Build with: cargo build --release"
fi

if [[ ! -e "$FRONTIER_SOURCE" ]]; then
  fail "Frontier source missing: $FRONTIER_SOURCE"
fi

cp "$FRONTIER_SOURCE" "$WORK_FRONTIER"

log ""
log "Step 1/9 — benchmark suite quality gate"
"$BINARY" bench --suite "$BENCHMARK_SUITE" --json > "$BENCHMARK_REPORT"
log "[PASS] benchmark suite: $(jq -r '(.metrics.tasks_passed|tostring) + "/" + (.metrics.tasks_total|tostring) + " tasks passed"' "$BENCHMARK_REPORT")"
log "Wrote benchmark report to: $BENCHMARK_REPORT"

log ""
log "Step 2/9 — frontier check"
"$BINARY" check "$WORK_FRONTIER" --json > "$CHECK_REPORT"
log "[PASS] check: $(jq -r '.summary.status + " (" + (.summary.checked_findings|tostring) + " findings)"' "$CHECK_REPORT")"
log "Wrote check report to: $CHECK_REPORT"

log ""
log "Step 3/9 — CLI frontier stats"
"$BINARY" stats "$WORK_FRONTIER" | tee -a "$LOG"

log ""
log "Step 4/9 — CLI search for BBB design-relevant evidence"
SEARCH_RESULT="$WORKDIR/search-output.txt"
"$BINARY" search "LRP1" --source "$WORK_FRONTIER" --limit 5 | tee "$SEARCH_RESULT"
log "Wrote search output to: $SEARCH_RESULT"

log ""
log "Step 5/9 — CLI tensions + gap ranking"
"$BINARY" tensions "$WORK_FRONTIER" --both-high --top 5 | tee "$WORKDIR/tensions-output.txt"
"$BINARY" gaps rank "$WORK_FRONTIER" --top 5 | tee "$WORKDIR/gaps-output.txt"
log "Wrote tensions output to: $WORKDIR/tensions-output.txt"
log "Wrote gaps output to: $WORKDIR/gaps-output.txt"

log ""
log "Step 6/9 — serve frontier for HTTP + MCP checks"
"$BINARY" serve "$WORK_FRONTIER" --http "$PORT" &>/dev/null &
SERVER_PID=$!
if ! wait_for_server; then
  fail "HTTP server did not start on port $PORT"
fi

log "Server ready on ${BASE}"

# /api/stats
http_get "${BASE}/api/stats"
assert_http_ok "200" "/api/stats"
assert_json_has '.stats.findings > 0' "/api/stats missing stats.findings"
log "[PASS] /api/stats: $(jq -r '.stats.findings' "$HTTP_BODY") findings"

# /api/findings search endpoint
http_get "${BASE}/api/findings?query=LRP1&limit=5"
assert_http_ok "200" "/api/findings"
assert_body_contains "findings matched" "/api/findings search result did not return expected payload"
FIRST_ID=$(extract_finding_id "/api/findings query")
log "[PASS] /api/findings query returned id: $FIRST_ID"

# /api/findings/{id}
http_get "${BASE}/api/findings/$FIRST_ID"
assert_http_ok "200" "/api/findings/:id"
assert_json_has ".id == \"$FIRST_ID\"" "/api/findings/:id does not match requested id"
log "[PASS] /api/findings/$FIRST_ID"

# MCP-like tool endpoints
http_post "${BASE}/api/tool" '{"name":"frontier_stats","arguments":{}}'
assert_http_ok "200" "tool frontier_stats"
assert_json_has '.result|fromjson|.stats.findings > 0' "frontier_stats result missing findings"
log "[PASS] tool frontier_stats"

http_post "${BASE}/api/tool" '{"name":"list_gaps","arguments":{}}'
assert_http_ok "200" "tool list_gaps"
assert_body_contains "gap" "list_gaps result missing gap output"
log "[PASS] tool list_gaps"

http_post "${BASE}/api/tool" '{"name":"list_contradictions","arguments":{}}'
assert_http_ok "200" "tool list_contradictions"
assert_body_contains "contradiction" "list_contradictions result missing contradiction output"
log "[PASS] tool list_contradictions"

http_post "${BASE}/api/tool" '{"name":"find_bridges","arguments":{"min_categories":2,"limit":5}}'
assert_http_ok "200" "tool find_bridges"
assert_json_has '.result|fromjson|.count >= 1' "find_bridges result missing bridge entities"
log "[PASS] tool find_bridges"

http_post "${BASE}/api/tool" '{"name":"apply_observer","arguments":{"policy":"academic","limit":5}}'
assert_http_ok "200" "tool apply_observer"
assert_json_has '.result|fromjson|.top_findings | length > 0' "apply_observer missing top_findings"
log "[PASS] tool apply_observer"

# Optional /api/tools catalog check
http_get "${BASE}/api/tools"
assert_http_ok "200" "/api/tools"
assert_json_has 'length > 0' "tool catalog should be non-empty"
log "[PASS] /api/tools returned $(jq 'length' "$HTTP_BODY") tools"

log ""
log "Step 7/9 — export canonical proof packet"
rm -rf "$PACKET_DIR"
"$BINARY" proof "$WORK_FRONTIER" --out "$PACKET_DIR" --gold "$BENCHMARK_SUITE" | tee -a "$LOG"

if [[ ! -f "$PACKET_DIR/manifest.json" ]]; then
  fail "export did not produce packet manifest at $PACKET_DIR/manifest.json"
fi

log ""
log "Step 8/9 — validate proof packet"
"$BINARY" packet validate "$PACKET_DIR" | tee -a "$LOG"
validate_proof_trace "$PACKET_DIR"

log ""
log "Step 9/9 — apply a correction and confirm stale proof"
FINDING_ID=$("$BINARY" search "LRP1" --source "$WORK_FRONTIER" --limit 1 --json | jq -r '.results[0].id')
[[ -n "$FINDING_ID" && "$FINDING_ID" != "null" ]] || fail "Unable to select finding for correction"
"$BINARY" review "$WORK_FRONTIER" "$FINDING_ID" \
  --status contested \
  --reason "Demo correction: preserve mouse-only scope and force proof refresh." \
  --reviewer "reviewer:demo" \
  --apply \
  --json > "$WORKDIR/review-apply.json"
set +e
"$BINARY" check "$WORK_FRONTIER" --strict --json > "$WORKDIR/stale-check.json"
STALE_CHECK_RC=$?
set -e
[[ "$STALE_CHECK_RC" -ne 0 ]] || fail "Strict check should fail after accepted correction makes proof stale"
jq -e '.proof_state.latest_packet.status == "stale"' "$WORKDIR/stale-check.json" >/dev/null \
  || fail "Stale proof state was not surfaced after accepted correction"

log ""
log "Step 10/10 — proof summary"
TENSION_COUNT_FILE="$WORKDIR/tensions-output.txt"
GAP_COUNT_FILE="$WORKDIR/gaps-output.txt"
PACKET_FINDINGS=$(jq '.findings' "$PACKET_DIR/overview.json")
PACKET_STATUS=$("$BINARY" packet validate "$PACKET_DIR" 2>/dev/null | awk '/status:/ {print $0}')
jq -n \
  --arg frontier "$FRONTIER_SOURCE" \
  --arg benchmark_report "$BENCHMARK_REPORT" \
  --arg check_report "$CHECK_REPORT" \
  --arg packet_dir "$PACKET_DIR" \
  --arg http_endpoint "$BASE" \
  --arg log "$LOG" \
  --slurpfile benchmark "$BENCHMARK_REPORT" \
  --slurpfile check "$CHECK_REPORT" \
  --argjson packet_findings "$PACKET_FINDINGS" \
  '{
    ok: true,
    frontier: $frontier,
    benchmark: {
      report: $benchmark_report,
      ok: $benchmark[0].ok,
      tasks_passed: $benchmark[0].metrics.tasks_passed,
      tasks_total: $benchmark[0].metrics.tasks_total
    },
    check: {
      report: $check_report,
      ok: $check[0].ok,
      status: $check[0].summary.status,
      checked_findings: $check[0].summary.checked_findings,
      errors: $check[0].summary.errors,
      warnings: $check[0].summary.warnings
    },
    proof: {
      packet_dir: $packet_dir,
      findings: $packet_findings,
      validation: {
        status: "ok"
      },
      freshness_after_correction: "stale"
    },
    serve: {
      http_endpoint: $http_endpoint
    },
    log: $log
  }' > "$SUMMARY_JSON"
log "Frontier source: $FRONTIER_SOURCE"
log "HTTP endpoint: $BASE"
log "Packet: $PACKET_DIR"
log "Logged outputs:"
log "  - $BENCHMARK_REPORT"
log "  - $CHECK_REPORT"
log "  - $SEARCH_RESULT"
log "  - $TENSION_COUNT_FILE"
log "  - $GAP_COUNT_FILE"
log "  - $SUMMARY_JSON"
log "  - $LOG"
log "Summary:"
log "  packet findings: $PACKET_FINDINGS"
log "  packet status: $PACKET_STATUS"
log "  summary json: $SUMMARY_JSON"

log "BBB proof workflow completed."
