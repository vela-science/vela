# Inspect one exact Frontier continuation

Work only in `/workspace/frontier`, an isolated checkout of the exact commit
named in `/opt/vela-input/fixture.json`.

{{TOOL_GUIDANCE}}

The fixture identifies an accepted source Claim and its exact foreign-reference
archive, but intentionally does not name the receiver Proposal. Identify the one
current receiver Proposal that binds that source anchor. Distinguish accepted
source Standing from pending local Standing, reject any typo or unrelated
Proposal, and report the Proposal's Submission and Verification identities, its
explicitly scoped conditional Standing change, and the actions that follow a
human Decision. The receiver has no configured Target; do not invent one.

Write exactly one JSON answer conforming to
`/opt/vela-input/answer.schema.json` at `/logs/artifacts/answer.json`. Do not
modify the checkout. Do not perform or simulate Accept, Reject, Cancel, signing,
publication, or any authority action. Verification is evidence, not acceptance.
