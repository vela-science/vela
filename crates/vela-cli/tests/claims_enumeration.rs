//! `vela claims` is the verb that makes the Claim surface reachable.
//!
//! Every other Claim read — `show`, `why` — takes a full 64-hex `vcl_` and
//! nothing produced one. `review list` reaches only the Claims a retained
//! Proposal carries, which on a compacted repository is a handful out of
//! thousands. These tests assert the properties that closed that gap and would
//! silently reopen: an id that the other read verbs actually accept, a page
//! boundary that resumes rather than restarts, a Standing filter spelled the
//! way the rows report Standing, and a row count a caller can trust.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

mod support;
use support::EphemeralAgent;

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

fn json(output: &Output) -> Value {
    serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim())
        .expect("vela --json must emit one JSON object")
}

/// `vela submit` commits what it retains, so the fixture needs a Git identity
/// the commit can carry. Without it the first Submission leaves its Artifact
/// installed but untracked and the second refuses the occupied path.
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

fn ids(payload: &Value) -> Vec<String> {
    payload["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["claim_id"].as_str().expect("claim id").to_string())
        .collect()
}

/* One test, not six: each needs an initialized repository with retained Claims,
and two ephemeral signing agents starting concurrently in the same process
race each other. */
#[test]
fn claims_enumerates_the_manifest_and_hands_back_usable_ids() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela claims test");
    let home = temporary.path().join("home");
    std::fs::create_dir_all(&home).expect("isolated home");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let socket = agent.socket();

    /* The repository id derives from name, scope, and key, and the trust anchor
    init installs is keyed by it in the OS account home, which no environment
    variable redirects. A fixed name would collide with the last run's. */
    let name = format!(
        "Claims enumeration fixture {}",
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unique")
    );
    let initialized = run(
        temporary.path(),
        &home,
        socket,
        &[
            "init",
            &frontier_text,
            "--name",
            &name,
            "--scope",
            "Exercise the Claim enumeration surface.",
            "--json",
        ],
    );
    let _anchor =
        support::RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized.stdout));

    configure_git_identity(&frontier);
    std::fs::create_dir_all(frontier.join("artifacts")).expect("artifacts directory");
    std::fs::write(
        frontier.join("artifacts/note.json"),
        b"{\"note\":\"claims fixture\"}\n",
    )
    .expect("fixture artifact");
    for assertion in [
        "First exact bounded fixture claim.",
        "Second exact bounded fixture claim.",
        "Third exact bounded fixture claim.",
    ] {
        let submitted = run(
            temporary.path(),
            &home,
            socket,
            &[
                "submit",
                "--repo",
                &frontier_text,
                "--claim",
                assertion,
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
                "--json",
            ],
        );
        let submitted = json(&submitted);
        assert_eq!(submitted["ok"], true, "submission failed: {submitted}");
    }

    // A Submission is not acceptance, so nothing stands and everything waits.
    let accepted = json(&run(
        temporary.path(),
        &home,
        socket,
        &["claims", &frontier_text, "--json"],
    ));
    assert_eq!(accepted["ok"], true);
    assert_eq!(accepted["command"], "claims");
    assert_eq!(accepted["schema"], "vela.claims.v1");
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(
        accepted["total"], 0,
        "a Submission cannot make a Claim stand"
    );

    /* The filter spells Claim standings. Accepting a near-miss would give a
    caller a filter that matches nothing and reports zero, which reads exactly
    like an empty queue — and `pending_review` is now a near-miss of exactly
    that kind: it is the Proposal's status, not this Claim's standing. */
    for near_miss in ["pending", "pending_review"] {
        let refused = run(
            temporary.path(),
            &home,
            socket,
            &["claims", &frontier_text, "--status", near_miss, "--json"],
        );
        assert_eq!(
            refused.status.code(),
            Some(2),
            "an unknown Standing filter is a usage error"
        );
        assert_eq!(json(&refused)["ok"], false);
    }

    let pending = json(&run(
        temporary.path(),
        &home,
        socket,
        &["claims", &frontier_text, "--status", "unassessed", "--json"],
    ));
    let all_ids = ids(&pending);
    assert_eq!(
        pending["indexed"],
        serde_json::json!({"accepted": 0, "unassessed": 3}),
        "the index counter is keyed by standing, not by Proposal status"
    );
    assert_eq!(pending["total"], 3);
    assert_eq!(all_ids.len(), 3);
    assert_eq!(pending["unreadable_returned"], 0);
    assert_eq!(pending["next_cursor"], Value::Null);
    assert_eq!(
        all_ids,
        {
            let mut sorted = all_ids.clone();
            sorted.sort();
            sorted
        },
        "rows must come out in the manifest's own Claim id order"
    );
    /* Asserting `pending_review` as the standing was asserting the collapse.
    The Proposal's status is answered by `why`, below, and this verb reports
    only the axis the manifest binds — a row that named a Proposal status would
    be naming a Proposal it never opened. */
    for item in pending["items"].as_array().expect("items") {
        assert_eq!(item["standing"], "unassessed");
        assert!(
            item.get("proposal_status").is_none(),
            "a claims row must not report an axis it does not read"
        );
        assert_eq!(item["readable"], true);
        assert_eq!(item["assertion_kind"], "theoretical");
        assert!(
            item["assertion"]
                .as_str()
                .is_some_and(|text| text.ends_with("exact bounded fixture claim.")),
            "each row must carry its own assertion: {item}"
        );
    }

    /* A genesis repository's origin commit bound no Claim, so everything here
    arrived after it. The era column is only worth printing if it can tell the
    two apart. */
    assert_eq!(pending["origin_claims"], 0);
    for item in pending["items"].as_array().expect("items") {
        assert_eq!(item["origin_era"], "post_origin");
    }

    // The gap this verb closes: an id the rest of the read surface accepts.
    let claim_id = all_ids.first().expect("at least one Claim").clone();
    assert!(claim_id.starts_with("vcl_") && claim_id.len() == 68);
    let shown = json(&run(
        temporary.path(),
        &home,
        socket,
        &["show", &frontier_text, &claim_id, "--json"],
    ));
    assert_eq!(shown["ok"], true, "claims produced an id `show` refuses");
    let why = json(&run(
        temporary.path(),
        &home,
        socket,
        &["why", &frontier_text, &claim_id, "--json"],
    ));
    assert_eq!(why["ok"], true, "claims produced an id `why` refuses");
    assert_eq!(why["claim_id"], claim_id.as_str());
    assert_eq!(why["standing"], "unassessed");
    assert_eq!(why["proposal_status"], "pending_review");

    // Paging resumes after the row the cursor names; it does not restart.
    let first = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "claims",
            &frontier_text,
            "--status",
            "unassessed",
            "--limit",
            "2",
            "--json",
        ],
    ));
    assert_eq!(first["total"], 3, "total counts the set, not the page");
    assert_eq!(ids(&first), all_ids[..2]);
    let cursor = first["next_cursor"]
        .as_str()
        .expect("a further row must produce a cursor")
        .to_string();
    assert_eq!(cursor, all_ids[1]);
    let second = json(&run(
        temporary.path(),
        &home,
        socket,
        &[
            "claims",
            &frontier_text,
            "--status",
            "unassessed",
            "--limit",
            "2",
            "--cursor",
            &cursor,
            "--json",
        ],
    ));
    assert_eq!(ids(&second), all_ids[2..]);
    assert_eq!(
        second["next_cursor"],
        Value::Null,
        "the last page must not invite one more round trip"
    );

    /* A cursor naming no row is refused. Silently restarting would return page
    one to a caller that believes it is reading page two. */
    let lost = run(
        temporary.path(),
        &home,
        socket,
        &[
            "claims",
            &frontier_text,
            "--cursor",
            &format!("vcl_{}", "0".repeat(64)),
            "--json",
        ],
    );
    assert_eq!(json(&lost)["ok"], false);
    assert_eq!(
        json(&lost)["error"]["message"],
        "claims cursor does not name an exact current Claim"
    );

    // A person gets a rendering, not a serialized object — and still gets the
    // full ids, which is the only reason to run this verb by hand.
    let rendered = stdout(&run(
        temporary.path(),
        &home,
        socket,
        &["claims", &frontier_text, "--status", "unassessed"],
    ));
    assert!(
        serde_json::from_str::<Value>(rendered.trim()).is_err(),
        "`vela claims` without --json returned a JSON document:\n{rendered}"
    );
    for id in &all_ids {
        assert!(
            rendered.contains(id.as_str()),
            "the human rendering must print the full Claim id {id}:\n{rendered}"
        );
    }
    assert!(rendered.contains("post_origin"), "{rendered}");
}
