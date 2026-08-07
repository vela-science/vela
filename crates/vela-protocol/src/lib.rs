//! Closed values and deterministic validation for current Vela repositories.
//!
//! The protocol crate performs no filesystem writes, Git publication, runtime
//! authentication, or scientific Decision. Those capabilities live at explicit
//! edge crates. Pre-epoch implementations remain available through Git history
//! and pinned Frontier predecessor archives, not through the current runtime.

mod shape;
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
