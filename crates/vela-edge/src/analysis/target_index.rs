//! Current Target Index values and read-only repository assessment.
//!
//! Target catalogues are derived briefing projections. This module proves
//! their exact Git inputs and current repository context before they can
//! become producer offers; it does not rank domain work or grant authority.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Output, Stdio};

use super::repository_write::{PreparedRepositoryFileReplacement, RepositoryFileReplacementMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use vela_protocol::canonical;
use vela_protocol::current_repository::CurrentRepositoryV3;
use vela_protocol::repository_inputs::{
    GitObjectFormat, RetainedObjectEntryV1, RetainedObjectManifestV1,
};

pub const TARGET_INDEX_SCHEMA_V4: &str = "vela.target-index.v4";
pub const TARGET_INDEX_CANDIDATE_SCHEMA_V1: &str = "vela.target-index-candidate.v1";
pub const TARGET_INDEX_INPUT_MANIFEST_SCHEMA_V1: &str = "vela.target-index-input-manifest.v1";
pub const TARGET_TASK_BINDING_SCHEMA_V3: &str = "vela.target-task-binding.v3";

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

/// Current-repository binding for a derived Target Index.
///
/// Unlike Target Index v2, this binding does not retain Era-0 event,
/// proposal, identity, or dependency roots. The exact current repository
/// manifest is the sole scientific-state read boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexRepositoryV4 {
    pub origin_id: String,
    pub repository_root: String,
}

/// Event-free Target Index for current repository origins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetIndexV4 {
    pub schema: String,
    pub frontier_id: String,
    pub source: TargetIndexSourceV2,
    pub inputs: TargetIndexInputManifestV1,
    pub repository: TargetIndexRepositoryV4,
    pub claim_boundary: TargetIndexClaimBoundaryV2,
    pub generated_by: TargetIndexGeneratorV2,
    pub targets: Vec<TargetIndexEntryV2>,
    pub index_root: String,
}

#[derive(Debug, Clone)]
pub struct CurrentTargetIndexAssessment {
    pub index: TargetIndexV4,
    pub global_issues: Vec<TargetIndexIssue>,
    pub target_issues: BTreeMap<String, Vec<TargetIndexIssue>>,
    packet_values: BTreeMap<String, Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTaskClaimReadSetV2 {
    pub git_object_format: GitObjectFormat,
    pub git_commit: String,
    pub git_tree: String,
}

/// Exact producer-task binding for a current repository origin.
///
/// The event-rooted read set in v1 is intentionally absent. Scientific state
/// is the exact current repository manifest; Git retains the producer's
/// reproducible starting point without becoming an authority surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetTaskBindingV3 {
    pub schema: String,
    pub frontier_id: String,
    pub target_id: String,
    pub target_index_root: String,
    pub source: TargetIndexSourceV2,
    pub input_root: String,
    pub packet: TargetPacketRefV2,
    pub repository: TargetIndexRepositoryV4,
    pub claim_read_set: TargetTaskClaimReadSetV2,
    pub binding_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetIndexIssue {
    pub code: &'static str,
    pub target_id: Option<String>,
    pub message: String,
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
/// Complete, write-free result of sealing one domain-owned candidate for a
/// current Profile v2 repository. The exact repository origin and manifest
/// root replace every Era-0 event/proposal root.
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
    pub index: TargetIndexV4,
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
    if vela_protocol::execution_binding::is_full_sha256_root(value) {
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

fn require_origin_id(field: &str, value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("vro_") else {
        return Err(format!("{field} must use the vro_<16 lowercase hex> form"));
    };
    if suffix.len() == 16
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!("{field} must use the vro_<16 lowercase hex> form"))
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

fn validate_target_fields(
    id: &str,
    title: &str,
    why: &str,
    state: &str,
    rank: u64,
    objective: &str,
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
    bounded_text(objective, "target.objective", 4_096)
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
    validate_target_fields(id, title, why, state, rank, objective)?;
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

fn validate_target_index_common(
    frontier_id: &str,
    source: &TargetIndexSourceV2,
    inputs: &TargetIndexInputManifestV1,
    claim_boundary: &TargetIndexClaimBoundaryV2,
    generated_by: &TargetIndexGeneratorV2,
    targets: &[TargetIndexEntryV2],
) -> Result<(), String> {
    require_frontier_id(frontier_id)?;
    require_git_object(
        "source.git_commit",
        &source.git_commit,
        source.git_object_format,
    )?;
    require_git_object(
        "source.git_tree",
        &source.git_tree,
        source.git_object_format,
    )?;
    inputs.validate()?;
    if claim_boundary
        != &(TargetIndexClaimBoundaryV2 {
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
    if generated_by.program != "vela" {
        return Err("generated_by.program must be `vela`".to_string());
    }
    validate_semver(&generated_by.version)?;
    if targets.len() > TARGET_INDEX_MAX_TARGETS {
        return Err(format!(
            "target index has {} targets; limit is {TARGET_INDEX_MAX_TARGETS}",
            targets.len()
        ));
    }
    for target in targets {
        target.validate()?;
    }
    validate_unique_target_ids(targets.iter().map(|target| target.id.as_str()))?;
    validate_target_order(
        targets
            .iter()
            .map(|target| (target.id.as_str(), target.rank)),
    )?;
    let packet_paths = targets
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
    for input in &inputs.entries {
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
    Ok(())
}

impl TargetIndexV4 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_INDEX_SCHEMA_V4 {
            return Err(format!("schema must be {TARGET_INDEX_SCHEMA_V4}"));
        }
        validate_target_index_common(
            &self.frontier_id,
            &self.source,
            &self.inputs,
            &self.claim_boundary,
            &self.generated_by,
            &self.targets,
        )?;
        require_origin_id("repository.origin_id", &self.repository.origin_id)?;
        require_sha256_root(
            "repository.repository_root",
            &self.repository.repository_root,
        )?;
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

impl TargetTaskBindingV3 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_TASK_BINDING_SCHEMA_V3 {
            return Err(format!(
                "binding.schema must be {TARGET_TASK_BINDING_SCHEMA_V3}"
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
        require_origin_id("repository.origin_id", &self.repository.origin_id)?;
        require_sha256_root(
            "repository.repository_root",
            &self.repository.repository_root,
        )?;
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

fn exact_current_repository(
    repo_path: &Path,
    frontier_id: &str,
    origin_id: &str,
    repository_root: &str,
) -> Result<CurrentRepositoryV3, String> {
    let path = repo_path.join(".vela/repository.json");
    let bytes = read_regular_file(&path, 8 * 1024 * 1024, "current repository manifest")?;
    let tracked = exact_tracked_head_bytes(repo_path, ".vela/repository.json", 8 * 1024 * 1024)?;
    if tracked != bytes {
        return Err(
            "current repository manifest differs from the exact tracked HEAD blob".to_string(),
        );
    }
    let repository = CurrentRepositoryV3::parse(&bytes)?;
    if repository.frontier_id != frontier_id
        || repository.origin_id != origin_id
        || repository.canonical_root()? != repository_root
    {
        return Err(
            "current repository manifest does not match the requested Frontier origin and root"
                .to_string(),
        );
    }
    Ok(repository)
}

/// Build the exact private-work binding for a current repository Target Offer.
///
/// This is read-only. It does not create a lease event, consult Era-0 replay,
/// or require any authority credential.
pub fn build_current_target_task_binding(
    repo_path: &Path,
    assessment: &CurrentTargetIndexAssessment,
    frontier_id: &str,
    origin_id: &str,
    repository_root: &str,
    target_id: &str,
) -> Result<TargetTaskBindingV3, String> {
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        return Err(format!(
            "current target task binding refuses stale or invalid target {target_id:?}"
        ));
    }
    let target = assessment
        .index
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("current target task binding cannot find target {target_id:?}"))?;
    if target.state != "open" {
        return Err(format!(
            "current target task binding requires an open target; {target_id:?} is {}",
            target.state
        ));
    }
    if assessment.index.frontier_id != frontier_id
        || assessment.index.repository.origin_id != origin_id
        || assessment.index.repository.repository_root != repository_root
    {
        return Err(
            "current target task binding index does not match the exact current repository"
                .to_string(),
        );
    }
    exact_current_repository(repo_path, frontier_id, origin_id, repository_root)?;

    let git_object_format = repository_object_format(repo_path)?;
    let git_commit = git_text(repo_path, &["rev-parse", "HEAD^{commit}"])?;
    require_git_object("claim_read_set.git_commit", &git_commit, git_object_format)?;
    let git_tree = git_text(repo_path, &["rev-parse", "HEAD^{tree}"])?;
    require_git_object("claim_read_set.git_tree", &git_tree, git_object_format)?;
    let mut binding = TargetTaskBindingV3 {
        schema: TARGET_TASK_BINDING_SCHEMA_V3.to_string(),
        frontier_id: frontier_id.to_string(),
        target_id: target.id.clone(),
        target_index_root: assessment.index.index_root.clone(),
        source: assessment.index.source.clone(),
        input_root: assessment.index.inputs.input_root.clone(),
        packet: target.packet.clone(),
        repository: assessment.index.repository.clone(),
        claim_read_set: TargetTaskClaimReadSetV2 {
            git_object_format,
            git_commit,
            git_tree,
        },
        binding_root: format!("sha256:{}", "0".repeat(64)),
    };
    binding.binding_root = binding.computed_binding_root()?;
    binding.validate()?;
    Ok(binding)
}

/// Revalidate a retained current task binding before creating a Submission.
///
/// Descendant Git commits are allowed, but the exact repository, target
/// index, packet, source, and original claim tree must remain available.
pub fn revalidate_current_target_task_binding(
    repo_path: &Path,
    binding: &TargetTaskBindingV3,
) -> Result<(), String> {
    binding.validate()?;
    exact_current_repository(
        repo_path,
        &binding.frontier_id,
        &binding.repository.origin_id,
        &binding.repository.repository_root,
    )?;
    let resolved = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{commit}}", binding.claim_read_set.git_commit),
        ],
    )?;
    if resolved != binding.claim_read_set.git_commit {
        return Err("current target binding claim commit did not resolve exactly".to_string());
    }
    let claim_tree = git_text(
        repo_path,
        &[
            "rev-parse",
            &format!("{}^{{tree}}", binding.claim_read_set.git_commit),
        ],
    )?;
    if claim_tree != binding.claim_read_set.git_tree {
        return Err("current target binding claim tree changed".to_string());
    }
    let assessment = assess_current_target_index(
        repo_path,
        &binding.frontier_id,
        &binding.repository.origin_id,
        &binding.repository.repository_root,
    )?
    .ok_or_else(|| "current target binding Target Index is unavailable".to_string())?;
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(&binding.target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        return Err("current target binding index or packet is stale".to_string());
    }
    let target = assessment
        .index
        .targets
        .iter()
        .find(|target| target.id == binding.target_id)
        .ok_or_else(|| "current target binding target is absent".to_string())?;
    if target.state != "open"
        || binding.target_index_root != assessment.index.index_root
        || binding.source != assessment.index.source
        || binding.input_root != assessment.index.inputs.input_root
        || binding.packet != target.packet
        || binding.repository != assessment.index.repository
    {
        return Err(
            "current target binding no longer matches its index, source, inputs, packet, or repository"
                .to_string(),
        );
    }
    Ok(())
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

fn index_entries(repo: &Path) -> Result<BTreeMap<String, GitTreeEntry>, String> {
    let output = git(repo, &["ls-files", "--stage", "-z"])?;
    let mut entries = BTreeMap::new();
    for raw in output
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let parsed = parse_index_record(raw)?;
        if entries.insert(parsed.path.clone(), parsed).is_some() {
            return Err("Git index contains duplicate stage-0 paths".to_string());
        }
    }
    Ok(entries)
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

/// Verify many tracked packet paths with one tree read, one index read, and one
/// batched blob process.
///
/// The per-path verifier above is appropriate at a transaction edge. A read
/// projection can contain thousands of packets, however, and spawning three
/// Git processes for each packet made `status` scale with process-launch
/// overhead rather than packet bytes. This batch retains the same HEAD/index/
/// worktree equality checks and returns an independent result per requested
/// path so one stale packet does not hide the rest of the assessment.
fn exact_tracked_head_bytes_batch(
    repo: &Path,
    paths: impl IntoIterator<Item = String>,
    max_bytes: u64,
) -> Result<BTreeMap<String, Result<Vec<u8>, String>>, String> {
    let requested = paths.into_iter().collect::<BTreeSet<_>>();
    for path in &requested {
        validate_repository_path(path, "Git path", 1_024)?;
    }

    let head_entries = tree_entries(repo, "HEAD")?
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let staged_entries = index_entries(repo)?;
    let mut results = BTreeMap::new();
    let mut valid = Vec::new();

    for path in requested {
        let Some(head) = head_entries.get(&path) else {
            results.insert(path.clone(), Err(format!("{path:?} is absent from HEAD")));
            continue;
        };
        let Some(staged) = staged_entries.get(&path) else {
            results.insert(
                path.clone(),
                Err(format!("{path:?} is absent from the Git index")),
            );
            continue;
        };
        if !matches!(head.mode.as_str(), "100644" | "100755")
            || head.kind != "blob"
            || staged.mode != head.mode
            || staged.object != head.object
        {
            results.insert(
                path.clone(),
                Err(format!(
                    "{path:?} must be an unchanged tracked regular file in HEAD and the Git index"
                )),
            );
            continue;
        }
        valid.push((path, head.clone()));
    }

    let blobs = batch_blobs(
        repo,
        &valid.iter().map(|(_, entry)| entry).collect::<Vec<_>>(),
    )?;
    for ((path, _), blob_bytes) in valid.into_iter().zip(blobs) {
        let result = safe_worktree_file(repo, &path, max_bytes).and_then(|worktree| {
            if worktree == blob_bytes {
                Ok(worktree)
            } else {
                Err(format!("{path:?} working bytes do not match HEAD"))
            }
        });
        results.insert(path, result);
    }
    Ok(results)
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

/// Derive a complete Target Index v4 for one current repository.
///
/// This function performs no writes. Candidate semantics come only from the
/// closed domain-owned candidate. Vela derives the exact Git input manifest,
/// packet roots, repository binding, and index root.
pub fn prepare_current_target_index_seal(
    repo_path: &Path,
    candidate_path: &Path,
    binary_version: &str,
    frontier_id: &str,
    origin_id: &str,
    repository_root: &str,
) -> Result<TargetIndexSealPlan, String> {
    validate_semver(binary_version)?;
    require_frontier_id(frontier_id)?;
    require_origin_id("origin_id", origin_id)?;
    require_sha256_root("repository_root", repository_root)?;
    exact_current_repository(repo_path, frontier_id, origin_id, repository_root)?;

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
    if candidate.frontier_id != frontier_id {
        return Err(format!(
            "{CODE_FRONTIER_MISMATCH}: candidate Frontier {} differs from current repository {frontier_id}",
            candidate.frontier_id
        ));
    }
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
    let source = TargetIndexSourceV2 {
        git_object_format,
        git_commit: candidate.source.git_commit.clone(),
        git_tree: git_text(
            repo_path,
            &[
                "rev-parse",
                &format!("{}^{{tree}}", candidate.source.git_commit),
            ],
        )?,
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

    let mut index = TargetIndexV4 {
        schema: TARGET_INDEX_SCHEMA_V4.to_string(),
        frontier_id: frontier_id.to_string(),
        source: source.clone(),
        inputs,
        repository: TargetIndexRepositoryV4 {
            origin_id: origin_id.to_string(),
            repository_root: repository_root.to_string(),
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
        index_root: String::new(),
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

/// Pin a current-repository target-index destination and its exact preimage
/// before the current repository write gate is evaluated.
pub fn prepare_current_target_index_seal_install(
    repo_path: &Path,
    plan: &TargetIndexSealPlan,
) -> Result<PreparedTargetIndexSealInstall, String> {
    ensure_only_allowed_seal_dirt(repo_path, &plan.allowed_dirty_paths)?;
    let replacement = PreparedRepositoryFileReplacement::prepare_observed(
        repo_path,
        Path::new(plan.index_path),
        plan.canonical_json.as_bytes(),
        RepositoryFileReplacementMode::Exact(0o644),
        TARGET_INDEX_JSON_MAX_BYTES,
    )?;
    Ok(PreparedTargetIndexSealInstall { replacement })
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

fn assess_open_target_packets(
    repo_path: &Path,
    targets: &[TargetIndexEntryV2],
    target_issues: &mut BTreeMap<String, Vec<TargetIndexIssue>>,
    packet_values: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    let open_packet_bytes = exact_tracked_head_bytes_batch(
        repo_path,
        targets
            .iter()
            .filter(|target| target.state == "open")
            .map(|target| target.packet.path.clone()),
        TARGET_PACKET_MAX_BYTES,
    )?;
    for target in targets.iter().filter(|target| target.state == "open") {
        match open_packet_bytes.get(&target.packet.path) {
            Some(Ok(packet_bytes)) => {
                let digest = sha256_root(packet_bytes);
                if packet_bytes.len() as u64 != target.packet.size || digest != target.packet.sha256
                {
                    push_target_issue(
                        target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        format!(
                            "packet bytes at {:?} differ from the sealed size or digest",
                            target.packet.path
                        ),
                    );
                    continue;
                }
                match serde_json::from_slice::<Value>(packet_bytes) {
                    Ok(packet)
                        if packet.is_object()
                            && packet.get("schema").and_then(Value::as_str)
                                == Some(target.packet.schema.as_str()) =>
                    {
                        packet_values.insert(target.id.clone(), packet);
                    }
                    Ok(_) => push_target_issue(
                        target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        "packet must be one JSON object with the exact sealed schema",
                    ),
                    Err(error) => push_target_issue(
                        target_issues,
                        &target.id,
                        CODE_PACKET_MISMATCH,
                        format!("packet JSON is invalid: {error}"),
                    ),
                }
            }
            Some(Err(error)) => {
                push_target_issue(target_issues, &target.id, CODE_OUTPUT_NOT_TRACKED, error)
            }
            None => push_target_issue(
                target_issues,
                &target.id,
                CODE_OUTPUT_NOT_TRACKED,
                "packet path was absent from the batched tracked-file assessment",
            ),
        }
    }
    Ok(())
}

fn validate_input_git_bytes_for(
    repo_path: &Path,
    source_binding: &TargetIndexSourceV2,
    inputs: &TargetIndexInputManifestV1,
) -> Result<Vec<TargetIndexIssue>, String> {
    let mut issues = Vec::new();
    let input_root = inputs.computed_root()?;
    if input_root != inputs.input_root {
        issues.push(issue(
            CODE_INPUT_ROOT_MISMATCH,
            format!(
                "declared input root {} differs from derived {input_root}",
                inputs.input_root
            ),
        ));
    }
    for declared in &inputs.entries {
        let source = tree_entry(repo_path, &source_binding.git_commit, &declared.path)?;
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

/// Assess the event-free Target Index used by current repository origins.
///
/// Scientific standing is bound only through the exact current repository
/// root. Git source/input checks and packet checks retain the same fail-closed
/// work-advice guarantees as Target Index v2 without consulting Era-0 state.
pub fn assess_current_target_index(
    repo_path: &Path,
    frontier_id: &str,
    origin_id: &str,
    repository_root: &str,
) -> Result<Option<CurrentTargetIndexAssessment>, String> {
    require_frontier_id(frontier_id)?;
    require_origin_id("origin_id", origin_id)?;
    require_sha256_root("repository_root", repository_root)?;
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
    if schema != TARGET_INDEX_SCHEMA_V4 {
        return Err(format!(
            "{CODE_PROFILE_UPGRADE_REQUIRED}: current repository requires {TARGET_INDEX_SCHEMA_V4}, found {schema}"
        ));
    }
    let index: TargetIndexV4 = serde_json::from_value(envelope)
        .map_err(|error| format!("{CODE_SCHEMA_INVALID}: parse v4 index: {error}"))?;
    index.validate()?;
    if index.canonical_bytes()? != bytes {
        return Err(format!(
            "{CODE_SCHEMA_INVALID}: tracked targets.json must be exact canonical JSON without whitespace or a trailing newline"
        ));
    }

    let mut global_issues = Vec::new();
    let mut target_issues = BTreeMap::new();
    let mut packet_values = BTreeMap::new();
    if index.frontier_id != frontier_id {
        global_issues.push(issue(
            CODE_FRONTIER_MISMATCH,
            "index Frontier differs from the current repository",
        ));
    }
    if index.repository.origin_id != origin_id
        || index.repository.repository_root != repository_root
    {
        global_issues.push(issue(
            CODE_STATE_ROOT_MISMATCH,
            "index current-repository binding differs from the exact current repository",
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
        global_issues.extend(validate_input_git_bytes_for(
            repo_path,
            &index.source,
            &index.inputs,
        )?);
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

    assess_open_target_packets(
        repo_path,
        &index.targets,
        &mut target_issues,
        &mut packet_values,
    )?;
    sort_issues(&mut global_issues);
    for issues in target_issues.values_mut() {
        sort_issues(issues);
    }
    Ok(Some(CurrentTargetIndexAssessment {
        index,
        global_issues,
        target_issues,
        packet_values,
    }))
}

impl CurrentTargetIndexAssessment {
    pub fn configured_open(&self) -> usize {
        self.index
            .targets
            .iter()
            .filter(|target| target.state == "open")
            .count()
    }

    pub fn fresh_open_targets(&self) -> Vec<&TargetIndexEntryV2> {
        if !self.global_issues.is_empty() {
            return Vec::new();
        }
        self.index
            .targets
            .iter()
            .filter(|target| {
                target.state == "open"
                    && self.target_issues.get(&target.id).is_none_or(Vec::is_empty)
            })
            .collect()
    }

    pub fn packet_value(&self, target_id: &str) -> Option<&Value> {
        self.packet_values.get(target_id)
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
