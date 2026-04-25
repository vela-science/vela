# Math and principles

This note describes the mathematical backbone behind Vela. It is
non-normative for v0: the release implements the state kernel, not the full
theory stack.

The public product should not lead with category theory, sheaves, topos theory,
or Byzantine fault tolerance. Those are deep structure. The release-facing
claim remains:

> Vela is portable, correctable frontier state for bounded scientific
> questions.

The internal technical identity is sharper:

> Vela compiles scientific artifacts into typed, replayable state transitions
> over bounded frontier state.

Those transitions preserve evidence, scope, uncertainty, contradiction, and
provenance so they can later be projected into constellations and written back
through runtime and network layers.

## 0. meta: typed state transitions

The practical foundation is not abstract math first. It is a typed event
schema:

- scientific objects have explicit types
- durable writes are state transitions
- transitions compose through an event log
- provenance, uncertainty, scope, and review status are effects carried by
  those transitions

Category theory is useful as an interpretation: artifacts, findings, evidence,
events, and projections can be seen as typed objects and transformations
between representational spaces. But the engineering rule is simpler:

> Build type signatures first. Let categorical semantics justify the shape
> later.

## 1. state: dependent scientific objects

Scientific state depends on context. A finding is incomplete without its source,
evidence, conditions, entities, confidence, links, provenance, and review
history.

**Now**

- finding bundles
- source records
- evidence atoms
- condition records
- typed links
- proposals
- canonical events
- proof freshness

**Partial**

- replay
- diff / merge
- compatibility projections for old review/confidence fields

**Future**

- first-class contradiction records
- richer scope variables
- dependent validation rules across protocol, experiment, result, and review
  objects

## 2. runtime: belief under partial observability

Science is a partially observed dynamical system. A frontier is not truth; it
is a bounded belief state under incomplete evidence and incomplete review.
("Belief state" here is the theory-side nomenclature. Operational Vela —
README, protocol, CLI — calls this "frontier state.")

Simple Bayesian updating is not enough for the long-term theory because
scientific contradictions are often actionable structures, not noise to average
away.

The intended future model is:

- belief-state dynamics under partial observability
- imprecise probability / credal sets for live uncertainty
- dual-control experiment choice, where actions both test and change the state
- value-of-information prioritization for what would reduce uncertainty next

**Now**

- `confidence.score` is a bounded epistemic support measure on a finding
- contested flags, caveats, and review queues preserve uncertainty
- tensions and contradiction links are review leads

**Partial**

- confidence is deterministic, not a full posterior
- contradiction structure is link-derived and signal-derived, not yet a
  first-class state object

**Future**

- explicit `CredalUpdate` records
- contradiction-preserving belief updates
- runtime protocol / experiment / result writeback

## 3. continuity: information channels and error correction

Scientific state degrades when artifacts are compressed into memory, evidence
detaches from claims, reviews fail to propagate, or proof packets go stale.

Continuity is therefore an information-channel problem:

- rate-distortion describes artifact-to-state compression
- error-correcting codes describe redundancy through evidence, replication,
  review, provenance, and benchmark anchors
- proof freshness describes whether later state transitions invalidate an
  exported packet

**Now**

- source records and evidence atoms keep source grounding attached
- condition records prevent silent overgeneralization
- proof packets export a reviewable snapshot
- replay checks verify event consistency
- benchmark fixtures act as calibration anchors

**Partial**

- correction propagation is simulated over declared typed links
- proof freshness is frontier-local

**Future**

- propagation obligations across institutions and branches
- stronger continuity guarantees for copied, merged, or summarized frontier
  state

## 4. network: incentives, trust, and governance

Open scientific state has adversarial pressure: fraud, p-hacking, strategic
citation, captured review, benchmark gaming, and low-quality agent writeback.

The long-term theory needs:

- mechanism design for contribution and correction incentives
- Ostrom-style commons governance for shared scientific state
- BFT-inspired trust models for faulty or adversarial actors
- evolutionary dynamics for norm stability

This is not an MVP consensus protocol. Near-term implementation should remain
softer:

- actor identity
- source provenance
- review quorum
- trust policies
- conflict flags
- audit trails
- challenge and correction events

**Now**

- reviewer identity fields
- proposal records
- canonical event history
- optional signatures
- auditability through Git and proof packets

**Future**

- institutional trust policies
- reputation-weighted validation
- adversarially robust propagation semantics
- network governance protocols

## 5. constellations: Local-To-Global projection

A constellation is not a graph visualization. It is a local-to-global
projection over scientific state.

Scientific fields are not globally consistent maps. They are overlapping local
maps:

- lab-level maps
- subfield maps
- organism-specific maps
- method-specific maps
- clinical maps
- mechanistic maps
- therapeutic maps

A constellation asks what can be glued from those local views into a navigable
global picture, and where the gluing fails.

Sheaf-like language is useful internally:

- local scientific regions carry local findings, assumptions, instruments, and
  standards
- compatibility rules describe when local views can be assembled
- obstructions identify unresolved contradictions, scope mismatches, or
  translation failures

Plainly:

> A constellation shows what can be consistently assembled from local
> scientific knowledge, and where the field fails to glue into a coherent
> global picture.

**Now**

- gaps, bridges, tensions, review queues, typed links, and dependency chains are
  early projection surfaces

**Future**

- explicit `ConstellationProjection`
- local scientific regions
- gluing rules
- obstructions
- action frontiers for runtime writeback

## What Vela v0 actually proves

Vela v0 proves a bounded claim:

- a frontier can be a durable, reviewable state kernel for scientific work
- finding bundles can act as the primary state object
- proposals and canonical events can act as the primary write primitive
- evidence, conditions, review, and proof can remain inspectable together
- proof packets can export a portable replayable snapshot

Vela v0 does **not** yet prove:

- full belief-state or credal semantics
- first-class contradiction objects
- first-class causal intervention semantics
- runtime protocol / experiment / result ownership
- networked propagation and institutional coordination
- formal constellation projection

State kernel now. Runtime, network, and constellations later.
