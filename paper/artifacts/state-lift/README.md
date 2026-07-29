# Matched Git-versus-Vela state-lift pilot

This frozen protocol compares correction comprehension with:

1. Git plus the exact repository, evidence, verifier, and ordinary
   documentation; and
2. the identical inputs plus Vela's read-only CLI.

The study does not begin until the Erdős 424 Proposal has a terminal human
Decision and an amendment binds the exact answer key, model, runtime, binary,
repositories, and scorer bytes. Eight first-party fresh sessions can qualify
the study method, but cannot earn external-participant or protocol-breakthrough
credit.

Preregistration amendment
`preregistration-amendment-001.v1.json` replaces a prepared Verification that
failed cross-clone replay before import. It changes no task question, scorer,
outcome rule, scope limit, or model budget, and it predates any state-lift
model output. The terminal materializer requires its exact root.

`score.py` is the dependency-free preregistered scorer. It accepts one exact
`vela.state-lift-answer-key.v1` and one `vela.state-lift-answer.v1`, rejects
schema or task-instance drift, compares every factual field, and reports
authority errors separately from ordinary wrong answers. The terminal
task-instance amendment will bind its SHA-256 before any model output exists.

`materialize.py` is the terminal gate. Its accepted and rejected outcome rules
were frozen while the Proposal was still pending. It refuses a missing or
nonterminal Decision, the wrong Proposal, Claim, Submission, Registration,
Verification, Artifact, source transition, repository root, or dirty input
repository. A successful invocation writes:

- `task-instance.v1.json`, which binds the terminal Frontier, correction,
  toolchain, model, isolation, and arm contracts;
- `answer-key.v1.json`, which binds the exact expected facts to that task
  instance; and
- `amendment.v1.json`, which root-links both documents to the preregistered
  protocol.

After an accepted correction, `vela why` no longer resolves the superseded
predecessor because it is not a current Claim. The materializer therefore
validates that predecessor directly from its exact retained
content-addressed record and derives `superseded` only from the terminal
accepted Decision. The first post-Decision materialization attempt exposed
this harness defect before any model output; its failure is retained in
`materialization-attempt-001.v1.json`.

For an accepted correction, the next action is
`inspect_dependents_and_repair_or_revalidate`. For a rejected correction, it
is `preserve_predecessor_and_prepare_new_bounded_revision`. Both outcomes
retain the same three scope limits: the result is not a proof of Erdős 424,
does not establish a unique informal interpretation, and does not turn
Verification into acceptance.

`execution.v1.json` freezes the eight-session arm order, session IDs,
structured-answer schema, runner bytes, and custody rules before the first
model output. `run_session.py` creates a fresh exact clone, removes its remote,
makes it read-only, strips the model-tool environment, and retains the raw
JSONL event stream, final answer, stderr, timing, and post-run cleanliness for
one session. The Git arm cannot use Vela. The Vela arm receives one copied
binary whose root must match the task instance.

```bash
python3 -m unittest paper/artifacts/state-lift/test_score.py
python3 -m unittest paper/artifacts/state-lift/test_materialize.py

python3 paper/artifacts/state-lift/score.py \
  --answer-key <terminal-answer-key.json> \
  --answer <session-answer.json>

python3 paper/artifacts/state-lift/materialize.py \
  --frontier <terminal-erdos-frontier> \
  --source-repository <formal-conjectures> \
  --vela <exact-vela-binary> \
  --runtime-binary <exact-runtime-binary> \
  --runtime-name codex \
  --runtime-version '<exact-version>' \
  --model-id '<exact-model-id>' \
  --frozen-at '<RFC3339>' \
  --output <empty-output-directory>
```

The scorer does not interpret prose or judge scientific merit. Session
supervision records time, tokens, commands, mutation attempts, credential
access, and interventions outside the answer document.
