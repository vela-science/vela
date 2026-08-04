//! Cold-start CLI contract for a native Frontier before repository authority.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("SSH_AUTH_SOCK")
        .output()
        .expect("run vela")
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
    let initialized = run(
        temporary.path(),
        &[
            "init",
            &frontier_text,
            "--name",
            "Cold-start UX",
            "--scope",
            "Exercise phase-aware CLI diagnostics.",
            "--json",
        ],
    );
    assert!(initialized.status.success());

    let nested = frontier.join("notes/drafts");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let status = run(&nested, &["status", "--json"]);
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
        let blocked = run(&nested, &args);
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
                .is_some_and(|hint| hint.contains("vela authority init")),
            "args={args:?}"
        );
    }
}

#[test]
fn review_decision_preflight_keeps_json_error_contract() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    assert!(
        run(
            temporary.path(),
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
        .success()
    );

    let blocked = run(
        temporary.path(),
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
