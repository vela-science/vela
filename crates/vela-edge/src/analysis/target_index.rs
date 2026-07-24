//! Closed Target Index v2 values and read-only repository assessment.
//!
//! Target catalogues are derived briefing projections. This module proves
//! their exact Git inputs and current repository context before they can
//! become producer offers; it does not rank domain work or grant authority.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::process::Command;
use std::process::{Output, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use vela_protocol::events::{
    self, EVENT_KIND_FRONTIER_REPOSITORY_BOUND, StateEvent, event_content_preimage_bytes,
};
use vela_protocol::frontier_profile::FrontierProfileV1;
use vela_protocol::frontier_repository::{
    FrontierIdentityV1, FrontierRepositoryBoundaryMode, FrontierRepositoryBoundaryPayloadV1,
    FrontierRepositoryTrustMode, GitObjectFormat, RetainedObjectEntryV1, RetainedObjectManifestV1,
    exact_dependency_root, new_repository_boundary_event, repository_boundary_event_content_root,
    repository_boundary_payload_from_event_shape, validate_repository_boundary_event_set,
};
use vela_protocol::project::Project;
use vela_protocol::{canonical, proposals, repo};

use super::frontier_repository::{
    RepositoryTrustAnchor, verify_repository_boundary_context_with_trust_anchor,
};
use super::repository_write::{PreparedRepositoryFileReplacement, RepositoryFileReplacementMode};

pub const TARGET_INDEX_SCHEMA_V2: &str = "vela.target-index.v2";
pub const TARGET_INDEX_CANDIDATE_SCHEMA_V1: &str = "vela.target-index-candidate.v1";
pub const TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1: &str = "vela.target-index-input-manifest.v1";
pub const TARGET_TASK_BINDING_SCHEMA_V1: &str = "vela.target-task-binding.v1";
pub const TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1: &str = "vela.target-index-migration-context.v1";
pub const TARGET_INDEX_SCHEMA_V1: &str = "vela.target-index.v1";

pub const TARGET_INDEX_JSON_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const TARGET_PACKET_MAX_BYTES: u64 = 1024 * 1024;
pub const TARGET_INDEX_MAX_TARGETS: usize = 16_384;
pub const TARGET_INDEX_MAX_LABELS: usize = 64;
pub const EXTERNAL_TARGET_ID_MAX_BYTES: usize = 256;
const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const SOURCE_VIEW_BLOB_MAX_BYTES: u64 = 64 * 1024 * 1024;
const SOURCE_VIEW_TOTAL_MAX_BYTES: u64 = 1024 * 1024 * 1024;

pub const CODE_SCHEMA_INVALID: &str = "target_index_schema_invalid";
pub const CODE_FRONTIER_MISMATCH: &str = "target_index_frontier_mismatch";
pub const CODE_SOURCE_UNAVAILABLE: &str = "target_index_source_unavailable";
pub const CODE_SOURCE_NOT_ANCESTOR: &str = "target_index_source_not_ancestor";
pub const CODE_SOURCE_TREE_MISMATCH: &str = "target_index_source_tree_mismatch";
pub const CODE_SOURCE_SELF_REFERENCE: &str = "target_index_source_self_reference";
pub const CODE_EVENT_ROOT_MISMATCH: &str = "target_index_event_root_mismatch";
pub const CODE_STATE_ROOT_MISMATCH: &str = "target_index_state_root_mismatch";
pub const CODE_PROPOSAL_ROOT_MISMATCH: &str = "target_index_proposal_root_mismatch";
pub const CODE_IDENTITY_ROOT_MISMATCH: &str = "target_index_identity_root_mismatch";
pub const CODE_DEPENDENCY_ROOT_MISMATCH: &str = "target_index_dependency_root_mismatch";
pub const CODE_INPUT_ROOT_MISMATCH: &str = "target_index_input_root_mismatch";
pub const CODE_INDEX_ROOT_MISMATCH: &str = "target_index_index_root_mismatch";
pub const CODE_PACKET_MISMATCH: &str = "target_index_packet_mismatch";
pub const CODE_OUTPUT_NOT_TRACKED: &str = "target_index_output_not_tracked";
pub const CODE_DUPLICATE_TARGET: &str = "target_index_duplicate_target";
pub const CODE_INVALID_PATH: &str = "target_index_invalid_path";
pub const CODE_INVALID_TARGET: &str = "target_index_invalid_target";
pub const CODE_PROFILE_UPGRADE_REQUIRED: &str = "target_index_profile_upgrade_required";

const TARGET_INDEX_REGENERATION_INSTRUCTION: &str = "Regenerate the closed domain-owned candidate and its packet outputs before running the seal check; Vela will not invent or repin target semantics.";
const TARGET_INDEX_MIGRATION_INSTRUCTION: &str = "Generate a Profile v1 candidate and the closed domain-owned target-index candidate, then preview the protected frontier-repo-v1 migration; historical Target Index v1 remains inspectable but cannot become producer work.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexSourceV2 {
    pub git_object_format: GitObjectFormat,
    pub git_commit: String,
    pub git_tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexInputManifestV1 {
    pub schema: String,
    pub input_root: String,
    pub entries: Vec<RetainedObjectEntryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexRootsV2 {
    pub event_log_root: String,
    pub event_count: u64,
    pub nonlease_event_log_root: String,
    pub scientific_state_root: String,
    pub proposal_root: String,
    pub identity_root: String,
    pub dependency_root: String,
    pub observed_profile_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexClaimBoundaryV2 {
    pub derived: bool,
    pub authoritative: bool,
    pub deletable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexGeneratorV2 {
    pub program: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetPacketRefV2 {
    pub schema: String,
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexEntryV2 {
    pub id: String,
    pub title: String,
    pub why: String,
    pub state: String,
    pub rank: u64,
    pub objective: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub packet: TargetPacketRefV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexV2 {
    pub schema: String,
    pub frontier_id: String,
    pub source: TargetIndexSourceV2,
    pub inputs: TargetIndexInputManifestV1,
    pub roots: TargetIndexRootsV2,
    pub claim_boundary: TargetIndexClaimBoundaryV2,
    pub generated_by: TargetIndexGeneratorV2,
    pub targets: Vec<TargetIndexEntryV2>,
    pub index_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexCandidateSourceV1 {
    pub git_commit: String,
    pub input_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetPacketCandidateV1 {
    pub schema: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexCandidateEntryV1 {
    pub id: String,
    pub title: String,
    pub why: String,
    pub state: String,
    pub rank: u64,
    pub objective: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub packet: TargetPacketCandidateV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexCandidateV1 {
    pub schema: String,
    pub frontier_id: String,
    pub source: TargetIndexCandidateSourceV1,
    pub targets: Vec<TargetIndexCandidateEntryV1>,
}

/// Exact pre/post-boundary context for deriving a Target Index v2 during the
/// one legacy Repository Profile migration transaction.
///
/// The context is deliberately independent of `targets.json`: domain target
/// semantics still come only from the separately rooted candidate. The
/// boundary may be unsigned during key-free preview or signed during apply;
/// its canonical content core must be identical in either phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexMigrationContextV1 {
    pub schema: String,
    pub anchor_git_commit: String,
    pub anchor_git_tree: String,
    pub source_event_log_root: String,
    pub source_event_count: u64,
    pub source_nonlease_event_log_root: String,
    pub planned_boundary_event: StateEvent,
    pub planned_boundary_event_content_root: String,
    pub final_roots: TargetIndexRootsV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTaskIndexRootsV1 {
    pub event_log_root: String,
    pub event_count: u64,
    pub nonlease_event_log_root: String,
    pub scientific_state_root: String,
    pub proposal_root: String,
    pub identity_root: String,
    pub dependency_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTaskClaimReadSetV1 {
    pub event_log_root: String,
    pub event_count: u64,
    pub git_object_format: GitObjectFormat,
    pub git_commit: String,
    pub git_tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTaskBindingV1 {
    pub schema: String,
    pub frontier_id: String,
    pub target_id: String,
    pub target_index_root: String,
    pub source: TargetIndexSourceV2,
    pub input_root: String,
    pub packet: TargetPacketRefV2,
    pub index_roots: TargetTaskIndexRootsV1,
    pub claim_read_set: TargetTaskClaimReadSetV1,
    pub binding_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TargetIndexV1 {
    pub(crate) schema: String,
    pub(crate) frontier_id: String,
    pub(crate) as_of: TargetIndexAsOfV1,
    pub(crate) targets: Vec<TargetIndexEntryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TargetIndexAsOfV1 {
    pub(crate) snapshot_hash: String,
    pub(crate) event_log_hash: String,
    pub(crate) proposal_state_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TargetPacketRefV1 {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TargetIndexEntryV1 {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) why: String,
    pub(crate) state: String,
    pub(crate) rank: u64,
    pub(crate) objective: String,
    #[serde(default)]
    pub(crate) labels: Vec<String>,
    pub(crate) packet: TargetPacketRefV1,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetIndexIssue {
    pub code: &'static str,
    pub target_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub(crate) enum TargetIndexDocument {
    HistoricalV1(TargetIndexV1),
    V2(TargetIndexV2),
}

#[derive(Debug, Clone)]
pub struct TargetIndexAssessment {
    pub document_root: String,
    pub(crate) document: TargetIndexDocument,
    pub global_issues: Vec<TargetIndexIssue>,
    pub target_issues: BTreeMap<String, Vec<TargetIndexIssue>>,
    packet_values: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TargetIndexTargetInspection {
    pub schema: &'static str,
    pub index_schema: String,
    pub index_root: String,
    pub target_id: String,
    pub title: String,
    pub state: String,
    pub historical_only: bool,
    pub actionable: bool,
    pub codes: Vec<&'static str>,
    pub packet_ref: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet: Option<Value>,
}

/// Complete, write-free result of sealing one domain-owned target-index
/// candidate. The candidate supplies target semantics; every security- or
/// integrity-bearing value in `index` is derived by Vela.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexSealPlan {
    pub schema: &'static str,
    pub frontier_id: String,
    pub candidate_path: String,
    pub candidate_root: String,
    pub source: TargetIndexSourceV2,
    pub input_paths: Vec<String>,
    pub packet_paths: Vec<String>,
    pub index_path: &'static str,
    pub index_root: String,
    pub canonical_json: String,
    pub index: TargetIndexV2,
    pub touched_paths: Vec<String>,
    #[serde(skip)]
    allowed_dirty_paths: BTreeSet<String>,
}

/// Closed read-only repair result. It reports drift and the one explicit
/// command that can prepare a replacement seal; it never regenerates domain
/// semantics or repins an existing index.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexRepairReport {
    pub schema: &'static str,
    pub frontier_id: String,
    pub index_schema: String,
    pub index_root: String,
    pub historical_only: bool,
    pub codes: Vec<&'static str>,
    pub changed_declared_paths: Vec<String>,
    pub candidate_path: &'static str,
    pub generator_instruction: &'static str,
    pub repair_command: String,
}

/// Closed summary used when `inspect` is invoked without a target ID.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexInspectionSummary {
    pub schema: &'static str,
    pub frontier_id: String,
    pub index_schema: String,
    pub index_root: String,
    pub historical_only: bool,
    pub configured_open: usize,
    pub stale_open: usize,
    pub codes: Vec<&'static str>,
    pub repair_command: String,
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn canonical_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    canonical::to_canonical_bytes(value)
        .map(|bytes| sha256_root(&bytes))
        .map_err(|error| error.to_string())
}

fn require_sha256_root(field: &str, value: &str) -> Result<(), String> {
    if vela_protocol::receipt_v1::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "{field} must use the sha256:<64 lowercase hex> form"
        ))
    }
}

fn require_frontier_id(value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("vfr_") else {
        return Err("frontier_id must use the vfr_<16 lowercase hex> form".to_string());
    };
    if suffix.len() == 16
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err("frontier_id must use the vfr_<16 lowercase hex> form".to_string())
    }
}

fn git_digest_len(format: GitObjectFormat) -> usize {
    match format {
        GitObjectFormat::Sha1 => 40,
        GitObjectFormat::Sha256 => 64,
    }
}

fn require_git_object(field: &str, value: &str, format: GitObjectFormat) -> Result<(), String> {
    let expected = git_digest_len(format);
    if value.len() == expected
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!(
            "{field} must be exactly {expected} lowercase hex digits for {format:?}"
        ))
    }
}

fn bounded_text(value: &str, field: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    if value.len() > max {
        return Err(format!("{field} must be at most {max} UTF-8 bytes"));
    }
    if value.nfc().collect::<String>() != value {
        return Err(format!("{field} must already be Unicode NFC"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} contains a forbidden control character"));
    }
    Ok(())
}

fn validate_repository_path(path: &str, field: &str, max: usize) -> Result<(), String> {
    bounded_text(path, field, max)?;
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(format!(
            "{field} must be a normalized frontier-relative path"
        ));
    }
    Ok(())
}

fn portable_path_key(path: &str) -> String {
    path.nfc().flat_map(char::to_lowercase).collect()
}

fn is_protected_frontier_path(path: &str) -> bool {
    path == "frontier.yaml"
        || path == "frontier.json"
        || path == "vela.lock"
        || path.starts_with(".vela/")
        || path.starts_with("records/receipts/sha256/")
}

fn validate_semver(value: &str) -> Result<(), String> {
    bounded_text(value, "generated_by.version", 128)?;
    let (version_and_pre, build) = value
        .split_once('+')
        .map_or((value, None), |(left, right)| (left, Some(right)));
    if build.is_some_and(|build| {
        build.is_empty()
            || build.contains('+')
            || build.split('.').any(|identifier| {
                identifier.is_empty()
                    || !identifier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
    }) {
        return Err("generated_by.version must be a canonical semantic version".to_string());
    }
    let (core, prerelease) = version_and_pre
        .split_once('-')
        .map_or((version_and_pre, None), |(left, right)| (left, Some(right)));
    if prerelease.is_some_and(|prerelease| {
        prerelease.is_empty()
            || prerelease.split('.').any(|identifier| {
                identifier.is_empty()
                    || !identifier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    || (identifier.bytes().all(|byte| byte.is_ascii_digit())
                        && identifier.len() > 1
                        && identifier.starts_with('0'))
            })
    }) {
        return Err("generated_by.version must be a canonical semantic version".to_string());
    }
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err("generated_by.version must be a canonical semantic version".to_string());
    }
    Ok(())
}

fn scientific_target_punctuation_is_balanced(target: &str) -> bool {
    let mut depth = 0_u8;
    for byte in target.bytes() {
        match byte {
            b'[' => {
                depth = match depth.checked_add(1) {
                    Some(depth) if depth <= 2 => depth,
                    _ => return false,
                };
            }
            b']' => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
            }
            b',' if depth == 0 => return false,
            _ => {}
        }
    }
    depth == 0
}

/// The one external target-ID grammar shared by indexes, campaigns, and
/// coordination leases.
pub fn validate_target_id(target: &str) -> Result<(), String> {
    if target.is_empty() || target.len() > EXTERNAL_TARGET_ID_MAX_BYTES {
        return Err(format!(
            "external target id must be 1..={EXTERNAL_TARGET_ID_MAX_BYTES} bytes"
        ));
    }
    if target.starts_with('-') || target.starts_with("vf_") {
        return Err(
            "external target id must not start with '-' or use the reserved vf_ prefix".to_string(),
        );
    }
    if !target.contains(':')
        || target
            .split(':')
            .any(|segment| segment.is_empty() || segment.starts_with('-'))
        || !target.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b':' | b'.' | b'_' | b'-' | b'[' | b']' | b',')
        })
        || !scientific_target_punctuation_is_balanced(target)
    {
        return Err(
            "external target id must have non-empty, non-option-like ':'-separated segments using ASCII letters, digits, '.', '_', '-', or balanced square-bracket notation"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_state(value: &str) -> Result<(), String> {
    if matches!(value, "open" | "paused" | "blocked" | "done" | "retired") {
        Ok(())
    } else {
        Err(format!("unsupported target state {value:?}"))
    }
}

fn validate_labels(labels: &[String], field: &str) -> Result<(), String> {
    if labels.len() > TARGET_INDEX_MAX_LABELS {
        return Err(format!(
            "{field} has more than {TARGET_INDEX_MAX_LABELS} labels"
        ));
    }
    let mut previous: Option<&str> = None;
    for label in labels {
        bounded_text(label, field, 128)?;
        if let Some(prior) = previous {
            if label == prior {
                return Err(format!("{field} contains duplicate label {label:?}"));
            }
            if label.as_str() < prior {
                return Err(format!("{field} must be sorted in UTF-8 byte order"));
            }
        }
        previous = Some(label);
    }
    Ok(())
}

fn validate_target_common(
    id: &str,
    title: &str,
    why: &str,
    state: &str,
    rank: u64,
    objective: &str,
    labels: &[String],
) -> Result<(), String> {
    validate_target_id(id)?;
    bounded_text(title, "target.title", 512)?;
    bounded_text(why, "target.why", 2_048)?;
    validate_state(state)?;
    if rank > JSON_SAFE_INTEGER_MAX {
        return Err(format!(
            "target.rank must be at most {JSON_SAFE_INTEGER_MAX}"
        ));
    }
    bounded_text(objective, "target.objective", 4_096)?;
    validate_labels(labels, "target.labels")
}

fn validate_target_order<'a>(
    targets: impl IntoIterator<Item = (&'a str, u64)>,
) -> Result<(), String> {
    let mut previous: Option<(u64, &str)> = None;
    for (id, rank) in targets {
        let key = (rank, id);
        if let Some(prior) = previous {
            if key == prior {
                return Err(format!("duplicate target id {id:?}"));
            }
            if key < prior {
                return Err("targets must be sorted by ascending (rank, id)".to_string());
            }
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_unique_target_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(format!("duplicate target id {id:?}"));
        }
    }
    Ok(())
}

impl TargetIndexInputManifestV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1 {
            return Err(format!(
                "inputs.schema must be {TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1}"
            ));
        }
        require_sha256_root("inputs.input_root", &self.input_root)?;
        RetainedObjectManifestV1(self.entries.clone()).validate()?;
        for entry in &self.entries {
            if entry.size > JSON_SAFE_INTEGER_MAX {
                return Err(format!(
                    "input {} size exceeds the JSON safe-integer limit",
                    entry.path
                ));
            }
        }
        if self.computed_root()? != self.input_root {
            return Err(
                "inputs.input_root does not match the canonical input manifest".to_string(),
            );
        }
        Ok(())
    }

    pub fn computed_root(&self) -> Result<String, String> {
        #[derive(Serialize)]
        struct Preimage<'a> {
            schema: &'a str,
            entries: &'a [RetainedObjectEntryV1],
        }
        canonical_root(&Preimage {
            schema: &self.schema,
            entries: &self.entries,
        })
    }
}

impl TargetPacketRefV2 {
    fn validate(&self) -> Result<(), String> {
        bounded_text(&self.schema, "packet.schema", 256)?;
        validate_repository_path(&self.path, "packet.path", 1_024)?;
        if is_protected_frontier_path(&self.path) {
            return Err(format!(
                "packet path {:?} overlaps protected Frontier state",
                self.path
            ));
        }
        if self.size > TARGET_PACKET_MAX_BYTES || self.size > JSON_SAFE_INTEGER_MAX {
            return Err(format!(
                "packet.size must be at most {TARGET_PACKET_MAX_BYTES}"
            ));
        }
        require_sha256_root("packet.sha256", &self.sha256)
    }
}

impl TargetIndexEntryV2 {
    fn validate(&self) -> Result<(), String> {
        validate_target_common(
            &self.id,
            &self.title,
            &self.why,
            &self.state,
            self.rank,
            &self.objective,
            &self.labels,
        )?;
        self.packet.validate()
    }
}

impl TargetIndexV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_INDEX_SCHEMA_V2 {
            return Err(format!("schema must be {TARGET_INDEX_SCHEMA_V2}"));
        }
        require_frontier_id(&self.frontier_id)?;
        require_git_object(
            "source.git_commit",
            &self.source.git_commit,
            self.source.git_object_format,
        )?;
        require_git_object(
            "source.git_tree",
            &self.source.git_tree,
            self.source.git_object_format,
        )?;
        self.inputs.validate()?;
        for (field, root) in [
            ("roots.event_log_root", &self.roots.event_log_root),
            (
                "roots.nonlease_event_log_root",
                &self.roots.nonlease_event_log_root,
            ),
            (
                "roots.scientific_state_root",
                &self.roots.scientific_state_root,
            ),
            ("roots.proposal_root", &self.roots.proposal_root),
            ("roots.identity_root", &self.roots.identity_root),
            ("roots.dependency_root", &self.roots.dependency_root),
            (
                "roots.observed_profile_root",
                &self.roots.observed_profile_root,
            ),
        ] {
            require_sha256_root(field, root)?;
        }
        if self.roots.event_count > JSON_SAFE_INTEGER_MAX {
            return Err(format!(
                "roots.event_count must be at most {JSON_SAFE_INTEGER_MAX}"
            ));
        }
        if self.claim_boundary
            != (TargetIndexClaimBoundaryV2 {
                derived: true,
                authoritative: false,
                deletable: true,
            })
        {
            return Err(
                "claim_boundary must be exactly derived=true, authoritative=false, deletable=true"
                    .to_string(),
            );
        }
        if self.generated_by.program != "vela" {
            return Err("generated_by.program must be `vela`".to_string());
        }
        validate_semver(&self.generated_by.version)?;
        if self.targets.len() > TARGET_INDEX_MAX_TARGETS {
            return Err(format!(
                "target index has {} targets; limit is {TARGET_INDEX_MAX_TARGETS}",
                self.targets.len()
            ));
        }
        for target in &self.targets {
            target.validate()?;
        }
        validate_unique_target_ids(self.targets.iter().map(|target| target.id.as_str()))?;
        validate_target_order(
            self.targets
                .iter()
                .map(|target| (target.id.as_str(), target.rank)),
        )?;

        let packet_paths = self
            .targets
            .iter()
            .map(|target| target.packet.path.as_str())
            .collect::<BTreeSet<_>>();
        let mut packet_portable = BTreeMap::<String, &str>::new();
        for path in &packet_paths {
            let key = portable_path_key(path);
            if let Some(previous) = packet_portable.insert(key, path)
                && previous != *path
            {
                return Err(format!(
                    "packet paths {previous:?} and {path:?} have a portable collision"
                ));
            }
        }
        for input in &self.inputs.entries {
            if input.path == "targets.json"
                || input.path == ".vela/tmp/target-index-candidate.json"
                || packet_paths.contains(input.path.as_str())
            {
                return Err(format!(
                    "input path {:?} is a target-index output or candidate path",
                    input.path
                ));
            }
        }
        require_sha256_root("index_root", &self.index_root)?;
        if self.computed_index_root()? != self.index_root {
            return Err("index_root does not match the canonical index preimage".to_string());
        }
        Ok(())
    }

    pub fn computed_index_root(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "target index did not serialize as an object".to_string())?;
        object.remove("index_root");
        canonical_root(&value)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        canonical::to_canonical_bytes(self).map_err(|error| error.to_string())
    }
}

impl TargetIndexCandidateV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_INDEX_CANDIDATE_SCHEMA_V1 {
            return Err(format!(
                "candidate.schema must be {TARGET_INDEX_CANDIDATE_SCHEMA_V1}"
            ));
        }
        require_frontier_id(&self.frontier_id)?;
        if !matches!(self.source.git_commit.len(), 40 | 64) {
            return Err(
                "candidate.source.git_commit must be a full sha1 or sha256 Git object".to_string(),
            );
        }
        require_git_object(
            "candidate.source.git_commit",
            &self.source.git_commit,
            if self.source.git_commit.len() == 40 {
                GitObjectFormat::Sha1
            } else {
                GitObjectFormat::Sha256
            },
        )?;
        let mut previous_path: Option<&str> = None;
        let mut portable = BTreeSet::new();
        for path in &self.source.input_paths {
            validate_repository_path(path, "candidate.source.input_paths[]", 1_024)?;
            if let Some(previous) = previous_path {
                if path == previous {
                    return Err(format!("duplicate candidate input path {path:?}"));
                }
                if path.as_str() < previous {
                    return Err("candidate input paths must be sorted".to_string());
                }
            }
            previous_path = Some(path);
            if !portable.insert(portable_path_key(path)) {
                return Err(format!(
                    "candidate input path {path:?} has a portable collision"
                ));
            }
        }
        if self.targets.len() > TARGET_INDEX_MAX_TARGETS {
            return Err(format!(
                "candidate has {} targets; limit is {TARGET_INDEX_MAX_TARGETS}",
                self.targets.len()
            ));
        }
        for target in &self.targets {
            validate_target_common(
                &target.id,
                &target.title,
                &target.why,
                &target.state,
                target.rank,
                &target.objective,
                &target.labels,
            )?;
            bounded_text(&target.packet.schema, "candidate.packet.schema", 256)?;
            validate_repository_path(&target.packet.path, "candidate.packet.path", 1_024)?;
            if is_protected_frontier_path(&target.packet.path) {
                return Err(format!(
                    "candidate packet path {:?} overlaps protected Frontier state",
                    target.packet.path
                ));
            }
        }
        validate_unique_target_ids(self.targets.iter().map(|target| target.id.as_str()))?;
        validate_target_order(
            self.targets
                .iter()
                .map(|target| (target.id.as_str(), target.rank)),
        )?;
        let outputs = self
            .targets
            .iter()
            .map(|target| target.packet.path.as_str())
            .collect::<BTreeSet<_>>();
        for input in &self.source.input_paths {
            if input == "targets.json"
                || input == ".vela/tmp/target-index-candidate.json"
                || outputs.contains(input.as_str())
            {
                return Err(format!(
                    "candidate input path {input:?} is a target-index output or candidate path"
                ));
            }
        }
        Ok(())
    }
}

impl TargetIndexMigrationContextV1 {
    pub fn validate(&self) -> Result<FrontierRepositoryBoundaryPayloadV1, String> {
        if self.schema != TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1 {
            return Err(format!(
                "migration context schema must be {TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1}"
            ));
        }
        let object_format = if self.anchor_git_commit.len() == 40 {
            GitObjectFormat::Sha1
        } else if self.anchor_git_commit.len() == 64 {
            GitObjectFormat::Sha256
        } else {
            return Err(
                "migration anchor_git_commit must be a full sha1 or sha256 Git object".to_string(),
            );
        };
        require_git_object(
            "migration.anchor_git_commit",
            &self.anchor_git_commit,
            object_format,
        )?;
        require_git_object(
            "migration.anchor_git_tree",
            &self.anchor_git_tree,
            object_format,
        )?;
        for (field, root) in [
            (
                "migration.source_event_log_root",
                &self.source_event_log_root,
            ),
            (
                "migration.source_nonlease_event_log_root",
                &self.source_nonlease_event_log_root,
            ),
            (
                "migration.planned_boundary_event_content_root",
                &self.planned_boundary_event_content_root,
            ),
            (
                "migration.final_roots.event_log_root",
                &self.final_roots.event_log_root,
            ),
            (
                "migration.final_roots.nonlease_event_log_root",
                &self.final_roots.nonlease_event_log_root,
            ),
            (
                "migration.final_roots.scientific_state_root",
                &self.final_roots.scientific_state_root,
            ),
            (
                "migration.final_roots.proposal_root",
                &self.final_roots.proposal_root,
            ),
            (
                "migration.final_roots.identity_root",
                &self.final_roots.identity_root,
            ),
            (
                "migration.final_roots.dependency_root",
                &self.final_roots.dependency_root,
            ),
            (
                "migration.final_roots.observed_profile_root",
                &self.final_roots.observed_profile_root,
            ),
        ] {
            require_sha256_root(field, root)?;
        }
        if self.source_event_count > JSON_SAFE_INTEGER_MAX
            || self.final_roots.event_count > JSON_SAFE_INTEGER_MAX
        {
            return Err("migration event counts exceed the JSON safe-integer limit".to_string());
        }
        if self.final_roots.event_count != self.source_event_count + 1 {
            return Err(
                "migration final event count must add exactly one repository boundary".to_string(),
            );
        }

        let mut unsigned = self.planned_boundary_event.clone();
        unsigned.signature = None;
        let payload: FrontierRepositoryBoundaryPayloadV1 =
            serde_json::from_value(unsigned.payload.clone())
                .map_err(|error| format!("invalid planned repository-boundary payload: {error}"))?;
        payload.validate()?;
        let expected =
            new_repository_boundary_event(payload.clone(), &unsigned.reason, &unsigned.timestamp)?;
        if serde_json::to_value(&unsigned).map_err(|error| error.to_string())?
            != serde_json::to_value(&expected).map_err(|error| error.to_string())?
        {
            return Err(
                "planned repository boundary does not match its exact constructor-derived core"
                    .to_string(),
            );
        }
        let content_root = sha256_root(&event_content_preimage_bytes(&unsigned));
        if content_root != self.planned_boundary_event_content_root {
            return Err(
                "planned repository boundary content root does not match its exact core"
                    .to_string(),
            );
        }
        if payload.mode != FrontierRepositoryBoundaryMode::TemporalizeExisting
            || payload.trust_mode != FrontierRepositoryTrustMode::Tofu
            || payload.previous_identity_event_root.is_some()
        {
            return Err(
                "target-index migration context requires the first temporalize_existing TOFU boundary"
                    .to_string(),
            );
        }
        if payload.git_object_format != object_format
            || payload.anchor_git_commit != self.anchor_git_commit
            || payload.anchor_git_tree != self.anchor_git_tree
            || payload.anchor_event_log_root != self.source_event_log_root
            || payload.anchor_event_count != self.source_event_count
        {
            return Err(
                "planned repository boundary does not match the migration Git/event anchor"
                    .to_string(),
            );
        }
        if payload.identity_root != self.final_roots.identity_root
            || payload.dependency_root != self.final_roots.dependency_root
            || payload.observed_profile_root != self.final_roots.observed_profile_root
        {
            return Err(
                "planned repository boundary does not match the final target-index identity/profile roots"
                    .to_string(),
            );
        }
        Ok(payload)
    }
}

impl TargetTaskBindingV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_TASK_BINDING_SCHEMA_V1 {
            return Err(format!(
                "binding.schema must be {TARGET_TASK_BINDING_SCHEMA_V1}"
            ));
        }
        require_frontier_id(&self.frontier_id)?;
        validate_target_id(&self.target_id)?;
        require_sha256_root("target_index_root", &self.target_index_root)?;
        require_git_object(
            "source.git_commit",
            &self.source.git_commit,
            self.source.git_object_format,
        )?;
        require_git_object(
            "source.git_tree",
            &self.source.git_tree,
            self.source.git_object_format,
        )?;
        require_sha256_root("input_root", &self.input_root)?;
        self.packet.validate()?;
        for (field, value) in [
            (
                "index_roots.event_log_root",
                &self.index_roots.event_log_root,
            ),
            (
                "index_roots.nonlease_event_log_root",
                &self.index_roots.nonlease_event_log_root,
            ),
            (
                "index_roots.scientific_state_root",
                &self.index_roots.scientific_state_root,
            ),
            ("index_roots.proposal_root", &self.index_roots.proposal_root),
            ("index_roots.identity_root", &self.index_roots.identity_root),
            (
                "index_roots.dependency_root",
                &self.index_roots.dependency_root,
            ),
            (
                "claim_read_set.event_log_root",
                &self.claim_read_set.event_log_root,
            ),
        ] {
            require_sha256_root(field, value)?;
        }
        if self.index_roots.event_count > JSON_SAFE_INTEGER_MAX
            || self.claim_read_set.event_count > JSON_SAFE_INTEGER_MAX
        {
            return Err("binding event counts exceed the JSON safe-integer limit".to_string());
        }
        require_git_object(
            "claim_read_set.git_commit",
            &self.claim_read_set.git_commit,
            self.claim_read_set.git_object_format,
        )?;
        require_git_object(
            "claim_read_set.git_tree",
            &self.claim_read_set.git_tree,
            self.claim_read_set.git_object_format,
        )?;
        require_sha256_root("binding_root", &self.binding_root)?;
        if self.computed_binding_root()? != self.binding_root {
            return Err("binding_root does not match the canonical binding preimage".to_string());
        }
        Ok(())
    }

    pub fn computed_binding_root(&self) -> Result<String, String> {
        let mut value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "target task binding did not serialize as an object".to_string())?;
        object.remove("binding_root");
        canonical_root(&value)
    }
}

/// Build the exact, closed producer-task binding at the claim read edge.
///
/// The target index and packet have already been checked by `assessment`.
/// This final step additionally proves that the loaded Frontier is exactly the
/// Frontier committed at `HEAD`, so the private work session can retain a
/// replayable Git/event read set rather than a mutable worktree observation.
pub fn build_target_task_binding(
    project: &Project,
    repo_path: &Path,
    assessment: &TargetIndexAssessment,
    target_id: &str,
) -> Result<TargetTaskBindingV1, String> {
    let index = assessment
        .v2()
        .ok_or_else(|| "historical target indexes cannot create task bindings".to_string())?;
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        return Err(format!(
            "target task binding refuses stale or invalid target {target_id:?}"
        ));
    }
    let target = index
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("target task binding cannot find target {target_id:?}"))?;
    if target.state != "open" {
        return Err(format!(
            "target task binding requires an open target; {target_id:?} is {}",
            target.state
        ));
    }

    let git_object_format = repository_object_format(repo_path)?;
    let git_commit = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
    require_git_object("claim_read_set.git_commit", &git_commit, git_object_format)?;
    let git_tree = git_text(repo_path, &["rev-parse", "HEAD^{tree}"])?;
    require_git_object("claim_read_set.git_tree", &git_tree, git_object_format)?;
    let (_view, committed, _profile) = materialize_project_at_commit(repo_path, &git_commit)?;
    let event_log_root = format!("sha256:{}", events::event_log_hash(&project.events));
    if committed.frontier_id() != project.frontier_id()
        || committed.events.len() != project.events.len()
        || format!("sha256:{}", events::event_log_hash(&committed.events)) != event_log_root
    {
        return Err(
            "target task binding requires the loaded Frontier event set to match exact HEAD"
                .to_string(),
        );
    }

    let mut binding = TargetTaskBindingV1 {
        schema: TARGET_TASK_BINDING_SCHEMA_V1.to_string(),
        frontier_id: project.frontier_id(),
        target_id: target.id.clone(),
        target_index_root: index.index_root.clone(),
        source: index.source.clone(),
        input_root: index.inputs.input_root.clone(),
        packet: target.packet.clone(),
        index_roots: TargetTaskIndexRootsV1 {
            event_log_root: index.roots.event_log_root.clone(),
            event_count: index.roots.event_count,
            nonlease_event_log_root: index.roots.nonlease_event_log_root.clone(),
            scientific_state_root: index.roots.scientific_state_root.clone(),
            proposal_root: index.roots.proposal_root.clone(),
            identity_root: index.roots.identity_root.clone(),
            dependency_root: index.roots.dependency_root.clone(),
        },
        claim_read_set: TargetTaskClaimReadSetV1 {
            event_log_root,
            event_count: project.events.len() as u64,
            git_object_format,
            git_commit,
            git_tree,
        },
        binding_root: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
    };
    binding.binding_root = binding.computed_binding_root()?;
    binding.validate()?;
    Ok(binding)
}

/// Revalidate a retained task binding at the landing edge.
///
/// Later lease events are allowed, but the original claim read set must still
/// resolve through its exact Git commit and every index, source, input, packet,
/// and repository-context root must still agree with the current assessment.
pub fn revalidate_target_task_binding(
    project: &Project,
    repo_path: &Path,
    binding: &TargetTaskBindingV1,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<(), String> {
    binding.validate()?;
    if binding.frontier_id != project.frontier_id() {
        return Err("target task binding belongs to a different Frontier".to_string());
    }
    let actual_format = repository_object_format(repo_path)?;
    if binding.claim_read_set.git_object_format != actual_format {
        return Err("target task binding Git object format changed".to_string());
    }
    let resolved = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{commit}}", binding.claim_read_set.git_commit),
        ],
    )?;
    if resolved != binding.claim_read_set.git_commit {
        return Err("target task binding claim commit did not resolve exactly".to_string());
    }
    let claim_tree = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{tree}}", binding.claim_read_set.git_commit),
        ],
    )?;
    if claim_tree != binding.claim_read_set.git_tree {
        return Err("target task binding claim tree changed".to_string());
    }
    let head = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
    let ancestor = command(
        repo_path,
        &[
            "merge-base",
            "--is-ancestor",
            &binding.claim_read_set.git_commit,
            &head,
        ],
    )?;
    if !ancestor.status.success() {
        return Err("target task binding claim commit is not an ancestor of HEAD".to_string());
    }
    let source_ancestor = command(
        repo_path,
        &[
            "merge-base",
            "--is-ancestor",
            &binding.source.git_commit,
            &binding.claim_read_set.git_commit,
        ],
    )?;
    if !source_ancestor.status.success() {
        return Err(
            "target task binding source is not an ancestor of its claim read set".to_string(),
        );
    }
    let (_view, claim_project, _profile) =
        materialize_project_at_commit(repo_path, &binding.claim_read_set.git_commit)?;
    if claim_project.frontier_id() != binding.frontier_id
        || claim_project.events.len() as u64 != binding.claim_read_set.event_count
        || format!("sha256:{}", events::event_log_hash(&claim_project.events))
            != binding.claim_read_set.event_log_root
    {
        return Err("target task binding claim read set does not match its Git commit".to_string());
    }

    let assessment = assess_target_index_with_trust_anchor(project, repo_path, trust_anchor)?
        .ok_or_else(|| "target task binding requires targets.json at landing".to_string())?;
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(&binding.target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        return Err("target task binding index or packet is stale at landing".to_string());
    }
    let index = assessment
        .v2()
        .ok_or_else(|| "target task binding cannot resolve through historical v1".to_string())?;
    let target = index
        .targets
        .iter()
        .find(|target| target.id == binding.target_id)
        .ok_or_else(|| "target task binding target is absent at landing".to_string())?;
    let expected_roots = TargetTaskIndexRootsV1 {
        event_log_root: index.roots.event_log_root.clone(),
        event_count: index.roots.event_count,
        nonlease_event_log_root: index.roots.nonlease_event_log_root.clone(),
        scientific_state_root: index.roots.scientific_state_root.clone(),
        proposal_root: index.roots.proposal_root.clone(),
        identity_root: index.roots.identity_root.clone(),
        dependency_root: index.roots.dependency_root.clone(),
    };
    if binding.target_index_root != index.index_root
        || binding.source != index.source
        || binding.input_root != index.inputs.input_root
        || binding.packet != target.packet
        || binding.index_roots != expected_roots
    {
        return Err(
            "target task binding no longer matches its index, source, inputs, packet, or roots"
                .to_string(),
        );
    }
    Ok(())
}

impl TargetIndexV1 {
    fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_INDEX_SCHEMA_V1 {
            return Err(format!("schema must be {TARGET_INDEX_SCHEMA_V1}"));
        }
        require_frontier_id(&self.frontier_id)?;
        for (field, root) in [
            ("as_of.snapshot_hash", &self.as_of.snapshot_hash),
            ("as_of.event_log_hash", &self.as_of.event_log_hash),
            ("as_of.proposal_state_hash", &self.as_of.proposal_state_hash),
        ] {
            require_sha256_root(field, root)?;
        }
        if self.targets.len() > TARGET_INDEX_MAX_TARGETS {
            return Err(format!(
                "target index has {} targets; limit is {TARGET_INDEX_MAX_TARGETS}",
                self.targets.len()
            ));
        }
        for target in &self.targets {
            validate_target_common(
                &target.id,
                &target.title,
                &target.why,
                &target.state,
                target.rank,
                &target.objective,
                &target.labels,
            )?;
            bounded_text(&target.packet.schema, "packet.schema", 256)?;
            validate_repository_path(&target.packet.path, "packet.path", 1_024)?;
            require_sha256_root("packet.sha256", &target.packet.sha256)?;
        }
        validate_unique_target_ids(self.targets.iter().map(|target| target.id.as_str()))
    }
}

fn read_regular_file(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "{label} {} exceeds the {max_bytes}-byte limit",
            path.display()
        ));
    }
    let initial_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("identify {label} {}: {error}", path.display()))?;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("open {label} {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("inspect open {label} {}: {error}", path.display()))?;
    if !opened.is_file() || opened.len() > max_bytes {
        return Err(format!(
            "{label} must remain a regular file within the {max_bytes}-byte limit: {}",
            path.display()
        ));
    }
    let opened_identity = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone open {label} {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("identify open {label} {}: {error}", path.display()))?;
    if initial_identity != opened_identity {
        return Err(format!(
            "{label} changed while it was being opened: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label} {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "{label} {} exceeds the {max_bytes}-byte limit",
            path.display()
        ));
    }
    let named = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reinspect {label} {}: {error}", path.display()))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("reidentify {label} {}: {error}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || opened_identity != final_identity {
        return Err(format!(
            "{label} changed while it was being read: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn command(repo: &Path, args: &[&str]) -> Result<Output, String> {
    super::git_read::hardened_command(repo, "target-index Git repository")?
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitTreeEntry {
    mode: String,
    kind: String,
    object: String,
    path: String,
}

fn parse_tree_record(raw: &[u8]) -> Result<GitTreeEntry, String> {
    let record = std::str::from_utf8(raw)
        .map_err(|error| format!("Git tree contains a non-UTF-8 path: {error}"))?;
    let (metadata, path) = record
        .split_once('\t')
        .ok_or_else(|| format!("malformed git ls-tree record {record:?}"))?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(format!("malformed git ls-tree metadata {metadata:?}"));
    }
    Ok(GitTreeEntry {
        mode: fields[0].to_string(),
        kind: fields[1].to_string(),
        object: fields[2].to_string(),
        path: path.to_string(),
    })
}

fn tree_entries(repo: &Path, commit: &str) -> Result<Vec<GitTreeEntry>, String> {
    let output = git(repo, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    let mut entries = output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(parse_tree_record)
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn tree_entry(repo: &Path, treeish: &str, path: &str) -> Result<Option<GitTreeEntry>, String> {
    validate_repository_path(path, "Git path", 1_024)?;
    let output = git(repo, &["ls-tree", "-z", "--full-tree", treeish, "--", path])?;
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty());
    let Some(first) = records.next() else {
        return Ok(None);
    };
    let parsed = parse_tree_record(first)?;
    if records.next().is_some() || parsed.path != path {
        return Err(format!("Git path lookup for {path:?} was ambiguous"));
    }
    Ok(Some(parsed))
}

fn blob(repo: &Path, entry: &GitTreeEntry) -> Result<Vec<u8>, String> {
    if entry.kind != "blob" {
        return Err(format!(
            "tracked path {} is a Git {}, not a regular blob",
            entry.path, entry.kind
        ));
    }
    git(repo, &["cat-file", "blob", &entry.object])
        .map_err(|error| format!("read Git blob {}: {error}", entry.path))
}

/// Read an ordered set of exact Git blobs through one hardened `cat-file`
/// process.
///
/// Source-frontier materialization previously spawned one Git process per
/// `.vela` file. A real 2,189-event frontier therefore took minutes merely to
/// reconstruct one anchored view. `--batch` preserves the same exact object-ID
/// boundary while keeping process count constant.
fn batch_blobs(repo: &Path, entries: &[&GitTreeEntry]) -> Result<Vec<Vec<u8>>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    for entry in entries {
        if entry.kind != "blob"
            || !matches!(entry.object.len(), 40 | 64)
            || !entry
                .object
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "tracked path {} does not name an exact regular Git blob",
                entry.path
            ));
        }
    }

    let mut child = super::git_read::hardened_command(repo, "target-index Git repository")?
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start batched Git blob reader: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "batched Git blob reader has no stdin".to_string())?;
    let requests = entries
        .iter()
        .map(|entry| (entry.object.clone(), entry.path.clone()))
        .collect::<Vec<_>>();
    let writer = std::thread::spawn(move || {
        for (object, path) in requests {
            writeln!(stdin, "{object}")
                .map_err(|error| format!("request Git blob {path}: {error}"))?;
        }
        Ok::<_, String>(())
    });

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("batched Git blob reader has no stdout".to_string());
        }
    };
    let mut reader = BufReader::new(stdout);
    let mut parsed = (|| {
        let mut blobs = Vec::with_capacity(entries.len());
        let mut total = 0_u64;
        for entry in entries {
            let mut header = String::new();
            reader
                .read_line(&mut header)
                .map_err(|error| format!("read Git blob header for {}: {error}", entry.path))?;
            let fields = header.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 3 || fields[0] != entry.object || fields[1] != "blob" {
                return Err(format!(
                    "Git blob header for {} did not match the requested object",
                    entry.path
                ));
            }
            let size = fields[2]
                .parse::<u64>()
                .map_err(|error| format!("parse Git blob size for {}: {error}", entry.path))?;
            if size > SOURCE_VIEW_BLOB_MAX_BYTES {
                return Err(format!(
                    "source Git blob {} is {size} bytes; limit is {SOURCE_VIEW_BLOB_MAX_BYTES}",
                    entry.path
                ));
            }
            total = total
                .checked_add(size)
                .ok_or_else(|| "source Git blob bytes overflowed u64".to_string())?;
            if total > SOURCE_VIEW_TOTAL_MAX_BYTES {
                return Err(format!(
                    "source Frontier view exceeds {SOURCE_VIEW_TOTAL_MAX_BYTES} bytes"
                ));
            }
            let length = usize::try_from(size)
                .map_err(|_| format!("Git blob {} does not fit in memory", entry.path))?;
            let mut bytes = vec![0_u8; length];
            reader
                .read_exact(&mut bytes)
                .map_err(|error| format!("read Git blob {}: {error}", entry.path))?;
            let mut terminator = [0_u8; 1];
            reader
                .read_exact(&mut terminator)
                .map_err(|error| format!("read Git blob terminator for {}: {error}", entry.path))?;
            if terminator[0] != b'\n' {
                return Err(format!(
                    "Git blob {} has an invalid batch terminator",
                    entry.path
                ));
            }
            blobs.push(bytes);
        }
        Ok(blobs)
    })();
    if parsed.is_err() {
        let _ = child.kill();
    }
    let writer_result = writer
        .join()
        .map_err(|_| "batched Git blob request writer panicked".to_string())?;
    if let Err(error) = writer_result {
        if parsed.is_ok() {
            parsed = Err(error);
        }
        let _ = child.kill();
    }
    let mut stderr = String::new();
    if let Some(mut stream) = child.stderr.take() {
        stream
            .read_to_string(&mut stderr)
            .map_err(|error| format!("read batched Git blob error output: {error}"))?;
    }
    let status = child
        .wait()
        .map_err(|error| format!("wait for batched Git blob reader: {error}"))?;
    if !status.success() {
        return Err(if stderr.trim().is_empty() {
            format!("batched Git blob reader failed with {status}")
        } else {
            stderr.trim().to_string()
        });
    }
    parsed
}

fn repository_object_format(repo: &Path) -> Result<GitObjectFormat, String> {
    match git_text(repo, &["rev-parse", "--show-object-format"])?.as_str() {
        "sha1" => Ok(GitObjectFormat::Sha1),
        "sha256" => Ok(GitObjectFormat::Sha256),
        other => Err(format!("unsupported Git object format {other:?}")),
    }
}

fn parse_index_record(raw: &[u8]) -> Result<GitTreeEntry, String> {
    let record = std::str::from_utf8(raw)
        .map_err(|error| format!("Git index contains a non-UTF-8 path: {error}"))?;
    let (metadata, path) = record
        .split_once('\t')
        .ok_or_else(|| format!("malformed Git index record {record:?}"))?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3 || fields[2] != "0" {
        return Err(format!(
            "Git index record for {path:?} is not an exact stage-0 entry"
        ));
    }
    validate_repository_path(path, "Git index path", 4_096)?;
    Ok(GitTreeEntry {
        mode: fields[0].to_string(),
        kind: if fields[0] == "160000" {
            "commit".to_string()
        } else {
            "blob".to_string()
        },
        object: fields[1].to_string(),
        path: path.to_string(),
    })
}

fn index_entry(repo: &Path, path: &str) -> Result<Option<GitTreeEntry>, String> {
    let output = git(repo, &["ls-files", "--stage", "-z", "--", path])?;
    let mut records = output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty());
    let Some(raw) = records.next() else {
        return Ok(None);
    };
    if records.next().is_some() {
        return Err(format!("Git index has multiple stages for {path:?}"));
    }
    let parsed = parse_index_record(raw)?;
    if parsed.path != path {
        return Err(format!(
            "Git index record for {path:?} is not an exact stage-0 entry"
        ));
    }
    Ok(Some(parsed))
}

fn exact_tracked_head_bytes(repo: &Path, path: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let head =
        tree_entry(repo, "HEAD", path)?.ok_or_else(|| format!("{path:?} is absent from HEAD"))?;
    let staged =
        index_entry(repo, path)?.ok_or_else(|| format!("{path:?} is absent from the Git index"))?;
    if !matches!(head.mode.as_str(), "100644" | "100755")
        || head.kind != "blob"
        || staged.mode != head.mode
        || staged.object != head.object
    {
        return Err(format!(
            "{path:?} must be an unchanged tracked regular file in HEAD and the Git index"
        ));
    }
    let worktree = safe_worktree_file(repo, path, max_bytes)?;
    if blob(repo, &head)? != worktree {
        return Err(format!("{path:?} working bytes do not match HEAD"));
    }
    Ok(worktree)
}

fn safe_worktree_file(repo_path: &Path, relative: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    validate_repository_path(relative, "packet.path", 1_024)?;
    let mut cursor = repo_path.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            unreachable!("packet path was validated above");
        };
        cursor.push(component);
        let metadata = std::fs::symlink_metadata(&cursor)
            .map_err(|error| format!("inspect packet path {}: {error}", cursor.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "packet path must not contain symlinks: {}",
                cursor.display()
            ));
        }
    }
    let root = std::fs::canonicalize(repo_path)
        .map_err(|error| format!("resolve Frontier {}: {error}", repo_path.display()))?;
    let resolved = std::fs::canonicalize(repo_path.join(relative))
        .map_err(|error| format!("resolve packet {relative:?}: {error}"))?;
    if !resolved.starts_with(root) {
        return Err(format!("packet path {relative:?} escapes the Frontier"));
    }
    read_regular_file(&repo_path.join(relative), max_bytes, "target packet")
}

#[derive(Debug, Clone)]
struct EffectiveRepositoryRoots {
    profile_root: String,
    identity_root: String,
    dependency_root: String,
    scientific_state_root: String,
    proposal_root: String,
    event_log_root: String,
    event_count: u64,
    nonlease_event_log_root: String,
}

fn profile_at_path(path: &Path) -> Result<FrontierProfileV1, String> {
    let bytes = read_regular_file(path, 1024 * 1024, "Frontier Profile")?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("Frontier Profile is not UTF-8: {error}"))?;
    FrontierProfileV1::from_yaml_str(text)
}

fn latest_boundary(events: &[StateEvent]) -> Result<Option<&StateEvent>, String> {
    let errors = validate_repository_boundary_event_set(events);
    if !errors.is_empty() {
        return Err(format!(
            "repository identity event set is invalid: {}",
            errors.join(" | ")
        ));
    }
    let boundaries = events
        .iter()
        .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
        .map(|event| {
            let root = repository_boundary_event_content_root(event)?;
            let payload = repository_boundary_payload_from_event_shape(event)?;
            Ok((root, payload, event))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if boundaries.is_empty() {
        return Ok(None);
    }
    let referenced = boundaries
        .iter()
        .filter_map(|(_, payload, _)| payload.previous_identity_event_root.clone())
        .collect::<BTreeSet<_>>();
    let leaves = boundaries
        .iter()
        .filter(|(root, _, _)| !referenced.contains(root))
        .map(|(_, _, event)| *event)
        .collect::<Vec<_>>();
    match leaves.as_slice() {
        [leaf] => Ok(Some(*leaf)),
        _ => Err(format!(
            "repository identity boundary graph has {} terminal events",
            leaves.len()
        )),
    }
}

fn derive_effective_roots(
    project: &Project,
    repo_path: &Path,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<EffectiveRepositoryRoots, String> {
    let profile = profile_at_path(&repo_path.join("frontier.yaml"))?;
    derive_effective_roots_with_profile(project, repo_path, &profile, trust_anchor)
}

fn derive_effective_roots_with_profile(
    project: &Project,
    git_repo_path: &Path,
    profile: &FrontierProfileV1,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<EffectiveRepositoryRoots, String> {
    profile.assert_frontier_id(&project.frontier_id())?;
    let profile_root = profile.profile_root()?;

    let (identity_root, dependency_root) = if let Some(boundary) = latest_boundary(&project.events)?
    {
        verify_repository_boundary_context_with_trust_anchor(
            project,
            git_repo_path,
            boundary,
            trust_anchor,
        )?;
        let payload: FrontierRepositoryBoundaryPayloadV1 =
            repository_boundary_payload_from_event_shape(boundary)?;
        (payload.identity_root, payload.dependency_root)
    } else {
        let genesis = project
            .events
            .iter()
            .filter(|event| event.kind.as_str() == "frontier.created")
            .collect::<Vec<_>>();
        let [genesis] = genesis.as_slice() else {
            return Err(format!(
                "Profile v1 requires exactly one frontier.created event when no repository boundary exists; found {}",
                genesis.len()
            ));
        };
        let identity = FrontierIdentityV1::from_genesis_event(genesis)?;
        if identity.frontier_id != project.frontier_id() {
            return Err("frontier.created identity does not match the loaded Frontier".to_string());
        }
        (identity.root()?, exact_dependency_root(&[])?)
    };
    let scientific_state_root = vela_protocol::scientific_state::scientific_state_root_v2(
        project,
        &identity_root,
        &dependency_root,
    )?;
    Ok(EffectiveRepositoryRoots {
        profile_root,
        identity_root,
        dependency_root,
        scientific_state_root,
        proposal_root: format!(
            "sha256:{}",
            proposals::proposal_state_hash(&project.proposals)
        ),
        event_log_root: format!("sha256:{}", events::event_log_hash(&project.events)),
        event_count: project.events.len() as u64,
        nonlease_event_log_root: format!(
            "sha256:{}",
            events::nonlease_event_log_hash(&project.events)
        ),
    })
}

fn materialize_project_at_commit(
    repo_path: &Path,
    commit: &str,
) -> Result<(tempfile::TempDir, Project, FrontierProfileV1), String> {
    let (temporary, project) = materialize_project_only_at_commit(repo_path, commit)?;
    let profile = profile_at_path(&temporary.path().join("frontier.yaml"))?;
    Ok((temporary, project, profile))
}

fn materialize_project_only_at_commit(
    repo_path: &Path,
    commit: &str,
) -> Result<(tempfile::TempDir, Project), String> {
    let entries = tree_entries(repo_path, commit)?;
    let project_entries = entries
        .iter()
        .filter(|entry| entry.path == "frontier.yaml" || entry.path.starts_with(".vela/"))
        .collect::<Vec<_>>();
    let blobs = batch_blobs(repo_path, &project_entries)?;
    let temporary =
        tempfile::tempdir().map_err(|error| format!("create target-index source view: {error}"))?;
    for (entry, bytes) in project_entries.into_iter().zip(blobs) {
        if !matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob" {
            return Err(format!(
                "source project input {} must be a tracked regular file",
                entry.path
            ));
        }
        let target = temporary.path().join(&entry.path);
        let parent = target
            .parent()
            .ok_or_else(|| format!("source path {} has no parent", entry.path))?;
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create source path {}: {error}", parent.display()))?;
        std::fs::write(&target, bytes)
            .map_err(|error| format!("write source path {}: {error}", target.display()))?;
    }
    let project = repo::load_from_path(temporary.path())
        .map_err(|error| format!("load target-index source Frontier: {error}"))?;
    Ok((temporary, project))
}

fn source_frontier_schema(repo_path: &Path, commit: &str) -> Result<Option<String>, String> {
    let Some(entry) = tree_entry(repo_path, commit, "frontier.yaml")? else {
        return Ok(None);
    };
    if !matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob" {
        return Err("source frontier.yaml must be a tracked regular file".to_string());
    }
    let bytes = blob(repo_path, &entry)?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&bytes)
        .map_err(|error| format!("source frontier.yaml is invalid YAML: {error}"))?;
    Ok(value
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("schema".to_string())))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string))
}

fn exact_candidate_path(repo_path: &Path, candidate_path: &Path) -> PathBuf {
    if candidate_path.is_absolute() {
        candidate_path.to_path_buf()
    } else {
        repo_path.join(candidate_path)
    }
}

fn repo_relative_existing_path(repo_path: &Path, path: &Path) -> Result<Option<String>, String> {
    let repo = std::fs::canonicalize(repo_path)
        .map_err(|error| format!("resolve Frontier {}: {error}", repo_path.display()))?;
    let path = std::fs::canonicalize(path)
        .map_err(|error| format!("resolve {}: {error}", path.display()))?;
    let Ok(relative) = path.strip_prefix(repo) else {
        return Ok(None);
    };
    let relative = relative
        .to_str()
        .ok_or_else(|| "candidate path inside the Frontier is not UTF-8".to_string())?
        .replace('\\', "/");
    validate_repository_path(&relative, "candidate path", 1_024)?;
    Ok(Some(relative))
}

fn ensure_only_allowed_seal_dirt(
    repo_path: &Path,
    allowed: &BTreeSet<String>,
) -> Result<(), String> {
    let unexpected = super::git_read::dirty_worktree_paths(repo_path, true)?
        .into_iter()
        .filter(|path| !allowed.contains(path))
        .collect::<Vec<_>>();
    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "target-index sealing refuses unrelated worktree dirt: {}",
            unexpected.join(", ")
        ))
    }
}

fn source_input_entry(
    repo_path: &Path,
    source_commit: &str,
    path: &str,
) -> Result<RetainedObjectEntryV1, String> {
    let entry = tree_entry(repo_path, source_commit, path)?
        .ok_or_else(|| format!("candidate input path {path:?} is absent from the source commit"))?;
    if !matches!(entry.mode.as_str(), "100644" | "100755") || entry.kind != "blob" {
        return Err(format!(
            "candidate input path {path:?} must be a tracked regular blob"
        ));
    }
    let bytes = blob(repo_path, &entry)?;
    Ok(RetainedObjectEntryV1 {
        path: path.to_string(),
        git_mode: entry.mode,
        size: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    })
}

enum TargetIndexSealContext<'a> {
    Current {
        trust_anchor: Option<&'a RepositoryTrustAnchor>,
    },
    LegacyMigration(&'a TargetIndexMigrationContextV1),
}

fn migration_effective_roots(
    repo_path: &Path,
    current: &Project,
    source: &TargetIndexSourceV2,
    context: &TargetIndexMigrationContextV1,
) -> Result<EffectiveRepositoryRoots, String> {
    let payload = context.validate()?;
    if source.git_commit != context.anchor_git_commit
        || source.git_tree != context.anchor_git_tree
        || source.git_object_format != payload.git_object_format
    {
        return Err(
            "candidate source does not match the exact legacy migration anchor".to_string(),
        );
    }
    let head = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
    let head_tree = git_text(repo_path, &["rev-parse", "HEAD^{tree}"])?;
    if head != context.anchor_git_commit || head_tree != context.anchor_git_tree {
        return Err(
            "legacy migration target-index seal requires the exact anchored HEAD commit and tree"
                .to_string(),
        );
    }
    if source_frontier_schema(repo_path, &source.git_commit)?.as_deref()
        != Some(vela_protocol::frontier_repo::FRONTIER_MANIFEST_SCHEMA)
    {
        return Err(
            "legacy migration target-index seal requires an exact v0.1 Frontier manifest source"
                .to_string(),
        );
    }
    let (_source_view, source_project) =
        materialize_project_only_at_commit(repo_path, &source.git_commit)?;
    if source_project.frontier_id() != current.frontier_id()
        || !source_events_are_retained(&source_project, current)?
        || source_project.events.len() != current.events.len()
    {
        return Err(
            "legacy migration source does not match the exact current Frontier event set"
                .to_string(),
        );
    }

    let source_event_log_root =
        format!("sha256:{}", events::event_log_hash(&source_project.events));
    let source_nonlease_event_log_root = format!(
        "sha256:{}",
        events::nonlease_event_log_hash(&source_project.events)
    );
    if context.source_event_log_root != source_event_log_root
        || context.source_event_count != source_project.events.len() as u64
        || context.source_nonlease_event_log_root != source_nonlease_event_log_root
    {
        return Err(
            "legacy migration context does not match the exact source event roots".to_string(),
        );
    }

    let source_proposal_root = format!(
        "sha256:{}",
        proposals::proposal_state_hash(&source_project.proposals)
    );
    if payload.anchor_proposal_root != source_proposal_root
        || context.final_roots.proposal_root != source_proposal_root
    {
        return Err(
            "legacy migration must preserve the exact proposal root in the target-index seal"
                .to_string(),
        );
    }

    let mut after = source_project;
    after.events.push(context.planned_boundary_event.clone());
    let derived_final = TargetIndexRootsV2 {
        event_log_root: format!("sha256:{}", events::event_log_hash(&after.events)),
        event_count: after.events.len() as u64,
        nonlease_event_log_root: format!(
            "sha256:{}",
            events::nonlease_event_log_hash(&after.events)
        ),
        scientific_state_root: vela_protocol::scientific_state::scientific_state_root_v2(
            &after,
            &payload.identity_root,
            &payload.dependency_root,
        )?,
        proposal_root: source_proposal_root,
        identity_root: payload.identity_root,
        dependency_root: payload.dependency_root,
        observed_profile_root: payload.observed_profile_root,
    };
    if context.final_roots != derived_final {
        return Err(
            "legacy migration final target-index roots do not match the one planned boundary delta"
                .to_string(),
        );
    }
    Ok(EffectiveRepositoryRoots {
        profile_root: derived_final.observed_profile_root,
        identity_root: derived_final.identity_root,
        dependency_root: derived_final.dependency_root,
        scientific_state_root: derived_final.scientific_state_root,
        proposal_root: derived_final.proposal_root,
        event_log_root: derived_final.event_log_root,
        event_count: derived_final.event_count,
        nonlease_event_log_root: derived_final.nonlease_event_log_root,
    })
}

/// Derive a complete Target Index v2 from one closed domain-owned candidate.
///
/// This function performs no writes. Packet paths are read exactly once and
/// the source inputs are read from the named immutable Git object rather than
/// from mutable worktree bytes.
pub fn prepare_target_index_seal(
    repo_path: &Path,
    candidate_path: &Path,
    binary_version: &str,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<TargetIndexSealPlan, String> {
    prepare_target_index_seal_with_context(
        repo_path,
        candidate_path,
        binary_version,
        TargetIndexSealContext::Current { trust_anchor },
    )
}

/// Derive the same closed Target Index v2 during the one protected legacy
/// Profile migration, before any signing key is read.
///
/// This function is write-free. It never reads target semantics from a legacy
/// `targets.json`: the caller must supply the external closed candidate path,
/// whose exact byte root is returned in the seal plan. The special context
/// permits exactly one planned `frontier.repository_bound` delta and no other
/// event, proposal, or source-root change.
pub fn prepare_target_index_seal_for_migration(
    repo_path: &Path,
    candidate_path: &Path,
    binary_version: &str,
    context: &TargetIndexMigrationContextV1,
) -> Result<TargetIndexSealPlan, String> {
    prepare_target_index_seal_with_context(
        repo_path,
        candidate_path,
        binary_version,
        TargetIndexSealContext::LegacyMigration(context),
    )
}

fn prepare_target_index_seal_with_context(
    repo_path: &Path,
    candidate_path: &Path,
    binary_version: &str,
    seal_context: TargetIndexSealContext<'_>,
) -> Result<TargetIndexSealPlan, String> {
    validate_semver(binary_version)?;
    let candidate_path = exact_candidate_path(repo_path, candidate_path);
    let candidate_bytes = read_regular_file(
        &candidate_path,
        TARGET_INDEX_JSON_MAX_BYTES,
        "target-index candidate",
    )?;
    let candidate_value: Value = serde_json::from_slice(&candidate_bytes)
        .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse candidate: {error}"))?;
    let candidate: TargetIndexCandidateV1 = serde_json::from_value(candidate_value)
        .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse candidate: {error}"))?;
    candidate.validate()?;
    let candidate_root = sha256_root(&candidate_bytes);

    let mut allowed_dirty_paths = candidate
        .targets
        .iter()
        .map(|target| target.packet.path.clone())
        .collect::<BTreeSet<_>>();
    allowed_dirty_paths.insert("targets.json".to_string());
    let candidate_display = match repo_relative_existing_path(repo_path, &candidate_path)? {
        Some(path) => {
            allowed_dirty_paths.insert(path.clone());
            path
        }
        None => candidate_path.display().to_string(),
    };
    ensure_only_allowed_seal_dirt(repo_path, &allowed_dirty_paths)?;

    let current = repo::load_from_path(repo_path)
        .map_err(|error| format!("load Frontier for target-index seal: {error}"))?;
    if candidate.frontier_id != current.frontier_id() {
        return Err(format!(
            "{CODE_FRONTIER_MISMATCH}: candidate Frontier {} differs from loaded {}",
            candidate.frontier_id,
            current.frontier_id()
        ));
    }

    let git_object_format = repository_object_format(repo_path)?;
    require_git_object(
        "candidate.source.git_commit",
        &candidate.source.git_commit,
        git_object_format,
    )?;
    let resolved = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{commit}}", candidate.source.git_commit),
        ],
    )
    .map_err(|error| format!("{CODE_SOURCE_UNAVAILABLE}: {error}"))?;
    if resolved != candidate.source.git_commit {
        return Err(format!(
            "{CODE_SOURCE_UNAVAILABLE}: candidate source did not resolve to the exact object"
        ));
    }
    let head = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
    let ancestor = command(
        repo_path,
        &[
            "merge-base",
            "--is-ancestor",
            &candidate.source.git_commit,
            &head,
        ],
    )?;
    if !ancestor.status.success() {
        return Err(format!(
            "{CODE_SOURCE_NOT_ANCESTOR}: candidate source is not an ancestor of HEAD"
        ));
    }
    let git_tree = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{tree}}", candidate.source.git_commit),
        ],
    )?;
    let source = TargetIndexSourceV2 {
        git_object_format,
        git_commit: candidate.source.git_commit.clone(),
        git_tree,
    };

    let effective = match seal_context {
        TargetIndexSealContext::Current { trust_anchor } => {
            let (_source_view, source_project, source_profile) =
                materialize_project_at_commit(repo_path, &source.git_commit)?;
            if source_project.frontier_id() != candidate.frontier_id {
                return Err(format!(
                    "{CODE_FRONTIER_MISMATCH}: source Frontier {} differs from candidate {}",
                    source_project.frontier_id(),
                    candidate.frontier_id
                ));
            }
            let effective = derive_effective_roots_with_profile(
                &source_project,
                repo_path,
                &source_profile,
                trust_anchor,
            )?;
            if !source_events_are_retained(&source_project, &current)? {
                return Err(format!(
                    "{CODE_EVENT_ROOT_MISMATCH}: source event set is not retained byte-for-byte in the current Frontier"
                ));
            }
            let current_effective = derive_effective_roots(&current, repo_path, trust_anchor)?;
            for (code, label, source_value, current_value) in [
                (
                    CODE_EVENT_ROOT_MISMATCH,
                    "non-lease event root",
                    effective.nonlease_event_log_root.as_str(),
                    current_effective.nonlease_event_log_root.as_str(),
                ),
                (
                    CODE_STATE_ROOT_MISMATCH,
                    "scientific-state root",
                    effective.scientific_state_root.as_str(),
                    current_effective.scientific_state_root.as_str(),
                ),
                (
                    CODE_PROPOSAL_ROOT_MISMATCH,
                    "proposal root",
                    effective.proposal_root.as_str(),
                    current_effective.proposal_root.as_str(),
                ),
                (
                    CODE_IDENTITY_ROOT_MISMATCH,
                    "identity root",
                    effective.identity_root.as_str(),
                    current_effective.identity_root.as_str(),
                ),
                (
                    CODE_DEPENDENCY_ROOT_MISMATCH,
                    "dependency root",
                    effective.dependency_root.as_str(),
                    current_effective.dependency_root.as_str(),
                ),
            ] {
                if source_value != current_value {
                    return Err(format!(
                        "{code}: candidate source {label} differs from the current Frontier"
                    ));
                }
            }
            effective
        }
        TargetIndexSealContext::LegacyMigration(context) => {
            migration_effective_roots(repo_path, &current, &source, context)?
        }
    };

    let entries = candidate
        .source
        .input_paths
        .iter()
        .map(|path| source_input_entry(repo_path, &source.git_commit, path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut inputs = TargetIndexInputManifestV1 {
        schema: TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1.to_string(),
        input_root: sha256_root(&[]),
        entries,
    };
    inputs.input_root = inputs.computed_root()?;

    let mut targets = Vec::with_capacity(candidate.targets.len());
    for target in &candidate.targets {
        let packet_bytes =
            safe_worktree_file(repo_path, &target.packet.path, TARGET_PACKET_MAX_BYTES)?;
        let packet_value: Value = serde_json::from_slice(&packet_bytes).map_err(|error| {
            format!(
                "{CODE_PACKET_MISMATCH}: packet {:?} is not valid JSON: {error}",
                target.packet.path
            )
        })?;
        if !packet_value.is_object()
            || packet_value.get("schema").and_then(Value::as_str)
                != Some(target.packet.schema.as_str())
        {
            return Err(format!(
                "{CODE_PACKET_MISMATCH}: packet {:?} must be one object with schema {:?}",
                target.packet.path, target.packet.schema
            ));
        }
        targets.push(TargetIndexEntryV2 {
            id: target.id.clone(),
            title: target.title.clone(),
            why: target.why.clone(),
            state: target.state.clone(),
            rank: target.rank,
            objective: target.objective.clone(),
            labels: target.labels.clone(),
            packet: TargetPacketRefV2 {
                schema: target.packet.schema.clone(),
                path: target.packet.path.clone(),
                size: packet_bytes.len() as u64,
                sha256: sha256_root(&packet_bytes),
            },
        });
    }

    let mut index = TargetIndexV2 {
        schema: TARGET_INDEX_SCHEMA_V2.to_string(),
        frontier_id: candidate.frontier_id,
        source: source.clone(),
        inputs,
        roots: TargetIndexRootsV2 {
            event_log_root: effective.event_log_root,
            event_count: effective.event_count,
            nonlease_event_log_root: effective.nonlease_event_log_root,
            scientific_state_root: effective.scientific_state_root,
            proposal_root: effective.proposal_root,
            identity_root: effective.identity_root,
            dependency_root: effective.dependency_root,
            observed_profile_root: effective.profile_root,
        },
        claim_boundary: TargetIndexClaimBoundaryV2 {
            derived: true,
            authoritative: false,
            deletable: true,
        },
        generated_by: TargetIndexGeneratorV2 {
            program: "vela".to_string(),
            version: binary_version.to_string(),
        },
        targets,
        index_root: sha256_root(&[]),
    };
    index.index_root = index.computed_index_root()?;
    let canonical_bytes = index.canonical_bytes()?;
    if let Some(source_index) = tree_entry(repo_path, &source.git_commit, "targets.json")?
        && blob(repo_path, &source_index)? == canonical_bytes
    {
        return Err(format!(
            "{CODE_SOURCE_SELF_REFERENCE}: source tree already contains the exact sealed targets.json bytes"
        ));
    }
    let canonical_json = String::from_utf8(canonical_bytes)
        .map_err(|error| format!("canonical target index is not UTF-8: {error}"))?;
    let packet_paths = index
        .targets
        .iter()
        .map(|target| target.packet.path.clone())
        .collect::<Vec<_>>();
    let input_paths = index
        .inputs
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    Ok(TargetIndexSealPlan {
        schema: "vela.target-index-seal-plan.v1",
        frontier_id: index.frontier_id.clone(),
        candidate_path: candidate_display,
        candidate_root,
        source,
        input_paths,
        packet_paths,
        index_path: "targets.json",
        index_root: index.index_root.clone(),
        canonical_json,
        index,
        touched_paths: vec!["targets.json".to_string()],
        allowed_dirty_paths,
    })
}

/// A target-index replacement whose repository root, parent, and exact
/// present/absent preimage were pinned before the Profile v1 write gate ran.
#[derive(Debug)]
pub struct PreparedTargetIndexSealInstall {
    replacement: PreparedRepositoryFileReplacement,
}

impl PreparedTargetIndexSealInstall {
    /// Install the prepared target index through the pinned descriptor edge.
    pub fn install(self) -> Result<bool, String> {
        self.replacement.install()
    }

    /// Deterministic race-test/cancellation hook immediately before the final
    /// named-path and preimage revalidation.
    pub fn install_with_hook(
        self,
        before_replace: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        self.replacement.install_with_hook(before_replace)
    }
}

/// Pin the target-index destination and its exact preimage before the
/// repository write gate is evaluated.
pub fn prepare_target_index_seal_install(
    repo_path: &Path,
    plan: &TargetIndexSealPlan,
) -> Result<PreparedTargetIndexSealInstall, String> {
    ensure_only_allowed_seal_dirt(repo_path, &plan.allowed_dirty_paths)?;
    let bytes = plan.canonical_json.as_bytes();
    let replacement = PreparedRepositoryFileReplacement::prepare_observed(
        repo_path,
        Path::new(plan.index_path),
        bytes,
        RepositoryFileReplacementMode::Exact(0o644),
        TARGET_INDEX_JSON_MAX_BYTES,
    )?;
    Ok(PreparedTargetIndexSealInstall { replacement })
}

/// Atomically install a previously derived seal plan. Production CLI callers
/// prepare before the Profile v1 write gate; this convenience wrapper retains
/// the same sound edge for direct library callers.
pub fn install_target_index_seal(
    repo_path: &Path,
    plan: &TargetIndexSealPlan,
) -> Result<bool, String> {
    prepare_target_index_seal_install(repo_path, plan)?.install()
}

fn event_map(project: &Project) -> Result<BTreeMap<&str, Vec<u8>>, String> {
    let mut map = BTreeMap::new();
    for event in &project.events {
        let bytes = canonical::to_canonical_bytes(event).map_err(|error| error.to_string())?;
        if map.insert(event.id.as_str(), bytes).is_some() {
            return Err(format!("duplicate event id {}", event.id));
        }
    }
    Ok(map)
}

fn source_events_are_retained(source: &Project, current: &Project) -> Result<bool, String> {
    let source = event_map(source)?;
    let current = event_map(current)?;
    Ok(source
        .iter()
        .all(|(id, bytes)| current.get(id).is_some_and(|current| current == bytes)))
}

fn issue(code: &'static str, message: impl Into<String>) -> TargetIndexIssue {
    TargetIndexIssue {
        code,
        target_id: None,
        message: message.into(),
    }
}

fn target_issue(
    code: &'static str,
    target_id: &str,
    message: impl Into<String>,
) -> TargetIndexIssue {
    TargetIndexIssue {
        code,
        target_id: Some(target_id.to_string()),
        message: message.into(),
    }
}

fn sort_issues(issues: &mut Vec<TargetIndexIssue>) {
    issues.sort();
    issues.dedup();
}

fn push_target_issue(
    issues: &mut BTreeMap<String, Vec<TargetIndexIssue>>,
    target_id: &str,
    code: &'static str,
    message: impl Into<String>,
) {
    issues
        .entry(target_id.to_string())
        .or_default()
        .push(target_issue(code, target_id, message));
}

fn validate_input_git_bytes(
    repo_path: &Path,
    index: &TargetIndexV2,
) -> Result<Vec<TargetIndexIssue>, String> {
    let mut issues = Vec::new();
    let input_root = index.inputs.computed_root()?;
    if input_root != index.inputs.input_root {
        issues.push(issue(
            CODE_INPUT_ROOT_MISMATCH,
            format!(
                "declared input root {} differs from derived {input_root}",
                index.inputs.input_root
            ),
        ));
    }
    for declared in &index.inputs.entries {
        let source = tree_entry(repo_path, &index.source.git_commit, &declared.path)?;
        let head = tree_entry(repo_path, "HEAD", &declared.path)?;
        let Some(source) = source else {
            issues.push(issue(
                CODE_INPUT_ROOT_MISMATCH,
                format!("declared source input {:?} is absent", declared.path),
            ));
            continue;
        };
        if !matches!(source.mode.as_str(), "100644" | "100755") || source.kind != "blob" {
            issues.push(issue(
                CODE_INVALID_PATH,
                format!(
                    "declared source input {:?} is not a regular blob",
                    declared.path
                ),
            ));
            continue;
        }
        let bytes = blob(repo_path, &source)?;
        let matches_declaration = source.mode == declared.git_mode
            && bytes.len() as u64 == declared.size
            && hex::encode(Sha256::digest(&bytes)) == declared.sha256;
        if !matches_declaration {
            issues.push(issue(
                CODE_INPUT_ROOT_MISMATCH,
                format!(
                    "declared source input {:?} does not match Git",
                    declared.path
                ),
            ));
        }
        if head.as_ref().is_none_or(|entry| {
            entry.mode != source.mode || entry.kind != source.kind || entry.object != source.object
        }) {
            issues.push(issue(
                CODE_INPUT_ROOT_MISMATCH,
                format!(
                    "current HEAD changed declared source input {:?}",
                    declared.path
                ),
            ));
        }
    }
    Ok(issues)
}

fn is_exact_legacy_migration_source(
    project: &Project,
    index: &TargetIndexV2,
    source_project: &Project,
    effective: &EffectiveRepositoryRoots,
) -> Result<bool, String> {
    if source_project.frontier_id() != project.frontier_id()
        || !source_events_are_retained(source_project, project)?
    {
        return Ok(false);
    }
    let source_ids = source_project
        .events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<BTreeSet<_>>();
    let delta = project
        .events
        .iter()
        .filter(|event| !source_ids.contains(event.id.as_str()))
        .collect::<Vec<_>>();
    if delta.iter().any(|event| {
        event.kind.as_str() != EVENT_KIND_FRONTIER_REPOSITORY_BOUND
            && event.kind.as_str() != events::EVENT_KIND_ATTEMPT_CLAIMED
    }) {
        return Ok(false);
    }
    let boundaries = delta
        .iter()
        .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
        .copied()
        .collect::<Vec<_>>();
    let [boundary] = boundaries.as_slice() else {
        return Ok(false);
    };
    let payload = match repository_boundary_payload_from_event_shape(boundary) {
        Ok(payload) => payload,
        Err(_) => return Ok(false),
    };
    if payload.mode != FrontierRepositoryBoundaryMode::TemporalizeExisting
        || payload.trust_mode != FrontierRepositoryTrustMode::Tofu
        || payload.previous_identity_event_root.is_some()
        || payload.anchor_git_commit != index.source.git_commit
        || payload.anchor_git_tree != index.source.git_tree
        || payload.git_object_format != index.source.git_object_format
    {
        return Ok(false);
    }
    let source_event_log_root =
        format!("sha256:{}", events::event_log_hash(&source_project.events));
    if payload.anchor_event_log_root != source_event_log_root
        || payload.anchor_event_count != source_project.events.len() as u64
    {
        return Ok(false);
    }
    let source_proposal_root = format!(
        "sha256:{}",
        proposals::proposal_state_hash(&source_project.proposals)
    );
    let mut sealed_events = source_project.events.clone();
    sealed_events.push((*boundary).clone());
    Ok(
        index.roots.event_log_root == format!("sha256:{}", events::event_log_hash(&sealed_events))
            && index.roots.event_count == sealed_events.len() as u64
            && index.roots.nonlease_event_log_root
                == format!("sha256:{}", events::nonlease_event_log_hash(&sealed_events))
            && index.roots.nonlease_event_log_root == effective.nonlease_event_log_root
            && index.roots.proposal_root == source_proposal_root
            && index.roots.observed_profile_root == effective.profile_root,
    )
}

fn assess_v2(
    project: &Project,
    repo_path: &Path,
    bytes: &[u8],
    index: TargetIndexV2,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<TargetIndexAssessment, String> {
    index.validate()?;
    let canonical_bytes =
        canonical::to_canonical_bytes(&index).map_err(|error| error.to_string())?;
    if canonical_bytes != bytes {
        return Err(format!(
            "{CODE_SCHEMA_INVALID}: tracked targets.json must be exact canonical JSON without whitespace or a trailing newline"
        ));
    }

    let mut global_issues = Vec::new();
    let mut target_issues = BTreeMap::new();
    let mut packet_values = BTreeMap::new();

    if index.frontier_id != project.frontier_id() {
        global_issues.push(issue(
            CODE_FRONTIER_MISMATCH,
            format!(
                "index Frontier {} differs from loaded {}",
                index.frontier_id,
                project.frontier_id()
            ),
        ));
    }

    let effective = derive_effective_roots(project, repo_path, trust_anchor)?;
    if index.roots.identity_root != effective.identity_root {
        global_issues.push(issue(
            CODE_IDENTITY_ROOT_MISMATCH,
            "index identity root differs from repository-context-verified identity",
        ));
    }
    if index.roots.dependency_root != effective.dependency_root {
        global_issues.push(issue(
            CODE_DEPENDENCY_ROOT_MISMATCH,
            "index dependency root differs from the latest verified boundary",
        ));
    }
    if index.roots.scientific_state_root != effective.scientific_state_root {
        global_issues.push(issue(
            CODE_STATE_ROOT_MISMATCH,
            "index scientific-state root differs from the current derived state",
        ));
    }
    if index.roots.proposal_root != effective.proposal_root {
        global_issues.push(issue(
            CODE_PROPOSAL_ROOT_MISMATCH,
            "index proposal root differs from current retained proposals",
        ));
    }
    if index.roots.nonlease_event_log_root != effective.nonlease_event_log_root {
        global_issues.push(issue(
            CODE_EVENT_ROOT_MISMATCH,
            "a non-lease event changed after the index was sealed",
        ));
    }

    let actual_format = repository_object_format(repo_path)?;
    if actual_format != index.source.git_object_format {
        global_issues.push(issue(
            CODE_SOURCE_TREE_MISMATCH,
            "index Git object format differs from this repository",
        ));
    }
    let resolved_source = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{commit}}", index.source.git_commit),
        ],
    );
    let source_available = match resolved_source {
        Ok(resolved) if resolved == index.source.git_commit => true,
        Ok(_) => {
            global_issues.push(issue(
                CODE_SOURCE_UNAVAILABLE,
                "source commit did not resolve to the exact declared object",
            ));
            false
        }
        Err(error) => {
            global_issues.push(issue(
                CODE_SOURCE_UNAVAILABLE,
                format!("source commit is unavailable: {error}"),
            ));
            false
        }
    };

    if source_available {
        let source_tree = git_text(
            repo_path,
            &[
                "rev-parse",
                &format!("{}^{{tree}}", index.source.git_commit),
            ],
        )?;
        if source_tree != index.source.git_tree {
            global_issues.push(issue(
                CODE_SOURCE_TREE_MISMATCH,
                format!(
                    "source commit derives tree {source_tree}, not {}",
                    index.source.git_tree
                ),
            ));
        }
        let head = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
        let ancestor = command(
            repo_path,
            &[
                "merge-base",
                "--is-ancestor",
                &index.source.git_commit,
                &head,
            ],
        )?;
        if !ancestor.status.success() {
            global_issues.push(issue(
                CODE_SOURCE_NOT_ANCESTOR,
                "source commit is not an ancestor of current HEAD",
            ));
        }
        if let Some(source_index) = tree_entry(repo_path, &index.source.git_commit, "targets.json")?
            && blob(repo_path, &source_index)? == bytes
        {
            global_issues.push(issue(
                CODE_SOURCE_SELF_REFERENCE,
                "source tree already contains the exact sealed targets.json bytes",
            ));
        }

        match materialize_project_at_commit(repo_path, &index.source.git_commit) {
            Ok((_temporary, source_project, source_profile)) => {
                let source_event_root =
                    format!("sha256:{}", events::event_log_hash(&source_project.events));
                let source_nonlease_root = format!(
                    "sha256:{}",
                    events::nonlease_event_log_hash(&source_project.events)
                );
                if index.roots.event_log_root != source_event_root
                    || index.roots.event_count != source_project.events.len() as u64
                    || index.roots.nonlease_event_log_root != source_nonlease_root
                    || !source_events_are_retained(&source_project, project)?
                {
                    global_issues.push(issue(
                        CODE_EVENT_ROOT_MISMATCH,
                        "source event prefix or count does not match the current retained event set",
                    ));
                }
                source_profile.assert_frontier_id(&project.frontier_id())?;
                if index.roots.observed_profile_root != source_profile.profile_root()? {
                    global_issues.push(issue(
                        CODE_STATE_ROOT_MISMATCH,
                        "observed profile root was not derived from the source commit",
                    ));
                }
            }
            Err(profile_error) => {
                match materialize_project_only_at_commit(repo_path, &index.source.git_commit) {
                    Ok((_temporary, source_project))
                        if source_frontier_schema(repo_path, &index.source.git_commit)?
                            .as_deref()
                            == Some(vela_protocol::frontier_repo::FRONTIER_MANIFEST_SCHEMA)
                            && is_exact_legacy_migration_source(
                                project,
                                &index,
                                &source_project,
                                &effective,
                            )? => {}
                    Ok(_) | Err(_) => global_issues.push(issue(
                        CODE_SOURCE_UNAVAILABLE,
                        format!("source Frontier cannot be materialized: {profile_error}"),
                    )),
                }
            }
        }

        global_issues.extend(validate_input_git_bytes(repo_path, &index)?);
    }

    if index.computed_index_root()? != index.index_root {
        global_issues.push(issue(
            CODE_INDEX_ROOT_MISMATCH,
            "index_root differs from the canonical index preimage",
        ));
    }
    match exact_tracked_head_bytes(repo_path, "targets.json", TARGET_INDEX_JSON_MAX_BYTES) {
        Ok(tracked) if tracked == bytes => {}
        Ok(_) => global_issues.push(issue(
            CODE_OUTPUT_NOT_TRACKED,
            "targets.json bytes differ from the exact tracked HEAD blob",
        )),
        Err(error) => global_issues.push(issue(CODE_OUTPUT_NOT_TRACKED, error)),
    }

    for target in index.targets.iter().filter(|target| target.state == "open") {
        match exact_tracked_head_bytes(repo_path, &target.packet.path, TARGET_PACKET_MAX_BYTES) {
            Ok(packet_bytes) => {
                let digest = sha256_root(&packet_bytes);
                if packet_bytes.len() as u64 != target.packet.size || digest != target.packet.sha256
                {
                    push_target_issue(
                        &mut target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        format!(
                            "packet bytes at {:?} differ from the sealed size or digest",
                            target.packet.path
                        ),
                    );
                    continue;
                }
                match serde_json::from_slice::<Value>(&packet_bytes) {
                    Ok(packet)
                        if packet.is_object()
                            && packet.get("schema").and_then(Value::as_str)
                                == Some(target.packet.schema.as_str()) =>
                    {
                        packet_values.insert(target.id.clone(), packet);
                    }
                    Ok(_) => push_target_issue(
                        &mut target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        "packet must be one JSON object with the exact sealed schema",
                    ),
                    Err(error) => push_target_issue(
                        &mut target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        format!("packet JSON is invalid: {error}"),
                    ),
                }
            }
            Err(error) => push_target_issue(
                &mut target_issues,
                &target.id,
                CODE_OUTPUT_NOT_TRACKED,
                error,
            ),
        }
    }

    // Profile drift alone is intentionally audit context rather than
    // staleness. Deriving it above still proves that it is valid Profile v1.
    let _current_profile_root = effective.profile_root;
    let _current_full_event_root = effective.event_log_root;
    let _current_event_count = effective.event_count;

    sort_issues(&mut global_issues);
    for issues in target_issues.values_mut() {
        sort_issues(issues);
    }
    Ok(TargetIndexAssessment {
        document_root: sha256_root(bytes),
        document: TargetIndexDocument::V2(index),
        global_issues,
        target_issues,
        packet_values,
    })
}

fn assess_v1(
    project: &Project,
    bytes: &[u8],
    index: TargetIndexV1,
) -> Result<TargetIndexAssessment, String> {
    index.validate()?;
    let mut global_issues = Vec::new();
    if index.frontier_id != project.frontier_id() {
        global_issues.push(issue(
            CODE_FRONTIER_MISMATCH,
            "historical v1 index names a different Frontier",
        ));
    }
    let current_snapshot = format!("sha256:{}", events::snapshot_hash(project));
    let current_events = format!("sha256:{}", events::event_log_hash(&project.events));
    let current_proposals = format!(
        "sha256:{}",
        proposals::proposal_state_hash(&project.proposals)
    );
    if index.as_of.snapshot_hash != current_snapshot {
        global_issues.push(issue(
            CODE_STATE_ROOT_MISMATCH,
            "historical v1 snapshot root is stale",
        ));
    }
    if index.as_of.event_log_hash != current_events {
        global_issues.push(issue(
            CODE_EVENT_ROOT_MISMATCH,
            "historical v1 event root is stale",
        ));
    }
    if index.as_of.proposal_state_hash != current_proposals {
        global_issues.push(issue(
            CODE_PROPOSAL_ROOT_MISMATCH,
            "historical v1 proposal root is stale",
        ));
    }
    sort_issues(&mut global_issues);
    Ok(TargetIndexAssessment {
        document_root: sha256_root(bytes),
        document: TargetIndexDocument::HistoricalV1(index),
        global_issues,
        target_issues: BTreeMap::new(),
        packet_values: BTreeMap::new(),
    })
}

/// Read and assess `targets.json` once against immutable Git objects and the
/// loaded Frontier. A valid return is still non-authoritative: callers must
/// use [`TargetIndexAssessment::fresh_open_v2_targets`] to obtain offerable
/// work and must re-run this assessment at the lease transaction edge.
pub fn assess_target_index(
    project: &Project,
    repo_path: &Path,
) -> Result<Option<TargetIndexAssessment>, String> {
    assess_target_index_with_trust_anchor(project, repo_path, None)
}

/// Assess `targets.json` with an independently reviewed initial repository
/// trust anchor. Legacy TOFU-rooted repository-boundary chains fail closed
/// when this value is absent; callers must never derive it from repository
/// bytes under assessment.
pub fn assess_target_index_with_trust_anchor(
    project: &Project,
    repo_path: &Path,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<Option<TargetIndexAssessment>, String> {
    let path = repo_path.join("targets.json");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect target index {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{CODE_OUTPUT_NOT_TRACKED}: targets.json must be a regular non-symlink file"
        ));
    }
    let bytes = read_regular_file(&path, TARGET_INDEX_JSON_MAX_BYTES, "target index")?;
    let envelope: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse targets.json: {error}"))?;
    let schema = envelope
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{CODE_SCHEMA_INVALID}: targets.json has no string schema"))?;
    match schema {
        TARGET_INDEX_SCHEMA_V1 => {
            let index: TargetIndexV1 = serde_json::from_value(envelope)
                .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse v1 index: {error}"))?;
            assess_v1(project, &bytes, index).map(Some)
        }
        TARGET_INDEX_SCHEMA_V2 => {
            let index: TargetIndexV2 = serde_json::from_value(envelope)
                .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse v2 index: {error}"))?;
            assess_v2(project, repo_path, &bytes, index, trust_anchor).map(Some)
        }
        other => Err(format!(
            "{CODE_SCHEMA_INVALID}: unsupported target index schema {other:?}"
        )),
    }
}

impl TargetIndexAssessment {
    pub fn index_schema(&self) -> &str {
        match &self.document {
            TargetIndexDocument::HistoricalV1(index) => &index.schema,
            TargetIndexDocument::V2(index) => &index.schema,
        }
    }

    pub fn all_codes(&self) -> Vec<&'static str> {
        let mut codes = self
            .global_issues
            .iter()
            .map(|issue| issue.code)
            .collect::<BTreeSet<_>>();
        for issues in self.target_issues.values() {
            codes.extend(issues.iter().map(|issue| issue.code));
        }
        codes.into_iter().collect()
    }

    pub fn index_root(&self) -> &str {
        match &self.document {
            TargetIndexDocument::HistoricalV1(_) => &self.document_root,
            TargetIndexDocument::V2(index) => &index.index_root,
        }
    }

    pub fn is_historical_v1(&self) -> bool {
        matches!(self.document, TargetIndexDocument::HistoricalV1(_))
    }

    pub fn configured_open(&self) -> usize {
        match &self.document {
            TargetIndexDocument::HistoricalV1(index) => index
                .targets
                .iter()
                .filter(|target| target.state == "open")
                .count(),
            TargetIndexDocument::V2(index) => index
                .targets
                .iter()
                .filter(|target| target.state == "open")
                .count(),
        }
    }

    pub fn indexed_ids(&self) -> BTreeSet<&str> {
        match &self.document {
            TargetIndexDocument::HistoricalV1(index) => index
                .targets
                .iter()
                .map(|target| target.id.as_str())
                .collect(),
            TargetIndexDocument::V2(index) => index
                .targets
                .iter()
                .map(|target| target.id.as_str())
                .collect(),
        }
    }

    pub fn fresh_open_v2_targets(&self) -> Vec<&TargetIndexEntryV2> {
        let TargetIndexDocument::V2(index) = &self.document else {
            return Vec::new();
        };
        if !self.global_issues.is_empty() {
            return Vec::new();
        }
        index
            .targets
            .iter()
            .filter(|target| {
                target.state == "open"
                    && self.target_issues.get(&target.id).is_none_or(Vec::is_empty)
            })
            .collect()
    }

    pub fn stale_open(&self) -> usize {
        self.configured_open()
            .saturating_sub(self.fresh_open_v2_targets().len())
    }

    pub fn packet_value(&self, target_id: &str) -> Option<&Value> {
        self.packet_values.get(target_id)
    }

    pub fn v2(&self) -> Option<&TargetIndexV2> {
        match &self.document {
            TargetIndexDocument::V2(index) => Some(index),
            TargetIndexDocument::HistoricalV1(_) => None,
        }
    }
}

fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Return the copyable, read-only repair command used by compact producer
/// projections. Keeping quoting here prevents CLI surfaces from inventing a
/// second command-rendering rule for the same Target Index contract.
pub fn target_index_repair_command(frontier_arg: &str) -> String {
    format!(
        "vela target-index repair {} --json",
        shell_word(frontier_arg)
    )
}

fn assessment_codes(assessment: &TargetIndexAssessment) -> Vec<&'static str> {
    let mut codes = assessment.all_codes().into_iter().collect::<BTreeSet<_>>();
    if assessment.is_historical_v1() {
        codes.insert(CODE_PROFILE_UPGRADE_REQUIRED);
    }
    codes.into_iter().collect()
}

fn target_index_profile_migration_command(frontier_arg: &str) -> String {
    format!(
        "vela migrate {} --to frontier-repo-v1 --check --profile ../frontier-profile-v1.yaml --target-candidate ../target-index-candidate.json --as reviewer:ADMINISTRATOR --reason 'Bind exact legacy repository' --json",
        shell_word(frontier_arg)
    )
}

fn changed_declared_paths(assessment: &TargetIndexAssessment, repo_path: &Path) -> Vec<String> {
    let mut changed = BTreeSet::new();
    let TargetIndexDocument::V2(index) = &assessment.document else {
        return Vec::new();
    };
    for input in &index.inputs.entries {
        let source = tree_entry(repo_path, &index.source.git_commit, &input.path);
        let head = tree_entry(repo_path, "HEAD", &input.path);
        let exact = matches!((source, head), (Ok(Some(source)), Ok(Some(head)))
            if source.mode == head.mode && source.kind == head.kind && source.object == head.object);
        if !exact {
            changed.insert(input.path.clone());
        }
    }
    for target in &index.targets {
        if assessment
            .target_issues
            .get(&target.id)
            .is_some_and(|issues| !issues.is_empty())
        {
            changed.insert(target.packet.path.clone());
        }
    }
    changed.into_iter().collect()
}

/// Report exactly why the current index needs domain regeneration or
/// resealing. This function performs no writes and intentionally cannot
/// create a candidate.
pub fn target_index_repair_report(
    project: &Project,
    repo_path: &Path,
    frontier_arg: &str,
) -> Result<Option<TargetIndexRepairReport>, String> {
    target_index_repair_report_with_trust_anchor(project, repo_path, frontier_arg, None)
}

pub fn target_index_repair_report_with_trust_anchor(
    project: &Project,
    repo_path: &Path,
    frontier_arg: &str,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<Option<TargetIndexRepairReport>, String> {
    let Some(assessment) = assess_target_index_with_trust_anchor(project, repo_path, trust_anchor)?
    else {
        return Ok(None);
    };
    let historical_only = assessment.is_historical_v1();
    let candidate_path = if historical_only {
        "../target-index-candidate.json"
    } else {
        ".vela/tmp/target-index-candidate.json"
    };
    Ok(Some(TargetIndexRepairReport {
        schema: "vela.target-index-repair.v1",
        frontier_id: project.frontier_id(),
        index_schema: assessment.index_schema().to_string(),
        index_root: assessment.index_root().to_string(),
        historical_only,
        codes: assessment_codes(&assessment),
        changed_declared_paths: changed_declared_paths(&assessment, repo_path),
        candidate_path,
        generator_instruction: if historical_only {
            TARGET_INDEX_MIGRATION_INSTRUCTION
        } else {
            TARGET_INDEX_REGENERATION_INSTRUCTION
        },
        repair_command: if historical_only {
            target_index_profile_migration_command(frontier_arg)
        } else {
            format!(
                "vela target-index seal {} --candidate {candidate_path} --check --json",
                shell_word(frontier_arg)
            )
        },
    }))
}

/// Summarize the current index without selecting or enabling work.
pub fn target_index_inspection_summary(
    project: &Project,
    repo_path: &Path,
    frontier_arg: &str,
) -> Result<Option<TargetIndexInspectionSummary>, String> {
    target_index_inspection_summary_with_trust_anchor(project, repo_path, frontier_arg, None)
}

pub fn target_index_inspection_summary_with_trust_anchor(
    project: &Project,
    repo_path: &Path,
    frontier_arg: &str,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<Option<TargetIndexInspectionSummary>, String> {
    let Some(assessment) = assess_target_index_with_trust_anchor(project, repo_path, trust_anchor)?
    else {
        return Ok(None);
    };
    Ok(Some(TargetIndexInspectionSummary {
        schema: "vela.target-index-inspection-summary.v1",
        frontier_id: project.frontier_id(),
        index_schema: assessment.index_schema().to_string(),
        index_root: assessment.index_root().to_string(),
        historical_only: assessment.is_historical_v1(),
        configured_open: assessment.configured_open(),
        stale_open: assessment.stale_open(),
        codes: assessment_codes(&assessment),
        repair_command: target_index_repair_command(frontier_arg),
    }))
}

/// Inspect one exact target without granting an offer. Historical v1 packet
/// bytes are exposed only after bounded path, digest, object, and schema
/// checks; any mismatch remains visible as a non-actionable inspection.
pub fn inspect_target_index_target(
    project: &Project,
    repo_path: &Path,
    target_id: &str,
) -> Result<Option<TargetIndexTargetInspection>, String> {
    inspect_target_index_target_with_trust_anchor(project, repo_path, target_id, None)
}

pub fn inspect_target_index_target_with_trust_anchor(
    project: &Project,
    repo_path: &Path,
    target_id: &str,
    trust_anchor: Option<&RepositoryTrustAnchor>,
) -> Result<Option<TargetIndexTargetInspection>, String> {
    let Some(assessment) = assess_target_index_with_trust_anchor(project, repo_path, trust_anchor)?
    else {
        return Ok(None);
    };
    let mut codes = assessment
        .global_issues
        .iter()
        .map(|issue| issue.code)
        .collect::<BTreeSet<_>>();
    if assessment.is_historical_v1() {
        codes.insert(CODE_PROFILE_UPGRADE_REQUIRED);
    }
    if let Some(issues) = assessment.target_issues.get(target_id) {
        codes.extend(issues.iter().map(|issue| issue.code));
    }
    match &assessment.document {
        TargetIndexDocument::HistoricalV1(index) => {
            let Some(target) = index.targets.iter().find(|target| target.id == target_id) else {
                return Ok(None);
            };
            let packet =
                match safe_worktree_file(repo_path, &target.packet.path, TARGET_PACKET_MAX_BYTES) {
                    Ok(bytes)
                        if sha256_root(&bytes) == target.packet.sha256
                            && serde_json::from_slice::<Value>(&bytes).is_ok() =>
                    {
                        let value: Value = serde_json::from_slice(&bytes)
                            .map_err(|error| format!("parse historical packet: {error}"))?;
                        if value.is_object()
                            && value.get("schema").and_then(Value::as_str)
                                == Some(target.packet.schema.as_str())
                        {
                            Some(value)
                        } else {
                            codes.insert(CODE_PACKET_MISMATCH);
                            None
                        }
                    }
                    Ok(_) | Err(_) => {
                        codes.insert(CODE_PACKET_MISMATCH);
                        None
                    }
                };
            Ok(Some(TargetIndexTargetInspection {
                schema: "vela.target-index-inspection.v1",
                index_schema: index.schema.clone(),
                index_root: assessment.document_root,
                target_id: target.id.clone(),
                title: target.title.clone(),
                state: target.state.clone(),
                historical_only: true,
                actionable: false,
                codes: codes.into_iter().collect(),
                packet_ref: serde_json::to_value(&target.packet)
                    .map_err(|error| error.to_string())?,
                packet,
            }))
        }
        TargetIndexDocument::V2(index) => {
            let Some(target) = index.targets.iter().find(|target| target.id == target_id) else {
                return Ok(None);
            };
            Ok(Some(TargetIndexTargetInspection {
                schema: "vela.target-index-inspection.v1",
                index_schema: index.schema.clone(),
                index_root: index.index_root.clone(),
                target_id: target.id.clone(),
                title: target.title.clone(),
                state: target.state.clone(),
                historical_only: false,
                // Inspection is deliberately not an offer edge. Freshness is
                // visible through `codes`, while only `next`/`work` may turn a
                // fresh open v2 entry into an actionable producer task.
                actionable: false,
                codes: codes.into_iter().collect(),
                packet_ref: serde_json::to_value(&target.packet)
                    .map_err(|error| error.to_string())?,
                packet: assessment.packet_value(target_id).cloned(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_protocol::frontier_profile::{FrontierProfileLicenseV1, FrontierProfileScopeV1};

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn input(path: &str, bytes: &[u8]) -> RetainedObjectEntryV1 {
        RetainedObjectEntryV1 {
            path: path.to_string(),
            git_mode: "100644".to_string(),
            size: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(bytes)),
        }
    }

    fn base_index() -> TargetIndexV2 {
        let entries = vec![input("domain/source.json", br#"{"open":[1056]}"#)];
        let mut inputs = TargetIndexInputManifestV1 {
            schema: TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1.to_string(),
            input_root: root('0'),
            entries,
        };
        inputs.input_root = inputs.computed_root().unwrap();
        let mut index = TargetIndexV2 {
            schema: TARGET_INDEX_SCHEMA_V2.to_string(),
            frontier_id: "vfr_1234567890abcdef".to_string(),
            source: TargetIndexSourceV2 {
                git_object_format: GitObjectFormat::Sha1,
                git_commit: "1".repeat(40),
                git_tree: "2".repeat(40),
            },
            inputs,
            roots: TargetIndexRootsV2 {
                event_log_root: root('3'),
                event_count: 42,
                nonlease_event_log_root: root('4'),
                scientific_state_root: root('5'),
                proposal_root: root('6'),
                identity_root: root('7'),
                dependency_root: root('8'),
                observed_profile_root: root('9'),
            },
            claim_boundary: TargetIndexClaimBoundaryV2 {
                derived: true,
                authoritative: false,
                deletable: true,
            },
            generated_by: TargetIndexGeneratorV2 {
                program: "vela".to_string(),
                version: "0.914.0".to_string(),
            },
            targets: vec![TargetIndexEntryV2 {
                id: "erdos:1056".to_string(),
                title: "Erdos 1056".to_string(),
                why: "First bounded open target".to_string(),
                state: "open".to_string(),
                rank: 17_619_056,
                objective: "Produce one decision-relevant artifact.".to_string(),
                labels: vec!["erdos".to_string(), "upstream-open".to_string()],
                packet: TargetPacketRefV2 {
                    schema: "erdos-frontier.problem-work.v1".to_string(),
                    path: "site/problems/1056.json".to_string(),
                    size: 456,
                    sha256: root('a'),
                },
            }],
            index_root: root('0'),
        };
        index.index_root = index.computed_index_root().unwrap();
        index
    }

    #[test]
    fn target_index_v2_root_is_closed_and_deterministic() {
        let index = base_index();
        index.validate().unwrap();
        assert_eq!(
            index.index_root,
            "sha256:c2b65099d8bd2e55dabbc14d17bfd42db33a5e00d17bdc6b9455fba97fd767ce"
        );
        let mut changed = index.clone();
        changed.targets[0].rank += 1;
        assert!(changed.validate().unwrap_err().contains("index_root"));
    }

    #[test]
    fn target_index_v2_rejects_order_paths_and_output_inputs() {
        let mut index = base_index();
        let mut second = index.targets[0].clone();
        second.id = "erdos:1055".to_string();
        second.rank = index.targets[0].rank;
        second.packet.path = "site/problems/1055.json".to_string();
        index.targets.push(second);
        index.index_root = index.computed_index_root().unwrap();
        assert!(index.validate().unwrap_err().contains("sorted"));

        let mut index = base_index();
        index.targets[0].packet.path = "../outside.json".to_string();
        index.index_root = index.computed_index_root().unwrap();
        assert!(
            index
                .validate()
                .unwrap_err()
                .contains("normalized frontier-relative")
        );

        let mut index = base_index();
        index.inputs.entries[0].path = index.targets[0].packet.path.clone();
        index.inputs.input_root = index.inputs.computed_root().unwrap();
        index.index_root = index.computed_index_root().unwrap();
        assert!(
            index
                .validate()
                .unwrap_err()
                .contains("output or candidate")
        );
    }

    #[test]
    fn target_index_v2_limits_and_generator_semver_are_exact() {
        let mut index = base_index();
        index.roots.event_count = JSON_SAFE_INTEGER_MAX;
        index.inputs.entries[0].size = JSON_SAFE_INTEGER_MAX;
        index.inputs.input_root = index.inputs.computed_root().unwrap();
        index.targets[0].rank = JSON_SAFE_INTEGER_MAX;
        index.targets[0].labels = (0..TARGET_INDEX_MAX_LABELS)
            .map(|value| format!("label-{value:02}"))
            .collect();
        index.targets[0].packet.size = TARGET_PACKET_MAX_BYTES;
        index.generated_by.version = "0.914.0-rc.1+fixture.7".to_string();
        index.index_root = index.computed_index_root().unwrap();
        index.validate().unwrap();

        let mut invalid = index.clone();
        invalid.targets[0].rank = JSON_SAFE_INTEGER_MAX + 1;
        assert!(invalid.validate().unwrap_err().contains("target.rank"));

        let mut invalid = index.clone();
        invalid.targets[0].packet.size = TARGET_PACKET_MAX_BYTES + 1;
        assert!(invalid.validate().unwrap_err().contains("packet.size"));

        let mut invalid = index.clone();
        invalid.targets[0].labels.push("label-64".to_string());
        assert!(invalid.validate().unwrap_err().contains("more than"));

        for version in ["01.2.3", "1.2", "1.2.3-", "1.2.3+", "1.2.3-01"] {
            let mut invalid = index.clone();
            invalid.generated_by.version = version.to_string();
            assert!(
                invalid.validate().unwrap_err().contains("semantic version"),
                "accepted invalid semantic version {version:?}"
            );
        }
    }

    #[test]
    fn input_and_task_binding_roots_cover_their_closed_preimages() {
        let mut index = base_index();
        assert_eq!(
            index.inputs.input_root,
            "sha256:b08486c1c108fb397824e0e8ca563486c862af0112cd794261615bcf0e8d78b0"
        );
        assert_eq!(
            sha256_root(br#"{\"problem\":1056,\"schema\":\"erdos-frontier.problem-work.v1\"}"#),
            "sha256:33294359556a3b0d66a19431bd86efa94f7cb1662dcefdde85512ee4aa81e436"
        );
        index.inputs.input_root = index.inputs.computed_root().unwrap();
        index.index_root = index.computed_index_root().unwrap();
        index.validate().unwrap();

        let mut changed_input = index.inputs.clone();
        changed_input.entries[0].size += 1;
        assert!(changed_input.validate().unwrap_err().contains("input_root"));

        let mut binding = TargetTaskBindingV1 {
            schema: TARGET_TASK_BINDING_SCHEMA_V1.to_string(),
            frontier_id: index.frontier_id.clone(),
            target_id: index.targets[0].id.clone(),
            target_index_root: index.index_root.clone(),
            source: index.source.clone(),
            input_root: index.inputs.input_root.clone(),
            packet: index.targets[0].packet.clone(),
            index_roots: TargetTaskIndexRootsV1 {
                event_log_root: index.roots.event_log_root.clone(),
                event_count: index.roots.event_count,
                nonlease_event_log_root: index.roots.nonlease_event_log_root.clone(),
                scientific_state_root: index.roots.scientific_state_root.clone(),
                proposal_root: index.roots.proposal_root.clone(),
                identity_root: index.roots.identity_root.clone(),
                dependency_root: index.roots.dependency_root.clone(),
            },
            claim_read_set: TargetTaskClaimReadSetV1 {
                event_log_root: root('9'),
                event_count: index.roots.event_count + 1,
                git_object_format: index.source.git_object_format,
                git_commit: "3".repeat(40),
                git_tree: "4".repeat(40),
            },
            binding_root: root('0'),
        };
        binding.binding_root = binding.computed_binding_root().unwrap();
        assert_eq!(
            binding.binding_root,
            "sha256:1e370daa0628b2f26c1a84a073cbab78ba5760a85e50e908f3b80be2f6d80851"
        );
        binding.validate().unwrap();
        const EXPECTED_BINDING_JSON: &str = r#"{"binding_root":"sha256:1e370daa0628b2f26c1a84a073cbab78ba5760a85e50e908f3b80be2f6d80851","claim_read_set":{"event_count":43,"event_log_root":"sha256:9999999999999999999999999999999999999999999999999999999999999999","git_commit":"3333333333333333333333333333333333333333","git_object_format":"sha1","git_tree":"4444444444444444444444444444444444444444"},"frontier_id":"vfr_1234567890abcdef","index_roots":{"dependency_root":"sha256:8888888888888888888888888888888888888888888888888888888888888888","event_count":42,"event_log_root":"sha256:3333333333333333333333333333333333333333333333333333333333333333","identity_root":"sha256:7777777777777777777777777777777777777777777777777777777777777777","nonlease_event_log_root":"sha256:4444444444444444444444444444444444444444444444444444444444444444","proposal_root":"sha256:6666666666666666666666666666666666666666666666666666666666666666","scientific_state_root":"sha256:5555555555555555555555555555555555555555555555555555555555555555"},"input_root":"sha256:b08486c1c108fb397824e0e8ca563486c862af0112cd794261615bcf0e8d78b0","packet":{"path":"site/problems/1056.json","schema":"erdos-frontier.problem-work.v1","sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":456},"schema":"vela.target-task-binding.v1","source":{"git_commit":"1111111111111111111111111111111111111111","git_object_format":"sha1","git_tree":"2222222222222222222222222222222222222222"},"target_id":"erdos:1056","target_index_root":"sha256:c2b65099d8bd2e55dabbc14d17bfd42db33a5e00d17bdc6b9455fba97fd767ce"}"#;
        let binding_bytes = canonical::to_canonical_bytes(&binding).unwrap();
        assert_eq!(binding_bytes, EXPECTED_BINDING_JSON.as_bytes());
        assert_eq!(
            sha256_root(&binding_bytes),
            "sha256:1f956f2e9f7e291af0e0bba9f1a95507b6805898211b7010063b06476fb2976d"
        );

        let mut changed_binding = binding.clone();
        changed_binding.claim_read_set.event_count += 1;
        assert!(
            changed_binding
                .validate()
                .unwrap_err()
                .contains("binding_root")
        );
    }

    #[test]
    fn candidate_cannot_supply_seal_owned_fields() {
        let closed = TargetIndexCandidateV1 {
            schema: TARGET_INDEX_CANDIDATE_SCHEMA_V1.to_string(),
            frontier_id: "vfr_1234567890abcdef".to_string(),
            source: TargetIndexCandidateSourceV1 {
                git_commit: "1".repeat(40),
                input_paths: Vec::new(),
            },
            targets: Vec::new(),
        };
        closed.validate().unwrap();
        assert_eq!(
            canonical_root(&closed).unwrap(),
            "sha256:4eafe672c73aa162f1d9813f939bf70176d13a4ce3cb7325948f626d793f9fe6"
        );

        let candidate = serde_json::json!({
            "schema": TARGET_INDEX_CANDIDATE_SCHEMA_V1,
            "frontier_id": "vfr_1234567890abcdef",
            "source": {
                "git_commit": "1".repeat(40),
                "input_paths": []
            },
            "targets": [],
            "index_root": root('a')
        });
        assert!(serde_json::from_value::<TargetIndexCandidateV1>(candidate).is_err());
    }

    #[test]
    fn legacy_migration_seal_binds_external_candidate_and_one_planned_boundary() {
        let directory = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize_minimal(
            directory.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "Legacy target migration fixture",
                initialize_git: true,
            },
        )
        .unwrap();
        run(directory.path(), &["config", "user.name", "Vela Test"]);
        run(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        let mut project = repo::load_from_path(directory.path()).unwrap();
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
        repo::save_to_path(directory.path(), &project).unwrap();
        run(directory.path(), &["add", "-A"]);
        run(directory.path(), &["commit", "-qm", "legacy anchor"]);
        let anchor_commit = run(directory.path(), &["rev-parse", "HEAD^{commit}"]);
        let anchor_tree = run(directory.path(), &["rev-parse", "HEAD^{tree}"]);
        let source_event_log_root = format!("sha256:{}", events::event_log_hash(&project.events));
        let source_nonlease_event_log_root = format!(
            "sha256:{}",
            events::nonlease_event_log_hash(&project.events)
        );
        let proposal_root = format!(
            "sha256:{}",
            proposals::proposal_state_hash(&project.proposals)
        );
        let legacy_identity_preimage_root = root('8');
        let identity_root = vela_protocol::frontier_repository::LegacyFrontierOriginV1 {
            schema: vela_protocol::frontier_repository::LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: project.frontier_id(),
            legacy_identity_preimage_root: legacy_identity_preimage_root.clone(),
            git_object_format: GitObjectFormat::Sha1,
            anchor_git_commit: anchor_commit.clone(),
            anchor_git_tree: anchor_tree.clone(),
            anchor_event_log_root: source_event_log_root.clone(),
            anchor_event_count: project.events.len() as u64,
        }
        .identity_root()
        .unwrap();
        let dependency_root = exact_dependency_root(&[]).unwrap();
        let observed_profile_root = root('9');
        let payload = FrontierRepositoryBoundaryPayloadV1 {
            schema: vela_protocol::frontier_repository::FRONTIER_REPOSITORY_BOUNDARY_SCHEMA
                .to_string(),
            mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
            frontier_id: project.frontier_id(),
            identity_root: identity_root.clone(),
            observed_profile_root: observed_profile_root.clone(),
            dependency_root: dependency_root.clone(),
            dependencies: Vec::new(),
            previous_identity_event_root: None,
            legacy_identity_preimage_root: Some(legacy_identity_preimage_root),
            administrator_actor_id: "reviewer:migration".to_string(),
            administrator_public_key: "1".repeat(64),
            administrator_algorithm: "ed25519".to_string(),
            trust_mode: FrontierRepositoryTrustMode::Tofu,
            git_object_format: GitObjectFormat::Sha1,
            anchor_git_commit: anchor_commit.clone(),
            anchor_git_tree: anchor_tree.clone(),
            anchor_event_log_root: source_event_log_root.clone(),
            anchor_event_count: project.events.len() as u64,
            anchor_snapshot_root: root('a'),
            anchor_snapshot_schema: "vela.snapshot.v0.1".to_string(),
            anchor_proposal_root: proposal_root.clone(),
            anchor_actor_registry_root: root('b'),
            anchor_artifact_registry_root: root('c'),
            anchor_canonical_store_root: root('d'),
        };
        let boundary = new_repository_boundary_event(
            payload,
            "Bind exact legacy repository",
            "2026-07-22T12:00:00Z",
        )
        .unwrap();
        let boundary_content_root = sha256_root(&event_content_preimage_bytes(&boundary));
        let mut after = repo::load_from_path(directory.path()).unwrap();
        after.events.push(boundary.clone());
        let final_roots = TargetIndexRootsV2 {
            event_log_root: format!("sha256:{}", events::event_log_hash(&after.events)),
            event_count: after.events.len() as u64,
            nonlease_event_log_root: format!(
                "sha256:{}",
                events::nonlease_event_log_hash(&after.events)
            ),
            scientific_state_root: vela_protocol::scientific_state::scientific_state_root_v2(
                &after,
                &identity_root,
                &dependency_root,
            )
            .unwrap(),
            proposal_root,
            identity_root,
            dependency_root,
            observed_profile_root,
        };
        let context = TargetIndexMigrationContextV1 {
            schema: TARGET_INDEX_MIGRATION_CONTEXT_SCHEMA_V1.to_string(),
            anchor_git_commit: anchor_commit.clone(),
            anchor_git_tree: anchor_tree,
            source_event_log_root,
            source_event_count: project.events.len() as u64,
            source_nonlease_event_log_root,
            planned_boundary_event: boundary,
            planned_boundary_event_content_root: boundary_content_root,
            final_roots: final_roots.clone(),
        };
        let packet = br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#;
        write(&directory.path().join("site/problems/1056.json"), packet);
        let candidate = serde_json::json!({
            "schema": TARGET_INDEX_CANDIDATE_SCHEMA_V1,
            "frontier_id": project.frontier_id(),
            "source": {"git_commit": anchor_commit, "input_paths": []},
            "targets": [{
                "id": "erdos:1056",
                "title": "Erdős 1056",
                "why": "First exact migration target.",
                "state": "open",
                "rank": 1,
                "objective": "Produce one bounded artifact.",
                "labels": ["erdos", "open"],
                "packet": {
                    "schema": "erdos-frontier.problem-work.v1",
                    "path": "site/problems/1056.json"
                }
            }]
        });
        let candidate_bytes = serde_json::to_vec_pretty(&candidate).unwrap();
        let candidate_path = directory
            .path()
            .join(".vela/tmp/target-index-candidate.json");
        write(&candidate_path, &candidate_bytes);

        let plan = prepare_target_index_seal_for_migration(
            directory.path(),
            &candidate_path,
            "0.914.0",
            &context,
        )
        .unwrap();
        assert_eq!(plan.candidate_root, sha256_root(&candidate_bytes));
        assert_eq!(plan.index.roots, final_roots);
        assert_eq!(plan.index.targets[0].id, "erdos:1056");
        assert!(
            prepare_target_index_seal(directory.path(), &candidate_path, "0.914.0", None).is_err(),
            "ordinary Profile v1 sealing must not silently use the migration bridge"
        );

        let mut wrong = context;
        wrong.final_roots.event_count += 1;
        assert!(
            prepare_target_index_seal_for_migration(
                directory.path(),
                &candidate_path,
                "0.914.0",
                &wrong,
            )
            .unwrap_err()
            .contains("exactly one")
        );
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

    fn write(path: &Path, bytes: impl AsRef<[u8]>) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    struct GitFixture {
        directory: tempfile::TempDir,
        project: Project,
        index: TargetIndexV2,
    }

    fn git_fixture() -> GitFixture {
        let directory = tempfile::tempdir().unwrap();
        run(directory.path(), &["init", "-q", "-b", "main"]);
        run(directory.path(), &["config", "user.name", "Vela Test"]);
        run(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        let project = vela_protocol::project::assemble_profile_v1(
            "target-index-v2",
            Vec::new(),
            0,
            0,
            "fixture",
        );
        repo::init_repo(directory.path(), &project).unwrap();
        let profile = FrontierProfileV1 {
            schema: vela_protocol::frontier_profile::FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            frontier_id: project.frontier_id(),
            name: "Target Index Fixture".to_string(),
            summary: "Exact target-index repository assessment fixture.".to_string(),
            scope: FrontierProfileScopeV1 {
                question: "Can exact source bytes produce one bounded offer?".to_string(),
                includes: vec![],
                excludes: vec![],
            },
            maintainers: vec!["fixture@example.invalid".to_string()],
            license: FrontierProfileLicenseV1 {
                content: "CC-BY-4.0".to_string(),
                code: "Apache-2.0".to_string(),
                data: "CC0-1.0".to_string(),
            },
        };
        write(
            &directory.path().join("frontier.yaml"),
            serde_yaml::to_string(&profile).unwrap(),
        );
        let input_bytes = br#"{"open":[1056]}"#;
        write(&directory.path().join("domain/source.json"), input_bytes);
        run(directory.path(), &["add", "-A"]);
        run(directory.path(), &["commit", "-qm", "source"]);
        let source_commit = run(directory.path(), &["rev-parse", "HEAD^{commit}"]);
        let source_tree = run(directory.path(), &["rev-parse", "HEAD^{tree}"]);
        let packet_bytes = br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#;
        write(
            &directory.path().join("site/problems/1056.json"),
            packet_bytes,
        );

        let mut inputs = TargetIndexInputManifestV1 {
            schema: TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1.to_string(),
            input_root: root('0'),
            entries: vec![input("domain/source.json", input_bytes)],
        };
        inputs.input_root = inputs.computed_root().unwrap();
        let effective = derive_effective_roots(&project, directory.path(), None).unwrap();
        let mut index = TargetIndexV2 {
            schema: TARGET_INDEX_SCHEMA_V2.to_string(),
            frontier_id: project.frontier_id(),
            source: TargetIndexSourceV2 {
                git_object_format: GitObjectFormat::Sha1,
                git_commit: source_commit,
                git_tree: source_tree,
            },
            inputs,
            roots: TargetIndexRootsV2 {
                event_log_root: effective.event_log_root,
                event_count: effective.event_count,
                nonlease_event_log_root: effective.nonlease_event_log_root,
                scientific_state_root: effective.scientific_state_root,
                proposal_root: effective.proposal_root,
                identity_root: effective.identity_root,
                dependency_root: effective.dependency_root,
                observed_profile_root: effective.profile_root,
            },
            claim_boundary: TargetIndexClaimBoundaryV2 {
                derived: true,
                authoritative: false,
                deletable: true,
            },
            generated_by: TargetIndexGeneratorV2 {
                program: "vela".to_string(),
                version: "0.914.0".to_string(),
            },
            targets: vec![TargetIndexEntryV2 {
                id: "erdos:1056".to_string(),
                title: "Erdős 1056".to_string(),
                why: "First exact bounded target".to_string(),
                state: "open".to_string(),
                rank: 1,
                objective: "Produce one bounded artifact.".to_string(),
                labels: vec!["erdos".to_string(), "open".to_string()],
                packet: TargetPacketRefV2 {
                    schema: "erdos-frontier.problem-work.v1".to_string(),
                    path: "site/problems/1056.json".to_string(),
                    size: packet_bytes.len() as u64,
                    sha256: sha256_root(packet_bytes),
                },
            }],
            index_root: root('0'),
        };
        index.index_root = index.computed_index_root().unwrap();
        write(
            &directory.path().join("targets.json"),
            index.canonical_bytes().unwrap(),
        );
        run(directory.path(), &["add", "-A"]);
        run(directory.path(), &["commit", "-qm", "sealed index"]);
        GitFixture {
            directory,
            project,
            index,
        }
    }

    #[test]
    fn source_materialization_batches_exact_git_blobs() {
        let fixture = git_fixture();
        let entries =
            tree_entries(fixture.directory.path(), &fixture.index.source.git_commit).unwrap();
        let project_entries = entries
            .iter()
            .filter(|entry| entry.path == "frontier.yaml" || entry.path.starts_with(".vela/"))
            .collect::<Vec<_>>();
        let batched = batch_blobs(fixture.directory.path(), &project_entries).unwrap();
        assert_eq!(batched.len(), project_entries.len());
        for (entry, bytes) in project_entries.iter().zip(&batched) {
            assert_eq!(*bytes, blob(fixture.directory.path(), entry).unwrap());
        }
        let repeated = vec![project_entries[0]; 4_096];
        let repeated_bytes = batch_blobs(fixture.directory.path(), &repeated).unwrap();
        assert_eq!(repeated_bytes.len(), repeated.len());
        assert!(
            repeated_bytes
                .iter()
                .all(|bytes| bytes == &repeated_bytes[0])
        );

        let (_view, materialized) = materialize_project_only_at_commit(
            fixture.directory.path(),
            &fixture.index.source.git_commit,
        )
        .unwrap();
        assert_eq!(materialized.frontier_id(), fixture.project.frontier_id());
        assert_eq!(
            events::event_log_hash(&materialized.events),
            events::event_log_hash(&fixture.project.events)
        );
        assert_eq!(
            proposals::proposal_state_hash(&materialized.proposals),
            proposals::proposal_state_hash(&fixture.project.proposals)
        );
    }

    fn replacement_seal_plan(fixture: &GitFixture) -> TargetIndexSealPlan {
        let mut index = fixture.index.clone();
        index.generated_by.version = "0.914.1".to_string();
        index.index_root = index.computed_index_root().unwrap();
        let canonical_json = String::from_utf8(index.canonical_bytes().unwrap()).unwrap();
        TargetIndexSealPlan {
            schema: "vela.target-index-seal-plan.v1",
            frontier_id: index.frontier_id.clone(),
            candidate_path: "/outside/closed-target-candidate.json".to_string(),
            candidate_root: root('f'),
            source: index.source.clone(),
            input_paths: index
                .inputs
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect(),
            packet_paths: index
                .targets
                .iter()
                .map(|target| target.packet.path.clone())
                .collect(),
            index_path: "targets.json",
            index_root: index.index_root.clone(),
            canonical_json,
            index,
            touched_paths: vec!["targets.json".to_string()],
            allowed_dirty_paths: BTreeSet::from(["targets.json".to_string()]),
        }
    }

    fn no_replacement_temporaries(directory: &Path) -> bool {
        std::fs::read_dir(directory)
            .unwrap()
            .flatten()
            .all(|entry| {
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".vela-replace-")
            })
    }

    #[cfg(unix)]
    #[test]
    fn target_index_install_handles_exact_existing_and_absent_preimages() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = git_fixture();
        let plan = replacement_seal_plan(&fixture);
        let target = fixture.directory.path().join("targets.json");

        let prepared = prepare_target_index_seal_install(fixture.directory.path(), &plan).unwrap();
        assert!(prepared.install().unwrap());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            plan.canonical_json.as_bytes()
        );
        assert_eq!(
            std::fs::symlink_metadata(&target)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
        assert!(!install_target_index_seal(fixture.directory.path(), &plan).unwrap());

        std::fs::remove_file(&target).unwrap();
        let mut absent_plan = plan;
        absent_plan.index.generated_by.version = "0.915.1".to_string();
        absent_plan.index.index_root = absent_plan.index.computed_index_root().unwrap();
        absent_plan.index_root = absent_plan.index.index_root.clone();
        absent_plan.canonical_json =
            String::from_utf8(absent_plan.index.canonical_bytes().unwrap()).unwrap();
        let prepared =
            prepare_target_index_seal_install(fixture.directory.path(), &absent_plan).unwrap();
        assert!(prepared.install().unwrap());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            absent_plan.canonical_json.as_bytes()
        );
        assert!(no_replacement_temporaries(fixture.directory.path()));
    }

    #[cfg(unix)]
    #[test]
    fn target_index_install_rejects_a_symlinked_repository_root() {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture();
        let plan = replacement_seal_plan(&fixture);
        let root = fixture.directory.path().to_path_buf();
        let real_root = root.with_extension("real-root");
        let original = std::fs::read(root.join("targets.json")).unwrap();
        std::fs::rename(&root, &real_root).unwrap();
        symlink(&real_root, &root).unwrap();

        let error = prepare_target_index_seal_install(&root, &plan).unwrap_err();
        assert!(
            error.contains("non-symlink repository")
                || error.contains("repository root")
                || error.contains("work tree"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(real_root.join("targets.json")).unwrap(),
            original
        );

        std::fs::remove_file(&root).unwrap();
        std::fs::rename(&real_root, &root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn target_index_absent_leaf_symlink_swap_fails_without_clobber() {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture();
        let plan = replacement_seal_plan(&fixture);
        let target = fixture.directory.path().join("targets.json");
        let outside = fixture.directory.path().with_extension("outside-targets");
        let outside_bytes = b"outside must remain byte-identical";
        std::fs::remove_file(&target).unwrap();
        std::fs::write(&outside, outside_bytes).unwrap();
        let prepared = prepare_target_index_seal_install(fixture.directory.path(), &plan).unwrap();

        let error = prepared
            .install_with_hook(|| symlink(&outside, &target).map_err(|error| error.to_string()))
            .unwrap_err();
        assert!(
            error.contains("targets.json") || error.contains("repository file"),
            "{error}"
        );
        assert_eq!(std::fs::read(&outside).unwrap(), outside_bytes);
        assert!(
            std::fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(no_replacement_temporaries(fixture.directory.path()));
        std::fs::remove_file(&outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn target_index_existing_leaf_symlink_swap_fails_without_clobber() {
        use std::os::unix::fs::symlink;

        let fixture = git_fixture();
        let plan = replacement_seal_plan(&fixture);
        let target = fixture.directory.path().join("targets.json");
        let displaced = fixture.directory.path().join("targets.original.json");
        let original = std::fs::read(&target).unwrap();
        let outside = fixture
            .directory
            .path()
            .with_extension("outside-existing-targets");
        let outside_bytes = b"outside must remain byte-identical";
        std::fs::write(&outside, outside_bytes).unwrap();
        let prepared = prepare_target_index_seal_install(fixture.directory.path(), &plan).unwrap();

        let error = prepared
            .install_with_hook(|| {
                std::fs::rename(&target, &displaced).map_err(|error| error.to_string())?;
                symlink(&outside, &target).map_err(|error| error.to_string())
            })
            .unwrap_err();
        assert!(
            error.contains("targets.json") || error.contains("repository file"),
            "{error}"
        );
        assert_eq!(std::fs::read(&outside).unwrap(), outside_bytes);
        assert_eq!(std::fs::read(&displaced).unwrap(), original);
        assert!(
            std::fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(no_replacement_temporaries(fixture.directory.path()));
        std::fs::remove_file(&outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn target_index_absent_root_parent_swap_fails_closed() {
        let fixture = git_fixture();
        let plan = replacement_seal_plan(&fixture);
        let root = fixture.directory.path().to_path_buf();
        let displaced_root = root.with_extension("displaced-root");
        std::fs::remove_file(root.join("targets.json")).unwrap();
        let prepared = prepare_target_index_seal_install(&root, &plan).unwrap();

        let error = prepared
            .install_with_hook(|| {
                std::fs::rename(&root, &displaced_root).map_err(|error| error.to_string())?;
                std::fs::create_dir(&root).map_err(|error| error.to_string())
            })
            .unwrap_err();
        assert!(
            error.contains("repository parent of targets.json changed"),
            "{error}"
        );
        assert!(!root.join("targets.json").exists());
        assert!(!displaced_root.join("targets.json").exists());
        assert!(no_replacement_temporaries(&displaced_root));

        std::fs::remove_dir(&root).unwrap();
        std::fs::rename(&displaced_root, &root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn target_index_dirt_check_disables_hostile_repo_local_helpers_and_is_read_only() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = git_fixture();
        let git_dir = fixture.directory.path().join(".git");
        let head_before = git_text(fixture.directory.path(), &["rev-parse", "HEAD^{commit}"])
            .expect("read exact HEAD before hostile configuration");
        let index_before = git(fixture.directory.path(), &["ls-files", "--stage", "-z"])
            .expect("read exact index before hostile configuration");
        let targets_before = std::fs::read(fixture.directory.path().join("targets.json")).unwrap();
        let fsmonitor_helper = git_dir.join("hostile-target-index-fsmonitor");
        let fsmonitor_marker = git_dir.join("hostile-target-index-fsmonitor-ran");
        write(
            &fsmonitor_helper,
            format!(
                "#!/bin/sh\nprintf ran > '{}'\nexit 1\n",
                fsmonitor_marker.display()
            ),
        );
        std::fs::set_permissions(&fsmonitor_helper, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let filter_helper = git_dir.join("hostile-target-index-filter");
        let filter_marker = git_dir.join("hostile-target-index-filter-ran");
        write(
            &filter_helper,
            format!(
                "#!/bin/sh\nprintf ran > '{}'\nexit 1\n",
                filter_marker.display()
            ),
        );
        std::fs::set_permissions(&filter_helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        run(
            fixture.directory.path(),
            &[
                "config",
                "core.fsmonitor",
                fsmonitor_helper
                    .to_str()
                    .expect("temporary helper path is UTF-8"),
            ],
        );
        run(
            fixture.directory.path(),
            &[
                "config",
                "filter.hostile.clean",
                filter_helper
                    .to_str()
                    .expect("temporary helper path is UTF-8"),
            ],
        );
        run(
            fixture.directory.path(),
            &["config", "filter.hostile.required", "true"],
        );
        write(
            &fixture.directory.path().join(".gitattributes"),
            b"domain/source.json filter=hostile\n",
        );
        write(
            &fixture.directory.path().join("domain/source.json"),
            br#"{"open":[1056],"hostile":true}"#,
        );

        let error =
            ensure_only_allowed_seal_dirt(fixture.directory.path(), &BTreeSet::new()).unwrap_err();
        assert!(error.contains(".gitattributes"), "{error}");
        assert!(error.contains("domain/source.json"), "{error}");
        assert!(
            !fsmonitor_marker.exists(),
            "target-index dirt inspection executed a repository-configured fsmonitor"
        );
        assert!(
            !filter_marker.exists(),
            "target-index dirt inspection executed a repository-configured clean filter"
        );
        assert_eq!(
            git_text(fixture.directory.path(), &["rev-parse", "HEAD^{commit}"])
                .expect("read exact HEAD after hostile inspection"),
            head_before
        );
        assert_eq!(
            git(fixture.directory.path(), &["ls-files", "--stage", "-z"])
                .expect("read exact index after hostile inspection"),
            index_before
        );
        assert_eq!(
            std::fs::read(fixture.directory.path().join("targets.json")).unwrap(),
            targets_before
        );
    }

    #[test]
    fn target_index_v2_git_assessment_is_fresh_and_exact() {
        let fixture = git_fixture();
        let assessment = assess_target_index(&fixture.project, fixture.directory.path())
            .unwrap()
            .unwrap();
        assert!(assessment.global_issues.is_empty());
        assert!(assessment.target_issues.is_empty());
        assert_eq!(assessment.configured_open(), 1);
        assert_eq!(assessment.stale_open(), 0);
        assert_eq!(assessment.fresh_open_v2_targets()[0].id, "erdos:1056");
        assert_eq!(assessment.v2().unwrap(), &fixture.index);

        let inspection =
            inspect_target_index_target(&fixture.project, fixture.directory.path(), "erdos:1056")
                .unwrap()
                .unwrap();
        assert!(!inspection.actionable, "inspection never grants an offer");
        assert!(!inspection.historical_only);
        assert_eq!(inspection.packet.unwrap()["problem"], 1056);
    }

    #[test]
    fn target_index_v2_worktree_packet_drift_is_stale() {
        let fixture = git_fixture();
        write(
            &fixture.directory.path().join("site/problems/1056.json"),
            br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1","tampered":true}"#,
        );
        let assessment = assess_target_index(&fixture.project, fixture.directory.path())
            .unwrap()
            .unwrap();
        assert!(assessment.fresh_open_v2_targets().is_empty());
        assert_eq!(assessment.stale_open(), 1);
        assert_eq!(
            assessment.target_issues["erdos:1056"][0].code,
            CODE_OUTPUT_NOT_TRACKED
        );
    }

    #[test]
    fn target_task_binding_revalidates_historical_claim_and_exact_outputs() {
        let fixture = git_fixture();
        let assessment = assess_target_index(&fixture.project, fixture.directory.path())
            .unwrap()
            .unwrap();
        let binding = build_target_task_binding(
            &fixture.project,
            fixture.directory.path(),
            &assessment,
            "erdos:1056",
        )
        .unwrap();
        binding.validate().unwrap();
        assert_eq!(binding.target_index_root, fixture.index.index_root);
        assert_eq!(
            binding.claim_read_set.git_commit,
            run(fixture.directory.path(), &["rev-parse", "HEAD^{commit}"])
        );

        write(
            &fixture.directory.path().join("notes.txt"),
            b"later lease-like transport\n",
        );
        run(fixture.directory.path(), &["add", "notes.txt"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "later unrelated commit"],
        );
        revalidate_target_task_binding(&fixture.project, fixture.directory.path(), &binding, None)
            .unwrap();

        write(
            &fixture.directory.path().join("site/problems/1056.json"),
            br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1","drifted":true}"#,
        );
        let error = revalidate_target_task_binding(
            &fixture.project,
            fixture.directory.path(),
            &binding,
            None,
        )
        .unwrap_err();
        assert!(error.contains("stale at landing"), "{error}");
    }

    #[test]
    fn target_index_v2_may_be_resealed_from_a_source_with_an_older_index() {
        let fixture = git_fixture();
        let source_commit = run(fixture.directory.path(), &["rev-parse", "HEAD^{commit}"]);
        let source_tree = run(fixture.directory.path(), &["rev-parse", "HEAD^{tree}"]);
        let mut index = fixture.index.clone();
        index.source.git_commit = source_commit;
        index.source.git_tree = source_tree;
        index.index_root = index.computed_index_root().unwrap();
        write(
            &fixture.directory.path().join("targets.json"),
            index.canonical_bytes().unwrap(),
        );
        run(fixture.directory.path(), &["add", "targets.json"]);
        run(
            fixture.directory.path(),
            &["commit", "-qm", "self reference"],
        );
        let assessment = assess_target_index(&fixture.project, fixture.directory.path())
            .unwrap()
            .unwrap();
        assert!(assessment.global_issues.is_empty());
        assert_eq!(assessment.fresh_open_v2_targets().len(), 1);
    }

    #[test]
    fn target_index_v2_portable_conformance_fixture_is_exact_and_replayable() {
        use std::io::Write as _;

        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("conformance/target-index-v2");
        let manifest_bytes = std::fs::read(fixture_dir.join("fixture.manifest.json"))
            .expect("read fixture manifest");
        let manifest: Value =
            serde_json::from_slice(&manifest_bytes).expect("parse fixture manifest");
        assert_eq!(
            manifest.get("schema").and_then(Value::as_str),
            Some("vela.target-index-conformance-manifest.v1")
        );
        assert_eq!(
            manifest
                .as_object()
                .expect("manifest object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["doctrine", "files", "schema"])
        );

        let expected_files = BTreeSet::from([
            "candidate.json",
            "expected.json",
            "input-manifest.json",
            "packet.json",
            "repository.fast-import",
            "target-task-binding.json",
            "targets.json",
        ]);
        let entries = manifest
            .get("files")
            .and_then(Value::as_array)
            .expect("manifest files");
        let mut listed_files = BTreeSet::new();
        for entry in entries {
            let object = entry.as_object().expect("manifest file entry");
            assert_eq!(
                object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                BTreeSet::from(["bytes", "path", "sha256"])
            );
            let name = object
                .get("path")
                .and_then(Value::as_str)
                .expect("manifest path");
            assert!(
                listed_files.insert(name),
                "duplicate fixture manifest path {name}"
            );
            let bytes = std::fs::read(fixture_dir.join(name))
                .unwrap_or_else(|error| panic!("read fixture {name}: {error}"));
            let actual_digest = sha256_root(&bytes);
            assert_eq!(
                object.get("bytes").and_then(Value::as_u64),
                Some(bytes.len() as u64),
                "fixture byte count drifted for {name}"
            );
            assert_eq!(
                object.get("sha256").and_then(Value::as_str),
                Some(actual_digest.as_str()),
                "fixture digest drifted for {name}"
            );
        }
        assert_eq!(listed_files, expected_files);
        let disk_files = std::fs::read_dir(&fixture_dir)
            .expect("list target-index fixture directory")
            .map(|entry| {
                entry
                    .expect("read target-index fixture directory entry")
                    .file_name()
                    .into_string()
                    .expect("fixture file name is UTF-8")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            disk_files,
            expected_files
                .iter()
                .copied()
                .chain(["fixture.manifest.json"])
                .map(str::to_string)
                .collect()
        );

        let expected_bytes =
            std::fs::read(fixture_dir.join("expected.json")).expect("read expected fixture values");
        let expected: Value =
            serde_json::from_slice(&expected_bytes).expect("parse expected fixture values");
        assert_eq!(
            expected.get("schema").and_then(Value::as_str),
            Some("vela.target-index-conformance-expected.v1")
        );
        assert_eq!(
            expected.get("git_object_format").and_then(Value::as_str),
            Some("sha1")
        );
        let fixed_commits = [
            (
                "A",
                "A",
                "0c8cc9e6b3f98a5581a3e9d459bb73962ec6fad6",
                "7919a7d199aba59dcf1bad74bfdfe42e16489772",
            ),
            (
                "B",
                "B",
                "725e991e44aad89f84e987bad46447e71753ef22",
                "5639cac381a4b35fd963e07cbc2e5847201a8885",
            ),
            (
                "C",
                "C",
                "225abdf5494ab15d7b29fc3ecd4ee3983f2d4d46",
                "50c6de175d946a811503c0e7d68fbbf8f9a7b65b",
            ),
            (
                "D",
                "D",
                "4549c014332609b7bc5e0e7c4d864353f8969454",
                "f1c3af4fe251736bfe8e9dfdc467d71b60ef9b7b",
            ),
            (
                "D_nonlease",
                "D-nonlease",
                "78bbd7f9baa006bdd1d10264a5ce351eba17dd99",
                "6210487490aeb07bb970e847d73b32b197854df2",
            ),
        ];
        for (name, _, commit, tree) in fixed_commits {
            assert_eq!(
                expected
                    .pointer(&format!("/commits/{name}/commit"))
                    .and_then(Value::as_str),
                Some(commit)
            );
            assert_eq!(
                expected
                    .pointer(&format!("/commits/{name}/tree"))
                    .and_then(Value::as_str),
                Some(tree)
            );
        }
        let expected_root = |name: &str| {
            expected
                .pointer(&format!("/roots/{name}"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing expected root {name}"))
        };

        let candidate_bytes =
            std::fs::read(fixture_dir.join("candidate.json")).expect("read candidate fixture");
        assert_eq!(
            sha256_root(&candidate_bytes),
            expected_root("candidate_document_root")
        );
        let candidate: TargetIndexCandidateV1 =
            serde_json::from_slice(&candidate_bytes).expect("parse candidate fixture");
        candidate.validate().expect("validate candidate fixture");

        let inputs_bytes = std::fs::read(fixture_dir.join("input-manifest.json"))
            .expect("read input manifest fixture");
        let inputs: TargetIndexInputManifestV1 =
            serde_json::from_slice(&inputs_bytes).expect("parse input manifest fixture");
        inputs.validate().expect("validate input manifest fixture");
        assert_eq!(inputs.input_root, expected_root("input_root"));
        assert_eq!(
            canonical::to_canonical_bytes(&inputs).expect("canonical input manifest"),
            inputs_bytes
        );

        let packet_bytes =
            std::fs::read(fixture_dir.join("packet.json")).expect("read packet fixture");
        assert_eq!(sha256_root(&packet_bytes), expected_root("packet_root"));
        let packet: Value = serde_json::from_slice(&packet_bytes).expect("parse packet fixture");
        assert_eq!(
            packet.get("schema").and_then(Value::as_str),
            Some("erdos-frontier.problem-work.v1")
        );

        let index_bytes =
            std::fs::read(fixture_dir.join("targets.json")).expect("read target index fixture");
        let index: TargetIndexV2 =
            serde_json::from_slice(&index_bytes).expect("parse target index fixture");
        assert_eq!(index.index_root, expected_root("index_root"));
        assert_eq!(index.inputs, inputs);
        assert_eq!(index.targets[0].packet.sha256, expected_root("packet_root"));
        assert_eq!(index.targets[0].packet.size, packet_bytes.len() as u64);
        index.validate().expect("validate target index fixture");
        assert_eq!(
            index.canonical_bytes().expect("canonical target index"),
            index_bytes
        );

        let binding_bytes = std::fs::read(fixture_dir.join("target-task-binding.json"))
            .expect("read target task binding fixture");
        assert_eq!(
            sha256_root(&binding_bytes),
            expected_root("binding_document_root")
        );
        let binding: TargetTaskBindingV1 =
            serde_json::from_slice(&binding_bytes).expect("parse target task binding fixture");
        assert_eq!(binding.binding_root, expected_root("binding_root"));
        assert_eq!(binding.target_index_root, expected_root("index_root"));
        assert_eq!(binding.packet.sha256, expected_root("packet_root"));
        binding
            .validate()
            .expect("validate target task binding fixture");
        assert_eq!(
            canonical::to_canonical_bytes(&binding).expect("canonical target task binding"),
            binding_bytes
        );

        let imported = tempfile::tempdir().expect("create imported fixture repository");
        run(imported.path(), &["init", "-q", "-b", "main"]);
        let mut child = Command::new("git")
            .arg("-C")
            .arg(imported.path())
            .args(["fast-import", "--quiet"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("spawn git fast-import");
        child
            .stdin
            .as_mut()
            .expect("open git fast-import stdin")
            .write_all(
                &std::fs::read(fixture_dir.join("repository.fast-import"))
                    .expect("read fast-import stream"),
            )
            .expect("write fast-import stream");
        assert!(
            child.wait().expect("wait for git fast-import").success(),
            "git fast-import failed"
        );
        for (_, reference_suffix, commit, tree) in fixed_commits {
            let reference = format!("refs/heads/fixture-{reference_suffix}");
            assert_eq!(
                run(
                    imported.path(),
                    &["rev-parse", &format!("{reference}^{{commit}}")]
                ),
                commit
            );
            assert_eq!(
                run(
                    imported.path(),
                    &["rev-parse", &format!("{reference}^{{tree}}")]
                ),
                tree
            );
        }

        run(imported.path(), &["checkout", "-qf", "fixture-A"]);
        let project_a = repo::load_from_path(imported.path()).expect("load A project");
        assert!(
            assess_target_index(&project_a, imported.path())
                .expect("assess A")
                .is_none(),
            "A must contain no Target Index"
        );

        for branch in ["fixture-B", "fixture-C", "fixture-D"] {
            run(imported.path(), &["checkout", "-qf", branch]);
            let project = repo::load_from_path(imported.path())
                .unwrap_or_else(|error| panic!("load {branch} project: {error}"));
            let assessment = assess_target_index(&project, imported.path())
                .unwrap_or_else(|error| panic!("assess {branch}: {error}"))
                .unwrap_or_else(|| panic!("{branch} must contain a Target Index"));
            assert!(
                assessment.global_issues.is_empty(),
                "{branch} global issues: {:?}",
                assessment.global_issues
            );
            assert_eq!(assessment.fresh_open_v2_targets().len(), 1, "{branch}");
            assert_eq!(assessment.stale_open(), 0, "{branch}");
            if branch == "fixture-D" {
                revalidate_target_task_binding(&project, imported.path(), &binding, None)
                    .expect("revalidate exact D task binding");
            }
        }

        run(imported.path(), &["checkout", "-qf", "fixture-D-nonlease"]);
        let nonlease_project =
            repo::load_from_path(imported.path()).expect("load D-nonlease project");
        let nonlease_assessment = assess_target_index(&nonlease_project, imported.path())
            .expect("assess D-nonlease")
            .expect("D-nonlease contains a Target Index");
        assert!(nonlease_assessment.fresh_open_v2_targets().is_empty());
        assert_eq!(nonlease_assessment.stale_open(), 1);
        assert!(
            nonlease_assessment
                .all_codes()
                .contains(&CODE_EVENT_ROOT_MISMATCH)
        );

        let shallow_parent = tempfile::tempdir().expect("create shallow clone parent");
        let shallow_path = shallow_parent.path().join("repo");
        let source_url = format!("file://{}", imported.path().display());
        let shallow = Command::new("git")
            .args([
                "clone",
                "-q",
                "--depth",
                "1",
                "--single-branch",
                "--branch",
                "fixture-B",
                &source_url,
            ])
            .arg(&shallow_path)
            .output()
            .expect("clone shallow fixture repository");
        assert!(
            shallow.status.success(),
            "git shallow clone: {}",
            String::from_utf8_lossy(&shallow.stderr)
        );
        let shallow_project = repo::load_from_path(&shallow_path).expect("load shallow B project");
        let shallow_assessment = assess_target_index(&shallow_project, &shallow_path)
            .expect("assess shallow B")
            .expect("shallow B contains a Target Index");
        assert!(shallow_assessment.fresh_open_v2_targets().is_empty());
        assert!(
            shallow_assessment
                .all_codes()
                .contains(&CODE_SOURCE_UNAVAILABLE)
        );
    }
}
