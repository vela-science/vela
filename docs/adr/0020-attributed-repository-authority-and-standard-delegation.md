# ADR 0020: Attributed repository authority

- Status: Accepted; partially superseded
- Accepted: 2026-07-31
- Protocol effect: repository authority records, exact principals, restricted
  policy evaluation, and transaction-bound Decision Events
- Scientific effect: none; the authority mechanism records who may change
  Standing but does not supply scientific evidence
- Superseded scope:
  - ADR 0027 replaces the migration-era and dual-reader design;
  - ADR 0031 removes capabilities, the Vela-owned runner, Campaign host, and
    private Run receipts; and
  - ADR 0032 removes repository countersignatures from routine Submission and
    Verification intake; and
  - ADR 0037 replaces per-signature SSH confirmation with session-authenticated
    local repository authority.

## Context

Vela's durable authority invariant is small:

```text
evidence is not a verdict
verification is not acceptance
only an authorized Decision changes Standing
the exact transition is replayable
corrections append
```

The predecessor design protected that invariant with personal Vela signing
keys, custom key-custody helpers, several policy generations, and manual
copying of transaction roots. Those mechanisms made a person operate internal
transaction plumbing without providing the lifecycle and recovery properties
of a mature identity system.

Vela instead needs portable process authority: authenticate an exact
principal, authorize one explicit action under restricted policy, bind the
current read set and intended consequence, and record the complete transition.

## Decision retained

### Scientific-state kernel

Retain:

- canonical bytes and content-derived identities;
- producer-signed Submissions and verifier-signed Verification Records;
- Proposals, append-only Decision Events, corrections, and deterministic
  Standing replay;
- exact intent and read-set binding;
- human judgment for scientific Decisions; and
- structural prevention of agent self-approval.

Git remains publication transport, lineage, and distributed backup. Evidence,
verification, policy authorization, publication, and scientific Standing are
separate facts.

### Repository authority transaction

A Decision or repository-administration change is recorded by one exact
repository-authority transaction. Its authority record binds:

- operation, transaction, and prior authority-record identity;
- before and after Event roots and generated Event IDs;
- the complete canonical object delta;
- authenticated principal and attribution snapshot;
- restricted Cedar request, determining policy, outcome, and diagnostics;
- semantic approval when the consequence requires it;
- final read set and execution identity; and
- repository-authority key identity and signature.

The repository-authority signature means that the exact transaction passed
the recorded authentication, authorization, approval, verification, and
final-state checks. It does not turn the repository authority into a scientific
author and does not make evidence true.

Authority records use DSSE so the signature binds both the payload type and
the canonical authority-record bytes. The Frontier authority keyset supplies
the verification history. Vela does not invent a second signature envelope.

### Principals and policy

Principal IDs are exact namespaced issuer-subject bindings. Display names,
email addresses, affiliations, GitHub handles, and unlinked ORCID values do not
confer identity or authority. Non-human principals are explicit and cannot
perform a human scientific Decision.

The policy engine is restricted and deny-by-default. Unknown actions,
diagnostics, stale inputs, mismatched resources, and incomplete approval data
fail closed. A final read-set check occurs under the write barrier before the
authority record is signed or published.

The repository-authority key represents a governed repository role, not a
person. Its provider may be a local SSH agent today and may later be backed by
a mature institutional key service without changing Vela's authority
semantics.

## Current product boundary

Native tools perform scientific work and emit authenticated evidence. Vela
does not own their runner, scheduler, transcript store, or orchestration graph.

The read-only Decision Inbox projects real pending Proposals. Each entry shows
the exact proposed Claim, evidence and Verification set, current authority
heads, semantic change to Standing, staleness, blockers, and next obligation.
It has no independent lifecycle and grants no authority.

Routine work does not interrupt a person merely to reproduce internal roots or
countersign evidence that already authenticates its producer. ADR 0032 defines
the implemented evidence/authority split: producer and verifier evidence uses
the routine append-only path, while only human Decisions and rare repository
administration use repository authority.

This follows established system boundaries rather than introducing a Vela
workflow runtime:

- [DSSE](https://github.com/secure-systems-lab/dsse/blob/master/protocol.md)
  supplies the signed-envelope primitive;
- [in-toto](https://github.com/in-toto/docs/blob/master/in-toto-spec.md)
  demonstrates that signed process attestations are policy inputs rather than
  an application's semantic verdict;
- [OpenAI Agents SDK HITL](https://openai.github.io/openai-agents-python/human_in_the_loop/)
  demonstrates action-class approval and durable pause/resume in the native
  runner; and
- Git compare-and-swap publication supplies repository concurrency and
  ancestry rather than another Vela coordination service.

## Superseded portions

This ADR no longer specifies:

- capability grants or delegated agent authority;
- a Vela runner, Campaign host, controller, signer cache, or Run receipt;
- an authority-countersigned routine-evidence path;
- migration-era readers or current-binary compatibility for predecessor data;
- a separate staged-review or batch-planner object model; or
- a native Campaign Cockpit over private execution state.

ADR 0027 records the current-only repository boundary. ADR 0031 records the
native-tool execution boundary. The active campaign document owns product
evaluation and fresh-user comprehension gates rather than using them to keep
this architectural decision perpetually Proposed.

## Evidence and audit history

The attributed-authority implementation was exercised through fresh
repository initialization and real accept/reject paths. It repaired the
Decision boundary without allowing verification to update Standing or an
agent to decide its own Proposal.

The former 1,941-line version mixed this durable rationale with implementation
diaries, retired migration plans, removed capabilities, a deleted runner, and
unimplemented UI specifications. That exact record remains recoverable from
Git blob `10e310ca76cb9a0ede1621fbc1861c8eadd3dd12` (file SHA-256
`cc20465364cbed9bab234cef4fa9b44c72698e0869218a40df02c6c1094b5afd`).
It is not duplicated under another active or historical path.

## Rejected alternatives

### Make GitHub, a database, or an identity provider scientific authority

Rejected. Those systems may authenticate a principal or publish bytes; they
do not evaluate the scientific consequence or derive Vela Standing.

### Let a verifier accept its own result

Rejected. Verification is evidence. Only an authorized Decision changes
Standing.

### Keep personal-signing and copied-root ceremonies

Rejected. People decide semantic consequences; deterministic transaction code
binds exact roots and postimages.

### Cache a high-power repository key for routine evidence work

Rejected. Hiding prompts would preserve the wrong semantic boundary and expose
an authority credential to ordinary evidence intake. ADR 0037 separately
permits a dedicated repository service key during an explicitly authorized
local authority session; it does not make that key part of producer work.

### Build a Vela runner to make approval state durable

Rejected. Native agent systems already own durable execution and tool-action
approval. Vela records evidence and the consequential Decision boundary.
