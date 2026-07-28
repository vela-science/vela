# Source-only evaluation

This directory implements Canopus ADR 0011. It is not included in the npm
package and creates no Vela state.

`canopus.evaluation-plan.v1` registrations freeze exact tasks, arms, budgets,
scorers, custody rules, and publication rules before usable model output.
There is deliberately no live registration until Canopus `0.8.0` and the
current Erdős producer loop pass.

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

Stage A must retain both native controls: ordinary native Codex and native
Codex with the exact packet and frozen verifier used by Canopus. Execution,
state, and inheritance lift are reported separately. Matching the native
same-packet arm is a deletion signal for Canopus complexity, not permission to
invent another layer.
