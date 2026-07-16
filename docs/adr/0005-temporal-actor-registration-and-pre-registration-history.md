# ADR 0005: Temporal actor registration and pre-registration history

- Status: Accepted 2026-07-16. Approved by the user for implementation.
- Scope: actor registration, strict signature checks, immutable event history,
  Git-rooted replay, and cold-use validation.
- Baseline: Vela `v0.800.19`, commit
  `5a270f8b5ec038ade7c1274dc64a33dd99117851`.
- Authority: this ADR proposes an engineering rule. It is not a scientific
  event, a registry mutation, a human decision, or permission to read a key.
- Implementation state: the local, unreleased `0.800.20` reference candidate contains
  the event schema, explicit reducer no-op, Git-rooted assessment, strict
  classifications, hostile fixtures, CLI check integration, and
  Rust/Python/TypeScript reducer parity, and the human-only activation
  porcelain. The public release, real Erdős activation ceremony, and cold-use
  benchmark remain undone.

## Decision summary

Vela should support temporal activation of an actor registration through one
signed, non-scientific event:

```text
kind: actor.registration_activated
payload schema: vela.actor-registration-boundary.v1
```

The event binds an actor key to an exact pre-registration Git and Vela root.
Strict verification then applies the key requirement to events outside that
anchored history. Events already present at the anchor retain their historical
signature state. An unsigned anchored event remains legacy and unauthenticated;
the activation event does not attribute it to the key holder.

Vela must determine pre-registration membership from the anchored Git tree and
event-log root. `created_at` and event timestamps do not define membership.
This rule prevents an attacker from inserting a backdated unsigned event after
activation.

Frontiers without an activation event keep the current timeless rule. Once an
actor appears in `.vela/actors.json`, every matching human event must carry a
valid signature. This compatibility fallback fails closed for old frontiers.

## The Erdős evidence

The standalone Erdős frontier at
`d0a2f56dfecf7027248403e43ba133e18e56b3c6` exposes the concrete failure.
Commit `bba62e8d85393887f00caffe7c28d005a1786b3f` registered
`reviewer:will-blair` with this record:

```json
{
  "id": "reviewer:will-blair",
  "public_key": "4892f93877e637b5f59af31d9ec6704814842fb278cacb0eb94704baef99455e",
  "algorithm": "ed25519",
  "created_at": "2026-07-16T14:01:49.316573+00:00"
}
```

The registration commit has these exact roots:

| Field | Value |
| --- | --- |
| Git commit | `bba62e8d85393887f00caffe7c28d005a1786b3f` |
| Git tree | `2c5c5a6c688a274b40017321d920f258f1c70c04` |
| Event count | `2185` |
| Event-log root | `sha256:e16e57f2e39957ddc2fb529e1b715547c27935fa04730e3c78b4a997eb6f4678` |
| Snapshot root | `sha256:61a6817b0f5ba3620faaf99061a7d356b9dc38c3cfe0a4b7796fcdb690dd16fb` |
| Actor-registry byte root | `sha256:665f3e1c48f0a50fac949681c0af01bdd28de2991f2cdc5cc4cddbe69df6311b` |
| Actor-registry Git blob | `21128fbd8183b85d1ed6b4da3340f7d4bd5bf6cd` |

The current frontier contains 213 events from this actor:

| Class | Count |
| --- | ---: |
| Unsigned `finding.asserted` | 75 |
| Unsigned `finding.retracted` | 6 |
| Signed `finding.asserted` | 12 |
| Signed `statement.attested` | 111 |
| Signed `verifier_attachment.added` | 8 |
| Signed `review.accepted` | 1 |
| Total unsigned | 81 |
| Total signed | 132 |

All 81 unsigned events and 131 of the signed events were present at the
registration root. The only later matching event is signed
`review.accepted` event `vev_27922b9c8dab0575`.

Released `v0.800.19` produces these results:

- `vela check . --json` passes and reports 81
  `unsigned_registered_actor` signals.
- `vela check . --strict --json` fails because those 81 signals block
  `strict_check` and `proof_ready`.
- reducer replay succeeds and reports no structural conflict.

The signal implementation looks up the actor ID in the current registry and
applies the public key to every matching human event. It does not consult
`ActorRecord.created_at`. Decision-time authority uses `created_at`, so Vela
currently has two different temporal interpretations of the same actor record.

Registration changed `.vela/actors.json`, the materialized snapshot, the lock,
and proof projections. It did not change the event-log root. The later addition
of the signed governance event changed the event log as expected.

## Protocol primitive

ADR 0005 proposes one event kind and one closed payload. It does not add an
object family, identifier prefix, signature algorithm, registry service, or
accepted-state store.

### Event shape

An activation uses the existing `vela.event.v0.1` envelope:

```json
{
  "schema": "vela.event.v0.1",
  "id": "vev_...",
  "kind": "actor.registration_activated",
  "target": {
    "type": "actor",
    "id": "reviewer:example"
  },
  "actor": {
    "type": "human",
    "id": "reviewer:example"
  },
  "timestamp": "2026-07-16T00:00:00Z",
  "reason": "Activate signature enforcement after the anchored history.",
  "before_hash": "sha256:null",
  "after_hash": "sha256:null",
  "payload": {
    "schema": "vela.actor-registration-boundary.v1",
    "mode": "temporalize_existing",
    "frontier_id": "vfr_...",
    "actor_id": "reviewer:example",
    "public_key": "<64 lowercase hex>",
    "algorithm": "ed25519",
    "anchor": {
      "git_object_format": "sha1",
      "git_commit": "<40 lowercase hex>",
      "git_tree": "<40 lowercase hex>",
      "event_log_root": "sha256:<64 lowercase hex>",
      "event_count": 1,
      "actor_registry_root": "sha256:<64 lowercase hex>"
    }
  },
  "caveats": [
    "The anchor preserves legacy bytes but does not authenticate unsigned history."
  ],
  "signature": "v1:<128 lowercase hex>"
}
```

The event uses null state hashes because it changes no finding. It records an
identity-enforcement boundary and enters the canonical audit log.

The event ID and signature follow the existing canonical event rules. The
ordinary event signature binds the actor, timestamp, target, anchor, mode,
reason, and caveats.

### Modes

`temporalize_existing` applies to a timeless actor record already present at
the anchor.

- The anchored actor registry must contain exactly one record with the payload
  actor ID, public key, and algorithm.
- The activation signature must verify with that anchored public key.
- The current actor record must retain the same ID, public key, and algorithm,
  unless a separate valid rotation or revocation path accounts for the change.

`bootstrap` applies to an empty actor registry.

- The anchored actor registry must be empty.
- The payload public key verifies the activation signature as proof of
  possession.
- The registration transaction must install one matching actor record and the
  activation event together.

`bootstrap` does not extend an established registry. Later membership and key
rotation continue to require signed governance. ADR 0005 does not define that
governance path.

### Anchor resolution

Strict verification resolves the anchor from the frontier's Git repository:

1. Require the full commit and tree objects.
2. Require the anchor commit to be an ancestor of the checked Git revision.
3. Read the exact `.vela/events/*.json` and `.vela/actors.json` bytes at the
   anchor commit.
4. Recompute the anchor tree, event count, event-log root, and actor-registry
   byte root.
5. Validate every anchored event ID and event content preimage.
6. Validate the activation signature with the key selected by its mode.
7. Inspect descendant Git history for an activation event that was later
   removed, and reject the removal even when the actor record was removed in
   the same change.

The actor-registry root is SHA-256 over the exact `.vela/actors.json` bytes at
the anchor. The Git commit and tree already bind those bytes; the SHA-256 field
keeps the Vela payload explicit and independent of Git's object format.

A shallow clone or exported directory that lacks the anchor cannot establish
the exemption. Strict verification fails with a typed anchor-unavailable
blocker. A complete Git bundle can carry the required objects for offline use.

### Pre-registration membership

The anchored event set defines pre-registration history. Event timestamps do
not affect membership.

For each anchored event, the current frontier must contain an event with the
same ID and canonical event content preimage. The comparison excludes the
signature field, matching Vela's event ID and event-log commitment rules.

This permits a key holder to add a valid signature to an anchored unsigned
event without changing its event ID. It does not permit semantic edits,
deletion, or replacement.

An event absent from the anchor is post-registration for this actor. It must
carry a valid signature even when its timestamp sorts before the activation
event or before the actor record's `created_at`.

### Anchored signature preservation

The activation event grants no permission to strip earlier signatures.

- An anchored unsigned event may remain unsigned.
- An anchored unsigned event may gain a valid signature under the activated
  key.
- An anchored signed event must retain a valid signature under the activated
  key.
- An anchored event with an invalid signature makes activation invalid.
- Removing a valid anchored signature produces a strict blocker.

These rules preserve existing authenticated history while leaving unsigned
legacy history unauthenticated.

## Strict and non-strict behavior

Frontiers without a valid activation event keep the timeless rule.

A valid activation changes signal classification for matching actor events:

| Event class | Signal | Strict effect |
| --- | --- | --- |
| Anchored, unsigned, valid content | `pre_registration_unsigned_actor_event` | informational; no `strict_check` or `proof_ready` block |
| Anchored, signed, valid signature | none | pass |
| Anchored, signed at anchor, signature removed or invalid | `pre_registration_signature_lost` | block |
| Post-anchor, valid signature | none | pass |
| Post-anchor, missing or invalid signature | `unsigned_registered_actor` | block |
| Activation or anchor invalid | `actor_registration_anchor_invalid` | block; grant no exemption |
| Anchor Git objects unavailable | `actor_registration_anchor_unavailable` | block; grant no exemption |

Non-strict mode retains its current reporting convention. It may return success
when no structural errors exist, but it must include the typed signals and
proof-readiness state. An invalid or unavailable activation must never suppress
the timeless blockers in either mode.

For Erdős, a valid activation should reclassify 81 actor-signature blockers as
81 informational legacy records. Other strict blockers remain. ADR 0005 does
not claim that the frontier becomes proof-ready.

## Migration and replay

### Existing frontiers

Actor records without an activation event retain the `v0.800.19` timeless
semantics. New Vela releases must replay those frontiers without changing event
bytes, event IDs, or roots.

A frontier may opt into temporal semantics through a human-only activation
ceremony. The future command should:

1. load and display the actor record;
2. display the selected anchor commit, tree, event root, event count, registry
   root, and historical signature counts;
3. state that unsigned anchored events remain unauthenticated;
4. require one confirmation;
5. read the matching human key once;
6. append one signed activation event through the recoverable frontier
   transaction; and
7. publish through the existing exact Git transaction.

An agent may prepare the inspection report and conformance fixture. An agent
may not confirm the boundary, read the key, sign the event, or perform the
ceremony.

### Erdős migration

The proposed Erdős activation uses:

```text
mode: temporalize_existing
actor: reviewer:will-blair
anchor commit: bba62e8d85393887f00caffe7c28d005a1786b3f
anchor tree: 2c5c5a6c688a274b40017321d920f258f1c70c04
anchor event count: 2185
anchor event-log root:
  sha256:e16e57f2e39957ddc2fb529e1b715547c27935fa04730e3c78b4a997eb6f4678
anchor actor-registry root:
  sha256:665f3e1c48f0a50fac949681c0af01bdd28de2991f2cdc5cc4cddbe69df6311b
```

The migration must append one activation event. It must leave all 2,185
anchored event files byte-identical. The later signed
`vev_27922b9c8dab0575` event must continue to verify.

The local unreleased implementation produced a key-free preview against the
clean standalone frontier at
`d0a2f56dfecf7027248403e43ba133e18e56b3c6`. The preview root is
`sha256:87c5a97b6d7944a6025334e24af40a73dd474402068b77cc0b1af92ec1005175`.
It re-derived 81 anchored unsigned events, 131 anchored signed events, zero
post-anchor unsigned events, and one post-anchor signed event. The command read
no key and left Git and frontier bytes unchanged. The exact record is
`conformance/erdos-actor-registration-preview-v1.json`.

Expected actor-signature results after migration:

- 81 anchored unsigned events reported as legacy and unauthenticated;
- zero `unsigned_registered_actor` blockers among anchored events;
- every existing anchored signature still valid;
- every post-anchor matching event signature required; and
- no change to unrelated strict signals.

### Replay compatibility

The event introduces a new known kind, so a pre-ADR binary may reject a
frontier that contains it. That rejection is an intentional protocol-version
boundary. It does not justify rewriting old history.

The implementing release must retain:

- old event and actor decoding;
- timeless behavior when no activation event exists;
- signature-independent event IDs and event-log roots;
- current revocation checks; and
- deterministic replay from a complete clone or Git bundle.

Git history supplies immutable bytes and ancestry. The activation signature
supplies the actor's key-bound decision. A Git commit, merge, tag, or host
permission does not activate an actor by itself.

## Adversarial cases

### Backdated insertion

An attacker appends an unsigned matching event with a timestamp before the
activation. The event is absent from the anchor set, so strict verification
requires a signature and blocks it.

### Anchor substitution

An attacker changes the anchor commit, tree, count, event root, or registry
root. The activation signature no longer verifies, or the recomputed anchor
does not match. Strict verification grants no exemption.

### Non-ancestor anchor

An attacker points to a commit from another fork with matching-looking files.
The commit is not an ancestor of the checked revision. Strict verification
blocks the activation.

### Registry replacement

An attacker changes the current public key and signs a replacement activation
with the new key. `temporalize_existing` resolves the verification key from the
anchored registry, so the attacker still needs the anchored private key.

### Registry deletion

An attacker deletes the current actor record. The activation event remains in
the event log and references a missing actor binding. Strict verification
blocks the frontier.

### Activation deletion

An attacker deletes the activation event and regenerates derived files. With
the actor record intact, the frontier returns to timeless behavior and the
unsigned matching events become strict blockers. With both the actor record
and event removed, the complete Git history still contains the activation.
Strict verification rejects a descendant that removes an activated boundary.

A force-pushed history that omits the old commit is a new delivered lineage.
A consumer with a pinned prior root rejects the non-descendant history. A fresh
un-pinned clone cannot infer bytes that its source withheld; this is the same
Git-custody limit that applies to deletion of any accepted event.

### Historical event deletion or mutation

An attacker deletes an anchored event or changes its semantic content. Anchor
continuity fails. A changed event also fails its content-addressed ID check.

### Signature stripping

An attacker removes a valid signature from an anchored signed event. The
anchored Git object proves that the event carried a signature. Strict
verification emits `pre_registration_signature_lost`.

### Shallow history

A clone omits the anchor commit. Non-strict output reports the missing anchor.
Strict verification blocks and applies no historical exemption.

### Duplicate boundary

Two activation events target the same actor. Strict verification rejects the
ambiguous boundary. Key rotation or a second signed era requires its own
governance design rather than precedence by timestamp.

## Options considered

### Timeless registration

Timeless registration preserves `v0.800.19` behavior and remains the default
for actor records without an activation event. It binds all matching history to
the current key. Erdős shows the cost: registering one reviewer turned 81
unchanged legacy events into strict blockers.

This option fails closed but creates retroactive key-custody work whenever a
frontier adopts registration after accepting unsigned history.

### Registration effective from an exact signed root

Selected.

The signed activation event binds the key decision to a Git commit, Git tree,
Vela event root, event count, and registry root. Exact anchor membership
prevents timestamp backdating. The event log preserves the boundary after
registry edits.

### New actor identity for the signed era

A new identity can separate unsigned and signed eras on a greenfield frontier.
It does not repair Erdős. The frontier already contains 132 valid signatures
under `reviewer:will-blair`, including the governance event. Removing the old
registry binding would discard verifiable authority evidence. Keeping both
bindings would leave the 81 timeless blockers.

### Re-sign all 81 historical events

Vela event IDs and event-log roots exclude signatures, so a human can add
signatures without changing event identity. This option remains fail-closed.

It still rewrites 81 canonical files, asks a key holder to attest historical
events one by one or through a new batch ceremony, and lacks maintained
released porcelain in `v0.800.19`. It also converts legacy records into
authenticated events, which is a stronger claim than preserving their original
status.

### Use `ActorRecord.created_at`

Rejected.

The registry file can be rewritten, and an event writer controls event
timestamps. A timestamp cutoff lets an attacker backdate a new unsigned event
into the exempt era. It cannot prove which event bytes existed when the actor
registered.

### Add a boundary field only to `.vela/actors.json`

Rejected.

A registry-only field leaves no canonical event after deletion or replacement.
An attacker could remove the temporal field and replace the registry in the
same Git change. A signed event keeps the boundary in the append-only audit
log and makes deletion fail closed.

## Exact conformance contract

Implementation must add a compact Git fixture with an anchor commit and a
descendant activation commit. The fixture must use current event bytes and
ordinary Git objects rather than a mocked ancestry flag.

| Test | Required result |
| --- | --- |
| `legacy_actor_without_boundary_remains_timeless` | anchored-looking unsigned matching event still emits `unsigned_registered_actor` |
| `valid_boundary_exempts_only_anchor_members` | anchored unsigned event becomes informational; post-anchor unsigned event blocks |
| `backdated_post_anchor_event_requires_signature` | post-anchor event with an earlier timestamp blocks |
| `valid_post_anchor_signature_passes` | signed event absent from anchor passes |
| `anchor_signature_is_preserved` | valid anchored signed event passes |
| `anchor_signature_removal_fails` | removal emits `pre_registration_signature_lost` |
| `unsigned_anchor_event_may_gain_signature` | valid added signature passes without event ID or event-log-root drift |
| `anchor_event_deletion_fails` | missing anchored event invalidates activation |
| `anchor_event_mutation_fails` | event content or ID drift invalidates activation |
| `anchor_root_mismatch_fails` | wrong tree, count, event root, or registry root invalidates activation |
| `anchor_must_be_ancestor` | unrelated fork anchor fails |
| `missing_anchor_objects_fail_closed` | shallow history grants no exemption |
| `registry_key_tamper_fails` | replacement key cannot verify the anchored activation |
| `registry_record_deletion_fails` | activation cannot resolve the current actor |
| `activation_deletion_returns_to_timeless` | legacy unsigned events block again |
| `activation_and_registry_deletion_fails_with_history` | descendant Git history proves the removed activation and strict verification blocks |
| `duplicate_actor_boundaries_fail` | no timestamp or file-order precedence |
| `bootstrap_requires_empty_anchor_registry` | non-empty registry rejects bootstrap |
| `bootstrap_proves_key_possession` | payload key verifies the activation and current record matches |
| `non_strict_reports_without_exempting_invalid_anchor` | command may pass, signals remain complete, and timeless blockers remain |
| `erdos_registration_regression_vector` | exact `81 unsigned / 132 signed / 1 later signed` evidence and roots match the frozen manifest |

The focused implementation commands are:

```bash
cargo test -p vela-protocol actor_registration_boundary
cargo test -p vela-edge temporal_actor_registration
cargo test -p vela-cli --test task_first_workflows temporal_actor_registration
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

External Lean, Diderot, live-network checks, provider benchmarks, and broad
release suites are outside this ADR's focused implementation gate.

## Consequences

The selected design asks a key holder for one future activation signature
instead of historical event signatures. It preserves the original
authentication status of pre-registration events and keeps future writes
key-bound.

Strict verification gains a dependency on the anchor Git objects for
frontiers that opt into temporal activation. Complete clones and Git bundles
already carry those objects. Shallow exports must report that they cannot
establish the boundary.

The event kind expands the known-event registry. The expansion addresses a
reproduced failure that a registry-only change cannot handle without a
tampering gap.

## Non-goals

ADR 0005 does not:

- authenticate or assign authorship to unsigned legacy events;
- accept, reject, revise, or retract a scientific finding;
- define general actor membership governance;
- replace existing key rotation or revocation rules;
- make Git commits scientific authority;
- let an agent run a human ceremony;
- repair unrelated Erdős condition or provenance debt;
- add a hosted identity service; or
- authorize implementation before the owner accepts this ADR.

## Review questions

Owner review should confirm:

1. whether one signed activation event is the smallest acceptable primitive;
2. whether complete Git history is an acceptable requirement for temporal
   activation;
3. whether an anchored signed event may accept a different valid signature or
   must preserve the exact signature bytes; and
4. whether `bootstrap` belongs in ADR 0005 or should wait for a separate
   registration-governance ADR.
