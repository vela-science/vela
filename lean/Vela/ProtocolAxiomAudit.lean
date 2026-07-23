import Vela.AxiomAuditRegistry

/-!
# Protocol-model axiom audit

Reports only the small structural model surface classified as current protocol
theory in `docs/THEORY.md`. A clean report means Lean checked those exact
statements under their reported assumptions. It does not prove the Rust
implementation, cryptographic primitives, Git, or scientific conclusions.
-/

run_cmd Vela.AxiomAudit.emit Vela.AxiomAudit.protocolDeclarations
