#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's current protocol, task-first writer
# boundary, and frozen exact verifiers. External Lean is an optional
# integration lane and must never be pulled into this gate accidentally.
./conformance/check-retired-surface.sh
PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify.py
cargo test --quiet -p vela-verify --lib
# `cli_release_contract` executes the real product binary. Build it explicitly
# so a clean checkout does not accidentally depend on a stale local artifact.
cargo build --quiet -p vela-cli --bin vela
cargo test --quiet -p vela-protocol --lib \
  --test action_contracts \
  --test canonical_hashing_conformance \
  --test cli_release_contract \
  --test current_object_interop \
  --test exact_witness_floor_fixture \
  --test foreign_transfer_contract_gap \
  --test frontier_settings_v1
cargo test --quiet -p vela-edge --lib --test correction_impact
cargo test --quiet -p vela-cli --lib \
  --test current_genesis \
  --test env_isolation \
  --test claim_write_boundary \
  --test target_index_cli
printf 'core surface: ok (external Lean not selected)\n'
