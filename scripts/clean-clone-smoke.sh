#!/usr/bin/env bash
#
# Verify that Vela works from a fresh clone with no hidden local state.
#
# By default this runs only deterministic, checked-in artifact paths. Set
# VELA_CLEAN_CLONE_WITH_COMPILE=1 to also run a live one-paper compile smoke.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKDIR="${VELA_CLEAN_CLONE_WORKDIR:-$(mktemp -d "${TMPDIR:-/tmp}/vela-clean-clone.XXXXXX")}"
CLONE_DIR="$WORKDIR/vela"

cleanup() {
  if [[ "${VELA_CLEAN_CLONE_KEEP:-0}" != "1" ]]; then
    rm -rf "$WORKDIR"
  fi
}
trap cleanup EXIT

echo "== Clean clone smoke =="
echo "source: $ROOT"
echo "workdir: $WORKDIR"

git clone --quiet --local "$ROOT" "$CLONE_DIR"
cd "$CLONE_DIR"

git status --short --branch

echo "== Build release binary =="
cargo build --release -p vela-protocol
VELA="./target/release/vela"

echo "== Core deterministic workflow =="
"$VELA" --help >/dev/null
"$VELA" stats frontiers/bbb-alzheimer.json --json >/tmp/vela-clean-stats.json
"$VELA" check frontiers/bbb-alzheimer.json --json >/tmp/vela-clean-check.json
"$VELA" bench frontiers/bbb-alzheimer.json --gold benchmarks/gold-50.json --json >/tmp/vela-clean-bench.json
cp frontiers/bbb-alzheimer.json "$WORKDIR/bbb-alzheimer-proof-input.json"
"$VELA" proof "$WORKDIR/bbb-alzheimer-proof-input.json" --out /tmp/vela-clean-proof-packet --json >/tmp/vela-clean-proof.json
"$VELA" packet validate /tmp/vela-clean-proof-packet >/tmp/vela-clean-packet.txt

if [[ "${VELA_CLEAN_CLONE_WITH_COMPILE:-0}" == "1" ]]; then
  echo "== Live compile smoke =="
  "$VELA" compile "blood brain barrier Alzheimer" -n 1 --output /tmp/vela-clean-frontier.json
  "$VELA" check /tmp/vela-clean-frontier.json
fi

echo "Clean clone smoke passed."
