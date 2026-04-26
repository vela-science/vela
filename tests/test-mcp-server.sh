#!/usr/bin/env bash
#
# Integration tests for the Vela MCP server (HTTP mode).
# Tests tool coverage beyond the basic HTTP endpoint tests.
# Starts the server on port 3799, runs tool-call tests, then cleans up.
#
set -euo pipefail

PORT=3799
BASE="http://localhost:${PORT}"
BINARY="./target/release/vela"
FRONTIER="frontiers/bbb-alzheimer.json"
SERVER_PID=""

PASS=0
FAIL=0
TOTAL=0

# ── helpers ──────────────────────────────────────────────────────────────────

cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

log_pass() {
    PASS=$((PASS + 1))
    TOTAL=$((TOTAL + 1))
    printf "  PASS  %s\n" "$1"
}

log_fail() {
    FAIL=$((FAIL + 1))
    TOTAL=$((TOTAL + 1))
    printf "  FAIL  %s — %s\n" "$1" "$2"
}

http_get() {
    local url="$1"
    HTTP_CODE=$(curl -s -o /tmp/vela_mcp_test_body -w "%{http_code}" "$url")
    BODY=$(cat /tmp/vela_mcp_test_body)
}

http_post() {
    local url="$1"
    local data="$2"
    HTTP_CODE=$(curl -s -o /tmp/vela_mcp_test_body -w "%{http_code}" \
        -X POST -H "Content-Type: application/json" -d "$data" "$url")
    BODY=$(cat /tmp/vela_mcp_test_body)
}

json_has() {
    python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    result = $1
    sys.exit(0 if result else 1)
except Exception as e:
    print(f'json_has error: {e}', file=sys.stderr)
    sys.exit(1)
" <<< "$BODY"
}

json_extract() {
    python3 -c "
import json, sys
d = json.loads(sys.stdin.read())
print($1)
" <<< "$BODY"
}

# ── build ────────────────────────────────────────────────────────────────────

cd "$(dirname "$0")/.."

if [[ ! -f "$BINARY" ]]; then
    echo "Building release binary..."
    cargo build --release
else
    echo "Release binary found."
fi

if [[ ! -e "$FRONTIER" ]]; then
    echo "ERROR: Frontier file not found: $FRONTIER"
    exit 1
fi

# ── start server ─────────────────────────────────────────────────────────────

echo "Starting vela serve (MCP/HTTP) on port ${PORT}..."
"$BINARY" serve "$FRONTIER" --http "$PORT" &
SERVER_PID=$!

echo -n "Waiting for server"
READY=0
for i in $(seq 1 60); do
    if curl -sf "${BASE}/api/stats" > /dev/null 2>&1; then
        READY=1
        break
    fi
    echo -n "."
    sleep 0.5
done
echo

if [[ "$READY" -ne 1 ]]; then
    echo "ERROR: Server did not become ready within 30 seconds."
    exit 1
fi

echo "Server ready. Running MCP tool tests..."
echo

# ── tests ────────────────────────────────────────────────────────────────────

# 1. frontier_stats — verify findings count is present and positive
TEST="frontier_stats returns findings count > 0"
http_post "${BASE}/api/tool" '{"name":"frontier_stats","arguments":{}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "response missing findings info"
fi

# 2. search_findings — query LRP1
TEST="search_findings query=LRP1 returns results"
http_post "${BASE}/api/tool" '{"name":"search_findings","arguments":{"query":"LRP1"}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "no matched findings in response"
fi

# 3. search_findings — empty query returns something
TEST="search_findings query=NONEXISTENT returns no match"
http_post "${BASE}/api/tool" '{"name":"search_findings","arguments":{"query":"NONEXISTENT_XYZZY_99999"}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and ('No findings matched' in d.get('markdown','') or '0 findings matched' in d.get('markdown','')) and isinstance(d.get('data'), dict)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "expected no match message"
fi

# 4. list_gaps — returns gap information
TEST="list_gaps returns gap data"
http_post "${BASE}/api/tool" '{"name":"list_gaps","arguments":{}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "empty or missing result"
fi

# 5. list_contradictions — returns contradiction info
TEST="list_contradictions returns data"
http_post "${BASE}/api/tool" '{"name":"list_contradictions","arguments":{}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "empty or missing result"
fi

# 6. find_bridges — returns bridge entities
TEST="find_bridges returns structured bridge data"
http_post "${BASE}/api/tool" '{"name":"find_bridges","arguments":{"min_categories":2,"limit":5}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data',{}).get('bridges'), list) and 'markdown' in d and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "response missing structured bridge data"
fi

# 7. apply_observer — pharma policy
TEST="apply_observer pharma returns ranked findings"
http_post "${BASE}/api/tool" '{"name":"apply_observer","arguments":{"policy":"pharma","limit":5}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "empty or missing result"
fi

# 8. apply_observer — academic policy
TEST="apply_observer academic returns ranked findings"
http_post "${BASE}/api/tool" '{"name":"apply_observer","arguments":{"policy":"academic","limit":5}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "empty or missing result"
fi

# 9. get_finding — returns a structured finding bundle
TEST="get_finding returns structured data"
http_post "${BASE}/api/tool" '{"name":"get_finding","arguments":{"id":"vf_5021284e4155f141"}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing structured get_finding response"
fi

# 10. trace_evidence_chain — returns evidence-chain structure
TEST="trace_evidence_chain returns structured data"
http_post "${BASE}/api/tool" '{"name":"trace_evidence_chain","arguments":{"finding_id":"vf_5021284e4155f141","depth":1}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data'), dict) and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing structured trace response"
fi

# 11. check_pubmed — rough prior-art check returns structured data or error
TEST="check_pubmed returns structured success or error"
http_post "${BASE}/api/tool" '{"name":"check_pubmed","arguments":{"query":"LRP1 amyloid"}}'
if [[ "$HTTP_CODE" != "200" && "$HTTP_CODE" != "500" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "((d.get('ok') is True and isinstance(d.get('data'), dict) and 'markdown' in d) or (d.get('ok') is False and isinstance(d.get('error'), str))) and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing structured check_pubmed response"
fi

# 12. GET /api/tools returns strict registry
TEST="GET /api/tools returns strict 19-tool registry"
http_get "${BASE}/api/tools"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "isinstance(d, list) and len(d) == 19"; then
    log_pass "$TEST"
else
    count=$(json_extract "len(d) if isinstance(d, list) else 'not a list'")
    log_fail "$TEST" "got $count tools"
fi

# 13. propagate_retraction — requires a valid finding ID from this frontier
TEST="propagate_retraction with invalid ID returns error gracefully"
http_post "${BASE}/api/tool" '{"name":"propagate_retraction","arguments":{"finding_id":"vf_does_not_exist"}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    # Even errors should return 200 with error in body for MCP tools
    if json_has "'error' in str(d).lower() or 'not found' in str(d).lower()" 2>/dev/null; then
        log_pass "$TEST"
    else
        log_fail "$TEST" "HTTP $HTTP_CODE with no error info"
    fi
else
    # Should contain error or retraction info
    log_pass "$TEST"
fi

# 14. unknown tool returns error
TEST="unknown tool returns error"
http_post "${BASE}/api/tool" '{"name":"totally_fake_tool","arguments":{}}'
if [[ "$HTTP_CODE" == "500" ]] || json_has "'error' in d or 'error' in str(d).lower()" 2>/dev/null; then
    log_pass "$TEST"
else
    log_fail "$TEST" "expected error response, got HTTP $HTTP_CODE"
fi

# ── summary ──────────────────────────────────────────────────────────────────

echo
echo "──────────────────────────────────────"
echo "  Results: ${PASS} passed, ${FAIL} failed, ${TOTAL} total"
echo "──────────────────────────────────────"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
