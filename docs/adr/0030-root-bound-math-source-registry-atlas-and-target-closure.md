# ADR 0030: Root-bound Math Source Registry, Atlas, and Target closure

- Status: Proposed
- Proposed: 2026-07-30
- Protocol effect: none while Proposed; no new canonical object or reducer rule
- Authority effect: none
- Product effect: build an exact Math Source Registry, make the existing
  Observatory a complete first-party Math Atlas, and make stale work a
  release-blocking failure
- Service effect: reuse the existing normalized Neon read model; no new
  service, database, writer, repository, or hosted authority
- Builds on:
  [ADR 0025](0025-math-first-compounding-product-architecture.md) and
  [ADR 0028](0028-living-frontier-map-and-native-system-boundary.md)

## Context

Vela has now completed a real bounded sequence:

```text
Target
-> Run
-> Submission
-> Verification
-> human Decision
-> Event
-> replayed Standing
-> rebuilt projection
```

The sequence exposed a product failure. At proposal time the accepted Erdős
range `10429401..10429600` remained the first available offer after the
Decision. Vela correctly rebound the Target Index to the new repository root,
but a new root alone did not establish that domain-level work remained valid.
Formal Conjectures similarly continued to offer a foreign-reference-retention
Target whose Submission and Verification were already retained.

The map changed while its first action stayed stale. That is not a complete
`map -> target -> run -> verify -> commit -> compound` loop.

The first implementation slice now validates the completed Erdős packet
against its exact retained Git bytes and accepted evidence, exposes the
contiguous successor `10429601..10429800`, and closes the completed Formal
retention Target without importing foreign Standing. ADR acceptance still
waits for all-four-Frontier closure checks and an exact Atlas reconstruction.

The next real Formal mission reproduced the remaining lifecycle defect more
precisely. Submission `vsb_b47c353dd4a5409f` was registered and published at
Formal commit `de9caba17b484f19eaff5d7bab462920d85b5c6f`, but its exact Target
remained exposed as `open` and became non-actionable with
`target_index_input_root_mismatch`. The source-local generator had declared
`.vela/repository.json` as a Target Index input while the index and packet also
bound the mutable repository root. Registering the Submission therefore
invalidated the offer that produced it. The Submission is complete canonical
evidence and must not be retried.

The current read product already has the correct architectural pieces:

- four canonical Git Frontier repositories;
- released Vela replay and root verification;
- one `@vela/frontier-data` projector;
- one normalized, release-scoped Neon observatory schema;
- atomic projection activation and bounded release retention;
- a SELECT-only application role; and
- the existing Vela Observatory.

It does not need a new Atlas repository, graph database, hosted
semantic-package Registry, global ontology, second writer, or public mutation
API. It needs a strict Math Source Registry, a stricter read contract, and a
complete usable product.

“Atlas” has also accumulated two incompatible meanings. This ADR uses:

- **Math Atlas** for the bounded read-only product over the four declared
  first-party mathematical Frontiers; and
- **federated/global Atlas** for a future product spanning independently
  governed external Frontiers.

Only the first is proposed here.

## Decision proposed

### 1. Build a Math Source Registry, not a truth registry

The Math Atlas needs an exact registry of the native sources it observes.
Add a non-protocol, release-scoped Math Source Registry to the existing
`@vela/frontier-data` projector and observatory read model.

Each source record binds:

- stable source ID and native namespace;
- source-declared publisher or maintainer and source kind;
- one or more observed retrieval locators;
- optional attributed ownership or canonical-locator claims;
- the meaning the native source declares;
- license, access, redistribution, and snapshot policy;
- adapter identity, source root, version, and environment;
- current native release, commit, archive, or observation identity;
- raw snapshot root when exact bytes are retained;
- projected record count and root;
- coverage, omissions, inaccessible material, and tombstones; and
- the successful observation identity.

Each native record binds its exact source, native ID, kind, revision, locator,
metadata root, retained content root when available, and observation.
Frontier bindings distinguish:

```text
reference   native identity only
snapshot    retained exact bytes
admission   local Submission, Verification, and Decision
```

The inventory describes where a record came from and how it was observed. It
does not assert truth, infer equivalence, create a Claim, or change Standing.
Similarity or a shared label is never stored as an exact binding.

Initial entries cover only the native sources already used by the Erdős,
Formal, Sidon, and Quantum Frontiers. A new source requires a named Atlas gap
and a source-specific rights, identity, versioning, pagination, and update
audit before observation.

Each adapter implements the native source contract. It preserves that source's
identifiers, revisions, pagination, deletion or tombstone semantics, rights
boundary, and completeness checks. A generic “math record” importer cannot
replace these adapters because the Erdős problem corpus, Git repositories,
OEIS records, and retained quantum certificate do not share an identity or
update model.

Source adapters split networked acquisition from offline projection. Networked
acquisition fetches into an isolated temporary workspace, validates
source-specific invariants, and emits an immutable rooted observation bundle.
Offline projection consumes only that bundle, the checked-in typed source
declaration, the released Vela binary, and exact Frontier checkouts. It emits
coverage and loss manifests, builds a complete candidate transaction, and
leaves the active release unchanged on any failure. Failed attempts are
retained as immutable projection-run audit records outside the active Atlas
release. The web application never fetches a native source at request time.

An observation is immutable and independent of a Vela Web release. Once its
source revision, permitted bytes, records, coverage, omissions, adapter, and
root are fixed, later web releases reference that observation by root instead
of copying or relabeling it. Frontier bindings remain separate records. They
state which Frontier object references, snapshots, or admits one observed
native record at one Frontier release. A binding can change while the source
observation remains byte-identical.

Source identity, publisher/maintainer claims, observed locators, rights
decisions, adapter versions, and snapshot policy are checked-in typed
configuration under `@vela/frontier-data`. They are never authored in Neon or
inferred from mutable fetched metadata.

Retain raw source archives only when licensing permits and reproduction
requires them. Large retained archives use immutable GitHub release or OCI
artifacts with full roots; Neon contains searchable projections rather than a
second canonical bulk archive.

This source inventory is distinct from a future semantic-package Registry.
Package discovery remains gated by repeated independent reuse and net
deletion.

### 2. Define the Math Atlas as the existing Observatory

The canonical product surface remains `app.vela.space`. The existing
`/frontiers` route is the Math Atlas overview. Stable Frontier, problem, Claim,
work, review, run, reproduction, search, and graph routes remain its object
surfaces.

Do not create:

- another application;
- an `/atlas` route that duplicates `/frontiers`;
- another repository or package merely for naming;
- a canonical Atlas object; or
- a writable Atlas API.

The Atlas is an exact, replaceable view. Deleting it changes no Claim,
Submission, Verification, Decision, Event, or Standing.

### 3. Keep Frontier-local sovereignty

Each Git Frontier remains canonical for its bounded state. The released Vela
implementation remains the sole supported first-party repository writer and
repository-authority transaction executor. Independent conforming validators
and readers remain permitted and are required for evidence.

The Atlas may compare and connect attributed state, but:

- Standing is always qualified by Frontier;
- a foreign accepted Claim is not locally accepted;
- a retained foreign reference reports local Standing effect `none`;
- graph position, search rank, similarity, or shared notation establishes no
  authority or equivalence;
- native systems retain their own identifiers, semantics, packages, and
  verifier rules; and
- derived Obligations and transition diffs are read products, not canonical
  scientific objects.

### 4. Extend one release manifest

Evolve the current Observatory release manifest rather than adding a parallel
Atlas manifest.

Each activated Math Atlas release binds:

- Vela version and checked binary root;
- source inventory, adapter, observation, snapshot, coverage, omission, and
  native-binding roots;
- every source Frontier's slug, ID, Git commit, tree, origin, repository root,
  authority keyset root, and authority-policy root;
- Claim, Standing, review, Submission, Registration, Verification, Artifact,
  problem, graph, and current-work counts;
- graph source and layout roots;
- declared corpus and source coverage;
- every explicit omission, inaccessible source, and unsupported meaning;
- each current Target ID, packet root, availability, and closure assessment;
- exact native-system references and retained snapshots;
- attributed foreign references and their local Standing effect;
- table roots, release root, activation time, and projector identity.

A changed count or omission is valid only when the new release binds the
changed exact source. Observation time alone does not create a new scientific
release.

The implementation adds one additive migration for checked source
declarations, immutable observations and native records, release-to-observation
membership, and release-scoped Frontier bindings. Observation and native-record
roots exclude the web release identity. The release manifest commits to the
exact observation membership and Frontier bindings, so two releases may share
one immutable observation without sharing a mutable row.

Candidate loads use PostgreSQL `COPY FROM STDIN` in bounded chunks inside the
candidate transaction. Every chunk has a declared table, schema, row count,
and deterministic row-root input. The projector checks inserted counts and
table roots before moving `current_release`. It does not issue one insert per
record or store one source as a large JSONB document.

Each table participates in table-root verification, manifest normalization,
retained-version readers, and release-skew checks. The application reader has
explicit SELECT access only to curated public tables or views. Operational
acquisition evidence, restricted locators, and nonredistributable details are
outside that grant.

### 5. Make Target closure a release invariant

A Target exposed as current must be:

- tracked and content-addressed;
- bound to the current Frontier root;
- not completed by accepted exact evidence;
- not a duplicate or unexplained overlap of retained completed work;
- not already satisfied by the Submission or Verification it requests;
- paired with an existing verifier or an explicit verifier requirement;
- within the Frontier's declared scope; and
- reproducibly ranked from the current head.

Producer-work closure follows the exact domain completion contract. For the
current Attempt contract, one valid registered Submission completes the
producer Target. Registration must therefore close that offer, or the owning
Frontier must block or preserve it with an exact reason why the Submission did
not satisfy the contract. It must not remain available merely because
Verification or a human Decision has not occurred.

Verification and Decision remain separate:

- Verification records scoped evidence and changes no Standing by itself.
- Decision changes Standing through repository authority.
- Neither is a substitute for producer-work closure.

An accepted or rejected Decision does not mechanically close every Target.
The owning Frontier still evaluates the Target's exact domain completion
contract. Before an Atlas release activates, it must either:

1. close the completed Target and derive a valid successor;
2. preserve it with an exact reason why the retained Submission, Verification,
   or Decision did not discharge its declared completion contract; or
3. block work publication with a precise diagnostic.

Merely rebinding an unchanged Target Index to a new repository root cannot
convert stale work into fresh work.

The immediate implementation guard rejects `.vela/repository.json` as a
declared Target Index input because it duplicates the mutable repository
binding and creates a self-invalidating dependency. This prevents another
index from being sealed with the reproduced defect, but it does not by itself
justify a protocol migration.

A future, separately accepted contract should remove mutable repository-root
churn from the derived index. The stable Target Index would bind the Frontier
origin, source inputs, ordered Target semantics, and packet roots. The private
Attempt would continue to bind the exact repository root observed at start.
Successful Registration would durably retain:

```text
target ID
Target Index root
packet root
Attempt ID and binding root
starting repository root
```

That binding belongs at the Vela-issued Registration edge rather than being
invented by a producer or inferred later from Claim prose. This ADR does not
select a schema version, authorize a migration, or change retained bytes.

Target validation starts in the owning Frontier because searched numerical
ranges, Lean declarations, quantum certificate conditions, and Sidon witness
bounds have different semantics.

The source-local validator emits a non-protocol `vela.target-closure.v1`
envelope containing status `available|closed|blocked`, exact reason, evidence
roots, completion-contract root, and successor packet reference. The Frontier
generator and CI use it to rewrite and validate the tracked Target Index before
exposure. Canopus and the projector verify the same envelope; released Vela
reads the resulting tracked Target Index. None may silently skip a blocked
Target or invent a replacement.

This avoids duplicating closure logic while keeping domain semantics out of
Vela core. If `vela next` itself must invoke arbitrary domain validators, that
would require a separate generic interface and a compatible Vela release.

The one-time Formal recovery is source-local: bind the existing Target to its
already-published Claim, Submission, Registration, Proposal, Artifacts, and
retained transaction evidence; emit the exact closure envelope; remove or mark
the discharged offer closed; and seal the resulting Target Index. Recovery
must not register another Submission, rewrite the published record, or wait
for Verification or Decision merely to stop duplicate producer work.

The Erdős-to-Formal relation is projected only from the retained
content-addressed foreign-reference artifact and its exact Verification
record. Claim prose, labels, and inferred similarity are never relation
sources.

### 6. Make the product answer-first

The Math Atlas provides six exact views:

1. **Corpus map:** declared scope, coverage, omissions, Standing, and work.
2. **Problem map:** statement, current Claims, evidence, open core, and Target.
3. **Why this stands:** shortest exact path through Decision, Verification,
   Submission, and evidence.
4. **Scientific Diff:** before/after roots, semantic changes, surviving state,
   closed/opened Obligations, and Target movement.
5. **Evidence map:** Claim, Artifact, Submission, Verification, Proposal,
   Decision, and replay connection.
6. **Exploration:** exact search and typed graph with an equivalent ledger.

Every primary record answers:

```text
What is this?
What currently stands and why?
What is safe and useful to do next?
```

Exact roots remain one disclosure away. Verification never looks or reads like
acceptance.

### 7. Keep the read model disposable

The existing normalized Neon schema remains the sole hosted read model.
Projection writes occur only in the exact refresh workflow. The web role
remains SELECT-only and scoped to the observatory schema.

Neon uses one durable production branch. The refresh workflow may create one
temporary migration branch or one temporary benchmark branch. It deletes that
branch after apply, discard, or evidence capture. Release rollback lives in
immutable release rows and the atomic `current_release` pointer, not in
standing database branches. Do not maintain parallel staging, archival,
event-era, or per-release branches.

Prefer bounded SQL views and deterministic queries to duplicate stored
objects. Add a persisted projection only when:

- its complete source set and root are explicit;
- deterministic recomputation is tested;
- it materially reduces bounded read or build cost; and
- it cannot affect replay or Standing.

Search documents, graph layout, derived Obligations, coverage summaries,
transition diffs, and relation neighborhoods are disposable. An unavailable
or stale projection fails visibly; the application never falls back to
request-time Git or mutable external data.

Collection reads use keyset pagination over a stable sort key plus full object
ID. A root-bound cursor also commits to the active Atlas release and query
filters. Offset pagination is not part of the read contract. Graph reads return
one bounded typed neighborhood or a keyset-paginated ledger, including explicit
returned, hidden-neighbor, and total-match counts. Ordinary routes never load
the full graph.

### 8. Prove scale before adding scale infrastructure

The alpha release must pass a rooted 100,000-record benchmark. The frozen
benchmark includes source-native records, immutable observations, Frontier
bindings, and representative graph edges. It measures:

- chunked `COPY` load, count and table-root verification, and atomic
  activation;
- deterministic clean rebuild and failed-load pointer containment;
- keyset page correctness, cursor stability, transferred bytes, and p50/p95
  latency; and
- bounded graph-neighborhood and equivalent-ledger reads.

The benchmark plan fixes the dataset generator or retained inputs, environment,
chunk bounds, query mix, repetitions, and budgets before execution. Passing
means exact roots, zero silent loss, bounded responses, and all preregistered
budgets met. It does not establish production scale or general workload
performance.

Vela Web may make a scalability claim only after the same contract passes a
separate rooted 1,000,000-record benchmark. Until a measured bottleneck
appears, do not add table partitioning, a graph database, a vector database or
embedding index, a second read store, or a streaming ingestion service. A
future change must name the failed budget, compare the simpler PostgreSQL
design, and show that the added system improves the measured constraint.

### 9. Earn shared packages and expansion

The Atlas does not authorize `@vela/math`, a hosted semantic-package Registry,
or federation.

A shared Math contract requires:

- two maintained consumers;
- deterministic offline generation;
- exact native-identity preservation;
- explicit unsupported meaning and loss;
- no replay or authority effect; and
- deletion of more maintained duplication than the shared module adds.

A static package index requires independent reuse. A hosted Registry requires
external publishers and consumers plus demonstrated Git-release friction. A
federated Atlas requires independently governed Frontiers, exact
cross-Frontier correction, authority containment, and measured cold-user lift.

## Release and acceptance gates

### Alpha implementation gate

The first working Math Atlas alpha requires:

1. all four source checkouts clean and equal to their remote `main`;
2. every observed native source has an exact inventory entry, rights decision,
   adapter root, observation identity, coverage, omissions, and rebuild path;
3. full and applicable incremental observation reproduce their expected roots
   and failed observation cannot move the active release;
4. complete exact projection of the declared current Claim and Erdős problem
   inventory;
5. explicit coverage and omissions;
6. zero completed or overlapping Targets exposed as current;
7. complete Erdős 1056 problem, evidence, Decision, and next-action views;
8. separate Formal kernel, fidelity, Verification, and Standing views;
9. correct classification of the retained quantum witness and Target;
10. exact Why this stands and Scientific Diff derivation;
11. root, schema, target, and source drift failing closed;
12. read-only Neon access and no request-time Git or source fetch;
13. clean reconstruction without Neon or the Observatory;
14. mobile, keyboard, long-root, empty, and integrity-error access;
15. the rooted 100,000-record ingestion and read benchmark passes its frozen
    correctness and performance budgets; and
16. production uses one durable Neon branch and has no retained migration,
    benchmark, or per-release branch.

Accept this ADR at the Vela Web `0.430.0` alpha release only when all sixteen
implementation gates pass and the production manifest, web tag, source roots,
and active projection agree. The acceptance covers only the bounded
Math Source Registry, exact first-party Atlas read model, and Target-closure
invariant.

Product adoption, measured lift, additional native sources, reusable packages,
correction propagation, and global/federated Atlas claims remain separately
gated by the campaign and ADRs 0026 and 0029.

## Rejected alternatives

- **Create a central Atlas database as truth.** Rejected because Frontiers and
  native systems remain sovereign.
- **Ingest every mathematical source into a giant warehouse first.** Rejected
  because native identity, licensing, coverage, and exact update semantics must
  be proven source by source.
- **Treat the source inventory as a theorem or package Registry.** Rejected
  because source observation, package distribution, and scientific Standing
  are different contracts.
- **Create an Atlas repository or service.** Rejected because the existing
  Observatory and projector already own the read product.
- **Use one generic importer for every mathematical source.** Rejected because
  native identifiers, revisions, rights, pagination, and deletion semantics
  differ by source.
- **Copy each observation into every web release.** Rejected because immutable
  observation identity belongs to the source revision; releases should bind it
  by root and keep Frontier bindings separate.
- **Use row-at-a-time inserts or unbounded reads.** Rejected because bounded
  `COPY`, keyset pagination, and explicit graph neighborhoods provide exact
  performance contracts without another service.
- **Add partitioning, a graph database, or a vector stack preemptively.**
  Rejected until the 100,000-record benchmark exposes a measured limit and the
  1,000,000-record benchmark justifies a scalability claim.
- **Treat a new repository root as proof that work is fresh.** Rejected by the
  reproduced stale-target failure.
- **Put all domain completion semantics into the Vela kernel now.** Rejected
  until stable recurrence exists across domains.
- **Use graph size as the product or success metric.** Rejected because a map
  must improve correct comprehension and continuation.
- **Infer cross-Frontier truth or equivalence.** Rejected because foreign
  references carry attribution, not local authority.
- **Publish a Math package, package Registry, or federation roadmap first.**
  Rejected until reuse, net deletion, and external value earn them.

## Failure disposition

If the Atlas remains exact but does not improve correct cold use, retain it as
a bounded reader and make no adoption claim. If derived Obligations,
Scientific Diff, graph expansion, foreign envelopes, or shared profile code do
not improve correctness, continuation, or maintained simplicity, delete or
narrow them.

Failure does not authorize another layer.
