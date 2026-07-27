//! Key-free planning and exact archival for ADR 0022 repository epochs.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::runtime_authentication::{AuthenticationRequest, RuntimeSessionState};
use vela_protocol::authority::{PrincipalSnapshotV1, SemanticApprovalV1};
use vela_protocol::authority_history::{
    AUTHORITY_INITIALIZATION_SCHEMA_V1, AUTHORITY_INITIALIZE_ACTION,
    AUTHORITY_INITIALIZED_EVENT_KIND, AuthorityInitializationV1,
};
use vela_protocol::bundle::{Artifact, FindingBundle};
use vela_protocol::claim_record::{
    ClaimAssertion, ClaimEvidenceRef, ClaimRecordV1, ClaimRelation, ClaimSource,
    ImportedClaimSource, LEGACY_FINDING_EXTENSION,
};
use vela_protocol::current_repository::{
    CURRENT_ARTIFACT_RECORD_SCHEMA_V1, CURRENT_REPOSITORY_SCHEMA_V2, ClaimStandingRefV1,
    CurrentArtifactRecordV1, CurrentFrontierProfileV2, CurrentRepositoryV2, RepositoryObjectRefV1,
};
use vela_protocol::proposal_v1::{
    ImportedProposalSource, ProposalProducerPackage, ProposalSubject, ProposalV1,
};
use vela_protocol::repository_epoch::{PredecessorRoots, RepositoryEpochV1};
use vela_protocol::{
    events::{EventKind, NULL_HASH, StateActor, StateTarget},
    principal_capability::PrincipalClass,
};

use crate::authority_transaction::{
    AuthorityDerivedDraft, AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityObjectDraft,
    AuthorityTransactionRequest, execute_authority_transaction, execution_binary_sha256,
};
use crate::frontier_txn::{FrontierTxn, WriteClass};
use crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner;

const PLAN_SCHEMA: &str = "vela.repository-upgrade-plan.v1";
const OBJECT_MANIFEST_SCHEMA: &str = "vela.git-object-manifest.v1";
const CANDIDATE_MANIFEST_SCHEMA: &str = "vela.repository-upgrade-candidate.v1";
const EQUIVALENCE_SCHEMA: &str = "vela.repository-equivalence.v1";
const ARCHIVE_INDEX_SCHEMA: &str = "vela.archived-object-index.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitObjectEntry {
    path: String,
    git_mode: String,
    git_object_type: String,
    git_object_id: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitObjectManifest {
    schema: String,
    commit: String,
    tree: String,
    entries: Vec<GitObjectEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct ObjectRef {
    schema: String,
    id: String,
    root: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ClaimMapping {
    source_id: String,
    source_root: String,
    claim_id: String,
    claim_root: String,
    path: String,
    standing: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ProposalMapping {
    source_id: String,
    source_root: String,
    proposal_id: String,
    proposal_root: String,
    claim_id: String,
    claim_root: String,
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ArchivedObject {
    path: String,
    schema: Option<String>,
    id: Option<String>,
    root: String,
    classification: String,
}

#[derive(Debug, Clone, Serialize)]
struct EquivalenceReport {
    schema: &'static str,
    frontier_id: String,
    predecessor_scientific_state_root: String,
    source_claim_count: usize,
    imported_claim_count: usize,
    source_semantics_root: String,
    imported_semantics_root: String,
    source_relation_count: usize,
    imported_relation_count: usize,
    standing: &'static str,
    equivalent: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateManifest {
    schema: &'static str,
    frontier_id: String,
    predecessor_commit: String,
    claims: Vec<ClaimMapping>,
    pending_claims: Vec<ObjectRef>,
    proposals: Vec<ProposalMapping>,
    retained_current_objects: Vec<ObjectRef>,
    imported_claim_set_root: String,
    retained_current_object_set_root: String,
    equivalence_report_root: String,
    epoch_id: String,
    epoch_root: String,
    repository_root: String,
    profile_root: String,
    authority_keyset_root: String,
    authority_policy_root: String,
}

#[derive(Debug, Clone)]
struct ConvertedArtifact {
    reference: ObjectRef,
    record: CurrentArtifactRecordV1,
}

#[derive(Debug, Clone, Serialize)]
struct ArchiveIndex {
    schema: &'static str,
    frontier_id: String,
    predecessor_commit: String,
    objects: Vec<ArchivedObject>,
}

#[derive(Debug, Clone, Serialize)]
struct RepositoryUpgradePlan {
    schema: &'static str,
    ok: bool,
    command: &'static str,
    frontier: String,
    frontier_id: String,
    target: &'static str,
    predecessor_remote: String,
    predecessor_tag: String,
    predecessor_commit: String,
    predecessor_tree: String,
    predecessor_roots: PredecessorRoots,
    profile_root: String,
    epoch_id: String,
    epoch_root: String,
    repository_root: String,
    authority_keyset_root: String,
    authority_policy_root: String,
    authority_key_id: String,
    git_object_manifest_path: String,
    git_object_manifest_root: String,
    archive_bundle_path: String,
    archive_bundle_sha256: String,
    candidate_manifest_path: String,
    candidate_manifest_root: String,
    archived_object_index_path: String,
    archived_object_index_root: String,
    imported_claim_set_root: String,
    retained_current_object_set_root: String,
    equivalence_report_root: String,
    counts: Value,
    reason: String,
    writes_frontier_now: bool,
    next_command: String,
    plan_root: String,
}

pub(crate) fn cmd_repository_upgrade(
    frontier: &Path,
    target: &str,
    archive_dir: &Path,
    reason: &str,
    confirm_root: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("repository upgrade", json_out);
    if target != "current" {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            "repository upgrade target must be `current`",
            None,
        );
    }
    let plan = prepare_repository_upgrade(frontier, archive_dir, reason)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if let Some(confirm_root) = confirm_root {
        if confirm_root != plan.plan_root {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Domain,
                "repository upgrade confirmation root does not match the rederived plan",
                Some("rerun without --confirm-root and inspect the new exact plan"),
            );
        }
        let result =
            apply_repository_upgrade(&plan).unwrap_or_else(|error| crate::cli::fail_return(&error));
        if json_out {
            crate::cli::print_json(&result);
        } else {
            println!("repository epoch applied");
            println!("  frontier: {}", result["frontier_id"]);
            println!("  commit: {}", result["commit"]);
            println!("  authority record: {}", result["authority_record_root"]);
            println!("  predecessor tag: {}", result["predecessor_tag"]);
        }
        return;
    }
    if json_out {
        crate::cli::print_json(&plan);
    } else {
        println!("repository upgrade plan");
        println!("  frontier: {}", plan.frontier_id);
        println!("  predecessor: {}", plan.predecessor_commit);
        println!("  imported Claims: {}", plan.counts["imported_claims"]);
        println!("  archived objects: {}", plan.counts["archived_objects"]);
        println!("  plan root: {}", plan.plan_root);
        println!("  writes Frontier now: no");
        println!("  next: {}", plan.next_command);
    }
}

fn prepare_repository_upgrade(
    frontier: &Path,
    archive_dir: &Path,
    reason: &str,
) -> Result<RepositoryUpgradePlan, String> {
    require_reason(reason)?;
    let frontier = frontier
        .canonicalize()
        .map_err(|error| format!("resolve Frontier {}: {error}", frontier.display()))?;
    require_clean_synced_main(&frontier)?;
    verify_recovery_barrier(&frontier)?;

    let project = vela_protocol::repo::load_from_path(&frontier)?;
    let lock = match vela_protocol::frontier_repo::read_repository_lock(&frontier)? {
        Some(vela_protocol::frontier_repo::FrontierLockFile::V1(lock)) => lock,
        Some(vela_protocol::frontier_repo::FrontierLockFile::LegacyV0_1(_)) => {
            return Err(
                "repository epoch requires a Profile v1 vela.lock; migrate the legacy lock first"
                    .into(),
            );
        }
        None => return Err("Profile v1 Frontier is missing vela.lock".into()),
    };
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])?;
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])?;
    let remote = canonical_github_remote(&git_text(&frontier, &["remote", "get-url", "origin"])?)?;
    let tag = format!("pre-current-epoch/{}", &commit[..12]);

    let authority = crate::cli::load_repository_authority(&frontier, &project)?
        .ok_or_else(|| "repository epoch requires established repository authority".to_string())?;
    let authority_head = authority
        .verification
        .final_authority_record_root
        .clone()
        .ok_or_else(|| "repository authority has no final record root".to_string())?;
    let (authority_key_id, _) = crate::cli::active_repository_key(&authority)?;
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let local = crate::cli::local_session(&observed_at)?;
    let (authority_policy, authority_policy_input) =
        crate::cli::fresh_authority_policy(&frontier, &project, &local.principal_id, &observed_at)?;
    let authority_policy_material =
        vela_authority::CedarPolicyMaterial::from_evaluation(&authority_policy_input);
    authority_policy_material.validate_against(&authority_policy)?;
    let authority_keyset = authority.history.authority_keyset.clone();
    authority_keyset.validate()?;
    let authority_keyset_root = authority_keyset.root()?;
    let authority_policy_root = authority_policy.root()?;
    let (profile_bytes, profile_root) = current_profile(&frontier)?;

    let object_manifest = git_object_manifest(&frontier, &commit, &tree)?;
    let object_manifest_bytes = vela_protocol::canonical::to_canonical_bytes(&object_manifest)?;
    let object_manifest_root = root_bytes(&object_manifest_bytes);

    let converted_artifacts = convert_artifacts(&project.artifacts, &commit)?;
    let artifact_evidence = artifact_evidence_by_finding(&project.artifacts, &converted_artifacts)?;
    let (claims, claim_records, semantic_root, source_relation_count) =
        convert_accepted_claims(&project.findings, &artifact_evidence, &commit)?;
    let (pending_claims, pending_claim_records, proposals, proposal_records) =
        convert_current_pending_proposals(&project.proposals, &commit)?;
    let mut retained_current_objects = retained_current_objects(&frontier)?;
    retained_current_objects.extend(
        converted_artifacts
            .iter()
            .map(|artifact| artifact.reference.clone()),
    );
    retained_current_objects.sort_by(|left, right| left.path.cmp(&right.path));
    if retained_current_objects
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err("current object conversion produced a duplicate path".into());
    }

    let imported_claim_set_root = root_canonical(&json!({
        "schema": "vela.claim-standing-set.v1",
        "claims": claims.iter().map(|mapping| json!({
            "claim_id": mapping.claim_id,
            "claim_root": mapping.claim_root,
            "standing": mapping.standing,
        })).collect::<Vec<_>>(),
    }))?;
    let retained_current_object_set_root = root_canonical(&json!({
        "schema": "vela.current-object-set.v1",
        "objects": retained_current_objects,
        "pending_claims": pending_claims,
        "proposals": proposals,
    }))?;
    let imported_semantics_root = semantic_projection_root(&claim_records)?;
    let imported_relation_count = claim_records
        .iter()
        .map(|record| record.relations.len())
        .sum();
    let equivalence = EquivalenceReport {
        schema: EQUIVALENCE_SCHEMA,
        frontier_id: project.frontier_id(),
        predecessor_scientific_state_root: lock.scientific_state_root.clone(),
        source_claim_count: project.findings.len(),
        imported_claim_count: claim_records.len(),
        source_semantics_root: semantic_root.clone(),
        imported_semantics_root: imported_semantics_root.clone(),
        source_relation_count,
        imported_relation_count,
        standing: "accepted",
        equivalent: semantic_root == imported_semantics_root
            && project.findings.len() == claim_records.len()
            && source_relation_count == imported_relation_count,
    };
    if !equivalence.equivalent {
        return Err("Claim migration semantic equivalence failed".into());
    }
    let equivalence_report_root = root_canonical(&equivalence)?;

    let legacy_paths = object_manifest
        .entries
        .iter()
        .filter(|entry| is_predecessor_archive_path(&entry.path))
        .map(|entry| archived_object(&frontier, entry))
        .collect::<Result<Vec<_>, String>>()?;
    let archive_index = ArchiveIndex {
        schema: ARCHIVE_INDEX_SCHEMA,
        frontier_id: project.frontier_id(),
        predecessor_commit: commit.clone(),
        objects: legacy_paths,
    };
    let archive_index_bytes = vela_protocol::canonical::to_canonical_bytes(&archive_index)?;
    let archive_index_root = root_bytes(&archive_index_bytes);

    fs::create_dir_all(archive_dir).map_err(|error| {
        format!(
            "create archive directory {}: {error}",
            archive_dir.display()
        )
    })?;
    let stem = format!(
        "{}-{}",
        frontier
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("frontier"),
        &commit[..12]
    );
    let object_manifest_path = archive_dir.join(format!("{stem}.git-objects.json"));
    let candidate_path = archive_dir.join(format!("{stem}.candidate.json"));
    let archive_index_path = archive_dir.join(format!("{stem}.archived-objects.json"));
    let equivalence_path = archive_dir.join(format!("{stem}.equivalence.json"));
    let bundle_path = archive_dir.join(format!("{stem}.bundle"));
    create_or_verify_bundle(&frontier, &bundle_path)?;
    let bundle_root = root_bytes(
        &fs::read(&bundle_path)
            .map_err(|error| format!("read archive bundle {}: {error}", bundle_path.display()))?,
    );

    let actor_registry_root = root_bytes(
        &fs::read(frontier.join(".vela/actors.json"))
            .map_err(|error| format!("read .vela/actors.json: {error}"))?,
    );
    let artifact_registry_root = root_canonical(&project.artifacts)?;
    let predecessor_roots = PredecessorRoots {
        event_log: lock.event_log_root.clone(),
        scientific_state: lock.scientific_state_root.clone(),
        compatibility_snapshot: lock.legacy_snapshot_root.clone(),
        proposal_state: lock.proposal_root.clone(),
        actor_registry: actor_registry_root,
        artifact_registry: artifact_registry_root,
        authority_head,
        authority_event_log: authority.verification.final_event_log_root.clone(),
    };
    let epoch = RepositoryEpochV1::build(
        project.frontier_id(),
        1,
        remote.clone(),
        tag.clone(),
        commit.clone(),
        tree.clone(),
        "vela.frontier-profile.v1".into(),
        predecessor_roots.clone(),
        object_manifest_root.clone(),
        bundle_root.clone(),
        imported_claim_set_root.clone(),
        retained_current_object_set_root.clone(),
        archive_index_root.clone(),
        equivalence_report_root.clone(),
        reason.into(),
    )?;
    let epoch_root = epoch.canonical_root()?;

    let repository = current_repository(
        &project.frontier_id(),
        &profile_root,
        &epoch,
        &epoch_root,
        &claims,
        &pending_claims,
        &proposals,
        &retained_current_objects,
        &authority_keyset_root,
        &authority_policy_root,
    )?;
    let repository_root = repository.canonical_root()?;
    let candidate = CandidateManifest {
        schema: CANDIDATE_MANIFEST_SCHEMA,
        frontier_id: project.frontier_id(),
        predecessor_commit: commit.clone(),
        claims,
        pending_claims,
        proposals,
        retained_current_objects,
        imported_claim_set_root: imported_claim_set_root.clone(),
        retained_current_object_set_root: retained_current_object_set_root.clone(),
        equivalence_report_root: equivalence_report_root.clone(),
        epoch_id: epoch.epoch_id.clone(),
        epoch_root: epoch_root.clone(),
        repository_root: repository_root.clone(),
        profile_root: profile_root.clone(),
        authority_keyset_root: authority_keyset_root.clone(),
        authority_policy_root: authority_policy_root.clone(),
    };
    let candidate_bytes = vela_protocol::canonical::to_canonical_bytes(&candidate)?;
    let candidate_root = root_bytes(&candidate_bytes);

    write_exact(&object_manifest_path, &object_manifest_bytes)?;
    write_exact(&candidate_path, &candidate_bytes)?;
    write_exact(&archive_index_path, &archive_index_bytes)?;
    write_exact(
        &equivalence_path,
        &vela_protocol::canonical::to_canonical_bytes(&equivalence)?,
    )?;
    let records_path = archive_dir.join(format!("{stem}.records"));
    if records_path.exists() {
        fs::remove_dir_all(&records_path).map_err(|error| {
            format!(
                "replace current candidate directory {}: {error}",
                records_path.display()
            )
        })?;
    }
    write_candidate_records(
        &records_path,
        &claim_records,
        &pending_claim_records,
        &proposal_records,
        &converted_artifacts,
    )?;
    copy_retained_current_objects(
        &frontier,
        &records_path,
        &candidate.retained_current_objects,
    )?;
    write_current_candidate(
        &records_path,
        &profile_bytes,
        &epoch,
        &repository,
        &authority_keyset,
        &authority_policy,
        &authority_policy_material,
    )?;
    verify_current_repository_at(&records_path, false)?;
    let archived_count = archive_index.objects.len();
    let next = format!(
        "vela repository upgrade {} --to current --archive-dir {} --reason {} --confirm-root <plan-root> --json",
        frontier.display(),
        archive_dir.display(),
        shell_quote(reason),
    );
    let mut plan = RepositoryUpgradePlan {
        schema: PLAN_SCHEMA,
        ok: true,
        command: "repository upgrade",
        frontier: frontier.display().to_string(),
        frontier_id: project.frontier_id(),
        target: "current",
        predecessor_remote: remote,
        predecessor_tag: tag,
        predecessor_commit: commit,
        predecessor_tree: tree,
        predecessor_roots,
        profile_root,
        epoch_id: epoch.epoch_id,
        epoch_root,
        repository_root,
        authority_keyset_root,
        authority_policy_root,
        authority_key_id,
        git_object_manifest_path: object_manifest_path.display().to_string(),
        git_object_manifest_root: object_manifest_root,
        archive_bundle_path: bundle_path.display().to_string(),
        archive_bundle_sha256: bundle_root,
        candidate_manifest_path: candidate_path.display().to_string(),
        candidate_manifest_root: candidate_root,
        archived_object_index_path: archive_index_path.display().to_string(),
        archived_object_index_root: archive_index_root,
        imported_claim_set_root,
        retained_current_object_set_root,
        equivalence_report_root,
        counts: json!({
            "imported_claims": project.findings.len(),
            "pending_current_claims": pending_claim_records.len(),
            "current_proposals": proposal_records.len(),
            "retained_current_objects": candidate.retained_current_objects.len(),
            "archived_objects": archived_count,
            "tracked_objects": object_manifest.entries.len(),
        }),
        reason: reason.to_string(),
        writes_frontier_now: false,
        next_command: next,
        plan_root: String::new(),
    };
    plan.plan_root = plan_root(&plan)?;
    plan.next_command = plan.next_command.replace("<plan-root>", &plan.plan_root);
    verify_recovery_barrier(&frontier)?;
    require_clean_synced_main(&frontier)?;
    Ok(plan)
}

fn apply_repository_upgrade(plan: &RepositoryUpgradePlan) -> Result<Value, String> {
    let frontier = PathBuf::from(&plan.frontier)
        .canonicalize()
        .map_err(|error| format!("resolve Frontier {}: {error}", plan.frontier))?;
    require_clean_synced_main(&frontier)?;
    if git_text(&frontier, &["rev-parse", "HEAD^{commit}"])? != plan.predecessor_commit {
        return Err("repository upgrade predecessor changed after confirmation".into());
    }

    let object_manifest: GitObjectManifest =
        serde_json::from_slice(&fs::read(&plan.git_object_manifest_path).map_err(|error| {
            format!(
                "read Git object manifest {}: {error}",
                plan.git_object_manifest_path
            )
        })?)
        .map_err(|error| format!("parse Git object manifest: {error}"))?;
    if object_manifest.commit != plan.predecessor_commit
        || object_manifest.tree != plan.predecessor_tree
    {
        return Err("Git object manifest no longer matches the confirmed predecessor".into());
    }

    let candidate_root = candidate_records_path(&plan.candidate_manifest_path)?;
    verify_current_repository_at(&candidate_root, false)?;
    let archive_root = PathBuf::from(&plan.candidate_manifest_path)
        .parent()
        .ok_or_else(|| "candidate manifest has no archive directory".to_string())?
        .to_path_buf();
    let worktree = archive_root.join("worktrees").join(format!(
        "{}-{}",
        frontier
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("frontier"),
        plan.plan_root
            .trim_start_matches("sha256:")
            .get(..16)
            .unwrap_or("current-epoch")
    ));
    if worktree.exists() {
        return Err(format!(
            "preserved repository-upgrade worktree already exists at {}; inspect or remove it before retrying",
            worktree.display()
        ));
    }
    if let Some(parent) = worktree.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    git_success(
        &frontier,
        &[
            "worktree",
            "add",
            "--detach",
            &worktree.display().to_string(),
            &plan.predecessor_commit,
        ],
        "create isolated repository-upgrade worktree",
    )?;

    let application = apply_repository_upgrade_in_worktree(
        plan,
        &frontier,
        &worktree,
        &candidate_root,
        &object_manifest,
    );
    let result = match application {
        Ok(result) => result,
        Err(error) => {
            return Err(format!(
                "{error}\nThe isolated worktree is preserved at {} for exact recovery.",
                worktree.display()
            ));
        }
    };

    git_success(
        &frontier,
        &["pull", "--ff-only", "origin", "main"],
        "fast-forward source Frontier after epoch push",
    )?;
    let clean_clone = archive_root.join("verification").join(format!(
        "{}-{}",
        plan.frontier_id,
        result["commit"].as_str().unwrap_or("current")
    ));
    if clean_clone.exists() {
        fs::remove_dir_all(&clean_clone)
            .map_err(|error| format!("remove stale verification clone: {error}"))?;
    }
    if let Some(parent) = clean_clone.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    git_success(
        &archive_root,
        &[
            "clone",
            "--no-local",
            &plan.predecessor_remote,
            &clean_clone.display().to_string(),
        ],
        "clone applied repository epoch",
    )?;
    verify_current_repository_at(&clean_clone, true)?;
    require_git_clean(&clean_clone)?;
    fs::remove_dir_all(&clean_clone)
        .map_err(|error| format!("remove verified clean clone: {error}"))?;
    git_success(
        &frontier,
        &[
            "worktree",
            "remove",
            "--force",
            &worktree.display().to_string(),
        ],
        "remove verified repository-upgrade worktree",
    )?;
    Ok(result)
}

fn apply_repository_upgrade_in_worktree(
    plan: &RepositoryUpgradePlan,
    source_frontier: &Path,
    worktree: &Path,
    candidate_root: &Path,
    object_manifest: &GitObjectManifest,
) -> Result<Value, String> {
    let project = vela_protocol::repo::load_from_path(worktree)?;
    if project.frontier_id() != plan.frontier_id {
        return Err("isolated worktree names the wrong Frontier".into());
    }
    let authority = crate::cli::load_repository_authority(worktree, &project)?
        .ok_or_else(|| "predecessor repository authority is missing".to_string())?;
    let (_, public_key) = crate::cli::active_repository_key(&authority)?;
    let authority_keyset = authority.history.authority_keyset.clone();
    if authority_keyset.root()? != plan.authority_keyset_root {
        return Err("confirmed authority keyset root changed".into());
    }

    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut authentication = crate::cli::local_session(&recorded_at)?;
    let principal = PrincipalSnapshotV1 {
        principal_id: authentication.principal_id.clone(),
        principal_class: PrincipalClass::Human,
        display_name: Some("Repository administrator".into()),
        affiliation: None,
        account_links: vec![authentication.principal_id.clone()],
    };
    let (policy_bundle, authorization_input) = crate::cli::fresh_authority_policy(
        worktree,
        &project,
        &principal.principal_id,
        &recorded_at,
    )?;
    if policy_bundle.root()? != plan.authority_policy_root {
        return Err("confirmed authority policy root changed".into());
    }

    let candidate_files = candidate_file_map(candidate_root)?;
    let mut object_drafts = Vec::new();
    let mut derived_drafts = Vec::new();
    for entry in object_manifest
        .entries
        .iter()
        .filter(|entry| is_predecessor_archive_path(&entry.path))
    {
        if let Some(candidate) = candidate_files.get(&entry.path) {
            let current = fs::read(worktree.join(&entry.path))
                .map_err(|error| format!("read predecessor {}: {error}", entry.path))?;
            if &current == candidate {
                continue;
            }
        }
        if entry.path == "frontier.json" || entry.path == "vela.lock" {
            derived_drafts.push(AuthorityDerivedDraft {
                path: entry.path.clone(),
                postimage: None,
            });
        } else {
            object_drafts.push(epoch_object_draft(&entry.path, None)?);
        }
    }
    for (path, bytes) in &candidate_files {
        if path.starts_with(".vela/authority/") {
            continue;
        }
        let current = fs::read(worktree.join(path)).ok();
        if current.as_deref() == Some(bytes.as_slice()) {
            continue;
        }
        object_drafts.push(epoch_object_draft(path, Some(bytes.clone()))?);
    }
    object_drafts.sort_by(|left, right| left.path.cmp(&right.path));
    if object_drafts
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err("repository epoch derives duplicate canonical object drafts".into());
    }

    let actor_registry_bytes = fs::read(worktree.join(".vela/actors.json"))
        .map_err(|error| format!("read predecessor actor registry: {error}"))?;
    let initialization = AuthorityInitializationV1 {
        schema: AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
        frontier_id: plan.frontier_id.clone(),
        initial_event_log_root: plan.predecessor_roots.event_log.clone(),
        initial_actor_registry_root: plan.predecessor_roots.actor_registry.clone(),
        new_authority_keyset_root: plan.authority_keyset_root.clone(),
        new_policy_bundle_root: plan.authority_policy_root.clone(),
        new_principal_id: principal.principal_id.clone(),
        minimum_writer_version: env!("CARGO_PKG_VERSION").into(),
        reason: plan.reason.clone(),
    };
    initialization.validate()?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve Vela executable: {error}"))?;
    let binary_sha256 = execution_binary_sha256(&executable)?;
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(worktree)?;
    let barrier = FrontierTxn::acquire_repository_authority_write_barrier(worktree, &journal_dir)
        .map_err(|error| error.to_string())?;
    let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(
        plan.authority_key_id.clone(),
        &public_key,
    )?;
    let transaction = execute_authority_transaction(
        barrier,
        worktree,
        AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                frontier_id: plan.frontier_id.clone(),
                legacy_events: project.events.clone(),
                legacy_actor_registry_bytes: actor_registry_bytes,
                legacy_active_policy_head_root: NULL_HASH.into(),
                legacy_policy_store_manifest_root: NULL_HASH.into(),
                authority_keyset,
                policy_bundle,
                retained_authority_keysets: Vec::new(),
                retained_policy_bundles: Vec::new(),
                authority_events: Vec::new(),
                authority_envelopes: Vec::new(),
            },
            intent_digest: plan.plan_root.clone(),
            principal: principal.clone(),
            authentication_request: AuthenticationRequest {
                principal_id: principal.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: principal.principal_id.clone(),
                role: "frontier_administrator".into(),
                action: AUTHORITY_INITIALIZE_ACTION.into(),
                reason: plan.reason.clone(),
                approved_at: recorded_at.clone(),
                intent_digest: plan.plan_root.clone(),
            }],
            event_drafts: vec![AuthorityEventDraft {
                kind: EventKind::Other(AUTHORITY_INITIALIZED_EVENT_KIND.into()),
                target: StateTarget {
                    r#type: "frontier".into(),
                    id: plan.frontier_id.clone(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: principal.principal_id.clone(),
                },
                timestamp: recorded_at.clone(),
                reason: plan.reason.clone(),
                before_hash: NULL_HASH.into(),
                after_hash: NULL_HASH.into(),
                payload: serde_json::to_value(&initialization)
                    .map_err(|error| format!("encode epoch initialization: {error}"))?,
                caveats: vec![
                    "Repository authority authenticates the epoch transition; it does not establish scientific truth."
                        .into(),
                    "Predecessor signatures authenticate only their exact archived schemas and bytes."
                        .into(),
                ],
            }],
            object_drafts,
            derived_drafts,
            retire_legacy_history: true,
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set: Vec::new(),
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at,
        },
        &mut authentication,
        &mut signer,
    )
    .map_err(|error| error.to_string())?;
    verify_current_repository_at(worktree, true)?;

    git_success(worktree, &["add", "--all"], "stage repository epoch")?;
    git_success(
        worktree,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "Migrate to the current Vela repository epoch",
        ],
        "commit repository epoch",
    )?;
    let commit = git_text(worktree, &["rev-parse", "HEAD^{commit}"])?;
    if git_text(worktree, &["rev-parse", "HEAD^1^{commit}"])? != plan.predecessor_commit {
        return Err("repository epoch commit is not the direct predecessor child".into());
    }
    match git_text(
        source_frontier,
        &["rev-parse", &format!("refs/tags/{}", plan.predecessor_tag)],
    ) {
        Ok(existing) if existing != plan.predecessor_commit => {
            return Err("predecessor tag already names a different commit".into());
        }
        Ok(_) => {}
        Err(_) => {
            git_success(
                worktree,
                &["tag", &plan.predecessor_tag, &plan.predecessor_commit],
                "create predecessor tag",
            )?;
        }
    }
    git_success(
        worktree,
        &[
            "push",
            "--atomic",
            "origin",
            "HEAD:refs/heads/main",
            &format!("refs/tags/{}", plan.predecessor_tag),
        ],
        "publish repository epoch and predecessor tag",
    )?;
    require_git_clean(worktree)?;
    Ok(json!({
        "schema": "vela.repository-upgrade-result.v1",
        "ok": true,
        "command": "repository upgrade",
        "frontier_id": plan.frontier_id,
        "predecessor_commit": plan.predecessor_commit,
        "predecessor_tag": plan.predecessor_tag,
        "epoch_id": plan.epoch_id,
        "epoch_root": plan.epoch_root,
        "repository_root": plan.repository_root,
        "plan_root": plan.plan_root,
        "authority_event_ids": transaction.event_ids,
        "authority_record_id": transaction.authority_record_id,
        "authority_record_root": transaction.authority_record_root,
        "commit": commit,
        "writes_now": true
    }))
}

fn candidate_records_path(candidate_manifest: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(candidate_manifest);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "candidate manifest path has no UTF-8 filename".to_string())?;
    let stem = name
        .strip_suffix(".candidate.json")
        .ok_or_else(|| "candidate manifest filename has the wrong suffix".to_string())?;
    Ok(path.with_file_name(format!("{stem}.records")))
}

fn candidate_file_map(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    for path in files_recursive(root)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "candidate file escaped its root".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        files.insert(
            relative,
            fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?,
        );
    }
    Ok(files)
}

fn epoch_object_draft(
    path: &str,
    postimage: Option<Vec<u8>>,
) -> Result<AuthorityObjectDraft, String> {
    let (object_kind, class) = if path == "frontier.yaml" {
        ("frontier_profile", WriteClass::CanonicalEvidence)
    } else if path.starts_with(".vela/authority/") {
        ("predecessor_authority", WriteClass::Authority)
    } else if path.starts_with(".vela/proposals/")
        || path.starts_with("records/receipts/")
        || path.starts_with("records/review/")
        || path.starts_with("records/decision-evidence/")
        || is_root_receipt(path)
        || (path.starts_with("records/") && !path.starts_with("records/artifacts/"))
    {
        ("repository_record", WriteClass::PublicReview)
    } else if path.starts_with(".vela/") || path.starts_with("records/artifacts/") {
        ("canonical_evidence", WriteClass::CanonicalEvidence)
    } else {
        return Err(format!(
            "repository epoch does not recognize canonical object path {path}"
        ));
    };
    Ok(AuthorityObjectDraft {
        path: path.into(),
        object_kind: object_kind.into(),
        class,
        postimage,
    })
}

fn git_success(root: &Path, args: &[&str], action: &str) -> Result<(), String> {
    let output = crate::git_hardened::output(root, args)?;
    if !output.status.success() {
        return Err(format!(
            "{action}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn require_git_clean(root: &Path) -> Result<(), String> {
    if !git_text(root, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty() {
        return Err(format!(
            "repository {} is not clean after migration",
            root.display()
        ));
    }
    Ok(())
}

fn convert_accepted_claims(
    findings: &[FindingBundle],
    artifact_evidence: &BTreeMap<String, Vec<ClaimEvidenceRef>>,
    predecessor_commit: &str,
) -> Result<(Vec<ClaimMapping>, Vec<ClaimRecordV1>, String, usize), String> {
    let mut base = Vec::with_capacity(findings.len());
    let mut by_source = BTreeMap::new();
    for finding in findings {
        let record = claim_from_finding(
            finding,
            artifact_evidence
                .get(&finding.id)
                .cloned()
                .unwrap_or_default(),
            predecessor_commit,
            Vec::new(),
        )
        .map_err(|error| {
            format!(
                "Finding {} (assertion {}): {error}",
                finding.id,
                serde_json::to_string(&finding.assertion)
                    .unwrap_or_else(|_| "<unserializable>".into())
            )
        })?;
        if by_source
            .insert(finding.id.clone(), record.claim_id.clone())
            .is_some()
        {
            return Err(format!("duplicate historical Finding {}", finding.id));
        }
        base.push((finding, record));
    }
    let mut records = Vec::with_capacity(findings.len());
    let mut mappings = Vec::with_capacity(findings.len());
    let mut relation_count = 0usize;
    for (finding, base_record) in base {
        let relations = finding
            .links
            .iter()
            .map(|link| {
                relation_count += 1;
                let target = by_source.get(&link.target).ok_or_else(|| {
                    format!(
                        "Finding {} has dangling relation target {}",
                        finding.id, link.target
                    )
                })?;
                Ok(ClaimRelation {
                    kind: link.link_type.clone(),
                    target_claim_id: target.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let record = claim_from_finding(
            finding,
            artifact_evidence
                .get(&finding.id)
                .cloned()
                .unwrap_or_default(),
            predecessor_commit,
            relations,
        )
        .map_err(|error| {
            format!(
                "Finding {} (assertion {}): {error}",
                finding.id,
                serde_json::to_string(&finding.assertion)
                    .unwrap_or_else(|_| "<unserializable>".into())
            )
        })?;
        if record.claim_id != base_record.claim_id {
            return Err(format!(
                "Finding {} relation metadata changed its Claim identity",
                finding.id
            ));
        }
        let root = record.canonical_root()?;
        let path = format!(
            "records/claims/sha256/{}.json",
            root.trim_start_matches("sha256:")
        );
        mappings.push(ClaimMapping {
            source_id: finding.id.clone(),
            source_root: source_finding_root(finding)?,
            claim_id: record.claim_id.clone(),
            claim_root: root,
            path,
            standing: "accepted",
        });
        records.push(record);
    }
    mappings.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    records.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let semantic_root = source_semantic_projection_root(findings, &by_source)?;
    Ok((mappings, records, semantic_root, relation_count))
}

fn current_profile(frontier: &Path) -> Result<(Vec<u8>, String), String> {
    let source = fs::read_to_string(frontier.join("frontier.yaml"))
        .map_err(|error| format!("read frontier.yaml: {error}"))?;
    let profile = vela_protocol::frontier_profile::FrontierProfileV1::from_yaml_str(&source)?;
    let profile = CurrentFrontierProfileV2::from_v1(profile)?;
    let profile_root = profile.profile_root()?;
    let bytes = serde_yaml::to_string(&profile)
        .map_err(|error| format!("serialize Frontier Profile v2: {error}"))?
        .into_bytes();
    Ok((bytes, profile_root))
}

#[allow(clippy::too_many_arguments)]
fn current_repository(
    frontier_id: &str,
    profile_root: &str,
    epoch: &RepositoryEpochV1,
    epoch_root: &str,
    claims: &[ClaimMapping],
    pending_claims: &[ObjectRef],
    proposals: &[ProposalMapping],
    objects: &[ObjectRef],
    authority_keyset_root: &str,
    authority_policy_root: &str,
) -> Result<CurrentRepositoryV2, String> {
    let mut accepted_claims = claims
        .iter()
        .map(|claim| ClaimStandingRefV1 {
            claim_id: claim.claim_id.clone(),
            claim_root: claim.claim_root.clone(),
            standing: "accepted".into(),
            path: claim.path.clone(),
        })
        .collect::<Vec<_>>();
    accepted_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut pending_claims = pending_claims
        .iter()
        .map(|claim| ClaimStandingRefV1 {
            claim_id: claim.id.clone(),
            claim_root: claim.root.clone(),
            standing: "pending_review".into(),
            path: claim.path.clone(),
        })
        .collect::<Vec<_>>();
    pending_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut proposal_refs = proposals
        .iter()
        .map(|proposal| RepositoryObjectRefV1 {
            schema: "vela.proposal.v1".into(),
            id: proposal.proposal_id.clone(),
            root: proposal.proposal_root.clone(),
            path: proposal.path.clone(),
        })
        .collect::<Vec<_>>();
    proposal_refs.sort_by(|left, right| left.id.cmp(&right.id));

    let group = |prefix: &str| {
        let mut references = objects
            .iter()
            .filter(|object| object.path.starts_with(prefix))
            .map(repository_object_ref)
            .collect::<Vec<_>>();
        references.sort_by(|left, right| left.id.cmp(&right.id));
        references
    };
    let repository = CurrentRepositoryV2 {
        schema: CURRENT_REPOSITORY_SCHEMA_V2.into(),
        frontier_id: frontier_id.into(),
        profile_root: profile_root.into(),
        epoch_id: epoch.epoch_id.clone(),
        epoch_root: epoch_root.into(),
        accepted_claims,
        pending_claims,
        proposals: proposal_refs,
        submissions: group("records/submissions/sha256/"),
        registrations: group("records/registrations/sha256/"),
        verifications: group("records/verifications/sha256/"),
        artifacts: group("records/artifacts/sha256/"),
        authority_keyset_root: authority_keyset_root.into(),
        authority_policy_root: authority_policy_root.into(),
    };
    repository.verify()?;
    Ok(repository)
}

fn repository_object_ref(object: &ObjectRef) -> RepositoryObjectRefV1 {
    RepositoryObjectRefV1 {
        schema: object.schema.clone(),
        id: object.id.clone(),
        root: object.root.clone(),
        path: object.path.clone(),
    }
}

fn write_current_candidate(
    root: &Path,
    profile_bytes: &[u8],
    epoch: &RepositoryEpochV1,
    repository: &CurrentRepositoryV2,
    keyset: &vela_protocol::authority::AuthorityKeysetV1,
    policy: &vela_protocol::authority::PolicyBundleV1,
    material: &vela_authority::CedarPolicyMaterial,
) -> Result<(), String> {
    write_exact(&root.join("frontier.yaml"), profile_bytes)?;
    write_exact(&root.join(".vela/epoch.json"), &epoch.canonical_bytes()?)?;
    write_exact(
        &root.join(".vela/repository.json"),
        &repository.canonical_bytes()?,
    )?;
    let keyset_root = keyset.root()?;
    write_exact(
        &root.join(format!(
            ".vela/authority/keysets/{}.json",
            keyset_root.trim_start_matches("sha256:")
        )),
        &vela_protocol::canonical::to_canonical_bytes(keyset)?,
    )?;
    let policy_root = policy.root()?;
    write_exact(
        &root.join(format!(
            ".vela/authority/policies/{}.json",
            policy_root.trim_start_matches("sha256:")
        )),
        &vela_protocol::canonical::to_canonical_bytes(policy)?,
    )?;
    let paths = crate::authority_transaction::authority_policy_material_paths(policy)
        .map_err(|error| error.to_string())?;
    write_exact(&root.join(&paths[0]), material.schema.as_bytes())?;
    write_exact(&root.join(&paths[1]), material.policies.as_bytes())?;
    write_exact(
        &root.join(&paths[2]),
        &vela_protocol::canonical::to_canonical_bytes(&material.entities)?,
    )?;
    Ok(())
}

/// Verify a current-only repository without consulting Era-0 events, locks, or
/// generated snapshots. The optional authority-record requirement is enabled
/// for an applied epoch and disabled only for the key-free candidate preview.
fn verify_current_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV2, String> {
    let profile_source = fs::read_to_string(root.join("frontier.yaml"))
        .map_err(|error| format!("read current frontier.yaml: {error}"))?;
    let profile = CurrentFrontierProfileV2::from_yaml_str(&profile_source)?;
    let profile_root = profile.profile_root()?;

    let epoch_bytes = fs::read(root.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryEpochV1::parse(&epoch_bytes)?;
    if epoch.canonical_bytes()? != epoch_bytes {
        return Err("current repository epoch bytes are not canonical JSON".into());
    }
    let epoch_root = epoch.canonical_root()?;

    let repository_bytes = fs::read(root.join(".vela/repository.json"))
        .map_err(|error| format!("read current repository manifest: {error}"))?;
    let repository = CurrentRepositoryV2::parse(&repository_bytes)?;
    if repository.frontier_id != profile.frontier_id
        || repository.frontier_id != epoch.frontier_id
        || repository.profile_root != profile_root
        || repository.epoch_id != epoch.epoch_id
        || repository.epoch_root != epoch_root
    {
        return Err(
            "current Profile, repository manifest, and epoch do not bind the same identity".into(),
        );
    }

    for reference in &repository.accepted_claims {
        let bytes = read_rooted_object(root, &reference.path, &reference.claim_root)?;
        let claim = ClaimRecordV1::parse(&bytes)?;
        if claim.canonical_bytes()? != bytes || claim.claim_id != reference.claim_id {
            return Err(format!(
                "{} does not contain the declared canonical Claim",
                reference.path
            ));
        }
    }
    for reference in &repository.pending_claims {
        let bytes = read_rooted_object(root, &reference.path, &reference.claim_root)?;
        let claim = ClaimRecordV1::parse(&bytes)?;
        if claim.canonical_bytes()? != bytes || claim.claim_id != reference.claim_id {
            return Err(format!(
                "{} does not contain the declared canonical pending Claim",
                reference.path
            ));
        }
    }
    for reference in &repository.proposals {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&bytes)?;
        if proposal.canonical_bytes()? != bytes || proposal.proposal_id != reference.id {
            return Err(format!(
                "{} does not contain the declared canonical Proposal",
                reference.path
            ));
        }
    }
    for reference in &repository.submissions {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let submission = vela_protocol::submission_v1::SubmissionV1::parse(&bytes)?;
        if submission.canonical_bytes()? != bytes || submission.submission_id != reference.id {
            return Err(format!(
                "{} does not contain the declared canonical Submission",
                reference.path
            ));
        }
    }
    for reference in &repository.registrations {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let registration = vela_protocol::registration_record::RegistrationRecordV1::parse(&bytes)?;
        if registration.canonical_bytes()? != bytes
            || registration.registration_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Registration Record",
                reference.path
            ));
        }
    }
    for reference in &repository.verifications {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let verification = vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)?;
        if verification.canonical_bytes()? != bytes
            || verification.verification_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Verification Record",
                reference.path
            ));
        }
    }
    for reference in &repository.artifacts {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        if reference.schema == "content-addressed-artifact" {
            continue;
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{} is not JSON: {error}", reference.path))?;
        if value.get("schema").and_then(Value::as_str) != Some(reference.schema.as_str()) {
            return Err(format!("{} has the wrong Artifact schema", reference.path));
        }
        if reference.schema == CURRENT_ARTIFACT_RECORD_SCHEMA_V1 {
            let artifact = CurrentArtifactRecordV1::parse(&bytes)?;
            if artifact.artifact_id != reference.id {
                return Err(format!(
                    "{} does not contain the declared canonical Artifact",
                    reference.path
                ));
            }
        }
    }

    let keyset_path = root.join(format!(
        ".vela/authority/keysets/{}.json",
        repository
            .authority_keyset_root
            .trim_start_matches("sha256:")
    ));
    let keyset_bytes = fs::read(&keyset_path)
        .map_err(|error| format!("read current authority keyset: {error}"))?;
    let keyset: vela_protocol::authority::AuthorityKeysetV1 = serde_json::from_slice(&keyset_bytes)
        .map_err(|error| format!("parse current authority keyset: {error}"))?;
    keyset.validate()?;
    if keyset.frontier_id != repository.frontier_id
        || keyset.root()? != repository.authority_keyset_root
        || vela_protocol::canonical::to_canonical_bytes(&keyset)? != keyset_bytes
    {
        return Err("current repository authority keyset binding is invalid".into());
    }

    let policy_path = root.join(format!(
        ".vela/authority/policies/{}.json",
        repository
            .authority_policy_root
            .trim_start_matches("sha256:")
    ));
    let policy_bytes = fs::read(&policy_path)
        .map_err(|error| format!("read current authority policy: {error}"))?;
    let policy: vela_protocol::authority::PolicyBundleV1 = serde_json::from_slice(&policy_bytes)
        .map_err(|error| format!("parse current authority policy: {error}"))?;
    policy.validate()?;
    if policy.frontier_id != repository.frontier_id
        || policy.root()? != repository.authority_policy_root
        || vela_protocol::canonical::to_canonical_bytes(&policy)? != policy_bytes
    {
        return Err("current repository policy binding is invalid".into());
    }
    let material_paths = crate::authority_transaction::authority_policy_material_paths(&policy)
        .map_err(|error| error.to_string())?;
    let material = vela_authority::CedarPolicyMaterial {
        schema: fs::read_to_string(root.join(&material_paths[0]))
            .map_err(|error| format!("read current Cedar schema: {error}"))?,
        policies: fs::read_to_string(root.join(&material_paths[1]))
            .map_err(|error| format!("read current Cedar policies: {error}"))?,
        entities: serde_json::from_slice(
            &fs::read(root.join(&material_paths[2]))
                .map_err(|error| format!("read current Cedar entities: {error}"))?,
        )
        .map_err(|error| format!("parse current Cedar entities: {error}"))?,
    };
    material.validate_against(&policy)?;

    for path in files_recursive(root)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "current repository path escaped its root".to_string())?
            .to_string_lossy();
        if is_retired_current_path(&relative) {
            return Err(format!(
                "current repository retains retired protocol path {relative}"
            ));
        }
    }
    if require_authority_record {
        verify_current_epoch_authority(root, &repository, &epoch, &keyset)?;
    }
    Ok(repository)
}

fn read_rooted_object(root: &Path, path: &str, expected_root: &str) -> Result<Vec<u8>, String> {
    let bytes = fs::read(root.join(path))
        .map_err(|error| format!("read current object {path}: {error}"))?;
    if root_bytes(&bytes) != expected_root {
        return Err(format!(
            "current object {path} does not match its declared root"
        ));
    }
    let expected_name = expected_root.trim_start_matches("sha256:");
    if Path::new(path).file_stem().and_then(|value| value.to_str()) != Some(expected_name) {
        return Err(format!(
            "current object {path} filename does not match its declared root"
        ));
    }
    Ok(bytes)
}

fn verify_current_epoch_authority(
    root: &Path,
    repository: &CurrentRepositoryV2,
    epoch: &RepositoryEpochV1,
    keyset: &vela_protocol::authority::AuthorityKeysetV1,
) -> Result<(), String> {
    let event_path = only_regular_file(&root.join(".vela/authority/events"), ".json")?;
    let event_bytes = fs::read(&event_path)
        .map_err(|error| format!("read current epoch authority event: {error}"))?;
    let event: vela_protocol::authority::AuthorityEventV1 = serde_json::from_slice(&event_bytes)
        .map_err(|error| format!("parse current epoch authority event: {error}"))?;
    event.validate()?;
    if vela_protocol::canonical::to_canonical_bytes(&event)? != event_bytes
        || event_path.file_stem().and_then(|value| value.to_str()) != Some(event.id.as_str())
        || event.content.kind.as_str() != AUTHORITY_INITIALIZED_EVENT_KIND
        || event.content.target.r#type != "frontier"
        || event.content.target.id != repository.frontier_id
        || event.content.actor.r#type != "human"
        || event.content.actor.id != event.content.principal_id
        || event.content.before_hash != NULL_HASH
        || event.content.after_hash != NULL_HASH
    {
        return Err("current epoch authority event has an invalid canonical shape".into());
    }
    let initialization: AuthorityInitializationV1 =
        serde_json::from_value(event.content.payload.clone())
            .map_err(|error| format!("parse current epoch initialization payload: {error}"))?;
    initialization.validate()?;
    if initialization.frontier_id != repository.frontier_id
        || initialization.initial_event_log_root != epoch.predecessor_roots.event_log
        || initialization.initial_actor_registry_root != epoch.predecessor_roots.actor_registry
        || initialization.new_authority_keyset_root != repository.authority_keyset_root
        || initialization.new_policy_bundle_root != repository.authority_policy_root
        || initialization.new_principal_id != event.content.principal_id
        || initialization.reason != epoch.reason
    {
        return Err(
            "current epoch authority initialization does not bind the exact predecessor and current roots"
                .into(),
        );
    }

    let record_path = only_regular_file(&root.join(".vela/authority/records"), ".dsse.json")?;
    let envelope_bytes = fs::read(&record_path)
        .map_err(|error| format!("read current epoch authority record: {error}"))?;
    let envelope: vela_protocol::authority::AuthorityEnvelopeV1 =
        serde_json::from_slice(&envelope_bytes)
            .map_err(|error| format!("parse current epoch authority envelope: {error}"))?;
    if vela_protocol::canonical::to_canonical_bytes(&envelope)? != envelope_bytes {
        return Err("current epoch authority envelope is not canonical JSON".into());
    }
    let verified = vela_protocol::authority::verify_authority_envelope(
        &envelope,
        keyset,
        &repository.frontier_id,
        1,
        None,
    )?;
    let record = &verified.record;
    if record_path.file_name().and_then(|value| value.to_str())
        != Some(format!("{}.dsse.json", record.record_id).as_str())
        || record.content.event_ids != vec![event.id.clone()]
        || record.content.before_event_log_root != epoch.predecessor_roots.event_log
        || record.content.principal.principal_id != event.content.principal_id
        || record.content.authorization.policy_bundle_root != repository.authority_policy_root
    {
        return Err(
            "current epoch authority record does not bind its exact event and roots".into(),
        );
    }
    let expected_after = vela_protocol::authority_history::authority_event_log_root(
        &epoch.predecessor_roots.event_log,
        &[&event],
    )?;
    if record.content.after_event_log_root != expected_after {
        return Err("current epoch authority record has the wrong after-event root".into());
    }
    let required_delta = [
        (
            crate::authority_transaction::authority_event_path(&event.id),
            event.root()?,
        ),
        (".vela/epoch.json".into(), epoch.canonical_root()?),
        (".vela/repository.json".into(), repository.canonical_root()?),
    ];
    for (path, after_root) in required_delta {
        let matching = record
            .content
            .object_delta
            .iter()
            .filter(|delta| delta.path == path && delta.after_root.as_deref() == Some(&after_root))
            .count();
        if matching != 1 {
            return Err(format!(
                "current epoch authority record does not cover exact postimage {path}"
            ));
        }
    }
    Ok(())
}

fn only_regular_file(directory: &Path, suffix: &str) -> Result<PathBuf, String> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(suffix))
        })
        .collect::<Vec<_>>();
    files.sort();
    let [path] = files.as_slice() else {
        return Err(format!(
            "{} must contain exactly one current epoch object ending in {suffix}; found {}",
            directory.display(),
            files.len()
        ));
    };
    Ok(path.clone())
}

fn claim_from_finding(
    finding: &FindingBundle,
    mut evidence: Vec<ClaimEvidenceRef>,
    predecessor_commit: &str,
    relations: Vec<ClaimRelation>,
) -> Result<ClaimRecordV1, String> {
    evidence.extend(finding.evidence.evidence_spans.iter().filter_map(|span| {
        Some(ClaimEvidenceRef {
            relation: "supports".into(),
            artifact_id: None,
            artifact_root: span.get("artifact_sha256")?.as_str()?.to_string(),
            artifact_path: span
                .get("artifact_path")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }));
    evidence.sort_by(|left, right| {
        left.artifact_root
            .cmp(&right.artifact_root)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
            .then_with(|| left.artifact_path.cmp(&right.artifact_path))
    });
    evidence.dedup();
    let mut conditions = Vec::new();
    if !finding.conditions.text.trim().is_empty() {
        conditions.push(finding.conditions.text.trim().to_string());
    }
    if let Some(duration) = &finding.conditions.duration
        && !duration.trim().is_empty()
    {
        conditions.push(format!("Duration: {}", duration.trim()));
    }
    let locator = finding
        .provenance
        .doi
        .as_ref()
        .map(|doi| format!("https://doi.org/{doi}"))
        .or_else(|| finding.provenance.url.clone());
    let mut extensions = BTreeMap::new();
    extensions.insert(
        LEGACY_FINDING_EXTENSION.into(),
        json!({
            "source_schema": "https://vela.science/schema/finding-bundle/v0.10.0",
            "source_version": finding.version,
            "previous_version": finding.previous_version,
            "confidence": finding.confidence,
            "flags": finding.flags,
            "legacy_evidence": finding.evidence,
            "legacy_conditions": finding.conditions,
            "legacy_provenance": finding.provenance,
            "legacy_links": finding.links,
            "annotations": finding.annotations,
            "attachments": finding.attachments,
            "updated": finding.updated,
        }),
    );
    ClaimRecordV1::build(
        finding.version,
        ClaimAssertion {
            text: finding.assertion.text.trim().to_string(),
            kind: finding.assertion.assertion_type.trim().to_string(),
        },
        conditions,
        evidence,
        vec![ClaimSource {
            kind: finding.provenance.source_type.trim().to_string(),
            title: finding.provenance.title.trim().to_string(),
            locator,
            authors: finding
                .provenance
                .authors
                .iter()
                .map(|author| author.name.trim())
                .filter(|author| !author.is_empty())
                .map(str::to_string)
                .collect(),
            year: finding.provenance.year,
        }],
        relations,
        finding.created.clone(),
        Some(ImportedClaimSource {
            era: "finding_v0_10".into(),
            object_id: finding.id.clone(),
            object_root: source_finding_root(finding)?,
            predecessor_commit: predecessor_commit.to_string(),
        }),
        extensions,
    )
}

fn convert_current_pending_proposals(
    proposals: &[vela_protocol::proposals::StateProposal],
    predecessor_commit: &str,
) -> Result<
    (
        Vec<ObjectRef>,
        Vec<ClaimRecordV1>,
        Vec<ProposalMapping>,
        Vec<ProposalV1>,
    ),
    String,
> {
    let mut claim_refs = Vec::new();
    let mut claim_records = Vec::new();
    let mut proposal_refs = Vec::new();
    let mut proposal_records = Vec::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
    {
        let Some(link) = proposal.payload.get("submission") else {
            continue;
        };
        let finding: FindingBundle =
            serde_json::from_value(
                proposal.payload.get("finding").cloned().ok_or_else(|| {
                    format!("{}: current Proposal has no Claim body", proposal.id)
                })?,
            )
            .map_err(|error| format!("{}: decode current pending Claim: {error}", proposal.id))?;
        let claim = claim_from_finding(&finding, Vec::new(), predecessor_commit, Vec::new())
            .map_err(|error| format!("Proposal {} Claim: {error}", proposal.id))?;
        let claim_root = claim.canonical_root()?;
        claim_refs.push(ObjectRef {
            schema: "vela.claim-record.v1".into(),
            id: claim.claim_id.clone(),
            root: claim_root.clone(),
            path: format!(
                "records/claims/sha256/{}.json",
                claim_root.trim_start_matches("sha256:")
            ),
        });
        let source_root = root_canonical(proposal)?;
        let current = ProposalV1::build(
            match proposal.kind.as_str() {
                "finding.add" => "claim.add",
                "finding.revise" => "claim.revise",
                other => {
                    return Err(format!(
                        "{}: unsupported current action {other}",
                        proposal.id
                    ));
                }
            }
            .into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim.claim_id.clone(),
                root: claim_root.clone(),
            },
            proposal.actor.id.clone(),
            proposal.created_at.clone(),
            proposal.reason.clone(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: required_str(link, "submission_id", &proposal.id)?.into(),
                root: required_str(link, "submission_root", &proposal.id)?.into(),
                path: required_str(link, "submission_path", &proposal.id)?.into(),
            },
            proposal.caveats.clone(),
            Some(ImportedProposalSource {
                proposal_id: proposal.id.clone(),
                proposal_root: source_root.clone(),
                predecessor_commit: predecessor_commit.into(),
            }),
        )?;
        let proposal_root = current.canonical_root()?;
        proposal_refs.push(ProposalMapping {
            source_id: proposal.id.clone(),
            source_root,
            proposal_id: current.proposal_id.clone(),
            proposal_root: proposal_root.clone(),
            claim_id: claim.claim_id.clone(),
            claim_root,
            path: format!(
                "records/proposals/sha256/{}.json",
                proposal_root.trim_start_matches("sha256:")
            ),
        });
        claim_records.push(claim);
        proposal_records.push(current);
    }
    Ok((claim_refs, claim_records, proposal_refs, proposal_records))
}

fn convert_artifacts(
    artifacts: &[Artifact],
    predecessor_commit: &str,
) -> Result<Vec<ConvertedArtifact>, String> {
    let mut converted = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        artifact.validate_reference_axes()?;
        let record = CurrentArtifactRecordV1::build(
            artifact.clone(),
            root_canonical(artifact)?,
            predecessor_commit.into(),
        )?;
        let root = record.canonical_root()?;
        converted.push(ConvertedArtifact {
            reference: ObjectRef {
                schema: CURRENT_ARTIFACT_RECORD_SCHEMA_V1.into(),
                id: artifact.id.clone(),
                root: root.clone(),
                path: format!(
                    "records/artifacts/sha256/{}.json",
                    root.trim_start_matches("sha256:")
                ),
            },
            record,
        });
    }
    converted.sort_by(|left, right| left.reference.id.cmp(&right.reference.id));
    if converted
        .windows(2)
        .any(|pair| pair[0].reference.id == pair[1].reference.id)
    {
        return Err("legacy Artifact conversion found a duplicate artifact id".into());
    }
    Ok(converted)
}

fn artifact_evidence_by_finding(
    artifacts: &[Artifact],
    converted: &[ConvertedArtifact],
) -> Result<BTreeMap<String, Vec<ClaimEvidenceRef>>, String> {
    let by_id = converted
        .iter()
        .map(|artifact| (artifact.reference.id.as_str(), &artifact.reference))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = BTreeMap::<String, Vec<ClaimEvidenceRef>>::new();
    for artifact in artifacts {
        let reference = by_id
            .get(artifact.id.as_str())
            .ok_or_else(|| format!("missing converted Artifact {}", artifact.id))?;
        for finding_id in &artifact.target_findings {
            evidence
                .entry(finding_id.clone())
                .or_default()
                .push(ClaimEvidenceRef {
                    relation: "supports".into(),
                    artifact_id: Some(reference.id.clone()),
                    artifact_root: reference.root.clone(),
                    artifact_path: Some(reference.path.clone()),
                });
        }
    }
    for references in evidence.values_mut() {
        references.sort_by(|left, right| {
            left.artifact_root
                .cmp(&right.artifact_root)
                .then_with(|| left.artifact_id.cmp(&right.artifact_id))
        });
        references.dedup();
    }
    Ok(evidence)
}

fn retained_current_objects(frontier: &Path) -> Result<Vec<ObjectRef>, String> {
    let mut records = Vec::new();
    for (directory, id_field) in [
        ("records/submissions/sha256", "submission_id"),
        ("records/registrations/sha256", "registration_record_id"),
        ("records/verifications/sha256", "verification_record_id"),
        ("records/artifacts/sha256", "artifact_id"),
    ] {
        let directory_path = frontier.join(directory);
        if !directory_path.exists() {
            continue;
        }
        for path in files_recursive(&directory_path)? {
            let bytes = fs::read(&path)
                .map_err(|error| format!("read current object {}: {error}", path.display()))?;
            let relative = path
                .strip_prefix(frontier)
                .map_err(|_| "current object escaped Frontier".to_string())?
                .to_string_lossy()
                .to_string();
            let root = root_bytes(&bytes);
            if path.file_stem().and_then(|value| value.to_str())
                != Some(root.trim_start_matches("sha256:"))
            {
                return Err(format!(
                    "{relative}: filename does not match canonical bytes"
                ));
            }
            let value: Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!({"schema": "content-addressed-artifact"}));
            records.push(ObjectRef {
                schema: value["schema"]
                    .as_str()
                    .unwrap_or("content-addressed-artifact")
                    .into(),
                id: value[id_field]
                    .as_str()
                    .unwrap_or(root.trim_start_matches("sha256:"))
                    .into(),
                root,
                path: relative,
            });
        }
    }
    records.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(records)
}

fn semantic_projection_root(records: &[ClaimRecordV1]) -> Result<String, String> {
    let mut entries = records
        .iter()
        .map(|record| {
            let legacy = record
                .extensions
                .get(LEGACY_FINDING_EXTENSION)
                .ok_or_else(|| format!("{} lacks migration extension", record.claim_id))?;
            Ok(json!({
                "claim_id": record.claim_id,
                "assertion": record.assertion,
                "conditions": legacy["legacy_conditions"],
                "evidence": legacy["legacy_evidence"],
                "provenance": legacy["legacy_provenance"],
                "confidence": legacy["confidence"],
                "flags": legacy["flags"],
                "relations": record.relations,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left["claim_id"].as_str().cmp(&right["claim_id"].as_str()));
    root_canonical(&json!({"schema": "vela.claim-semantic-projection.v1", "claims": entries}))
}

fn source_semantic_projection_root(
    findings: &[FindingBundle],
    by_source: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut entries = findings
        .iter()
        .map(|finding| {
            let claim_id = by_source
                .get(&finding.id)
                .ok_or_else(|| format!("missing Claim mapping for {}", finding.id))?;
            let relations = finding
                .links
                .iter()
                .map(|link| {
                    Ok(json!({
                        "kind": link.link_type,
                        "target_claim_id": by_source.get(&link.target).ok_or_else(|| {
                            format!("dangling Finding relation {}", link.target)
                        })?,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(json!({
                "claim_id": claim_id,
                "assertion": {
                    "text": finding.assertion.text.trim(),
                    "kind": finding.assertion.assertion_type.trim(),
                },
                "conditions": finding.conditions,
                "evidence": finding.evidence,
                "provenance": finding.provenance,
                "confidence": finding.confidence,
                "flags": finding.flags,
                "relations": relations,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort_by(|left, right| left["claim_id"].as_str().cmp(&right["claim_id"].as_str()));
    root_canonical(&json!({"schema": "vela.claim-semantic-projection.v1", "claims": entries}))
}

fn git_object_manifest(
    frontier: &Path,
    commit: &str,
    tree: &str,
) -> Result<GitObjectManifest, String> {
    let output =
        crate::git_hardened::output(frontier, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    let mut entries = Vec::new();
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|raw| !raw.is_empty())
    {
        let tab = raw
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| "git ls-tree entry has no path separator".to_string())?;
        let header = std::str::from_utf8(&raw[..tab])
            .map_err(|error| format!("git ls-tree header is not UTF-8: {error}"))?;
        let path = std::str::from_utf8(&raw[tab + 1..])
            .map_err(|error| format!("tracked path is not UTF-8: {error}"))?;
        let mut parts = header.split_whitespace();
        let mode = parts.next().ok_or_else(|| "missing Git mode".to_string())?;
        let object_type = parts
            .next()
            .ok_or_else(|| "missing Git object type".to_string())?;
        let oid = parts
            .next()
            .ok_or_else(|| "missing Git object id".to_string())?;
        if object_type != "blob" {
            return Err(format!("tracked {object_type} at {path} is unsupported"));
        }
        let bytes = git_bytes(frontier, &["cat-file", "blob", oid])?;
        entries.push(GitObjectEntry {
            path: path.into(),
            git_mode: mode.into(),
            git_object_type: object_type.into(),
            git_object_id: oid.into(),
            byte_length: bytes.len() as u64,
            sha256: root_bytes(&bytes),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(GitObjectManifest {
        schema: OBJECT_MANIFEST_SCHEMA.into(),
        commit: commit.into(),
        tree: tree.into(),
        entries,
    })
}

fn archived_object(frontier: &Path, entry: &GitObjectEntry) -> Result<ArchivedObject, String> {
    let bytes = fs::read(frontier.join(&entry.path))
        .map_err(|error| format!("read archived path {}: {error}", entry.path))?;
    let value = serde_json::from_slice::<Value>(&bytes).ok();
    Ok(ArchivedObject {
        path: entry.path.clone(),
        schema: value
            .as_ref()
            .and_then(|value| value.get("schema"))
            .and_then(Value::as_str)
            .map(str::to_string),
        id: value.as_ref().and_then(record_id).map(str::to_string),
        root: root_bytes(&bytes),
        classification: legacy_classification(&entry.path).into(),
    })
}

fn record_id(value: &Value) -> Option<&str> {
    ["id", "proposal_id", "receipt_id", "event_id", "artifact_id"]
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str))
}

fn legacy_classification(path: &str) -> &'static str {
    if path.starts_with(".vela/events/") {
        "era0_event"
    } else if path.starts_with(".vela/findings/") {
        "finding_record"
    } else if path.starts_with(".vela/proposals/") {
        "proposal_v0"
    } else if path.starts_with(".vela/artifacts/") {
        "artifact_v0"
    } else if path.starts_with(".vela/policies/") {
        "acceptance_policy"
    } else if path.starts_with(".vela/authority/") {
        "predecessor_authority"
    } else if path == ".vela/actors.json" {
        "actor_registry"
    } else if path == "vela.lock" {
        "legacy_profile_lock"
    } else if path.starts_with("records/receipts/") || is_root_receipt(path) {
        "receipt_v1"
    } else if path == "frontier.json" {
        "legacy_snapshot"
    } else {
        "legacy_projection"
    }
}

fn is_predecessor_archive_path(path: &str) -> bool {
    path == "vela.lock" || path.starts_with(".vela/authority/") || is_retired_current_path(path)
}

fn is_retired_current_path(path: &str) -> bool {
    path == ".vela/actors.json"
        || path == "frontier.json"
        || path.starts_with(".vela/events/")
        || path.starts_with(".vela/findings/")
        || path.starts_with(".vela/proposals/")
        || path.starts_with(".vela/artifacts/")
        || path.starts_with(".vela/policies/")
        || path.starts_with("records/receipts/")
        || path.starts_with("records/review/")
        || path.starts_with("records/decision-evidence/")
        || is_root_receipt(path)
}

fn is_root_receipt(path: &str) -> bool {
    path.starts_with("records/vrc_") && path.ends_with(".json")
}

fn require_clean_synced_main(frontier: &Path) -> Result<(), String> {
    let branch = git_text(frontier, &["branch", "--show-current"])?;
    if branch != "main" {
        return Err(format!("repository upgrade requires main, found {branch}"));
    }
    let status = git_text(
        frontier,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err("repository upgrade refuses tracked or untracked dirt".into());
    }
    let head = git_text(frontier, &["rev-parse", "HEAD^{commit}"])?;
    let remote = git_text(frontier, &["rev-parse", "origin/main^{commit}"])?;
    if head != remote {
        return Err(format!(
            "repository upgrade requires HEAD == origin/main ({head} != {remote})"
        ));
    }
    Ok(())
}

fn verify_recovery_barrier(frontier: &Path) -> Result<(), String> {
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    crate::frontier_txn::FrontierTxn::verify_recovery_barrier_read_only(frontier, &journal_dir)
        .map_err(|error| error.to_string())
}

fn canonical_github_remote(value: &str) -> Result<String, String> {
    let path = if let Some(path) = value.strip_prefix("git@github.com:") {
        path
    } else if let Some(path) = value.strip_prefix("ssh://git@github.com/") {
        path
    } else if let Some(path) = value.strip_prefix("https://github.com/") {
        if path.contains('@') {
            return Err("credential-bearing GitHub remote is not allowed".into());
        }
        path
    } else {
        return Err("repository upgrade requires a canonical GitHub origin".into());
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return Err("GitHub remote must name exactly one owner/repository".into());
    }
    Ok(format!("https://github.com/{owner}/{repo}.git"))
}

fn create_or_verify_bundle(frontier: &Path, path: &Path) -> Result<(), String> {
    if path.exists() {
        let output = crate::git_hardened::output(
            frontier,
            &["bundle", "verify", &path.display().to_string()],
        )?;
        if !output.status.success() {
            return Err(format!(
                "existing archive bundle {} does not verify: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        return Ok(());
    }
    let output = crate::git_hardened::output(
        frontier,
        &["bundle", "create", &path.display().to_string(), "HEAD"],
    )?;
    if !output.status.success() {
        return Err(format!(
            "create archive bundle {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    create_or_verify_bundle(frontier, path)
}

fn write_candidate_records(
    root: &Path,
    claims: &[ClaimRecordV1],
    pending_claims: &[ClaimRecordV1],
    proposals: &[ProposalV1],
    artifacts: &[ConvertedArtifact],
) -> Result<(), String> {
    for claim in claims.iter().chain(pending_claims) {
        let bytes = claim.canonical_bytes()?;
        let object_root = root_bytes(&bytes);
        write_exact(
            &root.join("records/claims/sha256").join(format!(
                "{}.json",
                object_root.trim_start_matches("sha256:")
            )),
            &bytes,
        )?;
    }
    for proposal in proposals {
        let bytes = proposal.canonical_bytes()?;
        let object_root = root_bytes(&bytes);
        write_exact(
            &root.join("records/proposals/sha256").join(format!(
                "{}.json",
                object_root.trim_start_matches("sha256:")
            )),
            &bytes,
        )?;
    }
    for artifact in artifacts {
        let bytes = artifact.record.canonical_bytes()?;
        write_exact(&root.join(&artifact.reference.path), &bytes)?;
    }
    Ok(())
}

fn copy_retained_current_objects(
    frontier: &Path,
    candidate_root: &Path,
    objects: &[ObjectRef],
) -> Result<(), String> {
    for object in objects {
        let destination = candidate_root.join(&object.path);
        if destination.is_file() {
            if root_bytes(
                &fs::read(&destination)
                    .map_err(|error| format!("read {}: {error}", destination.display()))?,
            ) != object.root
            {
                return Err(format!(
                    "candidate object {} does not match its declared root",
                    object.path
                ));
            }
            continue;
        }
        let source = frontier.join(&object.path);
        let bytes = fs::read(&source)
            .map_err(|error| format!("read retained object {}: {error}", source.display()))?;
        if root_bytes(&bytes) != object.root {
            return Err(format!(
                "retained object {} changed after planning",
                object.path
            ));
        }
        write_exact(&destination, &bytes)?;
    }
    Ok(())
}

fn files_recursive(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
        {
            let path = entry
                .map_err(|error| format!("read {} entry: {error}", directory.display()))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn write_exact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    if path.exists() {
        let existing =
            fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
        if existing != bytes {
            return Err(format!(
                "existing archive path {} has different bytes",
                path.display()
            ));
        }
        return Ok(());
    }
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn source_finding_root(finding: &FindingBundle) -> Result<String, String> {
    root_canonical(finding)
}

fn required_str<'a>(value: &'a Value, field: &str, context: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context}: missing {field}"))
}

fn root_canonical<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)?
    ))
}

fn root_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn plan_root(plan: &RepositoryUpgradePlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan)
        .map_err(|error| format!("serialize repository upgrade plan: {error}"))?;
    value["plan_root"] = Value::String(String::new());
    value["next_command"] = Value::String(String::new());
    root_canonical(&value)
}

fn git_text(frontier: &Path, args: &[&str]) -> Result<String, String> {
    crate::git_hardened::text(frontier, args)
}

fn git_bytes(frontier: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = crate::git_hardened::output(frontier, args)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    Ok(output.stdout)
}

fn require_reason(reason: &str) -> Result<(), String> {
    if reason.trim().is_empty()
        || reason != reason.trim()
        || reason.len() > 2_048
        || reason.chars().any(char::is_control)
    {
        return Err("repository upgrade reason must be non-empty, trimmed text".into());
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_remote_normalization_is_closed() {
        assert_eq!(
            canonical_github_remote("git@github.com:vela-science/example.git").unwrap(),
            "https://github.com/vela-science/example.git"
        );
        assert!(
            canonical_github_remote("https://token@github.com/vela-science/example.git").is_err()
        );
        assert!(canonical_github_remote("https://example.com/vela-science/example.git").is_err());
    }

    #[test]
    fn legacy_path_classification_is_explicit() {
        assert!(is_predecessor_archive_path(".vela/events/vev_x.json"));
        assert!(is_predecessor_archive_path(".vela/artifacts/va_x.json"));
        assert!(is_predecessor_archive_path(
            "records/receipts/sha256/x.json"
        ));
        assert!(is_predecessor_archive_path("records/vrc_x.json"));
        assert!(is_predecessor_archive_path("vela.lock"));
        assert!(is_predecessor_archive_path(
            ".vela/authority/records/var_x.dsse.json"
        ));
        assert!(!is_retired_current_path(
            ".vela/authority/records/var_x.dsse.json"
        ));
        assert!(!is_predecessor_archive_path(
            "records/submissions/sha256/x.json"
        ));
        assert!(!is_predecessor_archive_path("graph/frontier-map.json"));
    }
}
