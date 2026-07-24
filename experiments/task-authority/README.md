# Source-local task-authority hostile experiment

Status: non-normative experiment. Nothing in this directory is shipped in the
Canopus npm package or interpreted by Vela.

This experiment compares:

1. the released `canopus.mission.v1` execution boundary; and
2. one exact task-authority packet that adds current workload/grant identity,
   source classes, host-effect limits, monitor facts, and reauthorization
   triggers.

The baseline is intentionally strong. It already binds the exact worker,
packet, verifier, paths, budgets, environment roots, and landing ceiling. The
experiment asks whether those facts are sufficient for eight task-authority
hostiles identified in the July 24 external-movements assessment.

Run:

```bash
bun experiments/task-authority/run.mjs
bun test tests/task-authority-experiment.test.mjs
```

The evaluator is deterministic, source-local, dependency-free, and
non-authoritative. A violation remains a violation even when provenance,
signature, or verifier fields are green. The output declares:

```text
scientific: none
authority: none
standing: none
```

The experiment can justify an operational Canopus or workbench control. It
cannot by itself promote a Vela protocol object.
