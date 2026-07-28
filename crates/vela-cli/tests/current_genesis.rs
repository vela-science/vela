//! Native current-repository bootstrap and authority-genesis regression.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

use ed25519_dalek::SigningKey;
use serde_json::Value;
use sha2::{Digest, Sha256};
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::submission_v1::{
    RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    SubmissionV1,
};

mod support;
use support::EphemeralAgent;

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
        command.env_remove("SSH_AUTH_SOCK");
    }
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

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn fresh_current_repository_replays_from_a_clean_clone() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela native genesis test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();

    let initialized = success_json(&run(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
            "--name",
            "Native genesis fixture",
            "--scope",
            "Exercise one native current repository bootstrap.",
            "--json",
        ],
    ));
    assert_eq!(initialized["schema"], "vela.frontier-init.v2");
    assert_eq!(initialized["authority"], "uninitialized");
    for retired in [
        ".vela/events",
        ".vela/actors.json",
        "frontier.json",
        "vela.lock",
    ] {
        assert!(!frontier.join(retired).exists(), "retired path {retired}");
    }

    let before = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(before["phase"], "authority_uninitialized");
    assert_eq!(before["integrity"]["strict"], "blocked");

    let authority = success_json(&run(
        &frontier,
        Some(agent.socket()),
        &[
            "authority",
            "init",
            ".",
            "--reason",
            "Establish native repository authority.",
            "--json",
        ],
    ));
    assert_eq!(
        authority["schema"],
        "vela.authority-initialization-result.v2"
    );
    assert_eq!(authority["writes_now"], true);

    let verified = success_json(&run(
        &frontier,
        None,
        &["repository", "verify", ".", "--json"],
    ));
    assert_eq!(verified["ok"], true);
    let checked = success_json(&run(&frontier, None, &["check", ".", "--strict", "--json"]));
    assert_eq!(checked["repository_root"], verified["repository_root"]);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["integrity"]["replay"], "verified");
    assert_eq!(status["integrity"]["strict"], "pass");

    let clone = temporary.path().join("clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&frontier)
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
}

#[test]
fn current_check_refuses_retired_repositories_before_parsing_them() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    std::fs::write(
        temporary.path().join("frontier.yaml"),
        "schema: vela.frontier-profile.v1\n",
    )
    .expect("write retired profile marker");

    for command in [
        vec!["check", ".", "--strict", "--json"],
        vec!["reproduce", ".", "--json"],
    ] {
        let output = run(temporary.path(), None, &command);
        assert_eq!(output.status.code(), Some(1));
        let payload: Value = serde_json::from_slice(&output.stdout).expect("decode error JSON");
        assert_eq!(payload["ok"], false);
        assert!(
            payload["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("current repository epochs"))
        );
    }
}

#[test]
fn current_check_blocks_sensitive_local_files() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela sensitive path test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    success_json(&run(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
            "--name",
            "Sensitive path fixture",
            "--scope",
            "Reject local custody material.",
            "--json",
        ],
    ));
    success_json(&run(
        &frontier,
        Some(agent.socket()),
        &[
            "authority",
            "init",
            ".",
            "--reason",
            "Establish native repository authority.",
            "--json",
        ],
    ));
    std::fs::write(frontier.join("accidental-private.key"), "not a real key")
        .expect("write sensitive-looking file");

    let output = run(&frontier, None, &["check", ".", "--strict", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let payload: Value = serde_json::from_slice(&output.stdout).expect("decode error JSON");
    assert!(
        payload["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("sensitive-looking files"))
    );
}

#[test]
fn current_submission_commits_and_replays_without_changing_accepted_state() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela current submit test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    success_json(&run(
        temporary.path(),
        None,
        &[
            "init",
            &frontier_text,
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
    let authority = success_json(&run(
        &frontier,
        Some(agent.socket()),
        &[
            "authority",
            "init",
            ".",
            "--reason",
            "Establish native repository authority.",
            "--json",
        ],
    ));
    let record_root = authority["authority_record_root"]
        .as_str()
        .expect("authority record root");
    let pinned = success_json(&run(
        &frontier,
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
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        pinned["authority_trust_anchor_path"]
            .as_str()
            .expect("trust anchor path"),
    ));

    let artifact = b"{\"bounded\":true}\n";
    let artifact_digest = format!("sha256:{}", hex::encode(Sha256::digest(artifact)));
    let artifact_stem = artifact_digest.trim_start_matches("sha256:");
    let bundle = temporary.path().join("bundle");
    std::fs::create_dir_all(bundle.join("artifacts/sha256")).expect("artifact directory");
    std::fs::write(
        bundle.join("artifacts/sha256").join(artifact_stem),
        artifact,
    )
    .expect("artifact bytes");
    let actor = "agent:current-submission-regression";
    let emitted_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let producer_key = SigningKey::from_bytes(&[57_u8; 32]);
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.into(),
            actor_class: ActorClass::Agent,
            created_at: emitted_at.clone(),
        },
        &producer_key,
    )
    .expect("identity binding");
    let submission = SubmissionV1::build(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion: "The disposable fixture artifact contains bounded JSON evidence.".into(),
                claim_type: "computational".into(),
                conditions: vec!["Disposable integration fixture only.".into()],
            },
            artifacts: vec![SubmissionArtifact {
                kind: "witness".into(),
                path: format!("records/artifacts/sha256/{artifact_stem}"),
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
                source_attempt: None,
                source_run: Some("current-submission-regression".into()),
                emitted_at,
            },
            execution_binding: None,
        },
        identity,
        &producer_key,
    )
    .expect("Submission");
    let submission_path = bundle.join("submission.json");
    std::fs::write(
        &submission_path,
        submission.canonical_bytes().expect("Submission bytes"),
    )
    .expect("write Submission");
    let before = Command::new("git")
        .current_dir(&frontier)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("read before commit");
    assert!(before.status.success());

    let submitted = success_json(&run(
        &frontier,
        Some(agent.socket()),
        &[
            "submit",
            submission_path.to_str().expect("Submission path"),
            "--frontier",
            ".",
            "--as",
            actor,
            "--json",
        ],
    ));
    assert_eq!(submitted["schema"], "vela.submit-result.v1");
    assert_eq!(submitted["route"], "pending_review");
    assert_eq!(submitted["accepted_event_delta"], 0);
    assert_eq!(submitted["publication"]["state"], "committed_local");

    let after = Command::new("git")
        .current_dir(&frontier)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("read after commit");
    assert!(after.status.success());
    assert_ne!(before.stdout, after.stdout);
    let checked = success_json(&run(
        &frontier,
        None,
        &["repository", "verify", ".", "--json"],
    ));
    assert_eq!(checked["counts"]["accepted_claims"], 0);
    assert_eq!(checked["counts"]["pending_claims"], 1);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["integrity"]["strict"], "pass");

    let clone = temporary.path().join("submission-clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&frontier)
        .arg(&clone)
        .output()
        .expect("clone submitted repository");
    assert!(
        cloned.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let replayed = success_json(&run(&clone, None, &["repository", "verify", ".", "--json"]));
    assert_eq!(replayed["repository_root"], checked["repository_root"]);
    assert_eq!(replayed["counts"]["accepted_claims"], 0);
}
