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
        command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock"));
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
        command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock"));
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

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn git_text(frontier: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(frontier)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("UTF-8 git output")
        .trim()
        .to_string()
}

fn configure_test_git_identity(frontier: &Path) {
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

fn install_current_target_index(frontier: &Path, _socket: &Path) {
    std::fs::create_dir_all(frontier.join("domain")).expect("domain directory");
    std::fs::write(frontier.join("domain/source.json"), br#"{"open":[1056]}"#)
        .expect("target source");
    std::fs::create_dir_all(frontier.join("site/problems")).expect("packet directory");
    std::fs::write(
        frontier.join("site/problems/1056.json"),
        br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1","verifier_profile":"exact-replay-v1"}"#,
    )
    .expect("target packet");
    let committed = Command::new("git")
        .current_dir(frontier)
        .args(["add", "domain/source.json", "site/problems/1056.json"])
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
    let source = git_text(frontier, &["rev-parse", "HEAD^{commit}"]);
    let source_tree = git_text(frontier, &["rev-parse", "HEAD^{tree}"]);
    let profile_source =
        std::fs::read_to_string(frontier.join("vela.toml")).expect("repository profile");
    let repository_id =
        vela_protocol::current_repository::CurrentRepositoryProfileV1::from_toml_str(
            &profile_source,
        )
        .expect("current profile")
        .repository_id;
    let repository_bytes =
        std::fs::read(frontier.join(".vela/repository.json")).expect("repository manifest");
    let repository =
        vela_protocol::current_repository::CurrentRepositoryV4::parse(&repository_bytes)
            .expect("current repository");
    let source_bytes = std::fs::read(frontier.join("domain/source.json")).expect("source bytes");
    let mut inputs = vela_edge::target_index::TargetIndexInputManifestV1 {
        schema: vela_edge::target_index::TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1.to_string(),
        input_root: format!("sha256:{}", "0".repeat(64)),
        entries: vec![vela_protocol::repository_inputs::RetainedObjectEntryV1 {
            path: "domain/source.json".to_string(),
            git_mode: "100644".to_string(),
            size: source_bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&source_bytes)),
        }],
    };
    inputs.input_root = inputs.computed_root().expect("input root");
    let packet_bytes =
        std::fs::read(frontier.join("site/problems/1056.json")).expect("packet bytes");
    let mut index = vela_edge::target_index::TargetIndexV5 {
        schema: vela_edge::target_index::TARGET_INDEX_SCHEMA_V5.to_string(),
        repository_id,
        source: vela_edge::target_index::TargetIndexSourceV2 {
            git_object_format: vela_protocol::repository_inputs::GitObjectFormat::Sha1,
            git_commit: source,
            git_tree: source_tree,
        },
        inputs,
        repository: vela_edge::target_index::TargetIndexRepositoryV4 {
            origin_id: repository.origin_id.clone(),
            repository_root: repository.canonical_root().expect("repository root"),
        },
        claim_boundary: vela_edge::target_index::TargetIndexClaimBoundaryV2 {
            derived: true,
            authoritative: false,
            deletable: true,
        },
        targets: vec![vela_edge::target_index::TargetIndexEntryV2 {
            id: "erdos:1056".to_string(),
            title: "Erdős 1056".to_string(),
            why: "First exact bounded target.".to_string(),
            presence: "open".to_string(),
            rank: 1,
            objective: "Produce one bounded artifact.".to_string(),
            labels: vec!["erdos".to_string(), "open".to_string()],
            packet: vela_edge::target_index::TargetPacketRefV2 {
                schema: "erdos-frontier.problem-work.v1".to_string(),
                path: "site/problems/1056.json".to_string(),
                size: packet_bytes.len() as u64,
                sha256: format!("sha256:{}", hex::encode(Sha256::digest(&packet_bytes))),
            },
        }],
        index_root: format!("sha256:{}", "0".repeat(64)),
    };
    index.index_root = index.computed_index_root().expect("index root");
    std::fs::write(
        frontier.join("targets.json"),
        index.canonical_bytes().expect("canonical Target Index"),
    )
    .expect("write Target Index");
    let committed = Command::new("git")
        .current_dir(frontier)
        .args(["add", "targets.json", "site/problems/1056.json"])
        .status()
        .expect("stage Target Index");
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
            "add target index",
        ])
        .status()
        .expect("commit Target Index");
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
        Some(agent.socket()),
        &[
            "init",
            &frontier_text,
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
        assert!(!frontier.join(retired).exists(), "retired path {retired}");
    }

    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    let verified = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["command"], "replay");
    let checked = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(checked["repository_root"], verified["repository_root"]);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["integrity"]["replay"], "verified");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["work"]["ready_target_count"], 0);
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
fn current_replay_refuses_retired_repositories_before_parsing_them() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    std::fs::write(
        temporary.path().join("vela.toml"),
        "schema = \"vela.frontier-profile.v1\"\n",
    )
    .expect("write retired profile marker");

    for command in [
        vec!["replay", ".", "--json"],
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
fn current_replay_blocks_sensitive_local_files() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela sensitive path test");
    let frontier = temporary.path().join("frontier");
    let frontier_text = frontier.to_string_lossy().into_owned();
    let initialized = success_json(&run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &frontier_text,
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
    std::fs::write(frontier.join("accidental-private.key"), "not a real key")
        .expect("write sensitive-looking file");

    let output = run(&frontier, None, &["replay", ".", "--json"]);
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
    let initialized = success_json(&run(
        temporary.path(),
        Some(agent.socket()),
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
    configure_test_git_identity(&frontier);
    let record_root = initialized["authority"]["record_root"]
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
    assert_eq!(pinned["operation"], "unchanged");
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
    let anchor_path = std::path::PathBuf::from(
        pinned["authority_trust_anchor_path"]
            .as_str()
            .expect("trust anchor path"),
    );
    let _anchor = RemoveOnDrop(anchor_path.clone());
    install_current_target_index(&frontier, agent.socket());
    std::fs::remove_file(&anchor_path).expect("remove routine writer trust pin");
    let actor = "agent:current-submission-regression";
    let offer = success_json(&run(&frontier, None, &["next", ".", "--json"]));
    assert_eq!(offer["targets"][0]["queue_position"], 1);
    assert_eq!(offer["targets"][0]["rank"], 1);
    let briefing = success_json(&run(
        &frontier,
        None,
        &["start", "erdos:1056", "--repo", ".", "--json"],
    ));
    assert_eq!(briefing["schema"], "vela.start-briefing.v2");
    assert_eq!(briefing["target"]["id"], "erdos:1056");
    assert_eq!(briefing["objective"], "Produce one bounded artifact.");
    assert_eq!(
        briefing["scope"]["question"],
        "Commit and replay one current authenticated Submission."
    );
    assert_eq!(briefing["packet"]["problem"], 1056);
    assert_eq!(briefing["verifier"], "exact-replay-v1");
    assert!(vela_protocol::execution_binding::is_full_sha256_root(
        briefing["packet_root"].as_str().expect("packet root")
    ));
    assert!(
        briefing["repository"]["origin_id"]
            .as_str()
            .expect("origin id")
            .starts_with("vro_")
    );
    assert!(briefing["target_index_root"].as_str().is_some());
    assert_eq!(briefing["git"]["role"], "target_index_source");
    /* The one place in the suite with a live Target Index, so the one place
    that can reach `start`'s miss rather than the earlier "no Target Index"
    domain failure. tests/exit_code_contract.rs covers the rest of the codes. */
    let absent = run(
        &frontier,
        None,
        &["start", "erdos:0", "--repo", ".", "--json"],
    );
    assert_eq!(absent.status.code(), Some(3), "start on an absent Target");
    let absent: Value = serde_json::from_slice(&absent.stdout).expect("decode start failure");
    assert_eq!(absent["error"]["kind"], "not_found");
    assert!(briefing["git"]["commit"].as_str().is_some());
    assert!(briefing["git"]["tree"].as_str().is_some());
    assert!(
        briefing["authority_ceiling"]
            .as_str()
            .expect("authority ceiling")
            .contains("human Decision")
    );
    let keys = briefing
        .as_object()
        .expect("start briefing object")
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "authority_ceiling",
            "git",
            "objective",
            "packet",
            "packet_root",
            "repository",
            "schema",
            "scope",
            "target",
            "target_index_root",
            "verifier",
        ])
    );

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
    let authority_events_before =
        exact_directory_snapshot(&frontier.join(".vela/authority/events"));
    let authority_records_before =
        exact_directory_snapshot(&frontier.join(".vela/authority/records"));

    let submission_path_text = submission_path.to_string_lossy().into_owned();
    let submitted = success_json(&run(
        &frontier,
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
    std::fs::remove_file(&transport_artifact)
        .expect("remove producer-side transport path after canonical retention");

    let method_path = "verification/exact-replay-v1.json";
    std::fs::create_dir_all(frontier.join("verification")).expect("method directory");
    std::fs::write(
        frontier.join(method_path),
        br#"{"command":"sha256sum records/artifacts/sha256/<digest>","schema":"vela.test-method.v1"}"#,
    )
    .expect("method manifest");
    let untracked_method_home = temporary.path().join("untracked-method-home");
    std::fs::create_dir_all(&untracked_method_home).expect("untracked method home");
    let untracked_method = run_with_home(
        &frontier,
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
    let verification_path_text = verification_inbox_path.to_string_lossy().into_owned();
    let verified = success_json(&run(
        &frontier,
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
        exact_directory_snapshot(&frontier.join(".vela/authority/events")),
        authority_events_before,
        "routine evidence must not append an authority Event"
    );
    assert_eq!(
        exact_directory_snapshot(&frontier.join(".vela/authority/records")),
        authority_records_before,
        "routine evidence must not append an Authority Record"
    );
    assert_eq!(imported_again["idempotent"], true);

    let after = Command::new("git")
        .current_dir(&frontier)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("read after commit");
    assert!(after.status.success());
    assert_ne!(before.stdout, after.stdout);
    let checked = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(checked["counts"]["accepted_claims"], 0);
    assert_eq!(checked["counts"]["pending_claims"], 1);
    assert_eq!(checked["counts"]["verifications"], 1);
    let status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["git"]["role"], "repository_head");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["work"]["ready_target_count"], 1);
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
    assert_eq!(status["actions"]["work"]["mode"], "target");
    assert!(work_action.starts_with("vela next "));
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
    let parallel_status = success_json(&run(&frontier, None, &["status", ".", "--json"]));
    assert_eq!(parallel_status["decision_inbox"]["pending_count"], 1);
    assert_eq!(parallel_status["actions"]["work"]["mode"], "target");
    assert_eq!(parallel_status["actions"]["work"]["ready_target_count"], 1);
    assert!(
        parallel_status["actions"]["review"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela review inbox "))
    );
    assert!(
        parallel_status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela next "))
    );
    let human_inbox = run(&frontier, None, &["review", "inbox", "."]);
    assert!(human_inbox.status.success());
    let human_inbox = String::from_utf8(human_inbox.stdout).expect("inbox text");
    assert_eq!(human_inbox.matches("Inspect:").count(), 1);
    let inbox = success_json(&run(&frontier, None, &["review", "inbox", ".", "--json"]));
    assert_eq!(inbox["schema"], "vela.decision-inbox.v2");
    assert_eq!(inbox["entries"].as_array().map(Vec::len), Some(1));
    let reviewed_entry_root = inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("Decision Inbox entry root")
        .to_string();
    let proposal_id = submitted["proposal_id"].as_str().expect("Proposal ID");
    let review = success_json(&run(
        &frontier,
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
    let after_inspection = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(
        after_inspection["repository_root"],
        checked["repository_root"]
    );
    assert_eq!(after_inspection["counts"]["accepted_claims"], 0);
    let stale_entry_root = format!("sha256:{}", "0".repeat(64));
    let stale_decision = run(
        &frontier,
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
    let stale_error = format!(
        "{}{}",
        String::from_utf8_lossy(&stale_decision.stdout),
        String::from_utf8_lossy(&stale_decision.stderr)
    );
    assert!(stale_error.contains("Decision Inbox entry changed"));
    assert!(stale_error.contains("no authority signature was requested"));
    let after_stale_refusal = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(
        after_stale_refusal["repository_root"],
        after_inspection["repository_root"]
    );
    let target_index: Value = serde_json::from_slice(
        &std::fs::read(frontier.join("targets.json")).expect("rebound Target Index"),
    )
    .expect("Target Index JSON");
    assert_eq!(target_index["schema"], "vela.target-index.v5");
    assert_eq!(target_index["targets"][0]["presence"], "open");
    assert!(target_index["targets"][0].get("state").is_none());
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
    // a later human Decision still requires the independent sequence-one pin.
    let unpinned_decision = run(
        &frontier,
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
    assert_eq!(repinned["operation"], "installed");

    // A later human Decision must checkpoint the exact self-authenticated
    // evidence overlay instead of requiring each routine write to have carried
    // an Authority Record of its own.
    let rejected = success_json(&run(
        &frontier,
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
    let decided = success_json(&run(&frontier, None, &["replay", ".", "--json"]));
    assert_eq!(decided["counts"]["accepted_claims"], 0);
    assert_eq!(decided["counts"]["pending_claims"], 0);

    let decided_clone = temporary.path().join("decided-clone");
    let cloned = Command::new("git")
        .args(["clone", "-q"])
        .arg(&frontier)
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

    std::fs::write(
        decided_clone.join("domain/source.json"),
        br#"{"open":[1056,1057]}"#,
    )
    .expect("mutate declared Target Index input");
    let stale_input = run(&decided_clone, None, &["replay", ".", "--json"]);
    assert!(!stale_input.status.success());
    assert!(
        String::from_utf8_lossy(&stale_input.stdout).contains("target_index_output_not_tracked"),
        "unexpected stale-input output: {}",
        String::from_utf8_lossy(&stale_input.stdout)
    );

    std::fs::write(
        decided_clone.join("domain/source.json"),
        br#"{"open":[1056]}"#,
    )
    .expect("restore declared Target Index input");
    std::fs::write(
        decided_clone.join("site/problems/1056.json"),
        br#"{"problem":1056,"schema":"changed.packet.v1"}"#,
    )
    .expect("mutate Target packet");
    let stale_packet = run(&decided_clone, None, &["replay", ".", "--json"]);
    assert!(!stale_packet.status.success());
    assert!(
        String::from_utf8_lossy(&stale_packet.stdout).contains("target_index_output_not_tracked"),
        "unexpected stale-packet output: {}",
        String::from_utf8_lossy(&stale_packet.stdout)
    );
}
