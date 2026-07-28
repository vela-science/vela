# Source-only evaluation

This directory implements Canopus ADR 0011. It is not included in the npm
package and creates no Vela state.

`canopus.evaluation-plan.v1` registrations freeze exact source, task packet,
verifier, executable, dependency lock, environment, arm, budget, scorer,
custody, and publication bytes before usable model output.
Canopus `0.8.0` and the first Erdős producer loop have passed. The next live
registration is created only after both Stage A tasks, all three matched arms,
and every source snapshot, packet, verifier, executable, scorer, dependency
lock, and environment manifest are frozen and rehashed by `eval:validate`.
Each arm also binds its trusted wrapper as a separate exact file. The wrapper
is inserted only through the `{wrapper}` argv placeholder, so pinning the
runtime executable cannot accidentally leave a mutable script outside the
registration.

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

Stage A must retain both native controls: ordinary native Codex and native
Codex with the exact packet and frozen verifier used by Canopus. Execution,
state, and inheritance lift are bound to three distinct scorer roots and
reported separately. Matching the native same-packet arm is a deletion signal
for Canopus complexity, not permission to invent another layer.

A registered Stage A plan contains exactly one math task, one
scientific-computing task, the three matched arms, two repetitions of every
task/arm pair, and 12 assignments. Its total time and token ceilings must cover
the sum of all assignment ceilings. Process failures remain retained Runs; the
runner does not stop merely because one arm exits nonzero.

The runner uses the exact executable resolved during validation. It writes an
explicit stopped run set when a hard runtime bound prevents remaining cells.
Reporting refuses missing indexes, mixed plan roots, unregistered Runs, and
Run bytes that do not match the rooted run set.
