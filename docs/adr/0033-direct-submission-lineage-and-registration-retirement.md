# ADR 0033: Direct Submission lineage; Registration retirement

- Status: Accepted and implemented on `main`
- Proposed: 2026-08-01
- Protocol effect: removes `vela.registration-record.v1` and adopts
  `vela.repository.v4`
- Scientific effect: none; accepted Claims, Events, Verification Records, and
  Standing must remain byte-identical
- Authority effect: none after the one-time repository rewrite
- Compatibility: historical Git and pinned binaries preserve old records; the
  current runtime has no Registration reader

## Context

Every current Proposal already binds one exact producer-signed Submission, and
that Submission binds its Claim, artifacts, caveats, provenance, and requested
change. Every Verification Record binds the exact Proposal, Claim, Submission,
artifact, and verifier scope it evaluated. A Registration Record repeated a
subset of those links without its own signature, Event, Decision, or Standing
effect.

The four controlled Frontiers contain ten Registration Records. Each is
one-to-one with an exact Submission and Proposal, all route to
`pending_review`, all record zero accepted-state change, and none protects a
fact absent from the signed records. Keeping the object adds a schema, writer,
reader, directory, manifest list, projection table, UI step, fixtures, and
documentation without protecting a distinct invariant.

## Decision

The current evidence chain is:

```text
signed Submission -> Proposal -> Verification Record -> human Decision -> Event -> Standing
```

A producer may instead close its own still-pending branch without entering the
authority chain:

```text
signed Submission -> Proposal -> producer-signed Withdrawal
```

Withdrawal is append-only lifecycle evidence. It creates no Event and cannot
change accepted Standing.

`vela.repository.v4` removes the `registrations` collection. `vela submit`
writes the signed Submission, its proposed Claim and Proposal, and deterministic
projections in one routine evidence transaction. Idempotence locates the exact
Proposal through its `producer_package` binding. Strict replay verifies the
direct chain and rejects missing, duplicated, mismatched, malformed, or
unretained objects.

Registration Record code, paths, database projections, UI labels, fixtures,
and active documentation are deleted. No alias or compatibility parser remains
in the shipping runtime.

## Controlled rewrite

Because the repository manifest is replayed through Git history, the four
pre-release Frontiers received one bounded current-state rewrite. It:

1. verify the complete version-3 repository and all direct lineage first;
2. remove only Registration files and references;
3. update deterministic Target closures to bind the exact Proposal and
   Verification records directly;
4. preserve every Claim, Proposal, Submission, Verification Record, Artifact,
   Decision Event, and accepted-Standing reference byte-for-byte;
5. preserve the exact accepted Event and Standing roots;
6. bind the predecessor commit and resulting version-4 repository root; and
7. pass strict replay from a clean clone before the temporary rewrite tool is
   deleted.

This is one pre-release schema cut, not a permanent migration framework. Old
Git commits remain inspectable with their pinned old binary.

## Conformance

Tests must prove:

- submit emits no Registration Record and remains idempotent;
- Proposal-to-Submission-to-Claim lineage is exact and one-to-one;
- malformed or duplicated Proposal links fail closed;
- Verification still cannot change Standing;
- accepted Standing still requires an authorized human Decision;
- all four rewritten Frontiers preserve exact Claim, Event, and Standing
  roots; and
- current source, documentation, web projection, and database schema contain
  no Registration implementation.

## Rejected alternatives

### Keep the field but stop writing records

Rejected. It preserves the schema and reader burden while hiding dead state.

### Add another compatibility layer

Rejected. There are no external users. Historical Git and pinned releases are
the compatibility mechanism.

### Let a Proposal or Verification change Standing

Rejected. Removing a duplicate receipt does not weaken the human Decision
boundary.
