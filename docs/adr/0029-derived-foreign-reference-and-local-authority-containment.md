# ADR 0029: Derived foreign reference and local authority containment

- Status: Proposed
- Proposed: 2026-07-29
- Protocol effect: none while Proposed
- Product effect: experiment only
- Authority effect: none
- Service effect: none
- Builds on:
  [ADR 0026](0026-correction-benchmark-and-whitepaper-evidence-contract.md)
  and the B8 failure in the
  [protocol breakthrough benchmark](../BREAKTHROUGH_BENCHMARK.md)

## Context

Benchmark family B8 asks whether a second Frontier can retain and check one
exact accepted transition from a source Frontier without treating the source
Decision as local authority.

The frozen source audit reproduced a narrow gap. Current migration-lineage
fields do not bind the source Frontier, repository root, Decision, authority
record, completeness, or explicit absence of local authority. Rust and
clean-room Python readers agree on that inventory. Reinterpreting migration
fields would be an unsafe semantic rebind.

The real Erdős 424 source correction is now terminal, verified, accepted, and
clean-clone replayed. The experiment is therefore eligible to test the
smallest response to the reproduced gap. It is not eligible to add a
federation service, global identifier system, resolver, Registry, second
writer, distributed transaction, or imported authority.

## Decision proposed

### 1. Begin with a derived envelope

Define non-protocol `vela.foreign-reference.v1` in the replaceable
`vela-edge` reader layer. The envelope is a canonical manifest over exact
source bytes. It binds:

- source Frontier ID;
- source Git commit and tree;
- source repository root;
- accepted Claim ID and root;
- Proposal ID and root;
- Decision Event ID and root;
- source authority-record ID and root;
- the exact retained object set and its canonical root;
- completeness and every missing required role;
- source Standing;
- local Standing effect `none`;
- the requirement for a separate local Decision; and
- explicit nonclaims.

The minimum complete object set is:

```text
repository manifest
Claim
Submission
Proposal
Verification
Decision Event
applied semantic Event
authority record
```

Every object path is relative and every root is a full lowercase SHA-256.
Each entry separately binds its semantic object root and raw byte root; these
are equal for canonical JSON records but differ for a signed DSSE authority
envelope whose payload has its own root. Objects are sorted and unique by role,
ID, semantic root, byte root, and path. Package verification rehashes every
retained byte. Missing inputs produce an explicitly incomplete assessment;
substitution, shortened roots, path escape, object-set drift,
source-binding drift, or authority escalation fail closed.

### 2. Keep the result non-authoritative

The full canonical envelope root is its identity; there is no parallel short
ID. `vela.foreign-reference-assessment.v1` reports source identity, source
Standing, completeness, and local Standing effect. It cannot:

- write a Frontier;
- create a Claim, Proposal, Verification, Decision, or Event;
- change Standing;
- infer local acceptance from source acceptance;
- resolve a mutable URL; or
- fetch from a hosted Vela service.

A receiving Frontier may retain the envelope only through the ordinary
producer path as evidence for a bounded local Claim. Registration must remain
`pending_review` with accepted-event delta zero. Any later local acceptance,
rejection, narrowing, or supersession uses that Frontier's ordinary authority
boundary.

### 3. Qualify two readers before real retention

The Rust edge reader and dependency-free Python clean-room reader consume the
same language-neutral fixture and must emit byte-identical assessments. The
fixture includes an explicit authority-escalation adversary. This establishes
implementation qualification only, not organizational independence or B8.

The real envelope is materialized only after the four current-state
compactions succeed. This avoids binding new evidence to a predecessor
repository root that the current product is about to retire and avoids
invalidating the already authorized compaction candidates.

### 4. Use one first-party second-Frontier experiment

After compaction:

1. materialize the exact Erdős 424 accepted correction package from the
   compacted Erdős repository;
2. require Rust and clean-room assessment agreement;
3. register the envelope as a bounded producer Artifact in the Formal
   Conjectures Frontier, because that Frontier owns the referenced Lean source;
4. require `pending_review`, accepted-event delta zero, strict replay, and a
   clean clone;
5. import a scoped Verification that checks every retained byte and the source
   repository authority chain;
6. confirm that neither registration nor Verification changes local accepted
   Standing; and
7. leave the resulting local Proposal pending unless a human independently
   chooses to decide it.

The two repositories currently share one operator. This experiment is
first-party protocol qualification and cannot earn external-governance or
adoption credit.

### 5. Delete or promote by evidence

Reject this ADR and delete the envelope if:

- two readers cannot agree;
- exact source bytes are insufficient to verify the attributed transition;
- retention requires a hosted service or second writer;
- the receiver cannot distinguish source Standing from local Standing; or
- the representation adds no measurable transfer or inheritance value over a
  plain rooted manifest.

Accepting this ADR after a successful first-party experiment still does not
make the envelope a canonical protocol object. Promotion requires a held-out
second case and an independently implemented or governed consumer that
reproduces the same missing contract.

## Acceptance gate

Accept only after:

1. the four current-state compactions and clean-clone replays pass;
2. the real source package binds the compacted Erdős root;
3. Rust and clean-room readers agree on the real assessment;
4. Formal retains the envelope through the ordinary producer path;
5. accepted-event delta remains zero through registration and Verification;
6. source and receiver replay without Neon, Observatory, Canopus, or another
   hosted service; and
7. the result and its limits are added to the public artifact package.

## Rejected alternatives

- **Reuse `imported_from`.** Rejected because it is predecessor migration
  lineage and omits the required source and authority bindings.
- **Add `vela federation import`.** Rejected because the derived experiment
  has not earned a public writer surface.
- **Put the envelope in Neon or the Observatory.** Rejected because both are
  disposable read projections.
- **Import source acceptance.** Rejected because authority is local to the
  receiving Frontier.
- **Coordinate both repositories transactionally.** Rejected because B8
  requires independent local transactions, not distributed atomicity.
- **Create a global registry or resolver.** Rejected because exact Git and
  retained bytes are sufficient for the experiment.

## Consequences

- B8 changes from an unimplemented gap to a bounded, falsifiable experiment.
- The current public protocol and CLI remain unchanged.
- Actual second-Frontier retention waits for the already authorized
  compaction boundary.
- Failure removes code rather than expanding architecture.
