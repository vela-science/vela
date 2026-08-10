# ADR 0032: Self-authenticated evidence; human Decision authority

- Status: Accepted and implemented on `main`
- Target-surface disposition: any Vela Target Index or `next`/`start` language
  below is historical; commit `719cbc77` retired that core surface on 2026-08-10.
- Proposed: 2026-07-31
- Accepted: 2026-07-31
- Protocol effect: repository evidence coverage changes
- Authority effect: routine evidence no longer uses repository authority
- Product effect: ordinary agent work does not prompt for an authority signature

## Context

A Submission already carries and verifies the producer's whole-body Ed25519
signature. A Verification Record does the same for its verifier. Neither record
creates an Event or changes accepted Standing.

The prior runtime nevertheless required a second repository-authority SSH
signature for every Submission and Verification import. The resulting Authority
Record has no scientific Event; it exists because the storage verifier requires
every `records/**` path and every repository-manifest postimage to appear in the
repository-authority delta chain.

That countersignature does not protect a distinct scientific invariant. It
interrupts long-running work, exposes a more powerful credential to routine
intake code, and routes evidence through the same transaction vocabulary as a
human Decision.

## Decision

Separate the repository write model into three planes:

```text
evidence     producer/verifier-signed records and content-addressed artifacts
authority    human Decisions, policy, schema, membership, and authority changes
projection   repository.json, targets.json, review/search/status views
```

Routine Submission and Verification intake uses a bounded
`RoutineEvidenceTransaction` that:

1. verifies the producer or verifier signature over the complete record;
2. permits only new content-addressed evidence and deterministic projections;
3. rejects deletion, replacement, policy/schema/authority paths, Events, and
   accepted-Standing changes, while permitting one exact pending-Claim
   projection removal only when paired with a valid appended producer
   Withdrawal;
4. requires Event, authority, and accepted-Standing roots to remain identical;
5. uses the existing repository write barrier and exact Git publication path;
6. never loads a repository-authority or human key.

Human Decision, repository initialization, policy/schema changes, and authority
administration continue to use the repository-authority transaction. An agent
cannot accept or reject a Proposal. The producer that signed a Submission may
withdraw only its own still-pending Proposal by signing a
`vela.proposal-withdrawal.v1` lifecycle record with that exact key. This uses
the routine evidence transaction and changes no accepted Claim, Event, or
authority state.

## Strict replay

Strict verification must establish both planes without conflating them:

- verify every Submission, Verification, and Proposal Withdrawal signature;
- verify content-addressed object paths and Artifact bytes;
- validate Claim, Proposal, Submission, Verification, and Withdrawal links
  directly;
- replay signed Decision Events to derive accepted Standing;
- rebuild deterministic projections and compare their exact bytes;
- require evidence referenced by an accepted Decision to remain present; and
- require post-origin evidence history to be append-only through Git
  ancestry.

Git commit ancestry and compare-and-swap ref updates provide publication
lineage. Vela does not add another attestation format or evidence signer.

## Adoption

No Frontier migration or administrative signature is required. Existing
Authority Records remain valid checkpoints over their exact historical
postimages. The current verifier accepts later self-authenticated evidence only
when Git ancestry proves an append-only overlay from the last signed
checkpoint. The next human Decision binds that exact overlay as its preimage
and creates the next signed checkpoint.

Fresh authority policy contains only human Decision and repository
administration actions. Historical policy bundles remain immutable but no
longer govern routine evidence intake.

## Conformance

The change is complete only when tests prove:

- Submission and Verification imports succeed with no repository key loaded;
- their actor signatures remain mandatory;
- evidence writes leave Event and accepted-Standing roots unchanged;
- Verification `pass` never becomes acceptance;
- producer withdrawal requires the exact retained Submission identity, closes
  only a pending Proposal, and leaves accepted Standing byte-identical;
- routine writers cannot write Events, Decisions, policy, schema, membership,
  authority state, or accepted Claims;
- new evidence is content-addressed and idempotent while collisions, rewrites,
  and deletions fail;
- concurrent evidence writes serialize and stale writers lose cleanly;
- a concurrent Decision invalidates stale evidence and vice versa before
  publication;
- clean clones rebuild identical repository and Target projections;
- missing evidence referenced by accepted Standing fails replay; and
- the four current Frontiers preserve exact accepted Claims and Event roots.

The integrated disposable-Frontier test performs unsigned Submission and
Verification intake, proves that authority Event and Record stores remain
byte-identical, applies a later human rejection, publishes the Decision as one
exact local Git commit, and verifies a clean clone. The current binary also
strictly replays Erdős, Formal Conjectures, Quantum Codes, and Sidon without
changing any Frontier byte.

## Consequences

Ordinary agents can work for hours in their native execution environment after
reading one exact Target briefing, without repeated repository-authority
prompts or a Vela-owned lease. The Decision boundary becomes clearer: producer
and verifier signatures authenticate evidence; only an authorized human
Decision changes Standing.

The implementation retains one crash-recovery journal and Git compare-and-swap
publication because they protect concrete durability and concurrency
invariants. Human Decisions now publish their exact local Git commit in the
same command instead of leaving a dirty, unverifiable Target Index.

## Rejected alternatives

### Cache or daemonize the repository key

Rejected. It hides the prompt while retaining the wrong semantics and exposes a
high-power credential to routine evidence intake.

### Let verification update Standing

Rejected. Evidence and verdict remain separate even when a verifier passes.

### Build another runner or workflow engine

Rejected. Native agents and tools own execution. Vela owns the evidence,
Decision, replay, and Standing boundary.
