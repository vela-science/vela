# ADR 0014: Policy-scoped producer credentials for exact Permit

- Status: Proposed
- Candidate target: Vela `v0.920.0`
- Entry gate: passed; exact Sidon policy binding deferred solely on producer
  registry membership
- Protocol effect if accepted: AcceptancePolicy-only; no new event kind,
  registry service, or accepted-state rule

## Context

ADR 0013 closed a real authorization gap by binding a Permit rule to the full
roots of one target packet, producer profile, verifier capsule, and result
contract. The live Sidon experiment then reproduced the next boundary. Policy
`vap_540ab5be1b4cb70f011d1b52ec155c5f` matched all four exact roots, Vela
rederived assurance level two, method integrity, and exact replay, and the
frozen verifier reproduced the 7,193-point witness. Receipt
`sha256:dc4fc6351fea032f55c0b3b3b9f159fe434bfbbcb3099d1cc59fe68cdd6b8a5c`
still routed to proposal `vpr_42e07539b01d81b0` as Defer for exactly
`search-witness-v1:credential_invalid`.

The producer's retained key and Receipt v1 self-signed identity binding are
valid. The frontier registry, however, contains a different historical agent
key whose private half is no longer in local custody. Re-running the search,
recovering that lost secret, weakening `credential_valid`, or rewriting the
registry would not improve the scientific evidence and would damage the trust
boundary.

This is not evidence for open actor enrollment or a global identity service.
The authority question is smaller: can the human who signs an exact,
frontier-local policy delegate this one frozen Permit lane to one full producer
credential without granting general frontier membership?

That pattern has established supply-chain analogues. An
[in-toto layout](https://github.com/in-toto/docs/blob/master/in-toto-spec.md)
lets the project owner authorize named functionary keys for specific steps and
material/product rules. [TUF delegated roles](https://theupdateframework.github.io/specification/v1.0.26/)
bind exact keys and thresholds to scoped target paths. Neither analogy supplies
scientific authority, but both support the narrower design rule: signed,
explicit, least-privilege delegation can be safer than treating a global
membership roster as the only credential source.

## Decision

If accepted, add one optional AcceptancePolicy v0.3 constraint:

```text
allowed_producer_credential_roots: [sha256:<64 lowercase hex>, ...]
```

A producer credential root is SHA-256 over the Vela-canonical
`vela.identity_binding.v0.1` preimage with `binding_id` and `signature` cleared,
the same full preimage whose first 16 hexadecimal characters form the readable
`vib_` handle. The full root is the authorization identity; `vib_` remains a
routing handle and is never sufficient in a policy.

For a Permit rule, `credential_valid` is true when either:

1. the existing actor-registry validation succeeds unchanged; or
2. every policy-scoped check below succeeds.

The scoped checks are closed and conjunctive:

- the Receipt v1 body is retained and strictly valid;
- its embedded producer identity binding rederives, self-signs successfully,
  is agent-class, and exactly matches the Receipt submitter actor and public
  key;
- the full credential root appears exactly once in the matching policy rule;
- the rule also satisfies ADR 0013: nonempty exact allowlists for packet,
  profile, verifier capsule, and result contract, plus exact replayability;
- the policy is active, human-signed, frontier-local, causally current, and
  passes all existing Engine and transaction gates; and
- the retained Receipt and proposal bytes match the inputs from which the
  policy context was derived.

The policy plan and protected card must show the producer actor, public-key
fingerprint, full credential root, and whether the credential is global or
policy-scoped. Activation or rotation remains one exact human decision. The
producer never receives a human key and cannot add itself to the allowlist.

The scoped credential authorizes only the matching Permit rule. It does not:

- add an actor to `.vela/actors.json`;
- authenticate any earlier or later event from that actor;
- permit human review, policy administration, proposal decisions, or
  withdrawal under a different key;
- grant access to another frontier, packet, profile, capsule, result class, or
  replay mode;
- establish personhood, institutional affiliation, expertise, independence,
  or scientific truth; or
- change an accepted finding without the existing policy event and reducer
  path.

Policy rotation or revocation removes future authority for the scoped root.
Historical events admitted under an earlier valid causal policy remain
verifiable under that historical policy. A later loss of the producer key
does not invalidate retained evidence; compromise is handled by rotating or
revoking the policy before further admission. A general actor-registration
addition, key-rotation event, or institutional credential system remains a
separate decision and is not implied by this ADR.

## Why this is the smallest authority-preserving change

The exact policy already names the work Vela is willing to admit. Adding the
full producer credential to that same signed rule completes the delegation
without creating a second authority plane. Receipt v1 already carries proof of
possession, the policy already supplies human authority, the causal policy
head already constrains time, and the event log remains the only accepted-state
record.

The rule reduces trust-boundary friction because a replaceable producer can be
authorized for one content-addressed job without a permanent global registry
mutation. It does not weaken authority: current registry validation remains a
valid path, every scoped fact is full-root-bound and human-approved, and any
unknown or mismatch continues to Defer or Deny.

## Alternatives

### Keep global registration as the only credential path

This remains the compatibility fallback and is appropriate for standing
frontier actors. For ephemeral exact-work producers it creates unnecessary
key-lifecycle coupling: losing one old registered producer key can make a new,
fully bound producer unable to use a policy explicitly written for its work.

### Add a governed actor-registration or key-rotation event

This would solve broader lifecycle problems, but it grants frontier-wide
identity meaning and needs separate rules for who may add, rotate, revoke, and
recover actors. The Sidon evidence does not require that surface.

### Replace the producer identity with the old registered identity

The old secret is not in custody. Manufacturing, recovering, or relabeling a
key would be unsafe and would make the Receipt provenance false.

### Treat any valid self-signed Receipt identity as credential-valid

Rejected. Proof of possession is not authorization. An agent can mint
arbitrarily many self-signed identities, so this would turn every matching
verifier result into an authority candidate without a human-scoped delegation.

### Use the truncated `vib_` identity-binding handle

Rejected. It is only a 16-hex routing handle. Authorization requires the full
typed SHA-256 root and ambiguity must fail closed.

### Permit the exact result by proposal or Receipt root

This would authorize an already-produced object rather than a reusable bounded
producer lane and would collapse policy delegation into a disguised individual
decision. The ordinary protected review path already handles individual
proposals.

## Compatibility and replay

- AcceptancePolicy v0.1 and v0.2 bytes, decisions, and historical policy-lane
  events replay unchanged.
- An absent `allowed_producer_credential_roots` field preserves current global
  registry semantics exactly.
- A v0.3 Permit rule using scoped credentials must also carry the four ADR 0013
  allowlists and exact replayability. It cannot broaden a generic rule.
- Older binaries may reject v0.3 as an intentional pre-1.0 policy-version
  boundary; they must never reinterpret it as v0.1 or v0.2.
- The producer credential root is derived from bytes already retained inside
  Receipt v1. No per-Receipt migration or historical rewrite is permitted.
- Git history remains transport and byte lineage. Policy activation membership
  and Vela causal roots, not `created_at`, filesystem order, or Git commit time,
  determine whether the scoped authority was active.

## Adversarial and failure cases

- A backdated identity binding gains nothing: the signed policy must already
  allow its full root at the active causal head.
- A valid binding under the wrong actor, key, class, Receipt, frontier, packet,
  profile, capsule, result contract, or replayability does not Permit.
- A short digest, mixed-case or malformed root, duplicate allowlist entry,
  truncated-ID collision, unknown field, or ambiguous binding fails closed.
- Stripping or changing the binding signature, binding preimage, Receipt body,
  policy bytes, policy signature, policy-head event, proposal, or retained
  verifier evidence prevents Permit and produces the existing strict signal
  appropriate to the corrupted object.
- Registry tampering cannot manufacture the scoped path and remains a strict
  blocker under existing actor-registry rules.
- Policy rotation racing with landing is serialized by the existing frontier
  transaction barrier. Only the exact causal policy head observed by the
  transaction may authorize the event.
- Non-strict mode may report malformed or unmatched scoped credentials, but it
  never converts them into an exemption or Permit. Strict replay blocks
  malformed authority material.
- Two policy rules that would authorize the same class under conflicting
  credential scopes fail policy validation rather than selecting by order.

## Exact conformance contract

Focused fixtures must prove:

1. A v0.2 policy and historical registry-backed producer retain byte-identical
   decisions.
2. The exact Sidon Receipt remains `credential_invalid` under its current v0.2
   policy.
3. The same retained Receipt is eligible under a v0.3 rule containing its full
   credential root and all four exact ADR 0013 roots, assuming every other gate
   passes.
4. Removing or changing any one of the five roots Defer/Deny; no partial match
   is accepted.
5. An unregistered producer with a valid but unlisted self-signed binding
   remains invalid.
6. Wrong actor, key, actor class, signature, Receipt submitter, binding ID,
   full root, policy frontier, causal policy head, or replay mode fails closed.
7. Backdated bindings, duplicate roots, truncated collisions, malformed roots,
   unknown fields, and rule overlap cannot Permit.
8. Policy rotation removes future scoped authority while historical admitted
   events replay under their original causal policy.
9. A scoped producer cannot sign human decisions, policy events, actor
   registration events, or unrelated scientific events.
10. Clean-clone replay of the Sidon vector produces the same policy context,
    proposal standing, accepted-event delta, and exact roots.

Future implementation checks are limited initially to:

```bash
cargo test -p vela-protocol policy_scoped_producer_credential
cargo test -p vela-cli --test policy_workflows scoped_producer_credential
cargo test -p vela-protocol --test cross_impl_reducer_fixtures
python3 conformance/verify.py
```

No external Lean, Diderot, live-network, site, or broad release suite is part
of the ADR decision. The ordinary release union runs once only if the ADR is
accepted and a real release is cut.

## Acceptance gate

The reproduced Sidon Defer is sufficient to draft this ADR, not to accept it.
Acceptance requires review of the exact credential-root derivation, hostile
fixtures showing that self-signing alone grants nothing, a protected policy
preview showing the full scoped credential, and confirmation that an
equivalent one-proposal protected decision would be preferable if the intended
authority is not genuinely reusable. Until then, the current Defer is correct
and no policy or credential rule changes.
