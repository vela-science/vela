# ADR 0013: Exact verifier and work-binding constraints for Permit

- Status: Accepted
- Candidate target: Vela `v0.920.0`
- Decision date: 2026-07-18
- Entry gate: passed; hostile AcceptancePolicy v0.1 shadow fixture reproduced

## Context

The current `search-witness` rule is frontier scoped and fail-closed, but its
live policy context exposes a generic `receipt_computational` class plus
assurance, integrity, impact, and replayability properties. It does not expose
the full target packet, Canopus profile, verifier capsule, or result-contract
roots. A human who intends to delegate one frozen search therefore cannot yet
state that exact restriction in the signed policy language.

This was a candidate gap, not permission to add a general method registry. The
entry test froze three otherwise equivalent A3 contexts: the then-current Sidon
`a(24)>7179` search, a different capsule in the same computational class, and a
different target/packet. Before the live run was frozen, retained repository
evidence reproduced 7,192 points, so the non-duplicate target was tightened to
`a(24)>7192`; the decision criterion and hostile result are unchanged.

The registered fixture is
`conformance/fixtures/permit-shadow-v1.json`, with byte root
`sha256:87274e4cfc44a335fbcbccb432d66f7dc9bcdb45c92afdd3da986958aa745742`.
All three cases reduce to the same v0.1 policy-language digest,
`sha256:05f4c43817a4301da40e476393639ba042756f593d48582718135e92b653c7ac`,
and all three receive Permit. Both hostile substitutions therefore pass the
entry gate and prove the missing restriction.

## Decision

Add only:

1. a body-bound Receipt v1 extension `vela.execution-binding.v1` containing
   full SHA-256 roots for the target packet, producer profile, verifier
   capsule, and closed result contract; and
2. AcceptancePolicy v0.2 exact allowlists for those four roots and exact
   replayability.

The extension lives at `environment["vela:execution_binding"]`. It is closed
and contains exactly `schema`, `packet_root`, `profile_root`,
`verifier_capsule_root`, and `result_contract_root`. Every root is lowercase
`sha256:` plus 64 hexadecimal characters. The extension is covered by Receipt
v1's existing whole-body binding and changes the Receipt root if any byte
changes.

The new policy fields are optional in the shared decoder so historical v0.1
objects retain identical bytes and content addresses. A v0.2 Permit rule,
however, must provide a nonempty allowlist for every one of the four roots and
must require `replayability = exact`; omission or malformed roots make the
policy fail closed. Non-Permit rules need no execution constraint.

The policy context derives the roots exclusively from retained, validated
Receipt bytes. Callers cannot supply policy booleans or roots. Missing or
unknown bindings, a short digest, changed packet, profile substitution,
capsule substitution, result-class drift, replayability drift, or altered
Receipt bytes can only Defer/Deny or block strict replay.

The addition creates no verifier registry, hosted authority, object family,
signature algorithm, dependency graph, or accepted-state rule. A policy v0.2
Permit still requires the existing strict Engine gate and signed causal policy
head. Corrections and later evidence continue to use existing event semantics.

The advanced authoring path is deliberately narrow:

```text
vela policy draft search-witness <frontier> \
  --packet-root <sha256:...> \
  --profile-root <sha256:...> \
  --verifier-capsule-root <sha256:...> \
  --result-contract-root <sha256:...>
```

All four flags are required together. The result is a staged unsigned v0.2
policy. Authority still arrives only through one exact protected
`vela policy decide` approval. Legacy `policy sign` remains advanced-only.

The existing flag-authored producer path accepts the same four roots on
`vela land`. Vela validates them together and inserts the closed extension
before deriving the operation id, whole-Receipt attestation, Receipt root, and
policy context. Canopus therefore composes through the sole Vela Receipt
builder instead of duplicating attestation or identity-binding code.

## Compatibility and conformance

- Policy v0.1 and every historical policy-lane event replay byte-for-byte.
- Old binaries may reject policy v0.2 as the intentional pre-1.0 version
  boundary; they must never interpret it as broader v0.1 authority.
- Exact intended binding permits only when all existing Engine, credential,
  evidence, policy-head, and transaction gates also pass.
- Wrong packet, target, profile, capsule, result contract, or replayability
  does not permit.
- Unknown fields, malformed schemas, short roots, incomplete allowlists, and
  missing retained extension bytes fail closed.
- Tampered policy, Receipt, attachment, decision certificate, or policy-lane
  event remains a strict blocker.
- A valid Permit changes only the proposal the exact policy covers and cannot
  mutate unrelated accepted state.

The ADR decision is accepted because the frozen entry gate reproduced the gap.
The `v0.920.0` release still requires the v0.1/v0.2 cross-implementation
vector, exact policy-display coverage, one real Sidon run, and clean-clone
replay. A negative Sidon search is never eligible for this positive-witness
Permit lane.

## Frozen live candidate

The first real positive-only profile is `sidon-a24-improve` at Canopus commit
`b9f1567498b7c0ebc80007a304a8847353c66734`. It binds:

- Sidon commit `342061330a57676c911ca02b66a67954436c96db` and packet root
  `sha256:da2ecf8b213c3166ff258834a1b81f2a21f7c8d6074589098261fe4cf1e82df1`;
- profile root
  `sha256:4c5669b5f85c2f10461e0c8ab13ad97fdaed542d95615c99e88d118717ddf5cb`;
- Linux arm64 capsule root
  `sha256:372cbd0d71f82f33953499c677182d00d9c6b97f6098562a093450d063fbfc5e`;
  and
- positive result-contract root
  `sha256:7dfe79ca2a616d9afb8438aad71a1a0ae134d3325c7d536d45ed8cf3d567e967`.

The result contract fixes the exact claim to “There exists a Sidon subset of
`{0,1}^24` with at least 7,193 elements.” Canopus replaces model-authored
claim prose with those registered bytes only after the exact verifier passes.

The unsigned shadow policy is `vap_4e88fea478811f0553d8ce6c9c91fc45`.
It remains non-authoritative until an exact protected activation. A capsule,
profile, packet, or result-contract substitution does not match its v0.2 rule.

The full Permit path is not yet eligible for protected activation. Live audit
after freezing these roots showed that `vela land` correctly derives assurance
only from retained, event-derived verifier attachments; a producer-declared
Receipt `verifier_runs` row is not load-bearing. Canopus currently emits no
such pre-state evidence, so the exact search-witness rule would Defer at A0.
The release gate therefore requires a fresh exact verification floor or
retained event evidence that strict replay can rederive. Lowering the policy's
A2 floor or treating self-reported verifier success as assurance is forbidden.
