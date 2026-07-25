//! Root-bound migration planning for Frontier Repository Profile v1.
//!
//! Preview is deliberately key-free and write-free. Apply rederives that exact
//! plan, holds the recovery barrier across one protected repository-boundary
//! approval, and promotes the signed delta into one recoverable transaction.
//! No plaintext or generic signing path is available.

use super::{fail_return, print_json};
use crate::config::git_publish::{
    PublicationOutcome, PublicationState, manual_uncommitted_exact_delta,
};
use crate::frontier_txn::{
    ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn, FrontierTxnPlan, FrontierTxnPlanSpec,
    InputBinding, MigrationCeremonySpec, OperationId, OperationKind, PlannedWrite, RepoPath,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use vela_protocol::events::{
    EVENT_KIND_FRONTIER_REPOSITORY_BOUND, StateEvent, event_content_preimage_bytes,
};
use vela_protocol::frontier_profile::FrontierProfileV1;
use vela_protocol::frontier_repo::{
    FRONTIER_MANIFEST_SCHEMA, FRONTIER_REPO_LAYOUT, FrontierManifest, FrontierProfileFile,
};
use vela_protocol::frontier_repository::{
    ExactFrontierDependencyV1, FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryMode,
    FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, LegacyFrontierOriginV1,
    new_repository_boundary_event, repository_identity_event_content_root,
};
use vela_protocol::frontier_settings::{
    FRONTIER_SETTINGS_SCHEMA, FrontierSettingsV1, McpSettingsV1, PublishSettingsV1, WorkSettingsV1,
};
use vela_protocol::project::{Project, ProjectDependency};

const MIGRATION_TARGET: &str = "frontier-repo-v1";
const MIGRATION_PLAN_SCHEMA: &str = "vela.frontier-repository-migration-plan.v1";
const MIGRATION_PREVIEW_SCHEMA: &str = "vela.frontier-repository-migration-preview.v1";
const MIGRATION_PLAN_DOMAIN: &[u8] = b"vela.frontier-repository-migration-plan.v1\0";
const MIGRATION_DEPENDENCY_INPUT_SCHEMA: &str = "vela.frontier-dependency-migration.v1";

const DERIVED_OUTPUTS: &[&str] = &[
    "frontier.json",
    "vela.lock",
    "proof/latest.json",
    "proof/events.manifest.jsonl",
    "proof/replay.trace.jsonl",
    "proof/freshness.md",
    "proof/hashes.json",
    "targets.json",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MigrationRootFamily {
    event_log_root: String,
    event_count: u64,
    legacy_snapshot_root: String,
    proposal_root: String,
    actor_registry_root: String,
    artifact_registry_root: String,
    canonical_store_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MigrationSemanticAfterRoots {
    event_log_root: String,
    event_count: u64,
    legacy_snapshot_root: String,
    proposal_root: String,
    actor_registry_root: String,
    artifact_registry_root: String,
    profile_root: String,
    identity_root: String,
    dependency_root: String,
    scientific_state_root: String,
}

/// State of the exact raw retained-store root in a two-phase migration.
///
/// The raw root includes the canonical signed event bytes. It therefore cannot
/// be known by a key-free preview: the Ed25519 signature is produced only by
/// the late protected key read. The executor must replace this state with an
/// exact root in its execution result and verify the resulting postimages
/// before crossing the transaction commit marker.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SignedStoreRootState {
    PendingProtectedSignature,
    Exact { canonical_store_root: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MigrationTouch {
    path: String,
    operation: String,
    class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegacyProjectConfig {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frontier_id: Option<String>,
    #[serde(default)]
    compiled_at: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_compiler")]
    compiler: String,
    #[serde(default)]
    papers_processed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegacyConfig {
    project: LegacyProjectConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    publish: Option<PublishSettingsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    work: Option<WorkSettingsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mcp: Option<McpSettingsV1>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegacyConfigCommitment {
    source_root: Option<String>,
    project: LegacyProjectConfig,
    settings: FrontierSettingsV1,
    settings_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LegacyDependencyDescriptorV1 {
    name: String,
    source: String,
    version: Option<String>,
    pinned_hash: Option<String>,
    vfr_id: Option<String>,
    locator: Option<String>,
    pinned_snapshot_hash: Option<String>,
}

impl From<&ProjectDependency> for LegacyDependencyDescriptorV1 {
    fn from(value: &ProjectDependency) -> Self {
        Self {
            name: value.name.clone(),
            source: value.source.clone(),
            version: value.version.clone(),
            pinned_hash: value.pinned_hash.clone(),
            vfr_id: value.vfr_id.clone(),
            locator: value.locator.clone(),
            pinned_snapshot_hash: value.pinned_snapshot_hash.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DependencyMigrationEntryV1 {
    legacy: LegacyDependencyDescriptorV1,
    repository_path: String,
    boundary_content_root: String,
    trust_anchor: vela_edge::frontier_repository::RepositoryTrustAnchor,
    exact: ExactFrontierDependencyV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DependencyMigrationInputV1 {
    schema: String,
    entries: Vec<DependencyMigrationEntryV1>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DependencyMigrationCommitmentV1 {
    schema: String,
    source_path: Option<String>,
    source_root: Option<String>,
    entries: Vec<DependencyMigrationEntryV1>,
    dependency_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MigrationBlocker {
    code: String,
    message: String,
    recovery: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct RepositoryMigrationPlan {
    schema: String,
    ok: bool,
    command: String,
    target: String,
    frontier: String,
    frontier_id: String,
    git_commit: String,
    git_tree: String,
    vela_version: String,
    vela_binary_path: String,
    vela_binary_sha256: String,
    candidate_profile_path: String,
    candidate_profile_source_root: String,
    candidate_profile: FrontierProfileV1,
    candidate_profile_root: String,
    legacy_manifest: FrontierManifest,
    legacy_manifest_source_root: String,
    legacy_config: LegacyConfigCommitment,
    dependency_migration: DependencyMigrationCommitmentV1,
    signer_actor: String,
    signer_public_key: String,
    reason: String,
    observed_at: String,
    trust_mode: FrontierRepositoryTrustMode,
    boundary_payload: FrontierRepositoryBoundaryPayloadV1,
    boundary_event: StateEvent,
    boundary_event_content_root: String,
    trust_anchor: vela_edge::repository_write::RepositoryTrustAnchorV1,
    trust_anchor_root: String,
    roots_before: MigrationRootFamily,
    semantic_after: MigrationSemanticAfterRoots,
    signed_store_root_state: SignedStoreRootState,
    target_index: vela_edge::target_index::TargetIndexSealPlan,
    touched: Vec<MigrationTouch>,
    blockers: Vec<MigrationBlocker>,
    ready_for_protected_apply: bool,
    plan_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MigrationInputs {
    profile_bytes: Vec<u8>,
    settings_bytes: Vec<u8>,
    target_index_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MigrationPreview {
    plan: RepositoryMigrationPlan,
    inputs: MigrationInputs,
}

/// Reader-facing projection of an exact migration plan.
///
/// The complete target candidate and sealed index remain independently
/// content-addressed and are included in `plan_root`. Repeating their targets,
/// packet paths, and canonical JSON in the command response made a 1,217-target
/// Erdős preview larger than one megabyte without adding verification value.
fn migration_preview_json(plan: &RepositoryMigrationPlan) -> Result<serde_json::Value, String> {
    let mut projection = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    let object = projection
        .as_object_mut()
        .ok_or_else(|| "migration preview must serialize as an object".to_string())?;
    object.insert(
        "schema".to_string(),
        serde_json::Value::String(MIGRATION_PREVIEW_SCHEMA.to_string()),
    );
    object.insert(
        "plan_schema".to_string(),
        serde_json::Value::String(plan.schema.clone()),
    );

    let target_index = object
        .get_mut("target_index")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "migration preview is missing target_index".to_string())?;
    target_index.remove("canonical_json");
    target_index.remove("packet_paths");
    target_index.insert(
        "packet_count".to_string(),
        serde_json::json!(plan.target_index.packet_paths.len()),
    );

    let mut state_counts = BTreeMap::<String, u64>::new();
    for target in &plan.target_index.index.targets {
        *state_counts.entry(target.state.clone()).or_default() += 1;
    }
    let index = target_index
        .get_mut("index")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "migration preview target_index is missing index".to_string())?;
    index.remove("targets");
    index.insert(
        "target_count".to_string(),
        serde_json::json!(plan.target_index.index.targets.len()),
    );
    index.insert(
        "target_state_counts".to_string(),
        serde_json::to_value(state_counts).map_err(|error| error.to_string())?,
    );
    Ok(projection)
}

/// A verified, still-uninstalled protocol delta.
///
/// This foundation is intentionally incapable of writing a repository. It
/// lets the eventual protected executor receive a signed event and construct
/// the exact authoritative/profile delta only after proof of possession.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedMigrationDelta {
    writes: BTreeMap<String, Vec<u8>>,
    deletes: Vec<String>,
    signed_canonical_store_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MigrationExecutionResult {
    schema: String,
    ok: bool,
    command: String,
    plan_root: String,
    operation_id: String,
    event_id: String,
    event_log_root: String,
    event_count: u64,
    canonical_delta_root: String,
    signed_store_root_state: SignedStoreRootState,
    trust_anchor_root: String,
    trust_anchor_path: String,
    target_index_root: String,
    publication: PublicationOutcome,
    replay_ok: bool,
}

fn migration_result_human(frontier: &str, result: &MigrationExecutionResult) -> String {
    let publication = match &result.publication.state {
        PublicationState::Uncommitted { reason, .. } => format!("uncommitted · {reason}"),
        PublicationState::Unchanged { commit } => format!("unchanged · {commit}"),
        PublicationState::Stale {
            candidate,
            expected,
            actual,
        } => format!("stale · candidate {candidate} expected {expected}, target is {actual}"),
        PublicationState::CommittedLocal { commit } => format!("committed locally · {commit}"),
        PublicationState::Pushed { commit, remote } => {
            format!("pushed · {commit} on {remote}")
        }
        PublicationState::Unknown { reason } => format!("unknown · {reason}"),
    };
    let next = result
        .publication
        .recovery_command
        .as_deref()
        .unwrap_or("git status --short");
    format!(
        "migrated {frontier}\n  operation: {}\n  event: {}\n  event root: {}\n  canonical delta: {}\n  target index: {}\n  trust anchor: {}\n  Git publication: {}\n  next: {}",
        result.operation_id,
        result.event_id,
        result.event_log_root,
        result.canonical_delta_root,
        result.target_index_root,
        result.trust_anchor_root,
        publication,
        next
    )
}

fn default_compiler() -> String {
    vela_protocol::project::VELA_COMPILER_VERSION.to_string()
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn event_content_root(event: &StateEvent) -> String {
    sha256_root(&event_content_preimage_bytes(event))
}

fn git(frontier: &Path, args: &[&str]) -> Result<String, String> {
    crate::git_hardened::text(frontier, args)
}

fn assert_migration_checkout(frontier: &Path) -> Result<(String, String), String> {
    let head = git(frontier, &["rev-parse", "HEAD^{commit}"])?;
    let tree = git(frontier, &["rev-parse", "HEAD^{tree}"])?;
    let disallowed = vela_edge::git_read::dirty_worktree_paths(frontier, false)?;
    if !disallowed.is_empty() {
        return Err(format!(
            "migration requires a clean checkout; found {}",
            disallowed.join(", ")
        ));
    }

    if let Ok(upstream) = git(
        frontier,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    ) {
        let output = crate::git_hardened::output(
            frontier,
            &["merge-base", "--is-ancestor", &upstream, "HEAD"],
        )
        .map_err(|error| format!("check upstream ancestry: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "HEAD is behind or forked from {upstream}; fast-forward or reconcile before migration"
            ));
        }
    }

    let git_journal = git(
        frontier,
        &["rev-parse", "--git-path", "vela/operation-journals"],
    )?;
    let journals = {
        let path = PathBuf::from(git_journal);
        if path.is_absolute() {
            path
        } else {
            frontier.join(path)
        }
    };
    drop(
        crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier, &journals)
            .map_err(|error| error.to_string())?,
    );
    Ok((head, tree))
}

fn read_candidate_profile(
    frontier: &Path,
    candidate: &Path,
) -> Result<(PathBuf, Vec<u8>, FrontierProfileV1), String> {
    let metadata = std::fs::symlink_metadata(candidate)
        .map_err(|error| format!("inspect candidate profile {}: {error}", candidate.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("candidate profile must be one regular, non-symlinked file".to_string());
    }
    let candidate = std::fs::canonicalize(candidate)
        .map_err(|error| format!("resolve candidate profile: {error}"))?;
    if candidate.starts_with(frontier) {
        return Err(
            "candidate profile must be outside the Frontier checkout so preview remains write-free and source-clean"
                .to_string(),
        );
    }
    let bytes = std::fs::read(&candidate)
        .map_err(|error| format!("read candidate profile {}: {error}", candidate.display()))?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("candidate profile is not UTF-8: {error}"))?;
    let profile = FrontierProfileV1::from_yaml_str(source)?;
    Ok((candidate, bytes, profile))
}

fn legacy_settings(
    frontier: &Path,
    project: &Project,
) -> Result<(LegacyConfigCommitment, Vec<u8>), String> {
    let path = frontier.join(".vela/config.toml");
    let (source_root, config) = if path.is_file() {
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(".vela/config.toml must be a regular file".to_string());
        }
        let bytes =
            std::fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|error| format!("parse {} as UTF-8: {error}", path.display()))?;
        let config: LegacyConfig = toml::from_str(source)
            .map_err(|error| format!("unknown or invalid legacy settings: {error}"))?;
        (Some(sha256_root(&bytes)), config)
    } else {
        (
            None,
            LegacyConfig {
                project: LegacyProjectConfig {
                    name: project.project.name.clone(),
                    frontier_id: Some(project.frontier_id()),
                    compiled_at: project.project.compiled_at.clone(),
                    description: project.project.description.clone(),
                    compiler: project.project.compiler.clone(),
                    papers_processed: project.project.papers_processed,
                },
                publish: None,
                work: None,
                mcp: None,
            },
        )
    };

    let expected_frontier_id = project.frontier_id();
    let configured_frontier_id = config
        .project
        .frontier_id
        .as_deref()
        .unwrap_or(expected_frontier_id.as_str());
    let project_mismatch = config.project.name != project.project.name
        || configured_frontier_id != expected_frontier_id
        || config.project.compiled_at != project.project.compiled_at
        || config.project.description != project.project.description
        || config.project.compiler != project.project.compiler
        || config.project.papers_processed != project.project.papers_processed;
    if project_mismatch {
        return Err(
            "legacy .vela/config.toml project seed does not match the exact loaded Frontier"
                .to_string(),
        );
    }

    let settings = FrontierSettingsV1 {
        schema: FRONTIER_SETTINGS_SCHEMA.to_string(),
        publish: config.publish,
        work: config.work,
        mcp: config.mcp,
    };
    settings.validate()?;
    let settings_bytes = settings.to_toml()?.into_bytes();
    let commitment = LegacyConfigCommitment {
        source_root,
        project: config.project,
        settings,
        settings_root: sha256_root(&settings_bytes),
    };
    Ok((commitment, settings_bytes))
}

fn normalize_legacy_snapshot_root(value: &str) -> Result<String, String> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "legacy dependency pinned_snapshot_hash must be a full lowercase SHA-256 digest"
                .to_string(),
        );
    }
    Ok(format!("sha256:{digest}"))
}

fn dependency_migration(
    frontier: &Path,
    project: &Project,
    input_path: Option<&Path>,
) -> Result<
    (
        DependencyMigrationCommitmentV1,
        Vec<ExactFrontierDependencyV1>,
    ),
    String,
> {
    let legacy = &project.project.dependencies;
    if legacy.is_empty() {
        if input_path.is_some() {
            return Err(
                "a dependency migration input is forbidden when the legacy dependency list is empty"
                    .to_string(),
            );
        }
        let dependencies = Vec::new();
        return Ok((
            DependencyMigrationCommitmentV1 {
                schema: MIGRATION_DEPENDENCY_INPUT_SCHEMA.to_string(),
                source_path: None,
                source_root: None,
                entries: Vec::new(),
                dependency_root: vela_protocol::frontier_repository::exact_dependency_root(
                    &dependencies,
                )?,
            },
            dependencies,
        ));
    }
    let input_path = input_path.ok_or_else(|| {
        "legacy dependencies require --dependency-input with one exact repository-bound resolution per legacy entry"
            .to_string()
    })?;
    let metadata = std::fs::symlink_metadata(input_path).map_err(|error| {
        format!(
            "inspect dependency migration input {}: {error}",
            input_path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(
            "dependency migration input must be one regular, non-symlinked file".to_string(),
        );
    }
    let input_path = std::fs::canonicalize(input_path)
        .map_err(|error| format!("resolve dependency migration input: {error}"))?;
    if input_path.starts_with(frontier) {
        return Err("dependency migration input must be outside the Frontier checkout".to_string());
    }
    let bytes = std::fs::read(&input_path).map_err(|error| {
        format!(
            "read dependency migration input {}: {error}",
            input_path.display()
        )
    })?;
    let mut input: DependencyMigrationInputV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode closed dependency migration input: {error}"))?;
    if input.schema != MIGRATION_DEPENDENCY_INPUT_SCHEMA {
        return Err(format!(
            "dependency migration input schema must be {MIGRATION_DEPENDENCY_INPUT_SCHEMA}"
        ));
    }
    if input.entries.len() != legacy.len() {
        return Err(format!(
            "dependency migration input has {} entries, but the legacy Frontier has {} dependencies",
            input.entries.len(),
            legacy.len()
        ));
    }

    let mut unmatched = legacy
        .iter()
        .map(LegacyDependencyDescriptorV1::from)
        .collect::<Vec<_>>();
    let mut exact = Vec::with_capacity(input.entries.len());
    for entry in &mut input.entries {
        entry
            .exact
            .validate()
            .map_err(|error| format!("invalid expected exact dependency: {error}"))?;
        let Some(index) = unmatched
            .iter()
            .position(|candidate| candidate == &entry.legacy)
        else {
            return Err(format!(
                "dependency migration entry for {:?} does not match exactly one legacy dependency",
                entry.legacy.name
            ));
        };
        unmatched.remove(index);

        let legacy_frontier_id = entry.legacy.vfr_id.as_deref().ok_or_else(|| {
            format!(
                "legacy dependency {:?} has no full Frontier ID and is ambiguous",
                entry.legacy.name
            )
        })?;
        let legacy_snapshot = entry
            .legacy
            .pinned_snapshot_hash
            .as_deref()
            .ok_or_else(|| {
                format!(
                    "legacy dependency {:?} has no full pinned snapshot hash",
                    entry.legacy.name
                )
            })
            .and_then(normalize_legacy_snapshot_root)?;

        let repository = std::fs::canonicalize(&entry.repository_path).map_err(|error| {
            format!(
                "resolve dependency repository {}: {error}",
                entry.repository_path
            )
        })?;
        if repository == frontier || repository.starts_with(frontier) {
            return Err(
                "dependency repository cannot be inside the migrating Frontier".to_string(),
            );
        }
        let dirt = vela_edge::git_read::dirty_worktree_paths(&repository, true)?;
        if !dirt.is_empty() {
            return Err(format!(
                "dependency repository {} must be an exact clean checkout; found {}",
                repository.display(),
                dirt.join(", ")
            ));
        }
        let dependency_project = vela_protocol::repo::load_from_path(&repository)
            .map_err(|error| format!("load dependency repository: {error}"))?;
        let matches = dependency_project
            .events
            .iter()
            .filter(|event| {
                repository_identity_event_content_root(event)
                    .is_ok_and(|root| root == entry.boundary_content_root)
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "dependency boundary root {} resolves to {} events, expected exactly one",
                entry.boundary_content_root,
                matches.len()
            ));
        }
        let boundary = matches[0];
        let context =
            vela_edge::frontier_repository::verify_repository_boundary_context_with_trust_anchor(
                &dependency_project,
                &repository,
                boundary,
                Some(&entry.trust_anchor),
            )?;
        let derived = if entry.exact.git_commit == context.anchor.git_commit {
            vela_edge::frontier_repository::derive_exact_dependency_at_boundary(
                &dependency_project,
                &repository,
                boundary,
                &entry.trust_anchor,
            )?
        } else {
            vela_edge::frontier_repository::derive_exact_dependency_at_temporalized_ancestor(
                &dependency_project,
                &repository,
                boundary,
                &entry.trust_anchor,
                &entry.exact.git_commit,
                &legacy_snapshot,
            )?
        };
        if derived != entry.exact {
            return Err(format!(
                "dependency {:?} exact v1 pin does not match the authenticated exact dependency state",
                entry.legacy.name
            ));
        }
        if derived.frontier_id != legacy_frontier_id {
            return Err(format!(
                "dependency {:?} Frontier ID changed during migration",
                entry.legacy.name
            ));
        }
        let resolved_facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            &repository,
            &derived.git_commit,
        )?;
        if resolved_facts.snapshot_root != legacy_snapshot {
            return Err(format!(
                "dependency {:?} legacy snapshot pin does not match the authenticated exact dependency state",
                entry.legacy.name
            ));
        }
        entry.repository_path = repository.display().to_string();
        exact.push(derived);
    }
    if !unmatched.is_empty() {
        return Err("one or more legacy dependencies were not resolved exactly".to_string());
    }
    exact.sort_by(|left, right| {
        (&left.frontier_id, &left.identity_root).cmp(&(&right.frontier_id, &right.identity_root))
    });
    let dependency_root = vela_protocol::frontier_repository::exact_dependency_root(&exact)?;
    input.entries.sort_by(|left, right| {
        (&left.exact.frontier_id, &left.exact.identity_root)
            .cmp(&(&right.exact.frontier_id, &right.exact.identity_root))
    });
    Ok((
        DependencyMigrationCommitmentV1 {
            schema: MIGRATION_DEPENDENCY_INPUT_SCHEMA.to_string(),
            source_path: Some(input_path.display().to_string()),
            source_root: Some(sha256_root(&bytes)),
            entries: input.entries,
            dependency_root,
        },
        exact,
    ))
}

fn validate_legacy_manifest(manifest: &FrontierManifest, project: &Project) -> Result<(), String> {
    if manifest.schema != FRONTIER_MANIFEST_SCHEMA || manifest.layout != FRONTIER_REPO_LAYOUT {
        return Err(format!(
            "migration requires {FRONTIER_MANIFEST_SCHEMA} / {FRONTIER_REPO_LAYOUT}, found {} / {}",
            manifest.schema, manifest.layout
        ));
    }
    if manifest
        .frontier_id
        .as_deref()
        .is_some_and(|id| id != project.frontier_id())
    {
        return Err("legacy frontier.yaml Frontier ID does not match loaded state".to_string());
    }
    if manifest.name.trim().is_empty() {
        return Err("legacy frontier.yaml has no unambiguous name".to_string());
    }
    if !manifest.dependencies.frontiers.is_empty()
        || !manifest.dependencies.packages.is_empty()
        || !manifest.dependencies.adapters.is_empty()
    {
        return Err(
            "legacy unstructured dependency strings cannot be migrated without an exact structured source entry"
                .to_string(),
        );
    }
    if manifest.dependencies.frontiers_v2 != project.project.dependencies {
        return Err(
            "legacy frontier.yaml dependency entries do not match the exact loaded Project dependencies"
                .to_string(),
        );
    }
    Ok(())
}

fn migration_touches(event_id: &str, has_config: bool) -> Vec<MigrationTouch> {
    let mut touched = vec![
        MigrationTouch {
            path: format!(".vela/events/{event_id}.json"),
            operation: "create".to_string(),
            class: "canonical_authority".to_string(),
        },
        MigrationTouch {
            path: "frontier.yaml".to_string(),
            operation: "replace".to_string(),
            class: "profile".to_string(),
        },
        MigrationTouch {
            path: ".vela/settings.toml".to_string(),
            operation: "create".to_string(),
            class: "operational_settings".to_string(),
        },
    ];
    if has_config {
        touched.push(MigrationTouch {
            path: ".vela/config.toml".to_string(),
            operation: "delete_after_parity".to_string(),
            class: "legacy_seed".to_string(),
        });
    }
    touched.extend(DERIVED_OUTPUTS.iter().map(|path| MigrationTouch {
        path: (*path).to_string(),
        operation: "regenerate".to_string(),
        class: "derived".to_string(),
    }));
    touched
}

fn compute_plan_root(plan: &RepositoryMigrationPlan) -> Result<String, String> {
    let mut value =
        serde_json::to_value(plan).map_err(|error| format!("encode migration plan: {error}"))?;
    value
        .as_object_mut()
        .ok_or_else(|| "migration plan is not an object".to_string())?
        .remove("plan_root");
    let canonical = vela_protocol::canonical::to_canonical_bytes(&value)?;
    let mut digest = Sha256::new();
    digest.update(MIGRATION_PLAN_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn verify_plan_root(plan: &RepositoryMigrationPlan) -> Result<(), String> {
    if plan.schema != MIGRATION_PLAN_SCHEMA
        || !plan.ok
        || plan.command != "migrate"
        || plan.target != MIGRATION_TARGET
    {
        return Err("migration plan has an invalid JSON command envelope".to_string());
    }
    let computed = compute_plan_root(plan)?;
    if computed != plan.plan_root {
        return Err(format!(
            "migration plan root mismatch: expected {}, computed {computed}",
            plan.plan_root
        ));
    }
    Ok(())
}

fn prepare_migration(
    frontier: &Path,
    candidate_profile: &Path,
    dependency_input: Option<&Path>,
    target_candidate: &Path,
    signer: &str,
    reason: &str,
    observed_at: &str,
) -> Result<MigrationPreview, String> {
    if reason.trim().is_empty() || reason != reason.trim() {
        return Err("migration reason must be non-empty and have no outer whitespace".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("migration observation time must be RFC3339: {error}"))?;
    let frontier =
        std::fs::canonicalize(frontier).map_err(|error| format!("resolve Frontier: {error}"))?;
    let (head, tree) = assert_migration_checkout(&frontier)?;
    let project = vela_protocol::repo::load_from_path(&frontier)?;
    let replay = vela_protocol::reducer::verify_replay(&project);
    if !replay.ok {
        return Err(format!(
            "migration requires exact legacy replay; found {} diff(s)",
            replay.diffs.len()
        ));
    }
    if project
        .events
        .iter()
        .any(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
    {
        return Err("Frontier already contains a repository-boundary event".to_string());
    }

    let manifest_bytes = std::fs::read(frontier.join("frontier.yaml"))
        .map_err(|error| format!("read legacy frontier.yaml: {error}"))?;
    let manifest = match vela_protocol::frontier_repo::read_repository_profile(&frontier)? {
        Some(FrontierProfileFile::LegacyV0_1(manifest)) => manifest,
        Some(FrontierProfileFile::V1(_)) => {
            return Err("Frontier is already Profile v1".to_string());
        }
        None => return Err("legacy Frontier has no frontier.yaml".to_string()),
    };
    validate_legacy_manifest(&manifest, &project)?;

    let (candidate_path, profile_bytes, profile) =
        read_candidate_profile(&frontier, candidate_profile)?;
    profile.assert_frontier_id(&project.frontier_id())?;
    let profile_root = profile.profile_root()?;
    let (legacy_config, settings_bytes) = legacy_settings(&frontier, &project)?;
    let actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
        &project,
        signer,
        observed_at,
    )?;

    let anchor = vela_edge::frontier_repository::derive_repository_anchor_facts(&frontier, &head)?;
    let legacy_identity_preimage_root =
        vela_edge::frontier_repository::derive_legacy_identity_preimage_root(&project)?;
    let origin = LegacyFrontierOriginV1 {
        schema: vela_protocol::frontier_repository::LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
        frontier_id: project.frontier_id(),
        legacy_identity_preimage_root: legacy_identity_preimage_root.clone(),
        git_object_format: anchor.git_object_format,
        anchor_git_commit: anchor.git_commit.clone(),
        anchor_git_tree: anchor.git_tree.clone(),
        anchor_event_log_root: anchor.event_log_root.clone(),
        anchor_event_count: anchor.event_count,
    };
    let identity_root = origin.identity_root()?;
    let (dependency_migration, dependencies) =
        dependency_migration(&frontier, &project, dependency_input)?;
    let dependency_root = dependency_migration.dependency_root.clone();
    let boundary_payload = FrontierRepositoryBoundaryPayloadV1 {
        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
        mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
        frontier_id: project.frontier_id(),
        identity_root: identity_root.clone(),
        observed_profile_root: profile_root.clone(),
        dependency_root: dependency_root.clone(),
        dependencies,
        previous_identity_event_root: None,
        legacy_identity_preimage_root: Some(legacy_identity_preimage_root),
        administrator_actor_id: actor.id.clone(),
        administrator_public_key: actor.public_key.clone(),
        administrator_algorithm: actor.algorithm.clone(),
        trust_mode: FrontierRepositoryTrustMode::Tofu,
        git_object_format: anchor.git_object_format,
        anchor_git_commit: anchor.git_commit.clone(),
        anchor_git_tree: anchor.git_tree.clone(),
        anchor_event_log_root: anchor.event_log_root.clone(),
        anchor_event_count: anchor.event_count,
        anchor_snapshot_root: anchor.snapshot_root.clone(),
        anchor_snapshot_schema: anchor.snapshot_schema.clone(),
        anchor_proposal_root: anchor.proposal_root.clone(),
        anchor_actor_registry_root: anchor.actor_registry_root.clone(),
        anchor_artifact_registry_root: anchor.artifact_registry_root.clone(),
        anchor_canonical_store_root: anchor.canonical_store_root.clone(),
    };
    let boundary_event =
        new_repository_boundary_event(boundary_payload.clone(), reason, observed_at)?;
    let boundary_event_content_root = event_content_root(&boundary_event);
    let trust_anchor = vela_edge::repository_write::RepositoryTrustAnchorV1 {
        schema: vela_edge::repository_write::REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        frontier_id: project.frontier_id(),
        identity_root: identity_root.clone(),
        boundary_content_root: boundary_event_content_root.clone(),
        administrator_actor_id: actor.id.clone(),
        administrator_public_key: actor.public_key.clone(),
    };
    let trust_anchor_root = trust_anchor.root()?;

    let mut after_project: Project =
        serde_json::from_value(serde_json::to_value(&project).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after_project.events.push(boundary_event.clone());
    let after_profile_project =
        vela_protocol::frontier_repo::project_for_profile_migration_materialization(
            &after_project,
            &profile,
        )?;
    let roots_before = MigrationRootFamily {
        event_log_root: anchor.event_log_root.clone(),
        event_count: anchor.event_count,
        legacy_snapshot_root: anchor.snapshot_root.clone(),
        proposal_root: anchor.proposal_root.clone(),
        actor_registry_root: anchor.actor_registry_root.clone(),
        artifact_registry_root: anchor.artifact_registry_root.clone(),
        canonical_store_root: anchor.canonical_store_root.clone(),
    };
    let semantic_after = MigrationSemanticAfterRoots {
        event_log_root: format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&after_project.events)
        ),
        event_count: anchor.event_count + 1,
        legacy_snapshot_root: format!(
            "sha256:{}",
            vela_protocol::events::snapshot_hash(&after_profile_project)
        ),
        proposal_root: anchor.proposal_root.clone(),
        actor_registry_root: anchor.actor_registry_root.clone(),
        artifact_registry_root: anchor.artifact_registry_root.clone(),
        profile_root: profile_root.clone(),
        identity_root: identity_root.clone(),
        dependency_root: dependency_root.clone(),
        scientific_state_root: vela_protocol::scientific_state::scientific_state_root_v2(
            &after_profile_project,
            &identity_root,
            &dependency_root,
        )?,
    };
    let target_context = vela_edge::target_index::TargetIndexMigrationContextV1 {
        schema: vela_edge::target_index::TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1.to_string(),
        anchor_git_commit: anchor.git_commit.clone(),
        anchor_git_tree: anchor.git_tree.clone(),
        source_event_log_root: anchor.event_log_root.clone(),
        source_event_count: anchor.event_count,
        source_nonlease_event_log_root: format!(
            "sha256:{}",
            vela_protocol::events::nonlease_event_log_hash(&project.events)
        ),
        planned_boundary_event: boundary_event.clone(),
        planned_boundary_event_content_root: boundary_event_content_root.clone(),
        final_roots: vela_edge::target_index::TargetIndexRootsV2 {
            event_log_root: semantic_after.event_log_root.clone(),
            event_count: semantic_after.event_count,
            nonlease_event_log_root: format!(
                "sha256:{}",
                vela_protocol::events::nonlease_event_log_hash(&after_project.events)
            ),
            scientific_state_root: semantic_after.scientific_state_root.clone(),
            proposal_root: semantic_after.proposal_root.clone(),
            identity_root: semantic_after.identity_root.clone(),
            dependency_root: semantic_after.dependency_root.clone(),
            observed_profile_root: semantic_after.profile_root.clone(),
        },
    };
    let target_index = vela_edge::target_index::prepare_target_index_seal_for_migration(
        &frontier,
        target_candidate,
        env!("CARGO_PKG_VERSION"),
        &target_context,
    )?;
    let target_index_bytes = target_index.canonical_json.as_bytes().to_vec();

    let executable =
        std::env::current_exe().map_err(|error| format!("resolve current Vela binary: {error}"))?;
    let blockers = Vec::new();
    let has_config = legacy_config.source_root.is_some();
    let mut plan = RepositoryMigrationPlan {
        schema: MIGRATION_PLAN_SCHEMA.to_string(),
        ok: true,
        command: "migrate".to_string(),
        target: MIGRATION_TARGET.to_string(),
        frontier: frontier.display().to_string(),
        frontier_id: project.frontier_id(),
        git_commit: head,
        git_tree: tree,
        vela_version: env!("CARGO_PKG_VERSION").to_string(),
        vela_binary_path: executable.display().to_string(),
        vela_binary_sha256: vela_signer::contract::file_sha256(&executable)?,
        candidate_profile_path: candidate_path.display().to_string(),
        candidate_profile_source_root: sha256_root(&profile_bytes),
        candidate_profile: profile,
        candidate_profile_root: profile_root,
        legacy_manifest: manifest,
        legacy_manifest_source_root: sha256_root(&manifest_bytes),
        legacy_config,
        dependency_migration,
        signer_actor: actor.id,
        signer_public_key: actor.public_key,
        reason: reason.to_string(),
        observed_at: observed_at.to_string(),
        trust_mode: FrontierRepositoryTrustMode::Tofu,
        boundary_payload,
        boundary_event,
        boundary_event_content_root,
        trust_anchor,
        trust_anchor_root,
        roots_before,
        semantic_after,
        signed_store_root_state: SignedStoreRootState::PendingProtectedSignature,
        target_index,
        touched: migration_touches(
            &after_project
                .events
                .last()
                .expect("boundary event appended")
                .id,
            has_config,
        ),
        blockers,
        ready_for_protected_apply: true,
        plan_root: String::new(),
    };
    plan.plan_root = compute_plan_root(&plan)?;
    verify_plan_root(&plan)?;
    Ok(MigrationPreview {
        plan,
        inputs: MigrationInputs {
            profile_bytes,
            settings_bytes,
            target_index_bytes,
        },
    })
}

fn prepare_signed_delta(
    frontier: &Path,
    preview: &MigrationPreview,
    signed_event: &StateEvent,
) -> Result<PreparedMigrationDelta, String> {
    verify_plan_root(&preview.plan)?;
    let payload = vela_protocol::frontier_repository::verify_repository_boundary_signature_only(
        signed_event,
        &preview.plan.signer_public_key,
    )?;
    if payload != preview.plan.boundary_payload {
        return Err(
            "signed boundary payload differs from the confirmed migration plan".to_string(),
        );
    }
    let mut unsigned = signed_event.clone();
    unsigned.signature = None;
    if serde_json::to_value(&unsigned).map_err(|error| error.to_string())?
        != serde_json::to_value(&preview.plan.boundary_event).map_err(|error| error.to_string())?
    {
        return Err(
            "signed boundary core, reason, actor, or observation time differs from the confirmed migration plan"
                .to_string(),
        );
    }
    let current = vela_protocol::repo::load_from_path(frontier)?;
    let actual_event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&current.events)
    );
    if current.frontier_id() != preview.plan.frontier_id
        || actual_event_root != preview.plan.roots_before.event_log_root
        || current.events.len() as u64 != preview.plan.roots_before.event_count
    {
        return Err("migration source drifted before signed postimage rendering".to_string());
    }
    let mut after: Project =
        serde_json::from_value(serde_json::to_value(&current).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after.events.push(signed_event.clone());
    let mut writes = vela_protocol::frontier_repo::render_visible_repo_files_for_profile_migration(
        frontier,
        &after,
        &preview.plan.candidate_profile,
        &preview.inputs.profile_bytes,
    )?;
    writes.insert(
        format!(".vela/events/{}.json", signed_event.id),
        serde_json::to_vec_pretty(signed_event)
            .map_err(|error| format!("encode signed boundary event: {error}"))?,
    );
    writes.insert(
        "frontier.yaml".to_string(),
        preview.inputs.profile_bytes.clone(),
    );
    writes.insert(
        ".vela/settings.toml".to_string(),
        preview.inputs.settings_bytes.clone(),
    );
    writes.insert(
        "targets.json".to_string(),
        preview.inputs.target_index_bytes.clone(),
    );
    let deletes = preview
        .plan
        .legacy_config
        .source_root
        .as_ref()
        .map(|_| vec![".vela/config.toml".to_string()])
        .unwrap_or_default();
    let signed_canonical_store_root =
        vela_edge::frontier_repository::derive_migration_signed_store_root(
            frontier,
            &preview.plan.git_commit,
            signed_event,
        )?;
    Ok(PreparedMigrationDelta {
        writes,
        deletes,
        signed_canonical_store_root,
    })
}

fn build_repository_signer_request(
    preview: &MigrationPreview,
    profile: &crate::cli_identity::ProtectedSignerProfile,
) -> Result<vela_signer::RepositoryBoundarySignerRequest, String> {
    use rand::RngCore;

    verify_plan_root(&preview.plan)?;
    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    if helper_sha256 != profile.helper_sha256 {
        return Err(format!(
            "installed signer helper {} does not match protected identity pin {}; rerun `vela id protect --user-presence --remove-source-key` to authorize this helper",
            helper_sha256, profile.helper_sha256
        ));
    }
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::RepositoryBoundarySignerRequest {
        schema: vela_signer::REPOSITORY_REQUEST_SCHEMA.to_string(),
        nonce: hex::encode(nonce),
        expires_at: (now
            + chrono::Duration::seconds(
                vela_signer::REPOSITORY_REQUEST_LIFETIME_SECONDS,
            ))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: preview.plan.vela_binary_sha256.clone(),
        helper_sha256,
        frontier_id: preview.plan.frontier_id.clone(),
        frontier_path: preview.plan.frontier.clone(),
        reason: preview.plan.reason.clone(),
        administrator_actor: preview.plan.signer_actor.clone(),
        administrator_public_key: preview.plan.signer_public_key.clone(),
        observed_at: preview.plan.observed_at.clone(),
        boundary_plan_root: preview.plan.plan_root.clone(),
        provider: profile.provider.clone(),
        protection_grade: profile.protection_grade.clone(),
        protection_mode: profile.mode,
        display: vela_signer::RepositoryBoundaryDisplay {
            frontier_name: preview.plan.candidate_profile.name.clone(),
            profile_version: vela_protocol::frontier_profile::FRONTIER_PROFILE_SCHEMA_V1
                .to_string(),
            dependency_summary: format!(
                "{} exact dependencies · {}",
                preview.plan.boundary_payload.dependencies.len(),
                preview.plan.boundary_payload.dependency_root
            ),
            consequence: concat!(
                "temporalize existing repository; first boundary requires an out-of-band pin; ",
                "append one non-scientific identity boundary and install the exact Profile v1, ",
                "settings, and target-index projections without rewriting historical canonical bytes"
            )
            .to_string(),
        },
        event: preview.plan.boundary_event.clone(),
    };
    vela_signer::validate_repository_boundary_request(&request, now)?;
    Ok(request)
}

fn request_protected_repository_signature(
    request: &vela_signer::RepositoryBoundarySignerRequest,
) -> Result<(vela_signer::RepositoryBoundarySignerResponse, StateEvent), String> {
    use std::io::Write;
    use std::process::Stdio;

    let helper = PathBuf::from(&request.vela_binary_path)
        .parent()
        .ok_or_else(|| "running Vela binary has no parent directory".to_string())?
        .join(if cfg!(target_os = "windows") {
            "vela-signer.exe"
        } else {
            "vela-signer"
        });
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("encode repository-boundary signer request: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("approve-repository-boundary")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start pinned signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write repository-boundary signer request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for repository-boundary signer helper: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "signer helper declined or failed: {}",
            crate::cli::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::RepositoryBoundarySignerResponse =
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("decode repository-boundary signer response: {error}"))?;
    vela_signer::validate_repository_boundary_response(request, &response)?;
    let mut signed = request.event.clone();
    signed.signature = Some(response.event_signature.clone());
    vela_protocol::frontier_repository::verify_repository_boundary_signature_only(
        &signed,
        &request.administrator_public_key,
    )?;
    Ok((response, signed))
}

fn migration_read_set(
    frontier: &Path,
    preview: &MigrationPreview,
    project: &Project,
) -> Result<Vec<InputBinding>, String> {
    let mut read_set =
        vec![InputBinding::project_snapshot(project).map_err(|error| error.to_string())?];
    let paths = preview
        .plan
        .target_index
        .input_paths
        .iter()
        .chain(preview.plan.target_index.packet_paths.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    for path in paths {
        read_set.push(
            InputBinding::existing_file(
                frontier,
                RepoPath::parse(path).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(read_set)
}

fn migration_trust_anchor_state(
    trust_home: &Path,
    preview: &MigrationPreview,
) -> Result<
    Option<(
        PathBuf,
        String,
        vela_edge::repository_write::RepositoryTrustAnchorV1,
    )>,
    String,
> {
    let loaded = vela_edge::repository_write::load_repository_trust_anchor_from_home(
        trust_home,
        &preview.plan.frontier_id,
    )?;
    if let Some(existing) = &loaded
        && (existing.anchor != preview.plan.trust_anchor
            || existing.root != preview.plan.trust_anchor_root)
    {
        return Err(format!(
            "a different consumer trust anchor already exists at {}; inspect it before migration",
            existing.path.display()
        ));
    }
    Ok(loaded.map(|existing| (existing.path, existing.root, existing.anchor)))
}

fn execute_confirmed_migration_with_signer<F>(
    frontier: &Path,
    preview: &MigrationPreview,
    request: &vela_signer::RepositoryBoundarySignerRequest,
    trust_home: &Path,
    signer: F,
) -> Result<MigrationExecutionResult, String>
where
    F: FnOnce(
        &vela_signer::RepositoryBoundarySignerRequest,
    ) -> Result<(vela_signer::RepositoryBoundarySignerResponse, StateEvent), String>,
{
    execute_confirmed_migration_with_signer_and_post_commit(
        frontier,
        preview,
        request,
        trust_home,
        signer,
        || Ok(()),
    )
}

fn execute_confirmed_migration_with_signer_and_post_commit<F, H>(
    frontier: &Path,
    preview: &MigrationPreview,
    request: &vela_signer::RepositoryBoundarySignerRequest,
    trust_home: &Path,
    signer: F,
    post_commit: H,
) -> Result<MigrationExecutionResult, String>
where
    F: FnOnce(
        &vela_signer::RepositoryBoundarySignerRequest,
    ) -> Result<(vela_signer::RepositoryBoundarySignerResponse, StateEvent), String>,
    H: FnOnce() -> Result<(), String>,
{
    verify_plan_root(&preview.plan)?;
    if request.boundary_plan_root != preview.plan.plan_root
        || request.frontier_id != preview.plan.frontier_id
        || request.frontier_path != preview.plan.frontier
        || request.event.id != preview.plan.boundary_event.id
    {
        return Err(
            "protected repository-boundary request differs from the confirmed migration plan"
                .to_string(),
        );
    }
    let frontier = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve migration Frontier: {error}"))?;
    if frontier != Path::new(&preview.plan.frontier) {
        return Err(format!(
            "migration destination differs from confirmed plan: expected {}, found {}",
            preview.plan.frontier,
            frontier.display()
        ));
    }
    let initial_trust_anchor = migration_trust_anchor_state(trust_home, preview)?;
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(&frontier)?;
    let ceremony_spec = MigrationCeremonySpec::from_protected_request(request)
        .map_err(|error| error.to_string())?;
    let ceremony =
        FrontierTxn::acquire_migration_ceremony_barrier(&frontier, &journal_dir, ceremony_spec)
            .map_err(|error| error.to_string())?;

    // The recovery lock remains held while the one-shot helper obtains fresh
    // user presence. Cancellation therefore leaves no event, transaction
    // marker, profile, target index, or trust pin.
    let (response, signed_event) = signer(request)?;
    if migration_trust_anchor_state(trust_home, preview)? != initial_trust_anchor {
        return Err(
            "consumer trust-anchor state changed during protected migration approval; zero canonical writes were authorized"
                .to_string(),
        );
    }
    let prepared = prepare_signed_delta(&frontier, preview, &signed_event)?;
    let managed = vela_protocol::repo::ManagedFileSet {
        writes: prepared.writes.clone(),
        deletes: prepared.deletes.iter().cloned().collect(),
    };
    let writes = PlannedWrite::from_managed_files(managed).map_err(|error| error.to_string())?;
    let draft = DeltaDraft::prepare(&frontier, writes).map_err(|error| error.to_string())?;
    let delta_root = draft.delta.root().as_str().to_string();
    let publication_paths = draft
        .delta
        .public_writes()
        .map(|write| write.path.as_str().to_string())
        .collect::<Vec<_>>();
    let barrier = FrontierTxn::authorize_migration_write_barrier(
        ceremony,
        request,
        &response,
        delta_root.clone(),
    )
    .map_err(|error| error.to_string())?;

    let before = vela_protocol::repo::load_from_path(&frontier)?;
    let read_set = migration_read_set(&frontier, preview, &before)?;
    let mut after: Project =
        serde_json::from_value(serde_json::to_value(&before).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after.events.push(signed_event.clone());
    let mut resulting_event_ids = after
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    resulting_event_ids.dedup();
    if resulting_event_ids.len() != after.events.len() {
        return Err("migration would create a duplicate event identifier".to_string());
    }
    let external_anchor = vela_edge::frontier_repository::RepositoryTrustAnchor {
        boundary_content_root: preview.plan.boundary_event_content_root.clone(),
        administrator_public_key: preview.plan.signer_public_key.clone(),
    };
    // All repository-boundary facts are available before the transaction
    // marker is written. Validate them here so an invalid legacy anchor cannot
    // become a completed migration that only reports failure afterward.
    vela_edge::frontier_repository::verify_repository_boundary_context_with_trust_anchor(
        &after,
        &frontier,
        &signed_event,
        Some(&external_anchor),
    )?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": preview.plan.frontier_id,
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))?;
    let operation_id = OperationId::derive("frontier-repo-v1", preview.plan.plan_root.as_bytes());
    let result = serde_json::json!({
        "schema": "vela.frontier-repository-migration-result.internal.v1",
        "plan_root": preview.plan.plan_root,
        "boundary_event_id": signed_event.id,
        "boundary_event_content_root": preview.plan.boundary_event_content_root,
        "signed_canonical_store_root": prepared.signed_canonical_store_root,
        "target_index_root": preview.plan.target_index.index_root,
        "trust_anchor_root": preview.plan.trust_anchor_root,
    });
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Maintenance,
            operation_id: operation_id.clone(),
            request_root: ContentDigest::parse(preview.plan.plan_root.clone())
                .map_err(|error| error.to_string())?,
            frontier: FrontierBinding::new(&frontier, preview.plan.frontier_id.clone(), &layout)
                .map_err(|error| error.to_string())?,
            fixed_time: preview.plan.observed_at.clone(),
            expected_event_log_root: ContentDigest::parse(
                preview.plan.roots_before.event_log_root.clone(),
            )
            .map_err(|error| error.to_string())?,
            resulting_event_log_root: ContentDigest::parse(
                preview.plan.semantic_after.event_log_root.clone(),
            )
            .map_err(|error| error.to_string())?,
            resulting_event_ids,
            read_set,
            result,
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())?;
    let mut transaction = FrontierTxn::prepare_with_migration_barrier(barrier, plan, draft)
        .map_err(|error| error.to_string())?;
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    // Test harnesses use this seam to model a process exit after the durable
    // commit marker but before installation. Production always supplies the
    // zero-effect closure above. Recovery then uses only the journaled plan
    // and blobs; it never repeats signing or semantic planning.
    post_commit()?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;

    let installed = vela_edge::repository_write::install_repository_trust_anchor_from_home(
        trust_home,
        &preview.plan.trust_anchor,
    )
    .map_err(|error| {
        format!(
            "migration committed, but the confirmed local trust anchor was not installed: {error}; recover by running `vela frontier trust pin {} --boundary-root {} --json` and confirming that exact plan",
            frontier.display(),
            preview.plan.boundary_event_content_root
        )
    })?;
    if installed.root != preview.plan.trust_anchor_root
        || installed.anchor != preview.plan.trust_anchor
    {
        return Err(
            "migration committed, but the installed local trust anchor differs from the confirmed plan; inspect the protected trust store before any further write"
                .to_string(),
        );
    }

    let migrated = vela_protocol::repo::load_from_path(&frontier)?;
    let replay = vela_protocol::reducer::verify_replay(&migrated);
    if !replay.ok {
        return Err(format!(
            "migration committed but exact replay failed with {} difference(s); preserve the completed journal and inspect before committing Git",
            replay.diffs.len()
        ));
    }
    match vela_protocol::frontier_repo::read_repository_profile(&frontier)? {
        Some(FrontierProfileFile::V1(profile))
            if profile.profile_root()? == preview.plan.candidate_profile_root => {}
        _ => {
            return Err(
                "migration committed but the installed Profile v1 does not match the confirmed candidate"
                    .to_string(),
            );
        }
    }
    let boundary = migrated
        .events
        .iter()
        .find(|event| event.id == signed_event.id)
        .ok_or_else(|| {
            "migration committed but the signed boundary event is missing".to_string()
        })?;
    vela_edge::frontier_repository::verify_repository_boundary_context_with_trust_anchor(
        &migrated,
        &frontier,
        boundary,
        Some(&external_anchor),
    )?;
    let target_assessment = vela_edge::target_index::assess_target_index_with_trust_anchor(
        &migrated,
        &frontier,
        Some(&external_anchor),
    )?
    .ok_or_else(|| "migration committed but targets.json is missing".to_string())?;
    let target_codes = target_assessment.all_codes();
    let only_expected_untracked_output =
        target_codes.as_slice() == [vela_edge::target_index::CODE_OUTPUT_NOT_TRACKED];
    if target_assessment.index_root() != preview.plan.target_index.index_root
        || (!target_codes.is_empty() && !only_expected_untracked_output)
    {
        return Err(format!(
            "migration committed but Target Index v2 verification failed: expected {}, found {} with {:?}",
            preview.plan.target_index.index_root,
            target_assessment.index_root(),
            target_codes
        ));
    }
    let publication = manual_uncommitted_exact_delta(
        &frontier,
        operation_id.as_str(),
        &delta_root,
        &publication_paths,
    );

    Ok(MigrationExecutionResult {
        schema: "vela.frontier-repository-migration-execution.v1".to_string(),
        ok: true,
        command: "migrate".to_string(),
        plan_root: preview.plan.plan_root.clone(),
        operation_id: operation_id.as_str().to_string(),
        event_id: signed_event.id,
        event_log_root: preview.plan.semantic_after.event_log_root.clone(),
        event_count: preview.plan.semantic_after.event_count,
        canonical_delta_root: delta_root,
        signed_store_root_state: SignedStoreRootState::Exact {
            canonical_store_root: prepared.signed_canonical_store_root,
        },
        trust_anchor_root: installed.root,
        trust_anchor_path: installed.path.display().to_string(),
        target_index_root: preview.plan.target_index.index_root.clone(),
        publication,
        replay_ok: true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_migrate(
    frontier: &Path,
    target: &str,
    apply: bool,
    profile: &Path,
    dependency_input: Option<&Path>,
    target_candidate: &Path,
    actor: &str,
    reason: &str,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("migrate", json_out);
    if target != MIGRATION_TARGET {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            &format!("unsupported migration target {target:?}"),
            Some("use --to frontier-repo-v1"),
        );
    }
    if apply {
        let confirm_root = confirm_root.unwrap_or_else(|| {
            fail_return("migration --apply requires --confirm-root and --confirm-at")
        });
        let confirm_at = confirm_at.unwrap_or_else(|| {
            fail_return("migration --apply requires --confirm-root and --confirm-at")
        });
        crate::decision_plan::validate_scripted_confirmation_time(confirm_at)
            .unwrap_or_else(|error| fail_return(&format!("{}: {}", error.code, error.message)));
        let preview = prepare_migration(
            frontier,
            profile,
            dependency_input,
            target_candidate,
            actor,
            reason,
            confirm_at,
        )
        .unwrap_or_else(|error| fail_return(&error));
        if preview.plan.plan_root != confirm_root {
            fail_return::<()>(&format!(
                "migration confirmation root mismatch: supplied {confirm_root}, current {}",
                preview.plan.plan_root
            ));
        }
        let protected = crate::cli_identity::protected_signer_profile()
            .unwrap_or_else(|error| fail_return(&error));
        let request = build_repository_signer_request(&preview, &protected)
            .unwrap_or_else(|error| fail_return(&error));
        let trust_home = crate::frontier_txn::operating_system_account_home()
            .unwrap_or_else(|error| fail_return(&error.to_string()));
        let result = execute_confirmed_migration_with_signer(
            frontier,
            &preview,
            &request,
            &trust_home,
            request_protected_repository_signature,
        )
        .unwrap_or_else(|error| fail_return(&error));
        if json_out {
            print_json(&result);
        } else {
            println!(
                "{}",
                migration_result_human(&preview.plan.frontier, &result)
            );
        }
        return;
    }

    if confirm_root.is_some() || confirm_at.is_some() {
        fail_return::<()>(
            "--confirm-root/--confirm-at are valid only with --apply; preview is key-free",
        );
    }
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let preview = prepare_migration(
        frontier,
        profile,
        dependency_input,
        target_candidate,
        actor,
        reason,
        &observed_at,
    )
    .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(
            &migration_preview_json(&preview.plan).unwrap_or_else(|error| fail_return(&error)),
        );
    } else {
        println!("migrate · protected preview · {}", preview.plan.frontier);
        println!("  target: {}", preview.plan.target);
        println!("  signer: {}", preview.plan.signer_actor);
        println!("  event: {}", preview.plan.boundary_event.id);
        println!("  plan root: {}", preview.plan.plan_root);
        println!("  confirm at: {}", preview.plan.observed_at);
        println!("  writes now: none");
        for blocker in &preview.plan.blockers {
            println!("  blocked: {} · {}", blocker.code, blocker.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use std::collections::BTreeMap;
    use vela_protocol::frontier_profile::{
        FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileLicenseV1, FrontierProfileScopeV1,
    };
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};

    const OBSERVED_AT: &str = "2026-07-22T12:00:00Z";
    const RETAINED_POLICY_ID: &str = "vap_0123456789abcdef0123456789abcdef";

    fn run(path: &Path, args: &[&str]) {
        let output = crate::git_hardened::output(path, args).unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn tree_bytes(path: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
            let mut entries = std::fs::read_dir(current)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                if path.file_name().and_then(|name| name.to_str()) == Some(".git") {
                    continue;
                }
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    walk(root, &path, files);
                } else if metadata.is_file() {
                    files.insert(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                        std::fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        walk(path, path, &mut files);
        files
    }

    fn directory_bytes_root(path: &Path) -> String {
        fn walk(root: &Path, current: &Path, paths: &mut Vec<PathBuf>) {
            let mut entries = std::fs::read_dir(current)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    walk(root, &path, paths);
                } else {
                    assert!(
                        metadata.is_file() || metadata.file_type().is_symlink(),
                        "unsupported repository path {}",
                        path.display()
                    );
                    paths.push(path.strip_prefix(root).unwrap().to_path_buf());
                }
            }
        }

        let mut paths = Vec::new();
        walk(path, path, &mut paths);
        paths.sort();
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        for relative in paths {
            let source = path.join(&relative);
            let metadata = std::fs::symlink_metadata(&source).unwrap();
            let display = relative.to_string_lossy().replace('\\', "/");
            digest.update((display.len() as u64).to_be_bytes());
            digest.update(display.as_bytes());
            if metadata.file_type().is_symlink() {
                digest.update(b"symlink\0");
                let target = std::fs::read_link(&source).unwrap();
                let target = target.to_string_lossy();
                digest.update((target.len() as u64).to_be_bytes());
                digest.update(target.as_bytes());
            } else {
                digest.update(b"file\0");
                digest.update(metadata.len().to_be_bytes());
                let mut file = std::fs::File::open(&source).unwrap();
                use std::io::Read;
                loop {
                    let read = file.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    digest.update(&buffer[..read]);
                }
            }
        }
        format!("sha256:{}", hex::encode(digest.finalize()))
    }

    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let metadata = std::fs::symlink_metadata(&source_path).unwrap();
            if metadata.is_dir() {
                copy_tree(&source_path, &destination_path);
            } else if metadata.is_file() {
                std::fs::copy(&source_path, &destination_path).unwrap();
            } else {
                panic!(
                    "migration fixture copy encountered unsupported path {}",
                    source_path.display()
                );
            }
        }
    }

    fn preboundary_canonical_bytes(path: &Path) -> BTreeMap<String, Vec<u8>> {
        tree_bytes(path)
            .into_iter()
            .filter(|(relative, _)| {
                relative.starts_with(".vela/events/")
                    || relative.starts_with(".vela/proposals/")
                    || relative == ".vela/actors.json"
                    || relative.starts_with("records/receipts/sha256/")
                    || relative == &format!(".vela/policies/{RETAINED_POLICY_ID}.json")
                    || relative == &format!(".vela/policies/{RETAINED_POLICY_ID}.sig.json")
                    || relative == "artifacts/evidence/migration-fixture.bin"
            })
            .collect()
    }

    fn protected_request_for_test(
        preview: &MigrationPreview,
    ) -> vela_signer::RepositoryBoundarySignerRequest {
        let now = chrono::Utc::now();
        let request = vela_signer::RepositoryBoundarySignerRequest {
            schema: vela_signer::REPOSITORY_REQUEST_SCHEMA.to_string(),
            nonce: "11".repeat(32),
            expires_at: (now + chrono::Duration::seconds(90))
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            vela_binary_path: preview.plan.vela_binary_path.clone(),
            vela_binary_sha256: preview.plan.vela_binary_sha256.clone(),
            helper_sha256: format!("sha256:{}", "22".repeat(32)),
            frontier_id: preview.plan.frontier_id.clone(),
            frontier_path: preview.plan.frontier.clone(),
            reason: preview.plan.reason.clone(),
            administrator_actor: preview.plan.signer_actor.clone(),
            administrator_public_key: preview.plan.signer_public_key.clone(),
            observed_at: preview.plan.observed_at.clone(),
            boundary_plan_root: preview.plan.plan_root.clone(),
            provider: "os_store".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: vela_signer::ProtectionMode::Session,
            display: vela_signer::RepositoryBoundaryDisplay {
                frontier_name: preview.plan.candidate_profile.name.clone(),
                profile_version: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
                dependency_summary: format!(
                    "{} exact dependencies · {}",
                    preview.plan.boundary_payload.dependencies.len(),
                    preview.plan.boundary_payload.dependency_root
                ),
                consequence: concat!(
                    "temporalize existing repository; first boundary requires an out-of-band pin; ",
                    "append the exact protected boundary and install Profile v1"
                )
                .to_string(),
            },
            event: preview.plan.boundary_event.clone(),
        };
        vela_signer::validate_repository_boundary_request(&request, now).unwrap();
        request
    }

    fn approve_for_test(
        request: &vela_signer::RepositoryBoundarySignerRequest,
        key: &SigningKey,
    ) -> Result<(vela_signer::RepositoryBoundarySignerResponse, StateEvent), String> {
        let signature = sign_event(&request.event, key)?;
        let response = vela_signer::RepositoryBoundarySignerResponse {
            schema: vela_signer::REPOSITORY_RESPONSE_SCHEMA.to_string(),
            request_root: vela_signer::repository_boundary_request_root(request)?,
            administrator_public_key: request.administrator_public_key.clone(),
            helper_version: env!("CARGO_PKG_VERSION").to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            protection_mode: request.protection_mode,
            event_id: request.event.id.clone(),
            event_signature: signature.clone(),
        };
        let mut signed = request.event.clone();
        signed.signature = Some(signature);
        Ok((response, signed))
    }

    struct Fixture {
        frontier: tempfile::TempDir,
        profile_file: tempfile::NamedTempFile,
        target_candidate_file: tempfile::NamedTempFile,
        key: SigningKey,
        actor: ActorRecord,
    }

    fn fixture() -> Fixture {
        let frontier = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize_minimal(
            frontier.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "Migration fixture",
                initialize_git: true,
            },
        )
        .unwrap();
        run(frontier.path(), &["config", "user.name", "Vela Test"]);
        run(
            frontier.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        let key = SigningKey::from_bytes(&[41; 32]);
        let actor = ActorRecord {
            id: "reviewer:migration".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let mut project = vela_protocol::repo::load_from_path(frontier.path()).unwrap();
        project.actors = vec![actor.clone()];
        let mut historical = StateEvent {
            schema: vela_protocol::events::EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "frontier.observation_reviewed".into(),
            target: vela_protocol::events::StateTarget {
                r#type: "frontier".to_string(),
                id: project.frontier_id(),
            },
            actor: vela_protocol::events::StateActor {
                r#type: "agent".to_string(),
                id: "agent:migration-fixture".to_string(),
            },
            timestamp: "2026-07-21T00:00:00Z".to_string(),
            reason: "Retain one exact legacy event.".to_string(),
            before_hash: vela_protocol::events::NULL_HASH.to_string(),
            after_hash: vela_protocol::events::NULL_HASH.to_string(),
            payload: serde_json::json!({
                "proposal_id": "vpr_0123456789abcdef",
                "proposal_kind": "research_trace.review",
                "status": "accepted"
            }),
            caveats: Vec::new(),
            signature: None,
        };
        historical.id = vela_protocol::events::compute_event_id(&historical);
        project.events = vec![historical];
        let historical_snapshot_root =
            format!("sha256:{}", vela_protocol::events::snapshot_hash(&project));
        let evidence_bytes = b"exact retained migration evidence\n";
        let evidence_sha256 = hex::encode(Sha256::digest(evidence_bytes));
        let producer_key = SigningKey::from_bytes(&[42; 32]);
        let producer_identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:migration-fixture".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-21T00:00:01Z".to_string(),
            },
            &producer_key,
        )
        .unwrap();
        let receipt = ReceiptBuilder::build(
            ReceiptInput::new(
                "The retained migration fixture preserves one exact evidence object.".to_string(),
                "computational".to_string(),
                "exact".to_string(),
                vec![
                    ArtifactInput::new(
                        "artifacts/evidence/migration-fixture.bin".to_string(),
                        "witness".to_string(),
                        Some(evidence_sha256),
                        None,
                    )
                    .unwrap(),
                ],
                vec!["This fixture establishes migration byte preservation only.".to_string()],
                Vec::new(),
                producer_identity.actor_id.clone(),
                "2026-07-21T00:00:02Z".to_string(),
                format!(
                    "sha256:{}",
                    vela_protocol::events::event_log_hash(&project.events)
                ),
                ".".to_string(),
                format!("vop_{}", "b".repeat(64)),
                "urn:vela:policy:none".to_string(),
            )
            .unwrap(),
            &producer_identity,
        )
        .unwrap();
        let receipt_root = receipt.canonical_root().unwrap();
        let receipt_path = format!(
            "records/receipts/sha256/{}.json",
            receipt_root.strip_prefix("sha256:").unwrap()
        );
        let proposal = vela_protocol::proposals::new_proposal_at(
            "research_trace.review",
            vela_protocol::events::StateTarget {
                r#type: "frontier".to_string(),
                id: project.frontier_id(),
            },
            producer_identity.actor_id.clone(),
            "agent",
            "Retain one pending migration proposal.",
            serde_json::json!({
                "legacy_snapshot_hash": historical_snapshot_root,
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": receipt_root,
                    "receipt_path": receipt_path,
                    "record_id": "vrc_0123456789abcdef",
                    "operation_id": format!("vop_{}", "b".repeat(64)),
                    "review_material_path": "records/review/sha256/migration-fixture.json"
                }
            }),
            Vec::new(),
            vec!["Pending review remains non-authoritative.".to_string()],
            "2026-07-21T00:00:03Z",
        );
        project.proposals = vec![proposal];
        vela_protocol::repo::save_to_path(frontier.path(), &project).unwrap();
        let evidence_path = frontier
            .path()
            .join("artifacts/evidence/migration-fixture.bin");
        std::fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
        std::fs::write(&evidence_path, evidence_bytes).unwrap();
        let receipt_file = frontier.path().join(&receipt_path);
        std::fs::create_dir_all(receipt_file.parent().unwrap()).unwrap();
        std::fs::write(&receipt_file, receipt.canonical_bytes().unwrap()).unwrap();
        let policy_dir = frontier.path().join(".vela/policies");
        std::fs::create_dir_all(&policy_dir).unwrap();
        std::fs::write(
            policy_dir.join(format!("{RETAINED_POLICY_ID}.json")),
            format!(
                "{{\"schema\":\"vela.test-policy-history.v1\",\"legacy_snapshot_hash\":\"{historical_snapshot_root}\"}}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            policy_dir.join(format!("{RETAINED_POLICY_ID}.sig.json")),
            b"{\"schema\":\"vela.test-policy-signature-history.v1\",\"signature\":\"retained-fixture\"}\n",
        )
        .unwrap();
        run(frontier.path(), &["add", "."]);
        run(
            frontier.path(),
            &["commit", "-qm", "legacy migration anchor"],
        );

        let profile = FrontierProfileV1 {
            schema: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            frontier_id: project.frontier_id(),
            name: "Migration fixture".to_string(),
            summary: "Evaluate one exact migration contract.".to_string(),
            scope: FrontierProfileScopeV1 {
                question: "Can this exact Frontier migrate without rewriting history?".to_string(),
                includes: vec!["The anchored canonical repository.".to_string()],
                excludes: vec!["Any scientific decision.".to_string()],
            },
            maintainers: vec![actor.id.clone()],
            license: FrontierProfileLicenseV1 {
                content: "CC-BY-4.0".to_string(),
                code: "Apache-2.0".to_string(),
                data: "varies".to_string(),
            },
        };
        let mut profile_file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        profile_file
            .write_all(serde_yaml::to_string(&profile).unwrap().as_bytes())
            .unwrap();
        let packet_path = frontier.path().join("work/packets/migration.json");
        std::fs::create_dir_all(packet_path.parent().unwrap()).unwrap();
        std::fs::write(
            &packet_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "vela.test-migration-packet.v1",
                "objective": "Preserve one bounded migration target."
            }))
            .unwrap(),
        )
        .unwrap();
        let source_commit = git(frontier.path(), &["rev-parse", "HEAD^{commit}"]).unwrap();
        let candidate = serde_json::json!({
            "schema": vela_edge::target_index::TARGET_INDEX_CANDIDATE_SCHEMA_V1,
            "frontier_id": project.frontier_id(),
            "source": {
                "git_commit": source_commit,
                "input_paths": ["README.md"]
            },
            "targets": [{
                "id": "migration:1",
                "title": "Verify the exact migration fixture",
                "why": "The fixture needs one explicit domain-owned target.",
                "state": "open",
                "rank": 1,
                "objective": "Preserve the exact candidate and packet roots.",
                "labels": ["migration"],
                "packet": {
                    "schema": "vela.test-migration-packet.v1",
                    "path": "work/packets/migration.json"
                }
            }]
        });
        let mut target_candidate_file = tempfile::NamedTempFile::new().unwrap();
        target_candidate_file
            .write_all(&serde_json::to_vec_pretty(&candidate).unwrap())
            .unwrap();
        Fixture {
            frontier,
            profile_file,
            target_candidate_file,
            key,
            actor,
        }
    }

    #[test]
    fn migration_frontier_repo_v1_preview_is_zero_writes_and_root_binds_inputs() {
        let fixture = fixture();
        let before = tree_bytes(fixture.frontier.path());
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        assert_eq!(tree_bytes(fixture.frontier.path()), before);
        assert!(preview.plan.ok);
        assert_eq!(preview.plan.command, "migrate");
        assert!(preview.plan.ready_for_protected_apply);
        assert!(preview.plan.blockers.is_empty());
        assert_eq!(
            preview.plan.roots_before.proposal_root,
            preview.plan.semantic_after.proposal_root
        );
        assert_eq!(
            preview.plan.roots_before.actor_registry_root,
            preview.plan.semantic_after.actor_registry_root
        );
        assert_eq!(
            preview.plan.roots_before.artifact_registry_root,
            preview.plan.semantic_after.artifact_registry_root
        );
        assert_eq!(
            preview.plan.semantic_after.event_count,
            preview.plan.roots_before.event_count + 1
        );
        assert_eq!(
            preview.plan.signed_store_root_state,
            SignedStoreRootState::PendingProtectedSignature
        );

        let changed_reason = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "A different exact reason",
            OBSERVED_AT,
        )
        .unwrap();
        assert_ne!(preview.plan.plan_root, changed_reason.plan.plan_root);
    }

    #[test]
    fn migration_frontier_repo_v1_json_preview_is_compact_and_keeps_exact_roots() {
        let fixture = fixture();
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let projection = migration_preview_json(&preview.plan).unwrap();

        assert_eq!(
            projection["schema"],
            serde_json::json!(MIGRATION_PREVIEW_SCHEMA)
        );
        assert_eq!(
            projection["plan_schema"],
            serde_json::json!(MIGRATION_PLAN_SCHEMA)
        );
        assert_eq!(
            projection["plan_root"],
            serde_json::json!(preview.plan.plan_root)
        );
        assert_eq!(
            projection["target_index"]["candidate_root"],
            serde_json::json!(preview.plan.target_index.candidate_root)
        );
        assert_eq!(
            projection["target_index"]["index_root"],
            serde_json::json!(preview.plan.target_index.index_root)
        );
        assert_eq!(projection["target_index"]["packet_count"], 1);
        assert_eq!(projection["target_index"]["index"]["target_count"], 1);
        assert_eq!(
            projection["target_index"]["index"]["target_state_counts"]["open"],
            1
        );
        assert!(projection["target_index"].get("canonical_json").is_none());
        assert!(projection["target_index"].get("packet_paths").is_none());
        assert!(projection["target_index"]["index"].get("targets").is_none());
        assert!(
            serde_json::to_vec(&projection).unwrap().len()
                < serde_json::to_vec(&preview.plan).unwrap().len()
        );
    }

    #[test]
    fn migration_frontier_repo_v1_fake_signer_prepares_only_the_safe_delta() {
        let fixture = fixture();
        let before = tree_bytes(fixture.frontier.path());
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let mut event = preview.plan.boundary_event.clone();
        event.signature = Some(sign_event(&event, &fixture.key).unwrap());
        let delta = prepare_signed_delta(fixture.frontier.path(), &preview, &event).unwrap();
        assert_eq!(tree_bytes(fixture.frontier.path()), before);
        assert!(delta.writes.len() >= 4);
        assert!(delta.writes.contains_key("frontier.yaml"));
        assert!(delta.writes.contains_key(".vela/settings.toml"));
        assert!(delta.writes.contains_key("targets.json"));
        assert!(
            delta
                .writes
                .contains_key(&format!(".vela/events/{}.json", event.id))
        );
        let frontier_projection: serde_json::Value =
            serde_json::from_slice(&delta.writes["frontier.json"]).unwrap();
        assert_eq!(
            frontier_projection["_meta"]["schema"],
            "vela.frontier_state_meta.v1"
        );
        let proof_projection: serde_json::Value =
            serde_json::from_slice(&delta.writes["proof/latest.json"]).unwrap();
        assert_eq!(proof_projection["schema"], "vela.frontier_repo_proof.v1");
        assert_eq!(delta.deletes, vec![".vela/config.toml"]);
        assert!(delta.signed_canonical_store_root.starts_with("sha256:"));

        let mut drifted = event;
        drifted.reason = "drifted after approval".to_string();
        drifted.id = vela_protocol::events::compute_event_id(&drifted);
        drifted.signature = Some(sign_event(&drifted, &fixture.key).unwrap());
        assert!(prepare_signed_delta(fixture.frontier.path(), &preview, &drifted).is_err());
    }

    #[test]
    fn migration_frontier_repo_v1_injected_signer_applies_one_exact_transaction() {
        let fixture = fixture();
        let historical_events = std::fs::read_dir(fixture.frontier.path().join(".vela/events"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let request = protected_request_for_test(&preview);
        let trust_home = tempfile::tempdir().unwrap();
        let key = fixture.key.clone();
        let result = execute_confirmed_migration_with_signer(
            fixture.frontier.path(),
            &preview,
            &request,
            trust_home.path(),
            |request| {
                let signature = sign_event(&request.event, &key)?;
                let response = vela_signer::RepositoryBoundarySignerResponse {
                    schema: vela_signer::REPOSITORY_RESPONSE_SCHEMA.to_string(),
                    request_root: vela_signer::repository_boundary_request_root(request)?,
                    administrator_public_key: request.administrator_public_key.clone(),
                    helper_version: env!("CARGO_PKG_VERSION").to_string(),
                    helper_sha256: request.helper_sha256.clone(),
                    provider: request.provider.clone(),
                    protection_grade: request.protection_grade.clone(),
                    approved_at: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                    protection_mode: request.protection_mode,
                    event_id: request.event.id.clone(),
                    event_signature: signature.clone(),
                };
                let mut signed = request.event.clone();
                signed.signature = Some(signature);
                Ok((response, signed))
            },
        )
        .unwrap();

        assert!(result.replay_ok);
        assert_eq!(result.event_id, preview.plan.boundary_event.id);
        assert_eq!(
            result.target_index_root,
            preview.plan.target_index.index_root
        );
        assert!(matches!(
            result.signed_store_root_state,
            SignedStoreRootState::Exact { .. }
        ));
        assert!(!fixture.frontier.path().join(".vela/config.toml").exists());
        assert!(
            fixture
                .frontier
                .path()
                .join(".vela/settings.toml")
                .is_file()
        );
        assert!(fixture.frontier.path().join("targets.json").is_file());
        assert!(Path::new(&result.trust_anchor_path).starts_with(trust_home.path()));
        let wire = serde_json::to_value(&result).unwrap();
        assert_eq!(wire["ok"], true);
        assert_eq!(wire["command"], "migrate");
        assert_eq!(wire["publication"]["state"], "uncommitted");
        assert_eq!(wire["canonical_delta_root"], result.canonical_delta_root);
        let publication_reason = wire["publication"]["reason"].as_str().unwrap();
        assert!(publication_reason.contains(&result.operation_id));
        assert!(publication_reason.contains(&result.canonical_delta_root));
        let recovery = wire["publication"]["recovery_command"].as_str().unwrap();
        assert!(recovery.starts_with("git -C "));
        assert!(recovery.contains(" status --short -- "));
        assert!(recovery.contains("'.vela/config.toml'"));
        assert!(recovery.contains("'.vela/settings.toml'"));
        let human = migration_result_human(&preview.plan.frontier, &result);
        assert!(human.contains("Git publication: uncommitted"));
        assert!(human.contains(&result.operation_id));
        assert!(human.contains(&result.canonical_delta_root));
        assert!(human.contains(recovery));
        assert!(!human.contains("not performed"));

        for (name, bytes) in historical_events {
            assert_eq!(
                std::fs::read(fixture.frontier.path().join(".vela/events").join(name)).unwrap(),
                bytes,
                "migration rewrote a historical event"
            );
        }
        let migrated = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        assert_eq!(
            migrated.events.len() as u64,
            preview.plan.roots_before.event_count + 1
        );
        assert!(
            vela_protocol::frontier_repo::layout_issues(fixture.frontier.path(), &migrated,)
                .is_empty(),
            "a completed migration must install strict-clean derived Profile v1 postimages"
        );
        assert_eq!(
            migrated
                .events
                .iter()
                .find(|event| event.id == result.event_id)
                .unwrap()
                .signature
                .as_deref()
                .map(str::len),
            Some(131)
        );
    }

    #[test]
    fn migration_rejects_exact_clone_destination_substitution_before_signing() {
        let fixture = fixture();
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let request = protected_request_for_test(&preview);
        let clone_parent = tempfile::tempdir().unwrap();
        let clone = clone_parent.path().join("substituted-frontier");
        copy_tree(fixture.frontier.path(), &clone);
        assert_eq!(
            git(&clone, &["rev-parse", "HEAD"]).unwrap(),
            preview.plan.git_commit,
            "regression requires a destination with the exact confirmed Git anchor"
        );
        let clone_before = tree_bytes(&clone);
        let trust_home = tempfile::tempdir().unwrap();
        let signer_calls = std::cell::Cell::new(0_u8);

        let error = execute_confirmed_migration_with_signer(
            &clone,
            &preview,
            &request,
            trust_home.path(),
            |_| {
                signer_calls.set(signer_calls.get() + 1);
                Err("protected signer must not be called for a substituted checkout".to_string())
            },
        )
        .unwrap_err();

        assert!(
            error.contains("migration destination differs from confirmed plan"),
            "{error}"
        );
        assert_eq!(signer_calls.get(), 0);
        assert_eq!(tree_bytes(&clone), clone_before);
        assert!(
            std::fs::read_dir(trust_home.path())
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn migration_preserves_all_preboundary_canonical_bytes() {
        let fixture = fixture();
        let before = preboundary_canonical_bytes(fixture.frontier.path());
        assert!(
            before.keys().any(|path| path.starts_with(".vela/events/"))
                && before
                    .keys()
                    .any(|path| path.starts_with(".vela/proposals/"))
                && before.contains_key(".vela/actors.json")
                && before
                    .keys()
                    .any(|path| path.starts_with("records/receipts/sha256/"))
                && before.contains_key(&format!(".vela/policies/{RETAINED_POLICY_ID}.json"))
                && before.contains_key(&format!(".vela/policies/{RETAINED_POLICY_ID}.sig.json"))
                && before.contains_key("artifacts/evidence/migration-fixture.bin"),
            "fixture must cover events, proposals, actors, Receipts, policy history, and evidence"
        );
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let request = protected_request_for_test(&preview);
        let trust_home = tempfile::tempdir().unwrap();
        let key = fixture.key.clone();
        execute_confirmed_migration_with_signer(
            fixture.frontier.path(),
            &preview,
            &request,
            trust_home.path(),
            |request| approve_for_test(request, &key),
        )
        .unwrap();

        let after = preboundary_canonical_bytes(fixture.frontier.path());
        for (path, bytes) in before {
            assert_eq!(
                after.get(&path),
                Some(&bytes),
                "migration rewrote pre-boundary canonical bytes at {path}"
            );
        }
    }

    #[test]
    fn migration_cancellation_zero_writes_and_crash_recovers() {
        let cancellation_fixture = fixture();
        let before = tree_bytes(cancellation_fixture.frontier.path());
        let preview = prepare_migration(
            cancellation_fixture.frontier.path(),
            cancellation_fixture.profile_file.path(),
            None,
            cancellation_fixture.target_candidate_file.path(),
            &cancellation_fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let request = protected_request_for_test(&preview);
        let trust_home = tempfile::tempdir().unwrap();
        let error = execute_confirmed_migration_with_signer(
            cancellation_fixture.frontier.path(),
            &preview,
            &request,
            trust_home.path(),
            |_| Err("user cancelled protected approval".to_string()),
        )
        .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");

        let after = tree_bytes(cancellation_fixture.frontier.path());
        for (path, bytes) in before {
            assert_eq!(
                after.get(&path),
                Some(&bytes),
                "protected cancellation changed {path}"
            );
        }
        assert!(
            !cancellation_fixture
                .frontier
                .path()
                .join(".vela/settings.toml")
                .exists()
        );
        assert!(
            !cancellation_fixture
                .frontier
                .path()
                .join("targets.json")
                .exists()
        );
        assert!(
            !cancellation_fixture
                .frontier
                .path()
                .join(format!(
                    ".vela/events/{}.json",
                    preview.plan.boundary_event.id
                ))
                .exists()
        );
        let journal = cancellation_fixture
            .frontier
            .path()
            .join(".vela/operation-journals");
        if journal.is_dir() {
            assert!(
                std::fs::read_dir(journal)
                    .unwrap()
                    .filter_map(Result::ok)
                    .all(|entry| !entry.file_name().to_string_lossy().starts_with("vop_")),
                "cancellation created a transaction journal"
            );
        }

        let crash_fixture = fixture();
        let retained_before = preboundary_canonical_bytes(crash_fixture.frontier.path());
        let crash_preview = prepare_migration(
            crash_fixture.frontier.path(),
            crash_fixture.profile_file.path(),
            None,
            crash_fixture.target_candidate_file.path(),
            &crash_fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let crash_request = protected_request_for_test(&crash_preview);
        let crash_trust_home = tempfile::tempdir().unwrap();
        let crash_key = crash_fixture.key.clone();
        let mut expected_signed_event = crash_preview.plan.boundary_event.clone();
        expected_signed_event.signature =
            Some(sign_event(&expected_signed_event, &crash_key).unwrap());
        let expected_delta = prepare_signed_delta(
            crash_fixture.frontier.path(),
            &crash_preview,
            &expected_signed_event,
        )
        .unwrap();
        let error = execute_confirmed_migration_with_signer_and_post_commit(
            crash_fixture.frontier.path(),
            &crash_preview,
            &crash_request,
            crash_trust_home.path(),
            |request| approve_for_test(request, &crash_key),
            || Err("injected process exit after durable commit marker".to_string()),
        )
        .unwrap_err();
        assert!(error.contains("injected process exit"), "{error}");
        assert!(
            !std::fs::read_to_string(crash_fixture.frontier.path().join("frontier.yaml"))
                .unwrap()
                .contains(FRONTIER_PROFILE_SCHEMA_V1),
            "post-marker crash unexpectedly installed the profile before recovery"
        );

        let journal_dir =
            crate::workflow::frontier_transaction_journal_dir(crash_fixture.frontier.path())
                .unwrap();
        let operation_id =
            OperationId::derive("frontier-repo-v1", crash_preview.plan.plan_root.as_bytes());
        assert_eq!(
            FrontierTxn::recover(crash_fixture.frontier.path(), &journal_dir, &operation_id)
                .unwrap(),
            crate::frontier_txn::RecoveryOutcome::Completed
        );
        assert_eq!(
            FrontierTxn::recover(crash_fixture.frontier.path(), &journal_dir, &operation_id)
                .unwrap(),
            crate::frontier_txn::RecoveryOutcome::AlreadyCompleted
        );
        for (path, expected) in &expected_delta.writes {
            assert_eq!(
                std::fs::read(crash_fixture.frontier.path().join(path)).unwrap(),
                *expected,
                "transaction recovery installed the wrong exact postimage at {path}"
            );
        }
        for path in &expected_delta.deletes {
            assert!(
                !crash_fixture.frontier.path().join(path).exists(),
                "transaction recovery failed to install the exact deletion at {path}"
            );
        }
        let retained_after = preboundary_canonical_bytes(crash_fixture.frontier.path());
        for (path, bytes) in retained_before {
            assert_eq!(
                retained_after.get(&path),
                Some(&bytes),
                "transaction recovery rewrote pre-boundary canonical bytes at {path}"
            );
        }
        let migrated = vela_protocol::repo::load_from_path(crash_fixture.frontier.path()).unwrap();
        assert!(
            vela_protocol::reducer::verify_replay(&migrated).ok,
            "recovered migration must replay exactly"
        );
        assert_eq!(
            migrated.events.len() as u64,
            crash_preview.plan.roots_before.event_count + 1
        );
        assert!(matches!(
            vela_protocol::frontier_repo::read_repository_profile(crash_fixture.frontier.path())
                .unwrap(),
            Some(FrontierProfileFile::V1(_))
        ));
        let installed = vela_edge::repository_write::install_repository_trust_anchor_from_home(
            crash_trust_home.path(),
            &crash_preview.plan.trust_anchor,
        )
        .unwrap();
        assert_eq!(installed.root, crash_preview.plan.trust_anchor_root);
    }

    #[test]
    fn pending_proposals_and_policy_history_survive_root_v2_without_substitution() {
        let fixture = fixture();
        let project_before = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        let pending_before = project_before
            .proposals
            .iter()
            .find(|proposal| proposal.status == "pending_review")
            .unwrap()
            .clone();
        let proposal_path = fixture
            .frontier
            .path()
            .join(".vela/proposals")
            .join(format!("{}.json", pending_before.id));
        let proposal_bytes = std::fs::read(&proposal_path).unwrap();
        let policy_path = fixture
            .frontier
            .path()
            .join(format!(".vela/policies/{RETAINED_POLICY_ID}.json"));
        let policy_signature_path = fixture
            .frontier
            .path()
            .join(format!(".vela/policies/{RETAINED_POLICY_ID}.sig.json"));
        let policy_bytes = std::fs::read(&policy_path).unwrap();
        let policy_signature_bytes = std::fs::read(&policy_signature_path).unwrap();
        let historical_snapshot_hash = pending_before.payload["legacy_snapshot_hash"]
            .as_str()
            .unwrap()
            .to_string();

        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        assert_ne!(
            historical_snapshot_hash, preview.plan.semantic_after.scientific_state_root,
            "the v2 scientific-state root must not masquerade as the historical snapshot hash"
        );
        let request = protected_request_for_test(&preview);
        let trust_home = tempfile::tempdir().unwrap();
        let key = fixture.key.clone();
        execute_confirmed_migration_with_signer(
            fixture.frontier.path(),
            &preview,
            &request,
            trust_home.path(),
            |request| approve_for_test(request, &key),
        )
        .unwrap();

        assert_eq!(std::fs::read(&proposal_path).unwrap(), proposal_bytes);
        assert_eq!(std::fs::read(&policy_path).unwrap(), policy_bytes);
        assert_eq!(
            std::fs::read(&policy_signature_path).unwrap(),
            policy_signature_bytes
        );
        let migrated = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        let pending_after = migrated
            .proposals
            .iter()
            .find(|proposal| proposal.id == pending_before.id)
            .unwrap();
        assert_eq!(pending_after.status, "pending_review");
        assert_eq!(pending_after, &pending_before);
        assert_eq!(
            pending_after.payload["legacy_snapshot_hash"].as_str(),
            Some(historical_snapshot_hash.as_str())
        );
        assert!(
            !String::from_utf8(policy_bytes)
                .unwrap()
                .contains(&preview.plan.semantic_after.scientific_state_root),
            "migration substituted the v2 root into historical policy bytes"
        );
    }

    #[test]
    #[ignore = "requires VELA_ERDOS_REGRESSION_FRONTIER pointing at the exact read-only Erdős vector"]
    fn erdos_13_pending_proposals_read_only_regression_vector() {
        const EXPECTED_COMMIT: &str = "e79feaeddf2d4c68ce395d2e7daec1e7fae41702";
        const EXPECTED_EVENT_ROOT: &str =
            "sha256:a06797bc0d1b0e3c88a2f97507fe0832661e3992d8df41187a0aa6d3ceee9bde";
        const EXPECTED_SNAPSHOT_ROOT: &str =
            "sha256:1faedc24f040a60a22177b456c74b969a61ce8836082297b1835797a57b4fa56";
        const EXPECTED_PROPOSAL_ROOT: &str =
            "sha256:e69b38037814f2e8ca826942cfc50ab370993889be2913cac1c0b3e77711160f";
        const EXPECTED_PENDING_BYTES_ROOT: &str =
            "sha256:9e7c6cc1de996f34621291c8c5b9378e67d991b44b4989f7d43174a2f771f044";

        let frontier = PathBuf::from(
            std::env::var("VELA_ERDOS_REGRESSION_FRONTIER")
                .expect("set VELA_ERDOS_REGRESSION_FRONTIER to the exact Erdős checkout"),
        );
        let status_before =
            crate::git_hardened::text(&frontier, &["status", "--short", "--untracked-files=all"])
                .unwrap();
        assert!(status_before.is_empty(), "Erdős vector must be clean");
        assert_eq!(
            git(&frontier, &["rev-parse", "HEAD^{commit}"]).unwrap(),
            EXPECTED_COMMIT
        );
        let project = vela_protocol::repo::load_from_path(&frontier).unwrap();
        assert_eq!(
            format!(
                "sha256:{}",
                vela_protocol::events::event_log_hash(&project.events)
            ),
            EXPECTED_EVENT_ROOT
        );
        assert_eq!(
            format!("sha256:{}", vela_protocol::events::snapshot_hash(&project)),
            EXPECTED_SNAPSHOT_ROOT
        );
        assert_eq!(
            format!(
                "sha256:{}",
                vela_protocol::proposals::proposal_state_hash(&project.proposals)
            ),
            EXPECTED_PROPOSAL_ROOT
        );

        let pending = project
            .proposals
            .iter()
            .filter(|proposal| proposal.status == "pending_review")
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 13);
        let mut pending_paths = pending
            .iter()
            .map(|proposal| {
                (
                    format!(".vela/proposals/{}.json", proposal.id),
                    proposal.id.clone(),
                )
            })
            .collect::<Vec<_>>();
        pending_paths.sort();
        let mut pending_bytes = Sha256::new();
        for (path, _) in &pending_paths {
            let bytes = std::fs::read(frontier.join(path)).unwrap();
            pending_bytes.update(path.as_bytes());
            pending_bytes.update([0]);
            pending_bytes.update((bytes.len() as u64).to_be_bytes());
            pending_bytes.update(bytes);
        }
        assert_eq!(
            format!("sha256:{}", hex::encode(pending_bytes.finalize())),
            EXPECTED_PENDING_BYTES_ROOT
        );
        let status_after =
            crate::git_hardened::text(&frontier, &["status", "--short", "--untracked-files=all"])
                .unwrap();
        assert_eq!(
            status_after, status_before,
            "read-only Erdős regression inspection mutated the checkout"
        );
    }

    #[test]
    #[ignore = "requires exact canonical Erdős/Formal checkouts and external migration inputs"]
    fn erdos_formal_historical_dependency_read_only_regression() {
        const ERDOS_COMMIT: &str = "6bcc3f478fdeaaed03579f2463f278035f389fd0";
        const HISTORICAL_FORMAL_COMMIT: &str = "a143c351f8488e0c621598307e248373d9dc3374";
        const HISTORICAL_FORMAL_TREE: &str = "093e84c03a722e5367812a6e6240b1c28042f969";
        const HISTORICAL_FORMAL_SNAPSHOT: &str =
            "sha256:48ec4e84bb4640fa54023db58d7eabc6a713a46b053b6ccc3050414ab18520ec";
        const FORMAL_TEMPORALIZATION_ANCHOR_COMMIT: &str =
            "6056124b436bfefd76f02f6836d951c947189fe6";
        const FORMAL_TEMPORALIZATION_ANCHOR_TREE: &str = "dddc0325ca52ddf56a7a51049cc5f90dd7071d23";
        const FORMAL_TEMPORALIZATION_ANCHOR_SNAPSHOT: &str =
            "sha256:45fa712bd6d9a8d4c8514a7cba107e7f814f2c1368805abd577e762ccb6123a4";
        const FORMAL_FRONTIER_ID: &str = "vfr_97d7d25957384f80";
        const FORMAL_IDENTITY_ROOT: &str =
            "sha256:841f38525cf2f4862c5fdc2217247a189ab8a0414418b8b782a88c9dd0206731";
        const HISTORICAL_FORMAL_SCIENTIFIC_STATE_ROOT: &str =
            "sha256:4924adbbea6dfe288d14af03cf3d544f73c511df6b6ef8b938c8291685101444";
        const FORMAL_ADMINISTRATOR_PUBLIC_KEY: &str =
            "4892f93877e637b5f59af31d9ec6704814842fb278cacb0eb94704baef99455e";
        const ERDOS_EVENT_ROOT: &str =
            "sha256:a06797bc0d1b0e3c88a2f97507fe0832661e3992d8df41187a0aa6d3ceee9bde";
        const ERDOS_SNAPSHOT_ROOT: &str =
            "sha256:1faedc24f040a60a22177b456c74b969a61ce8836082297b1835797a57b4fa56";
        const ERDOS_PROPOSAL_ROOT: &str =
            "sha256:e69b38037814f2e8ca826942cfc50ab370993889be2913cac1c0b3e77711160f";
        const ERDOS_ACTOR_ROOT: &str =
            "sha256:665f3e1c48f0a50fac949681c0af01bdd28de2991f2cdc5cc4cddbe69df6311b";
        const ERDOS_ARTIFACT_ROOT: &str =
            "sha256:3d58619c5cfb7e28de2f344476e35c9f0b80709c996b2a1bfdb2e11496f7e1da";

        fn exact_env_path(name: &str) -> PathBuf {
            let path = PathBuf::from(
                std::env::var(name).unwrap_or_else(|_| panic!("set {name} to the exact input")),
            );
            std::fs::canonicalize(&path)
                .unwrap_or_else(|error| panic!("resolve {name} {}: {error}", path.display()))
        }

        fn git_status(path: &Path) -> String {
            crate::git_hardened::text(path, &["status", "--short", "--untracked-files=all"])
                .unwrap()
        }

        fn strict_blocker_counts(project: &Project, frontier: &Path) -> BTreeMap<String, usize> {
            let mut counts = BTreeMap::new();
            for signal in vela_edge::signals::analyze_at(project, &[], Some(frontier))
                .signals
                .into_iter()
                .filter(|signal| signal.blocks.iter().any(|block| block == "strict_check"))
            {
                *counts.entry(signal.kind).or_default() += 1;
            }
            counts
        }

        let erdos = exact_env_path("VELA_ERDOS_REGRESSION_FRONTIER");
        let formal = exact_env_path("VELA_FORMAL_REGRESSION_FRONTIER");
        let profile = exact_env_path("VELA_ERDOS_REGRESSION_PROFILE");
        let target_candidate = exact_env_path("VELA_ERDOS_REGRESSION_TARGET_CANDIDATE");
        let dependency_input = exact_env_path("VELA_ERDOS_REGRESSION_DEPENDENCY_INPUT");
        for external in [&profile, &target_candidate, &dependency_input] {
            assert!(
                !external.starts_with(&erdos) && !external.starts_with(&formal),
                "migration input {} must remain outside both Frontier checkouts",
                external.display()
            );
        }

        let erdos_status_before = git_status(&erdos);
        let formal_status_before = git_status(&formal);
        assert!(
            erdos_status_before.is_empty(),
            "Erdős checkout must be clean"
        );
        assert!(
            formal_status_before.is_empty(),
            "Formal checkout must be clean"
        );
        let erdos_dot_vela_before = directory_bytes_root(&erdos.join(".vela"));
        let formal_dot_vela_before = directory_bytes_root(&formal.join(".vela"));
        let profile_root_before = sha256_root(&std::fs::read(&profile).unwrap());
        let target_candidate_root_before = sha256_root(&std::fs::read(&target_candidate).unwrap());
        let dependency_input_root_before = sha256_root(&std::fs::read(&dependency_input).unwrap());

        let erdos_head = git(&erdos, &["rev-parse", "HEAD^{commit}"]).unwrap();
        assert_eq!(erdos_head, ERDOS_COMMIT);
        let erdos_project = vela_protocol::repo::load_from_path(&erdos).unwrap();
        let erdos_facts =
            vela_edge::frontier_repository::derive_repository_anchor_facts(&erdos, &erdos_head)
                .unwrap();
        assert_eq!(erdos_facts.event_log_root, ERDOS_EVENT_ROOT);
        assert_eq!(erdos_facts.snapshot_root, ERDOS_SNAPSHOT_ROOT);
        assert_eq!(erdos_facts.proposal_root, ERDOS_PROPOSAL_ROOT);
        assert_eq!(erdos_facts.actor_registry_root, ERDOS_ACTOR_ROOT);
        assert_eq!(erdos_facts.artifact_registry_root, ERDOS_ARTIFACT_ROOT);
        assert_eq!(
            strict_blocker_counts(&erdos_project, &erdos),
            BTreeMap::from([
                ("missing_conditions".to_string(), 1_511),
                ("unsigned_registered_actor".to_string(), 81),
            ])
        );

        let candidate: vela_edge::target_index::TargetIndexCandidateV1 =
            serde_json::from_slice(&std::fs::read(&target_candidate).unwrap()).unwrap();
        candidate.validate().unwrap();
        assert_eq!(candidate.frontier_id, erdos_project.frontier_id());
        assert_eq!(candidate.source.git_commit, erdos_head);
        assert_eq!(candidate.targets.len(), 1_217);
        let candidate_ids = candidate
            .targets
            .iter()
            .map(|target| target.id.clone())
            .collect::<BTreeSet<_>>();
        let expected_ids = (1..=1_217)
            .map(|problem| format!("erdos:{problem}"))
            .collect::<BTreeSet<_>>();
        assert_eq!(candidate_ids, expected_ids);
        for path in candidate
            .source
            .input_paths
            .iter()
            .chain(candidate.targets.iter().map(|target| &target.packet.path))
        {
            assert!(
                erdos.join(path).is_file(),
                "candidate path {path:?} is absent from the exact Erdős checkout"
            );
        }

        let dependency: DependencyMigrationInputV1 =
            serde_json::from_slice(&std::fs::read(&dependency_input).unwrap()).unwrap();
        assert_eq!(dependency.schema, MIGRATION_DEPENDENCY_INPUT_SCHEMA);
        assert_eq!(dependency.entries.len(), 1);
        let entry = &dependency.entries[0];
        assert_eq!(
            entry.legacy,
            LegacyDependencyDescriptorV1::from(&erdos_project.project.dependencies[0])
        );
        assert_eq!(
            std::fs::canonicalize(&entry.repository_path).unwrap(),
            formal
        );
        assert_eq!(
            entry.boundary_content_root,
            entry.trust_anchor.boundary_content_root
        );
        assert_eq!(
            entry.trust_anchor.administrator_public_key,
            FORMAL_ADMINISTRATOR_PUBLIC_KEY
        );
        assert_eq!(entry.exact.frontier_id, FORMAL_FRONTIER_ID);
        assert_eq!(entry.exact.identity_root, FORMAL_IDENTITY_ROOT);
        assert_eq!(
            entry.exact.scientific_state_root,
            HISTORICAL_FORMAL_SCIENTIFIC_STATE_ROOT
        );
        assert_eq!(entry.exact.git_commit, HISTORICAL_FORMAL_COMMIT);
        assert_eq!(entry.exact.git_tree, HISTORICAL_FORMAL_TREE);

        let historical_facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            &formal,
            HISTORICAL_FORMAL_COMMIT,
        )
        .unwrap();
        assert_eq!(historical_facts.git_tree, HISTORICAL_FORMAL_TREE);
        assert_eq!(historical_facts.snapshot_root, HISTORICAL_FORMAL_SNAPSHOT);
        let temporalization_anchor =
            vela_edge::frontier_repository::derive_repository_anchor_facts(
                &formal,
                FORMAL_TEMPORALIZATION_ANCHOR_COMMIT,
            )
            .unwrap();
        assert_eq!(
            temporalization_anchor.git_tree,
            FORMAL_TEMPORALIZATION_ANCHOR_TREE
        );
        assert_eq!(
            temporalization_anchor.snapshot_root,
            FORMAL_TEMPORALIZATION_ANCHOR_SNAPSHOT
        );

        // The external target candidate must independently pass the complete
        // write-free Target Index v2 seal before the dependency trust gate is
        // evaluated. This keeps the pre-ceremony failure specific: an absent
        // Formal boundary grants no dependency authority, but it cannot hide
        // a malformed or incomplete 1,217-target Erdős candidate.
        let (_, _, candidate_profile) = read_candidate_profile(&erdos, &profile).unwrap();
        let observed_profile_root = candidate_profile.profile_root().unwrap();
        let legacy_identity_preimage_root =
            vela_edge::frontier_repository::derive_legacy_identity_preimage_root(&erdos_project)
                .unwrap();
        let origin = LegacyFrontierOriginV1 {
            schema: vela_protocol::frontier_repository::LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: erdos_project.frontier_id(),
            legacy_identity_preimage_root: legacy_identity_preimage_root.clone(),
            git_object_format: erdos_facts.git_object_format,
            anchor_git_commit: erdos_facts.git_commit.clone(),
            anchor_git_tree: erdos_facts.git_tree.clone(),
            anchor_event_log_root: erdos_facts.event_log_root.clone(),
            anchor_event_count: erdos_facts.event_count,
        };
        let identity_root = origin.identity_root().unwrap();
        let mut exact_dependencies = dependency
            .entries
            .iter()
            .map(|entry| entry.exact.clone())
            .collect::<Vec<_>>();
        exact_dependencies.sort_by(|left, right| {
            (&left.frontier_id, &left.identity_root)
                .cmp(&(&right.frontier_id, &right.identity_root))
        });
        let dependency_root =
            vela_protocol::frontier_repository::exact_dependency_root(&exact_dependencies).unwrap();
        let actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
            &erdos_project,
            "reviewer:will-blair",
            OBSERVED_AT,
        )
        .unwrap();
        let planned_payload = FrontierRepositoryBoundaryPayloadV1 {
            schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
            mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
            frontier_id: erdos_project.frontier_id(),
            identity_root: identity_root.clone(),
            observed_profile_root: observed_profile_root.clone(),
            dependency_root: dependency_root.clone(),
            dependencies: exact_dependencies,
            previous_identity_event_root: None,
            legacy_identity_preimage_root: Some(legacy_identity_preimage_root),
            administrator_actor_id: actor.id.clone(),
            administrator_public_key: actor.public_key.clone(),
            administrator_algorithm: actor.algorithm.clone(),
            trust_mode: FrontierRepositoryTrustMode::Tofu,
            git_object_format: erdos_facts.git_object_format,
            anchor_git_commit: erdos_facts.git_commit.clone(),
            anchor_git_tree: erdos_facts.git_tree.clone(),
            anchor_event_log_root: erdos_facts.event_log_root.clone(),
            anchor_event_count: erdos_facts.event_count,
            anchor_snapshot_root: erdos_facts.snapshot_root.clone(),
            anchor_snapshot_schema: erdos_facts.snapshot_schema.clone(),
            anchor_proposal_root: erdos_facts.proposal_root.clone(),
            anchor_actor_registry_root: erdos_facts.actor_registry_root.clone(),
            anchor_artifact_registry_root: erdos_facts.artifact_registry_root.clone(),
            anchor_canonical_store_root: erdos_facts.canonical_store_root.clone(),
        };
        let planned_boundary = new_repository_boundary_event(
            planned_payload,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let planned_boundary_content_root = event_content_root(&planned_boundary);
        let mut after_project: Project = serde_json::from_value(
            serde_json::to_value(&erdos_project).expect("encode exact Erdős project"),
        )
        .expect("clone exact Erdős project");
        after_project.events.push(planned_boundary.clone());
        let target_context = vela_edge::target_index::TargetIndexMigrationContextV1 {
            schema: vela_edge::target_index::TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1.to_string(),
            anchor_git_commit: erdos_facts.git_commit.clone(),
            anchor_git_tree: erdos_facts.git_tree.clone(),
            source_event_log_root: erdos_facts.event_log_root.clone(),
            source_event_count: erdos_facts.event_count,
            source_nonlease_event_log_root: format!(
                "sha256:{}",
                vela_protocol::events::nonlease_event_log_hash(&erdos_project.events)
            ),
            planned_boundary_event: planned_boundary,
            planned_boundary_event_content_root: planned_boundary_content_root,
            final_roots: vela_edge::target_index::TargetIndexRootsV2 {
                event_log_root: format!(
                    "sha256:{}",
                    vela_protocol::events::event_log_hash(&after_project.events)
                ),
                event_count: erdos_facts.event_count + 1,
                nonlease_event_log_root: format!(
                    "sha256:{}",
                    vela_protocol::events::nonlease_event_log_hash(&after_project.events)
                ),
                scientific_state_root: vela_protocol::scientific_state::scientific_state_root_v2(
                    &after_project,
                    &identity_root,
                    &dependency_root,
                )
                .unwrap(),
                proposal_root: erdos_facts.proposal_root.clone(),
                identity_root,
                dependency_root,
                observed_profile_root,
            },
        };
        let sealed_target_index = vela_edge::target_index::prepare_target_index_seal_for_migration(
            &erdos,
            &target_candidate,
            env!("CARGO_PKG_VERSION"),
            &target_context,
        )
        .expect("seal the exact 1,217-target Erdős candidate");
        assert_eq!(sealed_target_index.index.targets.len(), 1_217);
        assert_eq!(sealed_target_index.source.git_commit, ERDOS_COMMIT);
        assert_eq!(
            sealed_target_index.candidate_root,
            target_candidate_root_before
        );
        assert_eq!(
            sealed_target_index
                .index
                .targets
                .iter()
                .map(|target| target.id.clone())
                .collect::<BTreeSet<_>>(),
            expected_ids
        );

        let formal_project = vela_protocol::repo::load_from_path(&formal).unwrap();
        let repository_boundaries = formal_project
            .events
            .iter()
            .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
            .collect::<Vec<_>>();
        let preview = prepare_migration(
            &erdos,
            &profile,
            Some(&dependency_input),
            &target_candidate,
            "reviewer:will-blair",
            "Bind exact legacy repository",
            OBSERVED_AT,
        );
        if repository_boundaries.is_empty() {
            assert_eq!(
                repository_boundaries.len(),
                0,
                "pre-ceremony Formal history must contain exactly zero repository boundaries"
            );
            let error = preview.expect_err(
                "an unmigrated Formal repository must not authenticate the historical pin",
            );
            assert!(
                error.contains("dependency boundary root")
                    && error.contains("resolves to 0 events, expected exactly one"),
                "unexpected pre-ceremony blocker: {error}"
            );
        } else {
            assert_eq!(
                repository_boundaries.len(),
                1,
                "post-ceremony Formal history must contain exactly one repository boundary"
            );
            let formal_boundary = repository_boundaries[0];
            assert_eq!(
                repository_identity_event_content_root(formal_boundary).unwrap(),
                entry.boundary_content_root,
                "the selected boundary must be the exact externally pinned event"
            );
            let formal_boundary_payload =
                vela_protocol::frontier_repository::repository_boundary_payload_from_event_shape(
                    formal_boundary,
                )
                .unwrap();
            assert_eq!(
                formal_boundary_payload.mode,
                FrontierRepositoryBoundaryMode::TemporalizeExisting
            );
            assert_eq!(
                formal_boundary_payload.anchor_git_commit,
                FORMAL_TEMPORALIZATION_ANCHOR_COMMIT
            );
            assert_eq!(
                formal_boundary_payload.anchor_git_tree,
                FORMAL_TEMPORALIZATION_ANCHOR_TREE
            );
            assert_eq!(
                formal_boundary_payload.anchor_snapshot_root,
                FORMAL_TEMPORALIZATION_ANCHOR_SNAPSHOT
            );
            let preview = preview.unwrap();
            assert!(preview.plan.ready_for_protected_apply);
            assert!(preview.plan.blockers.is_empty());
            assert_eq!(preview.plan.git_commit, erdos_head);
            assert_eq!(preview.plan.roots_before.event_log_root, ERDOS_EVENT_ROOT);
            assert_eq!(
                preview.plan.roots_before.legacy_snapshot_root,
                ERDOS_SNAPSHOT_ROOT
            );
            assert_eq!(preview.plan.roots_before.proposal_root, ERDOS_PROPOSAL_ROOT);
            assert_eq!(
                preview.plan.roots_before.actor_registry_root,
                ERDOS_ACTOR_ROOT
            );
            assert_eq!(
                preview.plan.roots_before.artifact_registry_root,
                ERDOS_ARTIFACT_ROOT
            );
            assert_eq!(
                preview.plan.dependency_migration.entries,
                dependency.entries
            );
            assert_eq!(preview.plan.boundary_payload.dependencies.len(), 1);
            assert_eq!(preview.plan.boundary_payload.dependencies[0], entry.exact);
            assert_eq!(
                preview.plan.boundary_payload.dependency_root,
                preview.plan.dependency_migration.dependency_root
            );
        }

        let erdos_after = vela_protocol::repo::load_from_path(&erdos).unwrap();
        assert_eq!(
            strict_blocker_counts(&erdos_after, &erdos),
            BTreeMap::from([
                ("missing_conditions".to_string(), 1_511),
                ("unsigned_registered_actor".to_string(), 81),
            ])
        );
        assert_eq!(git_status(&erdos), erdos_status_before);
        assert_eq!(git_status(&formal), formal_status_before);
        assert_eq!(
            directory_bytes_root(&erdos.join(".vela")),
            erdos_dot_vela_before
        );
        assert_eq!(
            directory_bytes_root(&formal.join(".vela")),
            formal_dot_vela_before
        );
        assert_eq!(
            sha256_root(&std::fs::read(&profile).unwrap()),
            profile_root_before
        );
        assert_eq!(
            sha256_root(&std::fs::read(&target_candidate).unwrap()),
            target_candidate_root_before
        );
        assert_eq!(
            sha256_root(&std::fs::read(&dependency_input).unwrap()),
            dependency_input_root_before
        );
        let erdos_facts_after =
            vela_edge::frontier_repository::derive_repository_anchor_facts(&erdos, &erdos_head)
                .unwrap();
        assert_eq!(erdos_facts_after, erdos_facts);
    }

    #[test]
    fn migration_hostile_git_environment_cannot_redirect_source_or_reach_protected_signer() {
        const CHILD_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_GIT_CHILD";
        const SOURCE_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_SOURCE";
        const DECOY_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_DECOY";
        const PROFILE_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_PROFILE";
        const TARGET_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_TARGET";
        const TRUST_ENV: &str = "VELA_TEST_HOSTILE_MIGRATION_TRUST";
        const TEST_NAME: &str = concat!(
            "cli::migration::tests::",
            "migration_hostile_git_environment_cannot_redirect_source_or_reach_protected_signer"
        );

        if std::env::var_os(CHILD_ENV).is_none() {
            // Environment mutation is process-global. Run the hostile case in
            // an exact, single-test child process so no parallel test can
            // observe the injected Git repository/config redirection.
            let fixture = fixture();
            let hostile_root = tempfile::tempdir().unwrap();
            let decoy = hostile_root.path().join("decoy");
            let clone = std::process::Command::new("git")
                .args([
                    "clone",
                    "--no-hardlinks",
                    fixture.frontier.path().to_str().unwrap(),
                    decoy.to_str().unwrap(),
                ])
                .output()
                .unwrap();
            assert!(
                clone.status.success(),
                "create exact decoy clone: {}",
                String::from_utf8_lossy(&clone.stderr)
            );
            let source_head = git(fixture.frontier.path(), &["rev-parse", "HEAD"]).unwrap();
            assert_eq!(
                git(&decoy, &["rev-parse", "HEAD"]).unwrap(),
                source_head,
                "decoy must start at the exact source anchor"
            );

            let trust_home = hostile_root.path().join("trust-home");
            std::fs::create_dir(&trust_home).unwrap();

            let mut child = Command::new(std::env::current_exe().unwrap());
            child.args(["--exact", TEST_NAME, "--nocapture"]);
            for (name, _) in std::env::vars_os() {
                if name.to_str().is_some_and(|name| name.starts_with("GIT_")) {
                    child.env_remove(name);
                }
            }
            child
                .env(CHILD_ENV, "1")
                .env(SOURCE_ENV, fixture.frontier.path())
                .env(DECOY_ENV, &decoy)
                .env(PROFILE_ENV, fixture.profile_file.path())
                .env(TARGET_ENV, fixture.target_candidate_file.path())
                .env(TRUST_ENV, &trust_home)
                .env("GIT_DIR", decoy.join(".git"))
                .env("GIT_WORK_TREE", &decoy)
                .env("GIT_COMMON_DIR", decoy.join(".git"))
                .env("GIT_INDEX_FILE", decoy.join(".git/index"))
                .env("GIT_OBJECT_DIRECTORY", decoy.join(".git/objects"))
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "core.hooksPath")
                .env(
                    "GIT_CONFIG_VALUE_0",
                    hostile_root.path().join("hostile-hooks"),
                );
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "hostile migration child failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let source = PathBuf::from(std::env::var_os(SOURCE_ENV).unwrap());
        let decoy = PathBuf::from(std::env::var_os(DECOY_ENV).unwrap());
        let profile = PathBuf::from(std::env::var_os(PROFILE_ENV).unwrap());
        let target = PathBuf::from(std::env::var_os(TARGET_ENV).unwrap());
        let trust_home = PathBuf::from(std::env::var_os(TRUST_ENV).unwrap());

        let source_anchor = git(&source, &["rev-parse", "HEAD"]).unwrap();
        let decoy_head = git(&decoy, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(source_anchor, decoy_head);
        let preview = prepare_migration(
            &source,
            &profile,
            None,
            &target,
            "reviewer:migration",
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        assert_eq!(
            preview.plan.git_commit, source_anchor,
            "preview followed ambient Git redirection instead of the named source"
        );
        let request = protected_request_for_test(&preview);

        std::fs::write(
            source.join("README.md"),
            "# Migration fixture\n\nPost-preview source drift.\n",
        )
        .unwrap();
        run(&source, &["add", "README.md"]);
        run(&source, &["commit", "-qm", "post-preview source drift"]);
        let (source_head, source_tree) = assert_migration_checkout(&source).unwrap();
        assert_ne!(
            source_head, preview.plan.git_commit,
            "named source must carry the post-preview drift commit"
        );
        assert_ne!(
            source_tree, preview.plan.git_tree,
            "named source must carry the post-preview drift tree"
        );
        assert_eq!(
            decoy_head, preview.plan.git_commit,
            "decoy must retain the exact confirmed anchor"
        );
        let before = tree_bytes(&source);
        let decoy_before = tree_bytes(&decoy);
        let signer_calls = std::cell::Cell::new(0_u8);

        let error = execute_confirmed_migration_with_signer(
            &source,
            &preview,
            &request,
            &trust_home,
            |_| {
                signer_calls.set(signer_calls.get() + 1);
                Err("protected signer must not be called after source drift".to_string())
            },
        )
        .unwrap_err();
        assert!(error.contains("migration Git anchor drifted"), "{error}");
        assert_eq!(
            signer_calls.get(),
            0,
            "hostile Git redirection reached the protected signer"
        );
        assert_eq!(
            tree_bytes(&source),
            before,
            "failed migration changed the named source checkout"
        );
        assert_eq!(
            git(&source, &["rev-parse", "HEAD"]).unwrap(),
            source_head,
            "failed migration moved the named source HEAD"
        );
        assert_eq!(
            git(&decoy, &["rev-parse", "HEAD"]).unwrap(),
            decoy_head,
            "failed migration moved the decoy HEAD"
        );
        assert_eq!(
            tree_bytes(&decoy),
            decoy_before,
            "failed migration changed the decoy checkout"
        );
        assert!(
            std::fs::read_dir(&trust_home).unwrap().next().is_none(),
            "failed migration installed a consumer trust pin"
        );
        assert!(!source.join(".vela/settings.toml").exists());
        assert!(!source.join("targets.json").exists());
        assert!(
            !source
                .join(format!(
                    ".vela/events/{}.json",
                    preview.plan.boundary_event.id
                ))
                .exists()
        );
        let journal_path = PathBuf::from(
            git(
                &source,
                &["rev-parse", "--git-path", "vela/operation-journals"],
            )
            .unwrap(),
        );
        let journal_path = if journal_path.is_absolute() {
            journal_path
        } else {
            source.join(journal_path)
        };
        if journal_path.is_dir() {
            assert!(
                std::fs::read_dir(&journal_path)
                    .unwrap()
                    .filter_map(Result::ok)
                    .all(|entry| !entry.file_name().to_string_lossy().starts_with("vop_")),
                "failed migration created a transaction journal or commit marker"
            );
        }
    }

    #[test]
    fn consumer_pin_drift_before_migration_commit_is_zero_canonical_writes() {
        let fixture = fixture();
        let before = tree_bytes(fixture.frontier.path());
        let preview = prepare_migration(
            fixture.frontier.path(),
            fixture.profile_file.path(),
            None,
            fixture.target_candidate_file.path(),
            &fixture.actor.id,
            "Bind exact legacy repository",
            OBSERVED_AT,
        )
        .unwrap();
        let request = protected_request_for_test(&preview);
        let trust_home = tempfile::tempdir().unwrap();
        let key = fixture.key.clone();
        let anchor = preview.plan.trust_anchor.clone();
        let error = execute_confirmed_migration_with_signer(
            fixture.frontier.path(),
            &preview,
            &request,
            trust_home.path(),
            |request| {
                vela_edge::repository_write::install_repository_trust_anchor_from_home(
                    trust_home.path(),
                    &anchor,
                )?;
                let signature = sign_event(&request.event, &key)?;
                let response = vela_signer::RepositoryBoundarySignerResponse {
                    schema: vela_signer::REPOSITORY_RESPONSE_SCHEMA.to_string(),
                    request_root: vela_signer::repository_boundary_request_root(request)?,
                    administrator_public_key: request.administrator_public_key.clone(),
                    helper_version: env!("CARGO_PKG_VERSION").to_string(),
                    helper_sha256: request.helper_sha256.clone(),
                    provider: request.provider.clone(),
                    protection_grade: request.protection_grade.clone(),
                    approved_at: chrono::Utc::now()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                    protection_mode: request.protection_mode,
                    event_id: request.event.id.clone(),
                    event_signature: signature.clone(),
                };
                let mut signed = request.event.clone();
                signed.signature = Some(signature);
                Ok((response, signed))
            },
        )
        .unwrap_err();
        assert!(error.contains("changed during protected"), "{error}");

        let after = tree_bytes(fixture.frontier.path());
        for (path, bytes) in before {
            assert_eq!(
                after.get(&path),
                Some(&bytes),
                "trust-pin drift changed canonical path {path}"
            );
        }
        assert!(!fixture.frontier.path().join(".vela/settings.toml").exists());
        assert!(!fixture.frontier.path().join("targets.json").exists());
        assert!(
            !fixture
                .frontier
                .path()
                .join(format!(
                    ".vela/events/{}.json",
                    preview.plan.boundary_event.id
                ))
                .exists()
        );
    }

    #[test]
    fn migration_frontier_repo_v1_settings_translation_is_closed_and_lossless() {
        let fixture = fixture();
        let config_path = fixture.frontier.path().join(".vela/config.toml");
        let mut config: toml::Value = std::fs::read_to_string(&config_path)
            .unwrap()
            .parse()
            .unwrap();
        let table = config.as_table_mut().unwrap();
        table.insert(
            "publish".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "git_push".to_string(),
                toml::Value::String("off".to_string()),
            )])),
        );
        table.insert(
            "work".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "lease_ttl_seconds".to_string(),
                toml::Value::Integer(3600),
            )])),
        );
        table.insert(
            "mcp".to_string(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "profile".to_string(),
                toml::Value::String("read-only".to_string()),
            )])),
        );
        std::fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
        run(fixture.frontier.path(), &["add", ".vela/config.toml"]);
        run(
            fixture.frontier.path(),
            &["commit", "-qm", "record legacy runtime settings"],
        );
        let project = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        let (commitment, bytes) = legacy_settings(fixture.frontier.path(), &project).unwrap();
        let translated =
            FrontierSettingsV1::from_toml(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(translated, commitment.settings);
        assert_eq!(
            translated.publish.unwrap().git_push,
            vela_protocol::frontier_settings::FrontierGitPush::Off
        );
        assert_eq!(translated.work.unwrap().lease_ttl_seconds, 3600);
        assert_eq!(
            translated.mcp.unwrap().profile,
            vela_protocol::frontier_settings::McpProfileV1::ReadOnly
        );

        let mut unknown = config;
        unknown.as_table_mut().unwrap().insert(
            "credential".to_string(),
            toml::Value::String("forbidden".to_string()),
        );
        std::fs::write(&config_path, toml::to_string_pretty(&unknown).unwrap()).unwrap();
        assert!(
            legacy_settings(fixture.frontier.path(), &project)
                .unwrap_err()
                .contains("unknown or invalid legacy settings")
        );
    }

    #[test]
    fn legacy_dependency_migration_requires_one_external_exact_resolution() {
        let fixture = fixture();
        let mut project = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        project.project.dependencies.push(ProjectDependency {
            name: "formal-conjectures".to_string(),
            source: "vela.hub".to_string(),
            version: Some("legacy".to_string()),
            pinned_hash: None,
            vfr_id: Some("vfr_0123456789abcdef".to_string()),
            locator: Some("https://legacy.invalid/frontier.json".to_string()),
            pinned_snapshot_hash: Some(format!("sha256:{}", "1".repeat(64))),
        });
        let error = dependency_migration(fixture.frontier.path(), &project, None).unwrap_err();
        assert!(error.contains("--dependency-input"), "{error}");
    }

    #[test]
    fn migration_historical_dependency_preview_uses_authenticated_ancestor() {
        let fixture = fixture();
        let before = tree_bytes(fixture.frontier.path());
        let dependency = tempfile::tempdir().unwrap();
        run(dependency.path(), &["init", "-q", "-b", "main"]);
        run(dependency.path(), &["config", "user.name", "Vela Test"]);
        run(
            dependency.path(),
            &["config", "user.email", "vela@example.invalid"],
        );

        let administrator_key = SigningKey::from_bytes(&[57; 32]);
        let administrator = ActorRecord {
            id: "reviewer:dependency-administrator".to_string(),
            public_key: pubkey_hex(&administrator_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-20T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let mut historical = vela_protocol::project::assemble(
            "Historical dependency fixture",
            Vec::new(),
            0,
            0,
            "Authenticate one retained ancestor",
        );
        historical.frontier_id = Some("vfr_fedcba9876543210".to_string());
        historical.actors = vec![administrator.clone()];
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(dependency.path().to_path_buf()),
            &historical,
        )
        .unwrap();
        run(dependency.path(), &["add", "."]);
        run(
            dependency.path(),
            &["commit", "-qm", "historical dependency state"],
        );
        let historical_commit = git(dependency.path(), &["rev-parse", "HEAD^{commit}"]).unwrap();
        let historical_facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            dependency.path(),
            &historical_commit,
        )
        .unwrap();

        let mut anchored: Project =
            serde_json::from_value(serde_json::to_value(&historical).unwrap()).unwrap();
        let mut retained_event = StateEvent {
            schema: vela_protocol::events::EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "frontier.observation_reviewed".into(),
            target: vela_protocol::events::StateTarget {
                r#type: "frontier".to_string(),
                id: anchored.frontier_id(),
            },
            actor: vela_protocol::events::StateActor {
                r#type: "agent".to_string(),
                id: "agent:dependency-fixture".to_string(),
            },
            timestamp: "2026-07-20T00:00:30Z".to_string(),
            reason: "Retain one descendant event before temporalization.".to_string(),
            before_hash: vela_protocol::events::NULL_HASH.to_string(),
            after_hash: vela_protocol::events::NULL_HASH.to_string(),
            payload: serde_json::json!({
                "proposal_id": "vpr_fedcba9876543210",
                "proposal_kind": "research_trace.review",
                "status": "accepted"
            }),
            caveats: Vec::new(),
            signature: None,
        };
        retained_event.id = vela_protocol::events::compute_event_id(&retained_event);
        anchored.events.push(retained_event);
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(dependency.path().to_path_buf()),
            &anchored,
        )
        .unwrap();
        run(dependency.path(), &["add", "."]);
        run(
            dependency.path(),
            &["commit", "-qm", "later temporalization anchor"],
        );
        let anchor_commit = git(dependency.path(), &["rev-parse", "HEAD^{commit}"]).unwrap();
        let anchor_facts = vela_edge::frontier_repository::derive_repository_anchor_facts(
            dependency.path(),
            &anchor_commit,
        )
        .unwrap();
        let legacy_identity_root =
            vela_edge::frontier_repository::derive_legacy_identity_preimage_root(&anchored)
                .unwrap();
        let identity_root = LegacyFrontierOriginV1 {
            schema: vela_protocol::frontier_repository::LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: anchored.frontier_id(),
            legacy_identity_preimage_root: legacy_identity_root.clone(),
            git_object_format: anchor_facts.git_object_format,
            anchor_git_commit: anchor_facts.git_commit.clone(),
            anchor_git_tree: anchor_facts.git_tree.clone(),
            anchor_event_log_root: anchor_facts.event_log_root.clone(),
            anchor_event_count: anchor_facts.event_count,
        }
        .identity_root()
        .unwrap();
        let empty_dependency_root =
            vela_protocol::frontier_repository::exact_dependency_root(&[]).unwrap();
        let mut boundary = new_repository_boundary_event(
            FrontierRepositoryBoundaryPayloadV1 {
                schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
                frontier_id: anchored.frontier_id(),
                identity_root: identity_root.clone(),
                observed_profile_root: sha256_root(b"dependency profile"),
                dependency_root: empty_dependency_root.clone(),
                dependencies: Vec::new(),
                previous_identity_event_root: None,
                legacy_identity_preimage_root: Some(legacy_identity_root),
                administrator_actor_id: administrator.id.clone(),
                administrator_public_key: administrator.public_key.clone(),
                administrator_algorithm: administrator.algorithm.clone(),
                trust_mode: FrontierRepositoryTrustMode::Tofu,
                git_object_format: anchor_facts.git_object_format,
                anchor_git_commit: anchor_facts.git_commit.clone(),
                anchor_git_tree: anchor_facts.git_tree.clone(),
                anchor_event_log_root: anchor_facts.event_log_root.clone(),
                anchor_event_count: anchor_facts.event_count,
                anchor_snapshot_root: anchor_facts.snapshot_root.clone(),
                anchor_snapshot_schema: anchor_facts.snapshot_schema.clone(),
                anchor_proposal_root: anchor_facts.proposal_root.clone(),
                anchor_actor_registry_root: anchor_facts.actor_registry_root.clone(),
                anchor_artifact_registry_root: anchor_facts.artifact_registry_root.clone(),
                anchor_canonical_store_root: anchor_facts.canonical_store_root.clone(),
            },
            "Authenticate retained dependency history.",
            "2026-07-20T00:01:00Z",
        )
        .unwrap();
        boundary.signature = Some(sign_event(&boundary, &administrator_key).unwrap());
        anchored.events.push(boundary.clone());
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(dependency.path().to_path_buf()),
            &anchored,
        )
        .unwrap();
        run(dependency.path(), &["add", "."]);
        run(
            dependency.path(),
            &["commit", "-qm", "install signed temporalization boundary"],
        );

        let exact = ExactFrontierDependencyV1 {
            frontier_id: historical.frontier_id(),
            identity_root,
            scientific_state_root: vela_protocol::scientific_state::scientific_state_root_v2(
                &historical,
                &vela_protocol::frontier_repository::repository_boundary_payload_from_event_shape(
                    &boundary,
                )
                .unwrap()
                .identity_root,
                &empty_dependency_root,
            )
            .unwrap(),
            git_object_format: historical_facts.git_object_format,
            git_commit: historical_facts.git_commit.clone(),
            git_tree: historical_facts.git_tree,
        };
        let legacy_dependency = ProjectDependency {
            name: "historical-dependency".to_string(),
            source: "git".to_string(),
            version: Some("historical".to_string()),
            pinned_hash: None,
            vfr_id: Some(historical.frontier_id()),
            locator: Some(dependency.path().display().to_string()),
            pinned_snapshot_hash: Some(historical_facts.snapshot_root),
        };
        let mut migrating = vela_protocol::repo::load_from_path(fixture.frontier.path()).unwrap();
        migrating.project.dependencies = vec![legacy_dependency.clone()];
        let trust_anchor = vela_edge::frontier_repository::RepositoryTrustAnchor {
            boundary_content_root: repository_identity_event_content_root(&boundary).unwrap(),
            administrator_public_key: administrator.public_key,
        };
        let input = DependencyMigrationInputV1 {
            schema: MIGRATION_DEPENDENCY_INPUT_SCHEMA.to_string(),
            entries: vec![DependencyMigrationEntryV1 {
                legacy: LegacyDependencyDescriptorV1::from(&legacy_dependency),
                repository_path: dependency.path().display().to_string(),
                boundary_content_root: trust_anchor.boundary_content_root.clone(),
                trust_anchor,
                exact: exact.clone(),
            }],
        };
        let mut input_file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        input_file
            .write_all(&serde_json::to_vec_pretty(&input).unwrap())
            .unwrap();

        let (commitment, resolved) =
            dependency_migration(fixture.frontier.path(), &migrating, Some(input_file.path()))
                .unwrap();
        assert_eq!(resolved, vec![exact]);
        assert_eq!(commitment.entries.len(), 1);
        assert_eq!(tree_bytes(fixture.frontier.path()), before);

        let mut wrong_tree = input.clone();
        wrong_tree.entries[0].exact.git_tree = "0".repeat(40);
        let mut wrong_tree_file = tempfile::NamedTempFile::new().unwrap();
        wrong_tree_file
            .write_all(&serde_json::to_vec_pretty(&wrong_tree).unwrap())
            .unwrap();
        let error = dependency_migration(
            fixture.frontier.path(),
            &migrating,
            Some(wrong_tree_file.path()),
        )
        .unwrap_err();
        assert!(
            error.contains("exact v1 pin does not match the authenticated exact dependency state"),
            "{error}"
        );

        let mut wrong_trust = input;
        wrong_trust.entries[0].trust_anchor.boundary_content_root =
            format!("sha256:{}", "0".repeat(64));
        let mut wrong_trust_file = tempfile::NamedTempFile::new().unwrap();
        wrong_trust_file
            .write_all(&serde_json::to_vec_pretty(&wrong_trust).unwrap())
            .unwrap();
        let error = dependency_migration(
            fixture.frontier.path(),
            &migrating,
            Some(wrong_trust_file.path()),
        )
        .unwrap_err();
        assert!(
            error.contains("repository trust anchor boundary root mismatch"),
            "{error}"
        );
        assert_eq!(tree_bytes(fixture.frontier.path()), before);
    }
}
