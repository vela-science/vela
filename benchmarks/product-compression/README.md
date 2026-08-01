# Product-compression benchmark

This read-only benchmark asks one bounded question: does Vela help a cold
researcher find the exact next work and the exact pending scientific Decision
more reliably or efficiently than Git and files alone?

## Boundary

[Harbor 0.20.0](https://www.harborframework.com/docs/core-concepts) owns the
task format, containers, Codex execution and OAuth, retries, trajectories,
artifacts, verifier rewards, timing, cost, and raw results. Vela contributes
only:

- `materialize.py`: one exact Target and Decision-Inbox fixture plus two
  ready-to-run local Harbor tasks;
- `answer.schema.json`: the participant output contract;
- `task/tests/verify.py`: an offline exact scorer; and
- `summarize.py`: the prospective two-arm comparison rule.

There is no Vela runner, session protocol, OAuth adapter, token accountant, or
parallel result model. Harbor's native `result.json` is the execution record.
The retained Vela summary points to its native result bytes and states only the
bounded conclusion used by the paper.

Harbor evidence never changes scientific Standing. Verification is not
acceptance. The task containers receive no authority credentials, no Git
remote, and no mutable canonical checkout; the verifier runs in a separate
no-network container. Safety is enforced by custody rather than claimed by the
participant.

## Study

The two matched arms inspect the same isolated Frontier commit:

- `git-files`: ordinary Git and file-reading tools;
- `vela-guided`: the same tools plus one exact read-only Vela binary.

Harbor runs two native attempts per task. All four
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
  --frontier /exact/frontier \
  --vela /exact/vela \
  --proposal vpr_<id> \
  --vela-linux /exact/static-linux-vela \
  --model gpt-5.6-terra \
  --codex-version 0.145.0 \
  --job-name vela-product-compression \
  --output "$VELA_BENCH_CACHE/study"

(
  cd "$VELA_BENCH_CACHE/study"
  env -u OPENAI_API_KEY CODEX_AUTH_JSON_PATH="$HOME/.codex/auth.json" \
    harbor run \
      --config harbor-job.json \
      --max-retries 0 \
      --jobs-dir "$VELA_BENCH_CACHE/runs"
)

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
`paper/artifacts/`. A reproducible result retains the frozen plan, fixture,
answer key, Harbor job config/result, each trial config/result, participant
answer, verifier output/reward, compact summary, and a SHA-256 manifest.
Trajectories, session logs, recordings, credentials, Docker caches, and other
generated execution state do not belong in Git.

The current compact result is
[`paper/artifacts/product-compression-v9`](../../paper/artifacts/product-compression-v9/README.md).
Its complete native Harbor study and job are retained outside the source tree
under a SHA-256 manifest so generated execution state does not become product
code.

## History

The active tree keeps one historical v6 summary because it motivated the clean
rerun. Earlier failed and invalidated iterations remain available in Git
history; they are not active harnesses, compatibility targets, or runtime
inputs.
