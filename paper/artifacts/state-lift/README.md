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

The active tree keeps only the compact evidence needed to inspect that result:

- the frozen protocol, execution records, and amendments;
- the terminal task instance, answer key, and amendment;
- the structured answer schema and dependency-free exact-field scorer;
- each completed session's answer, record, score, and stderr; and
- `result.v2.json`, which records the stop condition and unrun sessions.

The protocol chain also records the two pre-output schema failures and the
pre-output materialization repair. Raw Codex JSONL streams, the custom runner,
materializer, report generator, schema validator, failed-attempt directories,
and their tests were removed from the active tree. They are historical
execution machinery, not Vela protocol or scientific evidence, and remain
recoverable from Git history at commit
`4ea5ebe89a1f6881a54a8d308dde3ebeecb35621`.

The retained scorer can still recheck either structured answer:

```bash
python3 -m unittest paper/artifacts/state-lift/test_score.py

python3 paper/artifacts/state-lift/score.py \
  --answer-key paper/artifacts/state-lift/terminal/answer-key.v1.json \
  --answer paper/artifacts/state-lift/sessions/git-v2-01/answer.v1.json
```

## Current evaluation path

Prospective execution evaluation uses
[`benchmarks/product-compression`](../../../benchmarks/product-compression/README.md)
and native Harbor. Harbor owns task execution, isolated trials, retries,
trajectories, timing, cost, and raw results. Vela contributes only the bounded
fixture and exact scorer. Harbor output is evidence; it cannot change
scientific Standing or perform a human Decision.
