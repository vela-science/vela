//! Read-only repository-context verification for `frontier.repository_bound`.
//!
//! The protocol crate validates the closed event bytes, signature possession,
//! and the identity-event chain. This edge module supplies the facts that are
//! intentionally absent from those bytes: immutable Git object availability,
//! ancestry, exact anchored Vela roots, retained bytes, and the active
//! administrator record at the anchor. It never uses event timestamps to infer
//! membership or authority and never mutates the repository.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path};
#[cfg(test)]
use std::process::Command;
use std::process::{Output, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use vela_protocol::events::{self, EVENT_KIND_KEY_REVOKE, StateEvent};
use vela_protocol::frontier_repository::{
    ExactFrontierDependencyV1, FrontierRepositoryBoundaryPayloadV1, GitObjectFormat,
    RetainedObjectEntryV1, RetainedObjectManifestV1, repository_boundary_payload_from_event_shape,
    repository_identity_event_content_root, validate_repository_boundary_event_set,
    verify_repository_boundary_signature_only,
};
use vela_protocol::project::Project;
use vela_protocol::receipt_v1::ReceiptV1;
use vela_protocol::{canonical, proposals, repo};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryAnchorFacts {
    pub git_object_format: GitObjectFormat,
    pub git_commit: String,
    pub git_tree: String,
    pub event_log_root: String,
    pub event_count: u64,
    pub snapshot_root: String,
    pub snapshot_schema: String,
    pub proposal_root: String,
    pub actor_registry_root: String,
    pub artifact_registry_root: String,
    pub canonical_store_root: String,
    pub retained_object_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryBoundaryContext {
    pub event_id: String,
    pub frontier_id: String,
    pub anchor: RepositoryAnchorFacts,
}

/// Derive the exact dependency pin represented by one verified repository
/// boundary.
///
/// This deliberately resolves the boundary's immutable anchor rather than the
/// dependency repository's mutable current worktree. It is the migration
/// bridge from a legacy snapshot pin to Profile v1 identity and scientific
/// state. The ordinary boundary verifier supplies Git ancestry, retained-byte,
/// actor-registry, signature, and external-pin checks before any value is
/// returned.
pub fn derive_exact_dependency_at_boundary(
    project: &Project,
    repo_path: &Path,
    boundary_event: &StateEvent,
    trust_anchor: &RepositoryTrustAnchor,
) -> Result<ExactFrontierDependencyV1, String> {
    let context = verify_repository_boundary_context_with_trust_anchor(
        project,
        repo_path,
        boundary_event,
        Some(trust_anchor),
    )?;
    let payload = verify_repository_boundary_signature_only(
        boundary_event,
        &trust_anchor.administrator_public_key,
    )?;
    let anchored = anchored_repository(repo_path, &context.anchor.git_commit)?;
    if anchored.project.frontier_id() != payload.frontier_id {
        return Err(
            "dependency boundary anchor Frontier ID does not match the signed boundary".to_string(),
        );
    }
    let scientific_state_root = vela_protocol::scientific_state::scientific_state_root_v2(
        &anchored.project,
        &payload.identity_root,
        &payload.dependency_root,
    )?;
    Ok(ExactFrontierDependencyV1 {
        frontier_id: payload.frontier_id,
        identity_root: payload.identity_root,
        scientific_state_root,
        git_object_format: context.anchor.git_object_format,
        git_commit: context.anchor.git_commit,
        git_tree: context.anchor.git_tree,
    })
}

/// Derive an exact historical dependency state authenticated by the first
/// temporalization boundary of a fully verified repository chain.
///
/// This is deliberately narrower than arbitrary historical resolution. The
/// selected commit must be an exact retained ancestor of the signed
/// temporalization anchor, every historical canonical object must remain in
/// that anchor, and both states must have the empty dependency context. The
/// boundary authenticates stable repository identity; it does not
/// retroactively attribute historical events to the administrator or confer
/// scientific standing.
pub fn derive_exact_dependency_at_temporalized_ancestor(
    project: &Project,
    repo_path: &Path,
    boundary_event: &StateEvent,
    trust_anchor: &RepositoryTrustAnchor,
    historical_commit: &str,
    expected_legacy_snapshot_root: &str,
) -> Result<ExactFrontierDependencyV1, String> {
    validate_sha256_root(
        "expected historical legacy snapshot root",
        expected_legacy_snapshot_root,
    )?;
    verify_repository_boundary_context_with_trust_anchor(
        project,
        repo_path,
        boundary_event,
        Some(trust_anchor),
    )?;

    let chain = select_unique_boundary_chain(&project.events, boundary_event)?;
    let temporalization = chain
        .entries
        .first()
        .ok_or_else(|| "repository boundary chain is empty".to_string())?;
    if temporalization.payload.mode
        != vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::TemporalizeExisting
    {
        return Err(
            "historical dependency state requires a legacy temporalization boundary".to_string(),
        );
    }

    let boundary_anchor =
        anchored_repository(repo_path, &temporalization.payload.anchor_git_commit)?;
    let historical = anchored_repository(repo_path, historical_commit)
        .map_err(|error| format!("historical dependency state unavailable: {error}"))?;
    let historical_replay = vela_protocol::reducer::verify_replay(&historical.project);
    if !historical_replay.ok {
        return Err(format!(
            "historical dependency state does not replay exactly: found {} diff(s)",
            historical_replay.diffs.len()
        ));
    }
    if historical.facts.git_object_format != boundary_anchor.facts.git_object_format {
        return Err(
            "historical dependency Git object format does not match the temporalization anchor"
                .to_string(),
        );
    }
    verify_anchor_descends_from(
        repo_path,
        &historical.facts.git_commit,
        &boundary_anchor.facts.git_commit,
    )
    .map_err(|_| {
        "historical dependency commit is not an ancestor of the signed temporalization anchor"
            .to_string()
    })?;
    if historical.project.frontier_id() != temporalization.payload.frontier_id {
        return Err(
            "historical dependency Frontier ID does not match the signed temporalization boundary"
                .to_string(),
        );
    }

    let historical_legacy_identity = derive_legacy_identity_preimage_root(&historical.project)?;
    if temporalization
        .payload
        .legacy_identity_preimage_root
        .as_deref()
        != Some(historical_legacy_identity.as_str())
    {
        return Err(
            "historical dependency legacy identity preimage does not match the temporalization boundary"
                .to_string(),
        );
    }
    verify_event_membership(&boundary_anchor.project, &historical.project)
        .map_err(|error| format!("historical dependency event history is not retained: {error}"))?;
    verify_anchored_proposal_history(&boundary_anchor.project, &historical.project).map_err(
        |error| format!("historical dependency proposal history is not retained: {error}"),
    )?;
    verify_retained_manifest_membership(
        &boundary_anchor.retained_manifest,
        &historical.retained_manifest,
    )?;

    if !historical.project.project.dependencies.is_empty()
        || !boundary_anchor.project.project.dependencies.is_empty()
        || !temporalization.payload.dependencies.is_empty()
    {
        return Err(
            "historical dependency authentication currently requires an empty dependency context"
                .to_string(),
        );
    }
    let empty_dependency_root = vela_protocol::frontier_repository::exact_dependency_root(&[])?;
    if temporalization.payload.dependency_root != empty_dependency_root {
        return Err(
            "temporalization boundary does not bind the canonical empty dependency root"
                .to_string(),
        );
    }
    if historical.facts.snapshot_root != expected_legacy_snapshot_root {
        return Err(format!(
            "historical dependency legacy snapshot mismatch: expected {expected_legacy_snapshot_root}, derived {}",
            historical.facts.snapshot_root
        ));
    }

    let scientific_state_root = vela_protocol::scientific_state::scientific_state_root_v2(
        &historical.project,
        &temporalization.payload.identity_root,
        &empty_dependency_root,
    )?;
    Ok(ExactFrontierDependencyV1 {
        frontier_id: temporalization.payload.frontier_id.clone(),
        identity_root: temporalization.payload.identity_root.clone(),
        scientific_state_root,
        git_object_format: historical.facts.git_object_format,
        git_commit: historical.facts.git_commit,
        git_tree: historical.facts.git_tree,
    })
}

/// Compute the exact raw retained-store root after appending one signed
/// migration boundary to an immutable anchor.
///
/// Profile, settings, lock, proof, and target-index files are intentionally
/// absent because the retained-store contract covers protocol authority and
/// evidence bytes, not derived presentation. The new event is serialized with
/// the same pretty-JSON representation used by the repository renderer.
pub fn derive_migration_signed_store_root(
    repo_path: &Path,
    anchor_commit: &str,
    signed_boundary_event: &StateEvent,
) -> Result<String, String> {
    let payload = verify_repository_boundary_signature_only(
        signed_boundary_event,
        &repository_boundary_payload_from_event_shape(signed_boundary_event)?
            .administrator_public_key,
    )?;
    if payload.anchor_git_commit != anchor_commit {
        return Err(
            "signed migration boundary does not name the retained-store anchor commit".to_string(),
        );
    }
    let anchored = anchored_repository(repo_path, anchor_commit)?;
    let event_path = format!(".vela/events/{}.json", signed_boundary_event.id);
    if anchored
        .retained_manifest
        .0
        .iter()
        .any(|entry| entry.path == event_path)
    {
        return Err("signed migration boundary path already exists at the anchor".to_string());
    }
    let bytes = serde_json::to_vec_pretty(signed_boundary_event)
        .map_err(|error| format!("encode signed migration boundary: {error}"))?;
    let mut entries = anchored.retained_manifest.0;
    entries.push(RetainedObjectEntryV1 {
        path: event_path,
        git_mode: "100644".to_string(),
        size: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    });
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    RetainedObjectManifestV1(entries).root()
}

/// Out-of-band trust required to consume an administrator-bound repository
/// chain.
///
/// These values must come from a reviewed release, exact handoff, or another
/// channel independent of the repository being verified. In-repository actor
/// and boundary bytes cannot establish their own initial trust. A structural
/// genesis proves identity continuity, not that a particular human
/// administrator boundary is the one the consumer intended to trust.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryTrustAnchor {
    /// Full SHA-256 content root of the chain's first administrator boundary.
    /// This can be either a legacy `temporalize_existing` boundary or the
    /// first native genesis-rooted dependency boundary.
    pub boundary_content_root: String,
    /// Exact Ed25519 public key of that boundary's administrator.
    pub administrator_public_key: String,
}

impl RepositoryTrustAnchor {
    pub fn validate(&self) -> Result<(), String> {
        validate_sha256_root(
            "trust anchor boundary_content_root",
            &self.boundary_content_root,
        )?;
        validate_lower_hex(
            "trust anchor administrator_public_key",
            &self.administrator_public_key,
            64,
        )
    }
}

#[derive(Debug, Clone)]
struct GitTreeEntry {
    mode: String,
    kind: String,
    object: String,
    path: String,
}

const MAX_PROJECT_INPUT_BLOB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PROJECT_INPUT_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RETAINED_BLOB_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RETAINED_TOTAL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_RECEIPT_BLOB_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ACTOR_REGISTRY_BLOB_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct BlobReadLimits {
    max_blob_bytes: u64,
    max_total_bytes: u64,
}

#[derive(Debug)]
struct BlobReadBudget {
    limits: BlobReadLimits,
    consumed_bytes: u64,
}

impl BlobReadBudget {
    fn new(limits: BlobReadLimits) -> Self {
        Self {
            limits,
            consumed_bytes: 0,
        }
    }

    fn reserve(&mut self, entry: &GitTreeEntry, size: u64) -> Result<(), String> {
        if size > self.limits.max_blob_bytes {
            return Err(format!(
                "retained Git blob {} is {size} bytes, exceeding the per-blob limit of {} bytes",
                entry.path, self.limits.max_blob_bytes
            ));
        }
        let next = self
            .consumed_bytes
            .checked_add(size)
            .ok_or_else(|| "retained Git blob aggregate size overflowed u64".to_string())?;
        if next > self.limits.max_total_bytes {
            return Err(format!(
                "retained Git blob aggregate would reach {next} bytes at {}, exceeding the limit of {} bytes",
                entry.path, self.limits.max_total_bytes
            ));
        }
        self.consumed_bytes = next;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct GitBlobFacts {
    size: u64,
    sha256: String,
}

struct RetainedBlobReader<'a> {
    repo_path: &'a Path,
    budget: BlobReadBudget,
    facts_by_object: BTreeMap<String, GitBlobFacts>,
}

impl<'a> RetainedBlobReader<'a> {
    fn new(repo_path: &'a Path, limits: BlobReadLimits) -> Self {
        Self {
            repo_path,
            budget: BlobReadBudget::new(limits),
            facts_by_object: BTreeMap::new(),
        }
    }

    fn facts(&mut self, entry: &GitTreeEntry) -> Result<GitBlobFacts, String> {
        if let Some(facts) = self.facts_by_object.get(&entry.object) {
            return Ok(facts.clone());
        }
        let size = git_blob_size(self.repo_path, entry)?;
        self.budget.reserve(entry, size)?;
        let sha256 = hash_git_blob(self.repo_path, entry, size)?;
        let facts = GitBlobFacts { size, sha256 };
        self.facts_by_object
            .insert(entry.object.clone(), facts.clone());
        Ok(facts)
    }
}

#[derive(Debug)]
struct AnchoredRepository {
    facts: RepositoryAnchorFacts,
    project: Project,
    retained_manifest: RetainedObjectManifestV1,
}

fn command(repo: &Path, args: &[&str]) -> Result<Output, String> {
    super::git_read::hardened_command(repo, "frontier repository")?
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = command(repo, args)?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!("git {} failed with {}", args.join(" "), output.status)
    } else {
        stderr
    })
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git(repo, args)?)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("git {} output was not UTF-8: {error}", args.join(" ")))
}

fn read_pinned_worktree_file(
    repo_path: &Path,
    relative: &str,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, String> {
    validate_git_tree_path(relative)?;
    let relative_path = Path::new(relative);
    let components = relative_path.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label} has a non-normalized repository path"));
    }

    let mut directories = Vec::with_capacity(components.len());
    let mut current = repo_path.to_path_buf();
    directories.push(current.clone());
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            unreachable!("closed component validation above")
        };
        current.push(name);
        directories.push(current.clone());
    }
    let mut parent_handles = Vec::with_capacity(directories.len());
    for directory in &directories {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("inspect parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        parent_handles.push(
            same_file::Handle::from_path(directory)
                .map_err(|error| format!("identify parent of {label}: {error}"))?,
        );
    }

    let path = repo_path.join(relative_path);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("{label} is absent from the current checkout"));
        }
        Err(error) => return Err(format!("inspect {label}: {error}")),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must remain a regular non-symlink file"));
    }
    let expected_file = same_file::Handle::from_path(&path)
        .map_err(|error| format!("identify {label}: {error}"))?;
    let mut file = fs::File::open(&path).map_err(|error| format!("open {label}: {error}"))?;
    let opened_file = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone open {label}: {error}"))?,
    )
    .map_err(|error| format!("identify open {label}: {error}"))?;
    if opened_file != expected_file {
        return Err(format!("{label} changed while it was opened"));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label}: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{label} exceeds its anchored size bound"));
    }

    let final_metadata =
        fs::symlink_metadata(&path).map_err(|error| format!("reinspect {label}: {error}"))?;
    let final_file = same_file::Handle::from_path(&path)
        .map_err(|error| format!("reidentify {label}: {error}"))?;
    if final_metadata.file_type().is_symlink()
        || !final_metadata.is_file()
        || final_file != opened_file
    {
        return Err(format!("{label} changed while it was read"));
    }
    for (directory, expected) in directories.iter().zip(&parent_handles) {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("reinspect parent of {label}: {error}"))?;
        let actual = same_file::Handle::from_path(directory)
            .map_err(|error| format!("reidentify parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() || &actual != expected {
            return Err(format!(
                "repository parent of {label} changed while it was read"
            ));
        }
    }
    Ok(bytes)
}

fn require_current_tracked_regular_file(
    repo_path: &Path,
    relative: &str,
    expected_mode: &str,
) -> Result<(), String> {
    let output = git(repo_path, &["ls-files", "--stage", "-z", "--", relative])?;
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    if records.len() != 1 {
        return Err(format!(
            "anchored immutable retained object {relative:?} must remain one exact tracked worktree file"
        ));
    }
    let record = std::str::from_utf8(records[0])
        .map_err(|error| format!("tracked entry for {relative:?} is not UTF-8: {error}"))?;
    let (metadata, indexed_path) = record
        .split_once('\t')
        .ok_or_else(|| format!("malformed tracked entry for {relative:?}"))?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3
        || fields[0] != expected_mode
        || fields[2] != "0"
        || indexed_path != relative
    {
        return Err(format!(
            "anchored immutable retained object {relative:?} is untracked, conflicted, mode-changed, or replaced by a submodule"
        ));
    }
    Ok(())
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn canonical_root<T: Serialize>(value: &T) -> Result<String, String> {
    canonical::to_canonical_bytes(value)
        .map(|bytes| sha256_root(&bytes))
        .map_err(|error| error.to_string())
}

fn validate_lower_hex(field: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{field} must be exactly {length} lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn validate_sha256_root(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{field} must use the sha256:<64 lowercase hex> form"))?;
    validate_lower_hex(field, digest, 64)
}

fn validate_git_tree_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path.nfc().collect::<String>() != path
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(format!(
            "Git tree path {path:?} is not a normalized relative NFC repository path"
        ));
    }
    Ok(())
}

fn validate_git_tree_entries(entries: &[GitTreeEntry]) -> Result<(), String> {
    let mut portable_paths = BTreeMap::<String, &str>::new();
    for entry in entries {
        validate_git_tree_path(&entry.path)?;
        if is_project_input(&entry.path)
            && (!matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob")
        {
            return Err(format!(
                "anchored project input {} must be a tracked regular blob, got mode {} type {}",
                entry.path, entry.mode, entry.kind
            ));
        }
        let portable = entry
            .path
            .nfc()
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if let Some(previous) = portable_paths.insert(portable, &entry.path) {
            return Err(format!(
                "Git tree paths {previous:?} and {:?} have a portable case-fold or Unicode-normalization collision",
                entry.path
            ));
        }
    }
    Ok(())
}

fn repository_object_format(repo: &Path) -> Result<GitObjectFormat, String> {
    match git_text(repo, &["rev-parse", "--show-object-format"])?.as_str() {
        "sha1" => Ok(GitObjectFormat::Sha1),
        "sha256" => Ok(GitObjectFormat::Sha256),
        other => Err(format!("unsupported Git object format {other:?}")),
    }
}

fn tree_entries(repo: &Path, commit: &str) -> Result<Vec<GitTreeEntry>, String> {
    let output = git(repo, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    let mut entries = Vec::new();
    for raw in output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let record = std::str::from_utf8(raw)
            .map_err(|error| format!("Git tree contains a non-UTF-8 path: {error}"))?;
        let (metadata, path) = record
            .split_once('\t')
            .ok_or_else(|| format!("malformed git ls-tree record {record:?}"))?;
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(format!("malformed git ls-tree metadata {metadata:?}"));
        }
        let entry = GitTreeEntry {
            mode: fields[0].to_string(),
            kind: fields[1].to_string(),
            object: fields[2].to_string(),
            path: path.to_string(),
        };
        // Validate the raw Git path before it can reach a join, lookup, blob
        // read, or portable manifest calculation.
        validate_git_tree_path(&entry.path)?;
        entries.push(entry);
    }
    validate_git_tree_entries(&entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn git_blob_size(repo: &Path, entry: &GitTreeEntry) -> Result<u64, String> {
    if entry.kind != "blob" {
        return Err(format!(
            "retained path {} is a Git {}, not a regular blob",
            entry.path, entry.kind
        ));
    }
    git_text(repo, &["cat-file", "-s", &entry.object])?
        .parse::<u64>()
        .map_err(|error| format!("Git blob size for {} is invalid: {error}", entry.path))
}

fn stream_git_blob<F>(
    repo: &Path,
    entry: &GitTreeEntry,
    expected_size: u64,
    mut consume: F,
) -> Result<(), String>
where
    F: FnMut(&[u8]) -> Result<(), String>,
{
    let mut child = super::git_read::hardened_command(repo, "frontier repository")?
        .args(["cat-file", "blob", entry.object.as_str()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("read anchored blob {}: {error}", entry.path))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Git blob stream for {} has no stdout", entry.path))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut observed = 0_u64;
    let stream_result = (|| {
        loop {
            let count = stdout
                .read(&mut buffer)
                .map_err(|error| format!("read anchored blob {}: {error}", entry.path))?;
            if count == 0 {
                break;
            }
            observed = observed
                .checked_add(count as u64)
                .ok_or_else(|| format!("anchored blob {} size overflowed u64", entry.path))?;
            if observed > expected_size {
                return Err(format!(
                    "anchored blob {} emitted more than its declared {expected_size} bytes",
                    entry.path
                ));
            }
            consume(&buffer[..count])?;
        }
        Ok(())
    })();
    drop(stdout);
    if let Err(error) = stream_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for anchored blob {}: {error}", entry.path))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!(
                "git cat-file blob failed for {} with {}",
                entry.path, output.status
            )
        } else {
            stderr
        });
    }
    if observed != expected_size {
        return Err(format!(
            "anchored blob {} emitted {observed} bytes, expected {expected_size}",
            entry.path
        ));
    }
    Ok(())
}

fn hash_git_blob(repo: &Path, entry: &GitTreeEntry, size: u64) -> Result<String, String> {
    let mut digest = Sha256::new();
    stream_git_blob(repo, entry, size, |chunk| {
        digest.update(chunk);
        Ok(())
    })?;
    Ok(hex::encode(digest.finalize()))
}

fn read_git_blob_bounded(
    repo: &Path,
    entry: &GitTreeEntry,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let size = git_blob_size(repo, entry)?;
    if size > max_bytes {
        return Err(format!(
            "Git blob {} is {size} bytes, exceeding the read limit of {max_bytes} bytes",
            entry.path
        ));
    }
    let capacity = usize::try_from(size)
        .map_err(|_| format!("Git blob {} does not fit in memory", entry.path))?;
    let mut bytes = Vec::with_capacity(capacity);
    stream_git_blob(repo, entry, size, |chunk| {
        bytes.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(bytes)
}

fn is_project_input(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        ".vela/findings/",
        ".vela/reviews/",
        ".vela/confidence-updates/",
        ".vela/events/",
        ".vela/proposals/",
        ".vela/artifacts/",
    ];
    const FILES: &[&str] = &[
        "frontier.yaml",
        ".vela/config.toml",
        ".vela/proof-state.json",
        ".vela/actors.json",
        ".vela/signatures.json",
    ];
    PREFIXES.iter().any(|prefix| path.starts_with(prefix)) || FILES.contains(&path)
}

fn is_base_retained_path(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        ".vela/events/",
        ".vela/proposals/",
        ".vela/findings/",
        ".vela/reviews/",
        ".vela/confidence-updates/",
        ".vela/artifacts/",
        ".vela/artifact-blobs/",
        "records/receipts/sha256/",
    ];
    const FILES: &[&str] = &[
        ".vela/actors.json",
        ".vela/signatures.json",
        "review/policy.yaml",
        "proof/policy.yaml",
    ];
    PREFIXES.iter().any(|prefix| path.starts_with(prefix))
        || is_immutable_policy_snapshot(path)
        || FILES.contains(&path)
}

fn is_immutable_policy_snapshot(path: &str) -> bool {
    let Some(file) = path.strip_prefix(".vela/policies/") else {
        return false;
    };
    let policy_id = file
        .strip_suffix(".sig.json")
        .or_else(|| file.strip_suffix(".json"));
    policy_id.is_some_and(|policy_id| {
        policy_id.strip_prefix("vap_").is_some_and(|digest| {
            digest.len() == 32
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    })
}

fn entry_by_path<'a>(entries: &'a [GitTreeEntry], path: &str) -> Result<&'a GitTreeEntry, String> {
    entries
        .binary_search_by(|entry| entry.path.as_str().cmp(path))
        .ok()
        .map(|index| &entries[index])
        .ok_or_else(|| format!("retained path {path:?} is absent from the anchored Git tree"))
}

fn normalized_local_path(value: &str) -> Result<Option<String>, String> {
    if value.starts_with("https://")
        || value.starts_with("http://")
        || value.starts_with("s3://")
        || value.starts_with("urn:")
        || value.starts_with("custodian:")
        || value.starts_with("opaque:")
    {
        return Ok(None);
    }
    if value.trim().is_empty()
        || value != value.trim()
        || value.starts_with('/')
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value.nfc().collect::<String>() != value
    {
        return Err(format!(
            "retained locator {value:?} is not a normalized repository path"
        ));
    }
    let mut segments = value.split('/');
    if segments
        .clone()
        .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(format!(
            "retained locator {value:?} is not a normalized repository path"
        ));
    }
    if segments.next().is_none() {
        return Err(format!("retained locator {value:?} is empty"));
    }
    Ok(Some(value.to_string()))
}

fn receipt_paths(project: &Project) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    for proposal in &project.proposals {
        let Some(submission) = proposal.payload.get("vela_submission") else {
            continue;
        };
        let Some(path) = submission.get("receipt_path").and_then(Value::as_str) else {
            continue;
        };
        let Some(path) = normalized_local_path(path)? else {
            return Err(format!(
                "proposal {} Receipt path must be repository-local",
                proposal.id
            ));
        };
        paths.insert(path);
    }
    Ok(paths)
}

fn derive_retained_manifest(
    repo_path: &Path,
    entries: &[GitTreeEntry],
    project: &Project,
) -> Result<RetainedObjectManifestV1, String> {
    derive_retained_manifest_with_limits(
        repo_path,
        entries,
        project,
        BlobReadLimits {
            max_blob_bytes: MAX_RETAINED_BLOB_BYTES,
            max_total_bytes: MAX_RETAINED_TOTAL_BYTES,
        },
    )
}

fn derive_retained_manifest_with_limits(
    repo_path: &Path,
    entries: &[GitTreeEntry],
    project: &Project,
    limits: BlobReadLimits,
) -> Result<RetainedObjectManifestV1, String> {
    let mut blobs = RetainedBlobReader::new(repo_path, limits);
    let mut retained = entries
        .iter()
        .filter(|entry| is_base_retained_path(&entry.path))
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();

    for path in receipt_paths(project)? {
        retained.insert(path);
    }
    for artifact in &project.artifacts {
        if !matches!(artifact.storage_mode.as_str(), "local_blob" | "local_file") {
            continue;
        }
        let locator = artifact
            .locator
            .as_deref()
            .ok_or_else(|| format!("local artifact {} has no retained locator", artifact.id))?;
        let Some(path) = normalized_local_path(locator)? else {
            return Err(format!(
                "local artifact {} locator is not repository-local",
                artifact.id
            ));
        };
        let entry = entry_by_path(entries, &path)?;
        let facts = blobs.facts(entry)?;
        if format!("sha256:{}", facts.sha256) != artifact.content_hash {
            return Err(format!(
                "artifact {} retained bytes do not match {}",
                artifact.id, artifact.content_hash
            ));
        }
        if artifact
            .size_bytes
            .is_some_and(|expected| expected != facts.size)
        {
            return Err(format!(
                "artifact {} retained byte size disagrees",
                artifact.id
            ));
        }
        retained.insert(path);
    }

    // Every retained Receipt is itself parsed under the closed Receipt v1
    // contract. Its local artifact paths join the same closure; remote and
    // opaque descriptors do not pretend to name Git objects.
    let receipt_files = retained
        .iter()
        .filter(|path| path.starts_with("records/receipts/sha256/"))
        .cloned()
        .collect::<Vec<_>>();
    for receipt_path in receipt_files {
        let receipt_entry = entry_by_path(entries, &receipt_path)?;
        let _facts = blobs.facts(receipt_entry)?;
        let receipt_bytes =
            read_git_blob_bounded(repo_path, receipt_entry, MAX_RECEIPT_BLOB_BYTES)?;
        let receipt = ReceiptV1::parse(&receipt_bytes)
            .map_err(|error| format!("invalid retained Receipt {receipt_path}: {error}"))?;
        let root = receipt
            .canonical_root()
            .map_err(|error| format!("root retained Receipt {receipt_path}: {error}"))?;
        for proposal in &project.proposals {
            let Some(submission) = proposal.payload.get("vela_submission") else {
                continue;
            };
            if submission.get("receipt_path").and_then(Value::as_str) != Some(receipt_path.as_str())
            {
                continue;
            }
            if let Some(expected) = submission.get("receipt_root").and_then(Value::as_str)
                && expected != root
            {
                return Err(format!(
                    "proposal {} Receipt root mismatch: declared {expected}, derived {root}",
                    proposal.id
                ));
            }
        }
        let artifacts = receipt
            .as_value()
            .get("artifacts")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("retained Receipt {receipt_path} has no artifacts array"))?;
        for (index, artifact) in artifacts.iter().enumerate() {
            let Some(path) = artifact.get("path").and_then(Value::as_str) else {
                return Err(format!(
                    "retained Receipt {receipt_path} artifact {index} has no path"
                ));
            };
            let Some(path) = normalized_local_path(path)? else {
                continue;
            };
            let Ok(entry) = entry_by_path(entries, &path) else {
                if artifact.get("uri").and_then(Value::as_str).is_some() {
                    continue;
                }
                return Err(format!(
                    "retained Receipt {receipt_path} artifact {index} path {path:?} is absent"
                ));
            };
            let facts = blobs.facts(entry)?;
            if let Some(expected) = artifact.get("sha256").and_then(Value::as_str)
                && expected != facts.sha256
            {
                return Err(format!(
                    "retained Receipt {receipt_path} artifact {index} digest mismatch"
                ));
            }
            retained.insert(path);
        }
    }

    let mut manifest = Vec::with_capacity(retained.len());
    for path in retained {
        let entry = entry_by_path(entries, &path)?;
        if !matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob" {
            return Err(format!(
                "retained path {} must be a tracked regular file, got mode {} type {}",
                entry.path, entry.mode, entry.kind
            ));
        }
        let facts = blobs.facts(entry)?;
        manifest.push(RetainedObjectEntryV1 {
            path,
            git_mode: entry.mode.clone(),
            size: facts.size,
            sha256: facts.sha256,
        });
    }
    let manifest = RetainedObjectManifestV1(manifest);
    manifest.validate()?;
    Ok(manifest)
}

fn materialize_project_inputs(
    repo_path: &Path,
    entries: &[GitTreeEntry],
) -> Result<(tempfile::TempDir, Project), String> {
    // Keep this defensive check even though production callers receive the
    // result of `tree_entries`: it guarantees no future caller can construct a
    // raw entry and reach `Path::join` first.
    validate_git_tree_entries(entries)?;
    let temporary =
        tempfile::tempdir().map_err(|error| format!("create anchored repository view: {error}"))?;
    let mut budget = BlobReadBudget::new(BlobReadLimits {
        max_blob_bytes: MAX_PROJECT_INPUT_BLOB_BYTES,
        max_total_bytes: MAX_PROJECT_INPUT_TOTAL_BYTES,
    });
    for entry in entries.iter().filter(|entry| is_project_input(&entry.path)) {
        if !matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob" {
            return Err(format!(
                "anchored project input {} must be a tracked regular file",
                entry.path
            ));
        }
        let target = temporary.path().join(&entry.path);
        let parent = target
            .parent()
            .ok_or_else(|| format!("anchored path {} has no parent", entry.path))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("create anchored path {}: {error}", parent.display()))?;
        let size = git_blob_size(repo_path, entry)?;
        budget.reserve(entry, size)?;
        fs::write(
            &target,
            read_git_blob_bounded(repo_path, entry, MAX_PROJECT_INPUT_BLOB_BYTES)?,
        )
        .map_err(|error| format!("write anchored path {}: {error}", target.display()))?;
    }
    if !temporary.path().join(".vela").is_dir() {
        return Err("anchored Git tree has no .vela repository".to_string());
    }
    let project = repo::load_from_path(temporary.path())
        .map_err(|error| format!("load anchored Vela repository: {error}"))?;
    Ok((temporary, project))
}

fn anchored_repository(repo_path: &Path, commit: &str) -> Result<AnchoredRepository, String> {
    let format = repository_object_format(repo_path)?;
    let resolved_commit = git_text(repo_path, &["rev-parse", &format!("{commit}^{{commit}}")])
        .map_err(|error| format!("anchor commit unavailable: {error}"))?;
    if resolved_commit != commit {
        return Err("anchor commit did not resolve to the exact signed object".to_string());
    }
    let git_tree = git_text(repo_path, &["rev-parse", &format!("{commit}^{{tree}}")])?;
    let entries = tree_entries(repo_path, commit)?;
    let (_temporary, project) = materialize_project_inputs(repo_path, &entries)?;
    let retained_manifest = derive_retained_manifest(repo_path, &entries, &project)?;

    let actors_entry = entries
        .iter()
        .find(|entry| entry.path == ".vela/actors.json");
    let actor_registry_root = if let Some(entry) = actors_entry {
        sha256_root(&read_git_blob_bounded(
            repo_path,
            entry,
            MAX_ACTOR_REGISTRY_BLOB_BYTES,
        )?)
    } else {
        canonical_root(&Vec::<vela_protocol::sign::ActorRecord>::new())?
    };
    let facts = RepositoryAnchorFacts {
        git_object_format: format,
        git_commit: resolved_commit,
        git_tree,
        event_log_root: format!("sha256:{}", events::event_log_hash(&project.events)),
        event_count: project.events.len() as u64,
        snapshot_root: format!("sha256:{}", events::snapshot_hash(&project)),
        snapshot_schema: project.schema.clone(),
        proposal_root: format!(
            "sha256:{}",
            proposals::proposal_state_hash(&project.proposals)
        ),
        artifact_registry_root: canonical_root(&project.artifacts)?,
        actor_registry_root,
        canonical_store_root: retained_manifest.root()?,
        retained_object_count: retained_manifest.0.len(),
    };
    Ok(AnchoredRepository {
        facts,
        project,
        retained_manifest,
    })
}

/// Derive all repository-context facts for an exact immutable Git commit.
///
/// This is useful to a migration preview, but it grants no authority by
/// itself. Authority arrives only after a signed boundary has bound these
/// values and [`verify_repository_boundary_context`] has rederived them.
pub fn derive_repository_anchor_facts(
    repo_path: &Path,
    commit: &str,
) -> Result<RepositoryAnchorFacts, String> {
    anchored_repository(repo_path, commit).map(|anchor| anchor.facts)
}

fn compare_anchor(
    payload: &FrontierRepositoryBoundaryPayloadV1,
    actual: &RepositoryAnchorFacts,
) -> Result<(), String> {
    let checks = [
        (
            "anchor_git_commit",
            payload.anchor_git_commit.as_str(),
            actual.git_commit.as_str(),
        ),
        (
            "anchor_git_tree",
            payload.anchor_git_tree.as_str(),
            actual.git_tree.as_str(),
        ),
        (
            "anchor_event_log_root",
            payload.anchor_event_log_root.as_str(),
            actual.event_log_root.as_str(),
        ),
        (
            "anchor_snapshot_root",
            payload.anchor_snapshot_root.as_str(),
            actual.snapshot_root.as_str(),
        ),
        (
            "anchor_snapshot_schema",
            payload.anchor_snapshot_schema.as_str(),
            actual.snapshot_schema.as_str(),
        ),
        (
            "anchor_proposal_root",
            payload.anchor_proposal_root.as_str(),
            actual.proposal_root.as_str(),
        ),
        (
            "anchor_actor_registry_root",
            payload.anchor_actor_registry_root.as_str(),
            actual.actor_registry_root.as_str(),
        ),
        (
            "anchor_artifact_registry_root",
            payload.anchor_artifact_registry_root.as_str(),
            actual.artifact_registry_root.as_str(),
        ),
        (
            "anchor_canonical_store_root",
            payload.anchor_canonical_store_root.as_str(),
            actual.canonical_store_root.as_str(),
        ),
    ];
    for (field, expected, observed) in checks {
        if expected != observed {
            return Err(format!(
                "{field} mismatch: boundary declares {expected}, Git anchor derives {observed}"
            ));
        }
    }
    if payload.git_object_format != actual.git_object_format {
        return Err("git_object_format does not match the repository".to_string());
    }
    if payload.anchor_event_count != actual.event_count {
        return Err(format!(
            "anchor_event_count mismatch: boundary declares {}, Git anchor derives {}",
            payload.anchor_event_count, actual.event_count
        ));
    }
    Ok(())
}

fn verify_ancestry(repo_path: &Path, anchor: &str) -> Result<(), String> {
    let head = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])
        .map_err(|error| format!("checked revision unavailable: {error}"))?;
    let output = command(repo_path, &["merge-base", "--is-ancestor", anchor, &head])?;
    if output.status.success() {
        Ok(())
    } else {
        Err("anchor commit is not an ancestor of the checked revision".to_string())
    }
}

fn event_content_root(event: &StateEvent) -> String {
    sha256_root(&events::event_content_preimage_bytes(event))
}

/// Prove that every exact anchored event preimage remains a member of the
/// current canonical event set.
///
/// Event-log commitments are ID-sorted sets, not append-order vectors. A new
/// event may sort lexically before an anchored member without changing the
/// anchored membership claim. Conversely, matching only the short `vev_`
/// handle would permit a different preimage to stand in for anchored history.
fn verify_event_membership(current: &Project, anchored: &Project) -> Result<(), String> {
    if current.events.len() < anchored.events.len() {
        return Err(format!(
            "current event log has {} events but the anchored event set has {}",
            current.events.len(),
            anchored.events.len()
        ));
    }

    let mut current_by_root = BTreeMap::<String, &StateEvent>::new();
    let mut current_id_roots = BTreeMap::<&str, BTreeSet<String>>::new();
    for event in &current.events {
        let root = event_content_root(event);
        if current_by_root.insert(root.clone(), event).is_some() {
            return Err(format!(
                "current event set contains duplicate canonical event root {root}"
            ));
        }
        current_id_roots
            .entry(event.id.as_str())
            .or_default()
            .insert(root);
    }
    if let Some((id, roots)) = current_id_roots.iter().find(|(_, roots)| roots.len() != 1) {
        return Err(format!(
            "current event handle {id} identifies {} distinct canonical preimages",
            roots.len()
        ));
    }

    let mut anchored_roots = BTreeSet::new();
    for anchored_event in &anchored.events {
        let root = event_content_root(anchored_event);
        if !anchored_roots.insert(root.clone()) {
            return Err(format!(
                "anchored event set contains duplicate canonical event root {root}"
            ));
        }
        let Some(current_event) = current_by_root.get(&root) else {
            if current_id_roots.contains_key(anchored_event.id.as_str()) {
                return Err(format!(
                    "anchored event {} was replaced by a different canonical preimage under the same display handle",
                    anchored_event.id
                ));
            }
            return Err(format!(
                "anchored canonical event {} ({root}) is absent from the current event set",
                anchored_event.id
            ));
        };
        if current_event.id != anchored_event.id {
            return Err(format!(
                "anchored canonical event {} changed its content-addressed event ID to {}",
                anchored_event.id, current_event.id
            ));
        }
        if events::event_content_preimage_bytes(anchored_event)
            != events::event_content_preimage_bytes(current_event)
        {
            return Err(format!(
                "anchored canonical event {} changed preimage bytes",
                anchored_event.id
            ));
        }
        if anchored_event.signature.is_some() || current_event.signature.is_some() {
            let expected_key = if anchored_event.kind.as_str()
                == vela_protocol::events::EVENT_KIND_FRONTIER_REPOSITORY_BOUND
            {
                repository_boundary_payload_from_event_shape(anchored_event)?
                    .administrator_public_key
            } else {
                anchored
                    .actors
                    .iter()
                    .find(|actor| actor.id == anchored_event.actor.id)
                    .map(|actor| actor.public_key.clone())
                    .ok_or_else(|| {
                        format!(
                            "anchored event {} carries or acquired a signature but its actor {} has no anchored public key",
                            anchored_event.id, anchored_event.actor.id
                        )
                    })?
            };
            if anchored_event.signature.is_some() && current_event.signature.is_none() {
                return Err(format!(
                    "anchored event {} lost its historical signature",
                    anchored_event.id
                ));
            }
            if !vela_protocol::sign::verify_event_signature(current_event, &expected_key)? {
                return Err(format!(
                    "anchored event {} current signature does not verify",
                    anchored_event.id
                ));
            }
        }
    }
    Ok(())
}

/// Prove that every retained canonical object in an older exact state remains
/// byte-identical at a later temporalization anchor.
fn verify_retained_manifest_membership(
    later: &RetainedObjectManifestV1,
    historical: &RetainedObjectManifestV1,
) -> Result<(), String> {
    later.validate()?;
    historical.validate()?;
    let later_by_path = later
        .0
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    for expected in &historical.0 {
        let Some(observed) = later_by_path.get(expected.path.as_str()) else {
            return Err(format!(
                "historical retained object {:?} is absent from the temporalization anchor",
                expected.path
            ));
        };
        if *observed != expected {
            return Err(format!(
                "historical retained object {:?} changed path, mode, size, or digest before the temporalization anchor",
                expected.path
            ));
        }
    }
    Ok(())
}

/// Derive the exact legacy v0.1 fallback-identity preimage root.
///
/// Migration planners must call this function instead of trusting a supplied
/// digest. Repository-context verification calls the same implementation when
/// it checks the immutable Git anchor, so a preview and a later verifier
/// cannot silently disagree about the legacy identity being temporalized.
pub fn derive_legacy_identity_preimage_root(project: &Project) -> Result<String, String> {
    // This is exactly the v0.1 fallback identity preimage used by
    // `derive_frontier_id_from_meta`: do not trust a digest supplied by the
    // boundary that is meant to authenticate these anchored bytes.
    canonical_root(&serde_json::json!({
        "name": project.project.name,
        "compiled_at": project.project.compiled_at,
        "compiler": project.project.compiler,
    }))
}

struct BoundaryChainEntry<'a> {
    root: String,
    event: &'a StateEvent,
    payload: FrontierRepositoryBoundaryPayloadV1,
}

struct BoundaryChain<'a> {
    selected_root: String,
    /// Every repository boundary on the selected identity chain, ordered from
    /// the temporal/genesis-rooted boundary through the selected leaf.
    ///
    /// Keeping the whole chain is security-relevant: validating only the root
    /// and leaf would let a correctly signed intermediate dependency update
    /// carry a fabricated Git/Vela anchor and then disappear behind a later,
    /// otherwise valid boundary.
    entries: Vec<BoundaryChainEntry<'a>>,
}

fn select_unique_boundary_chain<'a>(
    events: &'a [StateEvent],
    selected: &'a StateEvent,
) -> Result<BoundaryChain<'a>, String> {
    let selected_root = repository_identity_event_content_root(selected)?;
    let mut boundaries =
        BTreeMap::<String, (&StateEvent, FrontierRepositoryBoundaryPayloadV1)>::new();
    let mut referenced_parents = BTreeSet::new();
    for event in events
        .iter()
        .filter(|event| event.kind.as_str() == events::EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
    {
        let root = repository_identity_event_content_root(event)?;
        let payload = repository_boundary_payload_from_event_shape(event)?;
        if let Some(parent) = payload.previous_identity_event_root.as_ref() {
            referenced_parents.insert(parent.clone());
        }
        if boundaries.insert(root.clone(), (event, payload)).is_some() {
            return Err(format!(
                "repository identity event set contains duplicate full content root {root}"
            ));
        }
    }

    let matching_selected = boundaries.contains_key(&selected_root);
    if !matching_selected {
        return Err(format!(
            "selected repository boundary full content root {selected_root} is absent from the current event set"
        ));
    }
    let leaves = boundaries
        .keys()
        .filter(|root| !referenced_parents.contains(*root))
        .cloned()
        .collect::<Vec<_>>();
    let [leaf] = leaves.as_slice() else {
        return Err(format!(
            "repository identity event set must have exactly one valid boundary leaf, found {}",
            leaves.len()
        ));
    };
    if leaf != &selected_root {
        return Err(format!(
            "selected repository boundary {selected_root} is not the unique valid chain leaf {leaf}"
        ));
    }

    let mut cursor = selected_root.as_str();
    let mut entries = Vec::new();
    loop {
        let (event, payload) = boundaries
            .get(cursor)
            .ok_or_else(|| format!("repository boundary chain is missing {cursor}"))?;
        entries.push(BoundaryChainEntry {
            root: cursor.to_string(),
            event,
            payload: payload.clone(),
        });
        if payload.mode == vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::TemporalizeExisting {
            entries.reverse();
            return Ok(BoundaryChain { selected_root, entries });
        }
        let parent = payload
            .previous_identity_event_root
            .as_deref()
            .ok_or_else(|| format!("repository boundary {cursor} has no identity parent"))?;
        if boundaries.contains_key(parent) {
            cursor = parent;
            continue;
        }
        // The parent can be the unsigned structural genesis rather than
        // another boundary. Retain this first administrator boundary as the
        // chain root: structural continuity does not authenticate which
        // administrator fork a consumer intended to trust.
        entries.reverse();
        return Ok(BoundaryChain {
            selected_root,
            entries,
        });
    }
}

fn verify_repository_trust(
    chain: &BoundaryChain<'_>,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<(), String> {
    use vela_protocol::frontier_repository::{
        FrontierRepositoryBoundaryMode, FrontierRepositoryTrustMode,
    };

    let root = chain
        .entries
        .first()
        .ok_or_else(|| "repository boundary chain is empty".to_string())?;
    if root.payload.mode == FrontierRepositoryBoundaryMode::TemporalizeExisting
        && root.payload.trust_mode != FrontierRepositoryTrustMode::Tofu
    {
        return Err("legacy repository identity root is not marked as TOFU".to_string());
    }
    let anchor = trust_anchor.ok_or_else(|| {
        "repository administrator boundary requires an explicit out-of-band RepositoryTrustAnchor"
            .to_string()
    })?;
    anchor.validate()?;
    if anchor.boundary_content_root != root.root {
        return Err(format!(
            "repository trust anchor boundary root mismatch: expected {}, observed {}",
            anchor.boundary_content_root, root.root
        ));
    }
    if anchor.administrator_public_key != root.payload.administrator_public_key {
        return Err("repository trust anchor administrator public key mismatch".to_string());
    }
    Ok(())
}

fn verify_anchor_descends_from(
    repo_path: &Path,
    parent_commit: &str,
    child_commit: &str,
) -> Result<(), String> {
    let output = command(
        repo_path,
        &["merge-base", "--is-ancestor", parent_commit, child_commit],
    )?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "repository boundary anchor {child_commit} is not a descendant of preceding anchor {parent_commit}"
        ))
    }
}

fn verify_immutable_retained_objects_in_worktree(
    repo_path: &Path,
    manifest: &RetainedObjectManifestV1,
) -> Result<(), String> {
    manifest.validate()?;
    for entry in manifest.0.iter().filter(|entry| {
        entry.path.starts_with("records/receipts/sha256/")
            || entry.path.starts_with(".vela/artifact-blobs/")
            || is_immutable_policy_snapshot(&entry.path)
            || (!entry.path.starts_with(".vela/")
                && !matches!(
                    entry.path.as_str(),
                    "review/policy.yaml" | "proof/policy.yaml"
                ))
    }) {
        require_current_tracked_regular_file(repo_path, &entry.path, &entry.git_mode)?;
        let path = repo_path.join(&entry.path);
        let _metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "anchored immutable retained object {:?} is absent from the current checkout",
                    entry.path
                ));
            }
            Err(error) => {
                return Err(format!("inspect retained object {:?}: {error}", entry.path));
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let current_mode = if _metadata.permissions().mode() & 0o111 == 0 {
                "100644"
            } else {
                "100755"
            };
            if current_mode != entry.git_mode {
                return Err(format!(
                    "anchored immutable retained object {:?} changed Git file mode from {} to {current_mode}",
                    entry.path, entry.git_mode
                ));
            }
        }
        let bytes = read_pinned_worktree_file(
            repo_path,
            &entry.path,
            entry.size,
            &format!("anchored immutable retained object {:?}", entry.path),
        )?;
        if bytes.len() as u64 != entry.size || hex::encode(Sha256::digest(&bytes)) != entry.sha256 {
            return Err(format!(
                "anchored immutable retained object {:?} changed byte content in the current checkout",
                entry.path
            ));
        }
    }
    Ok(())
}

fn immutable_proposal_root(
    proposal: &vela_protocol::proposals::StateProposal,
) -> Result<String, String> {
    canonical_root(&serde_json::json!({
        "schema": proposal.schema,
        "id": proposal.id,
        "kind": proposal.kind,
        "target": proposal.target,
        "actor": proposal.actor,
        "created_at": proposal.created_at,
        "drafted_at": proposal.drafted_at,
        "reason": proposal.reason,
        "payload": proposal.payload,
        "source_refs": proposal.source_refs,
        "caveats": proposal.caveats,
        "agent_run": proposal.agent_run,
    }))
}

/// Preserve every proposal that was present at a signed repository anchor.
///
/// Proposal decision fields are intentionally absent from the immutable
/// projection: `status`, reviewer metadata, the decision reason, and the
/// applied event id are caches derived from signed `review.*` or withdrawal
/// events. The existing parity reducer is therefore the only allowed path for
/// those fields to change. All proposal identity and producer provenance,
/// including timestamps and agent-run traces that are deliberately excluded
/// from the retry-stable proposal id, remain byte-semantically immutable.
fn verify_anchored_proposal_history(frontier: &Project, anchored: &Project) -> Result<(), String> {
    let parity_conflicts = proposals::verify_proposal_decision_parity(frontier);
    if !parity_conflicts.is_empty() {
        return Err(format!(
            "current proposal standing is not an event-backed projection: {}",
            parity_conflicts.join(" | ")
        ));
    }

    let current = frontier
        .proposals
        .iter()
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect::<BTreeMap<_, _>>();
    for proposal in &anchored.proposals {
        let Some(observed) = current.get(proposal.id.as_str()) else {
            return Err(format!(
                "anchored proposal {} is absent from the current proposal store",
                proposal.id
            ));
        };
        let expected_root = immutable_proposal_root(proposal)?;
        let observed_root = immutable_proposal_root(observed)?;
        if observed_root != expected_root {
            return Err(format!(
                "anchored proposal {} changed immutable identity or producer provenance: expected {expected_root}, observed {observed_root}",
                proposal.id
            ));
        }
    }
    Ok(())
}

fn current_actor_registry_root(repo_path: &Path, frontier: &Project) -> Result<String, String> {
    let path = repo_path.join(".vela/actors.json");
    if fs::symlink_metadata(&path).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        if frontier.actors.is_empty() {
            return canonical_root(&Vec::<vela_protocol::sign::ActorRecord>::new());
        }
        return Err(
            "current actor registry is absent while the loaded Frontier has actors".to_string(),
        );
    }
    let bytes = read_pinned_worktree_file(
        repo_path,
        ".vela/actors.json",
        16 * 1024 * 1024,
        "current actor registry",
    )?;
    let records: Vec<vela_protocol::sign::ActorRecord> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode current actor registry: {error}"))?;
    if records != frontier.actors {
        return Err(
            "current actor registry bytes do not decode to the loaded actor registry".to_string(),
        );
    }
    Ok(sha256_root(&bytes))
}

fn verify_boundary_anchor_context(
    frontier: &Project,
    repo_path: &Path,
    entry: &BoundaryChainEntry<'_>,
) -> Result<AnchoredRepository, String> {
    if frontier.frontier_id() != entry.payload.frontier_id {
        return Err(format!(
            "boundary frontier {} does not match current {}",
            entry.payload.frontier_id,
            frontier.frontier_id()
        ));
    }
    verify_repository_boundary_signature_only(
        entry.event,
        &entry.payload.administrator_public_key,
    )?;
    verify_ancestry(repo_path, &entry.payload.anchor_git_commit)?;
    let anchored = anchored_repository(repo_path, &entry.payload.anchor_git_commit)?;
    compare_anchor(&entry.payload, &anchored.facts)?;
    verify_event_membership(frontier, &anchored.project)?;
    verify_anchored_proposal_history(frontier, &anchored.project)?;
    verify_identity_parent_membership(&entry.payload, &anchored.project)?;
    verify_active_administrator(&entry.payload, &anchored.project)?;
    anchored
        .retained_manifest
        .verify_root(&entry.payload.anchor_canonical_store_root)?;
    verify_immutable_retained_objects_in_worktree(repo_path, &anchored.retained_manifest)?;
    Ok(anchored)
}

fn verify_identity_parent_membership(
    payload: &FrontierRepositoryBoundaryPayloadV1,
    anchored: &Project,
) -> Result<(), String> {
    let Some(parent) = payload.previous_identity_event_root.as_deref() else {
        return Ok(());
    };
    let found = anchored.events.iter().any(|event| {
        repository_identity_event_content_root(event).is_ok_and(|root| root == parent)
    });
    if !found {
        return Err(format!(
            "previous identity event {parent} is absent from the anchored event set"
        ));
    }
    Ok(())
}

fn verify_active_administrator(
    payload: &FrontierRepositoryBoundaryPayloadV1,
    anchored: &Project,
) -> Result<(), String> {
    if payload.mode
        == vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::UpdateDependencies
        && payload.trust_mode
            == vela_protocol::frontier_repository::FrontierRepositoryTrustMode::Genesis
        && anchored.actors.len() != 1
    {
        return Err(format!(
            "first native administrator boundary requires the exact one-actor bootstrap registry, found {} records",
            anchored.actors.len()
        ));
    }
    let matches = anchored
        .actors
        .iter()
        .filter(|actor| actor.id == payload.administrator_actor_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "anchored actor registry must contain exactly one administrator record for {}, found {}",
            payload.administrator_actor_id,
            matches.len()
        ));
    }
    let actor = matches[0];
    if actor.public_key != payload.administrator_public_key
        || actor.algorithm != payload.administrator_algorithm
    {
        return Err("anchored administrator record does not match the boundary key".to_string());
    }
    // Repository administration is evaluated at an exact causal anchor, not
    // at the boundary's freely chosen timestamp. A revocation already present
    // at that anchor cannot be bypassed by backdating the boundary event.
    if actor.revoked_at.is_some()
        || anchored.events.iter().any(|event| {
            event.kind.as_str() == EVENT_KIND_KEY_REVOKE
                && (event.actor.id == payload.administrator_actor_id
                    || event.target.id == payload.administrator_actor_id)
                && event.payload.get("revoked_pubkey").and_then(Value::as_str)
                    == Some(payload.administrator_public_key.as_str())
        })
    {
        return Err(
            "anchored administrator key is revoked in the anchored causal state".to_string(),
        );
    }
    Ok(())
}

/// Verify one repository boundary against the exact repository and complete
/// current event set.
///
/// Success proves the signed boundary's Git/Vela anchor and administrator
/// facts. It does not elevate TOFU to external trust and does not make any
/// scientific claim or accepted-state decision.
pub fn verify_repository_boundary_context(
    frontier: &Project,
    repo_path: &Path,
    boundary: &StateEvent,
) -> Result<RepositoryBoundaryContext, String> {
    verify_repository_boundary_context_with_trust_anchor(frontier, repo_path, boundary, None)
}

/// Verify one repository boundary with an optional independently supplied
/// trust anchor.
///
/// `trust_anchor` is mandatory for every administrator-boundary chain. A
/// `frontier.created` genesis proves structural identity continuity, but it
/// cannot select between two independently signed administrator forks. The
/// three-argument compatibility entry point above therefore fails closed for
/// every repository containing a boundary.
pub fn verify_repository_boundary_context_with_trust_anchor(
    frontier: &Project,
    repo_path: &Path,
    boundary: &StateEvent,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<RepositoryBoundaryContext, String> {
    let event_set_errors = validate_repository_boundary_event_set(&frontier.events);
    if !event_set_errors.is_empty() {
        return Err(format!(
            "repository identity event set is invalid: {}",
            event_set_errors.join(" | ")
        ));
    }
    let chain = select_unique_boundary_chain(&frontier.events, boundary)?;
    verify_repository_trust(&chain, trust_anchor)?;
    let mut previous: Option<(&BoundaryChainEntry<'_>, AnchoredRepository)> = None;
    let mut selected_anchor = None;
    for entry in &chain.entries {
        let anchored =
            verify_boundary_anchor_context(frontier, repo_path, entry).map_err(|error| {
                format!(
                    "repository boundary {} context invalid: {error}",
                    entry.root
                )
            })?;

        if let Some((parent_entry, parent_anchor)) = previous.as_ref() {
            verify_anchor_descends_from(
                repo_path,
                &parent_entry.payload.anchor_git_commit,
                &entry.payload.anchor_git_commit,
            )?;
            if anchored.facts.actor_registry_root != parent_anchor.facts.actor_registry_root {
                return Err(format!(
                    "repository boundary anchor prefix {} -> {} changed the administrator actor registry without a registry-governance primitive",
                    parent_entry.root, entry.root
                ));
            }
            verify_event_membership(&anchored.project, &parent_anchor.project).map_err(
                |error| {
                    format!(
                        "repository boundary anchor prefix {} -> {} is not continuous: {error}",
                        parent_entry.root, entry.root
                    )
                },
            )?;
        }

        if entry.payload.mode
            == vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::TemporalizeExisting
        {
            let derived_legacy_root =
                derive_legacy_identity_preimage_root(&anchored.project)?;
            if entry.payload.legacy_identity_preimage_root.as_deref()
                != Some(derived_legacy_root.as_str())
            {
                return Err(format!(
                    "legacy_identity_preimage_root mismatch: boundary declares {}, anchored v0.1 identity inputs derive {derived_legacy_root}",
                    entry
                        .payload
                        .legacy_identity_preimage_root
                        .as_deref()
                        .unwrap_or("null")
                ));
            }
        }

        if entry.root == chain.selected_root {
            selected_anchor = Some(anchored.facts.clone());
        }
        previous = Some((entry, anchored));
    }

    let selected_anchor = selected_anchor.ok_or_else(|| {
        format!(
            "selected repository boundary {} has no verified anchor context",
            chain.selected_root
        )
    })?;
    let current_actor_root = current_actor_registry_root(repo_path, frontier)?;
    if current_actor_root != selected_anchor.actor_registry_root {
        return Err(format!(
            "current actor registry root {current_actor_root} differs from the administrator boundary root {}; ADR 0016 defines no registry-governance primitive",
            selected_anchor.actor_registry_root
        ));
    }
    let payload = repository_boundary_payload_from_event_shape(boundary)?;
    Ok(RepositoryBoundaryContext {
        event_id: boundary.id.clone(),
        frontier_id: payload.frontier_id,
        anchor: selected_anchor,
    })
}

/// Verify the current artifact registry from the exact Git-anchored legacy
/// registry plus every post-anchor artifact mutation.
///
/// `reducer::verify_replay` historically compares findings only. A Profile v1
/// write gate must not therefore treat a hand-edited `.vela/artifacts` cache
/// as replay-verified scientific state. Legacy artifacts that predate event
/// sourcing remain valid because the signed repository boundary commits their
/// exact anchored registry; later mutations must reduce from that baseline.
pub(crate) fn verify_repository_artifact_projection(
    frontier: &Project,
    repo_path: &Path,
    boundary: &StateEvent,
) -> Result<(), String> {
    let payload = repository_boundary_payload_from_event_shape(boundary)?;
    let anchored = anchored_repository(repo_path, &payload.anchor_git_commit)?;
    compare_anchor(&payload, &anchored.facts)?;
    verify_event_membership(frontier, &anchored.project)?;

    let anchored_event_roots = anchored
        .project
        .events
        .iter()
        .map(event_content_root)
        .collect::<BTreeSet<_>>();
    let mut replayed = anchored.project;
    for event in vela_protocol::reducer::sorted_for_replay(&frontier.events) {
        if anchored_event_roots.contains(&event_content_root(&event)) {
            continue;
        }
        let affects_artifacts = matches!(
            event.kind.as_str(),
            "artifact.asserted" | "artifact.reviewed" | "artifact.retracted"
        ) || (event.kind.as_str() == "tier.set"
            && event.payload.get("object_type").and_then(Value::as_str) == Some("artifact"));
        if affects_artifacts {
            vela_protocol::reducer::apply_event(&mut replayed, &event)?;
        }
    }
    let expected = canonical_root(&replayed.artifacts)?;
    let observed = canonical_root(&frontier.artifacts)?;
    if observed != expected {
        return Err(format!(
            "artifact registry is not reproducible from the anchored registry and post-anchor artifact events: expected {expected}, observed {observed}"
        ));
    }
    Ok(())
}

/// Verify the current finding projection from the exact Git-anchored legacy
/// findings plus every post-anchor reducer event.
///
/// `reducer::verify_replay` must seed pre-event findings from the materialized
/// cache for old repositories. That compatibility behavior cannot itself prove
/// that a legacy remnant was not edited after the signed temporal boundary:
/// seeding an edited current remnant would simply reproduce the edit. Profile
/// v1 instead takes the remnant baseline from the immutable boundary anchor,
/// hydrates any proposal-backed post-anchor findings, and applies only the
/// post-anchor events before comparing the scientific finding projection.
pub(crate) fn verify_repository_finding_projection(
    frontier: &Project,
    repo_path: &Path,
    initial_boundary: &StateEvent,
) -> Result<(), String> {
    let payload = repository_boundary_payload_from_event_shape(initial_boundary)?;
    if payload.mode
        != vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::TemporalizeExisting
    {
        return Err(
            "anchored legacy finding projection requires the initial temporal boundary".to_string(),
        );
    }
    let anchored = anchored_repository(repo_path, &payload.anchor_git_commit)?;
    compare_anchor(&payload, &anchored.facts)?;
    verify_event_membership(frontier, &anchored.project)?;

    let anchored_event_roots = anchored
        .project
        .events
        .iter()
        .map(event_content_root)
        .collect::<BTreeSet<_>>();
    let post_anchor_events = vela_protocol::reducer::sorted_for_replay(&frontier.events)
        .into_iter()
        .filter(|event| !anchored_event_roots.contains(&event_content_root(event)))
        .collect::<Vec<_>>();
    let (hydrated_findings, hydration_diagnostics) =
        vela_protocol::reducer::seed_genesis(&post_anchor_events, &frontier.proposals);
    if !hydration_diagnostics.is_empty() {
        return Err(format!(
            "post-anchor finding hydration failed: {}",
            hydration_diagnostics.join(" | ")
        ));
    }

    let mut replayed = anchored.project;
    for hydrated in hydrated_findings {
        if let Some(existing) = replayed
            .findings
            .iter()
            .find(|finding| finding.id == hydrated.id)
        {
            if events::finding_hash(existing) != events::finding_hash(&hydrated) {
                return Err(format!(
                    "post-anchor finding {} conflicts with its anchored legacy remnant",
                    hydrated.id
                ));
            }
            continue;
        }
        replayed.findings.push(hydrated);
    }
    for event in post_anchor_events {
        vela_protocol::reducer::apply_event(&mut replayed, &event)?;
    }

    let expected = scientific_finding_projection_root(&replayed.findings)?;
    let observed = scientific_finding_projection_root(&frontier.findings)?;
    if observed != expected {
        return Err(format!(
            "finding projection is not reproducible from the anchored legacy findings and post-anchor events: expected {expected}, observed {observed}"
        ));
    }
    Ok(())
}

fn scientific_finding_projection_root(
    findings: &[vela_protocol::bundle::FindingBundle],
) -> Result<String, String> {
    let mut projected = findings
        .iter()
        .map(|finding| (finding.id.clone(), events::finding_hash(finding)))
        .collect::<Vec<_>>();
    projected.sort();
    canonical_root(&projected)
}

/// Verify scientific sidecars that do not yet have reducer reconstruction.
///
/// A legacy migration may retain their exact pre-boundary bytes because the
/// initial temporal boundary signs their Git/Vela anchor. No later unsigned
/// insert, edit, or deletion may enter Profile v1 scientific state. Native
/// genesis repositories are handled by the write gate and require these
/// unreplayed collections to remain empty.
pub(crate) fn verify_repository_unreplayed_sidecars(
    frontier: &Project,
    repo_path: &Path,
    initial_boundary: &StateEvent,
) -> Result<(), String> {
    let payload = repository_boundary_payload_from_event_shape(initial_boundary)?;
    if payload.mode
        != vela_protocol::frontier_repository::FrontierRepositoryBoundaryMode::TemporalizeExisting
    {
        return Err("unreplayed legacy sidecars require the initial temporal boundary".to_string());
    }
    let anchored = anchored_repository(repo_path, &payload.anchor_git_commit)?;
    compare_anchor(&payload, &anchored.facts)?;
    verify_event_membership(frontier, &anchored.project)?;
    for (name, expected, observed) in [
        (
            "review_events",
            canonical_root(&anchored.project.review_events)?,
            canonical_root(&frontier.review_events)?,
        ),
        (
            "confidence_updates",
            canonical_root(&anchored.project.confidence_updates)?,
            canonical_root(&frontier.confidence_updates)?,
        ),
    ] {
        if observed != expected {
            return Err(format!(
                "{name} is not reducer-reproducible and changed after the initial legacy boundary: expected {expected}, observed {observed}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::repository_write::{
        REPOSITORY_TRUST_ANCHOR_SCHEMA_V1, RepositoryTrustAnchorV1, RepositoryWriteGateCode,
        VerifiedRepositoryIdentity, verify_repository_for_write,
    };
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use vela_protocol::bundle::{ConfidenceUpdate, ReviewAction, ReviewEvent};
    use vela_protocol::events::{EVENT_SCHEMA, NULL_HASH, StateActor, StateTarget};
    use vela_protocol::frontier_profile::{
        FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileLicenseV1, FrontierProfileScopeV1,
        FrontierProfileV1,
    };
    use vela_protocol::frontier_repository::{
        FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryMode,
        FrontierRepositoryTrustMode, LEGACY_FRONTIER_ORIGIN_SCHEMA, LegacyFrontierOriginV1,
        exact_dependency_root, new_repository_boundary_event,
    };
    use vela_protocol::frontier_settings::{FRONTIER_SETTINGS_SCHEMA, FrontierSettingsV1};
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};
    use vela_protocol::test_support::make_finding;

    struct Fixture {
        directory: tempfile::TempDir,
        anchor: RepositoryAnchorFacts,
        project: Project,
        key: SigningKey,
        boundary: StateEvent,
    }

    fn run(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
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

    fn resign(event: &mut StateEvent, key: &SigningKey) {
        event.id = events::compute_event_id(event);
        event.signature = Some(sign_event(event, key).unwrap());
    }

    fn refresh_legacy_identity(payload: &mut FrontierRepositoryBoundaryPayloadV1) {
        payload.identity_root = LegacyFrontierOriginV1 {
            schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: payload.frontier_id.clone(),
            legacy_identity_preimage_root: payload
                .legacy_identity_preimage_root
                .clone()
                .expect("legacy boundary root"),
            git_object_format: payload.git_object_format,
            anchor_git_commit: payload.anchor_git_commit.clone(),
            anchor_git_tree: payload.anchor_git_tree.clone(),
            anchor_event_log_root: payload.anchor_event_log_root.clone(),
            anchor_event_count: payload.anchor_event_count,
        }
        .identity_root()
        .unwrap();
    }

    fn replace_anchor(event: &mut StateEvent, anchor: &RepositoryAnchorFacts, key: &SigningKey) {
        let mut payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(event.payload.clone()).unwrap();
        payload.git_object_format = anchor.git_object_format;
        payload.anchor_git_commit = anchor.git_commit.clone();
        payload.anchor_git_tree = anchor.git_tree.clone();
        payload.anchor_event_log_root = anchor.event_log_root.clone();
        payload.anchor_event_count = anchor.event_count;
        payload.anchor_snapshot_root = anchor.snapshot_root.clone();
        payload.anchor_snapshot_schema = anchor.snapshot_schema.clone();
        payload.anchor_proposal_root = anchor.proposal_root.clone();
        payload.anchor_actor_registry_root = anchor.actor_registry_root.clone();
        payload.anchor_artifact_registry_root = anchor.artifact_registry_root.clone();
        payload.anchor_canonical_store_root = anchor.canonical_store_root.clone();
        refresh_legacy_identity(&mut payload);
        event.payload = serde_json::to_value(payload).unwrap();
        resign(event, key);
    }

    fn dependency_update(
        parent: &StateEvent,
        anchor: &RepositoryAnchorFacts,
        key: &SigningKey,
        reason: &str,
        timestamp: &str,
    ) -> StateEvent {
        let mut payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(parent.payload.clone()).unwrap();
        payload.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
        payload.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
        payload.previous_identity_event_root =
            Some(repository_identity_event_content_root(parent).unwrap());
        payload.git_object_format = anchor.git_object_format;
        payload.anchor_git_commit = anchor.git_commit.clone();
        payload.anchor_git_tree = anchor.git_tree.clone();
        payload.anchor_event_log_root = anchor.event_log_root.clone();
        payload.anchor_event_count = anchor.event_count;
        payload.anchor_snapshot_root = anchor.snapshot_root.clone();
        payload.anchor_snapshot_schema = anchor.snapshot_schema.clone();
        payload.anchor_proposal_root = anchor.proposal_root.clone();
        payload.anchor_actor_registry_root = anchor.actor_registry_root.clone();
        payload.anchor_artifact_registry_root = anchor.artifact_registry_root.clone();
        payload.anchor_canonical_store_root = anchor.canonical_store_root.clone();
        let mut update = new_repository_boundary_event(payload, reason, timestamp).unwrap();
        resign(&mut update, key);
        update
    }

    fn commit_project(directory: &Path, project: &Project, message: &str) -> RepositoryAnchorFacts {
        repo::save(
            &repo::VelaSource::VelaRepo(directory.to_path_buf()),
            project,
        )
        .unwrap();
        run(directory, &["add", "."]);
        run(directory, &["commit", "-qm", message]);
        let commit = run(directory, &["rev-parse", "HEAD"]);
        derive_repository_anchor_facts(directory, &commit).unwrap()
    }

    fn clone_project(project: &Project) -> Project {
        serde_json::from_value(serde_json::to_value(project).unwrap()).unwrap()
    }

    fn trust_anchor(boundary: &StateEvent) -> RepositoryTrustAnchor {
        let payload = repository_boundary_payload_from_event_shape(boundary).unwrap();
        RepositoryTrustAnchor {
            boundary_content_root: repository_identity_event_content_root(boundary).unwrap(),
            administrator_public_key: payload.administrator_public_key,
        }
    }

    fn trust_anchor_v1(boundary: &StateEvent) -> RepositoryTrustAnchorV1 {
        let payload = repository_boundary_payload_from_event_shape(boundary).unwrap();
        RepositoryTrustAnchorV1 {
            schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: payload.frontier_id,
            identity_root: payload.identity_root,
            boundary_content_root: repository_identity_event_content_root(boundary).unwrap(),
            administrator_actor_id: payload.administrator_actor_id,
            administrator_public_key: payload.administrator_public_key,
        }
    }

    fn install_profile_v1_files(fixture: &Fixture) {
        let profile = FrontierProfileV1 {
            schema: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            frontier_id: fixture.project.frontier_id(),
            name: "Migrated repository boundary fixture".to_string(),
            summary: "Exact migrated legacy repository fixture.".to_string(),
            scope: FrontierProfileScopeV1 {
                question: "Does the exact migrated repository pass the write gate?".to_string(),
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
        fs::write(
            fixture.directory.path().join("frontier.yaml"),
            serde_yaml::to_string(&profile).unwrap(),
        )
        .unwrap();
        fs::write(
            fixture.directory.path().join(".vela/settings.toml"),
            FrontierSettingsV1 {
                schema: FRONTIER_SETTINGS_SCHEMA.to_string(),
                publish: None,
                work: None,
                mcp: None,
            }
            .to_toml()
            .unwrap(),
        )
        .unwrap();
    }

    fn verify_with_boundary_anchor(
        project: &Project,
        directory: &Path,
        boundary: &StateEvent,
    ) -> Result<RepositoryBoundaryContext, String> {
        verify_repository_boundary_context_with_trust_anchor(
            project,
            directory,
            boundary,
            Some(&trust_anchor(boundary)),
        )
    }

    fn fixture_with_anchored_blob() -> (Fixture, std::path::PathBuf) {
        let mut fixture = fixture();
        let blob_path = fixture
            .directory
            .path()
            .join(".vela/artifact-blobs/sha256/fixture.bin");
        fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, b"immutable artifact bytes").unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "anchor retained immutable blob"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        *fixture.project.events.last_mut().unwrap() = fixture.boundary.clone();
        verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap();
        (fixture, blob_path)
    }

    fn fixture() -> Fixture {
        let directory = tempfile::tempdir().unwrap();
        run(directory.path(), &["init", "-q", "-b", "main"]);
        run(directory.path(), &["config", "user.name", "Vela Test"]);
        run(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );

        let key = SigningKey::from_bytes(&[17; 32]);
        let actor = ActorRecord {
            id: "reviewer:administrator".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let mut project = vela_protocol::project::assemble(
            "Repository boundary fixture",
            vec![],
            0,
            0,
            "Exact anchored repository fixture",
        );
        project.frontier_id = Some("vfr_0123456789abcdef".to_string());
        project.actors = vec![actor.clone()];
        let mut historical = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "frontier.observation_reviewed".into(),
            target: StateTarget {
                r#type: "frontier".to_string(),
                id: "vfr_0123456789abcdef".to_string(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: actor.id.clone(),
            },
            timestamp: "2026-07-22T00:00:00Z".to_string(),
            reason: "retained historical event".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: json!({
                "proposal_id": "vpr_0123456789abcdef",
                "proposal_kind": "research_trace.review",
                "status": "accepted"
            }),
            caveats: vec![],
            signature: None,
        };
        historical.id = events::compute_event_id(&historical);
        project.events = vec![historical];
        repo::save(
            &repo::VelaSource::VelaRepo(directory.path().to_path_buf()),
            &project,
        )
        .unwrap();
        run(directory.path(), &["add", "."]);
        run(directory.path(), &["commit", "-qm", "anchor"]);
        let commit = run(directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(directory.path(), &commit).unwrap();
        let legacy_root = derive_legacy_identity_preimage_root(&project).unwrap();
        let identity_root = LegacyFrontierOriginV1 {
            schema: LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: project.frontier_id(),
            legacy_identity_preimage_root: legacy_root.clone(),
            git_object_format: anchor.git_object_format,
            anchor_git_commit: anchor.git_commit.clone(),
            anchor_git_tree: anchor.git_tree.clone(),
            anchor_event_log_root: anchor.event_log_root.clone(),
            anchor_event_count: anchor.event_count,
        }
        .identity_root()
        .unwrap();
        let mut boundary = new_repository_boundary_event(
            FrontierRepositoryBoundaryPayloadV1 {
                schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
                frontier_id: project.frontier_id(),
                identity_root,
                observed_profile_root: sha256_root(b"profile"),
                dependency_root: exact_dependency_root(&[]).unwrap(),
                dependencies: vec![],
                previous_identity_event_root: None,
                legacy_identity_preimage_root: Some(legacy_root),
                administrator_actor_id: actor.id,
                administrator_public_key: actor.public_key,
                administrator_algorithm: actor.algorithm,
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
            },
            "bind exact legacy repository",
            "2026-07-22T00:01:00Z",
        )
        .unwrap();
        resign(&mut boundary, &key);
        project.events.push(boundary.clone());
        Fixture {
            directory,
            anchor,
            project,
            key,
            boundary,
        }
    }

    fn fixture_with_legacy_finding() -> Fixture {
        let mut fixture = fixture();
        fixture.project.events.pop();
        fixture.project.findings = vec![make_finding("vf_legacy_remnant", 0.75, "computational")];
        let anchor = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "anchor legacy finding",
        );
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor;
        fixture.project.events.push(fixture.boundary.clone());
        install_profile_v1_files(&fixture);
        fixture
    }

    fn fixture_with_anchored_proposal() -> Fixture {
        let mut fixture = fixture();
        fixture.project.events.pop();
        fixture
            .project
            .proposals
            .push(vela_protocol::proposals::new_proposal_at(
                "finding.note",
                StateTarget {
                    r#type: "finding".to_string(),
                    id: "vf_anchored_proposal".to_string(),
                },
                "agent:fixture",
                "agent",
                "retain exact producer provenance",
                json!({"note": "bounded evidence"}),
                vec!["src:anchored".to_string()],
                vec!["review remains required".to_string()],
                "2026-07-22T00:00:30Z",
            ));
        let anchor = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "anchor pending proposal",
        );
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor;
        fixture.project.events.push(fixture.boundary.clone());
        fixture
    }

    fn append_ancestor_test_event(project: &mut Project, timestamp: &str, reason: &str) {
        let mut event = project.events[0].clone();
        event.id.clear();
        event.timestamp = timestamp.to_string();
        event.reason = reason.to_string();
        event.signature = None;
        event.id = events::compute_event_id(&event);
        project.events.push(event);
    }

    fn fixture_with_historical_ancestor() -> (Fixture, RepositoryAnchorFacts) {
        let mut fixture = fixture();
        let historical = fixture.anchor.clone();
        fixture.project.events.pop();
        append_ancestor_test_event(
            &mut fixture.project,
            "2026-07-22T00:00:30Z",
            "retained post-history event",
        );
        let anchor = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "later temporalization anchor",
        );
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor;
        fixture.project.events.push(fixture.boundary.clone());
        (fixture, historical)
    }

    fn fixture_with_changed_historical_blob(
        replacement: Option<&[u8]>,
    ) -> (Fixture, RepositoryAnchorFacts) {
        let mut fixture = fixture();
        fixture.project.events.pop();
        let blob_path = fixture
            .directory
            .path()
            .join(".vela/artifact-blobs/sha256/historical.bin");
        fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, b"historical retained bytes").unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "retain historical blob"],
        );
        let historical_commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let historical =
            derive_repository_anchor_facts(fixture.directory.path(), &historical_commit).unwrap();

        if let Some(bytes) = replacement {
            fs::write(&blob_path, bytes).unwrap();
        } else {
            fs::remove_file(&blob_path).unwrap();
        }
        run(fixture.directory.path(), &["add", "-A"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "change historical blob"],
        );
        let anchor_commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor =
            derive_repository_anchor_facts(fixture.directory.path(), &anchor_commit).unwrap();
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor;
        fixture.project.events.push(fixture.boundary.clone());
        (fixture, historical)
    }

    #[test]
    fn frontier_repository_bound_exact_anchor() {
        let fixture = fixture();
        let verified = verify_repository_boundary_context_with_trust_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust_anchor(&fixture.boundary)),
        )
        .unwrap();
        assert_eq!(verified.anchor, fixture.anchor);
        assert!(verified.anchor.retained_object_count >= 2);
    }

    #[test]
    fn exact_dependency_pin_is_derived_from_the_boundary_anchor() {
        let fixture = fixture();
        let exact = derive_exact_dependency_at_boundary(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
        )
        .unwrap();
        let payload = repository_boundary_payload_from_event_shape(&fixture.boundary).unwrap();
        assert_eq!(exact.frontier_id, fixture.project.frontier_id());
        assert_eq!(exact.identity_root, payload.identity_root);
        assert_eq!(exact.git_commit, fixture.anchor.git_commit);
        assert_eq!(exact.git_tree, fixture.anchor.git_tree);
        assert_eq!(
            exact.scientific_state_root,
            vela_protocol::scientific_state::scientific_state_root_v2(
                &vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap(),
                &payload.identity_root,
                &payload.dependency_root,
            )
            .unwrap()
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_derives_exact_v1_pin() {
        let (fixture, historical) = fixture_with_historical_ancestor();
        let exact = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &historical.git_commit,
            &historical.snapshot_root,
        )
        .unwrap();
        let historical_project =
            anchored_repository(fixture.directory.path(), &historical.git_commit).unwrap();
        let payload = repository_boundary_payload_from_event_shape(&fixture.boundary).unwrap();

        assert_eq!(exact.frontier_id, fixture.project.frontier_id());
        assert_eq!(exact.identity_root, payload.identity_root);
        assert_eq!(exact.git_commit, historical.git_commit);
        assert_eq!(exact.git_tree, historical.git_tree);
        assert_eq!(
            exact.scientific_state_root,
            vela_protocol::scientific_state::scientific_state_root_v2(
                &historical_project.project,
                &payload.identity_root,
                &exact_dependency_root(&[]).unwrap(),
            )
            .unwrap()
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_uses_temporalization_identity() {
        let (fixture, historical) = fixture_with_historical_ancestor();
        let mut project = clone_project(&fixture.project);
        let update_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "retain temporalization boundary for dependency update",
        );
        let mut update = dependency_update(
            &fixture.boundary,
            &update_anchor,
            &fixture.key,
            "advance dependency context",
            "2026-07-22T00:02:00Z",
        );
        let mut update_payload = repository_boundary_payload_from_event_shape(&update).unwrap();
        update_payload.dependencies = vec![ExactFrontierDependencyV1 {
            frontier_id: "vfr_abcdef0123456789".to_string(),
            identity_root: sha256_root(b"later dependency identity"),
            scientific_state_root: sha256_root(b"later dependency state"),
            git_object_format: update_anchor.git_object_format,
            git_commit: update_anchor.git_commit.clone(),
            git_tree: update_anchor.git_tree.clone(),
        }];
        update_payload.dependency_root =
            exact_dependency_root(&update_payload.dependencies).unwrap();
        update.payload = serde_json::to_value(&update_payload).unwrap();
        resign(&mut update, &fixture.key);
        project.events.push(update.clone());

        let exact = derive_exact_dependency_at_temporalized_ancestor(
            &project,
            fixture.directory.path(),
            &update,
            &trust_anchor(&fixture.boundary),
            &historical.git_commit,
            &historical.snapshot_root,
        )
        .unwrap();
        let historical_project =
            anchored_repository(fixture.directory.path(), &historical.git_commit).unwrap();
        let temporalization_payload =
            repository_boundary_payload_from_event_shape(&fixture.boundary).unwrap();
        let root_context_state = vela_protocol::scientific_state::scientific_state_root_v2(
            &historical_project.project,
            &temporalization_payload.identity_root,
            &temporalization_payload.dependency_root,
        )
        .unwrap();
        let leaf_context_state = vela_protocol::scientific_state::scientific_state_root_v2(
            &historical_project.project,
            &update_payload.identity_root,
            &update_payload.dependency_root,
        )
        .unwrap();

        assert_eq!(exact.identity_root, temporalization_payload.identity_root);
        assert_eq!(exact.scientific_state_root, root_context_state);
        assert_ne!(exact.scientific_state_root, leaf_context_state);
        assert_eq!(exact.git_commit, historical.git_commit);
        assert_eq!(exact.git_tree, historical.git_tree);
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_missing_or_forked_history() {
        let (fixture, historical) = fixture_with_historical_ancestor();
        run(
            fixture.directory.path(),
            &["checkout", "-qb", "sibling-history", &historical.git_commit],
        );
        let mut sibling = vela_protocol::repo::load_from_path(fixture.directory.path()).unwrap();
        append_ancestor_test_event(
            &mut sibling,
            "2026-07-22T00:00:45Z",
            "sibling history event",
        );
        let sibling_anchor = commit_project(fixture.directory.path(), &sibling, "sibling history");
        run(fixture.directory.path(), &["checkout", "-q", "main"]);

        let error = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &sibling_anchor.git_commit,
            &sibling_anchor.snapshot_root,
        )
        .unwrap_err();
        assert!(
            error.contains(
                "historical dependency commit is not an ancestor of the signed temporalization anchor"
            ),
            "{error}"
        );

        let missing = "f".repeat(historical.git_commit.len());
        let error = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &missing,
            &historical.snapshot_root,
        )
        .unwrap_err();
        assert!(
            error.contains("historical dependency state unavailable"),
            "{error}"
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_snapshot_mismatch() {
        let (fixture, historical) = fixture_with_historical_ancestor();
        let error = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &historical.git_commit,
            &sha256_root(b"wrong historical snapshot"),
        )
        .unwrap_err();
        assert!(
            error.contains("historical dependency legacy snapshot mismatch"),
            "{error}"
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_nonreplayable_history() {
        let mut fixture = fixture();
        fixture.project.events.pop();
        let asserted = make_finding("vf_nonreplayable_history", 0.75, "computational");
        let mut assertion = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "finding.asserted".into(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: asserted.id.clone(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: "reviewer:administrator".to_string(),
            },
            timestamp: "2026-07-22T00:00:15Z".to_string(),
            reason: "assert exact historical finding".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: events::finding_hash(&asserted),
            payload: json!({"finding": asserted}),
            caveats: vec![],
            signature: None,
        };
        assertion.id = events::compute_event_id(&assertion);
        fixture.project.events.push(assertion);
        fixture.project.findings = vec![make_finding(
            "vf_nonreplayable_history",
            0.25,
            "computational",
        )];
        let historical = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "historical non-replayable state",
        );
        append_ancestor_test_event(
            &mut fixture.project,
            "2026-07-22T00:00:45Z",
            "retain non-replayable historical state",
        );
        let anchor = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "later temporalization anchor over non-replayable history",
        );
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor;
        fixture.project.events.push(fixture.boundary.clone());

        let error = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &historical.git_commit,
            &historical.snapshot_root,
        )
        .unwrap_err();
        assert!(
            error.contains("historical dependency state does not replay exactly"),
            "{error}"
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_retained_object_loss_or_mutation() {
        for (replacement, expected) in [
            (None, "is absent from the temporalization anchor"),
            (
                Some(b"mutated retained bytes".as_slice()),
                "changed path, mode, size, or digest",
            ),
        ] {
            let (fixture, historical) = fixture_with_changed_historical_blob(replacement);
            let error = derive_exact_dependency_at_temporalized_ancestor(
                &fixture.project,
                fixture.directory.path(),
                &fixture.boundary,
                &trust_anchor(&fixture.boundary),
                &historical.git_commit,
                &historical.snapshot_root,
            )
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_event_signature_or_proposal_loss() {
        let mut signed_fixture = fixture();
        signed_fixture.project.events.pop();
        signed_fixture.project.events[0].signature =
            Some(sign_event(&signed_fixture.project.events[0], &signed_fixture.key).unwrap());
        let signed_history = commit_project(
            signed_fixture.directory.path(),
            &signed_fixture.project,
            "signed historical event",
        );
        signed_fixture.project.events[0].signature = None;
        let stripped_anchor = commit_project(
            signed_fixture.directory.path(),
            &signed_fixture.project,
            "strip historical signature",
        );
        replace_anchor(
            &mut signed_fixture.boundary,
            &stripped_anchor,
            &signed_fixture.key,
        );
        signed_fixture.anchor = stripped_anchor;
        signed_fixture
            .project
            .events
            .push(signed_fixture.boundary.clone());
        let error = derive_exact_dependency_at_temporalized_ancestor(
            &signed_fixture.project,
            signed_fixture.directory.path(),
            &signed_fixture.boundary,
            &trust_anchor(&signed_fixture.boundary),
            &signed_history.git_commit,
            &signed_history.snapshot_root,
        )
        .unwrap_err();
        assert!(error.contains("lost its historical signature"), "{error}");

        let mut proposal_fixture = fixture_with_anchored_proposal();
        let proposal_history = proposal_fixture.anchor.clone();
        proposal_fixture.project.events.pop();
        proposal_fixture.project.proposals.clear();
        let proposal_loss_anchor = commit_project(
            proposal_fixture.directory.path(),
            &proposal_fixture.project,
            "remove historical proposal",
        );
        replace_anchor(
            &mut proposal_fixture.boundary,
            &proposal_loss_anchor,
            &proposal_fixture.key,
        );
        proposal_fixture.anchor = proposal_loss_anchor;
        proposal_fixture
            .project
            .events
            .push(proposal_fixture.boundary.clone());
        let error = derive_exact_dependency_at_temporalized_ancestor(
            &proposal_fixture.project,
            proposal_fixture.directory.path(),
            &proposal_fixture.boundary,
            &trust_anchor(&proposal_fixture.boundary),
            &proposal_history.git_commit,
            &proposal_history.snapshot_root,
        )
        .unwrap_err();
        assert!(
            error.contains("proposal history is not retained"),
            "{error}"
        );
    }

    #[test]
    fn authenticated_ancestor_dependency_rejects_nonempty_dependency_context() {
        let mut fixture = fixture();
        fixture.project.events.pop();
        fixture
            .project
            .project
            .dependencies
            .push(vela_protocol::project::ProjectDependency {
                name: "nonempty-context".to_string(),
                source: "fixture".to_string(),
                version: Some("1".to_string()),
                pinned_hash: Some(sha256_root(b"dependency")),
                vfr_id: None,
                locator: None,
                pinned_snapshot_hash: None,
            });
        let anchor = commit_project(
            fixture.directory.path(),
            &fixture.project,
            "nonempty dependency context",
        );
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.anchor = anchor.clone();
        fixture.project.events.push(fixture.boundary.clone());

        let error = derive_exact_dependency_at_temporalized_ancestor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            &trust_anchor(&fixture.boundary),
            &anchor.git_commit,
            &anchor.snapshot_root,
        )
        .unwrap_err();
        assert!(
            error.contains(
                "historical dependency authentication currently requires an empty dependency context"
            ),
            "{error}"
        );
    }

    #[test]
    fn signed_store_root_is_exact_before_the_migration_commit_exists() {
        let fixture = fixture();
        let planned = derive_migration_signed_store_root(
            fixture.directory.path(),
            &fixture.anchor.git_commit,
            &fixture.boundary,
        )
        .unwrap();
        vela_protocol::repo::save_to_path(fixture.directory.path(), &fixture.project).unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "install signed boundary"],
        );
        let commit = git_text(fixture.directory.path(), &["rev-parse", "HEAD"]).unwrap();
        let installed = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        assert_eq!(planned, installed.canonical_store_root);
    }

    #[test]
    fn current_actor_registry_must_match_the_initial_boundary_bytes() {
        let fixture = fixture();
        let actors_path = fixture.directory.path().join(".vela/actors.json");
        let mut actors: Vec<ActorRecord> =
            serde_json::from_slice(&fs::read(&actors_path).unwrap()).unwrap();
        let extra_key = SigningKey::from_bytes(&[29; 32]);
        actors.push(ActorRecord {
            id: "reviewer:injected".to_string(),
            public_key: pubkey_hex(&extra_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:02:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        fs::write(&actors_path, serde_json::to_vec_pretty(&actors).unwrap()).unwrap();
        let mut tampered = clone_project(&fixture.project);
        tampered.actors = actors;

        let error = verify_repository_boundary_context_with_trust_anchor(
            &tampered,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust_anchor(&fixture.boundary)),
        )
        .unwrap_err();
        assert!(error.contains("current actor registry root"), "{error}");
    }

    #[test]
    fn anchored_proposal_cannot_be_deleted() {
        let fixture = fixture_with_anchored_proposal();
        let proposal_id = fixture.project.proposals[0].id.clone();
        let proposal_path = fixture
            .directory
            .path()
            .join(format!(".vela/proposals/{proposal_id}.json"));
        fs::remove_file(&proposal_path).unwrap();
        let mut deleted = clone_project(&fixture.project);
        deleted.proposals.clear();

        let error =
            verify_with_boundary_anchor(&deleted, fixture.directory.path(), &fixture.boundary)
                .unwrap_err();
        assert!(
            error.contains(&format!("anchored proposal {proposal_id} is absent")),
            "{error}"
        );
    }

    #[test]
    fn anchored_proposal_provenance_cannot_be_rewritten() {
        let fixture = fixture_with_anchored_proposal();
        let proposal_id = fixture.project.proposals[0].id.clone();
        let proposal_path = fixture
            .directory
            .path()
            .join(format!(".vela/proposals/{proposal_id}.json"));
        let mut stored: Value = serde_json::from_slice(&fs::read(&proposal_path).unwrap()).unwrap();
        stored["agent_run"] = json!({
            "agent": "agent:forged",
            "model": "forged-model",
            "run_id": "forged-run",
            "started_at": "2026-07-22T00:00:00Z"
        });
        fs::write(&proposal_path, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
        let mut tampered = repo::load_from_path(fixture.directory.path()).unwrap();
        tampered.events.push(fixture.boundary.clone());

        // Agent-run provenance is deliberately outside the retry-stable
        // proposal id, so ordinary proposal parity alone accepts this edit.
        assert!(
            proposals::verify_proposal_decision_parity(&tampered).is_empty(),
            "fixture must isolate anchored provenance rather than id parity"
        );
        let error =
            verify_with_boundary_anchor(&tampered, fixture.directory.path(), &fixture.boundary)
                .unwrap_err();
        assert!(
            error.contains("changed immutable identity or producer provenance"),
            "{error}"
        );
    }

    #[test]
    fn anchored_proposal_may_follow_a_signed_terminal_event_projection() {
        let mut fixture = fixture_with_anchored_proposal();
        let proposal = fixture.project.proposals[0].clone();
        let decided_at = "2026-07-22T00:03:00Z";
        let decision_reason = "the retained evidence does not establish the claim";
        let mut decision = events::new_review_decision_event(
            &proposal.id,
            &proposal.kind,
            "rejected",
            None,
            "reviewer:administrator",
            decision_reason,
            Some(decided_at),
        )
        .unwrap();
        decision.signature = Some(sign_event(&decision, &fixture.key).unwrap());
        let stored = &mut fixture.project.proposals[0];
        stored.status = "rejected".to_string();
        stored.reviewed_by = Some("reviewer:administrator".to_string());
        stored.reviewed_at = Some(decided_at.to_string());
        stored.decision_reason = Some(decision_reason.to_string());
        fixture.project.events.push(decision);
        repo::save_to_path(fixture.directory.path(), &fixture.project).unwrap();
        let reloaded = repo::load_from_path(fixture.directory.path()).unwrap();

        verify_with_boundary_anchor(&reloaded, fixture.directory.path(), &fixture.boundary)
            .unwrap();
    }

    #[test]
    fn anchored_unsigned_event_may_gain_only_a_valid_signature() {
        let fixture = fixture();
        let trust = trust_anchor(&fixture.boundary);
        let mut signed = clone_project(&fixture.project);
        signed.events[0].signature = Some(sign_event(&signed.events[0], &fixture.key).unwrap());
        verify_repository_boundary_context_with_trust_anchor(
            &signed,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust),
        )
        .unwrap();

        signed.events[0].signature = Some(format!("v1:{}", "0".repeat(128)));
        let error = verify_repository_boundary_context_with_trust_anchor(
            &signed,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust),
        )
        .unwrap_err();
        assert!(error.contains("signature does not verify"), "{error}");
    }

    #[test]
    fn repository_write_gate_requires_exact_legacy_consumer_pin() {
        let fixture = fixture();
        install_profile_v1_files(&fixture);

        let missing = verify_repository_for_write(fixture.directory.path(), &fixture.project, None)
            .unwrap_err();
        assert_eq!(
            missing.code,
            RepositoryWriteGateCode::RepositoryTrustAnchorRequired
        );

        let exact = trust_anchor_v1(&fixture.boundary);
        let mut wrong = exact.clone();
        wrong.boundary_content_root = format!("sha256:{}", "0".repeat(64));
        let mismatch =
            verify_repository_for_write(fixture.directory.path(), &fixture.project, Some(&wrong))
                .unwrap_err();
        assert_eq!(
            mismatch.code,
            RepositoryWriteGateCode::RepositoryTrustAnchorInvalid
        );

        let verified =
            verify_repository_for_write(fixture.directory.path(), &fixture.project, Some(&exact))
                .unwrap();
        assert!(matches!(
            verified.identity,
            VerifiedRepositoryIdentity::PinnedBoundary {
                origin: crate::analysis::repository_write::VerifiedBoundaryOrigin::LegacyBoundary,
                ..
            }
        ));
    }

    #[test]
    fn repository_write_gate_freezes_unreplayed_legacy_sidecars_at_initial_boundary() {
        let mut fixture = fixture();
        fixture.project.events.pop();
        fixture.project.review_events = vec![ReviewEvent {
            id: "rev_anchored".to_string(),
            workspace: None,
            finding_id: "vf_anchored".to_string(),
            reviewer: "reviewer:administrator".to_string(),
            reviewed_at: "2026-07-22T00:00:00Z".to_string(),
            scope: None,
            status: None,
            action: ReviewAction::Approved,
            reason: "anchored legacy review".to_string(),
            evidence_considered: Vec::new(),
            state_change: None,
        }];
        fixture.project.confidence_updates = vec![ConfidenceUpdate {
            finding_id: "vf_anchored".to_string(),
            previous_score: 0.2,
            new_score: 0.4,
            basis: "anchored legacy confidence".to_string(),
            updated_by: "reviewer:administrator".to_string(),
            updated_at: "2026-07-22T00:00:00Z".to_string(),
        }];
        repo::save(
            &repo::VelaSource::VelaRepo(fixture.directory.path().to_path_buf()),
            &fixture.project,
        )
        .unwrap();
        fs::create_dir_all(fixture.directory.path().join(".vela/reviews")).unwrap();
        fs::write(
            fixture
                .directory
                .path()
                .join(".vela/reviews/rev_anchored.json"),
            serde_json::to_vec_pretty(&fixture.project.review_events[0]).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(fixture.directory.path().join(".vela/confidence-updates")).unwrap();
        fs::write(
            fixture
                .directory
                .path()
                .join(".vela/confidence-updates/vf_anchored.json"),
            serde_json::to_vec_pretty(&fixture.project.confidence_updates[0]).unwrap(),
        )
        .unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "anchor legacy sidecars"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        fixture.project.events.push(fixture.boundary.clone());
        install_profile_v1_files(&fixture);
        let trust = trust_anchor_v1(&fixture.boundary);
        verify_repository_for_write(fixture.directory.path(), &fixture.project, Some(&trust))
            .unwrap();

        let mut mutated = clone_project(&fixture.project);
        mutated.review_events[0].reason = "tampered after boundary".to_string();
        let error = verify_repository_for_write(fixture.directory.path(), &mutated, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("review_events"));

        let mut deleted = clone_project(&fixture.project);
        deleted.confidence_updates.clear();
        let error = verify_repository_for_write(fixture.directory.path(), &deleted, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("confidence_updates"));

        let mut inserted = clone_project(&fixture.project);
        inserted.confidence_updates.push(ConfidenceUpdate {
            finding_id: "vf_inserted".to_string(),
            previous_score: 0.4,
            new_score: 0.6,
            basis: "unsigned post-boundary insertion".to_string(),
            updated_by: "agent:tamper".to_string(),
            updated_at: "2026-07-22T00:02:00Z".to_string(),
        });
        let error = verify_repository_for_write(fixture.directory.path(), &inserted, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("confidence_updates"));
    }

    #[test]
    fn repository_write_gate_freezes_legacy_finding_remnants_at_initial_boundary() {
        let fixture = fixture_with_legacy_finding();
        let trust = trust_anchor_v1(&fixture.boundary);
        verify_repository_for_write(fixture.directory.path(), &fixture.project, Some(&trust))
            .unwrap();

        let mut mutated = clone_project(&fixture.project);
        mutated.findings[0].assertion.text = "hand-edited after the boundary".to_string();
        let error = verify_repository_for_write(fixture.directory.path(), &mutated, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(
            error.message.contains("finding projection"),
            "{}",
            error.message
        );

        let mut deleted = clone_project(&fixture.project);
        deleted.findings.clear();
        let error = verify_repository_for_write(fixture.directory.path(), &deleted, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(
            error.message.contains("finding projection"),
            "{}",
            error.message
        );

        let mut inserted = clone_project(&fixture.project);
        inserted
            .findings
            .push(make_finding("vf_unsigned_insert", 0.4, "computational"));
        let error = verify_repository_for_write(fixture.directory.path(), &inserted, Some(&trust))
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(
            error.message.contains("finding projection"),
            "{}",
            error.message
        );
    }

    #[test]
    fn repository_write_gate_accepts_valid_post_boundary_finding_event() {
        let fixture = fixture_with_legacy_finding();
        let trust = trust_anchor_v1(&fixture.boundary);
        let asserted = make_finding("vf_post_boundary", 0.9, "computational");
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "finding.asserted".into(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: asserted.id.clone(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: "reviewer:administrator".to_string(),
            },
            timestamp: "2026-07-22T00:02:00Z".to_string(),
            reason: "assert one event-derived post-boundary finding".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: events::finding_hash(&asserted),
            payload: json!({"finding": asserted}),
            caveats: vec![],
            signature: None,
        };
        resign(&mut event, &fixture.key);

        let mut current = clone_project(&fixture.project);
        current.events.push(event);
        current
            .findings
            .push(make_finding("vf_post_boundary", 0.9, "computational"));
        verify_repository_for_write(fixture.directory.path(), &current, Some(&trust)).unwrap();
    }

    #[test]
    fn frontier_repository_bound_wrong_git_tree_event_snapshot_registry_artifact_fails() {
        let fixture = fixture();
        for field in [
            "anchor_git_tree",
            "anchor_event_log_root",
            "anchor_snapshot_root",
            "anchor_actor_registry_root",
            "anchor_artifact_registry_root",
            "anchor_canonical_store_root",
        ] {
            let mut boundary = fixture.boundary.clone();
            let mut payload: FrontierRepositoryBoundaryPayloadV1 =
                serde_json::from_value(boundary.payload.clone()).unwrap();
            let replacement = if field == "anchor_git_tree" {
                "f".repeat(fixture.anchor.git_tree.len())
            } else {
                format!("sha256:{}", "f".repeat(64))
            };
            match field {
                "anchor_git_tree" => payload.anchor_git_tree = replacement,
                "anchor_event_log_root" => payload.anchor_event_log_root = replacement,
                "anchor_snapshot_root" => payload.anchor_snapshot_root = replacement,
                "anchor_actor_registry_root" => payload.anchor_actor_registry_root = replacement,
                "anchor_artifact_registry_root" => {
                    payload.anchor_artifact_registry_root = replacement;
                }
                "anchor_canonical_store_root" => {
                    payload.anchor_canonical_store_root = replacement;
                }
                _ => unreachable!(),
            }
            refresh_legacy_identity(&mut payload);
            boundary.payload = serde_json::to_value(payload).unwrap();
            resign(&mut boundary, &fixture.key);
            let mut project = clone_project(&fixture.project);
            *project.events.last_mut().unwrap() = boundary.clone();
            let error = verify_with_boundary_anchor(&project, fixture.directory.path(), &boundary)
                .unwrap_err();
            assert!(error.contains(field), "{field}: {error}");
        }
    }

    #[test]
    fn anchor_membership_is_exact_but_independent_of_event_vector_and_lexical_order() {
        let fixture = fixture();
        let mut missing = clone_project(&fixture.project);
        missing.events.remove(0);
        let error = verify_repository_boundary_context_with_trust_anchor(
            &missing,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust_anchor(&fixture.boundary)),
        )
        .unwrap_err();
        assert!(error.contains("anchored canonical event"), "{error}");

        let mut reordered = clone_project(&fixture.project);
        reordered.events.swap(0, 1);
        verify_repository_boundary_context_with_trust_anchor(
            &reordered,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&trust_anchor(&fixture.boundary)),
        )
        .unwrap();

        let anchored_id = fixture.project.events[0].id.clone();
        let mut lexically_earlier = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: "frontier.observation_reviewed".into(),
            target: StateTarget {
                r#type: "frontier".to_string(),
                id: fixture.project.frontier_id(),
            },
            actor: StateActor {
                r#type: "agent".to_string(),
                id: "agent:later".to_string(),
            },
            timestamp: "2026-07-22T00:02:00Z".to_string(),
            reason: String::new(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: json!({
                "proposal_id": "vpr_1123456789abcdef",
                "proposal_kind": "research_trace.review",
                "status": "accepted"
            }),
            caveats: vec![],
            signature: None,
        };
        for nonce in 0..100_000u64 {
            lexically_earlier.reason = format!("post-anchor event {nonce}");
            lexically_earlier.id = events::compute_event_id(&lexically_earlier);
            if lexically_earlier.id < anchored_id {
                break;
            }
        }
        assert!(lexically_earlier.id < anchored_id);
        let mut extended = clone_project(&fixture.project);
        extended.events.push(lexically_earlier);
        verify_with_boundary_anchor(&extended, fixture.directory.path(), &fixture.boundary)
            .unwrap();

        let mut backdated = fixture.boundary.clone();
        backdated.timestamp = "2000-01-01T00:00:00Z".to_string();
        resign(&mut backdated, &fixture.key);
        let mut project = clone_project(&fixture.project);
        *project.events.last_mut().unwrap() = backdated.clone();
        verify_with_boundary_anchor(&project, fixture.directory.path(), &backdated).unwrap();
    }

    #[test]
    fn truncated_event_handle_cannot_substitute_for_anchored_preimage() {
        let fixture = fixture();
        let mut project = clone_project(&fixture.project);
        let anchored_id = project.events[0].id.clone();
        project.events[0].reason = "attacker changed the canonical preimage".to_string();
        // Retain the short display handle to model a truncated-ID collision or
        // substitution. Membership must compare the full canonical preimage.
        project.events[0].id = anchored_id;
        let error =
            verify_with_boundary_anchor(&project, fixture.directory.path(), &fixture.boundary)
                .unwrap_err();
        assert!(error.contains("same display handle"), "{error}");
    }

    #[test]
    fn legacy_tofu_requires_the_exact_out_of_band_root_and_key() {
        let fixture = fixture();
        let error = verify_repository_boundary_context(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap_err();
        assert!(
            error.contains("out-of-band RepositoryTrustAnchor"),
            "{error}"
        );

        let mut wrong = trust_anchor(&fixture.boundary);
        wrong.boundary_content_root = format!("sha256:{}", "f".repeat(64));
        let error = verify_repository_boundary_context_with_trust_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&wrong),
        )
        .unwrap_err();
        assert!(error.contains("boundary root mismatch"), "{error}");
    }

    #[test]
    fn native_administrator_forks_require_their_exact_first_boundary_pin() {
        let directory = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize_profile_v1_minimal(
            directory.path(),
            vela_protocol::frontier_repo::ProfileV1InitOptions {
                name: "Native administrator fork fixture",
                scope: "Can an exact consumer pin distinguish administrator forks?",
                initialize_git: false,
            },
        )
        .unwrap();
        let mut base = repo::load_from_path(directory.path()).unwrap();
        let genesis = base.events.first().unwrap().clone();
        let identity =
            vela_protocol::frontier_repository::FrontierIdentityV1::from_genesis_event(&genesis)
                .unwrap();
        let alice_key = SigningKey::from_bytes(&[31; 32]);
        let bob_key = SigningKey::from_bytes(&[47; 32]);
        let alice = ActorRecord {
            id: "reviewer:alice".to_string(),
            public_key: pubkey_hex(&alice_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let bob = ActorRecord {
            id: "reviewer:bob".to_string(),
            public_key: pubkey_hex(&bob_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        base.actors = vec![alice.clone(), bob.clone()];
        repo::save(
            &repo::VelaSource::VelaRepo(directory.path().to_path_buf()),
            &base,
        )
        .unwrap();
        run(directory.path(), &["init", "-q", "-b", "main"]);
        run(directory.path(), &["config", "user.name", "Vela Test"]);
        run(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        run(directory.path(), &["add", "."]);
        run(
            directory.path(),
            &["commit", "-qm", "shared native genesis anchor"],
        );
        let commit = run(directory.path(), &["rev-parse", "HEAD"]);
        let facts = derive_repository_anchor_facts(directory.path(), &commit).unwrap();

        let build_boundary =
            |actor: &ActorRecord, key: &SigningKey, reason: &str, timestamp: &str| {
                let mut event = new_repository_boundary_event(
                    FrontierRepositoryBoundaryPayloadV1 {
                        schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                        mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
                        frontier_id: identity.frontier_id.clone(),
                        identity_root: identity.root().unwrap(),
                        observed_profile_root: sha256_root(b"profile"),
                        dependency_root: exact_dependency_root(&[]).unwrap(),
                        dependencies: Vec::new(),
                        previous_identity_event_root: Some(
                            repository_identity_event_content_root(&genesis).unwrap(),
                        ),
                        legacy_identity_preimage_root: None,
                        administrator_actor_id: actor.id.clone(),
                        administrator_public_key: actor.public_key.clone(),
                        administrator_algorithm: actor.algorithm.clone(),
                        trust_mode: FrontierRepositoryTrustMode::Genesis,
                        git_object_format: facts.git_object_format,
                        anchor_git_commit: facts.git_commit.clone(),
                        anchor_git_tree: facts.git_tree.clone(),
                        anchor_event_log_root: facts.event_log_root.clone(),
                        anchor_event_count: facts.event_count,
                        anchor_snapshot_root: facts.snapshot_root.clone(),
                        anchor_snapshot_schema: facts.snapshot_schema.clone(),
                        anchor_proposal_root: facts.proposal_root.clone(),
                        anchor_actor_registry_root: facts.actor_registry_root.clone(),
                        anchor_artifact_registry_root: facts.artifact_registry_root.clone(),
                        anchor_canonical_store_root: facts.canonical_store_root.clone(),
                    },
                    reason,
                    timestamp,
                )
                .unwrap();
                resign(&mut event, key);
                event
            };
        let alice_boundary = build_boundary(
            &alice,
            &alice_key,
            "Alice administers this fork",
            "2026-07-22T00:01:00Z",
        );
        let bob_boundary = build_boundary(
            &bob,
            &bob_key,
            "Bob administers this fork",
            "2026-07-22T00:01:00Z",
        );
        let mut alice_project = clone_project(&base);
        alice_project.events.push(alice_boundary.clone());
        let mut bob_project = clone_project(&base);
        bob_project.events.push(bob_boundary.clone());
        let alice_pin = trust_anchor(&alice_boundary);
        let bob_pin = trust_anchor(&bob_boundary);

        let alice_error = verify_repository_boundary_context_with_trust_anchor(
            &alice_project,
            directory.path(),
            &alice_boundary,
            Some(&alice_pin),
        )
        .unwrap_err();
        assert!(
            alice_error.contains("exact one-actor bootstrap registry"),
            "{alice_error}"
        );
        let bob_error = verify_repository_boundary_context_with_trust_anchor(
            &bob_project,
            directory.path(),
            &bob_boundary,
            Some(&bob_pin),
        )
        .unwrap_err();
        assert!(
            bob_error.contains("exact one-actor bootstrap registry"),
            "{bob_error}"
        );

        let missing =
            verify_repository_boundary_context(&alice_project, directory.path(), &alice_boundary)
                .unwrap_err();
        assert!(
            missing.contains("out-of-band RepositoryTrustAnchor"),
            "{missing}"
        );
        let alice_with_bob_pin = verify_repository_boundary_context_with_trust_anchor(
            &alice_project,
            directory.path(),
            &alice_boundary,
            Some(&bob_pin),
        )
        .unwrap_err();
        assert!(
            alice_with_bob_pin.contains("boundary root mismatch"),
            "{alice_with_bob_pin}"
        );
        let bob_with_alice_pin = verify_repository_boundary_context_with_trust_anchor(
            &bob_project,
            directory.path(),
            &bob_boundary,
            Some(&alice_pin),
        )
        .unwrap_err();
        assert!(
            bob_with_alice_pin.contains("boundary root mismatch"),
            "{bob_with_alice_pin}"
        );
    }

    #[test]
    fn selected_boundary_must_be_the_unique_full_root_chain_leaf() {
        let fixture = fixture();
        repo::save(
            &repo::VelaSource::VelaRepo(fixture.directory.path().to_path_buf()),
            &fixture.project,
        )
        .unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "record initial repository boundary"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();

        let mut payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(fixture.boundary.payload.clone()).unwrap();
        payload.mode = FrontierRepositoryBoundaryMode::UpdateDependencies;
        payload.trust_mode = FrontierRepositoryTrustMode::PreviousBoundary;
        payload.previous_identity_event_root =
            Some(repository_identity_event_content_root(&fixture.boundary).unwrap());
        payload.git_object_format = anchor.git_object_format;
        payload.anchor_git_commit = anchor.git_commit.clone();
        payload.anchor_git_tree = anchor.git_tree.clone();
        payload.anchor_event_log_root = anchor.event_log_root.clone();
        payload.anchor_event_count = anchor.event_count;
        payload.anchor_snapshot_root = anchor.snapshot_root.clone();
        payload.anchor_snapshot_schema = anchor.snapshot_schema.clone();
        payload.anchor_proposal_root = anchor.proposal_root.clone();
        payload.anchor_actor_registry_root = anchor.actor_registry_root.clone();
        payload.anchor_artifact_registry_root = anchor.artifact_registry_root.clone();
        payload.anchor_canonical_store_root = anchor.canonical_store_root.clone();
        let mut update = new_repository_boundary_event(
            payload,
            "advance exact dependency boundary",
            "2026-07-22T00:03:00Z",
        )
        .unwrap();
        resign(&mut update, &fixture.key);
        let mut project = clone_project(&fixture.project);
        project.events.push(update.clone());

        let reviewed_anchor = trust_anchor(&fixture.boundary);
        let error = verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &fixture.boundary,
            Some(&reviewed_anchor),
        )
        .unwrap_err();
        assert!(error.contains("not the unique valid chain leaf"), "{error}");

        verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &update,
            Some(&reviewed_anchor),
        )
        .unwrap();
    }

    #[test]
    fn later_boundary_cannot_launder_actor_registry_replacement() {
        let fixture = fixture();
        let mut project = clone_project(&fixture.project);
        let first_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "record initial repository boundary",
        );
        let first_update = dependency_update(
            &fixture.boundary,
            &first_anchor,
            &fixture.key,
            "first dependency update",
            "2026-07-22T00:03:00Z",
        );
        project.events.push(first_update.clone());

        let injected_key = SigningKey::from_bytes(&[31; 32]);
        project.actors.push(ActorRecord {
            id: "reviewer:injected".to_string(),
            public_key: pubkey_hex(&injected_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:03:30Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let replacement_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "attempt to launder a replaced registry",
        );
        let second_update = dependency_update(
            &first_update,
            &replacement_anchor,
            &fixture.key,
            "second dependency update",
            "2026-07-22T00:05:00Z",
        );
        project.events.push(second_update.clone());

        let error = verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &second_update,
            Some(&trust_anchor(&fixture.boundary)),
        )
        .unwrap_err();
        assert!(
            error.contains("actors.json")
                || error.contains("changed the administrator actor registry"),
            "{error}"
        );
    }

    #[test]
    fn every_intermediate_boundary_anchor_is_context_verified() {
        let fixture = fixture();
        let mut project = clone_project(&fixture.project);
        let reviewed_anchor = trust_anchor(&fixture.boundary);

        let first_update_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "record initial repository boundary",
        );
        let mut first_update = dependency_update(
            &fixture.boundary,
            &first_update_anchor,
            &fixture.key,
            "first dependency update",
            "2026-07-22T00:03:00Z",
        );
        let mut first_payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(first_update.payload.clone()).unwrap();
        first_payload.anchor_snapshot_root = format!("sha256:{}", "f".repeat(64));
        first_update.payload = serde_json::to_value(first_payload).unwrap();
        resign(&mut first_update, &fixture.key);
        project.events.push(first_update.clone());

        let second_update_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "record malformed intermediate boundary",
        );
        let second_update = dependency_update(
            &first_update,
            &second_update_anchor,
            &fixture.key,
            "second dependency update",
            "2026-07-22T00:04:00Z",
        );
        project.events.push(second_update.clone());

        let error = verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &second_update,
            Some(&reviewed_anchor),
        )
        .unwrap_err();
        assert!(error.contains("context invalid"), "{error}");
        assert!(error.contains("anchor_snapshot_root mismatch"), "{error}");
    }

    #[test]
    fn child_anchor_cannot_delete_and_later_reintroduce_parent_event_prefix() {
        let fixture = fixture();
        let mut project = clone_project(&fixture.project);
        let reviewed_anchor = trust_anchor(&fixture.boundary);
        let historical = project.events.remove(0);
        fs::remove_file(
            fixture
                .directory
                .path()
                .join(".vela/events")
                .join(format!("{}.json", historical.id)),
        )
        .unwrap();

        let mut replacement = historical.clone();
        replacement.actor.id = "agent:replacement".to_string();
        replacement.reason = "replacement event keeps the child count advancing".to_string();
        replacement.id = events::compute_event_id(&replacement);
        project.events.push(replacement);

        let first_update_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "delete an event from the inherited prefix",
        );
        let first_update = dependency_update(
            &fixture.boundary,
            &first_update_anchor,
            &fixture.key,
            "first dependency update",
            "2026-07-22T00:03:00Z",
        );
        project.events.push(first_update.clone());

        // Restore the exact old event only after the malformed child anchor.
        // Root and leaf membership checks alone would accept this current set.
        project.events.push(historical);
        let second_update_anchor = commit_project(
            fixture.directory.path(),
            &project,
            "reintroduce the deleted anchored event",
        );
        let second_update = dependency_update(
            &first_update,
            &second_update_anchor,
            &fixture.key,
            "second dependency update",
            "2026-07-22T00:04:00Z",
        );
        project.events.push(second_update.clone());

        let error = verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &second_update,
            Some(&reviewed_anchor),
        )
        .unwrap_err();
        assert!(error.contains("anchor prefix"), "{error}");
        assert!(error.contains("anchored canonical event"), "{error}");
    }

    #[test]
    fn wrong_legacy_identity_preimage_fails_even_when_boundary_is_pinned() {
        let fixture = fixture();
        let mut boundary = fixture.boundary.clone();
        let mut payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(boundary.payload.clone()).unwrap();
        payload.legacy_identity_preimage_root = Some(format!("sha256:{}", "f".repeat(64)));
        refresh_legacy_identity(&mut payload);
        boundary.payload = serde_json::to_value(payload).unwrap();
        resign(&mut boundary, &fixture.key);
        let mut project = clone_project(&fixture.project);
        *project.events.last_mut().unwrap() = boundary.clone();
        let error =
            verify_with_boundary_anchor(&project, fixture.directory.path(), &boundary).unwrap_err();
        assert!(
            error.contains("legacy_identity_preimage_root mismatch"),
            "{error}"
        );
    }

    #[test]
    fn malicious_registry_replacement_cannot_self_authorize_boundary() {
        let fixture = fixture();
        let mut revoked = fixture.project.actors[0].clone();
        revoked.revoked_at = Some("2026-07-22T00:00:30Z".to_string());
        let bytes = serde_json::to_vec_pretty(&vec![revoked]).unwrap();
        fs::write(fixture.directory.path().join(".vela/actors.json"), bytes).unwrap();
        run(fixture.directory.path(), &["add", ".vela/actors.json"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "revoked anchor"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        assert_ne!(
            anchor.actor_registry_root,
            fixture.anchor.actor_registry_root
        );

        let mut boundary = fixture.boundary.clone();
        replace_anchor(&mut boundary, &anchor, &fixture.key);
        let mut project = clone_project(&fixture.project);
        *project.events.last_mut().unwrap() = boundary.clone();
        let error =
            verify_with_boundary_anchor(&project, fixture.directory.path(), &boundary).unwrap_err();
        assert!(error.contains("revoked"), "{error}");
    }

    #[test]
    fn attacker_registry_and_key_replacement_cannot_replace_tofu_pin() {
        let fixture = fixture();
        let reviewed_anchor = trust_anchor(&fixture.boundary);
        let attacker_key = SigningKey::from_bytes(&[91; 32]);
        let attacker = ActorRecord {
            id: "reviewer:administrator".to_string(),
            public_key: pubkey_hex(&attacker_key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        fs::write(
            fixture.directory.path().join(".vela/actors.json"),
            serde_json::to_vec_pretty(&vec![attacker.clone()]).unwrap(),
        )
        .unwrap();
        run(fixture.directory.path(), &["add", ".vela/actors.json"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "attacker registry replacement"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();

        let mut boundary = fixture.boundary.clone();
        replace_anchor(&mut boundary, &anchor, &attacker_key);
        let mut payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(boundary.payload.clone()).unwrap();
        payload.administrator_public_key = attacker.public_key;
        boundary.payload = serde_json::to_value(payload).unwrap();
        resign(&mut boundary, &attacker_key);
        let mut project = clone_project(&fixture.project);
        *project.events.last_mut().unwrap() = boundary.clone();

        let error = verify_repository_boundary_context_with_trust_anchor(
            &project,
            fixture.directory.path(),
            &boundary,
            Some(&reviewed_anchor),
        )
        .unwrap_err();
        assert!(
            error.contains("repository trust anchor boundary root mismatch")
                || error.contains("administrator public key mismatch"),
            "{error}"
        );
    }

    #[test]
    fn raw_tree_paths_fail_before_materialization_can_escape() {
        let escape_name = format!("vela-repository-boundary-escape-{}", std::process::id());
        let outside = std::env::temp_dir().join(&escape_name);
        let _ = fs::remove_file(&outside);
        let traversal = GitTreeEntry {
            mode: "100644".to_string(),
            kind: "blob".to_string(),
            object: "0".repeat(40),
            path: format!("../{escape_name}"),
        };
        let error = materialize_project_inputs(Path::new("."), &[traversal]).unwrap_err();
        assert!(error.contains("normalized relative NFC"), "{error}");
        assert!(
            !outside.exists(),
            "traversal path wrote outside temporary root"
        );

        let symlinked_project_input = GitTreeEntry {
            mode: "120000".to_string(),
            kind: "blob".to_string(),
            object: "0".repeat(40),
            path: ".vela/events/vev_0123456789abcdef.json".to_string(),
        };
        let error = validate_git_tree_entries(&[symlinked_project_input]).unwrap_err();
        assert!(error.contains("tracked regular blob"), "{error}");

        let collisions = [
            GitTreeEntry {
                mode: "100644".to_string(),
                kind: "blob".to_string(),
                object: "0".repeat(40),
                path: "Evidence/Result.json".to_string(),
            },
            GitTreeEntry {
                mode: "100644".to_string(),
                kind: "blob".to_string(),
                object: "1".repeat(40),
                path: "evidence/result.json".to_string(),
            },
        ];
        let error = validate_git_tree_entries(&collisions).unwrap_err();
        assert!(error.contains("portable case-fold"), "{error}");

        let decomposed = GitTreeEntry {
            mode: "100644".to_string(),
            kind: "blob".to_string(),
            object: "2".repeat(40),
            path: "evidence/cafe\u{301}.json".to_string(),
        };
        let error = validate_git_tree_entries(&[decomposed]).unwrap_err();
        assert!(error.contains("normalized relative NFC"), "{error}");
    }

    #[test]
    fn frontier_repository_bound_nonancestor_anchor_fails_closed() {
        let fixture = fixture();
        let tree = run(fixture.directory.path(), &["rev-parse", "HEAD^{tree}"]);
        let orphan = run(
            fixture.directory.path(),
            &["commit-tree", &tree, "-m", "unrelated root"],
        );
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &orphan).unwrap();
        let mut boundary = fixture.boundary.clone();
        replace_anchor(&mut boundary, &anchor, &fixture.key);
        let mut project = clone_project(&fixture.project);
        *project.events.last_mut().unwrap() = boundary.clone();
        let error =
            verify_with_boundary_anchor(&project, fixture.directory.path(), &boundary).unwrap_err();
        assert!(error.contains("not an ancestor"), "{error}");
    }

    #[test]
    fn retained_object_manifest_binds_only_the_exact_retained_closure() {
        let fixture = fixture();
        let original = fixture.anchor.canonical_store_root;
        const POLICY_ID: &str = "vap_0123456789abcdef0123456789abcdef";

        fs::create_dir_all(fixture.directory.path().join("notes")).unwrap();
        fs::write(
            fixture.directory.path().join("notes/unrelated.txt"),
            b"display-only notes",
        )
        .unwrap();
        run(fixture.directory.path(), &["add", "notes/unrelated.txt"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "unrelated note"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let unrelated = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        assert_eq!(unrelated.canonical_store_root, original);

        fs::create_dir_all(fixture.directory.path().join(".vela/policies")).unwrap();
        fs::write(
            fixture
                .directory
                .path()
                .join(format!(".vela/policies/{POLICY_ID}.json")),
            br#"{"policy":"historical"}"#,
        )
        .unwrap();
        run(
            fixture.directory.path(),
            &["add", &format!(".vela/policies/{POLICY_ID}.json")],
        );
        run(
            fixture.directory.path(),
            &["commit", "-qm", "retained policy"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let policy = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        assert_ne!(policy.canonical_store_root, original);

        fs::write(
            fixture.directory.path().join(".vela/policies/active.json"),
            br#"{"policy":"mutable pointer"}"#,
        )
        .unwrap();
        run(
            fixture.directory.path(),
            &["add", ".vela/policies/active.json"],
        );
        run(
            fixture.directory.path(),
            &["commit", "-qm", "mutable active policy pointer"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let active = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        assert_eq!(active.canonical_store_root, policy.canonical_store_root);

        fs::create_dir_all(fixture.directory.path().join("records/receipts/sha256")).unwrap();
        fs::write(
            fixture
                .directory
                .path()
                .join("records/receipts/sha256/not-a-receipt.json"),
            b"{}",
        )
        .unwrap();
        run(
            fixture.directory.path(),
            &["add", "records/receipts/sha256/not-a-receipt.json"],
        );
        run(
            fixture.directory.path(),
            &["commit", "-qm", "invalid retained receipt"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let error = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap_err();
        assert!(error.contains("invalid retained Receipt"), "{error}");
    }

    #[test]
    fn retained_manifest_rejects_per_blob_and_aggregate_budget_exhaustion() {
        let (fixture, blob_path) = fixture_with_anchored_blob();
        fs::write(&blob_path, vec![b'x'; 1024 * 1024]).unwrap();
        run(
            fixture.directory.path(),
            &["add", ".vela/artifact-blobs/sha256/fixture.bin"],
        );
        run(
            fixture.directory.path(),
            &["commit", "-qm", "large retained blob"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let entries = tree_entries(fixture.directory.path(), &commit).unwrap();
        let (_temporary, anchored_project) =
            materialize_project_inputs(fixture.directory.path(), &entries).unwrap();
        let blob_size = fs::metadata(&blob_path).unwrap().len();
        assert!(blob_size > 1);

        let per_blob = derive_retained_manifest_with_limits(
            fixture.directory.path(),
            &entries,
            &anchored_project,
            BlobReadLimits {
                max_blob_bytes: blob_size - 1,
                max_total_bytes: u64::MAX,
            },
        )
        .unwrap_err();
        assert!(per_blob.contains("per-blob limit"), "{per_blob}");
        assert!(per_blob.contains("artifact-blobs"), "{per_blob}");

        let aggregate = derive_retained_manifest_with_limits(
            fixture.directory.path(),
            &entries,
            &anchored_project,
            BlobReadLimits {
                max_blob_bytes: u64::MAX,
                max_total_bytes: blob_size - 1,
            },
        )
        .unwrap_err();
        assert!(aggregate.contains("aggregate would reach"), "{aggregate}");
        assert!(aggregate.contains("artifact-blobs"), "{aggregate}");
    }

    #[test]
    fn anchored_artifact_blob_and_policy_snapshot_cannot_change_or_disappear() {
        let mut fixture = fixture();
        const POLICY_ID: &str = "vap_0123456789abcdef0123456789abcdef";
        let blob_path = fixture
            .directory
            .path()
            .join(".vela/artifact-blobs/sha256/fixture.bin");
        let policy_path = fixture
            .directory
            .path()
            .join(format!(".vela/policies/{POLICY_ID}.json"));
        let signature_path = fixture
            .directory
            .path()
            .join(format!(".vela/policies/{POLICY_ID}.sig.json"));
        fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
        fs::create_dir_all(policy_path.parent().unwrap()).unwrap();
        fs::write(&blob_path, b"immutable artifact bytes").unwrap();
        fs::write(&policy_path, br#"{"schema":"test.policy.v1"}"#).unwrap();
        fs::write(&signature_path, br#"{"signature":"test"}"#).unwrap();
        run(fixture.directory.path(), &["add", "."]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "anchor retained immutable bytes"],
        );
        let commit = run(fixture.directory.path(), &["rev-parse", "HEAD"]);
        let anchor = derive_repository_anchor_facts(fixture.directory.path(), &commit).unwrap();
        replace_anchor(&mut fixture.boundary, &anchor, &fixture.key);
        *fixture.project.events.last_mut().unwrap() = fixture.boundary.clone();
        verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap();

        fs::write(&blob_path, b"tampered artifact bytes").unwrap();
        let error = verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap_err();
        assert!(error.contains("artifact-blobs"), "{error}");
        assert!(error.contains("changed byte content"), "{error}");

        fs::write(&blob_path, b"immutable artifact bytes").unwrap();
        fs::remove_file(&signature_path).unwrap();
        let error = verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap_err();
        assert!(error.contains(POLICY_ID), "{error}");
        assert!(
            error.contains("absent from the current checkout"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn anchored_retained_object_rejects_a_symlinked_parent_even_with_identical_bytes() {
        use std::os::unix::fs::symlink;

        let (fixture, blob_path) = fixture_with_anchored_blob();
        let original_parent = blob_path.parent().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let relocated = outside.path().join("sha256");
        fs::rename(original_parent, &relocated).unwrap();
        symlink(&relocated, original_parent).unwrap();

        let error = verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap_err();
        assert!(
            error.contains("real non-symlink repository directories"),
            "{error}"
        );
    }

    #[test]
    fn anchored_retained_object_rejects_an_untracked_matching_replacement() {
        let (fixture, blob_path) = fixture_with_anchored_blob();
        run(
            fixture.directory.path(),
            &[
                "rm",
                "--cached",
                "--",
                ".vela/artifact-blobs/sha256/fixture.bin",
            ],
        );
        assert_eq!(fs::read(&blob_path).unwrap(), b"immutable artifact bytes");

        let error = verify_with_boundary_anchor(
            &fixture.project,
            fixture.directory.path(),
            &fixture.boundary,
        )
        .unwrap_err();
        assert!(
            error.contains("must remain one exact tracked worktree file"),
            "{error}"
        );
    }
}
