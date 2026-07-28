# ADR 0011: Framework-neutral evaluation and removable engine experiments

- Status: Proposed
- Candidate release: Canopus `v0.9.0` only after the registered adoption gate
- Protocol effect: None
- Vela authority effect: None

## Context

Canopus currently has one narrow product responsibility: turn one exact Vela
offer into a bounded, independently replayed Run and, through an explicit
second step, an authenticated Submission. Its released core has no runtime
dependencies. The existing `Engine` and `EngineResult` interfaces already
separate product orchestration from the native Codex implementation.

Agent frameworks, schedulers, checkpoint stores, tracing products, and
multi-agent protocols may improve some research tasks. They may also add
latency, hidden state, authority confusion, dependency risk, and new replay
requirements without improving scientific output. Installing one in Canopus
before a matched evaluation would turn an experiment into permanent product
surface.

The question is not which framework has the richest feature set. It is:

> Under the same model, task facts, tools, verifier, budget, retries, and
> stopping rule, does an orchestration layer produce more verifier-passing
> bounded work per all-in cost and expert-minute than native Codex and a plain
> TypeScript runner?

## Decision

Keep the published Canopus core framework-neutral and zero-runtime-dependency.

1. `Engine` and `EngineResult` remain the only supported engine seam.
2. Do not add a capabilities interface, universal `WorkGraph`, checkpoint
   database, workflow language, or framework-specific Mission fields.
3. Experimental engine drivers and evaluator bridges remain source-only,
   outside the npm package payload, with separate exact dependency locks.
4. Inspect and Harbor remain external evaluation bridges. Their records do not
   become Canopus or Vela canonical state.
5. A formal process-level engine-driver contract waits until two independent
   engines demonstrate the same missing interface.
6. Trace export is deterministic and metadata-only by default. It excludes
   chain of thought, private prompts, credentials, environment secrets, human
   keys, and repository-authority material.
7. Vela remains the sole protocol, Standing, repository-authority, and
   scientific Decision boundary.

## Registered evaluation

The non-normative `canopus.evaluation-plan.v1` binds exact:

- task, model, Codex, Canopus, Vela, Git, environment, and dependency
  identities;
- packet, artifact, verifier, and source roots;
- arms, assignment, repetitions, budgets, retries, stopping, scorers,
  exclusions, and publication policy;
- secret-custody and authority restrictions; and
- a canonical full plan root.

An amendment names the full root of its predecessor and states the reason.
Usable output never permits a silent in-place plan edit.

Source-only evaluation commands are:

```text
bun run eval:validate
bun run eval:run
bun run eval:report
bun run trace:export -- --format otlp-json --content none
```

They are not public Canopus commands and are excluded from package files.

## Stages and adoption gate

Stage A compares three arms on one exact bounded Erdős task and one qualifying
deterministic scientific-computing task:

1. native Codex with its ordinary task surface;
2. native Codex with the exact same structured packet and frozen verifier
   available to Canopus; and
3. the current Canopus single-engine path.

The second arm prevents Canopus from receiving credit for task packaging or a
verifier that could have been composed directly with the native agent.

Stage B runs only after a safe Stage A. It compares a plain TypeScript fixed
graph, stateless LangGraph, and OpenAI Agents SDK on the same tasks.

Stage C runs only after Stage B identifies a candidate winner. It repeats the
candidate, the stronger registered native control, and plain TypeScript on
held-out tasks. The complete campaign therefore has a hard maximum of 36 model
calls: 12 in each stage.

First-party repetitions earn no independent-participant credit.

A framework becomes supported only if:

- no hard safety or integrity gate fails;
- it wins both task classes in Stage B;
- the result repeats on held-out Stage C; and
- primary efficiency improves by at least 20 percent over both native Codex
  and plain TypeScript.

Otherwise remove the supported runtime integration, retain the rooted
evaluation evidence, and do not release Canopus `v0.9.0`.

The evaluation reports execution lift, state lift, and inheritance lift
separately. The registered plan binds each function to a distinct scorer root;
one scorer cannot silently stand in for all three. Its north star is genuine
reusable scientific progress per scarce human judgment. A verifier pass, a
successful workflow, and a correct scientific disposition remain different
outcomes.

If native Codex with the same packet and verifier matches Canopus, remove the
Canopus machinery that did not create lift. If Git plus the same signed
structured evidence and verifier matches Vela in the registered state and
inheritance study, simplify Vela rather than moving the goalposts.

## Failure boundaries

An evaluation is disqualified by:

- unauthorized Vela or Git mutation;
- credential, key, private prompt, or unrelated host-data exposure;
- verifier success presented as acceptance;
- hidden failed runs, exclusions, retries, or benchmark-answer leakage;
- post-output plan mutation;
- an external framework becoming required for replay;
- a different task, information set, verifier, budget, or stopping rule across
  matched arms; or
- deletion of negative evidence because an aggregate score improved.

Infrastructure may retry once only when failure occurs before usable model
output. The retry and failure remain recorded.

## Consequences

Canopus can compare modern orchestration without becoming an orchestration
framework. A neutral result simplifies the product. A positive result must
survive held-out evidence before it earns a release dependency. Deleting every
experimental driver leaves Run replay, Submission authentication, and Vela
Standing unchanged.
