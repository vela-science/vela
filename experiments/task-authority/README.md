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

## Live shadow and client substitution

The preregistered follow-up composes three retained facts without another
model call:

- one fresh `canopus replay` of the exact Erdős 1056 run;
- the 10/10 correction-aware continuation previously produced by Codex CLI;
- the 10/10 continuation previously produced from the same packet by Claude
  Code.

It then injects post-approval evidence drift and repeats the frozen eight-case
hostile boundary against each exact workbench identity:

```bash
bun run experiment:task-authority-shadow
```

Both clean shadows pass, both evidence-drift injections require
reauthorization, both workbench bindings reject 8/8 hostiles, and accepted
state remains unchanged. The result remains `PIVOT_OPERATIONAL_ONLY`: the
Claude Code continuation was tool-free, so it does not independently observe
real source access or host effects. No runtime contract, package surface, or
Vela object is promoted from this first-party evidence.
