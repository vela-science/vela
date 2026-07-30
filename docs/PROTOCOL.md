# Vela protocol

Status: pre-1.0 current repository-origin contract.

Vela is version control for scientific state. The protocol defines how exact
Claims, evidence, Verification Records, Proposals, Decisions, and Standing are
preserved in one Git repository without confusing production, verification,
authorization, or presentation.

## 1. Invariants

1. Canonical objects have closed schemas and full content roots.
2. Git commits and trees identify published repository bytes.
3. Producer authentication grants no verification or review authority.
4. Verification reports one scoped outcome and changes no Standing.
5. Only an authorized Decision admits a state transition.
6. Repository authority records the exact authenticated and authorized write;
   it does not supply scientific judgment.
7. Canonical history is append-only. Corrections add relations and Events
   rather than rewriting prior Claims or Decisions.
8. Derived readers and indexes are disposable and non-authoritative.
9. Unknown schemas, ambiguous identities, shortened security digests, missing
   trust roots, and incomplete histories fail closed.

## 2. User and object lifecycle

```text
Target
  -> Attempt
  -> Submission
  -> Registration Record
  -> pending Proposal
  -> Verification Record(s)
  -> Decision
  -> Event
  -> Standing
```

| Object | Role | Authority effect |
| --- | --- | --- |
| Target | Derived bounded unit of work | None |
| Attempt | Local work coordination against exact roots | None |
| Submission | Authenticated producer request and evidence | None |
| Registration Record | Proof of exact repository intake | None |
| Claim Record | Versioned assertion, conditions, evidence, and provenance | None by itself |
| Verification Record | Scoped verifier observation over exact inputs | None |
| Proposal | Candidate repository transition | None until decided |
| Decision | Authorized accept or reject action | Determines transition |
| Event | Canonical admitted transition | Changes replay |
| Standing | Deterministic result of current replay | Derived |

## 3. Current repository origin

A current Frontier contains:

```text
frontier.yaml
.vela/repository.json
.vela/origin.json
.vela/authority/events/
.vela/authority/records/
.vela/authority/keysets/
.vela/authority/policies/
.vela/authority/policy-material/
records/claims/sha256/
records/submissions/sha256/
records/registrations/sha256/
records/verifications/sha256/
records/proposals/sha256/
records/artifacts/sha256/
targets.json
```

`frontier.yaml` identifies the bounded repository. `.vela/repository.json`
is the closed `vela.repository.v3` index of active object sets.
`.vela/origin.json` is the immutable `vela.repository-origin.v1` commitment.

The active repository contains no predecessor scientific Event log, actor
registry, AcceptancePolicy store, Finding bundle, Receipt store, legacy
Proposal directory, or materialized Project snapshot.

### 3.1 Repository origin

`vela.repository-origin.v1` binds:

- Frontier identity and Profile v2 root;
- a content-derived `vro_` identity and full origin root;
- generation and exact initial object-set root;
- `kind = genesis | compaction`;
- for a genesis, no predecessor and an empty initial object set; or
- for a compacted pre-release repository, one exact predecessor remote, tag,
  commit, tree, repository and authority roots, archive digest, object
  manifest root, and equivalence-report root.

`vela init` creates Profile v2 and repository scaffolding. `vela authority
init` installs a genesis origin, repository v3 manifest, keyset, Cedar policy,
and sequence-one authority history in one recoverable transaction. Until then,
strict repository verification is blocked.

The four controlled repositories use compacted origins whose predecessor
fields are immutable provenance. The migration writer and alternate repository
readers are not part of the current binary. Historical execution requires the
predecessor tag or archive and its pinned historical Vela release.

### 3.2 Claim Record

`vela.claim-record.v1` contains:

```text
claim_id
revision
assertion
conditions
evidence
provenance
relations
created_at
extensions
```

The content-derived Claim identity commits to the revision, assertion,
conditions, evidence references, and provenance. Relations use full Claim
identities. The full canonical record root additionally commits to relation
metadata and namespaced non-authoritative extensions. Evidence identifiers are
exact lowercase 64-hex content hashes; aliases and retired migration handles
fail.

### 3.3 Submission

`vela.submission.v1` is the portable producer boundary. It binds:

- producer identity and signature;
- Frontier and Target;
- exact Claim request;
- requested change;
- conditions and caveats;
- content-addressed Artifacts;
- replayability and method facts;
- declared verification requirements; and
- source workbench metadata.

A workbench can produce Submission bytes without importing Vela internals.
Submission identity is over the exact closed canonical bytes.

### 3.4 Registration Record

`vela.registration-record.v1` proves that Vela validated and retained one
Submission, its Artifacts, resulting Claim Record, and pending Proposal inside
one repository-authority transaction.

It binds unchanged authority-event before/after roots for object-only intake,
the repository roots, object roots, principal attribution, and transaction
identity. Registration proves intake, not truth, verification, or acceptance.
Its Artifact list uses full lowercase content hashes.

### 3.5 Verification Record

`vela.verification-record.v1` binds:

- exact Frontier, Claim, Submission, and Proposal;
- only retained content-addressed Artifacts;
- verifier identity and independence disclosure;
- method and implementation;
- environment and execution-evidence roots;
- scoped property and explicit nonclaims;
- outcome; and
- verifier signature.

Artifact references use the repository object's full lowercase 64-hex content
hash. Import resolves every non-empty reference against exact repository
membership; aliases and short digests are not accepted.

Outcomes are:

```text
pass fail inconclusive error unavailable not_run
```

Import is an object-only repository-authority transaction. It changes no Claim
Standing and appends no scientific Event.

### 3.6 Proposal

`vela.proposal.v1` binds the requested transition, exact Claim, Submission,
Registration Record, base repository root, required verification, and current
status.

Statuses are:

```text
pending_review accepted rejected
```

A terminal Proposal retains the exact Decision and authority references. It is
never deleted or reopened.

### 3.7 Artifact

Artifacts use full SHA-256 content identity and canonical paths under
`records/artifacts/sha256/`. A Submission can reference only bytes that match
the declared digest and bounded path contract.

Artifacts provide evidence inputs. Their existence, signature, or
reproduction does not establish scientific Standing.

## 4. Repository authority

Repository authority consists of:

- `vela.authority-keyset.v1`;
- `vela.policy-bundle.v1` with exact retained Cedar schema, policy, and entity
  roots;
- `vela.event.v1` semantic Events; and
- DSSE-wrapped `vela.authority-record.v1` transaction records.

Authority records form a contiguous full-root chain. Each record covers:

- sequence and previous record root;
- exact keyset and policy;
- authenticated principal and scoped capability;
- semantic action and intent digest;
- authority Event before/after roots;
- repository before/after roots;
- complete canonical write-set commitment; and
- execution identity.

The selected Ed25519 repository key signs the DSSE envelope. The initial local
provider is the standard OpenSSH agent.

### 4.1 Trust anchor

Consumers obtain the full sequence-one authority-record root through an
independent channel and store:

```text
vela.authority-trust-anchor.v1
  frontier_id
  first_authority_record_root
```

Repository bytes may not choose their own trust anchor. Pinning changes no
Frontier byte and grants no authority.

### 4.2 Principal and action

The authority record distinguishes:

- the repository service identity that signs the record;
- the authenticated human, agent, or workload principal that requested the
  semantic action; and
- the Cedar authorization that allowed that action.

Producer identities cannot acquire review, administration, or recovery
authority by signing producer objects.

### 4.3 Decision

`review accept` and `review reject` are direct semantic actions. Their Decision
Plan binds:

- repository origin and root;
- Proposal, Claim, Submission, and ordered Verification Records;
- action and reason;
- authenticated principal;
- observation time;
- current authority head, keyset, and policy; and
- exact canonical delta.

Acceptance requires every declared verification property to have an exact,
independent passing Verification Record over the same Claim, Submission, and
Proposal. A fail blocks. Missing, dependent, inconclusive, error, unavailable,
or not-run records do not satisfy the requirement.

Verification eligibility constrains a Decision; it does not perform or
recommend one, and it does not silently satisfy a separately registered value,
consumer, or external-independence gate.

Rejection removes the pending transition and leaves accepted Standing
unchanged. Acceptance applies exactly the requested add, revision,
supersession, correction, or retraction and appends the linked scientific and
review Events.

## 5. Canonical operations

### 5.1 Initialize

`vela init` creates structural repository identity only.

`vela authority init` is valid only for an untouched current structural
Frontier. It binds one loaded OpenSSH-agent Ed25519 identity, current keyset,
Cedar bundle, authenticated OS principal, and reason in the sequence-one
authority record. It changes no scientific Standing.

### 5.2 Start

`vela start` creates `vela.attempt.v4` only in ignored local coordination. The
Attempt closes over:

- repository origin and root;
- Target Index root;
- Target and packet;
- source Git commit/tree;
- completion contract;
- controller and runner build identities;
- closed routine operations and Artifact classes;
- enforced Submission, Artifact, and byte budgets;
- an `evidence_only` or `pending_review` consequence ceiling; and
- local expiry.

It creates no canonical Event, repository record, or authority-key read.

### 5.3 Submit

`vela submit` installs the exact authenticated Submission, declared Artifacts,
derived Claim Record, Registration Record, and pending Proposal in one
recoverable object-only authority transaction.

New Claims enter `pending_claims`. Accepted Standing does not change. A
successful Submission increments the ignored Attempt counters and leaves the
authorization active. Every later Submission revalidates the current exact
Target binding. The Attempt ends only through expiry or explicit `start
--drop`; it cannot accept or reject a Proposal.

### 5.4 Import verification

`vela verification import` accepts only a signed record bound to one current
pending Proposal and its exact current objects. It advances the repository
manifest and changes no Standing.

### 5.5 Decide

`vela review accept|reject` rederives the complete Decision Plan, authenticates
and authorizes the semantic principal, rechecks the read set, requests the
repository-authority signature, and installs the canonical transaction through
the recoverable journal.

Any drift or failure before the commit marker produces no canonical mutation.

## 6. Target Index

`vela.target-index.v4` is derived and non-authoritative. It binds:

- repository origin and root;
- exact source and input roots;
- ordered Targets;
- packet and task contracts;
- verifier profiles; and
- deterministic rank facts.

`next` validates the full index and returns producer work only. `start`
revalidates the selected Target and packet. A stale or invalid index grants no
Offer.

Ranking and graph position never imply authority.

## 7. Replay and Standing

Strict replay verifies:

1. the repository origin and any exact predecessor commitment;
2. the independently pinned sequence-one authority root;
3. contiguous authority records and valid DSSE signatures;
4. activated keyset and Cedar material;
5. canonical object schemas and full roots;
6. Proposal, Submission, Registration, Verification, Claim, Artifact, and
   Event relations;
7. repository-manifest parity;
8. accepted and pending Claim sets; and
9. transaction-journal integrity.

Standing is derived only from valid admitted Events. It is never read from a
database, Web page, mutable status field, verifier outcome, or Git branch name.

Non-strict checking reports defects but grants no trust or exemption.

## 8. Correction

A correction is a new Submission and Proposal targeting the exact accepted
Claim identity. It preserves the predecessor Claim and Decision.

Current relations include:

```text
revises supersedes corrects retracts supports opposes depends_on
```

Acceptance updates Standing according to that exact relation. Consumers can
therefore reconstruct both what previously stood and what stands now.

## 9. Derived readers

The Observatory, Neon, search, graphs, embeddings, exports, and local indexes
are projections over verified repository roots. They may improve discovery but
cannot:

- register a Submission;
- import Verification;
- append an Event;
- sign an authority record;
- accept or reject a Proposal; or
- define Standing.

Disagreement is resolved from an exact Git checkout with `vela check --strict`
and the declared frozen verifiers.

## 10. Interoperability

The public write boundaries are:

- `vela.submission.v1` for producer input; and
- `vela.verification-record.v1` for verifier observations.

Adapters disclose source identity, versions, exact roots, transformations,
losses, and nonclaims. They never emit Vela authority Events or infer Standing.

Domain semantics remain Frontier-local. Cross-domain bridges bind exact source
and target roots, assumptions, mappings, consequence tier, version, and
correction behavior.

## 11. Conformance

Protocol conformance requires:

- closed-schema positive and negative fixtures;
- canonicalization and full-root agreement;
- signature and authority-chain validation;
- invalid-object, missing-object, fork, rollback, and tamper refusal;
- object-relation and Standing replay parity;
- exact current repository fixtures;
- cross-implementation Submission and Verification fixtures; and
- failure before mutation for stale or unauthorized writes.

Focused checks:

```bash
cargo check -p vela-cli
cargo clippy -p vela-cli --all-targets -- -D warnings
python3 conformance/verify.py
```

The deterministic full release union runs once per release boundary.

## 12. Predecessor verification

The current binary verifies the repository-origin boundary and current state.
It does not retain predecessor writers or parse predecessor protocol objects
as active state.

Exact historical execution uses the tagged source, commit, tree, Git-object
manifest, archive digest, canonical roots, and pinned historical binary named
by the compacted origin. Current verification confirms those commitments
without reissuing old signatures under new schemas.

See [ADR 0027](adr/0027-pre-release-current-state-compaction.md) for the
completed transition.
