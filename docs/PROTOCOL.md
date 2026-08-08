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
5. Only an authorized human Decision admits a state transition.
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
  -> native work
  -> Submission
  -> pending Proposal
  -> Verification Record(s)
  -> Decision
  -> Event
  -> Standing
```

| Object | Role | Authority effect |
| --- | --- | --- |
| Target | Derived bounded unit of work | None |
| Native run | Execution retained by an external agent or scientific tool | None |
| Submission | Authenticated producer request and evidence | None |
| Claim Record | Versioned assertion, conditions, evidence, and provenance | None by itself |
| Verification Record | Scoped verifier observation over exact inputs | None |
| Proposal | Candidate repository transition | None until decided |
| Decision | Authorized accept or reject action | Determines transition |
| Event | Canonical admitted transition | Changes replay |
| Standing | Deterministic result of current replay | Derived |

### 2.1 Canonical language and the closed loop

Interfaces use the object names above as proper terms. They do not rename a
Submission as a pull request, a Verification Record as approval, a Decision as
verification, or Standing as status. `Review` is the CLI and product area that
shows Proposals, Verification Records, and Decisions; it is not another
protocol object. A native run is external execution activity, and Vela
retains no object of its own for it.

The canonical closed-loop Frontier is:

```text
Standing
  -> derived Target Index
  -> Target
  -> native work or run
  -> Submission
  -> pending Proposal
  -> Verification Record(s)
  -> human Decision
  -> Event
  -> strict replay
  -> Standing
  -> next Target
```

The loop is a product and operating model, not a new authority path. Targets,
runs, indexes, search, graphs, and Web projections remain non-authoritative.
Only the Decision admits an Event; replay deterministically derives Standing;
the Frontier's domain adapter may then derive the next Target Index.

Canonical user verbs follow the CLI: `init`, `next`, `start`, `submit`,
`verification record|import`, `review show|accept|reject|withdraw`, `replay`,
`reproduce`, `show`, and `why`. `Reproduce` is the user operation; strict
replay is the validation it performs. Product navigation may group these exact
objects under Problems, Frontiers, Work, Review, Activity, and Sources, but it
must not invent substitute protocol nouns or imply authority from a grouping.

## 3. Current repository origin

A current Frontier contains:

```text
vela.toml
.vela/repository.json
.vela/origin.json
.vela/authority/events/
.vela/authority/records/
.vela/authority/keysets/
.vela/authority/policies/
.vela/authority/policy-material/
records/claims/sha256/
records/submissions/sha256/
records/verifications/sha256/
records/proposals/sha256/
records/proposal-withdrawals/sha256/
records/artifacts/sha256/
targets.json
```

`vela.toml` identifies the bounded repository. `.vela/repository.json`
is the closed `vela.repository.v4` index of active object sets.
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

`vela init` creates Profile v2, the genesis origin, repository v4 manifest,
keyset, Cedar policy, sequence-one authority history, local trust anchor, and
initial Git commit in one recoverable transaction. If signing is unavailable,
the exact Profile remains and the same `vela init` command resumes the
operation. Strict repository verification remains blocked until that operation
completes.

Cedar is confined to repository-authority Decisions and rare authority
administration. It is not an agent permission system, campaign runtime, or
ordinary evidence-ingest gate, and Vela exposes no additional Cedar ceremony
in the producer workflow. The current restricted profile is frozen unless a
reproduced authority defect requires a compatible change.

The four controlled repositories use compacted origins whose predecessor
fields are immutable provenance. The migration writer and alternate repository
readers are not part of the current binary. Historical execution requires the
predecessor tag or archive and its pinned historical Vela release.

### 3.2 Claim Record

`vela.claim-record.v1` contains:

```text
schema
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

`schema` is required and must equal `vela.claim-record.v1`. The object rejects
unknown fields, so a producer that omits `schema`, or adds anything not on this
list, fails to parse.

The nested shapes a hand-builder cannot guess:

```text
assertion    text, kind
evidence[]   relation, artifact_root, optional artifact_id and artifact_path
provenance[] kind, title, optional locator, optional authors, optional year
relations[]  kind, target_claim_id
```

The content-derived Claim identity commits to the revision, assertion,
conditions, evidence references, and provenance. Relations use full Claim
identities. The full canonical record root additionally commits to relation
metadata and namespaced non-authoritative extensions. Evidence identifiers are
exact lowercase 64-hex content hashes; aliases and retired migration handles
fail. Section 8 declares the two vocabularies `relations[].kind` draws from.
`evidence[].relation` is a different axis and draws from neither.

### 3.3 Submission

`vela.submission.v1` is the portable producer boundary. Its complete closed
field set is:

```text
schema
submission_id
claim                      assertion, type, conditions
artifacts[]                kind, path, digest
caveats[]
replayability
producer_checks[]          method, outcome, authority
verification_requirements[]
requested_change           kind, optional target { claim_id, claim_root }
provenance                 producer, source_system, optional source_attempt,
                           optional source_run, emitted_at
execution_binding          optional `vela.execution-binding.v1`
authentication             algorithm, identity_binding, signature
```

So it binds:

- producer identity and signature;
- the exact Claim request and its conditions and caveats;
- the requested change and, for anything but `add_claim`, the exact target
  Claim identity and full root it changes;
- content-addressed Artifacts;
- replayability, producer-reported checks, and method facts;
- declared verification requirements;
- source workbench metadata; and
- an optional rooted `execution_binding` naming the packet, profile, verifier
  capsule, and result contract the work ran against.

A Submission binds no Frontier and no Target. It has carried neither since the
object was introduced, and both are absent from the live Submission bytes in
all four controlled repositories. This is what portability costs: the same
bytes are replayable into any repository by anyone holding them, and the
Frontier association is made by the receiving repository at `vela submit`
time, not asserted by the producer. An adapter must not add `frontier` or
`target` keys; the schema is closed and rejects them. The nearest thing to a
Target reference the object carries is `execution_binding.packet_root`, which
the producer sets to the Target packet root that `vela start` printed. It is a
producer declaration checked only for root shape, not a repository-verified
link back to the Target Index.

A workbench can produce Submission bytes without importing Vela internals.
Submission identity is over the exact closed canonical bytes.

### 3.4 Verification Record

`vela.verification-record.v1` binds:

- an exact subject: Claim, Submission id and full Submission root, Proposal,
  and the Artifact ids under scope;
- only retained content-addressed Artifacts, including `output_artifact_ids`;
- verifier identity and independence disclosure;
- method profile, implementation, and one `environment_root`;
- scoped property and explicit nonclaims (`scope.does_not_establish`);
- outcome;
- `started_at` and `completed_at`; and
- verifier signature.

Like a Submission, a Verification Record binds no Frontier. Its subject names
objects, and the Frontier is whichever repository holds those objects. Import
resolves every reference against exact repository membership, which is what
confines the record to one Frontier in practice.

Artifact references use the repository object's full lowercase 64-hex content
hash. Import resolves every non-empty reference against exact repository
membership; aliases and short digests are not accepted.

Outcomes are:

```text
pass fail inconclusive error
```

Import is a bounded routine-evidence transaction authenticated by the verifier
signature. It reads no repository-authority key, changes no Claim Standing,
and appends no scientific Event.

### 3.5 Proposal

`vela.proposal.v1`'s complete closed field set is:

```text
schema
proposal_id
action                 claim.add | claim.revise | claim.withdraw
subject                kind (always `claim`), id, root
actor
created_at
reason
producer_package       kind (always `submission_v1`), id, root, path
caveats[]
```

So it binds the requested transition (`action`), the exact Claim it acts on
(`subject`), and the signed Submission package that requested it
(`producer_package`).

It binds no status and no verification requirement. Invariants 3 and 5 are why:
a producer's signature grants no review authority, and only an authorized human
Decision admits a transition, so status is not state the producer can write and
therefore is not a field. Status is read back by evaluating the Proposal
against the covering authority Events; the object carries only the request. A
producer that writes a `status` key gets a parse rejection, because the schema
is closed. The declared verification requirements live on the Submission
(`verification_requirements`), not here.

Derived statuses are:

```text
pending_review accepted rejected withdrawn
```

`withdrawn` is the status of a Proposal closed by a
`vela.proposal-withdrawal.v1` rather than by a Decision. `vela review list
--status` accepts all four.

A terminal Proposal retains the exact Decision and authority references
through those Events. It is never deleted or reopened.

### 3.6 Artifact

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
- authenticated principal and recorded action authorization;
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
  repository_id
  first_authority_record_root
```

Repository bytes may not choose their own trust anchor. Pinning changes no
Frontier byte and grants no authority.

### 4.2 Principal and action

The authority record distinguishes:

- the repository service identity that signs the record;
- the authenticated human, agent, or workload principal that requested the
  semantic action; and
- the retained repository-authority policy decision that allowed that closed
  Vela action.

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

Clients can pass the exact derived Decision Inbox `entry_root` they presented
to the reviewer as `--if-entry-root`. Vela recomputes that packet before
authority signing and refuses a mismatch without requesting a signature. This
is a freshness guard for user interfaces and automation, not a new canonical
object or approval ceremony; Decision execution still re-prepares the full
read set and fails closed under the repository write barrier.

Acceptance requires every declared verification property to have an exact,
independent passing Verification Record over the same Claim, Submission, and
Proposal. A fail blocks. Missing, dependent, inconclusive, error, unavailable,
or not-run records do not satisfy the requirement.

Verification eligibility constrains a Decision; it does not perform or
recommend one, and it does not silently satisfy a separately declared value,
consumer, or external-independence gate.

Rejection removes the pending transition and leaves accepted Standing
unchanged. Acceptance applies exactly the requested add, revision,
supersession, correction, or retraction and appends the linked scientific and
review Events.

## 5. Canonical operations

### 5.1 Initialize

`vela init` creates the structural repository identity and binds one loaded
OpenSSH-agent Ed25519 identity, current keyset, Cedar bundle, authenticated OS
principal, and reason in the sequence-one authority record. It is resumable
after signing failure and changes no scientific Standing.

### 5.2 Start

`vela start` is a read-only orientation command. It validates and returns:

- repository origin and root;
- Target Index root;
- Target objective and Frontier scope;
- exact packet and packet root;
- source Git identity;
- declared verifier profile; and
- the human-Decision authority ceiling.

It creates no file, lease, run record, counter, budget, canonical Event,
repository record, or authority-key read. Vela does not launch or wrap an
agent, verifier, or workflow engine.

### 5.3 Submit

`vela submit` installs the exact producer-authenticated Submission, declared
Artifacts, derived Claim Record, and pending Proposal in one bounded
routine-evidence transaction.

New Claims enter `pending_claims`. Accepted Standing does not change. The
transaction reads no repository-authority key and cannot accept or reject a
Proposal.

### 5.4 Import verification

`vela verification import` accepts only a signed record bound to one current
pending Proposal and its exact current objects. It advances the repository
manifest and changes no Standing.

### 5.5 Withdraw a pending Proposal

`vela review withdraw` is producer-owned queue hygiene. It appends one
`vela.proposal-withdrawal.v1` signed by the exact key bound in the Proposal's
retained Submission and removes only that Proposal's Claim from the pending
projection. It reads no repository-authority key, emits no Event, and leaves
accepted Standing unchanged. A decided Proposal cannot be withdrawn, and a
withdrawn Proposal cannot later be decided or verified.

### 5.6 Decide

`vela review accept|reject` rederives the complete Decision Plan, authenticates
and authorizes the semantic principal, rechecks the read set, requests the
repository-authority signature, and installs the canonical transaction through
the recoverable journal.

Any drift or failure before the commit marker produces no canonical mutation.

## 6. Target Index

`vela.target-index.v5` is derived and non-authoritative. It binds:

- repository origin and root;
- exact source and input roots;
- ordered Targets;
- packet and task contracts; and
- deterministic rank facts.

`next` validates the full index and returns producer work only. `start`
revalidates the selected Target and packet. A stale or invalid index grants no
Offer.

The Frontier's domain adapter writes the final tracked index directly. Vela
does not maintain a candidate, seal, apply, inspect, or repair lifecycle for
this disposable projection.

Ranking and graph position never imply authority.

## 7. Replay and Standing

Strict replay verifies:

1. the repository origin and any exact predecessor commitment;
2. the independently pinned sequence-one authority root;
3. contiguous authority records and valid DSSE signatures;
4. activated keyset and Cedar material;
5. canonical object schemas and full roots;
6. Proposal, Submission, Verification, Withdrawal, Claim, Artifact, and Event
   relations;
7. repository-manifest parity;
8. accepted and pending Claim sets; and
9. transaction-journal integrity.

Standing is derived only from valid admitted Events. It is never read from a
database, Web page, mutable status field, verifier outcome, or Git branch name.

`vela replay` fails closed on defects and grants no trust or exemption.

## 8. Correction

A correction is a new Submission and Proposal targeting the exact accepted
Claim identity. It preserves the predecessor Claim and Decision.

A Claim Record's `relations` field carries two vocabularies that read alike and
behave nothing alike. Only the first has authority.

### 8.1 The correction algebra

```text
corrects supersedes
```

This set is closed and authoritative. A Claim admitted while carrying one of
these names exactly one accepted predecessor, and acceptance retires that
predecessor. Standing moves because the Decision admitted a `claim.revise`
Proposal, and the relation is what tells the replay which accepted Claim the
successor replaces. A `claim.add` Proposal carries no relation; a
`claim.withdraw` retracts through its own action and needs none either.
Consumers can therefore reconstruct both what previously stood and what stands
now.

### 8.2 Descriptive relations

```text
contradicts depends replicates supports synthesized_from
```

These are retained context: where a Claim came from, what it agrees or
disagrees with, what it rests on. No Decision reads them and none of them moves
Standing. The set is enumerated from what the maintained Frontiers actually
hold rather than from intent, and it is open. A Frontier may record a kind
this list does not name, and doing so grants that kind no authority. A derived
reader may give a descriptive relation meaning of its own, such as a dependency
edge or a support route, but that meaning is the reader's and is never Standing.

A relation kind is lowercase ASCII words joined by single underscores, at most
64 characters. Anything else fails the parse.

### 8.3 Canonical spellings

Retained records cannot be rewritten, so two spellings recognised on input
resolve to one canonical name:

| Recorded | Canonical | Why |
| --- | --- | --- |
| `depends_on` | `depends` | ADR 0004 named `depends` the stored wire value and `depends_on` the derived-graph rendering |

Producers emit the canonical spelling. Consumers resolve before matching. A
consumer that matches only `depends_on` sees none of the recorded dependencies.

`revises`, `retracts` and `opposes` are withdrawn: nothing ever emitted or read
any of them. `opposes` was carried as an alias for `contradicts` until it was
noticed that a near-miss table is for spellings a retained record holds, and no
record holds this one.

Do not confuse `relations[].kind` with `evidence[].relation`, which names the
role an Artifact plays for one Claim rather than a link between two Claims.

The vocabulary is fixed by
`conformance/fixtures/claim-relation-vocabulary-v1.json`.

## 9. Derived readers

The Observatory, Neon, search, graphs, embeddings, exports, and local indexes
are projections over verified repository roots. They may improve discovery but
cannot:

- submit or retain a Submission;
- import Verification;
- append an Event;
- sign an authority record;
- accept or reject a Proposal; or
- define Standing.

Disagreement is resolved from an exact Git checkout with `vela replay`
and the declared frozen verifiers.

## 10. Interoperability

The public write boundaries are:

- `vela.submission.v1` for producer input;
- `vela.verification-record.v1` for verifier observations; and
- `vela.proposal-withdrawal.v1` for producer-owned closure of one pending
  Proposal.

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
uv run --project conformance --locked python conformance/verify.py
```

The deterministic full release union runs once per release boundary.

## 12. Predecessor verification

The current binary verifies the repository-origin boundary and current state.
It does not retain predecessor writers or parse predecessor protocol objects
as active state.

For accepted Claims, `vela why` may follow the verified compacted-origin chain
through exact local Git objects. It reports recovered Proposal, Verification,
Decision, and supersession context as `compacted_origin`, and distinguishes
`verified_local`, `origin_object_set_only`, and `predecessor_unavailable`.
This is a read-only explanation path: it never fetches from the network,
extracts the retained archive, or treats predecessor objects as current state.

Exact historical execution uses the tagged source, commit, tree, Git-object
manifest, archive digest, canonical roots, and pinned historical binary named
by the compacted origin. Current verification confirms those commitments
without reissuing old signatures under new schemas.

See [ADR 0027](adr/0027-pre-release-current-state-compaction.md) for the
completed transition.
