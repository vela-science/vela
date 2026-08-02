//! Current operational edge for Vela producer work.
//!
//! The protocol crate owns canonical objects and authority. This crate retains
//! only the replaceable filesystem/Git adapters needed by the current product:
//! agent identity custody, repository file replacement/trust anchors, and the
//! derived Target Index.

mod analysis;
pub use analysis::{correction_impact, repository_write, target_index};
pub mod agent_identity;
pub mod git;
