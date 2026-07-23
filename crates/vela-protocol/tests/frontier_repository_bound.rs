use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use vela_protocol::events::{
    EVENT_KIND_FRONTIER_REPOSITORY_BOUND, EventKind, StateEvent, compute_event_id, replay_report,
    validate_event_payload,
};
use vela_protocol::frontier_repository::{
    ExactFrontierDependencyV1, FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierIdentityV1,
    FrontierRepositoryBoundaryMode, FrontierRepositoryBoundaryPayloadV1,
    FrontierRepositoryTrustMode, GitObjectFormat, LEGACY_FRONTIER_ORIGIN_SCHEMA,
    LegacyFrontierOriginV1, RETAINED_OBJECT_MANIFEST_SCHEMA, RetainedObjectEntryV1,
    RetainedObjectManifestV1, exact_dependency_root, new_repository_boundary_event,
    repository_boundary_event_content_root, repository_boundary_payload_from_event_shape,
    repository_identity_event_content_root, validate_repository_boundary_event_set,
    verify_repository_boundary_signature_only,
};
use vela_protocol::project;
use vela_protocol::reducer::verify_replay;
use vela_protocol::sign::{pubkey_hex, sign_event};

fn root(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn dependency(frontier_hex: &str, identity: char) -> ExactFrontierDependencyV1 {
    ExactFrontierDependencyV1 {
        frontier_id: format!("vfr_{frontier_hex}"),
        identity_root: root(identity),
        scientific_state_root: root('9'),
        git_object_format: GitObjectFormat::Sha1,
        git_commit: "a".repeat(40),
        git_tree: "b".repeat(40),
    }
}

fn temporal_payload(public_key: &str) -> FrontierRepositoryBoundaryPayloadV1 {
    let frontier_id = "vfr_1234567890abcdef".to_string();
    let legacy_identity_preimage_root = root('1');
    let origin = LegacyFrontierOriginV1 {
        schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
        frontier_id: frontier_id.clone(),
        legacy_identity_preimage_root: legacy_identity_preimage_root.clone(),
        git_object_format: GitObjectFormat::Sha1,
        anchor_git_commit: "2".repeat(40),
        anchor_git_tree: "3".repeat(40),
        anchor_event_log_root: root('4'),
        anchor_event_count: 11,
    };
    FrontierRepositoryBoundaryPayloadV1 {
        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
        mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
        frontier_id,
        identity_root: origin.identity_root().unwrap(),
        observed_profile_root: root('5'),
        dependency_root: exact_dependency_root(&[]).unwrap(),
        dependencies: vec![],
        previous_identity_event_root: None,
        legacy_identity_preimage_root: Some(legacy_identity_preimage_root),
        administrator_actor_id: "reviewer:fixture".to_string(),
        administrator_public_key: public_key.to_string(),
        administrator_algorithm: "ed25519".to_string(),
        trust_mode: FrontierRepositoryTrustMode::Tofu,
        git_object_format: GitObjectFormat::Sha1,
        anchor_git_commit: "2".repeat(40),
        anchor_git_tree: "3".repeat(40),
        anchor_event_log_root: root('4'),
        anchor_event_count: 11,
        anchor_snapshot_root: root('6'),
        anchor_snapshot_schema: "vela.project.v0.1".to_string(),
        anchor_proposal_root: root('7'),
        anchor_actor_registry_root: root('8'),
        anchor_artifact_registry_root: root('a'),
        anchor_canonical_store_root: root('b'),
    }
}

fn signed_temporal_event() -> (vela_protocol::events::StateEvent, SigningKey) {
    let key = SigningKey::generate(&mut OsRng);
    let payload = temporal_payload(&pubkey_hex(&key));
    let mut event = new_repository_boundary_event(
        payload,
        "Bind the exact legacy repository history.",
        "2026-07-22T12:00:00Z",
    )
    .unwrap();
    event.signature = Some(sign_event(&event, &key).unwrap());
    (event, key)
}

#[test]
fn frontier_repository_boundary_fixed_wire_vector() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let mut event = new_repository_boundary_event(
        temporal_payload(&pubkey_hex(&key)),
        "Bind the exact legacy repository history.",
        "2026-07-22T12:00:00Z",
    )
    .unwrap();
    event.signature = Some(sign_event(&event, &key).unwrap());
    assert_eq!(event.id, "vev_daa40248a7cd5a84");
    assert_eq!(
        repository_boundary_event_content_root(&event).unwrap(),
        "sha256:daa40248a7cd5a84aa34393699d35352f4387d499d4a3152a0c703f28ba071ff"
    );
    assert_eq!(
        event.signature.as_deref(),
        Some(
            "v1:5a21201882578c55a34ddac85954e2d2b9e620ecc7fd38982f07782c87f45fc23e9b691c47881bf80abc4077a4d297a32642482e5797fbd8dcb54cf41254070a"
        )
    );
    verify_repository_boundary_signature_only(&event, &pubkey_hex(&key)).unwrap();
}

fn resign(event: &mut vela_protocol::events::StateEvent, key: &SigningKey) {
    event.id = compute_event_id(event);
    event.signature = Some(sign_event(event, key).unwrap());
}

fn genesis_project() -> vela_protocol::project::Project {
    project::assemble_profile_v1(
        "repository-boundary-fixture",
        vec![],
        0,
        0,
        "Repository boundary fixture.",
    )
}

fn historical_genesis_event() -> StateEvent {
    let mut event = genesis_project().events[0].clone();
    event.payload = serde_json::json!({
        "name": event.target.id,
        "creator": event.actor.id,
        "schema_version": "0.1",
        "compiled_at": event.timestamp,
    });
    event.id = compute_event_id(&event);
    event
}

fn genesis_update_payload(
    genesis: &StateEvent,
    public_key: &str,
) -> FrontierRepositoryBoundaryPayloadV1 {
    let identity = FrontierIdentityV1::from_genesis_event(genesis).unwrap();
    FrontierRepositoryBoundaryPayloadV1 {
        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
        mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
        frontier_id: identity.frontier_id.clone(),
        identity_root: identity.root().unwrap(),
        observed_profile_root: root('5'),
        dependency_root: exact_dependency_root(&[]).unwrap(),
        dependencies: vec![],
        previous_identity_event_root: Some(
            repository_identity_event_content_root(genesis).unwrap(),
        ),
        legacy_identity_preimage_root: None,
        administrator_actor_id: "reviewer:fixture".to_string(),
        administrator_public_key: public_key.to_string(),
        administrator_algorithm: "ed25519".to_string(),
        trust_mode: FrontierRepositoryTrustMode::Genesis,
        git_object_format: GitObjectFormat::Sha1,
        anchor_git_commit: "2".repeat(40),
        anchor_git_tree: "3".repeat(40),
        anchor_event_log_root: root('4'),
        anchor_event_count: 1,
        anchor_snapshot_root: root('6'),
        anchor_snapshot_schema: "vela.project.v0.1".to_string(),
        anchor_proposal_root: root('7'),
        anchor_actor_registry_root: root('8'),
        anchor_artifact_registry_root: root('a'),
        anchor_canonical_store_root: root('b'),
    }
}

fn signed_genesis_update(genesis: &StateEvent) -> (StateEvent, SigningKey) {
    let key = SigningKey::generate(&mut OsRng);
    let payload = genesis_update_payload(genesis, &pubkey_hex(&key));
    let mut event = new_repository_boundary_event(
        payload,
        "Bind the first exact dependency state.",
        "2099-07-22T12:00:00Z",
    )
    .unwrap();
    event.signature = Some(sign_event(&event, &key).unwrap());
    (event, key)
}

#[test]
fn dependency_bytes_rederive_dependency_root() {
    let first = dependency("1111111111111111", '1');
    let second = dependency("2222222222222222", '2');
    let canonical = vec![first.clone(), second.clone()];
    let expected = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&canonical).unwrap()
    );
    assert_eq!(exact_dependency_root(&canonical).unwrap(), expected);

    assert!(exact_dependency_root(&[second, first.clone()]).is_err());
    assert!(exact_dependency_root(&[first.clone(), first]).is_err());

    let mut short = dependency("3333333333333333", '3');
    short.scientific_state_root = "sha256:abc".to_string();
    assert!(exact_dependency_root(&[short]).is_err());

    let mut sha256_git = dependency("4444444444444444", '4');
    sha256_git.git_object_format = GitObjectFormat::Sha256;
    sha256_git.git_commit = "c".repeat(64);
    sha256_git.git_tree = "d".repeat(64);
    assert!(exact_dependency_root(&[sha256_git]).is_ok());
}

#[test]
fn frontier_repository_bound_retained_manifest_is_portably_closed() {
    assert_eq!(
        RETAINED_OBJECT_MANIFEST_SCHEMA,
        "vela.retained-object-manifest.v1"
    );
    let manifest = RetainedObjectManifestV1(vec![
        RetainedObjectEntryV1 {
            path: ".vela/events/vev_a.json".to_string(),
            git_mode: "100644".to_string(),
            size: 12,
            sha256: "1".repeat(64),
        },
        RetainedObjectEntryV1 {
            path: "records/receipt.json".to_string(),
            git_mode: "100755".to_string(),
            size: 20,
            sha256: "2".repeat(64),
        },
    ]);
    let manifest_root = manifest.root().unwrap();
    manifest.verify_root(&manifest_root).unwrap();
    assert!(manifest.verify_root(&root('f')).is_err());

    let collision = RetainedObjectManifestV1(vec![
        RetainedObjectEntryV1 {
            path: "A/file".to_string(),
            git_mode: "100644".to_string(),
            size: 1,
            sha256: "3".repeat(64),
        },
        RetainedObjectEntryV1 {
            path: "a/file".to_string(),
            git_mode: "100644".to_string(),
            size: 1,
            sha256: "4".repeat(64),
        },
    ]);
    assert!(collision.validate().is_err());

    for invalid_path in ["../escape", "/absolute", "a//b", "a\\b", "e\u{301}/file"] {
        let invalid = RetainedObjectManifestV1(vec![RetainedObjectEntryV1 {
            path: invalid_path.to_string(),
            git_mode: "100644".to_string(),
            size: 1,
            sha256: "5".repeat(64),
        }]);
        assert!(
            invalid.validate().is_err(),
            "accepted path {invalid_path:?}"
        );
    }

    let symlink = RetainedObjectManifestV1(vec![RetainedObjectEntryV1 {
        path: "link".to_string(),
        git_mode: "120000".to_string(),
        size: 1,
        sha256: "6".repeat(64),
    }]);
    assert!(symlink.validate().is_err());
}

#[test]
fn frontier_repository_bound_recomputes_identity_and_dependency_roots() {
    let (event, key) = signed_temporal_event();
    assert_eq!(event.kind, EVENT_KIND_FRONTIER_REPOSITORY_BOUND);
    assert_eq!(
        EventKind::from(EVENT_KIND_FRONTIER_REPOSITORY_BOUND),
        EventKind::FrontierRepositoryBound
    );
    let payload = verify_repository_boundary_signature_only(&event, &pubkey_hex(&key)).unwrap();
    validate_event_payload(event.kind.as_str(), &event.payload).unwrap();
    assert_eq!(payload.dependencies, Vec::new());

    let mut open_payload = event.payload.clone();
    open_payload["unregistered_field"] = serde_json::json!(true);
    assert!(validate_event_payload(event.kind.as_str(), &open_payload).is_err());

    let mut wrong_dependency = payload.clone();
    wrong_dependency.dependency_root = root('c');
    assert!(wrong_dependency.validate().is_err());

    let mut wrong_identity = payload;
    wrong_identity.identity_root = root('d');
    assert!(wrong_identity.validate().is_err());
}

#[test]
fn frontier_repository_bound_event_shape_binds_fixed_core() {
    let (event, key) = signed_temporal_event();

    let mut wrong_target = event.clone();
    wrong_target.target.id = "vfr_aaaaaaaaaaaaaaaa".to_string();
    resign(&mut wrong_target, &key);
    assert!(repository_boundary_payload_from_event_shape(&wrong_target).is_err());

    let mut wrong_actor = event.clone();
    wrong_actor.actor.id = "reviewer:other".to_string();
    resign(&mut wrong_actor, &key);
    assert!(repository_boundary_payload_from_event_shape(&wrong_actor).is_err());

    let mut wrong_hash = event.clone();
    wrong_hash.before_hash = root('e');
    resign(&mut wrong_hash, &key);
    assert!(repository_boundary_payload_from_event_shape(&wrong_hash).is_err());

    let mut wrong_id = event.clone();
    wrong_id.id = "vev_0000000000000000".to_string();
    assert!(repository_boundary_payload_from_event_shape(&wrong_id).is_err());
}

#[test]
fn frontier_repository_bound_signature_only_rejects_wrong_signer_and_key() {
    let (mut event, key) = signed_temporal_event();
    let other = SigningKey::generate(&mut OsRng);
    event.signature = Some(sign_event(&event, &other).unwrap());
    assert!(verify_repository_boundary_signature_only(&event, &pubkey_hex(&key)).is_err());

    let (event, _) = signed_temporal_event();
    assert!(verify_repository_boundary_signature_only(&event, &pubkey_hex(&other)).is_err());
}

#[test]
fn frontier_repository_bound_rejects_identity_change_and_chain_break() {
    let (previous_event, key) = signed_temporal_event();
    let previous = repository_boundary_payload_from_event_shape(&previous_event).unwrap();
    let added = dependency("aaaaaaaaaaaaaaaa", 'a');
    let mut update = previous.clone();
    update.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
    update.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
    update.previous_identity_event_root =
        Some(repository_boundary_event_content_root(&previous_event).unwrap());
    update.dependencies = vec![added];
    update.dependency_root = exact_dependency_root(&update.dependencies).unwrap();
    update.observed_profile_root = root('e');
    update.anchor_git_commit = "c".repeat(40);
    update.anchor_git_tree = "d".repeat(40);
    update.anchor_event_log_root = root('f');
    update.anchor_event_count += 1;
    update.validate_chain(&previous_event).unwrap();

    let mut event = new_repository_boundary_event(
        update.clone(),
        "Update exact dependency pins.",
        "2026-07-22T12:01:00Z",
    )
    .unwrap();
    event.signature = Some(sign_event(&event, &key).unwrap());
    verify_repository_boundary_signature_only(&event, &pubkey_hex(&key)).unwrap();

    for mutate in [
        |payload: &mut FrontierRepositoryBoundaryPayloadV1| payload.identity_root = root('0'),
        |payload: &mut FrontierRepositoryBoundaryPayloadV1| {
            payload.administrator_actor_id = "reviewer:other".to_string()
        },
        |payload: &mut FrontierRepositoryBoundaryPayloadV1| {
            payload.legacy_identity_preimage_root = Some(root('0'))
        },
        |payload: &mut FrontierRepositoryBoundaryPayloadV1| {
            payload.frontier_id = "vfr_0000000000000000".to_string()
        },
        |payload: &mut FrontierRepositoryBoundaryPayloadV1| {
            payload.administrator_public_key = "0".repeat(64)
        },
    ] {
        let mut changed = update.clone();
        mutate(&mut changed);
        assert!(changed.validate_chain(&previous_event).is_err());
    }

    let mut wrong_previous = update;
    wrong_previous.previous_identity_event_root = Some(root('0'));
    assert!(wrong_previous.validate_chain(&previous_event).is_err());
}

#[test]
fn frontier_repository_bound_mode_and_trust_rules_fail_closed() {
    let key = SigningKey::generate(&mut OsRng);
    let mut payload = temporal_payload(&pubkey_hex(&key));

    payload.legacy_identity_preimage_root = None;
    assert!(payload.validate().is_err());

    let mut payload = temporal_payload(&pubkey_hex(&key));
    payload.previous_identity_event_root = Some(root('1'));
    assert!(payload.validate().is_err());

    let mut payload = temporal_payload(&pubkey_hex(&key));
    payload.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
    assert!(payload.validate().is_err());

    let mut payload = temporal_payload(&pubkey_hex(&key));
    payload.observed_profile_root = "sha256:abc".to_string();
    assert!(payload.validate().is_err());

    let mut update = temporal_payload(&pubkey_hex(&key));
    update.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
    update.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
    assert!(update.validate().is_err());

    update.previous_identity_event_root = Some(root('2'));
    update.validate().unwrap();

    update.previous_identity_event_root = Some("sha256:abc".to_string());
    assert!(update.validate().is_err());
}

#[test]
fn frontier_repository_bound_genesis_identity_is_derived_not_supplied() {
    let project = genesis_project();
    let genesis = &project.events[0];
    let identity = FrontierIdentityV1::from_genesis_event(genesis).unwrap();
    assert_eq!(Some(identity.frontier_id.clone()), project.frontier_id);
    assert_eq!(
        identity.origin_commitment,
        repository_identity_event_content_root(genesis).unwrap()
    );

    let mut tampered = genesis.clone();
    tampered.payload["name"] = serde_json::json!("different-frontier");
    assert!(FrontierIdentityV1::from_genesis_event(&tampered).is_err());
}

#[test]
fn historical_frontier_created_replays_but_cannot_seed_profile_v1_identity() {
    let historical = historical_genesis_event();
    validate_event_payload("frontier.created", &historical.payload).unwrap();
    assert!(FrontierIdentityV1::from_genesis_event(&historical).is_err());
    assert!(repository_identity_event_content_root(&historical).is_err());

    // A retained historical creation record is ordinary anchored legacy
    // history. It must not block the separately protected TOFU boundary.
    let (temporal, _) = signed_temporal_event();
    assert!(validate_repository_boundary_event_set(&[historical, temporal]).is_empty());
}

#[test]
fn frontier_repository_bound_first_genesis_update_forms_a_valid_chain() {
    let project = genesis_project();
    let genesis = &project.events[0];
    let (event, _) = signed_genesis_update(genesis);
    let payload = repository_boundary_payload_from_event_shape(&event).unwrap();
    payload.validate_chain(genesis).unwrap();
    assert!(validate_repository_boundary_event_set(&[genesis.clone(), event]).is_empty());
}

#[test]
fn frontier_repository_bound_full_event_set_rejects_unsigned_missing_parent_and_fork() {
    let (temporal, key) = signed_temporal_event();

    let mut unsigned = temporal.clone();
    unsigned.signature = None;
    let errors = validate_repository_boundary_event_set(&[unsigned]);
    assert!(errors.iter().any(|error| error.contains("signature")));

    let previous = repository_boundary_payload_from_event_shape(&temporal).unwrap();
    let mut missing = previous.clone();
    missing.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
    missing.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
    missing.previous_identity_event_root = Some(root('0'));
    missing.anchor_event_count += 1;
    let mut missing_event = new_repository_boundary_event(
        missing,
        "Reference a missing repository boundary.",
        "2026-07-22T12:01:00Z",
    )
    .unwrap();
    missing_event.signature = Some(sign_event(&missing_event, &key).unwrap());
    let errors = validate_repository_boundary_event_set(&[temporal.clone(), missing_event]);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("missing identity parent"))
    );

    let parent_root = repository_boundary_event_content_root(&temporal).unwrap();
    let mut children = Vec::new();
    for (index, dependency_id) in ['a', 'b'].into_iter().enumerate() {
        let mut child = previous.clone();
        child.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
        child.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
        child.previous_identity_event_root = Some(parent_root.clone());
        child.anchor_event_count += 1;
        child.dependencies = vec![dependency(
            if dependency_id == 'a' {
                "aaaaaaaaaaaaaaaa"
            } else {
                "bbbbbbbbbbbbbbbb"
            },
            dependency_id,
        )];
        child.dependency_root = exact_dependency_root(&child.dependencies).unwrap();
        let mut event = new_repository_boundary_event(
            child,
            "Create a conflicting signed child.",
            &format!("2026-07-22T12:0{}:00Z", index + 2),
        )
        .unwrap();
        event.signature = Some(sign_event(&event, &key).unwrap());
        children.push(event);
    }
    let errors =
        validate_repository_boundary_event_set(&[temporal, children.remove(0), children.remove(0)]);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("conflicting children"))
    );
}

#[test]
fn frontier_repository_bound_anchor_count_must_advance() {
    let (previous_event, _) = signed_temporal_event();
    let previous = repository_boundary_payload_from_event_shape(&previous_event).unwrap();
    let mut update = previous.clone();
    update.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
    update.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
    update.previous_identity_event_root =
        Some(repository_boundary_event_content_root(&previous_event).unwrap());
    assert!(update.validate_chain(&previous_event).is_err());
    update.anchor_event_count -= 1;
    assert!(update.validate_chain(&previous_event).is_err());
}

#[test]
fn frontier_repository_bound_replay_rejects_an_unsigned_boundary() {
    let mut project = genesis_project();
    let genesis = project.events[0].clone();
    let (signed, _) = signed_genesis_update(&genesis);
    project.events.push(signed.clone());
    assert!(verify_replay(&project).ok);

    project.events[1].signature = None;
    let report = replay_report(&project);
    assert!(!report.ok);
    assert!(
        report
            .conflicts
            .iter()
            .any(|error| error.contains("signature"))
    );
    let verification = verify_replay(&project);
    assert!(!verification.ok);
    assert!(
        verification
            .diffs
            .iter()
            .any(|error| error.contains("signature"))
    );
}
