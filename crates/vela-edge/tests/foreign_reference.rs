use vela_edge::foreign_reference::{
    ForeignReferenceAssessmentV1, ForeignReferenceV1, assess_foreign_reference,
    foreign_object_set_root, foreign_reference_root, verify_foreign_reference_package,
};

fn fixture() -> ForeignReferenceV1 {
    serde_json::from_str(include_str!(
        "../../../conformance/fixtures/transfer/foreign-reference-input.v1.json"
    ))
    .expect("parse foreign-reference fixture")
}

#[test]
fn complete_foreign_reference_retains_identity_without_local_authority() {
    let reference = fixture();
    let expected: ForeignReferenceAssessmentV1 = serde_json::from_str(include_str!(
        "../../../conformance/fixtures/transfer/foreign-reference-expected.v1.json"
    ))
    .expect("parse expected assessment");

    assert_eq!(
        foreign_object_set_root(&reference.objects).unwrap(),
        reference.object_set_root
    );
    assert_eq!(
        foreign_reference_root(&reference).unwrap(),
        expected.reference_root
    );
    assert_eq!(assess_foreign_reference(&reference).unwrap(), expected);
    assert_eq!(expected.status, "complete");
    assert_eq!(expected.source_standing, "accepted");
    assert_eq!(expected.local_standing_effect, "none");
    assert!(expected.requires_local_decision);
}

#[test]
fn incomplete_reference_is_explicit_and_never_claims_complete_transfer() {
    let mut reference = fixture();
    reference
        .objects
        .retain(|object| object.role != "verification");
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();
    reference.completeness.status = "incomplete".into();
    reference.completeness.missing_roles = vec!["verification".into()];

    let assessment = assess_foreign_reference(&reference).unwrap();
    assert_eq!(assessment.status, "incomplete");
    assert_eq!(assessment.diagnostics, ["missing_role:verification"]);
    assert_eq!(assessment.local_standing_effect, "none");
}

#[test]
fn authority_escalation_or_object_substitution_fails_closed() {
    let mut escalation = fixture();
    escalation.authority.local_standing_effect = "accepted".into();
    assert_eq!(
        assess_foreign_reference(&escalation).unwrap_err(),
        "foreign_reference_authority_escalation"
    );

    let mut substitution = fixture();
    substitution.objects[0].root = format!("sha256:{}", "9".repeat(64));
    assert_eq!(
        assess_foreign_reference(&substitution).unwrap_err(),
        "foreign_reference_object_set_root_mismatch"
    );

    let mut path_escape = fixture();
    path_escape.objects[0].path = "../outside".into();
    assert_eq!(
        assess_foreign_reference(&path_escape).unwrap_err(),
        "foreign_reference_object_path_invalid"
    );
}

#[test]
fn completeness_and_exact_source_bindings_fail_closed_on_drift() {
    let mut silent_truncation = fixture();
    silent_truncation
        .objects
        .retain(|object| object.role != "verification");
    silent_truncation.object_set_root =
        foreign_object_set_root(&silent_truncation.objects).unwrap();
    assert_eq!(
        assess_foreign_reference(&silent_truncation).unwrap_err(),
        "foreign_reference_completeness_mismatch"
    );

    let mut decision_drift = fixture();
    decision_drift.source.decision_event.root = format!("sha256:{}", "9".repeat(64));
    assert_eq!(
        assess_foreign_reference(&decision_drift).unwrap_err(),
        "foreign_reference_role_binding_mismatch:decision_event"
    );
}

#[test]
fn package_verification_hashes_every_retained_object_byte() {
    use sha2::{Digest, Sha256};
    use std::fs;

    let temp = tempfile::tempdir().unwrap();
    let mut reference = fixture();
    for object in &mut reference.objects {
        let bytes = format!("exact foreign object bytes for {}", object.role);
        let path = temp.path().join(&object.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes.as_bytes()).unwrap();
        object.bytes_root = format!("sha256:{}", hex::encode(Sha256::digest(bytes.as_bytes())));
        if object.role != "authority_record" {
            object.root = object.bytes_root.clone();
        }
        match object.role.as_str() {
            "repository_manifest" => reference.source.repository_root = object.root.clone(),
            "claim" => reference.source.claim.root = object.root.clone(),
            "submission" => reference.source.submission.root = object.root.clone(),
            "proposal" => reference.source.proposal.root = object.root.clone(),
            "verification" => reference.source.verification.root = object.root.clone(),
            "decision_event" => reference.source.decision_event.root = object.root.clone(),
            "applied_event" => reference.source.applied_event.root = object.root.clone(),
            "authority_record" => {}
            _ => {}
        }
    }
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();

    assert_eq!(
        verify_foreign_reference_package(&reference, temp.path())
            .unwrap()
            .status,
        "complete"
    );
    let claim = reference
        .objects
        .iter()
        .find(|object| object.role == "claim")
        .unwrap();
    fs::write(temp.path().join(&claim.path), b"tampered").unwrap();
    assert_eq!(
        verify_foreign_reference_package(&reference, temp.path()).unwrap_err(),
        "foreign_reference_object_bytes_mismatch:claim"
    );
}

#[cfg(unix)]
#[test]
fn package_verification_rejects_symlink_escape() {
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::os::unix::fs::symlink;

    let package = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut reference = fixture();
    for object in &mut reference.objects {
        let bytes = format!("exact foreign object bytes for {}", object.role);
        let path = package.path().join(&object.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes.as_bytes()).unwrap();
        object.bytes_root = format!("sha256:{}", hex::encode(Sha256::digest(bytes.as_bytes())));
        if object.role != "authority_record" {
            object.root = object.bytes_root.clone();
        }
        match object.role.as_str() {
            "repository_manifest" => reference.source.repository_root = object.root.clone(),
            "claim" => reference.source.claim.root = object.root.clone(),
            "submission" => reference.source.submission.root = object.root.clone(),
            "proposal" => reference.source.proposal.root = object.root.clone(),
            "verification" => reference.source.verification.root = object.root.clone(),
            "decision_event" => reference.source.decision_event.root = object.root.clone(),
            "applied_event" => reference.source.applied_event.root = object.root.clone(),
            "authority_record" => {}
            _ => {}
        }
    }
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();

    let claim = reference
        .objects
        .iter()
        .find(|object| object.role == "claim")
        .unwrap();
    let escaped_bytes = fs::read(package.path().join(&claim.path)).unwrap();
    let escaped_path = outside.path().join("claim.json");
    fs::write(&escaped_path, escaped_bytes).unwrap();
    fs::remove_file(package.path().join(&claim.path)).unwrap();
    symlink(&escaped_path, package.path().join(&claim.path)).unwrap();

    assert_eq!(
        verify_foreign_reference_package(&reference, package.path()).unwrap_err(),
        "foreign_reference_object_path_escape:claim"
    );
}
