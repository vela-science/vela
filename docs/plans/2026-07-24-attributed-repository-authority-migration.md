# Attributed repository authority migration

- Status: Active implementation plan
- Governing decision: ADR 0020, Proposed
- Current replay baseline: Vela `v0.915.1`
- Target release: Vela `v0.930.0`
- Authority rule: no agent process reads, invokes, or approves a human
  scientific credential

## Objective

Replace Vela's personal-key and custom-policy product with a smaller,
standards-aligned authority boundary while preserving the scientific-state
invariant and every historical byte.

The end state is:

```text
evidence producer
  -> deterministic verifier
  -> proposal
  -> Cedar authorization and semantic human judgment where required
  -> exact transaction recheck
  -> repository-authority record
  -> append-only Vela state
```

Humans authenticate normally. Agents use short-lived scoped capabilities. One
repository authority signs the exact transaction. Vela remains offline
verifiable and correctable.

## Product story

Vela keeps work separate from standing.

Agents, notebooks, solvers, and lab systems produce evidence and run exact
checks. Passing a check does not make a claim accepted. Routine work proceeds
without prompts when it fits an already approved policy.

When judgment is necessary, a researcher sees one clear semantic review:

- what claim is being decided;
- which checks passed or failed;
- what is missing;
- what accepting or rejecting changes; and
- how the decision can later be corrected.

They decide the science, not a cryptographic operation. Vela records the
principal, role, policy, exact transition, and correction history in a portable
authority record. There are no personal key files, copied hashes, helper
rebinds, or agent self-approval.

## Release train

| Release | Gate | Outcome |
| --- | --- | --- |
| Vela `v0.930.0-rc.1` | contract, dual verifier, Cedar shadow equivalence, fixture writer | first complete candidate; no active Frontier migration |
| Vela `v0.930.0-rc.2` | one disposable and one low-risk active Frontier | single-new-writer proof and recovery exercise |
| Vela `v0.930.0` | every active Frontier migrated; legacy writers and helper deleted | breaking pre-1.0 authority simplification |
| Canopus `v0.7.0` | released Vela capability profile and zero-prompt producer run | replace long-lived producer keys with short-lived grants |
| Vela Web `v0.430.0` | exact new authority history available read-only | render attribution, authorization, verification, and standing separately |

There is no intermediate feature release. Compatible `0.915.x` releases are
reserved only for independently reproduced Era-0 replay defects.

## Phase 0: freeze and audit

1. Preserve Vela `v0.915.1`, its release artifacts, and the exact active
   Frontier roots as the Era-0 baseline.
2. Inventory every live call to:
   - `vela-signer`;
   - identity-v2 custody;
   - signer sessions;
   - helper and binary rebind;
   - `--confirm-root` and `--confirm-at`;
   - AcceptancePolicy evaluation and authoring;
   - registered agent and CI keys.
3. Freeze AcceptancePolicy v0.1 through v0.3. Add a build gate rejecting v0.4.
4. Record current prompt count, setup time, failure modes, recovery behavior,
   and comprehension for the current operator path.

Exit: the current writer is fully enumerated and no new legacy surface may
enter `main`.

## Phase 1: contract and dual verifier

1. Implement pure, closed Rust types for:
   - `vela.authority-keyset.v1`;
   - `vela.policy-bundle.v1`;
   - `vela.authority-record.v1`;
   - `vela.event.v1`;
   - DSSE authority envelopes.
2. Define canonical preimages, full digests, short display IDs, signature
   domains, sequence rules, event-coverage rules, and sorted diagnostics.
3. Pin `cedar-policy` exactly; disable top-level defaults and reject every
   compiled Cedar extension constructor or value in the Vela profile.
4. Implement the restricted Cedar schema and fail-closed Vela wrapper:
   validation error or evaluation diagnostic never permits.
5. Implement legacy/new dual verification. Do not add a new writer.
6. Add cross-implementation JSON fixtures and a network-disabled clean-clone
   verifier.

Exit: authority-record verification is complete, old fixtures replay
byte-identically, and malformed Era-1 inputs fail closed.

Progress evidence, 2026-07-24: the pure dual-history verifier now accepts
unchanged Era-0-only history and validates one closed legacy-signed
`vela.authority-model-migration.v1` bridge into a contiguous DSSE
authority-record chain. Seven focused adversarial tests reject a later legacy
write, actor-registry or signature tampering, missing or duplicate coverage,
transaction substitution, wrong roots, authority forks, policy substitution,
and malformed bridge payloads. The bridge is reducer-neutral and no writer or
Frontier migration exists. A committed cross-implementation vector at
`sha256:11ced5de2441214b3325bb4368f900d111c944b878088b04966194353aa175f8`
is now independently rederived through OpenSSL Ed25519 and DSSE verification.
Six hostile mutations fail. The exact detached Git commit replays from a clean
clone with network access denied. Phase 1 is complete; Phase 3 is the next
implementation gate.

## Phase 2: policy translation and shadow evaluation

1. Translate each AcceptancePolicy version mechanically into one Cedar bundle.
2. Preserve exact semantics for:
   - claim class;
   - assurance;
   - impact;
   - contestation and semantic mutation;
   - governance mutation;
   - independence and method integrity;
   - packet, profile, verifier, result-contract, replayability, and producer
     bindings;
   - expiry, revocation, and quorum eligibility.
3. Evaluate every retained proposal and policy-lane admission through both
   engines.
4. Add all ADR 0013 and 0014 hostile substitution fixtures.
5. Fail on any new Cedar Permit. Produce a typed broader/equivalent/narrower
   diff.
6. Generate a plain-language authority summary and policy effect diff from
   typed data, never model prose.

Exit: the translated Cedar path is identical or stricter over the complete
frozen corpus.

Progress evidence, 2026-07-24: the complete retained policy-object corpus
contains two historical Erdős v0.1 policies and one active Sidon v0.2 policy,
with four Permit rules total. No retained event records an automatic
policy-delegation admission. The four source-bound rule cases plus the ADR 0013
and ADR 0014 hostile fixtures are equivalent under the translated Cedar path:
zero narrower routes, zero broader routes, and zero new Permits. The
content-addressed report root is
`sha256:92f4c7568d74a87844d9b306a2dd64c95456dc867d3f8b3e9a0c6ad30c810504`.
This completes the current Phase 2 corpus gate. It does not migrate a writer.
The separately completed Phase 1 cross-implementation and clean-clone gate
permits work on Phase 3; neither gate accepts ADR 0020.

## Phase 3: principal and capability model

1. Add stable principal IDs and explicit external-account links.
2. Define human, agent, workload, service, and institution principal types.
3. Add the closed short-lived capability claim profile.
4. Enforce agent/workload unconditional forbids in both Cedar and application
   code.
5. Implement local OS-session authentication for solo mode.
6. Implement one optional OIDC/passkey adapter only after the local writer is
   complete. Authentication tokens never enter canonical history.
7. Implement logout, expiry, revocation, and recent-recovery context.

Exit: no live agent or workload operation requires a permanent Frontier key.

Progress evidence, 2026-07-24: the read-only candidate now defines closed
`vela.principal.v1`, `vela.capability-grant.v1`, and retained bearer-free
verified capability claims. Human identity is an exact retained
issuer-subject link, never an inferred email or display field. Agent and
workload grants bind the repository-authority audience, one Frontier, sorted
full-root resources and execution inputs, a maximum 24-hour window, at most
one attenuating delegation, and a pending-review or policy-routed consequence
ceiling. Human governance actions are structurally absent from the capability
enum and rejected from agent/workload Cedar requests by the same
protocol-owned forbid list.

The cross-implementation fixture root is
`sha256:67bf660a0733bbc7579a883e8cc2e1b9ae09843e6ecee856794e2c07f1f5ef2d`.
An independent Python verifier rederives all roots and rejects eight hostile
cases, including identity inference, authority escalation, time widening,
parent substitution, bearer retention, and revocation. Phase 3's stable claim
contract is complete. Runtime local-session authentication, logout/recovery
context, and the writer remain later gates; no token format or identity
provider has been added.

## Phase 4: authority transaction writer

1. Adapt the existing recoverable transaction barrier rather than creating a
   second journal.
2. Validate:
   - intent;
   - authentication claim;
   - capability;
   - deterministic evidence;
   - Cedar request and diagnostics;
   - semantic approval and quorum;
   - final read set.
3. Build the complete event set and object delta.
4. Build one authority record covering the exact transition.
5. Sign through a provider interface whose first implementation uses the
   standard SSH agent.
6. Atomically install events, objects, record, policy snapshot, and keyset.
7. Add authority rotation, emergency close, idempotency, and fork refusal.
8. Keep Decision Plan roots private as `intent_digest`; remove copied root/time
   inputs from the new path.

Exit: one disposable Frontier uses only the Era-1 writer and verifies offline.

## Phase 5: migration

Migration order:

```text
fixture
quantum-codes
sidon-sets
formal-conjectures
erdos
```

For each Frontier:

1. preview the exact legacy roots, translated policy, new principal, keyset,
   and writer marker;
2. require the prior authorized human to approve one semantic migration;
3. append one legacy-signed migration event;
4. append authority-record sequence 1 under the new repository key;
5. verify the cross-signing bridge and unchanged pre-boundary history;
6. switch to single-new-write;
7. reject subsequent legacy-form authority writes;
8. verify from a clean offline clone; and
9. publish an exact migration manifest.

The agent may prepare and invoke the semantic migration request. It may not
approve it or access the legacy human key. Cancellation produces zero writes.

Exit: every active Frontier uses Era 1 and no historical byte changed.

## Phase 6: deletion and ecosystem adoption

Delete from live product paths:

1. `crates/vela-signer`;
2. the `vela-signer` binary;
3. OS-specific custody and prompt adapters;
4. identity-v2 helper fields and migration journals;
5. signer-session records;
6. helper/binary rebind;
7. live `vela sign` and file-key writers;
8. AcceptancePolicy authoring and live evaluation;
9. long-lived agent/CI registration for routine production;
10. root/time confirmation UX.

Retain:

- Era-0 signature and policy verification;
- historical ADRs and migration evidence;
- current actor registries as read-only history;
- the transaction journal;
- exact protected scientific-state semantics.

Add grep, dependency, help-surface, and binary-package gates that fail if a
deleted live writer reappears.

Canopus adopts capability input and emits truthful workload attribution. It
never becomes an issuer, reviewer, policy administrator, or authority signer.

The Observatory remains read-only. It renders:

```text
produced by
verified by
authorized under
decided by
recorded by repository authority
current standing
correction history
```

It adds no login, write API, policy service, or hosted authority.

## Verification

Focused development:

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

Release boundary:

```bash
./scripts/full-conformance.sh --suite core --mode=ci
./scripts/full-conformance.sh --suite frontier --mode=ci
./scripts/full-conformance.sh --suite full --mode=ci
```

The full deterministic union runs once for `v0.930.0`. External Lean, Diderot,
and live-network suites remain excluded unless directly selected by a named
migration vector.

## Product acceptance

Release requires:

- zero prompts for routine policy-covered work;
- one semantic action for an exceptional scientific decision;
- one semantic action plus configured step-up or quorum for durable authority
  changes;
- no private-key path, seed, fingerprint, helper, rebind, root, or timestamp in
  the ordinary path;
- no new automatic Permit during translation;
- explicit agent and workload attribution;
- lost-device recovery without a private-seed backup;
- repository-key rotation without historical invalidation;
- Git-host and identity-provider migration without standing changes;
- correct offline replay with no live service;
- a blinded user correctly distinguishing evidence, verification,
  authorization, acceptance, publication, and correction.

## Stop conditions

Stop on:

- any rewritten historical event, proposal, Receipt, policy, registration, or
  accepted-state byte;
- any new Permit relative to the frozen AcceptancePolicy corpus;
- any agent path to self-grant, policy administration, membership, recovery, or
  scientific decision;
- any request-time identity, policy, Git, KMS, or transparency dependency in
  offline replay;
- any authority-record gap, overlap, fork, rollback, wrong root, or stale
  approval;
- any product flow that exposes cryptographic plumbing as the scientific
  decision;
- any attempt to treat provenance or verification as standing; or
- any dual live writer after a Frontier migration.

## Explicit non-goals

Do not build:

- AcceptancePolicy v0.4;
- a Vela OAuth or identity provider;
- a Vela passkey event-signature format;
- a custom human PKI;
- blockchain or decentralized identity;
- a hosted authority dependency for replay;
- GitHub-only standing;
- Sigstore standing;
- a universal governance ontology;
- SPIFFE for solo mode;
- a model risk score in authority; or
- a second transaction or canonical database.
