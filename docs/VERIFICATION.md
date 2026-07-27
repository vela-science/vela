# Verification

Vela separates byte integrity, mechanical verification, scientific authority,
and presentation. Conflating them is the central failure this gate prevents.

## Four different facts

1. **Artifact integrity:** a digest identifies the exact bytes.
2. **Verifier outcome:** a named, pinned method returned an outcome for those
   bytes and inputs.
3. **Claim binding:** the checked property matches the scoped claim recorded in
   Receipt v1.
4. **Admission:** a governed policy Permit or attributed human decision
   authorized the exact state transition.

A signature alone does not prove correctness. A passing verifier alone does not
prove novelty, significance, statement faithfulness, or acceptance. Admission
does not retroactively broaden what a verifier checked.

## Frozen reproduction

`vela reproduce <frontier>` re-runs the frontier's frozen verifiers against the
stored witness bytes and environment pins. It must not consult an unpinned
network resource or model judgment. The command reports the property each
verifier checked and refuses malformed or mismatched inputs.

`vela check <frontier> --strict --json` validates the frontier record: content
addresses, accepted-event replay, signatures and policy certificates, required
artifacts, derived-state parity, and strict trust debt. For Profile v1 it also
validates the closed profile/settings, complete identity-boundary chain, Git
anchors and ancestry, retained canonical bytes, actor registry, and the
consumer's independently installed first-boundary pin whenever an
administrator boundary exists. It does not replace the domain verifier.

A verified pinned temporalization boundary may preserve proposal logical-ID
conflicts that already existed at its exact Git anchor. Strict checking labels
them `anchored_immutable_unauthenticated` and keeps them nonblocking only after
the repository gate proves that the conflict set did not expand and every
conflicted proposal byte remains exact. This is replay compatibility, not
authentication: native conflicts, new conflicts, changed historical bytes,
invalid boundaries, and missing or wrong trust pins fail closed.

Run both from the exact Git tree being cited:

```bash
vela check . --strict --json
vela reproduce .
```

Non-strict `vela check . --json` reports the same typed Profile v1
repository-context defects and keeps that context invalid. It is diagnostic,
not a compatibility bypass: an invalid boundary grants no identity,
dependency, signature, or historical exemption, and canonical writers still
fail before transaction journaling.

When a pending proposal retains a frontier-local frozen witness, `review show`
advertises proposal-scoped reproduction:

```bash
vela reproduce . --proposal <vpr_id> --json
```

The output names its scope `pending_proposal` rather than
`accepted_frontier`, and reports `authority_effect: none`. Vela does not
advertise this command for a proposal that retains only artifact bytes and a
producer-side verifier observation. Such a proposal remains inspectable, but
re-running its external verifier requires the producer's exact replay bundle;
an integrity digest or historical pass record is not silently treated as a
locally executable verifier.

## Verifier evidence

Receipt v1 records verifier runs as attributed evidence:

- method and implementation identity;
- exact inputs and artifact digests;
- pinned toolchain or environment;
- outcome and retained log;
- the property checked;
- caveats and checks not performed.

The producer must report only runs that actually occurred. A failed or
inconclusive run is useful negative state and must not be rewritten as a pass.
Producer-maintained verification is disclosed as such; it is not independent
corroboration merely because it is deterministic.

Vela may derive a gate status from matched verifier attachments:

```bash
vela gate check --claim <exact-claim> --attachments <attachments.json> --json
```

That status is computed, not set by a writer. The gate checks attachment
well-formedness, claim matching, declared method independence, and surviving
negative probes. It is one input to signed policy, never a separate acceptance
command.

Durable independent evidence is retained with:

```bash
vela verify attach . attachment.json --proposal <vpr_id> \
  --as verifier:<actor> --json
```

This evidence event requires a content-addressed attachment, the proposal's
exact full claim root, an explicit implementation, full execution-evidence
roots, and a rooted record for every adversarial probe. It rejects stale claim
bindings and declared independence across shared lineage couplings. The event
may change the derived verification gate; it cannot accept, reject, or finalize
the proposal. `vela review show` reports the exact next reproduction,
attachment, and attributed human-decision commands as separate actions.

## Landing and admission

The ordinary path is:

```text
verified artifact and scoped claim
    -> vela land
    -> governed policy evaluation
    -> Permit: admit the exact authorized class
       Defer: preserve the proposal for attributed review
       Deny: refuse and return a repairable result
```

A Permit is not unauthenticated auto-admission. Its authority comes from a
previously human-signed policy, and replay verifies the active causal policy
head and certificate. An old unsigned `policy.auto_admitted` event may remain
decodable in immutable history, but there is no current writer for it.

Defer is the correct outcome when the claim class, replayability, verifier,
artifact, caveat, base, or effect is outside the signed Permit. The producer
does not route around Defer through a direct finding, attempt, review, or MCP
writer.

## Human decision

Direct `vela review accept|reject` commands are the ordinary human decision
surface on an Era-1
Frontier. It binds one proposal, action, reason, principal, policy, authority
head, read set, binary identity, and exact canonical delta. The command is the
semantic human action. The local operating-system session authenticates the
principal, restricted Cedar authorizes the action, and the standard OpenSSH
agent repository-authority key signs the covering DSSE record. Vela reads no
human scientific key and accepts no copied root, timestamp, batch answer, or
custom-helper response.

Any drift aborts before the commit marker. Acceptance additionally requires an
eligible Review Packet and strict aggregate Engine gate, then verifies that
the covered scientific domain event and explicit review event replay together
across both histories. Rejection changes no accepted scientific state.

Era-0 decisions remain byte-verifiable but have no live writer in the current
candidate. Use Vela `0.915.1` only for exact historical command replay.

Human acceptance is a statement of scoped judgment, not a claim that every
possible property was verified. The decision record should retain the relevant
scope and caveats, especially for formal results where kernel checking and
informal statement faithfulness are separate questions.

## Adding a frozen verifier

A verifier belongs in the trusted gate only when it has:

1. a closed input and output contract;
2. deterministic behavior under a pinned environment;
3. explicit resource bounds and failure modes;
4. positive fixtures with independently known expected results;
5. meaningful mutants and malformed negative fixtures it rejects;
6. exact claim-binding rules;
7. source and license suitable for independent reproduction;
8. focused conformance tests shared by implementations;
9. documentation of what a pass does and does not establish.

The bar is adversarial. A checker that accepts the positive fixture but cannot
reject realistic mutants is a rubber stamp. A verifier that fetches mutable
state, calls a model, or requires undisclosed credentials is an adapter, not a
frozen authority-path verifier.

## External Lean

External Lean reproduction is an optional producer adapter. It pins the source
commit, Lean and Mathlib environment, declaration, axiom policy, and retained
logs, then emits unsigned or producer-signed Receipt v1 evidence. A kernel pass
establishes that the named declaration checks under that environment. It does
not establish that the declaration faithfully formalizes the intended theorem
or that the result is novel and important.

External Lean runs only by an explicit manual request. Ordinary changes never
select it automatically, including changes to the adapter itself. It is not a
generic Vela release gate, and no early external project is a compatibility
authority. Formal Lean checks inside Vela remain separately changed-path
selected.

## Offline Merkle primitive

`crates/vela-protocol/src/objects/merkle.rs` provides an optional RFC 6962-style Merkle
construction for local event-log proofs. It is an offline protocol primitive,
not a service dependency. A derived reader does not issue signed tree heads,
hold a transparency signing key, expose proof authority, or turn agreement
between readers into scientific authority.

The decisive checks remain the exact Git bytes, event signatures and policy
certificates, deterministic replay, and frozen verifier results.
