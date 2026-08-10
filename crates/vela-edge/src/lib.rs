//! Operational edge for Vela producer work.
//!
//! The protocol crate owns canonical objects and authority. This crate retains
//! only the replaceable filesystem/Git adapters needed by the current product:
//! agent identity custody, descriptor-hardened authority trust-anchor storage,
//! and correction-impact derivation.

mod analysis;
pub use analysis::{correction_impact, repository_write};
pub mod agent_identity;
pub mod git;
