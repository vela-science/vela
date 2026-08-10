//! Recoverable, path-bound repository filesystem transactions.
//!
//! This is private durability plumbing, not a protocol object. A caller first
//! builds a pure [`CanonicalDelta`], then persists its plan and postimage blobs,
//! writes a durable commit marker, and finally installs the exact bytes. Once a
//! marker exists recovery only replays the journal; it never re-runs policy,
//! verification, clocks, or key-bearing code.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};
use unicode_normalization::UnicodeNormalization;
use vela_edge::repository_write::load_authority_trust_anchor_from_home;

use crate::operation_journal;

pub(crate) const REPOSITORY_TXN_SCHEMA: &str = "vela.repository-txn.internal.v2";
const REPOSITORY_TXN_BLOB_SCHEMA: &str = "vela.repository-txn-blob.internal.v1";
const REPOSITORY_TXN_MARKER_SCHEMA: &str = "vela.repository-txn-marker.internal.v1";
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

    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(RepositoryTxnError::InvalidDigest(value));
        };
        if !vela_protocol::is_lower_hex_64(hex) {
            return Err(RepositoryTxnError::InvalidDigest(value));
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

    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("vop_") else {
            return Err(RepositoryTxnError::InvalidOperationId(value));
        };
        if !vela_protocol::is_lower_hex_64(hex) {
            return Err(RepositoryTxnError::InvalidOperationId(value));
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

impl RepoPath {
    pub(crate) fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\0')
            || value.contains('\\')
            || value.contains("//")
        {
            return Err(RepositoryTxnError::InvalidPath {
                path: value,
                reason: "path must be a non-empty normalized relative path".to_string(),
            });
        }
        if value.nfc().ne(value.chars()) {
            return Err(RepositoryTxnError::InvalidPath {
                path: value,
                reason: "path must already be Unicode NFC".to_string(),
            });
        }
        for segment in value.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(RepositoryTxnError::InvalidPath {
                    path: value,
                    reason: "dot and empty path components are forbidden".to_string(),
                });
            }
            if segment.eq_ignore_ascii_case(".git") {
                return Err(RepositoryTxnError::InvalidPath {
                    path: value,
                    reason: ".git is outside the repository write boundary".to_string(),
                });
            }
            if segment
                .chars()
                .any(|character| character.is_control() || "*?[]{}:\"<>|".contains(character))
            {
                return Err(RepositoryTxnError::InvalidPath {
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
            return Err(RepositoryTxnError::InvalidPath {
                path: value,
                reason: "path is not lexically relative and normalized".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn target(&self, root: &Path) -> Result<PathBuf, RepositoryTxnError> {
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
    PrivateCoordination,
}

impl WriteClass {
    fn install_order(self) -> u8 {
        match self {
            Self::CanonicalEvidence => 10,
            Self::PublicReview => 20,
            Self::Authority => 30,
            Self::PrivateCoordination => 40,
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
    fn new(mut writes: Vec<StagedWrite>) -> Result<Self, RepositoryTxnError> {
        writes.sort_by(|left, right| {
            left.class
                .install_order()
                .cmp(&right.class.install_order())
                .then_with(|| left.path.cmp(&right.path))
        });
        let mut paths = BTreeSet::new();
        let mut portable_paths = BTreeMap::new();
        for write in &writes {
            if !paths.insert(write.path.clone()) {
                return Err(RepositoryTxnError::DuplicatePath(
                    write.path.as_str().to_string(),
                ));
            }
            let portable_key = write
                .path
                .as_str()
                .nfc()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if let Some(previous) = portable_paths.insert(portable_key, write.path.clone()) {
                return Err(RepositoryTxnError::PortablePathCollision {
                    first: previous.as_str().to_string(),
                    second: write.path.as_str().to_string(),
                });
            }
        }
        let root = Self::compute_root(&writes)?;
        Ok(Self {
            schema: CANONICAL_DELTA_SCHEMA.to_string(),
            root,
            writes,
        })
    }

    fn compute_root(writes: &[StagedWrite]) -> Result<ContentDigest, RepositoryTxnError> {
        let bytes = vela_protocol::canonical::to_canonical_bytes(&DeltaCommitment {
            schema: CANONICAL_DELTA_SCHEMA,
            writes,
        })
        .map_err(RepositoryTxnError::Canonicalize)?;
        Ok(ContentDigest::hash(bytes))
    }

    pub(crate) fn verify(&self) -> Result<(), RepositoryTxnError> {
        if self.schema != CANONICAL_DELTA_SCHEMA {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "unexpected canonical delta schema {}",
                self.schema
            )));
        }
        let normalized = Self::new(self.writes.clone())?;
        if normalized.writes != self.writes || normalized.root != self.root {
            return Err(RepositoryTxnError::CorruptPlan(
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

    pub(crate) fn delete(path: RepoPath, class: WriteClass) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::Absent,
        }
    }

    /// Consume one already-bounded planned write for inclusion in an Era-1
    /// repository-authority object delta.
    ///
    /// Authority transactions own regular canonical/public object bytes.
    /// Executable modes are intentionally rejected rather than silently
    /// weakening the signed object commitment.
    pub(crate) fn into_authority_object_parts(
        self,
    ) -> Result<(String, WriteClass, Option<Vec<u8>>), RepositoryTxnError> {
        let postimage = match self.postimage {
            PlannedPostimage::Absent => None,
            PlannedPostimage::File { bytes, mode } => {
                if mode.is_some_and(|mode| mode != FileMode::Regular) {
                    return Err(RepositoryTxnError::CorruptPlan(format!(
                        "authority object {} cannot install an executable mode",
                        self.path.as_str()
                    )));
                }
                Some(bytes)
            }
        };
        Ok((self.path.as_str().to_string(), self.class, postimage))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DeltaDraft {
    pub(crate) delta: CanonicalDelta,
    blobs: BTreeMap<ContentDigest, Vec<u8>>,
}

impl DeltaDraft {
    pub(crate) fn prepare(
        repository_root: &Path,
        writes: Vec<PlannedWrite>,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let mut seen = BTreeSet::new();
        let mut staged = Vec::new();
        let mut blobs = BTreeMap::new();
        for write in writes {
            if !seen.insert(write.path.clone()) {
                return Err(RepositoryTxnError::DuplicatePath(
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RepositoryBinding {
    canonical_root: String,
    repository_id: String,
}

impl RepositoryBinding {
    pub(crate) fn new(
        repository_root: &Path,
        repository_id: impl Into<String>,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let repository_id = repository_id.into();
        if repository_id.trim().is_empty() {
            return Err(RepositoryTxnError::CorruptPlan(
                "repository binding has an empty repository id".to_string(),
            ));
        }
        Ok(Self {
            canonical_root: root.to_string_lossy().into_owned(),
            repository_id,
        })
    }

    fn verify_root(&self, repository_root: &Path) -> Result<PathBuf, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        if root.as_os_str() != self.canonical_root.as_str() {
            return Err(RepositoryTxnError::RepositoryBindingMismatch {
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
    ProposalWithdrawal,
    Verification,
    Decision,
    Maintenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InputBinding {
    pub(crate) name: String,
    pub(crate) digest: ContentDigest,
}

const REPOSITORY_FILE_INPUT_PREFIX: &str = "repository_file:";
const REPOSITORY_FILE_INPUT_SCHEMA: &str = "vela.repository-file-input.internal.v1";
const REPOSITORY_DIRECTORY_INPUT_PREFIX: &str = "repository_directory:";
const REPOSITORY_DIRECTORY_INPUT_SCHEMA: &str = "vela.repository-directory-input.internal.v1";

#[derive(Serialize)]
struct RepositoryFileInputCommitment<'a> {
    schema: &'a str,
    path: &'a RepoPath,
    state: &'a FileState,
}

#[derive(Serialize)]
struct RepositoryDirectoryInputCommitment<'a> {
    schema: &'a str,
    path: &'a RepoPath,
    state: &'a RepositoryDirectoryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RepositoryDirectoryState {
    Absent,
    Directory { entries: Vec<DirectoryEntryState> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DirectoryEntryState {
    path: RepoPath,
    state: FileState,
}

impl InputBinding {
    /// Bind the exact current direct membership and file states of a
    /// repository directory.
    ///
    /// This is used when the caller must preserve an append-only store whose
    /// historical members are discovered from the held repository rather
    /// than supplied as a parsed object list.
    pub(crate) fn current_directory(
        repository_root: &Path,
        path: RepoPath,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let state = inspect_directory_state(&root, &path)?;
        Self::from_directory_state(path, state)
    }

    /// Bind either the exact current file state or its exact absence.
    ///
    /// This is the read-set counterpart to a bounded caller read: it does not
    /// assume that a decision-critical receipt or policy path exists, and the
    /// marker check rejects creation, deletion, byte drift, or mode drift.
    pub(crate) fn current_file(
        repository_root: &Path,
        path: RepoPath,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let state = inspect_file_state(&root, &path)?;
        Self::from_repository_state(path, state)
    }

    /// Bind a regular repository file as a mutable planning input. The path tag
    /// is encoded in the existing `name` field so old digest-only journal
    /// records remain wire-compatible and continue to deserialize unchanged.
    #[cfg(test)]
    pub(crate) fn existing_file(
        repository_root: &Path,
        path: RepoPath,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let state = inspect_file_state(&root, &path)?;
        if matches!(state, FileState::Absent) {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "cannot bind missing repository input {} as an existing file",
                path.as_str()
            )));
        }
        Self::from_repository_state(path, state)
    }

    /// Bind the absence of a relative repository file. Creation of that file
    /// before the commit marker is therefore stale input, not a policy result
    /// that can be committed under changed authority bytes.
    pub(crate) fn absent_file(
        repository_root: &Path,
        path: RepoPath,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let state = inspect_file_state(&root, &path)?;
        if !matches!(state, FileState::Absent) {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "cannot bind present repository input {} as absent",
                path.as_str()
            )));
        }
        Self::from_repository_state(path, state)
    }

    /// Bind the exact bytes already loaded by a caller without reading the
    /// path a second time. Marker-time verification still inspects the path,
    /// so any drift between that snapshot and commit fails closed.
    #[cfg(test)]
    pub(crate) fn file_snapshot(
        path: RepoPath,
        bytes: Option<&[u8]>,
    ) -> Result<Self, RepositoryTxnError> {
        let state = match bytes {
            Some(bytes) => FileState::File {
                digest: ContentDigest::hash(bytes),
                size: bytes.len() as u64,
                mode: FileMode::Regular,
            },
            None => FileState::Absent,
        };
        Self::from_repository_state(path, state)
    }

    /// Bind a regular file to exact caller-supplied bytes and regular mode.
    ///
    /// This closes the gap between a parsed, caller-supplied history object
    /// and the canonical bytes actually present in the held repository.
    pub(crate) fn exact_file(
        repository_root: &Path,
        path: RepoPath,
        bytes: &[u8],
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let expected = FileState::File {
            digest: ContentDigest::hash(bytes),
            size: bytes.len() as u64,
            mode: FileMode::Regular,
        };
        let actual = inspect_file_state(&root, &path)?;
        if actual != expected {
            return Err(RepositoryTxnError::StaleInput {
                name: format!("{REPOSITORY_FILE_INPUT_PREFIX}{}", path.as_str()),
                path: path.clone(),
                expected: repository_file_input_digest(&path, &expected)?,
                actual: repository_file_input_digest(&path, &actual)?,
            });
        }
        Self::from_repository_state(path, expected)
    }

    /// Bind a directory only if its direct membership equals the supplied
    /// canonical path set.
    pub(crate) fn exact_directory(
        repository_root: &Path,
        path: RepoPath,
        expected_paths: &[RepoPath],
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let state = inspect_directory_state(&root, &path)?;
        let mut expected = expected_paths.to_vec();
        expected.sort();
        if expected.windows(2).any(|pair| pair[0] == pair[1])
            || expected.iter().any(|entry| {
                entry
                    .as_str()
                    .strip_prefix(path.as_str())
                    .and_then(|suffix| suffix.strip_prefix('/'))
                    .is_none_or(|suffix| suffix.is_empty() || suffix.contains('/'))
            })
        {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "expected membership for {} is not a sorted set of direct child paths",
                path.as_str()
            )));
        }
        let actual = match &state {
            RepositoryDirectoryState::Absent => Vec::new(),
            RepositoryDirectoryState::Directory { entries } => {
                entries.iter().map(|entry| entry.path.clone()).collect()
            }
        };
        if actual != expected {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "directory {} membership differs from the verified repository history",
                path.as_str()
            )));
        }
        Self::from_directory_state(path, state)
    }

    fn from_repository_state(path: RepoPath, state: FileState) -> Result<Self, RepositoryTxnError> {
        Ok(Self {
            name: format!("{REPOSITORY_FILE_INPUT_PREFIX}{}", path.as_str()),
            digest: repository_file_input_digest(&path, &state)?,
        })
    }

    fn from_directory_state(
        path: RepoPath,
        state: RepositoryDirectoryState,
    ) -> Result<Self, RepositoryTxnError> {
        Ok(Self {
            name: format!("{REPOSITORY_DIRECTORY_INPUT_PREFIX}{}", path.as_str()),
            digest: repository_directory_input_digest(&path, &state)?,
        })
    }

    fn repository_path(&self) -> Result<Option<RepoPath>, RepositoryTxnError> {
        let Some(path) = self.name.strip_prefix(REPOSITORY_FILE_INPUT_PREFIX) else {
            return Ok(None);
        };
        RepoPath::parse(path.to_string()).map(Some)
    }

    fn repository_directory_path(&self) -> Result<Option<RepoPath>, RepositoryTxnError> {
        let Some(path) = self.name.strip_prefix(REPOSITORY_DIRECTORY_INPUT_PREFIX) else {
            return Ok(None);
        };
        RepoPath::parse(path.to_string()).map(Some)
    }

    fn verify_shape(&self) -> Result<(), RepositoryTxnError> {
        if self.name.trim().is_empty() {
            return Err(RepositoryTxnError::CorruptPlan(
                "repository transaction input has an empty name".to_string(),
            ));
        }
        ContentDigest::parse(self.digest.as_str().to_string())?;
        self.repository_path()?;
        self.repository_directory_path()?;
        Ok(())
    }

    fn verify_current(&self, root: &Path) -> Result<(), RepositoryTxnError> {
        if let Some(path) = self.repository_directory_path()? {
            let state = inspect_directory_state(root, &path)?;
            let actual = repository_directory_input_digest(&path, &state)?;
            if actual != self.digest {
                return Err(RepositoryTxnError::StaleSnapshot {
                    name: self.name.clone(),
                    expected: self.digest.clone(),
                    actual,
                });
            }
            return Ok(());
        }
        let Some(path) = self.repository_path()? else {
            return Ok(());
        };
        let state = inspect_file_state(root, &path)?;
        let actual = repository_file_input_digest(&path, &state)?;
        if actual != self.digest {
            return Err(RepositoryTxnError::StaleInput {
                name: self.name.clone(),
                path,
                expected: self.digest.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn repository_file_input_digest(
    path: &RepoPath,
    state: &FileState,
) -> Result<ContentDigest, RepositoryTxnError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(&RepositoryFileInputCommitment {
        schema: REPOSITORY_FILE_INPUT_SCHEMA,
        path,
        state,
    })
    .map_err(RepositoryTxnError::Canonicalize)?;
    Ok(ContentDigest::hash(bytes))
}

fn repository_directory_input_digest(
    path: &RepoPath,
    state: &RepositoryDirectoryState,
) -> Result<ContentDigest, RepositoryTxnError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(&RepositoryDirectoryInputCommitment {
        schema: REPOSITORY_DIRECTORY_INPUT_SCHEMA,
        path,
        state,
    })
    .map_err(RepositoryTxnError::Canonicalize)?;
    Ok(ContentDigest::hash(bytes))
}

#[derive(Debug, Clone)]
pub(crate) struct RepositoryTxnPlanSpec {
    pub(crate) kind: OperationKind,
    pub(crate) operation_id: OperationId,
    pub(crate) request_root: ContentDigest,
    pub(crate) repository: RepositoryBinding,
    pub(crate) fixed_time: String,
    pub(crate) read_set: Vec<InputBinding>,
    pub(crate) result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryTxnPlan {
    schema: String,
    root: ContentDigest,
    pub(crate) kind: OperationKind,
    pub(crate) operation_id: OperationId,
    pub(crate) request_root: ContentDigest,
    pub(crate) repository: RepositoryBinding,
    pub(crate) fixed_time: String,
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
    repository: &'a RepositoryBinding,
    fixed_time: &'a str,
    read_set: &'a [InputBinding],
    canonical_delta: &'a CanonicalDelta,
    result: &'a serde_json::Value,
}

impl RepositoryTxnPlan {
    pub(crate) fn new(
        spec: RepositoryTxnPlanSpec,
        canonical_delta: CanonicalDelta,
    ) -> Result<Self, RepositoryTxnError> {
        canonical_delta.verify()?;
        OperationId::parse(spec.operation_id.as_str())?;
        let mut plan = Self {
            schema: REPOSITORY_TXN_SCHEMA.to_string(),
            root: ContentDigest::hash([]),
            kind: spec.kind,
            operation_id: spec.operation_id,
            request_root: spec.request_root,
            repository: spec.repository,
            fixed_time: spec.fixed_time,
            read_set: spec.read_set,
            canonical_delta,
            result: spec.result,
        };
        plan.root = plan.compute_root()?;
        Ok(plan)
    }

    fn compute_root(&self) -> Result<ContentDigest, RepositoryTxnError> {
        let bytes = vela_protocol::canonical::to_canonical_bytes(&PlanCommitment {
            schema: REPOSITORY_TXN_SCHEMA,
            kind: &self.kind,
            operation_id: &self.operation_id,
            request_root: &self.request_root,
            repository: &self.repository,
            fixed_time: &self.fixed_time,
            read_set: &self.read_set,
            canonical_delta: &self.canonical_delta,
            result: &self.result,
        })
        .map_err(RepositoryTxnError::Canonicalize)?;
        Ok(ContentDigest::hash(bytes))
    }

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        if self.schema != REPOSITORY_TXN_SCHEMA {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "unexpected repository transaction schema {}",
                self.schema
            )));
        }
        OperationId::parse(self.operation_id.as_str())?;
        for input in &self.read_set {
            input.verify_shape()?;
        }
        self.canonical_delta.verify()?;
        if self.compute_root()? != self.root {
            return Err(RepositoryTxnError::CorruptPlan(
                "repository transaction plan root does not match its body".to_string(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn root(&self) -> &ContentDigest {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommitMarker {
    schema: String,
    operation_id: OperationId,
    plan_root: ContentDigest,
    delta_root: ContentDigest,
}

impl CommitMarker {
    fn from_plan(plan: &RepositoryTxnPlan) -> Self {
        Self {
            schema: REPOSITORY_TXN_MARKER_SCHEMA.to_string(),
            operation_id: plan.operation_id.clone(),
            plan_root: plan.root.clone(),
            delta_root: plan.canonical_delta.root.clone(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BlobRetention {
    Retained,
    Pruned,
}

fn retained_blob_journals() -> BlobRetention {
    BlobRetention::Retained
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryTxnJournal {
    schema: String,
    plan: RepositoryTxnPlan,
    recovery: RecoveryState,
    /// Postimage bytes are required until installation is verified. Completed
    /// transactions retain their exact plan, marker, file-state commitments,
    /// and event membership after these private recovery copies are pruned.
    #[serde(default = "retained_blob_journals")]
    blob_retention: BlobRetention,
}

impl RepositoryTxnJournal {
    fn verify(&self) -> Result<(), RepositoryTxnError> {
        if self.schema != REPOSITORY_TXN_SCHEMA {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "unexpected repository transaction journal schema {}",
                self.schema
            )));
        }
        if self.blob_retention == BlobRetention::Pruned
            && !matches!(self.recovery, RecoveryState::Completed)
        {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "transaction {} pruned recovery blobs before completion",
                self.plan.operation_id.as_str()
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
struct RepositoryTxnPaths {
    plan: PathBuf,
    marker: PathBuf,
    blob_dir: PathBuf,
}

impl RepositoryTxnPaths {
    fn new(journal_dir: &Path, operation_id: &OperationId) -> Self {
        let repository_dir = journal_dir.join("repository");
        Self {
            plan: operation_journal::path(&repository_dir, operation_id.as_str()),
            marker: operation_journal::path(
                &repository_dir.join("committed"),
                operation_id.as_str(),
            ),
            blob_dir: repository_dir.join("blobs"),
        }
    }

    fn blob(&self, digest: &ContentDigest) -> PathBuf {
        self.blob_dir.join(format!("{}.json", digest.file_stem()))
    }
}

#[derive(Debug)]
struct RepositoryWriteLock {
    _file: File,
}

impl Drop for RepositoryWriteLock {
    fn drop(&mut self) {
        // Release synchronously at the transaction boundary. Relying only on
        // descriptor teardown made immediate same-process replanning flaky on
        // some filesystems under the parallel workspace harness.
        let _ = self._file.unlock();
    }
}

impl RepositoryWriteLock {
    fn acquire(journal_dir: &Path, root: &Path) -> Result<Self, RepositoryTxnError> {
        let lock_id = ContentDigest::hash(root.to_string_lossy().as_bytes());
        let path = journal_dir
            .join("repository-locks")
            .join(format!("{}.lock", lock_id.file_stem()));
        let parent = path.parent().ok_or_else(|| {
            RepositoryTxnError::Io(format!(
                "repository lock path has no parent: {}",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            RepositoryTxnError::Io(format!(
                "create repository lock directory {}: {error}",
                parent.display()
            ))
        })?;
        let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
            RepositoryTxnError::Io(format!(
                "inspect repository lock directory {}: {error}",
                parent.display()
            ))
        })?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(RepositoryTxnError::Io(format!(
                "repository lock directory is not a regular non-symlink directory: {}",
                parent.display()
            )));
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(RepositoryTxnError::Io(format!(
                    "repository lock is not a regular non-symlink file: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "inspect repository lock {}: {error}",
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
                RepositoryTxnError::Io(format!("open repository lock {}: {error}", path.display()))
            })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(RepositoryTxnError::Busy),
            Err(std::fs::TryLockError::Error(error)) => Err(RepositoryTxnError::Io(format!(
                "lock repository {}: {error}",
                path.display()
            ))),
        }
    }
}

/// An exclusive repository lock whose recovery barrier was checked before new
/// semantic planning began. Keeping this value alive prevents another writer
/// from crossing the same barrier until it is consumed by
/// [`RepositoryTxn::prepare_with_barrier`] or dropped.
#[derive(Debug)]
pub(crate) struct RepositoryRecoveryBarrier {
    root: PathBuf,
    journal_dir: PathBuf,
    lock: RepositoryWriteLock,
}

/// A recovery barrier that has additionally passed the repository-generation
/// write gate.
///
/// The authorization is deliberately in-memory and non-serializable. A
/// durable Prepared journal therefore cannot recreate permission to cross the
/// commit-marker boundary after a process restart.
#[derive(Debug)]
pub(crate) struct CanonicalWriteBarrier {
    recovery: RepositoryRecoveryBarrier,
    authorization: RepositoryTxnAuthorization,
}

#[derive(Debug)]
struct RepositoryAuthorityWriteAuthorization {
    repository_id: String,
    boundary_event_id: String,
    boundary_event_root: ContentDigest,
}

#[derive(Debug)]
struct RoutineEvidenceWriteAuthorization {
    repository_id: String,
    origin_id: String,
    repository_root: ContentDigest,
    authority_record_root: ContentDigest,
    authority_event_log_root: ContentDigest,
}

#[derive(Debug)]
struct FreshRepositoryAuthorization {
    repository_id: String,
    context_root: ContentDigest,
    trusted_user_home: PathBuf,
    delta_root: Option<ContentDigest>,
}

fn verify_fresh_repository_authorization(
    root: &Path,
    trusted_user_home: &Path,
) -> Result<FreshRepositoryAuthorization, RepositoryTxnError> {
    let trusted_user_home = fs::canonicalize(trusted_user_home).map_err(|error| {
        RepositoryTxnError::RepositoryTrustAnchor(format!(
            "resolve operating-system account home for trust store: {error}"
        ))
    })?;
    if root.join(".vela/origin.json").exists() || root.join(".vela/repository.json").exists() {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority_initialization",
            reason: "fresh repository authority requires an uninitialized current repository"
                .into(),
        });
    }
    let profile = crate::repository::verify_bootstrap_at(root).map_err(|reason| {
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority_initialization",
            reason,
        }
    })?;
    let profile_root = profile.profile_root().map_err(|reason| {
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority_initialization",
            reason,
        }
    })?;
    let context_root =
        fresh_repository_context_root(&profile.repository_id, &profile_root, &trusted_user_home)?;
    Ok(FreshRepositoryAuthorization {
        repository_id: profile.repository_id,
        context_root,
        trusted_user_home,
        delta_root: None,
    })
}

fn fresh_repository_context_root(
    repository_id: &str,
    profile_root: &str,
    trusted_user_home: &Path,
) -> Result<ContentDigest, RepositoryTxnError> {
    Ok(ContentDigest::hash(
        vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
            "schema": "vela.repository-bootstrap-authorization.internal.v3",
            "repository_id": repository_id,
            "profile_root": profile_root,
            "trusted_user_home": trusted_user_home.to_string_lossy(),
        }))
        .map_err(RepositoryTxnError::Canonicalize)?,
    ))
}

fn verify_routine_evidence_write_era(
    root: &Path,
) -> Result<RoutineEvidenceWriteAuthorization, RepositoryTxnError> {
    let intent = "routine_evidence";
    let repository = crate::repository::verify_repository_at(root, true).map_err(|error| {
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent,
            reason: format!("repository origin is invalid: {error}"),
        }
    })?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| RepositoryTxnError::Io(format!("read repository origin: {error}")))?;
    let origin = vela_protocol::repository_origin::RepositoryOriginV1::parse(&origin_bytes)
        .map_err(|error| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent,
            reason: format!("repository origin is invalid: {error}"),
        })?;
    let authority =
        crate::cli::load_repository_authority(root, &repository, &origin).map_err(|error| {
            RepositoryTxnError::RepositoryWriteIntentDenied {
                intent,
                reason: format!("current repository-authority history is invalid: {error}"),
            }
        })?;
    if authority.verification.closed {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent,
            reason: "repository-authority history is closed".into(),
        });
    }
    let authority_record_root = authority
        .verification
        .final_authority_record_root
        .as_deref()
        .ok_or_else(|| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent,
            reason: "current repository-authority history has no head record".into(),
        })?;
    let repository_root = ContentDigest::parse(repository.canonical_root().map_err(|error| {
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent,
            reason: format!("current repository has no valid root: {error}"),
        }
    })?)?;
    Ok(RoutineEvidenceWriteAuthorization {
        repository_id: repository.repository_id,
        origin_id: origin.id().map_err(RepositoryTxnError::CorruptPlan)?,
        repository_root,
        authority_record_root: ContentDigest::parse(authority_record_root.to_string())?,
        authority_event_log_root: ContentDigest::parse(
            authority.verification.final_event_log_root,
        )?,
    })
}

fn verify_repository_authority_write_era(
    root: &Path,
) -> Result<RepositoryAuthorityWriteAuthorization, RepositoryTxnError> {
    let repository = crate::repository::verify_repository_at(root, true).map_err(|error| {
        RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: format!("repository origin is invalid: {error}"),
        }
    })?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| RepositoryTxnError::Io(format!("read repository origin: {error}")))?;
    let origin = vela_protocol::repository_origin::RepositoryOriginV1::parse(&origin_bytes)
        .map_err(|error| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: format!("repository origin is invalid: {error}"),
        })?;
    let authority =
        crate::cli::load_repository_authority(root, &repository, &origin).map_err(|error| {
            RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "repository_authority",
                reason: format!("current repository-authority history is invalid: {error}"),
            }
        })?;
    verified_repository_authority_write_authorization(&repository, &authority)
}

fn verified_repository_authority_write_authorization(
    repository: &vela_protocol::repository::RepositoryV4,
    authority: &crate::cli::LoadedRepositoryAuthority,
) -> Result<RepositoryAuthorityWriteAuthorization, RepositoryTxnError> {
    if authority.verification.closed {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: "repository-authority history is closed".into(),
        });
    }
    let first_root = authority
        .verification
        .first_authority_record_root
        .as_deref()
        .ok_or_else(|| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: "current repository-authority history has no sequence-one root".into(),
        })?;
    let trusted_user_home = operating_system_account_home()?;
    let anchor =
        load_authority_trust_anchor_from_home(&trusted_user_home, &repository.repository_id)
            .map_err(|error| RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "repository_authority",
                reason: format!("load local authority trust anchor: {error}"),
            })?
            .ok_or_else(|| RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "repository_authority",
                reason: format!(
                    "current repository-authority writes require an independent sequence-one pin; run `vela authority trust pin . --record-root {first_root} --json`"
                ),
            })?;
    let anchor_selects = anchor
        .anchor
        .verify_sequence_one(&repository.repository_id, first_root)
        .is_ok();
    if !anchor_selects {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: format!(
                "local authority trust anchor does not select current sequence one {first_root}"
            ),
        });
    }
    let initialization_event_id = authority
        .verification
        .initialization_event_id
        .as_deref()
        .ok_or_else(|| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: "current repository authority has no origin initialization event".into(),
        })?;
    let event = authority
        .history
        .authority_events
        .iter()
        .find(|event| event.id == initialization_event_id)
        .ok_or_else(|| RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "repository_authority",
            reason: format!(
                "current origin initialization event {initialization_event_id} is missing"
            ),
        })?;
    Ok(RepositoryAuthorityWriteAuthorization {
        repository_id: repository.repository_id.clone(),
        boundary_event_id: initialization_event_id.to_string(),
        boundary_event_root: ContentDigest::parse(event.root().map_err(|error| {
            RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "repository_authority",
                reason: format!(
                    "current origin initialization event {initialization_event_id} has no valid root: {error}"
                ),
            }
        })?)?,
    })
}

fn staged_authority_event(
    write: &StagedWrite,
    mut read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
) -> Result<Option<vela_protocol::authority::AuthorityEventV1>, RepositoryTxnError> {
    let Some(relative) = write.path.as_str().strip_prefix(".vela/authority/events/") else {
        return Ok(None);
    };
    let Some(event_id) = relative.strip_suffix(".json") else {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "invalid",
            reason: format!(
                "authority event write {} is not one direct JSON event",
                write.path.as_str()
            ),
        });
    };
    if write.class != WriteClass::Authority
        || !matches!(write.preimage, FileState::Absent)
        || !matches!(write.postimage, FileState::File { .. })
    {
        return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
            intent: "invalid",
            reason: format!(
                "authority events are append-only Authority writes: {}",
                write.path.as_str()
            ),
        });
    }
    let blob = write.payload.as_ref().ok_or_else(|| {
        RepositoryTxnError::CorruptPlan(format!(
            "authority event write {} has no postimage blob",
            write.path.as_str()
        ))
    })?;
    let bytes = read_blob(blob)?;
    let event = serde_json::from_slice::<vela_protocol::authority::AuthorityEventV1>(&bytes)
        .map_err(|error| {
            RepositoryTxnError::CorruptPlan(format!(
                "authority event write {} is invalid: {error}",
                write.path.as_str()
            ))
        })?;
    event.validate().map_err(RepositoryTxnError::CorruptPlan)?;
    if event.id != event_id {
        return Err(RepositoryTxnError::CorruptPlan(format!(
            "authority event write {} has a mismatched content id",
            write.path.as_str()
        )));
    }
    Ok(Some(event))
}

fn verify_fresh_repository_delta(
    delta: &CanonicalDelta,
    mut read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
) -> Result<(), RepositoryTxnError> {
    let deny = |reason: String| RepositoryTxnError::RepositoryWriteIntentDenied {
        intent: "repository_authority_initialization",
        reason,
    };
    let mut authority_initializations = 0_usize;
    let mut authority_records = 0_usize;
    let mut repository_origins = Vec::new();
    let mut repository_v3_manifests = Vec::new();

    for write in delta.writes() {
        let path = write.path.as_str();
        if path.starts_with(".vela/events/") {
            return Err(deny(
                "fresh repository authority cannot append a retired event".into(),
            ));
        }
        let authority_event = staged_authority_event(write, &mut read_blob)?;
        if let Some(event) = authority_event {
            if event.content.kind.as_str()
                != vela_protocol::authority_history::AUTHORITY_INITIALIZED_EVENT_KIND
            {
                return Err(deny(format!(
                    "fresh repository authority cannot append event kind {}",
                    event.content.kind
                )));
            }
            authority_initializations += 1;
        } else if path == ".vela/origin.json" {
            if write.class != WriteClass::CanonicalEvidence
                || !matches!(write.preimage, FileState::Absent)
                || !matches!(write.postimage, FileState::File { .. })
            {
                return Err(deny(
                    "repository initialization must create one canonical origin object".into(),
                ));
            }
            let blob = write.payload.as_ref().ok_or_else(|| {
                RepositoryTxnError::CorruptPlan("repository origin has no postimage blob".into())
            })?;
            let bytes = read_blob(blob)?;
            let origin = vela_protocol::repository_origin::RepositoryOriginV1::parse(&bytes)
                .map_err(RepositoryTxnError::CorruptPlan)?;
            repository_origins.push(origin);
        } else if path == ".vela/repository.json" {
            if write.class != WriteClass::CanonicalEvidence
                || !matches!(write.preimage, FileState::Absent)
                || !matches!(write.postimage, FileState::File { .. })
            {
                return Err(deny(
                    "repository genesis must create one canonical manifest".into(),
                ));
            }
            let blob = write.payload.as_ref().ok_or_else(|| {
                RepositoryTxnError::CorruptPlan("repository manifest has no postimage blob".into())
            })?;
            let bytes = read_blob(blob)?;
            let repository = vela_protocol::repository::RepositoryV4::parse(&bytes)
                .map_err(RepositoryTxnError::CorruptPlan)?;
            repository_v3_manifests.push(repository);
        } else if !path.starts_with(".vela/authority/")
            || write.class != WriteClass::Authority
            || !matches!(write.preimage, FileState::Absent)
            || !matches!(write.postimage, FileState::File { .. })
        {
            return Err(deny(format!(
                "fresh repository authority contains unrelated or non-append write {path}"
            )));
        } else if path.starts_with(".vela/authority/records/") && path.ends_with(".dsse.json") {
            authority_records += 1;
        }
    }

    if authority_initializations != 1 || authority_records != 1 {
        return Err(deny(format!(
            "fresh repository authority requires one initialization event and one covering record; found {authority_initializations} and {authority_records}"
        )));
    }

    if repository_origins.len() != 1 || repository_v3_manifests.len() != 1 {
        return Err(deny(format!(
            "fresh repository authority requires exactly one origin and one repository manifest; found {} origin and {} repository object(s)",
            repository_origins.len(),
            repository_v3_manifests.len()
        )));
    }

    let origin = repository_origins.first().expect("checked one origin");
    let repository = repository_v3_manifests
        .first()
        .expect("checked one repository");
    let origin_root = origin
        .canonical_root()
        .map_err(RepositoryTxnError::CorruptPlan)?;
    if repository.repository_id != origin.repository_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.id().map_err(deny)?
        || repository.origin_root != origin_root
    {
        return Err(deny(
            "repository origin and manifest do not bind the same exact identity".into(),
        ));
    }
    if !repository.accepted_claims.is_empty()
        || !repository.pending_claims.is_empty()
        || !repository.proposals.is_empty()
        || !repository.submissions.is_empty()
        || !repository.verifications.is_empty()
        || !repository.artifacts.is_empty()
    {
        return Err(deny(
            "fresh repository initialization requires a genesis origin and empty object set".into(),
        ));
    }
    Ok(())
}

fn bind_fresh_repository_authorization(
    authorization: &mut FreshRepositoryAuthorization,
    delta: &CanonicalDelta,
    read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
) -> Result<(), RepositoryTxnError> {
    verify_fresh_repository_delta(delta, read_blob)?;
    if let Some(bound) = &authorization.delta_root
        && bound != delta.root()
    {
        return Err(RepositoryTxnError::WriteAuthorizationDeltaMismatch {
            authorized: bound.clone(),
            planned: delta.root().clone(),
        });
    }
    authorization.delta_root = Some(delta.root().clone());
    Ok(())
}

/// Resolve the current operating-system account home without consulting
/// `HOME`, repository configuration, or a process-local override.
#[cfg(unix)]
#[allow(unsafe_code)]
pub(crate) fn operating_system_account_home() -> Result<PathBuf, RepositoryTxnError> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStringExt;

    // SAFETY: `geteuid` has no preconditions. `getpwuid_r` receives a live
    // passwd allocation, an owned writable buffer, and a result pointer for
    // the duration of each call. The returned `pw_dir` is copied before the
    // buffer is dropped.
    let uid = unsafe { libc::geteuid() };
    let suggested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let mut capacity = if suggested > 0 {
        usize::try_from(suggested).unwrap_or(16 * 1024)
    } else {
        16 * 1024
    }
    .clamp(1024, 1024 * 1024);
    loop {
        let mut buffer = vec![0_u8; capacity];
        let mut passwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                passwd.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE && capacity < 1024 * 1024 {
            capacity = (capacity * 2).min(1024 * 1024);
            continue;
        }
        if status != 0 {
            return Err(RepositoryTxnError::RepositoryTrustAnchor(format!(
                "resolve operating-system account home for effective uid {uid}: OS error {status}"
            )));
        }
        if result.is_null() {
            return Err(RepositoryTxnError::RepositoryTrustAnchor(format!(
                "operating-system account for effective uid {uid} has no password-database entry"
            )));
        }
        let directory = unsafe { CStr::from_ptr((*result).pw_dir) };
        if directory.to_bytes().is_empty() {
            return Err(RepositoryTxnError::RepositoryTrustAnchor(
                "operating-system account has an empty home directory".to_string(),
            ));
        }
        return Ok(PathBuf::from(OsString::from_vec(
            directory.to_bytes().to_vec(),
        )));
    }
}

impl RepositoryRecoveryBarrier {
    /// Return the already-verified completed plan for one operation while this
    /// barrier owns the repository lock. This closes the race where another
    /// process completes the same operation between an unlocked exact-retry
    /// lookup and barrier acquisition; callers can return the durable result
    /// without rederiving stale applied proposals or touching a private key.
    #[cfg(test)]
    pub(crate) fn completed_plan(
        &self,
        operation_id: &OperationId,
    ) -> Result<Option<RepositoryTxnPlan>, RepositoryTxnError> {
        let journals = repository_journals(&self.root, &self.journal_dir)?;
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

    /// Authorize a repository-authority write from state that was completely
    /// verified after this recovery barrier acquired the repository lock.
    ///
    /// This avoids repeating the same repository and authority replay inside
    /// one Decision while preserving the independent trust-anchor check and
    /// the exact delta authorization performed when the transaction is
    /// prepared.
    pub(crate) fn authorize_verified_repository_authority(
        self,
        repository: &vela_protocol::repository::RepositoryV4,
        authority: &crate::cli::LoadedRepositoryAuthority,
    ) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        let authorization =
            verified_repository_authority_write_authorization(repository, authority)?;
        Ok(CanonicalWriteBarrier {
            recovery: self,
            authorization: RepositoryTxnAuthorization::RepositoryAuthority(authorization),
        })
    }

    fn authorize_for_routine_evidence(self) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        let authorization = verify_routine_evidence_write_era(&self.root)?;
        Ok(CanonicalWriteBarrier {
            recovery: self,
            authorization: RepositoryTxnAuthorization::RoutineEvidence(authorization),
        })
    }

    fn authorize_for_fresh_repository(self) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        let trusted_user_home = operating_system_account_home()?;
        let authorization = verify_fresh_repository_authorization(&self.root, &trusted_user_home)?;
        Ok(CanonicalWriteBarrier {
            recovery: self,
            authorization: RepositoryTxnAuthorization::FreshRepository(authorization),
        })
    }

    #[cfg(test)]
    pub(crate) fn authorize_for_test(self) -> CanonicalWriteBarrier {
        CanonicalWriteBarrier {
            recovery: self,
            authorization: RepositoryTxnAuthorization::TestHarness,
        }
    }
}

impl CanonicalWriteBarrier {
    #[cfg(test)]
    pub(crate) fn completed_plan(
        &self,
        operation_id: &OperationId,
    ) -> Result<Option<RepositoryTxnPlan>, RepositoryTxnError> {
        self.recovery.completed_plan(operation_id)
    }
}

fn reverify_transaction_authorization(
    root: &Path,
    authorization: &RepositoryTxnAuthorization,
) -> Result<(), RepositoryTxnError> {
    match authorization {
        RepositoryTxnAuthorization::FreshRepository(expected) => {
            let actual = verify_fresh_repository_authorization(root, &expected.trusted_user_home)?;
            if actual.context_root != expected.context_root {
                return Err(RepositoryTxnError::StaleWriteAuthorization {
                    expected: expected.context_root.clone(),
                    actual: actual.context_root,
                });
            }
            Ok(())
        }
        RepositoryTxnAuthorization::RepositoryAuthority(expected) => {
            let actual = verify_repository_authority_write_era(root)?;
            if actual.repository_id != expected.repository_id
                || actual.boundary_event_id != expected.boundary_event_id
                || actual.boundary_event_root != expected.boundary_event_root
            {
                return Err(RepositoryTxnError::StaleWriteAuthorization {
                    expected: expected.boundary_event_root.clone(),
                    actual: actual.boundary_event_root,
                });
            }
            Ok(())
        }
        RepositoryTxnAuthorization::RoutineEvidence(expected) => {
            let actual = verify_routine_evidence_write_era(root)?;
            if actual.repository_id != expected.repository_id
                || actual.origin_id != expected.origin_id
                || actual.repository_root != expected.repository_root
                || actual.authority_record_root != expected.authority_record_root
                || actual.authority_event_log_root != expected.authority_event_log_root
            {
                return Err(RepositoryTxnError::StaleWriteAuthorization {
                    expected: expected.repository_root.clone(),
                    actual: actual.repository_root,
                });
            }
            Ok(())
        }
        #[cfg(test)]
        RepositoryTxnAuthorization::TestHarness => Ok(()),
    }
}

fn repository_journals(
    root: &Path,
    journal_dir: &Path,
) -> Result<Vec<(RepositoryTxnPaths, RepositoryTxnJournal)>, RepositoryTxnError> {
    let repository_dir = journal_dir.join("repository");
    let metadata = match fs::symlink_metadata(&repository_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect repository journal directory {}: {error}",
                repository_dir.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryTxnError::Journal(format!(
            "repository journal directory is not a regular non-symlink directory: {}",
            repository_dir.display()
        )));
    }

    let mut entries = fs::read_dir(&repository_dir)
        .map_err(|error| {
            RepositoryTxnError::Journal(format!(
                "read repository journal directory {}: {error}",
                repository_dir.display()
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            RepositoryTxnError::Journal(format!(
                "enumerate repository journal directory {}: {error}",
                repository_dir.display()
            ))
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut journals = Vec::new();
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            RepositoryTxnError::Journal(format!(
                "inspect repository journal entry {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RepositoryTxnError::Journal(format!(
                "repository journal entry is a symbolic link: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if name == "blobs" || name == "committed" {
                continue;
            }
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected directory in repository journal: {}",
                path.display()
            )));
        }
        if !metadata.is_file() || path.extension().is_none_or(|extension| extension != "json") {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected non-journal entry in repository journal: {}",
                path.display()
            )));
        }

        let journal: RepositoryTxnJournal =
            operation_journal::read_json(&path).map_err(RepositoryTxnError::Journal)?;
        journal.verify()?;
        let paths = RepositoryTxnPaths::new(journal_dir, &journal.plan.operation_id);
        if path != paths.plan {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "repository transaction {} is stored under the wrong journal name",
                journal.plan.operation_id.as_str()
            )));
        }
        if Path::new(&journal.plan.repository.canonical_root) == root {
            journal.plan.repository.verify_root(root)?;
            journals.push((paths, journal));
        }
    }
    Ok(journals)
}

fn journal_blob_digests(journal: &RepositoryTxnJournal) -> BTreeSet<ContentDigest> {
    journal
        .plan
        .canonical_delta
        .writes()
        .iter()
        .filter_map(|write| write.payload.as_ref().map(|blob| blob.digest.clone()))
        .collect()
}

fn require_journal_directory(path: &Path, label: &str) -> Result<(), RepositoryTxnError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        RepositoryTxnError::Journal(format!("inspect {label} {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryTxnError::Journal(format!(
            "{label} is not a regular non-symlink directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn read_commit_marker(
    paths: &RepositoryTxnPaths,
    journal: &RepositoryTxnJournal,
) -> Result<CommitMarker, RepositoryTxnError> {
    let marker_dir = paths.marker.parent().ok_or_else(|| {
        RepositoryTxnError::Journal(format!(
            "repository commit marker has no parent: {}",
            paths.marker.display()
        ))
    })?;
    match fs::symlink_metadata(marker_dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RepositoryTxnError::NotCommitted);
        }
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect repository commit-marker directory {}: {error}",
                marker_dir.display()
            )));
        }
        Ok(_) => require_journal_directory(marker_dir, "repository commit-marker directory")?,
    }
    let metadata = match fs::symlink_metadata(&paths.marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RepositoryTxnError::NotCommitted);
        }
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect repository commit marker {}: {error}",
                paths.marker.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RepositoryTxnError::Journal(format!(
            "repository commit marker is not a regular non-symlink file: {}",
            paths.marker.display()
        )));
    }
    let marker: CommitMarker =
        operation_journal::read_json(&paths.marker).map_err(RepositoryTxnError::Journal)?;
    let expected = CommitMarker::from_plan(&journal.plan);
    if marker != expected {
        return Err(RepositoryTxnError::CorruptPlan(
            "commit marker does not match the durable plan".to_string(),
        ));
    }
    Ok(marker)
}

fn read_blob_at(
    paths: &RepositoryTxnPaths,
    expected: &JournalBlobRef,
) -> Result<Vec<u8>, RepositoryTxnError> {
    let path = paths.blob(&expected.digest);
    match fs::symlink_metadata(&paths.blob_dir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RepositoryTxnError::MissingBlob(expected.digest.clone()));
        }
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect repository transaction blob directory {}: {error}",
                paths.blob_dir.display()
            )));
        }
        Ok(_) => {
            require_journal_directory(&paths.blob_dir, "repository transaction blob directory")?
        }
    }
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(RepositoryTxnError::MissingBlob(expected.digest.clone()));
        }
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect repository transaction blob {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RepositoryTxnError::CorruptBlob(expected.digest.clone()));
    }
    let blob: BlobJournal =
        operation_journal::read_json(&path).map_err(RepositoryTxnError::Journal)?;
    if blob.schema != REPOSITORY_TXN_BLOB_SCHEMA
        || blob.digest != expected.digest
        || blob.size != expected.size
    {
        return Err(RepositoryTxnError::CorruptBlob(expected.digest.clone()));
    }
    validate_blob_bytes(expected, &blob.bytes)?;
    Ok(blob.bytes)
}

fn verify_journal_blobs(
    paths: &RepositoryTxnPaths,
    journal: &RepositoryTxnJournal,
) -> Result<(), RepositoryTxnError> {
    for write in journal.plan.canonical_delta.writes() {
        if let Some(blob) = &write.payload {
            read_blob_at(paths, blob)?;
        }
    }
    Ok(())
}

fn verify_completed_marker_and_blobs(
    paths: &RepositoryTxnPaths,
    journal: &RepositoryTxnJournal,
) -> Result<(), RepositoryTxnError> {
    if !matches!(journal.recovery, RecoveryState::Completed) {
        return Err(RepositoryTxnError::CorruptPlan(format!(
            "transaction {} is not completed",
            journal.plan.operation_id.as_str()
        )));
    }
    read_commit_marker(paths, journal)?;
    if journal.blob_retention == BlobRetention::Retained {
        verify_journal_blobs(paths, journal)?;
    }
    Ok(())
}

fn verify_aborted_without_marker(
    paths: &RepositoryTxnPaths,
    journal: &RepositoryTxnJournal,
) -> Result<(), RepositoryTxnError> {
    if !matches!(journal.recovery, RecoveryState::Aborted) {
        return Err(RepositoryTxnError::CorruptPlan(format!(
            "transaction {} is not aborted",
            journal.plan.operation_id.as_str()
        )));
    }
    match read_commit_marker(paths, journal) {
        Err(RepositoryTxnError::NotCommitted) => Ok(()),
        Ok(_) => Err(RepositoryTxnError::CorruptPlan(format!(
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
    current_head: &[(RepositoryTxnPaths, RepositoryTxnJournal)],
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

fn completed_postimage_is_rematerializable(write: &StagedWrite) -> bool {
    // The current-repository manifest is an authenticated rolling head, not
    // immutable historical evidence. Current Submission and Verification
    // transactions replace it after independently verifying the repository
    // epoch and authority chain. Legacy completed journals therefore prove
    // the manifest bytes they installed, but cannot require those bytes to
    // remain the current head forever. This exception is deliberately
    // limited to completed-history checks: active installation and
    // completion still require the exact planned postimage, while every
    // immutable event, authority record, Proposal, Submission, and evidence
    // object remains byte-exact.
    write.path.as_str() == ".vela/repository.json"
}

fn verify_completed_history(
    root: &Path,
    completed: &[(RepositoryTxnPaths, RepositoryTxnJournal)],
) -> Result<(), RepositoryTxnError> {
    if completed.is_empty() {
        return Ok(());
    }
    for (paths, journal) in completed {
        verify_completed_marker_and_blobs(paths, journal)?;
    }

    /* Every completed journal belongs to the current postimage transition
    graph. A compaction origin used to archive a predecessor repository root
    and take its journals out of this set; there is one generation now, so
    there is nothing to exclude. The retired `.vela/events` set no longer
    participates in private crash recovery or completed-history checks. */
    let current_head = completed.to_vec();

    // Validate that each durable postimage is either still current or is
    // connected to the current bytes by another completed transaction's exact
    // preimage -> postimage edge. The rolling current-repository manifest is
    // the sole rematerializable exception. Active installation and completion
    // still verify every write class.
    for (_, journal) in &current_head {
        for write in journal
            .plan
            .canonical_delta
            .writes()
            .iter()
            .filter(|write| !completed_postimage_is_rematerializable(write))
        {
            let actual = inspect_file_state(root, &write.path)?;
            if postimage_reaches_current(&write.path, &write.postimage, &actual, &current_head) {
                continue;
            }
            return Err(RepositoryTxnError::CompletedPostimageMismatch {
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
) -> Result<(), RepositoryTxnError> {
    let journals = repository_journals(root, journal_dir)?;
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
                return Err(RepositoryTxnError::RecoveryRequired {
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
    mut read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
) -> Result<Vec<ResolvedWrite>, RepositoryTxnError> {
    let mut writes = delta
        .public_writes()
        .map(|write| {
            let postimage_bytes = match (&write.postimage, &write.payload) {
                (FileState::Absent, None) => None,
                (FileState::File { .. }, Some(blob)) => Some(read_blob(blob)?),
                (FileState::Absent, Some(_)) => {
                    return Err(RepositoryTxnError::CorruptPlan(format!(
                        "deleted postimage {} carries a blob reference",
                        write.path.as_str()
                    )));
                }
                (FileState::File { .. }, None) => {
                    return Err(RepositoryTxnError::CorruptPlan(format!(
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
        .collect::<Result<Vec<_>, RepositoryTxnError>>()?;
    // Git's exact-delta boundary is path-sorted, while installation order is
    // semantic (evidence before review before authority before private state).
    writes.sort_by(|left, right| left.staged.path.cmp(&right.staged.path));
    Ok(writes)
}

fn validate_blob_bytes(expected: &JournalBlobRef, bytes: &[u8]) -> Result<(), RepositoryTxnError> {
    if bytes.len() as u64 != expected.size || ContentDigest::hash(bytes) != expected.digest {
        return Err(RepositoryTxnError::CorruptBlob(expected.digest.clone()));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct RepositoryTxn {
    root: PathBuf,
    paths: RepositoryTxnPaths,
    journal: RepositoryTxnJournal,
    authorization: Option<RepositoryTxnAuthorization>,
    _lock: RepositoryWriteLock,
}

#[derive(Debug)]
enum RepositoryTxnAuthorization {
    FreshRepository(FreshRepositoryAuthorization),
    RepositoryAuthority(RepositoryAuthorityWriteAuthorization),
    RoutineEvidence(RoutineEvidenceWriteAuthorization),
    #[cfg(test)]
    TestHarness,
}

impl RepositoryTxnAuthorization {
    fn verified_repository_id(&self) -> Option<&str> {
        match self {
            Self::FreshRepository(authorization) => Some(&authorization.repository_id),
            Self::RepositoryAuthority(authorization) => Some(&authorization.repository_id),
            Self::RoutineEvidence(authorization) => Some(&authorization.repository_id),
            #[cfg(test)]
            Self::TestHarness => None,
        }
    }
}

#[cfg(test)]
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
pub(crate) enum RepositoryTxnStep {
    BeforeBlobJournalWrite { index: usize },
    AfterBlobJournalWrite { index: usize },
    BeforePreparedJournalWrite,
    AfterPreparedJournalWrite,
    BeforeAbortedJournalWrite,
    AfterAbortedJournalWrite,
    BeforeCommitMarkerWrite,
    AfterCommitMarkerWrite,
    BeforeCommittedJournalWrite,
    AfterCommittedJournalWrite,
    BeforeInstallWrite { index: usize },
    AfterInstallWrite { index: usize },
    BeforeCommittedConflictJournalWrite { index: usize },
    AfterCommittedConflictJournalWrite { index: usize },
    BeforeInstallingJournalWrite { index: usize },
    AfterInstallingJournalWrite { index: usize },
    BeforeInstalledJournalWrite,
    AfterInstalledJournalWrite,
    BeforeInstalledStateVerification,
    AfterInstalledStateVerification,
    BeforeCompletedJournalWrite,
    AfterCompletedJournalWrite,
}

trait RepositoryTxnFailpoints {
    fn check(&mut self, step: RepositoryTxnStep) -> Result<(), RepositoryTxnError>;
}

struct NoRepositoryTxnFailpoints;

impl RepositoryTxnFailpoints for NoRepositoryTxnFailpoints {
    #[inline]
    fn check(&mut self, _step: RepositoryTxnStep) -> Result<(), RepositoryTxnError> {
        Ok(())
    }
}

#[cfg(test)]
struct FailAtRepositoryTxnStep {
    target: RepositoryTxnStep,
}

#[cfg(test)]
impl RepositoryTxnFailpoints for FailAtRepositoryTxnStep {
    fn check(&mut self, step: RepositoryTxnStep) -> Result<(), RepositoryTxnError> {
        if step == self.target {
            return Err(RepositoryTxnError::InjectedFailure { step });
        }
        Ok(())
    }
}

impl RepositoryTxn {
    #[cfg(test)]
    pub(crate) fn verify_recovery_barrier_read_only(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<(), RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        ensure_recovery_barrier_locked(&root, journal_dir, None)
    }

    /// Acquire the repository-wide recovery barrier before loading mutable
    /// repository inputs for a new operation. The returned guard deliberately
    /// holds the write lock through planning and must be consumed by
    /// [`Self::prepare_with_barrier`].
    pub(crate) fn acquire_recovery_barrier(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<RepositoryRecoveryBarrier, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let lock = RepositoryWriteLock::acquire(journal_dir, &root)?;
        ensure_recovery_barrier_locked(&root, journal_dir, None)?;
        Ok(RepositoryRecoveryBarrier {
            root,
            journal_dir: journal_dir.to_path_buf(),
            lock,
        })
    }

    pub(crate) fn acquire_routine_evidence_write_barrier(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        // Reject invalid, closed, or pre-current repositories before creating
        // even the ignored lock. Routine evidence binds the repository and
        // authority heads but deliberately does not require a caller-local
        // trust pin; no scientific Standing can change through this path.
        verify_routine_evidence_write_era(&root)?;
        Self::acquire_recovery_barrier(&root, journal_dir)?.authorize_for_routine_evidence()
    }

    pub(crate) fn acquire_repository_authority_initialization_barrier(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let trusted_user_home = operating_system_account_home()?;
        verify_fresh_repository_authorization(&root, &trusted_user_home)?;
        Self::acquire_recovery_barrier(&root, journal_dir)?.authorize_for_fresh_repository()
    }

    #[cfg(test)]
    pub(crate) fn acquire_write_barrier_for_test(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<CanonicalWriteBarrier, RepositoryTxnError> {
        Ok(Self::acquire_recovery_barrier(repository_root, journal_dir)?.authorize_for_test())
    }

    #[cfg(test)]
    pub(crate) fn prepare(
        repository_root: &Path,
        journal_dir: &Path,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
    ) -> Result<Self, RepositoryTxnError> {
        let barrier = Self::acquire_recovery_barrier(repository_root, journal_dir)?;
        Self::prepare_with_recovery_barrier_and_authorization(
            barrier,
            RepositoryTxnAuthorization::TestHarness,
            plan,
            draft,
            &mut NoRepositoryTxnFailpoints,
        )
    }

    pub(crate) fn prepare_with_barrier(
        barrier: CanonicalWriteBarrier,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
    ) -> Result<Self, RepositoryTxnError> {
        let CanonicalWriteBarrier {
            recovery,
            authorization,
        } = barrier;
        Self::prepare_with_recovery_barrier_and_authorization(
            recovery,
            authorization,
            plan,
            draft,
            &mut NoRepositoryTxnFailpoints,
        )
    }

    fn prepare_with_recovery_barrier_and_authorization(
        barrier: RepositoryRecoveryBarrier,
        mut authorization: RepositoryTxnAuthorization,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<Self, RepositoryTxnError> {
        plan.verify()?;
        if let RepositoryTxnAuthorization::FreshRepository(verified) = &mut authorization {
            bind_fresh_repository_authorization(verified, &plan.canonical_delta, |blob| {
                let bytes = draft
                    .blobs
                    .get(&blob.digest)
                    .cloned()
                    .ok_or_else(|| RepositoryTxnError::MissingBlob(blob.digest.clone()))?;
                validate_blob_bytes(blob, &bytes)?;
                Ok(bytes)
            })?;
        }
        if let Some(authorized) = authorization.verified_repository_id()
            && plan.repository.repository_id != authorized
        {
            return Err(RepositoryTxnError::WriteAuthorizationRepositoryMismatch {
                authorized: authorized.to_string(),
                planned: plan.repository.repository_id.clone(),
            });
        }
        if plan.canonical_delta != draft.delta {
            return Err(RepositoryTxnError::CorruptPlan(
                "plan delta differs from prepared postimage blobs".to_string(),
            ));
        }
        let root = plan.repository.verify_root(&barrier.root)?;
        let RepositoryRecoveryBarrier {
            root: barrier_root,
            journal_dir,
            lock,
        } = barrier;
        if root != barrier_root {
            return Err(RepositoryTxnError::RepositoryBindingMismatch {
                expected: barrier_root.display().to_string(),
                actual: root.display().to_string(),
            });
        }
        let paths = RepositoryTxnPaths::new(&journal_dir, &plan.operation_id);

        match fs::symlink_metadata(&paths.plan) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(RepositoryTxnError::Journal(format!(
                    "repository transaction journal is not a regular non-symlink file: {}",
                    paths.plan.display()
                )));
            }
            Ok(_) => {
                let journal: RepositoryTxnJournal = operation_journal::read_json(&paths.plan)
                    .map_err(RepositoryTxnError::Journal)?;
                journal.verify()?;
                if matches!(journal.recovery, RecoveryState::Aborted) {
                    verify_aborted_without_marker(&paths, &journal)?;
                    if journal.plan.request_root != plan.request_root {
                        return Err(RepositoryTxnError::OperationConflict {
                            operation_id: plan.operation_id.as_str().to_string(),
                        });
                    }
                } else {
                    if journal.plan.root != plan.root {
                        return Err(RepositoryTxnError::OperationConflict {
                            operation_id: plan.operation_id.as_str().to_string(),
                        });
                    }
                    let txn = Self {
                        root,
                        paths,
                        journal,
                        authorization: Some(authorization),
                        _lock: lock,
                    };
                    txn.verify_recovery_blobs()?;
                    if matches!(txn.journal.recovery, RecoveryState::Completed) {
                        txn.verify_completed_state()?;
                    }
                    return Ok(txn);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(RepositoryTxnError::Journal(format!(
                    "inspect repository transaction {}: {error}",
                    paths.plan.display()
                )));
            }
        }

        for (index, (digest, bytes)) in draft.blobs.iter().enumerate() {
            let blob = BlobJournal {
                schema: REPOSITORY_TXN_BLOB_SCHEMA.to_string(),
                digest: digest.clone(),
                size: bytes.len() as u64,
                bytes: bytes.clone(),
            };
            failpoints.check(RepositoryTxnStep::BeforeBlobJournalWrite { index })?;
            operation_journal::write_json(&paths.blob(digest), &blob)
                .map_err(RepositoryTxnError::Journal)?;
            failpoints.check(RepositoryTxnStep::AfterBlobJournalWrite { index })?;
        }
        let journal = RepositoryTxnJournal {
            schema: REPOSITORY_TXN_SCHEMA.to_string(),
            plan,
            recovery: RecoveryState::Prepared,
            blob_retention: BlobRetention::Retained,
        };
        failpoints.check(RepositoryTxnStep::BeforePreparedJournalWrite)?;
        operation_journal::write_json(&paths.plan, &journal)
            .map_err(RepositoryTxnError::Journal)?;
        failpoints.check(RepositoryTxnStep::AfterPreparedJournalWrite)?;
        let txn = Self {
            root,
            paths,
            journal,
            authorization: Some(authorization),
            _lock: lock,
        };
        txn.verify_blobs()?;
        Ok(txn)
    }

    #[cfg(test)]
    fn prepare_at_failpoint(
        repository_root: &Path,
        journal_dir: &Path,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
        step: RepositoryTxnStep,
    ) -> Result<Self, RepositoryTxnError> {
        let barrier = Self::acquire_recovery_barrier(repository_root, journal_dir)?;
        Self::prepare_with_recovery_barrier_and_authorization(
            barrier,
            RepositoryTxnAuthorization::TestHarness,
            plan,
            draft,
            &mut FailAtRepositoryTxnStep { target: step },
        )
    }

    #[cfg(test)]
    pub(crate) fn open(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Self, RepositoryTxnError> {
        Self::open_if_present(repository_root, journal_dir, operation_id)?.ok_or_else(|| {
            RepositoryTxnError::Journal(format!(
                "repository transaction {} was not found",
                operation_id.as_str()
            ))
        })
    }

    #[cfg(test)]
    pub(crate) fn open_if_present(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Option<Self>, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let lock = RepositoryWriteLock::acquire(journal_dir, &root)?;
        let paths = RepositoryTxnPaths::new(journal_dir, operation_id);
        let repository_journal_dir = paths.plan.parent().ok_or_else(|| {
            RepositoryTxnError::Journal(format!(
                "repository transaction has no journal directory: {}",
                paths.plan.display()
            ))
        })?;
        match fs::symlink_metadata(repository_journal_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RepositoryTxnError::Journal(format!(
                    "inspect repository journal directory {}: {error}",
                    repository_journal_dir.display()
                )));
            }
            Ok(_) => {
                require_journal_directory(repository_journal_dir, "repository journal directory")?
            }
        }
        match fs::symlink_metadata(&paths.plan) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RepositoryTxnError::Journal(format!(
                    "inspect repository transaction {}: {error}",
                    paths.plan.display()
                )));
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(RepositoryTxnError::Journal(format!(
                    "repository transaction journal is not a regular non-symlink file: {}",
                    paths.plan.display()
                )));
            }
            Ok(_) => {}
        }
        let journal: RepositoryTxnJournal =
            operation_journal::read_json(&paths.plan).map_err(RepositoryTxnError::Journal)?;
        journal.verify()?;
        journal.plan.repository.verify_root(&root)?;
        let txn = Self {
            root,
            paths,
            journal,
            authorization: None,
            _lock: lock,
        };
        match txn.journal.recovery {
            RecoveryState::Aborted => {
                verify_aborted_without_marker(&txn.paths, &txn.journal)?;
            }
            RecoveryState::Completed => {
                txn.verify_recovery_blobs()?;
                txn.verify_completed_state()?;
            }
            _ => txn.verify_blobs()?,
        }
        Ok(Some(txn))
    }

    #[cfg(test)]
    pub(crate) fn plan(&self) -> &RepositoryTxnPlan {
        &self.journal.plan
    }

    #[cfg(test)]
    pub(crate) fn recovery_state(&self) -> &RecoveryState {
        &self.journal.recovery
    }

    pub(crate) fn mark_committed(&mut self) -> Result<(), RepositoryTxnError> {
        self.mark_committed_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    #[cfg(test)]
    pub(crate) fn reauthorize_prepared_for_test(&mut self) -> Result<(), RepositoryTxnError> {
        if !matches!(self.journal.recovery, RecoveryState::Prepared) {
            return Err(RepositoryTxnError::WriteAuthorizationNotApplicable {
                state: self.journal.recovery.clone(),
            });
        }
        self.authorization = Some(RepositoryTxnAuthorization::TestHarness);
        Ok(())
    }

    fn mark_committed_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
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
            Err(RepositoryTxnError::NotCommitted) => {
                if !matches!(self.journal.recovery, RecoveryState::Prepared) {
                    return Err(RepositoryTxnError::CorruptPlan(format!(
                        "transaction {} is {:?} but has no commit marker",
                        self.journal.plan.operation_id.as_str(),
                        self.journal.recovery
                    )));
                }
                if self.authorization.is_none() {
                    return Err(RepositoryTxnError::WriteAuthorizationRequired {
                        operation_id: self.journal.plan.operation_id.as_str().to_string(),
                    });
                }
                let preflight = (|| {
                    reverify_transaction_authorization(
                        &self.root,
                        self.authorization
                            .as_ref()
                            .expect("authorization checked above"),
                    )?;
                    if let Some(RepositoryTxnAuthorization::FreshRepository(verified)) =
                        self.authorization.as_ref()
                    {
                        if verified.delta_root.as_ref()
                            != Some(self.journal.plan.canonical_delta.root())
                        {
                            return Err(RepositoryTxnError::WriteAuthorizationDeltaMismatch {
                                authorized: verified
                                    .delta_root
                                    .clone()
                                    .unwrap_or_else(|| ContentDigest::hash(b"unbound")),
                                planned: self.journal.plan.canonical_delta.root().clone(),
                            });
                        }
                        verify_fresh_repository_delta(
                            &self.journal.plan.canonical_delta,
                            |blob| self.read_blob(blob),
                        )?;
                    }
                    ensure_recovery_barrier_locked(
                        &self.root,
                        self.paths
                            .plan
                            .parent()
                            .and_then(Path::parent)
                            .ok_or_else(|| {
                                RepositoryTxnError::Journal(format!(
                                    "repository transaction path has no journal root: {}",
                                    self.paths.plan.display()
                                ))
                            })?,
                        Some(&self.journal.plan.operation_id),
                    )?;
                    for input in &self.journal.plan.read_set {
                        input.verify_current(&self.root)?;
                    }
                    for write in self.journal.plan.canonical_delta.writes() {
                        let current = inspect_file_state(&self.root, &write.path)?;
                        if current != write.preimage {
                            return Err(RepositoryTxnError::StalePreimage {
                                path: write.path.clone(),
                                expected: write.preimage.clone(),
                                actual: current,
                            });
                        }
                    }
                    self.verify_blobs()
                })();
                if let Err(error) = preflight {
                    self.abort_prepared()?;
                    return Err(error);
                }
                failpoints.check(RepositoryTxnStep::BeforeCommitMarkerWrite)?;
                operation_journal::write_json(&self.paths.marker, &expected_marker)
                    .map_err(RepositoryTxnError::Journal)?;
                failpoints.check(RepositoryTxnStep::AfterCommitMarkerWrite)?;
            }
            Err(error) => return Err(error),
        }
        self.journal.recovery = RecoveryState::Committed;
        failpoints.check(RepositoryTxnStep::BeforeCommittedJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(RepositoryTxnStep::AfterCommittedJournalWrite)
    }

    #[cfg(test)]
    fn mark_committed_at_failpoint(
        &mut self,
        step: RepositoryTxnStep,
    ) -> Result<(), RepositoryTxnError> {
        self.mark_committed_with_failpoints(&mut FailAtRepositoryTxnStep { target: step })
    }

    /// Permanently discard a marker-free plan. Since no commit marker exists,
    /// this state transition has no repository delta and a later plan may safely
    /// reuse the operation id.
    pub(crate) fn abort_prepared(&mut self) -> Result<(), RepositoryTxnError> {
        self.abort_prepared_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    fn abort_prepared_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
        if !matches!(self.journal.recovery, RecoveryState::Prepared) {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "cannot abort transaction {} from {:?}",
                self.journal.plan.operation_id.as_str(),
                self.journal.recovery
            )));
        }
        match read_commit_marker(&self.paths, &self.journal) {
            Err(RepositoryTxnError::NotCommitted) => {}
            Ok(_) => {
                return Err(RepositoryTxnError::CorruptPlan(format!(
                    "cannot abort committed transaction {}",
                    self.journal.plan.operation_id.as_str()
                )));
            }
            Err(error) => return Err(error),
        }
        failpoints.check(RepositoryTxnStep::BeforeAbortedJournalWrite)?;
        self.journal.recovery = RecoveryState::Aborted;
        self.persist_journal()?;
        failpoints.check(RepositoryTxnStep::AfterAbortedJournalWrite)
    }

    #[cfg(test)]
    fn abort_prepared_at_failpoint(
        &mut self,
        step: RepositoryTxnStep,
    ) -> Result<(), RepositoryTxnError> {
        self.abort_prepared_with_failpoints(&mut FailAtRepositoryTxnStep { target: step })
    }

    pub(crate) fn install(&mut self) -> Result<(), RepositoryTxnError> {
        self.install_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    fn install_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
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
                    failpoints
                        .check(RepositoryTxnStep::BeforeCommittedConflictJournalWrite { index })?;
                    self.persist_journal()?;
                    failpoints
                        .check(RepositoryTxnStep::AfterCommittedConflictJournalWrite { index })?;
                    return Err(RepositoryTxnError::CommittedConflict {
                        path: write.path,
                        expected_preimage: Box::new(write.preimage),
                        expected_postimage: Box::new(write.postimage),
                        actual: Box::new(current),
                    });
                }
                failpoints.check(RepositoryTxnStep::BeforeInstallWrite { index })?;
                self.install_write(&write)?;
                failpoints.check(RepositoryTxnStep::AfterInstallWrite { index })?;
            }
            let installed = index + 1;
            self.journal.recovery = RecoveryState::Installing { installed, total };
            failpoints.check(RepositoryTxnStep::BeforeInstallingJournalWrite { index })?;
            self.persist_journal()?;
            failpoints.check(RepositoryTxnStep::AfterInstallingJournalWrite { index })?;
        }
        self.journal.recovery = RecoveryState::Installed;
        failpoints.check(RepositoryTxnStep::BeforeInstalledJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(RepositoryTxnStep::AfterInstalledJournalWrite)
    }

    #[cfg(test)]
    pub(crate) fn install_at_failpoint(
        &mut self,
        step: RepositoryTxnStep,
    ) -> Result<(), RepositoryTxnError> {
        self.install_with_failpoints(&mut FailAtRepositoryTxnStep { target: step })
    }

    pub(crate) fn complete(&mut self) -> Result<(), RepositoryTxnError> {
        self.complete_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    /// Retire private recovery copies after exact Git publication and strict
    /// repository verification have both succeeded.
    ///
    /// The durable plan, commit marker, read set, and file-state commitments
    /// remain intact. Only this completed transaction is marked pruned, and a
    /// blob file is removed only when no journal that still retains recovery
    /// bytes references it. The transaction lock remains held throughout, so
    /// another writer cannot acquire a reference between the scan and unlink.
    ///
    /// Callers deliberately invoke this as best-effort maintenance after the
    /// semantic operation is already published. An error must be reported as a
    /// diagnostic, never converted into a false operation failure.
    pub(crate) fn retire_completed_recovery_blobs(&mut self) -> Result<usize, RepositoryTxnError> {
        if !matches!(self.journal.recovery, RecoveryState::Completed) {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "cannot retire recovery blobs for transaction {} from {:?}",
                self.journal.plan.operation_id.as_str(),
                self.journal.recovery
            )));
        }
        self.verify_completed_state()?;

        let candidates = journal_blob_digests(&self.journal);
        if self.journal.blob_retention == BlobRetention::Retained {
            self.journal.blob_retention = BlobRetention::Pruned;
            self.persist_journal()?;
        }

        let journal_dir = self
            .paths
            .plan
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                RepositoryTxnError::Journal(format!(
                    "repository transaction path has no journal root: {}",
                    self.paths.plan.display()
                ))
            })?;
        let journals = repository_journals(&self.root, journal_dir)?;
        let retained = journals
            .iter()
            .filter(|(_, journal)| {
                journal.blob_retention == BlobRetention::Retained
                    || !matches!(journal.recovery, RecoveryState::Completed)
            })
            .flat_map(|(_, journal)| journal_blob_digests(journal))
            .collect::<BTreeSet<_>>();

        let mut removed = 0;
        for digest in candidates.difference(&retained) {
            operation_journal::remove(&self.paths.blob(digest))
                .map_err(RepositoryTxnError::Journal)?;
            removed += 1;
        }
        Ok(removed)
    }

    fn complete_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
        if !matches!(
            self.journal.recovery,
            RecoveryState::Installed | RecoveryState::Completed
        ) {
            return Err(RepositoryTxnError::CorruptPlan(
                "cannot complete a transaction before all writes are installed".to_string(),
            ));
        }
        if matches!(self.journal.recovery, RecoveryState::Completed) {
            self.verify_completed_state()?;
            return Ok(());
        }
        failpoints.check(RepositoryTxnStep::BeforeInstalledStateVerification)?;
        self.verify_installed_state()?;
        failpoints.check(RepositoryTxnStep::AfterInstalledStateVerification)?;
        self.journal.recovery = RecoveryState::Completed;
        failpoints.check(RepositoryTxnStep::BeforeCompletedJournalWrite)?;
        self.persist_journal()?;
        failpoints.check(RepositoryTxnStep::AfterCompletedJournalWrite)
    }

    #[cfg(test)]
    fn complete_at_failpoint(&mut self, step: RepositoryTxnStep) -> Result<(), RepositoryTxnError> {
        self.complete_with_failpoints(&mut FailAtRepositoryTxnStep { target: step })
    }

    #[cfg(test)]
    pub(crate) fn recover(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<RecoveryOutcome, RepositoryTxnError> {
        let mut txn = Self::open(repository_root, journal_dir, operation_id)?;
        if matches!(txn.journal.recovery, RecoveryState::Aborted) {
            return Ok(RecoveryOutcome::Aborted);
        }
        if matches!(txn.journal.recovery, RecoveryState::Completed) {
            return Ok(RecoveryOutcome::AlreadyCompleted);
        }
        match read_commit_marker(&txn.paths, &txn.journal) {
            Ok(_) => {}
            Err(RepositoryTxnError::NotCommitted)
                if matches!(txn.journal.recovery, RecoveryState::Prepared) =>
            {
                return Ok(RecoveryOutcome::Prepared);
            }
            Err(RepositoryTxnError::NotCommitted) => {
                return Err(RepositoryTxnError::CorruptPlan(format!(
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

    pub(crate) fn resolved_public_writes(&self) -> Result<Vec<ResolvedWrite>, RepositoryTxnError> {
        resolve_public_writes(&self.journal.plan.canonical_delta, |blob| {
            match self.journal.blob_retention {
                BlobRetention::Retained => self.read_blob(blob),
                BlobRetention::Pruned => self.read_pruned_blob_from_current(blob),
            }
        })
    }

    pub(crate) fn canonical_delta_root(&self) -> &str {
        self.journal.plan.canonical_delta.root().as_str()
    }

    fn install_write(&self, write: &StagedWrite) -> Result<(), RepositoryTxnError> {
        let target = write.path.target(&self.root)?;
        match &write.postimage {
            FileState::Absent => atomic_delete(&self.root, &target),
            FileState::File { mode, .. } => {
                let blob = write.payload.as_ref().ok_or_else(|| {
                    RepositoryTxnError::CorruptPlan(format!(
                        "file postimage {} has no blob reference",
                        write.path.as_str()
                    ))
                })?;
                let bytes = self.read_blob(blob)?;
                atomic_write(&self.root, &target, &bytes, *mode)
            }
        }
    }

    fn verify_marker(&self, marker: &CommitMarker) -> Result<(), RepositoryTxnError> {
        let expected = CommitMarker::from_plan(&self.journal.plan);
        if marker != &expected {
            return Err(RepositoryTxnError::CorruptPlan(
                "commit marker does not match the durable plan".to_string(),
            ));
        }
        Ok(())
    }

    fn verify_blobs(&self) -> Result<(), RepositoryTxnError> {
        verify_journal_blobs(&self.paths, &self.journal)
    }

    fn verify_recovery_blobs(&self) -> Result<(), RepositoryTxnError> {
        match self.journal.blob_retention {
            BlobRetention::Retained => self.verify_blobs(),
            BlobRetention::Pruned if matches!(self.journal.recovery, RecoveryState::Completed) => {
                Ok(())
            }
            BlobRetention::Pruned => Err(RepositoryTxnError::CorruptPlan(format!(
                "transaction {} pruned recovery blobs before completion",
                self.journal.plan.operation_id.as_str()
            ))),
        }
    }

    fn read_blob(&self, expected: &JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError> {
        read_blob_at(&self.paths, expected)
    }

    fn read_pruned_blob_from_current(
        &self,
        expected: &JournalBlobRef,
    ) -> Result<Vec<u8>, RepositoryTxnError> {
        for write in self.journal.plan.canonical_delta.writes() {
            if write.payload.as_ref() != Some(expected) {
                continue;
            }
            let target = validate_target(&self.root, &write.path)?;
            let metadata = match fs::symlink_metadata(&target) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(RepositoryTxnError::Io(format!(
                        "inspect pruned journal postimage {}: {error}",
                        target.display()
                    )));
                }
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(RepositoryTxnError::UnsafeTarget {
                    path: write.path.clone(),
                    reason: "pruned journal postimage is not a regular, non-symlink file"
                        .to_string(),
                });
            }
            let bytes = fs::read(&target).map_err(|error| {
                RepositoryTxnError::Io(format!(
                    "read pruned journal postimage {}: {error}",
                    target.display()
                ))
            })?;
            if validate_blob_bytes(expected, &bytes).is_ok() {
                return Ok(bytes);
            }
        }
        Err(RepositoryTxnError::MissingBlob(expected.digest.clone()))
    }

    fn verify_installed_state(&self) -> Result<(), RepositoryTxnError> {
        read_commit_marker(&self.paths, &self.journal)?;
        self.verify_blobs()?;
        for write in self.journal.plan.canonical_delta.writes() {
            let actual = inspect_file_state(&self.root, &write.path)?;
            if actual != write.postimage {
                return Err(RepositoryTxnError::CompletedPostimageMismatch {
                    operation_id: self.journal.plan.operation_id.as_str().to_string(),
                    path: write.path.clone(),
                    expected: Box::new(write.postimage.clone()),
                    actual: Box::new(actual),
                });
            }
        }
        Ok(())
    }

    fn verify_completed_state(&self) -> Result<(), RepositoryTxnError> {
        let journal_dir = self
            .paths
            .plan
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                RepositoryTxnError::Journal(format!(
                    "repository transaction path has no journal root: {}",
                    self.paths.plan.display()
                ))
            })?;
        ensure_recovery_barrier_locked(&self.root, journal_dir, None)
    }

    fn persist_journal(&self) -> Result<(), RepositoryTxnError> {
        operation_journal::write_json(&self.paths.plan, &self.journal)
            .map_err(RepositoryTxnError::Journal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepositoryTxnError {
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
    PortablePathCollision {
        first: String,
        second: String,
    },
    RepositoryBindingMismatch {
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
    WriteAuthorizationRequired {
        operation_id: String,
    },
    #[cfg(test)]
    WriteAuthorizationNotApplicable {
        state: RecoveryState,
    },
    WriteAuthorizationRepositoryMismatch {
        authorized: String,
        planned: String,
    },
    WriteAuthorizationDeltaMismatch {
        authorized: ContentDigest,
        planned: ContentDigest,
    },
    RepositoryWriteIntentDenied {
        intent: &'static str,
        reason: String,
    },
    StaleWriteAuthorization {
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
    MissingBlob(ContentDigest),
    CorruptBlob(ContentDigest),
    NotCommitted,
    Busy,
    #[cfg(test)]
    InjectedFailure {
        step: RepositoryTxnStep,
    },
    Canonicalize(String),
    RepositoryTrustAnchor(String),
    CorruptPlan(String),
    Journal(String),
    Io(String),
}

impl fmt::Display for RepositoryTxnError {
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
            Self::PortablePathCollision { first, second } => write!(
                formatter,
                "staged paths {first:?} and {second:?} collide under portable Unicode/case normalization"
            ),
            Self::RepositoryBindingMismatch { expected, actual } => write!(
                formatter,
                "repository binding mismatch: expected {expected}, found {actual}"
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
                "repository transaction {operation_id} requires recovery from {state:?} before another operation can plan or commit"
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
                "repository input {name} changed before commit at {}",
                path.as_str()
            ),
            Self::StaleSnapshot { name, .. } => {
                write!(
                    formatter,
                    "repository snapshot {name} changed before commit"
                )
            }
            Self::WriteAuthorizationRequired { operation_id } => write!(
                formatter,
                "repository transaction {operation_id} is Prepared without an in-memory canonical write authorization; explicitly reauthorize before commit"
            ),
            #[cfg(test)]
            Self::WriteAuthorizationNotApplicable { state } => write!(
                formatter,
                "canonical write reauthorization applies only to a marker-free Prepared transaction, found {state:?}"
            ),
            Self::WriteAuthorizationRepositoryMismatch {
                authorized,
                planned,
            } => write!(
                formatter,
                "repository write authorization for repository {authorized} cannot authorize transaction plan for {planned}"
            ),
            Self::WriteAuthorizationDeltaMismatch {
                authorized,
                planned,
            } => write!(
                formatter,
                "repository write authorization for delta {} cannot authorize transaction delta {}",
                authorized.as_str(),
                planned.as_str()
            ),
            Self::RepositoryWriteIntentDenied { intent, reason } => write!(
                formatter,
                "repository_write_intent_denied: {intent}: {reason}"
            ),
            Self::StaleWriteAuthorization { expected, actual } => write!(
                formatter,
                "repository write authorization drifted before commit: expected {}, found {}",
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
                "completed repository transaction {operation_id} has a missing or corrupt postimage at {}",
                path.as_str()
            ),
            Self::MissingBlob(digest) => {
                write!(formatter, "missing transaction blob {}", digest.as_str())
            }
            Self::CorruptBlob(digest) => {
                write!(formatter, "corrupt transaction blob {}", digest.as_str())
            }
            Self::NotCommitted => write!(formatter, "repository transaction has no commit marker"),
            Self::Busy => write!(
                formatter,
                "another repository transaction holds the write lock"
            ),
            #[cfg(test)]
            Self::InjectedFailure { step } => {
                write!(
                    formatter,
                    "injected repository transaction failure at {step:?}"
                )
            }
            Self::Canonicalize(error) => write!(formatter, "canonicalize transaction: {error}"),
            Self::RepositoryTrustAnchor(error) => {
                write!(formatter, "repository_trust_anchor_invalid: {error}")
            }
            Self::CorruptPlan(error) => write!(formatter, "corrupt transaction plan: {error}"),
            Self::Journal(error) => write!(formatter, "repository transaction journal: {error}"),
            Self::Io(error) => write!(formatter, "repository transaction I/O: {error}"),
        }
    }
}

impl std::error::Error for RepositoryTxnError {}

fn canonical_repository_root(path: &Path) -> Result<PathBuf, RepositoryTxnError> {
    let metadata = fs::metadata(path).map_err(|error| {
        RepositoryTxnError::Io(format!("read repository root {}: {error}", path.display()))
    })?;
    if !metadata.is_dir() {
        return Err(RepositoryTxnError::Io(format!(
            "repository root is not a directory: {}",
            path.display()
        )));
    }
    fs::canonicalize(path).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "canonicalize repository root {}: {error}",
            path.display()
        ))
    })
}

/// Reject path escapes, symbolic links, and non-directory ancestors observed
/// while resolving a transaction target.
///
/// This is a fail-closed check for a stable filesystem plus Vela's cooperative
/// repository lock; it is not a sandbox against a hostile process that can
/// mutate the repository with the same operating-system permissions. Rust's
/// portable `std::fs` path APIs do not provide a complete dirfd-relative,
/// no-follow rename/unlink walk, and this crate denies unsafe code. A hostile
/// local process can therefore race an ancestor rename between this check and
/// a later path-based read, rename, or unlink. Every preflight and install
/// rechecks the path and refuses observed drift, but deployments that require
/// protection from such a process must protect the repository directory with OS
/// ownership/permissions. Do not describe this function as eliminating that
/// TOCTOU boundary.
fn validate_target(root: &Path, path: &RepoPath) -> Result<PathBuf, RepositoryTxnError> {
    let mut current = root.to_path_buf();
    let segments = path.as_str().split('/').collect::<Vec<_>>();
    for (index, segment) in segments.iter().enumerate() {
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(RepositoryTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is a symbolic link", current.display()),
                    });
                }
                let is_target = index + 1 == segments.len();
                if !is_target && !metadata.is_dir() {
                    return Err(RepositoryTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is not a directory", current.display()),
                    });
                }
                if is_target && !metadata.is_file() {
                    return Err(RepositoryTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!("{} is not a regular file", current.display()),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "inspect transaction target {}: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(root.join(path.as_str()))
}

fn inspect_file_state(root: &Path, path: &RepoPath) -> Result<FileState, RepositoryTxnError> {
    let target = validate_target(root, path)?;
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(FileState::Absent),
        Err(error) => {
            return Err(RepositoryTxnError::Io(format!(
                "inspect transaction target {}: {error}",
                target.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RepositoryTxnError::UnsafeTarget {
            path: path.clone(),
            reason: "target is not a regular, non-symlink file".to_string(),
        });
    }
    let bytes = fs::read(&target).map_err(|error| {
        RepositoryTxnError::Io(format!(
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

fn inspect_directory_state(
    root: &Path,
    path: &RepoPath,
) -> Result<RepositoryDirectoryState, RepositoryTxnError> {
    let mut current = root.to_path_buf();
    for segment in path.as_str().split('/') {
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(RepositoryTxnError::UnsafeTarget {
                        path: path.clone(),
                        reason: format!(
                            "{} is not a regular, non-symlink directory",
                            current.display()
                        ),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RepositoryDirectoryState::Absent);
            }
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "inspect transaction input directory {}: {error}",
                    current.display()
                )));
            }
        }
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&current).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "read transaction input directory {}: {error}",
            current.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            RepositoryTxnError::Io(format!(
                "enumerate transaction input directory {}: {error}",
                current.display()
            ))
        })?;
        let name = entry.file_name().into_string().map_err(|_| {
            RepositoryTxnError::Io(format!(
                "transaction input directory contains a non-UTF-8 entry: {}",
                current.display()
            ))
        })?;
        let entry_path = RepoPath::parse(format!("{}/{}", path.as_str(), name))?;
        let state = inspect_file_state(root, &entry_path)?;
        entries.push(DirectoryEntryState {
            path: entry_path,
            state,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RepositoryDirectoryState::Directory { entries })
}

fn file_mode(metadata: &fs::Metadata) -> FileMode {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            return FileMode::Executable;
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    FileMode::Regular
}

fn ensure_parent_dirs(root: &Path, parent: &Path) -> Result<(), RepositoryTxnError> {
    let relative = parent.strip_prefix(root).map_err(|_| {
        RepositoryTxnError::Io(format!(
            "transaction parent {} escaped repository {}",
            parent.display(),
            root.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(RepositoryTxnError::Io(format!(
                "transaction parent is not normalized: {}",
                parent.display()
            )));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(RepositoryTxnError::Io(format!(
                    "transaction parent is not a safe directory: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let previous = current.parent().expect("repository child has a parent");
                fs::create_dir(&current).map_err(|error| {
                    RepositoryTxnError::Io(format!(
                        "create transaction directory {}: {error}",
                        current.display()
                    ))
                })?;
                sync_directory(previous)?;
            }
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
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
) -> Result<(), RepositoryTxnError> {
    let parent = target.parent().ok_or_else(|| {
        RepositoryTxnError::Io(format!(
            "transaction target has no parent: {}",
            target.display()
        ))
    })?;
    ensure_parent_dirs(root, parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "create transaction temporary file in {}: {error}",
            parent.display()
        ))
    })?;
    temporary.write_all(bytes).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "write transaction postimage {}: {error}",
            target.display()
        ))
    })?;
    set_mode(temporary.as_file(), mode)?;
    temporary.as_file().sync_all().map_err(|error| {
        RepositoryTxnError::Io(format!(
            "fsync transaction postimage {}: {error}",
            target.display()
        ))
    })?;
    temporary.persist(target).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "install transaction postimage {}: {}",
            target.display(),
            error.error
        ))
    })?;
    sync_directory(parent)
}

fn atomic_delete(root: &Path, target: &Path) -> Result<(), RepositoryTxnError> {
    let parent = target.parent().ok_or_else(|| {
        RepositoryTxnError::Io(format!(
            "transaction target has no parent: {}",
            target.display()
        ))
    })?;
    ensure_parent_dirs(root, parent)?;
    match fs::remove_file(target) {
        Ok(()) => sync_directory(parent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RepositoryTxnError::Io(format!(
            "delete transaction target {}: {error}",
            target.display()
        ))),
    }
}

fn set_mode(file: &File, mode: FileMode) -> Result<(), RepositoryTxnError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(match mode {
            FileMode::Regular => 0o644,
            FileMode::Executable => 0o755,
        });
        file.set_permissions(permissions).map_err(|error| {
            RepositoryTxnError::Io(format!("set transaction file mode: {error}"))
        })?;
    }
    #[cfg(not(unix))]
    let _ = (file, mode);
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), RepositoryTxnError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            RepositoryTxnError::Io(format!(
                "fsync transaction directory {}: {error}",
                path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture_root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn fixture_authority_initialization_write(repository_id: &str) -> PlannedWrite {
        let event = vela_protocol::authority::AuthorityEventV1::new(
            vela_protocol::authority::AuthorityEventContentV1 {
                transaction_id: "vtx_fixture_initialization".into(),
                principal_id: "local:fixture".into(),
                authority_mode: vela_protocol::authority::AUTHORITY_MODE.into(),
                kind: vela_protocol::events::EventKind::Other(
                    vela_protocol::authority_history::AUTHORITY_INITIALIZED_EVENT_KIND.into(),
                ),
                target: vela_protocol::events::StateTarget {
                    r#type: "repository".into(),
                    id: repository_id.into(),
                },
                actor: vela_protocol::events::StateActor {
                    r#type: "human".into(),
                    id: "local:fixture".into(),
                },
                timestamp: "2026-07-29T00:00:00Z".into(),
                reason: "Initialize exact repository authority fixture.".into(),
                before_hash: vela_protocol::events::NULL_HASH.into(),
                after_hash: vela_protocol::events::NULL_HASH.into(),
                payload: json!({}),
                caveats: Vec::new(),
            },
        )
        .unwrap();
        PlannedWrite::write(
            RepoPath::parse(format!(".vela/authority/events/{}.json", event.id)).unwrap(),
            WriteClass::Authority,
            vela_protocol::canonical::to_canonical_bytes(&event).unwrap(),
        )
    }

    fn fixture_covering_authority_record_write() -> PlannedWrite {
        PlannedWrite::write(
            RepoPath::parse(".vela/authority/records/var_fixture.dsse.json").unwrap(),
            WriteClass::Authority,
            b"fixture covering record".to_vec(),
        )
    }

    fn fixture_repository(
        origin: &vela_protocol::repository_origin::RepositoryOriginV1,
    ) -> vela_protocol::repository::RepositoryV4 {
        vela_protocol::repository::RepositoryV4 {
            schema: vela_protocol::repository::REPOSITORY_SCHEMA_V4.into(),
            repository_id: origin.repository_id.clone(),
            profile_root: origin.profile_root.clone(),
            origin_id: origin.id().unwrap(),
            origin_root: origin.canonical_root().unwrap(),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: fixture_root('a'),
            authority_model_root: fixture_root('b'),
        }
    }

    fn verify_fresh_fixture(
        root: &Path,
        origin: &vela_protocol::repository_origin::RepositoryOriginV1,
    ) -> Result<(), RepositoryTxnError> {
        let repository = fixture_repository(origin);
        let writes = vec![
            PlannedWrite::write(
                RepoPath::parse(".vela/origin.json").unwrap(),
                WriteClass::CanonicalEvidence,
                origin.canonical_bytes().unwrap(),
            ),
            PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                repository.canonical_bytes().unwrap(),
            ),
            fixture_authority_initialization_write(&origin.repository_id),
            fixture_covering_authority_record_write(),
        ];
        let draft = DeltaDraft::prepare(root, writes).unwrap();
        verify_fresh_repository_delta(&draft.delta, |blob| {
            draft.blobs.get(&blob.digest).cloned().ok_or_else(|| {
                RepositoryTxnError::CorruptPlan(format!(
                    "fixture has no blob {}",
                    blob.digest.as_str()
                ))
            })
        })
    }

    #[test]
    fn fresh_repository_delta_accepts_only_empty_genesis_origin() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repository_id = "01234567-89ab-4def-8123-456789abcdef";
        let profile_root = fixture_root('c');
        let genesis = vela_protocol::repository_origin::RepositoryOriginV1::genesis(
            repository_id.into(),
            profile_root.clone(),
            "Establish current repository authority.".into(),
        )
        .unwrap();
        verify_fresh_fixture(root, &genesis).unwrap();

        /* Compaction is gone, so an origin claiming a generation beyond the
        first is an object this runtime does not define. It fails at the
        origin's own reader rather than at the fresh-repository check. */
        let mut beyond_genesis = genesis.clone();
        beyond_genesis.generation = 2;
        assert!(beyond_genesis.verify().is_err());
    }

    fn fixture_plan(root: &Path, draft: &DeltaDraft, identity: &[u8]) -> RepositoryTxnPlan {
        let operation_id = OperationId::derive("submission", identity);
        let request_root = ContentDigest::hash(identity);
        let repository_id = crate::repository::verify_repository_at(root, false)
            .map(|repository| repository.repository_id)
            .unwrap_or_else(|_| "33333333-3333-4333-8333-333333333333".to_string());
        RepositoryTxnPlan::new(
            RepositoryTxnPlanSpec {
                kind: OperationKind::Submission,
                operation_id,
                request_root,
                repository: RepositoryBinding::new(root, repository_id).unwrap(),
                fixed_time: "2026-07-13T00:00:00Z".to_string(),
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

    #[test]
    fn operating_system_account_home_ignores_hostile_home_environment() {
        const CHILD: &str = "VELA_OS_ACCOUNT_HOME_REDIRECTION_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let hostile = fs::canonicalize(
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .expect("child HOME is set"),
            )
            .unwrap();
            let actual = fs::canonicalize(operating_system_account_home().unwrap()).unwrap();
            assert_ne!(actual, hostile, "hostile HOME redirected trust-pin lookup");
            return;
        }

        let attacker_home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("repository_txn::tests::operating_system_account_home_ignores_hostile_home_environment")
            .arg("--nocapture")
            .env(CHILD, "1")
            .env("HOME", attacker_home.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "hostile-HOME child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn initialize_failpoint_repository(root: &Path) {
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
                RepoPath::parse("repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"materialized repository".to_vec(),
            ),
            PlannedWrite::delete(
                RepoPath::parse("obsolete.json").unwrap(),
                WriteClass::PublicReview,
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
                "repository.json".to_string(),
                b"materialized repository".to_vec(),
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

    fn assert_injected<T>(result: Result<T, RepositoryTxnError>, expected: RepositoryTxnStep) {
        match result {
            Err(RepositoryTxnError::InjectedFailure { step }) => assert_eq!(step, expected),
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
            RepositoryTxn::recover(root, journals, operation_id).unwrap(),
            RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted
        ));
        assert_eq!(snapshot_files(root), expected_failpoint_postimage());
        let first_recovery = snapshot_files(root);
        assert_eq!(
            RepositoryTxn::recover(root, journals, operation_id).unwrap(),
            RecoveryOutcome::AlreadyCompleted
        );
        assert_eq!(
            snapshot_files(root),
            first_recovery,
            "a completed recovery must be byte-idempotent"
        );
    }

    #[test]
    fn repository_txn_rejects_unsafe_paths_and_symlink_ancestors() {
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
            "records/cafe\u{301}.json",
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
            let root = temp.path().join("repository");
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
            assert!(matches!(error, RepositoryTxnError::UnsafeTarget { .. }));
        }
    }

    #[test]
    fn open_if_present_returns_none_only_for_an_absent_journal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let operation_id = OperationId::derive("submission", b"absent request");

        assert!(
            RepositoryTxn::open_if_present(&root, &journals, &operation_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn open_if_present_exposes_request_identity_and_resumes_marker_window() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        let marker = CommitMarker::from_plan(txn.plan());
        operation_journal::write_json(&txn.paths.marker, &marker).unwrap();
        assert_eq!(txn.recovery_state(), &RecoveryState::Prepared);
        drop(txn);

        let mut reopened = RepositoryTxn::open_if_present(&root, &journals, &operation_id)
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
    fn canonical_delta_is_sorted_unique_and_root_bound() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("existing.json"), b"before").unwrap();
        let writes = || {
            vec![
                PlannedWrite::write(
                    RepoPath::parse("z.json").unwrap(),
                    WriteClass::PublicReview,
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
                    WriteClass::PublicReview,
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
        assert!(matches!(duplicate, RepositoryTxnError::DuplicatePath(_)));

        let portable_collision = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse("records/Foo.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    vec![1],
                ),
                PlannedWrite::write(
                    RepoPath::parse("records/foo.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    vec![2],
                ),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            portable_collision,
            RepositoryTxnError::PortablePathCollision { .. }
        ));
    }

    #[test]
    fn journal_v2_rejects_v1_and_retired_event_fields() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(&root, vec![]).unwrap();
        let plan = fixture_plan(&root, &draft, b"journal v2 schema");

        let mut retired = serde_json::to_value(&plan).unwrap();
        retired["expected_event_log_root"] = json!(fixture_root('1'));
        assert!(serde_json::from_value::<RepositoryTxnPlan>(retired).is_err());

        let mut v1 = serde_json::to_value(&plan).unwrap();
        v1["schema"] = json!("vela.repository-txn.internal.v1");
        assert!(matches!(
            serde_json::from_value::<RepositoryTxnPlan>(v1)
                .unwrap()
                .verify(),
            Err(RepositoryTxnError::CorruptPlan(message))
                if message.contains("unexpected repository transaction schema")
        ));

        let marker = CommitMarker::from_plan(&plan);
        let mut retired_marker = serde_json::to_value(marker).unwrap();
        retired_marker["resulting_event_log_root"] = json!(fixture_root('2'));
        assert!(serde_json::from_value::<CommitMarker>(retired_marker).is_err());
    }

    #[test]
    fn pre_marker_failpoints_leave_zero_repository_delta_and_retry_exactly() {
        let blob_count = 5;
        let mut prepare_failpoints = Vec::new();
        for index in 0..blob_count {
            prepare_failpoints.push(RepositoryTxnStep::BeforeBlobJournalWrite { index });
            prepare_failpoints.push(RepositoryTxnStep::AfterBlobJournalWrite { index });
        }
        prepare_failpoints.extend([
            RepositoryTxnStep::BeforePreparedJournalWrite,
            RepositoryTxnStep::AfterPreparedJournalWrite,
        ]);

        for step in prepare_failpoints {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("repository");
            let journals = temp.path().join("journals");
            initialize_failpoint_repository(&root);
            let before = snapshot_files(&root);
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            assert_eq!(draft.blobs.len(), blob_count);
            let plan = fixture_plan(&root, &draft, format!("prepare {step:?}").as_bytes());
            let operation_id = plan.operation_id.clone();
            let paths = RepositoryTxnPaths::new(&journals, &operation_id);

            assert_injected(
                RepositoryTxn::prepare_at_failpoint(&root, &journals, plan, draft, step),
                step,
            );

            assert_eq!(
                snapshot_files(&root),
                before,
                "pre-marker failpoint {step:?} changed the repository"
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
                let retry = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
                assert_eq!(
                    retry.plan().root(),
                    retry_plan.root(),
                    "a durable Prepared journal must bind the exact retry plan"
                );
                retry
            } else {
                RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap()
            };
            if retry.authorization.is_none() {
                retry.reauthorize_prepared_for_test().unwrap();
            }
            retry.mark_committed().unwrap();
            retry.install().unwrap();
            retry.complete().unwrap();
            drop(retry);
            assert_eq!(snapshot_files(&root), expected_failpoint_postimage());
        }

        // Aborting a marker-free plan is itself a durable journal transition.
        // A failure on either side of that atomic replacement must still leave
        // zero repository delta, no marker, and a retryable operation identity.
        for step in [
            RepositoryTxnStep::BeforeAbortedJournalWrite,
            RepositoryTxnStep::AfterAbortedJournalWrite,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("repository");
            let journals = temp.path().join("journals");
            initialize_failpoint_repository(&root);
            let before = snapshot_files(&root);
            let identity = format!("abort {step:?}");
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            let plan = fixture_plan(&root, &draft, identity.as_bytes());
            let operation_id = plan.operation_id.clone();
            let paths = RepositoryTxnPaths::new(&journals, &operation_id);
            let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

            assert_injected(txn.abort_prepared_at_failpoint(step), step);
            assert_eq!(
                snapshot_files(&root),
                before,
                "pre-marker abort failpoint {step:?} changed the repository"
            );
            assert!(
                !paths.marker.exists(),
                "pre-marker abort failpoint {step:?} wrote a commit marker"
            );
            drop(txn);

            let mut reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
            match step {
                RepositoryTxnStep::BeforeAbortedJournalWrite => {
                    assert_eq!(reopened.recovery_state(), &RecoveryState::Prepared);
                    reopened.abort_prepared().unwrap();
                }
                RepositoryTxnStep::AfterAbortedJournalWrite => {
                    assert_eq!(reopened.recovery_state(), &RecoveryState::Aborted);
                }
                _ => unreachable!(),
            }
            drop(reopened);

            let retry_draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            let retry_plan = fixture_plan(&root, &retry_draft, identity.as_bytes());
            let mut retry =
                RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
            retry.mark_committed().unwrap();
            retry.install().unwrap();
            retry.complete().unwrap();
            drop(retry);
            assert_eq!(snapshot_files(&root), expected_failpoint_postimage());
        }

        // A safely injected marker-write error occurs before the atomic,
        // fsync-backed journal replacement. The old state is therefore a
        // complete Prepared journal with no marker and no repository delta.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        initialize_failpoint_repository(&root);
        let before = snapshot_files(&root);
        let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
        let plan = fixture_plan(&root, &draft, b"marker write failure");
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        let step = RepositoryTxnStep::BeforeCommitMarkerWrite;
        assert_injected(txn.mark_committed_at_failpoint(step), step);
        assert_eq!(snapshot_files(&root), before);
        assert!(!txn.paths.marker.exists());
        drop(txn);
        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Prepared
        );
        assert_eq!(snapshot_files(&root), before);
    }

    #[test]
    fn reused_operation_id_with_changed_request_is_rejected_after_abort_and_completion() {
        for terminal_state in [RecoveryState::Aborted, RecoveryState::Completed] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("repository");
            let journals = temp.path().join("journals");
            fs::create_dir_all(&root).unwrap();
            let identity = format!("operation collision {terminal_state:?}");
            let original_draft = DeltaDraft::prepare(
                &root,
                vec![PlannedWrite::write(
                    RepoPath::parse("records/original.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"original".to_vec(),
                )],
            )
            .unwrap();
            let original_plan = fixture_plan(&root, &original_draft, identity.as_bytes());
            let operation_id = original_plan.operation_id.clone();
            let mut original =
                RepositoryTxn::prepare(&root, &journals, original_plan, original_draft).unwrap();
            match terminal_state {
                RecoveryState::Aborted => original.abort_prepared().unwrap(),
                RecoveryState::Completed => {
                    original.mark_committed().unwrap();
                    original.install().unwrap();
                    original.complete().unwrap();
                }
                _ => unreachable!(),
            }
            drop(original);

            let changed_draft = DeltaDraft::prepare(
                &root,
                vec![PlannedWrite::write(
                    RepoPath::parse("records/changed.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"changed".to_vec(),
                )],
            )
            .unwrap();
            let mut changed_plan = fixture_plan(&root, &changed_draft, identity.as_bytes());
            assert_eq!(changed_plan.operation_id, operation_id);
            changed_plan.request_root = ContentDigest::hash(b"different normalized request");
            changed_plan.root = changed_plan.compute_root().unwrap();

            assert!(matches!(
                RepositoryTxn::prepare(&root, &journals, changed_plan, changed_draft),
                Err(RepositoryTxnError::OperationConflict {
                    operation_id: conflict
                }) if conflict == operation_id.as_str()
            ));
        }
    }

    #[test]
    fn post_marker_failpoints_recover_the_exact_delta_idempotently() {
        let mut failpoints = vec![
            RepositoryTxnStep::AfterCommitMarkerWrite,
            // This is the durable-marker/Prepared-journal window produced by
            // a committed-journal write failure.
            RepositoryTxnStep::BeforeCommittedJournalWrite,
            RepositoryTxnStep::AfterCommittedJournalWrite,
        ];
        for index in 0..failpoint_writes().len() {
            failpoints.extend([
                RepositoryTxnStep::BeforeInstallWrite { index },
                RepositoryTxnStep::AfterInstallWrite { index },
                RepositoryTxnStep::BeforeInstallingJournalWrite { index },
                RepositoryTxnStep::AfterInstallingJournalWrite { index },
            ]);
        }
        failpoints.extend([
            RepositoryTxnStep::BeforeInstalledJournalWrite,
            RepositoryTxnStep::AfterInstalledJournalWrite,
            RepositoryTxnStep::BeforeInstalledStateVerification,
            RepositoryTxnStep::AfterInstalledStateVerification,
            RepositoryTxnStep::BeforeCompletedJournalWrite,
            RepositoryTxnStep::AfterCompletedJournalWrite,
        ]);

        for step in failpoints {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("repository");
            let journals = temp.path().join("journals");
            initialize_failpoint_repository(&root);
            let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
            assert!(draft.delta.writes().iter().any(|write| {
                write.class == WriteClass::CanonicalEvidence
                    && matches!(write.postimage, FileState::File { .. })
            }));
            assert!(draft.delta.writes().iter().any(|write| {
                write.class == WriteClass::PublicReview
                    && matches!(write.postimage, FileState::Absent)
            }));
            let plan = fixture_plan(&root, &draft, format!("post marker {step:?}").as_bytes());
            let operation_id = plan.operation_id.clone();
            let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

            let result = match step {
                RepositoryTxnStep::AfterCommitMarkerWrite
                | RepositoryTxnStep::BeforeCommittedJournalWrite
                | RepositoryTxnStep::AfterCommittedJournalWrite => {
                    txn.mark_committed_at_failpoint(step)
                }
                RepositoryTxnStep::BeforeInstallWrite { .. }
                | RepositoryTxnStep::AfterInstallWrite { .. }
                | RepositoryTxnStep::BeforeInstallingJournalWrite { .. }
                | RepositoryTxnStep::AfterInstallingJournalWrite { .. }
                | RepositoryTxnStep::BeforeInstalledJournalWrite
                | RepositoryTxnStep::AfterInstalledJournalWrite => {
                    txn.mark_committed().unwrap();
                    txn.install_at_failpoint(step)
                }
                RepositoryTxnStep::BeforeInstalledStateVerification
                | RepositoryTxnStep::AfterInstalledStateVerification
                | RepositoryTxnStep::BeforeCompletedJournalWrite
                | RepositoryTxnStep::AfterCompletedJournalWrite => {
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
    fn committed_conflict_journal_failpoints_preserve_drift_and_recover_after_repair() {
        for index in 0..failpoint_writes().len() {
            for step in [
                RepositoryTxnStep::BeforeCommittedConflictJournalWrite { index },
                RepositoryTxnStep::AfterCommittedConflictJournalWrite { index },
            ] {
                let temp = tempfile::tempdir().unwrap();
                let root = temp.path().join("repository");
                let journals = temp.path().join("journals");
                initialize_failpoint_repository(&root);
                let draft = DeltaDraft::prepare(&root, failpoint_writes()).unwrap();
                let conflicted_write = draft.delta.writes()[index].clone();
                let conflicted_target = conflicted_write.path.target(&root).unwrap();
                let original_bytes = fs::read(&conflicted_target).ok();
                let plan = fixture_plan(&root, &draft, format!("conflict {step:?}").as_bytes());
                let operation_id = plan.operation_id.clone();
                let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
                txn.mark_committed().unwrap();

                fs::create_dir_all(conflicted_target.parent().unwrap()).unwrap();
                fs::write(&conflicted_target, b"third-party drift").unwrap();
                assert_injected(txn.install_at_failpoint(step), step);
                assert_eq!(
                    fs::read(&conflicted_target).unwrap(),
                    b"third-party drift",
                    "conflict failpoint {step:?} overwrote external drift"
                );
                assert!(txn.paths.marker.exists());
                drop(txn);

                assert!(matches!(
                    RepositoryTxn::recover(&root, &journals, &operation_id),
                    Err(RepositoryTxnError::CommittedConflict { path, .. })
                        if path == conflicted_write.path
                ));
                assert_eq!(fs::read(&conflicted_target).unwrap(), b"third-party drift");

                match original_bytes {
                    Some(bytes) => fs::write(&conflicted_target, bytes).unwrap(),
                    None => fs::remove_file(&conflicted_target).unwrap(),
                }
                assert_post_marker_recovery_is_exact(&root, &journals, &operation_id);
            }
        }
    }

    #[test]
    fn committed_install_is_idempotent_and_recovers_after_failpoint() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
                    RepoPath::parse("repository.json").unwrap(),
                    WriteClass::PublicReview,
                    b"repository".to_vec(),
                ),
            ],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"recoverable request");
        let operation_id = plan.operation_id.clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        let step = RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 };
        let error = txn.install_at_failpoint(step).unwrap_err();
        assert!(matches!(
            error,
            RepositoryTxnError::InjectedFailure { step: actual } if actual == step
        ));
        drop(txn);

        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        let reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
        assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
        drop(reopened);
        assert_eq!(
            fs::read(root.join("records/receipt.json")).unwrap(),
            b"receipt"
        );
        assert_eq!(
            fs::read(root.join("repository.json")).unwrap(),
            b"repository"
        );
        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::AlreadyCompleted,
            "replaying a completed transaction must remain idempotent"
        );
    }

    #[test]
    fn completed_recovery_blob_retirement_preserves_plan_marker_and_replay() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/submission.json").unwrap(),
                WriteClass::PublicReview,
                b"published submission".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"retire completed recovery blobs");
        let operation_id = plan.operation_id.clone();
        let paths = RepositoryTxnPaths::new(&journals, &operation_id);
        let blob = draft.delta.writes()[0]
            .payload
            .as_ref()
            .unwrap()
            .digest
            .clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();

        assert!(paths.plan.is_file());
        assert!(paths.marker.is_file());
        assert!(paths.blob(&blob).is_file());
        assert_eq!(txn.retire_completed_recovery_blobs().unwrap(), 1);
        assert!(paths.plan.is_file(), "the durable plan must remain");
        assert!(paths.marker.is_file(), "the commit marker must remain");
        assert!(!paths.blob(&blob).exists());
        let retained: RepositoryTxnJournal = operation_journal::read_json(&paths.plan).unwrap();
        assert_eq!(retained.recovery, RecoveryState::Completed);
        assert_eq!(retained.blob_retention, BlobRetention::Pruned);
        drop(txn);

        let reopened = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
        assert_eq!(reopened.recovery_state(), &RecoveryState::Completed);
        assert_eq!(
            reopened.resolved_public_writes().unwrap()[0]
                .postimage_bytes
                .as_deref(),
            Some(b"published submission".as_slice())
        );
        drop(reopened);
        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::AlreadyCompleted
        );
    }

    #[test]
    fn recovery_blobs_survive_a_crash_until_explicit_completed_retirement() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/verification.json").unwrap(),
                WriteClass::PublicReview,
                b"verified bytes".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"crash before completed retirement");
        let operation_id = plan.operation_id.clone();
        let paths = RepositoryTxnPaths::new(&journals, &operation_id);
        let blob = draft.delta.writes()[0]
            .payload
            .as_ref()
            .unwrap()
            .digest
            .clone();
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        let step = RepositoryTxnStep::AfterInstallWrite { index: 0 };
        assert!(matches!(
            txn.install_at_failpoint(step),
            Err(RepositoryTxnError::InjectedFailure { step: actual }) if actual == step
        ));
        assert!(paths.blob(&blob).is_file());
        assert!(matches!(
            txn.retire_completed_recovery_blobs(),
            Err(RepositoryTxnError::CorruptPlan(message))
                if message.contains("cannot retire recovery blobs")
        ));
        assert!(paths.blob(&blob).is_file());
        drop(txn);

        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        assert!(paths.blob(&blob).is_file());
        let mut recovered = RepositoryTxn::open(&root, &journals, &operation_id).unwrap();
        assert_eq!(recovered.retire_completed_recovery_blobs().unwrap(), 1);
        assert!(!paths.blob(&blob).exists());
    }

    #[test]
    fn shared_blob_is_removed_only_after_every_referencing_journal_is_pruned() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();

        let first_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/first.json").unwrap(),
                WriteClass::PublicReview,
                b"shared recovery bytes".to_vec(),
            )],
        )
        .unwrap();
        let shared_blob = first_draft.delta.writes()[0]
            .payload
            .as_ref()
            .unwrap()
            .digest
            .clone();
        let first_plan = fixture_plan(&root, &first_draft, b"first shared blob journal");
        let first_operation = first_plan.operation_id.clone();
        let first_paths = RepositoryTxnPaths::new(&journals, &first_operation);
        let mut first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
        first.mark_committed().unwrap();
        first.install().unwrap();
        first.complete().unwrap();
        drop(first);

        let second_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("records/second.json").unwrap(),
                WriteClass::PublicReview,
                b"shared recovery bytes".to_vec(),
            )],
        )
        .unwrap();
        assert_eq!(
            second_draft.delta.writes()[0]
                .payload
                .as_ref()
                .unwrap()
                .digest,
            shared_blob
        );
        let second_plan = fixture_plan(&root, &second_draft, b"second shared blob journal");
        let second_operation = second_plan.operation_id.clone();
        let second_paths = RepositoryTxnPaths::new(&journals, &second_operation);
        let mut second =
            RepositoryTxn::prepare(&root, &journals, second_plan, second_draft).unwrap();
        second.mark_committed().unwrap();
        second.install().unwrap();
        second.complete().unwrap();

        assert_eq!(second.retire_completed_recovery_blobs().unwrap(), 0);
        assert!(second_paths.blob(&shared_blob).is_file());
        drop(second);

        let mut first = RepositoryTxn::open(&root, &journals, &first_operation).unwrap();
        assert_eq!(first.retire_completed_recovery_blobs().unwrap(), 1);
        assert!(!first_paths.blob(&shared_blob).exists());
        drop(first);

        let second = RepositoryTxn::open(&root, &journals, &second_operation).unwrap();
        assert_eq!(second.recovery_state(), &RecoveryState::Completed);
    }

    #[test]
    fn incomplete_journal_is_a_repository_wide_recovery_barrier() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
                    RepoPath::parse("repository.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"first repository".to_vec(),
                ),
            ],
        )
        .unwrap();
        let first_plan = fixture_plan(&root, &first_draft, b"first operation");
        let first_operation = first_plan.operation_id.clone();
        let mut first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
        first.mark_committed().unwrap();
        let step = RepositoryTxnStep::AfterInstallingJournalWrite { index: 0 };
        assert!(matches!(
            first.install_at_failpoint(step),
            Err(RepositoryTxnError::InjectedFailure { step: actual }) if actual == step
        ));
        drop(first);

        let barrier_error = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap_err();
        assert!(matches!(
            barrier_error,
            RepositoryTxnError::RecoveryRequired {
                operation_id,
                state: RecoveryState::Installing {
                    installed: 1,
                    total: 2
                }
            } if operation_id == first_operation.as_str()
        ));
        assert!(matches!(
            RepositoryTxn::verify_recovery_barrier_read_only(&root, &journals),
            Err(RepositoryTxnError::RecoveryRequired { operation_id, .. })
                if operation_id == first_operation.as_str()
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
            RepositoryTxn::prepare(&root, &journals, second_plan, second_draft),
            Err(RepositoryTxnError::RecoveryRequired { operation_id, .. })
                if operation_id == first_operation.as_str()
        ));

        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &first_operation).unwrap(),
            RecoveryOutcome::Completed
        );
        let barrier = RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap();
        drop(barrier);
        RepositoryTxn::verify_recovery_barrier_read_only(&root, &journals).unwrap();
    }

    #[test]
    fn completed_journal_fails_closed_when_a_postimage_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        drop(txn);

        fs::remove_file(root.join("records/receipt.json")).unwrap();
        assert!(matches!(
            RepositoryTxn::open_if_present(&root, &journals, &operation_id),
            Err(RepositoryTxnError::CompletedPostimageMismatch {
                operation_id: corrupt_operation,
                ..
            }) if corrupt_operation == operation_id.as_str()
        ));
        assert!(matches!(
            RepositoryTxn::acquire_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::CompletedPostimageMismatch { .. })
        ));
    }

    #[test]
    fn completed_journal_allows_a_later_repository_head_but_not_evidence_drift() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(root.join(".vela")).unwrap();

        let draft = DeltaDraft::prepare(
            &root,
            vec![
                PlannedWrite::write(
                    RepoPath::parse(".vela/repository.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"authenticated repository head one".to_vec(),
                ),
                PlannedWrite::write(
                    RepoPath::parse("records/receipt.json").unwrap(),
                    WriteClass::CanonicalEvidence,
                    b"immutable receipt".to_vec(),
                ),
            ],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"rolling repository head");
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        txn.complete().unwrap();
        drop(txn);

        // A current object transaction may install a later, independently
        // verified repository head without participating in this legacy
        // journal generation.
        fs::write(
            root.join(".vela/repository.json"),
            b"authenticated repository head two",
        )
        .unwrap();
        drop(RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap());

        // The exception is only for the rolling repository head. Immutable
        // canonical evidence written by the same completed transaction remains
        // byte-exact and fails closed if altered.
        fs::write(root.join("records/receipt.json"), b"altered receipt").unwrap();
        assert!(matches!(
            RepositoryTxn::acquire_recovery_barrier(&root, &journals),
            Err(RepositoryTxnError::CompletedPostimageMismatch { path, .. })
                if path.as_str() == "records/receipt.json"
        ));
    }

    #[test]
    fn completed_history_proves_superseded_postimages() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();

        let first_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"first head".to_vec(),
            )],
        )
        .unwrap();
        let first_plan = fixture_plan(&root, &first_draft, b"first neutral operation");
        let first_operation = first_plan.operation_id.clone();
        let mut first = RepositoryTxn::prepare(&root, &journals, first_plan, first_draft).unwrap();
        first.mark_committed().unwrap();
        first.install().unwrap();
        first.complete().unwrap();
        drop(first);

        let second_draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse(".vela/repository.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"second head".to_vec(),
            )],
        )
        .unwrap();
        let second_plan = fixture_plan(&root, &second_draft, b"second neutral operation");
        let mut second =
            RepositoryTxn::prepare(&root, &journals, second_plan, second_draft).unwrap();
        second.mark_committed().unwrap();
        second.install().unwrap();
        second.complete().unwrap();
        drop(second);

        let first_retry = RepositoryTxn::open(&root, &journals, &first_operation).unwrap();
        assert_eq!(first_retry.recovery_state(), &RecoveryState::Completed);
        drop(first_retry);
        drop(RepositoryTxn::acquire_recovery_barrier(&root, &journals).unwrap());
    }

    #[test]
    fn completed_history_rejects_corrupt_marker_and_blob() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        txn.install().unwrap();
        let completion_step = RepositoryTxnStep::AfterCompletedJournalWrite;
        assert!(matches!(
            txn.complete_at_failpoint(completion_step),
            Err(RepositoryTxnError::InjectedFailure { step }) if step == completion_step
        ));
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
        assert!(RepositoryTxn::open(&root, &journals, &operation_id).is_err());
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
            RepositoryTxn::open(&root, &journals, &operation_id),
            Err(RepositoryTxnError::CorruptBlob(_))
        ));
    }

    #[test]
    fn committed_install_never_overwrites_post_marker_drift() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let journals = temp.path().join("journals");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("state.json"), b"before").unwrap();
        let draft = DeltaDraft::prepare(
            &root,
            vec![PlannedWrite::write(
                RepoPath::parse("state.json").unwrap(),
                WriteClass::PublicReview,
                b"after".to_vec(),
            )],
        )
        .unwrap();
        let plan = fixture_plan(&root, &draft, b"conflicting request");
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();
        fs::write(root.join("state.json"), b"third party drift").unwrap();

        let error = txn.install().unwrap_err();

        assert!(matches!(
            error,
            RepositoryTxnError::CommittedConflict { .. }
        ));
        assert_eq!(
            fs::read(root.join("state.json")).unwrap(),
            b"third party drift"
        );
    }

    #[cfg(unix)]
    #[test]
    fn observed_parent_symlink_swap_after_marker_never_writes_outside_the_repository() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        txn.mark_committed().unwrap();

        symlink(&outside, root.join("records")).unwrap();
        assert!(matches!(
            txn.install(),
            Err(RepositoryTxnError::UnsafeTarget { .. })
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
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        assert_eq!(
            fs::read(root.join("records/receipt.json")).unwrap(),
            b"receipt"
        );
    }

    #[test]
    fn recovery_before_marker_has_zero_repository_delta() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();
        drop(txn);

        assert_eq!(
            RepositoryTxn::recover(&root, &journals, &operation_id).unwrap(),
            RecoveryOutcome::Prepared
        );
        assert!(!root.join("pending.json").exists());
    }

    #[test]
    fn path_bound_file_snapshot_commits_supplied_bytes_without_rereading() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        fs::create_dir_all(root.join(".vela/policies")).unwrap();
        let policy_path = RepoPath::parse(".vela/policies/active.json").unwrap();

        let snapshot =
            InputBinding::file_snapshot(policy_path.clone(), Some(b"loaded policy")).unwrap();
        fs::write(root.join(policy_path.as_str()), b"loaded policy").unwrap();
        snapshot.verify_current(&root).unwrap();

        fs::write(root.join(policy_path.as_str()), b"rotated policy").unwrap();
        assert!(matches!(
            snapshot.verify_current(&root),
            Err(RepositoryTxnError::StaleInput { path, .. }) if path == policy_path
        ));

        let signature_path = RepoPath::parse(".vela/policies/active.sig.json").unwrap();
        let absent = InputBinding::file_snapshot(signature_path.clone(), None).unwrap();
        absent.verify_current(&root).unwrap();
        fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
        assert!(matches!(
            absent.verify_current(&root),
            Err(RepositoryTxnError::StaleInput { path, .. }) if path == signature_path
        ));
    }

    #[test]
    fn path_bound_existing_input_drift_refuses_the_commit_marker() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::write(root.join(policy_path.as_str()), b"policy after").unwrap();
        let error = txn.mark_committed().unwrap_err();

        assert!(matches!(
            error,
            RepositoryTxnError::StaleInput { path, .. } if path == policy_path
        ));
        assert_eq!(txn.recovery_state(), &RecoveryState::Aborted);
        assert!(!txn.paths.marker.exists());
        assert!(!root.join("pending.json").exists());
        drop(txn);
        assert_eq!(
            RepositoryTxn::recover(
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
            RepositoryTxn::prepare(&root, &journals, unrelated_plan, unrelated_draft).unwrap();
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
        let mut retry = RepositoryTxn::prepare(&root, &journals, retry_plan, retry_draft).unwrap();
        retry.mark_committed().unwrap();
        retry.install().unwrap();
        retry.complete().unwrap();
        assert_eq!(retry.recovery_state(), &RecoveryState::Completed);
    }

    #[test]
    fn path_bound_absent_input_creation_refuses_the_commit_marker() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::write(root.join(signature_path.as_str()), b"new signature").unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(RepositoryTxnError::StaleInput { path, .. }) if path == signature_path
        ));
        assert!(!txn.paths.marker.exists());
        assert!(!root.join("pending.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn path_bound_input_rejects_a_symlink_swap_before_the_marker() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
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
        let mut txn = RepositoryTxn::prepare(&root, &journals, plan, draft).unwrap();

        fs::remove_file(&policy_target).unwrap();
        symlink(&outside, &policy_target).unwrap();
        assert!(matches!(
            txn.mark_committed(),
            Err(RepositoryTxnError::UnsafeTarget { .. })
        ));
        assert!(!txn.paths.marker.exists());
    }
}
