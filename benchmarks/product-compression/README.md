# Product-compression benchmark

This read-only benchmark asks one bounded question: does Vela help a cold
researcher find the exact next work and the exact pending scientific Decision
more reliably or efficiently than Git and files alone?

## Boundary

[Harbor 0.20.0](https://www.harborframework.com/docs/core-concepts) owns the
task format, containers, Codex execution and OAuth, retries, trajectories,
artifacts, verifier rewards, timing, cost, and raw results. Vela contributes
only:

- `materialize.py`: one exact Target and Decision-Inbox fixture;
- `answer.schema.json`: the participant output contract;
- `task/tests/verify.py`: an offline exact scorer; and
- `summarize.py`: the prospective two-arm comparison rule.

There is no Vela runner, session protocol, OAuth adapter, token accountant, or
parallel result model. Harbor's native `result.json` is the execution record.
The retained Vela summary points to its native result bytes and states only the
bounded conclusion used by the paper.

Harbor evidence never changes scientific Standing. Verification is not
acceptance, and the tasks receive no authority credentials.

## Study

The two matched arms inspect the same isolated Frontier commit:

- `git-files`: ordinary Git and file-reading tools;
- `vela-guided`: the same tools plus one exact read-only Vela binary.

There are two fresh Codex sessions per arm in counterbalanced order. All four
must be eligible. Vela receives bounded task-specific credit only when its arm
is exact twice, is at least as exact as the baseline, and has no median cost
regression. If both arms are exact twice, Vela must reduce median agent time by
at least 20 percent.

This is first-party evidence from one task. It does not establish independent
adoption, general scientific productivity, or scientific acceptance.

## Run

Keep generated Harbor jobs outside the repository:

```bash
export VELA_BENCH_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/vela/harbor/product-compression"
mkdir -p "$VELA_BENCH_CACHE"

python3 benchmarks/product-compression/materialize.py \
  --frontier /exact/frontier --vela /exact/vela \
  --proposal vpr_<id> --output "$VELA_BENCH_CACHE/materials"

python3 benchmarks/product-compression/prepare.py \
  --materials "$VELA_BENCH_CACHE/materials" \
  --frontier /exact/frontier \
  --vela-linux /exact/static-linux-vela \
  --model gpt-5.6-terra \
  --codex-version 0.145.0 \
  --vela-version 'vela 0.950.1' \
  --job-name vela-product-compression \
  --output "$VELA_BENCH_CACHE/study"

env -u OPENAI_API_KEY CODEX_AUTH_JSON_PATH="$HOME/.codex/auth.json" \
  harbor run \
    --config "$VELA_BENCH_CACHE/study/harbor-job.json" \
    --max-retries 0 \
    --jobs-dir "$VELA_BENCH_CACHE/runs"

python3 benchmarks/product-compression/summarize.py \
  --plan "$VELA_BENCH_CACHE/study/plan.json" \
  --job "$VELA_BENCH_CACHE/runs/vela-product-compression" \
  --output "$VELA_BENCH_CACHE/result.json"
```

Run focused contract tests with:

```bash
python3 -m unittest discover \
  -s benchmarks/product-compression -p 'test_*.py'
```

Compact, claim-limited results cited by the paper belong under
`paper/artifacts/`. Raw Harbor tasks, trajectories, and caches do not.

## History

`paper/artifacts/product-compression-v1` through `v5` preserve failed,
invalidated, or superseded study conclusions without remaining active harnesses.
The bounded passing v6 result remains unchanged.
