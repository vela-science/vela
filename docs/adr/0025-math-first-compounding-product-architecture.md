# ADR 0025: Math-first compounding product architecture

- Status: Accepted
- Accepted: 2026-07-28
- Protocol effect: none
- Product effect: adopt the public loop
  `map -> target -> run -> verify -> commit -> compound`
- Authority effect: none
- Package effect: source-local research only until two maintained consumers
  and deleted duplication earn extraction
- Service effect: no Registry, Atlas, hosted authority, second writer, or
  canonical database is authorized
- Evidence: completed Erdős Submission, Verification, and human Decision loop;
  repaired registered Stage A; current four-Frontier replay

## Context

Vela's current protocol path accurately separates producer input, scoped
verification, authorized Decisions, canonical Events, and replayed Standing:

```text
Target
  -> Attempt
  -> Submission
  -> Registration Record
  -> Proposal
  -> Verification Record
  -> Decision
  -> Event
  -> Standing
```

That is the interoperability and authority contract. It is not the clearest
description of the user outcome. The former public sequence
`inspect -> attempt -> submit -> verify -> decide -> continue` foregrounded
review plumbing and did not name correction, inheritance, or reuse.

The math-first campaign has now produced two relevant facts:

1. one exact bounded Erdős result survived Submission, independent
   Verification, human Decision, strict replay, and clean history without an
   agent crossing the authority boundary; and
2. repaired registered Stage A produced 11 verifier-passing artifacts in 12
   matched cells, with Canopus passing 4/4 while using fewer observed tokens
   and less wall time per pass than both native baselines.

These facts justify a sharper product thesis and the next math-first tests.
They do not justify a hosted Registry, universal ontology, graph database,
package ecosystem, orchestration framework, or science-factory platform.

## Decision

### 1. Adopt one outcome-oriented product loop

```text
map -> target -> run -> verify -> commit -> compound
```

- `map` reads exact state, disagreement, dependencies, and gaps;
- `target` turns one gap into bounded work;
- `run` executes in any suitable human or machine environment;
- `verify` records one scoped check over exact inputs and explicit nonclaims;
- `commit` explains the authorized Decision, Event, and root transition; and
- `compound` makes the new state, correction, or failed route improve later
  work.

The protocol object names and daily commands remain unchanged. Product verbs
may summarize several exact objects but may not blur their authority effects.

### 2. Treat Frontier Commit as product language only

A Frontier Commit is:

```text
authorized Decision
+ canonical Event
+ exact before and after roots
+ replayed Standing
```

It is not a new schema, identifier, event kind, reducer transition, CLI writer,
or policy. A verifier pass, package publication, Git merge, database update,
or model confidence cannot create one.

### 3. Use mathematics as the first complete domain proving ground

Vela Math is a domain profile under test, not a second Kernel or permanent
domain limit. The first slice must preserve separate answers to:

```text
Does the formal artifact check?
Does the formal statement match the intended source Claim?
What has a named Frontier decided?
```

Lean, Lake, Mathlib, Formal Conjectures, OEIS, LMFDB, GitHub, journals, and
other native systems retain their identifiers, package resolution, checking,
and community processes.

### 4. Keep four planes separate

```text
activity          Runs, traces, branches, attempts, raw artifacts
scientific state  Claims, checks, Decisions, Events, Standing
package           optional reusable language and capability
discovery         disposable maps, search, rankings, and explanations
```

Vela owns the scientific-state transition. Workbenches own activity. Packages
confer no Standing. Discovery views bind exact roots and remain rebuildable.
No inner execution or campaign loop gains ambient authority over the slower
state loop.

### 5. Test portable correction-aware transitions as the protocol breakthrough

The protocol-scale hypothesis is:

```text
exact prior Frontier state
+ portable proposed transition
+ scoped checks
+ local authorized Decision
-> deterministic resulting state
-> deterministic bounded correction consequences
```

Vela already demonstrates the single-Frontier admission path. It has not yet
demonstrated a clean-room implementation that derives the same correction
impact, preserves an independent support route, opens the same repair
Obligation, and imports the source transition into a second independently
governed Frontier.

Historical Finding-era propagation is not promoted. It conflates support and
dependency, has an arbitrary depth cap, includes scalar-confidence behavior,
and cannot model alternative-route survival. History using that implementation
remains replayable while a real fixture determines the minimum future
semantics.

The first experiment uses a derived transition and scientific-diff projection
over existing rooted objects. A new Frontier Commit schema, dependency algebra,
correction Event, resolver, or federation service requires a reproduced gap
that the projection cannot close.

### 6. Make Obligation a product primitive before a protocol primitive

An Obligation names one unresolved requirement needed to assess, establish,
transfer, repair, or use a Claim. Existing Claims, evidence, Target Index
facts, and projections may expose Obligations now.

This ADR adds no canonical Obligation writer. A protocol object is considered
only after two real domains require the same missing transition or replay
semantics.

### 7. Earn packages before a Registry

The permitted sequence is:

```text
source-local profile
-> exact lock
-> two maintained consumers
-> deleted maintained duplication
-> read-only package index
-> federated discovery
```

A candidate Vela Math profile may map Problem, Claim, Formalization, Result,
Obligation, and statement-fidelity review concepts source-locally. It may not
replace native packages or become a Kernel dependency.

Shared extraction requires deterministic offline generation, exact roots,
native identifier preservation, explicit loss, no replay effect, and deletion
of maintained duplication. This preserves ADR 0019's rejected shared-package
disposition until new evidence satisfies its reopening gate.

### 8. Build correction and inheritance before platform surfaces

The next state-lift tests are:

1. one named Formal Conjectures vertical slice;
2. one exact correction cascade through a dependent Claim and repair
   Obligation;
3. one scientific state diff tested against Git plus the same evidence; and
4. one cold successor test measuring time to correct next action.

A read surface must reduce evidence-location, correction, or continuation time
by at least 20 percent before becoming product work. A package, adapter, or
framework must meet its own registered adoption gate.

### 9. Require independent conformance before protocol expansion

The current `PROTOCOL.md`, public authority-free TypeScript contracts, and
conformance vectors remain the normative specification. Do not create a parallel Vela Protocol Specification
series until a clean-room implementer shows the current documents cannot state
the required boundary clearly.

The independent reader must not import Rust or private implementation state.
It must agree on canonical bytes, transition validity, authority
non-escalation, resulting roots, correction consequences, truncation, and
failure cases. Colocated TypeScript tests are useful implementation diversity;
they are not external independence.

Formal proof returns only for a current invariant that protects or enables the
real fixture and is not adequately covered by finite conformance. Candidate
properties include Standing noninterference, correction no-invention,
independent-route survival, and foreign-import authority non-escalation.

### 10. Keep the public topology small

Current owners remain:

```text
vela        Kernel, CLI, protocol contracts, removable Agent candidate, conformance
vela-web    editorial site and read-only products
Frontiers   bounded canonical state and authority
.github     organization policy and reusable workflows
```

Historical Canopus Runs remain reproducible, while current execution is exposed
only through `vela agent` and its private removable helper under ADR 0031.
Registry, Atlas, Navigator, Actions, Spaces, Collections, Campaigns, and
Control Room are descriptive surfaces, not repositories, brands, or scheduled
services.

## Rejected alternatives

- **Rename protocol objects to match the product loop.** Rejected because the
  current object lifecycle is the precise interoperability contract.
- **Add a Frontier Commit object.** Rejected because the existing authorized
  transition already contains the required exact evidence.
- **Standardize the historical correction cascade.** Rejected because its
  bounded BFS and record mutation do not preserve typed alternative routes.
- **Promote Obligation immediately into the Kernel.** Rejected because current
  product and projection facts can test its usefulness first.
- **Publish `@vela/math` immediately.** Rejected because ADR 0019 found no
  maintained duplication to delete.
- **Build the Registry or Atlas now.** Rejected because reusable packages and
  cross-Frontier recurrence have not earned them.
- **Encode the ecosystem in Writ now.** Rejected because a DSL would add a
  compiler and generated contract before a repeated authoring gap exists.
- **Create six new protocol-specification documents now.** Rejected because the
  current protocol, schemas, and fixtures must first be tested by a clean-room
  implementer.
- **Make Canopus, an orchestrator, a verifier, or a database the state loop.**
  Rejected because activity, checking, projection, and authority remain
  separate.

## Consequences

- Public product language emphasizes scientific inheritance and reuse without
  changing current bytes or commands.
- The active campaign now has a coherent order from real math work to
  correction, package extraction, external transfer, and only then read-only
  network products.
- The long-range architecture remains visible but falsifiable. Every new layer
  has an evidence gate and deletion rule.
- No Vela release is required for this documentation and product-architecture
  decision.
