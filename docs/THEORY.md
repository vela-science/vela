# Scientific state theory

Vela is built on a narrow but ambitious claim:

> Science is not fundamentally a literature system. It is a system for
> maintaining and improving shared state about the world.

The literature is one projection of that process. It is not the process itself.
Papers, notes, benchmarks, lab logs, and agent traces are source artifacts. The
hard infrastructure question is what durable state survives after those
artifacts are read, challenged, corrected, and reused.

This document names the broader theory behind Vela without changing the current
v0 claim surface.

Publicly, Vela is a scientific state layer: it turns artifacts into typed
frontier state that can be inspected, corrected, replayed, served, and exported.
It should not lead with category theory, sheaves, Byzantine fault tolerance, or
other formal machinery.

Internally, the sharper identity is:

> Vela is a compiler from scientific artifact-space to scientific state-space.

That means papers, datasets, protocols, figures, benchmarks, notebook entries,
clinical observations, simulation outputs, failed experiments, and agent traces
are inputs. The output is not prose. The output is bounded frontier state and
typed state transitions over that state.

## Core claim

Science is a partially observed dynamical system whose progress depends on
whether findings, evidence, corrections, and experiments become durable shared
state transitions that can compound across time, institutions, and agents.

That yields three levels:

1. **Latent reality** — the causal world science is trying to model.
2. **Scientific state** — the current bounded answer to what a field believes,
   under what conditions, why, and what would change its mind.
3. **Scientific activity** — papers, reviews, experiments, notebook entries,
   benchmarks, lab logs, and agent traces that may update that state.

Vela focuses on level 2. It turns activity into portable, correctable frontier
state without pretending that source artifacts are truth by default.

## Primitive objects

The theory has two primary primitives.

### 1. finding bundle

The durable unit is not the paper. It is the finding bundle:

- assertion
- evidence
- provenance
- conditions
- confidence
- typed links
- review history

A claim is text inside the finding. The finding bundle is the state object.

### 2. state transition

Science advances when the field changes its mind in a traceable way. A state
transition records:

- what was believed
- what evidence arrived
- what changed
- what downstream findings now need review

In Vela v0, review, caveat, revise, reject, retract, and finding-add flows are
all state transitions over frontier state.

Canonical events are the protocol form of those state transitions. They are the
durable change primitive in the current repo.

The broader target is a typed transition language where support, contradiction,
refinement, weakening, retraction, scope change, and projection change are
explicit effects. The v0 protocol realizes the narrow write path first:
proposal -> accepted canonical event -> reducer -> replayable frontier state.
See [State Transition Spec](STATE_TRANSITION_SPEC.md) for the non-normative
bridge from the current event protocol to that larger transition language.

## Contradictions

Scientific disagreement should remain live state.

Vela should not flatten every conflict into one scalar confidence score. A
field often needs to preserve the contradiction itself because it tells
reviewers what scope variable, method difference, model system, assay, or
replication would resolve the disagreement.

In v0, contradiction-preserving structure is represented through:

- typed `contradicts` links
- contested review state
- caveats and notes
- candidate tensions
- proposal and event history

That is intentionally modest. Later versions may promote contradictions into
first-class records with suspected resolution variables, required review or
experiment actions, and status transitions. The rule should stay the same:
scalar summaries may help humans scan, but protocol state should preserve why
the disagreement exists.

## The three-Layer theory

The full theory has three layers:

1. **State**
   - findings
   - evidence
   - provenance
   - conditions
   - typed links
   - canonical events

2. **Runtime**
   - protocols
   - experiments
   - trials
   - interventions
   - measurements
   - writeback

3. **Network**
   - propagation
   - interoperability
   - attribution
   - federation
   - institutional memory
   - coordination

Vela v0 realizes the first layer directly. The second and third layers are the
theory's continuation, not the current release claim.

Runtime remains future-facing in v0. The reserved conceptual objects are:

- `ProtocolRecord`
- `ExperimentRecord`
- `ResultRecord`

They are not first-class protocol objects yet. Promotion into the public
protocol requires replay semantics, writeback semantics, proof/export semantics,
and review/merge semantics.

## Constellations

In the broader theory, a constellation is the navigable local-to-global
projection of scientific state.

It is not a new truth object. It is the visible map of:

- strong and weak findings
- contradiction structure
- bridges between regions
- frontier gaps
- dependency chains

Scientific fields are not one globally consistent map. They are overlapping
local maps: lab-level maps, organism-specific maps, method-specific maps,
clinical maps, mechanistic maps, and therapeutic maps. A constellation asks
what can be assembled from those local views and where the assembly fails.

That makes contradictions structural. They are not only red flags on a graph;
they are obstructions where local scientific views do not glue into one coherent
picture.

Constellations matter because a field becomes navigable only when its state can
be seen as terrain rather than reconstructed from scattered prose. The formal
analogy is sheaf-like, but the product obligation is plain: show what can be
consistently assembled from frontier state and where scope, method, evidence,
or translation failures block assembly.

Vela v0 already computes pieces of this map through tensions, gaps, bridges, and
review queues. A dedicated constellation interface remains a later surface.

## Continuity and coordination

The theory distinguishes continuity from coordination.

- **Continuity** is the ability for knowledge to survive noise, delay, turnover,
  and institutional boundaries.
- **Coordination** is what becomes possible once continuity is good enough.

Vela's current work is continuity infrastructure:

- evidence stays attached
- conditions stay visible
- corrections become replayable
- proof packets preserve review state

Runtime and network layers are where that continuity can later become
coordination.

## Mathematical backbone

The current mathematical framing is documented in
[Math and Principles](MATH.md). Vela v0 does not claim to implement that full
program. It implements the state kernel that such a program would need.

## What Vela proves today

The current repo aims to prove one bounded proposition:

> A replayable frontier is a better working memory for AI-accelerated science
> than papers, logs, notebooks, and private agent memory alone.

That means:

- candidate findings can be bootstrapped from bounded sources
- evidence and conditions remain inspectable
- review and correction events change inherited state
- proof packets preserve a portable reviewable snapshot
- humans and agents can inspect the same frontier through one substrate

## What remains future

The broader theory is larger than the current release.

Not yet realized as core public claims:

- first-class experiment and protocol objects
- runtime writeback from real scientific execution systems
- networked federation and institutional sharing
- a full constellation product surface
- full coordination / transaction layers above continuity

Those are legitimate future layers. They are not current v0 claims.

## Practical reading

If you only need the release boundary, read:

- [Core Doctrine](CORE_DOCTRINE.md)
- [Protocol](PROTOCOL.md)
- [Proof](PROOF.md)

If you need the larger conceptual frame, read this document as the theory note
behind those narrower release documents.
