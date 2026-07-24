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

A human principal is a stable Vela identifier with namespaced account links:

```text
principal:01J...
  <- local:<device-id>|uid:<uid>
  <- oidc:<issuer>|<subject>
  <- orcid:<issuer>|<subject>
```

Authorization uses the stable principal and governed role bindings. Email,
display name, GitHub handle, and ORCID are never security identifiers by
themselves. Display name and affiliation are retained only as readable
snapshots.

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
previous_keyset_digest
activation_record
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

The first capability profile records:

```text
issuer
subject
current actor
delegation chain
audience
frontier
actions
resource bindings
exact execution bindings
issued_at
expires_at
token_id
maximum delegation depth
revocation reference
```

Runtime bearer credentials are never committed. The authority record retains
only verified claims and the grant identity. OAuth token-exchange/SciTokens
claims are adapters, not Vela protocol dependencies. DPoP or SPIFFE is added
only for a reproduced network or institutional workload threat.

An agent or workload can never obtain:

```text
decideClaim
managePolicy
manageMembership
approveRecovery
rotateAuthority
correctBulk
destroy
```

This is enforced both by unconditional Cedar forbids and by application
invariants outside the policy engine.

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

Strict replay requires every post-migration canonical mutation to be covered
exactly once by a valid authority record. Gaps, overlaps, duplicate coverage,
sequence reuse, wrong transaction IDs, wrong before/after roots, or authority
forks fail closed.

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
