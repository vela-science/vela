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
    run_with_advice_setting(cwd, socket, args, "0")
}

fn run_with_advice(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    run_with_advice_setting(cwd, socket, args, "1")
}

fn run_with_advice_setting(
    cwd: &Path,
    socket: Option<&Path>,
    args: &[&str],
    advice: &str,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", advice);
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
fn replay_is_the_only_repository_replay_verb() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let help = run(temporary.path(), None, &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("replay"));
    assert!(!help.contains("check"));

    let retired = run(temporary.path(), None, &["check", "--json"]);
    assert_eq!(retired.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&retired.stderr).contains("unrecognized subcommand 'check'"));

    let verification_help = run(
        temporary.path(),
        None,
        &["verification", "record", "--help"],
    );
    assert!(verification_help.status.success());
    let verification_help = String::from_utf8_lossy(&verification_help.stdout);
    assert!(verification_help.contains("agent:<name>, ci:<name>, or verifier:<name>"));
    assert!(!verification_help.contains("reviewer:<you>"));
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
    let init_hint = initialized["error"]["hint"]
        .as_str()
        .expect("init recovery hint");
    assert!(init_hint.contains("ssh-add /path/to/private-key"));
    assert!(init_hint.contains("start ssh-agent first on Linux"));
    assert!(init_hint.contains("docs/QUICKSTART.md#first-time-authority-key-setup"));
    assert!(init_hint.contains(&format!("vela init '{frontier_text}'")));
    assert!(init_hint.ends_with("first-time-authority-key-setup"));

    let nested = frontier.join("notes/drafts");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let status = run(&nested, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    /* One command, one document: a cold Frontier and a replaying one both
    answer `vela.status.v4`, and the phase is a value inside it. */
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["actions"]["work"]["mode"], "authority_uninitialized");
    assert!(
        status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela init "))
    );
    assert_eq!(status["actions"]["review"], serde_json::Value::Null);
    assert_eq!(status["integrity"]["replay"], "not_initialized");
    assert_eq!(status["integrity"]["strict"], "blocked");
    assert_eq!(
        status["integrity"]["blockers_by_code"]["repository_authority_uninitialized"],
        1
    );

    for args in [
        vec!["replay", "--json"],
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
fn human_init_recovery_keeps_the_resume_command_human_readable() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let initialized = run_with_advice(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
            "--name",
            "Human recovery",
            "--scope",
            "Explain first-time key setup without switching output modes.",
        ],
    );

    assert_eq!(initialized.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&initialized.stderr);
    assert!(stderr.contains("ssh-add /path/to/private-key"));
    assert!(stderr.contains("key setup: https://github.com/vela-science/vela/"));
    assert!(stderr.contains(&format!("vela init '{frontier_text}'")));
    assert!(!stderr.contains("--json"));
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

    let readme = std::fs::read_to_string(frontier.join("README.md")).expect("frontier README");
    assert!(readme.contains("## Operator loop"));
    assert!(readme.contains("git add -- verification/method.json"));
    assert!(readme.contains("vela verification record"));
    assert!(readme.contains("vela review inbox"));
    assert!(readme.contains("vela review accept"));
    assert!(readme.contains("--if-entry-root"));
    let agent_charter =
        std::fs::read_to_string(frontier.join("AGENTS.md")).expect("frontier agent charter");
    assert!(agent_charter.contains("tracked, clean, and retained"));
    assert!(agent_charter.contains("vela verification record"));
    assert!(agent_charter.contains("vela review inbox"));
    assert!(agent_charter.contains("do not decide it yourself"));

    let status = run(&frontier, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    /* The same literal the cold-start test asserts, so the two branches cannot
    drift back into two documents without one of them failing here. */
    assert_eq!(status["schema"], "vela.status.v4");
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

#[test]
fn a_colliding_trust_pin_is_not_reported_as_a_signing_failure() {
    // The pin lives under the OS account home and keys on repository_id, so a
    // second authority initialization of the same retained Profile, with a
    // different key, targets the same pin path with a different record root.
    // That is not a signing failure and no key operation can clear it. Two
    // separately created repositories can no longer provoke this, because each
    // draws its own genesis identity; a copied bootstrap still can.
    let temporary = tempfile::tempdir().expect("temporary directory");
    let name = unique_name("Pin collision", &temporary);
    let scope = "Prove a colliding pin is classified apart from signing.";

    let first_root = temporary.path().join("first");
    std::fs::create_dir_all(&first_root).expect("first agent root");
    let first_agent = EphemeralAgent::start(&first_root, "vela pin collision first");
    let first_frontier = temporary.path().join("first/frontier");
    let first_frontier_text = first_frontier.to_string_lossy().into_owned();
    let established = run(
        temporary.path(),
        Some(first_agent.socket()),
        &[
            "init",
            &first_frontier_text,
            "--name",
            &name,
            "--scope",
            scope,
            "--json",
        ],
    );
    assert!(established.status.success());
    let established = json(&established);
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        established["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    // A second key so the second init derives a different authority record
    // root; an identical anchor would install idempotently and never collide.
    let second_root = temporary.path().join("second");
    std::fs::create_dir_all(&second_root).expect("second agent root");
    let second_agent = EphemeralAgent::start(&second_root, "vela pin collision second");
    let second_frontier = temporary.path().join("second/frontier");
    let second_frontier_text = second_frontier.to_string_lossy().into_owned();
    // Copy the first repository's retained bootstrap, which carries its exact
    // repository_id, and resume `vela init` there against the second key.
    std::fs::create_dir_all(second_frontier.join(".vela")).expect("second bootstrap .vela");
    for retained in [
        "frontier.toml",
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        ".gitignore",
        ".gitattributes",
    ] {
        std::fs::copy(
            first_frontier.join(retained),
            second_frontier.join(retained),
        )
        .unwrap_or_else(|error| panic!("copy retained bootstrap {retained}: {error}"));
    }
    let git = Command::new("git")
        .args(["init", "--quiet", "-b", "main"])
        .arg(&second_frontier)
        .output()
        .expect("git init the copied bootstrap");
    assert!(
        git.status.success(),
        "{}",
        String::from_utf8_lossy(&git.stderr)
    );
    let collided = run(
        temporary.path(),
        Some(second_agent.socket()),
        &["init", &second_frontier_text, "--json"],
    );
    assert_eq!(collided.status.code(), Some(1));
    let collided = json(&collided);
    assert_eq!(collided["command"], "init");
    assert_eq!(collided["error"]["kind"], "domain");
    let message = collided["error"]["message"]
        .as_str()
        .expect("collision message");
    assert!(!message.contains("signing could not complete"), "{message}");
    assert!(message.contains("local trust pin"), "{message}");
    let hint = collided["error"]["hint"].as_str().expect("collision hint");
    assert!(!hint.contains("ssh-add"), "{hint}");
    assert!(hint.contains("--previous-record-root"), "{hint}");
}

#[test]
fn two_repositories_on_the_same_question_receive_different_identities() {
    // A Frontier is one independently clonable repository, so identity must
    // distinguish repositories rather than questions. Two groups may open
    // repositories on the same bounded question with the same wording; neither
    // may take the other's identity or its user-local trust anchor.
    let temporary = tempfile::tempdir().expect("temporary directory");
    let name = unique_name("Same question", &temporary);
    let scope = "Prove independent repositories keep independent identities.";

    let mut identities = Vec::new();
    let mut anchors = Vec::new();
    for label in ["first", "second"] {
        let agent_root = temporary.path().join(label);
        std::fs::create_dir_all(&agent_root).expect("agent root");
        let agent = EphemeralAgent::start(&agent_root, &format!("vela same question {label}"));
        let frontier = agent_root.join("frontier");
        let frontier_text = frontier.to_string_lossy().into_owned();
        let created = run(
            temporary.path(),
            Some(agent.socket()),
            &[
                "init",
                &frontier_text,
                "--name",
                &name,
                "--scope",
                scope,
                "--json",
            ],
        );
        assert!(
            created.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&created.stderr)
        );
        let created = json(&created);
        anchors.push(RemoveOnDrop(std::path::PathBuf::from(
            created["authority"]["local_trust"]["anchor_path"]
                .as_str()
                .expect("local trust anchor path"),
        )));
        identities.push(
            created["repository_id"]
                .as_str()
                .expect("repository_id")
                .to_string(),
        );
    }

    assert_ne!(identities[0], identities[1]);
    assert_ne!(anchors[0].0, anchors[1].0);
}

/// Content-addressed paths must never be end-of-line normalized.
///
/// A record's root is sha256 over the exact bytes Git holds. `text` or `eol=`
/// on such a path rewrites them on checkout, and replay then reads a file whose
/// digest is not the one the manifest binds — so the scaffold shipping
/// the record family with `text eol=lf` meant every Frontier had to
/// hand-correct its own `.gitattributes` before it could verify. All four did,
/// independently.
/// `FRONTIER_REPOSITORY_PROFILE.md` states the rule; nothing held the scaffold
/// to it.
#[test]
fn the_scaffold_never_normalizes_a_content_addressed_path() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela gitattributes scaffold test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let initialized = run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &frontier_text,
            "--name",
            &unique_name("Byte stability", &temporary),
            "--scope",
            "Hold content-addressed paths out of every checkout filter.",
            "--json",
        ],
    );
    assert!(
        initialized.status.success(),
        "the fixture frontier must initialize"
    );
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        json(&initialized)["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    let attributes = std::fs::read_to_string(frontier.join(".gitattributes"))
        .expect("scaffolded .gitattributes");
    for family in [".vela/**", "records/**", "artifacts/**"] {
        let line = attributes
            .lines()
            .find(|line| line.starts_with(family))
            .unwrap_or_else(|| panic!("the scaffold must declare {family}"));
        assert!(
            line.contains("-text"),
            "{family} must be -text so its bytes survive checkout: {line}"
        );
        assert!(
            !line.contains("eol="),
            "{family} must not be end-of-line normalized: {line}"
        );
    }
}
