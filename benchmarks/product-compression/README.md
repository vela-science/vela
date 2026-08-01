# Product-compression benchmark

This read-only benchmark asks one bounded question across explicit scientific
scenarios: does Vela help a cold researcher recover the exact pending Decision,
its evidence limits, conditional Standing change, and next obligation more
reliably or efficiently than Git and files alone?

## Boundary

[Harbor 0.20.0](https://www.harborframework.com/docs/core-concepts) owns the
task format, containers, Codex execution and OAuth, retries, trajectories,
artifacts, verifier rewards, timing, cost, and raw results. Vela contributes
only:

- `materialize.py`: exact scenario qualification plus two ready-to-run local
  Harbor tasks;
- `answer.schema.json`: the participant output contract;
- `task/tests/verify.py`: an offline exact scorer; and
- `summarize.py`: the prospective two-arm comparison rule.

There is no Vela runner, session protocol, OAuth adapter, token accountant, or
parallel result model. Harbor's native `result.json` is the execution record.
The retained Vela summary points to its native result bytes and states only the
bounded conclusion used by the paper.

Harbor evidence never changes scientific Standing. Verification is not
acceptance. The task containers receive no authority credentials, no Git
remote, and no mutable canonical checkout; the verifier runs in Harbor's
post-agent phase with networking disabled, and its test script always
overwrites the reward. Safety is enforced by custody and phase policy rather
than claimed by the participant. A second verifier container is unnecessary
for this read-only comprehension benchmark.

## Study

The two matched arms inspect the same isolated Frontier commit:

- `git-files`: ordinary Git and file-reading tools;
- `vela-guided`: the same tools plus one exact read-only Vela binary.

Harbor runs two native attempts per task. All four
must be eligible. Vela receives bounded task-specific credit only when its arm
is exact twice, is at least as exact as the baseline, and has no median cost
regression. If both arms are exact twice, Vela must reduce median agent time by
at least 20 percent.

The materializer currently supports three explicit scenarios without a generic
correction framework:

- `formal-foreign-reference-continuation`: find a pending local Decision over
  an accepted foreign source reference without importing source authority;
- `quantum-certificate-supersession`: find a pending correction to one accepted
  quantum-code Claim, distinguish its two verifier scopes, and report the exact
  accept/reject branches.
- `erdos-post-decision-continuation`: recover one accepted bounded transition,
  separate later verified-but-pending producer completion, and identify the
  exact first non-overlapping Target without changing Standing.

These are first-party comprehension and continuation tasks. They do not
establish independent adoption, general scientific productivity, scientific
acceptance, post-correction remapping, or the full correction-and-inheritance
breakthrough benchmark.

## Run

Keep generated Harbor jobs outside the repository:

```bash
export VELA_BENCH_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/vela/harbor/product-compression"
mkdir -p "$VELA_BENCH_CACHE"

python3 benchmarks/product-compression/materialize.py \
  --frontier /exact/frontier \
  --vela /exact/vela \
  --proposal vpr_<id> \
  --scenario quantum-certificate-supersession \
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

Current compact results are
[`paper/artifacts/product-compression-v11`](../../paper/artifacts/product-compression-v11/README.md)
and
[`paper/artifacts/product-compression-erdos-post-decision-2026-08-01`](../../paper/artifacts/product-compression-erdos-post-decision-2026-08-01/README.md).
Their complete native Harbor studies and jobs are retained outside the source tree
under a SHA-256 manifest so generated execution state does not become product
code.

## History

The active tree keeps only the current pre-Decision correction and
post-Decision continuation results. Earlier failed, superseded, and invalidated
iterations remain available in Git history; they are not active harnesses,
compatibility targets, or runtime inputs.
