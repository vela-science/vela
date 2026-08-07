//! `vela` — one CLI over the protocol, evidence, and Decision boundary.

// Keep the protocol crate read/replay focused. Product commands and local
// repository writes live here; Vela exposes no server or workbench runtime.
pub(crate) mod authority_transaction;
mod bounded_file;
pub(crate) mod claim_standing;
pub(crate) mod command_handlers;
pub(crate) mod command_spec;
mod config;
pub(crate) mod current_claims;
pub(crate) mod current_init;
pub(crate) mod current_read;
pub(crate) mod current_repository_decision;
pub(crate) mod current_submission;
pub(crate) mod current_verification;
pub(crate) mod current_withdrawal;
pub(crate) mod current_work;
pub(crate) mod decision_inbox;
pub(crate) mod repository_authority_provider;
pub(crate) mod repository_ops;
pub(crate) mod routine_evidence_transaction;
pub(crate) mod style;
pub(crate) use config::cli_identity;
// Current repository verification and object projections.
pub(crate) mod current_repository;
mod operation_journal;
pub(crate) mod repository_txn;
pub(crate) mod ui;

pub mod cli;

pub fn run() {
    crate::cli::run_from_args();
}
