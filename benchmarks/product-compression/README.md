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

The retained v9 studies ran two native attempts per arm. All four trials had
to be eligible, and the compact result applied a bounded task-specific rule.
Those studies remain valid for their stated historical claims.

The next action-complete campaign uses two attempts per arm only as an
instrumentation pilot. It may confirm that task materialization, custody,
scoring, and telemetry work, but it may not support a general performance
claim. Confirmatory repetitions are computed from blinded pilot variance for
80 percent power, a two-sided 5 percent error rate, and a preregistered 20
percent minimum useful effect. Cost and observed tokens are measured outcomes,
not arbitrary campaign budgets.

The confirmation unit is a distinct frozen scientific task, not a repeated
model call on one fixture. The primary endpoint is target-blocked time to an
exact, authority-correct continuation, analyzed on the log scale. An inexact
answer receives the registered 900-second restricted time rather than being
dropped, so fast wrong answers cannot win. Cost to an exact answer is
secondary. All eligible failures remain in the denominator. A general positive
result requires at least two scientific task families, randomized arm order
within each frozen target block, a producer or model-family swap, a point
estimate above the registered 20 percent useful-effect threshold, and a
two-sided 95 percent interval excluding no lift. No secondary metric can rescue
a failed exactness or authority gate. The confirmatory plan and result use a
new contract; historical v11/v6 pilot evidence is never reinterpreted.

### Public scorecard

The public result leads with metrics that are recognizable outside Vela:

1. **Authority-correct exact pass@1**: exact answers divided by all eligible
   attempted frozen tasks, with the paired arm difference and 95 percent
   interval. Authority errors are a separate zero-tolerance hard gate.
2. **Restricted time-to-exact**: an inexact or timed-out task receives the
   registered 900-second limit. Report the paired geometric-mean time ratio and
   a task-blocked 95 percent interval. Never condition the headline on success.
3. **All-in cost per attempted task**: report total and median dollars, observed
   tokens, and cost per exact pass without hiding failed attempts.
4. **Real correction and inheritance**: report affected-set precision and
   recall, surviving-route recall, false pruning, exact repair obligation, and
   cold-successor next-obligation pass@1 for each qualifying real correction.

ETY, VPAC, FIE, and CPI remain useful mechanistic labels in the evidence
companion. They are not the headline vocabulary. A synthetic correction fixture
is a negative control and conformance case; it cannot earn product, scientific,
or protocol-breakthrough credit. A result from these Harbor-native custom tasks
must not be called a Terminal-Bench score or leaderboard result.

New studies use the v10 answer, v8 fixture, v12 plan, and v7 compact-result
contracts. Every two-attempt study is explicitly marked `claim_credit: false`;
it can validate instrumentation or reveal a failure, but cannot earn a general
performance claim.

The materializer currently supports four explicit scenarios without a generic
correction framework:

- `formal-foreign-reference-continuation`: find a pending local Decision over
  an accepted foreign source reference without importing source authority;
- `quantum-certificate-supersession`: find a pending correction to one accepted
  quantum-code Claim, distinguish its two verifier scopes, and report the exact
  accept/reject branches.
- `erdos-post-decision-continuation`: recover one accepted bounded transition
  and identify the exact first non-overlapping Target produced by the current
  post-Decision remap without changing Standing.
- `explicit-target-absence`: distinguish the missing canonical root
  `targets.json` from nested historical target evidence and return no work
  instead of inventing a scientific objective.

These are first-party comprehension and continuation tasks. They do not
establish independent adoption, general scientific productivity, scientific
acceptance, post-correction remapping, or the full correction-and-inheritance
breakthrough benchmark.

## Next preregistered tranche

After the live Erdős human Decision, the same Harbor boundary will cover five
explicit task classes:

1. `target_continuation` — find the first valid nonduplicate action after a
   rooted Decision and remap;
2. `standing_discrimination` — distinguish Submission, passing Verification,
   pending Proposal, Decision, and current Standing;
3. `cross_frontier_inheritance` — inspect a bounded foreign package without
   importing origin authority;
4. `controlled_correction_impact` — recover affected, surviving, and blocked
   relations from a closed-ground-truth correction fixture; and
5. `explicit_target_absence` — return the exact blocker instead of inventing
   work.

The first three retained scenarios may supply task material where their exact
roots still qualify, but no generic correction engine or parallel Vela harness
will be added. A real correction-and-inheritance claim remains separately
gated on a qualifying canonical correction with closed downstream ground
truth. The controlled fixture cannot satisfy that gate by itself.

The benchmark schema names are repository-local rooted task labels, not public
protocol compatibility promises. Each retained plan and generated task binds
the exact schema bytes it used.

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

Before starting a new action-complete campaign, freeze the exact current
read-only baseline. Download the deployed Observatory manifest once, then bind
it to the clean Vela and Frontier checkouts:

```bash
curl -fsSL https://app.vela.space/.well-known/vela-site.json \
  -o /tmp/vela-site.json

python3 benchmarks/product-compression/freeze_campaign.py \
  --vela-repository "$PWD" \
  --vela "$PWD/target/release/vela" \
  --frontier erdos="$HOME/personal/erdos-frontier" \
  --frontier formal-conjectures="$HOME/personal/formal-conjectures-frontier" \
  --frontier quantum-codes="$HOME/personal/quantum-codes-frontier" \
  --frontier sidon-sets="$HOME/personal/sidon-frontier" \
  --observatory-manifest /tmp/vela-site.json \
  --observed-at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --output /tmp/vela-action-complete-baseline.json
```

The freezer makes no scientific write. It fails if any checkout is dirty, a
Frontier does not replay strictly, the live projection does not bind the exact
four heads, Erdős does not expose its one expected next range, or another
Frontier invents work instead of returning its explicit blocker. It also binds
the Harbor-native custody, pilot, power, metric, and implementation contracts.
Model, agent, and task assignments remain part of the later frozen Harbor plan
rather than this source-state baseline.

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
[`paper/artifacts/product-compression-v11`](../../paper/artifacts/product-compression-v11/README.md),
[`paper/artifacts/product-compression-erdos-post-decision-2026-08-01`](../../paper/artifacts/product-compression-erdos-post-decision-2026-08-01/README.md),
and
[`paper/artifacts/product-compression-formal-foreign-reference-2026-08-01`](../../paper/artifacts/product-compression-formal-foreign-reference-2026-08-01/README.md).
The current action-complete instrumentation pilot is retained at
[`paper/artifacts/product-compression-erdos-action-complete-2026-08-03`](../../paper/artifacts/product-compression-erdos-action-complete-2026-08-03/README.md).
The failed Astra source-fidelity gate is retained separately at
[`paper/artifacts/product-compression-astra-fidelity-2026-08-01`](../../paper/artifacts/product-compression-astra-fidelity-2026-08-01/README.md).
Their complete native Harbor studies and jobs are retained outside the source tree
under a SHA-256 manifest so generated execution state does not become product
code.

## History

The active harness keeps only scenarios that remain useful. Failed,
superseded, and invalidated task implementations do not remain as runtime
surface merely to reproduce old runs; their frozen inputs, outputs, and
claim-limited compact results remain available as evidence.
