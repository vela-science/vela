//! `vela` — one CLI over the protocol, evidence, and Decision boundary.

// Keep the protocol crate read/replay focused. Product commands and local
// repository writes live here; Vela exposes no server or workbench runtime.
pub(crate) mod authority_transaction;
mod bounded_file;
pub(crate) mod claim_standing;
pub(crate) mod claims;
pub(crate) mod command_handlers;
pub(crate) mod command_spec;
mod config;
pub(crate) mod correction_impact;
pub(crate) mod decision_inbox;
pub(crate) mod init;
pub(crate) mod read;
pub(crate) mod repository_authority_provider;
pub(crate) mod repository_decision;
pub(crate) mod repository_ops;
pub(crate) mod routine_evidence_transaction;
pub(crate) mod style;
pub(crate) mod submission;
pub(crate) mod verification;
pub(crate) mod withdrawal;
pub(crate) mod work;
pub(crate) use config::cli_identity;
// Current repository verification and object projections.
mod operation_journal;
pub(crate) mod repository;
pub(crate) mod repository_txn;
pub(crate) mod ui;

/// The stable codes `error.code` may carry, re-exported because they are the
/// published half of the error surface. `tests/wording_contract.rs` holds the
/// emitted set to this list; the module around it stays crate-private.
pub use ui::ERROR_CODES;

pub mod cli;

pub fn run() {
    crate::cli::run_from_args();
}
