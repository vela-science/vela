//! Explicit, local consumer pinning for a Frontier's first administrator boundary.
//!
//! Repository bytes may prove a complete boundary chain, but they cannot
//! choose which first administrator fork the consumer intended. This command
//! therefore requires the full boundary root from an out-of-band source,
//! previews an exact local trust-anchor plan without writing, and installs it
//! only after a matching confirmation root is supplied.

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use vela_edge::frontier_repository::{
    RepositoryTrustAnchor, verify_repository_boundary_context_with_trust_anchor,
};
use vela_edge::repository_write::{
    REPOSITORY_TRUST_ANCHOR_SCHEMA_V1, RepositoryTrustAnchorV1,
    install_repository_trust_anchor_from_home,
};
use vela_protocol::events::event_log_hash;
use vela_protocol::frontier_profile::EffectiveFrontierAuthorityV1;
use vela_protocol::frontier_repo::{FrontierProfileFile, read_repository_profile};
use vela_protocol::frontier_repository::{
    FrontierRepositoryBoundaryMode, FrontierRepositoryTrustMode,
    repository_boundary_event_content_root, repository_boundary_payload_from_event_shape,
    repository_identity_event_content_root,
};

const TRUST_PIN_PLAN_SCHEMA: &str = "vela.repository-trust-pin-plan.v1";
const TRUST_PIN_PLAN_DOMAIN: &[u8] = b"vela.repository-trust-pin-plan.v1\0";

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryTrustPinPlan {
    schema: String,
    ok: bool,
    command: String,
    frontier: String,
    frontier_id: String,
    identity_root: String,
    identity_head_root: String,
    supplied_first_boundary_root: String,
    administrator_actor_id: String,
    administrator_public_key: String,
    boundary_mode: String,
    boundary_trust_mode: String,
    git_commit: String,
    git_tree: String,
    event_log_root: String,
    event_count: u64,
    trust_anchor: RepositoryTrustAnchorV1,
    trust_anchor_root: String,
    observed_at: String,
    writes: Vec<String>,
    plan_root: String,
}

fn git(frontier: &Path, args: &[&str]) -> Result<String, String> {
    crate::git_hardened::text(frontier, args)
}

fn compute_plan_root(plan: &RepositoryTrustPinPlan) -> Result<String, String> {
    let mut value =
        serde_json::to_value(plan).map_err(|error| format!("encode trust-pin plan: {error}"))?;
    value
        .as_object_mut()
        .ok_or_else(|| "trust-pin plan is not an object".to_string())?
        .remove("plan_root");
    let canonical = vela_protocol::canonical::to_canonical_bytes(&value)?;
    let mut digest = Sha256::new();
    digest.update(TRUST_PIN_PLAN_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn verify_plan(plan: &RepositoryTrustPinPlan) -> Result<(), String> {
    if plan.schema != TRUST_PIN_PLAN_SCHEMA
        || !plan.ok
        || plan.command != "frontier.trust.pin"
        || plan.plan_root != compute_plan_root(plan)?
    {
        return Err("repository trust-pin plan is malformed or has a stale root".to_string());
    }
    Ok(())
}

fn prepare_trust_pin(
    frontier: &Path,
    supplied_first_boundary_root: &str,
    observed_at: &str,
) -> Result<RepositoryTrustPinPlan, String> {
    let Some(digest) = supplied_first_boundary_root.strip_prefix("sha256:") else {
        return Err("supplied first boundary root must use sha256:<64 lowercase hex>".to_string());
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("supplied first boundary root must use sha256:<64 lowercase hex>".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("trust-pin observation time must be RFC3339: {error}"))?;
    let frontier = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve Frontier checkout: {error}"))?;
    let dirt = vela_edge::git_read::dirty_worktree_paths(&frontier, true)?;
    if !dirt.is_empty() {
        return Err(format!(
            "trust pinning requires a clean checkout; found {}",
            dirt.join(", ")
        ));
    }
    let project = vela_protocol::repo::load_from_path(&frontier)?;
    let profile = match read_repository_profile(&frontier)? {
        Some(FrontierProfileFile::V1(profile)) => profile,
        Some(FrontierProfileFile::LegacyV0_1(_)) | None => {
            return Err("trust pinning requires vela.frontier-profile.v1".to_string());
        }
    };
    profile.validate()?;
    let authority = EffectiveFrontierAuthorityV1::from_events(&project.events)?;
    profile.assert_frontier_id(&authority.frontier_id)?;
    let projection = profile.project(&project)?;
    let replay = vela_protocol::reducer::verify_replay(&project);
    if !replay.ok {
        return Err(format!(
            "trust pinning requires exact replay: {}",
            replay.diffs.join(" | ")
        ));
    }

    let supplied = project
        .events
        .iter()
        .filter(|event| {
            repository_boundary_event_content_root(event)
                .is_ok_and(|root| root == supplied_first_boundary_root)
        })
        .collect::<Vec<_>>();
    let [supplied] = supplied.as_slice() else {
        return Err(format!(
            "supplied boundary root must identify exactly one repository event, found {}",
            supplied.len()
        ));
    };
    let supplied_payload = repository_boundary_payload_from_event_shape(supplied)?;
    let structurally_first = match supplied_payload.mode {
        FrontierRepositoryBoundaryMode::TemporalizeExisting => {
            supplied_payload.trust_mode == FrontierRepositoryTrustMode::Tofu
                && supplied_payload.previous_identity_event_root.is_none()
        }
        FrontierRepositoryBoundaryMode::UpdateDependencies => {
            let Some(parent) = supplied_payload.previous_identity_event_root.as_deref() else {
                return Err("first native boundary has no genesis parent".to_string());
            };
            supplied_payload.trust_mode == FrontierRepositoryTrustMode::Genesis
                && project.events.iter().any(|event| {
                    event.kind.as_str() == "frontier.created"
                        && repository_identity_event_content_root(event)
                            .is_ok_and(|root| root == parent)
                })
        }
    };
    if !structurally_first {
        return Err("supplied root is not the chain's first administrator boundary".to_string());
    }

    let head = project
        .events
        .iter()
        .filter(|event| {
            repository_identity_event_content_root(event)
                .is_ok_and(|root| root == projection.identity_event_root)
        })
        .collect::<Vec<_>>();
    let [head] = head.as_slice() else {
        return Err(format!(
            "effective repository identity head must identify exactly one event, found {}",
            head.len()
        ));
    };
    verify_repository_boundary_context_with_trust_anchor(
        &project,
        &frontier,
        head,
        Some(&RepositoryTrustAnchor {
            boundary_content_root: supplied_first_boundary_root.to_string(),
            administrator_public_key: supplied_payload.administrator_public_key.clone(),
        }),
    )?;

    let anchor = RepositoryTrustAnchorV1 {
        schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        frontier_id: projection.frontier_id.clone(),
        identity_root: projection.identity_root.clone(),
        boundary_content_root: supplied_first_boundary_root.to_string(),
        administrator_actor_id: supplied_payload.administrator_actor_id.clone(),
        administrator_public_key: supplied_payload.administrator_public_key.clone(),
    };
    let anchor_root = anchor.root()?;
    let event_count =
        u64::try_from(project.events.len()).map_err(|_| "event count exceeds u64".to_string())?;
    let mut plan = RepositoryTrustPinPlan {
        schema: TRUST_PIN_PLAN_SCHEMA.to_string(),
        ok: true,
        command: "frontier.trust.pin".to_string(),
        frontier: frontier.display().to_string(),
        frontier_id: projection.frontier_id,
        identity_root: projection.identity_root,
        identity_head_root: projection.identity_event_root,
        supplied_first_boundary_root: supplied_first_boundary_root.to_string(),
        administrator_actor_id: supplied_payload.administrator_actor_id,
        administrator_public_key: supplied_payload.administrator_public_key,
        boundary_mode: match supplied_payload.mode {
            FrontierRepositoryBoundaryMode::TemporalizeExisting => "temporalize_existing",
            FrontierRepositoryBoundaryMode::UpdateDependencies => "update_dependencies",
        }
        .to_string(),
        boundary_trust_mode: match supplied_payload.trust_mode {
            FrontierRepositoryTrustMode::Tofu => "tofu",
            FrontierRepositoryTrustMode::Genesis => "genesis",
            FrontierRepositoryTrustMode::PreviousBoundary => "previous_boundary",
        }
        .to_string(),
        git_commit: git(&frontier, &["rev-parse", "HEAD^{commit}"])?,
        git_tree: git(&frontier, &["rev-parse", "HEAD^{tree}"])?,
        event_log_root: format!("sha256:{}", event_log_hash(&project.events)),
        event_count,
        trust_anchor: anchor,
        trust_anchor_root: anchor_root,
        observed_at: observed_at.to_string(),
        writes: vec![format!(
            "<os-account-home>/.vela/trust/frontiers/{}.json",
            project.frontier_id()
        )],
        plan_root: String::new(),
    };
    plan.plan_root = compute_plan_root(&plan)?;
    verify_plan(&plan)?;
    Ok(plan)
}

fn install_confirmed_trust_pin(
    frontier: &Path,
    supplied_first_boundary_root: &str,
    confirm_root: &str,
    confirm_at: &str,
    user_home: &Path,
) -> Result<RepositoryTrustPinPlan, String> {
    let plan = prepare_trust_pin(frontier, supplied_first_boundary_root, confirm_at)?;
    verify_plan(&plan)?;
    if plan.plan_root != confirm_root {
        return Err(format!(
            "trust-pin confirmation root mismatch: supplied {confirm_root}, current {}",
            plan.plan_root
        ));
    }
    let installed = install_repository_trust_anchor_from_home(user_home, &plan.trust_anchor)?;
    if installed.root != plan.trust_anchor_root || installed.anchor != plan.trust_anchor {
        return Err("installed trust anchor does not match the confirmed plan".to_string());
    }
    Ok(plan)
}

pub(crate) fn cmd_frontier_trust_pin(
    frontier: &Path,
    boundary_root: &str,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json: bool,
) {
    crate::ui::set_mode("frontier.trust.pin", json);
    match (confirm_root, confirm_at) {
        (None, None) => {
            let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let plan = prepare_trust_pin(frontier, boundary_root, &observed_at)
                .unwrap_or_else(|error| super::fail_return(&error));
            if json {
                super::print_json(&plan);
            } else {
                println!("frontier trust pin · key-free preview");
                println!("  frontier: {}", plan.frontier_id);
                println!("  first boundary: {}", plan.supplied_first_boundary_root);
                println!("  administrator: {}", plan.administrator_actor_id);
                println!("  public key: {}", plan.administrator_public_key);
                println!("  plan root: {}", plan.plan_root);
                println!("  confirm at: {}", plan.observed_at);
                println!("  writes now: none");
            }
        }
        (Some(confirm_root), Some(confirm_at)) => {
            crate::decision_plan::validate_scripted_confirmation_time(confirm_at).unwrap_or_else(
                |error| super::fail_return(&format!("{}: {}", error.code, error.message)),
            );
            let user_home = crate::frontier_txn::operating_system_account_home()
                .unwrap_or_else(|error| super::fail_return(&error.to_string()));
            let plan = install_confirmed_trust_pin(
                frontier,
                boundary_root,
                confirm_root,
                confirm_at,
                &user_home,
            )
            .unwrap_or_else(|error| super::fail_return(&error));
            if json {
                super::print_json(&serde_json::json!({
                    "ok": true,
                    "command": "frontier.trust.pin",
                    "plan": plan,
                    "installed": true,
                }));
            } else {
                println!("pinned {}", plan.supplied_first_boundary_root);
                println!("  trust anchor: {}", plan.trust_anchor_root);
            }
        }
        _ => super::fail_return::<()>(
            "--confirm-root and --confirm-at must be supplied together; omit both for preview",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::frontier_repo::{ProfileV1InitOptions, initialize_profile_v1_minimal};
    use vela_protocol::frontier_repository::{
        FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryPayloadV1, GitObjectFormat,
        exact_dependency_root, new_repository_boundary_event,
    };
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};

    const OBSERVED_AT: &str = "2026-07-22T12:00:00Z";

    fn run(path: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn fixture() -> (tempfile::TempDir, String) {
        let directory = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            directory.path(),
            ProfileV1InitOptions {
                name: "Pinned fixture",
                scope: "Can one exact first boundary be pinned?",
                initialize_git: true,
            },
        )
        .unwrap();
        run(directory.path(), &["config", "user.name", "Vela Test"]);
        run(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        let mut project = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let key = SigningKey::from_bytes(&[53; 32]);
        let actor = ActorRecord {
            id: "reviewer:pin".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        project.actors = vec![actor.clone()];
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        run(directory.path(), &["add", "."]);
        run(
            directory.path(),
            &["commit", "-qm", "native boundary anchor"],
        );

        let profile = match read_repository_profile(directory.path()).unwrap().unwrap() {
            FrontierProfileFile::V1(profile) => profile,
            _ => panic!("Profile v1"),
        };
        let authority = EffectiveFrontierAuthorityV1::from_events(&project.events).unwrap();
        let projection = profile.project(&project).unwrap();
        let anchor = vela_edge::frontier_repository::derive_repository_anchor_facts(
            directory.path(),
            &run(directory.path(), &["rev-parse", "HEAD"]),
        )
        .unwrap();
        let dependencies = Vec::new();
        let payload = FrontierRepositoryBoundaryPayloadV1 {
            schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
            mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
            frontier_id: authority.frontier_id,
            identity_root: projection.identity_root,
            observed_profile_root: profile.profile_root().unwrap(),
            dependency_root: exact_dependency_root(&dependencies).unwrap(),
            dependencies,
            previous_identity_event_root: Some(projection.identity_event_root),
            legacy_identity_preimage_root: None,
            administrator_actor_id: actor.id,
            administrator_public_key: actor.public_key,
            administrator_algorithm: actor.algorithm,
            trust_mode: FrontierRepositoryTrustMode::Genesis,
            git_object_format: GitObjectFormat::Sha1,
            anchor_git_commit: anchor.git_commit,
            anchor_git_tree: anchor.git_tree,
            anchor_event_log_root: anchor.event_log_root,
            anchor_event_count: anchor.event_count,
            anchor_snapshot_root: anchor.snapshot_root,
            anchor_snapshot_schema: anchor.snapshot_schema,
            anchor_proposal_root: anchor.proposal_root,
            anchor_actor_registry_root: anchor.actor_registry_root,
            anchor_artifact_registry_root: anchor.artifact_registry_root,
            anchor_canonical_store_root: anchor.canonical_store_root,
        };
        let mut boundary =
            new_repository_boundary_event(payload, "Bind first administrator", OBSERVED_AT)
                .unwrap();
        boundary.signature = Some(sign_event(&boundary, &key).unwrap());
        let root = repository_boundary_event_content_root(&boundary).unwrap();
        project.events.push(boundary);
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        run(directory.path(), &["add", "."]);
        run(directory.path(), &["commit", "-qm", "signed boundary"]);
        (directory, root)
    }

    #[test]
    fn preview_is_write_free_and_confirmed_install_is_non_replacing() {
        let (directory, root) = fixture();
        let home = tempfile::tempdir().unwrap();
        let plan = prepare_trust_pin(directory.path(), &root, OBSERVED_AT).unwrap();
        assert!(plan.ok);
        assert_eq!(plan.command, "frontier.trust.pin");
        let wire = serde_json::to_value(&plan).unwrap();
        assert_eq!(wire["ok"], true);
        assert_eq!(wire["command"], "frontier.trust.pin");
        assert!(!home.path().join(".vela").exists());
        let installed = install_confirmed_trust_pin(
            directory.path(),
            &root,
            &plan.plan_root,
            OBSERVED_AT,
            home.path(),
        )
        .unwrap();
        assert_eq!(installed.plan_root, plan.plan_root);
        assert!(
            home.path()
                .join(".vela/trust/frontiers")
                .join(format!("{}.json", plan.frontier_id))
                .is_file()
        );

        let wrong = format!("sha256:{}", "f".repeat(64));
        assert!(prepare_trust_pin(directory.path(), &wrong, OBSERVED_AT).is_err());
        assert!(
            install_confirmed_trust_pin(directory.path(), &root, &wrong, OBSERVED_AT, home.path(),)
                .is_err()
        );
    }

    #[test]
    fn hostile_git_environment_and_replacements_cannot_redirect_or_write_trust() {
        const CHILD_ENV: &str = "VELA_TEST_HOSTILE_TRUST_GIT_CHILD";
        const SOURCE_ENV: &str = "VELA_TEST_HOSTILE_TRUST_SOURCE";
        const DECOY_ENV: &str = "VELA_TEST_HOSTILE_TRUST_DECOY";
        const BOUNDARY_ENV: &str = "VELA_TEST_HOSTILE_TRUST_BOUNDARY";
        const HOME_ENV: &str = "VELA_TEST_HOSTILE_TRUST_HOME";
        const SOURCE_HEAD_ENV: &str = "VELA_TEST_HOSTILE_TRUST_SOURCE_HEAD";
        const SOURCE_TREE_ENV: &str = "VELA_TEST_HOSTILE_TRUST_SOURCE_TREE";
        const DECOY_HEAD_ENV: &str = "VELA_TEST_HOSTILE_TRUST_DECOY_HEAD";
        const DECOY_TREE_ENV: &str = "VELA_TEST_HOSTILE_TRUST_DECOY_TREE";
        const TEST_NAME: &str = concat!(
            "cli::repository_trust::tests::",
            "hostile_git_environment_and_replacements_cannot_redirect_or_write_trust"
        );

        if std::env::var_os(CHILD_ENV).is_none() {
            // Git environment variables are process-global inputs. Exercise
            // the hostile case in an exact single-test child so parallel tests
            // cannot observe them.
            let (source, boundary_root) = fixture();
            let hostile = tempfile::tempdir().unwrap();
            let decoy = hostile.path().join("decoy");
            let clone = std::process::Command::new("git")
                .args([
                    "clone",
                    "--no-hardlinks",
                    source.path().to_str().unwrap(),
                    decoy.to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert!(
                clone.status.success(),
                "clone decoy: {}",
                String::from_utf8_lossy(&clone.stderr)
            );
            run(&decoy, &["config", "user.name", "Vela Test"]);
            run(&decoy, &["config", "user.email", "vela@example.invalid"]);
            std::fs::write(decoy.join("decoy-only.txt"), b"decoy repository\n").unwrap();
            run(&decoy, &["add", "decoy-only.txt"]);
            run(&decoy, &["commit", "-qm", "decoy-only commit"]);

            let source_head =
                crate::git_hardened::text(source.path(), &["rev-parse", "HEAD^{commit}"]).unwrap();
            let source_tree =
                crate::git_hardened::text(source.path(), &["rev-parse", "HEAD^{tree}"]).unwrap();
            let decoy_head =
                crate::git_hardened::text(&decoy, &["rev-parse", "HEAD^{commit}"]).unwrap();
            let decoy_tree =
                crate::git_hardened::text(&decoy, &["rev-parse", "HEAD^{tree}"]).unwrap();
            assert_ne!(source_head, decoy_head);
            assert_ne!(source_tree, decoy_tree);

            // Install a real replacement ref in the named source. Raw Git now
            // resolves the source commit through the decoy commit, while the
            // trust runner must retain the source's actual tree.
            run(
                source.path(),
                &["fetch", decoy.to_str().unwrap(), &decoy_head],
            );
            run(source.path(), &["replace", &source_head, &decoy_head]);
            assert_eq!(
                run(source.path(), &["rev-parse", "HEAD^{tree}"]),
                decoy_tree,
                "test setup did not activate the replacement ref"
            );
            assert_eq!(
                crate::git_hardened::text(source.path(), &["rev-parse", "HEAD^{tree}"]).unwrap(),
                source_tree,
                "hardened Git must ignore repository replacement refs"
            );

            let trust_home = hostile.path().join("trust-home");
            std::fs::create_dir(&trust_home).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", TEST_NAME, "--nocapture"]);
            for (name, _) in std::env::vars_os() {
                if name.to_str().is_some_and(|name| name.starts_with("GIT_")) {
                    child.env_remove(name);
                }
            }
            child
                .env(CHILD_ENV, "1")
                .env(SOURCE_ENV, source.path())
                .env(DECOY_ENV, &decoy)
                .env(BOUNDARY_ENV, &boundary_root)
                .env(HOME_ENV, &trust_home)
                .env(SOURCE_HEAD_ENV, &source_head)
                .env(SOURCE_TREE_ENV, &source_tree)
                .env(DECOY_HEAD_ENV, &decoy_head)
                .env(DECOY_TREE_ENV, &decoy_tree)
                .env("GIT_DIR", decoy.join(".git"))
                .env("GIT_WORK_TREE", &decoy)
                .env("GIT_COMMON_DIR", decoy.join(".git"))
                .env("GIT_INDEX_FILE", decoy.join(".git/index"))
                .env("GIT_OBJECT_DIRECTORY", decoy.join(".git/objects"))
                .env(
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                    source.path().join(".git/objects"),
                )
                .env("GIT_REPLACE_REF_BASE", "refs/replace/hostile")
                .env("GIT_CONFIG_COUNT", "2")
                .env("GIT_CONFIG_KEY_0", "core.bare")
                .env("GIT_CONFIG_VALUE_0", "true")
                .env("GIT_CONFIG_KEY_1", "core.worktree")
                .env("GIT_CONFIG_VALUE_1", &decoy);
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "hostile trust child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let source = std::path::PathBuf::from(std::env::var_os(SOURCE_ENV).unwrap());
        let decoy = std::path::PathBuf::from(std::env::var_os(DECOY_ENV).unwrap());
        let boundary_root = std::env::var(BOUNDARY_ENV).unwrap();
        let trust_home = std::path::PathBuf::from(std::env::var_os(HOME_ENV).unwrap());
        let source_head = std::env::var(SOURCE_HEAD_ENV).unwrap();
        let source_tree = std::env::var(SOURCE_TREE_ENV).unwrap();
        let decoy_head = std::env::var(DECOY_HEAD_ENV).unwrap();
        let decoy_tree = std::env::var(DECOY_TREE_ENV).unwrap();

        let plan = prepare_trust_pin(&source, &boundary_root, OBSERVED_AT).unwrap();
        assert_eq!(plan.git_commit, source_head);
        assert_eq!(plan.git_tree, source_tree);
        assert_ne!(plan.git_commit, decoy_head);
        assert_ne!(plan.git_tree, decoy_tree);
        assert!(
            !trust_home.join(".vela").exists(),
            "key-free preview wrote consumer trust state"
        );

        // The exact named source drifts after confirmation while the hostile
        // decoy remains clean. A redirected Git runner would rederive the old
        // plan and install the pin; the hardened runner must fail before write.
        std::fs::write(source.join("post-preview-drift.txt"), b"source drift\n").unwrap();
        let error = install_confirmed_trust_pin(
            &source,
            &boundary_root,
            &plan.plan_root,
            OBSERVED_AT,
            &trust_home,
        )
        .unwrap_err();
        assert!(
            error.contains("clean checkout"),
            "unexpected trust-pin failure: {error}"
        );
        assert!(
            !trust_home.join(".vela").exists(),
            "failed trust-pin confirmation wrote consumer trust state"
        );
        assert!(
            !decoy.join("post-preview-drift.txt").exists(),
            "trust operation wrote into the redirected decoy worktree"
        );
    }
}
