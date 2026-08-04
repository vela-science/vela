//! Cold-start CLI contract for a native Frontier before repository authority.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

mod support;
use support::EphemeralAgent;

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn unique_name(prefix: &str, temporary: &tempfile::TempDir) -> String {
    format!(
        "{} {}",
        prefix,
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unique")
    )
}

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
        command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock"));
    }
    command.output().expect("run vela")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode Vela JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn bootstrap_discovery_and_blocked_commands_name_the_one_valid_next_action() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let name = unique_name("Cold-start UX", &temporary);
    let initialized = run(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
            "--name",
            &name,
            "--scope",
            "Exercise phase-aware CLI diagnostics.",
            "--json",
        ],
    );
    assert_eq!(initialized.status.code(), Some(1));
    let initialized = json(&initialized);
    assert_eq!(initialized["command"], "init");
    assert!(
        initialized["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("signing could not complete"))
    );

    let nested = frontier.join("notes/drafts");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let status = run(&nested, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    assert_eq!(status["phase"], "authority_uninitialized");
    assert_eq!(
        status["integrity"]["blockers_by_code"]["repository_authority_uninitialized"],
        1
    );

    for args in [
        vec!["check", "--json"],
        vec!["next", "--json"],
        vec!["start", "missing:target", "--json"],
        vec!["review", "inbox", &frontier_text, "--json"],
        vec!["show", &frontier_text, "vcl_missing", "--json"],
    ] {
        let blocked = run(&nested, None, &args);
        assert_eq!(blocked.status.code(), Some(1), "args={args:?}");
        let blocked = json(&blocked);
        assert_eq!(blocked["ok"], false, "args={args:?}");
        assert_eq!(blocked["error"]["kind"], "domain", "args={args:?}");
        assert_eq!(
            blocked["error"]["message"], "repository authority is not initialized",
            "args={args:?}"
        );
        assert!(
            blocked["error"]["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("vela init")),
            "args={args:?}"
        );
    }

    let agent = EphemeralAgent::start(temporary.path(), "vela resumable init test");
    let resumed = run(
        temporary.path(),
        Some(agent.socket()),
        &["init", &frontier_text, "--json"],
    );
    assert!(resumed.status.success());
    let resumed = json(&resumed);
    assert_eq!(resumed["schema"], "vela.frontier-init.v3");
    assert_eq!(resumed["resumed"], true);
    assert_eq!(resumed["authority"]["state"], "initialized");
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        resumed["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));
}

#[test]
fn init_creates_a_signed_ready_frontier_in_one_command() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela one-step init test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let name = unique_name("Ready Frontier", &temporary);
    let initialized = run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &frontier_text,
            "--name",
            &name,
            "--scope",
            "Exercise one-command initialization.",
            "--json",
        ],
    );
    assert!(initialized.status.success());
    let initialized = json(&initialized);
    assert_eq!(initialized["schema"], "vela.frontier-init.v3");
    assert_eq!(initialized["authority"]["state"], "initialized");
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));
    assert!(
        initialized["repository"]["repository_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    assert!(initialized["repository"]["git_commit"].as_str().is_some());

    let status = run(&frontier, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["actions"]["work"]["mode"], "direct_submission");
    assert!(
        status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela submit "))
    );

    let next = run(&frontier, None, &["next", "--json"]);
    assert!(next.status.success());
    let next = json(&next);
    assert_eq!(next["availability"]["configured"], 0);
    assert!(
        next["next_action"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela submit "))
    );
}

#[test]
fn review_decision_preflight_keeps_json_error_contract() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    assert!(
        run(
            temporary.path(),
            None,
            &[
                "init",
                &frontier_text,
                "--name",
                "Decision UX",
                "--scope",
                "Keep review errors machine-readable.",
                "--json",
            ],
        )
        .status
        .code()
            == Some(1)
    );

    let blocked = run(
        temporary.path(),
        None,
        &[
            "review",
            "accept",
            &frontier_text,
            "vpr_missing",
            "--reason",
            "Inspect the JSON contract.",
            "--json",
        ],
    );
    assert_eq!(blocked.status.code(), Some(1));
    let blocked = json(&blocked);
    assert_eq!(blocked["command"], "review.accept");
    assert_eq!(
        blocked["error"]["message"],
        "repository authority is not initialized"
    );
}
