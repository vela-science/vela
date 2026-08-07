# Historical Git-versus-Vela state-lift pilot

This directory retains compact evidence from a retired first-party pilot. It
compared Git plus the exact repository and evidence with the same inputs plus
Vela's read-only CLI. It is not an active benchmark harness.

## Result

The first matched pair is a registered negative result at
`sha256:af9af17824e15b14ea77aa2e9afec135b997cdcf026beb050b80cc51563e753a`.
The Git arm answered 22 of 25 exact fields in 268.040 seconds using 2,401,939
observed tokens. The Vela arm answered 24 of 25 in 146.425 seconds using
655,122 observed tokens. Both exceeded the registered 50,000-token limit and
neither was fully correct, so the remaining six sessions were not run.

This result establishes neither state lift, human reviewer efficiency,
external adoption, nor a protocol breakthrough. It showed only a directional
advantage in one invalidated first-party pair and motivated a smaller task.

## Retained evidence

The active tree keeps only `result.v2.json`. It binds the frozen protocol and
task roots, both completed session records, answers, scores, event streams, the
stop condition, and every unrun session. The complete historical harness and
evidence remain recoverable from Git at
`4ea5ebe89a1f6881a54a8d308dde3ebeecb35621`.

The scorer, schemas, amendments, task materialization, per-session files, and
stderr were removed because this pilot is closed. Keeping an executable
pre-Harbor harness beside the current benchmark would create two evaluation
systems without improving reproducibility or any scientific-state invariant.

## Current evaluation path

Prospective execution evaluation uses
[`benchmarks/product-compression`](https://github.com/vela-science/vela/blob/e68590415a0cc40ee489801f1f281dc8c5996337/benchmarks/product-compression/README.md)
and native Harbor. That directory was retired from the active tree in the same
way this pilot's harness was, and remains recoverable from Git at
`e68590415a0cc40ee489801f1f281dc8c5996337`, which is where the link reads it.
Harbor owns task execution, isolated trials, retries, trajectories, timing,
cost, and raw results. Vela contributes only the bounded fixture and exact
scorer. Harbor output is evidence; it cannot change scientific Standing or
perform a human Decision.
