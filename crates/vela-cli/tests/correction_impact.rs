//! One correction, driven end to end, and the two things it broke.
//!
//! Nothing in the suite had ever accepted a Proposal that corrects an already
//! accepted Claim. Doing it by hand for the first time produced a repository
//! its own loader refused:
//!
//! ```text
//! err · current Proposal vpr_… standing disagrees with the repository Claim indexes
//! ```
//!
//! Acceptance retires the predecessor, so it leaves the accepted index while
//! its own Proposal stays retained saying `accepted`.
//! `validate_current_proposal_standing` read those two facts as a
//! contradiction, and every read verb — `status`, `claims`, `replay`, `why`,
//! `review list` — failed on a repository that had done nothing but accept a
//! correction. A protocol whose central move is correction could not be read
//! after making one.
//!
//! The second half of this test is `vela correction impact`, which reaches
//! `vela-edge`'s correction-impact derivation. Its interesting property here is
//! that the projection root is identical before and after the Decision: the
//! transition and the two Claim roots are what the projection is over, and
//! ruling on it changes none of them. The verb resolves the predecessor from
//! the accepted index beforehand and from the content-addressed store
//! afterwards, and has to reach the same answer either way.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop};

fn run(cwd: &Path, socket: Option<&Path>, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_AGENT_KEY_HEX");
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
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

fn git(frontier: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(frontier)
        .args(["-c", "user.name=Vela Test"])
        .args(["-c", "user.email=vela@example.invalid"])
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed");
}

#[test]
fn an_accepted_correction_leaves_the_repository_readable_and_projectable() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let unique = temporary
        .path()
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture")
        .to_string();
    let agent = EphemeralAgent::start(temporary.path(), "vela correction impact test");
    let home = temporary.path().join("agent-home");
    std::fs::create_dir_all(&home).expect("isolated agent home");
    let frontier = temporary.path().join("frontier");
    let repository_path_text = frontier.to_string_lossy().into_owned();

    let init = run(
        temporary.path(),
        Some(agent.socket()),
        &home,
        &[
            "init",
            &repository_path_text,
            "--name",
            &format!("Correction impact fixture {unique}"),
            "--scope",
            "Accept one correction and read the repository afterwards.",
            "--json",
        ],
    );
    let _anchor = RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&init.stdout))
        .expect("init reports the local trust anchor it installed");
    success_json(&init);
    git(&frontier, &["config", "user.name", "Vela Test"]);
    git(&frontier, &["config", "user.email", "vela@example.invalid"]);

    std::fs::write(frontier.join("base.json"), b"{\"revision\":1}\n").expect("write base artifact");
    let submitted = success_json(&run(
        &frontier,
        None,
        &home,
        &[
            "submit",
            "--repo",
            ".",
            "--claim",
            "The fixture artifact records revision one.",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "base.json:witness",
            "--caveat",
            "This fixture makes no unrestricted scientific claim.",
            "--as",
            "agent:correction-impact-regression",
            "--json",
        ],
    ));
    let base_proposal = submitted["proposal_id"]
        .as_str()
        .expect("Proposal")
        .to_string();
    let base_claim = submitted["claim_id"].as_str().expect("Claim").to_string();

    success_json(&run(
        &frontier,
        Some(agent.socket()),
        &home,
        &[
            "review",
            "accept",
            &base_proposal,
            "--reason",
            "Admit the predecessor so a correction has something to correct.",
            "--json",
        ],
    ));

    let listed = success_json(&run(&frontier, None, &home, &["claims", ".", "--json"]));
    let base_root = listed["items"][0]["claim_root"]
        .as_str()
        .expect("accepted Claim root")
        .to_string();

    std::fs::write(frontier.join("corrected.json"), b"{\"revision\":2}\n")
        .expect("write corrected artifact");
    let correction = success_json(&run(
        &frontier,
        None,
        &home,
        &[
            "submit",
            "--repo",
            ".",
            "--claim",
            "The fixture artifact records revision two.",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "corrected.json:witness",
            "--caveat",
            "This fixture makes no unrestricted scientific claim.",
            "--corrects",
            &base_claim,
            "--target-root",
            &base_root,
            "--as",
            "agent:correction-impact-regression",
            "--json",
        ],
    ));
    let correction_proposal = correction["proposal_id"]
        .as_str()
        .expect("Proposal")
        .to_string();
    let correction_claim = correction["claim_id"].as_str().expect("Claim").to_string();

    // Asked before the ruling, which is when a repository authority has the
    // question: the successor is still unassessed and the predecessor still
    // stands.
    let before = success_json(&run(
        &frontier,
        None,
        &home,
        &["correction", "impact", ".", &correction_claim, "--json"],
    ));
    assert_eq!(before["projection"]["status"], "complete");
    assert_eq!(before["successor_standing"], "unassessed");
    assert_eq!(before["predecessor_retired"], false);
    assert_eq!(
        before["projection"]["transition"]["predecessor"]["claim_id"],
        base_claim.as_str()
    );
    let projection_root = before["projection_root"]
        .as_str()
        .expect("projection root")
        .to_string();

    success_json(&run(
        &frontier,
        Some(agent.socket()),
        &home,
        &[
            "review",
            "accept",
            &correction_proposal,
            "--reason",
            "Admit the correction and retire its predecessor.",
            "--json",
        ],
    ));

    // The regression. Before the loader learned that an accepted correction
    // retires its predecessor, every one of these failed.
    for verb in [
        vec!["status", ".", "--json"],
        vec!["claims", ".", "--json"],
        vec!["replay", ".", "--json"],
        vec!["review", "list", ".", "--json"],
    ] {
        let output = run(&frontier, None, &home, &verb);
        assert!(
            output.status.success(),
            "`vela {}` failed after an accepted correction: {}",
            verb.join(" "),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let listed = success_json(&run(&frontier, None, &home, &["claims", ".", "--json"]));
    assert_eq!(listed["indexed"]["accepted"], 1);
    assert_eq!(listed["items"][0]["claim_id"], correction_claim.as_str());

    // Asked after the ruling. The predecessor now has to come from the
    // content-addressed store, and the answer has to be the same one.
    let after = success_json(&run(
        &frontier,
        None,
        &home,
        &["correction", "impact", ".", &correction_claim, "--json"],
    ));
    assert_eq!(after["successor_standing"], "accepted");
    assert_eq!(after["predecessor_retired"], true);
    assert_eq!(
        after["projection_root"].as_str(),
        Some(projection_root.as_str()),
        "ruling on a correction must not move the projection it was ruled on"
    );

    // No `depends` or `supports` edge exists in this repository, and the write
    // path cannot author one — so the honest answer is an empty cascade, not a
    // fabricated one. The correction relation itself is reported as excluded
    // rather than dropped in silence.
    assert_eq!(
        after["projection"]["affected_claims"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        after["relations_excluded"]["unmapped_kind"][0]["kind"],
        "corrects"
    );
}
