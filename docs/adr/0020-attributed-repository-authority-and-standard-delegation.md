# ADR 0020: Attributed repository authority and standard delegation

- Status: Proposed
- Target release: Vela `v0.930.0`
- Protocol effect: new authority keyset, policy bundle, authority record, and
  transaction-linked event forms
- Product effect: replace personal Vela key setup and `vela-signer` with
  ordinary authentication, scoped delegation, and one repository-authority
  transaction edge
- Scientific authority effect: preserve human or governed scientific judgment;
  change how that judgment is authenticated, authorized, and recorded
- Compatibility: all Vela `0.915` history remains byte-identical and
  read-verifiable; only the new authority model has a live writer after
  migration
- Entry gate: source audit of the released `0.915.1` authority stack, repeated
  first-party usability failure, and the decision-grade authority architecture
  review dated 2026-07-24

## Context

Vela protects the right invariant through the wrong identity and signing
model.

The invariant is:

```text
evidence is not a verdict
verification is not acceptance
an authorized decision changes standing
the exact transition is replayable
corrections append
```

That invariant is Vela's core contribution and remains unchanged.

The released implementation additionally makes Vela responsible for:

- long-lived human Ed25519 identities;
- per-Frontier actor-key registration;
- raw seed custody through a custom cross-platform helper;
- OS authentication and prompt adapters;
- signed local signer sessions;
- binary and helper digest rebind ceremonies;
- manually echoed Decision Plan roots and timestamps;
- three generations of a custom AcceptancePolicy language; and
- long-lived agent credentials used as policy inputs.

The implementation is rigorous, but it is not a non-exportable signing
boundary. The protected helper retrieves a raw Ed25519 seed into process memory
before signing. It pays the product and platform cost of a bespoke custody
system without delivering the lifecycle, recovery, institutional membership,
or hardware-backed guarantees of a mature identity system.

AcceptancePolicy v0.1 through v0.3 closed real authority gaps. Their evolution
also demonstrates that Vela is becoming an authorization-language vendor.
Exact verifier, packet, result-contract, replayability, and producer-credential
constraints are legitimate authorization inputs. They do not justify a fourth
custom policy generation.

The current Decision Plan mechanism is valuable internal transaction plumbing.
Asking a person to copy its digest and timestamp is not. A person should decide
the scientific meaning shown in a contextual review surface. The transaction
edge should rederive and bind the exact bytes.

The target is not “just trust a database” and it is not “make GitHub the
authority.” The target is portable process authority:

```text
authenticated principal
  + current role or scoped delegation
  + restricted Cedar decision
  + semantic approval when consequence requires it
  + final read-set recheck
  + repository-authority signature
  -> attributable scientific authority
```

## Decision

Adopt attributed repository authority for every new write after an explicit
per-Frontier migration boundary.

### 1. Keep the scientific-state kernel

Retain:

- canonical JSON and full SHA-256 identities;
- Receipt v1 as the producer/evidence boundary;
- deterministic verifier attachments;
- proposal objects;
- append-only events and corrections;
- deterministic replay and clean-clone verification;
- the recoverable transaction journal;
- exact intent and read-set binding;
- human judgment for exceptional scientific decisions;
- prior human authorization for automatic policy lanes; and
- structural prevention of agent self-approval.

Git remains transport, lineage, and distributed backup. Git publication,
artifact provenance, deterministic verification, policy authorization, and
scientific standing remain separate facts.

### 2. Replace personal signing with one authority transaction

Introduce three canonical object families and one new event form:

```text
vela.authority-keyset.v1
vela.policy-bundle.v1
vela.authority-record.v1
vela.event.v1
```

A transaction may create multiple canonical objects and events. One authority
record covers the complete transaction:

- operation and transaction ID;
- exact internal `intent_digest`;
- previous authority-record digest and sequence;
- before and after event-log roots;
- every generated event ID;
- a canonical object-delta manifest;
- authenticated principal and attribution snapshot;
- delegation or workload claims;
- Cedar request, bundle, entity snapshot, outcome, diagnostics, and determining
  policies;
- semantic approval and quorum records when required;
- final read-set and execution identity;
- authority key ID; and
- one repository-authority signature.

The signature means:

> The repository authority attests that this exact transaction passed the
> recorded authentication, authorization, approval, verification, quorum, and
> final-state checks.

It does not mean that a personal key signed every event.

### 3. Use DSSE as the authority envelope

The canonical `vela.authority-record.v1` bytes are wrapped in one DSSE envelope
with payload type:

```text
application/vnd.vela.authority-record.v1+json
```

DSSE authenticates both payload type and payload and avoids inventing another
signature envelope. The authority-record payload remains canonical JSON for
Vela content addressing. DSSE key management is intentionally out of scope;
the authority keyset supplies the Frontier-specific verification history.

An authority record ID is derived from its unsigned canonical payload. The
DSSE envelope, signature, and authority key ID are verified before the record
may enter the authority log.

### 4. Separate human identity from authority keys

`vela.principal.v1` uses an exact namespaced issuer-subject identifier:

```text
local:<device-id>|uid:<uid>
oidc:<issuer>|<subject>
orcid:<issuer>|<subject>
```

A human principal ID must equal one retained local, OIDC, or ORCID account
link. It is never inferred from email, display name, affiliation, a GitHub
handle, or an unlinked ORCID value. Display name and affiliation are readable
snapshots only. Current account links and governed role bindings determine
authorization. Revocation appends to the account-link history rather than
silently reassigning a principal.

Non-human principal namespaces are explicit:

```text
agent:<provider>:<agent-id>:<run-id>
workload:<provider>:<workload-id>
service:<service-id>
institution:<institution-id>
```

Human authentication is replaceable:

- solo local mode uses the existing OS login session;
- online workbenches may use passkeys/WebAuthn or OIDC;
- institutions may use SSO and managed lifecycle;
- disconnected high-assurance mode may use a standard hardware-backed direct
  signer over the same authority-record payload.

Authentication proves control of a session or authenticator. Cedar determines
authorization. A semantic action supplies transaction approval. None alone
implies scientific correctness.

### 5. Make the repository authority a service identity

`vela.authority-keyset.v1` records:

```text
frontier_id
generation
threshold
keys[]
  key_id
  algorithm
  public_key
  valid_from_sequence
  valid_through_sequence
  purpose
previous_keyset_root
activation_record_root
```

The repository authority is not a person and never appears as the scientific
reviewer. Its private key may be held by a standard provider:

- SSH agent for the first solo local implementation;
- PKCS#11 token;
- cloud KMS or HSM;
- GitHub App authority process; or
- institutional authority service.

The provider signs only an already validated authority-record payload. It
receives no proposal prose, model output, repository file paths, or policy
authoring context beyond the digest and domain-separated payload required to
sign.

### 6. Replace AcceptancePolicy with restricted Cedar

Freeze AcceptancePolicy v0.1 through v0.3 as legacy replay inputs. There will
be no AcceptancePolicy v0.4 and no new legacy policy writer after migration.

`vela.policy-bundle.v1` binds:

```text
manifest
schema.cedarschema
policies.cedar
entities.json
tests/
engine name and exact version
restricted Vela profile version
previous bundle digest
activation authority record
human-readable authority summary
```

The first implementation pins `cedar-policy` exactly and disables its
top-level default features. Cedar core currently still compiles its built-in
extension implementations, so Vela additionally rejects extension
constructors and extension values before evaluation; compiled availability is
not protocol availability. The restricted Vela profile permits no network
access, file I/O, runtime policy fetch, model decision, nondeterministic score,
or registered extension value.

Closed policy and authority-record types remain in dependency-light
`vela-protocol`. The Cedar runtime lives in the narrow `vela-authority` crate.
Protocol consumers that only replay or inspect canonical records therefore do
not inherit the policy engine or its dependency graph.

Every policy and request is schema-validated before evaluation. Cedar's
authorization API skips an individual policy that errors; Vela therefore
applies the stricter application rule:

```text
any validation error
or any evaluation diagnostic
or any unknown action/entity/attribute
=> no automatic authorization
```

Cedar returns Allow or Deny. Vela preserves its domain routing:

- structural invalidity or an applicable `forbid` is `Deny`;
- valid proposal creation without authorization to auto-admit is `Defer`;
- one applicable `permit`, no forbid, no diagnostics, and all deterministic
  gates passing is `Permit`.

Quorum counting remains a small deterministic Vela state machine. Cedar
determines whether a principal may cast a decision; it does not implement a
temporal voting protocol.

### 7. Use short-lived capabilities for agents and workloads

Agents and workloads are non-human principals. They use short-lived,
audience-, Frontier-, resource-, action-, and expiry-bound capabilities.

The first capability profile is the closed, content-addressed
`vela.capability-grant.v1`. It records:

```text
issuer
subject
subject class
current actor
actor chain
parent capability full root
delegation depth and maximum depth
audience
frontier
actions
resource bindings
exact execution bindings
consequence ceiling
issued_at
not_before
expires_at
token_id
revocation reference
```

The profile permits only `agent` and `workload` subjects, the exact
`vela.repository-authority.v1` audience, full resource and execution roots,
and a maximum lifetime of 24 hours. Delegation depth is at most one. A child
must name the full parent root and may only narrow action, resource, execution,
time, and consequence scope. Expiry and revocation are evaluated at the
authority record's observation time, so a backdated operation cannot revive a
grant.

Runtime bearer credentials are never committed. The authority record retains
only `vela.verified-capability-claim.v1` fields and the grant identity, never a
JWT, CWT, OAuth token, refresh token, cookie, or provider assertion. OAuth
token exchange, OIDC, SciTokens, GitHub App identity, and local credentials are
replaceable adapters, not Vela protocol dependencies. DPoP or SPIFFE is added
only for a reproduced network or institutional workload threat.

An agent or workload can never obtain:

```text
authority_migrate
authority_revoke
authority_rotate
bulk_correct
destroy
membership_manage
policy_activate
policy_revoke
policy_rotate
quorum_manage
recovery_approve
review_accept
review_reject
```

Those actions are absent from the capability action enum and are also rejected
by the shared application invariant before Cedar evaluation. The same closed
list is used by `vela-protocol` and `vela-authority`, so policy text cannot
manufacture an agent or workload decision path.

### 8. Keep exact intent binding; remove root ceremonies

Rename Decision Plan concepts internally:

```text
intent_digest
transaction_plan
approval_intent
expected_before_root
```

The intent binds the exact canonical machine inputs and the exact sanitized
semantic summary shown to the user. The client holds an opaque short-lived
intent handle. The user never copies:

- a hash;
- timestamp;
- key fingerprint;
- helper digest; or
- policy schema identifier.

Routine policy-covered work requires no prompt. An exceptional decision
requires one semantic action. Policy, membership, recovery, authority rotation,
and high-impact bulk changes require one semantic review plus step-up or quorum
when the active policy requires it.

Cancellation, stale state, authentication failure, authorization failure,
quorum failure, signer failure, or read-set drift writes no canonical byte.

### 9. Link events without a hash cycle

`vela.event.v1` replaces per-event authority signatures with:

```text
transaction_id
attribution principal and authority mode
exact event content
```

Vela derives the complete event set and resulting event-log root first. It then
builds and signs the authority record covering those event IDs and roots.
Events, retained objects, the authority record, and required policy/key
snapshots install atomically.

The one legacy migration bridge remains under `.vela/events/`. Every
post-migration `vela.event.v1` is stored separately under
`.vela/authority/events/`, with its covering DSSE envelope under
`.vela/authority/records/`. The separation is load-bearing: the legacy event
loader continues to accept only `StateEvent` bytes, while dual-history replay
reads the Era-1 event and record stores explicitly.

Strict replay requires every post-migration canonical mutation to be covered
exactly once by a valid authority record. Gaps, overlaps, duplicate coverage,
sequence reuse, wrong transaction IDs, wrong before/after roots, or authority
forks fail closed.

The record's execution write-set root commits to the semantic transition:
transaction ID, before and after authority-event-log roots, sorted event IDs,
and exact object deltas. The covering DSSE envelope is excluded from that
semantic root to avoid a self-referential hash cycle. The existing recoverable
repository transaction still commits to and verifies every exact event and
envelope postimage before installation.

### 10. Preserve two verification eras and one live writer

Verification recognizes:

```text
Era 0: legacy per-event signatures and AcceptancePolicy certificates
Era 1: transaction authority records and Cedar bundles
```

Only Era 1 has a live writer after a Frontier migrates. Era 0 remains a
read-only verification obligation. A post-migration legacy authority event is
a strict blocker; it is not silently interpreted under the old rules.

Offline replay needs no live IdP, passkey service, Git host, KMS, policy
service, Fulcio, or Rekor lookup. It validates retained authentication claims
as claims attested by the repository authority at transaction time. It does
not claim to reauthenticate the biological person offline.

### 11. Cross the boundary once without rewriting history

Each active Frontier receives one explicit continuity bridge:

```text
vela.authority-model-migration.v1
frontier_id
legacy event-log root
legacy actor-registry root
legacy active-policy head and store-manifest roots
new authority-keyset digest
new policy-bundle digest
new principal ID
new minimum writer version
migration reason
```

The current authorized human key signs this one migration event. Authority
record sequence 1, signed by the new repository authority, covers that event
and begins the new chain.

No historical event is re-signed or rewritten. If the legacy key is
unavailable, Vela creates no fictional continuity. The only fallback is an
explicit trust reset with a new out-of-band trust anchor and a read-only legacy
lineage.

## Migration and release plan

The implementation sequence is normative:

1. add dual-read schemas, validation, and authority-record verification;
2. translate AcceptancePolicy v0.1-v0.3 to Cedar;
3. shadow-evaluate every retained and hostile policy vector;
4. require Cedar to be identical or stricter and never introduce a new Permit;
5. implement the authority transaction edge and one standard SSH-agent
   provider;
6. migrate a disposable fixture Frontier;
7. migrate one low-risk active Frontier;
8. migrate Formal, Quantum, Sidon, and Erdős only after the earlier gates pass;
9. reject every live legacy writer after its Frontier marker changes;
10. delete `vela-signer`, identity-v2 custody, signer sessions, rebind flows,
    AcceptancePolicy authoring, and long-lived agent onboarding;
11. retain only the minimum Era-0 verifier; and
12. release Vela `v0.930.0` only after every active Frontier and clean-clone
    verifier passes.

ADRs 0011 through 0014 remain historical records of the problems they solved.
ADR 0020 supersedes their writer and product decisions only after the new
migration marker is active. Vela `0.915.1` remains the exact Era-0 replay
baseline.

## Strict and non-strict behavior

Strict mode blocks:

- an invalid or missing migration bridge;
- an unknown authority writer marker;
- a missing, malformed, forked, or non-contiguous authority record;
- an invalid DSSE envelope or authority signature;
- an authority key outside its sequence range;
- missing or duplicate event coverage;
- wrong before/after roots or transaction IDs;
- a missing, unvalidated, or altered Cedar bundle;
- policy or evaluation diagnostics on an automatic path;
- a stale or unauthorized principal, role, capability, or quorum;
- a post-migration legacy write; and
- any retained-object or canonical-history mutation.

Non-strict mode reports the same defects and grants no standing, Permit,
exemption, or fallback. Unknown authority is never interpreted as legacy
authority.

## Adversarial cases

Conformance must reject:

- an agent represented as a human through a changed type or string prefix;
- an agent approving or broadening its own capability;
- an expired, wrong-audience, wrong-Frontier, wrong-resource, replayed, or
  over-delegated capability;
- a changed policy, entity snapshot, schema, evaluator version, request,
  semantic summary, reviewer set, event, object delta, or state root;
- a Cedar `permit` attempting to shadow an unconditional `forbid`;
- a skipped Cedar evaluation error being mistaken for safe authorization;
- a stale role or recovered account used contrary to the active bundle;
- an authority record replayed at another sequence or Frontier;
- signature verification under an inactive, revoked, or substituted key;
- an uncovered, multiply covered, or reordered event;
- an authority-log fork or rollback;
- a Git merge, CODEOWNERS approval, Sigstore attestation, or verifier pass being
  treated as scientific acceptance;
- historical re-signing presented as continuity; and
- a live network dependency during clean-clone replay.

## Exact conformance contract

Focused implementation gates:

```bash
cargo test -p vela-protocol authority_record
cargo test -p vela-authority cedar_profile
cargo test -p vela-protocol legacy_policy_translation
cargo test -p vela-cli authority_transaction
cargo test -p vela-cli authority_migration
cargo test -p vela-protocol --test authority_era_fixtures
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

Required vectors include:

- authority-record canonicalization, ID, DSSE domain separation, and signature;
- keyset rotation, revocation, sequence windows, fork, and rollback;
- event gap, overlap, duplicate coverage, wrong transaction, and wrong roots;
- schema and request validation;
- Cedar default deny, forbid precedence, and Vela deny-on-diagnostic behavior;
- exact v0.1, v0.2, and v0.3 policy translation;
- no-new-Permit shadow evaluation over every retained policy decision;
- agent/workload capability scope, expiry, audience, and delegation depth;
- semantic-summary substitution and transaction TOCTOU;
- cancellation and authentication/authorization/signer failures with zero
  writes;
- byte-identical Era-0 replay;
- one exact migration bridge and rejection of later legacy writes;
- clean-clone network-disabled replay of both eras; and
- unchanged accepted scientific state from authority migration alone.

The full deterministic release union runs once at the actual `v0.930.0`
boundary. External Lean, Diderot, and live-network suites remain excluded
unless a selected migration fixture directly requires them.

### Implementation evidence, 2026-07-24

The read-only candidate now implements the closed bridge payload and dual
history verifier. It preserves the ordinary Era-0 event-log commitment through
authority-record sequence 1. Subsequent mixed-history commitments use
`vela.authority-event-log.v1`, binding that frozen legacy root and the sorted
full roots of covered Era-1 events.

Focused fixtures prove:

- byte-identical legacy-only replay remains valid;
- the bridge requires an exact unrevoked actor-registry key and legacy event
  signature;
- sequence 1 covers only the bridge and binds the new keyset, policy bundle,
  principal, semantic approval, and event object root;
- later transactions have exact unique event coverage and attribution;
- extra legacy events, missing or duplicate coverage, transaction
  substitution, wrong roots, chain forks, registry drift, signature
  tampering, policy substitution, and unknown bridge fields fail closed; and
- the reducer treats the bridge as non-scientific.

This evidence does not accept the ADR, enable a live Era-1 writer, or migrate
a Frontier.

The CLI-unreachable sequence-1 candidate now implements the corresponding
writer boundary. It verifies the already legacy-signed bridge against the
exact held Era-0 history and initial manifests before runtime authentication
or repository signing. It then prepares one recoverable transaction covering
the bridge, initial full-root keyset, initial full-root policy bundle, and
sequence-1 DSSE record. Cancellation, signer refusal, legacy-signature drift,
runtime-policy byte substitution, and marker-time membership drift install no
canonical postimage. A committed partial install recovers and exact retry
requires neither another authentication nor another signature. The
application-level machine-authority forbid list explicitly includes
`authority_model_migrate`.

This completes only the sequence-1 installation sub-gate. The read-side
rotation law and its CLI-unreachable writer are now implemented and
adversarially tested. Emergency close, the full disposable Frontier drill,
active migration, CLI exposure, and ADR acceptance remain open.

Phase 1 now also has a committed
`vela.authority-history-conformance.v1` vector at
`sha256:5a609f00f97f9bda79ffceb77f34edfdc4b1ad3c1252f28b844b45b0d1f23806`.
Sequence 1 binds the full canonical signed migration-event root as its semantic
intent and contains exactly three initial object deltas: the bridge event, the
initial authority keyset snapshot, and the initial restricted policy snapshot.
The fixture was deliberately regenerated before any Era-1 writer release to
place post-migration events under `.vela/authority/events/` rather than the
legacy `StateEvent` directory.
An independent Python path rederives both event forms, legacy and mixed roots,
the actor-registry and policy inputs, the legacy Ed25519 signature, both DSSE
authority-record signatures, sequence and threshold rules, clean pinned Cedar
authorization, transaction coverage, attribution, and object deltas. Six
historical/DSSE hostile vectors and four authentication-observation hostile
vectors fail closed. The same vector passes from the exact detached Git commit
in a clean clone with network access denied. Phase 1 is therefore complete.
This advances the candidate only to the runtime authentication and
authority-transaction gates; it does not authorize a writer or substitute for
the deterministic release union at an actual release boundary.

The vector was deliberately regenerated before any writer release when the
provisional arbitrary authentication strings were replaced by the closed
`vela.authentication-observation.v1` contract. The observation binds exact
principal class and issuer-subject, closed method and assurance, a full
non-secret session root, authentication/observation/expiry times,
presence/verification facts, recovery context, and revocation reference.
Runtime cookies, bearer tokens, assertions, and raw session identifiers remain
outside canonical history.

Phase 3 now has a committed `vela.principal-capability-conformance.v1` vector
at
`sha256:67bf660a0733bbc7579a883e8cc2e1b9ae09843e6ecee856794e2c07f1f5ef2d`.
The Rust and independent Python paths rederive the exact human
issuer-subject identity, agent grant, attenuated workload child, and
bearer-free verified claim. Eight hostile cases reject email-based identity
inference, human capability subjects, human-only actions, lifetimes over 24
hours, delegation broadening, parent substitution, bearer-token retention,
and revocation. The application-level human-action forbid now consumes the
same protocol-owned list used by the capability contract.

The candidate now also implements a source-level runtime preflight behind an
injectable authentication adapter. It validates exact principal, observation
time, expiry, and a passed live revocation set; derives the reserved Cedar
authentication context; and exposes no filesystem or signer capability.
Caller principal or reserved-context substitution fails before adapter
invocation. Seven focused tests prove bearer-free local observation,
cancellation, identity/expiry/revocation failure, recovery-visible policy,
derived context, fail-closed Cedar behavior, and zero-write sentinels.

This evidence does not implement an OS or identity-provider session, issue a
runtime token, enable a live authority writer, accept this ADR, or migrate a
Frontier.

The disposable Phase 4 writer core now composes the verified dual-history
snapshot, runtime authentication preflight, Cedar result, semantic approval,
event construction, authority-record signing, DSSE verification, and the
existing recoverable frontier transaction. It has no CLI route or production
signer provider. Six focused tests prove one exact offline-replayable Era-1
transaction, transaction identity binding to read-set and binary-pin changes,
zero journal or canonical bytes after authentication cancellation or signer
failure, history and policy substitution rejection before signing, stale-read
rejection before the commit marker, and committed partial-install recovery
without reauthentication or resigning. The test signer uses only a
deterministic fixture key.

This advances Phase 4 to the provider and broader object-delta gates. It does
not authorize a live writer, accept the ADR, migrate a Frontier, or access any
human signing key.

The next disposable slice now qualifies the first repository-authority
provider against a real ephemeral OpenSSH agent. The provider implements only
the standard request-identities and sign-request messages, selects exactly one
plain Ed25519 identity matching the full keyset public key, signs the exact
DSSE pre-authentication encoding, and verifies the returned signature locally.
It rejects certificates, security-key identities, other algorithms, missing
or duplicate matching identities, malformed responses, wrong payload types,
and key substitution. It reads no private-key file and adds no new
cryptographic dependency graph; the three provider tests use only an
ephemeral fixture key loaded into a disposable `ssh-agent`.

The writer now also binds the complete on-disk authority history rather than
trusting a caller-supplied verified snapshot. Exact direct-directory manifests
cover legacy events, Era-1 events, and authority envelopes, while individual
bindings cover every expected canonical byte and the legacy actor registry.
Missing, extra, altered, mode-drifted, or symlinked history fails before
authentication or signing, and marker-time directory changes abort before any
canonical postimage. This closes the stale-snapshot sibling-fork path.

Eleven writer tests now additionally cover create, update, and delete deltas
across authority, public-review, and canonical-evidence classes; automatic
object read-set binding; no-op and non-canonical object refusal; post-signing
object drift; broader partial-install recovery; missing history; extra history;
and stale-history fork refusal. Authority-record validation also rejects any
object delta whose two roots are equal.

This evidence completes the provider and broader object-delta sub-gates. The
following installation and rotation slices close canonical keyset and
policy-snapshot installation, exact retry, and non-cyclic rotation. Phase 4
remains open for a CLI-unreachable disposable Frontier exercise; the later
terminal-close slice closes emergency revocation. It does not authorize a live
writer, accept this ADR, migrate an active Frontier, or access any human
signing key.

The following bounded writer slice now derives content-addressed keyset and
policy-manifest paths from the verified history. Missing snapshots are covered
by the same authority record and recoverable journal as the events and other
objects; an existing snapshot must match its canonical bytes. Exact
direct-directory bindings reject store membership changes before the marker.
The runtime Cedar schema, policies, and entities must independently rederive
the roots retained by the policy bundle, closing a policy-byte substitution
gap.

Completed authority operations also retain the full typed result in the
verified journal. An exact retry supplies that complete result and receives it
back only after the marker, blobs, postimages, and event commitment verify.
This path has no authentication adapter or signer argument. A changed
transaction ID, record root, event set, read-set root, or write-set root is not
a retry.

Thirteen writer tests now cover these additions. The later sequence-1 and
rotation slices close initial installation and rotation. Phase 4 remains open
for the full offline Frontier drill; the later terminal-close slice closes
emergency revocation. No live writer, CLI route, active Frontier, or human key
is involved.

The next read-side slice defines rotation without a hash cycle. A rotation
record is verified under the currently active keyset and policy, covers one
new immutable full-root keyset and/or policy snapshot, and carries the exact
`authority_rotate` and/or `policy_rotate` semantic approval. The next keyset
must advance generation by one, link the exact current keyset root, and bind
the authority-record root immediately preceding the rotation transaction.
The next policy bundle must link the exact current policy root. Both become
active only on the following record.

The verifier indexes the retained stores by full root and rejects duplicate
roots, wrong Frontiers, unactivated retained generations, snapshot path/root
substitution, missing rotation approval, skipped generation, wrong prior
chain head, policy substitution, and use of the old key after activation.
Keysets also reject duplicate public-key material under different key IDs,
preventing one Ed25519 key from satisfying a multi-key threshold through
aliases. A four-record protocol test proves old-authority rotation followed by
a new-authority transaction.

The CLI-unreachable writer now installs one exact keyset or policy transition
through the same recoverable transaction barrier. It refuses a combined
keyset-and-policy transition so every write has one semantic action. Wrong
transition links, missing rotation approval, and an active-snapshot/history
mismatch fail before authentication or signing. The completed candidate is
replayed against the retained stores before any journal is prepared. Focused
writer tests prove exact full-root installation, policy rotation, zero-sign
refusal, offline replay, and an ordinary post-rotation decision signed by the
new key. This closes the rotation writer sub-gate only.

The following emergency-close slice adds no recovery authority. A terminal
successor keyset is represented by the optional `closed: true` field, zero
threshold, and no keys. Open and historical v1 keysets omit the field, so
their bytes and roots remain unchanged; older binaries fail closed on the new
terminal object. The covering record is signed under the current authority,
requires `authority_close`, and covers exactly one `authority.closed` event
and the terminal snapshot. The event's closed payload binds the exact last
trusted record, keyset, policy, incident, and reason. Replay rejects any later
record.

The writer validates the close shape before authentication or signing, replays
the terminal candidate before journaling, and installs it through the existing
recoverable barrier. A focused fixture proves missing approval has zero signer
access, exact terminal installation, retained offline replay, unchanged
historical roots, and fail-closed attempted continuation. Agents and workloads
are structurally forbidden from `authority_close`.

This closes emergency revocation only. It deliberately does not invent a
break-glass signer or let a compromised or lost key silently manufacture
continuity. A future lineage after total authority loss is a trust reset
requiring a new out-of-band anchor, not recovery of this history.

The final CLI-unreachable Phase 4 drill composes all writer paths in one
disposable Frontier:

1. install the exact legacy-signed bridge and sequence-1 record;
2. append one ordinary Era-1 decision;
3. rotate to generation 2 under the old repository key;
4. append one ordinary decision under the new key;
5. append the terminal close under the new key;
6. commit only canonical authority bytes to a local Git repository;
7. clone with local object reuse disabled; and
8. replay all retained events, snapshots, and five records from that clean
   clone without any signer or authentication provider.

The final replay reports four Era-1 events, the terminal keyset, unchanged
policy, and closed authority. This closes the disposable Frontier writer gate.
It does not accept this ADR, migrate an active Frontier, or remove Era-0
replay.

The subsequent `0.930.0-rc.2` source slice exposes only sequence 1 through
`vela authority migrate`. The preview is key-free and write-free. Apply
rederives the exact plan under the recovery barrier, asks the closed helper to
sign only the legacy continuity event after fresh user presence, authenticates
one exact local issuer-subject principal, and asks a matching Ed25519 identity
in the standard OpenSSH agent to sign the covering repository record.

The first composed Git fixture found two defects that isolated canonical
fixtures did not:

- historical event files may have valid non-canonical JSON formatting, so the
  migration now compares typed content while binding and preserving exact
  existing file bytes; and
- the initial human Cedar rule must name the one migrated principal and type
  review actions against proposals rather than the Frontier.

The corrected fixture proves stable plan derivation, substitution sensitivity,
dirty and pre-existing-authority refusal, cancellation before custody access,
one exact protected signature, recoverable installation, and clean-clone
sequence-1 replay. No active Frontier or human credential was used. ADR 0020
remains Proposed.

The next safe deletion slice removes the obsolete Claude plugin authority
workflow before the final active migration. `/vela:sign-prep`, saved
`.vela/sign-session.json` answers, binary-pin preflight, and every plugin
invocation of legacy batch `vela sign` are gone. The surviving plugin has only
producer commands plus read-only `review list`, `show`, and `preview`.
`scripts/check-prelaunch-surface.sh` prevents the retired workflow from
returning. A real Erdős session-hook smoke reads the current 2,770 findings,
reproduced replay, strict-blocked state, and 15 pending proposals without
entering a signing path. No protocol, Frontier, proposal, Receipt, event,
policy, or scientific-state byte changes. The helper and protected identity
remain solely for the final sequence-1 continuity signature and are still
deleted only after Erdős migrates.

## Alternatives rejected

### Preserve or improve the current helper

Rejected. Better dialogs, longer sessions, Secure Enclave support, or another
custody backend leave Vela responsible for personal identity, recovery,
cross-platform prompts, and a growing signer protocol.

### Put each personal key behind SSH agent, PKCS#11, or KMS

Rejected as the default model. It improves key custody but preserves the false
product model that every event is a personal signing act. Standard providers
belong behind the repository authority instead.

### Make GitHub the authority

Rejected. GitHub is a useful authentication, workload, review, and publication
adapter. Its accounts, rules, audit logs, and availability are not the portable
scientific authority root.

### Use WebAuthn assertions as event signatures

Rejected. WebAuthn strongly authenticates an RP-scoped challenge. It does not
provide Vela's general portable event-signature semantics or prove scientific
comprehension.

### Use Sigstore or in-toto as scientific standing

Rejected. They are appropriate for build and execution provenance. Provenance
does not decide scientific standing.

### Build AcceptancePolicy v0.4

Rejected. Cedar supplies a bounded, schema-validated, default-deny,
forbid-overrides-permit authorization language with an embedded Rust
implementation. Vela retains only its consequence routing and quorum state
machine.

### Delete portable signatures

Rejected. A compromised Git host or hosted database must not be the only
historical authority. One repository-authority signature per exact transaction
is the smallest justified cryptographic core.

## Acceptance gates

ADR 0020 may become Accepted only when:

- every current and hostile AcceptancePolicy decision is identical or stricter
  under the pinned Cedar bundle;
- no translated case introduces a new automatic Permit;
- one disposable and one active Frontier use the new writer;
- clean-clone replay validates both eras without network access;
- every post-migration event has unique valid authority-record coverage;
- routine agent work produces zero prompts;
- one exceptional decision requires one semantic action and no key, hash,
  timestamp, fingerprint, or helper interaction;
- account recovery and authority-key rotation preserve historical verification;
- agent self-expansion, stale approval, token replay, policy broadening, and
  authority-fork fixtures fail closed;
- all active Frontiers migrate without rewriting pre-boundary history;
- `vela-signer`, live legacy writers, identity-v2 custody, signer sessions,
  rebind ceremonies, and AcceptancePolicy authoring are deleted; and
- a blinded user can distinguish evidence, verification, authorization,
  acceptance, publication, and correction.
