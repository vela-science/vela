//! Focused ADR 0016 target-index porcelain regressions.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::frontier_profile::FrontierProfileV1;
use vela_protocol::frontier_repository::{
    FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierIdentityV1, FrontierRepositoryBoundaryMode,
    FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, exact_dependency_root,
    new_repository_boundary_event, repository_boundary_event_content_root,
    repository_boundary_payload_from_event_shape, repository_identity_event_content_root,
};
use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_ACTOR_ID")
        .env_remove("VELA_AGENT_KEY_HEX")
        .env_remove("VELA_KEY_PATH")
        .env_remove("VELA_NO_PUBLISH")
        .output()
        .expect("run vela")
}

fn run_with_agent(home: &Path, cwd: &Path, args: &[&str], seed_hex: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("VELA_AGENT_KEY_HEX", seed_hex)
        .env_remove("VELA_ACTOR_ID")
        .env_remove("VELA_KEY_PATH")
        .env_remove("VELA_NO_PUBLISH")
        .output()
        .expect("run vela with agent key")
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn success_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?}\n{}",
        output.status.code(),
        combined(output)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("decode JSON: {error}\n{}", combined(output)))
}

fn failure_json(output: &Output) -> Value {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("decode failure JSON: {error}\n{}", combined(output)))
}

fn visible_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(current)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path == root.join(".git") {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, files);
            } else if path.is_file() {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

struct Fixture {
    directory: tempfile::TempDir,
    home: tempfile::TempDir,
    frontier_id: String,
    candidate: Value,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let frontier = directory.path();
        success_json(&run(
            home.path(),
            frontier,
            &[
                "init",
                ".",
                "--name",
                "Target index fixture",
                "--scope",
                "Can one domain candidate become an exact derived offer?",
                "--json",
            ],
        ));
        git(frontier, &["config", "user.name", "Vela Test"]);
        git(frontier, &["config", "user.email", "vela@example.invalid"]);
        write(&frontier.join("domain/source.json"), br#"{"open":[1056]}"#);
        git(frontier, &["add", "-A"]);
        git(frontier, &["commit", "-qm", "source"]);
        let source_commit = git(frontier, &["rev-parse", "HEAD^{commit}"]);
        let project = vela_protocol::repo::load_from_path(frontier).unwrap();
        let frontier_id = project.frontier_id();
        write(
            &frontier.join("site/problems/1056.json"),
            br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#,
        );
        let candidate = json!({
            "schema": "vela.target-index-candidate.v1",
            "frontier_id": frontier_id,
            "source": {
                "git_commit": source_commit,
                "input_paths": ["domain/source.json"]
            },
            "targets": [{
                "id": "erdos:1056",
                "title": "Erdős 1056",
                "why": "First exact bounded target.",
                "state": "open",
                "rank": 7,
                "objective": "Produce one bounded artifact.",
                "labels": ["erdos", "open"],
                "packet": {
                    "schema": "erdos-frontier.problem-work.v1",
                    "path": "site/problems/1056.json"
                }
            }]
        });
        write(
            &frontier.join(".vela/tmp/target-index-candidate.json"),
            serde_json::to_vec_pretty(&candidate).unwrap(),
        );
        Self {
            directory,
            home,
            frontier_id,
            candidate,
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn command(&self, args: &[&str]) -> Output {
        run(self.home.path(), self.path(), args)
    }

    fn check_args(&self) -> Vec<&str> {
        vec![
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--check",
            "--json",
        ]
    }

    fn apply(&self) -> Value {
        success_json(&self.command(&[
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--apply",
            "--json",
        ]))
    }

    fn install_administrator_boundary(
        &self,
    ) -> vela_edge::repository_write::RepositoryTrustAnchorV1 {
        let mut project = vela_protocol::repo::load_from_path(self.path()).unwrap();
        let genesis = project
            .events
            .iter()
            .find(|event| event.kind.as_str() == "frontier.created")
            .cloned()
            .unwrap();
        let identity = FrontierIdentityV1::from_genesis_event(&genesis).unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x41; 32]);
        let actor = ActorRecord {
            id: "reviewer:target-index-administrator".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T12:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        project.actors = vec![actor.clone()];
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(self.path().to_path_buf()),
            &project,
        )
        .unwrap();
        git(self.path(), &["add", "-A"]);
        git(self.path(), &["commit", "-qm", "anchor administrator"]);
        let anchor_commit = git(self.path(), &["rev-parse", "HEAD^{commit}"]);
        let facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            self.path(),
            &anchor_commit,
        )
        .unwrap();
        let profile = FrontierProfileV1::from_yaml_str(
            &std::fs::read_to_string(self.path().join("frontier.yaml")).unwrap(),
        )
        .unwrap();
        let mut boundary = new_repository_boundary_event(
            FrontierRepositoryBoundaryPayloadV1 {
                schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
                frontier_id: identity.frontier_id.clone(),
                identity_root: identity.root().unwrap(),
                observed_profile_root: profile.profile_root().unwrap(),
                dependency_root: exact_dependency_root(&[]).unwrap(),
                dependencies: Vec::new(),
                previous_identity_event_root: Some(
                    repository_identity_event_content_root(&genesis).unwrap(),
                ),
                legacy_identity_preimage_root: None,
                administrator_actor_id: actor.id,
                administrator_public_key: actor.public_key,
                administrator_algorithm: actor.algorithm,
                trust_mode: FrontierRepositoryTrustMode::Genesis,
                git_object_format: facts.git_object_format,
                anchor_git_commit: facts.git_commit,
                anchor_git_tree: facts.git_tree,
                anchor_event_log_root: facts.event_log_root,
                anchor_event_count: facts.event_count,
                anchor_snapshot_root: facts.snapshot_root,
                anchor_snapshot_schema: facts.snapshot_schema,
                anchor_proposal_root: facts.proposal_root,
                anchor_actor_registry_root: facts.actor_registry_root,
                anchor_artifact_registry_root: facts.artifact_registry_root,
                anchor_canonical_store_root: facts.canonical_store_root,
            },
            "Bind the exact target-index administrator.",
            "2026-07-22T12:01:00Z",
        )
        .unwrap();
        boundary.signature = Some(sign_event(&boundary, &key).unwrap());
        let payload = repository_boundary_payload_from_event_shape(&boundary).unwrap();
        let anchor = vela_edge::repository_write::RepositoryTrustAnchorV1 {
            schema: vela_edge::repository_write::REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: payload.frontier_id,
            identity_root: payload.identity_root,
            boundary_content_root: repository_boundary_event_content_root(&boundary).unwrap(),
            administrator_actor_id: payload.administrator_actor_id,
            administrator_public_key: payload.administrator_public_key,
        };
        project.events.push(boundary);
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(self.path().to_path_buf()),
            &project,
        )
        .unwrap();
        git(self.path(), &["add", "-A"]);
        git(self.path(), &["commit", "-qm", "bind repository"]);

        let mut candidate = self.candidate.clone();
        candidate["source"]["git_commit"] =
            json!(git(self.path(), &["rev-parse", "HEAD^{commit}"]));
        write(
            &self.path().join(".vela/tmp/target-index-candidate.json"),
            serde_json::to_vec_pretty(&candidate).unwrap(),
        );
        vela_edge::repository_write::install_repository_trust_anchor_from_home(
            self.home.path(),
            &anchor,
        )
        .unwrap();
        anchor
    }
}

#[test]
fn seal_check_is_zero_writes_and_reports_exact_read_and_touch_sets() {
    let fixture = Fixture::new();
    let before = visible_files(fixture.path());
    let head_before = git(fixture.path(), &["rev-parse", "HEAD"]);
    let cached_before = git(fixture.path(), &["diff", "--cached", "--name-only"]);
    let output = success_json(&fixture.command(&fixture.check_args()));

    assert_eq!(output["schema"], "vela.target-index-seal.v1");
    assert_eq!(output["mode"], "check");
    assert_eq!(output["changed"], false);
    assert_eq!(output["wrote"], json!([]));
    assert_eq!(output["plan"]["frontier_id"], fixture.frontier_id);
    assert_eq!(output["plan"]["input_paths"], json!(["domain/source.json"]));
    assert_eq!(
        output["plan"]["packet_paths"],
        json!(["site/problems/1056.json"])
    );
    assert_eq!(output["plan"]["touched_paths"], json!(["targets.json"]));
    assert!(output["plan"]["canonical_json"].as_str().is_some());
    assert_eq!(visible_files(fixture.path()), before);
    assert_eq!(git(fixture.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(fixture.path(), &["diff", "--cached", "--name-only"]),
        cached_before
    );
    assert!(!fixture.path().join("targets.json").exists());
}

#[test]
fn seal_apply_atomically_writes_only_targets_json_without_staging() {
    let fixture = Fixture::new();
    let before = visible_files(fixture.path());
    let packet_before = std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap();
    let candidate_before =
        std::fs::read(fixture.path().join(".vela/tmp/target-index-candidate.json")).unwrap();
    let output = fixture.apply();

    assert_eq!(output["mode"], "apply");
    assert_eq!(output["changed"], true);
    assert_eq!(output["wrote"], json!(["targets.json"]));
    let mut after = visible_files(fixture.path());
    let target_bytes = after.remove("targets.json").expect("sealed target index");
    assert_eq!(after, before);
    assert_eq!(
        target_bytes,
        output["plan"]["canonical_json"]
            .as_str()
            .unwrap()
            .as_bytes()
    );
    assert_eq!(
        std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap(),
        packet_before
    );
    assert_eq!(
        std::fs::read(fixture.path().join(".vela/tmp/target-index-candidate.json")).unwrap(),
        candidate_before
    );
    assert_eq!(
        git(fixture.path(), &["diff", "--cached", "--name-only"]),
        ""
    );
}

#[test]
fn next_and_work_reject_home_and_repository_fallbacks_for_the_first_boundary_pin() {
    let fixture = Fixture::new();
    let anchor = fixture.install_administrator_boundary();
    let boundary_anchor = vela_edge::frontier_repository::RepositoryTrustAnchor {
        boundary_content_root: anchor.boundary_content_root.clone(),
        administrator_public_key: anchor.administrator_public_key.clone(),
    };
    let plan = vela_edge::target_index::prepare_target_index_seal(
        fixture.path(),
        &fixture.path().join(".vela/tmp/target-index-candidate.json"),
        env!("CARGO_PKG_VERSION"),
        Some(&boundary_anchor),
    )
    .unwrap();
    vela_edge::target_index::install_target_index_seal(fixture.path(), &plan).unwrap();
    git(fixture.path(), &["add", "targets.json"]);
    git(
        fixture.path(),
        &["commit", "-qm", "seal pinned target index"],
    );
    let anchor_path = fixture
        .home
        .path()
        .join(".vela/trust/frontiers")
        .join(format!("{}.json", fixture.frontier_id));
    assert!(
        anchor_path.is_file(),
        "the hostile HOME contains an otherwise exact consumer pin"
    );

    let head = git(fixture.path(), &["rev-parse", "HEAD"]);
    let hostile_home_next = fixture.command(&["next", ".", "--limit", "1", "--json"]);
    assert!(!hostile_home_next.status.success());
    assert!(
        combined(&hostile_home_next).contains("RepositoryTrustAnchor"),
        "{}",
        combined(&hostile_home_next)
    );
    let repository_local_anchor = fixture
        .path()
        .join(".vela/trust/frontiers")
        .join(format!("{}.json", fixture.frontier_id));
    write(
        &repository_local_anchor,
        serde_json::to_vec_pretty(&anchor).unwrap(),
    );
    let repository_fallback = fixture.command(&["next", ".", "--limit", "1", "--json"]);
    assert!(!repository_fallback.status.success());
    assert!(
        combined(&repository_fallback).contains("RepositoryTrustAnchor"),
        "repository-local trust material must not satisfy the independent user pin: {}",
        combined(&repository_fallback)
    );
    std::fs::remove_file(&repository_local_anchor).unwrap();
    let seed = "22".repeat(32);
    let missing_work = run_with_agent(
        fixture.home.path(),
        fixture.path(),
        &["work", "erdos:1056", "--as", "agent:pinned", "--json"],
        &seed,
    );
    assert!(!missing_work.status.success());
    assert!(
        combined(&missing_work).contains("repository_trust_anchor_required"),
        "{}",
        combined(&missing_work)
    );
    assert_eq!(git(fixture.path(), &["rev-parse", "HEAD"]), head);
    assert!(!fixture.path().join(".vela/work").exists());
}

#[test]
fn candidate_is_closed_and_cannot_supply_seal_owned_fields() {
    for (field, value) in [
        ("index_root", json!(format!("sha256:{}", "0".repeat(64)))),
        ("generated_by", json!({"program":"vela","version":"0.0.0"})),
        ("roots", json!({})),
        ("unknown", json!(true)),
    ] {
        let fixture = Fixture::new();
        let mut candidate = fixture.candidate.clone();
        candidate
            .as_object_mut()
            .unwrap()
            .insert(field.to_string(), value);
        write(
            &fixture.path().join(".vela/tmp/target-index-candidate.json"),
            serde_json::to_vec(&candidate).unwrap(),
        );
        let output = fixture.command(&fixture.check_args());
        let payload = failure_json(&output);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(payload["command"], "target-index.seal");
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown field")
        );
        assert!(!fixture.path().join("targets.json").exists());
    }
}

#[test]
fn repair_is_read_only_and_stale_exact_id_inspection_is_never_actionable() {
    let fixture = Fixture::new();
    fixture.apply();
    git(
        fixture.path(),
        &["add", "targets.json", "site/problems/1056.json"],
    );
    git(fixture.path(), &["commit", "-qm", "sealed index"]);
    write(
        &fixture.path().join("site/problems/1056.json"),
        br#"{"changed":true,"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#,
    );
    let before = visible_files(fixture.path());
    let repair = success_json(&fixture.command(&["target-index", "repair", ".", "--json"]));
    assert_eq!(repair["report"]["schema"], "vela.target-index-repair.v1");
    assert_eq!(
        repair["report"]["changed_declared_paths"],
        json!(["site/problems/1056.json"])
    );
    assert_eq!(
        repair["report"]["repair_command"],
        "vela target-index seal . --candidate .vela/tmp/target-index-candidate.json --check --json"
    );
    assert_eq!(visible_files(fixture.path()), before);

    let inspection =
        success_json(&fixture.command(&["target-index", "inspect", ".", "erdos:1056", "--json"]));
    assert_eq!(inspection["target"]["target_id"], "erdos:1056");
    assert_eq!(inspection["target"]["actionable"], false);
    assert!(
        inspection["target"]["codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "target_index_output_not_tracked")
    );
}

#[test]
fn historical_v1_is_inspectable_but_never_actionable() {
    let fixture = Fixture::new();
    let project = vela_protocol::repo::load_from_path(fixture.path()).unwrap();
    let packet = std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap();
    let legacy = json!({
        "schema": "vela.target-index.v1",
        "frontier_id": project.frontier_id(),
        "as_of": {
            "snapshot_hash": format!("sha256:{}", vela_protocol::events::snapshot_hash(&project)),
            "event_log_hash": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
            "proposal_state_hash": format!("sha256:{}", vela_protocol::proposals::proposal_state_hash(&project.proposals))
        },
        "targets": [{
            "id": "erdos:1056",
            "title": "Erdős 1056",
            "why": "Historical target.",
            "state": "open",
            "rank": 1,
            "objective": "Inspect only.",
            "labels": ["upstream-open", "erdos"],
            "packet": {
                "path": "site/problems/1056.json",
                "sha256": format!("sha256:{}", hex::encode(Sha256::digest(&packet))),
                "schema": "erdos-frontier.problem-work.v1"
            }
        }]
    });
    write(
        &fixture.path().join("targets.json"),
        serde_json::to_vec(&legacy).unwrap(),
    );
    let summary = success_json(&fixture.command(&["target-index", "inspect", ".", "--json"]));
    assert_eq!(summary["summary"]["historical_only"], true);
    assert_eq!(summary["summary"]["configured_open"], 1);
    assert_eq!(summary["summary"]["stale_open"], 1);
    assert_eq!(
        summary["summary"]["codes"],
        json!(["target_index_profile_upgrade_required"])
    );

    let repair = success_json(&fixture.command(&["target-index", "repair", ".", "--json"]));
    assert_eq!(repair["report"]["historical_only"], true);
    assert_eq!(
        repair["report"]["codes"],
        json!(["target_index_profile_upgrade_required"])
    );
    assert!(
        repair["report"]["generator_instruction"]
            .as_str()
            .unwrap()
            .contains("protected frontier-repo-v1 migration")
    );
    assert_eq!(
        repair["report"]["repair_command"],
        "vela migrate . --to frontier-repo-v1 --check --profile ../frontier-profile-v1.yaml --target-candidate ../target-index-candidate.json --as reviewer:ADMINISTRATOR --reason 'Bind exact legacy repository' --json"
    );
    assert_eq!(
        repair["report"]["candidate_path"],
        "../target-index-candidate.json"
    );

    let output =
        success_json(&fixture.command(&["target-index", "inspect", ".", "erdos:1056", "--json"]));
    assert_eq!(output["target"]["index_schema"], "vela.target-index.v1");
    assert_eq!(output["target"]["historical_only"], true);
    assert_eq!(output["target"]["actionable"], false);
    assert_eq!(output["target"]["packet"]["problem"], 1056);
    assert_eq!(
        output["target"]["codes"],
        json!(["target_index_profile_upgrade_required"])
    );
}

#[test]
fn historical_v1_still_rejects_duplicate_labels() {
    let fixture = Fixture::new();
    let project = vela_protocol::repo::load_from_path(fixture.path()).unwrap();
    let packet = std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap();
    let legacy = json!({
        "schema": "vela.target-index.v1",
        "frontier_id": project.frontier_id(),
        "as_of": {
            "snapshot_hash": format!("sha256:{}", vela_protocol::events::snapshot_hash(&project)),
            "event_log_hash": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
            "proposal_state_hash": format!("sha256:{}", vela_protocol::proposals::proposal_state_hash(&project.proposals))
        },
        "targets": [{
            "id": "erdos:1056",
            "title": "Erdős 1056",
            "why": "Historical target.",
            "state": "open",
            "rank": 1,
            "objective": "Inspect only.",
            "labels": ["erdos", "erdos"],
            "packet": {
                "path": "site/problems/1056.json",
                "sha256": format!("sha256:{}", hex::encode(Sha256::digest(&packet))),
                "schema": "erdos-frontier.problem-work.v1"
            }
        }]
    });
    write(
        &fixture.path().join("targets.json"),
        serde_json::to_vec(&legacy).unwrap(),
    );

    let output = fixture.command(&["target-index", "inspect", ".", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        failure_json(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("duplicate label")
    );
}

#[test]
fn inspect_requires_an_exact_valid_full_target_id_and_has_no_bypass_flags() {
    let fixture = Fixture::new();
    fixture.apply();
    for (target, kind) in [("invalid", "usage"), ("erdos:10", "not_found")] {
        let output = fixture.command(&["target-index", "inspect", ".", target, "--json"]);
        let payload = failure_json(&output);
        assert_eq!(payload["error"]["kind"], kind);
        assert!(!combined(&output).contains("Erdős 1056"));
    }

    for args in [
        vec![
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--check",
            "--force",
            "--json",
        ],
        vec![
            "target-index",
            "inspect",
            ".",
            "erdos:1056",
            "--non-strict",
            "--json",
        ],
    ] {
        let output = fixture.command(&args);
        assert_eq!(output.status.code(), Some(2), "{}", combined(&output));
    }
}

#[test]
fn seal_refuses_unrelated_dirty_files_and_invalid_packet_outputs() {
    let fixture = Fixture::new();
    write(
        &fixture.path().join("unrelated.txt"),
        b"not part of the seal",
    );
    let output = fixture.command(&fixture.check_args());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        failure_json(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unrelated worktree dirt")
    );
    assert!(!fixture.path().join("targets.json").exists());

    std::fs::remove_file(fixture.path().join("unrelated.txt")).unwrap();
    write(
        &fixture.path().join("site/problems/1056.json"),
        br#"{"schema":"wrong.packet.v1"}"#,
    );
    let output = fixture.command(&fixture.check_args());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        failure_json(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("target_index_packet_mismatch")
    );
    assert!(!fixture.path().join("targets.json").exists());
}

#[test]
fn indexed_work_binding_survives_session_close_and_fails_closed_on_drift() {
    let fixture = Fixture::new();
    fixture.apply();
    git(
        fixture.path(),
        &["add", "targets.json", "site/problems/1056.json"],
    );
    git(fixture.path(), &["commit", "-qm", "seal target index"]);

    let next = success_json(&fixture.command(&["next", ".", "--limit", "1", "--json"]));
    assert_eq!(next["targets"][0]["target_id"], "erdos:1056");
    assert_eq!(
        next["targets"][0]["rank"], 7,
        "compact offer must retain the sealed canonical rank"
    );
    assert_eq!(next["availability"]["configured"], 1);
    assert_eq!(next["availability"]["stale"], 0);
    assert_eq!(next["availability"]["returned"], 1);
    assert!(
        next["availability"]["repair_command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela target-index repair "))
    );
    let seed = "11".repeat(32);
    let work = success_json(&run_with_agent(
        fixture.home.path(),
        fixture.path(),
        &["work", "erdos:1056", "--as", "agent:indexed", "--json"],
        &seed,
    ));
    let session_path = Path::new(work["session"]["path"].as_str().unwrap()).to_path_buf();
    let original_session: Value =
        serde_json::from_slice(&std::fs::read(&session_path).unwrap()).unwrap();
    let binding_value = original_session["target_task_binding"].clone();
    let binding: vela_edge::target_index::TargetTaskBindingV1 =
        serde_json::from_value(binding_value.clone()).unwrap();
    binding.validate().unwrap();
    assert_eq!(binding.frontier_id, fixture.frontier_id);
    assert_eq!(binding.target_id, "erdos:1056");
    assert_eq!(
        binding.claim_read_set.event_log_root,
        original_session["base_event_log_root"]
    );
    assert_eq!(
        binding.claim_read_set.git_commit,
        original_session["source_git_commit_oid"]
    );

    write(
        &fixture.path().join("artifacts/indexed-result.json"),
        br#"{"bounded":true}"#,
    );
    let land_args = [
        "land",
        "--work",
        "erdos:1056",
        "--claim",
        "The indexed bounded fixture produced one exact artifact.",
        "--type",
        "computational",
        "--replayability",
        "exact",
        "--artifact",
        "artifacts/indexed-result.json:witness",
        "--caveat",
        "Fixture scope only.",
        "--as",
        "agent:indexed",
        "--json",
    ];

    let mut wrong_session_identity = original_session.clone();
    wrong_session_identity["session_id"] = json!(format!("vws_{}", "0".repeat(64)));
    write(
        &session_path,
        serde_json::to_vec_pretty(&wrong_session_identity).unwrap(),
    );
    let tampered = run_with_agent(fixture.home.path(), fixture.path(), &land_args, &seed);
    assert!(!tampered.status.success());
    assert!(combined(&tampered).contains("work-session identity"));
    write(
        &session_path,
        serde_json::to_vec_pretty(&original_session).unwrap(),
    );

    let mut tampered_session = original_session.clone();
    tampered_session["target_task_binding"]["target_id"] = json!("erdos:1057");
    write(
        &session_path,
        serde_json::to_vec_pretty(&tampered_session).unwrap(),
    );
    let tampered = run_with_agent(fixture.home.path(), fixture.path(), &land_args, &seed);
    assert!(!tampered.status.success());
    assert!(combined(&tampered).contains("target task binding"));
    write(
        &session_path,
        serde_json::to_vec_pretty(&original_session).unwrap(),
    );

    let packet_path = fixture.path().join("site/problems/1056.json");
    let packet_bytes = std::fs::read(&packet_path).unwrap();
    write(&packet_path, br#"{"drifted":true}"#);
    let drifted = run_with_agent(fixture.home.path(), fixture.path(), &land_args, &seed);
    assert!(!drifted.status.success());
    assert!(
        combined(&drifted).contains("target task binding"),
        "{}",
        combined(&drifted)
    );
    write(&packet_path, packet_bytes);

    let index_path = fixture.path().join("targets.json");
    let index_bytes = std::fs::read(&index_path).unwrap();
    let mut changed_index = index_bytes.clone();
    changed_index.push(b' ');
    write(&index_path, changed_index);
    let drifted = run_with_agent(fixture.home.path(), fixture.path(), &land_args, &seed);
    assert!(!drifted.status.success());
    assert!(
        combined(&drifted).contains("tracked targets.json must be exact canonical JSON"),
        "{}",
        combined(&drifted)
    );
    write(&index_path, index_bytes);

    let landed = success_json(&run_with_agent(
        fixture.home.path(),
        fixture.path(),
        &land_args,
        &seed,
    ));
    assert!(!session_path.exists());
    let receipt_hex = landed["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let receipt_path = fixture
        .path()
        .join("records/receipts/sha256")
        .join(format!("{receipt_hex}.json"));
    let receipt_bytes = std::fs::read(&receipt_path).unwrap();
    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(&receipt_bytes).unwrap();
    let receipt_binding = &receipt.as_value()["environment"]["vela:target_task_binding"];
    assert_eq!(
        receipt_binding, &binding_value,
        "the landed Receipt must retain the exact session binding value"
    );
    assert_eq!(
        vela_protocol::canonical::to_canonical_bytes(receipt_binding).unwrap(),
        vela_protocol::canonical::to_canonical_bytes(&binding_value).unwrap(),
        "the Receipt extension must have byte-identical canonical binding content"
    );

    let mut tampered_receipt = receipt.as_value().clone();
    tampered_receipt["environment"]["vela:target_task_binding"]["packet"]["sha256"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    assert!(
        vela_protocol::receipt_v1::ReceiptV1::parse(
            &serde_json::to_vec(&tampered_receipt).unwrap()
        )
        .is_err(),
        "target binding tampering must invalidate the Receipt body/attestation round trip"
    );
}
