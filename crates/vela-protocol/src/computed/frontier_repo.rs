//! Canonical frontier repository layout helpers.
//!
//! This module keeps the user-facing repository shape separate from the
//! existing `.vela/` object/event storage. The visible files are the clone and
//! review surface; `.vela/` remains the substrate machinery.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::events;
use crate::frontier_profile::{
    FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileLicenseV1, FrontierProfileScopeV1, FrontierProfileV1,
};
use crate::frontier_settings::{FRONTIER_SETTINGS_SCHEMA, FrontierSettingsV1};
use crate::project::{self, Project, ProjectDependency};
use crate::proposals;

pub const FRONTIER_REPO_LAYOUT: &str = "vela.frontier_repo.v0.1";
pub const FRONTIER_REPO_LAYOUT_V1: &str = "vela.frontier_repo.v1";
pub const FRONTIER_MANIFEST_SCHEMA: &str = "vela.frontier_manifest.v0.1";
pub const FRONTIER_LOCK_SCHEMA: &str = "vela.frontier_lock.v0.1";
pub const FRONTIER_LOCK_SCHEMA_V1: &str = "vela.frontier_lock.v1";
pub const FRONTIER_INIT_SCHEMA: &str = "vela.frontier_repo_init.v0.1";
pub const FRONTIER_INIT_SCHEMA_V1: &str = "vela.frontier_repo_init.v1";
pub const FRONTIER_MATERIALIZE_SCHEMA: &str = "vela.frontier_materialize.v0.1";
pub const FRONTIER_REPO_STATUS_SCHEMA: &str = "vela.frontier_repo_status.v0.1";
pub const FRONTIER_REPO_DOCTOR_SCHEMA: &str = "vela.frontier_repo_doctor.v0.1";
pub const FRONTIER_PROOF_VERIFY_SCHEMA: &str = "vela.frontier_proof_verify.v0.1";
const VELA_ACTION_REPOSITORY: &str = "vela-science/vela";

fn current_vela_release() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierManifest {
    pub schema: String,
    pub layout: String,
    #[serde(default = "default_split_mode")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub scope: FrontierScope,
    /// Retired pre-0.9 integration metadata. This field is readable only so
    /// exact historical manifests can replay and migrate; Profile v1 has no
    /// corresponding surface and migration deliberately drops it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carina: Option<LegacyCarinaManifest>,
    pub vela: VelaManifest,
    pub paths: FrontierPaths,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maintainers: Vec<ManifestMaintainer>,
    #[serde(default)]
    pub policies: ManifestPolicies,
    #[serde(default)]
    pub license: ManifestLicense,
    #[serde(default)]
    pub dependencies: ManifestDependencies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LegacyCarinaManifest {
    pub kernel: String,
}

/// The two repository-profile generations accepted by this binary.
///
/// Dispatch is driven only by the exact top-level schema. An unknown schema
/// never falls through to the permissive legacy manifest reader, and a v1
/// profile is always parsed with its closed validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontierProfileFile {
    LegacyV0_1(FrontierManifest),
    V1(FrontierProfileV1),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VelaManifest {
    pub reducer: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierScope {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierPaths {
    pub state: String,
    pub sources: String,
    pub artifacts: String,
    pub review: String,
    pub proof: String,
    pub exports: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestMaintainer {
    pub id: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestPolicies {
    pub review: String,
    pub proof: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frontier: BTreeMap<String, String>,
}

impl Default for ManifestPolicies {
    fn default() -> Self {
        Self {
            review: "review/policy.yaml".to_string(),
            proof: "proof/policy.yaml".to_string(),
            frontier: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestLicense {
    pub content: String,
    pub code: String,
    pub data: String,
}

impl Default for ManifestLicense {
    fn default() -> Self {
        Self {
            content: "CC-BY-4.0".to_string(),
            code: "Apache-2.0".to_string(),
            data: "varies".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestDependencies {
    #[serde(default)]
    pub frontiers: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub adapters: Vec<String>,
    /// v0.59: structured cross-frontier dependency entries. Pre-v0.59
    /// split-repos persisted `Project.dependencies` only into the
    /// rendered `frontier.json`, which `vela frontier materialize`
    /// would regenerate without them. This field is the durable
    /// source of truth in the yaml manifest and is rehydrated into
    /// `Project.dependencies` on load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontiers_v2: Vec<ProjectDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierLock {
    pub schema: String,
    pub generated_at: String,
    pub vela_version: String,
    pub frontier_id: String,
    #[serde(default)]
    pub canonicalization: LockCanonicalization,
    #[serde(default)]
    pub reducer: LockPackage,
    #[serde(default)]
    pub verifiers: LockVerifiers,
    pub snapshot_hash: String,
    pub event_log_hash: String,
    pub proposal_state_hash: String,
    #[serde(default)]
    pub sources_hash: String,
    #[serde(default)]
    pub artifacts_hash: String,
    #[serde(default)]
    pub review_hash: String,
    pub proof_freshness: String,
    #[serde(default)]
    pub proof: LockProof,
    pub paths: LockPaths,
    /// v0.109: pinned cross-frontier dependencies. Each entry
    /// records the dependent frontier's `vfr_id`, the
    /// `pinned_snapshot_hash` declared in the manifest, and the
    /// `locator` (typically an https URL or hub registry pointer)
    /// the resolver was told to use. The lockfile reproduces this
    /// information in one place so a downstream consumer can
    /// verify "this frontier depended on exactly these snapshots
    /// of those dependencies" with no manifest cross-reference.
    /// Empty for frontiers with no cross-frontier dependencies;
    /// preserved across pre-v0.109 locks via #[serde(default)].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<LockedDependency>,
}

/// Generated Profile v1 lock projection.
///
/// Unlike the legacy lock, this names the closed scientific-state root and
/// labels the old broad Project hash explicitly as compatibility data. It is
/// derived entirely from a validated profile/event projection and grants no
/// authority of its own.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierLockV1 {
    pub schema: String,
    pub generated_at: String,
    pub vela_version: String,
    pub frontier_id: String,
    pub profile_root: String,
    pub identity_root: String,
    pub dependency_root: String,
    pub scientific_state_root: String,
    pub legacy_snapshot_root: String,
    pub event_log_root: String,
    pub event_count: u64,
    pub proposal_root: String,
    pub canonicalization: LockCanonicalization,
    pub reducer: LockPackage,
    pub verifiers: LockVerifiers,
    pub sources_hash: String,
    pub artifacts_hash: String,
    pub review_hash: String,
    pub proof_freshness: String,
    pub proof: LockProof,
    pub paths: LockPaths,
    pub dependencies: Vec<crate::frontier_repository::ExactFrontierDependencyV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontierLockFile {
    LegacyV0_1(FrontierLock),
    V1(FrontierLockV1),
}

/// v0.109: per-dependency pin entry inside `vela.lock`. Mirrors
/// the manifest's `ProjectDependency` fields that affect
/// reproducibility (id, snapshot, locator) and drops the rest
/// (display name, semver-style version) so the lockfile is the
/// minimum content-addressable witness.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedDependency {
    /// Display name from the manifest. Not part of the
    /// reproducibility witness; kept for human readability.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Source string from the manifest (typically an https URL
    /// or a `vfr_<id>` reference).
    pub source: String,
    /// Content-addressed frontier id of the dependent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vfr_id: Option<String>,
    /// Locator the resolver was told to fetch from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// SHA-256 of the dependent's canonical snapshot. The strict
    /// pull path verifies the fetched dependency matches this
    /// exact hash before satisfying any cross-frontier link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_snapshot_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockCanonicalization {
    pub json: String,
    pub yaml: String,
}

impl Default for LockCanonicalization {
    fn default() -> Self {
        Self {
            json: "vela-canonical-json-v0.1".to_string(),
            yaml: "vela-yaml-v0.1".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockPackage {
    pub package: String,
    pub digest: String,
}

/// 2026-06-25 (repo-native): the frozen-verifier pin. The lock already pins the
/// reducer that replays events into state; this pins the `vela-verify` package
/// that re-derives the frontier's witnesses and the set of verifier `kind`s those
/// witnesses declare. Together they make the frontier self-describing: a
/// different binary can replay AND re-verify it from the repo alone, with no hub
/// and no assumption about which version produced it. `kinds` is empty for
/// claim/cert frontiers that carry no witnesses.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockVerifiers {
    pub package: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockProof {
    pub latest: String,
    pub digest: String,
    pub freshness: String,
    pub events_manifest: String,
    pub replay_trace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockPaths {
    pub frontier: String,
    pub events: String,
}

#[derive(Debug, Clone)]
struct ProofWrite {
    digest: String,
    freshness: String,
    latest: String,
    events_manifest: String,
    replay_trace: String,
}

/// Exact bytes written by [`write_visible_repo_files`], keyed by normalized
/// frontier-relative path. Rendering is read-only: callers can bind these
/// bytes into a transaction before changing the frontier.
pub type VisibleRepoFiles = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone)]
struct RenderedProof {
    files: VisibleRepoFiles,
    proof: ProofWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLayoutIssue {
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct InitOptions<'a> {
    pub name: &'a str,
    pub initialize_git: bool,
}

/// Inputs for the Profile v1 initializer used by the ordinary CLI path.
///
/// The caller supplies the complete scientific scope. Initialization does not
/// infer a domain, target catalogue, verifier, maintainer, or authority.
#[derive(Debug, Clone)]
pub struct ProfileV1InitOptions<'a> {
    pub name: &'a str,
    pub scope: &'a str,
    pub initialize_git: bool,
}

pub fn initialize(path: &Path, options: InitOptions<'_>) -> Result<serde_json::Value, String> {
    if path.exists() && !path.is_dir() {
        return Err(format!("{} exists and is not a directory", path.display()));
    }
    fs::create_dir_all(path).map_err(|e| {
        format!(
            "Failed to create frontier directory '{}': {e}",
            path.display()
        )
    })?;

    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let project = empty_project(options.name, "", &now);
    crate::repo::init_repo(path, &project)?;
    write_frontier_card(path, options.name)?;
    write_scope(path, options.name)?;
    write_git_native_scaffold(path, options.name, true)?;
    initialize_git_repository(path, options.initialize_git)?;

    let payload = json!({
        "schema": FRONTIER_INIT_SCHEMA,
        "ok": true,
        "layout": FRONTIER_REPO_LAYOUT,
        "path": path.display().to_string(),
        "name": options.name,
        "wrote": [
            "README.md",
            "SCOPE.md",
            "frontier.yaml",
            "frontier.json",
            "vela.lock",
            ".gitignore",
            ".gitattributes",
            ".github/workflows/vela-frontier.yml",
            "VELA.md",
            ".mcp.json"
        ],
        "next_commands": init_next_commands(path)
    });
    Ok(payload)
}

/// Create the compact 0.9 frontier skeleton. The canonical store, manifest,
/// visible state, lock, Git safety files, and agent charter are present; proof
/// projections and optional integrations are generated only when requested.
pub fn initialize_minimal(
    path: &Path,
    options: InitOptions<'_>,
) -> Result<serde_json::Value, String> {
    let name = options.name.to_string();
    let mut payload = initialize(path, options)?;
    for relative in [".mcp.json", ".github/workflows/vela-frontier.yml"] {
        let candidate = path.join(relative);
        if candidate.is_file() {
            fs::remove_file(&candidate)
                .map_err(|error| format!("remove {}: {error}", candidate.display()))?;
        }
    }
    let proof_dir = path.join("proof");
    if proof_dir.exists() {
        fs::remove_dir_all(&proof_dir)
            .map_err(|error| format!("remove {}: {error}", proof_dir.display()))?;
    }
    let project = crate::repo::load_from_path(path)?;
    let generated_at = materialization_generated_at(path, &project);
    let empty_proof = ProofWrite {
        digest: directory_hash(&proof_dir),
        freshness: "not_materialized".to_string(),
        latest: String::new(),
        events_manifest: String::new(),
        replay_trace: String::new(),
    };
    write_lock(path, &project, &empty_proof, &generated_at)?;
    payload["name"] = json!(name);
    payload["wrote"] = json!([
        "README.md",
        "SCOPE.md",
        "frontier.yaml",
        "frontier.json",
        "vela.lock",
        ".gitignore",
        ".gitattributes",
        "VELA.md"
    ]);
    payload["next_commands"] = json!([
        format!(
            "vela doctor {} --json",
            posix_shell_arg(&path.display().to_string())
        ),
        format!(
            "vela status {} --json",
            posix_shell_arg(&path.display().to_string())
        ),
        format!(
            "vela next {} --json",
            posix_shell_arg(&path.display().to_string())
        )
    ]);
    Ok(payload)
}

/// Create a new, minimal Profile v1 Frontier without first materializing a
/// legacy repository. Historical fixture helpers continue to use
/// [`initialize`] and [`initialize_minimal`]; this function is the new-product
/// path used by `vela init`.
pub fn initialize_profile_v1_minimal(
    path: &Path,
    options: ProfileV1InitOptions<'_>,
) -> Result<serde_json::Value, String> {
    let name = options.name.trim();
    let scope = options.scope.trim();
    if name.is_empty() {
        return Err("Profile v1 initialization requires a non-empty name".to_string());
    }
    if scope.is_empty() {
        return Err("Profile v1 initialization requires a non-empty bounded scope".to_string());
    }

    let target_existed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "{} must be a real directory or an absent path",
                path.display()
            ));
        }
        Ok(_) => {
            let mut entries = fs::read_dir(path)
                .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?;
            if entries
                .next()
                .transpose()
                .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?
                .is_some()
            {
                return Err(format!(
                    "refusing to initialize non-empty directory {}; adopt an existing repository through the explicit migration path",
                    path.display()
                ));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|error| {
                format!(
                    "Failed to create frontier directory '{}': {error}",
                    path.display()
                )
            })?;
            false
        }
        Err(error) => {
            return Err(format!("inspect init target '{}': {error}", path.display()));
        }
    };

    // Construct the complete repository below a temporary child first. The
    // target is still empty, so an initialization failure cannot leave a
    // half-formed `.vela/` history or overwrite an existing project file.
    let staging = tempfile::Builder::new()
        .prefix(".vela-init-")
        .tempdir_in(path)
        .map_err(|error| format!("create initialization staging directory: {error}"))?;
    let staged = initialize_profile_v1_minimal_in_place(staging.path(), &options);
    let mut payload = match staged {
        Ok(payload) => payload,
        Err(error) => {
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(error);
        }
    };

    let mut entries = fs::read_dir(staging.path())
        .map_err(|error| format!("read initialization staging directory: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read initialization staging entry: {error}"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut installed = Vec::new();
    for entry in entries {
        let destination = path.join(entry.file_name());
        if fs::symlink_metadata(&destination).is_ok() {
            for (source, destination) in installed.iter().rev() {
                let _ = fs::rename(destination, source);
            }
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(format!(
                "initialization target changed while staging: {}",
                destination.display()
            ));
        }
        if let Err(error) = fs::rename(entry.path(), &destination) {
            for (source, destination) in installed.iter().rev() {
                let _ = fs::rename(destination, source);
            }
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(format!(
                "install initialized repository entry '{}': {error}",
                destination.display()
            ));
        }
        installed.push((entry.path(), destination));
    }
    drop(staging);

    payload["path"] = json!(path.display().to_string());
    payload["next_commands"] = json!([
        format!(
            "vela doctor {} --json",
            posix_shell_arg(&path.display().to_string())
        ),
        format!(
            "vela status {} --json",
            posix_shell_arg(&path.display().to_string())
        ),
        format!(
            "vela next {} --json",
            posix_shell_arg(&path.display().to_string())
        )
    ]);
    Ok(payload)
}

fn initialize_profile_v1_minimal_in_place(
    path: &Path,
    options: &ProfileV1InitOptions<'_>,
) -> Result<serde_json::Value, String> {
    let name = options.name.trim();
    let scope = options.scope.trim();

    let project = project::assemble_profile_v1(name, Vec::new(), 0, 0, scope);
    let frontier_id = project.frontier_id();
    let profile = FrontierProfileV1 {
        schema: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
        frontier_id: frontier_id.clone(),
        name: name.to_string(),
        // The CLI accepts one bounded sentence. Reusing it here avoids
        // inventing a broader summary during initialization.
        summary: scope.to_string(),
        scope: FrontierProfileScopeV1 {
            question: scope.to_string(),
            includes: Vec::new(),
            excludes: Vec::new(),
        },
        maintainers: Vec::new(),
        license: FrontierProfileLicenseV1 {
            content: "CC-BY-4.0".to_string(),
            code: "Apache-2.0".to_string(),
            data: "varies".to_string(),
        },
    };
    profile.validate()?;
    let profile_root = profile.profile_root()?;
    let dependency_root = crate::frontier_repository::exact_dependency_root(&[])?;
    let genesis = project
        .events
        .first()
        .ok_or_else(|| "Profile v1 initialization did not produce frontier.created".to_string())?;
    if project.events.len() != 1 || genesis.kind != "frontier.created" {
        return Err(
            "Profile v1 initialization must produce exactly one frontier.created event".to_string(),
        );
    }

    let profile_bytes = serde_yaml::to_string(&profile)
        .map_err(|error| format!("Failed to serialize Profile v1: {error}"))?;
    fs::write(path.join("frontier.yaml"), profile_bytes)
        .map_err(|error| format!("Failed to write frontier.yaml: {error}"))?;

    write_profile_v1_canonical_skeleton(path, &project)?;
    write_visible_state(
        path,
        &project,
        &materialization_generated_at(path, &project),
    )?;

    // Proof packets are explicit derived products, not part of the initial
    // repository. The initial v1 lock states that none has been materialized.
    let proof_dir = path.join("proof");
    let generated_at = materialization_generated_at(path, &project);
    let empty_proof = ProofWrite {
        digest: directory_hash(&proof_dir),
        freshness: "not_materialized".to_string(),
        latest: String::new(),
        events_manifest: String::new(),
        replay_trace: String::new(),
    };
    let (_, lock_bytes) = render_lock_v1(path, &project, &profile, &empty_proof, &generated_at)?;
    fs::write(path.join("vela.lock"), lock_bytes)
        .map_err(|error| format!("Failed to write vela.lock: {error}"))?;

    let settings = FrontierSettingsV1 {
        schema: FRONTIER_SETTINGS_SCHEMA.to_string(),
        publish: None,
        work: None,
        mcp: None,
    };
    fs::write(path.join(".vela/settings.toml"), settings.to_toml()?)
        .map_err(|error| format!("Failed to write .vela/settings.toml: {error}"))?;
    write_frontier_card_v1(path, name, scope)?;
    write_scope_v1(path, scope)?;
    write_git_native_scaffold(path, name, false)?;
    initialize_git_repository(path, options.initialize_git)?;

    let event_path = format!(".vela/events/{}.json", genesis.id);
    Ok(json!({
        "schema": FRONTIER_INIT_SCHEMA_V1,
        "ok": true,
        "layout": FRONTIER_REPO_LAYOUT_V1,
        "path": path.display().to_string(),
        "name": name,
        "scope": scope,
        "frontier_id": frontier_id,
        "frontier_created_event_id": genesis.id,
        "profile_root": profile_root,
        "dependency_root": dependency_root,
        "wrote": [
            "README.md",
            "SCOPE.md",
            "frontier.yaml",
            "frontier.json",
            "vela.lock",
            ".gitignore",
            ".gitattributes",
            "VELA.md",
            ".vela/settings.toml",
            event_path,
            ".vela/proof-state.json",
            ".vela/actors.json"
        ],
        "next_commands": [
            format!("vela doctor {} --json", posix_shell_arg(&path.display().to_string())),
            format!("vela status {} --json", posix_shell_arg(&path.display().to_string())),
            format!("vela next {} --json", posix_shell_arg(&path.display().to_string()))
        ]
    }))
}

fn write_profile_v1_canonical_skeleton(path: &Path, project: &Project) -> Result<(), String> {
    let vela_dir = path.join(".vela");
    for relative in ["findings", "events", "proposals", "artifacts"] {
        let directory = vela_dir.join(relative);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Failed to create {}: {error}", directory.display()))?;
    }
    for event in &project.events {
        let destination = vela_dir.join("events").join(format!("{}.json", event.id));
        let bytes = serde_json::to_vec_pretty(event)
            .map_err(|error| format!("Failed to serialize state event {}: {error}", event.id))?;
        fs::write(&destination, bytes)
            .map_err(|error| format!("Failed to write {}: {error}", destination.display()))?;
    }
    let proof_state = serde_json::to_vec_pretty(&project.proof_state)
        .map_err(|error| format!("Failed to serialize proof state: {error}"))?;
    fs::write(vela_dir.join("proof-state.json"), proof_state)
        .map_err(|error| format!("Failed to write .vela/proof-state.json: {error}"))?;
    let actors = serde_json::to_vec_pretty(&project.actors)
        .map_err(|error| format!("Failed to serialize actors: {error}"))?;
    fs::write(vela_dir.join("actors.json"), actors)
        .map_err(|error| format!("Failed to write .vela/actors.json: {error}"))?;
    Ok(())
}

fn initialize_git_repository(path: &Path, requested: bool) -> Result<(), String> {
    if !requested || path.join(".git").exists() {
        return Ok(());
    }
    // Pin the ecosystem branch name instead of inheriting Git's machine-local
    // default. This is repository setup only; it installs no hooks.
    let output = std::process::Command::new("git")
        .args(["init", "--quiet", "-b", "main"])
        .arg(path)
        .output()
        .map_err(|error| format!("Failed to run git init: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn init_next_commands(path: &Path) -> Vec<String> {
    let target = posix_shell_arg(&path.display().to_string());
    vec![
        format!("vela agents sync {target} --json"),
        format!("vela doctor {target} --json"),
        format!("vela status {target} --json"),
        format!("vela next {target} --json"),
        format!("vela check {target} --strict --json"),
    ]
}

/// Quote one argument for the POSIX shells supported by the prebuilt Vela
/// releases. Init returns copyable command strings, so ordinary spaces and
/// metacharacters must remain one literal path rather than becoming shell
/// syntax.
fn posix_shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// The git-native, AI-drivable scaffold: everything a fresh frontier needs
/// so that `git push` is publication, CI is the gate, and any agent can
/// drive it through MCP — written at init so the five-minute path is real.
///
/// - `.gitignore` COMMITS `.vela/` (the event log IS the repo; only
///   per-machine scratch is ignored) — the git-native inversion.
/// - `.gitattributes` makes canonical records raw Git blobs: local filters,
///   keyword expansion, encodings, and merge drivers cannot rewrite them.
/// - `.github/workflows/vela-frontier.yml` consumes the shared vela-check
///   Action: every push re-derives the frontier from a clean checkout.
/// - `VELA.md` is the canonical agent charter (`vela agents sync` generates
///   CLAUDE.md / AGENTS.md / editor adapters from it).
/// - `.mcp.json` selects the nonfinalizing draft MCP profile so agents can use
///   the task-first producer loop; no finalizer or human key is exposed.
fn write_git_native_scaffold(
    path: &Path,
    name: &str,
    include_optional_integrations: bool,
) -> Result<(), String> {
    let write = |rel: &str, contents: String| -> Result<(), String> {
        let dest = path.join(rel);
        if dest.exists() {
            return Ok(()); // never clobber an existing choice
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        fs::write(&dest, contents).map_err(|e| format!("write {}: {e}", dest.display()))
    };

    write(
        ".gitignore",
        "# A Vela frontier commits the record, not the derivation.\n\
         # Authority: .vela/events/ (signed log). Pins: vela.lock. Read-entry: frontier.json.\n\
         \n\
         # Producer-local scratch (not replayable truth).\n\
         /packets/\n/activity/\n/exports/\n\
         /.vela/agents/\n/.vela/keys/\n/.vela/operation-journals/\n/.vela/tasks/\n/.vela/work/\n/.vela/workspaces/\n/.vela/source-inbox/\n/.vela/artifact-blobs/\n\
         \n\
         # Tooling scratch\n\
         /target/\n__pycache__/\n*.pyc\n.venv/\n.pytest_cache/\n.DS_Store\n"
            .to_string(),
    )?;

    write(
        ".gitattributes",
        "# Ordinary text is normalized across platforms. More specific rules below win.\n\
         * text=auto eol=lf\n\
         \n\
         # Canonical events, proposals, receipts, and public review material are raw Git\n\
         # blobs. Repository filters, keyword expansion, working-tree encodings, and\n\
         # semantic merge drivers may not transform them. Vela publication also checks\n\
         # the effective attributes and raw staged blob before moving a ref, because a\n\
         # local .git/info/attributes file has higher precedence than this contract.\n\
         .vela/** -filter -ident -working-tree-encoding -merge -text\n\
         .vela/events/** -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         .vela/proposals/** -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         .vela/records/** -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         records/** -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         sources/** -filter -ident -working-tree-encoding -merge -text\n\
         artifacts/** -filter -ident -working-tree-encoding -merge -text\n\
         review/** -filter -ident -working-tree-encoding -merge -text\n\
         proof/** -filter -ident -working-tree-encoding -merge -text\n\
         frontier.yaml -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         frontier.json -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         vela.lock -filter -ident -working-tree-encoding -merge diff text eol=lf\n\
         \n\
         # Witnesses are evidence the frozen verifiers re-check. They can be large, so\n\
         # they may use Git LFS. The receipt's Vela digest remains content identity; the\n\
         # LFS object is transport and its absence is an explicit availability failure.\n\
         witnesses/** filter=lfs diff=lfs merge=lfs -text\n"
            .to_string(),
    )?;

    if include_optional_integrations {
        let vela_release = current_vela_release();
        let vela_action_ref = format!("{VELA_ACTION_REPOSITORY}@{vela_release}");
        let workflow = r#"name: Verify the signed frontier

# The git-native gate: every main push and every pull request runs the shared
# vela-check action. This is deliberately unfiltered: public canonical roots
# evolve, and a stale path allowlist would let scientific state bypass the gate.
# The action re-derives the frontier from a clean checkout and proves the
# working tree IS the signed state (replay + strict check + hash parity).
# It NEVER signs: acceptance stays a human-keyed event in the repo.
on:
  push:
    branches: [main]
  pull_request: {}
  workflow_dispatch: {}

permissions:
  contents: read

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: __VELA_ACTION_REF__
        with:
          vela-version: __VELA_RELEASE__
"#
        .replace("__VELA_ACTION_REF__", &vela_action_ref)
        .replace("__VELA_RELEASE__", &vela_release);
        write(".github/workflows/vela-frontier.yml", workflow)?;
    }

    write(
        "VELA.md",
        format!(
            r#"# {name} — agent charter

This frontier is driven by `vela` + `git`. The ordinary path is
`next -> work -> land -> sign`: agents stop after the routed landing; only a
human key or a previously human-signed Permit policy changes accepted state.
`vela agents sync` regenerates CLAUDE.md / AGENTS.md / editor adapters from
this file — edit here, never there.

## Agent rules

Agents may:

- inspect state: `vela status .`, `vela next .`, `vela check .`
- claim one target: `vela work <target> --as agent:<name> --json`
- land Receipt v1 work through that session: `vela land --work <target>
  --claim … --artifact … --caveat … --as agent:<name> --json`
- import a foreign producer's canonical Receipt v1: `vela land receipt.json
  --as agent:<name> --json`
- run the verifiers: `vela reproduce .`, `vela check . --strict`
- rebuild derived views: `vela frontier materialize .`

Agents may not:

- run `vela sign`, accept, reject, apply, or finalize a truth-bearing proposal
- sign with, read, or handle a human's key
- hand-edit accepted events or derived views such as `frontier.json` and proof
  packets

## Fast commands

```bash
vela next . --json                              # ranked offer
vela work <target> --as agent:<name> --json     # lease + briefing
vela land --work <target> --claim <claim> \
  --type computational --replayability exact \
  --artifact <path>:<kind> --caveat <limit> \
  --as agent:<name> --json                       # Receipt v1 + policy route
vela status . --json                             # accepted and pending state
vela check . --strict                            # replay and parity gate
vela frontier materialize .                      # rebuild derived views
git push                                         # publication; no authority
```
"#
        ),
    )?;

    if include_optional_integrations {
        write(
            ".mcp.json",
            r#"{
  "_generated_by": "vela agents sync (from VELA.md) — edit VELA.md, not this file",
  "mcpServers": {
    "vela-local": {
      "args": [
        "serve",
        ".",
        "--profile",
        "draft"
      ],
      "command": "vela"
    }
  }
}
"#
            .to_string(),
        )?;
    }

    Ok(())
}

pub fn materialize(path: &Path) -> Result<serde_json::Value, String> {
    let source = crate::repo::VelaSource::VelaRepo(path.to_path_buf());
    let project = crate::repo::load(&source)?;
    // No eager section-README scaffolding here: a materialize must not litter a
    // frontier with empty stub directories. Content dirs (proof/, …) are created
    // on demand by their writers; the lock's directory hashes tolerate absence.
    let generated_at = materialization_generated_at(path, &project);
    write_visible_state(path, &project, &generated_at)?;
    write_manifest(path, &project)?;
    let proof = write_proof(path, &project, &generated_at)?;
    if let Some(FrontierProfileFile::V1(profile)) = read_repository_profile(path)? {
        let (lock, bytes) = render_lock_v1(path, &project, &profile, &proof, &generated_at)?;
        fs::write(path.join("vela.lock"), bytes)
            .map_err(|error| format!("Failed to write vela.lock: {error}"))?;
        return Ok(json!({
            "schema": FRONTIER_MATERIALIZE_SCHEMA,
            "ok": true,
            "path": path.display().to_string(),
            "wrote_frontier": "frontier.json",
            "wrote_lock": "vela.lock",
            "wrote_proof": "proof/latest.json",
            "wrote_events_manifest": "proof/events.manifest.jsonl",
            "profile_root": lock.profile_root,
            "identity_root": lock.identity_root,
            "dependency_root": lock.dependency_root,
            "scientific_state_root": lock.scientific_state_root,
            "legacy_snapshot_root": lock.legacy_snapshot_root,
            "event_log_root": lock.event_log_root,
            "proposal_root": lock.proposal_root,
        }));
    }
    let lock = write_lock(path, &project, &proof, &generated_at)?;
    Ok(json!({
        "schema": FRONTIER_MATERIALIZE_SCHEMA,
        "ok": true,
        "path": path.display().to_string(),
        "wrote_frontier": "frontier.json",
        "wrote_lock": "vela.lock",
        "wrote_proof": "proof/latest.json",
        "wrote_events_manifest": "proof/events.manifest.jsonl",
        "snapshot_hash": lock.snapshot_hash,
        "event_log_hash": lock.event_log_hash,
        "proposal_state_hash": lock.proposal_state_hash,
    }))
}

pub fn write_visible_repo_files(path: &Path, project: &Project) -> Result<(), String> {
    let files = render_visible_repo_files(path, project)?;
    for (relative_path, bytes) in files {
        let destination = path.join(&relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        fs::write(&destination, bytes)
            .map_err(|e| format!("Failed to write {}: {e}", destination.display()))?;
    }
    Ok(())
}

/// Render the complete set of bytes that [`write_visible_repo_files`] would
/// write, without mutating the frontier. Existing user-owned manifest fields
/// and proof-side files are treated as read-only inputs.
pub fn render_visible_repo_files(
    path: &Path,
    project: &Project,
) -> Result<VisibleRepoFiles, String> {
    render_visible_repo_files_inner(path, project, None)
}

/// Render the exact Profile v1 migration postimage without first installing
/// the candidate profile in the source checkout.
///
/// This is the pure staging counterpart of [`render_visible_repo_files`].
/// The supplied bytes are preserved exactly while the parsed profile drives
/// the v1 lock projection. It exists so the protected migration transaction
/// can bind every derived postimage before its commit marker.
pub fn render_visible_repo_files_for_profile_migration(
    path: &Path,
    project: &Project,
    profile: &FrontierProfileV1,
    profile_bytes: &[u8],
) -> Result<VisibleRepoFiles, String> {
    profile.validate()?;
    profile.assert_frontier_id(&project.frontier_id())?;
    let reparsed = std::str::from_utf8(profile_bytes)
        .map_err(|error| format!("candidate Profile v1 bytes are not UTF-8: {error}"))
        .and_then(FrontierProfileV1::from_yaml_str)?;
    if &reparsed != profile {
        return Err(
            "candidate Profile v1 bytes do not match the parsed migration profile".to_string(),
        );
    }
    render_visible_repo_files_inner(path, project, Some((profile, profile_bytes)))
}

fn render_visible_repo_files_inner(
    path: &Path,
    project: &Project,
    migration_profile: Option<(&FrontierProfileV1, &[u8])>,
) -> Result<VisibleRepoFiles, String> {
    // Split-repository loading is filename ordered. New objects are commonly
    // appended in memory, and their content-derived ids do not necessarily
    // sort after the prior set. Render the visible projection in the same
    // deterministic order a clean clone will load, or `frontier.json` can
    // become stale immediately after an otherwise exact transaction.
    let mut projected: Project = serde_json::from_value(
        serde_json::to_value(project)
            .map_err(|error| format!("clone split-repository projection: {error}"))?,
    )
    .map_err(|error| format!("clone split-repository projection: {error}"))?;
    projected
        .findings
        .sort_by(|left, right| left.id.cmp(&right.id));
    projected
        .events
        .sort_by(|left, right| left.id.cmp(&right.id));
    projected
        .proposals
        .sort_by(|left, right| left.id.cmp(&right.id));
    projected
        .artifacts
        .sort_by(|left, right| left.id.cmp(&right.id));
    let project = &projected;
    let generated_at = materialization_generated_at(path, project);
    let mut files = VisibleRepoFiles::new();
    files.insert(
        "frontier.json".to_string(),
        render_visible_state(
            path,
            project,
            &generated_at,
            migration_profile.map(|(profile, _)| profile),
        )?,
    );
    let manifest = match migration_profile {
        Some((_, bytes)) => bytes.to_vec(),
        None => match read_repository_profile(path)? {
            None => render_manifest(path, project)?,
            Some(FrontierProfileFile::LegacyV0_1(_)) => {
                // v0.59: keep the structured cross-frontier deps in the
                // existing yaml in sync with `Project.dependencies`. We
                // intentionally only touch the `dependencies.frontiers_v2`
                // field; other user-edited fields (scope, maintainers,
                // policies) are preserved.
                render_synced_manifest(path, &project.project.dependencies)?
            }
            Some(FrontierProfileFile::V1(_)) => fs::read(path.join("frontier.yaml"))
                .map_err(|error| format!("Failed to preserve frontier.yaml: {error}"))?,
        },
    };
    files.insert("frontier.yaml".to_string(), manifest);

    let rendered_proof = render_proof(
        path,
        project,
        &generated_at,
        migration_profile.map(|(profile, _)| profile),
    )?;
    files.extend(
        rendered_proof
            .files
            .into_iter()
            .map(|(relative_path, bytes)| (format!("proof/{relative_path}"), bytes)),
    );
    let lock_bytes = match migration_profile {
        Some((profile, _)) => {
            render_lock_v1(path, project, profile, &rendered_proof.proof, &generated_at)?.1
        }
        None => match read_repository_profile(path)? {
            Some(FrontierProfileFile::V1(profile)) => {
                render_lock_v1(
                    path,
                    project,
                    &profile,
                    &rendered_proof.proof,
                    &generated_at,
                )?
                .1
            }
            _ => render_lock(path, project, &rendered_proof.proof, &generated_at)?.1,
        },
    };
    files.insert("vela.lock".to_string(), lock_bytes);
    Ok(files)
}

pub fn read_manifest(path: &Path) -> Result<Option<FrontierManifest>, String> {
    match read_repository_profile(path)? {
        Some(FrontierProfileFile::LegacyV0_1(manifest)) => Ok(Some(manifest)),
        Some(FrontierProfileFile::V1(_)) => Err(
            "frontier.yaml is a vela.frontier-profile.v1 document, not a legacy manifest"
                .to_string(),
        ),
        None => Ok(None),
    }
}

/// Parse `frontier.yaml` through an exact schema dispatch.
pub fn read_repository_profile(path: &Path) -> Result<Option<FrontierProfileFile>, String> {
    let profile_path = path.join("frontier.yaml");
    let Some(data) =
        read_repository_control_text(path, Path::new("frontier.yaml"), "frontier.yaml")?
    else {
        return Ok(None);
    };
    let value: serde_yaml::Value = serde_yaml::from_str(&data).map_err(|error| {
        format!(
            "Failed to parse frontier profile '{}': {error}",
            profile_path.display()
        )
    })?;
    let schema = value
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("schema".to_string())))
        .and_then(serde_yaml::Value::as_str)
        .ok_or_else(|| "frontier.yaml must contain a string schema field".to_string())?;
    match schema {
        FRONTIER_MANIFEST_SCHEMA => serde_yaml::from_value::<FrontierManifest>(value)
            .map(FrontierProfileFile::LegacyV0_1)
            .map(Some)
            .map_err(|error| {
                format!(
                    "Failed to parse legacy frontier manifest '{}': {error}",
                    profile_path.display()
                )
            }),
        FRONTIER_PROFILE_SCHEMA_V1 => FrontierProfileV1::from_yaml_str(&data)
            .map(FrontierProfileFile::V1)
            .map(Some),
        other => Err(format!(
            "unsupported frontier.yaml schema `{other}`; expected `{FRONTIER_MANIFEST_SCHEMA}` or `{FRONTIER_PROFILE_SCHEMA_V1}`"
        )),
    }
}

/// Read one repository-relative control file while pinning every directory in
/// its lexical parent chain. A safe leaf is insufficient when `.vela/` (or the
/// repository path itself) can be swapped for a symlink between authorization
/// and use. Relative paths are deliberately closed: no root, prefix, `.` or
/// `..` components are accepted.
pub fn read_repository_control_text(
    repository: &Path,
    relative: &Path,
    label: &str,
) -> Result<Option<String>, String> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{label} must use a normalized repository-relative path"
        ));
    }

    let mut directories = Vec::with_capacity(components.len());
    let mut current = repository.to_path_buf();
    directories.push(current.clone());
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            unreachable!("closed component check above")
        };
        current.push(name);
        directories.push(current.clone());
    }

    let mut pinned = Vec::with_capacity(directories.len());
    for directory in &directories {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("Failed to inspect parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        pinned.push(
            same_file::Handle::from_path(directory)
                .map_err(|error| format!("Failed to identify parent of {label}: {error}"))?,
        );
    }

    let value = read_regular_nonsymlink_text(&repository.join(relative), label)?;
    for (directory, expected) in directories.iter().zip(&pinned) {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("Failed to reinspect parent of {label}: {error}"))?;
        let actual = same_file::Handle::from_path(directory)
            .map_err(|error| format!("Failed to reidentify parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() || &actual != expected {
            return Err(format!(
                "repository parent of {label} changed while it was read"
            ));
        }
    }
    Ok(value)
}

/// Read one repository control file without following a symlink or accepting
/// a non-regular filesystem object. The descriptor identity is compared with
/// the path both before and after the read so a concurrent replacement fails
/// closed instead of authorizing bytes from outside the repository.
pub fn read_regular_nonsymlink_text(path: &Path, label: &str) -> Result<Option<String>, String> {
    use std::io::Read;

    const MAX_CONTROL_FILE_BYTES: u64 = 16 * 1024 * 1024;

    let linked = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to inspect {label}: {error}")),
    };
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    let inspected = same_file::Handle::from_path(path)
        .map_err(|error| format!("Failed to identify {label}: {error}"))?;
    let mut file =
        fs::File::open(path).map_err(|error| format!("Failed to open {label}: {error}"))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("Failed to clone the open {label} descriptor: {error}"))?,
    )
    .map_err(|error| format!("Failed to identify the open {label}: {error}"))?;
    if inspected != opened {
        return Err(format!("{label} changed while it was opened"));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_CONTROL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read {label}: {error}"))?;
    if bytes.len() as u64 > MAX_CONTROL_FILE_BYTES {
        return Err(format!("{label} exceeds the 16 MiB control-file limit"));
    }
    let data =
        String::from_utf8(bytes).map_err(|error| format!("{label} is not UTF-8: {error}"))?;
    let final_link = fs::symlink_metadata(path)
        .map_err(|error| format!("Failed to reinspect {label}: {error}"))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("Failed to reidentify {label}: {error}"))?;
    if final_link.file_type().is_symlink() || !final_link.is_file() || opened != final_identity {
        return Err(format!("{label} changed while it was read"));
    }
    Ok(Some(data))
}

pub fn read_lock(path: &Path) -> Result<Option<FrontierLock>, String> {
    match read_repository_lock(path)? {
        Some(FrontierLockFile::LegacyV0_1(lock)) => Ok(Some(lock)),
        Some(FrontierLockFile::V1(_)) => {
            Err("vela.lock is a vela.frontier_lock.v1 document".to_string())
        }
        None => Ok(None),
    }
}

pub fn read_repository_lock(path: &Path) -> Result<Option<FrontierLockFile>, String> {
    let lock_path = path.join("vela.lock");
    if !lock_path.is_file() {
        return Ok(None);
    }
    let data =
        fs::read_to_string(&lock_path).map_err(|e| format!("Failed to read vela.lock: {e}"))?;
    let value: serde_yaml::Value = serde_yaml::from_str(&data).map_err(|error| {
        format!(
            "Failed to parse frontier lock '{}': {error}",
            lock_path.display()
        )
    })?;
    let schema = value
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("schema".to_string())))
        .and_then(serde_yaml::Value::as_str)
        .ok_or_else(|| "vela.lock must contain a string schema field".to_string())?;
    match schema {
        FRONTIER_LOCK_SCHEMA => serde_yaml::from_value::<FrontierLock>(value)
            .map(FrontierLockFile::LegacyV0_1)
            .map(Some)
            .map_err(|error| {
                format!(
                    "Failed to parse frontier lock '{}': {error}",
                    lock_path.display()
                )
            }),
        FRONTIER_LOCK_SCHEMA_V1 => serde_yaml::from_value::<FrontierLockV1>(value)
            .map(FrontierLockFile::V1)
            .map(Some)
            .map_err(|error| {
                format!(
                    "Failed to parse frontier lock '{}': {error}",
                    lock_path.display()
                )
            }),
        other => Err(format!("unsupported vela.lock schema `{other}`")),
    }
}

pub fn layout_issues(path: &Path, project: &Project) -> Vec<RepoLayoutIssue> {
    if !path.is_dir() || !path.join(".vela").is_dir() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let profile = match read_repository_profile(path) {
        Ok(value) => value,
        Err(e) => {
            issues.push(issue("invalid_frontier_manifest", e));
            None
        }
    };
    let lock_file = match read_repository_lock(path) {
        Ok(value) => value,
        Err(e) => {
            issues.push(issue("invalid_frontier_lock", e));
            None
        }
    };

    if profile.is_none() {
        issues.push(issue(
            "missing_frontier_manifest",
            "Split frontier repo is missing frontier.yaml.",
        ));
    }
    let Some(lock_file) = lock_file else {
        // A missing vela.lock is benign: it is a DERIVED view, regenerated
        // byte-for-byte by `vela frontier materialize`. The lock is not
        // canonical custody: a fresh Git checkout may have no lock until the
        // first materialize. Treat it as "not yet materialized,"
        // not a layout fault, so the lock-dependent hash checks below are
        // skipped rather than flagged.
        return issues;
    };

    match (profile.as_ref(), lock_file) {
        (Some(FrontierProfileFile::V1(profile)), FrontierLockFile::V1(lock)) => {
            validate_v1_layout(path, project, profile, &lock, &mut issues);
            issues
        }
        (Some(FrontierProfileFile::V1(_)), FrontierLockFile::LegacyV0_1(_))
        | (Some(FrontierProfileFile::LegacyV0_1(_)), FrontierLockFile::V1(_)) => {
            issues.push(issue(
                "frontier_lock_schema_mismatch",
                "frontier.yaml and vela.lock use different repository-profile generations.",
            ));
            issues
        }
        (_, FrontierLockFile::V1(_)) => {
            issues.push(issue(
                "frontier_lock_schema_mismatch",
                "vela.lock v1 requires a validated Repository Profile v1.",
            ));
            issues
        }
        (_, FrontierLockFile::LegacyV0_1(lock)) => {
            validate_legacy_layout(path, project, lock, issues)
        }
    }
}

fn validate_legacy_layout(
    path: &Path,
    project: &Project,
    lock: FrontierLock,
    mut issues: Vec<RepoLayoutIssue>,
) -> Vec<RepoLayoutIssue> {
    let locked_project = project_with_frontier_id(project);
    let hash_project = locked_project.as_ref().unwrap_or(project);
    let expected_snapshot = prefixed(events::snapshot_hash(hash_project));
    let expected_event_log = prefixed(events::event_log_hash(&hash_project.events));
    let expected_proposals = proposal_state_hash(&project.proposals);
    let expected_frontier = hash_project.frontier_id();
    let expected_sources = directory_hash(&path.join("sources"));
    let expected_artifacts = directory_hash(&path.join("artifacts"));
    let expected_review = directory_hash(&path.join("review"));
    let expected_proof = directory_hash(&path.join("proof"));
    if lock.snapshot_hash != expected_snapshot {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock snapshot_hash does not match materialized frontier state: lock={}, current={expected_snapshot}",
                lock.snapshot_hash
            ),
        ));
    }
    if lock.event_log_hash != expected_event_log {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock event_log_hash does not match .vela/events: lock={}, current={expected_event_log}",
                lock.event_log_hash
            ),
        ));
    }
    if lock.proposal_state_hash != expected_proposals {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock proposal_state_hash does not match .vela/proposals: lock={}, current={expected_proposals}",
                lock.proposal_state_hash
            ),
        ));
    }
    if lock.frontier_id != expected_frontier {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock frontier_id does not match current frontier: lock={}, current={expected_frontier}",
                lock.frontier_id
            ),
        ));
    }
    if !lock.sources_hash.is_empty() && lock.sources_hash != expected_sources {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock sources_hash does not match sources/: lock={}, current={expected_sources}",
                lock.sources_hash
            ),
        ));
    }
    if !lock.artifacts_hash.is_empty() && lock.artifacts_hash != expected_artifacts {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock artifacts_hash does not match artifacts/: lock={}, current={expected_artifacts}",
                lock.artifacts_hash
            ),
        ));
    }
    if !lock.review_hash.is_empty() && lock.review_hash != expected_review {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock review_hash does not match review/: lock={}, current={expected_review}",
                lock.review_hash
            ),
        ));
    }
    if !lock.proof.digest.is_empty() && lock.proof.digest != expected_proof {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock proof digest does not match proof/: lock={}, current={expected_proof}",
                lock.proof.digest
            ),
        ));
    }
    // 2026-06-25 (repo-native): the frozen-verifier pin must match the witnesses
    // on disk, so the pin is load-bearing rather than decorative. This catches a
    // witness added or re-kinded without a re-materialize, which the snapshot /
    // event hashes (computed over frontier.json, not the sidecar witnesses) do
    // not see. Skipped for pre-pin locks, whose `verifiers.package` is empty.
    if !lock.verifiers.package.is_empty() {
        let actual_kinds = collect_verifier_kinds(path);
        if lock.verifiers.kinds != actual_kinds {
            issues.push(issue(
                "frontier_lock_mismatch",
                format!(
                    "vela.lock verifiers.kinds does not match the witnesses on disk: lock={:?}, current={actual_kinds:?} (run `vela frontier materialize`)",
                    lock.verifiers.kinds
                ),
            ));
        }
    }

    let visible_path = path.join("frontier.json");
    if !visible_path.is_file() {
        issues.push(issue(
            "missing_materialized_frontier",
            "Split frontier repo is missing frontier.json.",
        ));
        return issues;
    }
    match crate::repo::load_project_file(&visible_path) {
        Ok(visible) => {
            let visible_hash = prefixed(events::snapshot_hash(&visible));
            if visible_hash != expected_snapshot {
                issues.push(issue(
                    "frontier_lock_mismatch",
                    format!(
                        "frontier.json does not match .vela materialized state: visible={visible_hash}, current={expected_snapshot}",
                    ),
                ));
            }
        }
        Err(e) => issues.push(issue("invalid_materialized_frontier", e)),
    }

    issues
}

fn validate_v1_layout(
    path: &Path,
    project: &Project,
    profile: &FrontierProfileV1,
    lock: &FrontierLockV1,
    issues: &mut Vec<RepoLayoutIssue>,
) {
    let projection = match profile.project(project) {
        Ok(projection) => projection,
        Err(error) => {
            issues.push(issue("invalid_frontier_profile", error));
            return;
        }
    };
    let visible = project_with_frontier_id(project);
    let hash_project = visible.as_ref().unwrap_or(project);
    let expected_legacy_snapshot = prefixed(events::snapshot_hash(hash_project));
    let expected_event_log = prefixed(events::event_log_hash(&project.events));
    let expected_proposals = proposal_state_hash(&project.proposals);
    for (field, actual, expected) in [
        (
            "frontier_id",
            lock.frontier_id.as_str(),
            projection.frontier_id.as_str(),
        ),
        (
            "profile_root",
            lock.profile_root.as_str(),
            projection.profile_root.as_str(),
        ),
        (
            "identity_root",
            lock.identity_root.as_str(),
            projection.identity_root.as_str(),
        ),
        (
            "dependency_root",
            lock.dependency_root.as_str(),
            projection.dependency_root.as_str(),
        ),
        (
            "scientific_state_root",
            lock.scientific_state_root.as_str(),
            projection.scientific_state_root.as_str(),
        ),
        (
            "legacy_snapshot_root",
            lock.legacy_snapshot_root.as_str(),
            expected_legacy_snapshot.as_str(),
        ),
        (
            "event_log_root",
            lock.event_log_root.as_str(),
            expected_event_log.as_str(),
        ),
        (
            "proposal_root",
            lock.proposal_root.as_str(),
            expected_proposals.as_str(),
        ),
    ] {
        if actual != expected {
            issues.push(issue(
                "frontier_lock_mismatch",
                format!("vela.lock {field} does not match derived state: lock={actual}, current={expected}"),
            ));
        }
    }
    if lock.event_count != project.events.len() as u64 {
        issues.push(issue(
            "frontier_lock_mismatch",
            format!(
                "vela.lock event_count does not match .vela/events: lock={}, current={}",
                lock.event_count,
                project.events.len()
            ),
        ));
    }
    if lock.dependencies != projection.dependencies {
        issues.push(issue(
            "frontier_lock_mismatch",
            "vela.lock dependencies do not match the effective identity-event payload.",
        ));
    }
    for (field, actual, expected) in [
        (
            "sources_hash",
            lock.sources_hash.as_str(),
            directory_hash(&path.join("sources")),
        ),
        (
            "artifacts_hash",
            lock.artifacts_hash.as_str(),
            directory_hash(&path.join("artifacts")),
        ),
        (
            "review_hash",
            lock.review_hash.as_str(),
            directory_hash(&path.join("review")),
        ),
    ] {
        if actual != expected {
            issues.push(issue(
                "frontier_lock_mismatch",
                format!("vela.lock {field} does not match repository bytes: lock={actual}, current={expected}"),
            ));
        }
    }
    if !lock.verifiers.package.is_empty() && lock.verifiers.kinds != collect_verifier_kinds(path) {
        issues.push(issue(
            "frontier_lock_mismatch",
            "vela.lock verifiers.kinds does not match the witnesses on disk.",
        ));
    }
    if !lock.proof.digest.is_empty() && lock.proof.digest != directory_hash(&path.join("proof")) {
        issues.push(issue(
            "frontier_lock_mismatch",
            "vela.lock proof digest does not match proof/.",
        ));
    }
    let visible_path = path.join("frontier.json");
    if !visible_path.is_file() {
        issues.push(issue(
            "missing_materialized_frontier",
            "Split frontier repo is missing frontier.json.",
        ));
        return;
    }
    let expected_visible = render_visible_state(
        path,
        project,
        &materialization_generated_at(path, project),
        None,
    );
    match (fs::read(&visible_path), expected_visible) {
        (Ok(actual), Ok(expected)) if actual == expected => {}
        (Ok(_), Ok(_)) => issues.push(issue(
            "frontier_lock_mismatch",
            "frontier.json does not match the exact Profile v1 read projection.",
        )),
        (Err(error), _) => issues.push(issue("invalid_materialized_frontier", error.to_string())),
        (_, Err(error)) => issues.push(issue("invalid_materialized_frontier", error)),
    }
}

pub fn manifest_overrides(path: &Path) -> Result<Option<FrontierManifest>, String> {
    read_manifest(path)
}
pub fn proof_verify(path: &Path) -> Result<serde_json::Value, String> {
    let project = crate::repo::load_from_path(path)?;
    let lock = read_repository_lock(path)?;
    let proof_path = path.join("proof/latest.json");
    let mut issues = layout_issues(path, &project)
        .into_iter()
        .map(|issue| {
            json!({
                "rule_id": issue.rule_id,
                "message": issue.message,
            })
        })
        .collect::<Vec<_>>();
    let locked = project_with_frontier_id(&project)?;
    let snapshot_hash = prefixed(events::snapshot_hash(&locked));
    let event_log_hash = prefixed(events::event_log_hash(&locked.events));
    let mut latest_payload = serde_json::Value::Null;
    if !proof_path.is_file() {
        issues.push(json!({
            "rule_id": "missing_proof_latest",
            "message": "proof/latest.json is missing.",
        }));
    } else {
        let data = fs::read_to_string(&proof_path)
            .map_err(|e| format!("Failed to read proof/latest.json: {e}"))?;
        latest_payload = serde_json::from_str(&data).map_err(|e| {
            format!(
                "Failed to parse proof/latest.json '{}': {e}",
                proof_path.display()
            )
        })?;
        if latest_payload
            .get("frontier_hash")
            .and_then(|value| value.as_str())
            != Some(snapshot_hash.as_str())
        {
            issues.push(json!({
                "rule_id": "proof_snapshot_mismatch",
                "message": "proof/latest.json frontier_hash does not match replayed frontier state.",
            }));
        }
        if latest_payload
            .get("event_log_hash")
            .and_then(|value| value.as_str())
            != Some(event_log_hash.as_str())
        {
            issues.push(json!({
                "rule_id": "proof_event_log_mismatch",
                "message": "proof/latest.json event_log_hash does not match .vela/events/.",
            }));
        }
    }
    let proof_digest = directory_hash(&path.join("proof"));
    if let Some(lock) = &lock {
        let locked_proof = match lock {
            FrontierLockFile::LegacyV0_1(lock) => &lock.proof,
            FrontierLockFile::V1(lock) => &lock.proof,
        };
        if !locked_proof.digest.is_empty() && locked_proof.digest != proof_digest {
            issues.push(json!({
                "rule_id": "proof_digest_mismatch",
                "message": format!("proof/ digest does not match vela.lock: lock={}, current={proof_digest}", locked_proof.digest),
            }));
        }
    } else {
        issues.push(json!({
            "rule_id": "missing_frontier_lock",
            "message": "vela.lock is missing.",
        }));
    }

    Ok(json!({
        "schema": FRONTIER_PROOF_VERIFY_SCHEMA,
        "ok": issues.is_empty(),
        "path": path.display().to_string(),
        "frontier_id": locked.frontier_id(),
        "snapshot_hash": snapshot_hash,
        "event_log_hash": event_log_hash,
        "proof_digest": proof_digest,
        "proof": latest_payload,
        "issues": issues,
    }))
}

pub fn proof_explain(path: &Path) -> Result<String, String> {
    let project = crate::repo::load_from_path(path)?;
    let report = proof_verify(path)?;
    let ok = report.get("ok").and_then(|value| value.as_bool()) == Some(true);
    let locked = project_with_frontier_id(&project)?;
    let snapshot_hash = prefixed(events::snapshot_hash(&locked));
    let event_log_hash = prefixed(events::event_log_hash(&locked.events));
    let open_proposals = project
        .proposals
        .iter()
        .filter(|proposal| {
            !matches!(
                proposal.status.as_str(),
                "accepted" | "applied" | "rejected"
            )
        })
        .count();
    let status = if ok { "fresh" } else { "stale or invalid" };
    Ok(format!(
        "vela proof explain\n\nFrontier: {}\nFrontier id: {}\nProof status: {status}\nAccepted events: {}\nOpen proposals: {open_proposals}\nSnapshot hash: {snapshot_hash}\nEvent log hash: {event_log_hash}\n\nAuthority: `.vela/events/` is replayed into `frontier.json`.\nVisible proof: `proof/latest.json`, `proof/events.manifest.jsonl`, and `proof/replay.trace.jsonl`.\nLockfile: `vela.lock` binds the event log, Vela reducer, verifier set, visible state, and proof digest.\n",
        project.project.name,
        locked.frontier_id(),
        project.events.len(),
    ))
}

fn empty_project(name: &str, description: &str, compiled_at: &str) -> Project {
    Project {
        vela_version: project::VELA_SCHEMA_VERSION.to_string(),
        schema: project::VELA_SCHEMA_URL.to_string(),
        frontier_id: None,
        project: project::ProjectMeta {
            name: name.to_string(),
            description: description.to_string(),
            compiled_at: compiled_at.to_string(),
            compiler: project::VELA_COMPILER_VERSION.to_string(),
            papers_processed: 0,
            errors: 0,
            dependencies: Vec::new(),
        },
        stats: project::ProjectStats::default(),
        findings: Vec::new(),
        sources: Vec::new(),
        evidence_atoms: Vec::new(),
        condition_records: Vec::new(),
        review_events: Vec::new(),
        confidence_updates: Vec::new(),
        events: Vec::new(),
        proposals: Vec::new(),
        proof_state: proposals::ProofState::default(),
        signatures: Vec::new(),
        actors: Vec::new(),
        artifacts: Vec::new(),
        released_diff_packs: Vec::new(),
        verdict_conflicts: Vec::new(),
        contradictions: Vec::new(),
        verifier_attachments: Vec::new(),
        attempts: Vec::new(),
        attempt_resolutions: Vec::new(),
        transfers: Vec::new(),
        endorsements: Vec::new(),
        statement_attestations: Vec::new(),
        anchor_links: Vec::new(),
        attempt_claims: Vec::new(),
        statement_registrations: Vec::new(),
    }
}

fn write_visible_state(path: &Path, project: &Project, generated_at: &str) -> Result<(), String> {
    let bytes = render_visible_state(path, project, generated_at, None)?;
    fs::write(path.join("frontier.json"), bytes)
        .map_err(|e| format!("Failed to write frontier.json: {e}"))
}

fn render_visible_state(
    path: &Path,
    project: &Project,
    generated_at: &str,
    profile_override: Option<&FrontierProfileV1>,
) -> Result<Vec<u8>, String> {
    let visible = project_with_frontier_id(project)?;
    let snapshot_hash = prefixed(events::snapshot_hash(&visible));
    let event_log_hash = prefixed(events::event_log_hash(&visible.events));
    let mut value = serde_json::to_value(&visible)
        .map_err(|e| format!("Failed to prepare frontier.json: {e}"))?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "_warning".to_string(),
            serde_json::Value::String(
                "Generated by Vela. Do not edit frontier.json directly; use Vela's task-first receipt, signing, materialization, and proof commands."
                    .to_string(),
            ),
        );
        let profile = match profile_override {
            Some(profile) => Some(profile.clone()),
            None => match read_repository_profile(path)? {
                Some(FrontierProfileFile::V1(profile)) => Some(profile),
                _ => None,
            },
        };
        let metadata = match profile {
            Some(profile) => {
                let projection = profile.project(&visible)?;
                json!({
                    "schema": "vela.frontier_state_meta.v1",
                    "generated_at": generated_at,
                    "materialized_from": ".vela/events/",
                    "proof": "proof/latest.json",
                    "lockfile": "vela.lock",
                    "events_manifest": "proof/events.manifest.jsonl",
                    "replay_trace": "proof/replay.trace.jsonl",
                    "profile_root": projection.profile_root,
                    "identity_root": projection.identity_root,
                    "dependency_root": projection.dependency_root,
                    "scientific_state_root": projection.scientific_state_root,
                    "legacy_snapshot_root": snapshot_hash,
                    "event_log_root": event_log_hash,
                    "vela_reducer": format!("vela@{}", env!("CARGO_PKG_VERSION")),
                })
            }
            _ => json!({
                "schema": "vela.frontier_state_meta.v0.1",
                "generated_at": generated_at,
                "materialized_from": ".vela/events/",
                "proof": "proof/latest.json",
                "lockfile": "vela.lock",
                "events_manifest": "proof/events.manifest.jsonl",
                "replay_trace": "proof/replay.trace.jsonl",
                "snapshot_hash": snapshot_hash,
                "event_log_hash": event_log_hash,
                "vela_reducer": format!("vela@{}", env!("CARGO_PKG_VERSION")),
            }),
        };
        object.insert("_meta".to_string(), metadata);
    }
    serde_json::to_vec_pretty(&value).map_err(|e| format!("Failed to serialize frontier.json: {e}"))
}

fn render_synced_manifest(path: &Path, deps: &[ProjectDependency]) -> Result<Vec<u8>, String> {
    let mut manifest = match read_manifest(path)? {
        Some(m) => m,
        None => return Err("frontier.yaml disappeared while rendering visible files".to_string()),
    };
    manifest.dependencies.frontiers_v2 = deps.to_vec();
    serde_yaml::to_string(&manifest)
        .map(String::into_bytes)
        .map_err(|e| format!("Failed to serialize frontier.yaml: {e}"))
}

fn write_manifest(path: &Path, project: &Project) -> Result<(), String> {
    match read_repository_profile(path)? {
        Some(FrontierProfileFile::V1(_)) => Ok(()),
        _ => {
            let yaml = render_manifest(path, project)?;
            fs::write(path.join("frontier.yaml"), yaml)
                .map_err(|e| format!("Failed to write frontier.yaml: {e}"))
        }
    }
}

fn render_manifest(path: &Path, project: &Project) -> Result<Vec<u8>, String> {
    let existing = match read_repository_profile(path)? {
        Some(FrontierProfileFile::LegacyV0_1(manifest)) => Some(manifest),
        Some(FrontierProfileFile::V1(_)) => {
            return Err(
                "cannot render a legacy frontier manifest over a Profile v1 document".to_string(),
            );
        }
        None => None,
    };
    let existing_dependencies = existing
        .as_ref()
        .map(|manifest| manifest.dependencies.clone())
        .unwrap_or_default();
    let manifest = FrontierManifest {
        schema: FRONTIER_MANIFEST_SCHEMA.to_string(),
        layout: FRONTIER_REPO_LAYOUT.to_string(),
        mode: "split".to_string(),
        frontier_id: Some(project.frontier_id()),
        name: project.project.name.clone(),
        description: existing
            .as_ref()
            .map(|manifest| manifest.description.clone())
            .unwrap_or_else(|| project.project.description.clone()),
        visibility: "public".to_string(),
        scope: existing
            .as_ref()
            .map(|manifest| manifest.scope.clone())
            .unwrap_or_else(|| FrontierScope {
                question: project.project.description.clone(),
                includes: Vec::new(),
                excludes: Vec::new(),
            }),
        carina: None,
        vela: VelaManifest {
            reducer: format!("vela@{}", env!("CARGO_PKG_VERSION")),
        },
        paths: FrontierPaths {
            state: "frontier.json".to_string(),
            sources: "sources/".to_string(),
            artifacts: "artifacts/".to_string(),
            review: "review/".to_string(),
            proof: "proof/".to_string(),
            exports: "exports/".to_string(),
        },
        maintainers: existing
            .as_ref()
            .map(|manifest| manifest.maintainers.clone())
            .unwrap_or_default(),
        policies: existing
            .as_ref()
            .map(|manifest| manifest.policies.clone())
            .unwrap_or_default(),
        license: existing
            .as_ref()
            .map(|manifest| manifest.license.clone())
            .unwrap_or_default(),
        dependencies: ManifestDependencies {
            frontiers: existing_dependencies.frontiers,
            packages: existing_dependencies.packages,
            adapters: existing_dependencies.adapters,
            frontiers_v2: project.project.dependencies.clone(),
        },
    };
    serde_yaml::to_string(&manifest)
        .map(String::into_bytes)
        .map_err(|e| format!("Failed to serialize frontier.yaml: {e}"))
}

fn write_lock(
    path: &Path,
    project: &Project,
    proof: &ProofWrite,
    generated_at: &str,
) -> Result<FrontierLock, String> {
    let (lock, bytes) = render_lock(path, project, proof, generated_at)?;
    fs::write(path.join("vela.lock"), bytes)
        .map_err(|e| format!("Failed to write vela.lock: {e}"))?;
    Ok(lock)
}

fn render_lock(
    path: &Path,
    project: &Project,
    proof: &ProofWrite,
    generated_at: &str,
) -> Result<(FrontierLock, Vec<u8>), String> {
    let locked = project_with_frontier_id(project)?;
    let reducer_package = format!("vela@{}", env!("CARGO_PKG_VERSION"));
    let verifier_package = format!("vela-verify@{}", env!("CARGO_PKG_VERSION"));
    let lock = FrontierLock {
        schema: FRONTIER_LOCK_SCHEMA.to_string(),
        generated_at: generated_at.to_string(),
        vela_version: env!("CARGO_PKG_VERSION").to_string(),
        frontier_id: locked.frontier_id(),
        canonicalization: LockCanonicalization::default(),
        reducer: LockPackage {
            package: reducer_package.clone(),
            digest: identity_digest(&reducer_package),
        },
        verifiers: LockVerifiers {
            package: verifier_package.clone(),
            digest: identity_digest(&verifier_package),
            kinds: collect_verifier_kinds(path),
        },
        snapshot_hash: prefixed(events::snapshot_hash(&locked)),
        event_log_hash: prefixed(events::event_log_hash(&locked.events)),
        proposal_state_hash: proposal_state_hash(&locked.proposals),
        sources_hash: directory_hash(&path.join("sources")),
        artifacts_hash: directory_hash(&path.join("artifacts")),
        review_hash: directory_hash(&path.join("review")),
        proof_freshness: proof.freshness.clone(),
        proof: LockProof {
            latest: proof.latest.clone(),
            digest: proof.digest.clone(),
            freshness: proof.freshness.clone(),
            events_manifest: proof.events_manifest.clone(),
            replay_trace: proof.replay_trace.clone(),
        },
        paths: LockPaths {
            frontier: "frontier.json".to_string(),
            events: ".vela/events/".to_string(),
        },
        // v0.109: surface every cross-frontier dependency the
        // project declares, in deterministic source order, so the
        // lockfile alone witnesses what state the parent committed
        // to. Pre-v0.109 these pins lived only in `frontier.yaml`
        // and were absent from the lock; v0.109 mirrors them.
        dependencies: locked
            .project
            .dependencies
            .iter()
            .map(|d| LockedDependency {
                name: d.name.clone(),
                source: d.source.clone(),
                vfr_id: d.vfr_id.clone(),
                locator: d.locator.clone(),
                pinned_snapshot_hash: d.pinned_snapshot_hash.clone(),
            })
            .collect(),
    };
    let yaml = serde_yaml::to_string(&lock)
        .map(String::into_bytes)
        .map_err(|e| format!("Failed to serialize vela.lock: {e}"))?;
    Ok((lock, yaml))
}

fn render_lock_v1(
    path: &Path,
    project: &Project,
    profile: &FrontierProfileV1,
    proof: &ProofWrite,
    generated_at: &str,
) -> Result<(FrontierLockV1, Vec<u8>), String> {
    let locked = project_with_frontier_id(project)?;
    let projection = profile.project(&locked)?;
    let reducer_package = format!("vela@{}", env!("CARGO_PKG_VERSION"));
    let verifier_package = format!("vela-verify@{}", env!("CARGO_PKG_VERSION"));
    let lock = FrontierLockV1 {
        schema: FRONTIER_LOCK_SCHEMA_V1.to_string(),
        generated_at: generated_at.to_string(),
        vela_version: env!("CARGO_PKG_VERSION").to_string(),
        frontier_id: projection.frontier_id,
        profile_root: projection.profile_root,
        identity_root: projection.identity_root,
        dependency_root: projection.dependency_root,
        scientific_state_root: projection.scientific_state_root,
        legacy_snapshot_root: prefixed(events::snapshot_hash(&locked)),
        event_log_root: prefixed(events::event_log_hash(&locked.events)),
        event_count: locked.events.len() as u64,
        proposal_root: proposal_state_hash(&locked.proposals),
        canonicalization: LockCanonicalization::default(),
        reducer: LockPackage {
            package: reducer_package.clone(),
            digest: identity_digest(&reducer_package),
        },
        verifiers: LockVerifiers {
            package: verifier_package.clone(),
            digest: identity_digest(&verifier_package),
            kinds: collect_verifier_kinds(path),
        },
        sources_hash: directory_hash(&path.join("sources")),
        artifacts_hash: directory_hash(&path.join("artifacts")),
        review_hash: directory_hash(&path.join("review")),
        proof_freshness: proof.freshness.clone(),
        proof: LockProof {
            latest: proof.latest.clone(),
            digest: proof.digest.clone(),
            freshness: proof.freshness.clone(),
            events_manifest: proof.events_manifest.clone(),
            replay_trace: proof.replay_trace.clone(),
        },
        paths: LockPaths {
            frontier: "frontier.json".to_string(),
            events: ".vela/events/".to_string(),
        },
        dependencies: projection.dependencies,
    };
    let bytes = serde_yaml::to_string(&lock)
        .map(String::into_bytes)
        .map_err(|error| format!("Failed to serialize vela.lock v1: {error}"))?;
    Ok((lock, bytes))
}

/// Collect the sorted, unique verifier `kind`s this frontier's witnesses
/// declare, mirroring how `vela reproduce` finds witnesses: scan the top-level
/// `witnesses/` subdir if present, else the whole working tree (skipping `.vela`
/// and `.git`). Deterministic via the BTreeSet, so it does not perturb the
/// materialize-determinism gate. Empty for claim/cert frontiers with no witnesses.
fn collect_verifier_kinds(path: &Path) -> Vec<String> {
    let witnesses_dir = path.join("witnesses");
    let root = if witnesses_dir.is_dir() {
        witnesses_dir
    } else {
        path.to_path_buf()
    };
    let mut files = Vec::new();
    collect_witness_json(&root, &mut files);
    let mut kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in files {
        if let Ok(text) = fs::read_to_string(&f)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(k) = v.get("kind").and_then(|k| k.as_str())
        {
            kinds.insert(k.to_string());
        }
    }
    kinds.into_iter().collect()
}

fn collect_witness_json(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if matches!(
                p.file_name().and_then(|n| n.to_str()),
                Some(".vela") | Some(".git")
            ) {
                continue;
            }
            collect_witness_json(&p, out);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".witness.json"))
        {
            out.push(p);
        }
    }
}

/// The materialization timestamp stamped into `frontier.json`/`vela.lock`/`proof`.
///
/// Deterministic by construction: a pure function of the event log, never the
/// wall clock, so a given `.vela/events/` always materializes byte-identically
/// (the property CI's parity gate and a cold reproduce both rely on). We use the
/// latest event timestamp — the log head — and a fixed epoch sentinel for an
/// empty log. This value feeds no hash (`snapshot_hash`/`event_log_hash` exclude
/// it); it is human-facing metadata only.
fn materialization_generated_at(_path: &Path, project: &Project) -> String {
    project
        .events
        .iter()
        .map(|e| e.timestamp.as_str())
        .max()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

fn project_with_frontier_id(project: &Project) -> Result<Project, String> {
    let frontier_id = project.frontier_id();
    let mut value = serde_json::to_value(project)
        .map_err(|e| format!("Failed to prepare frontier state: {e}"))?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "frontier_id".to_string(),
            serde_json::Value::String(frontier_id),
        );
    }
    serde_json::from_value(value).map_err(|e| format!("Failed to normalize frontier state: {e}"))
}

fn write_frontier_card(path: &Path, name: &str) -> Result<(), String> {
    let text = format!(
        "# {name}\n\nThis is a Vela frontier repository. Agents follow `next -> work -> land`; a human uses `sign` for deferred decisions, and Git publishes the resulting bytes.\n\n- State entrypoint: `frontier.json`\n- Manifest: `frontier.yaml`\n- Lockfile: `vela.lock`\n\nRun:\n\n```bash\nvela agents sync . --json\nvela status . --json\nvela next . --json\nvela check . --strict --json\n```\n"
    );
    fs::write(path.join("README.md"), text).map_err(|e| format!("Failed to write README.md: {e}"))
}

fn write_frontier_card_v1(path: &Path, name: &str, scope: &str) -> Result<(), String> {
    let text = format!(
        "# {name}\n\n{scope}\n\nThis repository is a Vela Frontier. Canonical state lives in `.vela/`; `frontier.json` and `vela.lock` are generated read projections.\n\n## Start here\n\n```bash\nvela status . --json\nvela next . --json\nvela check . --strict --json\n```\n\nAgents may produce and land bounded evidence. Only verified policy or an exact protected human decision changes accepted scientific state.\n"
    );
    fs::write(path.join("README.md"), text).map_err(|error| {
        format!(
            "Failed to write Profile v1 README.md '{}': {error}",
            path.join("README.md").display()
        )
    })
}

fn write_scope(path: &Path, name: &str) -> Result<(), String> {
    let text = format!(
        "# Scope\n\nFrontier: {name}\n\nThis file records boundaries, exclusions, caveats, and review policy for the frontier.\n\nExternal artifacts and agent outputs are source material until reviewed into accepted Vela events.\n"
    );
    fs::write(path.join("SCOPE.md"), text).map_err(|e| format!("Failed to write SCOPE.md: {e}"))
}

fn write_scope_v1(path: &Path, scope: &str) -> Result<(), String> {
    let text = format!(
        "# Scope\n\n## Question\n\n{scope}\n\n## Includes\n\nNo additional inclusions are declared.\n\n## Excludes\n\nNo exclusions are declared.\n"
    );
    fs::write(path.join("SCOPE.md"), text).map_err(|error| {
        format!(
            "Failed to write Profile v1 SCOPE.md '{}': {error}",
            path.join("SCOPE.md").display()
        )
    })
}

fn write_proof(path: &Path, project: &Project, generated_at: &str) -> Result<ProofWrite, String> {
    let rendered = render_proof(path, project, generated_at, None)?;
    let proof_dir = path.join("proof");
    fs::create_dir_all(&proof_dir).map_err(|e| format!("Failed to create proof/: {e}"))?;
    for (relative_path, bytes) in &rendered.files {
        fs::write(proof_dir.join(relative_path), bytes)
            .map_err(|e| format!("Failed to write proof/{relative_path}: {e}"))?;
    }
    Ok(rendered.proof)
}

fn render_proof(
    path: &Path,
    project: &Project,
    generated_at: &str,
    profile_override: Option<&FrontierProfileV1>,
) -> Result<RenderedProof, String> {
    let locked = project_with_frontier_id(project)?;
    let proof_dir = path.join("proof");

    // Freshness skip (safe, deterministic): the proof packet is a pure function
    // of (event log, reducer version). If the recorded proof already pins this
    // exact event log and reducer, regenerating is byte-identical work — so skip
    // the O(N^2) manifest/trace rebuild (the cumulative per-event hash makes a
    // full rebuild quadratic in event count: the bbb-flagship 22s wall). Proof
    // freshness is defined by event_log_hash (see state_integrity::proof_freshness),
    // so gating on it matches the system's own freshness contract. The first
    // materialize after a real change still regenerates in full.
    let event_log_hash = prefixed(events::event_log_hash(&locked.events));
    let snapshot_hash = prefixed(events::snapshot_hash(&locked));
    let profile_projection = match profile_override {
        Some(profile) => Some(profile.project(&locked)?),
        None => match read_repository_profile(path)? {
            Some(FrontierProfileFile::V1(profile)) => Some(profile.project(&locked)?),
            _ => None,
        },
    };
    let (event_root_field, state_root_field, state_root) = match &profile_projection {
        Some(projection) => (
            "event_log_root",
            "scientific_state_root",
            projection.scientific_state_root.as_str(),
        ),
        None => ("event_log_hash", "frontier_hash", snapshot_hash.as_str()),
    };
    if proof_is_current(
        &proof_dir,
        event_root_field,
        &event_log_hash,
        state_root_field,
        state_root,
    ) {
        return Ok(RenderedProof {
            files: VisibleRepoFiles::new(),
            proof: ProofWrite {
                digest: directory_hash(&proof_dir),
                freshness: "fresh".to_string(),
                latest: "proof/latest.json".to_string(),
                events_manifest: "proof/events.manifest.jsonl".to_string(),
                replay_trace: "proof/replay.trace.jsonl".to_string(),
            },
        });
    }
    let proposal_state_hash = proposal_state_hash(&locked.proposals);
    let reducer_package = format!("vela@{}", env!("CARGO_PKG_VERSION"));

    let common_reducer = json!({
        "name": "vela",
        "version": env!("CARGO_PKG_VERSION"),
        "package": reducer_package,
        "digest": identity_digest(&format!("vela@{}", env!("CARGO_PKG_VERSION"))),
    });
    let common_paths = json!({
        "frontier": "frontier.json",
        "lockfile": "vela.lock",
        "events_authority": ".vela/events/",
        "events_manifest": "proof/events.manifest.jsonl",
        "replay_trace": "proof/replay.trace.jsonl"
    });
    let latest = match &profile_projection {
        Some(projection) => json!({
            "schema": "vela.frontier_repo_proof.v1",
            "frontier_id": projection.frontier_id,
            "profile_root": projection.profile_root,
            "identity_root": projection.identity_root,
            "dependency_root": projection.dependency_root,
            "scientific_state_root": projection.scientific_state_root,
            "legacy_snapshot_root": snapshot_hash,
            "event_log_root": event_log_hash,
            "proposal_root": proposal_state_hash,
            "reducer": common_reducer,
            "materialized_at": generated_at,
            "freshness": "fresh",
            "event_count": locked.events.len(),
            "paths": common_paths,
            "warning": "Generated integrity evidence. Scientific acceptance remains event-derived."
        }),
        None => json!({
            "schema": "vela.frontier_repo_proof.v0.1",
            "frontier_id": locked.frontier_id(),
            "frontier_hash": snapshot_hash,
            "event_log_hash": event_log_hash,
            "proposal_state_hash": proposal_state_hash,
            "reducer": common_reducer,
            "materialized_at": generated_at,
            "freshness": "fresh",
            "event_count": locked.events.len(),
            "paths": common_paths,
            "warning": "Do not edit frontier.json directly. Use Vela's task-first receipt, signing, materialization, and proof commands."
        }),
    };
    let mut files = VisibleRepoFiles::new();
    files.insert(
        "latest.json".to_string(),
        serde_json::to_vec_pretty(&latest)
            .map_err(|e| format!("Failed to serialize proof/latest.json: {e}"))?,
    );

    let mut manifest_lines = String::new();
    let mut trace_lines = String::new();
    // O(N) cumulative checkpoint chain: h_i = sha256(h_{i-1} || event_hash_i),
    // computed from the already-hashed event in O(1) per step. The previous
    // formulation recomputed event_log_hash over the whole [0..=idx] prefix each
    // step, which is O(N^2) and was the bbb-flagship 22s materialize wall. This
    // field is diagnostic (never verified against the canonical event_log_hash —
    // grep confirms nothing reads it), so a running chain is the correct O(N)
    // replacement. Schema bumped to v0.2 to mark the changed value semantics.
    let mut chained_log_hash = String::new();
    // Emit the manifest/trace in canonical event-id order, matching
    // `event_log_hash`'s ordering, so the proof files are byte-identical
    // regardless of how the log was loaded (directory read, packet array, …).
    let mut ordered: Vec<_> = locked.events.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    for (idx, event) in ordered.into_iter().enumerate() {
        let event_hash = prefixed(event_hash(event));
        let entry = json!({
            "schema": "vela.proof_event_manifest_entry.v0.1",
            "index": idx + 1,
            "id": event.id,
            "kind": event.kind,
            "target": event.target,
            "actor": event.actor,
            "timestamp": event.timestamp,
            "event_hash": event_hash,
            "before_hash": event.before_hash,
            "after_hash": event.after_hash,
            "caveat_count": event.caveats.len(),
        });
        manifest_lines.push_str(
            &serde_json::to_string(&entry)
                .map_err(|e| format!("Failed to serialize event manifest entry: {e}"))?,
        );
        manifest_lines.push('\n');

        chained_log_hash = hex::encode(Sha256::digest(
            format!("{chained_log_hash}{event_hash}").as_bytes(),
        ));
        let trace = json!({
            "schema": "vela.replay_trace_entry.v0.2",
            "step": idx + 1,
            "event": event.id,
            "kind": event.kind,
            "event_hash": event_hash,
            "event_log_hash_after": prefixed(chained_log_hash.clone()),
            "target_after_hash": event.after_hash,
        });
        trace_lines.push_str(
            &serde_json::to_string(&trace)
                .map_err(|e| format!("Failed to serialize replay trace entry: {e}"))?,
        );
        trace_lines.push('\n');
    }
    files.insert(
        "events.manifest.jsonl".to_string(),
        manifest_lines.into_bytes(),
    );
    files.insert("replay.trace.jsonl".to_string(), trace_lines.into_bytes());
    let state_label = if profile_projection.is_some() {
        "Scientific-state root"
    } else {
        "Snapshot hash"
    };
    files.insert(
        "freshness.md".to_string(),
        format!(
            "# Freshness\n\nCurrent proof status: fresh\n\n`frontier.json` was materialized from `.vela/events/` at {generated_at}.\n\nAccepted events: {}\nEvent log root: `{event_log_hash}`\n{state_label}: `{state_root}`\n\nRun:\n\n```bash\nvela check . --strict --json\nvela proof verify . --json\n```\n",
            locked.events.len()
        )
        .into_bytes(),
    );

    let hashes = match &profile_projection {
        Some(projection) => json!({
            "schema": "vela.frontier_repo_hashes.v1",
            "frontier_id": projection.frontier_id,
            "profile_root": projection.profile_root,
            "identity_root": projection.identity_root,
            "dependency_root": projection.dependency_root,
            "scientific_state_root": projection.scientific_state_root,
            "legacy_snapshot_root": snapshot_hash,
            "event_log_root": event_log_hash,
            "proposal_root": proposal_state_hash,
            "sources_hash": directory_hash(&path.join("sources")),
            "artifacts_hash": directory_hash(&path.join("artifacts")),
            "review_hash": directory_hash(&path.join("review")),
        }),
        None => json!({
            "schema": "vela.frontier_repo_hashes.v0.1",
            "frontier_id": locked.frontier_id(),
            "snapshot_hash": snapshot_hash,
            "event_log_hash": event_log_hash,
            "proposal_state_hash": proposal_state_hash,
            "sources_hash": directory_hash(&path.join("sources")),
            "artifacts_hash": directory_hash(&path.join("artifacts")),
            "review_hash": directory_hash(&path.join("review")),
        }),
    };
    files.insert(
        "hashes.json".to_string(),
        serde_json::to_vec_pretty(&hashes)
            .map_err(|e| format!("Failed to serialize proof/hashes.json: {e}"))?,
    );
    let digest = directory_hash_with_replacements(&proof_dir, &files);

    Ok(RenderedProof {
        files,
        proof: ProofWrite {
            digest,
            freshness: "fresh".to_string(),
            latest: "proof/latest.json".to_string(),
            events_manifest: "proof/events.manifest.jsonl".to_string(),
            replay_trace: "proof/replay.trace.jsonl".to_string(),
        },
    })
}

/// True when the recorded proof packet already pins this exact event log and
/// reducer version (and its files exist) — so regeneration would be a byte-for-byte
/// no-op and can be skipped. Reads only `proof/latest.json` (O(1)); the event-log
/// hash is computed once by the caller (O(N), not O(N^2)).
fn proof_is_current(
    proof_dir: &Path,
    event_root_field: &str,
    event_log_root: &str,
    state_root_field: &str,
    state_root: &str,
) -> bool {
    let Ok(text) = fs::read_to_string(proof_dir.join("latest.json")) else {
        return false;
    };
    let Ok(latest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let same_log = latest.get(event_root_field).and_then(|v| v.as_str()) == Some(event_log_root);
    let same_snapshot = latest.get(state_root_field).and_then(|v| v.as_str()) == Some(state_root);
    let same_reducer = latest.pointer("/reducer/version").and_then(|v| v.as_str())
        == Some(env!("CARGO_PKG_VERSION"));
    same_log
        && same_snapshot
        && same_reducer
        && proof_dir.join("events.manifest.jsonl").is_file()
        && proof_dir.join("replay.trace.jsonl").is_file()
        && proof_dir.join("hashes.json").is_file()
}

fn proposal_state_hash(proposals: &[crate::proposals::StateProposal]) -> String {
    let bytes = crate::canonical::to_canonical_bytes(proposals).unwrap_or_default();
    prefixed(hex::encode(Sha256::digest(bytes)))
}

fn directory_hash(path: &Path) -> String {
    let mut entries = Vec::new();
    if path.is_dir() {
        collect_file_entries(path, path, &mut entries);
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = crate::canonical::to_canonical_bytes(&entries).unwrap_or_default();
    prefixed(hex::encode(Sha256::digest(bytes)))
}

fn directory_hash_with_replacements(path: &Path, replacements: &VisibleRepoFiles) -> String {
    let mut entries = Vec::new();
    if path.is_dir() {
        collect_file_entries(path, path, &mut entries);
    }
    let mut entries = entries.into_iter().collect::<BTreeMap<_, _>>();
    for (relative_path, bytes) in replacements {
        entries.insert(
            relative_path.clone(),
            prefixed(hex::encode(Sha256::digest(bytes))),
        );
    }
    let entries = entries.into_iter().collect::<Vec<_>>();
    let bytes = crate::canonical::to_canonical_bytes(&entries).unwrap_or_default();
    prefixed(hex::encode(Sha256::digest(bytes)))
}

fn collect_file_entries(root: &Path, path: &Path, entries: &mut Vec<(String, String)>) {
    let Ok(read_dir) = fs::read_dir(path) else {
        return;
    };
    for entry in read_dir.flatten() {
        let entry_path = entry.path();
        let Some(name) = entry_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == ".DS_Store" {
            continue;
        }
        if entry_path.is_dir() {
            collect_file_entries(root, &entry_path, entries);
        } else if entry_path.is_file() {
            let rel = entry_path
                .strip_prefix(root)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");
            let digest = fs::read(&entry_path)
                .map(|bytes| prefixed(hex::encode(Sha256::digest(bytes))))
                .unwrap_or_else(|_| "sha256:unreadable".to_string());
            entries.push((rel, digest));
        }
    }
}

fn event_hash(event: &crate::events::StateEvent) -> String {
    let bytes = crate::canonical::to_canonical_bytes(event).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn identity_digest(value: &str) -> String {
    prefixed(hex::encode(Sha256::digest(value.as_bytes())))
}

fn prefixed(hash: String) -> String {
    if hash.starts_with("sha256:") {
        hash
    } else {
        format!("sha256:{hash}")
    }
}

fn issue(rule_id: &str, message: impl Into<String>) -> RepoLayoutIssue {
    RepoLayoutIssue {
        rule_id: rule_id.to_string(),
        message: message.into(),
    }
}

fn default_split_mode() -> String {
    "split".to_string()
}

fn default_visibility() -> String {
    "public".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_carina_field_is_readable_only_in_legacy_manifests() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("frontier.yaml"),
            r#"schema: vela.frontier_manifest.v0.1
layout: vela.frontier_repo.v0.1
mode: split
name: Historical formal frontier
carina:
  kernel: carina@0.1.0
vela:
  reducer: vela@0.757.4
paths:
  state: frontier.json
  sources: sources/
  artifacts: artifacts/
  review: review/
  proof: proof/
  exports: exports/
"#,
        )
        .expect("write historical manifest");

        let Some(FrontierProfileFile::LegacyV0_1(manifest)) =
            read_repository_profile(tmp.path()).expect("parse historical manifest")
        else {
            panic!("expected legacy manifest");
        };
        assert_eq!(
            manifest.carina,
            Some(LegacyCarinaManifest {
                kernel: "carina@0.1.0".to_string()
            })
        );

        let project = empty_project(
            "Historical formal frontier",
            "Legacy rendering drops retired integration metadata.",
            "2026-07-23T00:00:00Z",
        );
        let rendered = String::from_utf8(
            render_manifest(tmp.path(), &project).expect("render migrated legacy manifest"),
        )
        .expect("rendered manifest is UTF-8");
        assert!(
            !rendered.contains("carina:") && !rendered.contains("carina@0.1.0"),
            "legacy re-rendering must drop retired carina metadata:\n{rendered}"
        );
        let rendered_manifest: FrontierManifest =
            serde_yaml::from_str(&rendered).expect("parse rendered legacy manifest");
        assert_eq!(rendered_manifest.carina, None);

        let profile_v1_with_carina = r#"schema: vela.frontier-profile.v1
frontier_id: vfr_0123456789abcdef
name: Closed profile
summary: Profile v1 rejects retired integration metadata.
scope:
  question: Which exact result does this Frontier maintain?
  includes:
    - Rooted evidence
  excludes:
    - Unbounded discovery
maintainers:
  - maintainer:example
license:
  content: CC-BY-4.0
  code: Apache-2.0
  data: CC0-1.0
carina:
  kernel: carina@0.1.0
"#;
        let error = FrontierProfileV1::from_yaml_str(profile_v1_with_carina)
            .expect_err("Profile v1 must reject the retired carina field");
        assert!(
            error.contains("unknown field `carina`"),
            "unexpected closed Profile v1 error: {error}"
        );
    }

    #[test]
    fn visible_repo_render_is_read_only_and_byte_equivalent_to_write() {
        let tmp = tempfile::tempdir().expect("tempdir");
        initialize(
            tmp.path(),
            InitOptions {
                name: "Rendered frontier",
                initialize_git: false,
            },
        )
        .expect("initialize frontier");
        let source = crate::repo::VelaSource::VelaRepo(tmp.path().to_path_buf());
        let mut project = crate::repo::load(&source).expect("load frontier");
        project.project.description = "rendered without a frontier write".to_string();
        let original_frontier = fs::read(tmp.path().join("frontier.json")).unwrap();

        let rendered = render_visible_repo_files(tmp.path(), &project).expect("render files");

        assert_eq!(
            fs::read(tmp.path().join("frontier.json")).unwrap(),
            original_frontier,
            "rendering must not modify the frontier"
        );
        assert!(rendered.contains_key("frontier.json"));
        assert!(rendered.contains_key("frontier.yaml"));
        assert!(rendered.contains_key("proof/latest.json"));
        assert!(rendered.contains_key("vela.lock"));

        write_visible_repo_files(tmp.path(), &project).expect("write rendered files");
        for (relative_path, expected) in rendered {
            assert_eq!(
                fs::read(tmp.path().join(&relative_path)).unwrap(),
                expected,
                "rendered bytes differ for {relative_path}"
            );
        }
    }

    #[test]
    fn initialize_writes_unfiltered_frontier_gate_for_every_public_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        initialize(
            tmp.path(),
            InitOptions {
                name: "Unskippable frontier gate",
                initialize_git: false,
            },
        )
        .expect("initialize frontier");

        let workflow = fs::read_to_string(tmp.path().join(".github/workflows/vela-frontier.yml"))
            .expect("read generated frontier workflow");
        let triggers = workflow
            .split_once("\npermissions:")
            .map(|(triggers, _)| triggers)
            .expect("workflow has permissions boundary after triggers");

        assert!(triggers.contains("  push:\n    branches: [main]"));
        assert!(triggers.contains("  pull_request: {}"));
        assert!(triggers.contains("  workflow_dispatch: {}"));
        let expected_release = current_vela_release();
        assert!(workflow.contains(&format!(
            "      - uses: {VELA_ACTION_REPOSITORY}@{expected_release}"
        )));
        assert!(workflow.contains(&format!("          vela-version: {expected_release}")));
        assert!(!workflow.contains("          strict:"));
        assert!(!workflow.contains("@main"));
        assert!(!workflow.contains("constellate-science/vela"));
        assert!(!workflow.contains("vela sign"));

        let has_path_filter = triggers.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('#') && (line.contains("paths:") || line.contains("paths-ignore:"))
        });
        let public_canonical_changes = [
            ".vela/events/ve_example.json",
            "witnesses/vw_example/witness.json",
            "records/receipt.json",
            "sources/source.pdf",
            "artifacts/va_example",
            "review/decision-brief.json",
            "proof/latest.json",
            "frontier.json",
            "frontier.yaml",
            "vela.lock",
        ];
        for changed_path in public_canonical_changes {
            assert!(
                !has_path_filter,
                "a direct change to public canonical path {changed_path} must trigger the generated frontier gate"
            );
        }
    }

    #[test]
    fn init_commands_quote_frontier_paths_as_one_shell_argument() {
        let commands = init_next_commands(Path::new("frontier with 'quotes' and spaces"));
        assert_eq!(
            commands[0],
            "vela agents sync 'frontier with '\"'\"'quotes'\"'\"' and spaces' --json"
        );
        assert!(
            commands
                .iter()
                .all(|command| command.contains("'frontier with '\"'\"'quotes'\"'\"' and spaces'"))
        );
    }

    #[test]
    fn initialize_writes_canonical_git_attributes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let payload = initialize(
            tmp.path(),
            InitOptions {
                name: "Attribute-safe frontier",
                initialize_git: false,
            },
        )
        .expect("initialize frontier");

        let attributes = fs::read_to_string(tmp.path().join(".gitattributes"))
            .expect("read generated attributes");
        assert!(attributes.contains("* text=auto eol=lf"));
        assert!(attributes.contains(".vela/** -filter -ident -working-tree-encoding -merge -text"));
        assert!(attributes.contains(
            ".vela/events/** -filter -ident -working-tree-encoding -merge diff text eol=lf"
        ));
        assert!(attributes.contains(
            "frontier.json -filter -ident -working-tree-encoding -merge diff text eol=lf"
        ));
        assert!(
            attributes.contains("review/** -filter -ident -working-tree-encoding -merge -text")
        );
        assert!(attributes.contains("witnesses/** filter=lfs diff=lfs merge=lfs -text"));
        let ignore =
            fs::read_to_string(tmp.path().join(".gitignore")).expect("read generated ignore rules");
        assert!(ignore.contains("/.vela/agents/"));
        assert!(ignore.contains("/.vela/keys/"));
        assert!(ignore.contains("/.vela/operation-journals/"));
        assert!(ignore.contains("/.vela/work/"));
        assert!(ignore.contains("/.vela/artifact-blobs/"));
        assert!(ignore.contains("/exports/"));
        assert!(
            !ignore.lines().any(|line| line.trim() == "/proof/"),
            "materialized proof review bytes must remain clone-visible"
        );
        assert!(
            !ignore.lines().any(|line| line.trim() == "/records/"),
            "public compatibility records must survive a clean clone"
        );
        assert!(
            payload["wrote"]
                .as_array()
                .expect("wrote array")
                .iter()
                .any(|entry| entry == ".gitattributes")
        );
    }

    #[test]
    fn proof_current_requires_matching_snapshot_hash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let proof_dir = tmp.path();
        fs::write(
            proof_dir.join("latest.json"),
            serde_json::to_string_pretty(&json!({
                "event_log_hash": "sha256:event-log",
                "frontier_hash": "sha256:old-snapshot",
                "reducer": {
                    "version": env!("CARGO_PKG_VERSION")
                }
            }))
            .expect("serialize proof latest"),
        )
        .expect("write proof latest");
        fs::write(proof_dir.join("events.manifest.jsonl"), "").expect("write events manifest");
        fs::write(proof_dir.join("replay.trace.jsonl"), "").expect("write replay trace");
        fs::write(proof_dir.join("hashes.json"), "{}").expect("write hashes");

        assert!(!proof_is_current(
            proof_dir,
            "event_log_hash",
            "sha256:event-log",
            "frontier_hash",
            "sha256:new-snapshot"
        ));
    }

    #[test]
    fn collect_verifier_kinds_scans_witnesses_dir_sorted_unique() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let wdir = root.join("witnesses");
        fs::create_dir_all(&wdir).expect("mkdir witnesses");
        fs::write(wdir.join("a.witness.json"), r#"{"kind":"sidon","n":8}"#).unwrap();
        fs::write(wdir.join("b.witness.json"), r#"{"kind":"golomb","n":5}"#).unwrap();
        fs::write(wdir.join("c.witness.json"), r#"{"kind":"sidon","n":9}"#).unwrap();
        // a non-witness json is ignored
        fs::write(wdir.join("notes.json"), r#"{"kind":"ignored"}"#).unwrap();
        // When a top-level witnesses/ exists, only it is scanned, so a stray
        // witness elsewhere is NOT picked up (mirrors collect_witness_files).
        fs::write(root.join("stray.witness.json"), r#"{"kind":"cap"}"#).unwrap();
        assert_eq!(
            collect_verifier_kinds(root),
            vec!["golomb".to_string(), "sidon".to_string()]
        );
    }

    #[test]
    fn collect_verifier_kinds_falls_back_to_whole_tree_when_no_witnesses_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let nested = root.join("discoveries").join("sweep");
        fs::create_dir_all(&nested).expect("mkdir nested");
        fs::write(nested.join("x.witness.json"), r#"{"kind":"costas"}"#).unwrap();
        // .vela is skipped even though it may hold event payloads
        let vela = root.join(".vela").join("events");
        fs::create_dir_all(&vela).unwrap();
        fs::write(vela.join("e.witness.json"), r#"{"kind":"should_skip"}"#).unwrap();
        assert_eq!(collect_verifier_kinds(root), vec!["costas".to_string()]);
    }
}
