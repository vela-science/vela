# Canopus and Vela evaluation

Canopus is useful only if it turns a fixed model and task budget into more
verified, repairable, and reviewable work than a simpler runner. Vela is useful
only if its state and authority contracts make review, reuse, correction, and
handoff safer or cheaper than ordinary files, Git, and exact-lock baselines.

Evaluation therefore tests four claims separately:

1. **Canopus execution lift:** with the same model, task, information, tools,
   verifier, and budget, Canopus improves independently verified completion or
   reduces invalid artifacts.
2. **Vela state lift:** with fixed work products, Vela improves premise
   selection, replay, correction, review, or authority integrity.
3. **Combined-system efficiency:** Canopus and Vela increase correct scientific
   dispositions or reusable verified progress per all-in cost and expert-hour.
4. **Adoption:** a cold user can reproduce, interpret, and continue real work
   without maintainer coaching or replacing their existing workbench.

None of these claims is implied by the others. A verifier pass is not
scientific acceptance. Internal replay is not independent reproduction. One
useful frontier result is not evidence of a repeatable discovery engine.

## Release gates

Every report places these gates above aggregate outcomes:

- zero unauthorized accepted-state transitions;
- zero human, repository, or provider credential leaks;
- zero verifier-to-acceptance conflation;
- zero undetected registered-history tampering;
- zero hidden exclusion of registered targets or failed runs;
- zero unreported benchmark-answer leakage;
- exact replay of every reported supported case.

A gate failure disqualifies the run as release evidence. It cannot be averaged
away by task completion, model quality, or cost.

## Comparison contract

The smallest useful comparison includes:

- a native model or benchmark-native baseline;
- a native baseline with the same verifier and structured task facts;
- a Canopus arm;
- a Canopus plus Vela arm only when the task tests state, governance, or reuse.

The model, task bytes, information, tools, verifier, evaluator, budget, retry
rule, stopping rule, and primary outcome are frozen before usable model output.
Both matched-capability and best-product comparisons may be reported, but must
not be mixed.

Registrations bind exact tasks, versions, roots, arms, assignments, seeds,
plan-driven matrices, answer access, budgets, scorers, audits, hard gates,
exclusions, and publication rules. New execution uses
`canopus.evaluation-plan.v2`; retained `v1` plans remain replay evidence.
Amendments after usable output are visible corrections, never silent edits.

Every `v2` stage declares whether it measures held-out confirmatory generation,
reproduction, or scorer calibration. Publicly visible answers are forbidden
from confirmatory generation. Reproducing them can validate plumbing or
scorers, but cannot establish model or harness lift.

## Outcomes and cost

Reports keep these layers separate:

- attempt;
- artifact produced;
- independent verifier result;
- correct Vela route or scientific disposition;
- independent reproduction;
- downstream reuse;
- external recognition.

Primary metrics depend on the registered claim. Common measures are verified
completion, false scientific admission, epistemic rescue, intervention harm,
model-swap resilience, correction localization, duplicate-work rate, and
correct dispositions per expert-hour.

All-in cost includes model tokens or price, compute, setup, repair, verifier
time, human intervention, expert review, and operational overhead. A `10x`
claim requires a matched denominator and quality threshold, a preregistered
study, a held-out replication, and no hard-gate failure.

## Benchmark order

Use established suites before inventing Vela-specific tasks:

1. Harness-Bench and a compact Harbor Index subset for harness comparison and
   regression;
2. AstaBench, verified ScienceAgentBench, CORE-Bench, or
   SocSci-Repro-Bench for scientific execution and honest non-reproduction;
3. Corral for evidence uptake and response to refutation;
4. Continual Learning Bench for accepted-state inheritance and stale-state
   harm;
5. Formal Conjectures and exact frontier tasks for statement fidelity,
   correction, and field evidence.

Vela-specific suites are limited to invariants external benchmarks do not
measure: authority, accepted-state inheritance, correction, canonicalization,
multi-agent coordination, reviewer leverage, and cold adoption. They form a
registry of focused suites, not one blended leaderboard score.

## Immediate evidence loop

Before expanding the adapter surface:

1. close or explicitly stop every existing registered Canopus experiment;
2. preserve the July 26 Erdős rejected candidate and verifier-bound repair as
   one canonical regression case;
3. add a matched native Codex arm;
4. generalize registrations only enough to support multiple tasks, arms,
   seeds, and randomized blocks;
5. retain benchmark-native scoring unchanged;
6. add one InspectAI bridge and one Harbor or METR Task Standard bridge;
7. publish failures, exclusions, roots, costs, and raw reproducible records.

The existing `inheritance-001` ceiling tie is evidence against adding more
agent-state machinery. The composition smoke supports only a narrow
representation-compression claim. Neither result justifies a protocol object.

## Simplification rule

Evaluation support remains a removable Canopus layer. Raw records stay
immutable and content-addressed; generated reports and web projections are
disposable. Evaluation data does not enter Vela's canonical scientific log,
and no benchmark database becomes a source of truth.

For each Canopus or Vela subsystem, run a preregistered ablation when its value
is uncertain. If a simpler structured baseline matches quality and safety with
less friction, remove or narrow the subsystem. In particular:

- reduce Canopus to a thin reproducible producer wrapper if its mission
  machinery does not improve verified output;
- preserve only Vela's smallest universal invariants if Git, DSSE, exact locks,
  and a written reducer match its correction and authority behavior;
- stop discovery-engine claims if fixed-denominator recognized yield does not
  exceed the baseline;
- change the adoption wedge if cold users require live maintainer
  interpretation.

The north-star question is:

> Does the system produce more genuine, reusable science per scarce human
> judgment?

This document is an evaluation-layer contract. It introduces no Vela protocol
object, authority path, workflow language, or canonical result store.
