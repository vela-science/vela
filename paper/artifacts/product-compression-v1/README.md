# Product-compression cold-use benchmark

This source-only harness compares two matched ways to understand and continue
one real Vela lifecycle:

1. **Git/files** — exact Frontier checkouts, ordinary repository files, `git`,
   `jq`, `rg`, SHA-256, and the retained verifier evidence; and
2. **Vela guided** — the identical inputs plus the exact release-compatible
   Vela binaries and their product commands.

The frozen task asks a fresh participant to:

1. choose the only canonical Frontier with a fresh ranked Target;
2. identify its exact first Target and bounded scope;
3. start one private Attempt in the Vela arm, or reconstruct the same scope
   manually in the Git/files arm;
4. explain the Attempt budget, consequence ceiling, and lack of authority;
5. inspect the retained Agent Run reference, artifact, verifier, and replay
   boundary;
6. locate the exact Proposal in the Decision Inbox;
7. explain the proposed Standing diff and why protocol readiness is not a
   recommendation to accept; and
8. identify the exact next obligation from the accepted Erdős 424 correction.

The task uses two exact Erdős fixtures:

- the current four-Frontier heads for Target selection and the live
  producer-to-review path; and
- historical Erdős commit
  `c25e11d332cfbc12b048c314880662d507df53e0`, already frozen by the
  prior state-lift study, for the terminal correction.

The historical fixture uses its release-compatible Vela `0.940.9` binary.
The current Vela binary intentionally refuses that predecessor epoch. This
version pin is part of the exact fixture rather than an invisible repair.

## What is frozen

`plan.v1.json` binds:

- all four current Frontier commits, trees, and repository roots;
- the current and historical Vela binaries;
- exact Target, bundle, Submission, Verification, Proposal, and manifest
  bytes;
- the prior terminal-correction task and answer-key bytes;
- the matched arm order, budgets, allowed tools, stop rules, and publication
  policy; and
- the answer schema, answer key, validators, scorers, and tests.

No participant output existed when the plan was frozen. Amendments must retain
the prior plan root and invalidate product-lift credit if they change a task
fact, expected answer, arm, budget, scoring rule, or success threshold.

The answer key is supervisor-only. Participant workspaces receive exact
Frontier clones, the arm-specific tools, the task prompt, and
`answer.schema.json`; they do not receive this directory or any parent path.

## Validation

Validate the frozen sources before opening a participant session:

```bash
python3 paper/artifacts/product-compression-v1/validate.py \
  --plan paper/artifacts/product-compression-v1/plan.v1.json \
  --vela-repository . \
  --current-vela target/debug/vela \
  --historical-vela ~/.canopus/bin/vela-0.940.9-4813da26 \
  --erdos-frontier ~/personal/erdos-frontier \
  --formal-frontier ~/personal/formal-conjectures-frontier \
  --quantum-frontier ~/personal/quantum-codes-frontier \
  --sidon-frontier ~/personal/sidon-frontier
```

Score one retained session:

```bash
python3 paper/artifacts/product-compression-v1/score.py \
  --plan paper/artifacts/product-compression-v1/plan.v1.json \
  --answer-key paper/artifacts/product-compression-v1/answer-key.v1.json \
  --answer <session>/answer.v1.json \
  --output <session>/score.v1.json
```

Aggregate the exact registered assignment only after all eight sessions exist:

```bash
python3 paper/artifacts/product-compression-v1/report.py \
  --plan paper/artifacts/product-compression-v1/plan.v1.json \
  --scores <score-1.json> ... <score-8.json> \
  --output result.v1.json
```

Focused tests:

```bash
python3 -m unittest discover \
  -s paper/artifacts/product-compression-v1 \
  -p 'test_*.py'
```

## Interpretation

One answer passes at 95/100 with no hard authority, Target, replay, Inbox, or
correction error. The product demonstrates first-party compression only if
both arms meet the registered method floor and the Vela-guided arm preserves
correctness while reducing median elapsed time by at least 20 percent.

First-party fresh sessions do not count as external users, independent
participants, adoption, or protocol-breakthrough evidence. Verification
passage, protocol readiness, and graph position do not recommend acceptance.
The study performs no Decision and changes no scientific Standing.
