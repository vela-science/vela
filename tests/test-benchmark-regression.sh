#!/usr/bin/env bash
#
# Benchmark regression gate.
# Runs the Vela bench command against the gold standard and asserts
# the F1 score meets the minimum threshold.
#
set -euo pipefail

BINARY="./target/release/vela"
FRONTIER="frontiers/bbb-alzheimer.json"
SUITE="benchmarks/suites/bbb-core.json"
FINDING_GOLD="benchmarks/gold/findings/bbb-core-50.json"
ENTITY_GOLD="benchmarks/gold/entities/bbb-entity-50.json"
LINK_GOLD="benchmarks/gold/links/bbb-link-50.json"

PASS=0
FAIL=0
TOTAL=0

# ── helpers ──────────────────────────────────────────────────────────────────

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

# ── build ────────────────────────────────────────────────────────────────────

cd "$(dirname "$0")/.."

if [[ ! -f "$BINARY" ]]; then
    echo "Building release binary..."
    cargo build --release -p vela-protocol
else
    echo "Release binary found."
fi

if [[ ! -f "$FRONTIER" ]]; then
    echo "ERROR: Frontier file not found: $FRONTIER"
    exit 1
fi

if [[ ! -f "$SUITE" ]]; then
    echo "ERROR: Benchmark suite not found: $SUITE"
    exit 1
fi

# ── fixture validator ───────────────────────────────────────────────────────

echo "Validating benchmark fixtures..."
TEST="benchmark fixtures are tied to frontier"
if benchmarks/validate-benchmark-fixtures.py --suite "$SUITE" --json >/tmp/vela-benchmark-fixtures.json; then
  log_pass "$TEST"
else
  log_fail "$TEST" "fixture references drifted from $FRONTIER"
  cat /tmp/vela-benchmark-fixtures.json || true
fi

# ── run benchmark ────────────────────────────────────────────────────────────

echo "Running benchmark suite regression test..."
echo

RESULT=$("$BINARY" bench --suite "$SUITE" --json 2>/dev/null) || {
    echo "ERROR: benchmark suite failed"
    exit 1
}

# 1. Extract suite metrics
TEST="bench suite produces unified JSON"
METRICS=$(
  printf '%s\n' "$RESULT" | python3 -c "
import json
import sys

data = json.load(sys.stdin)
if not data.get('ok'):
    print(f\"suite not ok: {data.get('failures')}\", file=sys.stderr)
    sys.exit(1)
if data.get('benchmark_type') != 'suite':
    print('benchmark_type is not suite', file=sys.stderr)
    sys.exit(1)
if data.get('metrics', {}).get('standard_candles', 0) <= 0:
    print('suite is missing standard candle calibration anchors', file=sys.stderr)
    sys.exit(1)
if not data.get('standard_candles', {}).get('items'):
    print('standard_candles.items is empty', file=sys.stderr)
    sys.exit(1)
tasks = data.get('tasks', [])
modes = {task.get('mode') for task in tasks}
required = {'finding', 'entity', 'link', 'workflow'}
missing = sorted(required - modes)
if missing:
    print(f\"missing task modes: {', '.join(missing)}\", file=sys.stderr)
    sys.exit(1)
for task in tasks:
    if 'metrics' not in task or 'thresholds' not in task or 'failures' not in task:
        print(f\"task {task.get('task_id')} is missing unified envelope fields\", file=sys.stderr)
        sys.exit(1)
    if task.get('mode') == 'workflow':
        metrics = task.get('metrics', {})
        for key in ['evidence_span_coverage', 'provenance_coverage', 'total_provenance_complete']:
            if key not in metrics:
                print(f\"workflow task is missing {key}\", file=sys.stderr)
                sys.exit(1)

print(data['metrics']['tasks_passed'], data['metrics']['tasks_total'])
"
) || {
  log_fail "$TEST" "could not parse suite output"
  echo
  echo "──────────────────────────────────────"
  echo "  Results: ${PASS} passed, ${FAIL} failed, ${TOTAL} total"
  echo "──────────────────────────────────────"
  exit 1
}
read -r TASKS_PASSED TASKS_TOTAL <<< "$METRICS"
log_pass "$TEST"

echo "  suite tasks: $TASKS_PASSED / $TASKS_TOTAL passed"

# 2. Suite-ready helper should agree with the suite gate
TEST="suite-ready helper passes"
if "$BINARY" bench --suite "$SUITE" --suite-ready --json >/tmp/vela-suite-ready.json 2>/dev/null; then
  log_pass "$TEST"
else
  log_fail "$TEST" "--suite-ready failed"
fi

# 3. Single-mode finding JSON keeps the unified envelope
TEST="finding mode uses unified JSON envelope"
if "$BINARY" bench "$FRONTIER" --gold "$FINDING_GOLD" --json >/tmp/vela-bench-finding.json 2>/dev/null &&
   python3 - <<'PY'
import json
data = json.load(open('/tmp/vela-bench-finding.json'))
assert data['ok'] is True
assert data['mode'] == 'finding'
assert data['benchmark_type'] == 'finding'
assert data['metrics']['recall'] == 1.0
assert data['metrics']['exact_id_matches'] == data['gold']['items']
PY
then
  log_pass "$TEST"
else
  log_fail "$TEST" "finding envelope missing expected fields"
fi

# 4. Single-mode entity/link JSON envelopes should also pass
TEST="entity and link modes use unified JSON envelopes"
if "$BINARY" bench "$FRONTIER" --entity-gold "$ENTITY_GOLD" --json >/tmp/vela-bench-entity.json 2>/dev/null &&
   "$BINARY" bench "$FRONTIER" --link-gold "$LINK_GOLD" --json >/tmp/vela-bench-link.json 2>/dev/null &&
   python3 - <<'PY'
import json
entity = json.load(open('/tmp/vela-bench-entity.json'))
link = json.load(open('/tmp/vela-bench-link.json'))
assert entity['ok'] is True
assert entity['mode'] == 'entity'
assert entity['metrics']['type_accuracy'] == 1.0
assert link['ok'] is True
assert link['mode'] == 'link'
assert link['metrics']['f1'] == 1.0
PY
then
  log_pass "$TEST"
else
  log_fail "$TEST" "entity/link envelope missing expected fields"
fi

# 5. Removed legacy command should stay gone.
TEST="legacy benchmark command is removed"
if "$BINARY" benchmark --help >/dev/null 2>&1; then
  log_fail "$TEST" "legacy 'benchmark' command still exists; use 'bench'"
else
  log_pass "$TEST"
fi

# ── summary ──────────────────────────────────────────────────────────────────

echo
echo "──────────────────────────────────────"
echo "  Results: ${PASS} passed, ${FAIL} failed, ${TOTAL} total"
echo "──────────────────────────────────────"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
