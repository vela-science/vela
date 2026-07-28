# Vela roadmap

Vela is proving one compact thesis: exact scientific state should survive
different producers, verifiers, decisions, corrections, and readers without
turning any workbench or database into authority.

## P0 — complete the real loop

1. Complete one bounded Erdős
   Submission → Verification → human Decision loop.
2. Require exact replay, zero accepted-state mutation before Decision, and
   clean-clone reproduction.
3. Release Canopus `0.8.0` only after its package, custody, verifier, replay,
   and provenance gates pass.
4. Refresh the read-only Observatory only from the verified Frontier head.

The Decision may accept, reject, or cancel. Engineering completion does not
depend on acceptance.

## P0 — simplify repository ownership

1. Keep public architecture and roadmap here in `vela`.
2. Consolidate the TypeScript protocol SDK, shared conformance fixtures, and
   independently versioned Canopus package into the public Vela monorepo.
3. Preserve the public executable boundary: Canopus may consume protocol
   contracts but never authority internals.
4. Keep exact replay, Target Index checks, and domain verifiers in each
   Frontier.
5. Keep projection and read-only-boundary checks in `vela-web`.
6. Move reusable security and provenance workflows to
   `vela-science/.github`.
7. Import Canopus history without squash after `0.8.0`, then archive the old
   repository rather than maintaining a mirror.
8. Archive `vela-internal` after its load-bearing inventory reaches zero.

Do not create a replacement assembly or lab repository.

## P1 — measure before expanding

Run a registered framework-neutral evaluation of:

- native Codex;
- the existing Canopus single-engine path; and
- optional orchestration only after safety gates pass.

Retain an orchestration integration only if it wins both registered task
classes, repeats on held-out tasks, and improves verified scientific work per
all-in cost and expert-minute by at least 20 percent. Otherwise preserve the
result and delete the integration.

## P1 — math-first evidence

- Run one named mechanically checkable Formal Conjectures mission.
- Measure evidence-location, correction, replay, and continuation against Git
  plus the same verifier.
- Keep kernel verification, statement fidelity, and scientific acceptance
  distinct.

## P2 — one computational transfer

After the math campaign demonstrates measurable value, package one bounded
public computational replication with a source-local Canopus adapter and a
root-bound RO-Crate export. Publish an explicit loss report. Require a second
genuinely different format before proposing shared adapter infrastructure.

## Deferred

No `1.0.0` schedule, hosted authority, scheduler, graph database, universal
ontology, shared adapter registry, mandatory orchestration framework, or
second writer is planned.

Failure to demonstrate lift causes simplification and deletion, not another
architecture layer.
