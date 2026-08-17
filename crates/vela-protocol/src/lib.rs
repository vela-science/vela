//! Closed values and deterministic validation for current Vela repositories.
//!
//! The protocol crate performs no filesystem writes, Git publication, runtime
//! authentication, or scientific Decision. Those capabilities live at explicit
//! edge crates. Pre-epoch implementations remain available through Git history
//! and pinned repository predecessor archives, not through the current runtime.

mod shape;
/// The lowercase-hexadecimal rules every crate above this one restates.
///
/// `shape` itself stays private: its timestamp and bounded-text helpers carry
/// this crate's error prose and are not a contract. These predicates are shared
/// because every exact-root and identifier reader must apply the same shape.
pub use shape::{
    HANDLE_HEX_LEN, REPOSITORY_ID_CONTRACT, derive_handle, is_full_sha256_root, is_lower_hex,
    is_lower_hex_64, is_prefixed_lower_hex, is_repository_id,
};
pub mod wire_schema;

mod kernel;
pub use kernel::{
    authentication, authority, authority_history, authorization, canonical, dsse, events, principal,
};
mod objects;
pub use objects::{
    claim_record, proposal, proposal_withdrawal, repository, repository_origin, review_method,
    signer_identity, submission, verification_record,
};
mod read_surface;
pub use read_surface::{error, repository_projection, status};
