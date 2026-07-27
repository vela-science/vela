# Vela protocol: current contract

Status: proposed Vela `0.930.0-rc.13` repository-authority candidate. Vela
`0.915.1` is retained only as the historical Era-0 replay baseline.

This document defines the small protocol surface that Vela ships now. Git
stores and transports immutable bytes. Vela gives a scientific meaning to a
bounded subset of those bytes, records who had authority to change accepted
state, and deterministically rebuilds the current frontier from the event log.

The workspace candidate (`0.930.0-rc.13`), finding-bundle schema (`0.10.0`), and wire
schema names such as `vela.event.v0.1` are separate identifiers. New work uses
the current forms below. Older micro-version chronology belongs in Git history
and `CHANGELOG.md`, not in the active protocol.

## 1. Scope

The current narrow waist is:

```text
Target -> Attempt -> Submission -> Registration Record -> Proposal
       -> Verification Record(s) -> authorized Decision -> Event
       -> deterministic replay -> Standing
```

The user-facing cycle is:

```text
inspect -> attempt -> submit -> verify -> decide -> continue
```

Everything above or below this waist is replaceable. A producer may be a
notebook, proof assistant, laboratory system, Canopus, or another workbench.
A reader may be the Observatory, search, a wiki, or a graph. Integrating with
Vela grants neither role scientific authority.

Current interoperability contracts are:

- `vela.submission.v1`, the sole portable producer package;
- `vela.registration-record.v1`, Vela's exact intake record;
- `vela.verification-record.v1`, a scoped verifier observation;
- current Claim presentations over historical Finding-era bytes, plus current
  Artifact, Proposal, Decision/Event, actor, and policy bytes;
- canonical JSON, content addressing, signatures, replay, and repository
  authority; and
- stable CLI JSON for `start`, `submit`, `show`, `why`, verification import,
  direct review actions, checking, and reproduction.

Historical Receipt-era objects remain readable and replayable. Current writers
do not emit them.

## 2. Roles and authority

| Role | May do | May not do |
| --- | --- | --- |
| Producer or agent | inspect, start an Attempt, run tools, submit evidence, withdraw its own pending Proposal | create Verification, accept, reject, sign as a human, or invent authority |
| Verifier | evaluate exact bytes under a named method and emit a Verification Record | decide significance, standing, acceptance, or authorship |
| Signed policy | admit only an exact class previously authorized by a human | widen itself, treat model output as authority, or sign new policy |
| Human principal | accept, reject, retract, correct, or supersede one exact Proposal when authorized | delegate semantic judgment to an agent or bypass repository authorization |
| Git host | preserve and transport commits and refs | turn publication or merge into scientific acceptance |
| Derived reader | verify configured sources and serve projections | mutate a Frontier, sign, decide, or store authority |

Authentication, repository authorization, verification, and scientific
acceptance are separate dimensions. A model may prepare evidence or request an
exact protected action. It may not authorize that action or hold a human key.

## 3. Canonical bytes and identifiers

Canonical objects use deterministic UTF-8 JSON. Content roots use lowercase
SHA-256 as `sha256:<64 hex>`. A changed canonical preimage creates a different
identifier.

| Prefix | Current object |
| --- | --- |
| `vfr_` | Frontier identity |
| `vat_` | Attempt; operational and non-authoritative |
| `vsb_` | Submission |
| `vrr_` | Registration Record |
| `vpr_` | Proposal |
| `vvr_` | Verification Record |
| `vev_` | canonical Event |
| `vap_` | signed acceptance policy |

Historical `vf_`, `vrc_`, and `vva_` identifiers remain valid and replayable.
Readers may present them as historical claim, registration, and verification
records only while disclosing their exact source schema, ID, and root. They
must not manufacture current `vcl_`, `vrr_`, or `vvr_` identities by
relabeling historical bytes.

Git object IDs and Vela IDs are different. A Git commit identifies transport
history and a tree. A Vela ID identifies one typed scientific, evidence, or
governance object. Neither substitutes for the other.

## 4. Current objects and schemas

### 4.1 Submission, Registration Record, and Verification Record

`vela.submission.v1` is authenticated producer input. It binds one scoped
Claim, conditions, Artifacts, caveats, replayability, producer provenance,
producer-reported checks, independent verification requirements, requested
change, and optional exact execution binding. A Submission cannot assert
standing or contain a Vela Decision or Event.

`requested_change.kind=add_claim` has no target. `correct_claim`,
`supersede_claim`, and `retract_claim` require both the exact historical
`vf_` Claim identifier and its full SHA-256 Finding root. The target is
membership in retained canonical history, not a text match or mutable date.
Changing either target field changes the authenticated Submission.

`vela.registration-record.v1` is emitted by Vela after exact Submission bytes
cross the repository-authority intake boundary. It binds the Submission root,
Frontier, operation, registered Artifacts, resulting Proposal, route, and
transaction root. Registration proves intake, not truth, verification, or
acceptance.

`vela.verification-record.v1` is a verifier's authenticated, scoped observation
over exact Claim, Submission, Proposal, Artifact, method, implementation,
environment, and property roots. A passing Verification Record changes no
standing by itself. Before current acceptance is available, the Decision Brief
revalidates the retained Submission bytes and requires every declared
verification property to have an exact, independently produced passing record.
A failing record refutes the route; missing, invalid, producer-dependent, or
inconclusive records leave acceptance blocked. Repository authorization and a
human Decision remain separately required.

Implementations:

- [`crates/vela-protocol/src/objects/submission_v1.rs`](../crates/vela-protocol/src/objects/submission_v1.rs)
- [`crates/vela-protocol/src/objects/registration_record.rs`](../crates/vela-protocol/src/objects/registration_record.rs)
- [`crates/vela-protocol/src/objects/verification_record.rs`](../crates/vela-protocol/src/objects/verification_record.rs)

Historical `vela.receipt.v1` remains documented in [RECEIPTS.md](RECEIPTS.md)
for exact replay only. The term Receipt is reserved for a future Vela-issued,
verifiable inclusion proof; this protocol defines no such current object.

### 4.2 Historical Finding and artifact

A historical Finding (`vf_`) is the retained scientific claim primitive of the
Finding era. Its bundle binds the assertion, evidence, conditions, confidence
basis, provenance, flags, typed links, annotations, attachments, and
timestamps. The retained portable Finding schema is
[`finding-bundle.v0.10.0.json`](../schema/finding-bundle.v0.10.0.json).
Current readers expose it through `vela show`, `vela claim show`, and
`vela why` while disclosing its historical source era. Current writers do not
mint new `vf_` identities. The proposed `vcl_` Claim Record remains separately
gated by ADR 0021.

An artifact (`va_`) is a content-addressed descriptor for bytes or an immutable
external reference. Artifact disclosure, locator integrity, and observed
availability are independent axes. An artifact can support review without
being accepted as proof of its linked claim.

Findings and artifact descriptors are defined in
[`crates/vela-protocol/src/kernel/bundle.rs`](../crates/vela-protocol/src/kernel/bundle.rs).

### 4.3 Proposal

A proposal (`vela.proposal.v0.1`, `vpr_`) is a complete candidate transition.
It names the target, proposer, reason, payload, sources, caveats, and review
state. Creating or importing a proposal does not apply it.

Proposal status is checked against the append-only decision and domain events
that justify it. A stored status field is not independent authority. The
current type is
[`crates/vela-protocol/src/proposals/types.rs`](../crates/vela-protocol/src/proposals/types.rs).

### 4.4 Event

A canonical event (`vela.event.v0.1`, `vev_`) contains:

```text
schema, id, kind, target, actor, timestamp, reason,
before_hash, after_hash, payload, caveats, signature
```

The event payload contains enough typed data for replay. Truth-bearing human
events are signed. Policy-routed admission is accompanied by a verified
certificate from the applicable signed policy. Coordination and audit events
are explicitly distinguished from scientific state transitions.

`proposal.withdrawn` is a signed, non-scientific lifecycle event. Current
payload schema `vela.proposal-withdrawal.v2` binds the exact pending Proposal
root, Submission root, and producer identity-binding ID. The ordinary event
signature must verify under that Submission-bound agent key. Historical
`vela.proposal-withdrawal.v1` events retain their exact Receipt binding. A
valid event changes only Proposal Standing to `withdrawn`; it cannot delete
evidence or change accepted Claims. Invalid withdrawal bytes are reported and
projected as pending, and block strict verification.

Its closed payload is:

```text
schema, proposal_id, proposal_root, submission_root, identity_binding_id
```

The event targets the exact proposal, uses an agent actor and null scientific
before/after roots, and requires a non-empty reason. A proposal with a human
Decision event, more than one withdrawal event, no current Submission binding
(or historical Receipt binding), or a mismatched producer key cannot gain
withdrawn Standing.

The event type, known-kind registry, validation, and constructors live in
[`crates/vela-protocol/src/kernel/events.rs`](../crates/vela-protocol/src/kernel/events.rs).

New Profile v1 repositories begin with exactly one unsigned structural
`frontier.created` event. Its closed `vela.frontier-created.v1` payload contains
`name_at_creation`, `creator`, `profile_schema`, the canonical empty
`dependency_root`, and `created_at`. Those values must agree with the event
core; the event targets the creation name, uses null scientific hashes, and
carries neither a signature nor caveats. The full canonical event-content root
derives the readable `vfr_` handle and the closed Frontier identity record. It
is a structural identity commitment, not administrator authentication or human
authority. Historical pre-v1
`frontier.created` payloads remain replayable but cannot establish a Profile v1
genesis or parent a `trust_mode: genesis` repository boundary.

`frontier.repository_bound` is a signed, non-scientific repository-boundary
event with closed payload schema `vela.frontier-repository-boundary.v1`. Its
fixed core targets `{type: frontier, id: <frontier_id>}`, uses a human
administrator actor, null before/after scientific hashes, and an ordinary
Ed25519 event signature. The payload contains:

```text
schema, mode, frontier_id, identity_root, observed_profile_root,
dependency_root, dependencies, previous_identity_event_root,
legacy_identity_preimage_root, administrator_actor_id,
administrator_public_key, administrator_algorithm, trust_mode,
git_object_format, anchor_git_commit, anchor_git_tree,
anchor_event_log_root, anchor_event_count, anchor_snapshot_root,
anchor_snapshot_schema, anchor_proposal_root, anchor_actor_registry_root,
anchor_artifact_registry_root, anchor_canonical_store_root
```

Dependencies are exact closed records sorted by `(frontier_id,
identity_root)` with fields `frontier_id`, `identity_root`,
`scientific_state_root`, `git_object_format`, `git_commit`, and `git_tree`;
alternate ordering and duplicate keys are invalid. Retrieval locators are not
part of this security identity. `frontier_id + identity_root` identify the
authenticated dependency repository; the scientific-state root and exact Git
commit/tree identify the selected state of that repository. The dependency
record supplies context, not scientific evidence, transfer validity, standing,
or acceptance. The dependency-list root is recomputed from canonical JSON.
`temporalize_existing` requires `trust_mode: tofu`, a null
previous identity event, and a legacy identity root recomputed from the payload
anchor.
The first dependency update of a new v1 Frontier requires `trust_mode:
genesis` and chains `previous_identity_event_root` to the full canonical
content root of its `frontier.created` event. Every later update requires
`trust_mode: previous_boundary`, chains to the full preceding boundary content
root, preserves frontier identity, legacy identity root, administrator, key,
and algorithm, and strictly increases `anchor_event_count`.

Historical Vela versions created the first genesis and one-time legacy
temporalization boundaries. The current candidate verifies those events and
later `previous_boundary` events for replay compatibility but exposes no
boundary or dependency writer. Hand-authoring a continuation event is
unsupported.

Before replay accepts repository identity or dependency state, the complete
known boundary event set is validated independently of timestamps. Every event
must have its fixed core, closed payload, canonical ID, and valid signature;
the identity-event graph must be one linear chain. Unsigned or malformed known
events, missing parents, duplicate roots, forks, cycles, trust/mode mismatch,
identity drift, or rollback-shaped anchor counts fail closed. Signature-only
verification proves possession of the named key; full validity additionally
requires the anchored Git, retained-object, and active actor-registry checks
defined by
[ADR 0016](adr/0016-frontier-repository-profile-v1-and-legacy-identity-migration.md).

Any chain containing an administrator boundary also requires the consumer's
exact out-of-band `vela.repository-trust-anchor.v1` pin to the first such
boundary and its administrator key. This applies to both legacy temporalization
and the first native genesis-rooted dependency boundary: an unsigned genesis
can prove continuity but cannot select the intended administrator from two
otherwise valid forks. The pin is public local consumer configuration, never a
repository object, secret, scientific-state input, or source of acceptance
authority.

Repository-authority history has a separate first-root trust choice. Consumers
obtain the full sequence-1 `vela.authority-record.v1` root independently of the
checkout and install this minimal closed local record:

```json
{
  "schema": "vela.authority-trust-anchor.v1",
  "frontier_id": "vfr_...",
  "first_authority_record_root": "sha256:..."
}
```

That full record root already commits to the Frontier, initial keyset, policy
authorization, principal attribution, event and object delta, and execution
claim. The local anchor therefore duplicates none of those fields. It grants
no authority, changes no scientific state, and is never derived or
automatically trusted from repository-controlled bytes.

`vela.retained-object-manifest.v1` is a canonical sorted JSON list of
`{path, git_mode, size, sha256}` entries. Only tracked regular-file modes
`100644` and `100755` are valid; `sha256` is a bare 64-character lowercase
digest. Paths are NFC relative repository paths without traversal, backslashes,
controls, duplicates, or collisions under the documented conservative
portable key (NFC followed by Unicode lowercase).

The pure value, root, event-shape, signature-only, boundary-chain, and complete
event-set checks live in
[`crates/vela-protocol/src/kernel/frontier_repository.rs`](../crates/vela-protocol/src/kernel/frontier_repository.rs).

Repository Profile v1 replaces the legacy whole-`Project` snapshot identity
with the closed `vela.scientific-state.v2` component-root record. It contains
`identity_root`, `dependency_root`, and explicit roots for findings, sources,
evidence atoms, conditions, legacy review/confidence records, artifacts,
released diff packs, verdict conflicts, contradictions, verifier attachments,
attempts and resolutions, transfers, endorsements, statement attestations,
anchor links, and statement registrations. Every component binds canonical
JSON, including `[]` for an empty collection.

The findings component uses a closed scientific projection rather than raw
`FindingBundle` serialization. Mutable graph `links` and the read-side
`access_tier` are excluded; assertion, evidence, conditions, interpretation,
provenance, annotations, attachments, and version identity remain bound. The
record also deliberately excludes display metadata, counters, events,
proposals, signatures, actors, proof exports, and active leases. Those values
have separate roots or non-scientific roles; adding a new display or
operational field cannot silently change scientific identity. Its pure
implementation and fixed empty-state vector live in
[`crates/vela-protocol/src/computed/scientific_state.rs`](../crates/vela-protocol/src/computed/scientific_state.rs).

The Profile v1 lock separately pins the Vela reducer and verifier packages
that produced `frontier.json`, `proof/`, and the compatibility-only
`legacy_snapshot_root`. Compatible later readers validate those derived bytes
using the lock-pinned materializer version rather than their own package
version. The reducer and verifier package names must agree exactly with the
lock's Vela version. This preserves exact historical derived views without
turning display metadata into scientific identity; explicit materialization
may advance the derived version while leaving canonical history and
`scientific_state_root` unchanged.

### 4.5 Actor and policy

The frontier actor registry maps a namespaced actor ID to an Ed25519 public key
and role. Initial registration is a one-time bootstrap for an empty registry;
established authority changes require governance rather than a direct producer
write.

New event signatures use the current versioned signing input. Historical
signatures may still verify for immutable replay, but no new writer emits the
historical form. The historical `key.revoke` kind is audit-only: it can inform
verification against an already governed registry, but it does not itself
rewrite `.vela/actors.json`. Profile v1 therefore rejects any actor-registry
change until a separate repository-local rotation and recovery contract is
specified; it never delegates that authority to a reader or other service.

An established actor may opt into temporal registration through one signed
`actor.registration_activated` event carrying
`vela.actor-registration-boundary.v1`. The payload binds the actor key to an
exact ancestor Git commit and tree, event-log root and count, and actor-registry
byte root. Exact events present at that anchor may retain their historical
signature state. An unsigned anchor member remains legacy and unauthenticated;
the activation does not attribute it to the key holder. Every matching event
absent from the anchor requires a valid signature, regardless of timestamp.
Missing, forked, altered, or tampered anchor history fails closed and grants no
exemption. Actor records without a valid activation retain timeless signature
enforcement.

Historical AcceptancePolicy (`vap_`) is human-signed, frontier-scoped, and bounded by
its current causal head. The current lane is `vela.policy-lane.v2`; an
unbound, unknown, revoked, stale, or out-of-scope policy cannot authorize a
Permit. Policy suggestions and tests are derived advice, not authority.

AcceptancePolicy v0.1 retains the historical generic claim-class language.
AcceptancePolicy v0.2 adds exact full-root allowlists for Receipt execution
bindings and requires exact replayability. A v0.2 Permit rule without all four
nonempty allowlists, a valid retained binding, and `replayability = exact`
fails closed. Existing v0.1 policies and policy-lane events retain their
original bytes, content addresses, evaluator version, and replay behavior.

AcceptancePolicy v0.3 additionally requires each Permit rule to name exactly
one full producer credential root derived from a retained Receipt v1 identity
bindings. The full root, not the short `vib_` handle, is the authorization
identity. Global registry membership does not bypass a v0.3 allowlist. Missing,
unretained, malformed, repeated, or unmatched credentials Defer or Deny and
grant no authority. V0.1/v0.2 registry-backed decisions replay unchanged.

The relevant current implementations are:

- [`crates/vela-protocol/src/kernel/sign.rs`](../crates/vela-protocol/src/kernel/sign.rs)
- [`crates/vela-protocol/src/kernel/actor_registration.rs`](../crates/vela-protocol/src/kernel/actor_registration.rs)
- [`crates/vela-protocol/src/policy/acceptance_policy.rs`](../crates/vela-protocol/src/policy/acceptance_policy.rs)
- [`crates/vela-protocol/src/proposals/policy_accept.rs`](../crates/vela-protocol/src/proposals/policy_accept.rs)

#### Proposed Era-1 candidate contract

[ADR 0020](adr/0020-attributed-repository-authority-and-standard-delegation.md)
remains Proposed. Its candidate adds
closed `vela.authority-keyset.v1`, `vela.policy-bundle.v1`,
`vela.authority-record.v1`, and `vela.event.v1` objects plus one legacy-signed
continuity event:

```text
authority.model_migrated
  payload: vela.authority-model-migration.v1
```

The bridge binds the exact pre-migration event-log, actor-registry,
active-policy-head, policy-store-manifest, new keyset, and new Cedar-bundle
roots, plus the new principal and minimum writer version. It is
non-scientific, uses null before/after scientific roots, and must be signed by
an unrevoked registered Era-0 human key. Authority-record sequence 1, signed
by the new repository authority, covers that same event.

The candidate dual verifier accepts unchanged Era-0-only history and, after a
bridge, requires every Era-1 event to be covered exactly once by a contiguous
DSSE authority-record chain. It rejects a changed legacy prefix, any added
legacy event, gaps, overlaps, transaction substitution, wrong event-log or
object roots, chain forks, keyset or policy substitution, invalid legacy
signatures, and Cedar diagnostics or denial. Exact registry bytes are bound;
no network or live identity provider is needed for replay.

Fresh Profile v1 repositories use a distinct, narrower sequence-1 boundary:

```text
authority.initialized
  payload: vela.authority-initialization.v1
```

This is a `vela.event.v1` under `.vela/authority/events/`, not a legacy
`StateEvent`. It is valid only when the pre-authority repository contains
exactly the unsigned structural `frontier.created` event, an empty actor
registry, and no authority history. Its payload binds that exact event-log and
registry root, the initial keyset and policy roots, the authenticated OS
principal, writer version, and reason. The same initial repository key signs
the covering sequence-1 DSSE record, proving possession. The record also
covers the exact retained Cedar material. Consumers still obtain the first
full authority root through an independent distribution trust path and
install it with `vela authority trust pin`.

The fresh path cannot exempt, relabel, or authorize historical events and
cannot run on an established or migrated Frontier. Missing, duplicate,
backdated, substituted, or tampered initialization bytes fail closed.

A Profile v2/current repository epoch uses the same
`authority.initialized` shape for a narrower current-only replay boundary.
The active repository retains no Era-0 events or actor registry. Instead,
`.vela/epoch.json` binds their exact archived roots, and sequence 1 must bind
those same roots plus the retained current keyset and Cedar bundle. The
verifier rejects partial roots, root substitution, any mixture of archived
roots with retained Era-0 bytes, and every gap, fork, uncovered event, or
unactivated authority snapshot in the current chain. Subsequent object-only
records advance the DSSE chain without changing the authority event-log root.
Historical schemas remain replayable only from the pinned predecessor with
the pinned historical binary; no old signature is interpreted as covering a
current object.

Storage is deliberately split at that boundary. The one migration bridge
remains an ordinary Era-0 file at `.vela/events/<id>.json`. Post-migration
`vela.event.v1` files live at `.vela/authority/events/<id>.json`, and their
covering DSSE envelopes live at
`.vela/authority/records/<record-id>.dsse.json`. The legacy `StateEvent` loader
therefore never has to interpret Era-1 bytes.

An Era-1 event has two deliberately different identities. Its stored `id`
content-addresses repository attribution, including `transaction_id`. Its
semantic reducer identity is the ordinary unsigned `StateEvent` ID recomputed
from the shared kind, target, actor, time, reason, before/after roots, payload,
and caveats. `review.accepted.payload.applied_event_id` names that semantic
identity. This lets the review decision link the scientific transition before
the covering transaction ID exists, without weakening the authority record:
the DSSE record still covers every stored Era-1 event ID, full event byte
root, event-log root, and object postimage.

The proposed writer also retains the exact active manifests at
`.vela/authority/keysets/<sha256>.json` and
`.vela/authority/policies/<sha256>.json`. These paths use the full canonical
object root, not a generation number or mutable alias. A transaction verifies
that the runtime Cedar schema, policy, and entity bytes match the roots in the
retained policy manifest. Missing manifests are installed as covered object
deltas; existing manifests must match exactly. Direct store membership is a
transaction input, so an added, replaced, deleted, or symlinked snapshot
aborts before the commit marker.

The exact Cedar source behind each manifest is retained separately by full
digest:

```text
.vela/authority/policy-material/schema/<digest>.cedarschema
.vela/authority/policy-material/policies/<digest>.cedar
.vela/authority/policy-material/entities/<digest>.json
```

These are authority-class, history-retained objects. An initial bundle emitted
before this storage rule may be reconstructed only when its exact bundle root
matches the deterministic sequence-1 translator. Any other missing or partial
material fails closed.

Historical Era-0 producer work used two exact, short-lived authentication proofs:

- the existing signed lease event authenticates `work_claim` under
  `agent_event_signature`; and
- the signed activity record authenticates `receipt_land` under
  `agent_record_signature`.

The activity-record signature binds the producer, Receipt root, operation,
claim, artifacts, caveats, and Frontier head. The embedded Receipt identity
binding, lease key, activity-record signer, and acting agent must match.
Historical landing was an object-only authority transaction: it covered the pending
proposal, Receipt, activity record, review material, and retained artifacts,
while appending no scientific event. Its before/after event roots are
identical. Verification therefore cannot be mistaken for acceptance.

Current producer intake instead registers one authenticated Submission and one
pending Proposal. Independent verifiers append exact Verification Records.
Only a direct authorized Decision changes Standing. Historical producer
bundles and policy events remain replayable, but their writer commands are
retired. The repository
authority remains the sole Era-1 transaction signer. Producer authentication
grants no review, acceptance, policy, membership, recovery, or key-rotation
authority.

In a Profile v2/current repository epoch, `vela start` is entirely private:
`vela.attempt.v2` binds the current repository root, epoch, Target Index v3,
packet, task contract, and starting Git commit/tree. It creates no lease Event
or authority transaction. Local expiry and locking coordinate one checkout
only and confer no global work ownership. `vela submit` must revalidate the
retained binding before repository authority can register current Submission
records.

The rotation law and internal writer are also closed. A new keyset must name
the exact prior keyset root, advance generation by one, and bind the
authority-record chain head that existed immediately before the covering
rotation transaction.
That transaction is signed and authorized under the old keyset and policy,
covers the new full-root snapshots, and must contain the exact
`authority_rotate` and/or `policy_rotate` semantic approval. The new keyset
and policy become active only for the following authority-record sequence.
This avoids a self-hash cycle while preserving one unambiguous activation
point. Duplicate public-key material is forbidden even under different key
IDs, so aliases cannot satisfy a threshold. Wrong snapshot paths, missing
approvals, old-key use after activation, skipped generations, and retained
but unactivated snapshots fail closed.

The candidate writer permits one authority transition per transaction: either
one keyset rotation or one policy rotation. It verifies the exact transition
and required semantic approval before authentication or signing, installs the
new full-root snapshot through the existing recoverable transaction, and
replays the complete candidate history before preparing the journal. A
keyset-rotation fixture then performs an ordinary later authority transaction
under the new key and replays all retained objects offline. Combining keyset
and policy rotation in one record is deliberately unsupported by the writer;
the read verifier remains capable of validating such retained history.

Emergency close is the sole exception to an open keyset. It installs one
terminal successor `vela.authority-keyset.v1` with `closed: true`, threshold
zero, and no keys. The field is absent from every open or historical keyset,
so their canonical bytes and roots do not change. The terminal keyset still
advances generation by one and links the exact prior keyset and
pre-transaction authority-record root.

The covering transaction is authorized under the current authority, requires
the human-only `authority_close` semantic approval, and contains exactly one
`authority.closed` event plus the terminal keyset snapshot. Its
`vela.authority-close.v1` payload binds the Frontier, last trusted sequence and
record root, previous and terminal keyset roots, current policy root, incident
identifier, and reason. No later authority record is valid. Close never
rewrites history and cannot reopen or recover authority; continuation after a
loss of continuity requires an explicit new lineage and out-of-band trust
anchor. Older binaries reject the terminal keyset's new field, which is the
intentional `0.930` protocol boundary.

The authority record's `transaction_write_set_root` is a domain-separated
commitment over the transaction ID, before and after authority-event-log
roots, sorted event IDs, and exact object deltas. It deliberately excludes the
covering authority-record envelope: including the signature over the record
inside the record's own committed write set would create a hash cycle. The
recoverable repository transaction separately binds and validates the exact
event and envelope file postimages before installation.

For mixed histories, the first authority record ends at the ordinary legacy
event-log root containing the bridge. Later roots use
`vela.authority-event-log.v1`, which commits to that fixed legacy root and the
sorted full roots of all covered `vela.event.v1` objects. This preserves every
Era-0 byte while giving Era-1 one deterministic append-only commitment.

The candidate includes pure verification and one reusable authority writer:

- [`crates/vela-protocol/src/kernel/authority.rs`](../crates/vela-protocol/src/kernel/authority.rs)
- [`crates/vela-protocol/src/kernel/authority_history.rs`](../crates/vela-protocol/src/kernel/authority_history.rs)
- [`crates/vela-authority/src/legacy_translation.rs`](../crates/vela-authority/src/legacy_translation.rs)
- [`crates/vela-cli/src/authority_transaction.rs`](../crates/vela-cli/src/authority_transaction.rs)
- [`crates/vela-cli/src/cli/authority.rs`](../crates/vela-cli/src/cli/authority.rs)

Formal, Sidon, Quantum, and Erdős retain a verified sequence-1 bridge without
changed scientific roots. The one-time writer that created those bridges is
retired. Once `authority.model_migrated` is present, every legacy producer,
administrator, actor-registry, first-boundary, and historical-sign write fails
before a journal or key read.

The live current writer covers private Attempts, authenticated
Submission-bound pending Proposals, exact Verification Record imports, and
human `review_accept` / `review_reject`. A human decision is one exact semantic
command. Vela authenticates the local operating-system principal, evaluates
restricted Cedar, and asks the standard OpenSSH agent repository authority to
sign the covering record. It reads no personal Vela key and uses no custom
helper.

The Decision Plan binds the exact current repository, Proposal, Claim,
Submission, ordered Verification Record set, principal, action, reason,
observation time, authority-event head, and policy root. Acceptance is
unavailable when an exact Verification Record fails or errors, or when a
declared Submission requirement lacks an independent passing record that
explicitly names the producer it is independent from. Rejection changes no
accepted Claim standing. Acceptance atomically updates the current repository
and appends the scientific domain event plus an explicit linked
`review.accepted` event. Replay rejects missing, duplicate, or ambiguous
applied transitions. Era-0 events remain archived predecessor evidence and
are not consulted by the live writer.

The complete core lifecycle drill now uses one disposable Frontier
to install the legacy bridge, perform an ordinary Era-1 decision, rotate the
repository key, perform another decision under the new key, close authority,
commit the canonical bytes to Git, clone them without local object reuse, and
replay the five-record history from the clean clone. The old key is not used
after activation and replay requires no signer or provider. This proves the
writer composition and provider-exit seam but does not authorize an
active-Frontier migration. A second composed fixture exercises the public
migration command internals over an exact Git Frontier and replays sequence 1
from a clean clone without a signer.

The same candidate read contract defines `vela.principal.v1` and
`vela.capability-grant.v1`. A human principal is an exact retained
`local:issuer|subject`, `oidc:issuer|subject`, or `orcid:issuer|subject`
identity; email and display metadata never create identity. Runtime
capabilities are content-addressed, restricted to agent or workload subjects,
bound to the exact repository-authority audience, Frontier, resources,
execution inputs, action set, validity window, and consequence ceiling, and
expire within 24 hours. A single child delegation may only attenuate its
parent and must bind the parent's full root.

Bearer tokens and authentication material are never canonical. An authority
record retains only the independently replayable verified claim, including
issuer, subject, actor chain, grant root, exact scope, token identifier,
expiry, revocation reference, and observation time. Human governance actions
cannot be represented by the capability action enum and are also rejected for
agent/workload callers before Cedar evaluation. OAuth/OIDC, SciTokens, GitHub
App identity, SPIFFE, and local credentials remain replaceable runtime
adapters.

The candidate implementation is:

- [`crates/vela-protocol/src/kernel/principal_capability.rs`](../crates/vela-protocol/src/kernel/principal_capability.rs)
- [`crates/vela-protocol/src/kernel/authentication.rs`](../crates/vela-protocol/src/kernel/authentication.rs)
- [`crates/vela-authority/src/runtime_authentication.rs`](../crates/vela-authority/src/runtime_authentication.rs)
- [`conformance/fixtures/principal-capability-v1.json`](../conformance/fixtures/principal-capability-v1.json)
- [`conformance/verify_principal_capability.py`](../conformance/verify_principal_capability.py)

The runtime module remains adapter-only while ADR 0020 is Proposed. It
consumes an observation, validates expiry and a passed revocation set, derives
reserved Cedar context, and exposes no filesystem or signer capability. The
candidate migration seam uses the local OS-session adapter; it issues no
credential and enables no ordinary Era-1 writer.

`vela.authentication-observation.v1` replaces the provisional arbitrary
authentication strings inside the candidate authority record. It binds:

```text
principal ID and class
issuer and subject
closed method and assurance
full non-secret session root
authenticated, observed, and expiry times
user-presence and user-verification facts
recent-recovery context
optional revocation reference
```

Human observations must match the exact retained local/OIDC/ORCID
issuer-subject. Passkeys require user presence, user verification, and
phishing-resistant assurance. Workload observations use only closed workload
methods and never claim human presence. Observation must occur within a
maximum 24-hour window; expiry and revocation fail closed. Cookies, bearer
tokens, refresh tokens, assertions, and raw session identifiers remain with
the authentication provider and never enter canonical history.

### 4.6 Verification dimensions

Current verifiers emit `vela.verification-record.v1` (`vvr_`). Historical
`vela.verifier_attachment.v0.1` (`vva_`) remains replayable and is projected
as a historical Verification Record with its source era disclosed.

Verification, acceptance, and publication are independent:

| Axis | Question | Authority |
| --- | --- | --- |
| Integrity | Do canonical bytes, signatures, roots, and replay agree? | `vela check` and the reducer |
| Reproduction | Do frozen methods obtain the recorded result again? | `vela reproduce` and named verifier bytes |
| Verification | What exact property did one Verification Record pass, fail, or leave inconclusive? | the scoped record only |
| Acceptance | Should this Frontier adopt the proposed transition? | an authorized Decision |
| Publication | Did exact bytes reach the intended Git ref? | verified Git transaction |

A result can pass one axis and fail another. The protocol never flattens these
dimensions into one unqualified word such as “verified” or “published.”

## 5. The only producer write edge

The supported producer loop is:

```bash
vela next <frontier> --json
vela start <target> --frontier <frontier> --as agent:<name> --json
vela submit --frontier <frontier> --attempt <vat_id> \
  --claim <claim> --type computational --replayability exact \
  --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json
```

A foreign producer may instead pass one complete current Submission:

```bash
vela submit submission.json --frontier <frontier> --as agent:<name> --json
```

The transport directory contains `submission.json` and
`artifacts/sha256/<digest>`. Artifact paths inside the signed Submission name
their final `records/artifacts/sha256/<digest>` locations. Transport blobs stay
outside the Frontier until Vela verifies and creates those canonical paths in
the same repository-authority transaction. Pre-copying an untracked blob into
its canonical path fails closed.

Both paths converge on the same strict Submission verifier and recoverable
repository-authority transaction. Successful intake retains exact Submission
bytes, content-addressed Artifacts, one Registration Record, and one pending
Proposal. It writes no Verification Record, Decision, Event, or accepted-state
mutation.

In a current repository epoch, an add or revision request also creates one
`vela.claim-record.v1` under `pending_claims`; a retraction targets the exact
accepted Claim without creating another Claim body. The repository manifest,
not an Era-0 reducer, carries this pending standing. The Registration Record's
event-log before/after roots are the unchanged current authority-event root.
The covering object-only authority record binds every new object, the next
repository root, and the derived Target Index rebind. A private Attempt is
removed only after that transaction installs and the current repository
re-verifies.

A current Verification import likewise writes one exact
`vela.verification-record.v1` plus the next repository manifest under an
object-only authority record. The record must bind the exact current pending
Proposal, Submission root, Claim identity, and retained Artifact IDs. It
changes no Standing or scientific Event. An immutable Verification Record
retained across the repository epoch may refer to predecessor Proposal and
Claim IDs only when the current Proposal and Claim Record provide a unique
`imported_from` mapping. That mapping preserves the historical observation; it
does not reinterpret the old signature as covering current object identities.

The ordinary result is:

```text
Submission registered; review required.
Accepted scientific state changed: no.
```

An exact retry is idempotent. Reusing an operation identity with different
Submission bytes fails. A second Submission over related Claim text remains a
new contribution; Vela does not erase it through text deduplication.

The registration transaction and Git publication are separate. Failure before
the canonical commit marker is discardable. Failure after it is recovered from
a private operation journal using the exact prepared bytes. Failure to push
does not change Proposal standing or scientific authority.

## 6. Human decision and policy decision

Deferred proposals enter the terminal-only ceremony:

```bash
vela review accept <frontier> <vpr_id> --reason <text>
vela review reject <frontier> <vpr_id> --reason <text>
```

The ceremony renders a Review Packet from canonical frontier state. Internally
it prepares a content-addressed Decision Plan that binds the proposal, current
event-log roots, evidence roots, policy facts, semantic effect, authenticated
principal, and ordered event intents. The Decision Plan is private process
plumbing, not a new authority object.

Any change to the proposal, frontier head, evidence, policy, actor registry, or
semantic effect invalidates the prepared decision.

Era-0 decision events remain byte-verifiable but have no live writer in the
current candidate.

After a current repository epoch begins, rejection is a single semantic
command with no copied root or timestamp. Vela rederives the plan under the
authority barrier, authenticates the local operating-system principal, and
requires restricted Cedar to permit that principal's exact `review_reject`
action. The repository authority—not a human scientific identity—then signs
the covering DSSE transaction. For an add or revision Proposal, rejection
removes the exact Claim from `pending_claims`; for a withdrawal Proposal it
leaves the accepted Claim untouched. The append-only `review.rejected` event
retains the Proposal and before/after repository roots as audit evidence.
Failure or drift writes nothing. The path has no key path, custom helper,
batch, wildcard, saved answer, `--yes`, or persistent approval input.

Repository-authority acceptance is not inferred from verifier success. Before
the provider is asked to sign, every declared Submission verification
requirement must have an exact independent passing Verification Record and no
exact record may report failure or error. Acceptance then moves an add Claim
from pending to accepted, replaces exactly one accepted predecessor for a
revision, or removes exactly one accepted Claim for a withdrawal. The
transaction installs the next current repository, rebinds the derived Target
Index, and appends one scientific domain event plus one `review.accepted`
event whose `applied_event_id` names the domain event. It invokes no Era-0
Project reader, reducer, or Decision writer.

Current review readers retain every Proposal as an immutable record and derive
its standing from the covered current authority history. Exact terminal
Decision and applied-event roots are returned with the Proposal. Rejected,
withdrawn, and superseded Claim bytes remain inspectable by full root but are
not active standing. Repository verification fails on duplicate terminal
Decisions, unknown Proposal targets, missing or later applied events, the wrong
scientific transition, or any mismatch between Decision standing and
`accepted_claims` / `pending_claims`.

A signed policy uses the same causal discipline. It can permit a predeclared
class without a per-item human ceremony, but only while its ID, signature,
frontier, scope, expiry or revocation state, verifier requirements, and causal
head all match. A policy cannot authorize its own replacement.

Prelaunch frontiers may contain a policy/signature pair encoded before the
current closed policy format. Such bytes are not grandfathered into authority.
The protocol-local `governance.policy_legacy_retirement` proposal carries the
closed `vela.policy-legacy-retirement.v1` payload: stored `vap_` ID, raw SHA-256
roots for the active policy and signature bytes, and one boolean saying whether
the fixed same-ID snapshot pair is also an exact deletion target. It carries no
caller-selected path and no copy of the legacy bytes.

This is a recovery relation over existing primitives, not a second policy or
revocation mechanism. The keyless preparer only records a pending proposal.
The reference Decision Plan permits acceptance only when live replay is intact,
no signed policy head exists, no policy admission can be attributed to the
legacy ID (and no unattributed legacy auto-admission exists), both stored IDs
match, all byte roots still match, and the snapshot pair is absent or exactly
duplicated as declared. Those fixed files join the transaction read set. An
isolated registered human acceptance appends the existing signed
`review.accepted` event and atomically deletes the active pair plus the declared
exact duplicate snapshots. Rejection deletes nothing. The observation parser
is bounded and duplicate-key-safe but deliberately does not rederive a current
policy ID or verify the old signature; Git history retains the retired bytes.

No MCP profile solicits a human verdict, reads a human key, or creates a human
signature. Relaying already signed immutable bytes would be transport, not a
second signing surface.

## 7. Event log and replay

`.vela/events/` is the scientific authority log. Events are ordered
deterministically and folded by the reference reducer. The reducer validates
event IDs, target and payload shape, before/after roots, signatures or policy
certificates, proposal-decision parity, and kind-specific invariants.

Replay has three requirements:

1. the same accepted event bytes in the same order produce the same state;
2. deleting, reordering, duplicating, or changing an event is detectable; and
3. all displayed scientific state is derivable from committed inputs rather
   than a hidden database or cache.

Some events are coordination or audit records and deliberately do not mutate a
finding. Their null state hashes make that boundary explicit. They remain in
the ordered log because leases, decisions, policy admission, and revocation
must be auditable without pretending to be scientific evidence.

The reference reducer is
[`crates/vela-protocol/src/kernel/reducer.rs`](../crates/vela-protocol/src/kernel/reducer.rs).
Other implementations may use any internal design, but must agree with the
released vectors for the contract they claim.

## 8. Correction

Accepted history is immutable; current scientific state is revisable.

A correction is a new Submission or Proposal that names the affected object and
the proposed semantic change. Current corrective Submissions bind the exact
historical Claim ID and full Finding root. A human or applicable signed policy
decides it through the same boundary as any other transition. Retraction,
supersession, qualification, caveat, evidence repair, and review reversal append
new events. They never edit or delete the event being corrected.

Replay therefore preserves both facts:

- what the frontier previously accepted and under whose authority; and
- what the frontier now holds after the correction.

Derived consumers must show the current projection without hiding the
correction chain. A Git revert or force-push is not a scientific correction.

## 9. Repository layout

A frontier is an ordinary Git repository. The current owned paths are:

| Path | Role |
| --- | --- |
| `.vela/events/*.json` | immutable Era-0 semantic Events |
| `.vela/authority/events/*.json` | immutable Era-1 repository-authority Events; verified with their covering DSSE records |
| `.vela/proposals/*.json` | durable proposals and their checked decision projection |
| `.vela/actors.json` | actor-to-public-key registry and revocation state |
| `.vela/policies/active.json`, `.vela/policies/active.sig.json` | mutable candidate policy bytes and signature input; never causal authority without a signed policy head |
| `.vela/policies/<vap_>.json`, `.vela/policies/<vap_>.sig.json` | immutable retained policy pairs needed to replay actual policy-lane admissions |
| `.vela/findings/*.json`, `.vela/artifacts/*.json` | reducer-owned materialized object files; never hand-edit |
| `records/submissions/sha256/*.json` | exact current Submission bytes named by full digest |
| `records/registrations/sha256/*.json` | current Registration Records for accepted intake |
| `records/verifications/sha256/*.json` | current non-authorizing Verification Records |
| `records/receipts/sha256/*.json` | historical Receipt bytes retained for replay |
| `frontier.yaml` | repository manifest and declared dependency metadata |
| `frontier.json` | visible materialized frontier state |
| `vela.lock` | derived roots for events, proposals, sources, artifacts, proof, and dependencies |
| `proof/` | rebuildable replay and proof projections |
| `.vela/work/` | private, ignored Attempt state |
| `.vela/operation-journals/`, `.git/vela/operation-journals/` | private recovery state; never published as scientific state |

`vela frontier materialize <frontier>` verifies repository authority, forms the
transaction-independent semantic union of both retained Event eras, and
rebuilds visible state and lock/proof views from that union. It never copies
Era-1 Events into `.vela/events/` or rewrites either canonical history.
Generated views and indexes are not authority and must be safe to delete and
recreate. Read-only verification of an older
Profile v1 checkout uses the reducer and verifier versions pinned by that
checkout's lock when validating these non-scientific bytes; merely upgrading
the reader does not require a derived-view commit.

Completed frontier transactions retain their exact plan, commit marker,
event-set commitment, and before/after file-state digests. After exact
installation and completion verification, their private postimage byte copies
may be pruned. Prepared, committed, installing, installed, or conflicted
transactions retain every recovery blob and continue to block unrelated reads
and writes until recovered.

Git publication uses an isolated, exact-path candidate tree and compare-and-swap
ref movement. It must not consume unrelated caller staging. A Git commit proves
only that bytes entered a history; Vela event signatures and policy
certificates prove who authorized scientific state.

The layout implementation is
[`crates/vela-protocol/src/computed/frontier_repo.rs`](../crates/vela-protocol/src/computed/frontier_repo.rs).

## 10. Derived readers and indexes

A reader or index is a disposable projection over exact Frontier repositories.
It may clone or fetch Git, run strict verification, replay the event log, and
replace its projection only after the candidate snapshot passes. Vela ships
the local `vela serve` read surface; the optional Observatory is a separate
replaceable product.

No reader owns source registration, event append, signing, acceptance, policy,
peer authority, or witness authority. A database contains projection and
refresh state only. A notification may request a refresh; it cannot supply
canonical scientific bytes.

Graphs, semantic indexes, wikis, packets, dashboards, and AI context are also
derived systems. They should bind their output to the source Git commit and
Vela event-log root, label freshness and inference class, and remain safe to
replace. An inferred edge or generated summary enters accepted state only by
returning through a current Submission and authorized Decision. Historical
Receipt-based chains retain their original replay semantics.

See [Interoperability](INTEROPERABILITY.md).

## 11. Conformance

The current public checks are intentionally separated by what they establish.
From the Vela release repository:

```bash
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
cargo test -p vela-protocol submission_v1
cargo test -p vela-cli --test submission_surface_parity
cargo test -p vela-cli --test authority_initialization
python3 conformance/verify.py
```

The first command checks the Rust reducer against the shipped replay vectors.
The next two check the current Submission object and its CLI/MCP raw-wire
parity. The authority fixture proves add, independent verification, authorized
acceptance, exact-root correction, second verification and acceptance,
immutable historical Event bytes, dual-log materialization, and strict replay.
The final command runs the repository-local independent readers over the replay
fixture contract. This is the current end-to-end Submission and Decision
qualification, not a retained copy of the retired `work -> land -> sign`
harness.
No external partner, live network, or unrelated Lean campaign is a
prerequisite for these protocol checks.

A conformant implementation may claim only the surface it ran. Passing replay
vectors does not prove scientific correctness, verifier soundness, human
judgment, service availability, or ecosystem maturity.

The fixture contract is documented in [`conformance/README.md`](../conformance/README.md).
Security assumptions and abuse cases are in [Threat model](THREAT_MODEL.md).
The mathematical boundary is in [Theory](THEORY.md).

## Appendix A. Historical replay without historical producers

Immutable frontiers contain event kinds and signature forms that predate the
`0.800.0` hard cut. The known-kind registry and reducer retain the minimum code
needed to validate and replay those bytes. That is a read obligation, not a
live compatibility promise.

Current rules are:

- mutable frontiers and fixtures use the current finding schema;
- new events use the current event and signing form;
- retired objects have no CLI, adapter, reader route, or alternate writer;
- historical events are never rewritten merely to modernize their names;
- a retained replay arm does not make its old producer part of the protocol;
  and
- history that cannot be verified fails closed rather than being promoted
  through a snapshot backfill.

There is no second Carina kernel, public-mirror authority, or reader federation
write protocol. Early Diderot material is an inert exploratory evidence example,
not a Vela partner, compatibility target, architectural validation, or release
gate. Any future integration uses the generic Submission boundary.

Git history preserves the retired design chronology. The active contract stays
small so new producers and consumers can implement it without inheriting every
experiment that led here.
