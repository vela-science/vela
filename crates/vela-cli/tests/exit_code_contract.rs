//! The exit-code contract `ui.rs` publishes must be reachable from the CLI.
//!
//! `fail()` hardcodes `ErrorKind::Domain`, and for a long time every failure
//! that did not construct its kind by hand routed through it. So `show` on an
//! id that is not in the repository exited 1, exactly like a broken replay, and
//! the whole point of the scheme — an agent that knows WHY a call failed can
//! self-correct — was unreachable. These assertions pin the three codes an
//! agent has to be able to tell apart, on a real initialized repository.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

mod support;
use support::EphemeralAgent;

fn run(cwd: &Path, home: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
    command.output().expect("run vela")
}

/// `vela init` installs a local trust anchor under the OS account home, which
/// deliberately ignores `$HOME`, so the fixture has to take it back out.
struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `code` plus the `error.kind` string, because the JSON envelope and the exit
/// status are two halves of one contract and drifting apart is the failure this
/// file exists to catch.
fn assert_failure(output: &Output, expected_code: i32, expected_kind: &str, what: &str) {
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "{what}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{what}: failure envelope is not JSON ({error}): {}",
                String::from_utf8_lossy(&output.stdout)
            )
        });
    assert_eq!(envelope["ok"], false, "{what}");
    assert_eq!(envelope["error"]["kind"], expected_kind, "{what}");
}

#[test]
fn missing_objects_exit_3_and_malformed_flags_exit_2() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&home).expect("isolated home");
    let agent = EphemeralAgent::start(temporary.path(), "vela exit-code contract test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();

    let initialized = run(
        temporary.path(),
        &home,
        Some(agent.socket()),
        &[
            "init",
            &frontier_text,
            "--name",
            // The repository id derives from the name, and the trust anchor is
            // keyed by that id in a directory shared with every other run, so a
            // fixed name makes the second run collide with the first.
            &format!(
                "Exit code contract fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Exercise the published exit-code contract.",
            "--json",
        ],
    );
    assert!(
        initialized.status.success(),
        "init: {} {}",
        String::from_utf8_lossy(&initialized.stdout),
        String::from_utf8_lossy(&initialized.stderr)
    );
    let initialized: serde_json::Value =
        serde_json::from_slice(&initialized.stdout).expect("decode init");
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    let not_found = [
        (
            vec!["show", ".", "vcl_0000000000000000", "--json"],
            "show on an id no object carries",
        ),
        (
            vec!["why", ".", "vcl_0000000000000000", "--json"],
            "why on a Claim that does not exist",
        ),
        (
            vec!["review", "show", ".", "vpr_0000000000000000", "--json"],
            "review show on a Proposal that does not exist",
        ),
    ];
    for (args, what) in not_found {
        let output = run(&frontier, &home, None, &args);
        assert_failure(&output, 3, "not_found", what);
    }

    // A repository path that is not there is the same not-found, whether the verb
    // canonicalizes first or resolves the store first.
    let output = run(
        temporary.path(),
        &home,
        None,
        &["status", "no-such-repository", "--json"],
    );
    assert_failure(&output, 3, "not_found", "status on a missing directory");

    let usage = [
        (
            vec!["review", "list", ".", "--status", "bogus", "--json"],
            "review list with a status outside the vocabulary",
        ),
        (
            vec!["log", ".", "--as-of", "not-a-timestamp", "--json"],
            "log with an --as-of that is not RFC 3339",
        ),
    ];
    for (args, what) in usage {
        let output = run(&frontier, &home, None, &args);
        assert_failure(&output, 2, "usage", what);
    }

    // The reclassification must not have swept up the domain failures. A
    // request that is well formed and names a real directory, refused by the
    // repository contract, still exits 1 — including `start` on a repository
    // with no Target Index, which is a broken repository and not a bad
    // argument. (`start` on an absent Target needs a live Target Index to
    // reach; current_genesis.rs pins that one.)
    let bare = temporary.path().join("bare");
    std::fs::create_dir_all(&bare).expect("bare directory");
    let output = run(temporary.path(), &home, None, &["replay", "bare", "--json"]);
    assert_failure(
        &output,
        1,
        "domain",
        "replay on a directory with no Vela store",
    );
    let output = run(
        &frontier,
        &home,
        None,
        &["start", "no-such-target", "--repo", ".", "--json"],
    );
    assert_failure(&output, 1, "domain", "start with no Target Index at all");
}
