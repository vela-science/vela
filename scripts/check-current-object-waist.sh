#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
vela="${VELA_BIN:-$repo/target/debug/vela}"
for executable in "$vela" git jq node ssh-agent ssh-add ssh-keygen; do
  command -v "$executable" >/dev/null 2>&1 || {
    echo "missing required executable: $executable" >&2
    exit 2
  }
done

root="$(mktemp -d "${TMPDIR:-/tmp}/vela-current-waist.XXXXXX")"
agent_started=false
cleanup() {
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

"$vela" init "$frontier" \
  --name 'Interop Frontier' \
  --scope 'Exercise current Submission and Verification Record interoperability.' \
  --json >"$root/init.json"
git init -q --bare "$remote"
git -C "$frontier" init -q
git -C "$frontier" branch -M main
git -C "$frontier" add --all
git -C "$frontier" commit -q -m 'Initialize disposable Frontier'
git -C "$frontier" remote add origin "$remote"
git -C "$frontier" push -q -u origin main
git --git-dir="$remote" symbolic-ref HEAD refs/heads/main

"$vela" authority init "$frontier" \
  --reason 'Establish ephemeral authority for a disposable interoperability Frontier.' \
  --json >"$root/authority-init.json"
publish_fixture_delta 'Initialize disposable repository authority'

event_root_before="$("$vela" status "$frontier" --json | jq -r '.roots.event_log')"
"$vela" submit "$repo/conformance/current-objects/submission.json" \
  --frontier "$frontier" \
  --as agent:independent-js \
  --json >"$root/submit.json"
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
node "$repo/clients/javascript/vela_emit.mjs" verification \
  --draft "$root/verification-draft.json" \
  --seed-file "$repo/conformance/current-objects/verifier.seed.hex" \
  --output "$root/verification.json" \
  >"$root/emission.json"
"$vela" verification import "$frontier" "$root/verification.json" \
  --as verifier:independent-js \
  --json >"$root/import.json"
publish_fixture_delta 'Retain independent Verification Record'

"$vela" review show "$frontier" "$proposal_id" --json >"$root/review.json"
"$vela" status "$frontier" --json >"$root/status.json"
event_root_after="$(jq -r '.roots.event_log' "$root/status.json")"

[[ "$event_root_before" == "$event_root_after" ]]
[[ "$(jq -r '.accepted_event_delta' "$root/submit.json")" == 0 ]]
[[ "$(jq -r '.accepted_event_delta' "$root/import.json")" == 0 ]]
[[ "$(jq -r '.accepted_state_changed' "$root/submit.json")" == false ]]
[[ "$(jq -r '.verification_records | length' "$root/review.json")" == 1 ]]
[[ "$(jq -r '.review.brief.authority.actions[] | select(.action=="accept") | .eligibility' "$root/review.json")" == blocked ]]
[[ "$(jq -r '.review.brief.authority.actions[] | select(.action=="reject") | .eligibility' "$root/review.json")" == available ]]
[[ -z "$(git -C "$frontier" status --porcelain=v1 --untracked-files=all)" ]]

git clone -q --no-hardlinks "$remote" "$replay"
"$vela" status "$replay" --json >"$root/replay-status.json"
"$vela" review show "$replay" "$proposal_id" --json >"$root/replay-review.json"
[[ "$(jq -r '.roots.event_log' "$root/replay-status.json")" == "$event_root_after" ]]
[[ "$(jq -r '.verification_records[0].verification_record_id' "$root/replay-review.json")" == "$(jq -r '.verification_record_id' "$root/import.json")" ]]
[[ -z "$(git -C "$replay" status --porcelain=v1 --untracked-files=all)" ]]

jq -cn \
  --arg submission_id "$submission_id" \
  --arg submission_root "$submission_root" \
  --arg registration_record_id "$(jq -r '.registration_record_id' "$root/submit.json")" \
  --arg registration_record_root "$(jq -r '.registration_record_root' "$root/submit.json")" \
  --arg proposal_id "$proposal_id" \
  --arg claim_id "$claim_id" \
  --arg verification_record_id "$(jq -r '.verification_record_id' "$root/import.json")" \
  --arg verification_record_root "$(jq -r '.verification_record_root' "$root/import.json")" \
  --arg event_log_root "$event_root_after" \
  '{
    schema:"vela.current-object-waist-check.v1",
    ok:true,
    submission_id:$submission_id,
    submission_root:$submission_root,
    registration_record_id:$registration_record_id,
    registration_record_root:$registration_record_root,
    proposal_id:$proposal_id,
    claim_id:$claim_id,
    verification_record_id:$verification_record_id,
    verification_record_root:$verification_record_root,
    event_log_root:$event_log_root,
    accepted_event_delta:0,
    standing:"pending_review",
    clean_clone_replayed:true
  }'
