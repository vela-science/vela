//! Closed values and deterministic validation for current Vela repositories.
//!
//! The protocol crate performs no filesystem writes, Git publication, runtime
//! authentication, or scientific Decision. Those capabilities live at explicit
//! edge crates. Pre-epoch implementations remain available through Git history
//! and pinned Frontier predecessor archives, not through the current runtime.

mod kernel;
pub use kernel::{
    authentication, authority, authority_history, canonical, events, principal_capability, sign,
    signing_input,
};
mod computed;
pub use computed::frontier_settings;
mod objects;
pub use objects::{
    claim_record, cli_style, current_repository, current_state_equivalence, execution_binding,
    identity, proposal_v1, registration_record, repository_epoch, repository_inputs,
    repository_origin, submission_v1, verification_record,
};
