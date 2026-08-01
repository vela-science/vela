# Inspect one exact Frontier handoff

Work only in `/workspace/frontier`, an isolated checkout of the exact commit
named in `/opt/vela-input/fixture.json`.

{{TOOL_GUIDANCE}}

Inspect the pending Proposal named in `/opt/vela-input/fixture.json`. Determine
the current Target, its exact packet and next command, the Proposal's Submission
and Verification identities, the explicitly scoped conditional Standing change,
and the actions that follow a human Decision.

Write exactly one JSON answer conforming to
`/opt/vela-input/answer.schema.json` at `/logs/artifacts/answer.json`. Do not
modify the checkout. Do not perform or simulate Accept, Reject, Cancel, signing,
publication, or any authority action. Verification is evidence, not acceptance.

Session: `{{SESSION_ID}}`.
