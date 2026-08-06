//! The words the protocol fixes, and the one word it fixes on the wire.
//!
//! TERMINOLOGY.md bans "any unqualified use of `verified`, `valid`, `approved`,
//! or `complete`" in product wording, and prescribes the exact two sentences a
//! successful Submission reports. `status` and `replay` broke the first rule
//! and `submit` never printed the second, so this asserts both properties
//! rather than the particular replacement prose: the replacement is a matter of
//! taste and will change; that these words stay out of it is the contract.
//!
//! One exception is deliberate and asserted here too. `integrity.replay` in
//! `vela.status.v3` is a wire token, not prose: vela-web pins it as
//! `z.literal("verified")`. Retiring it is a coordinated schema change, so a
//! later prose sweep must not quietly take it with the others.
//!
//! `command` is asserted for the same reason. `review withdraw` named itself
//! `proposal.withdraw` when it succeeded and `proposal withdraw` when it
//! failed, so a caller keying on the field saw two names for one invocation and
//! neither was a verb the CLI accepts.

use std::path::Path;
use std::process::{Command, Output};

mod support;
use support::EphemeralAgent;

/// TERMINOLOGY.md, "Product wording": these are banned unqualified, so the test
/// looks for them as whole words and lets a qualified compound through.
const BANNED_UNQUALIFIED: [&str; 4] = ["verified", "valid", "approved", "complete"];

fn run(cwd: &Path, home: &Path, socket: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("SSH_AUTH_SOCK", socket)
        .output()
        .expect("run vela")
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "vela exited {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("vela output must be UTF-8")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("vela --json must emit one JSON object")
}

fn assert_no_banned_word(verb: &str, rendered: &str) {
    for word in rendered.split(|character: char| !character.is_ascii_alphanumeric()) {
        let word = word.to_ascii_lowercase();
        assert!(
            !BANNED_UNQUALIFIED.contains(&word.as_str()),
            "`vela {verb}` prints the unqualified word {word:?}, which TERMINOLOGY.md bans:\n{rendered}"
        );
    }
}

fn configure_git_identity(frontier: &Path) {
    for (key, value) in [
        ("user.name", "Vela Test"),
        ("user.email", "vela@example.invalid"),
    ] {
        let configured = Command::new("git")
            .current_dir(frontier)
            .args(["config", key, value])
            .status()
            .expect("configure test Git identity");
        assert!(configured.success());
    }
}

/* One test, not four: each needs an initialized Frontier, and two ephemeral
signing agents starting concurrently in the same process race each other. */
#[test]
fn the_cli_speaks_the_vocabulary_the_protocol_fixes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela wording contract test");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&home).expect("isolated home");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let socket = agent.socket();

    /* The Frontier id is derived from name, scope, and key, and the authority
    trust anchor it installs is keyed by that id in the operating-system account
    home, which no environment variable can redirect. A fixed name would make a
    second run collide with the anchor the first one left. */
    let name = format!(
        "Wording contract fixture {}",
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unique")
    );
    run(
        temporary.path(),
        &home,
        socket,
        &[
            "init",
            &frontier_text,
            "--name",
            &name,
            "--scope",
            "Exercise the wording TERMINOLOGY.md fixes.",
            "--json",
        ],
    );
    configure_git_identity(&frontier);

    for verb in ["status", "replay"] {
        let rendered = stdout(&run(
            temporary.path(),
            &home,
            socket,
            &[verb, &frontier_text],
        ));
        assert_no_banned_word(verb, &rendered);
    }

    let status = json(&run(
        temporary.path(),
        &home,
        socket,
        &["status", &frontier_text, "--json"],
    ));
    assert_eq!(
        status["integrity"]["replay"], "verified",
        "vela.status.v3 carries `verified` as a wire token; vela-web pins it as z.literal(\"verified\"), so it moves only with a schema bump"
    );

    std::fs::create_dir_all(frontier.join("artifacts")).expect("artifacts directory");
    std::fs::write(
        frontier.join("artifacts/note.json"),
        b"{\"note\":\"wording fixture\"}\n",
    )
    .expect("fixture artifact");
    let submit = [
        "submit",
        "--frontier",
        &frontier_text,
        "--claim",
        "Exact bounded fixture claim.",
        "--type",
        "theoretical",
        "--replayability",
        "exact",
        "--artifact",
        "artifacts/note.json:source-diff",
        "--caveat",
        "Fixture only.",
        "--as",
        "agent:fixture",
    ];
    let submitted = stdout(&run(temporary.path(), &home, socket, &submit));
    assert!(
        submitted.contains("Submission retained; review required."),
        "submit must report what a Submission is, in TERMINOLOGY.md's words:\n{submitted}"
    );
    assert!(
        submitted.contains("Accepted scientific state changed: no."),
        "submit must report what it did not change, in TERMINOLOGY.md's words:\n{submitted}"
    );

    let mut submit_json = submit.to_vec();
    submit_json.push("--json");
    submit_json[4] = "Second exact bounded fixture claim.";
    let second = json(&run(temporary.path(), &home, socket, &submit_json));
    assert_eq!(second["ok"], true, "second submission failed: {second}");
    let proposal = second["proposal_id"].as_str().expect("proposal id");

    let withdrawn = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "review",
            "withdraw",
            &frontier_text,
            proposal,
            "--as",
            "agent:fixture",
            "--reason",
            "Fixture withdrawal.",
            "--json",
        ],
    ));
    assert_eq!(withdrawn["ok"], true, "withdrawal failed: {withdrawn}");
    let refused = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "review",
            "withdraw",
            &frontier_text,
            &format!("vpr_{}", "0".repeat(16)),
            "--as",
            "agent:fixture",
            "--reason",
            "Fixture withdrawal.",
            "--json",
        ],
    ));
    assert_eq!(refused["ok"], false, "unknown Proposal was withdrawn");
    assert_eq!(
        withdrawn["command"], refused["command"],
        "`command` must not change with the outcome"
    );
    assert_eq!(
        withdrawn["command"], "review.withdraw",
        "`command` must name the verb the CLI accepts"
    );
}
