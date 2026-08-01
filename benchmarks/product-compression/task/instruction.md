# Inspect one exact Frontier continuation

Work only in `/workspace/frontier`, an isolated checkout of the exact commit
named in `/opt/vela-input/fixture.json`.

{{TOOL_GUIDANCE}}

{{SCENARIO_INSTRUCTION}}

Reject any typo or unrelated Proposal. Report its Submission, every scoped
Verification and nonclaim, the exact conditional Standing change, and all three
current/accept/reject next obligations. This Frontier has no configured Target;
do not invent one.

Write exactly one JSON answer conforming to
`/opt/vela-input/answer.schema.json` at `/logs/artifacts/answer.json`. Do not
modify the checkout. Do not perform or simulate Accept, Reject, Cancel, signing,
publication, or any authority action. Verification is evidence, not acceptance.
