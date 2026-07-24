//! The trust-critical core: canonical events, the deterministic reducer, Ed25519 signing, proof bundles, canonical bytes/ids, frontier I/O.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub mod actor_registration;
pub mod authentication;
pub mod authority;
pub mod authority_history;
pub mod bundle;
pub mod canonical;
pub mod detached;
pub mod events;
pub mod frontier_repository;
pub mod principal_capability;
pub mod reducer;
pub mod repo;
pub mod sign;
pub mod signing_input;
