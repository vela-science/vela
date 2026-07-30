# Source-only evaluation

This directory implements Canopus ADR 0011. It is not included in the npm
package and creates no Vela state.

`canopus.evaluation-plan.v2` registrations freeze exact source, task packet,
verifier, executable, dependency lock, environment, arm, plan-driven matrix,
answer access, budget, scorer, custody, and publication bytes before usable
model output. Retained `v1` plans remain parseable and reportable, but the
runner will not start new `v1` assignments.
Canopus `0.8.0`, the first Erdős producer loop, and the repaired registered
Stage A are complete. The retained plan root is
`sha256:31268241f0f1ada92fd78d245643ad9274308a74d617a965e8e2bcb46195fd47`.
Canopus passed four of four matched cells, ordinary native Codex passed three
of four, and same-packet native Codex passed four of four. Stage B was not run:
transparent diagnostics and repair consumed the registered campaign's
remaining call budget. No framework integration is supported and no Canopus
`0.9.0` release is justified.

A future live registration is a new experiment, not unfinished Stage A. It
may be created only after every task, arm, repetition, matrix purpose, source
snapshot, packet, verifier, executable, scorer, dependency lock, environment
manifest, budget, and stopping rule is frozen and rehashed by
`eval:validate`. Each arm also binds its trusted wrapper as a separate exact
file. The wrapper is inserted only through the `{wrapper}` argv placeholder,
so pinning the runtime executable cannot accidentally leave a mutable script
outside the registration.

```bash
bun run eval:validate
bun run eval:run -- --plan <registration.json> --stage A --output <new-dir>
bun run eval:report -- <run-dir>
bun run trace:export -- --input <run.json> --output <trace.json> \
  --format otlp-json --content none
```

The runner records process evidence only. Task-specific verifier results,
scientific disposition, cost, and expert-minute scoring remain explicit
registered scorer outputs. A passing process is not a Verification Record or
Decision.

Every trusted arm wrapper must write one
`canopus.evaluation-arm-result.v1` record to file descriptor 3 after its model
process exits. The wrapper must not inherit that descriptor into the model
sandbox. Standard output and the writable candidate workspace are evidence,
not control channels. The evaluation runner validates the control record and
then persists its exact bytes as `arm-result.json`; a model-created file at
that path fails closed. The result binds the assignment and reports
provider-observed input, cached-input, output, and reasoning-output tokens.
The registered `observed_tokens` budget uses input plus output tokens,
matching the released Canopus budget contract. Missing, malformed, mismatched,
per-task over-budget, or aggregate over-budget usage is a hard stop; the
failed Run remains rooted and later registered cells are reported as unrun.

Each task also binds one safe relative artifact path, byte ceiling, verifier
file, verifier runtime, and closed argument vector. After the arm exits, the
supervisor reads the artifact as a bounded non-linked regular file and invokes
the verifier with no Codex home or provider credential. Process completion,
artifact identity, and verifier passage remain separate fields in the rooted
Run.

Each `v2` plan declares the exact Cartesian matrix for every registered stage:
task IDs, arm IDs, repetitions, and whether the stage is confirmatory
generation, reproduction, or scorer calibration. Validation rejects missing
or extra cells. Confirmatory generation requires at least two matched arms,
two repetitions, and tasks declared held out before any output. A task whose
answer is already public may be reproduced or used to calibrate a scorer, but
it cannot earn generation-lift credit.

The next confirmatory execution study, if registered, uses three new held-out
task shapes—Erdős, Formal, and quantum—with the same exact packet and verifier
available to native Codex and Canopus, two repetitions per task and arm, and
12 assignments. The already-visible current Formal and quantum answers are
reproduction evidence only. Execution, state, and inheritance lift remain
bound to distinct scorer roots and reported separately.

Aggregate time and token ceilings must cover every assignment ceiling.
Process failures remain retained Runs; the runner does not stop merely because
one arm exits nonzero.

The runner uses the exact executable resolved during validation. It writes an
exact rooted plan snapshot beside the run set and writes an explicit stopped
run set when a hard runtime bound prevents remaining cells.
Reporting refuses missing indexes, mixed plan roots, unregistered Runs, and
Run bytes that do not match the rooted run set. Reports carry the matrix
purpose only after matching it to the retained rooted plan; a mutable run-set
label cannot create confirmatory eligibility. Only held-out
confirmatory-generation run sets are eligible for confirmatory interpretation.
