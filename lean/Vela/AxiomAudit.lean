import Vela.AxiomAuditRegistry

/-!
# Compatibility axiom audit

Emits the combined public audit report expected by historical tooling. New
assurance claims should cite `ProtocolAxiomAudit` or `ResearchAxiomAudit`
explicitly. All three reports derive membership from `AxiomAuditRegistry`.
-/

run_cmd Vela.AxiomAudit.emit Vela.AxiomAudit.allDeclarations
