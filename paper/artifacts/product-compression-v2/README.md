# Product-compression v2 source scaffold

This is an **unfrozen, source-only** study contract. It contains no fixture,
answer key, plan, participant session, model runner, or live identity. It cannot
mutate a Frontier or perform a Decision. The stopped v1 study remains unchanged
in `paper/artifacts/product-compression-v1`.

## Question

Does the shipped Vela product surface let a cold user inspect one real bounded
Agent campaign and one pending Decision materially faster than Git and files
alone, without losing exact provenance or confusing Verification with
acceptance?

The future fixture is deliberately narrow: one completed Erdős Attempt, two
root-linked Runs, one registered Submission with a Verification and pending
Proposal, one retained corroborating Run, and the exact successor Target. Run
order and registration order are independent. A participant reports:

1. the current Target and exact Attempt boundary;
2. the two Runs and receipt chain, the registered Run, and why the other Run
   must not be exported as duplicate work;
3. the Proposal's readiness and conditional Standing change; and
4. whether inspection exercised authority or changed accepted Standing; and
5. the exact successor Target that can begin under a new bounded Attempt.

`answer.schema.json` is the only participant-facing output contract. It is
recursively closed. `harness.py` uses explicit validators rather than a partial
JSON-Schema interpreter; the schema is an interoperability aid, not a silent
second implementation.

## Freeze gate

No session may run until independently reconstructible source tooling emits:

- an exact fixture root from named clean Git inputs and a source-compatible Vela
  binary;
- a supervisor-only answer key whose root is recomputed over the exact expected
  answer;
- a plan whose root binds that fixture and answer-key root, one exact model and
  configuration root, the exact future supervisor-runner root, distinct inline
  tool contracts for both arms, one shared budget, AB/BA assignment, and
  fail-inclusive first-party publication policy;
  and
- an isolated participant workspace that cannot read the answer key or invoke a
  mutation, authority, publication, or agent-execution command.

`materialize.py` closes the first two gates. It accepts one clean Frontier, one
exact Vela binary, the completed private Attempt record, and an explicit pending
Proposal ID. It invokes only `status`, `next`, and `review inbox`, verifies that
Git and repository state do not change, checks the complete receipt chain and
content-addressed Run, evidence, Submission, Verification, and successor packet
bytes, then writes a public `fixture.json` and supervisor-only
`answer-key.json`. Absolute private Run paths are never copied into the fixture.
The future runner must keep the answer key outside participant custody.

There is intentionally no plan example: placeholder roots would look frozen
without binding real material. `validate_plan` defines the closed plan contract,
and synthetic values exist only in unit tests.

Plan validation proves only that these declarations are closed and rooted. It
does **not** prove process isolation, read-only tool behavior, or live budget
enforcement. Those are pre-session conformance gates for the future rooted
supervisor runner; this scaffold intentionally does not implement that runner.

The two arms receive the same model configuration and budgets. The Git/files arm
receives only its frozen read-only argv prefixes. The Vela-guided arm receives
its own frozen read-only prefixes (`status`, `next`, `review show`, `show`,
`agent show`, and `check` when included by the real plan). Every executed argv
must match its arm's rooted contract. Shell wrappers are not implicitly allowed.

## Measurement and pass rule

The completed supervisor envelope is explicitly validated in code; there is no session
schema or participant-controlled metric summary. Command count and output bytes
come from the command log. Effective tokens are derived as:

```text
input_tokens - cached_input_tokens + output_tokens
```

This scorer detects retained budget violations after execution; it does not stop
a live process. The future bound runner must enforce time, token, command, and
output limits while the session is running. Semantic intervention count must be
zero. Plan, session, answer-key, score, and report
roots are recomputed with their own root field omitted. Any tampering, state
drift, forbidden argv, wrong plan binding, missing answer, or budget breach is a
failure. The report accepts exact sessions and recomputes every score against the
plan and answer key; it never trusts caller-supplied scores. A score or report
with any failure code cannot claim `passed: true`.

The four source-owned sessions are counterbalanced across two AB/BA pairs. A
prospective result passes only if all exact answers pass, Vela-guided is faster
in both pairs, median elapsed time improves by at least 20%, and median commands
and effective tokens do not regress. This first-party pilot cannot establish
independent adoption or external-user value.

## Local verification

```bash
python3 -m unittest discover \
  -s paper/artifacts/product-compression-v2 -p 'test_*.py'

python3 paper/artifacts/product-compression-v2/harness.py validate \
  --kind answer --input answer.json

python3 paper/artifacts/product-compression-v2/harness.py score \
  --plan plan.json --answer-key answer-key.json \
  --session session.json --output score.json

python3 paper/artifacts/product-compression-v2/harness.py report \
  --plan plan.json --answer-key answer-key.json \
  --sessions session-*.json --output result.json

python3 paper/artifacts/product-compression-v2/materialize.py \
  --frontier /exact/frontier --vela /exact/vela \
  --attempt /exact/private/attempt.json --proposal vpr_<id> \
  --output /private/study-material
```

These checks are local and do not depend on hosted CI.
