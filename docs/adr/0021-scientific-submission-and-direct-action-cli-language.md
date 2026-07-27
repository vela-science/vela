# ADR 0021: Scientific Submission and direct-action CLI language

- Status: Proposed
- Target release: Vela `v0.940.0`
- Protocol effect: one current producer era with Submission, Registration
  Record, Verification Record, and optionally Claim Record objects; historical
  Receipt-era bytes remain replayable
- Product effect: replace inverted producer and review language with
  `inspect -> attempt -> submit -> verify -> decide -> continue`
- Authority effect: none; the existing repository-authority transaction remains
  the only path that may change scientific standing
- Compatibility: read every retained era that matters; write exactly one
  current era; never rewrite canonical history
- Entry gate: the July 26 product-language and command-contract audit plus the
  completed Vela and Canopus ownership contraction

## Context

Vela already separates producer activity, retained evidence, scoped
verification, candidate transitions, authorized decisions, append-only events,
and deterministic replay. Its current vocabulary hides those separations:

- a producer creates a `Receipt`, although a receipt normally follows
  registration;
- a producer `land`s evidence, although the ordinary result is a pending
  Proposal with no accepted-state change;
- `work` names a command, session, object, and general activity;
- every candidate scientific assertion is a `Finding`, even when pending or
  rejected;
- a verifier emits an `attachment`, which names storage rather than meaning;
- consequential actions sit behind `review decide --accept|--reject`; and
- Canopus runs and submits as one operation unless explicitly told not to land.

These are authority defects in the product contract, not cosmetic wording
issues. A command verb tells a user what power is being exercised. A protocol
noun tells an external producer what object it is creating.

The public cycle is:

```text
inspect -> attempt -> submit -> verify -> decide -> continue
```

The canonical transition is:

```text
Target
  -> Attempt
  -> Submission
  -> Registration Record
  -> Proposal
  -> Verification Record(s)
  -> Decision
  -> Event
  -> replayed Standing
```

The architecture mnemonic remains:

```text
produce -> preserve -> check -> decide -> reuse
```

The first sequence describes user actions. The second describes ownership.

## Decision

### 1. Adopt one current product vocabulary

Keep:

```text
Frontier
Target
Offer
Artifact
Evidence
Proposal
Decision
Event
Standing
Canopus
Observatory
```

Adopt for current writers and readers:

| Historical/current term | Current term |
| --- | --- |
| Receipt v1 | Submission v1 |
| Activity or landing record | Registration Record |
| VerifierAttachment | Verification Record |
| work session | Attempt |
| Decision Brief | Review Packet |
| Finding | Claim Record; `finding` becomes an editorial label for a positively standing Claim |

The term `Receipt` is reserved for a future Vela-issued, independently
verifiable registration or inclusion proof. This ADR does not add that object.

### 2. Adopt one daily CLI

Default help exposes:

```text
init status next start submit show why review check reproduce log doctor
```

The direct replacements are:

```text
work                         -> start
land                         -> submit
review preview               -> review diff
review decide --accept       -> review accept
review decide --reject       -> review reject
review withdraw              -> proposal withdraw
verify attach                -> verification import
finding show                 -> show or claim show
```

Retired writable commands are not aliases. During one bounded migration window
they exit with code 2, state the exact replacement, and make no write.

`show` accepts any current or historical typed Vela ID and reports source era,
exact root, related objects, and authority effect.

`why` is a derived, root-bound explanation of current standing. It reports the
applicable Submission, Artifacts, Verification Records, Proposal, Decision,
Event, corrections, caveats, and exact reproduction or continuation command.
Deleting an explanation changes no canonical state.

### 3. Add one current producer era

The current portable producer object is:

```text
vela.submission.v1
vsb_
```

It binds a Claim, conditions, scope, caveats, Artifacts, evidence relations,
replayability, producer provenance, producer-reported checks, verification
requirements, requested change, optional exact execution binding, and
whole-body authentication.

Producer input may request a change. It may not assert standing, create
authority, manufacture an independent verification result, or contain a
canonical Event.

Vela records successful intake as:

```text
vela.registration-record.v1
vrr_
```

The record binds the Submission root, Frontier, operation and transaction,
registered Artifacts, resulting Claim and Proposal, route, before and after
intake roots, and Vela execution identity. It proves registration, not truth,
independent verification, or acceptance.

Scoped verifier output becomes:

```text
vela.verification-record.v1
vvr_
```

It binds the exact Claim, Submission, Proposal, Artifacts, method,
implementation, environment, property checked, outcome, limitations,
independence disclosure, and authentication. A passing Verification Record
changes no standing by itself.

### 4. Gate Claim Record separately

The target current claim primitive is:

```text
vela.claim-record.v1
vcl_
```

Historical `vf_` Finding bytes and IDs are never rewritten. New Claim Records
may be enabled only after cross-era relations, correction, supersession,
Observatory projection, and clean-clone replay pass focused conformance.

If that gate does not pass in the Submission release, Vela ships the
Submission/Registration/Verification vocabulary first and continues to read
historical Findings explicitly as historical claim records. It must not
manufacture new `vcl_` identities by relabeling old bytes.

### 5. Make authority effects explicit

Objects have these effects:

| Object | May change accepted scientific state? |
| --- | ---: |
| Attempt | No |
| Run | No |
| Artifact | No |
| Submission | No |
| Registration Record | No |
| Proposal | No |
| Verification Record | No |
| Review Packet | No |
| authorized Decision | Defines the exact transition intent |
| canonical Event | Records and replays the authorized effect |

Successful `submit` output always includes:

```text
accepted_state_changed: true | false
```

The ordinary route reads:

```text
Submission registered; review required.
Accepted scientific state changed: no.
```

Internal `Permit`, `Defer`, and `Deny` policy enums remain replayable where
canonical. Current human output says `accepted by signed policy`, `pending
review`, or `not registered`, with the exact responsible policy or reason.

### 6. Separate Canopus run, export, and submit

Canopus remains a replaceable producer. Its current daily surface becomes:

```text
doctor run show replay export submit
```

`run` is non-mutating by default. `export` writes a portable Submission.
`submit` explicitly delegates the exact Submission to released Vela.
`run --submit` may compose the two operations only when that effect is visible
in the command, profile, Run Record, and result.

Canopus does not implement registration, proposals, decisions, replay, or
authority. Deleting Canopus after registration leaves Frontier replay
unchanged.

### 7. Make the Observatory object- and explanation-first

The read model uses:

```text
Frontier
Problem
Claim
Submission
Proposal
Verification
Decision
Artifact
Run
```

Historical objects are projected under current labels only with their source
schema, source identifier, source root, transformation version, and any loss
visible.

The Claim page centers `Why this stands`, showing the shortest exact route
from evidence and decisions to current Standing. No bare `verified` badge may
imply acceptance. The Observatory remains credential-free and read-only.

### 8. Preserve eras; remove duplicate current writers

The compatibility rule is:

> Read every retained era that matters. Write exactly one current era. Never
> rewrite canonical history to make vocabulary cleaner.

Historical Receipt, `vf_`, `vrc_`, and `vva_` bytes remain valid under the
versions that created them. Current readers expose them honestly. Current
writers emit only the new era after migration.

Legacy import, if a named consumer needs it, is explicit and loss-aware:

```text
vela submission import-legacy <receipt.json>
```

It retains the original bytes and root, validates the historical schema,
reports the transformation, and follows the ordinary current Proposal path.
No silent normalization is allowed.

## Repository ownership

| Repository | Owns |
| --- | --- |
| `vela-science/vela` | current and historical schemas, canonicalization, intake, verification records, proposals, authority, replay, CLI, compatibility, conformance |
| `vela-science/vela-research-harness` | Missions, isolation, Runs, Artifacts, independent verifier execution, Submission export, delegation to Vela |
| `vela-science/vela-web` | read-only current and historical projections, explanations, comparison, reproduction and continuation |
| private integration repository | exact composition, migration evidence, adoption tests, retirement decisions |
| standalone Frontiers | bounded canonical scientific and authority history, domain Targets, packets, profiles, verifiers and corrections |

This decision creates no language repository, ontology service, registry,
adapter registry, hosted authority, database, or new product.

## Migration sequence

### Stage 0: contract and inventory

- accept this ADR direction as Proposed;
- publish `TERMINOLOGY.md` and the current command/schema inventory;
- classify actual consumers;
- freeze old writer releases and cross-era fixtures;
- decide the Claim Record gate independently.

### Stage 1: Vela current writer

- implement closed Submission, Registration Record, and Verification Record
  schemas and hostile vectors;
- implement `start`, `submit`, direct review verbs, `proposal withdraw`,
  `verification import`, universal `show`, and derived `why`;
- preserve the recoverable repository-authority transaction and all authority
  invariants;
- emit one current JSON vocabulary;
- provide diagnostics, not aliases, for retired writers.

### Stage 2: Canopus

- make `run` non-mutating;
- add `show`, `export`, and explicit `submit`;
- emit Submission v1;
- remove current landing language and writer coupling;
- prove the same Submission can come from a minimal second emitter.

### Stage 3: Observatory

- add current object pages and `Why this stands`;
- project both eras with source disclosure;
- remove ambiguous verification and landing language;
- preserve a rebuildable, read-only projection.

### Stage 4: real Frontiers

- do not rewrite historical bytes;
- use current writers for the next real contribution;
- demonstrate one pending Submission, one terminal rejection, and one
  correction across the four maintenance/expansion roles;
- require clean-clone replay.

### Stage 5: recurrence and retirement

- qualify a second producer and an independent reader;
- retire old writers and active landing terminology;
- move Receipt documentation to compatibility history;
- publish the compatibility matrix and source-recovery drill.

## Conformance

Required focused contracts include:

1. a valid Submission can remain pending or be rejected;
2. successful `submit` does not imply accepted-state change;
3. a Registration Record can exist without independent verification;
4. producer checks cannot be relabeled as Verification Records;
5. passing Verification Records change no standing;
6. Decisions bind the exact current Proposal and authority preimage;
7. corrections preserve every earlier canonical byte;
8. `why` binds the exact state root and includes applicable rejection,
   correction, and supersession;
9. removing Canopus or Web leaves replay unchanged;
10. historical public Frontiers replay byte-for-byte.

Hostile vectors include producer-asserted acceptance, mismatched Claim or
Artifact roots, forged Registration Records, verifier scope substitution,
stale Proposals, policy root substitution, hidden shared verifier lineage,
silent legacy normalization, omitted corrections, browser authority requests,
and retraction by deletion.

## Acceptance gates

This ADR becomes Accepted only after:

- canonical and hostile vectors pass for every current object;
- all retained public Frontiers replay unchanged;
- one current Frontier completes `start -> submit -> verify -> review ->
  replay -> why -> correction`;
- Canopus and a minimal second producer emit equivalent valid Submissions;
- the Observatory projects both eras without losing source identity;
- preregistered fresh users distinguish submission, verification, and
  acceptance at the required threshold;
- no agent, producer, reader, or browser reaches authority or a human key; and
- provider-exit and clean-source recovery drills pass.

## Consequences

The migration is intentionally breaking before 1.0. It removes semantic
inversions at the producer boundary, makes authority actions explicit, creates
a credible external Submission contract, and gives Vela a defining `why`
surface.

The cost is dual-era replay and coordinated migration across Vela, Canopus,
Web, and active Frontiers. That cost is lower now than after external adoption.
It does not justify rewriting history, preserving permanent writable aliases,
or creating another layer.

## Rejected alternatives

- Keep Receipt and land: rejected because both invert ordinary meaning.
- Rename only CLI porcelain: rejected because external producers still target
  the wrong object.
- Rename only the object: rejected because the action still implies
  acceptance.
- Signed Statement or Attestation as the primary noun: rejected because
  Submission is clearer and does not imply human endorsement.
- Evidence Bundle: rejected because the object also carries Claim, scope,
  caveats, provenance, requirements, and requested change.
- `review apply`: rejected because accept, reject, and retract are different
  scientific actions.
- Permanent aliases: rejected because they preserve two current products.
- A universal ontology or new repository: rejected because this is a narrow
  transition contract inside the existing architecture.
