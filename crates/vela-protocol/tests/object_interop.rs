//! The independently emitted current objects, read by the Rust contract.
//!
//! `conformance/emitters/javascript.mjs` writes these files without importing
//! anything from this crate. Agreement on the exact roots below is the whole
//! point: two implementations independently canonicalize, sign and address the
//! same scientific content, and land on the same bytes.

use std::path::PathBuf;

use vela_protocol::submission_v2::{RequestedChange, SubmissionRecordV2};
use vela_protocol::verification_record_v2::VerificationRecordEnvelopeV2;

const SUBMISSION_ROOT: &str =
    "sha256:8779dcb8999d6030c234a14fe3af0e3745b84e513c9791913c128d0750c86830";
const VERIFICATION_ROOT: &str =
    "sha256:e03b1f71c12d79489025dd846aa92a60673a8f7fcf6703935c838d1681b14ba8";

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/current-objects")
        .join(name);
    std::fs::read(root).expect("read current-object fixture")
}

#[test]
fn independent_javascript_submission_matches_rust_contract() {
    let record = SubmissionRecordV2::parse(&fixture("submission.json"))
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
            SubmissionRecordV2::parse(&bytes).err()
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
