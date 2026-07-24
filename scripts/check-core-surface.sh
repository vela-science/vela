#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# The routine contract exercises Vela's protocol, task-first writer boundary,
# frozen exact verifiers, and read-only Hub. External Lean is an optional
# integration lane and must never be pulled into this gate accidentally.
./scripts/check-prelaunch-surface.sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest conformance.test_verify_manifest
PYTHONDONTWRITEBYTECODE=1 python3 conformance/verify.py --authority-history-only
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
  --test frontier_repo \
  --test proposal_signature_parity \
  --test trust_invariants
cargo test --quiet -p vela-edge --lib
cargo test --quiet -p vela-cli --lib \
  --test aliases \
  --test env_isolation \
  --test finding_write_boundary \
  --test pre_adr_replay_golden \
  --test receipt_surface_parity \
  --test task_first_workflows
cargo test --quiet -p vela-hub --lib --bin vela-hub \
  --test event_kind_transparency

printf 'core surface: ok (external Lean not selected)\n'
