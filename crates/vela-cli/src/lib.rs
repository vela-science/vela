//! `vela` — the command-line binary.
//!
//! Hands off to `crate::cli::run_from_args`, after a small read-only
//! verify intercept (conjecture / proof-packet verification).

use vela_protocol::cli_style as style;

// The CLI / serve / workbench surface, relocated out of the
// `vela-protocol` library so the substrate crate stays a pure protocol
// library. These were `vela_protocol::{cli, serve, workbench, cli_*}`
// before; they now live here and reach into the substrate via
// `vela_protocol::*`.
mod atlas;
pub(crate) use atlas::decl_graph;
// Era-1 writer core. It is deliberately not wired to a CLI command until the
// disposable-frontier and migration gates in ADR 0020 pass.
#[allow(dead_code)]
pub(crate) mod authority_transaction;
// One CLI-unreachable sequence-1 bridge installs the exact migration event,
// initial keyset, policy bundle, and covering repository-authority record.
#[allow(dead_code)]
pub(crate) mod authority_migration;
mod bounded_file;
#[allow(dead_code)]
pub(crate) mod decision_plan;
mod frontier;
#[allow(dead_code)]
pub(crate) mod repository_authority_provider;
pub(crate) use frontier::{cli_frontier, cli_read};
mod write;
pub(crate) use write::{cli_finding, cli_write, review_work};
mod tools;
mod withdrawal;
pub(crate) use tools::{cli_check, cli_proof};
mod config;
pub(crate) mod git_hardened;
pub(crate) use config::{cli_admin, cli_agents, cli_identity};
// The full durability seam intentionally lands before every legacy writer is
// migrated, so some caller-facing pieces remain unused inside this slice.
#[allow(dead_code)]
pub(crate) mod frontier_txn;
mod operation_journal;
pub(crate) mod review_material;
mod server;
mod target_index;
pub(crate) mod ui;
pub(crate) mod workflow;
pub(crate) use server::{cli_commands, cli_engine, serve};
// Read-only integrations embed one dispatcher behind the supported local
// stdio and `vela serve --http` transports.
pub use server::serve::McpService;

pub mod cli;

pub fn run() {
    // Color discipline must hold for the read-only intercepts below too:
    // they print before `run_from_args` initializes styling, so without
    // this a piped or NO_COLOR invocation would leak ANSI. `init()` is
    // `Once`-guarded, so the later call in `run_from_args` is a no-op.
    style::init();

    // Atlas R.2 intercept: read-only verifier subcommands for the
    // primitives added in R.1 (v0.338). Live ahead of run_from_args()
    // because the dispatcher in vela-protocol/cli.rs predates these
    // primitives. When the next vela-protocol release lands them in the
    // dispatcher proper, this intercept can be removed.
    if try_handle_atlas_r2_verify_intercept() {
        return;
    }

    crate::cli::run_from_args();
}

fn try_handle_atlas_r2_verify_intercept() -> bool {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 3 {
        return false;
    }
    match (argv[1].as_str(), argv[2].as_str()) {
        ("conjecture", "verify") => {
            handle_conjecture_verify(&argv[3..]);
            true
        }
        ("proof-packet", "verify") => {
            handle_proof_packet_verify(&argv[3..]);
            true
        }
        ("proof-packet", "verify-external") => {
            handle_proof_packet_verify_external(&argv[3..]);
            true
        }
        _ => false,
    }
}

fn handle_conjecture_verify(args: &[String]) {
    let path = match args.first() {
        Some(p) => p,
        None => {
            eprintln!(
                "{} usage: vela conjecture verify <path>",
                style::err_prefix()
            );
            std::process::exit(2);
        }
    };
    let body = match std::fs::read_to_string(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} read {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    let conj: vela_protocol_core::conjecture::Conjecture = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} parse {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    if let Err(e) = conj.verify() {
        eprintln!("{} witness signature/id invalid: {e}", style::err_prefix());
        std::process::exit(1);
    }
    let cosigs = match conj.verify_cosignatures() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("{} co-signature invalid: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    println!(
        "  {} {} witness:{} cosigners:{} status:{:?}",
        style::moss("conjecture verified"),
        conj.id,
        conj.witness.actor_id,
        cosigs,
        conj.status,
    );
}

fn handle_proof_packet_verify(args: &[String]) {
    let path = match args.first() {
        Some(p) => p,
        None => {
            eprintln!(
                "{} usage: vela proof-packet verify <path>",
                style::err_prefix()
            );
            std::process::exit(2);
        }
    };
    let body = match std::fs::read_to_string(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} read {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    let packet: vela_edge::proof_packet::ProofPacket = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} parse {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    if let Err(e) = packet.verify() {
        eprintln!("{} packet invalid: {e}", style::err_prefix());
        std::process::exit(1);
    }
    println!(
        "  {} {} hash:{} signer:{}",
        style::moss("proof packet verified"),
        packet.packet_id,
        &packet.packet_hash[..24],
        packet.signer_actor_id,
    );
}

fn handle_proof_packet_verify_external(args: &[String]) {
    let path = match args.first() {
        Some(p) => p,
        None => {
            eprintln!(
                "{} usage: vela proof-packet verify-external <path>",
                style::err_prefix()
            );
            std::process::exit(2);
        }
    };
    let body = match std::fs::read_to_string(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{} read {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    let packet: vela_edge::proof_packet::ProofPacket = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} parse {path}: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    if let Err(e) = packet.verify() {
        eprintln!("{} packet invalid: {e}", style::err_prefix());
        std::process::exit(1);
    }
    let n = match packet.verify_external_verifications() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("{} external verification invalid: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };
    println!(
        "  {} {} external:{} (signer:{})",
        style::moss("proof packet + externals verified"),
        packet.packet_id,
        n,
        packet.signer_actor_id,
    );
}
