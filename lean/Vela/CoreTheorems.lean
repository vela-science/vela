import Vela.Protocol.Provenance
import Vela.Transfer.Transfer
import Vela.Transfer.TransferCWCtoDNA
import Vela.Transfer.TransferPackingToCWC
import Vela.Transfer.TransferBinaryCodeToCWC
import Vela.Transfer.TransferPackingToDisjunct
import Vela.Transfer.TransferCostasToGolomb
import Vela.Transfer.TransferHadamardToCWC
import Vela.Transfer.TransferOAtoCWC
import Vela.Transfer.TransferClassicalToCSS
import Vela.Transfer.TransferMDSToSecretSharing
import Vela.Transfer.TransferHypergraphProduct
import Vela.Transfer.TransferHypergraphProductRing
import Vela.Transfer.TransferLiftedProduct
import Vela.Protocol.ReducerModel
import Vela.Protocol.Core
import Vela.Protocol.Log
import Vela.Crypto.Signing
import Vela.Protocol.ReplayIndex
import Vela.Constructions.EGZ
import Vela.Crypto.CanonicalEventId
import Vela.Crypto.SignatureUniqueness
import Vela.Crypto.MultiSigThreshold
import Vela.Protocol.ConcurrentReplay
import Vela.Crypto.FrontierIdDeterminism
import Vela.Governance.ProposalIdempotency
import Vela.Governance.ConfidenceUpdate
import Vela.Governance.GovernedQuorumSoundness
import Vela.Protocol.SearchIndexDeterminism
import Vela.Governance.OwnerEpochChainMonotonicity
import Vela.Protocol.EmptyLogReplay
import Vela.Protocol.CanonicalSequenceLength
import Vela.Protocol.ReplayAppend
import Vela.Governance.ScientificDiffPackId
import Vela.Crypto.AgentAttestationInjectivity
import Vela.Governance.ToolDescriptorInjectivity
import Vela.Governance.DiffPackVerdictAtomicity
import Vela.Governance.EvaluationRecordInjectivity
import Vela.Governance.ToolDescriptorComposition
import Vela.Governance.ReleasedDiffPackAccumulation
import Vela.Governance.VerdictConflictResolution
import Vela.Governance.VerdictConflictAccumulation
import Vela.Governance.ReleasedDiffPackReplay
import Vela.Governance.EvaluationDescriptorComposition

/-!
# Vela compatibility theorem bundle

This module is a compatibility import target for scoped Lean models and domain
results related to Vela. Each declaration applies only to its definitions,
hypotheses, and axiom audit; the aggregate is not an end-to-end proof of the
Rust implementation.

- `Vela.Provenance`: substrate Theorems 2, 3, and 4.
- `Vela.Transfer`: property-preserving maps whose generic theorem projects a
  supplied preservation field, plus separately scoped concrete lemmas.
- `Vela.ReducerModel`: invariants of one deliberately small abstract reducer;
  no refinement to the Rust `Project` is established here.
- `Vela.Core`: dependency-free list models of selected provenance invariants.
- `Vela.Log`: substrate Theorems 1 and 5.
- `Vela.Signing`: Theorem 6 (v0.104 multi-sig canonical-bytes fix).
- `Vela.ReplayIndex`: Theorem 7 (v0.105 O(N) replay index maintenance).
- `Vela.EGZ`: Theorem 8 (Erdős-Ginzburg-Ziv 1961, n = 2 case).
- `Vela.CanonicalEventId`: Theorem 9 (canonical-event-id determinism).
- `Vela.SignatureUniqueness`: Theorem 10 (signature uniqueness under canonical bytes).
- `Vela.MultiSigThreshold`: Theorem 11 (multi-sig threshold soundness).
- `Vela.ConcurrentReplay`: Theorem 12 (concurrent-replay commutativity for disjoint events).
- `Vela.FrontierIdDeterminism`: Theorem 13 (frontier-id determinism).
- `Vela.ProposalIdempotency`: Theorem 14 (proposal-acceptance idempotency).
- `Vela.ConfidenceUpdate`: Theorem 15 (confidence-update bounds).
- `Vela.GovernedQuorumSoundness`: Theorem 16 (governed-quorum soundness).
- `Vela.SearchIndexDeterminism`: Theorem 17 (search-index determinism).
- `Vela.OwnerEpochChainMonotonicity`: Theorem 18 (owner-epoch chain monotone-by-one).
- `Vela.EmptyLogReplay`: Theorem 20 (empty-log replay identity — base case of replay convergence).
- `Vela.CanonicalSequenceLength`: Theorem 21 (canonical-sequence cardinality preservation).
- `Vela.ReplayAppend`: Theorem 22 (replay-compositional append; incremental-replay legitimacy).

This aggregate intentionally excludes the historical PoVD, trusted-fold
accumulation, protocol-keystone, folding, and sum-check research modules. They
remain importable at their exact paths for historical experiments, but they are
not part of the Vela Kernel or the active theorem surface. Current documentation
must cite a scoped module and its exact assumptions rather than treating this
compatibility aggregate as one end-to-end protocol proof.
-/
