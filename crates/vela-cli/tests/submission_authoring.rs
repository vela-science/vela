//! Focused current Submission authoring regressions.

use std::path::Path;
use std::process::{Command, Output};

fn run(home: &Path, frontier: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .arg("submit")
        .args(args)
        .args([
            "--claim",
            "Exact bounded fixture.",
            "--type",
            "theoretical",
            "--replayability",
            "exact",
            "--artifact",
            "missing.json:source-diff",
            "--caveat",
            "Exact fixture only.",
            "--as",
            "agent:fixture",
            "--repo",
            frontier.to_str().expect("utf-8 frontier"),
            "--json",
        ])
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("SSH_AUTH_SOCK")
        .output()
        .expect("run vela submit")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn new_claim_authoring_does_not_invent_an_attempt() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(directory.path(), directory.path(), &[]);
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --attempt"));
    assert!(message.contains("artifact 0"));
}

#[test]
fn exact_supersession_authoring_does_not_invent_an_attempt() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(
        directory.path(),
        directory.path(),
        &[
            "--supersedes",
            &format!("vcl_{}", "a".repeat(64)),
            "--target-root",
            &format!("sha256:{}", "b".repeat(64)),
        ],
    );
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --attempt"));
    assert!(message.contains("artifact 0"));
}
