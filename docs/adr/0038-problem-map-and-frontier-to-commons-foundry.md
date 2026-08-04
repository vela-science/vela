# ADR 0038: Problem map and frontier-to-commons foundry

- Status: Accepted, 2026-08-04
- Protocol effect: none
- Authority effect: none
- Product effect: makes the public problem map and reviewer-ready reuse path
  the active Vela programme

## Context

Vela can create, inspect, verify, decide, replay, and transfer exact scientific
state. The current Frontiers and Web projection already contain useful public
material: a rooted Erdős corpus, source records, exact problem pages, bounded
work offers, Decision history, and correction-aware Result Dossiers.

The remaining constraint is absorption. Researchers and maintainers need to
find the exact problem, understand what each source reports, see what a named
Frontier decided, identify the missing work, and move reusable results into the
right maintained commons without reconstructing several repositories.

Mathlib, Physlib, Formal Conjectures, VibeMathed, and native proof systems each
own parts of this lifecycle. Vela creates value at the boundary between them.
It must not replace their kernels, package systems, identifiers, governance, or
scientific authority.

## Decision

Adopt two coupled product surfaces:

1. `problems.science` is the accessible problem-centred map and action surface.
2. The frontier-to-commons foundry turns source-bound work into small,
   reviewer-ready candidates for the appropriate native library.

Both use the existing `vela-web` codebase, source registry, projection,
release manifest, and read-only database. `app.vela.space` remains the forensic
record surface. No second writer, database, repository, or canonical object is
created.

The first public collection is Erdős. Broad source coverage may use explicitly
labelled thin pages. Deep pages add local Claims, scoped Verification,
Decisions, corrections, current Targets, and reproduction. Status remains
qualified by source; an external label, open pull request, successful build,
or green verifier result does not become Vela Standing.

The first foundry proof uses accepted or verifier-passing Formal Conjectures or
Erdős artifacts. For each selected artifact, the owning Frontier records one
explicit disposition:

- extract a general candidate for Mathlib or another native commons;
- retain the result as source-local;
- archive it without a maintenance promise; or
- reject extraction because it duplicates existing APIs or lacks a durable
  owner.

The reviewer packet binds the exact source, declarations, toolchain and lock,
native checks, axiom inventory, import footprint, statement-fidelity review,
AI-assistance disclosure, nonclaims, intended native owner, and unresolved
maintainer decisions. It remains useful without Vela installed.

Physlib is the first external foundry pilot only after its public policy or a
maintainer supports the selected contribution class. Vela will start with a
small API-map, documentation, definition, or lemma gap and a human-owned pull
request. It will not send bulk-generated work to external maintainers.

## Repository placement

- `vela` owns this cross-Frontier product decision and the existing portable
  scientific-state contract. It gains no source-specific code.
- `vela-web` owns the problem map, source-qualified presentation, crosswalk,
  dossier projection, search, and maintenance views.
- Each source-owning Frontier owns candidate profiles, fidelity review,
  scientific Decisions, and exact next obligations until two maintained
  consumers prove identical reusable semantics.
- Mathlib, Physlib, Formal Conjectures, VibeMathed, and other native projects
  retain their identifiers, artifacts, review, merge, and maintenance state.

## Initial gates

The August public wedge is complete when:

- `problems.science` serves a useful landing page from the current root-bound
  Vela Web release;
- the Erdős collection exposes honest coverage, source-qualified status,
  several deep cases, exact current Targets, and machine-readable roots;
- stale Targets fail release activation; and
- a cold reader can distinguish source status, native Verification, local
  Standing, unresolved work, and the next valid action.

The first foundry proof is complete when:

- three source-bound artifacts receive explicit extraction dispositions;
- at least one reviewer packet can be reproduced from exact inputs;
- the packet finds source-fidelity or dependency defects before external
  review, or reduces the time needed to locate and assess the evidence; and
- no package, protocol object, or external acceptance is inferred from the
  result.

A shared package or hosted index remains blocked until two independent
maintained consumers use the same contract and extraction deletes more
maintained duplication than it adds.

## Measures

The programme measures accepted reusable value per scarce expert minute:

- evidence-location time;
- statement-fidelity defects found before external review;
- reviewer minutes and revision rounds;
- accepted declarations or documentation;
- independent maintained consumers;
- correction discovery and repair time; and
- cold-successor time to a useful action.

Generated proof count, agent count, token volume, package count, pull-request
count, stars, downloads, and green-check percentage are not success measures.

## Rejected alternatives

- A `vela-math` repository or separate `problems.science` data store.
- A Vela theorem prover, agent runtime, package resolver, or Lean registry.
- A universal ontology, trust score, reviewer marketplace, or automatic
  acceptance path.
- A `VelaLib`, `ScienceLib`, or other permanent library before recurring demand
  and durable stewardship exist.
- More architecture work that does not fix a named reader, reviewer, or
  maintainer failure.

## Consequences

Vela core returns to defect-driven maintenance. The active product work moves
to Vela Web and source-owning Frontiers. Scientific campaigns continue when
they produce consequential state, but local search volume no longer substitutes
for a useful public map or a reusable result.

This decision preserves ADR 0019's package gate, ADR 0030's source-observation
boundary, and ADR 0036's requirement for genuine scientific consequence.
