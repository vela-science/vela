#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use vela_protocol::submission_v1::{
    RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    SubmissionV1,
};

struct Agent {
    child: Child,
    socket: std::path::PathBuf,
    _directory: TempDir,
}

#[derive(Default)]
struct LocalTrustCleanup(Option<std::path::PathBuf>);

impl Drop for LocalTrustCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(&path);
            if let Some(directory) = path.parent() {
                let _ = fs::remove_dir(directory);
            }
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_agent() -> Agent {
    let directory = tempfile::Builder::new()
        .prefix("vela-authority-init-agent-")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = directory.path().join("agent.sock");
    let child = Command::new("ssh-agent")
        .arg("-D")
        .arg("-a")
        .arg(&socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("OpenSSH ssh-agent is required for this focused qualification");
    let mut agent = Agent {
        child,
        socket,
        _directory: directory,
    };
    for _ in 0..100 {
        if agent.socket.exists() {
            return agent;
        }
        if let Some(status) = agent.child.try_wait().unwrap() {
            panic!("ssh-agent exited before creating its socket: {status}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("ssh-agent did not create its socket");
}

fn load_identity(agent: &Agent, root: &std::path::Path) {
    let private_key = root.join("repository-authority");
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&private_key)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("ssh-add")
            .arg(&private_key)
            .env("SSH_AUTH_SOCK", &agent.socket)
            .env("SSH_ASKPASS_REQUIRE", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

fn vela() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vela"))
}

fn git(frontier: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(frontier)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn fresh_frontier_initializes_standard_repository_authority_and_replays_strictly() {
    let temporary = tempfile::tempdir().unwrap();
    let frontier = temporary.path().join("frontier");
    let agent = start_agent();
    load_identity(&agent, temporary.path());

    let initialized = vela()
        .args(["init"])
        .arg(&frontier)
        .args([
            "--name",
            "Fresh authority fixture",
            "--scope",
            "Prove one bounded fixture result.",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let authority = vela()
        .args(["authority", "init"])
        .arg(&frontier)
        .args([
            "--reason",
            "Establish the fresh repository writer.",
            "--json",
        ])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    assert!(
        authority.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&authority.stdout),
        String::from_utf8_lossy(&authority.stderr)
    );
    let result: Value = serde_json::from_slice(&authority.stdout).unwrap();
    assert_eq!(result["schema"], "vela.authority-initialization-result.v1");
    assert_eq!(result["ok"], true);
    assert!(
        result["repository_key_id"]
            .as_str()
            .unwrap()
            .starts_with("ssh-ed25519:SHA256:")
    );
    let sequence_one_root = result["authority_record_root"].as_str().unwrap();
    assert_eq!(
        result["consumer_pin"]["first_authority_record_root"],
        sequence_one_root
    );
    let mut trust_cleanup = LocalTrustCleanup::default();
    let wrong_pin = vela()
        .args(["authority", "trust", "pin"])
        .arg(&frontier)
        .args([
            "--record-root",
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!wrong_pin.status.success());

    let authority_root = frontier.join(".vela/authority");
    assert_eq!(
        fs::read_dir(authority_root.join("events")).unwrap().count(),
        1
    );
    assert_eq!(
        fs::read_dir(authority_root.join("records"))
            .unwrap()
            .count(),
        1
    );

    let unpinned_strict = vela()
        .args(["check"])
        .arg(&frontier)
        .args(["--strict", "--json"])
        .output()
        .unwrap();
    assert!(
        !unpinned_strict.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&unpinned_strict.stdout),
        String::from_utf8_lossy(&unpinned_strict.stderr)
    );
    let unpinned_strict_text = format!(
        "{}{}",
        String::from_utf8_lossy(&unpinned_strict.stdout),
        String::from_utf8_lossy(&unpinned_strict.stderr)
    );
    assert!(
        unpinned_strict_text.contains("independent sequence-1 pin"),
        "{unpinned_strict_text}"
    );

    git(&frontier, &["config", "user.name", "Vela Test"]);
    git(&frontier, &["config", "user.email", "vela@example.invalid"]);
    fs::create_dir_all(frontier.join("artifacts")).unwrap();
    let artifact_bytes = br#"{"bounded_result":"fixture"}"#;
    fs::write(frontier.join("artifacts/result.json"), artifact_bytes).unwrap();
    let producer_key = SigningKey::from_bytes(&[0x42; 32]);
    let producer = "agent:fresh-authority";
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: producer.into(),
            actor_class: ActorClass::Agent,
            created_at: "2026-07-27T00:00:00Z".into(),
        },
        &producer_key,
    )
    .unwrap();
    let submission = SubmissionV1::build(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion: "The bounded authority fixture produced its declared artifact.".into(),
                claim_type: "computational".into(),
                conditions: vec!["Only the retained fixture bytes are in scope.".into()],
            },
            artifacts: vec![SubmissionArtifact {
                kind: "fixture-result".into(),
                path: "artifacts/result.json".into(),
                digest: format!("sha256:{}", hex::encode(Sha256::digest(artifact_bytes))),
            }],
            caveats: vec!["This is a lifecycle fixture, not a scientific finding.".into()],
            replayability: "exact".into(),
            producer_checks: Vec::new(),
            verification_requirements: vec![
                "Import one independent Verification Record before acceptance.".into(),
            ],
            requested_change: RequestedChange {
                kind: "add_claim".into(),
            },
            provenance: SubmissionProvenance {
                producer: producer.into(),
                source_system: "authority-initialization-fixture".into(),
                source_attempt: None,
                source_run: None,
                emitted_at: "2026-07-27T00:00:00Z".into(),
            },
            execution_binding: None,
        },
        identity,
        &producer_key,
    )
    .unwrap();
    let submission_path = temporary.path().join("submission.json");
    fs::write(&submission_path, submission.canonical_bytes().unwrap()).unwrap();
    git(&frontier, &["add", "-A"]);
    git(&frontier, &["commit", "-qm", "initialize authority"]);
    let proposal_count_before = fs::read_dir(frontier.join(".vela/proposals"))
        .unwrap()
        .count();
    let unpinned_submit = vela()
        .arg("submit")
        .arg(&submission_path)
        .arg("--frontier")
        .arg(&frontier)
        .args(["--as", producer, "--json"])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    assert!(
        !unpinned_submit.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&unpinned_submit.stdout),
        String::from_utf8_lossy(&unpinned_submit.stderr)
    );
    let unpinned_submit_text = format!(
        "{}{}",
        String::from_utf8_lossy(&unpinned_submit.stdout),
        String::from_utf8_lossy(&unpinned_submit.stderr)
    );
    assert!(
        unpinned_submit_text.contains("independent sequence-1 pin"),
        "{unpinned_submit_text}"
    );
    assert_eq!(
        fs::read_dir(frontier.join(".vela/proposals"))
            .unwrap()
            .count(),
        proposal_count_before
    );

    let pinned = vela()
        .args(["authority", "trust", "pin"])
        .arg(&frontier)
        .args(["--record-root", sequence_one_root, "--json"])
        .output()
        .unwrap();
    assert!(
        pinned.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&pinned.stdout),
        String::from_utf8_lossy(&pinned.stderr)
    );
    let pinned: Value = serde_json::from_slice(&pinned.stdout).unwrap();
    assert_eq!(pinned["schema"], "vela.authority-trust-pin-result.v1");
    assert_eq!(pinned["first_authority_record_root"], sequence_one_root);
    assert_eq!(pinned["authority_granted"], false);
    assert_eq!(pinned["frontier_writes"].as_array().unwrap().len(), 0);
    let trust_path =
        std::path::PathBuf::from(pinned["authority_trust_anchor_path"].as_str().unwrap());
    assert!(trust_path.is_file());
    trust_cleanup.0 = Some(trust_path);

    let submitted = vela()
        .arg("submit")
        .arg(&submission_path)
        .arg("--frontier")
        .arg(&frontier)
        .args(["--as", producer, "--json"])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    assert!(
        submitted.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&submitted.stdout),
        String::from_utf8_lossy(&submitted.stderr)
    );
    let submitted: Value = serde_json::from_slice(&submitted.stdout).unwrap();
    assert_eq!(submitted["schema"], "vela.submit-result.v1");
    assert_eq!(submitted["route"], "pending_review");
    assert_eq!(submitted["accepted_state_changed"], false);
    let proposal_id = submitted["proposal_id"].as_str().unwrap();

    let strict = vela()
        .args(["check"])
        .arg(&frontier)
        .args(["--strict", "--json"])
        .output()
        .unwrap();
    assert!(
        strict.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&strict.stdout),
        String::from_utf8_lossy(&strict.stderr)
    );

    let rejected = vela()
        .args(["review", "reject"])
        .arg(&frontier)
        .arg(proposal_id)
        .args([
            "--reason",
            "The fixture proves lifecycle integrity only; no scientific Claim should be accepted.",
            "--json",
        ])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    assert!(
        rejected.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&rejected.stdout),
        String::from_utf8_lossy(&rejected.stderr)
    );
    let rejected: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(rejected["command"], "review.reject");
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);

    let event_path = fs::read_dir(authority_root.join("events"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            serde_json::from_slice::<Value>(&fs::read(path).unwrap())
                .is_ok_and(|event| event["content"]["kind"] == "authority.initialized")
        })
        .expect("authority initialization event");
    let event_bytes = fs::read(&event_path).unwrap();
    let mut event: Value = serde_json::from_slice(&event_bytes).unwrap();
    event["content"]["reason"] = Value::String("tampered".into());
    fs::write(&event_path, serde_json::to_vec(&event).unwrap()).unwrap();
    let tampered = vela()
        .args(["check"])
        .arg(&frontier)
        .args(["--strict", "--json"])
        .output()
        .unwrap();
    assert!(!tampered.status.success());
    fs::write(&event_path, event_bytes).unwrap();

    let repeated = vela()
        .args(["authority", "init"])
        .arg(&frontier)
        .args(["--reason", "Must not replace authority.", "--json"])
        .env("SSH_AUTH_SOCK", &agent.socket)
        .output()
        .unwrap();
    assert!(!repeated.status.success());
    let repeated: Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert!(
        repeated["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already initialized")
    );
}
