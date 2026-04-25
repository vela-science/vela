#!/usr/bin/env bash
#
# serve-web.sh — preview the Vela landing page locally with the same
# layout the GitHub Pages workflow deploys. Stages web/ + assets/brand
# into a temp directory and serves it on :8000.
#
# Mirrors .github/workflows/pages.yml — if this renders correctly
# locally, the Pages deploy will render correctly too.

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
STAGE="${TMPDIR:-/tmp}/vela-web-preview"
PORT="${1:-8000}"

rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -R "$ROOT/web/." "$STAGE/"
mkdir -p "$STAGE/assets"
cp -R "$ROOT/assets/brand" "$STAGE/assets/brand"

echo "serving $STAGE on http://localhost:$PORT"
exec python3 -m http.server --directory "$STAGE" "$PORT"
