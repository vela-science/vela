#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's checked-in Rust test union and current
# public object boundary. Git history retains removed products; routine CI does
# not spend time proving that deleted source is still absent.
PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify.py
cargo test --quiet --locked --workspace --all-targets
cargo test --quiet --locked --workspace --doc
printf 'core surface: ok (external Lean not selected)\n'
