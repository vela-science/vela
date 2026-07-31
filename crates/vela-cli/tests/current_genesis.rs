//! Native current-repository bootstrap and authority-genesis regression.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::submission_v1::{
    RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    SubmissionV1,
};
use vela_protocol::verification_record::{
    IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationRecordV1,
    VerificationScope, VerificationSubject,
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

fn run_with_home(cwd: &Path, socket: Option<&Path>, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_AGENT_KEY_HEX");
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    } else {
        command.env_remove("SSH_AUTH_SOCK");
    }
    command.output().expect("run vela with isolated home")
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

fn campaign_host_request(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    request: &Value,
) -> Value {
    serde_json::to_writer(&mut *stdin, request).expect("encode Campaign host request");
    stdin
        .write_all(b"\n")
        .and_then(|()| stdin.flush())
        .expect("write Campaign host request");
    let mut line = String::new();
    let bytes = stdout
        .read_line(&mut line)
        .expect("read Campaign host response");
    assert!(bytes > 0, "Campaign host closed before replying");
    serde_json::from_str(&line).expect("decode Campaign host response")
}

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn install_current_target_index(frontier: &Path, socket: &Path) {
    std::fs::create_dir_all(frontier.join("domain")).expect("domain directory");
    std::fs::write(frontier.join("domain/source.json"), br#"{"open":[1056]}"#)
        .expect("target source");
    let committed = Command::new("git")
        .current_dir(frontier)
        .args(["add", "domain/source.json"])
        .status()
        .expect("stage target source");
    assert!(committed.success());
    let committed = Command::new("git")
        .current_dir(frontier)
        .args([
            "-c",
            "user.name=Vela Test",
            "-c",
            "user.email=vela@example.invalid",
            "commit",
            "-qm",
            "target source",
        ])
        .status()
        .expect("commit target source");
    assert!(committed.success());
    let source = Command::new("git")
        .current_dir(frontier)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("target source commit");
    assert!(source.status.success());
    let source = String::from_utf8(source.stdout)
        .expect("UTF-8 source commit")
        .trim()
        .to_string();
    let profile_source =
        std::fs::read_to_string(frontier.join("frontier.yaml")).expect("frontier profile");
    let frontier_id =
        vela_protocol::current_repository::CurrentFrontierProfileV2::from_yaml_str(&profile_source)
            .expect("current profile")
            .frontier_id;
    std::fs::create_dir_all(frontier.join("site/problems")).expect("packet directory");
    std::fs::write(
        frontier.join("site/problems/1056.json"),
        br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#,
    )
    .expect("target packet");
    std::fs::create_dir_all(frontier.join(".vela/tmp")).expect("candidate directory");
    std::fs::write(
        frontier.join(".vela/tmp/target-index-candidate.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "vela.target-index-candidate.v1",
            "frontier_id": frontier_id,
            "source": {
                "git_commit": source,
                "input_paths": ["domain/source.json"]
            },
            "targets": [{
                "id": "erdos:1056",
                "title": "Erdős 1056",
                "why": "First exact bounded target.",
                "state": "open",
                "rank": 1,
                "objective": "Produce one bounded artifact.",
                "labels": ["erdos", "open"],
                "packet": {
                    "schema": "erdos-frontier.problem-work.v1",
                    "path": "site/problems/1056.json"
                }
            }]
        }))
        .expect("candidate JSON"),
    )
    .expect("target candidate");
    let sealed = success_json(&run(
        frontier,
        Some(socket),
        &[
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--apply",
            "--json",
        ],
    ));
    assert_eq!(sealed["schema"], "vela.target-index-seal.v1");
    std::fs::remove_file(frontier.join(".vela/tmp/target-index-candidate.json"))
        .expect("remove source-local candidate");
    let committed = Command::new("git")
        .current_dir(frontier)
        .args(["add", "targets.json", "site/problems/1056.json"])
        .status()
        .expect("stage sealed Target Index");
    assert!(committed.success());
    let committed = Command::new("git")
        .current_dir(frontier)
        .args([
            "-c",
            "user.name=Vela Test",
            "-c",
            "user.email=vela@example.invalid",
            "commit",
            "-qm",
            "seal target index",
        ])
        .status()
        .expect("commit sealed Target Index");
    assert!(committed.success());
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
    assert_eq!(before["schema"], "vela.status.v1");
    assert_eq!(before["phase"], "authority_uninitialized");
    assert_eq!(before["integrity"]["strict"], "blocked");
    assert_eq!(before["campaign"]["active_attempt_count"], 0);
    assert!(before["campaign"]["first_attempt"].is_null());
    assert_eq!(before["decision_inbox"]["pending_count"], 0);
    assert!(before["decision_inbox"]["projection_root"].is_null());

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
    assert_eq!(status["schema"], "vela.status.v1");
    assert_eq!(status["integrity"]["replay"], "verified");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["campaign"]["active_attempt_count"], 0);
    assert_eq!(status["decision_inbox"]["pending_count"], 0);
    assert!(
        status["decision_inbox"]["projection_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    assert!(
        status["next_action"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela next "))
    );

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
                .is_some_and(|message| message.contains("current repository origins"))
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
fn current_submission_and_verification_replay_without_changing_accepted_state() {
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
    assert_eq!(pinned["operation"], "installed");
    let repeated_pin = success_json(&run(
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
    assert_eq!(repeated_pin["operation"], "unchanged");
    assert!(
        repeated_pin["writes"]
            .as_array()
            .expect("idempotent pin writes")
            .is_empty()
    );
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        pinned["authority_trust_anchor_path"]
            .as_str()
            .expect("trust anchor path"),
    ));
    install_current_target_index(&frontier, agent.socket());
    let actor = "agent:current-submission-regression";
    let attempt = success_json(&run(
        &frontier,
        None,
        &[
            "start",
            "erdos:1056",
            "--frontier",
            ".",
            "--max-submissions",
            "1",
            "--max-verifications",
            "1",
            "--artifact-class",
            "witness",
            "--as",
            actor,
            "--json",
        ],
    ));
    let attempt_id = attempt["attempt"]["id"]
        .as_str()
        .expect("Attempt ID")
        .to_string();

    let artifact = b"{\"bounded\":true}\n";
    let artifact_digest = format!("sha256:{}", hex::encode(Sha256::digest(artifact)));
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
                source_attempt: Some(attempt_id.clone()),
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

    let mut host = Command::new(env!("CARGO_BIN_EXE_vela"));
    host.current_dir(&frontier)
        .args([
            "campaign",
            "host",
            "--frontier",
            ".",
            "--attempt",
            &attempt_id,
            "--inbox",
            bundle.to_str().expect("Campaign inbox path"),
        ])
        .env("SSH_AUTH_SOCK", agent.socket())
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut host = host.spawn().expect("start Campaign evidence host");
    let mut host_stdin = host.stdin.take().expect("Campaign host stdin");
    let mut host_stdout = BufReader::new(host.stdout.take().expect("Campaign host stdout"));
    let submitted_response = campaign_host_request(
        &mut host_stdin,
        &mut host_stdout,
        &json!({
            "operation": "register_submission",
            "request_id": "host:submission",
            "path": "submission.json",
        }),
    );
    let replayed_submission = campaign_host_request(
        &mut host_stdin,
        &mut host_stdout,
        &json!({
            "operation": "register_submission",
            "request_id": "host:submission",
            "path": "submission.json",
        }),
    );
    let conflicting_submission = campaign_host_request(
        &mut host_stdin,
        &mut host_stdout,
        &json!({
            "operation": "register_submission",
            "request_id": "host:submission",
            "path": "other.json",
        }),
    );
    assert_eq!(submitted_response["ok"], true);
    assert_eq!(replayed_submission, submitted_response);
    assert_eq!(conflicting_submission["ok"], false);
    assert!(
        conflicting_submission["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("already bound"))
    );
    let submitted = submitted_response["result"].clone();
    assert_eq!(submitted["schema"], "vela.submit-result.v1");
    assert_eq!(submitted["route"], "pending_review");
    assert_eq!(submitted["accepted_event_delta"], 0);
    assert_eq!(
        submitted["publication"]["state"], "committed_local",
        "unexpected publication outcome: {submitted}"
    );
    std::fs::remove_file(&transport_artifact)
        .expect("remove producer-side transport path after canonical retention");

    let method_path = "verification/exact-replay-v1.json";
    std::fs::create_dir_all(frontier.join("verification")).expect("method directory");
    std::fs::write(
        frontier.join(method_path),
        br#"{"command":"sha256sum records/artifacts/sha256/<digest>","schema":"vela.test-method.v1"}"#,
    )
    .expect("method manifest");
    let staged = Command::new("git")
        .current_dir(&frontier)
        .args(["add", method_path])
        .status()
        .expect("stage method manifest");
    assert!(staged.success());
    let committed = Command::new("git")
        .current_dir(&frontier)
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
        &frontier,
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
            "--attempt",
            &attempt_id,
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
        &frontier,
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
            "--attempt",
            &attempt_id,
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

    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let verifier_key = SigningKey::from_bytes(&[58_u8; 32]);
    let verifier_identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: verifier.clone(),
            actor_class: ActorClass::Agent,
            created_at: observed_at.clone(),
        },
        &verifier_key,
    )
    .expect("Verifier identity");
    let method_root = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            std::fs::read(frontier.join(method_path)).expect("method bytes")
        ))
    );
    let verification_record = VerificationRecordV1::build(
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
            verifier: verifier.clone(),
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
    std::fs::write(
        &verification_inbox_path,
        verification_record
            .canonical_bytes()
            .expect("Verification Record bytes"),
    )
    .expect("write Verification Record");
    let verified_response = campaign_host_request(
        &mut host_stdin,
        &mut host_stdout,
        &json!({
            "operation": "import_verification",
            "request_id": "host:verification",
            "path": "verification.json",
        }),
    );
    assert_eq!(verified_response["ok"], true);
    let verified = verified_response["result"].clone();
    drop(host_stdin);
    drop(host_stdout);
    let hosted = host.wait_with_output().expect("wait for Campaign host");
    assert!(
        hosted.status.success(),
        "Campaign host failed\nstderr={}",
        String::from_utf8_lossy(&hosted.stderr)
    );
    assert_eq!(verified["schema"], "vela.verification-import-result.v1");
    assert_eq!(verified["proposal_id"], submitted["proposal_id"]);
    assert_eq!(verified["claim_id"], submitted["claim_id"]);
    assert_eq!(verified["outcome"], "pass");
    assert_eq!(verified["accepted_event_delta"], 0);
    assert_eq!(verified["idempotent"], false);
    assert_eq!(verified["publication"]["state"], "committed_local");
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(
        status["campaign"]["first_attempt"]["usage"]["submissions"],
        1
    );
    assert_eq!(
        status["campaign"]["first_attempt"]["usage"]["verifications"], 1,
        "hosted Verification import must charge its exact live Attempt once"
    );

    let verification_root = verified["verification_record_root"]
        .as_str()
        .expect("Verification Record root");
    let verification_path = format!(
        "records/verifications/sha256/{}.json",
        verification_root
            .strip_prefix("sha256:")
            .expect("full Verification Record root")
    );
    let retained: Value = serde_json::from_slice(
        &std::fs::read(frontier.join(&verification_path)).expect("retained Verification Record"),
    )
    .expect("Verification Record JSON");
    assert_eq!(retained["verifier"], verifier);
    assert_eq!(retained["subject"]["proposal_id"], submitted["proposal_id"]);
    assert_eq!(
        retained["subject"]["submission_id"],
        submitted["submission_id"]
    );
    assert_eq!(
        retained["subject"]["submission_root"],
        submitted["submission_root"]
    );
    assert_eq!(retained["subject"]["artifact_ids"][0], artifact_stem);
    assert_eq!(retained["method"]["implementation"], method_path);
    assert_eq!(
        retained["method"]["environment_root"],
        format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                std::fs::read(frontier.join(method_path)).expect("method bytes")
            ))
        )
    );
    assert_eq!(
        retained["independence"]["declared_independent_of"][0],
        actor
    );

    let imported_again = success_json(
        &(run(
            &frontier,
            Some(agent.socket()),
            &[
                "verification",
                "import",
                ".",
                &verification_path,
                "--attempt",
                &attempt_id,
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
    assert_eq!(imported_again["idempotent"], true);

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
    assert_eq!(checked["counts"]["verifications"], 1);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["schema"], "vela.status.v1");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["campaign"]["active_attempt_count"], 1);
    assert_eq!(
        status["campaign"]["first_attempt"]["usage"]["verifications"],
        1
    );
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
    let next_action = status["next_action"].as_str().expect("status next action");
    assert!(next_action.starts_with("vela review inbox "));
    assert!(!next_action.contains(" accept "));
    assert!(!next_action.contains(" reject "));
    assert!(
        serde_json::to_vec(&status).expect("encode status").len() <= 16 * 1024,
        "status exceeds the compact projection budget"
    );
    let human_status = run(&frontier, None, &["status", "."]);
    assert!(human_status.status.success());
    let human_status = String::from_utf8(human_status.stdout).expect("status text");
    assert!(human_status.lines().count() <= 40);
    assert!(!human_status.contains("review accept"));
    assert!(!human_status.contains("review reject"));
    let human_inbox = run(&frontier, None, &["review", "inbox", "."]);
    assert!(human_inbox.status.success());
    let human_inbox = String::from_utf8(human_inbox.stdout).expect("inbox text");
    assert_eq!(human_inbox.matches("Inspect:").count(), 1);
    let target_index: Value = serde_json::from_slice(
        &std::fs::read(frontier.join("targets.json")).expect("rebound Target Index"),
    )
    .expect("Target Index JSON");
    assert_eq!(
        target_index["repository"]["repository_root"],
        checked["repository_root"]
    );

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
    assert!(
        !clone.join(".vela/work").exists(),
        "private Attempt scratch must not enter a clean clone"
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
}
