use vela_edge::foreign_reference::{
    ForeignReferenceAssessmentV1, ForeignReferenceV1, assess_foreign_reference,
    foreign_object_set_root, foreign_reference_root, verify_foreign_reference_package,
};

fn fixture() -> ForeignReferenceV1 {
    serde_json::from_str(include_str!(
        "../../../paper/artifacts/transfer/erdos-424/reference.v1.json"
    ))
    .expect("parse real foreign-reference package")
}

fn expected() -> ForeignReferenceAssessmentV1 {
    serde_json::from_str(include_str!(
        "../../../paper/artifacts/transfer/erdos-424/assessment.v1.json"
    ))
    .expect("parse expected real assessment")
}

fn make_incomplete_claim_package() -> (tempfile::TempDir, ForeignReferenceV1) {
    use sha2::{Digest, Sha256};
    use std::fs;

    let package = tempfile::tempdir().unwrap();
    let mut reference = fixture();
    reference.objects.retain(|object| object.role == "claim");
    let object = reference.objects.first_mut().unwrap();
    let bytes = b"exact incomplete foreign Claim bytes";
    let path = package.path().join(&object.path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    object.bytes_root = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
    object.root = object.bytes_root.clone();
    reference.source.claim.root = object.root.clone();
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();
    reference.completeness.status = "incomplete".into();
    reference.completeness.missing_roles = vec![
        "applied_event".into(),
        "authority_keyset".into(),
        "authority_record".into(),
        "current_repository_manifest".into(),
        "decision_event".into(),
        "proposal".into(),
        "repository_origin".into(),
        "submission".into(),
        "transition_repository_manifest".into(),
        "verification".into(),
    ];
    (package, reference)
}

fn copy_real_package(reference: &ForeignReferenceV1) -> tempfile::TempDir {
    use std::fs;

    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../paper/artifacts/transfer/erdos-424");
    let package = tempfile::tempdir().unwrap();
    for object in &reference.objects {
        let destination = package.path().join(&object.path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source.join(&object.path), destination).unwrap();
    }
    package
}

#[test]
fn complete_foreign_reference_retains_identity_without_local_authority() {
    let reference = fixture();
    let expected = expected();
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
    assert_eq!(
        expected.source_applied_semantic_event_id,
        "vev_7b5ae15a99689064"
    );
}

#[test]
fn real_compaction_aware_package_verifies_the_semantic_chain() {
    let reference = fixture();
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../paper/artifacts/transfer/erdos-424");
    let assessment = verify_foreign_reference_package(&reference, &package).unwrap();
    assert_eq!(assessment, expected());
    assert_eq!(
        assessment.source_current_repository_root,
        "sha256:8a98ff1c632232c7b227d87a0f1015aaa3429d38c83592ca66f8e465b06b0ee5"
    );
    assert_eq!(
        assessment.source_transition_repository_root,
        "sha256:391c2acb12ea1251b6614803d973fd7785826977b664bebcd7091d261133d8fc"
    );
}

#[test]
fn incomplete_reference_is_explicit_and_never_claims_complete_transfer() {
    let (_, reference) = make_incomplete_claim_package();
    let assessment = assess_foreign_reference(&reference).unwrap();
    assert_eq!(assessment.status, "incomplete");
    assert_eq!(assessment.local_standing_effect, "none");
    assert_eq!(assessment.diagnostics.len(), 10);
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
    use std::fs;

    let (package, reference) = make_incomplete_claim_package();
    assert_eq!(
        verify_foreign_reference_package(&reference, package.path())
            .unwrap()
            .status,
        "incomplete"
    );
    let claim = reference.objects.first().unwrap();
    fs::write(package.path().join(&claim.path), b"tampered").unwrap();
    assert_eq!(
        verify_foreign_reference_package(&reference, package.path()).unwrap_err(),
        "foreign_reference_object_bytes_mismatch:claim"
    );
}

#[test]
fn semantic_substitution_fails_even_when_all_declared_hashes_are_rebuilt() {
    use sha2::{Digest, Sha256};
    use std::fs;

    let mut reference = fixture();
    let package = copy_real_package(&reference);
    let object = reference
        .objects
        .iter_mut()
        .find(|object| object.role == "current_repository_manifest")
        .unwrap();
    let path = package.path().join(&object.path);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["accepted_claims"]
        .as_array_mut()
        .unwrap()
        .retain(|claim| claim["claim_id"] != reference.source.claim.id);
    let bytes = vela_protocol::canonical::to_canonical_bytes(&manifest).unwrap();
    fs::write(&path, &bytes).unwrap();
    let root = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    object.root = root.clone();
    object.bytes_root = root.clone();
    reference.source.current_repository.repository_root = root;
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();

    assert_eq!(
        verify_foreign_reference_package(&reference, package.path()).unwrap_err(),
        "foreign_reference_current_repository_mismatch"
    );
}

#[test]
fn authority_signature_tampering_fails_after_byte_roots_are_rebuilt() {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use std::fs;

    let mut reference = fixture();
    let package = copy_real_package(&reference);
    let object = reference
        .objects
        .iter_mut()
        .find(|object| object.role == "authority_record")
        .unwrap();
    let path = package.path().join(&object.path);
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    envelope["signatures"][0]["sig"] =
        serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode([0_u8; 64]));
    let bytes = vela_protocol::canonical::to_canonical_bytes(&envelope).unwrap();
    fs::write(&path, &bytes).unwrap();
    object.bytes_root = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    reference.object_set_root = foreign_object_set_root(&reference.objects).unwrap();

    assert_eq!(
        verify_foreign_reference_package(&reference, package.path()).unwrap_err(),
        "foreign_reference_authority_signature_invalid"
    );
}

#[cfg(unix)]
#[test]
fn package_verification_rejects_symlink_escape() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let (package, reference) = make_incomplete_claim_package();
    let outside = tempfile::tempdir().unwrap();
    let claim = reference.objects.first().unwrap();
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
