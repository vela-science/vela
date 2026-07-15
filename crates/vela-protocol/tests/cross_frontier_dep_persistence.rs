//! Cross-frontier dependency metadata survives a split-repo materialize cycle.
//!
//! Dependency mutation is no longer a direct CLI operation. This fixture starts
//! from already-accepted project state and verifies the read/materialize path:
//! `Project.dependencies` is mirrored into `frontier.yaml` and rehydrated on
//! every load without relying on `frontier.json` as an authority source.

use tempfile::TempDir;
use vela_protocol::project::ProjectDependency;

#[test]
fn accepted_cross_frontier_dependency_survives_materialize_cycle() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("dep-test");
    let target_vfr = "vfr_aaaaaaaaaaaaaaaa";
    let target_snapshot = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    let mut project =
        vela_protocol::project::assemble("Dep persistence test", Vec::new(), 0, 0, "fixture");
    project.project.dependencies.push(ProjectDependency {
        name: "test-target-frontier".to_string(),
        source: "git".to_string(),
        version: None,
        pinned_hash: None,
        vfr_id: Some(target_vfr.to_string()),
        locator: Some("https://example.test/frontier.json".to_string()),
        pinned_snapshot_hash: Some(target_snapshot.to_string()),
    });
    vela_protocol::repo::init_repo(&frontier, &project).expect("initialize split frontier");

    vela_protocol::frontier_repo::materialize(&frontier).expect("materialize derived views");

    let loaded = vela_protocol::repo::load_from_path(&frontier).expect("reload frontier");
    assert_eq!(loaded.project.dependencies.len(), 1);
    assert_eq!(
        loaded.project.dependencies[0].vfr_id.as_deref(),
        Some(target_vfr)
    );
    assert_eq!(
        loaded.project.dependencies[0]
            .pinned_snapshot_hash
            .as_deref(),
        Some(target_snapshot)
    );

    let manifest: serde_yaml::Value = serde_yaml::from_slice(
        &std::fs::read(frontier.join("frontier.yaml")).expect("read manifest"),
    )
    .expect("parse manifest");
    assert_eq!(
        manifest["dependencies"]["frontiers_v2"][0]["vfr_id"].as_str(),
        Some(target_vfr)
    );
}
