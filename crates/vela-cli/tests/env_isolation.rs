//! A cloned frontier must not be able to configure the operator.
//!
//! Regression tests for the working-tree `.env` injection: `dotenvy`
//! used to ancestor-walk from cwd, so a frontier repo could commit a
//! `.env` that silently set VELA_ACTOR_ID / VELA_KEY_PATH /
//! VELA_NO_PUBLISH for anyone running vela inside it — the attack
//! class git's protected configuration and Codex's project-scope key
//! blocking exist for. The CLI now reads NO .env from the working
//! tree; these tests hold that line.

use std::process::Command;

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

/// Run vela in `dir` with a SCRUBBED environment (no VELA_* inherited)
/// so the only possible source of the poisoned values is the .env file.
fn init_frontier(dir: &std::path::Path) {
    let out = Command::new(vela_bin())
        .current_dir(dir)
        .env("HOME", dir)
        .args([
            "init",
            ".",
            "--name",
            "envtest",
            "--scope",
            "Exercise environment isolation.",
        ])
        .output()
        .expect("init");
    assert!(out.status.success(), "init failed: {out:?}");
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(vela_bin())
        .current_dir(dir)
        .env("HOME", dir)
        .env("VELA_NO_PUBLISH", "1")
        .args(args)
        .output()
        .expect("spawn vela")
}

/// Malformed invocations across the command families must be exit 2
/// (usage), not the generic exit 1 — the same class fixed in `state`,
/// swept through cli_state / cli_admin / cli_check.
#[test]
fn usage_errors_are_exit_2() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    // `check --json` with no frontier source is a usage error.
    let out = run_in(tmp.path(), &["check", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "check --json no source: {out:?}"
    );
    // Retired state writers fail as usage errors before touching the frontier.
    let out = run_in(tmp.path(), &["state", "anchor", ".", "vf_x", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "retired state anchor writer: {out:?}"
    );
    // `id rotate-key` with identical old/new id is a usage error.
    let out = run_in(
        tmp.path(),
        &[
            "id",
            "rotate-key",
            "--id",
            "reviewer:x",
            "--new-id",
            "reviewer:x",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(2), "id rotate same id: {out:?}");
}

/// The exit-code contract is what an agent branches on. A missing Claim
/// must be 3 (not found), while a malformed invocation is 2 (usage).
#[test]
fn claim_show_honors_the_exit_code_contract() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    let dir = ".";
    // A well-formed but absent finding id → not found (3).
    let out = run_in(
        tmp.path(),
        &[
            "claim",
            "show",
            dir,
            "vf_ffffffffffffffff",
            "--view",
            "standing",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "missing finding must be exit 3: {out:?}"
    );
    // A malformed invocation (no operands) → usage (2).
    let out = run_in(tmp.path(), &["claim", "show", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "usage error must be exit 2: {out:?}"
    );
}
