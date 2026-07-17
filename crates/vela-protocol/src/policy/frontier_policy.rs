//! Frontier-owned policy files.
//!
//! Policy guides local review and validation. It is not evidence and it does
//! not mutate frontier state by itself.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::{canonical, repo};

pub const ENGINE_POLICY_MANIFEST_MAX_BYTES: usize = 256 * 1024;
pub const ENGINE_POLICY_DOCUMENT_MAX_BYTES: usize = 256 * 1024;
pub const ENGINE_POLICY_SUMMARY_OBSERVATION_SCHEMA: &str =
    "vela.engine-policy-summary-observation.v1";
const ENGINE_POLICY_SUMMARY_ROOT_DOMAIN: &str = "vela.engine-policy-summary-root.v1";
const ENGINE_POLICY_ERROR_ROOT_DOMAIN: &str = "vela.engine-policy-error-root.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDocumentKind {
    Evidence,
    Review,
    Confidence,
    Agent,
}

impl PolicyDocumentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Review => "review",
            Self::Confidence => "confidence",
            Self::Agent => "agent",
        }
    }

    pub fn filename(self) -> &'static str {
        match self {
            Self::Evidence => "evidence_policy.md",
            Self::Review => "review_policy.md",
            Self::Confidence => "confidence_policy.md",
            Self::Agent => "agent_policy.md",
        }
    }

    fn all() -> [Self; 4] {
        [Self::Evidence, Self::Review, Self::Confidence, Self::Agent]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDocumentSummary {
    pub kind: PolicyDocumentKind,
    pub path: String,
    pub title: String,
    pub body_sha256: String,
    pub bytes: usize,
    pub declared_in_manifest: bool,
    #[serde(default)]
    pub front_matter: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierPolicySummary {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_id: Option<String>,
    pub frontier_path: String,
    #[serde(default)]
    pub documents: Vec<PolicyDocumentSummary>,
    #[serde(default)]
    pub missing_required: Vec<String>,
    /// True when the frontier declares at least one policy path or carries at
    /// least one document at the conventional default path. This separates a
    /// deliberately unconfigured frontier from a configured-but-broken one.
    #[serde(default)]
    pub configured: bool,
    pub defaults_used: bool,
    pub canonical_json_sha256: String,
}

/// Read-only, bounded observation of the policy inputs consumed by Engine
/// preview. It never contains an absolute path or raw parser/filesystem error.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnginePolicySummaryState {
    Present,
    Absent,
    Invalid,
}

impl EnginePolicySummaryState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnginePolicySummaryObservation {
    pub schema: String,
    pub state: EnginePolicySummaryState,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_root: Option<String>,
}

#[derive(Debug)]
struct PolicyLoadError {
    code: &'static str,
    reference: String,
    input_root: Option<String>,
    message: String,
}

impl PolicyLoadError {
    fn new(
        code: &'static str,
        reference: impl Into<String>,
        input_root: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            reference: reference.into(),
            input_root,
            message: message.into(),
        }
    }

    fn io(code: &'static str, reference: &str, error: &std::io::Error) -> Self {
        let kind = format!("{:?}", error.kind()).to_ascii_lowercase();
        Self::new(
            code,
            reference,
            Some(text_root(&kind)),
            format!("{reference}: filesystem operation failed ({kind})"),
        )
    }

    fn stable_root(&self) -> String {
        domain_hash(
            ENGINE_POLICY_ERROR_ROOT_DOMAIN,
            &[
                Some(self.code),
                Some(&self.reference),
                self.input_root.as_deref(),
            ],
        )
    }
}

impl std::fmt::Display for PolicyLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "[{}] {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationReviewRequirement {
    pub review_class: String,
    pub required_reviewer_count: usize,
    #[serde(default)]
    pub reviewer_roles: Vec<String>,
    #[serde(default)]
    pub required_reason_fields: Vec<String>,
    #[serde(default)]
    pub allowed_agent_actions: Vec<String>,
    #[serde(default)]
    pub policy_sources: Vec<String>,
}

pub fn load_policy_summary(frontier_path: &Path) -> Result<FrontierPolicySummary, String> {
    load_policy_summary_inner(frontier_path, true).map_err(|error| error.to_string())
}

/// Observe the exact bounded policy inputs that Engine preview would read.
/// Invalid inputs remain cursor-bindable without exposing raw error text.
#[must_use]
pub fn engine_policy_summary_observation(frontier_path: &Path) -> EnginePolicySummaryObservation {
    match load_policy_summary_inner(frontier_path, false) {
        Ok(summary) => {
            let state = if summary.configured {
                EnginePolicySummaryState::Present
            } else {
                EnginePolicySummaryState::Absent
            };
            let summary_root = Some(summary.canonical_json_sha256);
            let root = observation_root(state, summary_root.as_deref(), None, None);
            EnginePolicySummaryObservation {
                schema: ENGINE_POLICY_SUMMARY_OBSERVATION_SCHEMA.to_string(),
                state,
                root,
                summary_root,
                error_code: None,
                error_root: None,
            }
        }
        Err(error) => {
            let error_root = error.stable_root();
            let root = observation_root(
                EnginePolicySummaryState::Invalid,
                None,
                Some(error.code),
                Some(&error_root),
            );
            EnginePolicySummaryObservation {
                schema: ENGINE_POLICY_SUMMARY_OBSERVATION_SCHEMA.to_string(),
                state: EnginePolicySummaryState::Invalid,
                root,
                summary_root: None,
                error_code: Some(error.code.to_string()),
                error_root: Some(error_root),
            }
        }
    }
}

/// Convenience root for callers that only need to bind the observation into a
/// larger read snapshot. Prefer [`engine_policy_summary_observation`] when the
/// present/absent/invalid state must also be displayed.
#[must_use]
pub fn engine_policy_summary_root(frontier_path: &Path) -> String {
    engine_policy_summary_observation(frontier_path).root
}

fn load_policy_summary_inner(
    frontier_path: &Path,
    allow_frontier_id_fallback: bool,
) -> Result<FrontierPolicySummary, PolicyLoadError> {
    let resolved = resolve_frontier_root(frontier_path)?;
    let manifest_bytes = read_frontier_regular_file(
        resolved.canonical.as_deref(),
        Path::new("frontier.yaml"),
        ENGINE_POLICY_MANIFEST_MAX_BYTES,
        "frontier_manifest",
    )?;
    let manifest = manifest_bytes.as_deref().map(parse_manifest).transpose()?;
    let frontier_id = manifest
        .as_ref()
        .and_then(|m| yaml_string_at(m, &["frontier_id"]))
        .or_else(|| {
            allow_frontier_id_fallback
                .then_some(resolved.canonical.as_deref())
                .flatten()
                .and_then(|root| repo::load_from_path(root).ok())
                .and_then(|project| project.frontier_id)
        });

    let manifest_refs = manifest
        .as_ref()
        .map(policy_refs_from_manifest)
        .unwrap_or_default();
    let has_manifest_refs = !manifest_refs.is_empty();
    let mut documents = Vec::new();
    let mut missing_required = Vec::new();
    let mut used_default_path = false;

    for kind in PolicyDocumentKind::all() {
        let manifest_ref = manifest_refs.get(&kind);
        let default_path = PathBuf::from(".vela").join("policy").join(kind.filename());
        let reference = format!("policy: {}", kind.as_str());
        let chosen = if let Some(path) = manifest_ref {
            // An explicit path is authoritative configuration. A missing
            // declared file must remain missing; silently falling back to a
            // conventional path would make a broken policy look healthy.
            read_frontier_regular_file(
                resolved.canonical.as_deref(),
                path,
                ENGINE_POLICY_DOCUMENT_MAX_BYTES,
                &reference,
            )?
            .map(|bytes| (path.clone(), bytes, true))
        } else {
            let default = read_frontier_regular_file(
                resolved.canonical.as_deref(),
                &default_path,
                ENGINE_POLICY_DOCUMENT_MAX_BYTES,
                &reference,
            )?;
            if default.is_some() {
                used_default_path = true;
            }
            default.map(|bytes| (default_path, bytes, false))
        };

        if let Some((path, bytes, declared_in_manifest)) = chosen {
            let body = std::str::from_utf8(&bytes).map_err(|_| {
                PolicyLoadError::new(
                    "document_invalid_utf8",
                    &reference,
                    Some(bytes_root(&bytes)),
                    format!("{reference}: policy document must be UTF-8"),
                )
            })?;
            let (front_matter, title) = parse_front_matter(body, kind);
            documents.push(PolicyDocumentSummary {
                kind,
                path: normalized_path_string(&path),
                title,
                body_sha256: format!("sha256:{}", hex::encode(Sha256::digest(body.as_bytes()))),
                bytes: body.len(),
                declared_in_manifest,
                front_matter,
            });
        } else {
            missing_required.push(kind.as_str().to_string());
        }
    }

    documents.sort_by(|a, b| a.kind.cmp(&b.kind));
    missing_required.sort();
    let configured = has_manifest_refs || !documents.is_empty();
    let mut summary = FrontierPolicySummary {
        ok: missing_required.is_empty(),
        frontier_id,
        frontier_path: resolved.display.display().to_string(),
        documents,
        missing_required,
        configured,
        defaults_used: used_default_path,
        canonical_json_sha256: String::new(),
    };
    summary.canonical_json_sha256 = summary_hash(&summary).map_err(|error| {
        PolicyLoadError::new(
            "summary_hash_failed",
            "policy_summary",
            None,
            format!("policy_summary: canonical hashing failed ({error})"),
        )
    })?;
    Ok(summary)
}

pub fn review_requirement_for_operation(
    summary: Option<&FrontierPolicySummary>,
    operation_class: &str,
    proposal_kind: &str,
    has_downstream_impact: bool,
) -> OperationReviewRequirement {
    let review_class =
        review_class_for_operation(operation_class, proposal_kind, has_downstream_impact);
    let mut reviewer_roles =
        policy_roles_for_review_class(summary, &review_class).unwrap_or_else(|| {
            default_roles_for_review_class(&review_class)
                .into_iter()
                .map(ToString::to_string)
                .collect()
        });
    reviewer_roles.sort();
    reviewer_roles.dedup();
    if reviewer_roles.is_empty() {
        reviewer_roles.push("local_reviewer".to_string());
    }

    let mut required_reason_fields = BTreeSet::new();
    required_reason_fields.insert("reason".to_string());
    if matches!(
        review_class.as_str(),
        "source_repair" | "clinical_translation" | "retraction_impact"
    ) || (review_class == "confidence_change"
        && confidence_policy_requires_source_or_evidence_ref(summary))
    {
        required_reason_fields.insert("source_or_evidence_ref".to_string());
    }
    if matches!(
        review_class.as_str(),
        "clinical_translation" | "retraction_impact" | "decision_impact"
    ) {
        required_reason_fields.insert("impact_scope".to_string());
    }

    let mut policy_sources = BTreeSet::new();
    let mut allowed_agent_actions = agent_allowed_actions(summary);
    allowed_agent_actions.sort();
    allowed_agent_actions.dedup();
    if summary.is_some() {
        policy_sources.insert("frontier_policy".to_string());
    } else {
        policy_sources.insert("built_in_defaults".to_string());
    }

    OperationReviewRequirement {
        review_class,
        required_reviewer_count: reviewer_roles.len().max(1),
        reviewer_roles,
        required_reason_fields: required_reason_fields.into_iter().collect(),
        allowed_agent_actions,
        policy_sources: policy_sources.into_iter().collect(),
    }
}
struct ResolvedFrontierRoot {
    display: PathBuf,
    canonical: Option<PathBuf>,
}

fn resolve_frontier_root(path: &Path) -> Result<ResolvedFrontierRoot, PolicyLoadError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            let canonical = path.canonicalize().map_err(|error| {
                PolicyLoadError::io("frontier_root_unavailable", "frontier_root", &error)
            })?;
            let metadata = std::fs::metadata(&canonical).map_err(|error| {
                PolicyLoadError::io("frontier_root_unavailable", "frontier_root", &error)
            })?;
            let root = if metadata.is_dir() {
                canonical
            } else if metadata.is_file() {
                canonical.parent().map(Path::to_path_buf).ok_or_else(|| {
                    PolicyLoadError::new(
                        "frontier_root_invalid",
                        "frontier_root",
                        None,
                        "frontier_root: file has no parent directory",
                    )
                })?
            } else {
                return Err(PolicyLoadError::new(
                    "frontier_root_invalid",
                    "frontier_root",
                    None,
                    "frontier_root: expected a directory or regular file",
                ));
            };
            Ok(ResolvedFrontierRoot {
                display: root.clone(),
                canonical: Some(root),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(ResolvedFrontierRoot {
            display: path.to_path_buf(),
            canonical: None,
        }),
        Err(error) => Err(PolicyLoadError::io(
            "frontier_root_unavailable",
            "frontier_root",
            &error,
        )),
    }
}

fn parse_manifest(bytes: &[u8]) -> Result<serde_yaml::Value, PolicyLoadError> {
    let body = std::str::from_utf8(bytes).map_err(|_| {
        PolicyLoadError::new(
            "manifest_invalid_utf8",
            "frontier_manifest",
            Some(bytes_root(bytes)),
            "frontier_manifest: manifest must be UTF-8",
        )
    })?;
    serde_yaml::from_str(body).map_err(|_| {
        PolicyLoadError::new(
            "manifest_invalid_yaml",
            "frontier_manifest",
            Some(bytes_root(bytes)),
            "frontier_manifest: manifest YAML is invalid",
        )
    })
}

fn read_frontier_regular_file(
    root: Option<&Path>,
    relative: &Path,
    max_bytes: usize,
    reference: &str,
) -> Result<Option<Vec<u8>>, PolicyLoadError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(PolicyLoadError::new(
            "path_not_normalized",
            reference,
            Some(text_root(&relative.to_string_lossy())),
            format!("{reference}: path must be normalized and frontier-relative"),
        ));
    }
    let Some(root) = root else {
        return Ok(None);
    };
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("relative path was normalized above")
        };
        current.push(component);
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(PolicyLoadError::io(
                    "path_inspection_failed",
                    reference,
                    &error,
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(PolicyLoadError::new(
                "path_symlink",
                reference,
                None,
                format!("{reference}: path must not traverse a symlink"),
            ));
        }
        let is_leaf = index + 1 == components.len();
        if is_leaf {
            if !metadata.is_file() {
                return Err(PolicyLoadError::new(
                    "path_not_regular_file",
                    reference,
                    None,
                    format!("{reference}: path must name a regular file"),
                ));
            }
            if metadata.len() > max_bytes as u64 {
                return Err(file_too_large(reference, metadata.len(), max_bytes));
            }
        } else if !metadata.is_dir() {
            return Err(PolicyLoadError::new(
                "path_ancestor_not_directory",
                reference,
                None,
                format!("{reference}: path ancestor must be a directory"),
            ));
        }
    }

    let file = std::fs::File::open(&current)
        .map_err(|error| PolicyLoadError::io("file_open_failed", reference, &error))?;
    let opened = file
        .metadata()
        .map_err(|error| PolicyLoadError::io("file_inspection_failed", reference, &error))?;
    if !opened.is_file() {
        return Err(PolicyLoadError::new(
            "path_not_regular_file",
            reference,
            None,
            format!("{reference}: opened descriptor is not a regular file"),
        ));
    }
    if opened.len() > max_bytes as u64 {
        return Err(file_too_large(reference, opened.len(), max_bytes));
    }
    let opened_identity = descriptor_identity(&file, reference)?;

    let linked = std::fs::symlink_metadata(&current)
        .map_err(|error| PolicyLoadError::io("path_reinspection_failed", reference, &error))?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(PolicyLoadError::new(
            "path_changed",
            reference,
            None,
            format!("{reference}: path changed while being opened"),
        ));
    }
    let canonical = current
        .canonicalize()
        .map_err(|error| PolicyLoadError::io("path_canonicalize_failed", reference, &error))?;
    if !canonical.starts_with(root) {
        return Err(PolicyLoadError::new(
            "path_outside_frontier",
            reference,
            None,
            format!("{reference}: path resolved outside the frontier"),
        ));
    }
    let named = std::fs::File::open(&current)
        .map_err(|error| PolicyLoadError::io("file_reopen_failed", reference, &error))?;
    if descriptor_identity(&named, reference)? != opened_identity {
        return Err(PolicyLoadError::new(
            "path_changed",
            reference,
            None,
            format!("{reference}: descriptor identity changed while being opened"),
        ));
    }
    let final_linked = std::fs::symlink_metadata(&current)
        .map_err(|error| PolicyLoadError::io("path_reinspection_failed", reference, &error))?;
    if final_linked.file_type().is_symlink() || !final_linked.is_file() {
        return Err(PolicyLoadError::new(
            "path_changed",
            reference,
            None,
            format!("{reference}: path did not remain a non-symlink regular file"),
        ));
    }

    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| PolicyLoadError::io("file_read_failed", reference, &error))?;
    if bytes.len() > max_bytes {
        return Err(file_too_large(reference, bytes.len() as u64, max_bytes));
    }
    Ok(Some(bytes))
}

fn file_too_large(reference: &str, bytes: u64, max_bytes: usize) -> PolicyLoadError {
    PolicyLoadError::new(
        "file_too_large",
        reference,
        Some(text_root(&bytes.to_string())),
        format!("{reference}: file exceeds the {max_bytes}-byte input limit"),
    )
}

fn descriptor_identity(
    file: &std::fs::File,
    reference: &str,
) -> Result<(u64, u64), PolicyLoadError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file
            .metadata()
            .map_err(|error| PolicyLoadError::io("file_identity_failed", reference, &error))?;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        windows_descriptor_identity(file)
            .map_err(|error| PolicyLoadError::io("file_identity_failed", reference, &error))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(PolicyLoadError::new(
            "file_identity_unsupported",
            reference,
            None,
            format!("{reference}: descriptor identity is unsupported on this platform"),
        ))
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn windows_descriptor_identity(file: &std::fs::File) -> Result<(u64, u64), std::io::Error> {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    // SAFETY: the raw handle is owned by `file`, remains valid for this call,
    // and Windows initializes the output structure when it reports success.
    let ok = unsafe {
        GetFileInformationByHandle(file.as_raw_handle().cast(), information.as_mut_ptr())
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a non-zero return guarantees the structure was initialized.
    let information = unsafe { information.assume_init() };
    let index =
        (u64::from(information.file_index_high) << 32) | u64::from(information.file_index_low);
    Ok((u64::from(information.volume_serial_number), index))
}

fn policy_refs_from_manifest(
    manifest: &serde_yaml::Value,
) -> BTreeMap<PolicyDocumentKind, PathBuf> {
    let mut out = BTreeMap::new();
    for kind in PolicyDocumentKind::all() {
        if let Some(value) = yaml_string_at(manifest, &["policies", "frontier", kind.as_str()]) {
            out.insert(kind, PathBuf::from(value));
        }
    }
    out
}

fn review_class_for_operation(
    operation_class: &str,
    proposal_kind: &str,
    has_downstream_impact: bool,
) -> String {
    let haystack = format!(
        "{} {}",
        operation_class.to_ascii_lowercase(),
        proposal_kind.to_ascii_lowercase()
    );
    if haystack.contains("clinical") || haystack.contains("translation") {
        return "clinical_translation".to_string();
    }
    if haystack.contains("retraction") || haystack.contains("retract") {
        return "retraction_impact".to_string();
    }
    match operation_class {
        "revise_confidence" => "confidence_change",
        "mark_contradiction" => "contradiction_change",
        "repair_locator" | "repair_span" | "add_evidence_atom" => "source_repair",
        "resolve_entity" => "entity_issue",
        "request_downstream_review" | "open_gap" => "decision_impact",
        _ if has_downstream_impact => "decision_impact",
        _ => "low_risk",
    }
    .to_string()
}

fn default_roles_for_review_class(review_class: &str) -> Vec<&'static str> {
    match review_class {
        "confidence_change" | "contradiction_change" | "retraction_impact" => {
            vec!["domain_reviewer", "method_reviewer"]
        }
        "clinical_translation" => vec!["domain_reviewer", "safety_reviewer"],
        "source_repair" => vec!["source_reviewer"],
        "entity_issue" => vec!["entity_reviewer"],
        "decision_impact" => vec!["frontier_reviewer"],
        _ => vec!["local_reviewer"],
    }
}

fn policy_roles_for_review_class(
    summary: Option<&FrontierPolicySummary>,
    review_class: &str,
) -> Option<Vec<String>> {
    let review_doc = summary?
        .documents
        .iter()
        .find(|doc| doc.kind == PolicyDocumentKind::Review && !doc.front_matter.is_empty())?;
    let roles = review_doc
        .front_matter
        .get("required_roles")?
        .get(review_class)?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    if roles.is_empty() { None } else { Some(roles) }
}

fn confidence_policy_requires_source_or_evidence_ref(
    summary: Option<&FrontierPolicySummary>,
) -> bool {
    summary
        .and_then(|s| {
            s.documents
                .iter()
                .find(|doc| doc.kind == PolicyDocumentKind::Confidence)
        })
        .and_then(|doc| doc.front_matter.get("requires_source_or_evidence_ref"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn agent_allowed_actions(summary: Option<&FrontierPolicySummary>) -> Vec<String> {
    summary
        .and_then(|s| {
            s.documents
                .iter()
                .find(|doc| doc.kind == PolicyDocumentKind::Agent)
        })
        .and_then(|doc| doc.front_matter.get("agents_may"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_front_matter(
    body: &str,
    kind: PolicyDocumentKind,
) -> (BTreeMap<String, serde_json::Value>, String) {
    let mut metadata = BTreeMap::new();
    if let Some(rest) = body.strip_prefix("---\n")
        && let Some((front, _body)) = rest.split_once("\n---")
        && let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(front)
        && let Some(map) = yaml.as_mapping()
    {
        for (key, value) in map {
            if let Some(key) = key.as_str()
                && let Ok(json_value) = serde_json::to_value(value)
            {
                metadata.insert(key.to_string(), json_value);
            }
        }
    }
    let title = metadata
        .get("title")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{} policy", kind.as_str()));
    (metadata, title)
}

fn yaml_string_at(value: &serde_yaml::Value, path: &[&str]) -> Option<String> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(ToString::to_string)
}

fn normalized_path_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn summary_hash(summary: &FrontierPolicySummary) -> Result<String, String> {
    let mut value = serde_json::to_value(summary).map_err(|e| format!("serialize policy: {e}"))?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("canonical_json_sha256");
        obj.remove("frontier_path");
        obj.remove("frontier_id");
    }
    let bytes = canonical::to_canonical_bytes(&value)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn observation_root(
    state: EnginePolicySummaryState,
    summary_root: Option<&str>,
    error_code: Option<&str>,
    error_root: Option<&str>,
) -> String {
    domain_hash(
        ENGINE_POLICY_SUMMARY_ROOT_DOMAIN,
        &[
            Some(ENGINE_POLICY_SUMMARY_OBSERVATION_SCHEMA),
            Some(state.as_str()),
            summary_root,
            error_code,
            error_root,
        ],
    )
}

fn bytes_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn text_root(text: &str) -> String {
    domain_hash("vela.engine-policy-text.v1", &[Some(text)])
}

fn domain_hash(domain: &str, parts: &[Option<&str>]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain.as_bytes());
    digest.update([0]);
    for part in parts {
        match part {
            Some(part) => {
                digest.update((part.len() as u64).to_be_bytes());
                digest.update(part.as_bytes());
            }
            None => digest.update(u64::MAX.to_be_bytes()),
        }
    }
    format!("sha256:{}", hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Write one policy doc to the frontier's default policy dir
    /// (`.vela/policy/<filename>`), creating the tree as needed.
    fn write_default_policy(root: &Path, kind: PolicyDocumentKind, body: &str) {
        let dir = root.join(".vela").join("policy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(kind.filename()), body).unwrap();
    }

    /// A frontier dir with all four required policy docs present at their
    /// default paths, so the summary is `ok`.
    fn complete_frontier() -> TempDir {
        let tmp = TempDir::new().unwrap();
        for kind in PolicyDocumentKind::all() {
            write_default_policy(
                tmp.path(),
                kind,
                &format!("---\ntitle: {} rules\n---\nbody\n", kind.as_str()),
            );
        }
        tmp
    }

    fn write_manifest(root: &Path, policies: &str) {
        std::fs::write(
            root.join("frontier.yaml"),
            format!(
                "schema: vela.frontier_manifest.v0.1\nfrontier_id: vfr_policy_test\npolicies:\n{policies}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn kind_str_and_filename_round_trip() {
        // Every kind has a distinct str and a filename that embeds it.
        let kinds = PolicyDocumentKind::all();
        let strs: BTreeSet<&str> = kinds.iter().map(|k| k.as_str()).collect();
        assert_eq!(strs.len(), 4, "kinds must have distinct as_str values");
        for kind in kinds {
            assert!(
                kind.filename().starts_with(kind.as_str()),
                "{} filename {} should embed its str",
                kind.as_str(),
                kind.filename()
            );
            assert!(kind.filename().ends_with(".md"));
        }
    }

    #[test]
    fn complete_frontier_summary_is_ok_and_titles_parse() {
        let tmp = complete_frontier();
        let summary = load_policy_summary(tmp.path()).unwrap();
        assert!(summary.ok, "all four docs present => ok");
        assert!(summary.configured);
        assert!(summary.missing_required.is_empty());
        assert_eq!(summary.documents.len(), 4);
        // Front-matter title is lifted verbatim.
        let review = summary
            .documents
            .iter()
            .find(|d| d.kind == PolicyDocumentKind::Review)
            .unwrap();
        assert_eq!(review.title, "review rules");
        // Default-path docs are not "declared in manifest".
        assert!(!review.declared_in_manifest);
        assert!(summary.defaults_used);
        assert!(summary.canonical_json_sha256.starts_with("sha256:"));
    }

    #[test]
    fn missing_documents_are_reported_and_summary_not_ok() {
        let tmp = TempDir::new().unwrap();
        // Only two of the four required docs are present.
        write_default_policy(tmp.path(), PolicyDocumentKind::Evidence, "evidence body");
        write_default_policy(tmp.path(), PolicyDocumentKind::Review, "review body");
        let summary = load_policy_summary(tmp.path()).unwrap();
        assert!(!summary.ok);
        assert!(summary.configured);
        assert_eq!(summary.documents.len(), 2);
        // missing_required is sorted and names exactly the absent kinds.
        assert_eq!(summary.missing_required, vec!["agent", "confidence"]);
    }

    #[test]
    fn declared_missing_documents_do_not_fall_back_or_look_absent() {
        let tmp = TempDir::new().unwrap();
        // Even a conventional file with the same kind cannot replace an
        // explicitly declared path that is missing.
        write_default_policy(
            tmp.path(),
            PolicyDocumentKind::Evidence,
            "must not be used as a fallback",
        );
        write_manifest(
            tmp.path(),
            "  frontier:\n    evidence: policy/missing-evidence.md\n    review: policy/missing-review.md\n    confidence: policy/missing-confidence.md\n    agent: policy/missing-agent.md\n",
        );

        let summary = load_policy_summary(tmp.path()).unwrap();
        assert!(!summary.ok);
        assert!(summary.configured);
        assert!(summary.documents.is_empty());
        assert!(!summary.defaults_used);
        assert_eq!(summary.missing_required.len(), 4);
        let observation = engine_policy_summary_observation(tmp.path());
        assert_eq!(observation.state, EnginePolicySummaryState::Present);
    }

    #[test]
    fn empty_frontier_policy_is_explicitly_unconfigured() {
        let tmp = TempDir::new().unwrap();
        let summary = load_policy_summary(tmp.path()).unwrap();
        assert!(!summary.ok);
        assert!(!summary.configured);
        assert!(summary.documents.is_empty());
        let observation = engine_policy_summary_observation(tmp.path());
        assert_eq!(observation.state, EnginePolicySummaryState::Absent);
    }

    #[test]
    fn title_falls_back_when_no_front_matter() {
        let tmp = TempDir::new().unwrap();
        // A body with no `---` front matter yields the default title.
        write_default_policy(
            tmp.path(),
            PolicyDocumentKind::Confidence,
            "plain body, no front matter\n",
        );
        let summary = load_policy_summary(tmp.path()).unwrap();
        let doc = &summary.documents[0];
        assert_eq!(doc.kind, PolicyDocumentKind::Confidence);
        assert_eq!(doc.title, "confidence policy");
        assert!(doc.front_matter.is_empty());
        // Body hash and byte count reflect the written body.
        assert_eq!(doc.bytes, "plain body, no front matter\n".len());
        assert!(doc.body_sha256.starts_with("sha256:"));
    }

    #[test]
    fn summary_hash_is_deterministic_and_excludes_itself() {
        let tmp = complete_frontier();
        let a = load_policy_summary(tmp.path()).unwrap();
        let b = load_policy_summary(tmp.path()).unwrap();
        // The canonical hash pins the whole summary and is reproducible.
        assert_eq!(a.canonical_json_sha256, b.canonical_json_sha256);
        // Recomputing the hash over the summary (which the field excludes)
        // reproduces the stored value.
        assert_eq!(summary_hash(&a).unwrap(), a.canonical_json_sha256);
    }

    #[test]
    fn engine_policy_observation_is_clone_stable_and_content_bound() {
        let first = complete_frontier();
        let second = complete_frontier();
        let first_observation = engine_policy_summary_observation(first.path());
        let second_observation = engine_policy_summary_observation(second.path());
        assert_eq!(first_observation.state, EnginePolicySummaryState::Present);
        assert_eq!(first_observation.root, second_observation.root);
        assert!(
            !first_observation
                .root
                .contains(&first.path().display().to_string())
        );

        write_default_policy(
            second.path(),
            PolicyDocumentKind::Review,
            "changed review policy",
        );
        let changed = engine_policy_summary_observation(second.path());
        assert_ne!(first_observation.root, changed.root);
    }

    #[test]
    fn manifest_traversal_is_invalid_and_never_read() {
        let tmp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("outside.md"), "must not be read").unwrap();
        write_manifest(tmp.path(), "  frontier:\n    evidence: ../outside.md\n");

        let error = load_policy_summary(tmp.path()).unwrap_err();
        assert!(error.contains("path_not_normalized"), "{error}");
        let observation = engine_policy_summary_observation(tmp.path());
        assert_eq!(observation.state, EnginePolicySummaryState::Invalid);
        assert_eq!(
            observation.error_code.as_deref(),
            Some("path_not_normalized")
        );
        assert!(observation.error_root.is_some());
        assert!(
            !serde_json::to_string(&observation)
                .unwrap()
                .contains(&outside.path().display().to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn manifest_selected_symlink_leaf_and_ancestor_are_invalid() {
        use std::os::unix::fs::symlink;

        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("review.md"), "outside review").unwrap();

        let leaf = TempDir::new().unwrap();
        std::fs::create_dir_all(leaf.path().join("policy")).unwrap();
        symlink(
            outside.path().join("review.md"),
            leaf.path().join("policy/review.md"),
        )
        .unwrap();
        write_manifest(leaf.path(), "  frontier:\n    review: policy/review.md\n");
        let leaf_observation = engine_policy_summary_observation(leaf.path());
        assert_eq!(leaf_observation.state, EnginePolicySummaryState::Invalid);
        assert_eq!(leaf_observation.error_code.as_deref(), Some("path_symlink"));

        let ancestor = TempDir::new().unwrap();
        symlink(outside.path(), ancestor.path().join("linked")).unwrap();
        write_manifest(
            ancestor.path(),
            "  frontier:\n    review: linked/review.md\n",
        );
        let ancestor_observation = engine_policy_summary_observation(ancestor.path());
        assert_eq!(
            ancestor_observation.state,
            EnginePolicySummaryState::Invalid
        );
        assert_eq!(
            ancestor_observation.error_code.as_deref(),
            Some("path_symlink")
        );
    }

    #[test]
    fn manifest_and_policy_documents_are_pre_read_bounded() {
        let policy = TempDir::new().unwrap();
        write_default_policy(
            policy.path(),
            PolicyDocumentKind::Evidence,
            &"x".repeat(ENGINE_POLICY_DOCUMENT_MAX_BYTES + 1),
        );
        let policy_observation = engine_policy_summary_observation(policy.path());
        assert_eq!(policy_observation.state, EnginePolicySummaryState::Invalid);
        assert_eq!(
            policy_observation.error_code.as_deref(),
            Some("file_too_large")
        );

        let manifest = TempDir::new().unwrap();
        std::fs::write(
            manifest.path().join("frontier.yaml"),
            vec![b'x'; ENGINE_POLICY_MANIFEST_MAX_BYTES + 1],
        )
        .unwrap();
        let manifest_observation = engine_policy_summary_observation(manifest.path());
        assert_eq!(
            manifest_observation.state,
            EnginePolicySummaryState::Invalid
        );
        assert_eq!(
            manifest_observation.error_code.as_deref(),
            Some("file_too_large")
        );
    }

    #[test]
    fn low_risk_operation_defaults_to_local_reviewer() {
        // With no policy summary, an unremarkable operation is low-risk and
        // needs one local reviewer, and only a `reason` field.
        let req = review_requirement_for_operation(None, "note", "comment", false);
        assert_eq!(req.review_class, "low_risk");
        assert_eq!(req.reviewer_roles, vec!["local_reviewer"]);
        assert_eq!(req.required_reviewer_count, 1);
        assert_eq!(req.required_reason_fields, vec!["reason"]);
        assert_eq!(req.policy_sources, vec!["built_in_defaults"]);
    }

    #[test]
    fn clinical_translation_escalates_roles_and_reason_fields() {
        // A clinical/translation keyword routes to the highest-touch class:
        // two reviewer roles and the extra reason fields.
        let req = review_requirement_for_operation(None, "revise_confidence", "clinical", false);
        assert_eq!(req.review_class, "clinical_translation");
        assert_eq!(
            req.reviewer_roles,
            vec!["domain_reviewer", "safety_reviewer"]
        );
        assert_eq!(req.required_reviewer_count, 2);
        // reason + source_or_evidence_ref + impact_scope, sorted.
        assert_eq!(
            req.required_reason_fields,
            vec!["impact_scope", "reason", "source_or_evidence_ref"]
        );
    }

    #[test]
    fn downstream_impact_promotes_unclassified_op_to_decision_impact() {
        // An operation with no dedicated class but downstream impact becomes
        // decision_impact (frontier_reviewer + impact_scope), whereas without
        // impact the same op is low_risk.
        let with_impact = review_requirement_for_operation(None, "misc_op", "misc", true);
        assert_eq!(with_impact.review_class, "decision_impact");
        assert_eq!(with_impact.reviewer_roles, vec!["frontier_reviewer"]);
        assert!(
            with_impact
                .required_reason_fields
                .contains(&"impact_scope".to_string())
        );

        let without = review_requirement_for_operation(None, "misc_op", "misc", false);
        assert_eq!(without.review_class, "low_risk");
    }

    #[test]
    fn policy_overrides_default_roles_and_surfaces_agent_actions() {
        // A frontier whose review policy declares custom roles for a class,
        // and whose agent policy declares allowed actions, is honored over
        // the built-in defaults.
        let tmp = complete_frontier();
        write_default_policy(
            tmp.path(),
            PolicyDocumentKind::Review,
            "---\nrequired_roles:\n  source_repair:\n    - senior_curator\n    - archivist\n---\nbody\n",
        );
        write_default_policy(
            tmp.path(),
            PolicyDocumentKind::Agent,
            "---\nagents_may:\n  - draft_receipt\n  - run_verifier\n---\nbody\n",
        );
        let summary = load_policy_summary(tmp.path()).unwrap();

        let req = review_requirement_for_operation(Some(&summary), "repair_locator", "fix", false);
        assert_eq!(req.review_class, "source_repair");
        // Custom roles replace the default `source_reviewer`, and are sorted.
        assert_eq!(req.reviewer_roles, vec!["archivist", "senior_curator"]);
        // Agent actions from the agent policy are surfaced, sorted.
        assert_eq!(
            req.allowed_agent_actions,
            vec!["draft_receipt", "run_verifier"]
        );
        // A real summary marks the source as the frontier policy.
        assert_eq!(req.policy_sources, vec!["frontier_policy"]);
    }

    #[test]
    fn confidence_change_requires_ref_only_when_policy_opts_in() {
        // confidence_change alone does not demand source_or_evidence_ref...
        let plain = review_requirement_for_operation(None, "revise_confidence", "update", false);
        assert_eq!(plain.review_class, "confidence_change");
        assert!(
            !plain
                .required_reason_fields
                .contains(&"source_or_evidence_ref".to_string())
        );

        // ...but a confidence policy that opts in flips the requirement on.
        let tmp = complete_frontier();
        write_default_policy(
            tmp.path(),
            PolicyDocumentKind::Confidence,
            "---\nrequires_source_or_evidence_ref: true\n---\nbody\n",
        );
        let summary = load_policy_summary(tmp.path()).unwrap();
        let gated =
            review_requirement_for_operation(Some(&summary), "revise_confidence", "update", false);
        assert_eq!(gated.review_class, "confidence_change");
        assert!(
            gated
                .required_reason_fields
                .contains(&"source_or_evidence_ref".to_string())
        );
    }
}
