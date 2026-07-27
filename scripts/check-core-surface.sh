#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's protocol, task-first writer boundary,
# frozen exact verifiers, and read-only derived projections. External Lean is
# an optional integration lane and must never be pulled into this gate
# accidentally.
./scripts/check-prelaunch-surface.sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest conformance.test_verify_manifest
PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify_principal_capability.py
cargo test --quiet -p vela-protocol-core --lib
cargo test --quiet -p vela-verify --lib
cargo test --quiet -p vela-protocol --lib \
  --test action_contracts \
  --test canonical_hashing_conformance \
  --test cli_release_contract \
  --test cross_frontier_dep_persistence \
  --test cross_impl_reducer_fixtures \
  --test evidence_ci \
  --test fixture_manifest_signature \
  --test frontier_policy \
  --test proposal_signature_parity \
  --test trust_invariants
cargo test --quiet -p vela-edge --lib
cargo test --quiet -p vela-cli --lib \
  --test canonical_source_commitment \
  --test env_isolation \
  --test claim_write_boundary \
  --test target_index_cli
printf 'core surface: ok (external Lean not selected)\n'
