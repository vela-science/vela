#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's complete checked-in Rust test union,
# current task-first writer boundary, and frozen exact verifiers. External Lean
# remains an explicit integration lane and is not selected by these targets.
./conformance/check-retired-surface.sh
PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify.py
cargo test --quiet --locked --workspace --all-targets
cargo test --quiet --locked --workspace --doc
printf 'core surface: ok (external Lean not selected)\n'
