//! Recoverable, path-bound frontier filesystem transactions.
//!
//! This is private durability plumbing, not a protocol object. A caller first
//! builds a pure [`CanonicalDelta`], then persists its plan and postimage blobs,
//! writes a durable commit marker, and finally installs the exact bytes. Once a
//! marker exists recovery only replays the journal; it never re-runs policy,
//! verification, clocks, or key-bearing code.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};

use crate::operation_journal;

pub(crate) const FRONTIER_TXN_SCHEMA: &str = "vela.frontier-txn.internal.v1";
const FRONTIER_TXN_BLOB_SCHEMA: &str = "vela.frontier-txn-blob.internal.v1";
const FRONTIER_TXN_MARKER_SCHEMA: &str = "vela.frontier-txn-marker.internal.v1";
const CANONICAL_DELTA_SCHEMA: &str = "vela.canonical-delta.internal.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ContentDigest(String);

impl ContentDigest {
    pub(crate) fn hash(bytes: impl AsRef<[u8]>) -> Self {
        Self(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(bytes.as_ref()))
        ))
    }

    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, FrontierTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(FrontierTxnError::InvalidDigest(value));
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(FrontierTxnError::InvalidDigest(value));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn file_stem(&self) -> &str {
        self.0
            .strip_prefix("sha256:")
            .expect("validated content digest")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct OperationId(String);

impl OperationId {
    pub(crate) fn derive(kind: &str, planning_identity: &[u8]) -> Self {
        Self(operation_journal::operation_id(kind, planning_identity))
    }

    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, FrontierTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("vop_") else {
            return Err(FrontierTxnError::InvalidOperationId(value));
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(FrontierTxnError::InvalidOperationId(value));
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct RepoPath(String);

fn is_windows_reserved_path_segment(segment: &str) -> bool {
    if segment.trim_end_matches([' ', '.']) != segment {
        return true;
    }
    let stem = segment
        .split_once('.')
        .map_or(segment, |(stem, _extension)| stem)
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

impl RepoPath {
    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, FrontierTxnError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\0')
            || value.contains('\\')
            || value.contains("//")
        {
            return Err(FrontierTxnError::InvalidPath {
                path: value,
                reason: "path must be a non-empty normalized relative path".to_string(),
            });
        }
        for segment in value.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(FrontierTxnError::InvalidPath {
                    path: value,
                    reason: "dot and empty path components are forbidden".to_string(),
                });
            }
            if segment.eq_ignore_ascii_case(".git") {
                return Err(FrontierTxnError::InvalidPath {
                    path: value,
                    reason: ".git is outside the frontier write boundary".to_string(),
                });
            }
            if is_windows_reserved_path_segment(segment) {
                return Err(FrontierTxnError::InvalidPath {
                    path: value,
                    reason: "path component is not portable across supported Git filesystems"
                        .to_string(),
                });
            }
            if segment
                .chars()
                .any(|character| character.is_control() || "*?[]{}:\"<>|".contains(character))
            {
                return Err(FrontierTxnError::InvalidPath {
                    path: value,
                    reason: "control, glob, and platform-reserved characters are forbidden"
                        .to_string(),
                });
            }
        }
        let path = Path::new(&value);
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(FrontierTxnError::InvalidPath {
                path: value,
                reason: "path is not lexically relative and normalized".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn target(&self, root: &Path) -> Result<PathBuf, FrontierTxnError> {
        validate_target(root, self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileMode {
    Regular,
    Executable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum FileState {
    Absent,
    File {
        digest: ContentDigest,
        size: u64,
        mode: FileMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JournalBlobRef {
    pub(crate) digest: ContentDigest,
    pub(crate) size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WriteClass {
    CanonicalEvidence,
    PublicReview,
    Authority,
    Derived,
    PrivateCoordination,
}

impl WriteClass {
    fn install_order(self) -> u8 {
        match self {
            Self::CanonicalEvidence => 10,
            Self::PublicReview => 20,
            Self::Authority => 30,
            Self::Derived => 40,
            Self::PrivateCoordination => 50,
        }
    }

    pub(crate) fn is_public(self) -> bool {
        !matches!(self, Self::PrivateCoordination)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StagedWrite {
    pub(crate) path: RepoPath,
    pub(crate) class: WriteClass,
    pub(crate) preimage: FileState,
    pub(crate) postimage: FileState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) payload: Option<JournalBlobRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CanonicalDelta {
    schema: String,
    root: ContentDigest,
    writes: Vec<StagedWrite>,
}

#[derive(Serialize)]
struct DeltaCommitment<'a> {
    schema: &'a str,
    writes: &'a [StagedWrite],
}

impl CanonicalDelta {
    fn new(mut writes: Vec<StagedWrite>) -> Result<Self, FrontierTxnError> {
        writes.sort_by(|left, right| {
            left.class
                .install_order()
                .cmp(&right.class.install_order())
                .then_with(|| left.path.cmp(&right.path))
        });
        let mut paths = BTreeSet::new();
        for write in &writes {
            if !paths.insert(write.path.clone()) {
                return Err(FrontierTxnError::DuplicatePath(
                    write.path.as_str().to_string(),
                ));
            }
        }
        let root = Self::compute_root(&writes)?;
        Ok(Self {
            schema: CANONICAL_DELTA_SCHEMA.to_string(),
            root,
            writes,
        })
    }

    fn compute_root(writes: &[StagedWrite]) -> Result<ContentDigest, FrontierTxnError> {
        let bytes = vela_protocol::canonical::to_canonical_bytes(&DeltaCommitment {
            schema: CANONICAL_DELTA_SCHEMA,
            writes,
        })
        .map_err(FrontierTxnError::Canonicalize)?;
        Ok(ContentDigest::hash(bytes))
    }

    pub(crate) fn verify(&self) -> Result<(), FrontierTxnError> {
        if self.schema != CANONICAL_DELTA_SCHEMA {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "unexpected canonical delta schema {}",
                self.schema
            )));
        }
        let normalized = Self::new(self.writes.clone())?;
        if normalized.writes != self.writes || normalized.root != self.root {
            return Err(FrontierTxnError::CorruptPlan(
                "canonical delta is not sorted or root-bound".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn root(&self) -> &ContentDigest {
        &self.root
    }

    pub(crate) fn writes(&self) -> &[StagedWrite] {
        &self.writes
    }

    pub(crate) fn public_writes(&self) -> impl Iterator<Item = &StagedWrite> {
        self.writes.iter().filter(|write| write.class.is_public())
    }
}

#[derive(Debug, Clone)]
enum PlannedPostimage {
    Absent,
    File {
        bytes: Vec<u8>,
        mode: Option<FileMode>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedWrite {
    path: RepoPath,
    class: WriteClass,
    postimage: PlannedPostimage,
}

impl PlannedWrite {
    pub(crate) fn write(path: RepoPath, class: WriteClass, bytes: Vec<u8>) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::File { bytes, mode: None },
        }
    }

    pub(crate) fn write_with_mode(
        path: RepoPath,
        class: WriteClass,
        bytes: Vec<u8>,
        mode: FileMode,
    ) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::File {
                bytes,
                mode: Some(mode),
            },
        }
    }

    pub(crate) fn delete(path: RepoPath, class: WriteClass) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::Absent,
        }
    }

    pub(crate) fn from_managed_files(
        managed: vela_protocol::repo::ManagedFileSet,
    ) -> Result<Vec<Self>, FrontierTxnError> {
        let mut writes = Vec::with_capacity(managed.writes.len() + managed.deletes.len());
        for (path, bytes) in managed.writes {
            let path = RepoPath::parse(path)?;
            let class = managed_write_class(&path);
            writes.push(Self::write(path, class, bytes));
        }
        for path in managed.deletes {
            let path = RepoPath::parse(path)?;
            let class = managed_write_class(&path);
            writes.push(Self::delete(path, class));
        }
        Ok(writes)
    }
}

fn managed_write_class(path: &RepoPath) -> WriteClass {
    if path.as_str().starts_with(".vela/events/") {
        WriteClass::Authority
    } else if path.as_str().starts_with(".vela/proposals/") {
        WriteClass::PublicReview
    } else if path.as_str().starts_with(".vela/") {
        WriteClass::CanonicalEvidence
    } else {
        WriteClass::Derived
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeltaDraft {
    pub(crate) delta: CanonicalDelta,
    blobs: BTreeMap<ContentDigest, Vec<u8>>,
}

impl DeltaDraft {
    pub(crate) fn prepare(
        frontier_root: &Path,
        writes: Vec<PlannedWrite>,
    ) -> Result<Self, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let mut seen = BTreeSet::new();
        let mut staged = Vec::new();
        let mut blobs = BTreeMap::new();
        for write in writes {
            if !seen.insert(write.path.clone()) {
                return Err(FrontierTxnError::DuplicatePath(
                    write.path.as_str().to_string(),
                ));
            }
            let preimage = inspect_file_state(&root, &write.path)?;
            let (postimage, payload) = match write.postimage {
                PlannedPostimage::Absent => (FileState::Absent, None),
                PlannedPostimage::File { bytes, mode } => {
                    let digest = ContentDigest::hash(&bytes);
                    let size = bytes.len() as u64;
                    let mode = mode.unwrap_or(match &preimage {
                        FileState::File { mode, .. } => *mode,
                        FileState::Absent => FileMode::Regular,
                    });
                    blobs.entry(digest.clone()).or_insert(bytes);
                    (
                        FileState::File {
                            digest: digest.clone(),
                            size,
                            mode,
                        },
                        Some(JournalBlobRef { digest, size }),
                    )
                }
            };
            if preimage != postimage {
                staged.push(StagedWrite {
                    path: write.path,
                    class: write.class,
                    preimage,
                    postimage,
                    payload,
                });
            }
        }
        let delta = CanonicalDelta::new(staged)?;
        let referenced = delta
            .writes
            .iter()
            .filter_map(|write| write.payload.as_ref().map(|blob| blob.digest.clone()))
            .collect::<BTreeSet<_>>();
        blobs.retain(|digest, _| referenced.contains(digest));
        Ok(Self { delta, blobs })
    }

    pub(crate) fn resolved_public_writes(&self) -> Result<Vec<ResolvedWrite>, FrontierTxnError> {
        resolve_public_writes(&self.delta, |expected| {
            let bytes = self
                .blobs
                .get(&expected.digest)
                .cloned()
                .ok_or_else(|| FrontierTxnError::MissingBlob(expected.digest.clone()))?;
            validate_blob_bytes(expected, &bytes)?;
            Ok(bytes)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FrontierBinding {
    canonical_root: String,
    frontier_id: String,
    layout_root: ContentDigest,
}

impl FrontierBinding {
    pub(crate) fn new(
        frontier_root: &Path,
        frontier_id: impl Into<String>,
        layout_identity: &[u8],
    ) -> Result<Self, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let frontier_id = frontier_id.into();
        if frontier_id.trim().is_empty() {
            return Err(FrontierTxnError::CorruptPlan(
                "frontier binding has an empty frontier id".to_string(),
            ));
        }
        Ok(Self {
            canonical_root: root.to_string_lossy().into_owned(),
            frontier_id,
            layout_root: ContentDigest::hash(layout_identity),
        })
    }

    fn verify_root(&self, frontier_root: &Path) -> Result<PathBuf, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        if root.as_os_str() != self.canonical_root.as_str() {
            return Err(FrontierTxnError::FrontierBindingMismatch {
                expected: self.canonical_root.clone(),
                actual: root.display().to_string(),
            });
        }
        Ok(root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OperationKind {
    Submission,
    Decision,
    Maintenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InputBinding {
    pub(crate) name: String,
    pub(crate) digest: ContentDigest,
}

const FRONTIER_FILE_INPUT_PREFIX: &str = "frontier_file:";
const FRONTIER_FILE_INPUT_SCHEMA: &str = "vela.frontier-file-input.internal.v1";
const FRONTIER_PROJECT_INPUT_NAME: &str = "frontier_project:vela.project-snapshot.internal.v1";
const ENGINE_POLICY_INPUT_NAME: &str =
    "frontier_observation:vela.engine-policy-summary-observation.v1";

#[derive(Serialize)]
struct FrontierFileInputCommitment<'a> {
    schema: &'a str,
    path: &'a RepoPath,
    state: &'a FileState,
}

impl InputBinding {
    /// Bind either the exact current file state or its exact absence.
    ///
    /// This is the read-set counterpart to a bounded caller read: it does not
    /// assume that a decision-critical receipt or policy path exists, and the
    /// marker check rejects creation, deletion, byte drift, or mode drift.
    pub(crate) fn current_file(
        frontier_root: &Path,
        path: RepoPath,
    ) -> Result<Self, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let state = inspect_file_state(&root, &path)?;
        Self::from_frontier_state(path, state)
    }

    /// Bind a regular frontier file as a mutable planning input. The path tag
    /// is encoded in the existing `name` field so old digest-only journal
    /// records remain wire-compatible and continue to deserialize unchanged.
    pub(crate) fn existing_file(
        frontier_root: &Path,
        path: RepoPath,
    ) -> Result<Self, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let state = inspect_file_state(&root, &path)?;
        if matches!(state, FileState::Absent) {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "cannot bind missing frontier input {} as an existing file",
                path.as_str()
            )));
        }
        Self::from_frontier_state(path, state)
    }

    /// Bind the absence of a relative frontier file. Creation of that file
    /// before the commit marker is therefore stale input, not a policy result
    /// that can be committed under changed authority bytes.
    pub(crate) fn absent_file(
        frontier_root: &Path,
        path: RepoPath,
    ) -> Result<Self, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let state = inspect_file_state(&root, &path)?;
        if !matches!(state, FileState::Absent) {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "cannot bind present frontier input {} as absent",
                path.as_str()
            )));
        }
        Self::from_frontier_state(path, state)
    }

    /// Bind the exact bytes already loaded by a caller without reading the
    /// path a second time. Marker-time verification still inspects the path,
    /// so any drift between that snapshot and commit fails closed.
    pub(crate) fn file_snapshot(
        path: RepoPath,
        bytes: Option<&[u8]>,
    ) -> Result<Self, FrontierTxnError> {
        let state = match bytes {
            Some(bytes) => FileState::File {
                digest: ContentDigest::hash(bytes),
                size: bytes.len() as u64,
                mode: FileMode::Regular,
            },
            None => FileState::Absent,
        };
        Self::from_frontier_state(path, state)
    }

    /// Bind the complete typed Project loaded under the recovery barrier.
    /// Event-log binding alone is insufficient: proposal answers, actor
    /// authority/revocation, and derived verifier state can change without
    /// changing the accepted event head.
    pub(crate) fn project_snapshot(
        project: &vela_protocol::project::Project,
    ) -> Result<Self, FrontierTxnError> {
        let bytes = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
            "schema": "vela.project-snapshot.internal.v1",
            "project": project,
        }))
        .map_err(FrontierTxnError::Canonicalize)?;
        Ok(Self {
            name: FRONTIER_PROJECT_INPUT_NAME.to_string(),
            digest: ContentDigest::hash(bytes),
        })
    }

    /// Bind the exact present/absent/invalid Engine-policy observation used
    /// during Decision Plan derivation. Marker-time verification re-runs the
    /// same bounded observation, closing the post-key/pre-marker policy race.
    pub(crate) fn engine_policy_observation(
        observation_root: &str,
    ) -> Result<Self, FrontierTxnError> {
        Ok(Self {
            name: ENGINE_POLICY_INPUT_NAME.to_string(),
            digest: ContentDigest::parse(observation_root.to_string())?,
        })
    }

    fn from_frontier_state(path: RepoPath, state: FileState) -> Result<Self, FrontierTxnError> {
        Ok(Self {
            name: format!("{FRONTIER_FILE_INPUT_PREFIX}{}", path.as_str()),
            digest: frontier_file_input_digest(&path, &state)?,
        })
    }

    fn frontier_path(&self) -> Result<Option<RepoPath>, FrontierTxnError> {
        let Some(path) = self.name.strip_prefix(FRONTIER_FILE_INPUT_PREFIX) else {
            return Ok(None);
        };
        RepoPath::parse(path.to_string()).map(Some)
    }

    fn verify_shape(&self) -> Result<(), FrontierTxnError> {
        if self.name.trim().is_empty() {
            return Err(FrontierTxnError::CorruptPlan(
                "frontier transaction input has an empty name".to_string(),
            ));
        }
        ContentDigest::parse(self.digest.as_str().to_string())?;
        self.frontier_path()?;
        Ok(())
    }

    fn verify_current(&self, root: &Path) -> Result<(), FrontierTxnError> {
        if self.name == FRONTIER_PROJECT_INPUT_NAME {
            let project = vela_protocol::repo::load_from_path(root).map_err(|error| {
                FrontierTxnError::Io(format!("reload bound Project snapshot: {error}"))
            })?;
            let actual = Self::project_snapshot(&project)?.digest;
            if actual != self.digest {
                return Err(FrontierTxnError::StaleSnapshot {
                    name: self.name.clone(),
                    expected: self.digest.clone(),
                    actual,
                });
            }
            return Ok(());
        }
        if self.name == ENGINE_POLICY_INPUT_NAME {
            let actual = ContentDigest::parse(
                vela_protocol::frontier_policy::engine_policy_summary_root(root),
            )?;
            if actual != self.digest {
                return Err(FrontierTxnError::StaleSnapshot {
                    name: self.name.clone(),
                    expected: self.digest.clone(),
                    actual,
                });
            }
            return Ok(());
        }
        let Some(path) = self.frontier_path()? else {
            return Ok(());
        };
        let state = inspect_file_state(root, &path)?;
        let actual = frontier_file_input_digest(&path, &state)?;
        if actual != self.digest {
            return Err(FrontierTxnError::StaleInput {
                name: self.name.clone(),
                path,
                expected: self.digest.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn frontier_file_input_digest(
    path: &RepoPath,
    state: &FileState,
) -> Result<ContentDigest, FrontierTxnError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(&FrontierFileInputCommitment {
        schema: FRONTIER_FILE_INPUT_SCHEMA,
        path,
        state,
    })
    .map_err(FrontierTxnError::Canonicalize)?;
    Ok(ContentDigest::hash(bytes))
}

#[derive(Debug, Clone)]
pub(crate) struct FrontierTxnPlanSpec {
    pub(crate) kind: OperationKind,
    pub(crate) operation_id: OperationId,
    pub(crate) request_root: ContentDigest,
    pub(crate) frontier: FrontierBinding,
    pub(crate) fixed_time: String,
    pub(crate) expected_event_log_root: ContentDigest,
    pub(crate) resulting_event_log_root: ContentDigest,
    pub(crate) resulting_event_ids: Vec<String>,
    pub(crate) read_set: Vec<InputBinding>,
    pub(crate) result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FrontierTxnPlan {
    schema: String,
    root: ContentDigest,
    pub(crate) kind: OperationKind,
    pub(crate) operation_id: OperationId,
    pub(crate) request_root: ContentDigest,
    pub(crate) frontier: FrontierBinding,
    pub(crate) fixed_time: String,
    pub(crate) expected_event_log_root: ContentDigest,
    pub(crate) resulting_event_log_root: ContentDigest,
    pub(crate) resulting_event_ids: Vec<String>,
    pub(crate) read_set: Vec<InputBinding>,
    pub(crate) canonical_delta: CanonicalDelta,
    pub(crate) result: serde_json::Value,
}

#[derive(Serialize)]
struct PlanCommitment<'a> {
    schema: &'a str,
    kind: &'a OperationKind,
    operation_id: &'a OperationId,
    request_root: &'a ContentDigest,
    frontier: &'a FrontierBinding,
    fixed_time: &'a str,
    expected_event_log_root: &'a ContentDigest,
    resulting_event_log_root: &'a ContentDigest,
    resulting_event_ids: &'a [String],
    read_set: &'a [InputBinding],
    canonical_delta: &'a CanonicalDelta,
    result: &'a serde_json::Value,
}

impl FrontierTxnPlan {
    pub(crate) fn new(
        spec: FrontierTxnPlanSpec,
        canonical_delta: CanonicalDelta,
    ) -> Result<Self, FrontierTxnError> {
        canonical_delta.verify()?;
        OperationId::parse(spec.operation_id.as_str())?;
        verify_resulting_event_ids(&spec.resulting_event_ids)?;
        let mut plan = Self {
            schema: FRONTIER_TXN_SCHEMA.to_string(),
            root: ContentDigest::hash([]),
            kind: spec.kind,
            operation_id: spec.operation_id,
            request_root: spec.request_root,
            frontier: spec.frontier,
            fixed_time: spec.fixed_time,
            expected_event_log_root: spec.expected_event_log_root,
            resulting_event_log_root: spec.resulting_event_log_root,
            resulting_event_ids: spec.resulting_event_ids,
            read_set: spec.read_set,
            canonical_delta,
            result: spec.result,
        };
        plan.root = plan.compute_root()?;
        Ok(plan)
    }

    fn compute_root(&self) -> Result<ContentDigest, FrontierTxnError> {
        let bytes = vela_protocol::canonical::to_canonical_bytes(&PlanCommitment {
            schema: FRONTIER_TXN_SCHEMA,
            kind: &self.kind,
            operation_id: &self.operation_id,
            request_root: &self.request_root,
            frontier: &self.frontier,
            fixed_time: &self.fixed_time,
            expected_event_log_root: &self.expected_event_log_root,
            resulting_event_log_root: &self.resulting_event_log_root,
            resulting_event_ids: &self.resulting_event_ids,
            read_set: &self.read_set,
            canonical_delta: &self.canonical_delta,
            result: &self.result,
        })
        .map_err(FrontierTxnError::Canonicalize)?;
        Ok(ContentDigest::hash(bytes))
    }

    fn verify(&self) -> Result<(), FrontierTxnError> {
        if self.schema != FRONTIER_TXN_SCHEMA {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "unexpected frontier transaction schema {}",
                self.schema
            )));
        }
        OperationId::parse(self.operation_id.as_str())?;
        verify_resulting_event_ids(&self.resulting_event_ids)?;
        for input in &self.read_set {
            input.verify_shape()?;
        }
        self.canonical_delta.verify()?;
        if self.compute_root()? != self.root {
            return Err(FrontierTxnError::CorruptPlan(
                "frontier transaction plan root does not match its body".to_string(),
            ));
        }
        Ok(())
    }

    pub(crate) fn root(&self) -> &ContentDigest {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CommitMarker {
    schema: String,
    operation_id: OperationId,
    plan_root: ContentDigest,
    delta_root: ContentDigest,
    expected_event_log_root: ContentDigest,
    resulting_event_log_root: ContentDigest,
}

impl CommitMarker {
    fn from_plan(plan: &FrontierTxnPlan) -> Self {
        Self {
            schema: FRONTIER_TXN_MARKER_SCHEMA.to_string(),
            operation_id: plan.operation_id.clone(),
            plan_root: plan.root.clone(),
            delta_root: plan.canonical_delta.root.clone(),
            expected_event_log_root: plan.expected_event_log_root.clone(),
            resulting_event_log_root: plan.resulting_event_log_root.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum RecoveryState {
    Prepared,
    Aborted,
    Committed,
    Installing { installed: usize, total: usize },
    Installed,
    Completed,
    CommittedConflict { path: RepoPath },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FrontierTxnJournal {
    schema: String,
    plan: FrontierTxnPlan,
    recovery: RecoveryState,
}

impl FrontierTxnJournal {
    fn verify(&self) -> Result<(), FrontierTxnError> {
        if self.schema != FRONTIER_TXN_SCHEMA {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "unexpected frontier transaction journal schema {}",
                self.schema
            )));
        }
        self.plan.verify()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BlobJournal {
    schema: String,
    digest: ContentDigest,
    size: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct FrontierTxnPaths {
    plan: PathBuf,
    marker: PathBuf,
    blob_dir: PathBuf,
}

impl FrontierTxnPaths {
    fn new(journal_dir: &Path, operation_id: &OperationId) -> Self {
        let frontier_dir = journal_dir.join("frontier");
        Self {
            plan: operation_journal::path(&frontier_dir, operation_id.as_str()),
            marker: operation_journal::path(&frontier_dir.join("committed"), operation_id.as_str()),
            blob_dir: frontier_dir.join("blobs"),
        }
    }

    fn blob(&self, digest: &ContentDigest) -> PathBuf {
        self.blob_dir.join(format!("{}.json", digest.file_stem()))
    }
}

#[derive(Debug)]
struct FrontierWriteLock {
    _file: File,
}

impl Drop for FrontierWriteLock {
    fn drop(&mut self) {
        // Release synchronously at the transaction boundary. Relying only on
        // descriptor teardown made immediate same-process replanning flaky on
        // some filesystems under the parallel workspace harness.
        let _ = self._file.unlock();
    }
}

impl FrontierWriteLock {
    fn acquire(journal_dir: &Path, root: &Path) -> Result<Self, FrontierTxnError> {
        let lock_id = ContentDigest::hash(root.to_string_lossy().as_bytes());
        let path = journal_dir
            .join("frontier-locks")
            .join(format!("{}.lock", lock_id.file_stem()));
        let parent = path.parent().ok_or_else(|| {
            FrontierTxnError::Io(format!(
                "frontier lock path has no parent: {}",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            FrontierTxnError::Io(format!(
                "create frontier lock directory {}: {error}",
                parent.display()
            ))
        })?;
        let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
            FrontierTxnError::Io(format!(
                "inspect frontier lock directory {}: {error}",
                parent.display()
            ))
        })?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(FrontierTxnError::Io(format!(
                "frontier lock directory is not a regular non-symlink directory: {}",
                parent.display()
            )));
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(FrontierTxnError::Io(format!(
                    "frontier lock is not a regular non-symlink file: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(FrontierTxnError::Io(format!(
                    "inspect frontier lock {}: {error}",
                    path.display()
                )));
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                FrontierTxnError::Io(format!("open frontier lock {}: {error}", path.display()))
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(FrontierTxnError::Busy),
            Err(std::fs::TryLockError::Error(error)) => Err(FrontierTxnError::Io(format!(
                "lock frontier {}: {error}",
                path.display()
            ))),
        }
    }
}

/// An exclusive frontier lock whose recovery barrier was checked before new
/// semantic planning began. Keeping this value alive prevents another writer
/// from crossing the same barrier until it is consumed by
/// [`FrontierTxn::prepare_with_barrier`] or dropped.
#[derive(Debug)]
pub(crate) struct FrontierRecoveryBarrier {
    root: PathBuf,
    journal_dir: PathBuf,
    lock: FrontierWriteLock,
}

impl FrontierRecoveryBarrier {
    /// Re-verify a caller's complete bound read set while retaining the
    /// frontier write lock. The lock coordinates Vela writers; it is advisory
    /// with respect to arbitrary filesystem processes, so this is a pre-key
    /// early-abort check rather than the authority boundary. Marker-time
    /// verification in [`FrontierTxn::mark_committed`] remains authoritative.
    pub(crate) fn verify_read_set(
        &self,
        read_set: &[InputBinding],
    ) -> Result<(), FrontierTxnError> {
        for binding in read_set {
            binding.verify_current(&self.root)?;
        }
        Ok(())
    }

    /// Return the already-verified completed plan for one operation while this
    /// barrier owns the frontier lock. This closes the race where another
    /// process completes the same operation between an unlocked exact-retry
    /// lookup and barrier acquisition; callers can return the durable result
    /// without rederiving stale applied proposals or touching a private key.
    pub(crate) fn completed_plan(
        &self,
        operation_id: &OperationId,
    ) -> Result<Option<FrontierTxnPlan>, FrontierTxnError> {
        let journals = frontier_journals(&self.root, &self.journal_dir)?;
        for (paths, journal) in journals {
            if journal.plan.operation_id != *operation_id {
                continue;
            }
            if matches!(journal.recovery, RecoveryState::Completed) {
                verify_completed_marker_and_blobs(&paths, &journal)?;
                return Ok(Some(journal.plan));
            }
        }
        Ok(None)
    }
}

fn frontier_journals(
    root: &Path,
    journal_dir: &Path,
) -> Result<Vec<(FrontierTxnPaths, FrontierTxnJournal)>, FrontierTxnError> {
    let frontier_dir = journal_dir.join("frontier");
    let metadata = match fs::symlink_metadata(&frontier_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(FrontierTxnError::Journal(format!(
                "inspect frontier journal directory {}: {error}",
                frontier_dir.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FrontierTxnError::Journal(format!(
            "frontier journal directory is not a regular non-symlink directory: {}",
            frontier_dir.display()
        )));
    }

    let mut entries = fs::read_dir(&frontier_dir)
        .map_err(|error| {
            FrontierTxnError::Journal(format!(
                "read frontier journal directory {}: {error}",
                frontier_dir.display()
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            FrontierTxnError::Journal(format!(
                "enumerate frontier journal directory {}: {error}",
                frontier_dir.display()
            ))
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut journals = Vec::new();
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            FrontierTxnError::Journal(format!(
                "inspect frontier journal entry {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(FrontierTxnError::Journal(format!(
                "frontier journal entry is a symbolic link: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if name == "blobs" || name == "committed" {
                continue;
            }
            return Err(FrontierTxnError::Journal(format!(
                "unexpected directory in frontier journal: {}",
                path.display()
            )));
        }
        if !metadata.is_file() || path.extension().is_none_or(|extension| extension != "json") {
            return Err(FrontierTxnError::Journal(format!(
                "unexpected non-journal entry in frontier journal: {}",
                path.display()
            )));
        }

        let journal: FrontierTxnJournal =
            operation_journal::read_json(&path).map_err(FrontierTxnError::Journal)?;
        journal.verify()?;
        let paths = FrontierTxnPaths::new(journal_dir, &journal.plan.operation_id);
        if path != paths.plan {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "frontier transaction {} is stored under the wrong journal name",
                journal.plan.operation_id.as_str()
            )));
        }
        if Path::new(&journal.plan.frontier.canonical_root) == root {
            journal.plan.frontier.verify_root(root)?;
            journals.push((paths, journal));
        }
    }
    Ok(journals)
}

fn require_journal_directory(path: &Path, label: &str) -> Result<(), FrontierTxnError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        FrontierTxnError::Journal(format!("inspect {label} {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(FrontierTxnError::Journal(format!(
            "{label} is not a regular non-symlink directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn read_commit_marker(
    paths: &FrontierTxnPaths,
    journal: &FrontierTxnJournal,
) -> Result<CommitMarker, FrontierTxnError> {
    let marker_dir = paths.marker.parent().ok_or_else(|| {
        FrontierTxnError::Journal(format!(
            "frontier commit marker has no parent: {}",
            paths.marker.display()
        ))
    })?;
    match fs::symlink_metadata(marker_dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(FrontierTxnError::NotCommitted);
        }
        Err(error) => {
            return Err(FrontierTxnError::Journal(format!(
                "inspect frontier commit-marker directory {}: {error}",
                marker_dir.display()
            )));
        }
        Ok(_) => require_journal_directory(marker_dir, "frontier commit-marker directory")?,
    }
    let metadata = match fs::symlink_metadata(&paths.marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(FrontierTxnError::NotCommitted);
        }
        Err(error) => {
            return Err(FrontierTxnError::Journal(format!(
                "inspect frontier commit marker {}: {error}",
                paths.marker.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FrontierTxnError::Journal(format!(
            "frontier commit marker is not a regular non-symlink file: {}",
            paths.marker.display()
        )));
    }
    let marker: CommitMarker =
        operation_journal::read_json(&paths.marker).map_err(FrontierTxnError::Journal)?;
    let expected = CommitMarker::from_plan(&journal.plan);
    if marker != expected {
        return Err(FrontierTxnError::CorruptPlan(
            "commit marker does not match the durable plan".to_string(),
        ));
    }
    Ok(marker)
}

fn read_blob_at(
    paths: &FrontierTxnPaths,
    expected: &JournalBlobRef,
) -> Result<Vec<u8>, FrontierTxnError> {
    let path = paths.blob(&expected.digest);
    match fs::symlink_metadata(&paths.blob_dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(FrontierTxnError::MissingBlob(expected.digest.clone()));
        }
        Err(error) => {
            return Err(FrontierTxnError::Journal(format!(
                "inspect frontier transaction blob directory {}: {error}",
                paths.blob_dir.display()
            )));
        }
        Ok(_) => require_journal_directory(&paths.blob_dir, "frontier transaction blob directory")?,
    }
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(FrontierTxnError::MissingBlob(expected.digest.clone()));
        }
        Err(error) => {
            return Err(FrontierTxnError::Journal(format!(
                "inspect frontier transaction blob {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FrontierTxnError::CorruptBlob(expected.digest.clone()));
    }
    let blob: BlobJournal =
        operation_journal::read_json(&path).map_err(FrontierTxnError::Journal)?;
    if blob.schema != FRONTIER_TXN_BLOB_SCHEMA
        || blob.digest != expected.digest
        || blob.size != expected.size
    {
        return Err(FrontierTxnError::CorruptBlob(expected.digest.clone()));
    }
    validate_blob_bytes(expected, &blob.bytes)?;
    Ok(blob.bytes)
}

fn verify_journal_blobs(
    paths: &FrontierTxnPaths,
    journal: &FrontierTxnJournal,
) -> Result<(), FrontierTxnError> {
    for write in journal.plan.canonical_delta.writes() {
        if let Some(blob) = &write.payload {
            read_blob_at(paths, blob)?;
        }
    }
    Ok(())
}

fn verify_completed_marker_and_blobs(
    paths: &FrontierTxnPaths,
    journal: &FrontierTxnJournal,
) -> Result<(), FrontierTxnError> {
    if !matches!(journal.recovery, RecoveryState::Completed) {
        return Err(FrontierTxnError::CorruptPlan(format!(
            "transaction {} is not completed",
            journal.plan.operation_id.as_str()
        )));
    }
    read_commit_marker(paths, journal)?;
    verify_journal_blobs(paths, journal)
}

fn verify_aborted_without_marker(
    paths: &FrontierTxnPaths,
    journal: &FrontierTxnJournal,
) -> Result<(), FrontierTxnError> {
    if !matches!(journal.recovery, RecoveryState::Aborted) {
        return Err(FrontierTxnError::CorruptPlan(format!(
            "transaction {} is not aborted",
            journal.plan.operation_id.as_str()
        )));
    }
    match read_commit_marker(paths, journal) {
        Err(FrontierTxnError::NotCommitted) => Ok(()),
        Ok(_) => Err(FrontierTxnError::CorruptPlan(format!(
            "aborted transaction {} has a commit marker",
            journal.plan.operation_id.as_str()
        ))),
        Err(error) => Err(error),
    }
}

fn postimage_reaches_current(
    path: &RepoPath,
    postimage: &FileState,
    current: &FileState,
    current_head: &[(FrontierTxnPaths, FrontierTxnJournal)],
) -> bool {
    let mut pending = vec![postimage.clone()];
    let mut visited = Vec::new();
    while let Some(state) = pending.pop() {
        if &state == current {
            return true;
        }
        if visited.contains(&state) {
            continue;
        }
        visited.push(state.clone());
        for (_, journal) in current_head {
            for write in journal
                .plan
                .canonical_delta
                .writes()
                .iter()
                .filter(|write| &write.path == path && write.preimage == state)
            {
                pending.push(write.postimage.clone());
            }
        }
    }
    false
}

fn verify_completed_history(
    root: &Path,
    completed: &[(FrontierTxnPaths, FrontierTxnJournal)],
) -> Result<(), FrontierTxnError> {
    if completed.is_empty() {
        return Ok(());
    }
    for (paths, journal) in completed {
        verify_completed_marker_and_blobs(paths, journal)?;
    }

    let current_events = current_event_log_events(root)?;
    let current_event_ids = event_ids(&current_events)?;
    let current_root = event_log_root(&current_events)?;
    let mut current_head = Vec::new();
    for (paths, journal) in completed {
        // Event-log commitments are ID-sorted sets, not append-order chains.
        // Select the journal's exact committed membership from today's log so
        // a legitimate later event may sort anywhere without fabricating a
        // prefix relation.
        let committed_root =
            event_log_root_for_ids(&current_events, &journal.plan.resulting_event_ids)?;
        if committed_root.as_ref() != Some(&journal.plan.resulting_event_log_root) {
            return Err(FrontierTxnError::CompletedEventLogMismatch {
                operation_id: journal.plan.operation_id.as_str().to_string(),
                expected: journal.plan.resulting_event_log_root.clone(),
                actual: current_root.clone(),
            });
        }
        if journal.plan.resulting_event_ids == current_event_ids {
            current_head.push((paths.clone(), journal.clone()));
        }
    }

    // Event-neutral transactions can share one event root. Validate that each
    // of their postimages is either still current or is connected to the
    // current bytes by another completed transaction's exact preimage ->
    // postimage edge. This preserves old exact retries without treating a
    // missing latest postimage as historical.
    for (_, journal) in &current_head {
        for write in journal.plan.canonical_delta.writes() {
            let actual = inspect_file_state(root, &write.path)?;
            if postimage_reaches_current(&write.path, &write.postimage, &actual, &current_head) {
                continue;
            }
            return Err(FrontierTxnError::CompletedPostimageMismatch {
                operation_id: journal.plan.operation_id.as_str().to_string(),
                path: write.path.clone(),
                expected: Box::new(write.postimage.clone()),
                actual: Box::new(actual),
            });
        }
    }
    Ok(())
}

fn ensure_recovery_barrier_locked(
    root: &Path,
    journal_dir: &Path,
    allowed_operation_id: Option<&OperationId>,
) -> Result<(), FrontierTxnError> {
    let journals = frontier_journals(root, journal_dir)?;
    let mut completed = Vec::new();
    for (paths, journal) in journals {
        if allowed_operation_id == Some(&journal.plan.operation_id) {
            continue;
        }
        match journal.recovery {
            RecoveryState::Aborted => {
                verify_aborted_without_marker(&paths, &journal)?;
            }
            RecoveryState::Completed => completed.push((paths, journal)),
            state => {
                return Err(FrontierTxnError::RecoveryRequired {
                    operation_id: journal.plan.operation_id.as_str().to_string(),
                    state,
                });
            }
        }
    }

    verify_completed_history(root, &completed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedWrite {
    pub(crate) staged: StagedWrite,
    pub(crate) postimage_bytes: Option<Vec<u8>>,
}

fn resolve_public_writes(
    delta: &CanonicalDelta,
    mut read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, FrontierTxnError>,
) -> Result<Vec<ResolvedWrite>, FrontierTxnError> {
    let mut writes = delta
        .public_writes()
        .map(|write| {
            let postimage_bytes = match (&write.postimage, &write.payload) {
                (FileState::Absent, None) => None,
                (FileState::File { .. }, Some(blob)) => Some(read_blob(blob)?),
                (FileState::Absent, Some(_)) => {
                    return Err(FrontierTxnError::CorruptPlan(format!(
                        "deleted postimage {} carries a blob reference",
                        write.path.as_str()
                    )));
                }
                (FileState::File { .. }, None) => {
                    return Err(FrontierTxnError::CorruptPlan(format!(
                        "file postimage {} has no blob reference",
                        write.path.as_str()
                    )));
                }
            };
            Ok(ResolvedWrite {
                staged: write.clone(),
                postimage_bytes,
            })
        })
        .collect::<Result<Vec<_>, FrontierTxnError>>()?;
    // Git's exact-delta boundary is path-sorted, while installation order is
    // semantic (evidence before authority before derived views).
    writes.sort_by(|left, right| left.staged.path.cmp(&right.staged.path));
    Ok(writes)
}

fn validate_blob_bytes(expected: &JournalBlobRef, bytes: &[u8]) -> Result<(), FrontierTxnError> {
    if bytes.len() as u64 != expected.size || ContentDigest::hash(bytes) != expected.digest {
        return Err(FrontierTxnError::CorruptBlob(expected.digest.clone()));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct FrontierTxn {
    root: PathBuf,
    paths: FrontierTxnPaths,
    journal: FrontierTxnJournal,
    _lock: FrontierWriteLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryOutcome {
    Prepared,
    Aborted,
    Completed,
    AlreadyCompleted,
}

/// Private durability boundaries used by the transaction test harness.
///
/// Journal writes are atomic, fsync-backed replacements. The corresponding
/// `Before*JournalWrite` and `After*JournalWrite` points therefore model the
/// only states the recovery contract promises: the old durable record, or the
/// complete new durable record. They deliberately do not pretend to model a
/// torn JSON file inside `operation_journal::write_json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrontierTxnStep {
    BeforeBlobJournalWrite { index: usize },
    AfterBlobJournalWrite { index: usize },
    BeforePreparedJournalWrite,
    AfterPreparedJournalWrite,
    BeforeCommitMarkerWrite,
    AfterCommitMarkerWrite,
    BeforeCommittedJournalWrite,
    AfterCommittedJournalWrite,
    BeforeInstallWrite { index: usize },
    AfterInstallWrite { index: usize },
    BeforeInstallingJournalWrite { index: usize },
    AfterInstallingJournalWrite { index: usize },
    BeforeInstalledJournalWrite,
    AfterInstalledJournalWrite,
    BeforeInstalledStateVerification,
    AfterInstalledStateVerification,
    BeforeCompletedJournalWrite,
    AfterCompletedJournalWrite,
}

trait FrontierTxnFailpoints {
    fn check(&mut self, step: FrontierTxnStep) -> Result<(), FrontierTxnError>;
}

struct NoFrontierTxnFailpoints;

impl FrontierTxnFailpoints for NoFrontierTxnFailpoints {
    #[inline]
    fn check(&mut self, _step: FrontierTxnStep) -> Result<(), FrontierTxnError> {
        Ok(())
    }
}

#[cfg(test)]
struct FailAtFrontierTxnStep {
    target: FrontierTxnStep,
}

#[cfg(test)]
impl FrontierTxnFailpoints for FailAtFrontierTxnStep {
    fn check(&mut self, step: FrontierTxnStep) -> Result<(), FrontierTxnError> {
        if step == self.target {
            return Err(FrontierTxnError::InjectedFailure { step });
        }
        Ok(())
    }
}

impl FrontierTxn {
    /// Acquire the frontier-wide recovery barrier before loading mutable
    /// frontier inputs for a new operation. The returned guard deliberately
    /// holds the write lock through planning and must be consumed by
    /// [`Self::prepare_with_barrier`].
    pub(crate) fn acquire_recovery_barrier(
        frontier_root: &Path,
        journal_dir: &Path,
    ) -> Result<FrontierRecoveryBarrier, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let lock = FrontierWriteLock::acquire(journal_dir, &root)?;
        ensure_recovery_barrier_locked(&root, journal_dir, None)?;
        Ok(FrontierRecoveryBarrier {
            root,
            journal_dir: journal_dir.to_path_buf(),
            lock,
        })
    }

    pub(crate) fn prepare(
        frontier_root: &Path,
        journal_dir: &Path,
        plan: FrontierTxnPlan,
        draft: DeltaDraft,
    ) -> Result<Self, FrontierTxnError> {
        let barrier = Self::acquire_recovery_barrier(frontier_root, journal_dir)?;
        Self::prepare_with_barrier(barrier, plan, draft)
    }

    pub(crate) fn prepare_with_barrier(
        barrier: FrontierRecoveryBarrier,
        plan: FrontierTxnPlan,
        draft: DeltaDraft,
    ) -> Result<Self, FrontierTxnError> {
        Self::prepare_with_barrier_and_failpoints(
            barrier,
            plan,
            draft,
            &mut NoFrontierTxnFailpoints,
        )
    }

    fn prepare_with_barrier_and_failpoints(
        barrier: FrontierRecoveryBarrier,
        plan: FrontierTxnPlan,
        draft: DeltaDraft,
        failpoints: &mut impl FrontierTxnFailpoints,
    ) -> Result<Self, FrontierTxnError> {
        plan.verify()?;
        if plan.canonical_delta != draft.delta {
            return Err(FrontierTxnError::CorruptPlan(
                "plan delta differs from prepared postimage blobs".to_string(),
            ));
        }
        let root = plan.frontier.verify_root(&barrier.root)?;
        let FrontierRecoveryBarrier {
            root: barrier_root,
            journal_dir,
            lock,
        } = barrier;
        if root != barrier_root {
            return Err(FrontierTxnError::FrontierBindingMismatch {
                expected: barrier_root.display().to_string(),
                actual: root.display().to_string(),
            });
        }
        let paths = FrontierTxnPaths::new(&journal_dir, &plan.operation_id);

        match fs::symlink_metadata(&paths.plan) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(FrontierTxnError::Journal(format!(
                    "frontier transaction journal is not a regular non-symlink file: {}",
                    paths.plan.display()
                )));
            }
            Ok(_) => {
                let journal: FrontierTxnJournal =
                    operation_journal::read_json(&paths.plan).map_err(FrontierTxnError::Journal)?;
                journal.verify()?;
                if matches!(journal.recovery, RecoveryState::Aborted) {
                    verify_aborted_without_marker(&paths, &journal)?;
                    if journal.plan.request_root != plan.request_root {
                        return Err(FrontierTxnError::OperationConflict {
                            operation_id: plan.operation_id.as_str().to_string(),
                        });
                    }
                } else {
                    if journal.plan.root != plan.root {
                        return Err(FrontierTxnError::OperationConflict {
                            operation_id: plan.operation_id.as_str().to_string(),
                        });
                    }
                    let txn = Self {
                        root,
                        paths,
                        journal,
                        _lock: lock,
                    };
                    txn.verify_blobs()?;
                    if matches!(txn.journal.recovery, RecoveryState::Completed) {
                        txn.verify_completed_state()?;
                    }
                    return Ok(txn);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(FrontierTxnError::Journal(format!(
                    "inspect frontier transaction {}: {error}",
                    paths.plan.display()
                )));
            }
        }

        for (index, (digest, bytes)) in draft.blobs.iter().enumerate() {
            let blob = BlobJournal {
                schema: FRONTIER_TXN_BLOB_SCHEMA.to_string(),
                digest: digest.clone(),
                size: bytes.len() as u64,
                bytes: bytes.clone(),
            };
            failpoints.check(FrontierTxnStep::BeforeBlobJournalWrite { index })?;
            operation_journal::write_json(&paths.blob(digest), &blob)
                .map_err(FrontierTxnError::Journal)?;
            failpoints.check(FrontierTxnStep::AfterBlobJournalWrite { index })?;
        }
        let journal = FrontierTxnJournal {
            schema: FRONTIER_TXN_SCHEMA.to_string(),
            plan,
            recovery: RecoveryState::Prepared,
        };
        failpoints.check(FrontierTxnStep::BeforePreparedJournalWrite)?;
        operation_journal::write_json(&paths.plan, &journal).map_err(FrontierTxnError::Journal)?;
        failpoints.check(FrontierTxnStep::AfterPreparedJournalWrite)?;
        let txn = Self {
            root,
            paths,
            journal,
            _lock: lock,
        };
        txn.verify_blobs()?;
        Ok(txn)
    }

    #[cfg(test)]
    fn prepare_at_failpoint(
        frontier_root: &Path,
        journal_dir: &Path,
        plan: FrontierTxnPlan,
        draft: DeltaDraft,
        step: FrontierTxnStep,
    ) -> Result<Self, FrontierTxnError> {
        let barrier = Self::acquire_recovery_barrier(frontier_root, journal_dir)?;
        Self::prepare_with_barrier_and_failpoints(
            barrier,
            plan,
            draft,
            &mut FailAtFrontierTxnStep { target: step },
        )
    }

    pub(crate) fn open(
        frontier_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Self, FrontierTxnError> {
        Self::open_if_present(frontier_root, journal_dir, operation_id)?.ok_or_else(|| {
            FrontierTxnError::Journal(format!(
                "frontier transaction {} was not found",
                operation_id.as_str()
            ))
        })
    }

    pub(crate) fn open_if_present(
        frontier_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Option<Self>, FrontierTxnError> {
        let root = canonical_frontier_root(frontier_root)?;
        let lock = FrontierWriteLock::acquire(journal_dir, &root)?;
        let paths = FrontierTxnPaths::new(journal_dir, operation_id);
        let frontier_journal_dir = paths.plan.parent().ok_or_else(|| {
            FrontierTxnError::Journal(format!(
                "frontier transaction has no journal directory: {}",
                paths.plan.display()
            ))
        })?;
        match fs::symlink_metadata(frontier_journal_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(FrontierTxnError::Journal(format!(
                    "inspect frontier journal directory {}: {error}",
                    frontier_journal_dir.display()
                )));
            }
            Ok(_) => require_journal_directory(frontier_journal_dir, "frontier journal directory")?,
        }
        match fs::symlink_metadata(&paths.plan) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(FrontierTxnError::Journal(format!(
                    "inspect frontier transaction {}: {error}",
                    paths.plan.display()
                )));
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(FrontierTxnError::Journal(format!(
                    "frontier transaction journal is not a regular non-symlink file: {}",
                    paths.plan.display()
                )));
            }
            Ok(_) => {}
        }
        let journal: FrontierTxnJournal =
            operation_journal::read_json(&paths.plan).map_err(FrontierTxnError::Journal)?;
        journal.verify()?;
        journal.plan.frontier.verify_root(&root)?;
        let txn = Self {
            root,
            paths,
            journal,
            _lock: lock,
        };
        match txn.journal.recovery {
            RecoveryState::Aborted => {
                verify_aborted_without_marker(&txn.paths, &txn.journal)?;
            }
            RecoveryState::Completed => {
                txn.verify_blobs()?;
                txn.verify_completed_state()?;
            }
            _ => txn.verify_blobs()?,
        }
        Ok(Some(txn))
    }

    pub(crate) fn plan(&self) -> &FrontierTxnPlan {
        &self.journal.plan
    }

    pub(crate) fn recovery_state(&self) -> &RecoveryState {
        &self.journal.recovery
    }

    pub(crate) fn mark_committed(&mut self) -> Result<(), FrontierTxnError> {
        self.mark_committed_with_failpoints(&mut NoFrontierTxnFailpoints)
    }

    fn mark_committed_with_failpoints(
        &mut self,
        failpoints: &mut impl FrontierTxnFailpoints,
    ) -> Result<(), FrontierTxnError> {
        let expected_marker = CommitMarker::from_plan(&self.journal.plan);
        match read_commit_marker(&self.paths, &self.journal) {
            Ok(marker) => {
                self.verify_marker(&marker)?;
                if matches!(self.journal.recovery, RecoveryState::Completed) {
                    return self.verify_completed_state();
                }
                if matches!(
                    self.journal.recovery,
                    RecoveryState::Installed | RecoveryState::Installing { .. }
                ) {
                    return Ok(());
                }
            }
            Err(FrontierTxnError::NotCommitted) => {
                if !matches!(self.journal.recovery, RecoveryState::Prepared) {
                    return Err(FrontierTxnError::CorruptPlan(format!(
                        "transaction {} is {:?} but has no commit marker",
                        self.journal.plan.operation_id.as_str(),
                        self.journal.recovery
                    )));
                }
                let preflight = (|| {
                    ensure_recovery_barrier_locked(
                        &self.root,
                        self.paths
                            .plan
                            .parent()
                            .and_then(Path::parent)
                            .ok_or_else(|| {
                                FrontierTxnError::Journal(format!(
                                    "frontier transaction path has no journal root: {}",
                                    self.paths.plan.display()
                                ))
                            })?,
                        Some(&self.journal.plan.operation_id),
                    )?;
                    for input in &self.journal.plan.read_set {
                        input.verify_current(&self.root)?;
                    }
                    let current_events = current_event_log_events(&self.root)?;
                    let actual_event_log_root = event_log_root(&current_events)?;
                    if actual_event_log_root != self.journal.plan.expected_event_log_root {
                        return Err(FrontierTxnError::StaleEventLog {
                            expected: self.journal.plan.expected_event_log_root.clone(),
                            actual: actual_event_log_root,
                        });
                    }
                    for write in self.journal.plan.canonical_delta.writes() {
                        let current = inspect_file_state(&self.root, &write.path)?;
                        if current != write.preimage {
                            return Err(FrontierTxnError::StalePreimage {
                                path: write.path.clone(),
                                expected: write.preimage.clone(),
                                actual: current,
                            });
                        }
                    }
                    self.verify_blobs()?;
                    self.verify_resulting_event_commitment(&current_events)
                })();
                if let Err(error) = preflight {
                    self.abort_prepared()?;
                    return Err(error);
                }
                failpoints.check(FrontierTxnStep::BeforeCommitMarkerWrite)?;
                operation_journal::write_json(&self.paths.marker, &expected_marker)
                    .map_err(FrontierTxnError::Journal)?;
                failpoints.check(FrontierTxnStep::AfterCommitMarkerWrite)?;
            }
            Err(error) => return Err(error),
        }
        self.journal.recovery = RecoveryState::Committed;
        failpoints.check(FrontierTxnStep::BeforeCommittedJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(FrontierTxnStep::AfterCommittedJournalWrite)
    }

    #[cfg(test)]
    fn mark_committed_at_failpoint(
        &mut self,
        step: FrontierTxnStep,
    ) -> Result<(), FrontierTxnError> {
        self.mark_committed_with_failpoints(&mut FailAtFrontierTxnStep { target: step })
    }

    /// Permanently discard a marker-free plan. Since no commit marker exists,
    /// this state transition has no frontier delta and a later plan may safely
    /// reuse the operation id.
    pub(crate) fn abort_prepared(&mut self) -> Result<(), FrontierTxnError> {
        if !matches!(self.journal.recovery, RecoveryState::Prepared) {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "cannot abort transaction {} from {:?}",
                self.journal.plan.operation_id.as_str(),
                self.journal.recovery
            )));
        }
        match read_commit_marker(&self.paths, &self.journal) {
            Err(FrontierTxnError::NotCommitted) => {}
            Ok(_) => {
                return Err(FrontierTxnError::CorruptPlan(format!(
                    "cannot abort committed transaction {}",
                    self.journal.plan.operation_id.as_str()
                )));
            }
            Err(error) => return Err(error),
        }
        self.journal.recovery = RecoveryState::Aborted;
        self.persist_journal()
    }

    pub(crate) fn install(&mut self) -> Result<(), FrontierTxnError> {
        self.install_with_failpoints(&mut NoFrontierTxnFailpoints)
    }

    fn install_with_failpoints(
        &mut self,
        failpoints: &mut impl FrontierTxnFailpoints,
    ) -> Result<(), FrontierTxnError> {
        if matches!(self.journal.recovery, RecoveryState::Completed) {
            return self.verify_completed_state();
        }
        let marker = read_commit_marker(&self.paths, &self.journal)?;
        self.verify_marker(&marker)?;
        let writes = self.journal.plan.canonical_delta.writes.clone();
        let total = writes.len();
        for (index, write) in writes.into_iter().enumerate() {
            let current = inspect_file_state(&self.root, &write.path)?;
            if current != write.postimage {
                if current != write.preimage {
                    self.journal.recovery = RecoveryState::CommittedConflict {
                        path: write.path.clone(),
                    };
                    self.persist_journal()?;
                    return Err(FrontierTxnError::CommittedConflict {
                        path: write.path,
                        expected_preimage: Box::new(write.preimage),
                        expected_postimage: Box::new(write.postimage),
                        actual: Box::new(current),
                    });
                }
                failpoints.check(FrontierTxnStep::BeforeInstallWrite { index })?;
                self.install_write(&write)?;
                failpoints.check(FrontierTxnStep::AfterInstallWrite { index })?;
            }
            let installed = index + 1;
            self.journal.recovery = RecoveryState::Installing { installed, total };
            failpoints.check(FrontierTxnStep::BeforeInstallingJournalWrite { index })?;
            self.persist_journal()?;
            failpoints.check(FrontierTxnStep::AfterInstallingJournalWrite { index })?;
        }
        self.journal.recovery = RecoveryState::Installed;
        failpoints.check(FrontierTxnStep::BeforeInstalledJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(FrontierTxnStep::AfterInstalledJournalWrite)
    }

    #[cfg(test)]
    pub(crate) fn install_at_failpoint(
        &mut self,
        step: FrontierTxnStep,
    ) -> Result<(), FrontierTxnError> {
        self.install_with_failpoints(&mut FailAtFrontierTxnStep { target: step })
    }

    pub(crate) fn complete(&mut self) -> Result<(), FrontierTxnError> {
        self.complete_with_failpoints(&mut NoFrontierTxnFailpoints)
    }

    fn complete_with_failpoints(
        &mut self,
        failpoints: &mut impl FrontierTxnFailpoints,
    ) -> Result<(), FrontierTxnError> {
        if !matches!(
            self.journal.recovery,
            RecoveryState::Installed | RecoveryState::Completed
        ) {
            return Err(FrontierTxnError::CorruptPlan(
                "cannot complete a transaction before all writes are installed".to_string(),
            ));
        }
        failpoints.check(FrontierTxnStep::BeforeInstalledStateVerification)?;
        self.verify_installed_state()?;
        failpoints.check(FrontierTxnStep::AfterInstalledStateVerification)?;
        if matches!(self.journal.recovery, RecoveryState::Completed) {
            return Ok(());
        }
        self.journal.recovery = RecoveryState::Completed;
        failpoints.check(FrontierTxnStep::BeforeCompletedJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(FrontierTxnStep::AfterCompletedJournalWrite)
    }

    #[cfg(test)]
    fn complete_at_failpoint(&mut self, step: FrontierTxnStep) -> Result<(), FrontierTxnError> {
        self.complete_with_failpoints(&mut FailAtFrontierTxnStep { target: step })
    }

    pub(crate) fn recover(
        frontier_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<RecoveryOutcome, FrontierTxnError> {
        let mut txn = Self::open(frontier_root, journal_dir, operation_id)?;
        if matches!(txn.journal.recovery, RecoveryState::Aborted) {
            return Ok(RecoveryOutcome::Aborted);
        }
        if matches!(txn.journal.recovery, RecoveryState::Completed) {
            return Ok(RecoveryOutcome::AlreadyCompleted);
        }
        match read_commit_marker(&txn.paths, &txn.journal) {
            Ok(_) => {}
            Err(FrontierTxnError::NotCommitted)
                if matches!(txn.journal.recovery, RecoveryState::Prepared) =>
            {
                return Ok(RecoveryOutcome::Prepared);
            }
            Err(FrontierTxnError::NotCommitted) => {
                return Err(FrontierTxnError::CorruptPlan(format!(
                    "transaction {} is {:?} but has no commit marker",
                    txn.journal.plan.operation_id.as_str(),
                    txn.journal.recovery
                )));
            }
            Err(error) => return Err(error),
        }
        txn.install()?;
        txn.complete()?;
        Ok(RecoveryOutcome::Completed)
    }

    pub(crate) fn resolved_public_writes(&self) -> Result<Vec<ResolvedWrite>, FrontierTxnError> {
        resolve_public_writes(&self.journal.plan.canonical_delta, |blob| {
            self.read_blob(blob)
        })
    }

    fn install_write(&self, write: &StagedWrite) -> Result<(), FrontierTxnError> {
        let target = write.path.target(&self.root)?;
        match &write.postimage {
            FileState::Absent => atomic_delete(&self.root, &target),
            FileState::File { mode, .. } => {
                let blob = write.payload.as_ref().ok_or_else(|| {
                    FrontierTxnError::CorruptPlan(format!(
                        "file postimage {} has no blob reference",
                        write.path.as_str()
                    ))
                })?;
                let bytes = self.read_blob(blob)?;
                atomic_write(&self.root, &target, &bytes, *mode)
            }
        }
    }

    fn verify_marker(&self, marker: &CommitMarker) -> Result<(), FrontierTxnError> {
        let expected = CommitMarker::from_plan(&self.journal.plan);
        if marker != &expected {
            return Err(FrontierTxnError::CorruptPlan(
                "commit marker does not match the durable plan".to_string(),
            ));
        }
        Ok(())
    }

    fn verify_blobs(&self) -> Result<(), FrontierTxnError> {
        verify_journal_blobs(&self.paths, &self.journal)
    }

    fn verify_resulting_event_commitment(
        &self,
        current_events: &[vela_protocol::events::StateEvent],
    ) -> Result<(), FrontierTxnError> {
        let mut events = BTreeMap::<String, vela_protocol::events::StateEvent>::new();
        for event in current_events {
            if events.insert(event.id.clone(), event.clone()).is_some() {
                return Err(FrontierTxnError::Io(format!(
                    "event log contains duplicate event id {}",
                    event.id
                )));
            }
        }

        for write in self.journal.plan.canonical_delta.writes() {
            let Some(relative) = write.path.as_str().strip_prefix(".vela/events/") else {
                continue;
            };
            let Some(event_id) = relative.strip_suffix(".json") else {
                return Err(FrontierTxnError::CorruptPlan(format!(
                    "event-log write has a non-JSON path: {}",
                    write.path.as_str()
                )));
            };
            if event_id.is_empty() || event_id.contains('/') {
                return Err(FrontierTxnError::CorruptPlan(format!(
                    "event-log write is not one direct event file: {}",
                    write.path.as_str()
                )));
            }

            match &write.postimage {
                FileState::Absent => {
                    events.remove(event_id);
                }
                FileState::File { .. } => {
                    let blob = write.payload.as_ref().ok_or_else(|| {
                        FrontierTxnError::CorruptPlan(format!(
                            "event-log postimage {} has no journal blob",
                            write.path.as_str()
                        ))
                    })?;
                    let bytes = self.read_blob(blob)?;
                    let event = serde_json::from_slice::<vela_protocol::events::StateEvent>(&bytes)
                        .map_err(|error| {
                            FrontierTxnError::CorruptPlan(format!(
                                "event-log postimage {} is not a StateEvent: {error}",
                                write.path.as_str()
                            ))
                        })?;
                    if event.id != event_id {
                        return Err(FrontierTxnError::CorruptPlan(format!(
                            "event-log postimage {} contains event id {}",
                            write.path.as_str(),
                            event.id
                        )));
                    }
                    events.insert(event_id.to_string(), event);
                }
            }
        }

        let events = events.into_values().collect::<Vec<_>>();
        let actual_ids = event_ids(&events)?;
        let actual_root = event_log_root(&events)?;
        if actual_ids != self.journal.plan.resulting_event_ids
            || actual_root != self.journal.plan.resulting_event_log_root
        {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "resulting event-log commitment does not match staged event postimages: planned {}, derived {}",
                self.journal.plan.resulting_event_log_root.as_str(),
                actual_root.as_str()
            )));
        }
        Ok(())
    }

    fn read_blob(&self, expected: &JournalBlobRef) -> Result<Vec<u8>, FrontierTxnError> {
        read_blob_at(&self.paths, expected)
    }

    fn verify_installed_state(&self) -> Result<(), FrontierTxnError> {
        read_commit_marker(&self.paths, &self.journal)?;
        self.verify_blobs()?;
        for write in self.journal.plan.canonical_delta.writes() {
            let actual = inspect_file_state(&self.root, &write.path)?;
            if actual != write.postimage {
                return Err(FrontierTxnError::CompletedPostimageMismatch {
                    operation_id: self.journal.plan.operation_id.as_str().to_string(),
                    path: write.path.clone(),
                    expected: Box::new(write.postimage.clone()),
                    actual: Box::new(actual),
                });
            }
        }
        let events = current_event_log_events(&self.root)?;
        let actual_event_ids = event_ids(&events)?;
        let actual = event_log_root(&events)?;
        if actual_event_ids != self.journal.plan.resulting_event_ids
            || actual != self.journal.plan.resulting_event_log_root
        {
            return Err(FrontierTxnError::CompletedEventLogMismatch {
                operation_id: self.journal.plan.operation_id.as_str().to_string(),
                expected: self.journal.plan.resulting_event_log_root.clone(),
                actual,
            });
        }
        Ok(())
    }

    fn verify_completed_state(&self) -> Result<(), FrontierTxnError> {
        let journal_dir = self
            .paths
            .plan
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                FrontierTxnError::Journal(format!(
                    "frontier transaction path has no journal root: {}",
                    self.paths.plan.display()
                ))
            })?;
        ensure_recovery_barrier_locked(&self.root, journal_dir, None)
    }

    fn persist_journal(&self) -> Result<(), FrontierTxnError> {
        operation_journal::write_json(&self.paths.plan, &self.journal)
            .map_err(FrontierTxnError::Journal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrontierTxnError {
    InvalidDigest(String),
    InvalidOperationId(String),
    InvalidPath {
        path: String,
        reason: String,
    },
    UnsafeTarget {
        path: RepoPath,
        reason: String,
    },
    DuplicatePath(String),
    FrontierBindingMismatch {
        expected: String,
        actual: String,
    },
    OperationConflict {
        operation_id: String,
    },
    RecoveryRequired {
        operation_id: String,
        state: RecoveryState,
    },
    StalePreimage {
        path: RepoPath,
        expected: FileState,
        actual: FileState,
    },
    StaleInput {
        name: String,
        path: RepoPath,
        expected: ContentDigest,
        actual: ContentDigest,
    },
    StaleSnapshot {
        name: String,
        expected: ContentDigest,
        actual: ContentDigest,
    },
    StaleEventLog {
        expected: ContentDigest,
        actual: ContentDigest,
    },
    CommittedConflict {
        path: RepoPath,
        expected_preimage: Box<FileState>,
        expected_postimage: Box<FileState>,
        actual: Box<FileState>,
    },
    CompletedPostimageMismatch {
        operation_id: String,
        path: RepoPath,
        expected: Box<FileState>,
        actual: Box<FileState>,
    },
    CompletedEventLogMismatch {
        operation_id: String,
        expected: ContentDigest,
        actual: ContentDigest,
    },
    MissingBlob(ContentDigest),
    CorruptBlob(ContentDigest),
    NotCommitted,
    Busy,
    #[cfg(test)]
    InjectedFailure {
        step: FrontierTxnStep,
    },
    Canonicalize(String),
    CorruptPlan(String),
    Journal(String),
    Io(String),
}

impl fmt::Display for FrontierTxnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDigest(value) => write!(formatter, "invalid SHA-256 digest: {value}"),
            Self::InvalidOperationId(value) => write!(formatter, "invalid operation id: {value}"),
            Self::InvalidPath { path, reason } => {
                write!(formatter, "invalid path {path:?}: {reason}")
            }
            Self::UnsafeTarget { path, reason } => {
                write!(formatter, "unsafe target {}: {reason}", path.as_str())
            }
            Self::DuplicatePath(path) => write!(formatter, "duplicate staged path: {path}"),
            Self::FrontierBindingMismatch { expected, actual } => write!(
                formatter,
                "frontier binding mismatch: expected {expected}, found {actual}"
            ),
            Self::OperationConflict { operation_id } => write!(
                formatter,
                "operation id {operation_id} is already bound to a different plan"
            ),
            Self::RecoveryRequired {
                operation_id,
                state,
            } => write!(
                formatter,
                "frontier transaction {operation_id} requires recovery from {state:?} before another operation can plan or commit"
            ),
            Self::StalePreimage { path, .. } => {
                write!(
                    formatter,
                    "preimage changed before commit: {}",
                    path.as_str()
                )
            }
            Self::StaleInput { name, path, .. } => write!(
                formatter,
                "frontier input {name} changed before commit at {}",
                path.as_str()
            ),
            Self::StaleSnapshot { name, .. } => {
                write!(formatter, "frontier snapshot {name} changed before commit")
            }
            Self::StaleEventLog { expected, actual } => write!(
                formatter,
                "event log changed before commit: expected {}, found {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::CommittedConflict { path, .. } => write!(
                formatter,
                "committed transaction conflicts at {}; refusing to overwrite drift",
                path.as_str()
            ),
            Self::CompletedPostimageMismatch {
                operation_id, path, ..
            } => write!(
                formatter,
                "completed frontier transaction {operation_id} has a missing or corrupt postimage at {}",
                path.as_str()
            ),
            Self::CompletedEventLogMismatch {
                operation_id,
                expected,
                actual,
            } => write!(
                formatter,
                "completed frontier transaction {operation_id} does not match the accepted-event root: expected {}, found {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::MissingBlob(digest) => {
                write!(formatter, "missing transaction blob {}", digest.as_str())
            }
            Self::CorruptBlob(digest) => {
                write!(formatter, "corrupt transaction blob {}", digest.as_str())
            }
            Self::NotCommitted => write!(formatter, "frontier transaction has no commit marker"),
            Self::Busy => write!(
                formatter,
                "another frontier transaction holds the write lock"
            ),
            #[cfg(test)]
            Self::InjectedFailure { step } => {
                write!(
                    formatter,
                    "injected frontier transaction failure at {step:?}"
                )
            }
            Self::Canonicalize(error) => write!(formatter, "canonicalize transaction: {error}"),
            Self::CorruptPlan(error) => write!(formatter, "corrupt transaction plan: {error}"),
            Self::Journal(error) => write!(formatter, "frontier transaction journal: {error}"),
            Self::Io(error) => write!(formatter, "frontier transaction I/O: {error}"),
        }
    }
}

impl std::error::Error for FrontierTxnError {}

fn canonical_frontier_root(path: &Path) -> Result<PathBuf, FrontierTxnError> {
    let metadata = fs::metadata(path).map_err(|error| {
        FrontierTxnError::Io(format!("read frontier root {}: {error}", path.display()))
    })?;
    if !metadata.is_dir() {
        return Err(FrontierTxnError::Io(format!(
            "frontier root is not a directory: {}",
            path.display()
        )));
    }
    fs::canonicalize(path).map_err(|error| {
        FrontierTxnError::Io(format!(
            "canonicalize frontier root {}: {error}",
            path.display()
        ))
    })
}

/// Reject path escapes, symbolic links, and non-directory ancestors observed
/// while resolving a transaction target.
///
/// This is a fail-closed check for a stable filesystem plus Vela's cooperative
/// frontier lock; it is not a sandbox against a hostile process that can
/// mutate the frontier with the same operating-system permissions. Rust's
/// portable `std::fs` path APIs do not provide a complete dirfd-relative,
/// no-follow rename/unlink walk, and this crate denies unsafe code. A hostile
/// local process can therefore race an ancestor rename between this check and
/// a later path-based read, rename, or unlink. Every preflight and install
/// rechecks the path and refuses observed drift, but deployments that require
/// protection from such a process must protect the frontier directory with OS
/// ownership/permissions. Do not describe this function as eliminating that
/// TOCTOU boundary.
fn validate_target(root: &Path, path: &RepoPath) -> Result<PathBuf, FrontierTxnError> {
    let mut current = root.to_path_buf();
    let segments = path.as_str().split('/').collect::<Vec<_>>();
    for (index, segment) in segments.iter().enumerate() {
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(FrontierTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is a symbolic link", current.display()),
                    });
                }
                let is_target = index + 1 == segments.len();
                if !is_target && !metadata.is_dir() {
                    return Err(FrontierTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is not a directory", current.display()),
                    });
                }
                if is_target && !metadata.is_file() {
                    return Err(FrontierTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is not a regular file", current.display()),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(FrontierTxnError::Io(format!(
                    "inspect transaction target {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(root.join(path.as_str()))
}

fn current_event_log_events(
    root: &Path,
) -> Result<Vec<vela_protocol::events::StateEvent>, FrontierTxnError> {
    let vela_dir = root.join(".vela");
    let events_dir = vela_dir.join("events");
    let mut events = Vec::new();
    for directory in [&vela_dir, &events_dir] {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(FrontierTxnError::Io(format!(
                    "event log directory is not a safe directory: {}",
                    directory.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(FrontierTxnError::Io(format!(
                    "inspect event log directory {}: {error}",
                    directory.display()
                )));
            }
        }
    }
    if events_dir.exists() {
        let mut paths = fs::read_dir(&events_dir)
            .map_err(|error| {
                FrontierTxnError::Io(format!(
                    "read event log directory {}: {error}",
                    events_dir.display()
                ))
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                FrontierTxnError::Io(format!(
                    "inspect event log file {}: {error}",
                    path.display()
                ))
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(FrontierTxnError::Io(format!(
                    "event log entry is not a regular non-symlink file: {}",
                    path.display()
                )));
            }
            let bytes = fs::read(&path).map_err(|error| {
                FrontierTxnError::Io(format!("read event log file {}: {error}", path.display()))
            })?;
            let event = serde_json::from_slice::<vela_protocol::events::StateEvent>(&bytes)
                .map_err(|error| {
                    FrontierTxnError::Io(format!(
                        "parse event log file {}: {error}",
                        path.display()
                    ))
                })?;
            let file_id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| {
                    FrontierTxnError::Io(format!(
                        "event log filename is not a portable UTF-8 event id: {}",
                        path.display()
                    ))
                })?;
            if event.id != file_id {
                return Err(FrontierTxnError::Io(format!(
                    "event log file {} contains event id {}",
                    path.display(),
                    event.id
                )));
            }
            events.push(event);
        }
    }
    events.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(events)
}

fn event_log_root(
    events: &[vela_protocol::events::StateEvent],
) -> Result<ContentDigest, FrontierTxnError> {
    for event in events {
        let derived = vela_protocol::events::event_id(event);
        if event.id != derived {
            return Err(FrontierTxnError::CorruptPlan(format!(
                "event {} does not match its content-derived id {derived}",
                event.id
            )));
        }
    }
    ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(events)
    ))
}

fn verify_resulting_event_ids(ids: &[String]) -> Result<(), FrontierTxnError> {
    if ids.iter().any(|id| id.is_empty()) {
        return Err(FrontierTxnError::CorruptPlan(
            "resulting event ids contain an empty id".to_string(),
        ));
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FrontierTxnError::CorruptPlan(
            "resulting event ids are not sorted and unique".to_string(),
        ));
    }
    Ok(())
}

fn event_ids(
    events: &[vela_protocol::events::StateEvent],
) -> Result<Vec<String>, FrontierTxnError> {
    let mut ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    verify_resulting_event_ids(&ids).map_err(|_| {
        FrontierTxnError::Io("event log contains an empty or duplicate event id".to_string())
    })?;
    Ok(ids)
}

fn event_log_root_for_ids(
    events: &[vela_protocol::events::StateEvent],
    ids: &[String],
) -> Result<Option<ContentDigest>, FrontierTxnError> {
    verify_resulting_event_ids(ids)?;
    let mut by_id = BTreeMap::new();
    for event in events {
        if by_id.insert(event.id.as_str(), event).is_some() {
            return Err(FrontierTxnError::Io(format!(
                "event log contains duplicate event id {}",
                event.id
            )));
        }
    }
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(event) = by_id.get(id.as_str()) else {
            return Ok(None);
        };
        selected.push((*event).clone());
    }
    event_log_root(&selected).map(Some)
}

fn current_event_log_root(root: &Path) -> Result<ContentDigest, FrontierTxnError> {
    event_log_root(&current_event_log_events(root)?)
}

fn inspect_file_state(root: &Path, path: &RepoPath) -> Result<FileState, FrontierTxnError> {
    let target = validate_target(root, path)?;
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(FileState::Absent),
        Err(error) => {
            return Err(FrontierTxnError::Io(format!(
                "inspect transaction target {}: {error}",
                target.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FrontierTxnError::UnsafeTarget {
            path: path.clone(),
            reason: "target is not a regular, non-symlink file".to_string(),
        });
    }
    let bytes = fs::read(&target).map_err(|error| {
        FrontierTxnError::Io(format!(
            "read transaction target {}: {error}",
            target.display()
        ))
    })?;
    Ok(FileState::File {
        digest: ContentDigest::hash(&bytes),
        size: bytes.len() as u64,
        mode: file_mode(&metadata),
    })
}

fn file_mode(metadata: &fs::Metadata) -> FileMode {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            return FileMode::Executable;
        }
    }
    FileMode::Regular
}

fn ensure_parent_dirs(root: &Path, parent: &Path) -> Result<(), FrontierTxnError> {
    let relative = parent.strip_prefix(root).map_err(|_| {
        FrontierTxnError::Io(format!(
            "transaction parent {} escaped frontier {}",
            parent.display(),
            root.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(FrontierTxnError::Io(format!(
                "transaction parent is not normalized: {}",
                parent.display()
            )));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(FrontierTxnError::Io(format!(
                    "transaction parent is not a safe directory: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let previous = current.parent().expect("frontier child has a parent");
                fs::create_dir(&current).map_err(|error| {
                    FrontierTxnError::Io(format!(
                        "create transaction directory {}: {error}",
                        current.display()
                    ))
                })?;
                sync_directory(previous)?;
            }
            Err(error) => {
                return Err(FrontierTxnError::Io(format!(
                    "inspect transaction directory {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn atomic_write(
    root: &Path,
    target: &Path,
    bytes: &[u8],
    mode: FileMode,
) -> Result<(), FrontierTxnError> {
    let parent = target.parent().ok_or_else(|| {
        FrontierTxnError::Io(format!(
            "transaction target has no parent: {}",
            target.display()
        ))
    })?;
    ensure_parent_dirs(root, parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        FrontierTxnError::Io(format!(
            "create transaction temporary file in {}: {error}",
            parent.display()
        ))
    })?;
    temporary.write_all(bytes).map_err(|error| {
        FrontierTxnError::Io(format!(
            "write transaction postimage {}: {error}",
            target.display()
        ))
    })?;
    set_mode(temporary.as_file(), mode)?;
    temporary.as_file().sync_all().map_err(|error| {
        FrontierTxnError::Io(format!(
            "fsync transaction postimage {}: {error}",
            target.display()
        ))
    })?;
    temporary.persist(target).map_err(|error| {
        FrontierTxnError::Io(format!(
            "install transaction postimage {}: {}",
            target.display(),
            error.error
        ))
    })?;
    sync_directory(parent)
}

fn atomic_delete(root: &Path, target: &Path) -> Result<(), FrontierTxnError> {
    let parent = target.parent().ok_or_else(|| {
        FrontierTxnError::Io(format!(
            "transaction target has no parent: {}",
            target.display()
        ))
    })?;
    ensure_parent_dirs(root, parent)?;
    match fs::remove_file(target) {
        Ok(()) => sync_directory(parent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FrontierTxnError::Io(format!(
            "delete transaction target {}: {error}",
            target.display()
        ))),
    }
}

fn set_mode(file: &File, mode: FileMode) -> Result<(), FrontierTxnError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(match mode {
            FileMode::Regular => 0o644,
            FileMode::Executable => 0o755,
        });
        file.set_permissions(permissions)
            .map_err(|error| FrontierTxnError::Io(format!("set transaction file mode: {error}")))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), FrontierTxnError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            FrontierTxnError::Io(format!(
                "fsync transaction directory {}: {error}",
                path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture_plan(root: &Path, draft: &DeltaDraft, identity: &[u8]) -> FrontierTxnPlan {
        let operation_id = OperationId::derive("submission", identity);
        let request_root = ContentDigest::hash(identity);
        let resulting_event_ids = current_event_log_events(root)
            .unwrap()
            .into_iter()
            .map(|event| event.id)
            .collect();
        FrontierTxnPlan::new(
            FrontierTxnPlanSpec {
                kind: OperationKind::Submission,
                operation_id,
                request_root,
                frontier: FrontierBinding::new(root, "vfr_test", b"split-layout-v1").unwrap(),
                fixed_time: "2026-07-13T00:00:00Z".to_string(),
                expected_event_log_root: current_event_log_root(root).unwrap(),
                resulting_event_log_root: current_event_log_root(root).unwrap(),
                resulting_event_ids,
                read_set: vec![InputBinding {
                    name: "receipt".to_string(),
                    digest: ContentDigest::hash(b"receipt"),
                }],
                result: json!({"proposal_id": "vpr_test"}),
            },
            draft.delta.clone(),
        )
        .unwrap()
    }

    fn fixture_event(label: &str) -> vela_protocol::events::StateEvent {
        let mut event = vela_protocol::events::StateEvent {
            schema: vela_protocol::events::EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: vela_protocol::events::EventKind::FrontierCreated,
            target: vela_protocol::events::StateTarget {
                r#type: "frontier".to_string(),
                id: "vfr_fixture".to_string(),
            },
            actor: vela_protocol::events::StateActor {
                id: "reviewer:fixture".to_string(),
                r#type: "human".to_string(),
            },
            timestamp: "2026-07-13T00:00:01Z".to_string(),
            reason: format!("fixture event: {label}"),
            before_hash: "sha256:null".to_string(),
            after_hash: "sha256:null".to_string(),
            payload: json!({}),
            caveats: vec![],
            signature: None,
            schema_artifact_id: None,
        };
        event.id = vela_protocol::events::event_id(&event);
        event
    }

    fn initialize_failpoint_frontier(root: &Path) {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join("keep.txt"), b"unchanged").unwrap();
        fs::write(root.join("obsolete.json"), b"remove me").unwrap();
    }

    fn failpoint_writes() -> Vec<PlannedWrite> {
        vec![
            PlannedWrite::write(
                RepoPath::parse("records/evidence.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"evidence".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("records/review/pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("records/authority.json").unwrap(),
                WriteClass::Authority,
                b"authority".to_vec(),
            ),
            PlannedWrite::write(
                RepoPath::parse("frontier.json").unwrap(),
                WriteClass::Derived,
                b"materialized frontier".to_vec(),
            ),
            PlannedWrite::delete(
                RepoPath::parse("obsolete.json").unwrap(),
                WriteClass::Derived,
            ),
            PlannedWrite::write(
                RepoPath::parse(".vela/work/session.json").unwrap(),
                WriteClass::PrivateCoordination,
                b"closed".to_vec(),
            ),
        ]
    }

    fn snapshot_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
            let mut entries = fs::read_dir(directory)
                .unwrap()
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                assert!(
                    !metadata.file_type().is_symlink(),
                    "fixture unexpectedly contains a symlink at {}",
                    path.display()
                );
                if metadata.is_dir() {
                    visit(root, &path, files);
                } else {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/");
                    files.insert(relative, fs::read(path).unwrap());
                }
            }
        }

        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    fn expected_failpoint_postimage() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            (".vela/work/session.json".to_string(), b"closed".to_vec()),
            (
                "frontier.json".to_string(),
                b"materialized frontier".to_vec(),
            ),
            ("keep.txt".to_string(), b"unchanged".to_vec()),
            ("records/authority.json".to_string(), b"authority".to_vec()),
            ("records/evidence.json".to_string(), b"evidence".to_vec()),
            (
                "records/review/pending.json".to_string(),
                b"pending".to_vec(),
            ),
        ])
    }

    fn assert_injected<T>(result: Result<T, FrontierTxnError>, expected: FrontierTxnStep) {
        match result {
            Err(FrontierTxnError::InjectedFailure { step }) => assert_eq!(step, expected),
            Err(error) => panic!("expected injected failure at {expected:?}, got {error}"),
            Ok(_) => panic!("failpoint {expected:?} was not reached"),
        }
    }

    fn assert_post_marker_recovery_is_exact(
        root: &Path,
        journals: &Path,
        operation_id: &OperationId,
    ) {
        assert!(matches!(
            FrontierTxn::recover(root, journals, operation_id).unwrap(),
            RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted
        ));
        assert_eq!(snapshot_files(root), expected_failpoint_postimage());
        let first_recovery = snapshot_files(root);
        assert_eq!(
            FrontierTxn::recover(root, journals, operation_id).unwrap(),
            RecoveryOutcome::AlreadyCompleted
        );
        assert_eq!(
            snapshot_files(root),
            first_recovery,
            "a completed recovery must be byte-idempotent"
        );
    }

    #[test]
    fn frontier_txn_rejects_unsafe_paths_and_symlink_ancestors() {
        for path in [
            "",
            "/absolute",
            "../escape",
            "a/../escape",
            "a//b",
            ".git/index",
            "safe\\unsafe",
            "glob/*.json",
            "records/quote\"",
            "records/less<than",
            "records/greater>than",
            "records/pipe|name",
            "records/trailing-space ",
            "records/trailing-dot.",
            "records/CON",
            "records/nul.json",
            "records/COM1.txt",
            "records/LPT¹.txt",
            "records/CLOCK$",
        ] {
            assert!(
                RepoPath::parse(path).is_err(),
                "accepted unsafe path {path}"
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("frontier");
            let outside = temp.path().join("outside");
            fs::create_dir_all(&root).unwrap();
            fs::create_dir_all(&outside).unwrap();
            symlink(&outside, root.join("linked")).unwrap();
            let error = DeltaDraft::prepare(
                &root,
                vec![PlannedWrite::write(
                    RepoPath::parse("linked/value.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"unsafe".to_vec(),
                )],
            )
            .unwrap_err();
            assert!(matches!(error, FrontierTxnError::UnsafeTarget { .. }));
        }
    }

    #[test]
    fn open_if_present_returns_none_only_for_an_absent_journal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let operation_id = OperationId::derive("submission", b"absent request");

        assert!(
            FrontierTxn::open_if_present(&root, &journals, &operation_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn open_if_present_exposes_request_identity_and_resumes_marker_window() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("review/pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"exact retry request");
        let operation_id = plan.operation_id.clone();
        let request_root = plan.request_root.clone();
        let result = plan.result.clone();
        let txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        let marker = CommitMarker::from_plan(txn.plan());
        operation_journal::write_json(&txn.paths.marker, &marker).unwrap();
        assert_eq!(txn.recovery_state(), &RecoveryState::Prepared);
        drop(txn);

        let mut reopened = FrontierTxn::open_if_present(&root, &journals, &operation_id)
            .unwrap()
            .expect("prepared journal");
        assert_eq!(reopened.plan().request_root, request_root);
        assert_eq!(reopened.plan().result, result);
        assert_eq!(reopened.recovery_state(), &RecoveryState::Prepared);
        reopened.mark_committed().unwrap();
        reopened.install().unwrap();
        assert_eq!(
            fs::read(root.join("review/pending.json")).unwrap(),
            b"pending"
        );
    }

    #[test]
    fn draft_and_journal_public_projections_are_identical() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("old.json"), b"old").unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse("z-event.json").unwrap(),
                    WriteClass::Authority,
                    b"event".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("a-receipt.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"receipt".to_vec(),
                ),
                PlannedWrite::delete(
                    RepoPath::parse("old.json").unwrap(),
                    WriteClass::PublicReview,
                ),
                PlannedWrite::write(
                    RepoPath::parse(".vela/work/session.json").unwrap(),
                    WriteClass::PrivateCoordination,
                    b"private".to_vec(),
                ),
            ],
        )
        .unwrap();
        let draft_projection = draft.resolved_public_writes().unwrap();
        assert_eq!(
            draft_projection
                .iter()
                .map(|write| write.staged.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a-receipt.json", "old.json", "z-event.json"]
        );
        let plan = fixture_plan(&root, &draft, b"projection equality");
        let txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();

        assert_eq!(txn.resolved_public_writes().unwrap(), draft_projection);
    }

    #[test]
    fn canonical_delta_is_sorted_unique_and_root_bound() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("existing.json"), b"before").unwrap();
        let writes = || {
            vec![
                PlannedWrite::write(
                    RepoPath::parse("z.json").unwrap(),
                    WriteClass::Derived,
                    b"z".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("existing.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"after".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("a.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"a".to_vec(),
                ),
            ]
        };
        let first = DeltaDraft::prepare(&root, writes()).unwrap();
        assert_eq!(
            first
                .delta
                .writes()
                .iter()
                .map(|write| write.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.json", "existing.json", "z.json"]
        );
        first.delta.verify().unwrap();

        fs::write(root.join("existing.json"), b"different preimage").unwrap();
        let second = DeltaDraft::prepare(&root, writes()).unwrap();
        assert_ne!(first.delta.root(), second.delta.root());

        let duplicate = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse("same.json").unwrap(),
                    WriteClass::Derived,
                    vec![1],
                ),
                PlannedWrite::write(
                    RepoPath::parse("same.json").unwrap(),
                    WriteClass::Authority,
                    vec![2],
                ),
            ],
        )
        .unwrap_err();
        assert!(matches!(duplicate, FrontierTxnError::DuplicatePath(_)));
    }

    #[test]
    fn resulting_event_ids_must_be_sorted_unique_and_are_plan_bound() {
        assert!(verify_resulting_event_ids(&["vev_a".to_string(), "vev_z".to_string()]).is_ok());
        assert!(verify_resulting_event_ids(&["vev_z".to_string(), "vev_a".to_string()]).is_err());
        assert!(verify_resulting_event_ids(&["vev_a".to_string(), "vev_a".to_string()]).is_err());

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(&root, vec![]).unwrap();
        let plan = fixture_plan(&root, &draft, b"event id commitment");
        let original_root = plan.root.clone();
        let mut changed = plan;
        changed.resulting_event_ids = vec!["vev_added".to_string()];
        assert_ne!(changed.compute_root().unwrap(), original_root);
    }

    #[test]
    fn pre_marker_failpoints_leave_zero_frontier_delta_and_retry_exactly() {
        let blob_count = 5;
        let mut prepare_failpoints = Vec::new();
        for index in 0..blob_count {
            prepare_failpoints.push(FrontierTxnStep::BeforeBlobJournalWrite { index });
            prepare_failpoints.push(FrontierTxnStep::AfterBlobJournalWrite { index });
        }
        prepare_failpoints.extend([
            FrontierTxnStep::BeforePreparedJournalWrite,
            FrontierTxnStep::AfterPreparedJournalWrite,
        ]);

        for step in prepare_failpoints {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("frontier");
            let journals = temp.path().join("journals");
            initialize_failpoint_frontier(&root);
            let before = snapshot_files(&root);
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            assert_eq!(draft.blobs.len(), blob_count);
            let plan = fixture_plan(&root, &draft, format!("prepare {step:?}").as_bytes());
            let operation_id = plan.operation_id.clone();
            let paths = FrontierTxnPaths::new(&journals, &operation_id);

            assert_injected(
                FrontierTxn::prepare_at_failpoint(&root, &journals, plan, draft, step),
                step,
            );

            assert_eq!(
                snapshot_files(&root),
                before,
                "pre-marker failpoint {step:?} changed the frontier"
            );
            assert!(
                !paths.marker.exists(),
                "pre-marker failpoint {step:?} wrote a commit marker"
            );

            // Partial private blob journals and a fully durable Prepared
            // journal are both safe to retry. Neither is canonical state.
            let retry_draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            let retry_plan =
                fixture_plan(&root, &retry_draft, format!("prepare {step:?}").as_bytes());
            let mut retry = if paths.plan.exists() {
                let retry = FrontierTxn::open(&root, &journals, &operation_id).unwrap();
                assert_eq!(
                    retry.plan().root(),
                    retry_plan.root(),
                    "a durable Prepared journal must bind the exact retry plan"
                );
                retry
            } else {
                FrontierTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap()
            };
            retry.mark_committed().unwrap();
            retry.install().unwrap();
            retry.complete().unwrap();
            drop(retry);
            assert_eq!(snapshot_files(&root), expected_failpoint_postimage());
        }

        // A safely injected marker-write error occurs before the atomic,
        // fsync-backed journal replacement. The old state is therefore a
        // complete Prepared journal with no marker and no frontier delta.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        initialize_failpoint_frontier(&root);
        let before = snapshot_files(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let plan = fixture_plan(&root, &draft, b"marker write failure");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        let step = FrontierTxnStep::BeforeCommitMarkerWrite;
        assert_injected(txn.mark_committed_at_failpoint(step), step);
        assert_eq!(snapshot_files(&root), before);
        assert!(!txn.paths.marker.exists());
        drop(txn);
        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Prepared
        );
        assert_eq!(snapshot_files(&root), before);
    }

    #[test]
    fn post_marker_failpoints_recover_the_exact_delta_idempotently() {
        let mut failpoints = vec![
            FrontierTxnStep::AfterCommitMarkerWrite,
            // This is the durable-marker/Prepared-journal window produced by
            // a committed-journal write failure.
            FrontierTxnStep::BeforeCommittedJournalWrite,
            FrontierTxnStep::AfterCommittedJournalWrite,
        ];
        for index in 0..failpoint_writes().len() {
            failpoints.extend([
                FrontierTxnStep::BeforeInstallWrite { index },
                FrontierTxnStep::AfterInstallWrite { index },
                FrontierTxnStep::BeforeInstallingJournalWrite { index },
                FrontierTxnStep::AfterInstallingJournalWrite { index },
            ]);
        }
        failpoints.extend([
            FrontierTxnStep::BeforeInstalledJournalWrite,
            FrontierTxnStep::AfterInstalledJournalWrite,
            FrontierTxnStep::BeforeInstalledStateVerification,
            FrontierTxnStep::AfterInstalledStateVerification,
            FrontierTxnStep::BeforeCompletedJournalWrite,
            FrontierTxnStep::AfterCompletedJournalWrite,
        ]);

        for step in failpoints {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("frontier");
            let journals = temp.path().join("journals");
            initialize_failpoint_frontier(&root);
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            assert!(
                draft
                    .delta
                    .writes()
                    .iter()
                    .any(|write| write.class == WriteClass::Derived),
                "fixture must exercise materialized-view installation"
            );
            let plan = fixture_plan(&root, &draft, format!("post marker {step:?}").as_bytes());
            let operation_id = plan.operation_id.clone();
            let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();

            let result = match step {
                FrontierTxnStep::AfterCommitMarkerWrite
                | FrontierTxnStep::BeforeCommittedJournalWrite
                | FrontierTxnStep::AfterCommittedJournalWrite => {
                    txn.mark_committed_at_failpoint(step)
                }
                FrontierTxnStep::BeforeInstallWrite { .. }
                | FrontierTxnStep::AfterInstallWrite { .. }
                | FrontierTxnStep::BeforeInstallingJournalWrite { .. }
                | FrontierTxnStep::AfterInstallingJournalWrite { .. }
                | FrontierTxnStep::BeforeInstalledJournalWrite
                | FrontierTxnStep::AfterInstalledJournalWrite => {
                    txn.mark_committed().unwrap();
                    txn.install_at_failpoint(step)
                }
                FrontierTxnStep::BeforeInstalledStateVerification
                | FrontierTxnStep::AfterInstalledStateVerification
                | FrontierTxnStep::BeforeCompletedJournalWrite
                | FrontierTxnStep::AfterCompletedJournalWrite => {
                    txn.mark_committed().unwrap();
                    txn.install().unwrap();
                    txn.complete_at_failpoint(step)
                }
                _ => unreachable!("not a post-marker failpoint: {step:?}"),
            };
            assert_injected(result, step);
            assert!(
                txn.paths.marker.exists(),
                "post-marker failpoint {step:?} lost the commit marker"
            );
            drop(txn);

            assert_post_marker_recovery_is_exact(&root, &journals, &operation_id);
        }
    }

    #[test]
    fn committed_install_is_idempotent_and_recovers_after_failpoint() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse("records/receipt.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"receipt".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("frontier.json").unwrap(),
                    WriteClass::Derived,
                    b"frontier".to_vec(),
                ),
            ],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"recoverable request");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        let step = FrontierTxnStep::AfterInstallingJournalWrite { index: 0 };
        let error = txn.install_at_failpoint(step).unwrap_err();
        assert!(matches!(
            error,
            FrontierTxnError::InjectedFailure { step: actual } if actual == step
        ));
        drop(txn);

        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        let reopened = FrontierTxn::open(&root, &journals, &operation_id).unwrap();
        assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
        drop(reopened);
        assert_eq!(
            fs::read(root.join("records/receipt.json")).unwrap(),
            b"receipt"
        );
        assert_eq!(fs::read(root.join("frontier.json")).unwrap(), b"frontier");
        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::AlreadyCompleted,
            "replaying a completed transaction must remain idempotent"
        );
    }

    #[test]
    fn incomplete_journal_is_a_frontier_wide_recovery_barrier() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let first_draft = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse("records/first.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"first".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("frontier.json").unwrap(),
                    WriteClass::Derived,
                    b"first frontier".to_vec(),
                ),
            ],
        )
        .unwrap();
        let first_plan = fixture_plan(&root, &first_draft, b"first operation");
        let first_operation = first_plan.operation_id.clone();
        let mut first = FrontierTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
        first.mark_committed().unwrap();
        let step = FrontierTxnStep::AfterInstallingJournalWrite { index: 0 };
        assert!(matches!(
            first.install_at_failpoint(step),
            Err(FrontierTxnError::InjectedFailure { step: actual }) if actual == step
        ));
        drop(first);

        let barrier_error = FrontierTxn::acquire_recovery_barrier(&root, &journals).unwrap_err();
        assert!(matches!(
            barrier_error,
            FrontierTxnError::RecoveryRequired {
                operation_id,
                state: RecoveryState::Installing {
                    installed: 1,
                    total: 2
                }
            } if operation_id == first_operation.as_str()
        ));

        let second_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/second.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"second".to_vec(),
            )],
        )
        .unwrap();
        let second_plan = fixture_plan(&root, &second_draft, b"second operation");
        assert!(matches!(
            FrontierTxn::prepare(&root, &journals, second_plan, second_draft),
            Err(FrontierTxnError::RecoveryRequired { operation_id, .. })
                if operation_id == first_operation.as_str()
        ));

        assert_eq!(
            FrontierTxn::recover(&root, &journals, &first_operation).unwrap(),
            RecoveryOutcome::Completed
        );
        let barrier = FrontierTxn::acquire_recovery_barrier(&root, &journals).unwrap();
        drop(barrier);
    }

    #[test]
    fn completed_journal_fails_closed_when_a_postimage_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"completed corruption");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        drop(txn);

        fs::remove_file(root.join("records/receipt.json")).unwrap();
        assert!(matches!(
            FrontierTxn::open_if_present(&root, &journals, &operation_id),
            Err(FrontierTxnError::CompletedPostimageMismatch {
                operation_id: corrupt_operation,
                ..
            }) if corrupt_operation == operation_id.as_str()
        ));
        assert!(matches!(
            FrontierTxn::acquire_recovery_barrier(&root, &journals),
            Err(FrontierTxnError::CompletedPostimageMismatch { .. })
        ));
    }

    #[test]
    fn historical_completed_journal_survives_a_later_event_that_sorts_before_it() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela/events")).unwrap();
        let first_candidate = fixture_event("sort candidate one");
        let second_candidate = fixture_event("sort candidate two");
        let (initial_event, later_event) = if first_candidate.id > second_candidate.id {
            (first_candidate, second_candidate)
        } else {
            (second_candidate, first_candidate)
        };
        assert!(later_event.id < initial_event.id);
        fs::write(
            root.join(format!(".vela/events/{}.json", initial_event.id)),
            serde_json::to_vec_pretty(&initial_event).unwrap(),
        )
        .unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("frontier.json").unwrap(),
                WriteClass::Derived,
                b"transaction head".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"completed event root");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        drop(txn);

        fs::write(
            root.join(format!(".vela/events/{}.json", later_event.id)),
            serde_json::to_vec_pretty(&later_event).unwrap(),
        )
        .unwrap();
        fs::write(root.join("frontier.json"), b"legitimate later head").unwrap();

        let reopened = FrontierTxn::open_if_present(&root, &journals, &operation_id)
            .unwrap()
            .expect("historical completed journal");
        assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
        drop(reopened);
        drop(FrontierTxn::acquire_recovery_barrier(&root, &journals).unwrap());
    }

    #[test]
    fn event_neutral_completed_history_proves_superseded_postimages() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();

        let first_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("frontier.json").unwrap(),
                WriteClass::Derived,
                b"first head".to_vec(),
            )],
        )
        .unwrap();
        let first_plan = fixture_plan(&root, &first_draft, b"first neutral operation");
        let first_operation = first_plan.operation_id.clone();
        let mut first = FrontierTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
        first.mark_committed().unwrap();
        first.install().unwrap();
        first.complete().unwrap();
        drop(first);

        let second_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("frontier.json").unwrap(),
                WriteClass::Derived,
                b"second head".to_vec(),
            )],
        )
        .unwrap();
        let second_plan = fixture_plan(&root, &second_draft, b"second neutral operation");
        let mut second = FrontierTxn::prepare(&root, &journals, second_plan, second_draft).unwrap();
        second.mark_committed().unwrap();
        second.install().unwrap();
        second.complete().unwrap();
        drop(second);

        let first_retry = FrontierTxn::open(&root, &journals, &first_operation).unwrap();
        assert_eq!(first_retry.recovery_state(), &RecoveryState::Completed);
        drop(first_retry);
        drop(FrontierTxn::acquire_recovery_barrier(&root, &journals).unwrap());
    }

    #[test]
    fn completed_history_rejects_corrupt_marker_and_blob() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"corrupt durable history");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        let marker_path = txn.paths.marker.clone();
        let blob_path = txn.paths.blob(
            &txn.plan()
                .canonical_delta
                .writes()
                .first()
                .unwrap()
                .payload
                .as_ref()
                .unwrap()
                .digest,
        );
        drop(txn);

        let marker_bytes = fs::read(&marker_path).unwrap();
        fs::write(&marker_path, b"{}").unwrap();
        assert!(FrontierTxn::open(&root, &journals, &operation_id).is_err());
        fs::write(&marker_path, marker_bytes).unwrap();

        let blob_bytes = fs::read(&blob_path).unwrap();
        let mut corrupt_blob: serde_json::Value = serde_json::from_slice(&blob_bytes).unwrap();
        corrupt_blob["bytes"] = json!([0, 1, 2, 3]);
        fs::write(
            &blob_path,
            serde_json::to_vec_pretty(&corrupt_blob).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            FrontierTxn::open(&root, &journals, &operation_id),
            Err(FrontierTxnError::CorruptBlob(_))
        ));
    }

    #[test]
    fn commit_marker_rejects_the_wrong_resulting_event_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"wrong resulting root");
        plan.resulting_event_log_root = ContentDigest::hash(b"not the event log");
        plan.root = plan.compute_root().unwrap();
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(FrontierTxnError::CorruptPlan(message))
                if message.contains("resulting event-log commitment")
        ));
        assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
        let paths = FrontierTxnPaths::new(&journals, &operation_id);
        assert!(!paths.marker.exists());
        assert!(!root.join("records/receipt.json").exists());
        drop(txn);

        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Aborted
        );

        let retry_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"replacement receipt".to_vec(),
            )],
        )
        .unwrap();
        let retry_plan = fixture_plan(&root, &retry_draft, b"valid transaction after bad root");
        let mut retry = FrontierTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        assert_eq!(
            fs::read(root.join("records/receipt.json")).unwrap(),
            b"replacement receipt"
        );
    }

    #[test]
    fn commit_marker_rejects_resulting_event_ids_that_do_not_match_the_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"wrong resulting event ids");
        plan.resulting_event_ids = vec!["vev_missing".to_string()];
        plan.root = plan.compute_root().unwrap();
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(FrontierTxnError::CorruptPlan(message))
                if message.contains("resulting event-log commitment")
        ));
        assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
        assert!(
            !FrontierTxnPaths::new(&journals, &operation_id)
                .marker
                .exists()
        );
        assert!(!root.join("records/receipt.json").exists());
    }

    #[test]
    fn commit_marker_accepts_the_event_set_derived_from_staged_postimages() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let event = fixture_event("staged event");
        let event_bytes = serde_json::to_vec_pretty(&event).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse(format!(".vela/events/{}.json", event.id)).unwrap(),
                WriteClass::Authority,
                event_bytes,
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"valid staged event result");
        plan.resulting_event_ids = vec![event.id.clone()];
        plan.resulting_event_log_root = event_log_root(&[event]).unwrap();
        plan.root = plan.compute_root().unwrap();

        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        assert_eq!(txn.recovery_state(), &RecoveryState::Completed);
    }

    #[test]
    fn committed_install_never_overwrites_post_marker_drift() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("state.json"), b"before").unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("state.json").unwrap(),
                WriteClass::Derived,
                b"after".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"conflicting request");
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        fs::write(root.join("state.json"), b"third party drift").unwrap();

        let error = txn.install().unwrap_err();

        assert!(matches!(error, FrontierTxnError::CommittedConflict { .. }));
        assert_eq!(
            fs::read(root.join("state.json")).unwrap(),
            b"third party drift"
        );
    }

    #[cfg(unix)]
    #[test]
    fn observed_parent_symlink_swap_after_marker_never_writes_outside_the_frontier() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/receipt.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"receipt".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"observed parent symlink swap");
        let operation_id = plan.operation_id.clone();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();

        symlink(&outside, root.join("records")).unwrap();
        assert!(matches!(
            txn.install(),
            Err(FrontierTxnError::UnsafeTarget { .. })
        ));
        assert!(
            !outside.join("receipt.json").exists(),
            "an observed stable symlink substitution must not redirect the write"
        );
        drop(txn);

        // This proves rejection when the substitution is visible to a path
        // check. It intentionally does not claim to eliminate a concurrent
        // hostile-local race between that check and a std::fs path operation;
        // `validate_target` documents that remaining permission boundary.
        fs::remove_file(root.join("records")).unwrap();
        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        assert_eq!(
            fs::read(root.join("records/receipt.json")).unwrap(),
            b"receipt"
        );
    }

    #[test]
    fn recovery_before_marker_has_zero_frontier_delta() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"prepared only");
        let operation_id = plan.operation_id.clone();
        let txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        drop(txn);

        assert_eq!(
            FrontierTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Prepared
        );
        assert!(!root.join("pending.json").exists());
    }

    #[test]
    fn path_bound_file_snapshot_commits_supplied_bytes_without_rereading() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        fs::create_dir_all(root.join(".vela/policies")).unwrap();
        let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();

        let snapshot =
            InputBinding::file_snapshot(policy_path.clone(), Some(b"loaded policy")).unwrap();
        fs::write(root.join(policy_path.as_str()), b"loaded policy").unwrap();
        snapshot.verify_current(&root).unwrap();

        fs::write(root.join(policy_path.as_str()), b"rotated policy").unwrap();
        assert!(matches!(
            snapshot.verify_current(&root),
            Err(FrontierTxnError::StaleInput { path, .. }) if path == policy_path
        ));

        let signature_path = RepoPath::parse(".vela/policies/active.sig.json").unwrap();
        let absent = InputBinding::file_snapshot(signature_path.clone(), None).unwrap();
        absent.verify_current(&root).unwrap();
        fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
        assert!(matches!(
            absent.verify_current(&root),
            Err(FrontierTxnError::StaleInput { path, .. }) if path == signature_path
        ));
    }

    #[test]
    fn path_bound_existing_input_drift_refuses_the_commit_marker() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela/policies")).unwrap();
        let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();
        fs::write(root.join(policy_path.as_str()), b"policy before").unwrap();
        let policy_input = InputBinding::existing_file(&root, policy_path.clone()).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"policy input drift");
        plan.read_set.push(policy_input);
        plan.read_set
            .sort_by(|left, right| left.name.cmp(&right.name));
        plan.root = plan.compute_root().unwrap();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::write(root.join(policy_path.as_str()), b"policy after").unwrap();
        let error = txn.mark_committed().unwrap_err();

        assert!(matches!(
            error,
            FrontierTxnError::StaleInput { path, .. } if path == policy_path
        ));
        assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
        assert!(!txn.paths.marker.exists());
        assert!(!root.join("pending.json").exists());
        drop(txn);
        assert_eq!(
            FrontierTxn::recover(
                &root,
                &journals,
                &OperationId::derive("submission", b"policy input drift")
            )
            .unwrap(),
            RecoveryOutcome::Aborted
        );

        // The aborted journal is terminal and does not block an unrelated
        // operation from planning and completing.
        let unrelated_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("unrelated.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"unrelated".to_vec(),
            )],
        )
        .unwrap();
        let unrelated_plan = fixture_plan(&root, &unrelated_draft, b"unrelated after policy abort");
        let mut unrelated =
            FrontierTxn::prepare(&root, &journals, unrelated_plan, unrelated_draft).unwrap();
        unrelated.mark_committed().unwrap();
        unrelated.install().unwrap();
        unrelated.complete().unwrap();
        drop(unrelated);

        // The same normalized request may also replan against the policy bytes
        // that are current now, replacing its marker-free aborted plan.
        let retry_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let mut retry_plan = fixture_plan(&root, &retry_draft, b"policy input drift");
        retry_plan
            .read_set
            .push(InputBinding::existing_file(&root, policy_path.clone()).unwrap());
        retry_plan
            .read_set
            .sort_by(|left, right| left.name.cmp(&right.name));
        retry_plan.root = retry_plan.compute_root().unwrap();
        let mut retry = FrontierTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        assert_eq!(retry.recovery_state(), &RecoveryState::Completed);
    }

    #[test]
    fn path_bound_absent_input_creation_refuses_the_commit_marker() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela/policies")).unwrap();
        let signature_path = RepoPath::parse(".vela/policies/active.sig.json").unwrap();
        let signature_input = InputBinding::absent_file(&root, signature_path.clone()).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"absent policy input drift");
        plan.read_set.push(signature_input);
        plan.read_set
            .sort_by(|left, right| left.name.cmp(&right.name));
        plan.root = plan.compute_root().unwrap();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(FrontierTxnError::StaleInput { path, .. }) if path == signature_path
        ));
        assert!(!txn.paths.marker.exists());
        assert!(!root.join("pending.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn path_bound_input_rejects_a_symlink_swap_before_the_marker() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela/policies")).unwrap();
        let outside = temp.path().join("outside-policy.json");
        fs::write(&outside, b"outside").unwrap();
        let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();
        let policy_target = root.join(policy_path.as_str());
        fs::write(&policy_target, b"policy before").unwrap();
        let policy_input = InputBinding::existing_file(&root, policy_path).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let mut plan = fixture_plan(&root, &draft, b"policy symlink swap");
        plan.read_set.push(policy_input);
        plan.read_set
            .sort_by(|left, right| left.name.cmp(&right.name));
        plan.root = plan.compute_root().unwrap();
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::remove_file(&policy_target).unwrap();
        symlink(&outside, &policy_target).unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(FrontierTxnError::UnsafeTarget { .. })
        ));
        assert!(!txn.paths.marker.exists());
    }

    #[test]
    fn stale_event_root_before_marker_has_zero_frontier_delta() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("frontier");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela/events")).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"stale event root");
        let mut txn = FrontierTxn::prepare(&root, &journals, plan, draft).unwrap();
        let event = fixture_event("simulate a legacy concurrent writer");
        fs::write(
            root.join(format!(".vela/events/{}.json", event.id)),
            serde_json::to_vec_pretty(&event).unwrap(),
        )
        .unwrap();

        let error = txn.mark_committed().unwrap_err();

        assert!(matches!(error, FrontierTxnError::StaleEventLog { .. }));
        assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
        assert!(!txn.paths.marker.exists());
        assert!(!root.join("pending.json").exists());
        drop(txn);

        let retry_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("pending.json").unwrap(),
                WriteClass::PublicReview,
                b"pending".to_vec(),
            )],
        )
        .unwrap();
        let retry_plan = fixture_plan(&root, &retry_draft, b"stale event root");
        let mut retry = FrontierTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        assert_eq!(retry.recovery_state(), &RecoveryState::Completed);
    }
}
