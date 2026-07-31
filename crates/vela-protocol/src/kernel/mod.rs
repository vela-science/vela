//! Trust-critical current primitives: authentication, repository authority,
//! canonical bytes and IDs, retained event identities, and signing inputs.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub mod authentication;
pub mod authority;
pub mod authority_history;
pub mod canonical;
pub mod events;
pub mod principal;
pub mod sign;
pub mod signing_input;
