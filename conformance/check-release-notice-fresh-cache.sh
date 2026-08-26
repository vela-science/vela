#!/usr/bin/env bash
# Prove the release entry point owns the cache precondition for frozen notices.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_ABOUT_VERSION="$(tr -d '\r\n' < .github/release/cargo-about-version)"
CARGO_ABOUT_BIN="${VELA_CARGO_ABOUT_BIN:-$(command -v cargo-about || true)}"
if [[ -z "$CARGO_ABOUT_BIN" ]]; then
  echo "fresh-cache notice test: cargo-about $CARGO_ABOUT_VERSION is required" >&2
  exit 1
fi
observed="$($CARGO_ABOUT_BIN --version | awk '{print $2}')"
if [[ "$observed" != "$CARGO_ABOUT_VERSION" ]]; then
  echo "fresh-cache notice test: cargo-about $observed is not $CARGO_ABOUT_VERSION" >&2
  exit 1
fi

SCRATCH="$(mktemp -d)"
cleanup() {
  rm -rf "$SCRATCH"
}
trap cleanup EXIT
FRESH_CARGO_HOME="$SCRATCH/cargo-home"
mkdir -p "$FRESH_CARGO_HOME"

about() {
  CARGO_HOME="$FRESH_CARGO_HOME" "$CARGO_ABOUT_BIN" \
    -L error --color never generate \
    --config .github/release/about.toml \
    --target x86_64-unknown-linux-musl \
    --format json --frozen --fail --locked --workspace \
    --output-file "$SCRATCH/about.json"
}

if about >"$SCRATCH/before.stdout" 2>"$SCRATCH/before.stderr"; then
  echo "fresh-cache notice test: frozen cargo-about unexpectedly passed before fetch" >&2
  exit 1
fi
if ! grep -Eq \
  'failed to download|attempting to make an HTTP request|no matching package named' \
  "$SCRATCH/before.stderr"; then
  echo "fresh-cache notice test: failure diagnostics did not expose the cache miss" >&2
  cat "$SCRATCH/before.stderr" >&2
  exit 1
fi

CARGO_HOME="$FRESH_CARGO_HOME" scripts/release.sh --fetch-locked-graph-only
about

echo "fresh-cache notice test: prefetch negative and entry-point positive passed"
