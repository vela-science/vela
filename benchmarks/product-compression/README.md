# Product-compression benchmark

This read-only study asks one bounded question: does Vela help a cold
researcher identify the exact next work and the exact pending scientific
Decision more reliably or efficiently than Git and files alone?

It cannot mutate a Frontier or perform a Decision.

## Boundary

[Harbor 0.20.0](https://www.harborframework.com/docs/core-concepts) owns tasks,
containers, agent execution, retries, trajectories, artifacts, verifier
rewards, timing, cost, and raw results. Vela adds only what Harbor cannot know:

- one exact Frontier fixture;
- one closed scientific answer contract;
- an offline semantic verifier; and
- one prospective comparison rule.

There is no Vela runner, session protocol, token accountant, OAuth adapter, or
second report framework. Harbor uses Codex's normal OAuth file. Its native
`result.json`, multi-metric rewards, and viewer remain the detailed execution
record. See Harbor's [task](https://www.harborframework.com/docs/tasks) and
[evaluation](https://www.harborframework.com/docs/run-jobs/run-evals)
contracts.

Harbor evidence never changes scientific Standing. Verification is not
acceptance, and no task receives authority credentials.

## What is measured

Each participant reports:

1. the exact Frontier and first ranked Target;
2. the selected Proposal, Submission, and Verification set;
3. the scoped Standing result if a human accepts or rejects; and
4. that inspection performed no authority action and changed no accepted state.

The v5 contract deliberately excludes:

- environment-specific shell paths;
- the root of unrelated Decision Inbox entries;
- global counts already implied by exact scoped Standing; and
- arbitrary token or tool-call pass/fail limits.

The materializer still checks source projection counts before emitting the
smaller participant contract. Exact Target, packet, Proposal, Submission,
Verification-set, selected inbox-entry, and conditional Standing roots remain
required.

## Study design

Four first-party sessions form two counterbalanced pairs:

- `git-files`: native repository tools;
- `vela-guided`: the same checkout plus the exact read-only Vela binary.

All sessions must be eligible and Vela-guided must be exact twice. A result
receives bounded task-specific credit when Vela-guided is more exact with no
median cost regression. If both arms are exact twice, Vela-guided must improve
median elapsed time by at least 20 percent with no median cost regression.
Otherwise the study reports no demonstrated lift.

Elapsed execution and retained file sizes are safety bounds. Tokens and tool
calls remain Harbor telemetry, not correctness gates. Four first-party sessions
cannot establish independent adoption or general scientific productivity.

## History

Earlier runs remain under `paper/artifacts/product-compression-*` as immutable
evidence, not active compatibility targets. In particular, v4 is classified as
an invalid study because its answer key graded a host-specific command and an
unrelated whole-inbox root. It is not evidence for or against product lift.

The active answer contract is v5. The current v6 plan keeps that contract and
uses a 15-minute execution safety limit after a five-minute pilot timed out by
roughly five seconds. Old generated tasks and custom-runner calibrations are not
part of the current implementation.

## Run locally

```bash
python3 -m unittest discover \
  -s benchmarks/product-compression -p 'test_*.py'

python3 benchmarks/product-compression/materialize.py \
  --frontier /exact/frontier --vela /exact/vela \
  --proposal vpr_<id> --output jobs/product-compression-v6/materials

python3 benchmarks/product-compression/study.py freeze-plan \
  --materials jobs/product-compression-v6/materials \
  --model gpt-5.6-terra --codex-version 0.145.0 \
  --vela-linux /exact/static-linux-vela --vela-version 'vela 0.950.1' \
  --output jobs/product-compression-v6/plan.json

python3 benchmarks/product-compression/study.py prepare-harbor \
  --plan jobs/product-compression-v6/plan.json \
  --materials jobs/product-compression-v6/materials \
  --frontier /exact/frontier --vela-linux /exact/static-linux-vela \
  --job-name vela-product-compression-v6-native \
  --output jobs/product-compression-v6/harbor-native

cd jobs/product-compression-v6/harbor-native
env -u OPENAI_API_KEY CODEX_AUTH_JSON_PATH="$HOME/.codex/auth.json" \
  harbor run --config harbor-job.json --max-retries 0 --jobs-dir ../runs

python3 ../../../../benchmarks/product-compression/study.py summarize-harbor \
  --plan ../plan.json --job ../runs/vela-product-compression-v6-native \
  --output ../result.json
```

`prepare-harbor` materializes standard Harbor tasks; it is not a runner or
adapter. Generated jobs remain ignored local evidence until deliberately
archived. Compact, claim-limited results belong under `paper/artifacts/`.
