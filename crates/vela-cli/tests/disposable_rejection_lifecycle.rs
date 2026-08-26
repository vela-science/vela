//! Maintained Core regression for the disposable Result Runner lifecycle:
//! empty init -> retained packet commit -> Submission -> failing Verification
//! -> rooted rejection -> status/replay/readback with no accepted Standing.

#![cfg(unix)]

use std::path::Path;
use std::process::Command;

use serde_json::Value;

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop, run_with_isolated_home as run, success_json};

fn git(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository)
        .args(args)
        .output()
        .expect("run Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git output is UTF-8")
        .trim()
        .into()
}

fn accepted_standing_commitment(repository: &Path) -> String {
    let repository: Value = serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json")).expect("repository manifest"),
    )
    .expect("repository JSON");
    format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&repository["accepted_claims"])
            .expect("accepted Standing commitment")
    )
}

#[test]
fn empty_disposable_repository_rejects_failed_verification_without_standing() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela disposable rejection test");
    let producer_home = temporary.path().join("producer-home");
    let verifier_home = temporary.path().join("verifier-home");
    std::fs::create_dir_all(&producer_home).expect("producer home");
    std::fs::create_dir_all(&verifier_home).expect("verifier home");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();

    assert!(
        !repository.exists(),
        "fixture must begin with an absent target"
    );
    let initialized = run(
        temporary.path(),
        Some(agent.socket()),
        &producer_home,
        &[
            "init",
            &repository_text,
            "--name",
            "Result Runner disposable qualification",
            "--scope",
            "Retain one non-scientific execution fixture",
            "--json",
        ],
    );
    let _anchor = RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized.stdout))
        .expect("init trust anchor");
    let initialized = success_json(&initialized);
    assert_eq!(initialized["scientific_object_count"], 0);
    git(&repository, &["config", "user.name", "Vela Result Runner"]);
    git(
        &repository,
        &["config", "user.email", "runner@invalid.local"],
    );

    std::fs::create_dir_all(repository.join("methods")).expect("method directory");
    std::fs::create_dir_all(repository.join("evidence")).expect("evidence directory");
    std::fs::write(
        repository.join("result.json"),
        b"{\"message\":\"integration\",\"qualification\":\"failed\"}\n",
    )
    .expect("result");
    std::fs::write(
        repository.join("evidence/verification-output.json"),
        b"{\"qualification\":\"fail\",\"scientific_claim\":false}\n",
    )
    .expect("verification output");
    let method = vela_protocol::review_method::ReviewMethodV1 {
        schema: vela_protocol::review_method::REVIEW_METHOD_V1_SCHEMA.into(),
        profile: "result-runner-qualification".into(),
        property: "Exact disposable Result Runner output retention".into(),
        question: "Did one bounded output traverse the disposable recording path?".into(),
        reviewer: vela_protocol::review_method::ReviewPerformerV1 {
            kind: "deterministic_tool".into(),
            display_name: "Deterministic Vela Result Runner qualification".into(),
            identifier: "sha256-and-git-replay".into(),
            provider: None,
            version: None,
        },
        attested_by_actor_id: "verifier:result-runner-qualification".into(),
        procedure: vec!["Compare the exact retained output and source identities.".into()],
        required_output: vec!["Retain the scoped qualification result.".into()],
        does_not_establish: vec![
            "Scientific truth, acceptance, utility, authority, or Standing.".into(),
        ],
    };
    std::fs::write(
        repository.join("methods/runner.json"),
        vela_protocol::canonical::to_canonical_bytes(&method).expect("canonical Review Method"),
    )
    .expect("Review Method");
    git(
        &repository,
        &[
            "add",
            "--",
            "result.json",
            "methods/runner.json",
            "evidence/verification-output.json",
        ],
    );
    git(
        &repository,
        &["commit", "-qm", "Retain disposable runner packet"],
    );
    let packet_commit = git(&repository, &["rev-parse", "HEAD^{commit}"]);

    let submitted = success_json(&run(
        &repository,
        None,
        &producer_home,
        &[
            "submit",
            "--repo",
            ".",
            "--claim",
            "Disposable Result Runner qualification failed; no scientific assertion.",
            "--type",
            "theoretical",
            "--replayability",
            "exact",
            "--artifact",
            "result.json:qualification-output",
            "--caveat",
            "Disposable runner qualification only; no scientific truth, utility, authority, or Standing.",
            "--requires-verification",
            "Exact disposable Result Runner output retention",
            "--source-run",
            "VELA-RESULT-RUNNER",
            "--as",
            "agent:result-runner",
            "--json",
        ],
    ));
    assert_eq!(submitted["accepted_event_delta"], 0);
    assert_eq!(submitted["accepted_state_changed"], false);
    let proposal = submitted["proposal_id"]
        .as_str()
        .expect("Proposal id")
        .to_string();
    let claim = submitted["claim_id"]
        .as_str()
        .expect("Claim id")
        .to_string();
    let standing_before_verification = accepted_standing_commitment(&repository);

    let verified = success_json(&run(
        &repository,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            &proposal,
            "--profile",
            "result-runner-qualification",
            "--method",
            "methods/runner.json",
            "--property",
            "Exact disposable Result Runner output retention",
            "--outcome",
            "fail",
            "--does-not-establish",
            "Scientific truth, acceptance, utility, authority, or Standing.",
            "--shared-dependency",
            "Same disposable host; this is not an independence claim.",
            "--output",
            "evidence/verification-output.json",
            "--as",
            "verifier:result-runner-qualification",
            "--json",
        ],
    ));
    assert_eq!(verified["outcome"], "fail");
    assert_eq!(verified["accepted_event_delta"], 0);

    let before = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(before["counts"]["accepted_claims"], 0);
    assert_eq!(before["counts"]["pending_claims"], 1);
    assert_eq!(before["counts"]["verifications"], 1);
    assert_eq!(
        accepted_standing_commitment(&repository),
        standing_before_verification,
        "a failing Verification Record must not change accepted Standing"
    );

    let incomplete_inbox = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["review", "inbox", ".", "--json"],
    ));
    assert_eq!(
        incomplete_inbox["entries"][0]["readiness"]["protocol_gate"],
        "blocked"
    );
    let incomplete_entry_root = incomplete_inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("incomplete rooted inbox entry")
        .to_string();
    let incomplete_accept = run(
        &repository,
        Some(agent.socket()),
        &producer_home,
        &[
            "review",
            "accept",
            ".",
            &proposal,
            "--if-entry-root",
            &incomplete_entry_root,
            "--reason",
            "This incomplete Decision must fail closed.",
            "--as",
            "agent:result-runner-qualification-owner",
            "--json",
        ],
    );
    assert!(!incomplete_accept.status.success());
    let after_incomplete = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(
        after_incomplete["repository_root"],
        before["repository_root"]
    );
    assert_eq!(
        accepted_standing_commitment(&repository),
        standing_before_verification
    );

    let passing = success_json(&run(
        &repository,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            &proposal,
            "--profile",
            "result-runner-qualification",
            "--method",
            "methods/runner.json",
            "--property",
            "Exact disposable Result Runner output retention",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific truth, acceptance, utility, authority, or Standing.",
            "--independent-of",
            "agent:result-runner",
            "--as",
            "verifier:result-runner-qualification",
            "--json",
        ],
    ));
    assert_eq!(passing["outcome"], "pass");
    assert_eq!(passing["accepted_event_delta"], 0);

    let contradicted = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(contradicted["counts"]["accepted_claims"], 0);
    assert_eq!(contradicted["counts"]["pending_claims"], 1);
    assert_eq!(contradicted["counts"]["verifications"], 2);
    assert_eq!(
        accepted_standing_commitment(&repository),
        standing_before_verification,
        "contradictory pass/fail Verification Records must not decide Standing"
    );

    let inbox = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["review", "inbox", ".", "--json"],
    ));
    assert_eq!(inbox["entries"][0]["readiness"]["protocol_gate"], "blocked");
    let mut roles = inbox["entries"][0]["verification_records"]
        .as_array()
        .expect("Verification Record views")
        .iter()
        .map(|record| {
            record["protocol_evidence_role"]
                .as_str()
                .expect("evidence role")
                .to_string()
        })
        .collect::<Vec<_>>();
    roles.sort();
    assert_eq!(roles, vec!["blocking", "requirement_satisfying"]);
    let entry_root = inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("rooted inbox entry")
        .to_string();
    let root_before_conflicting_accept = contradicted["repository_root"].clone();
    let conflicting_accept = run(
        &repository,
        Some(agent.socket()),
        &producer_home,
        &[
            "review",
            "accept",
            ".",
            &proposal,
            "--if-entry-root",
            &entry_root,
            "--reason",
            "This contradictory Decision must fail closed.",
            "--as",
            "agent:result-runner-qualification-owner",
            "--json",
        ],
    );
    assert!(!conflicting_accept.status.success());
    let after_conflicting = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(
        after_conflicting["repository_root"], root_before_conflicting_accept,
        "a contradictory Verification set must fail before mutation"
    );
    let rejected = success_json(&run(
        &repository,
        Some(agent.socket()),
        &producer_home,
        &[
            "review",
            "reject",
            ".",
            &proposal,
            "--if-entry-root",
            &entry_root,
            "--reason",
            "Qualification failed; reject without scientific Standing.",
            "--as",
            "agent:result-runner-qualification-owner",
            "--session-ref",
            "VELA-RESULT-RUNNER",
            "--json",
        ],
    ));
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);
    assert_eq!(
        rejected["repository_before"], contradicted["repository_root"],
        "the rejection must bind the exact repository containing both Verification Records"
    );

    let replay = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(replay["ok"], true);
    assert_eq!(replay["counts"]["accepted_claims"], 0);
    assert_eq!(replay["counts"]["pending_claims"], 0);
    assert_eq!(replay["counts"]["verifications"], 2);
    assert_eq!(
        accepted_standing_commitment(&repository),
        standing_before_verification,
        "an attributed rejection retains evidence and history without accepted Standing"
    );

    let status = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["status", ".", "--json"],
    ));
    assert_eq!(status["counts"]["accepted_claims"], 0);
    assert_eq!(status["counts"]["pending_claims"], 0);
    assert_eq!(status["decision_inbox"]["pending_count"], 0);

    let show = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["review", "show", ".", &proposal, "--json"],
    ));
    assert_eq!(show["status"], "rejected");
    assert_eq!(show["claim"]["claim_id"], claim);
    assert_eq!(show["decision"]["standing"], "rejected");
    assert_eq!(show["decision"]["session_ref"], "VELA-RESULT-RUNNER");

    let why = success_json(&run(
        &repository,
        None,
        &producer_home,
        &["why", ".", &claim, "--json"],
    ));
    assert_eq!(why["standing"], "unassessed");
    assert_eq!(why["proposal_status"], "rejected");
    assert_eq!(why["interpretation"]["verification_is_acceptance"], false);

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json")).expect("repository manifest"),
    )
    .expect("repository JSON");
    assert!(
        manifest["accepted_claims"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(
        manifest["pending_claims"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_ne!(
        git(&repository, &["rev-parse", "HEAD^{commit}"]),
        packet_commit
    );
    assert_eq!(
        git(
            &repository,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "the complete disposable lifecycle must leave one clean replayable Git state"
    );
}
