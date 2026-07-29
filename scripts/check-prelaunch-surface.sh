#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

retired_receipt_version=0

fail() {
  printf 'prelaunch surface drift: %s\n' "$1" >&2
  exit 1
}

# These were alternate implementations or writer paths, not published API.
# Reintroducing one must be an explicit architecture decision, not accidental
# compatibility growth before Vela's first external protocol release.
absent_paths=(
  .github/actions/vela-check
  .github/workflows/vela-check.yml
  bindings/python
  crates/vela-cli/src/cli/links.rs
  crates/vela-cli/src/tools/cli_attempt.rs
  crates/vela-cli/src/write/cli_claim.rs
  crates/vela-edge/embedded
  crates/vela-hub
  crates/vela-protocol/embedded/carina-schemas
  crates/vela-protocol/src/objects/frontier_template.rs
  docs/CARINA.md
  docs/HUB.md
  examples/carina-kernel
  integrations/claude-plugin
  schema/carina.artifact-packet.v0.1.json
  scripts/seed-erdos-formalization.sh
  templates/frontier
  "tools/receipt-v${retired_receipt_version}"
  tools/receipt-v1
)

for path in "${absent_paths[@]}"; do
  if [[ -f "$path" || -L "$path" ]] \
    || { [[ -d "$path" ]] && find "$path" -mindepth 1 -type f -print -quit | grep -q .; }
  then
    fail "retired path returned: $path"
  fi
done

# Generated first-run guidance is executable product surface. It must not
# resurrect commands removed by the prelaunch hard cut.
generated_guidance_surfaces=(
  crates/vela-cli/src/cli/help_text.rs
  crates/vela-protocol/src/computed/frontier_repo.rs
  crates/vela-protocol/src/analysis/evidence_diff.rs
  crates/vela-cli/src/current_doctor.rs
  docs/AGENT_QUICKSTART.md
)
retired_generated_command_pattern='vela (inbox|integrity|stats|task|source-inbox|claim diff)([^[:alnum:]_-]|$)|vela gate \.|attempt the accept|--template adoption-frontier'
if grep -nE "$retired_generated_command_pattern" "${generated_guidance_surfaces[@]}" >/dev/null; then
  grep -nE "$retired_generated_command_pattern" "${generated_guidance_surfaces[@]}" >&2
  fail "retired command remains in generated first-run guidance"
fi

if grep -nF '["serve", ".", "--profile", "draft"]' docs/AGENT_QUICKSTART.md >/dev/null; then
  grep -nF '["serve", ".", "--profile", "draft"]' docs/AGENT_QUICKSTART.md >&2
  fail "retired generated draft MCP profile remains in the agent quickstart"
fi
if ! grep -nF 'creates no MCP configuration' docs/AGENT_QUICKSTART.md >/dev/null; then
  fail "agent quickstart does not document minimal initialization"
fi

# Carina was a prelaunch duplicate kernel. It must not survive in either the
# maintained manifests or their regenerable materialized views.
while IFS= read -r path; do
  carina_pattern='(^|[^[:alnum:]_])carina(_kernel)?([^[:alnum:]_]|$)'
  if grep -nEi "$carina_pattern" "$path" >/dev/null; then
    grep -nEi "$carina_pattern" "$path" >&2
    fail "Carina metadata returned in a maintained frontier: $path"
  fi
done < <(
  find examples -type f \
    \( -name frontier.yaml -o -name frontier.json -o -name vela.lock -o -path '*/proof/latest.json' \) \
    | LC_ALL=C sort
)

receipt_v1_files=(
  crates/vela-cli/resources/receipt_v1.py
  crates/vela-cli/resources/vela_receipt_v1.py
  crates/vela-cli/resources/receipt_json.py
)
for path in "${receipt_v1_files[@]}"; do
  [[ -f "$path" ]] || fail "Receipt v1 portable tool is incomplete: $path"
done

if grep -nE '^def ' crates/vela-cli/resources/receipt_v1.py >/dev/null; then
  fail "Receipt v1 command harness reimplemented production logic"
fi
for symbol in \
  receipt_body_sha256 \
  make_receipt \
  attestation_binding \
  validate_safe_public_artifact_descriptors \
  validate_restricted_artifact_mirrors \
  validate_receipt
do
  definitions="$({ grep -hE "^def ${symbol}\\(" crates/vela-cli/resources/*.py || true; } | wc -l | tr -d ' ')"
  [[ "$definitions" == 1 ]] \
    || fail "Receipt v1 production symbol must have one implementation: $symbol ($definitions found)"
done
if git grep -nE 'from receipt_v1 import' -- \
  crates/vela-cli/resources scripts \
  ':(exclude)scripts/check-prelaunch-surface.sh' >/dev/null
then
  fail "installed code imports the command harness instead of the Receipt v1 core"
fi

retired_receipt_pattern="receipt-v${retired_receipt_version}|vela_receipt_v${retired_receipt_version}|vela-receipt-v${retired_receipt_version}"
if git grep -nE "$retired_receipt_pattern" -- \
  . ':(exclude)scripts/check-prelaunch-surface.sh' >/dev/null
then
  git grep -nE "$retired_receipt_pattern" -- \
    . ':(exclude)scripts/check-prelaunch-surface.sh' >&2
  fail "retired Receipt tool naming returned"
fi

receipt_binding_surfaces=(
  crates/vela-protocol/src/objects/receipt_v1.rs
  crates/vela-cli/resources/vela_receipt_v1.py
  crates/vela-edge/src/analysis/decision_brief.rs
  docs/history/RECEIPTS.md
)
retired_receipt_binding_pattern='AttestationBinding|LegacyUnbound|legacy[-_ ]unbound'
if grep -nE "$retired_receipt_binding_pattern" "${receipt_binding_surfaces[@]}" >/dev/null; then
  grep -nE "$retired_receipt_binding_pattern" "${receipt_binding_surfaces[@]}" >&2
  fail "retired unbound Receipt lane returned"
fi

retired_policy_surfaces=(
  crates/vela-protocol/src/policy/acceptance_policy.rs
  crates/vela-protocol/src/proposals/policy_accept.rs
  crates/vela-cli/src
  docs/PROTOCOL.md
  docs/adr/0003-rigorous-core-task-first-workflow.md
)
retired_policy_pattern='LegacyUnbound|legacy_unbound_closed|legacy_unbound_policy_content_address|verify_legacy_unbound_policy_pair|ClassifiedPolicyPair|verify_historical_policy_lane_event|HistoricalPolicyLane|legacy-policy-rotation|legacy_checkpoint_event_ids'
if git grep -nE "$retired_policy_pattern" -- "${retired_policy_surfaces[@]}" >/dev/null; then
  git grep -nE "$retired_policy_pattern" -- "${retired_policy_surfaces[@]}" >&2
  fail "retired acceptance-policy compatibility returned"
fi

forbidden_code=(
  'ProposalAction::(Accept|Reject)'
  'AttemptAction::Import'
  'FoundryAction::Run'
  'GateAction::AutoAdmit'
  'GateAction::(Backfill|Attach)'
  'FoundryAction::LeanRun'
  'publish_decision\('
  'create_or_apply\('
  '\bDEFAULT_CARINA_KERNEL\b'
  '\bcarina_kernel\b'
  'refuse_legacy_finding_apply\('
  '\bno_commit\b'
  '\bno_git\b'
  'publish\.git_commit'
  'propose_pending\('
  'FrontierAction::AddDep'
  '\bLinkAction\b'
  'ActorAction::Rotate'
  'register_witness_artifact\('
  'register_canonical_witnesses\('
  'apply_deposit_attempt_to_project\('
  'transact_attempt_deposit_candidate_with_barrier\('
  '\bdeposit_attempt\('
  'submit_diff_pack\('
  'record_scoped_attestation\('
  'release_pack_at_path\('
  'sign_registered_events\('
  'FindingCommands::(Add|Supersede|Note|Caveat|Revise|Review|Reject|Retract|Contribution)'
  'ProposalAction::Import'
  'cmd_finding_(add|supersede|note|caveat|revise|review|reject|retract|contribution)\('
  'cmd_review_fidelity_batch\('
  'apply_one_faithfulness\('
  'tool_propose\('
  'write_tool_propose\('
  'record_propose\('
  'transact_signed_proposal\('
  'transact_pending_proposal'
  'authorize_signed_proposal_write\('
  'create_with_verified_proposal_write_in_frontier\('
  'base_nonlease_event_log_root: Option'
  'pub fn frontier_next\('
  'pub fn (add_finding|review_finding|add_note|caveat_finding|revise_confidence|record_contribution|reject_finding|repair_finding_span|repair_evidence_atom_locator|retract_finding|add_artifact|supersede_finding)\('
)

for pattern in "${forbidden_code[@]}"; do
  if find crates -type f -name '*.rs' -exec grep -nE "$pattern" {} + >/dev/null; then
    find crates -type f -name '*.rs' -exec grep -nHE "$pattern" {} + >&2
    fail "retired Rust writer or command returned: $pattern"
  fi
done

# The first external-Lean experiment carried a producer-specific replay-packet
# lane. Current Vela exposes only the generic commit-pinned pull boundary; old
# packet compatibility and producer names must not return inside the installed
# verifier resource.
external_lean_surface=crates/vela-cli/resources/external_lean_verifier.py
retired_external_lean_packet_pattern='[Dd]iderot|KrafftSieve|EXTERNAL_LEAN_REPLAY_SCHEMA|packet_execution_index|controlled_lakefile|--packet|--bundle-root|--print-environment-contract-candidate'
if grep -nE -- "$retired_external_lean_packet_pattern" "$external_lean_surface" >/dev/null; then
  grep -nE -- "$retired_external_lean_packet_pattern" "$external_lean_surface" >&2
  fail "retired external-Lean packet compatibility returned"
fi

current_finding_schemas=(schema/finding-bundle.v*.json)
[[ ${#current_finding_schemas[@]} -eq 1 ]] \
  || fail "expected exactly one finding-bundle schema"
[[ ${current_finding_schemas[0]} == schema/finding-bundle.v0.10.0.json ]] \
  || fail "unexpected current finding-bundle schema: ${current_finding_schemas[*]}"

# Operational guidance exposes the one task-first producer loop. Historical
# ADRs and replay notes may still name retired surfaces as history.
operational_docs=(README.md docs/CLI.md docs/AGENT_QUICKSTART.md)
for command in \
  'vela gate auto-admit' \
  'vela proposals accept' \
  'vela proposals reject' \
  'vela finding link add' \
  'vela frontier add-dep' \
  'vela actor rotate' \
  'vela state anchor' \
  'vela state unanchor' \
  'vela finding add' \
  'vela finding supersede' \
  'vela finding note' \
  'vela finding caveat' \
  'vela finding revise' \
  'vela finding review' \
  'vela finding reject' \
  'vela finding retract' \
  'vela finding contribution' \
  'vela proposals import' \
  'vela sign --batch' \
  'vela gate attach' \
  'vela gate backfill' \
  'vela foundry lean-run'
do
  if grep -nF "$command" "${operational_docs[@]}" >/dev/null; then
    grep -nF "$command" "${operational_docs[@]}" >&2
    fail "retired command remains in operational guidance: $command"
  fi
done

# Active product and adapter guidance must never route a human back to the
# retired batch-signing workflow. Historical parsers and replay fixtures remain.
decision_guidance_files=(
  crates/vela-protocol/src/proposals/policy_accept.rs
  crates/vela-protocol/src/analysis/scientific_diff.rs
  crates/vela-cli/src/server/cli_commands.rs
)
for guidance in \
  'vela sign --frontier <frontier>' \
  'human decides it through `vela sign`' \
  'terminal-only (`vela sign`)' \
  'Re-issue the decision through `vela sign`' \
  'with their key via `vela sign`'
do
  if grep -nF "$guidance" "${decision_guidance_files[@]}" >/dev/null; then
    grep -nF "$guidance" "${decision_guidance_files[@]}" >&2
    fail "retired batch-signing guidance remains in an active product surface: $guidance"
  fi
done

# `work` has one exact task-first action set. A historical attempt event may
# remain replayable, but no current MCP adapter or generated guidance may offer
# the retired deposit writer.
work_surfaces=(
  crates/vela-cli/src/config/cli_agents.rs
  docs/AGENT_QUICKSTART.md
)
retired_work_pattern='Some\("deposit"\)|action=deposit|claim/land/drop/deposit|claim\|land\|drop\|deposit|deposit an attempt'
if grep -nE "$retired_work_pattern" "${work_surfaces[@]}" >/dev/null; then
  grep -nE "$retired_work_pattern" "${work_surfaces[@]}" >&2
  fail "retired MCP work deposit action returned"
fi

retired_mcp_propose_pattern='`propose`|"propose"'
mcp_propose_surfaces=(
  crates/vela-cli/src/config/cli_agents.rs
  docs/AGENT_QUICKSTART.md
)
if grep -nE "$retired_mcp_propose_pattern" "${mcp_propose_surfaces[@]}" >/dev/null; then
  grep -nE "$retired_mcp_propose_pattern" "${mcp_propose_surfaces[@]}" >&2
  fail "retired MCP propose tool returned"
fi
printf 'prelaunch surface: ok\n'
