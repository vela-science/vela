import Vela.CoreTheorems
import Vela.Transfer.TransferBinaryCodeToCWC

/-!
# Axiom audit registry

One fail-closed membership source for the public Lean declarations whose axiom
closures Vela records. The classification is interpretive:

- `protocol` contains the small set of current structural protocol models
  described in `docs/THEORY.md`;
- `research` contains compatibility models, governance lemmas, domain
  mathematics, and transfer research.

Classification does not change a declaration, its proof, or its axiom closure.
Double-backtick names fail elaboration if a registered declaration is missing
or renamed. `ProtocolAxiomAudit`, `ResearchAxiomAudit`, and the compatibility
`AxiomAudit` derive their declaration lists from this registry rather than
maintaining parallel lists.
-/

open Lean Elab Command

namespace Vela.AxiomAudit

inductive AuditClass where
  | protocol
  | research
  deriving DecidableEq

structure AuditEntry where
  category : AuditClass
  declaration : Name

/-- Public audited declarations in historical registry order. -/
def registry : List AuditEntry :=
  [{ category := .protocol, declaration := ``Vela.Log.replay_convergence_same_finite_log },
   { category := .protocol, declaration := ``Vela.Core.retraction_monotone },
   { category := .protocol, declaration := ``Vela.Core.status_provenance_sound_t },
   { category := .protocol, declaration := ``Vela.Core.frontier_upward_closed },
   { category := .protocol, declaration := ``Vela.Log.changed_core_changes_id },
   { category := .research, declaration := ``Vela.Signing.theorem6_signature_stable_under_flip },
   { category := .research, declaration := ``Vela.ReplayIndex.theorem7_index_maintenance_under_append },
   { category := .research, declaration := ``Vela.EGZ.theorem8_egz_two },
   { category := .protocol, declaration := ``Vela.CanonicalEventId.theorem9_canonical_event_id_injective },
   { category := .research, declaration := ``Vela.SignatureUniqueness.theorem10_signature_uniqueness_under_canonical },
   { category := .research, declaration := ``Vela.MultiSigThreshold.theorem11a_distinctness },
   { category := .protocol, declaration := ``Vela.ConcurrentReplay.theorem12_concurrent_replay_commutes },
   { category := .research, declaration := ``Vela.FrontierIdDeterminism.theorem13_frontier_id_injective },
   { category := .research, declaration := ``Vela.ProposalIdempotency.theorem14_accept_idempotent },
   { category := .research, declaration := ``Vela.ConfidenceUpdate.theorem15_confidence_update_bounded },
   { category := .research, declaration := ``Vela.GovernedQuorumSoundness.theorem16_governed_quorum_sound },
   { category := .research, declaration := ``Vela.SearchIndexDeterminism.theorem17_search_index_deterministic },
   { category := .research, declaration := ``Vela.OwnerEpochChainMonotonicity.theorem18_chain_monotone_single_step },
   { category := .protocol, declaration := ``Vela.EmptyLogReplay.theorem20_empty_log_replay_identity },
   { category := .protocol, declaration := ``Vela.CanonicalSequenceLength.theorem21_canonical_sequence_length },
   { category := .protocol, declaration := ``Vela.ReplayAppend.theorem22_replay_append },
   { category := .research, declaration := ``Vela.ScientificDiffPackId.theorem23_scientific_diff_pack_id_injective },
   { category := .research, declaration := ``Vela.AgentAttestationInjectivity.theorem24_agent_attestation_id_injective },
   { category := .research, declaration := ``Vela.ToolDescriptorInjectivity.theorem25_tool_descriptor_id_injective },
   { category := .research, declaration := ``Vela.DiffPackVerdictAtomicity.theorem26_diff_pack_verdict_atomicity },
   { category := .research, declaration := ``Vela.EvaluationRecordInjectivity.theorem27_evaluation_record_id_injective },
   { category := .research, declaration := ``Vela.ToolDescriptorComposition.theorem28_tool_descriptor_composition },
   { category := .research, declaration := ``Vela.ReleasedDiffPackAccumulation.theorem29_released_pack_accumulation },
   { category := .research, declaration := ``Vela.VerdictConflictResolution.theorem31_verdict_conflict_id_injective },
   { category := .research, declaration := ``Vela.VerdictConflictAccumulation.theorem32_verdict_conflict_accumulation },
   { category := .research, declaration := ``Vela.ReleasedDiffPackReplay.theorem33_released_pack_replay },
   { category := .research, declaration := ``Vela.EvaluationDescriptorComposition.theorem34_eval_descriptor_composition_eval_first },
   { category := .research, declaration := ``Vela.transfer_sound },
   { category := .research, declaration := ``Vela.TransferBinaryCodeToCWC.bincode_to_cwc_sound },
   { category := .research, declaration := ``Vela.TransferCWCtoDNA.cwc_to_dna_sound },
   { category := .research, declaration := ``Vela.TransferBinaryCodeToCWC.binCodeToDNA }]

def declarationsFor (category : AuditClass) : List Name :=
  registry.filterMap fun entry =>
    if entry.category = category then some entry.declaration else none

def protocolDeclarations : List Name := declarationsFor .protocol

def researchDeclarations : List Name := declarationsFor .research

def allDeclarations : List Name := registry.map (·.declaration)

def emit (declarations : List Name) : CommandElabM Unit := do
  for declaration in declarations do
    let axioms ← liftCoreM (Lean.collectAxioms declaration)
    let names := axioms.toList.map (fun name => name.toString)
    IO.println s!"AXIOMS {declaration.getString!} | {String.intercalate ", " names}"

end Vela.AxiomAudit
