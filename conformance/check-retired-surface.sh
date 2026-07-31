#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  printf 'retired surface drift: %s\n' "$1" >&2
  exit 1
}

# These paths belonged to predecessor products, migrations, or duplicate
# implementations. Historical commits and ADRs retain their evidence; the
# current source tree does not retain executable compatibility surfaces.
absent_paths=(
  .vela
  atlas
  clients
  examples
  frontiers
  lean
  research
  schema
  scripts
  crates/vela-edge/src/analysis/decision_brief.rs
  crates/vela-edge/src/analysis/frontier_repository.rs
  crates/vela-edge/src/registry
  crates/vela-edge/src/review
  crates/vela-edge/src/validation
  crates/vela-hub
  crates/vela-cli/resources
  conformance/fixtures/decision-brief-testing-v1
  conformance/target-index-v2
  conformance/readers/python/reducer.py
  conformance/readers/typescript/reducer.ts
  conformance/actor-registration-boundary-v1.json
  conformance/decision-binding.json
  conformance/erdos-actor-registration-preview-v1.json
  conformance/gate-vectors.json
  conformance/spec-surface.v1.json
  conformance/test_verify_manifest.py
  crates/vela-protocol/src/analysis
  crates/vela-protocol/src/domains
  crates/vela-protocol/src/kernel/actor_registration.rs
  crates/vela-protocol/src/kernel/reducer.rs
  crates/vela-protocol/src/policy
  crates/vela-protocol/src/proposals
)

for path in "${absent_paths[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    fail "retired path returned: $path"
  fi
done

if find conformance/fixtures -maxdepth 1 -type f \
  \( -name 'cascade-fixture-*.json' -o -name 'fixtures.manifest.json' \
     -o -name 'permit-shadow-v1.json' \
     -o -name 'policy-scoped-producer-credential-v1.json' \
     -o -name 'routine-work-policy-v1.json' \) \
  | grep -q .; then
  fail "retired reducer or policy fixture returned"
fi

# Current product code has one Submission writer, one Verification import
# edge, and repository-authority Decisions. Keep removed alternate writers
# from reappearing under another command or adapter.
forbidden_code=(
  'ProposalAction::(Accept|Reject|Import)'
  'AttemptAction::Import'
  'FoundryAction'
  'GateAction::(AutoAdmit|Backfill|Attach)'
  'FindingCommands::(Add|Supersede|Note|Caveat|Revise|Review|Reject|Retract|Contribution)'
  'FrontierAction::AddDep'
  'publish_decision\('
  'submit_diff_pack\('
  'record_scoped_attestation\('
  'sign_registered_events\('
  'transact_pending_proposal'
)

for pattern in "${forbidden_code[@]}"; do
  if find crates -type f -name '*.rs' -exec grep -nHE "$pattern" {} + > /tmp/vela-retired-surface.$$; then
    cat /tmp/vela-retired-surface.$$ >&2
    rm -f /tmp/vela-retired-surface.$$
    fail "retired Rust writer returned: $pattern"
  fi
done
rm -f /tmp/vela-retired-surface.$$

# Operational guidance must teach the current object language and targeted
# Decision boundary, not predecessor batch or finding-mutation commands.
operational_docs=(README.md docs/CLI.md docs/AGENT_QUICKSTART.md)
retired_guidance='vela (proposals|finding (add|supersede|note|caveat|revise|review|reject|retract|contribution)|gate (auto-admit|attach|backfill)|foundry|frontier add-dep|sign --batch)'
if grep -nE "$retired_guidance" "${operational_docs[@]}" > /tmp/vela-retired-guidance.$$; then
  cat /tmp/vela-retired-guidance.$$ >&2
  rm -f /tmp/vela-retired-guidance.$$
  fail "retired command remains in operational guidance"
fi
rm -f /tmp/vela-retired-guidance.$$

printf 'retired surface: ok\n'
