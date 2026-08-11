//! Recoverable, path-bound repository filesystem transactions.
//!
//! This is private durability plumbing, not a protocol object. A caller first
//! builds a pure [`CanonicalDelta`], then persists its plan and postimage blobs,
//! writes a durable commit marker, and finally installs the exact bytes. Once a
//! marker exists recovery only replays the journal; it never re-runs caller
//! policy, clocks, or key-bearing code.

mod operation_journal;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};
use unicode_normalization::UnicodeNormalization;

const REPOSITORY_TXN_SCHEMA: &str = "vela.repository-txn.internal.v2";
const REPOSITORY_TXN_BLOB_SCHEMA: &str = "vela.repository-txn-blob.internal.v1";
const REPOSITORY_TXN_MARKER_SCHEMA: &str = "vela.repository-txn-marker.internal.v1";
const CANONICAL_DELTA_SCHEMA: &str = "vela.canonical-delta.internal.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn hash(bytes: impl AsRef<[u8]>) -> Self {
        Self(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(bytes.as_ref()))
        ))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(RepositoryTxnError::InvalidDigest(value));
        };
        if !vela_protocol::is_lower_hex_64(hex) {
            return Err(RepositoryTxnError::InvalidDigest(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        Self::parse(self.0.clone()).map(|_| ())
    }

    fn file_stem(&self) -> &str {
        self.0
            .strip_prefix("sha256:")
            .expect("validated content digest")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationId(String);

impl OperationId {
    pub fn derive(kind: &str, planning_identity: &[u8]) -> Self {
        Self(operation_journal::operation_id(kind, planning_identity))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("vop_") else {
            return Err(RepositoryTxnError::InvalidOperationId(value));
        };
        if !vela_protocol::is_lower_hex_64(hex) {
            return Err(RepositoryTxnError::InvalidOperationId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPath(String);

impl RepoPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
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

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        Self::parse(self.0.clone()).map(|_| ())
    }

    fn target(&self, root: &Path) -> Result<PathBuf, RepositoryTxnError> {
        validate_target(root, self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMode {
    Regular,
    Executable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileState {
    Absent,
    File {
        digest: ContentDigest,
        size: u64,
        mode: FileMode,
    },
}

impl<'de> Deserialize<'de> for FileState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Absent {},
            File {
                digest: ContentDigest,
                size: u64,
                mode: FileMode,
            },
        }

        Ok(match Wire::deserialize(deserializer)? {
            Wire::Absent {} => Self::Absent,
            Wire::File { digest, size, mode } => Self::File { digest, size, mode },
        })
    }
}

impl FileState {
    fn verify(&self) -> Result<(), RepositoryTxnError> {
        match self {
            Self::Absent => Ok(()),
            Self::File { digest, .. } => digest.verify(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalBlobRef {
    digest: ContentDigest,
    size: u64,
}

impl JournalBlobRef {
    fn verify(&self) -> Result<(), RepositoryTxnError> {
        self.digest.verify()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CanonicalEvidence,
    PublicReview,
    Authority,
}

impl WriteClass {
    fn install_order(self) -> u8 {
        match self {
            Self::CanonicalEvidence => 10,
            Self::PublicReview => 20,
            Self::Authority => 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagedWrite {
    pub path: RepoPath,
    pub class: WriteClass,
    pub preimage: FileState,
    pub postimage: FileState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload: Option<JournalBlobRef>,
}

impl StagedWrite {
    fn verify_payload_binding(&self) -> Result<(), RepositoryTxnError> {
        self.path.verify()?;
        self.preimage.verify()?;
        self.postimage.verify()?;
        if let Some(payload) = &self.payload {
            payload.verify()?;
        }
        match (&self.postimage, &self.payload) {
            (FileState::Absent, None) => Ok(()),
            (
                FileState::File { digest, size, .. },
                Some(JournalBlobRef {
                    digest: payload_digest,
                    size: payload_size,
                }),
            ) if digest == payload_digest && size == payload_size => Ok(()),
            (FileState::Absent, Some(_)) => Err(RepositoryTxnError::CorruptPlan(format!(
                "deleted postimage {} carries a blob reference",
                self.path.as_str()
            ))),
            (FileState::File { .. }, None) => Err(RepositoryTxnError::CorruptPlan(format!(
                "file postimage {} has no blob reference",
                self.path.as_str()
            ))),
            (FileState::File { .. }, Some(_)) => Err(RepositoryTxnError::CorruptPlan(format!(
                "file postimage {} does not bind its payload digest and size",
                self.path.as_str()
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalDelta {
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
            write.verify_payload_binding()?;
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

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        if self.schema != CANONICAL_DELTA_SCHEMA {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "unexpected canonical delta schema {}",
                self.schema
            )));
        }
        self.root.verify()?;
        let normalized = Self::new(self.writes.clone())?;
        if normalized.writes != self.writes || normalized.root != self.root {
            return Err(RepositoryTxnError::CorruptPlan(
                "canonical delta is not sorted or root-bound".to_string(),
            ));
        }
        Ok(())
    }

    pub fn root(&self) -> &ContentDigest {
        &self.root
    }

    pub fn writes(&self) -> &[StagedWrite] {
        &self.writes
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
pub struct PlannedWrite {
    path: RepoPath,
    class: WriteClass,
    postimage: PlannedPostimage,
}

impl PlannedWrite {
    pub fn write(path: RepoPath, class: WriteClass, bytes: Vec<u8>) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::File { bytes, mode: None },
        }
    }

    pub fn delete(path: RepoPath, class: WriteClass) -> Self {
        Self {
            path,
            class,
            postimage: PlannedPostimage::Absent,
        }
    }

    /// Consume one already-bounded regular-file write as path, class, and
    /// optional postimage bytes. Executable modes are intentionally rejected
    /// because this representation has no mode field.
    pub fn into_regular_object_parts(
        self,
    ) -> Result<(String, WriteClass, Option<Vec<u8>>), RepositoryTxnError> {
        let postimage = match self.postimage {
            PlannedPostimage::Absent => None,
            PlannedPostimage::File { bytes, mode } => {
                if mode.is_some_and(|mode| mode != FileMode::Regular) {
                    return Err(RepositoryTxnError::CorruptPlan(format!(
                        "planned object {} cannot discard an executable mode",
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
pub struct DeltaDraft {
    pub delta: CanonicalDelta,
    blobs: BTreeMap<ContentDigest, Vec<u8>>,
}

impl DeltaDraft {
    pub fn prepare(
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

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        self.delta.verify()?;
        let referenced = self
            .delta
            .writes()
            .iter()
            .filter_map(|write| write.payload.as_ref().map(|blob| blob.digest.clone()))
            .collect::<BTreeSet<_>>();
        if referenced.len() != self.blobs.len()
            || referenced
                .iter()
                .any(|digest| !self.blobs.contains_key(digest))
        {
            return Err(RepositoryTxnError::CorruptPlan(
                "prepared delta does not contain exactly its referenced blobs".into(),
            ));
        }
        for blob in self
            .delta
            .writes()
            .iter()
            .filter_map(|write| write.payload.as_ref())
        {
            validate_blob_bytes(
                blob,
                self.blobs
                    .get(&blob.digest)
                    .expect("checked referenced draft blob"),
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBinding {
    canonical_root: String,
    repository_id: String,
}

const MAX_REPOSITORY_ID_BYTES: usize = 256;

impl RepositoryBinding {
    pub fn new(
        repository_root: &Path,
        repository_id: impl Into<String>,
    ) -> Result<Self, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let repository_id = repository_id.into();
        let binding = Self {
            canonical_root: root.to_string_lossy().into_owned(),
            repository_id,
        };
        binding.verify_shape()?;
        Ok(binding)
    }

    fn verify_repository_id(repository_id: &str) -> Result<(), RepositoryTxnError> {
        if repository_id.trim().is_empty() {
            return Err(RepositoryTxnError::CorruptPlan(
                "repository binding has an empty repository id".to_string(),
            ));
        }
        if repository_id.len() > MAX_REPOSITORY_ID_BYTES {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "repository binding id exceeds {MAX_REPOSITORY_ID_BYTES} bytes"
            )));
        }
        Ok(())
    }

    fn verify_shape(&self) -> Result<(), RepositoryTxnError> {
        Self::verify_repository_id(&self.repository_id)
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

    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationKind(String);

impl OperationKind {
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryTxnError> {
        let kind = Self(value.into());
        kind.verify()?;
        Ok(kind)
    }

    fn verify(&self) -> Result<(), RepositoryTxnError> {
        if self.0.is_empty()
            || self.0.len() > 64
            || !self.0.split('_').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
        {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "invalid internal operation kind {:?}",
                self.0
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputBinding {
    pub name: String,
    pub digest: ContentDigest,
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
    pub fn current_directory(
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
    /// assume that a caller-critical receipt or policy path exists, and the
    /// marker check rejects creation, deletion, byte drift, or mode drift.
    pub fn current_file(
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
    fn existing_file(repository_root: &Path, path: RepoPath) -> Result<Self, RepositoryTxnError> {
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
    /// before the commit marker is therefore stale input, not a result that
    /// can be committed under changed input bytes.
    pub fn absent_file(repository_root: &Path, path: RepoPath) -> Result<Self, RepositoryTxnError> {
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
    fn file_snapshot(path: RepoPath, bytes: Option<&[u8]>) -> Result<Self, RepositoryTxnError> {
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
    pub fn exact_file(
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
    pub fn exact_directory(
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
pub struct RepositoryTxnPlanSpec {
    pub kind: OperationKind,
    pub operation_id: OperationId,
    pub request_root: ContentDigest,
    pub repository: RepositoryBinding,
    pub fixed_time: String,
    pub read_set: Vec<InputBinding>,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryTxnPlan {
    schema: String,
    root: ContentDigest,
    kind: OperationKind,
    operation_id: OperationId,
    request_root: ContentDigest,
    repository: RepositoryBinding,
    fixed_time: String,
    read_set: Vec<InputBinding>,
    canonical_delta: CanonicalDelta,
    result: serde_json::Value,
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
    pub fn new(
        spec: RepositoryTxnPlanSpec,
        canonical_delta: CanonicalDelta,
    ) -> Result<Self, RepositoryTxnError> {
        canonical_delta.verify()?;
        OperationId::parse(spec.operation_id.as_str())?;
        spec.kind.verify()?;
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
        self.root.verify()?;
        OperationId::parse(self.operation_id.as_str())?;
        self.request_root.verify()?;
        self.kind.verify()?;
        self.repository.verify_shape()?;
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
    fn root(&self) -> &ContentDigest {
        &self.root
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitMarker {
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

    fn verify_shape(&self) -> Result<(), RepositoryTxnError> {
        OperationId::parse(self.operation_id.as_str())?;
        self.plan_root.verify()?;
        self.delta_root.verify()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RecoveryState {
    Prepared,
    Aborted,
    Committed,
    Installing { installed: usize, total: usize },
    Installed,
    Completed,
    CommittedConflict { path: RepoPath },
}

impl<'de> Deserialize<'de> for RecoveryState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Prepared {},
            Aborted {},
            Committed {},
            Installing { installed: usize, total: usize },
            Installed {},
            Completed {},
            CommittedConflict { path: RepoPath },
        }

        Ok(match Wire::deserialize(deserializer)? {
            Wire::Prepared {} => Self::Prepared,
            Wire::Aborted {} => Self::Aborted,
            Wire::Committed {} => Self::Committed,
            Wire::Installing { installed, total } => Self::Installing { installed, total },
            Wire::Installed {} => Self::Installed,
            Wire::Completed {} => Self::Completed,
            Wire::CommittedConflict { path } => Self::CommittedConflict { path },
        })
    }
}

impl RecoveryState {
    /// Stable lowercase token for operator and JSON diagnostics. Progress
    /// fields remain available on the typed variants without leaking Debug
    /// spellings into a product contract.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Aborted => "aborted",
            Self::Committed => "committed",
            Self::Installing { .. } => "installing",
            Self::Installed => "installed",
            Self::Completed => "completed",
            Self::CommittedConflict { .. } => "committed_conflict",
        }
    }
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
    /// and transition membership after these private recovery copies are pruned.
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
        self.plan.verify()?;
        self.verify_recovery_shape()
    }

    fn verify_recovery_shape(&self) -> Result<(), RepositoryTxnError> {
        let write_count = self.plan.canonical_delta.writes().len();
        match &self.recovery {
            RecoveryState::Installing { installed, total }
                if *total != write_count || *installed == 0 || *installed > *total =>
            {
                Err(RepositoryTxnError::CorruptPlan(format!(
                    "transaction {} has invalid installing progress {installed}/{total} for {write_count} writes",
                    self.plan.operation_id.as_str()
                )))
            }
            RecoveryState::CommittedConflict { path } => {
                path.verify()?;
                if self
                    .plan
                    .canonical_delta
                    .writes()
                    .iter()
                    .any(|write| &write.path == path)
                {
                    Ok(())
                } else {
                    Err(RepositoryTxnError::CorruptPlan(format!(
                        "transaction {} records a conflict outside its canonical delta at {}",
                        self.plan.operation_id.as_str(),
                        path.as_str()
                    )))
                }
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    fn path(journal_dir: &Path, root: &Path) -> PathBuf {
        let lock_id = ContentDigest::hash(root.to_string_lossy().as_bytes());
        journal_dir
            .join("repository-locks")
            .join(format!("{}.lock", lock_id.file_stem()))
    }

    fn validate_existing_path(path: &Path) -> Result<(), RepositoryTxnError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            RepositoryTxnError::Io(format!(
                "inspect repository lock {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(RepositoryTxnError::Io(format!(
                "repository lock is not a regular non-symlink file: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn lock_file(file: File, path: &Path) -> Result<Self, RepositoryTxnError> {
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(RepositoryTxnError::Busy),
            Err(std::fs::TryLockError::Error(error)) => Err(RepositoryTxnError::Io(format!(
                "lock repository {}: {error}",
                path.display()
            ))),
        }
    }

    fn acquire(journal_dir: &Path, root: &Path) -> Result<Self, RepositoryTxnError> {
        let path = Self::path(journal_dir, root);
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
            Ok(_) => Self::validate_existing_path(&path)?,
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
        Self::lock_file(file, &path)
    }

    /// Try to hold an existing repository lock without creating a directory or
    /// file. `None` means no lock byte exists, so the caller only has a
    /// race-prone diagnostic snapshot; authoritative writes use `acquire`.
    fn try_acquire_existing(
        journal_dir: &Path,
        root: &Path,
    ) -> Result<Option<Self>, RepositoryTxnError> {
        let path = Self::path(journal_dir, root);
        let parent = path.parent().ok_or_else(|| {
            RepositoryTxnError::Io(format!(
                "repository lock path has no parent: {}",
                path.display()
            ))
        })?;
        match fs::symlink_metadata(parent) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "inspect repository lock directory {}: {error}",
                    parent.display()
                )));
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(RepositoryTxnError::Io(format!(
                    "repository lock directory is not a regular non-symlink directory: {}",
                    parent.display()
                )));
            }
            Ok(_) => {}
        }
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "inspect repository lock {}: {error}",
                    path.display()
                )));
            }
            Ok(_) => Self::validate_existing_path(&path)?,
        }
        let file = match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(RepositoryTxnError::Io(format!(
                    "open repository lock {}: {error}",
                    path.display()
                )));
            }
        };
        Self::lock_file(file, &path).map(Some)
    }
}

/// An exclusive repository lock whose recovery barrier was checked before new
/// semantic planning began. Keeping this value alive prevents another writer
/// from crossing the same barrier until it is consumed by
/// [`RepositoryTxn::prepare_with_barrier`] or dropped.
#[derive(Debug)]
pub struct RepositoryRecoveryBarrier {
    root: PathBuf,
    journal_dir: PathBuf,
    lock: RepositoryWriteLock,
}

/// One move-only, in-memory capability authorizing an exact repository plan.
///
/// Concrete caller policy remains outside the transaction runtime. The runtime
/// only invokes these two lifecycle checks and never serializes the capability
/// or interprets its commitment.
pub trait TransactionAuthorization: fmt::Debug {
    /// Bind the exact verified plan before the transaction writes any journal
    /// byte.
    fn bind_plan(
        &mut self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError>;

    /// Revalidate policy as the last policy-dependent fallible check before
    /// the durable commit marker is created.
    fn revalidate_for_marker(
        &self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError>;
}

/// Read-only, exact transaction state exposed to one authorization capability.
///
/// Postimage access is bounded to a staged write in the canonical delta, and
/// every returned byte string is checked against its size and digest.
pub struct TransactionAuthorizationContext<'a> {
    repository_root: &'a Path,
    repository_binding: &'a RepositoryBinding,
    plan_root: &'a ContentDigest,
    canonical_delta: &'a CanonicalDelta,
    read_blob: &'a mut dyn FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
}

impl<'a> TransactionAuthorizationContext<'a> {
    fn new(
        repository_root: &'a Path,
        repository_binding: &'a RepositoryBinding,
        plan_root: &'a ContentDigest,
        canonical_delta: &'a CanonicalDelta,
        read_blob: &'a mut dyn FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
    ) -> Self {
        Self {
            repository_root,
            repository_binding,
            plan_root,
            canonical_delta,
            read_blob,
        }
    }

    pub fn repository_root(&self) -> &Path {
        self.repository_root
    }

    pub fn repository_binding(&self) -> &RepositoryBinding {
        self.repository_binding
    }

    pub fn plan_root(&self) -> &ContentDigest {
        self.plan_root
    }

    pub fn canonical_delta(&self) -> &CanonicalDelta {
        self.canonical_delta
    }

    pub fn postimage_bytes(
        &mut self,
        write: &StagedWrite,
    ) -> Result<Option<Vec<u8>>, RepositoryTxnError> {
        if !self
            .canonical_delta
            .writes()
            .iter()
            .any(|candidate| candidate == write)
        {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "authorization requested postimage bytes outside the canonical delta: {}",
                write.path.as_str()
            )));
        }
        write.verify_payload_binding()?;
        match (&write.postimage, &write.payload) {
            (FileState::Absent, None) => Ok(None),
            (FileState::File { .. }, Some(blob)) => {
                let bytes = (self.read_blob)(blob)?;
                validate_blob_bytes(blob, &bytes)?;
                Ok(Some(bytes))
            }
            (FileState::Absent, Some(_)) => Err(RepositoryTxnError::CorruptPlan(format!(
                "deleted postimage {} carries a blob reference",
                write.path.as_str()
            ))),
            (FileState::File { .. }, None) => Err(RepositoryTxnError::CorruptPlan(format!(
                "file postimage {} has no blob reference",
                write.path.as_str()
            ))),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestAuthorizationBinding {
    repository_id: String,
    plan_root: ContentDigest,
    delta_root: ContentDigest,
}

#[cfg(test)]
impl TestAuthorizationBinding {
    fn read(context: &TransactionAuthorizationContext<'_>) -> Self {
        Self {
            repository_id: context.repository_binding().repository_id().to_string(),
            plan_root: context.plan_root().clone(),
            delta_root: context.canonical_delta().root().clone(),
        }
    }

    fn verify(&self, actual: Self) -> Result<(), RepositoryTxnError> {
        if self.repository_id != actual.repository_id {
            return Err(RepositoryTxnError::WriteAuthorizationRepositoryMismatch {
                authorized: self.repository_id.clone(),
                planned: actual.repository_id,
            });
        }
        if self.delta_root != actual.delta_root {
            return Err(RepositoryTxnError::WriteAuthorizationDeltaMismatch {
                authorized: self.delta_root.clone(),
                planned: actual.delta_root,
            });
        }
        if self.plan_root != actual.plan_root {
            return Err(RepositoryTxnError::StaleWriteAuthorization {
                expected: self.plan_root.clone(),
                actual: actual.plan_root,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
struct RecordingTransactionAuthorization {
    binding: Option<TestAuthorizationBinding>,
    deny_bind: bool,
    deny_marker: bool,
    obstruct_marker_write_on_revalidate: Option<PathBuf>,
    calls: Option<std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>>,
}

#[cfg(test)]
impl RecordingTransactionAuthorization {
    fn record(&self, call: &'static str) {
        if let Some(calls) = &self.calls {
            calls
                .lock()
                .expect("test authorization call log")
                .push(call);
        }
    }
}

#[cfg(test)]
impl TransactionAuthorization for RecordingTransactionAuthorization {
    fn bind_plan(
        &mut self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        self.record("bind_plan");
        if self.deny_bind {
            return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "test_transaction",
                reason: "test authorization denied plan binding".into(),
            });
        }
        let candidate = TestAuthorizationBinding::read(context);
        if let Some(binding) = &self.binding {
            binding.verify(candidate)?;
        } else {
            self.binding = Some(candidate);
        }
        Ok(())
    }

    fn revalidate_for_marker(
        &self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        self.record("revalidate_for_marker");
        if self.deny_marker {
            return Err(RepositoryTxnError::RepositoryWriteIntentDenied {
                intent: "test_transaction",
                reason: "test authorization denied marker".into(),
            });
        }
        self.binding
            .as_ref()
            .ok_or_else(|| {
                RepositoryTxnError::WriteAuthorization("unbound test capability".into())
            })?
            .verify(TestAuthorizationBinding::read(context))?;
        if let Some(path) = &self.obstruct_marker_write_on_revalidate {
            fs::create_dir_all(path).map_err(|error| {
                RepositoryTxnError::Io(format!(
                    "obstruct marker write at {}: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
fn test_transaction_authorization() -> Box<dyn TransactionAuthorization> {
    Box::<RecordingTransactionAuthorization>::default()
}

/// A recovery barrier that has additionally passed the repository-generation
/// write gate.
///
/// The authorization is deliberately in-memory and non-serializable. A
/// durable Prepared journal therefore cannot recreate permission to cross the
/// commit-marker boundary after a process restart.
#[derive(Debug)]
pub struct CanonicalWriteBarrier {
    recovery: RepositoryRecoveryBarrier,
    authorization: Box<dyn TransactionAuthorization>,
}

impl RepositoryRecoveryBarrier {
    pub fn repository_root(&self) -> &Path {
        &self.root
    }

    pub fn authorize(
        self,
        authorization: Box<dyn TransactionAuthorization>,
    ) -> CanonicalWriteBarrier {
        CanonicalWriteBarrier {
            recovery: self,
            authorization,
        }
    }
}

#[derive(Debug)]
struct RepositoryJournalInventory {
    journals: Vec<(RepositoryTxnPaths, RepositoryTxnJournal)>,
    private_residue: Vec<ValidatedPrivateResidue>,
}

fn journal_directory_entries(
    path: &Path,
    label: &str,
) -> Result<Option<Vec<fs::DirEntry>>, RepositoryTxnError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(RepositoryTxnError::Journal(format!(
                "inspect {label} {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RepositoryTxnError::Journal(format!(
            "{label} is not a regular non-symlink directory: {}",
            path.display()
        )));
    }
    let mut entries = fs::read_dir(path)
        .map_err(|error| {
            RepositoryTxnError::Journal(format!("read {label} {}: {error}", path.display()))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            RepositoryTxnError::Journal(format!("enumerate {label} {}: {error}", path.display()))
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(Some(entries))
}

fn private_residue_entry(
    journal_dir: &Path,
    path: &Path,
    kind: ValidatedPrivateResidueKind,
) -> Result<ValidatedPrivateResidue, RepositoryTxnError> {
    let relative = path.strip_prefix(journal_dir).map_err(|_| {
        RepositoryTxnError::Journal(format!(
            "private recovery entry escapes its journal root: {}",
            path.display()
        ))
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        RepositoryTxnError::Journal(format!(
            "private recovery entry is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    Ok(ValidatedPrivateResidue {
        path: RepoPath::parse(relative.to_string())?,
        kind,
    })
}

fn require_regular_recovery_file(
    path: &Path,
    label: &str,
) -> Result<fs::Metadata, RepositoryTxnError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        RepositoryTxnError::Journal(format!("inspect {label} {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RepositoryTxnError::Journal(format!(
            "{label} is not a regular non-symlink file: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

fn verify_repository_lock_index(
    root: &Path,
    journal_dir: &Path,
    residue: &mut Vec<ValidatedPrivateResidue>,
) -> Result<(), RepositoryTxnError> {
    let lock_dir = journal_dir.join("repository-locks");
    let Some(entries) = journal_directory_entries(&lock_dir, "repository lock directory")? else {
        return Ok(());
    };
    let canonical_root = canonical_repository_root(root)?;
    let expected = RepositoryWriteLock::path(journal_dir, &canonical_root);
    for entry in entries {
        let path = entry.path();
        let metadata = require_regular_recovery_file(&path, "repository lock entry")?;
        if path != expected || metadata.len() != 0 {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected repository lock entry: {}",
                path.display()
            )));
        }
        residue.push(private_residue_entry(
            journal_dir,
            &path,
            ValidatedPrivateResidueKind::RegularFile,
        )?);
    }
    Ok(())
}

fn verify_blob_index(
    journal_dir: &Path,
    blob_dir: &Path,
    residue: &mut Vec<ValidatedPrivateResidue>,
) -> Result<(), RepositoryTxnError> {
    let Some(entries) =
        journal_directory_entries(blob_dir, "repository transaction blob directory")?
    else {
        return Ok(());
    };
    for entry in entries {
        let path = entry.path();
        require_regular_recovery_file(&path, "repository transaction blob entry")?;
        if operation_journal::is_owned_atomic_temp(&path) {
            residue.push(private_residue_entry(
                journal_dir,
                &path,
                ValidatedPrivateResidueKind::RegularFile,
            )?);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected repository transaction blob entry: {}",
                path.display()
            )));
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                RepositoryTxnError::Journal(format!(
                    "repository transaction blob has an invalid file name: {}",
                    path.display()
                ))
            })?;
        let filename_digest = ContentDigest::parse(format!("sha256:{stem}"))?;
        let blob: BlobJournal =
            operation_journal::read_json(&path).map_err(RepositoryTxnError::Journal)?;
        blob.digest.verify()?;
        if blob.schema != REPOSITORY_TXN_BLOB_SCHEMA
            || blob.digest != filename_digest
            || blob.size != blob.bytes.len() as u64
            || ContentDigest::hash(&blob.bytes) != blob.digest
        {
            return Err(RepositoryTxnError::CorruptBlob(filename_digest));
        }
        residue.push(private_residue_entry(
            journal_dir,
            &path,
            ValidatedPrivateResidueKind::RegularFile,
        )?);
    }
    Ok(())
}

fn verify_commit_marker_index(
    journal_dir: &Path,
    marker_dir: &Path,
    plans: &BTreeMap<OperationId, &RepositoryTxnJournal>,
    residue: &mut Vec<ValidatedPrivateResidue>,
) -> Result<(), RepositoryTxnError> {
    let Some(entries) =
        journal_directory_entries(marker_dir, "repository commit-marker directory")?
    else {
        return Ok(());
    };
    for entry in entries {
        let path = entry.path();
        require_regular_recovery_file(&path, "repository commit-marker entry")?;
        if operation_journal::is_owned_atomic_temp(&path) {
            residue.push(private_residue_entry(
                journal_dir,
                &path,
                ValidatedPrivateResidueKind::RegularFile,
            )?);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected repository commit-marker entry: {}",
                path.display()
            )));
        }
        let operation_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                RepositoryTxnError::CorruptPlan(format!(
                    "repository commit marker has an invalid file name: {}",
                    path.display()
                ))
            })?;
        let operation_id = OperationId::parse(operation_id.to_string())?;
        let journal = plans.get(&operation_id).ok_or_else(|| {
            RepositoryTxnError::CorruptPlan(format!(
                "commit marker {} has no matching durable plan",
                operation_id.as_str()
            ))
        })?;
        let marker: CommitMarker =
            operation_journal::read_json(&path).map_err(RepositoryTxnError::Journal)?;
        marker.verify_shape()?;
        if marker.schema != REPOSITORY_TXN_MARKER_SCHEMA
            || marker.operation_id != operation_id
            || marker != CommitMarker::from_plan(&journal.plan)
        {
            return Err(RepositoryTxnError::CorruptPlan(format!(
                "commit marker {} does not match its durable plan and file name",
                operation_id.as_str()
            )));
        }
        residue.push(private_residue_entry(
            journal_dir,
            &path,
            ValidatedPrivateResidueKind::RegularFile,
        )?);
    }
    Ok(())
}

fn repository_inventory(
    root: &Path,
    journal_dir: &Path,
) -> Result<RepositoryJournalInventory, RepositoryTxnError> {
    let mut residue = Vec::new();
    let Some(root_entries) = journal_directory_entries(journal_dir, "repository journal root")?
    else {
        return Ok(RepositoryJournalInventory {
            journals: Vec::new(),
            private_residue: residue,
        });
    };
    for entry in root_entries {
        let path = entry.path();
        let name = entry.file_name();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            RepositoryTxnError::Journal(format!(
                "inspect repository journal-root entry {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || (name != "repository" && name != "repository-locks")
        {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected repository journal-root entry: {}",
                path.display()
            )));
        }
        residue.push(private_residue_entry(
            journal_dir,
            &path,
            ValidatedPrivateResidueKind::Directory,
        )?);
    }
    verify_repository_lock_index(root, journal_dir, &mut residue)?;

    let repository_dir = journal_dir.join("repository");
    let Some(entries) = journal_directory_entries(&repository_dir, "repository journal directory")?
    else {
        residue.sort();
        return Ok(RepositoryJournalInventory {
            journals: Vec::new(),
            private_residue: residue,
        });
    };

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
                residue.push(private_residue_entry(
                    journal_dir,
                    &path,
                    ValidatedPrivateResidueKind::Directory,
                )?);
                continue;
            }
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected directory in repository journal: {}",
                path.display()
            )));
        }
        if !metadata.is_file() {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected non-journal entry in repository journal: {}",
                path.display()
            )));
        }
        if operation_journal::is_owned_atomic_temp(&path) {
            residue.push(private_residue_entry(
                journal_dir,
                &path,
                ValidatedPrivateResidueKind::RegularFile,
            )?);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            return Err(RepositoryTxnError::Journal(format!(
                "unexpected repository journal entry: {}",
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
        // The caller supplies one repository-local journal directory. A
        // durable plan bound to any other root is corruption, not an entry to
        // ignore while deciding whether this repository is safe to mutate.
        journal.plan.repository.verify_root(root)?;
        residue.push(private_residue_entry(
            journal_dir,
            &path,
            ValidatedPrivateResidueKind::RegularFile,
        )?);
        journals.push((paths, journal));
    }
    let plans = journals
        .iter()
        .map(|(_, journal)| (journal.plan.operation_id.clone(), journal))
        .collect::<BTreeMap<_, _>>();
    verify_blob_index(journal_dir, &repository_dir.join("blobs"), &mut residue)?;
    verify_commit_marker_index(
        journal_dir,
        &repository_dir.join("committed"),
        &plans,
        &mut residue,
    )?;
    residue.sort();
    Ok(RepositoryJournalInventory {
        journals,
        private_residue: residue,
    })
}

fn repository_journals(
    root: &Path,
    journal_dir: &Path,
) -> Result<Vec<(RepositoryTxnPaths, RepositoryTxnJournal)>, RepositoryTxnError> {
    Ok(repository_inventory(root, journal_dir)?.journals)
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
    marker.verify_shape()?;
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
    expected.verify()?;
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
    blob.digest.verify()?;
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

fn verify_completed_history_against_base(
    root: &Path,
    completed: &[(RepositoryTxnPaths, RepositoryTxnJournal)],
    pending_delta: Option<&CanonicalDelta>,
) -> Result<(), RepositoryTxnError> {
    if completed.is_empty() {
        return Ok(());
    }

    // Every completed journal participates in the same exact postimage
    // transition graph; there is no out-of-engine rematerialization exception.
    let current_head = completed.to_vec();

    // Validate that each durable postimage is either still current or is
    // connected to the current bytes by another completed transaction's exact
    // preimage -> postimage edge. No write may be rematerialized outside the
    // engine and remain verifiable completed history.
    for (_, journal) in &current_head {
        for write in journal.plan.canonical_delta.writes() {
            let actual = pending_delta
                .and_then(|delta| {
                    delta
                        .writes()
                        .iter()
                        .find(|pending| pending.path == write.path)
                })
                .map(|pending| pending.preimage.clone())
                .map_or_else(|| inspect_file_state(root, &write.path), Ok)?;
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

#[cfg(test)]
fn verify_completed_history(
    root: &Path,
    completed: &[(RepositoryTxnPaths, RepositoryTxnJournal)],
) -> Result<(), RepositoryTxnError> {
    for (paths, journal) in completed {
        verify_completed_marker_and_blobs(paths, journal)?;
    }
    verify_completed_history_against_base(root, completed, None)
}

fn corrupt_recovery_layout(
    journal: &RepositoryTxnJournal,
    detail: impl fmt::Display,
) -> RepositoryTxnError {
    RepositoryTxnError::CorruptPlan(format!(
        "transaction {} has an impossible durable recovery layout for {:?}: {detail}",
        journal.plan.operation_id.as_str(),
        journal.recovery
    ))
}

fn observe_recovery_layout(
    root: &Path,
    journal: &RepositoryTxnJournal,
) -> Result<usize, RepositoryTxnError> {
    let mut prefix = 0;
    let mut first_non_postimage = false;
    for write in journal.plan.canonical_delta.writes() {
        let actual = inspect_file_state(root, &write.path)?;
        if actual == write.postimage {
            if first_non_postimage {
                return Err(corrupt_recovery_layout(
                    journal,
                    format!(
                        "postimage at {} follows a preimage hole",
                        write.path.as_str()
                    ),
                ));
            }
            prefix += 1;
        } else if actual == write.preimage {
            first_non_postimage = true;
        } else {
            return Err(RepositoryTxnError::CommittedConflict {
                path: write.path.clone(),
                expected_preimage: Box::new(write.preimage.clone()),
                expected_postimage: Box::new(write.postimage.clone()),
                actual: Box::new(actual),
            });
        }
    }
    Ok(prefix)
}

fn verify_marker_bearing_recovery_layout(
    root: &Path,
    journal: &RepositoryTxnJournal,
) -> Result<(), RepositoryTxnError> {
    let writes = journal.plan.canonical_delta.writes();
    match &journal.recovery {
        RecoveryState::Prepared | RecoveryState::Committed => {
            let prefix = observe_recovery_layout(root, journal)?;
            if prefix <= usize::from(!writes.is_empty()) {
                Ok(())
            } else {
                Err(corrupt_recovery_layout(
                    journal,
                    format!("installed prefix {prefix} exceeds the single-write crash window"),
                ))
            }
        }
        RecoveryState::Installing { installed, total } => {
            let prefix = observe_recovery_layout(root, journal)?;
            let write_before_progress = installed.saturating_add(1).min(*total);
            if prefix == *installed || prefix == write_before_progress {
                Ok(())
            } else {
                Err(corrupt_recovery_layout(
                    journal,
                    format!(
                        "installed prefix {prefix} is neither durable progress {installed} nor its one-write crash window {write_before_progress}"
                    ),
                ))
            }
        }
        RecoveryState::Installed => {
            for write in writes {
                let actual = inspect_file_state(root, &write.path)?;
                if actual != write.postimage {
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
        RecoveryState::CommittedConflict { path } => {
            let conflict_index = writes
                .iter()
                .position(|write| &write.path == path)
                .ok_or_else(|| {
                    corrupt_recovery_layout(
                        journal,
                        format!(
                            "conflict path {} is outside the canonical delta",
                            path.as_str()
                        ),
                    )
                })?;
            for (index, write) in writes.iter().enumerate() {
                let actual = inspect_file_state(root, &write.path)?;
                if index < conflict_index && actual != write.postimage {
                    return Err(RepositoryTxnError::CompletedPostimageMismatch {
                        operation_id: journal.plan.operation_id.as_str().to_string(),
                        path: write.path.clone(),
                        expected: Box::new(write.postimage.clone()),
                        actual: Box::new(actual),
                    });
                }
                if index > conflict_index {
                    if actual == write.postimage {
                        return Err(corrupt_recovery_layout(
                            journal,
                            format!(
                                "{} is an out-of-order postimage beyond the recorded conflict",
                                write.path.as_str()
                            ),
                        ));
                    }
                    if actual != write.preimage {
                        return Err(RepositoryTxnError::CommittedConflict {
                            path: write.path.clone(),
                            expected_preimage: Box::new(write.preimage.clone()),
                            expected_postimage: Box::new(write.postimage.clone()),
                            actual: Box::new(actual),
                        });
                    }
                }
                // The conflict slot itself may still contain the third state
                // that caused the retry failure, or an operator may have
                // restored either exact endpoint before retrying.
            }
            Ok(())
        }
        RecoveryState::Aborted | RecoveryState::Completed => Err(corrupt_recovery_layout(
            journal,
            "terminal state was passed to incomplete recovery validation",
        )),
    }
}

#[derive(Debug, Clone)]
struct PendingRecovery {
    operation_id: OperationId,
    state: RecoveryState,
}

fn ensure_recovery_barrier_locked(
    root: &Path,
    journal_dir: &Path,
    allowed_operation_id: Option<&OperationId>,
) -> Result<(), RepositoryTxnError> {
    let journals = repository_journals(root, journal_dir)?;
    if let Some(pending) = validate_recovery_journals(root, &journals)? {
        if allowed_operation_id == Some(&pending.operation_id) {
            return Ok(());
        }
        return Err(RepositoryTxnError::RecoveryRequired {
            operation_id: pending.operation_id.as_str().to_string(),
            state: pending.state,
        });
    }
    Ok(())
}

fn verify_incomplete_recovery_candidate(
    root: &Path,
    paths: &RepositoryTxnPaths,
    journal: &RepositoryTxnJournal,
) -> Result<bool, RepositoryTxnError> {
    verify_journal_blobs(paths, journal)?;
    match read_commit_marker(paths, journal) {
        Ok(_) => {
            verify_marker_bearing_recovery_layout(root, journal)?;
            Ok(true)
        }
        Err(RepositoryTxnError::NotCommitted)
            if matches!(journal.recovery, RecoveryState::Prepared) =>
        {
            Ok(false)
        }
        Err(RepositoryTxnError::NotCommitted) => Err(RepositoryTxnError::CorruptPlan(format!(
            "transaction {} is {:?} but has no commit marker",
            journal.plan.operation_id.as_str(),
            journal.recovery
        ))),
        Err(error) => Err(error),
    }
}

fn validate_recovery_journals(
    root: &Path,
    journals: &[(RepositoryTxnPaths, RepositoryTxnJournal)],
) -> Result<Option<PendingRecovery>, RepositoryTxnError> {
    let mut completed = Vec::new();
    let mut pending = Vec::new();
    let repository_ids = journals
        .iter()
        .map(|(_, journal)| journal.plan.repository.repository_id.clone())
        .collect::<BTreeSet<_>>();
    if repository_ids.len() > 1 {
        return Err(RepositoryTxnError::MixedRepositoryIdentities {
            repository_ids: repository_ids.into_iter().collect(),
        });
    }

    for (paths, journal) in journals {
        match journal.recovery.clone() {
            RecoveryState::Aborted => verify_aborted_without_marker(paths, journal)?,
            RecoveryState::Completed => {
                verify_completed_marker_and_blobs(paths, journal)?;
                completed.push((paths.clone(), journal.clone()));
            }
            state => {
                let marker_present = verify_incomplete_recovery_candidate(root, paths, journal)?;
                pending.push((paths, journal, state, marker_present));
            }
        }
    }

    pending.sort_by(|left, right| left.1.plan.operation_id.cmp(&right.1.plan.operation_id));
    if pending.len() > 1 {
        return Err(RepositoryTxnError::MultiplePendingTransactions {
            operation_ids: pending
                .iter()
                .map(|(_, journal, _, _)| journal.plan.operation_id.as_str().to_string())
                .collect(),
        });
    }

    let pending_delta = pending.first().and_then(|(_, journal, _, marker_present)| {
        marker_present.then_some(&journal.plan.canonical_delta)
    });
    verify_completed_history_against_base(root, &completed, pending_delta)?;

    Ok(pending
        .into_iter()
        .next()
        .map(|(_, journal, state, _)| PendingRecovery {
            operation_id: journal.plan.operation_id.clone(),
            state,
        }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWrite {
    pub staged: StagedWrite,
    pub postimage_bytes: Option<Vec<u8>>,
}

fn resolve_public_writes(
    delta: &CanonicalDelta,
    mut read_blob: impl FnMut(&JournalBlobRef) -> Result<Vec<u8>, RepositoryTxnError>,
) -> Result<Vec<ResolvedWrite>, RepositoryTxnError> {
    let mut writes = delta
        .writes()
        .iter()
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
    // The external exact-delta boundary is path-sorted, while installation
    // preserves the durable class order.
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
pub struct RepositoryTxn {
    root: PathBuf,
    paths: RepositoryTxnPaths,
    journal: RepositoryTxnJournal,
    authorization: Option<Box<dyn TransactionAuthorization>>,
    _lock: RepositoryWriteLock,
}

/// Result of explicitly recovering one exact durable operation journal.
///
/// Recovery never reconstructs or invokes a write authorization. A definite
/// marker-free Prepared journal is durably aborted without installing its
/// canonical postimage; a valid commit marker is the complete authority to
/// finish exact installation and journal completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    AbortedPrepared,
    AlreadyAborted,
    Completed,
    AlreadyCompleted,
}

/// Stable facts returned after an explicit, exact-ID recovery attempt.
///
/// `next_operation_id` is present only when the named operation was already
/// terminal and exactly one other valid incomplete transaction still blocks
/// new writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRecoveryResult {
    pub operation_id: OperationId,
    pub repository_id: String,
    pub prior_state: RecoveryState,
    pub outcome: RecoveryOutcome,
    pub next_operation_id: Option<OperationId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidatedPrivateResidueKind {
    Directory,
    RegularFile,
}

/// One exact runtime-owned entry, relative to the caller-supplied private
/// journal directory. Every entry has already passed the closed inventory and
/// content checks; symlinks and other filesystem kinds never appear here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidatedPrivateResidue {
    path: RepoPath,
    kind: ValidatedPrivateResidueKind,
}

impl ValidatedPrivateResidue {
    pub fn path(&self) -> &RepoPath {
        &self.path
    }

    pub fn kind(&self) -> ValidatedPrivateResidueKind {
        self.kind
    }
}

/// Immutable result of validating one Completed operation and the complete
/// private residue tree under the same held repository lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCompletedOperation {
    canonical_delta: CanonicalDelta,
    read_set: Vec<InputBinding>,
    private_residue: Vec<ValidatedPrivateResidue>,
}

impl VerifiedCompletedOperation {
    pub fn canonical_delta(&self) -> &CanonicalDelta {
        &self.canonical_delta
    }

    pub fn read_set(&self) -> &[InputBinding] {
        &self.read_set
    }

    pub fn private_residue(&self) -> &[ValidatedPrivateResidue] {
        &self.private_residue
    }
}

/// Exact caller-owned facts that a durable Completed operation must match.
/// The runtime verifies its private plan and history first, then compares these
/// fields without interpreting their product meaning.
#[derive(Debug, Clone, Copy)]
pub struct CompletedOperationExpectation<'a> {
    pub repository_id: &'a str,
    pub kind: &'a OperationKind,
    pub request_root: &'a ContentDigest,
    pub fixed_time: &'a str,
    pub result: &'a serde_json::Value,
}

/// Private durability boundaries used by the transaction test harness.
///
/// Journal writes are atomic, fsync-backed replacements. The corresponding
/// `Before*JournalWrite` and `After*JournalWrite` points therefore model the
/// only states the recovery contract promises: the old durable record, or the
/// complete new durable record. They deliberately do not pretend to model a
/// torn JSON file inside `operation_journal::write_json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryTxnStep {
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

#[cfg(any(test, feature = "test-support"))]
struct FailAtRepositoryTxnStep {
    target: RepositoryTxnStep,
}

#[cfg(any(test, feature = "test-support"))]
impl RepositoryTxnFailpoints for FailAtRepositoryTxnStep {
    fn check(&mut self, step: RepositoryTxnStep) -> Result<(), RepositoryTxnError> {
        if step == self.target {
            return Err(RepositoryTxnError::InjectedFailure { step });
        }
        Ok(())
    }
}

impl RepositoryTxn {
    /// Inspect the repository-wide recovery barrier without writing any
    /// journal or lock byte.
    ///
    /// This is a diagnostic snapshot for reporting an exact typed recovery
    /// requirement after a higher layer has erased an earlier error. When the
    /// repository lock already exists, the method holds it through the scan
    /// and returns [`RepositoryTxnError::Busy`] instead of mistaking a live
    /// writer's Prepared journal for a recovery instruction. A missing lock is
    /// not created, so that case remains a race-prone read-only snapshot. Every
    /// writer must still acquire [`Self::acquire_recovery_barrier`] and rely on
    /// its authoritative locked recheck.
    pub fn verify_recovery_barrier(
        repository_root: &Path,
        journal_dir: &Path,
    ) -> Result<(), RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let _lock = RepositoryWriteLock::try_acquire_existing(journal_dir, &root)?;
        ensure_recovery_barrier_locked(&root, journal_dir, None)
    }

    /// Verify one exact Completed operation without writing, recovering, or
    /// exposing its private journal. The existing repository lock is held for
    /// the entire inventory, marker, blob, history, and expectation check; its
    /// absence fails closed rather than creating a byte in this read-only path.
    pub fn verify_completed_operation(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
        expected: &CompletedOperationExpectation<'_>,
    ) -> Result<VerifiedCompletedOperation, RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let _lock = RepositoryWriteLock::try_acquire_existing(journal_dir, &root)?
            .ok_or(RepositoryTxnError::RepositoryLockMissing)?;
        let inventory = repository_inventory(&root, journal_dir)?;
        if let Some(pending) = validate_recovery_journals(&root, &inventory.journals)? {
            return Err(RepositoryTxnError::RecoveryRequired {
                operation_id: pending.operation_id.as_str().to_string(),
                state: pending.state,
            });
        }
        let journal = inventory
            .journals
            .iter()
            .find_map(|(_, journal)| {
                (journal.plan.operation_id == *operation_id).then_some(journal)
            })
            .ok_or_else(|| RepositoryTxnError::OperationNotFound {
                operation_id: operation_id.as_str().to_string(),
            })?;
        if !matches!(journal.recovery, RecoveryState::Completed) {
            return Err(RepositoryTxnError::CompletedOperationNotCompleted {
                operation_id: operation_id.as_str().to_string(),
                state: journal.recovery.clone(),
            });
        }
        RepositoryBinding::verify_repository_id(expected.repository_id)?;
        expected.kind.verify()?;
        expected.request_root.verify()?;
        let mismatch = [
            (
                "repository identity",
                journal.plan.repository.repository_id == expected.repository_id,
            ),
            ("operation kind", journal.plan.kind == *expected.kind),
            (
                "request root",
                journal.plan.request_root == *expected.request_root,
            ),
            ("fixed time", journal.plan.fixed_time == expected.fixed_time),
            ("result", journal.plan.result == *expected.result),
        ]
        .into_iter()
        .find_map(|(field, matches)| (!matches).then_some(field));
        if let Some(field) = mismatch {
            return Err(RepositoryTxnError::CompletedOperationExpectationMismatch {
                operation_id: operation_id.as_str().to_string(),
                field,
            });
        }
        Ok(VerifiedCompletedOperation {
            canonical_delta: journal.plan.canonical_delta.clone(),
            read_set: journal.plan.read_set.clone(),
            private_residue: inventory.private_residue,
        })
    }

    /// Acquire the repository-wide recovery barrier before loading mutable
    /// repository inputs for a new operation. The returned guard deliberately
    /// holds the write lock through planning and must be consumed by
    /// [`Self::prepare_with_barrier`].
    pub fn acquire_recovery_barrier(
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

    #[cfg(test)]
    fn prepare(
        repository_root: &Path,
        journal_dir: &Path,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
    ) -> Result<Self, RepositoryTxnError> {
        let barrier = Self::acquire_recovery_barrier(repository_root, journal_dir)?;
        Self::prepare_with_recovery_barrier_and_authorization(
            barrier,
            test_transaction_authorization(),
            plan,
            draft,
            &mut NoRepositoryTxnFailpoints,
        )
    }

    pub fn prepare_with_barrier(
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
        mut authorization: Box<dyn TransactionAuthorization>,
        plan: RepositoryTxnPlan,
        draft: DeltaDraft,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<Self, RepositoryTxnError> {
        plan.verify()?;
        if plan.canonical_delta != draft.delta {
            return Err(RepositoryTxnError::CorruptPlan(
                "plan delta differs from prepared postimage blobs".to_string(),
            ));
        }
        draft.verify()?;
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
                        authorization: None,
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

        let mut read_draft_blob = |blob: &JournalBlobRef| {
            let bytes = draft
                .blobs
                .get(&blob.digest)
                .cloned()
                .ok_or_else(|| RepositoryTxnError::MissingBlob(blob.digest.clone()))?;
            validate_blob_bytes(blob, &bytes)?;
            Ok(bytes)
        };
        authorization.bind_plan(&mut TransactionAuthorizationContext::new(
            &root,
            &plan.repository,
            &plan.root,
            &plan.canonical_delta,
            &mut read_draft_blob,
        ))?;

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
            test_transaction_authorization(),
            plan,
            draft,
            &mut FailAtRepositoryTxnStep { target: step },
        )
    }

    #[cfg(test)]
    fn open(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Self, RepositoryTxnError> {
        Self::open_if_present(repository_root, journal_dir, operation_id)?.ok_or_else(|| {
            RepositoryTxnError::OperationNotFound {
                operation_id: operation_id.as_str().to_string(),
            }
        })
    }

    fn open_for_recovery(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<(Self, Option<PendingRecovery>), RepositoryTxnError> {
        let root = canonical_repository_root(repository_root)?;
        let lock = RepositoryWriteLock::acquire(journal_dir, &root)?;
        let journals = repository_journals(&root, journal_dir)?;
        let pending = validate_recovery_journals(&root, &journals)?;
        let (paths, journal) = journals
            .into_iter()
            .find(|(_, journal)| journal.plan.operation_id == *operation_id)
            .ok_or_else(|| RepositoryTxnError::OperationNotFound {
                operation_id: operation_id.as_str().to_string(),
            })?;
        Ok((
            Self {
                root,
                paths,
                journal,
                authorization: None,
                _lock: lock,
            },
            pending,
        ))
    }

    #[cfg(test)]
    fn open_if_present(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
    ) -> Result<Option<Self>, RepositoryTxnError> {
        Self::open_if_present_impl(repository_root, journal_dir, operation_id)
    }

    #[cfg(test)]
    fn open_if_present_impl(
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
    fn plan(&self) -> &RepositoryTxnPlan {
        &self.journal.plan
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn recovery_state(&self) -> &RecoveryState {
        &self.journal.recovery
    }

    pub fn mark_committed(&mut self) -> Result<(), RepositoryTxnError> {
        self.mark_committed_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    fn read_commit_marker_once(&mut self) -> Result<CommitMarker, RepositoryTxnError> {
        let result = read_commit_marker(&self.paths, &self.journal);
        if !matches!(&result, Err(RepositoryTxnError::NotCommitted)) {
            self.authorization.take();
        }
        result
    }

    #[cfg(test)]
    fn bind_exact_test_authorization(&mut self) -> Result<(), RepositoryTxnError> {
        if !matches!(self.journal.recovery, RecoveryState::Prepared) {
            return Err(RepositoryTxnError::WriteAuthorizationNotApplicable {
                state: self.journal.recovery.clone(),
            });
        }
        let mut authorization = test_transaction_authorization();
        let mut read_journal_blob = |blob: &JournalBlobRef| read_blob_at(&self.paths, blob);
        authorization.bind_plan(&mut TransactionAuthorizationContext::new(
            &self.root,
            &self.journal.plan.repository,
            &self.journal.plan.root,
            &self.journal.plan.canonical_delta,
            &mut read_journal_blob,
        ))?;
        self.authorization = Some(authorization);
        Ok(())
    }

    fn mark_committed_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
        let expected_marker = CommitMarker::from_plan(&self.journal.plan);
        match self.read_commit_marker_once() {
            Ok(_) => {
                // A durable marker is sufficient authority for recovery. No
                // policy capability survives or participates beyond it.
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
                    self.verify_blobs()?;

                    let authorization = self
                        .authorization
                        .as_ref()
                        .expect("authorization checked above");
                    let mut read_journal_blob =
                        |blob: &JournalBlobRef| read_blob_at(&self.paths, blob);
                    authorization.revalidate_for_marker(&mut TransactionAuthorizationContext::new(
                        &self.root,
                        &self.journal.plan.repository,
                        &self.journal.plan.root,
                        &self.journal.plan.canonical_delta,
                        &mut read_journal_blob,
                    ))
                })();
                if let Err(error) = preflight {
                    self.authorization.take();
                    self.abort_prepared()?;
                    return Err(error);
                }
                failpoints.check(RepositoryTxnStep::BeforeCommitMarkerWrite)?;
                // From the first marker write onward, durable state may be
                // ambiguous to this process. Permission is one-shot and must
                // not survive any write error or unwind.
                self.authorization.take();
                operation_journal::write_json(&self.paths.marker, &expected_marker)
                    .map_err(RepositoryTxnError::Journal)?;
                failpoints.check(RepositoryTxnStep::AfterCommitMarkerWrite)?;
            }
            Err(error) => {
                // Only a definite NotCommitted result permits another
                // authorized attempt. A malformed or unreadable marker may
                // already represent a durable commit boundary.
                return Err(error);
            }
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

    /// Durably close a marker-free plan as Aborted. Its journal remains as the
    /// terminal record, its canonical postimage is not installed, and the same
    /// exact plan may safely reuse the operation id.
    pub fn abort_prepared(&mut self) -> Result<(), RepositoryTxnError> {
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
        match self.read_commit_marker_once() {
            Err(RepositoryTxnError::NotCommitted) => {
                self.authorization.take();
            }
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

    pub fn install(&mut self) -> Result<(), RepositoryTxnError> {
        self.install_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    fn install_with_failpoints(
        &mut self,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<(), RepositoryTxnError> {
        if matches!(self.journal.recovery, RecoveryState::Completed) {
            return self.verify_completed_state();
        }
        self.read_commit_marker_once()?;
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn install_at_failpoint(
        &mut self,
        step: RepositoryTxnStep,
    ) -> Result<(), RepositoryTxnError> {
        self.install_with_failpoints(&mut FailAtRepositoryTxnStep { target: step })
    }

    pub fn complete(&mut self) -> Result<(), RepositoryTxnError> {
        self.complete_with_failpoints(&mut NoRepositoryTxnFailpoints)
    }

    /// Retire private recovery copies after external publication and strict
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
    pub fn retire_completed_recovery_blobs(&mut self) -> Result<usize, RepositoryTxnError> {
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

    /// Recover one exact durable operation while holding the repository write
    /// lock.
    ///
    /// The operation identifier is mandatory: recovery never guesses between
    /// journals. A marker-free Prepared journal is safely aborted. Once an
    /// exact marker exists, recovery installs and completes the durable plan
    /// idempotently without loading or invoking caller policy. Any other
    /// incomplete journal remains a repository-wide barrier and is reported
    /// before this method changes either journal or repository bytes.
    ///
    /// `expected_repository_id` is an opaque caller-owned binding checked
    /// under the write lock before recovery mutates either journal or
    /// repository bytes.
    pub fn recover(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
        expected_repository_id: &str,
    ) -> Result<RepositoryRecoveryResult, RepositoryTxnError> {
        Self::recover_with_failpoints(
            repository_root,
            journal_dir,
            operation_id,
            expected_repository_id,
            &mut NoRepositoryTxnFailpoints,
        )
    }

    fn recover_with_failpoints(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
        expected_repository_id: &str,
        failpoints: &mut impl RepositoryTxnFailpoints,
    ) -> Result<RepositoryRecoveryResult, RepositoryTxnError> {
        let (mut txn, pending) =
            match Self::open_for_recovery(repository_root, journal_dir, operation_id) {
                Ok(opened) => opened,
                Err(RepositoryTxnError::MultiplePendingTransactions { operation_ids }) => {
                    return Err(RepositoryTxnError::AmbiguousRecovery {
                        requested_operation_id: operation_id.as_str().to_string(),
                        other_operation_ids: operation_ids
                            .into_iter()
                            .filter(|candidate| candidate != operation_id.as_str())
                            .collect(),
                    });
                }
                Err(error) => return Err(error),
            };
        let prior_state = txn.journal.recovery.clone();
        let durable_operation_id = txn.journal.plan.operation_id.clone();
        let repository_id = txn.journal.plan.repository.repository_id.clone();
        RepositoryBinding::verify_repository_id(expected_repository_id)?;
        if expected_repository_id != repository_id {
            return Err(RepositoryTxnError::RepositoryIdentityMismatch {
                expected: expected_repository_id.to_string(),
                actual: repository_id,
            });
        }
        let selected_is_terminal = matches!(
            prior_state,
            RecoveryState::Aborted | RecoveryState::Completed
        );
        let blocker = pending.filter(|pending| pending.operation_id != *operation_id);
        if !selected_is_terminal && blocker.is_some() {
            return Err(RepositoryTxnError::AmbiguousRecovery {
                requested_operation_id: operation_id.as_str().to_string(),
                other_operation_ids: blocker
                    .into_iter()
                    .map(|pending| pending.operation_id.as_str().to_string())
                    .collect(),
            });
        }
        if selected_is_terminal {
            let outcome = match prior_state {
                RecoveryState::Aborted => RecoveryOutcome::AlreadyAborted,
                RecoveryState::Completed => RecoveryOutcome::AlreadyCompleted,
                _ => unreachable!("selected terminal state checked above"),
            };
            let next_operation_id = blocker.map(|pending| pending.operation_id);
            return Ok(RepositoryRecoveryResult {
                operation_id: durable_operation_id,
                repository_id,
                prior_state,
                outcome,
                next_operation_id,
            });
        }

        match read_commit_marker(&txn.paths, &txn.journal) {
            Ok(_) => {}
            Err(RepositoryTxnError::NotCommitted)
                if matches!(txn.journal.recovery, RecoveryState::Prepared) =>
            {
                txn.abort_prepared_with_failpoints(failpoints)?;
                ensure_recovery_barrier_locked(&txn.root, journal_dir, None)?;
                return Ok(RepositoryRecoveryResult {
                    operation_id: durable_operation_id,
                    repository_id,
                    prior_state,
                    outcome: RecoveryOutcome::AbortedPrepared,
                    next_operation_id: None,
                });
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
        txn.install_with_failpoints(failpoints)?;
        txn.complete_with_failpoints(failpoints)?;
        ensure_recovery_barrier_locked(&txn.root, journal_dir, None)?;
        Ok(RepositoryRecoveryResult {
            operation_id: durable_operation_id,
            repository_id,
            prior_state,
            outcome: RecoveryOutcome::Completed,
            next_operation_id: None,
        })
    }

    /// Exercise the exact production recovery engine with one injected durable
    /// interruption. This narrow seam is available only to in-crate tests and
    /// the non-default `test-support` feature used by product-boundary crash
    /// tests; it carries no authorization capability or alternate recovery
    /// policy.
    #[cfg(any(test, feature = "test-support"))]
    pub fn recover_at_failpoint(
        repository_root: &Path,
        journal_dir: &Path,
        operation_id: &OperationId,
        expected_repository_id: &str,
        step: RepositoryTxnStep,
    ) -> Result<RepositoryRecoveryResult, RepositoryTxnError> {
        Self::recover_with_failpoints(
            repository_root,
            journal_dir,
            operation_id,
            expected_repository_id,
            &mut FailAtRepositoryTxnStep { target: step },
        )
    }

    pub fn resolved_public_writes(&self) -> Result<Vec<ResolvedWrite>, RepositoryTxnError> {
        resolve_public_writes(&self.journal.plan.canonical_delta, |blob| {
            match self.journal.blob_retention {
                BlobRetention::Retained => self.read_blob(blob),
                BlobRetention::Pruned => self.read_pruned_blob_from_current(blob),
            }
        })
    }

    pub fn canonical_delta_root(&self) -> &str {
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
pub enum RepositoryTxnError {
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
    RepositoryIdentityMismatch {
        expected: String,
        actual: String,
    },
    MixedRepositoryIdentities {
        repository_ids: Vec<String>,
    },
    OperationConflict {
        operation_id: String,
    },
    OperationNotFound {
        operation_id: String,
    },
    AmbiguousRecovery {
        requested_operation_id: String,
        other_operation_ids: Vec<String>,
    },
    MultiplePendingTransactions {
        operation_ids: Vec<String>,
    },
    RepositoryLockMissing,
    CompletedOperationNotCompleted {
        operation_id: String,
        state: RecoveryState,
    },
    CompletedOperationExpectationMismatch {
        operation_id: String,
        field: &'static str,
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
    #[cfg(any(test, feature = "test-support"))]
    InjectedFailure {
        step: RepositoryTxnStep,
    },
    Canonicalize(String),
    WriteAuthorization(String),
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
            Self::RepositoryIdentityMismatch { expected, actual } => write!(
                formatter,
                "repository identity mismatch: expected {expected}, durable transaction is bound to {actual}"
            ),
            Self::MixedRepositoryIdentities { repository_ids } => write!(
                formatter,
                "repository journal set contains mixed repository identities: {}",
                repository_ids.join(", ")
            ),
            Self::OperationConflict { operation_id } => write!(
                formatter,
                "operation id {operation_id} is already bound to a different plan"
            ),
            Self::OperationNotFound { operation_id } => write!(
                formatter,
                "repository transaction {operation_id} was not found"
            ),
            Self::AmbiguousRecovery {
                requested_operation_id,
                other_operation_ids,
            } => write!(
                formatter,
                "cannot recover repository transaction {requested_operation_id} while other incomplete transactions exist: {}; recover an exact unambiguous journal set",
                other_operation_ids.join(", ")
            ),
            Self::MultiplePendingTransactions { operation_ids } => write!(
                formatter,
                "repository contains multiple incomplete transactions and cannot emit one exact recovery action: {}",
                operation_ids.join(", ")
            ),
            Self::RepositoryLockMissing => write!(
                formatter,
                "repository has durable transaction journals but no existing write lock to hold for read-only verification"
            ),
            Self::CompletedOperationNotCompleted {
                operation_id,
                state,
            } => write!(
                formatter,
                "repository transaction {operation_id} is {}, not completed",
                state.as_str()
            ),
            Self::CompletedOperationExpectationMismatch {
                operation_id,
                field,
            } => write!(
                formatter,
                "completed repository transaction {operation_id} does not match expected {field}"
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
            #[cfg(any(test, feature = "test-support"))]
            Self::InjectedFailure { step } => {
                write!(
                    formatter,
                    "injected repository transaction failure at {step:?}"
                )
            }
            Self::Canonicalize(error) => write!(formatter, "canonicalize transaction: {error}"),
            Self::WriteAuthorization(error) => {
                write!(formatter, "repository write authorization: {error}")
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
    let root = fs::canonicalize(path).map_err(|error| {
        RepositoryTxnError::Io(format!(
            "canonicalize repository root {}: {error}",
            path.display()
        ))
    })?;
    if root.to_str().is_none() {
        return Err(RepositoryTxnError::Io(format!(
            "canonical repository root is not valid UTF-8: {}",
            root.display()
        )));
    }
    Ok(root)
}

/// Reject path escapes, symbolic links, and non-directory ancestors observed
/// while resolving a transaction target.
///
/// This is a fail-closed check for a stable filesystem plus the cooperative
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
mod tests;
