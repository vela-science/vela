//! The independently emitted current objects, read by the Rust contract.
//!
//! `conformance/emitters/javascript.mjs` writes these files without importing
//! anything from this crate. Agreement on the exact roots below is the whole
//! point: two implementations independently canonicalize, sign and address the
//! same scientific content, and land on the same bytes.

use std::path::PathBuf;

use vela_protocol::submission::{RequestedChange, SubmissionRecordV3};
use vela_protocol::verification_record::VerificationRecordEnvelopeV2;

const SUBMISSION_ROOT: &str =
    "sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a";
const VERIFICATION_ROOT: &str =
    "sha256:41cebab0fc7408b59d1ab95b6037da76cdba555632e665b68688668f1da80d8d";

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/current-objects")
        .join(name);
    std::fs::read(root).expect("read current-object fixture")
}

#[test]
fn independent_javascript_submission_matches_rust_contract() {
    let record = SubmissionRecordV3::parse(&fixture("submission.json"))
        .expect("JavaScript Submission must satisfy the Rust parser");
    assert_eq!(record.root, SUBMISSION_ROOT);
    assert_eq!(
        record.id,
        vela_protocol::derive_handle("vsb_", SUBMISSION_ROOT).unwrap()
    );
    assert_eq!(record.submission.identity.actor_id, "agent:independent-js");
}

#[test]
fn independent_javascript_verification_matches_rust_contract() {
    let sealed = VerificationRecordEnvelopeV2::parse(&fixture("verification.json"))
        .expect("JavaScript Verification Record must satisfy the Rust parser");
    assert_eq!(sealed.root, VERIFICATION_ROOT);
    assert_eq!(
        sealed.id,
        vela_protocol::derive_handle("vvr_", VERIFICATION_ROOT).unwrap()
    );
    assert_eq!(sealed.record.subject.submission_root, SUBMISSION_ROOT);
    assert_eq!(
        sealed.record.subject.submission_id,
        vela_protocol::derive_handle("vsb_", SUBMISSION_ROOT).unwrap()
    );
    assert_eq!(sealed.record.verifier(), "verifier:independent-js");
}

/// Editing the payload of a signed envelope breaks it, in either direction.
#[test]
fn signed_current_objects_fail_closed_after_subject_drift() {
    for (name, pointer, replacement) in [
        (
            "submission.json",
            ["claim", "assertion"].as_slice(),
            "drifted assertion",
        ),
        (
            "verification.json",
            ["subject", "submission_root"].as_slice(),
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ),
    ] {
        let mut envelope: serde_json::Value = serde_json::from_slice(&fixture(name)).unwrap();
        let encoded = envelope["payload"].as_str().unwrap();
        let mut payload: serde_json::Value = serde_json::from_slice(
            &vela_protocol::dsse::decode_base64("payload", encoded).unwrap(),
        )
        .unwrap();
        let mut cursor = &mut payload;
        for step in &pointer[..pointer.len() - 1] {
            cursor = &mut cursor[*step];
        }
        cursor[pointer[pointer.len() - 1]] = serde_json::json!(replacement);
        envelope["payload"] = serde_json::json!(vela_protocol::dsse::encode_base64(
            &serde_json::to_vec(&payload).unwrap()
        ));

        let bytes = serde_json::to_vec(&envelope).unwrap();
        let parsed = if name == "submission.json" {
            SubmissionRecordV3::parse(&bytes).err()
        } else {
            VerificationRecordEnvelopeV2::parse(&bytes).err()
        };
        assert!(parsed.is_some(), "{name} accepted a payload nobody signed");
    }
}

#[test]
fn requested_change_vocabulary_matches_the_shared_typescript_matrix() {
    let cases: serde_json::Value =
        serde_json::from_slice(&fixture("requested-change-cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let parsed = serde_json::from_value::<RequestedChange>(case["value"].clone())
            .map_err(|error| error.to_string())
            .and_then(|value| value.validate());
        assert_eq!(parsed.is_ok(), case["valid"].as_bool().unwrap(), "{name}");
    }
}
