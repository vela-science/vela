use std::path::PathBuf;

use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/current-objects")
        .join(name);
    std::fs::read(root).expect("read current-object fixture")
}

#[test]
fn independent_javascript_submission_matches_rust_contract() {
    let submission = SubmissionV1::parse(&fixture("submission.json"))
        .expect("JavaScript Submission must satisfy the Rust parser");
    assert_eq!(submission.submission_id, "vsb_8a36cdb336823499");
    assert_eq!(
        submission.canonical_root().unwrap(),
        "sha256:81fd3abb891383dfc985d213b01b779f2edd51e6da23a68d3f85e0c8f6c41b82"
    );
    assert_eq!(
        submission.authentication.identity_binding.binding_id,
        "vib_2e85aeb82ac75615"
    );
}

#[test]
fn independent_javascript_verification_matches_rust_contract() {
    let record = VerificationRecordV1::parse(&fixture("verification.json"))
        .expect("JavaScript Verification Record must satisfy the Rust parser");
    assert_eq!(record.verification_record_id, "vvr_5565bbc76e7b40ae");
    assert_eq!(record.subject.submission_id, "vsb_8a36cdb336823499");
    assert_eq!(
        record.subject.submission_root,
        "sha256:81fd3abb891383dfc985d213b01b779f2edd51e6da23a68d3f85e0c8f6c41b82"
    );
    assert_eq!(
        record.authentication.identity_binding.binding_id,
        "vib_ddd94f07e1afcd52"
    );
    assert_eq!(
        record.canonical_root().unwrap(),
        "sha256:bc7c4231b91a747a0a28cacf96451ca3904cd3265f77a574ffb1fb962948467f"
    );
}

#[test]
fn signed_current_objects_fail_closed_after_subject_drift() {
    let mut submission: serde_json::Value =
        serde_json::from_slice(&fixture("submission.json")).unwrap();
    submission["claim"]["assertion"] = serde_json::json!("drifted assertion");
    assert!(SubmissionV1::parse(&serde_json::to_vec(&submission).unwrap()).is_err());

    let mut verification: serde_json::Value =
        serde_json::from_slice(&fixture("verification.json")).unwrap();
    verification["subject"]["submission_root"] = serde_json::json!(
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    assert!(VerificationRecordV1::parse(&serde_json::to_vec(&verification).unwrap()).is_err());
}
