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
pub(crate) use atlas::{atlas_adapters, cli_atlas};
mod bounded_file;
#[allow(dead_code)]
pub(crate) mod decision_plan;
mod frontier;
pub(crate) use frontier::{cli_frontier, cli_read, cli_registry};
mod write;
pub(crate) use write::{cli_claim, cli_finding, cli_write, review_work, solve_diff_triangle};
mod discovery;
pub(crate) use discovery::{campaign, cli_campaign};
mod tools;
pub(crate) use tools::{cli_attempt, cli_check, cli_lean, cli_log_verify, cli_proof};
mod config;
pub(crate) use config::{cli_admin, cli_agents, cli_experiment, cli_identity, cli_policy};
// The full durability seam intentionally lands before every legacy writer is
// migrated, so some caller-facing pieces remain unused inside this slice.
#[allow(dead_code)]
pub(crate) mod frontier_txn;
mod operation_journal;
pub(crate) mod review_material;
mod server;
pub(crate) mod ui;
pub(crate) mod workflow;
pub(crate) use server::{cli_commands, cli_engine, serve};
// The hosted MCP service: vela-hub embeds the serve dispatcher in-process
// to run `/mcp` over its git-ingest checkouts. One dispatcher behind every
// transport — stdio, `vela serve --http`, and the hub.
pub use server::serve::McpService;

pub mod cli;

pub fn run() {
    // Color discipline must hold for the read-only intercepts below too:
    // they print before `run_from_args` initializes styling, so without
    // this a piped or NO_COLOR invocation would leak ANSI. `init()` is
    // `Once`-guarded, so the later call in `run_from_args` is a no-op.
    style::init();

    if try_handle_external_lean_intercept() {
        return;
    }

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

fn find_external_lean_driver() -> Option<std::path::PathBuf> {
    if let Ok(root) = std::env::var("VELA_WORKSPACE_ROOT") {
        let candidate = std::path::Path::new(&root)
            .join("scripts")
            .join("diderot_lean_verifier.py");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let candidate = ancestor.join("scripts").join("diderot_lean_verifier.py");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        for ancestor in executable.ancestors() {
            let candidate = ancestor.join("scripts").join("diderot_lean_verifier.py");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn try_handle_external_lean_intercept() -> bool {
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) != Some("reproduce-external") {
        return false;
    }
    if argv.len() < 5 {
        eprintln!(
            "{} usage: vela reproduce-external <repo-url> <commit> <fully-qualified-decl> [--json]",
            style::err_prefix()
        );
        std::process::exit(2);
    }
    let Some(driver) = find_external_lean_driver() else {
        eprintln!(
            "{} external Lean driver not found; run from a Vela workspace or set VELA_WORKSPACE_ROOT",
            style::err_prefix()
        );
        std::process::exit(1);
    };
    let workspace_root = driver
        .parent()
        .and_then(std::path::Path::parent)
        .expect("external Lean driver must live under scripts/");
    let draft_frontier = workspace_root
        .join("projects")
        .join("formal-conjectures-lean");
    let mut command = std::process::Command::new("python3");
    command
        .arg(driver)
        .arg("--repo-url")
        .arg(&argv[2])
        .arg("--commit")
        .arg(&argv[3])
        .arg("--declaration")
        .arg(&argv[4]);
    if argv[2].starts_with("https://") && draft_frontier.join(".vela").is_dir() {
        command.arg("--draft-frontier").arg(draft_frontier);
    }
    for argument in &argv[5..] {
        command.arg(argument);
    }
    let status = command.status().unwrap_or_else(|error| {
        eprintln!(
            "{} could not start external Lean driver: {error}",
            style::err_prefix()
        );
        std::process::exit(1);
    });
    std::process::exit(status.code().unwrap_or(1));
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
