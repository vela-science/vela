//! Native current-repository bootstrap and authority-genesis regression.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ed25519_dalek::SigningKey;
use serde_json::Value;
use vela_protocol::canonical::sha256_root;
use vela_protocol::signer_identity::{ActorClass, SignerIdentityV1};
use vela_protocol::submission::{
    RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    SubmissionRecordV3,
};
use vela_protocol::verification_record::{
    IndependenceDisclosure, VerificationMethod, VerificationRecordDraft,
    VerificationRecordEnvelopeV2, VerificationScope, VerificationSubject,
};

mod support;
use support::{
    EphemeralAgent, RemoveAnchorOnDrop as RemoveOnDrop,
    configure_git_identity as configure_test_git_identity, run_with_isolated_home as run_with_home,
    success_json,
};

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

fn exact_directory_snapshot(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = std::fs::read_dir(directory)
        .expect("read exact directory snapshot")
        .map(|entry| {
            let path = entry.expect("directory entry").path();
            (
                path.file_name()
                    .and_then(|name| name.to_str())
                    .expect("UTF-8 filename")
                    .to_string(),
                std::fs::read(path).expect("snapshot bytes"),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn tree_snapshot(root: &Path) -> Vec<(PathBuf, u32, Vec<u8>)> {
    use std::os::unix::fs::MetadataExt;

    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, u32, Vec<u8>)>) {
        for entry in std::fs::read_dir(path).expect("read snapshot directory") {
            let entry = entry.expect("snapshot entry");
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).expect("snapshot metadata");
            if metadata.is_dir() {
                visit(root, &path, entries);
            } else {
                entries.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    metadata.mode(),
                    std::fs::read(path).expect("snapshot bytes"),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn fresh_current_repository_replays_from_a_clean_clone() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela native genesis test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();

    let initialized = success_json(&run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &format!(
                "Native genesis fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Exercise one native current repository bootstrap.",
            "--json",
        ],
    ));
    assert_eq!(initialized["schema"], "vela.repository-init.v1");
    assert_eq!(initialized["authority"]["state"], "initialized");
    for retired in [
        ".vela/events",
        ".vela/actors.json",
        "frontier.json",
        "vela.lock",
    ] {
        assert!(
            !repository_path.join(retired).exists(),
            "retired path {retired}"
        );
    }

    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    let verified = success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["command"], "replay");
    let checked = success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(checked["repository_root"], verified["repository_root"]);
    let status = success_json(&run(&repository_path, None, &["status", ".", "--json"]));
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["integrity"]["replay"], "verified");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["decision_inbox"]["pending_count"], 0);
    assert!(
        status["decision_inbox"]["projection_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    assert!(status["actions"]["review"].is_null());
    assert_eq!(status["actions"]["work"]["mode"], "direct_submission");
    assert!(
        status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela submit "))
    );

    let clone = temporary.path().join("clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&repository_path)
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

    for (name, value) in [
        ("remote.origin.promisor", "true"),
        ("remote.origin.partialclonefilter", "blob:none"),
    ] {
        let configured = Command::new("git")
            .current_dir(&clone)
            .args(["config", name, value])
            .output()
            .expect("configure deferred partial-clone fixture");
        assert!(configured.status.success());
    }
    let before = tree_snapshot(&clone);
    let refused = run(&clone, None, &["replay", ".", "--json"]);
    assert_eq!(refused.status.code(), Some(1));
    let refused: Value =
        serde_json::from_slice(&refused.stdout).expect("offline storage error JSON");
    assert_eq!(refused["ok"], false);
    assert_eq!(refused["command"], "replay");
    assert_eq!(refused["error"]["kind"], "domain");
    assert!(refused["error"]["code"].is_null());
    assert_eq!(
        refused["error"]["message"],
        "Git repository storage is unsupported for exact offline reads; use a complete local repository without shallow or grafted history, config includes, partial-clone/promisor settings, or object alternates"
    );
    assert_eq!(tree_snapshot(&clone), before);
}

#[test]
fn current_replay_refuses_retired_repositories_before_parsing_them() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    std::fs::write(
        temporary.path().join("vela.toml"),
        "schema = \"vela.repository_path-profile.v1\"\n",
    )
    .expect("write retired profile marker");

    let output = run(temporary.path(), None, &["replay", ".", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let payload: Value = serde_json::from_slice(&output.stdout).expect("decode error JSON");
    assert_eq!(payload["ok"], false);
    assert!(
        payload["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("current repository origins"))
    );
}

#[test]
fn current_replay_blocks_sensitive_local_files() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela sensitive path test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let initialized = success_json(&run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &format!(
                "Sensitive path fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Reject local custody material.",
            "--json",
        ],
    ));
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));
    std::fs::write(
        repository_path.join("accidental-private.key"),
        "not a real key",
    )
    .expect("write sensitive-looking file");

    let output = run(&repository_path, None, &["replay", ".", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let payload: Value = serde_json::from_slice(&output.stdout).expect("decode error JSON");
    assert!(
        payload["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("sensitive-looking files"))
    );
}

#[test]
fn current_submission_and_verification_replay_without_changing_accepted_state() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela current submit test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let initialized = success_json(&run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &format!(
                "Current submission fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Commit and replay one current authenticated Submission.",
            "--json",
        ],
    ));
    configure_test_git_identity(&repository_path);
    let record_root = initialized["authority"]["record_root"]
        .as_str()
        .expect("authority record root");
    let pinned = success_json(&run(
        &repository_path,
        None,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            record_root,
            "--json",
        ],
    ));
    assert_eq!(pinned["operation"], "unchanged");
    let repeated_pin = success_json(&run(
        &repository_path,
        None,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            record_root,
            "--json",
        ],
    ));
    assert_eq!(repeated_pin["operation"], "unchanged");
    assert!(
        repeated_pin["writes"]
            .as_array()
            .expect("idempotent pin writes")
            .is_empty()
    );
    let anchor_path = std::path::PathBuf::from(
        pinned["authority_trust_anchor_path"]
            .as_str()
            .expect("trust anchor path"),
    );
    let _anchor = RemoveOnDrop(anchor_path.clone());
    std::fs::remove_file(&anchor_path).expect("remove routine writer trust pin");
    let actor = "agent:current-submission-regression";
    let artifact = b"{\"bounded\":true}\n";
    let artifact_digest = sha256_root(artifact);
    let artifact_stem = artifact_digest.trim_start_matches("sha256:").to_string();
    let bundle = temporary.path().join("bundle");
    std::fs::create_dir_all(&bundle).expect("Submission bundle directory");
    let producer_artifact_path = format!("records/artifacts/sha256/{artifact_stem}");
    let transport_artifact = bundle.join("artifacts/sha256").join(&artifact_stem);
    std::fs::create_dir_all(
        transport_artifact
            .parent()
            .expect("Submission transport directory"),
    )
    .expect("Submission transport directory");
    std::fs::write(&transport_artifact, artifact).expect("artifact bytes");
    let emitted_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let producer_key = SigningKey::from_bytes(&[57_u8; 32]);
    let identity =
        SignerIdentityV1::new(actor, ActorClass::Agent, &producer_key, emitted_at.clone())
            .expect("identity binding");
    let submission = SubmissionRecordV3::seal(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion: "The disposable fixture artifact contains bounded JSON evidence.".into(),
                claim_type: "computational".into(),
                conditions: vec!["Disposable integration fixture only.".into()],
            },
            artifacts: vec![SubmissionArtifact {
                kind: "witness".into(),
                path: producer_artifact_path.clone(),
                digest: artifact_digest,
            }],
            caveats: vec!["This fixture makes no unrestricted scientific claim.".into()],
            replayability: "exact".into(),
            producer_checks: Vec::new(),
            verification_requirements: vec!["Replay the retained artifact bytes.".into()],
            requested_change: RequestedChange {
                kind: "add_claim".into(),
                target: None,
            },
            provenance: SubmissionProvenance {
                producer: actor.into(),
                source_system: "vela-cli-regression".into(),
                source_run: Some("current-submission-regression".into()),
                emitted_at,
            },
        },
        identity,
        &producer_key,
    )
    .expect("Submission");
    let submission_path = bundle.join("submission.json");
    std::fs::write(&submission_path, &submission.bytes).expect("write Submission");
    let before = Command::new("git")
        .current_dir(&repository_path)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("read before commit");
    assert!(before.status.success());
    let authority_events_before =
        exact_directory_snapshot(&repository_path.join(".vela/authority/events"));
    let authority_records_before =
        exact_directory_snapshot(&repository_path.join(".vela/authority/records"));

    let submission_path_text = submission_path.to_string_lossy().into_owned();
    let submitted = success_json(&run(
        &repository_path,
        None,
        &["submit", &submission_path_text, "--repo", ".", "--json"],
    ));
    assert_eq!(submitted["schema"], "vela.submit-result.v1");
    assert_eq!(submitted["route"], "pending_review");
    assert_eq!(submitted["accepted_event_delta"], 0);
    assert_eq!(
        submitted["publication"]["state"], "committed_local",
        "unexpected publication outcome: {submitted}"
    );
    let missing_independent = run(
        &repository_path,
        Some(agent.socket()),
        &[
            "review",
            "accept",
            ".",
            submitted["proposal_id"].as_str().expect("proposal id"),
            "--reason",
            "Exercise the pre-Decision independent-check refusal.",
            "--json",
        ],
    );
    assert_eq!(missing_independent.status.code(), Some(1));
    let missing_independent: Value = serde_json::from_slice(&missing_independent.stdout)
        .expect("missing-independent error JSON");
    assert_eq!(
        missing_independent["error"]["code"],
        "missing_independent_verification"
    );
    assert!(
        missing_independent["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("vela verification record"))
    );
    std::fs::remove_file(&transport_artifact)
        .expect("remove producer-side transport path after canonical retention");

    let method_path = "verification/exact-replay-v1.json";
    let review_output_path = "reviews/exact-replay-report.json";
    std::fs::create_dir_all(repository_path.join("verification")).expect("method directory");
    std::fs::create_dir_all(repository_path.join("reviews")).expect("review output directory");
    std::fs::write(
        repository_path.join(method_path),
        br#"{"command":"sha256sum records/artifacts/sha256/<digest>","schema":"vela.test-method.v1"}"#,
    )
    .expect("method manifest");
    let review_output = b"{\"finding\":\"The retained fixture digest replayed exactly.\",\"schema\":\"vela.test-review-output.v1\"}\n";
    std::fs::write(repository_path.join(review_output_path), review_output).expect("review output");
    let untracked_method_home = temporary.path().join("untracked-method-home");
    std::fs::create_dir_all(&untracked_method_home).expect("untracked method home");
    let untracked_method = run_with_home(
        &repository_path,
        Some(agent.socket()),
        &untracked_method_home,
        &[
            "verification",
            "record",
            ".",
            submitted["proposal_id"].as_str().expect("proposal id"),
            "--profile",
            "exact-replay-v1",
            "--method",
            method_path,
            "--property",
            "Replay the retained artifact bytes.",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--as",
            "verifier:untracked-method-regression",
            "--json",
        ],
    );
    assert_eq!(untracked_method.status.code(), Some(1));
    let untracked_method: Value =
        serde_json::from_slice(&untracked_method.stdout).expect("untracked method error JSON");
    assert_eq!(untracked_method["command"], "verification.record");
    assert_eq!(
        untracked_method["error"]["message"],
        "Verification method manifest must be retained in the current Git commit"
    );
    assert!(
        untracked_method["error"]["hint"].as_str().is_some_and(
            |hint| hint.contains(method_path) && hint.contains("current repository HEAD")
        )
    );
    assert!(
        !untracked_method_home.join(".vela/agents").exists(),
        "method retention preflight must fail before verifier key creation"
    );
    let staged = Command::new("git")
        .current_dir(&repository_path)
        .args(["add", method_path, review_output_path])
        .status()
        .expect("stage method manifest");
    assert!(staged.success());
    let staged_method_home = temporary.path().join("staged-method-home");
    std::fs::create_dir_all(&staged_method_home).expect("staged method home");
    let staged_method = run_with_home(
        &repository_path,
        Some(agent.socket()),
        &staged_method_home,
        &[
            "verification",
            "record",
            ".",
            submitted["proposal_id"].as_str().expect("proposal id"),
            "--profile",
            "exact-replay-v1",
            "--method",
            method_path,
            "--property",
            "Replay the retained artifact bytes.",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--as",
            "verifier:staged-method-regression",
            "--json",
        ],
    );
    assert_eq!(staged_method.status.code(), Some(1));
    let staged_method: Value =
        serde_json::from_slice(&staged_method.stdout).expect("staged method error JSON");
    assert_eq!(staged_method["command"], "verification.record");
    assert_eq!(
        staged_method["error"]["message"],
        "Verification method manifest differs from the retained current Git bytes"
    );
    assert!(
        staged_method["error"]["hint"].as_str().is_some_and(
            |hint| hint.contains(method_path) && hint.contains("current repository HEAD")
        )
    );
    assert!(
        !staged_method_home.join(".vela/agents").exists(),
        "staged method preflight must fail before verifier key creation"
    );
    let committed = Command::new("git")
        .current_dir(&repository_path)
        .args([
            "-c",
            "user.name=Vela Test",
            "-c",
            "user.email=vela@example.invalid",
            "commit",
            "-qm",
            "retain verification method",
        ])
        .status()
        .expect("commit method manifest");
    assert!(committed.success());

    let verifier = format!(
        "verifier:current-record-{}",
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("fixture")
    );
    let verifier_home = temporary.path().join("verifier-home");
    std::fs::create_dir_all(&verifier_home).expect("verifier home");
    let missing_proposal = run_with_home(
        &repository_path,
        Some(agent.socket()),
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            &format!("vpr_{}", "f".repeat(16)),
            "--profile",
            "exact-replay-v1",
            "--method",
            method_path,
            "--property",
            "Replay the retained artifact bytes.",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--as",
            &verifier,
            "--json",
        ],
    );
    assert!(!missing_proposal.status.success());
    assert!(
        !verifier_home.join(".vela/agents").exists(),
        "Proposal preflight must fail before verifier key creation"
    );
    let missing_method = run_with_home(
        &repository_path,
        Some(agent.socket()),
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            submitted["proposal_id"].as_str().expect("proposal id"),
            "--profile",
            "exact-replay-v1",
            "--method",
            "verification/missing.json",
            "--property",
            "Replay the retained artifact bytes.",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--as",
            &verifier,
            "--json",
        ],
    );
    assert!(!missing_method.status.success());
    assert!(
        !verifier_home.join(".vela/agents").exists(),
        "method preflight must fail before verifier key creation"
    );

    let output_verifier = format!(
        "verifier:current-output-record-{}",
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("fixture")
    );
    let output_verifier_home = temporary.path().join("output-verifier-home");
    std::fs::create_dir_all(&output_verifier_home).expect("output verifier home");
    let output_record = success_json(&run_with_home(
        &repository_path,
        Some(agent.socket()),
        &output_verifier_home,
        &[
            "verification",
            "record",
            ".",
            submitted["proposal_id"].as_str().expect("proposal id"),
            "--profile",
            "exact-replay-v1",
            "--method",
            method_path,
            "--property",
            "Replay the retained artifact bytes.",
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific acceptance.",
            "--output",
            review_output_path,
            "--as",
            &output_verifier,
            "--json",
        ],
    ));
    assert_eq!(output_record["accepted_event_delta"], 0);
    assert_eq!(output_record["idempotent"], false);
    assert_eq!(output_record["publication"]["state"], "committed_local");
    let output_artifact_id = sha256_root(review_output)
        .trim_start_matches("sha256:")
        .to_string();
    assert_eq!(
        std::fs::read(
            repository_path
                .join("records/artifacts/sha256")
                .join(&output_artifact_id)
        )
        .expect("retained review output Artifact"),
        review_output
    );
    let output_record_root = output_record["verification_record_root"]
        .as_str()
        .expect("output Verification root")
        .trim_start_matches("sha256:");
    let retained_output_record = VerificationRecordEnvelopeV2::parse(
        &std::fs::read(
            repository_path
                .join("records/verifications/sha256")
                .join(format!("{output_record_root}.json")),
        )
        .expect("output Verification Record"),
    )
    .expect("parse output Verification Record");
    assert_eq!(
        retained_output_record.record.output_artifact_ids,
        vec![output_artifact_id]
    );

    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let verifier_key = SigningKey::from_bytes(&[58_u8; 32]);
    let verifier_identity = SignerIdentityV1::new(
        verifier.clone(),
        ActorClass::Agent,
        &verifier_key,
        observed_at.clone(),
    )
    .expect("Verifier identity");
    let method_root =
        sha256_root(&std::fs::read(repository_path.join(method_path)).expect("method bytes"));
    let verification_record = VerificationRecordEnvelopeV2::seal(
        VerificationRecordDraft {
            subject: VerificationSubject {
                claim_id: submitted["claim_id"]
                    .as_str()
                    .expect("claim id")
                    .to_string(),
                artifact_ids: vec![artifact_stem.clone()],
                submission_id: submitted["submission_id"]
                    .as_str()
                    .expect("submission id")
                    .to_string(),
                submission_root: submitted["submission_root"]
                    .as_str()
                    .expect("submission root")
                    .to_string(),
                proposal_id: submitted["proposal_id"]
                    .as_str()
                    .expect("proposal id")
                    .to_string(),
                proposal_root: submitted["proposal_root"]
                    .as_str()
                    .expect("proposal root")
                    .to_string(),
            },
            method: VerificationMethod {
                profile: "exact-replay-v1".into(),
                implementation: method_path.into(),
                environment_root: method_root,
            },
            scope: VerificationScope {
                property: "Replay the retained artifact bytes.".into(),
                does_not_establish: vec!["Scientific acceptance.".into()],
            },
            outcome: "pass".into(),
            independence: IndependenceDisclosure {
                declared_independent_of: vec![actor.into()],
                shared_dependencies: Vec::new(),
            },
            output_artifact_ids: Vec::new(),
            started_at: observed_at.clone(),
            completed_at: observed_at,
        },
        verifier_identity,
        &verifier_key,
    )
    .expect("Verification Record");
    let verification_inbox_path = bundle.join("verification.json");
    std::fs::write(&verification_inbox_path, &verification_record.bytes)
        .expect("write Verification Record");
    let verification_path_text = verification_inbox_path.to_string_lossy().into_owned();
    let verified = success_json(&run(
        &repository_path,
        None,
        &[
            "verification",
            "import",
            ".",
            &verification_path_text,
            "--as",
            &verifier,
            "--json",
        ],
    ));
    assert_eq!(verified["schema"], "vela.verification-import-result.v1");
    assert_eq!(verified["proposal_id"], submitted["proposal_id"]);
    assert_eq!(verified["claim_id"], submitted["claim_id"]);
    assert_eq!(verified["outcome"], "pass");
    assert_eq!(verified["accepted_event_delta"], 0);
    assert_eq!(verified["idempotent"], false);
    assert_eq!(verified["publication"]["state"], "committed_local");
    let verification_root = verified["verification_record_root"]
        .as_str()
        .expect("Verification Record root");
    let verification_path = format!(
        "records/verifications/sha256/{}.json",
        verification_root
            .strip_prefix("sha256:")
            .expect("full Verification Record root")
    );
    let retained = VerificationRecordEnvelopeV2::parse(
        &std::fs::read(repository_path.join(&verification_path))
            .expect("retained Verification Record"),
    )
    .expect("retained Verification Record envelope");
    assert_eq!(retained.record.verifier(), verifier);
    assert_eq!(
        retained.record.subject.proposal_id,
        submitted["proposal_id"].as_str().expect("proposal id")
    );
    assert_eq!(
        retained.record.subject.submission_id,
        submitted["submission_id"].as_str().expect("submission id")
    );
    assert_eq!(
        retained.record.subject.submission_root,
        submitted["submission_root"]
            .as_str()
            .expect("submission root")
    );
    assert_eq!(retained.record.subject.artifact_ids[0], artifact_stem);
    assert_eq!(retained.record.method.implementation, method_path);
    assert_eq!(
        retained.record.method.environment_root,
        sha256_root(&std::fs::read(repository_path.join(method_path)).expect("method bytes"))
    );
    assert_eq!(
        retained.record.independence.declared_independent_of[0],
        actor
    );

    let imported_again = success_json(
        &(run(
            &repository_path,
            None,
            &[
                "verification",
                "import",
                ".",
                &verification_path,
                "--as",
                &verifier,
                "--json",
            ],
        )),
    );
    assert_eq!(
        imported_again["verification_record_id"],
        verified["verification_record_id"]
    );
    assert_eq!(
        imported_again["verification_record_root"],
        verified["verification_record_root"]
    );
    assert_eq!(imported_again["accepted_event_delta"], 0);
    assert_eq!(
        exact_directory_snapshot(&repository_path.join(".vela/authority/events")),
        authority_events_before,
        "routine evidence must not append an authority Event"
    );
    assert_eq!(
        exact_directory_snapshot(&repository_path.join(".vela/authority/records")),
        authority_records_before,
        "routine evidence must not append an Authority Record"
    );
    assert_eq!(imported_again["idempotent"], true);

    let after = Command::new("git")
        .current_dir(&repository_path)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("read after commit");
    assert!(after.status.success());
    assert_ne!(before.stdout, after.stdout);
    let checked = success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(checked["counts"]["accepted_claims"], 0);
    assert_eq!(checked["counts"]["pending_claims"], 1);
    assert_eq!(checked["counts"]["verifications"], 2);
    let status = success_json(&run(&repository_path, None, &["status", ".", "--json"]));
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["git"]["role"], "repository_head");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["decision_inbox"]["pending_count"], 1);
    assert_eq!(status["decision_inbox"]["protocol_ready_count"], 1);
    assert_eq!(status["decision_inbox"]["protocol_blocked_count"], 0);
    assert!(
        status["decision_inbox"]["projection_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    assert!(
        status["decision_inbox"]["first_entry_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    let review_action = status["actions"]["review"]["command"]
        .as_str()
        .expect("status review action");
    assert!(review_action.starts_with("vela review inbox "));
    assert!(!review_action.contains(" accept "));
    assert!(!review_action.contains(" reject "));
    let work_action = status["actions"]["work"]["command"]
        .as_str()
        .expect("status work action");
    assert_eq!(status["actions"]["work"]["mode"], "direct_submission");
    assert!(work_action.starts_with("vela submit "));
    assert!(
        serde_json::to_vec(&status).expect("encode status").len() <= 16 * 1024,
        "status exceeds the compact projection budget"
    );
    let human_status = run(&repository_path, None, &["status", "."]);
    assert!(human_status.status.success());
    let human_status = String::from_utf8(human_status.stdout).expect("status text");
    assert!(human_status.lines().count() <= 40);
    assert!(!human_status.contains("review accept"));
    assert!(!human_status.contains("review reject"));
    let parallel_status = success_json(&run(&repository_path, None, &["status", ".", "--json"]));
    assert_eq!(parallel_status["decision_inbox"]["pending_count"], 1);
    assert_eq!(
        parallel_status["actions"]["work"]["mode"],
        "direct_submission"
    );
    assert!(
        parallel_status["actions"]["review"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela review inbox "))
    );
    assert!(
        parallel_status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela submit "))
    );
    let inbox_state_before = tree_snapshot(&repository_path.join(".vela"));
    let human_inbox = run(&repository_path, None, &["review", "inbox", "."]);
    assert!(human_inbox.status.success());
    let human_inbox = String::from_utf8(human_inbox.stdout).expect("inbox text");
    assert_eq!(human_inbox.matches("Inspect:").count(), 1);
    let inbox_output = run(&repository_path, None, &["review", "inbox", ".", "--json"]);
    assert!(inbox_output.status.success());
    let repeated_inbox_output = run(&repository_path, None, &["review", "inbox", ".", "--json"]);
    assert!(repeated_inbox_output.status.success());
    assert_eq!(inbox_output.stdout, repeated_inbox_output.stdout);
    let inbox = success_json(&inbox_output);
    assert_eq!(inbox["schema"], "vela.decision-inbox.v3");
    assert_eq!(inbox["entries"].as_array().map(Vec::len), Some(1));
    let reviewed_entry_root = inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("Decision Inbox entry root")
        .to_string();
    let proposal_id = submitted["proposal_id"].as_str().expect("Proposal ID");
    let review = success_json(&run(
        &repository_path,
        None,
        &["review", "show", ".", proposal_id, "--json"],
    ));
    assert_eq!(
        review["decision_inbox"]["entry"]["entry_root"],
        inbox["entries"][0]["entry_root"]
    );
    assert_eq!(
        review["decision_inbox"]["entry"]["standing_delta"]["before"]["repository_root"],
        checked["repository_root"]
    );
    assert_eq!(
        review["decision_inbox"]["entry"]["standing_delta"]["scope"]["affected_claim_ids"],
        serde_json::json!([submitted["claim_id"]])
    );
    assert_eq!(
        review["decision_inbox"]["entry"]["standing_delta"]["counts"]["global_accepted_claims"],
        serde_json::json!({"before": 0, "if_accept": 1, "if_reject": 0})
    );
    let after_inspection = success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(
        after_inspection["repository_root"],
        checked["repository_root"]
    );
    assert_eq!(after_inspection["counts"]["accepted_claims"], 0);
    assert_eq!(
        tree_snapshot(&repository_path.join(".vela")),
        inbox_state_before,
        "Decision Inbox inspection must not write canonical or authority state"
    );
    let uppercase_entry_root = format!("sha256:{}", "A".repeat(64));
    let malformed_decision = run(
        &repository_path,
        Some(agent.socket()),
        &[
            "review",
            "reject",
            ".",
            proposal_id,
            "--if-entry-root",
            &uppercase_entry_root,
            "--reason",
            "Exercise malformed root classification.",
            "--json",
        ],
    );
    assert!(!malformed_decision.status.success());
    let malformed_error: Value = serde_json::from_slice(&malformed_decision.stdout)
        .expect("malformed entry-root error JSON");
    assert_eq!(malformed_error["error"]["code"], Value::Null);
    assert_eq!(
        malformed_error["error"]["message"],
        "--if-entry-root must use lowercase hexadecimal"
    );

    let stale_entry_root = format!("sha256:{}", "0".repeat(64));
    let stale_decision = run(
        &repository_path,
        Some(agent.socket()),
        &[
            "review",
            "reject",
            ".",
            proposal_id,
            "--if-entry-root",
            &stale_entry_root,
            "--reason",
            "The reviewed Decision packet is intentionally stale.",
            "--json",
        ],
    );
    assert!(!stale_decision.status.success());
    let stale_error: Value =
        serde_json::from_slice(&stale_decision.stdout).expect("stale entry-root error JSON");
    assert_eq!(stale_error["error"]["code"], "decision_entry_stale");
    assert!(
        stale_error["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Decision Inbox entry changed")
                && message.contains("no authority signature was requested"))
    );
    assert!(
        stale_error["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("new exact entry_root"))
    );
    let after_stale_refusal =
        success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(
        after_stale_refusal["repository_root"],
        after_inspection["repository_root"]
    );

    let clone = temporary.path().join("submission-clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&repository_path)
        .arg(&clone)
        .output()
        .expect("clone submitted repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let replayed = success_json(&run(&clone, None, &["replay", ".", "--json"]));
    assert_eq!(replayed["repository_root"], checked["repository_root"]);
    assert_eq!(replayed["counts"]["accepted_claims"], 0);
    assert!(
        !clone.join(".vela/work").exists(),
        "obsolete private workflow scratch must not enter a clean clone"
    );
    let replayed_import = success_json(&run(
        &clone,
        None,
        &[
            "verification",
            "import",
            ".",
            &verification_path,
            "--as",
            &verifier,
            "--json",
        ],
    ));
    assert_eq!(replayed_import["idempotent"], true);
    assert_eq!(
        replayed_import["verification_record_id"],
        verified["verification_record_id"]
    );

    // Routine evidence does not depend on caller-local authority custody, but
    // a later attributed Decision still requires the independent sequence-one pin.
    let unpinned_decision = run(
        &repository_path,
        Some(agent.socket()),
        &[
            "review",
            "reject",
            ".",
            proposal_id,
            "--if-entry-root",
            &reviewed_entry_root,
            "--reason",
            "The fixture proves the evidence path but is not a scientific result.",
            "--json",
        ],
    );
    assert!(!unpinned_decision.status.success());
    let unpinned_error = format!(
        "{}{}",
        String::from_utf8_lossy(&unpinned_decision.stdout),
        String::from_utf8_lossy(&unpinned_decision.stderr)
    );
    assert!(
        unpinned_error.contains("independent sequence-one pin"),
        "unexpected unpinned Decision error: {unpinned_error}"
    );
    let repinned = success_json(&run(
        &repository_path,
        None,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            record_root,
            "--json",
        ],
    ));
    assert_eq!(repinned["operation"], "installed");

    // A later attributed Decision must checkpoint the exact self-authenticated
    // evidence overlay instead of requiring each routine write to have carried
    // an Authority Record of its own.
    let rejected = success_json(&run(
        &repository_path,
        Some(agent.socket()),
        &[
            "review",
            "reject",
            ".",
            proposal_id,
            "--if-entry-root",
            &reviewed_entry_root,
            "--reason",
            "The fixture proves the evidence path but is not a scientific result.",
            "--json",
        ],
    ));
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);
    let decided = success_json(&run(&repository_path, None, &["replay", ".", "--json"]));
    assert_eq!(decided["counts"]["accepted_claims"], 0);
    assert_eq!(decided["counts"]["pending_claims"], 0);

    let decided_clone = temporary.path().join("decided-clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&repository_path)
        .arg(&decided_clone)
        .output()
        .expect("clone decided repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let replayed_decision = success_json(&run(&decided_clone, None, &["replay", ".", "--json"]));
    assert_eq!(
        replayed_decision["repository_root"],
        decided["repository_root"]
    );
}
