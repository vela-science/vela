//! Focused current Submission authoring regressions.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use vela_protocol::canonical::sha256_root;
use vela_protocol::dsse::{EnvelopeV1, SignatureV1, encode_base64};
use vela_protocol::signer_identity::{ActorClass, SignerIdentityV1};
use vela_protocol::submission::{
    RequestedChange, SUBMISSION_V3_PAYLOAD_TYPE, SubmissionArtifact, SubmissionClaim,
    SubmissionDraft, SubmissionProvenance, SubmissionRecordV3,
};

fn run(home: &Path, repository_path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .arg("submit")
        .args(args)
        .args([
            "--claim",
            "Exact bounded fixture.",
            "--type",
            "theoretical",
            "--replayability",
            "exact",
            "--artifact",
            "missing.json:source-diff",
            "--caveat",
            "Exact fixture only.",
            "--as",
            "agent:fixture",
            "--repo",
            repository_path.to_str().expect("utf-8 repository"),
            "--json",
        ])
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("SSH_AUTH_SOCK")
        .output()
        .expect("run vela submit")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn imported(home: &Path, repository_path: &Path, envelope: &EnvelopeV1) -> serde_json::Value {
    let path = home.join("submission.json");
    std::fs::write(
        &path,
        vela_protocol::canonical::to_canonical_bytes(envelope).unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args([
            "submit",
            path.to_str().unwrap(),
            "--repo",
            repository_path.to_str().unwrap(),
            "--json",
        ])
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn minimal_envelope(payload_type: &str, signature: Vec<u8>) -> EnvelopeV1 {
    let key = SigningKey::generate(&mut OsRng);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    let payload = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "identity": {"public_key_hex": public_key}
    }))
    .unwrap();
    EnvelopeV1 {
        payload_type: payload_type.into(),
        payload: encode_base64(&payload),
        signatures: vec![SignatureV1 {
            keyid: public_key,
            sig: encode_base64(&signature),
        }],
    }
}

fn valid_envelope_with_schema(schema: &str) -> EnvelopeV1 {
    let key = SigningKey::from_bytes(&[23; 32]);
    let actor = "agent:schema-code-fixture";
    let emitted_at = "2026-08-17T12:00:00Z".to_string();
    let identity = SignerIdentityV1::new(actor, ActorClass::Agent, &key, emitted_at.clone())
        .expect("fixture identity");
    let record = SubmissionRecordV3::seal(
        SubmissionDraft {
            claim: SubmissionClaim {
                assertion: "The fixture payload has one exact schema tag.".into(),
                claim_type: "theoretical".into(),
                conditions: vec!["CLI error-code fixture only.".into()],
            },
            artifacts: vec![SubmissionArtifact {
                kind: "fixture".into(),
                path: "fixture.json".into(),
                digest: sha256_root(b"fixture"),
            }],
            caveats: vec!["This establishes no scientific result.".into()],
            replayability: "exact".into(),
            producer_checks: Vec::new(),
            verification_requirements: vec!["Inspect the exact schema tag.".into()],
            requested_change: RequestedChange {
                kind: "add_claim".into(),
                target: None,
            },
            provenance: SubmissionProvenance {
                producer: actor.into(),
                source_system: "vela-cli-test".into(),
                source_run: None,
                emitted_at,
            },
        },
        identity,
        &key,
    )
    .expect("current fixture Submission");
    let mut payload = record.submission;
    payload.schema = schema.into();
    let bytes = vela_protocol::canonical::to_canonical_bytes(&payload).unwrap();
    EnvelopeV1::seal_single(&key, SUBMISSION_V3_PAYLOAD_TYPE, &bytes)
}

#[test]
fn new_claim_authoring_does_not_require_a_source_run() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(directory.path(), directory.path(), &[]);
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --source-run"));
    assert!(message.contains("artifact 0"));
}

#[test]
fn exact_supersession_authoring_does_not_require_a_source_run() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(
        directory.path(),
        directory.path(),
        &[
            "--supersedes",
            &format!("vcl_{}", "a".repeat(64)),
            "--target-root",
            &format!("sha256:{}", "b".repeat(64)),
        ],
    );
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --source-run"));
    assert!(message.contains("artifact 0"));
}

#[test]
fn retired_media_type_and_wrong_signer_have_stable_recovery_codes() {
    let directory = tempfile::tempdir().unwrap();
    let retired = imported(
        directory.path(),
        directory.path(),
        &minimal_envelope("application/vnd.vela.submission.v2+json", vec![0; 64]),
    );
    assert_eq!(
        retired["error"]["code"],
        "submission_media_type_unsupported"
    );
    assert!(
        retired["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("submission.v2+json"))
    );
    assert!(
        retired["next"]
            .as_str()
            .is_some_and(|next| next.contains("signed historical release"))
    );

    let predecessor_schema = imported(
        directory.path(),
        directory.path(),
        &valid_envelope_with_schema("vela.submission.v2"),
    );
    assert_eq!(
        predecessor_schema["error"]["code"],
        "submission_schema_unsupported"
    );
    assert!(
        predecessor_schema["next"]
            .as_str()
            .is_some_and(|next| next.contains("vela.submission.v3"))
    );

    let unsigned = imported(
        directory.path(),
        directory.path(),
        &minimal_envelope("application/vnd.vela.submission.v3+json", vec![0; 64]),
    );
    assert_eq!(unsigned["error"]["code"], "submission_signature_invalid");
    assert!(
        unsigned["next"]
            .as_str()
            .is_some_and(|next| next.contains("--as"))
    );
}
