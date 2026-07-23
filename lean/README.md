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
| `Vela/Accumulation/` | historical research models: monotone improvement, trusted folds, and scoped folding/sum-check lemmas; not active Kernel claims |
| `Vela/Governance/` | abstract and historical proposal, quorum, diff-pack, owner-epoch, and descriptor models |
| `Vela/Transfer/` | research proofs for cross-domain construction maps |
| `Vela/Constructions/` | verified math construction certs (Sidon — the OEIS A309370 cert, Erdős-Ginzburg-Ziv) |

`Vela/CoreTheorems.lean` is the compatibility aggregator.
`Vela/AxiomAuditRegistry.lean` is the single public audit-membership source;
`ProtocolAxiomAudit.lean` and `ResearchAxiomAudit.lean` produce the classified
reports, while `AxiomAudit.lean` preserves the combined historical report.
`Vela.lean` is the build root and imports the compatibility aggregate plus the
Sidon certificate. The aggregate excludes the historical
`Vela/Accumulation/` research tree; importing one of those modules explicitly
does not make its vocabulary part of the current Vela protocol.

## Build / verify

```bash
cd lean
lake build Vela
lake build Vela.ProtocolAxiomAudit Vela.ResearchAxiomAudit Vela.AxiomAudit
python3 ../scripts/check-lean-axiom-audits.py --project .
```

`lake build Vela` elaborates `Vela.lean` and its imported closure against the
pinned toolchain. Lean permits `sorry` with a warning, so that command alone is
not a no-`sorry` gate. It also does not inspect the Rust implementation or prove
that an abstract model refines it.

The classified audit modules print dependencies collected for the explicit
registry. They are not imported by `Vela.lean` and do not claim coverage of
every declaration in the repository. The checker requires the protocol report
to use only the frozen policy axioms, rejects `sorryAx` and compiler-trust
closures throughout the combined report, and proves the compatibility report
is exactly the disjoint protocol/research union. Research-specific axioms remain
visible rather than being misreported as protocol guarantees.

The exact assurance is therefore:

> Lean checks the listed statements for their listed models and assumptions.

Scientific witnesses remain independently reproducible through the frozen Rust
verifiers with `vela reproduce`; that evidence does not turn these abstract Lean
models into an end-to-end proof of Vela.
