//! Closed canonical object types: Claim Records, Submissions, Verification
//! Records, Proposals and their withdrawals, repository identity, and signer
//! identity.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub(crate) mod artifact_reference;
pub mod claim_record;
pub mod execution_binding;
pub mod proposal_v1;
pub mod proposal_withdrawal_v2;
pub mod repository;
pub mod repository_inputs;
pub mod repository_origin;
pub mod signer_identity;
pub mod submission_v2;
pub mod verification_record_v2;
