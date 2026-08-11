#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's checked-in Rust test union and current
# public object boundary. Git history retains removed products; routine CI does
# not spend time proving that deleted source is still absent.
uv run --project conformance --locked ruff check \
  conformance scripts .github/release/*.py examples/computational-science/experiment.py
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python conformance/verify.py
cargo test --quiet --locked --workspace --all-targets
cargo test --quiet --locked --workspace --doc
printf 'core surface: ok (external Lean not selected)\n'
