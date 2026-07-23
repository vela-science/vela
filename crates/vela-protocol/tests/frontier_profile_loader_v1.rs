use ed25519_dalek::SigningKey;
use tempfile::TempDir;
use vela_protocol::events::{event_log_hash, snapshot_hash};
use vela_protocol::frontier_profile::{FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileV1};
use vela_protocol::frontier_repo::{
    self, FRONTIER_LOCK_SCHEMA, FRONTIER_LOCK_SCHEMA_V1, FrontierLockFile, FrontierProfileFile,
};
use vela_protocol::frontier_repository::{
    FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryMode,
    FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, GitObjectFormat,
    LEGACY_FRONTIER_ORIGIN_SCHEMA, LegacyFrontierOriginV1, exact_dependency_root,
    new_repository_boundary_event,
};
use vela_protocol::project;
use vela_protocol::sign::{pubkey_hex, sign_event};

const FRONTIER_ID: &str = "vfr_1234567890abcdef";

fn root(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn profile(summary: &str, frontier_id: &str) -> String {
    format!(
        r#"schema: {FRONTIER_PROFILE_SCHEMA_V1}
frontier_id: {frontier_id}
name: Profile loader fixture
summary: {summary}
scope:
  question: Does Profile v1 remain separate from authority state?
  includes:
    - Rooted loader behavior
  excludes:
    - Human authority
maintainers:
  - maintainer:fixture
license:
  content: CC-BY-4.0
  code: Apache-2.0
  data: CC0-1.0
"#
    )
}

fn signed_legacy_boundary(profile_root: &str) -> vela_protocol::events::StateEvent {
    let key = SigningKey::from_bytes(&[0x41; 32]);
    let legacy_identity_preimage_root = root('1');
    let origin = LegacyFrontierOriginV1 {
        schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
        frontier_id: FRONTIER_ID.to_string(),
        legacy_identity_preimage_root: legacy_identity_preimage_root.clone(),
        git_object_format: GitObjectFormat::Sha1,
        anchor_git_commit: "2".repeat(40),
        anchor_git_tree: "3".repeat(40),
        anchor_event_log_root: root('4'),
        anchor_event_count: 1,
    };
    let payload = FrontierRepositoryBoundaryPayloadV1 {
        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
        mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
        frontier_id: FRONTIER_ID.to_string(),
        identity_root: origin.identity_root().unwrap(),
        observed_profile_root: profile_root.to_string(),
        dependency_root: exact_dependency_root(&[]).unwrap(),
        dependencies: vec![],
        previous_identity_event_root: None,
        legacy_identity_preimage_root: Some(legacy_identity_preimage_root),
        administrator_actor_id: "reviewer:fixture".to_string(),
        administrator_public_key: pubkey_hex(&key),
        administrator_algorithm: "ed25519".to_string(),
        trust_mode: FrontierRepositoryTrustMode::Tofu,
        git_object_format: GitObjectFormat::Sha1,
        anchor_git_commit: "2".repeat(40),
        anchor_git_tree: "3".repeat(40),
        anchor_event_log_root: root('4'),
        anchor_event_count: 1,
        anchor_snapshot_root: root('6'),
        anchor_snapshot_schema: "vela.project.v0.1".to_string(),
        anchor_proposal_root: root('7'),
        anchor_actor_registry_root: root('8'),
        anchor_artifact_registry_root: root('9'),
        anchor_canonical_store_root: root('a'),
    };
    let mut event = new_repository_boundary_event(
        payload,
        "Bind the exact legacy Profile v1 fixture.",
        "2026-07-22T12:00:00Z",
    )
    .unwrap();
    event.signature = Some(sign_event(&event, &key).unwrap());
    event
}

fn v1_repository() -> (TempDir, std::path::PathBuf) {
    let temporary = TempDir::new().unwrap();
    let frontier = temporary.path().join("profile-v1");
    let profile_bytes = profile("Initial display summary.", FRONTIER_ID);
    let parsed = FrontierProfileV1::from_yaml_str(&profile_bytes).unwrap();
    let mut project = project::assemble("legacy seed", vec![], 0, 0, "legacy seed");
    project.frontier_id = Some(FRONTIER_ID.to_string());
    project.events = vec![signed_legacy_boundary(&parsed.profile_root().unwrap())];
    vela_protocol::repo::init_repo(&frontier, &project).unwrap();
    std::fs::write(frontier.join("frontier.yaml"), profile_bytes).unwrap();
    std::fs::write(
        frontier.join(".vela/settings.toml"),
        "schema = \"vela.frontier-settings.v1\"\n",
    )
    .unwrap();
    std::fs::remove_file(frontier.join(".vela/config.toml")).unwrap();
    (temporary, frontier)
}

#[test]
fn profile_v1_wrong_schema_never_falls_back_to_legacy() {
    let (_temporary, frontier) = v1_repository();
    let invalid = profile("Initial display summary.", FRONTIER_ID)
        .replace(FRONTIER_PROFILE_SCHEMA_V1, "vela.frontier-profile.v2");
    std::fs::write(frontier.join("frontier.yaml"), invalid).unwrap();
    let error = vela_protocol::repo::load_from_path(&frontier).unwrap_err();
    assert!(
        error.contains("unsupported frontier.yaml schema"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn profile_loader_rejects_a_byte_identical_external_symlink() {
    use std::os::unix::fs::symlink;

    let (_temporary, frontier) = v1_repository();
    let profile_path = frontier.join("frontier.yaml");
    let external = TempDir::new().unwrap();
    let external_profile = external.path().join("frontier.yaml");
    std::fs::copy(&profile_path, &external_profile).unwrap();
    std::fs::remove_file(&profile_path).unwrap();
    symlink(&external_profile, &profile_path).unwrap();

    let error = frontier_repo::read_repository_profile(&frontier).unwrap_err();
    assert!(error.contains("regular non-symlink"), "{error}");
}

#[test]
fn profile_edit_changes_only_profile_root() {
    let (_temporary, frontier) = v1_repository();
    let initial = vela_protocol::repo::load_from_path(&frontier).unwrap();
    let initial_profile = match frontier_repo::read_repository_profile(&frontier).unwrap() {
        Some(FrontierProfileFile::V1(profile)) => profile,
        other => panic!("expected Profile v1, got {other:?}"),
    };
    let initial_projection = initial_profile.project(&initial).unwrap();

    frontier_repo::materialize(&frontier).unwrap();
    let initial_lock = match frontier_repo::read_repository_lock(&frontier).unwrap() {
        Some(FrontierLockFile::V1(lock)) => lock,
        other => panic!("expected lock v1, got {other:?}"),
    };
    assert_eq!(initial_lock.schema, FRONTIER_LOCK_SCHEMA_V1);
    assert_eq!(initial_lock.profile_root, initial_projection.profile_root);
    assert_eq!(
        initial_lock.scientific_state_root,
        initial_projection.scientific_state_root
    );

    let edited_bytes = profile("Edited display summary only.", FRONTIER_ID);
    std::fs::write(frontier.join("frontier.yaml"), &edited_bytes).unwrap();
    let edited = vela_protocol::repo::load_from_path(&frontier).unwrap();
    let edited_profile = match frontier_repo::read_repository_profile(&frontier).unwrap() {
        Some(FrontierProfileFile::V1(profile)) => profile,
        other => panic!("expected Profile v1, got {other:?}"),
    };
    let edited_projection = edited_profile.project(&edited).unwrap();

    assert_ne!(
        initial_projection.profile_root,
        edited_projection.profile_root
    );
    assert_eq!(
        initial_projection.frontier_id,
        edited_projection.frontier_id
    );
    assert_eq!(
        initial_projection.identity_root,
        edited_projection.identity_root
    );
    assert_eq!(
        initial_projection.dependency_root,
        edited_projection.dependency_root
    );
    assert_eq!(
        initial_projection.scientific_state_root,
        edited_projection.scientific_state_root
    );

    frontier_repo::materialize(&frontier).unwrap();
    assert_eq!(
        std::fs::read_to_string(frontier.join("frontier.yaml")).unwrap(),
        edited_bytes
    );
    let edited_lock = match frontier_repo::read_repository_lock(&frontier).unwrap() {
        Some(FrontierLockFile::V1(lock)) => lock,
        other => panic!("expected lock v1, got {other:?}"),
    };
    assert_eq!(edited_lock.profile_root, edited_projection.profile_root);
    assert_eq!(
        edited_lock.scientific_state_root,
        initial_lock.scientific_state_root
    );
    assert!(frontier_repo::layout_issues(&frontier, &edited).is_empty());
}

#[test]
fn profile_v1_id_mismatch_cannot_replace_event_derived_identity() {
    let (_temporary, frontier) = v1_repository();
    std::fs::write(
        frontier.join("frontier.yaml"),
        profile("Initial display summary.", "vfr_fedcba9876543210"),
    )
    .unwrap();
    let error = vela_protocol::repo::load_from_path(&frontier).unwrap_err();
    assert!(error.contains("does not match bound Frontier"), "{error}");
}

#[test]
fn legacy_profile_replay_and_materialization_remain_v0_1() {
    let temporary = TempDir::new().unwrap();
    let frontier = temporary.path().join("legacy");
    let project = project::assemble("Legacy fixture", vec![], 0, 0, "Legacy replay fixture.");
    vela_protocol::repo::init_repo(&frontier, &project).unwrap();
    let before = vela_protocol::repo::load_from_path(&frontier).unwrap();
    let event_root = event_log_hash(&before.events);
    let snapshot_root = snapshot_hash(&before);

    frontier_repo::materialize(&frontier).unwrap();
    let after = vela_protocol::repo::load_from_path(&frontier).unwrap();
    assert_eq!(event_log_hash(&after.events), event_root);
    assert_eq!(snapshot_hash(&after), snapshot_root);
    match frontier_repo::read_repository_profile(&frontier).unwrap() {
        Some(FrontierProfileFile::LegacyV0_1(manifest)) => {
            assert_eq!(manifest.schema, "vela.frontier_manifest.v0.1");
        }
        other => panic!("expected legacy manifest, got {other:?}"),
    }
    match frontier_repo::read_repository_lock(&frontier).unwrap() {
        Some(FrontierLockFile::LegacyV0_1(lock)) => {
            assert_eq!(lock.schema, FRONTIER_LOCK_SCHEMA);
        }
        other => panic!("expected legacy lock, got {other:?}"),
    }
}
