#!/usr/bin/env bash
#
# Core Vela release gate.
# This intentionally checks the v0 OSS surface: protocol CLI, conformance,
# BBB proof state, HTTP/MCP serving, proof packet export, and stale-name gates.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VELA="./target/release/vela"

assert_removed_command() {
  local command="$1"
  if "$VELA" "$command" --help >/dev/null 2>&1; then
    echo "Removed command is still exposed: vela $command"
    exit 1
  fi
}

assert_command_surface_snapshot() {
  local actual expected
  actual="$(mktemp)"
  expected="$(mktemp)"

  "$VELA" --help | awk '
    /^(Core commands|State commands):/ { in_commands = 1; next }
    /^Quick start:/ { in_commands = 0 }
    in_commands && /^  [[:alnum:]][[:alnum:]-]+/ { print $1 }
  ' > "$actual"

  cat > "$expected" <<'COMMANDS'
compile
check
normalize
proof
serve
stats
search
tensions
gaps
bridge
ingest
jats
export
packet
bench
conformance
sign
version
init
import
diff
proposals
finding
link
frontier
actor
registry
review
note
caveat
revise
reject
history
import-events
retract
propagate
COMMANDS

  if ! diff -u "$expected" "$actual"; then
    rm -f "$actual" "$expected"
    echo "Command surface changed. Update the strict release snapshot intentionally."
    exit 1
  fi

  rm -f "$actual" "$expected"
}

echo "== Design voice =="
./scripts/voice-check.sh

echo "== Rust formatting =="
cargo fmt --all -- --check

echo "== Rust tests =="
cargo test --workspace

echo "== Clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== Conformance =="
cargo run -p vela-protocol -- conformance tests/conformance/

echo "== Release binary =="
cargo build --release -p vela-protocol

echo "== Removed command gates =="
assert_removed_command benchmark
assert_removed_command claim
assert_removed_command packet-inspect
assert_removed_command packet-validate
assert_removed_command project
assert_removed_command pre-frontier
assert_removed_command hub
assert_removed_command desktop
assert_removed_command experiment
assert_removed_command protocol
assert_removed_command pipeline
assert_removed_command template
assert_removed_command inventory
assert_removed_command dashboard
assert_removed_command ai
assert_removed_command notebook
assert_removed_command open
assert_removed_command watch
assert_removed_command subscribe
assert_removed_command monitor
assert_removed_command extension
assert_removed_command market
assert_removed_command sync-db
assert_removed_command code
assert_removed_command write
assert_removed_command viz
assert_removed_command publish
assert_removed_command share
assert_removed_command agent

echo "== Command surface snapshot =="
assert_command_surface_snapshot

echo "== BBB state check =="
./target/release/vela check frontiers/bbb-alzheimer.json

echo "== BBB proof workflow =="
./demo/run-bbb-proof.sh

echo "== HTTP server smoke =="
./tests/test-http-server.sh

echo "== MCP server smoke =="
./tests/test-mcp-server.sh

echo "== Benchmark regression =="
./tests/test-benchmark-regression.sh

echo "== Local corpus workflow =="
./tests/test-local-corpus-workflow.sh

echo "== Release asset packaging =="
./scripts/package-release-assets.sh /tmp/vela-release-assets

echo "== Release surface path gate =="
blocked_paths='^(hub|desktop|future|sites|docs/archive|docs/reference|docs/strategy|crates/(api|commands|compat-harness|mock-anthropic-service|plugins|runtime|telemetry|tools|vela-agent|vela-tools)|\.mcp(|-multi)\.json|mock_parity_scenarios\.json|scripts/export-for-web\.py)(/|$)'
if git ls-files | rg -n "$blocked_paths"; then
  echo "Tangential UI/runtime/archive/reference/generated files are tracked in the release surface."
  exit 1
fi

echo "== Canonical artifact gate =="
if git ls-files 'frontiers/*.json' | rg -v '^frontiers/(bbb-alzheimer|bbb-extension|will-alzheimer-landscape)\.json$'; then
  echo "Only the canonical release frontiers (bbb-alzheimer, bbb-extension, will-alzheimer-landscape) may be tracked under frontiers/."
  exit 1
fi
if git ls-files | rg -n '^demo/bbb-proof-run-'; then
  echo "Generated BBB proof run directories are tracked."
  exit 1
fi
if git ls-files | rg -n '^projects/bbb-flagship/\.vela/(links|reviews|runs|trails)(/|$)'; then
  echo "Legacy BBB .vela sidecars are tracked in the release workspace."
  exit 1
fi

echo "== Current docs gate =="
approved_docs='^docs/(BENCHMARKS|BRAND|CLI_JSON|CORE_DOCTRINE|EVAL_CARD|FIRST_FRONTIER|FRONTIER_REVIEW|HUB|MATH|MCP|MCP_SETUP|PRIVATE_EVALUATOR_NOTE|PROOF|PROTOCOL|PUBLISHING|PYTHON|REGISTRY|STATE_TRANSITION_SPEC|THEORY|TIERS|TRACE_FORMAT|V0_DOGFOOD_REPORT|V0_RELEASE_NOTES|WORKBENCH)\.md$'
if find docs -maxdepth 1 -type f | sort | rg -v "$approved_docs"; then
  echo "Docs outside the current v0 surface are present."
  exit 1
fi

echo "== Stale release-surface name gate =="
legacy_pattern='c''law|\.''c''law|CLA''UDE|\.''claude-plugin|C''LAW_CONFIG_HOME|RUSTY_''CLAUDE|CLAUDE_''CODE|corridor'
if rg -n "$legacy_pattern" \
  --glob '!target/**' \
  --glob '!scripts/release-check.sh'
then
  echo "Stale Claw/Claude naming leaked into the release surface."
  exit 1
fi

echo "== Overclaim gate =="
overclaim_pattern='operating system for science|GitHub \+ Cursor \+ HuggingFace \+ Codex'
if rg -n "$overclaim_pattern" \
  README.md docs AGENTS.md CONTRIBUTING.md
then
  echo "Overclaim language leaked into the current product surface."
  exit 1
fi

echo "== Clean release tree gate =="
if ! git diff --exit-code -- frontiers/bbb-alzheimer.json README.md docs scripts crates tests examples .github; then
  echo "Release check mutated a tracked release-surface file."
  exit 1
fi
if [[ -n "$(git status --short)" ]]; then
  git status --short
  echo "Release check must leave a clean working tree."
  exit 1
fi

echo "Release check passed."
