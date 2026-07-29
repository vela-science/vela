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
  conformance/fixtures/decision-brief-testing-v1
  conformance/target-index-v2
)

for path in "${absent_paths[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    fail "retired path returned: $path"
  fi
done

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
