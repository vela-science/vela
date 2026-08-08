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
/// this crate's error prose and are not a contract. These three are, and they
/// were reachable from outside only through a one-line `pub fn` in
/// `execution_binding` that existed for no other purpose — so the rule
/// `shape.rs` was written to write down once was written down eleven more
/// times, in two different spellings, in `vela-edge` and `vela-cli`.
pub use shape::{is_full_sha256_root, is_lower_hex, is_lower_hex_64, is_prefixed_lower_hex};
pub mod wire_schema;

mod kernel;
pub use kernel::{
    authentication, authority, authority_history, authorization, canonical, events, principal, sign,
};
mod objects;
pub use objects::{
    claim_record, current_repository, execution_binding, identity, proposal_v1,
    proposal_withdrawal_v1, repository_inputs, repository_origin, submission_v1,
    verification_record,
};
mod read_surface;
pub use read_surface::status_v4;
