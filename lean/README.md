# Vela — Lean models and research proofs

This Lean 4 / Mathlib project contains structural models, domain proofs, and
research formalizations related to Vela. It is not a formalization of the full
Rust implementation and it is not the executable protocol's trust boundary.
Each declaration establishes only its stated result for its stated Lean model,
definitions, hypotheses, and axioms.

Some modules intentionally assume serializer or hash injectivity, and some
composition modules use opaque functions with explicit preservation axioms.
Those assumptions must remain visible; a successful build must not be described
as proving the implementation end to end or as showing that the whole bundle is
axiom-free.

## Layout

Modules are grouped by domain under `Vela/`:

| dir | what it holds |
|---|---|
| `Vela/Protocol/` | state, replay, log, reducer, provenance, canonical ordering |
| `Vela/Crypto/` | signing, signatures, multi-sig, canonical/event/frontier ids, checkpoints, attestation |
| `Vela/Accumulation/` | proof-carrying accumulation: folding, sumcheck, PoVD, the protocol keystone |
| `Vela/Governance/` | abstract and historical proposal, quorum, diff-pack, owner-epoch, and descriptor models |
| `Vela/Transfer/` | research proofs for cross-domain construction maps |
| `Vela/Constructions/` | verified math construction certs (Sidon — the OEIS A309370 cert, Erdős-Ginzburg-Ziv) |

`Vela/CoreTheorems.lean` (the theorem aggregator) and `Vela/AxiomAudit.lean`
(the axiom-report harness) stay at the `Vela/` root. `Vela.lean` is the build
root and imports the broad aggregator plus the Sidon certificate. Importing a
model here does not make its vocabulary part of the current Vela protocol.

## Build / verify

```bash
cd lean
lake build Vela
lake env lean Vela/AxiomAudit.lean
```

`lake build Vela` elaborates `Vela.lean` and its imported closure against the
pinned toolchain. Lean permits `sorry` with a warning, so that command alone is
not a no-`sorry` gate. It also does not inspect the Rust implementation or prove
that an abstract model refines it.

`Vela/AxiomAudit.lean` separately prints the dependencies collected for the
explicit `theoremsToAudit` registry. It is not imported by `Vela.lean`, is not
run by `lake build Vela`, and does not claim coverage of every declaration in
the repository. Its output must be reviewed or passed to the documented axiom
policy before making a claim about a registered theorem's trusted computing
base. The manual GitHub workflow currently runs only the model build; it does
not run or interpret this audit.

The exact assurance is therefore:

> Lean checks the listed statements for their listed models and assumptions.

Scientific witnesses remain independently reproducible through the frozen Rust
verifiers with `vela reproduce`; that evidence does not turn these abstract Lean
models into an end-to-end proof of Vela.
