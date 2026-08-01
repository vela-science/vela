//! Record and identity object types: anchors, attestations, identity, and Merkle data.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub(crate) mod artifact_reference;
pub mod claim_record;
pub mod current_repository;
pub mod execution_binding;
pub mod identity;
pub mod proposal_v1;
pub mod proposal_withdrawal_v1;
pub mod repository_inputs;
pub mod repository_origin;
pub mod submission_v1;
pub mod verification_record;
