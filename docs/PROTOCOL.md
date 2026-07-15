# Vela protocol: current contract

Status: current prelaunch contract for Vela `0.800.2`.

This document defines the small protocol surface that Vela ships now. Git
stores and transports immutable bytes. Vela gives a scientific meaning to a
bounded subset of those bytes, records who had authority to change accepted
state, and deterministically rebuilds the current frontier from the event log.

The workspace release (`0.800.2`), finding-bundle schema (`0.10.0`), and wire
schema names such as `vela.event.v0.1` are separate identifiers. New work uses
the current forms below. Older micro-version chronology belongs in Git history
and `CHANGELOG.md`, not in the active protocol.

## 1. Scope

The current protocol has one narrow waist:

```text
producer activity
    -> Receipt v1 (evidence, not a verdict)
    -> proposal (a candidate state transition)
    -> signed policy or human decision (authority)
    -> canonical event (the accepted transition)
    -> deterministic replay (current frontier state)
```

Everything above or below that waist is replaceable. A producer may be a
notebook, proof assistant, lab system, agent, hosted runtime, or another Git
repository. A consumer may be a graph, wiki, search service, article machine,
or user interface. Neither becomes an authority service by integrating with
Vela.

The current interoperability contracts are:

- `vela.receipt.v1` and its canonical whole-body binding;
- current finding, artifact, proposal, event, actor, and policy bytes;
- canonical JSON, content addressing, event signatures, and event replay;
- the committed frontier layout and its rebuildable views;
- stable CLI JSON for the documented producer and reviewer loop; and
- the public conformance vectors under `conformance/`.

Work sessions, Decision Plans, transaction journals, caches, indexes, Hub
tables, adapter-private results, and Rust module boundaries are implementation
details. They may change without becoming new protocol objects.

## 2. Roles and authority

Vela separates capabilities that other systems often collapse.

| Role | May do | May not do |
| --- | --- | --- |
| Producer or agent | inspect state, claim a work lease, run tools, emit a Receipt, land evidence, draft a correction | accept, reject, revise, sign as a human, or invent a policy certificate |
| Verifier | evaluate exact bytes under a named method and emit a bound result | decide significance, acceptance, or authorship |
| Signed policy | permit only the bounded class and causal state named by a prior human ceremony | widen its own scope, treat model output as authority, or sign new policy |
| Human key holder | accept, reject, or request revision through `vela sign`; sign and revoke policy | delegate the private key to an agent or unsigned service |
| Git host | preserve and transport commits and refs | turn a commit, merge, or pull request into scientific acceptance |
| Hub | clone configured Git sources, verify them, and serve read projections | register sources through a public write API, sign, accept, store canonical witness authority, or mutate a frontier |

Key custody is the trust boundary. A model may help prepare review material,
but no model, browser, MCP tool, Hub process, or background worker belongs in
the human signing path. Human finalization is terminal-only.

## 3. Canonical bytes and identifiers

Canonical objects use deterministic UTF-8 JSON. Content roots use lowercase
SHA-256 in the form `sha256:<64 hex>`. Object identifiers are derived from the
canonical or explicitly defined preimage for their type; a changed preimage
mints a different identifier.

The current waist uses these prefixes:

| Prefix | Object |
| --- | --- |
| `vfr_` | frontier identity |
| `vf_` | finding |
| `va_` | artifact descriptor |
| `vpr_` | proposal |
| `vev_` | canonical event |
| `vrc_` | durable landing record that points to a Receipt root |
| `vva_` | verifier attachment |
| `vap_` | signed acceptance policy |

Other prefixes belong to domain objects or private operations. They are not a
reason to expand the narrow waist. In particular, private operation and work
session identifiers do not carry scientific authority.

Git object IDs and Vela IDs are different. A Git commit identifies a tree and
its transport history. A Vela ID identifies a typed scientific or governance
object. Neither may be substituted for the other.

The canonical rules and vectors live in:

- [`crates/vela-protocol/src/kernel/canonical.rs`](../crates/vela-protocol/src/kernel/canonical.rs)
- [`conformance/canonical-hashing.json`](../conformance/canonical-hashing.json)
- [`conformance/decision-binding.json`](../conformance/decision-binding.json)

## 4. Current objects and schemas

### 4.1 Receipt v1

`vela.receipt.v1` is the sole portable producer input. It carries a scoped
claim, claim type, replayability, artifacts or immutable locators, caveats,
conditions, verification requirements, producer provenance, and optional
producer-reported verifier runs.

A Receipt is evidence. It is never an acceptance, gate verdict, authorship
decision, or human attestation. A producer-reported pass remains a producer
claim until a separate verifier attachment establishes its own bound result.

The complete Receipt body is canonically bound into its attestation. Parsers
reject duplicate JSON names, malformed or stale whole-body bindings, unsafe
paths, unsupported replayability values, invalid artifact references, and
producer attempts to claim Vela-side authority.

Public local artifacts are retained by content digest. Public remote artifacts
need an immutable locator, digest, and size. Restricted material uses an opaque
`custodian:` or `opaque:` reference; the public Receipt must not disclose the
payload or an equality-revealing digest.

The current schema and implementation are:

- [`docs/schemas/vela.receipt.v1.schema.json`](schemas/vela.receipt.v1.schema.json)
- [`crates/vela-protocol/src/objects/receipt_v1.rs`](../crates/vela-protocol/src/objects/receipt_v1.rs)
- [Receipts](RECEIPTS.md)

### 4.2 Finding and artifact

A finding (`vf_`) is the current scientific claim primitive. Its bundle binds
the assertion, evidence, conditions, confidence basis, provenance, flags,
typed links, annotations, attachments, and timestamps. The current portable
finding schema is
[`finding-bundle.v0.10.0.json`](../schema/finding-bundle.v0.10.0.json).

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

The event type, known-kind registry, validation, and constructors live in
[`crates/vela-protocol/src/kernel/events.rs`](../crates/vela-protocol/src/kernel/events.rs).

### 4.5 Actor and policy

The frontier actor registry maps a namespaced actor ID to an Ed25519 public key
and role. Initial registration is a one-time bootstrap for an empty registry;
established authority changes require governance rather than a direct producer
write.

New event signatures use the current versioned signing input. Historical
signatures may still verify for immutable replay, but no new writer emits the
historical form. Revocation invalidates signatures at or after its effective
time without rewriting valid earlier history.

An acceptance policy (`vap_`) is human-signed, frontier-scoped, and bounded by
its current causal head. The current lane is `vela.policy-lane.v2`; an
unbound, unknown, revoked, stale, or out-of-scope policy cannot authorize a
Permit. Policy suggestions and tests are derived advice, not authority.

The relevant current implementations are:

- [`crates/vela-protocol/src/kernel/sign.rs`](../crates/vela-protocol/src/kernel/sign.rs)
- [`crates/vela-protocol/src/policy/acceptance_policy.rs`](../crates/vela-protocol/src/policy/acceptance_policy.rs)
- [`crates/vela-protocol/src/proposals/policy_accept.rs`](../crates/vela-protocol/src/proposals/policy_accept.rs)

### 4.6 Verifier attachment

A verifier attachment (`vela.verifier_attachment.v0.1`, `vva_`) binds a method,
solver or implementation, result, artifact evidence, and exact claim target.
Gate status is derived from retained attachments; it is not set by a producer
or stored as a free-standing truth field.

Verification, acceptance, and publication are independent:

| Axis | Question | Authority |
| --- | --- | --- |
| Integrity | Do canonical bytes, signatures, roots, and replay agree? | `vela check` and the reducer |
| Reproduction | Do frozen verifiers obtain the recorded result again? | `vela reproduce` and named verifier bytes |
| Gate | Do independent, claim-matched attachments satisfy the declared verification rule? | deterministic gate projection |
| Acceptance | Should this frontier adopt the proposed transition? | signed policy or human key |
| Publication | Did the exact accepted or pending bytes reach the intended Git ref? | verified Git transaction |

A result can pass one axis and fail another. The protocol never flattens them
into one word such as “verified” or “published.”

## 5. The only producer write edge

The supported producer loop is:

```bash
vela next <frontier> --json
vela work <target> --frontier <frontier> --as agent:<name> --json
vela land --frontier <frontier> --work <target> \
  --claim <claim> --type computational --replayability exact \
  --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json
```

A foreign producer may instead pass a complete Receipt:

```bash
vela land receipt.json --frontier <frontier> --as agent:<name> --json
```

Flags, file import, installed adapters, and the draft MCP profile converge on
the same strict Receipt parser and landing service. There is no adapter-owned
event writer and no packet-specific acceptance path.

Landing prepares the exact Receipt bytes, artifact projection, landing record,
proposal, policy context, route, and materialized outputs before committing a
delta. The route is:

- **Deny:** reject before the commit marker; leave no canonical or Git delta.
- **Defer:** retain the Receipt and pending proposal for a human decision.
- **Permit:** install the accepted event only with a verified certificate from
  the applicable, previously human-signed policy.

An exact retry is idempotent. Reusing an operation identity with different
Receipt bytes fails. A new Receipt with the same claim but new evidence remains
a new contribution; it is not erased by claim-text deduplication.

The landing transaction and a Git push are separate. Failure before the
scientific commit marker is discardable. Failure after it is recovered from a
private operation journal using the exact prepared bytes. Failure to push does
not change the policy route or scientific authority.

## 6. Human decision and policy decision

Deferred proposals enter the terminal-only ceremony:

```bash
vela sign --frontier <frontier>
```

The ceremony renders a Decision Brief from canonical frontier state. Internally
it prepares a content-addressed Decision Plan that binds the proposal, current
event-log root, evidence roots, policy facts, semantic effect, actor key, and
ordered event intents. The Decision Plan is private process plumbing, not a new
authority object.

Any change to the proposal, frontier head, evidence, policy, actor registry, or
semantic effect invalidates the prepared decision. The key is read only after
the exact plan is shown and confirmed. Acceptance, rejection, and revision
requests all leave signed append-only decision events. Acceptance also installs
the corresponding domain event in the same transaction.

A signed policy uses the same causal discipline. It can permit a predeclared
class without a per-item human ceremony, but only while its ID, signature,
frontier, scope, expiry or revocation state, verifier requirements, and causal
head all match. A policy cannot authorize its own replacement.

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

A correction is a new Receipt or proposal that names the affected object and
the proposed semantic change. A human or applicable signed policy decides it
through the same boundary as any other transition. Retraction, supersession,
qualification, caveat, evidence repair, and review reversal append new events.
They never edit or delete the event being corrected.

Replay therefore preserves both facts:

- what the frontier previously accepted and under whose authority; and
- what the frontier now holds after the correction.

Derived consumers must show the current projection without hiding the
correction chain. A Git revert or force-push is not a scientific correction.

## 9. Repository layout

A frontier is an ordinary Git repository. The current owned paths are:

| Path | Role |
| --- | --- |
| `.vela/events/*.json` | canonical ordered authority events |
| `.vela/proposals/*.json` | durable proposals and their checked decision projection |
| `.vela/actors.json` | actor-to-public-key registry and revocation state |
| `.vela/findings/*.json`, `.vela/artifacts/*.json` | reducer-owned materialized object files; never hand-edit |
| `records/receipts/sha256/*.json` | exact durable Receipt bytes named by full digest |
| `frontier.yaml` | repository manifest and declared dependency metadata |
| `frontier.json` | visible materialized frontier state |
| `vela.lock` | derived roots for events, proposals, sources, artifacts, proof, and dependencies |
| `proof/` | rebuildable replay and proof projections |
| `.vela/work/` | private, ignored work-session state |
| `.vela/operation-journals/`, `.git/vela/operation-journals/` | private recovery state; never published as scientific state |

`vela frontier materialize <frontier>` rebuilds visible state and lock/proof
views from committed inputs. Generated views and indexes are not authority and
must be safe to delete and recreate.

Git publication uses an isolated, exact-path candidate tree and compare-and-swap
ref movement. It must not consume unrelated caller staging. A Git commit proves
only that bytes entered a history; Vela event signatures and policy
certificates prove who authorized scientific state.

The layout implementation is
[`crates/vela-protocol/src/computed/frontier_repo.rs`](../crates/vela-protocol/src/computed/frontier_repo.rs).

## 10. Hub and derived systems

The Hub is a disposable read index over repositories selected by an operator in
a versioned source catalog. Ingest clones or fetches Git, runs strict
verification, replays the event log, and replaces the projection only after the
candidate snapshot passes.

The Hub has no public source-registration, source-deprecation, event-append,
signing, acceptance, policy, peer-authority, or witness-object write API. Its
database contains projection and ingest state only. A webhook may request a
refresh; it cannot supply canonical scientific bytes.

Graphs, semantic indexes, wikis, packets, dashboards, and AI context are also
derived systems. They should bind their output to the source Git commit and
Vela event-log root, label freshness and inference class, and remain safe to
replace. An inferred edge or generated summary enters accepted state only by
returning through Receipt v1.

See [Interoperability](INTEROPERABILITY.md) and [Hub](HUB.md).

## 11. Conformance

The current public checks are intentionally separated by what they establish.
From the Vela release repository:

```bash
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
cargo test -p vela-cli --test task_first_workflows
```

The first command checks the Rust reducer against the shipped replay vectors.
The second runs the repository-local independent readers over the same fixture
contract. The third exercises the public task-first Receipt, policy, and human
boundary in isolated frontiers. Release automation may compose additional
focused checks, but no external partner, live network, or unrelated Lean
campaign is a prerequisite for these protocol checks.

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
- retired objects have no CLI, adapter, Hub route, or alternate writer;
- historical events are never rewritten merely to modernize their names;
- a retained replay arm does not make its old producer part of the protocol;
  and
- history that cannot be verified fails closed rather than being promoted
  through a snapshot backfill.

There is no second Carina kernel, public-mirror authority, or Hub federation
write protocol. Early Diderot material is an inert exploratory evidence example,
not a Vela partner, compatibility target, architectural validation, or release
gate. Any future integration uses the generic Receipt boundary.

Git history preserves the retired design chronology. The active contract stays
small so new producers and consumers can implement it without inheriting every
experiment that led here.
