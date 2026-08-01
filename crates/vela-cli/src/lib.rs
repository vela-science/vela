//! `vela` — the command-line binary.

// The CLI / serve / workbench surface, relocated out of the
// `vela-protocol` library so the substrate crate stays a pure protocol
// library. These were `vela_protocol::{cli, serve, workbench, cli_*}`
// before; they now live here and reach into the substrate via
// `vela_protocol::*`.
// The standard repository-authority transaction core used by fresh setup,
// producer intake, verification import, and exact review decisions.
pub(crate) mod authority_transaction;
mod bounded_file;
mod frontier;
pub(crate) mod repository_authority_provider;
pub(crate) mod routine_evidence_transaction;
pub(crate) use frontier::cli_read;
mod write;
pub(crate) use write::cli_write;
mod tools;
pub(crate) use tools::cli_check;
mod config;
pub(crate) mod current_doctor;
pub(crate) mod current_init;
pub(crate) mod current_read;
pub(crate) mod current_repository_decision;
pub(crate) mod current_submission;
pub(crate) mod current_verification;
pub(crate) mod current_work;
pub(crate) mod decision_inbox;
pub(crate) mod git_hardened;
pub(crate) use config::{cli_admin, cli_agents, cli_identity};
// Current repository verification and object projections.
pub(crate) mod current_repository;
pub(crate) mod frontier_txn;
mod operation_journal;
mod server;
mod target_index;
pub(crate) mod ui;
pub(crate) mod workflow;
pub(crate) use server::{cli_commands, cli_engine};

pub mod cli;

pub fn run() {
    crate::cli::run_from_args();
}
