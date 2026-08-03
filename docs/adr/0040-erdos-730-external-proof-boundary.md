# ADR 0040: Erdős 730 external-proof boundary

- Status: Accepted
- Accepted: 2026-08-03
- Protocol effect: none
- Scientific authority effect: exact bounded Claim accepted by a later human Decision
- Target: `erdos:730:external-proof-boundary` (completed and retired)

## Context

The pinned `lean-proofs` lineage contains a complete kernel-checked theorem,
`Erdos730.FullDensityTheorem.pairSet_infinite`, proving a stronger
positive-density statement about consecutive central binomial coefficients.
The retained terminal theorem compiles without `sorry` or `admit` and reports
only `propext`, `Classical.choice`, and `Quot.sound`.

This is unusually strong evidence, but it is not yet an accepted Erdős 730
result. Formal Conjectures and the retained public problem source still call
the problem open. The proof uses Lean 4.29.1 while the exact Formal Conjectures
statement uses Lean 4.27.0. The proof predates this Vela campaign, and no
external mathematical review or Vela Decision has occurred.

Erdős Frontier commit `ea44055f33ec04509385454228fd6cba8fcfe562`
already exposes packet root
`sha256:36dd946797305295d127d5c6fed23ffccd76609a8705f0155c9cf2f7f1c6e370`
and a source-local verifier for the boundary.

## Decision

Make Erdős 730 the next active scientific episode. Produce one compact report
covering exactly:

1. domain and pair order;
2. central-binomial definition;
3. equality of prime-factor support;
4. conclusion strength and implication direction;
5. proof assumptions and exact axiom inventory; and
6. the Lean 4.29.1/4.27.0 import boundary.

Mechanical Verification must independently bind both repositories, reproduce
terminal-solve ancestry and exact theorem bytes, compile the external theorem
and audit under Lean 4.29.1, confirm the Formal Conjectures bytes and 4.27.0
environment, and validate complete report coverage.

Semantic review remains separate from mechanical replay. The terminal packet
must offer exactly one of three conclusions: `equivalent`, `not_equivalent`,
or `indeterminate`. An `equivalent` report may support either an explicit
external-proof boundary or a native port/bridge; it does not itself change the
retained open status.

Only a later consequence-complete human Decision may change local Standing.
That Decision must preserve external source statuses and must not claim that
Vela caused, discovered, or globally accepted the proof.

## Success

Success is an independently reproducible equivalence conclusion, explicit
toolchain boundary, scoped Verification, and decision-ready packet. Acceptance
is one possible later outcome, not part of this ADR's evidence claim.

## Implementation outcome

The producer report concluded `equivalent` at
`sha256:42db39dd2b51e7821e02fc1acbb3e43cde83f269a8cb491f2925ad3aa233d106`.
Fresh source-first Verification `vvr_3b6d523c55a24dc9` passed. Attributed human
Decision event `vev_0ab843df6ad373ec` then accepted the exact local boundary
Claim, and replay reproduced repository root
`sha256:821cf0d94778f647305107943572f4916a6cf63fe5ea12506a471fabc07b7474`.
The generated Target Index retired the completed Target and binds a rooted
post-Decision Dossier handoff. External mathematical review, a native Lean
4.27.0 port, novelty, and global solution status remain outside this outcome.
