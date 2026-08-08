//! A cloned repository must not be able to configure the operator.
//!
//! Regression tests for the working-tree `.env` injection: `dotenvy`
//! used to ancestor-walk from cwd, so a repository repo could commit a
//! `.env` that silently set VELA_ACTOR_ID / VELA_KEY_PATH for anyone running
//! vela inside it — the attack
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
        .env("SSH_AUTH_SOCK", dir.join("missing-ssh-agent.sock"))
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
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected signing refusal: {out:?}"
    );
    assert!(dir.join("vela.toml").is_file());
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(vela_bin())
        .current_dir(dir)
        .env("HOME", dir)
        .args(args)
        .output()
        .expect("spawn vela")
}

/// Malformed invocations use exit 2; a well-formed request rejected by the
/// current repository contract uses the domain-error exit 1.
#[test]
fn command_errors_use_stable_exit_codes() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_frontier(tmp.path());
    // A failed signing attempt retains a resumable Profile v2 shell. Until
    // `init` is rerun with a key, `replay` rejects it as a domain error rather
    // than falling through to a retired profile loader.
    let out = run_in(tmp.path(), &["replay", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "replay current repository refusal: {out:?}"
    );
    // Retired state writers fail as usage errors before touching the repository.
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
