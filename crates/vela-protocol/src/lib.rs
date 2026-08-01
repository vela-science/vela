//! Closed values and deterministic validation for current Vela repositories.
//!
//! The protocol crate performs no filesystem writes, Git publication, runtime
//! authentication, or scientific Decision. Those capabilities live at explicit
//! edge crates. Pre-epoch implementations remain available through Git history
//! and pinned Frontier predecessor archives, not through the current runtime.

mod kernel;
pub use kernel::{
    authentication, authority, authority_history, canonical, events, principal, sign, signing_input,
};
mod computed;
mod objects;
pub use objects::{
    claim_record, cli_style, current_repository, execution_binding, identity, proposal_v1,
    registration_record, repository_inputs, repository_origin, submission_v1, verification_record,
};
