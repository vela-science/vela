#!/usr/bin/env bash
#
# Integration tests for the Vela HTTP server.
# Builds the release binary if needed, starts the server on port 3099,
# runs a battery of endpoint tests, then cleans up.
#
set -euo pipefail

PORT=3099
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

# Check that a curl response has HTTP 200 and store the body.
# Usage: http_get <url> <test_name>
#   Sets BODY and HTTP_CODE.
http_get() {
    local url="$1"
    HTTP_CODE=$(curl -s -o /tmp/vela_test_body -w "%{http_code}" "$url")
    BODY=$(cat /tmp/vela_test_body)
}

http_post() {
    local url="$1"
    local data="$2"
    HTTP_CODE=$(curl -s -o /tmp/vela_test_body -w "%{http_code}" \
        -X POST -H "Content-Type: application/json" -d "$data" "$url")
    BODY=$(cat /tmp/vela_test_body)
}

# JSON field checks via python3 (available everywhere, no jq dependency).
json_has() {
    # json_has <json_string> <python_expression_returning_bool>
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

echo "Starting vela serve on port ${PORT}..."
"$BINARY" serve "$FRONTIER" --http "$PORT" &
SERVER_PID=$!

# Wait for readiness (poll /api/stats, up to 30 seconds).
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

echo "Server ready. Running tests..."
echo

# ── tests ────────────────────────────────────────────────────────────────────

# 1. GET /api/stats
TEST="GET /api/stats returns stats.findings == 48"
http_get "${BASE}/api/stats"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('stats',{}).get('findings') == 48"; then
    log_pass "$TEST"
else
    actual=$(json_extract "d.get('stats',{}).get('findings','MISSING')")
    log_fail "$TEST" "stats.findings = $actual"
fi

# 2. GET /api/frontier
TEST="GET /api/frontier has vela_version and findings array"
http_get "${BASE}/api/frontier"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "isinstance(d.get('findings'), list) and len(d['findings']) > 0"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing or empty findings array"
fi

# 3. GET /api/tools
TEST="GET /api/tools returns strict 18-tool registry"
http_get "${BASE}/api/tools"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "isinstance(d, list) and len(d) == 18"; then
    log_pass "$TEST"
else
    count=$(json_extract "len(d) if isinstance(d, list) else 'not a list'")
    log_fail "$TEST" "got $count tools"
fi

# 4. GET /api/findings?query=LRP1
TEST="GET /api/findings?query=LRP1 returns matched findings"
http_get "${BASE}/api/findings?query=LRP1"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "'findings matched' in d.get('result','')"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "response did not contain 'findings matched'"
fi

# 5. GET /api/findings?query=NONEXISTENT_THING_12345
TEST="GET /api/findings?query=NONEXISTENT_THING_12345 returns no match"
http_get "${BASE}/api/findings?query=NONEXISTENT_THING_12345"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "'No findings matched' in d.get('result','')"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "expected 'No findings matched'"
fi

# 6. GET /api/observer/pharma
TEST="GET /api/observer/pharma has policy and top_findings"
http_get "${BASE}/api/observer/pharma"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('policy') == 'pharma' and isinstance(d.get('top_findings'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing policy or top_findings"
fi

# 7. GET /api/observer/invalid_policy defaults to academic
TEST="GET /api/observer/invalid_policy still returns data"
http_get "${BASE}/api/observer/invalid_policy"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('policy') == 'invalid_policy' and isinstance(d.get('top_findings'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "unexpected response structure"
fi

# 8. GET /api/propagate/vf_5021284e4155f141
TEST="GET /api/propagate/vf_5021284e4155f141 has retracted and directly_affected"
http_get "${BASE}/api/propagate/vf_5021284e4155f141"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "'retracted' in d and 'directly_affected' in d"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing retracted or directly_affected fields"
fi

# 9. GET /api/frontiers
TEST="GET /api/frontiers has frontier_count"
http_get "${BASE}/api/frontiers"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "'frontier_count' in d and d['frontier_count'] >= 1"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing or zero frontier_count"
fi

# 10. POST /api/tool with find_bridges
TEST="POST /api/tool find_bridges returns structured bridges with signals"
http_post "${BASE}/api/tool" '{"name":"find_bridges","arguments":{"min_categories":2,"limit":5}}'
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "d.get('ok') is True and isinstance(d.get('data',{}).get('bridges'), list) and 'markdown' in d and isinstance(d.get('signals'), list) and isinstance(d.get('caveats'), list)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "response missing structured data/markdown/caveats"
fi

# 11. POST /api/tool with unknown_tool
TEST="POST /api/tool unknown_tool returns error"
http_post "${BASE}/api/tool" '{"name":"unknown_tool","arguments":{}}'
if [[ "$HTTP_CODE" == "500" ]] || json_has "'error' in d" 2>/dev/null; then
    log_pass "$TEST"
else
    log_fail "$TEST" "expected error response, got HTTP $HTTP_CODE"
fi

# 12. GET /api/pubmed?query=LRP1
TEST="GET /api/pubmed?query=LRP1 returns structured success or error"
http_get "${BASE}/api/pubmed?query=LRP1"
if [[ "$HTTP_CODE" != "200" ]]; then
    log_fail "$TEST" "HTTP $HTTP_CODE"
elif json_has "('pubmed_results' in d and d.get('query') == 'LRP1') or isinstance(d.get('error'), str)"; then
    log_pass "$TEST"
else
    log_fail "$TEST" "missing structured PubMed response"
fi

# ── summary ──────────────────────────────────────────────────────────────────

echo
echo "──────────────────────────────────────"
echo "  Results: ${PASS} passed, ${FAIL} failed, ${TOTAL} total"
echo "──────────────────────────────────────"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
