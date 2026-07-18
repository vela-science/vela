# ADR 0013: Exact verifier and work-binding constraints for Permit

- Status: Proposed
- Candidate target: Vela `v0.920.0`
- Entry gate: hostile AcceptancePolicy v0.1 shadow fixture

## Context

The current `search-witness` rule is frontier scoped and fail-closed, but its
live policy context exposes a generic `receipt_computational` class plus
assurance, integrity, impact, and replayability properties. It does not expose
the full target packet, Canopus profile, verifier capsule, or result-contract
roots. A human who intends to delegate one frozen search therefore cannot yet
state that exact restriction in the signed policy language.

This is a candidate gap, not permission to add a general method registry. The
first test freezes three otherwise equivalent A3 contexts: the intended Sidon
`a(24)>7179` search, a different capsule in the same computational class, and a
different target/packet. If policy v0.1 defers or denies both hostile cases,
this ADR is rejected as unnecessary and no Vela 0.920 release is cut.

## Candidate decision

If either hostile case receives Permit, add only:

1. a body-bound Receipt v1 extension `vela.execution-binding.v1` containing
   full SHA-256 roots for the target packet, producer profile, verifier
   capsule, and closed result contract; and
2. AcceptancePolicy v0.2 optional exact allowlists for those four roots and
   required replayability.

The policy context derives the roots exclusively from retained Receipt and
evidence bytes. Callers cannot supply policy booleans or roots. Missing or
unknown bindings, a short digest, changed packet, profile substitution,
capsule substitution, result-class drift, replayability drift, or altered
Receipt bytes can only Defer/Deny or block strict replay.

The addition creates no verifier registry, hosted authority, object family,
signature algorithm, dependency graph, or accepted-state rule. A policy v0.2
Permit still requires the existing strict Engine gate and signed causal policy
head. Corrections and later evidence continue to use existing event semantics.

## Compatibility and conformance

- Policy v0.1 and every historical policy-lane event replay byte-for-byte.
- Old binaries may reject policy v0.2 as the intentional pre-1.0 version
  boundary; they must never interpret it as broader v0.1 authority.
- Exact intended binding permits only when all existing gates also pass.
- Wrong packet, target, profile, capsule, result contract, or replayability
  does not permit.
- Unknown fields and missing retained extension bytes fail closed.
- Tampered policy, Receipt, attachment, decision certificate, or policy-lane
  event remains a strict blocker.
- A valid Permit changes only the proposal the exact policy covers and cannot
  mutate unrelated accepted state.

Acceptance requires the frozen shadow vector, cross-implementation fixtures,
one real Sidon run, and clean-clone replay. A negative Sidon search is never
eligible for this positive-witness Permit lane.
