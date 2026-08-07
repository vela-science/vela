#!/usr/bin/env bash
set -euo pipefail

# This repository-wide check exercises the public cross-implementation waist.
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
vela="${VELA_BIN:-$repo/target/debug/vela}"
[[ "${VELA_EPHEMERAL_ACCOUNT_HOME:-}" == 1 ]] || {
  echo "refusing to write a synthetic trust pin outside an explicitly ephemeral account" >&2
  echo "run this check only with VELA_EPHEMERAL_ACCOUNT_HOME=1 on a disposable CI account" >&2
  exit 2
}
for executable in "$vela" git jq node ssh-agent ssh-add ssh-keygen; do
  command -v "$executable" >/dev/null 2>&1 || {
    echo "missing required executable: $executable" >&2
    exit 2
  }
done

root="$(mktemp -d "${TMPDIR:-/tmp}/vela-current-waist.XXXXXX")"
agent_started=false
trust_pin_path=""
cleanup() {
  case "$trust_pin_path" in
    */.vela/trust/authorities/vfr_*.json)
      /bin/rm -f -- "$trust_pin_path"
      ;;
  esac
  if [[ "$agent_started" == true ]]; then
    ssh-agent -k >/dev/null 2>&1 || true
  fi
  chmod -R u+w "$root" 2>/dev/null || true
  /bin/rm -rf -- "$root"
}
trap cleanup EXIT

home="$root/home"
frontier="$root/frontier"
remote="$root/remote.git"
replay="$root/replay"
mkdir -p "$home" "$frontier"
ssh-keygen -q -t ed25519 -N '' -C 'vela disposable authority' -f "$root/authority"
eval "$(ssh-agent -s)" >/dev/null
agent_started=true
ssh-add "$root/authority" >/dev/null

export HOME="$home"
export NO_COLOR=1
export VELA_ADVICE=0
git config --global user.name 'Vela Interop Fixture'
git config --global user.email 'fixture@vela.invalid'
publish_fixture_delta() {
  git -C "$frontier" add --all
  if ! git -C "$frontier" diff --cached --quiet; then
    git -C "$frontier" commit -q -m "$1"
  fi
  git -C "$frontier" push -q origin main
}

run_json() {
  local output="$1"
  shift
  if ! "$@" >"$output"; then
    cat "$output" >&2
    return 1
  fi
}

"$vela" init "$frontier" \
  --name 'Interop Frontier' \
  --scope 'Exercise current Submission and Verification Record interoperability.' \
  --json >"$root/init.json"
trust_pin_path="$(jq -er '.authority.local_trust.anchor_path' "$root/init.json")"
git init -q --bare "$remote"
git -C "$frontier" remote add origin "$remote"
git -C "$frontier" push -q -u origin main
git --git-dir="$remote" symbolic-ref HEAD refs/heads/main

"$vela" status "$frontier" --json >"$root/status-before.json"
accepted_claims_before="$(jq -r '.counts.accepted_claims' "$root/status-before.json")"
run_json "$root/submit.json" \
  "$vela" submit "$repo/conformance/current-objects/submission.json" \
  --repo "$frontier" \
  --as agent:independent-js \
  --json
publish_fixture_delta 'Register independent Submission'

claim_id="$(jq -r '.claim_id' "$root/submit.json")"
proposal_id="$(jq -r '.proposal_id' "$root/submit.json")"
submission_id="$(jq -r '.submission_id' "$root/submit.json")"
submission_root="$(jq -r '.submission_root' "$root/submit.json")"
jq \
  --arg claim "$claim_id" \
  --arg proposal "$proposal_id" \
  --arg submission "$submission_id" \
  --arg root "$submission_root" \
  '.subject.claim_id=$claim
   | .subject.proposal_id=$proposal
   | .subject.submission_id=$submission
   | .subject.submission_root=$root
   | .subject.artifact_ids=[]' \
  "$repo/conformance/current-objects/verification-draft.json" \
  >"$root/verification-draft.json"
cp "$repo/conformance/current-objects/verifier.seed.hex" "$root/verifier.seed.hex"
chmod 0600 "$root/verifier.seed.hex"
node "$repo/conformance/emitters/javascript.mjs" verification \
  --draft "$root/verification-draft.json" \
  --seed-file "$root/verifier.seed.hex" \
  --output "$root/verification.json" \
  >"$root/emission.json"
"$vela" verification import "$frontier" "$root/verification.json" \
  --as verifier:independent-js \
  --json >"$root/import.json"
publish_fixture_delta 'Retain independent Verification Record'

"$vela" review show "$frontier" "$proposal_id" --json >"$root/review.json"
"$vela" review inbox "$frontier" --json >"$root/inbox.json"
"$vela" status "$frontier" --json >"$root/status.json"
repository_root_after="$(jq -r '.roots.repository' "$root/status.json")"

[[ "$(jq -r '.counts.accepted_claims' "$root/status.json")" == "$accepted_claims_before" ]]
[[ "$(jq -r '.counts.pending_claims' "$root/status.json")" == 1 ]]
[[ "$(jq -r '.counts.pending_review' "$root/status.json")" == 1 ]]
[[ "$(jq -r '.accepted_event_delta' "$root/submit.json")" == 0 ]]
[[ "$(jq -r '.accepted_event_delta' "$root/import.json")" == 0 ]]
[[ "$(jq -r '.accepted_state_changed' "$root/submit.json")" == false ]]
[[ "$(jq -r '.verification_records | length' "$root/review.json")" == 1 ]]
# `status`, not `standing`. A Proposal has a lifecycle position; a Claim has a
# ruling. `review show` returned this one under `standing`, which was the last
# place a Proposal word travelled on the Claim axis, and renaming it left this
# check reading a key nothing emits — so `jq` answered `null` and the waist
# failed on a rename that was correct.
[[ "$(jq -r '.status' "$root/review.json")" == pending_review ]]
[[ "$(jq -r '.decision' "$root/review.json")" == null ]]
[[ "$(jq -r '.authority_boundary' "$root/review.json")" == "Verification records report bounded checks. A producer may close its own pending Proposal; only a repository-authority Decision can change accepted scientific Standing." ]]
[[ "$(jq -r '.schema' "$root/inbox.json")" == vela.decision-inbox.v2 ]]
[[ "$(jq -r '.entries | length' "$root/inbox.json")" == 1 ]]
[[ "$(jq -r '.decision_inbox.entry.entry_root' "$root/review.json")" == "$(jq -r '.entries[0].entry_root' "$root/inbox.json")" ]]
[[ "$(jq -r '.entries[0].standing_delta.before.repository_root' "$root/inbox.json")" == "$repository_root_after" ]]
[[ "$(jq -r '.entries[0].standing_delta.scope.affected_claim_ids | length' "$root/inbox.json")" == 1 ]]
[[ "$(jq -r '.entries[0].standing_delta.scope.affected_claim_ids[0]' "$root/inbox.json")" == "$claim_id" ]]
[[ "$(jq -r '.entries[0].standing_delta.counts.global_accepted_claims.before' "$root/inbox.json")" == "$accepted_claims_before" ]]
[[ "$(jq -r '.entries[0].standing_delta.counts.global_accepted_claims.if_accept' "$root/inbox.json")" == "$((accepted_claims_before + 1))" ]]
[[ "$(jq -r '.entries[0].standing_delta.counts.global_accepted_claims.if_reject' "$root/inbox.json")" == "$accepted_claims_before" ]]
[[ -z "$(git -C "$frontier" status --porcelain=v1 --untracked-files=all)" ]]

git clone -q --no-hardlinks "$remote" "$replay"
"$vela" status "$replay" --json >"$root/replay-status.json"
"$vela" review show "$replay" "$proposal_id" --json >"$root/replay-review.json"
"$vela" review inbox "$replay" --json >"$root/replay-inbox.json"
[[ "$(jq -r '.roots.repository' "$root/replay-status.json")" == "$repository_root_after" ]]
[[ "$(jq -r '.verification_records[0].record.verification_record_id' "$root/replay-review.json")" == "$(jq -r '.verification_record_id' "$root/import.json")" ]]
[[ "$(jq -r '.projection_root' "$root/replay-inbox.json")" == "$(jq -r '.projection_root' "$root/inbox.json")" ]]
[[ "$(jq -r '.entries[0].entry_root' "$root/replay-inbox.json")" == "$(jq -r '.entries[0].entry_root' "$root/inbox.json")" ]]
[[ -z "$(git -C "$replay" status --porcelain=v1 --untracked-files=all)" ]]

jq -cn \
  --arg submission_id "$submission_id" \
  --arg submission_root "$submission_root" \
  --arg proposal_id "$proposal_id" \
  --arg claim_id "$claim_id" \
  --arg verification_record_id "$(jq -r '.verification_record_id' "$root/import.json")" \
  --arg verification_record_root "$(jq -r '.verification_record_root' "$root/import.json")" \
  --arg repository_root "$repository_root_after" \
  --argjson accepted_claims "$accepted_claims_before" \
  '{
    schema:"vela.current-object-waist-check.v1",
    ok:true,
    submission_id:$submission_id,
    submission_root:$submission_root,
    proposal_id:$proposal_id,
    claim_id:$claim_id,
    verification_record_id:$verification_record_id,
    verification_record_root:$verification_record_root,
    repository_root:$repository_root,
    accepted_claims:$accepted_claims,
    accepted_event_delta:0,
    proposal_status:"pending_review",
    clean_clone_replayed:true
  }'
