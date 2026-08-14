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
//! the CLI-owned correction-impact reducer. Its interesting property here is
//! that the projection root is identical before and after the Decision: the
//! transition and the two Claim roots are what the projection is over, and
//! ruling on it changes none of them. The verb resolves the predecessor from
//! the accepted index beforehand and from the content-addressed store
//! afterwards, and has to reach the same answer either way.

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop, run_with_isolated_home as run, success_json};

fn git(repository_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repository_path)
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
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();

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
    git(&repository_path, &["config", "user.name", "Vela Test"]);
    git(
        &repository_path,
        &["config", "user.email", "vela@example.invalid"],
    );

    std::fs::write(repository_path.join("base.json"), b"{\"revision\":1}\n")
        .expect("write base artifact");
    let submitted = success_json(&run(
        &repository_path,
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
        &repository_path,
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

    let listed = success_json(&run(
        &repository_path,
        None,
        &home,
        &["claims", ".", "--json"],
    ));
    let base_root = listed["items"][0]["claim_root"]
        .as_str()
        .expect("accepted Claim root")
        .to_string();

    std::fs::write(
        repository_path.join("corrected.json"),
        b"{\"revision\":2}\n",
    )
    .expect("write corrected artifact");
    let correction = success_json(&run(
        &repository_path,
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
        &repository_path,
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
        &repository_path,
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
        let output = run(&repository_path, None, &home, &verb);
        assert!(
            output.status.success(),
            "`vela {}` failed after an accepted correction: {}",
            verb.join(" "),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let listed = success_json(&run(
        &repository_path,
        None,
        &home,
        &["claims", ".", "--json"],
    ));
    assert_eq!(listed["indexed"]["accepted"], 1);
    assert_eq!(listed["items"][0]["claim_id"], correction_claim.as_str());
    let correction_root = listed["items"][0]["claim_root"]
        .as_str()
        .expect("accepted correction Claim root");

    // The retained predecessor has historical Proposal status `accepted`, but
    // its current Claim Standing comes from the later supersession Event. The
    // two axes must not collapse in either machine or human read projections.
    let predecessor_why = success_json(&run(
        &repository_path,
        None,
        &home,
        &["why", ".", &base_claim, "--json"],
    ));
    assert_eq!(predecessor_why["claim_id"], base_claim.as_str());
    assert_eq!(predecessor_why["claim_root"], base_root.as_str());
    assert_eq!(predecessor_why["standing"], "superseded");
    assert_eq!(predecessor_why["proposal_status"], "accepted");
    assert_eq!(
        predecessor_why["chain"]["supersession"]["predecessor_claim_id"],
        base_claim.as_str()
    );
    assert_eq!(
        predecessor_why["chain"]["supersession"]["predecessor_claim_root"],
        base_root.as_str()
    );
    assert_eq!(
        predecessor_why["chain"]["supersession"]["successor_claim_id"],
        correction_claim.as_str()
    );
    assert_eq!(
        predecessor_why["chain"]["supersession"]["successor_claim_root"],
        correction_root
    );

    let successor_why = success_json(&run(
        &repository_path,
        None,
        &home,
        &["why", ".", &correction_claim, "--json"],
    ));
    assert_eq!(successor_why["standing"], "accepted");
    assert_eq!(successor_why["proposal_status"], "accepted");
    assert!(successor_why["chain"]["supersession"].is_null());

    let predecessor_show = success_json(&run(
        &repository_path,
        None,
        &home,
        &["show", ".", &base_claim, "--json"],
    ));
    assert_eq!(predecessor_show["object_kind"], "claim");
    assert_eq!(predecessor_show["content_root"], base_root.as_str());
    assert_eq!(
        predecessor_show["authority_effect"],
        "scientific standing is superseded, derived from current authority; the Proposal about it is accepted"
    );

    let human_why = run(&repository_path, None, &home, &["why", ".", &base_claim]);
    assert!(
        human_why.status.success(),
        "human why failed: {}",
        String::from_utf8_lossy(&human_why.stderr)
    );
    let human_why = String::from_utf8(human_why.stdout).expect("human why is UTF-8");
    assert!(human_why.contains(&format!(
        "why · {base_claim} · superseded · proposal accepted"
    )));
    assert!(human_why.contains(&format!("superseded by {correction_claim}")));

    // The shared Core projection keeps the authoritative Standing effect and
    // the successor's correction vocabulary on separate fields. A consumer
    // must never invent a `corrected` Standing from `relations[].kind`.
    let shared = success_json(&run(
        &repository_path,
        None,
        &home,
        &["projection", ".", "--json"],
    ));
    assert_eq!(shared["authority_effect"], "none");
    assert_eq!(shared["counts"]["claims"], 1);
    let projected_claims = shared["claims"].as_array().expect("projection Claims");
    let projected_predecessor = projected_claims
        .iter()
        .find(|claim| claim["claim_id"] == base_claim)
        .expect("retained projected predecessor");
    let projected_successor = projected_claims
        .iter()
        .find(|claim| claim["claim_id"] == correction_claim)
        .expect("projected correction successor");
    assert_eq!(projected_predecessor["standing"], "superseded");
    assert_eq!(projected_predecessor["proposal_status"], "accepted");
    assert_eq!(projected_successor["standing"], "accepted");
    let transitions = shared["transitions"]
        .as_array()
        .expect("projection transitions");
    assert_eq!(transitions.len(), 1, "one Decision produces one transition");
    let transition = &transitions[0];
    assert_eq!(
        transition["relation_kind"], "corrects",
        "relation vocabulary stays separate from Standing"
    );
    assert_eq!(transition["predecessor_claim_id"], base_claim);
    assert_eq!(transition["successor_claim_id"], correction_claim);
    assert_ne!(
        transition["decision_event"]["authority_event_id"],
        transition["applied_event"]["authority_event_id"],
        "review Decision and applied scientific Event identities stay distinct"
    );
    assert_eq!(
        shared["correction_impacts"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        shared["correction_impacts"][0]["projection_root"],
        projection_root
    );
    assert_eq!(shared["correction_impacts"][0]["predecessor_retired"], true);
    assert_eq!(
        shared["correction_impacts"][0]["projection"]["affected_claims"]
            .as_array()
            .map(Vec::len),
        Some(0),
        "a bounded empty cascade must stay explicit rather than disappear"
    );

    // Asked after the ruling. The predecessor now has to come from the
    // content-addressed store, and the answer has to be the same one.
    let after = success_json(&run(
        &repository_path,
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
