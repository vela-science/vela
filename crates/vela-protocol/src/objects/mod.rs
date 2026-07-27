//! Record/identity object types: anchors, attempts, attestations, identity, merkle, styling.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub mod activity;
pub mod anchor;
pub mod attempt;
pub mod cli_style;
pub mod identity;
pub mod merkle;
pub mod nanopub;
pub mod provenance;
pub mod receipt_v1;
pub mod record;
pub mod registration_record;
pub mod statement_attestation;
pub mod submission_v1;
#[path = "policy.rs"]
pub mod verification_policy;
pub mod verification_record;
pub mod verification_summary;
