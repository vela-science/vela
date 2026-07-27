//! Native current-repository bootstrap and authority-genesis regression.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

mod support;
use support::EphemeralAgent;

fn run(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    } else {
        command.env_remove("SSH_AUTH_SOCK");
    }
    command.output().expect("run vela")
}

fn success_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode Vela JSON")
}

#[test]
fn fresh_current_repository_replays_from_a_clean_clone() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela native genesis test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();

    let initialized = success_json(&run(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
            "--name",
            "Native genesis fixture",
            "--scope",
            "Exercise one native current repository bootstrap.",
            "--json",
        ],
    ));
    assert_eq!(initialized["schema"], "vela.frontier-init.v2");
    assert_eq!(initialized["authority"], "uninitialized");
    for retired in [
        ".vela/events",
        ".vela/actors.json",
        "frontier.json",
        "vela.lock",
    ] {
        assert!(!frontier.join(retired).exists(), "retired path {retired}");
    }

    let before = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(before["phase"], "authority_uninitialized");
    assert_eq!(before["integrity"]["strict"], "blocked");

    let authority = success_json(&run(
        &frontier,
        Some(agent.socket()),
        &[
            "authority",
            "init",
            ".",
            "--reason",
            "Establish native repository authority.",
            "--json",
        ],
    ));
    assert_eq!(
        authority["schema"],
        "vela.authority-initialization-result.v2"
    );
    assert_eq!(authority["writes_now"], true);

    let verified = success_json(&run(
        &frontier,
        None,
        &["repository", "verify", ".", "--json"],
    ));
    assert_eq!(verified["ok"], true);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["integrity"]["replay"], "verified");
    assert_eq!(status["integrity"]["strict"], "pass");

    let clone = temporary.path().join("clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&frontier)
        .arg(&clone)
        .output()
        .expect("clone native repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let clone_status = success_json(&run(&clone, None, &["status", ".", "--json"]));
    assert_eq!(clone_status["roots"], status["roots"]);
    let dirt = Command::new("git")
        .current_dir(&clone)
        .args(["status", "--porcelain"])
        .output()
        .expect("inspect clone");
    assert!(dirt.status.success());
    assert!(dirt.stdout.is_empty(), "clean clone must remain clean");
}
