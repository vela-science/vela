//! Decisions self-publish.
//!
//! Once a human key has signed, nothing that follows is a decision —
//! materializing derived views, staging the store, writing the commit
//! message, pushing — it is mechanical consequence, and mechanical
//! consequence is the substrate's job. Before this module, a reviewer
//! expressed one intention ("accept this") in four acts and the signed
//! decision routinely rotted uncommitted on one laptop, invisible to CI,
//! the hub, and everyone else.
//!
//! Custody note: nothing here signs anything. Publication only carries
//! events a key already signed; withholding publication was never a
//! second decision, just friction.
//!
//! Failure posture: publication never fails the decision. The signed
//! event is already in the store; a git hiccup degrades to a warning
//! with the exact manual command.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PUBLICATION_BUSY_REASON: &str = "another Git publication owns this repository";
const PUBLICATION_BUSY_RETRY_REASON: &str =
    "another Git publication owns this repository; retry after it completes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum PublicationState {
    /// The selected target commit already contains every exact postimage, so
    /// this publication attempt moved no ref and rewrote no caller index.
    Unchanged {
        commit: String,
    },
    Uncommitted {
        candidate: Option<String>,
        reason: String,
    },
    Stale {
        candidate: String,
        expected: String,
        actual: String,
    },
    CommittedLocal {
        commit: String,
    },
    Pushed {
        commit: String,
        remote: String,
    },
    Unknown {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PublicationOutcome {
    #[serde(flatten)]
    pub state: PublicationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_command: Option<String>,
}

pub(crate) fn publication_is_busy(outcome: &PublicationOutcome) -> bool {
    matches!(
        &outcome.state,
        PublicationState::Uncommitted { reason, .. }
            if reason == PUBLICATION_BUSY_REASON
                || reason == PUBLICATION_BUSY_RETRY_REASON
    )
}

/// The public, repository-relative projection of one committed frontier
/// transaction entry. `postimage == None` is an exact deletion; otherwise the
/// supplied bytes, not a later worktree scan, define the Git candidate.
///
/// This DTO deliberately does not depend on `FrontierTxn`: the transaction
/// layer resolves and digest-checks its private journal blobs before crossing
/// this publication boundary.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicationDeltaEntry {
    pub path: String,
    pub preimage_sha256: Option<String>,
    pub postimage: Option<Vec<u8>>,
    pub executable: bool,
}

/// A sorted, unique public projection of one `FrontierTxn` delta. `root` is
/// the caller-supplied canonical-delta root; publication additionally computes
/// and binds its own digest over this complete in-memory projection.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicationDelta {
    pub root: String,
    pub entries: Vec<PublicationDeltaEntry>,
}

/// The only typed exact-publication failure: the caller changed the supplied
/// public projection after preflight. Operational Git failures remain honest
/// `PublicationOutcome`s, matching the established publication API.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExactPublicationError {
    DeltaChanged {
        expected_sha256: String,
        actual_sha256: String,
    },
}

impl std::fmt::Display for ExactPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeltaChanged {
                expected_sha256,
                actual_sha256,
            } => write!(
                formatter,
                "exact publication delta changed after preflight (expected {expected_sha256}, got {actual_sha256})"
            ),
        }
    }
}

impl std::error::Error for ExactPublicationError {}

impl PublicationOutcome {
    fn uncommitted(reason: impl Into<String>) -> Self {
        Self {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: reason.into(),
            },
            recovery_command: None,
        }
    }

    fn unknown(reason: impl Into<String>) -> Self {
        Self {
            state: PublicationState::Unknown {
                reason: reason.into(),
            },
            recovery_command: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct GitOid {
    object_format: String,
    hex: String,
}

impl GitOid {
    fn parse(object_format: &str, value: &str) -> Result<Self, String> {
        let expected_len = match object_format {
            "sha1" => 40,
            "sha256" => 64,
            other => return Err(format!("unsupported Git object format `{other}`")),
        };
        if value.len() != expected_len || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!(
                "invalid full {object_format} Git object id `{value}`"
            ));
        }
        Ok(Self {
            object_format: object_format.to_string(),
            hex: value.to_ascii_lowercase(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct GitRefName(String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum TargetCheckoutState {
    Current { worktree: String },
    UncheckedOut,
    CheckedOutElsewhere { worktree: String },
}

#[derive(Debug, Clone)]
struct DesiredEntry {
    mode: String,
    oid: GitOid,
    bytes: Vec<u8>,
    content_mode: ContentMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentMode {
    Raw,
    Lfs,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LfsPointer {
    oid: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JournalEntry {
    path: String,
    mode: Option<String>,
    oid: Option<GitOid>,
    worktree_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PublicationScope {
    ExactDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PublicationJournal {
    schema: String,
    operation_id: String,
    repository: String,
    frontier: String,
    target_refname: GitRefName,
    target_checkout: TargetCheckoutState,
    expected_git_commit_oid: GitOid,
    candidate_tree_oid: GitOid,
    candidate_commit_oid: Option<GitOid>,
    message: String,
    author_name: String,
    author_email: String,
    commit_date: String,
    entries: Vec<JournalEntry>,
    scope: PublicationScope,
    lfs_objects: Vec<LfsPointer>,
    original_index: IndexMap,
    original_index_sha256: String,
    ref_moved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CompletedPublication {
    schema: String,
    operation_id: String,
    outcome: PublicationOutcome,
}

enum JournalIndexReconcileError {
    Refused(String),
    Retryable(String),
}

struct GitPublicationTxn {
    target_refname: GitRefName,
    target_checkout: TargetCheckoutState,
    expected_git_commit_oid: GitOid,
    candidate_tree_oid: GitOid,
    candidate_commit_oid: Option<GitOid>,
    lfs_objects: Vec<LfsPointer>,
}

#[derive(Debug)]
struct PublicationLock {
    _file: File,
    repository: PathBuf,
}

struct RealIndexLock {
    file: Option<File>,
    path: PathBuf,
}

impl RealIndexLock {
    fn acquire(index_path: &Path) -> Result<Self, String> {
        let mut lock_name = index_path.as_os_str().to_os_string();
        lock_name.push(".lock");
        let path = PathBuf::from(lock_name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| format!("acquire real Git index lock {}: {error}", path.display()))?;
        Ok(Self {
            file: Some(file),
            path,
        })
    }

    fn install(mut self, index_path: &Path, bytes: &[u8]) -> Result<(), String> {
        let file = self.file.as_mut().expect("live index lock");
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("write real Git index lock: {error}"))?;
        self.file.take();
        fs::rename(&self.path, index_path)
            .map_err(|error| format!("atomically install reconciled Git index: {error}"))?;
        if let Some(parent) = index_path.parent() {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("fsync Git index directory: {error}"))?;
        }
        Ok(())
    }
}

impl Drop for RealIndexLock {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

enum PublicationLockError {
    Busy,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpstreamTarget {
    remote: String,
    reference: String,
}

/// A held exact-delta publication lease. It binds repository/frontier
/// identity, target ref and expected commit, caller index bytes, and the
/// complete sorted public projection digest while retaining the repository's
/// publication lock through local ref movement.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ExactPublicationPreflight {
    repository: PathBuf,
    frontier: PathBuf,
    target_refname: GitRefName,
    target_checkout: TargetCheckoutState,
    expected_git_commit_oid: GitOid,
    original_index: IndexMap,
    original_index_sha256: String,
    allowed_input_hashes: BTreeMap<String, String>,
    delta_sha256: String,
    disposition: ExactPublicationDisposition,
    publication_lock: PublicationLock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExactPublicationDisposition {
    Pending,
    AlreadyPublished { commit: GitOid },
}

struct GitRunner {
    root: PathBuf,
    empty_hooks: PathBuf,
    empty_attributes: PathBuf,
    attribute_source: Option<String>,
}

impl GitRunner {
    fn run(
        &self,
        args: &[OsString],
        stdin: Option<&[u8]>,
        index: Option<&Path>,
        identity: Option<(&str, &str, &str)>,
    ) -> Result<Output, String> {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.root)
            .arg("-c")
            .arg(format!("core.hooksPath={}", self.empty_hooks.display()))
            .arg("-c")
            .arg("core.fsmonitor=false")
            .arg("-c")
            .arg(format!(
                "core.attributesFile={}",
                self.empty_attributes.display()
            ))
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, _) in std::env::vars_os() {
            if name.to_str().is_some_and(|name| name.starts_with("GIT_")) {
                command.env_remove(name);
            }
        }
        command.env("GIT_OPTIONAL_LOCKS", "0");
        command.env("GIT_LITERAL_PATHSPECS", "1");
        command.env("GIT_ATTR_NOSYSTEM", "1");
        if let Some(source) = &self.attribute_source {
            command.env("GIT_ATTR_SOURCE", source);
        }
        if let Some(path) = index {
            command.env("GIT_INDEX_FILE", path);
        }
        if let Some((name, email, date)) = identity {
            command
                .env("GIT_AUTHOR_NAME", name)
                .env("GIT_AUTHOR_EMAIL", email)
                .env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_NAME", name)
                .env("GIT_COMMITTER_EMAIL", email)
                .env("GIT_COMMITTER_DATE", date);
        }
        if stdin.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("start git: {error}"))?;
        if let Some(bytes) = stdin {
            child
                .stdin
                .as_mut()
                .ok_or_else(|| "git stdin was not available".to_string())?
                .write_all(bytes)
                .map_err(|error| format!("write git stdin: {error}"))?;
        }
        child
            .wait_with_output()
            .map_err(|error| format!("wait for git: {error}"))
    }

    fn checked(
        &self,
        args: &[OsString],
        stdin: Option<&[u8]>,
        index: Option<&Path>,
    ) -> Result<Vec<u8>, String> {
        let output = self.run(args, stdin, index, None)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn text(&self, args: &[&str]) -> Result<String, String> {
        let args = args.iter().map(OsString::from).collect::<Vec<_>>();
        let bytes = self.checked(&args, None, None)?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_string())
    }

    fn pin_attributes(&mut self, source: &GitOid) {
        self.attribute_source = Some(source.hex.clone());
    }
}

fn sanitized_git_runner(frontier: &Path) -> Result<(tempfile::TempDir, GitRunner), String> {
    let temporary = tempfile::tempdir().map_err(|error| format!("publication tempdir: {error}"))?;
    let empty_hooks = temporary.path().join("hooks");
    let empty_attributes = temporary.path().join("attributes");
    fs::create_dir(&empty_hooks)
        .map_err(|error| format!("create empty hooks directory: {error}"))?;
    fs::write(&empty_attributes, [])
        .map_err(|error| format!("create empty global attributes file: {error}"))?;
    let bootstrap = GitRunner {
        root: frontier.to_path_buf(),
        empty_hooks: empty_hooks.clone(),
        empty_attributes: empty_attributes.clone(),
        attribute_source: None,
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    Ok((
        temporary,
        GitRunner {
            root,
            empty_hooks,
            empty_attributes,
            attribute_source: None,
        },
    ))
}

/// Resolve the private operation-journal root shared by frontier and Git
/// transactions. Resolution uses the same sanitized Git environment as
/// publication. A non-Git frontier has no safe implicit fallback: callers get
/// an error and must not place transaction state in the public frontier.
#[allow(dead_code)]
pub(crate) fn publication_journal_dir(frontier: &Path) -> Result<PathBuf, String> {
    let (_temporary, runner) = sanitized_git_runner(frontier)?;
    git_private_path(&runner, "vela/operation-journals")
}

/// Convert a transaction's normalized frontier-relative path into the
/// normalized Git-root-relative coordinate consumed by `PublicationDelta`.
/// This performs no write and does not infer publicness; the exact-delta
/// validator separately checks the resolved frontier topology.
#[allow(dead_code)]
pub(crate) fn publication_repo_relative_path(
    frontier: &Path,
    frontier_relative: &str,
) -> Result<String, String> {
    if frontier_relative.is_empty()
        || frontier_relative.contains('\\')
        || frontier_relative.chars().any(char::is_control)
    {
        return Err(
            "publication path is empty, non-portable, or contains control bytes".to_string(),
        );
    }
    let relative = Path::new(frontier_relative);
    let components = relative.components().collect::<Vec<_>>();
    let normalized = components
        .iter()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value),
            _ => None,
        })
        .collect::<PathBuf>();
    let unsafe_component = components.iter().any(|component| match component {
        std::path::Component::Normal(value) => value.to_str().is_none_or(|value| {
            value
                .trim_end_matches([' ', '.'])
                .eq_ignore_ascii_case(".git")
                || value.starts_with(':')
                || value.contains(['*', '?', '['])
        }),
        _ => true,
    });
    if components.is_empty() || unsafe_component || normalized.as_os_str() != relative.as_os_str() {
        return Err(format!(
            "publication path is absolute, parent-bearing, Git-private, magic, or non-normalized: {frontier_relative}"
        ));
    }

    let (_temporary, runner) = sanitized_git_runner(frontier)?;
    let frontier_abs = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let frontier_prefix = frontier_abs
        .strip_prefix(&runner.root)
        .map_err(|_| "frontier is outside the resolved Git worktree".to_string())?;
    let repository_relative = frontier_prefix.join(relative);
    reject_symlink_ancestors(&runner.root, &repository_relative)?;
    repository_relative
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| "non-UTF-8 frontier paths are not publishable".to_string())
}

pub(crate) struct PublishOptions {
    pub no_push: bool,
    /// Push even when config resolves `publish.git_push` to "off" — the explicit
    /// `--push` flag. Ordinary calls leave this false, so the default
    /// (commit locally, do not push) holds and publishing stays deliberate.
    pub force_push: bool,
    /// An explicit local branch is required when HEAD is detached. Keeping the
    /// target in the options also makes un-checked-out publication testable
    /// without teaching the ordinary CLI a second default.
    pub target_refname: Option<String>,
    pub preflight_inputs: Vec<PathBuf>,
    #[cfg(test)]
    test_step: PublicationTestStep,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublicationTestStep {
    None,
    InterruptAfterPreparedJournal,
    InterruptAfterCommitTree,
    InterruptAfterCandidateJournal,
    AdvanceRefBeforeFinalObservation,
    AdvanceRefAfterFinalObservation,
    InterruptAfterRefCasBeforeMarker,
    FailCompletedRecordWrite,
    FailActiveJournalRemove,
    FailCompletedRecordPrune,
}

impl PublishOptions {
    pub(crate) fn new(no_push: bool) -> Self {
        Self {
            no_push,
            force_push: false,
            target_refname: None,
            preflight_inputs: Vec::new(),
            #[cfg(test)]
            test_step: PublicationTestStep::None,
        }
    }

    /// Explicit publish: commit locally and push regardless of config.
    pub(crate) fn pushing() -> Self {
        Self {
            no_push: false,
            force_push: true,
            target_refname: None,
            preflight_inputs: Vec::new(),
            #[cfg(test)]
            test_step: PublicationTestStep::None,
        }
    }

    pub(crate) fn with_preflight_inputs(mut self, paths: Vec<PathBuf>) -> Self {
        self.preflight_inputs = paths;
        self
    }

    #[cfg(test)]
    fn at_test_step(mut self, step: PublicationTestStep) -> Self {
        self.test_step = step;
        self
    }
}

/// Establish a publication lease before a `FrontierTxn` installs its public
/// postimages. The delta must already be strictly sorted and complete. This
/// preflight checks its exact preimages and rejects unrelated public dirt
/// without materializing or deriving a desired mapping from the worktree.
#[allow(dead_code)]
pub(crate) fn exact_publication_preflight(
    frontier: &Path,
    delta: &PublicationDelta,
    opts: &PublishOptions,
) -> Result<ExactPublicationPreflight, PublicationOutcome> {
    if let Some(reason) = publication_disabled_reason(frontier, opts) {
        return Err(PublicationOutcome::uncommitted(reason));
    }
    exact_publication_preflight_inner(frontier, delta, opts)
        .map_err(PublicationOutcome::uncommitted)
}

/// Reacquire an exact publication lease after the scientific transaction has
/// durably installed its postimages. Unlike the ordinary preflight, this seam
/// never expects preimage bytes in the worktree. It instead validates the
/// current target tree against the caller-bound preimages, validates the exact
/// postimages in the worktree, and records when the target already contains
/// those postimages so publication can return idempotently without creating a
/// second commit.
#[allow(dead_code)]
pub(crate) fn exact_publication_resume_preflight(
    frontier: &Path,
    delta: &PublicationDelta,
    opts: &PublishOptions,
) -> Result<ExactPublicationPreflight, PublicationOutcome> {
    if let Some(reason) = publication_disabled_reason(frontier, opts) {
        return Err(PublicationOutcome::uncommitted(reason));
    }
    exact_publication_resume_preflight_inner(frontier, delta, opts)
        .map_err(PublicationOutcome::uncommitted)
}

/// Discover the immutable Git publication that originally introduced one
/// exact, content-addressed receipt anchor. This is a read-only recovery seam:
/// it does not depend on private operation journals or the current worktree
/// matching an older delta, so exact retries remain attributable after later
/// publications and in clean clones.
///
/// A match must be unique in the selected target ref's ancestry. Its sole
/// parent must match every bound preimage, its tree must match every supplied
/// postimage, and its changed paths must equal the complete delta. The outcome
/// is reported as pushed only when the configured upstream's observed tip is
/// locally and cryptographically provable to descend from the matched commit.
#[allow(dead_code)]
pub(crate) fn discover_exact_publication(
    frontier: &Path,
    delta: &PublicationDelta,
    anchor_path: &str,
    opts: &PublishOptions,
) -> Result<Option<PublicationOutcome>, String> {
    discover_exact_publication_inner(frontier, delta, anchor_path, opts)
}

/// Clean-clone fallback for exact landing retries when the private
/// `FrontierTxn` delta is unavailable. The receipt bytes are the immutable Git
/// anchor; `operation_id` and `receipt_root` must also appear together in one
/// proposal introduced by the same commit. This reports provenance only and
/// never creates, moves, or pushes a ref.
#[allow(dead_code)]
pub(crate) fn discover_receipt_publication(
    frontier: &Path,
    receipt_bytes: &[u8],
    receipt_root: &str,
    operation_id: &str,
    opts: &PublishOptions,
) -> Result<Option<PublicationOutcome>, String> {
    discover_receipt_publication_inner(frontier, receipt_bytes, receipt_root, operation_id, opts)
}

/// Return the deliberate, non-Git reason publication is disabled, if any.
///
/// This check intentionally performs no repository discovery. Scientific
/// transactions use it before constructing a Git-relative delta. The worktree
/// cannot inject `VELA_NO_PUBLISH` because Vela never loads frontier-local
/// dotenv files.
pub(crate) fn publication_disabled_reason(
    _frontier: &Path,
    _opts: &PublishOptions,
) -> Option<String> {
    if !cfg!(test) && std::env::var("VELA_NO_PUBLISH").is_ok_and(|value| value == "1") {
        return Some("publication disabled by VELA_NO_PUBLISH".to_string());
    }
    None
}

fn exact_publication_preflight_inner(
    frontier: &Path,
    delta: &PublicationDelta,
    opts: &PublishOptions,
) -> Result<ExactPublicationPreflight, String> {
    let temporary = tempfile::tempdir().map_err(|error| format!("publication tempdir: {error}"))?;
    let empty_hooks = temporary.path().join("hooks");
    let empty_attributes = temporary.path().join("attributes");
    fs::create_dir(&empty_hooks)
        .map_err(|error| format!("create empty hooks directory: {error}"))?;
    fs::write(&empty_attributes, [])
        .map_err(|error| format!("create empty global attributes file: {error}"))?;
    let bootstrap = GitRunner {
        root: frontier.to_path_buf(),
        empty_hooks: empty_hooks.clone(),
        empty_attributes: empty_attributes.clone(),
        attribute_source: None,
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let mut runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
        attribute_source: None,
    };
    let publication_lock = acquire_publication_lock(&runner).map_err(|error| match error {
        PublicationLockError::Busy => PUBLICATION_BUSY_RETRY_REASON.to_string(),
        PublicationLockError::Failed(reason) => reason,
    })?;
    reject_local_attribute_overrides(&runner)?;
    let target_refname = resolve_target_ref(&runner, opts.target_refname.as_deref())?;
    let target_checkout = target_checkout_state(&runner, &target_refname)?;
    if let TargetCheckoutState::CheckedOutElsewhere { worktree } = &target_checkout {
        return Err(format!(
            "target ref {} is checked out in another worktree at {worktree}",
            target_refname.0
        ));
    }
    let frontier_abs = frontier
        .canonicalize()
        .unwrap_or_else(|_| frontier.to_path_buf());
    if !frontier_abs.starts_with(&root) {
        return Err("frontier is outside the resolved Git worktree".to_string());
    }
    let specs = frontier_specs(&frontier_abs, &root)?;
    validate_publication_delta(delta, &root, &specs)?;
    let paths = delta_paths(delta);
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let expected = GitOid::parse(
        &object_format,
        &runner.text(&["rev-parse", &format!("{}^{{commit}}", target_refname.0)])?,
    )?;
    runner.pin_attributes(&expected);
    reject_unsupported_index(&runner, &specs, &object_format)?;
    let parent = tree_entries(&runner, &expected, &specs)?;
    reject_parent_case_collisions(&paths, &parent)?;

    // Public inputs named by the receipt are read-only transaction inputs.
    // They may already exist (and be untracked or modified) because the
    // canonical transaction copies their verified bytes into its own
    // content-addressed record path. Bind and recheck them, but never sweep
    // their worktree paths into the publication candidate.
    let allowed_input_hashes =
        capture_preflight_input_hashes(&runner, &frontier_abs, &opts.preflight_inputs)?;
    let staged_inputs = git_paths(
        &runner,
        vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--name-only"),
            OsString::from("-z"),
            OsString::from(&expected.hex),
            OsString::from("--"),
        ],
        &specs,
    )?;
    if let Some(path) = staged_inputs
        .iter()
        .find(|path| allowed_input_hashes.contains_key(*path))
    {
        return Err(format!(
            "explicit preflight input {path} is staged; unstage it so exact publication can bind the worktree bytes without consuming caller-owned index state"
        ));
    }
    let mut allowed_paths = paths.clone();
    allowed_paths.extend(allowed_input_hashes.keys().cloned());

    let attribute_index = temporary.path().join("exact-preflight.index");
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&expected.hex)],
        None,
        Some(&attribute_index),
    )?;
    reject_unrelated_public_dirt(
        &runner,
        &expected,
        &specs,
        &allowed_paths,
        &parent,
        &object_format,
        &attribute_index,
    )?;
    validate_exact_preimages(
        &runner,
        delta,
        &parent,
        &specs,
        &object_format,
        &attribute_index,
    )?;
    let (original_index, original_index_sha256) = capture_index(&runner)?;
    Ok(ExactPublicationPreflight {
        repository: root,
        frontier: frontier_abs,
        target_refname,
        target_checkout,
        expected_git_commit_oid: expected,
        original_index,
        original_index_sha256,
        allowed_input_hashes,
        delta_sha256: publication_delta_sha256(delta),
        disposition: ExactPublicationDisposition::Pending,
        publication_lock,
    })
}

fn exact_publication_resume_preflight_inner(
    frontier: &Path,
    delta: &PublicationDelta,
    opts: &PublishOptions,
) -> Result<ExactPublicationPreflight, String> {
    let temporary = tempfile::tempdir().map_err(|error| format!("publication tempdir: {error}"))?;
    let empty_hooks = temporary.path().join("hooks");
    let empty_attributes = temporary.path().join("attributes");
    fs::create_dir(&empty_hooks)
        .map_err(|error| format!("create empty hooks directory: {error}"))?;
    fs::write(&empty_attributes, [])
        .map_err(|error| format!("create empty global attributes file: {error}"))?;
    let bootstrap = GitRunner {
        root: frontier.to_path_buf(),
        empty_hooks: empty_hooks.clone(),
        empty_attributes: empty_attributes.clone(),
        attribute_source: None,
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let mut runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
        attribute_source: None,
    };
    let publication_lock = acquire_publication_lock(&runner).map_err(|error| match error {
        PublicationLockError::Busy => PUBLICATION_BUSY_RETRY_REASON.to_string(),
        PublicationLockError::Failed(reason) => reason,
    })?;
    reject_local_attribute_overrides(&runner)?;
    let target_refname = resolve_target_ref(&runner, opts.target_refname.as_deref())?;
    let target_checkout = target_checkout_state(&runner, &target_refname)?;
    if let TargetCheckoutState::CheckedOutElsewhere { worktree } = &target_checkout {
        return Err(format!(
            "target ref {} is checked out in another worktree at {worktree}",
            target_refname.0
        ));
    }
    let frontier_abs = frontier
        .canonicalize()
        .unwrap_or_else(|_| frontier.to_path_buf());
    if !frontier_abs.starts_with(&root) {
        return Err("frontier is outside the resolved Git worktree".to_string());
    }
    let specs = frontier_specs(&frontier_abs, &root)?;
    validate_publication_delta(delta, &root, &specs)?;
    let paths = delta_paths(delta);
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let expected = GitOid::parse(
        &object_format,
        &runner.text(&["rev-parse", &format!("{}^{{commit}}", target_refname.0)])?,
    )?;
    runner.pin_attributes(&expected);
    reject_unsupported_index(&runner, &specs, &object_format)?;
    let parent = tree_entries(&runner, &expected, &specs)?;
    reject_parent_case_collisions(&paths, &parent)?;

    let allowed_input_hashes =
        capture_preflight_input_hashes(&runner, &frontier_abs, &opts.preflight_inputs)?;
    let staged_inputs = git_paths(
        &runner,
        vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--name-only"),
            OsString::from("-z"),
            OsString::from(&expected.hex),
            OsString::from("--"),
        ],
        &specs,
    )?;
    if let Some(path) = staged_inputs
        .iter()
        .find(|path| allowed_input_hashes.contains_key(*path))
    {
        return Err(format!(
            "explicit preflight input {path} is staged; unstage it so exact publication can bind the worktree bytes without consuming caller-owned index state"
        ));
    }
    let mut allowed_paths = paths;
    allowed_paths.extend(allowed_input_hashes.keys().cloned());

    let attribute_index = temporary.path().join("exact-resume-preflight.index");
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&expected.hex)],
        None,
        Some(&attribute_index),
    )?;
    reject_unrelated_public_dirt(
        &runner,
        &expected,
        &specs,
        &allowed_paths,
        &parent,
        &object_format,
        &attribute_index,
    )?;
    let (desired, _) = exact_desired_entries(
        &runner,
        delta,
        &root,
        &specs,
        &object_format,
        &attribute_index,
    )?;
    let target_has_preimages =
        target_matches_exact_preimages(&runner, delta, &parent, &specs, &attribute_index)?;
    let target_has_postimages = target_matches_exact_postimages(&parent, &desired);
    let disposition = if target_has_postimages {
        ExactPublicationDisposition::AlreadyPublished {
            commit: expected.clone(),
        }
    } else if target_has_preimages {
        ExactPublicationDisposition::Pending
    } else {
        return Err(format!(
            "target ref {} matches neither the bound exact-publication preimages nor its postimages",
            target_refname.0
        ));
    };
    let (original_index, original_index_sha256) = capture_index(&runner)?;
    Ok(ExactPublicationPreflight {
        repository: root,
        frontier: frontier_abs,
        target_refname,
        target_checkout,
        expected_git_commit_oid: expected,
        original_index,
        original_index_sha256,
        allowed_input_hashes,
        delta_sha256: publication_delta_sha256(delta),
        disposition,
        publication_lock,
    })
}

fn discover_exact_publication_inner(
    frontier: &Path,
    delta: &PublicationDelta,
    anchor_path: &str,
    opts: &PublishOptions,
) -> Result<Option<PublicationOutcome>, String> {
    let (temporary, runner) = sanitized_git_runner(frontier)?;
    reject_local_attribute_overrides(&runner)?;
    let frontier_abs = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    if !frontier_abs.starts_with(&runner.root) {
        return Err("frontier is outside the resolved Git worktree".to_string());
    }
    let specs = frontier_specs(&frontier_abs, &runner.root)?;
    validate_publication_delta(delta, &runner.root, &specs)?;
    let anchor = delta
        .entries
        .iter()
        .find(|entry| entry.path == anchor_path)
        .ok_or_else(|| {
            "exact publication anchor is not present in the supplied delta".to_string()
        })?;
    if anchor.preimage_sha256.is_some() || anchor.postimage.is_none() || anchor.executable {
        return Err(
            "exact publication anchor must be one newly introduced non-executable file".to_string(),
        );
    }

    let target_refname = resolve_target_ref(&runner, opts.target_refname.as_deref())?;
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let history = runner.checked(
        &[
            OsString::from("rev-list"),
            OsString::from("--full-history"),
            OsString::from("--topo-order"),
            OsString::from(&target_refname.0),
            OsString::from("--"),
            OsString::from(anchor_path),
        ],
        None,
        None,
    )?;
    let mut matches = Vec::new();
    for line in String::from_utf8(history)
        .map_err(|_| "Git ancestry output is not UTF-8".to_string())?
        .lines()
    {
        let commit = GitOid::parse(&object_format, line)?;
        let parents = commit_parents(&runner, &commit, &object_format)?;
        if parents.len() != 1 {
            continue;
        }
        if historical_exact_delta_matches(
            &runner,
            delta,
            &parents[0],
            &commit,
            &specs,
            temporary.path(),
        )? {
            matches.push(commit);
        }
    }
    let commit = match matches.as_slice() {
        [] => return Ok(None),
        [commit] => commit.clone(),
        _ => {
            return Err(format!(
                "exact publication anchor has {} fully verified introducing commits in target-ref ancestry",
                matches.len()
            ));
        }
    };
    Ok(Some(discovered_publication_outcome(
        &runner,
        &target_refname,
        commit,
    )))
}

fn discover_receipt_publication_inner(
    frontier: &Path,
    receipt_bytes: &[u8],
    receipt_root: &str,
    operation_id: &str,
    opts: &PublishOptions,
) -> Result<Option<PublicationOutcome>, String> {
    if !is_canonical_sha256(receipt_root) || sha256(receipt_bytes) != receipt_root {
        return Err("receipt publication discovery bytes do not match receipt_root".to_string());
    }
    if !valid_operation_id(operation_id) {
        return Err("receipt publication discovery operation id is invalid".to_string());
    }
    let receipt_hex = receipt_root
        .strip_prefix("sha256:")
        .expect("validated receipt root");
    let receipt_path = format!("records/receipts/sha256/{receipt_hex}.json");

    let (temporary, runner) = sanitized_git_runner(frontier)?;
    reject_local_attribute_overrides(&runner)?;
    let frontier_abs = frontier
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier: {error}"))?;
    let frontier_prefix = frontier_abs
        .strip_prefix(&runner.root)
        .map_err(|_| "frontier is outside the resolved Git worktree".to_string())?;
    let anchor_path = utf8_repo_path(&frontier_prefix.join(&receipt_path))?;
    let proposals_path = utf8_repo_path(&frontier_prefix.join(".vela/proposals"))?;
    let target_refname = resolve_target_ref(&runner, opts.target_refname.as_deref())?;
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let history = runner.checked(
        &[
            OsString::from("rev-list"),
            OsString::from("--full-history"),
            OsString::from("--topo-order"),
            OsString::from(&target_refname.0),
            OsString::from("--"),
            OsString::from(&anchor_path),
        ],
        None,
        None,
    )?;
    let mut matches = Vec::new();
    for line in String::from_utf8(history)
        .map_err(|_| "Git ancestry output is not UTF-8".to_string())?
        .lines()
    {
        let commit = GitOid::parse(&object_format, line)?;
        let parents = commit_parents(&runner, &commit, &object_format)?;
        if parents.len() != 1 {
            continue;
        }
        if historical_receipt_publication_matches(
            &runner,
            &parents[0],
            &commit,
            &anchor_path,
            receipt_bytes,
            &proposals_path,
            &receipt_path,
            receipt_root,
            operation_id,
            temporary.path(),
        )? {
            matches.push(commit);
        }
    }
    let commit = match matches.as_slice() {
        [] => return Ok(None),
        [commit] => commit.clone(),
        _ => {
            return Err(format!(
                "receipt publication anchor has {} fully verified introducing commits in target-ref ancestry",
                matches.len()
            ));
        }
    };
    Ok(Some(discovered_publication_outcome(
        &runner,
        &target_refname,
        commit,
    )))
}

fn capture_preflight_input_hashes(
    runner: &GitRunner,
    frontier_abs: &Path,
    inputs: &[PathBuf],
) -> Result<BTreeMap<String, String>, String> {
    let mut hashes = BTreeMap::new();
    for input in inputs {
        if !input.is_absolute()
            && input
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "preflight input must be normalized before frontier containment: {}",
                input.display()
            ));
        }
        let lexical = if input.is_absolute() {
            input.clone()
        } else {
            frontier_abs.join(input)
        };
        let metadata = fs::symlink_metadata(&lexical).map_err(|error| {
            format!(
                "inspect explicit preflight input {}: {error}",
                input.display()
            )
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "preflight input must be one regular non-symlink file: {}",
                input.display()
            ));
        }
        let absolute = lexical.canonicalize().map_err(|error| {
            format!(
                "canonicalize explicit preflight input {}: {error}",
                input.display()
            )
        })?;
        if absolute != lexical || !absolute.starts_with(frontier_abs) {
            return Err(format!(
                "preflight input has symlink or containment ambiguity: {}",
                input.display()
            ));
        }
        let relative = absolute
            .strip_prefix(&runner.root)
            .map_err(|_| "preflight input is outside the Git worktree".to_string())?
            .to_str()
            .ok_or_else(|| "preflight input path is not UTF-8".to_string())?
            .to_string();
        let bytes = fs::read(&absolute).map_err(|error| {
            format!("read explicit preflight input {}: {error}", input.display())
        })?;
        if hashes.insert(relative.clone(), sha256(&bytes)).is_some() {
            return Err(format!("duplicate explicit preflight input {relative}"));
        }
    }
    Ok(hashes)
}

/// Publish exactly the public postimages/deletions bound by an earlier exact
/// preflight. No materialization or broad worktree enumeration contributes to
/// the candidate mapping. Changing any caller-supplied delta field after
/// preflight is a typed programming error rather than an operational outcome.
#[allow(dead_code)]
pub(crate) fn publish_exact_delta(
    frontier: &Path,
    summary: &str,
    event_ids: &[String],
    delta: &PublicationDelta,
    preflight: ExactPublicationPreflight,
    opts: &PublishOptions,
) -> Result<PublicationOutcome, ExactPublicationError> {
    let actual_sha256 = publication_delta_sha256(delta);
    if actual_sha256 != preflight.delta_sha256 {
        return Err(ExactPublicationError::DeltaChanged {
            expected_sha256: preflight.delta_sha256.clone(),
            actual_sha256,
        });
    }
    if let Some(reason) = publication_disabled_reason(frontier, opts) {
        return Ok(PublicationOutcome::uncommitted(reason));
    }
    Ok(
        publish_exact_inner(frontier, summary, event_ids, delta, &preflight, opts)
            .unwrap_or_else(PublicationOutcome::uncommitted),
    )
}

/// Resume one journaled Git publication. This is the idempotent service seam
/// for the CLI's `vela publication recover --operation <vop_…>` command; it
/// performs no rendering and refuses checkout, index, worktree, or ref drift.
pub(crate) fn recover_publication(
    frontier: &Path,
    operation_id: &str,
    opts: &PublishOptions,
) -> PublicationOutcome {
    match recover_publication_inner(frontier, operation_id, opts) {
        Ok(outcome) => outcome,
        Err(error) => PublicationOutcome::unknown(error),
    }
}

fn recover_publication_inner(
    frontier: &Path,
    operation_id: &str,
    opts: &PublishOptions,
) -> Result<PublicationOutcome, String> {
    if !valid_operation_id(operation_id) {
        return Err("invalid publication operation id".to_string());
    }
    let temporary = tempfile::tempdir().map_err(|error| format!("publication tempdir: {error}"))?;
    let empty_hooks = temporary.path().join("hooks");
    let empty_attributes = temporary.path().join("attributes");
    fs::create_dir(&empty_hooks)
        .map_err(|error| format!("create empty hooks directory: {error}"))?;
    fs::write(&empty_attributes, [])
        .map_err(|error| format!("create empty global attributes file: {error}"))?;
    let bootstrap = GitRunner {
        root: frontier.to_path_buf(),
        empty_hooks: empty_hooks.clone(),
        empty_attributes: empty_attributes.clone(),
        attribute_source: None,
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let mut runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
        attribute_source: None,
    };
    let _publication_lock = match acquire_publication_lock(&runner) {
        Ok(lock) => lock,
        Err(PublicationLockError::Busy) => {
            return Ok(PublicationOutcome {
                state: PublicationState::Uncommitted {
                    candidate: None,
                    reason: PUBLICATION_BUSY_REASON.to_string(),
                },
                recovery_command: Some(format!(
                    "vela publication recover --operation {operation_id}"
                )),
            });
        }
        Err(PublicationLockError::Failed(reason)) => return Err(reason),
    };
    let journal_dir = git_private_path(&runner, "vela/operation-journals")?;
    let journal_path = crate::operation_journal::path(&journal_dir, operation_id);
    let completed_dir = journal_dir.join("completed");
    let completed_path = crate::operation_journal::path(&completed_dir, operation_id);
    if !journal_path.is_file() && completed_path.is_file() {
        let completed: CompletedPublication = crate::operation_journal::read_json(&completed_path)?;
        if completed.schema != crate::operation_journal::JOURNAL_SCHEMA
            || completed.operation_id != operation_id
        {
            return Err("completed publication identity or schema mismatch".to_string());
        }
        return Ok(completed.outcome);
    }
    let mut journal: PublicationJournal = crate::operation_journal::read_json(&journal_path)?;
    if journal.schema != crate::operation_journal::JOURNAL_SCHEMA
        || journal.operation_id != operation_id
    {
        return Err("publication journal identity or schema mismatch".to_string());
    }
    runner.pin_attributes(&journal.expected_git_commit_oid);
    let frontier_abs = frontier
        .canonicalize()
        .unwrap_or_else(|_| frontier.to_path_buf());
    if journal.repository != root.display().to_string()
        || journal.frontier != frontier_abs.display().to_string()
    {
        return Err(
            "publication journal belongs to a different repository or frontier".to_string(),
        );
    }
    let candidate = match &journal.candidate_commit_oid {
        Some(candidate) => candidate.clone(),
        None => {
            let output = runner.run(
                &[
                    OsString::from("commit-tree"),
                    OsString::from(&journal.candidate_tree_oid.hex),
                    OsString::from("-p"),
                    OsString::from(&journal.expected_git_commit_oid.hex),
                ],
                Some(journal.message.as_bytes()),
                None,
                Some((
                    &journal.author_name,
                    &journal.author_email,
                    &journal.commit_date,
                )),
            )?;
            if !output.status.success() {
                return Err(format!(
                    "reconstruct publication commit: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let candidate = GitOid::parse(
                &journal.expected_git_commit_oid.object_format,
                String::from_utf8_lossy(&output.stdout).trim(),
            )?;
            journal.candidate_commit_oid = Some(candidate.clone());
            crate::operation_journal::write_json(&journal_path, &journal)?;
            candidate
        }
    };
    let actual = runner.text(&["rev-parse", &journal.target_refname.0])?;
    if journal.ref_moved && actual == journal.expected_git_commit_oid.hex {
        return Err(
            "publication recovery refuses a ref rolled back after recorded movement; preserve the journal and inspect the manual repair"
                .to_string(),
        );
    }
    if actual != journal.expected_git_commit_oid.hex
        && actual != candidate.hex
        && commit_is_ancestor(&runner, &candidate, &actual)?
    {
        // The immutable candidate is already in the target branch's history,
        // but a later local publication advanced the branch and its worktree.
        // This is push-only recovery: do not require or rewrite the obsolete
        // checkout/index snapshot from the earlier operation.
        let txn = GitPublicationTxn {
            target_refname: journal.target_refname.clone(),
            target_checkout: journal.target_checkout.clone(),
            expected_git_commit_oid: journal.expected_git_commit_oid.clone(),
            candidate_tree_oid: journal.candidate_tree_oid.clone(),
            candidate_commit_oid: Some(candidate.clone()),
            lfs_objects: journal.lfs_objects.clone(),
        };
        let (push_mode, _) = crate::config::settings::resolve("publish.git_push", Some(frontier));
        let local_only = (opts.no_push || push_mode == "off") && !opts.force_push;
        let outcome = if local_only {
            PublicationOutcome {
                state: PublicationState::CommittedLocal {
                    commit: candidate.hex.clone(),
                },
                recovery_command: Some(push_command(&runner, &txn)),
            }
        } else {
            push_and_verify(&runner, &txn, candidate.clone())
        };
        return Ok(complete_publication(
            operation_id,
            &candidate,
            &journal_path,
            &completed_dir,
            outcome,
            local_only,
            opts,
        ));
    }

    let checkout = target_checkout_state(&runner, &journal.target_refname)?;
    if checkout != journal.target_checkout {
        return Err("publication recovery refuses checkout identity drift".to_string());
    }
    if !journal_worktree_matches(&root, &journal)? {
        return Err("publication recovery refuses Vela worktree drift".to_string());
    }
    if actual == journal.expected_git_commit_oid.hex {
        if target_checkout_state(&runner, &journal.target_refname)? != checkout {
            return Err(
                "publication recovery refuses checkout identity drift at the ref write boundary"
                    .to_string(),
            );
        }
        if matches!(checkout, TargetCheckoutState::Current { .. }) {
            let (current_index, current_index_sha256) = capture_index(&runner)?;
            if current_index != journal.original_index
                || current_index_sha256 != journal.original_index_sha256
            {
                return Err("publication recovery refuses caller-index drift".to_string());
            }
        }
        let update = runner.run(
            &[
                OsString::from("update-ref"),
                OsString::from(&journal.target_refname.0),
                OsString::from(&candidate.hex),
                OsString::from(&journal.expected_git_commit_oid.hex),
            ],
            None,
            None,
            None,
        )?;
        if !update.status.success() {
            let moved = runner.text(&["rev-parse", &journal.target_refname.0])?;
            if moved != candidate.hex {
                return Ok(PublicationOutcome {
                    state: PublicationState::Stale {
                        candidate: candidate.hex,
                        expected: journal.expected_git_commit_oid.hex,
                        actual: moved,
                    },
                    recovery_command: None,
                });
            }
        }
        journal.ref_moved = true;
        if crate::operation_journal::write_json(&journal_path, &journal).is_err() {
            return Ok(operation_recovery_outcome(&candidate, operation_id));
        }
    } else if actual != candidate.hex {
        return Ok(PublicationOutcome {
            state: PublicationState::Stale {
                candidate: candidate.hex,
                expected: journal.expected_git_commit_oid.hex,
                actual,
            },
            recovery_command: None,
        });
    }

    if matches!(checkout, TargetCheckoutState::Current { .. }) {
        match reconcile_journal_index(&runner, &journal) {
            Ok(()) => {}
            Err(JournalIndexReconcileError::Refused(reason)) => return Err(reason),
            Err(JournalIndexReconcileError::Retryable(_reason)) => {
                return Ok(operation_recovery_outcome(&candidate, operation_id));
            }
        }
    }
    let txn = GitPublicationTxn {
        target_refname: journal.target_refname,
        target_checkout: checkout,
        expected_git_commit_oid: journal.expected_git_commit_oid,
        candidate_tree_oid: journal.candidate_tree_oid,
        candidate_commit_oid: Some(candidate.clone()),
        lfs_objects: journal.lfs_objects,
    };
    let (push_mode, _) = crate::config::settings::resolve("publish.git_push", Some(frontier));
    let local_only = (opts.no_push || push_mode == "off") && !opts.force_push;
    let outcome = if local_only {
        PublicationOutcome {
            state: PublicationState::CommittedLocal {
                commit: candidate.hex.clone(),
            },
            recovery_command: Some(push_command(&runner, &txn)),
        }
    } else {
        push_and_verify(&runner, &txn, candidate.clone())
    };
    Ok(complete_publication(
        operation_id,
        &candidate,
        &journal_path,
        &completed_dir,
        outcome,
        local_only,
        opts,
    ))
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn inject_competing_ref_advance(
    runner: &GitRunner,
    target_refname: &GitRefName,
    expected: &GitOid,
    tree: &GitOid,
    author_name: &str,
    author_email: &str,
    commit_date: &str,
) -> Result<GitOid, String> {
    let output = runner.run(
        &[
            OsString::from("commit-tree"),
            OsString::from(&tree.hex),
            OsString::from("-p"),
            OsString::from(&expected.hex),
        ],
        Some(b"competing publication\n"),
        None,
        Some((author_name, author_email, commit_date)),
    )?;
    if !output.status.success() {
        return Err(format!(
            "construct injected competing commit: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let competing = GitOid::parse(
        &expected.object_format,
        String::from_utf8_lossy(&output.stdout).trim(),
    )?;
    runner.checked(
        &[
            OsString::from("update-ref"),
            OsString::from(&target_refname.0),
            OsString::from(&competing.hex),
            OsString::from(&expected.hex),
        ],
        None,
        None,
    )?;
    Ok(competing)
}

fn publish_exact_inner(
    frontier: &Path,
    summary: &str,
    event_ids: &[String],
    delta: &PublicationDelta,
    preflight: &ExactPublicationPreflight,
    opts: &PublishOptions,
) -> Result<PublicationOutcome, String> {
    let temporary = tempfile::tempdir().map_err(|error| format!("publication tempdir: {error}"))?;
    let empty_hooks = temporary.path().join("hooks");
    let empty_attributes = temporary.path().join("attributes");
    fs::create_dir(&empty_hooks)
        .map_err(|error| format!("create empty hooks directory: {error}"))?;
    fs::write(&empty_attributes, [])
        .map_err(|error| format!("create empty global attributes file: {error}"))?;
    let bootstrap = GitRunner {
        root: frontier.to_path_buf(),
        empty_hooks: empty_hooks.clone(),
        empty_attributes: empty_attributes.clone(),
        attribute_source: None,
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let mut runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
        attribute_source: None,
    };
    let publication_lock = &preflight.publication_lock;
    if publication_lock.repository != root {
        return Err("publication preflight lock belongs to a different repository".to_string());
    }
    reject_local_attribute_overrides(&runner)?;
    let target_refname = resolve_target_ref(&runner, opts.target_refname.as_deref())?;
    let target_checkout = target_checkout_state(&runner, &target_refname)?;
    if let TargetCheckoutState::CheckedOutElsewhere { worktree } = &target_checkout {
        return Err(format!(
            "target ref {} is checked out in another worktree at {worktree}",
            target_refname.0
        ));
    }

    let frontier_abs = frontier
        .canonicalize()
        .unwrap_or_else(|_| frontier.to_path_buf());
    if !frontier_abs.starts_with(&root) {
        return Err("frontier is outside the resolved Git worktree".to_string());
    }
    let specs = frontier_specs(&frontier_abs, &root)?;

    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let expected = GitOid::parse(
        &object_format,
        &runner.text(&["rev-parse", &format!("{}^{{commit}}", target_refname.0)])?,
    )?;
    runner.pin_attributes(&expected);

    reject_unsupported_index(&runner, &specs, &object_format)?;
    validate_publication_delta(delta, &root, &specs)?;
    let actual_delta_sha256 = publication_delta_sha256(delta);
    if actual_delta_sha256 != preflight.delta_sha256 {
        return Err("exact publication delta changed after digest validation".to_string());
    }
    if preflight.repository != root
        || preflight.frontier != frontier_abs
        || preflight.target_refname != target_refname
        || preflight.target_checkout != target_checkout
        || preflight.expected_git_commit_oid != expected
    {
        return Err("exact publication preflight identity is stale".to_string());
    }
    let (current_index, current_index_sha256) = capture_index(&runner)?;
    if current_index != preflight.original_index
        || current_index_sha256 != preflight.original_index_sha256
    {
        return Err("exact publication preflight refuses caller-index drift".to_string());
    }
    for (path, expected_hash) in &preflight.allowed_input_hashes {
        let bytes = fs::read(root.join(path))
            .map_err(|error| format!("re-read explicit preflight input {path}: {error}"))?;
        if sha256(&bytes) != *expected_hash {
            return Err(format!(
                "exact publication preflight refuses explicit-input drift at {path}"
            ));
        }
    }
    if matches!(target_checkout, TargetCheckoutState::Current { .. }) {
        let staged = git_paths(
            &runner,
            vec![
                OsString::from("diff"),
                OsString::from("--cached"),
                OsString::from("--name-only"),
                OsString::from("-z"),
                OsString::from(&expected.hex),
                OsString::from("--"),
            ],
            &specs,
        )?;
        let staged = staged
            .into_iter()
            .filter(|path| !is_private_vela_path(path, &specs))
            .collect::<BTreeSet<_>>();
        if !staged.is_empty() {
            return Err(format!(
                "publication refuses pre-existing staged Vela paths: {}",
                staged.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    let (original_index, original_index_sha256) = capture_index(&runner)?;
    let parent = tree_entries(&runner, &expected, &specs)?;
    let paths = delta_paths(delta);
    reject_parent_case_collisions(&paths, &parent)?;
    let index_path = temporary.path().join("publication.index");
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&expected.hex)],
        None,
        Some(&index_path),
    )?;
    let mut allowed = paths;
    allowed.extend(preflight.allowed_input_hashes.keys().cloned());
    reject_unrelated_public_dirt(
        &runner,
        &expected,
        &specs,
        &allowed,
        &parent,
        &object_format,
        &index_path,
    )?;
    let (desired, worktree_hashes) =
        exact_desired_entries(&runner, delta, &root, &specs, &object_format, &index_path)?;
    if let ExactPublicationDisposition::AlreadyPublished { commit } = &preflight.disposition {
        if commit != &expected || !target_matches_exact_postimages(&parent, &desired) {
            return Err("already-published exact publication identity is stale".to_string());
        }
        return exact_unchanged_outcome(
            &runner,
            frontier,
            target_refname,
            target_checkout,
            expected,
            &desired,
            &specs,
            &index_path,
            opts,
        );
    }
    if target_matches_exact_postimages(&parent, &desired) {
        return exact_unchanged_outcome(
            &runner,
            frontier,
            target_refname,
            target_checkout,
            expected,
            &desired,
            &specs,
            &index_path,
            opts,
        );
    }
    for (path, entry) in &desired {
        match entry {
            Some(entry) => {
                let written = hash_object(&runner, &object_format, &entry.bytes, true)?;
                if written != entry.oid {
                    return Err(format!("Git wrote a different blob for {path}"));
                }
                runner.checked(
                    &[
                        OsString::from("update-index"),
                        OsString::from("--add"),
                        OsString::from("--cacheinfo"),
                        OsString::from(&entry.mode),
                        OsString::from(&entry.oid.hex),
                        OsString::from(path),
                    ],
                    None,
                    Some(&index_path),
                )?;
            }
            None => {
                runner.checked(
                    &[
                        OsString::from("update-index"),
                        OsString::from("--force-remove"),
                        OsString::from("--"),
                        OsString::from(path),
                    ],
                    None,
                    Some(&index_path),
                )?;
            }
        }
    }
    validate_candidate_attributes(&runner, &desired, &specs, &index_path)?;

    let tree = GitOid::parse(
        &object_format,
        String::from_utf8_lossy(&runner.checked(
            &[OsString::from("write-tree")],
            None,
            Some(&index_path),
        )?)
        .trim(),
    )?;
    inspect_exact_candidate(&runner, &expected, &tree, &desired)?;

    let message = publication_message(summary, event_ids);
    let planning_identity = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        root.display(),
        frontier_abs.display(),
        target_refname.0,
        expected.hex,
        tree.hex,
        delta.root,
        message
    );
    let operation_id =
        crate::operation_journal::operation_id("git-publication", planning_identity.as_bytes());
    let journal_dir = git_private_path(&runner, "vela/operation-journals")?;
    let journal_path = crate::operation_journal::path(&journal_dir, &operation_id);
    let completed_dir = journal_dir.join("completed");
    if journal_path.is_file() {
        publication_lock
            ._file
            .unlock()
            .map_err(|error| format!("release publication lock for recovery: {error}"))?;
        return Ok(recover_publication(frontier, &operation_id, opts));
    }
    let author_name = identity_value(&runner, "user.name", "Vela")?;
    let author_email = identity_value(&runner, "user.email", "vela@localhost")?;
    let commit_date = format!("{} +0000", chrono::Utc::now().timestamp());
    let mut journal = PublicationJournal {
        schema: crate::operation_journal::JOURNAL_SCHEMA.to_string(),
        operation_id: operation_id.clone(),
        repository: root.display().to_string(),
        frontier: frontier_abs.display().to_string(),
        target_refname: target_refname.clone(),
        target_checkout: target_checkout.clone(),
        expected_git_commit_oid: expected.clone(),
        candidate_tree_oid: tree.clone(),
        candidate_commit_oid: None,
        message: message.clone(),
        author_name: author_name.clone(),
        author_email: author_email.clone(),
        commit_date: commit_date.clone(),
        entries: journal_entries(&desired, &worktree_hashes),
        scope: PublicationScope::ExactDelta,
        lfs_objects: desired
            .values()
            .filter_map(|entry| entry.as_ref())
            .filter(|entry| entry.content_mode == ContentMode::Lfs)
            .filter_map(|entry| parse_lfs_pointer(&entry.bytes).ok())
            .collect(),
        original_index: original_index.clone(),
        original_index_sha256: original_index_sha256.clone(),
        ref_moved: false,
    };
    crate::operation_journal::write_json(&journal_path, &journal)?;
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::InterruptAfterPreparedJournal {
        return Ok(uncommitted_operation_outcome(
            None,
            &operation_id,
            "injected interruption after prepared publication journal",
        ));
    }

    let commit_output = match runner.run(
        &[
            OsString::from("commit-tree"),
            OsString::from(&tree.hex),
            OsString::from("-p"),
            OsString::from(&expected.hex),
        ],
        Some(message.as_bytes()),
        None,
        Some((&author_name, &author_email, &commit_date)),
    ) {
        Ok(output) => output,
        Err(error) => {
            return Ok(uncommitted_operation_outcome(None, &operation_id, error));
        }
    };
    if !commit_output.status.success() {
        return Ok(uncommitted_operation_outcome(
            None,
            &operation_id,
            format!(
                "construct publication commit: {}",
                String::from_utf8_lossy(&commit_output.stderr).trim()
            ),
        ));
    }
    let candidate = match GitOid::parse(
        &object_format,
        String::from_utf8_lossy(&commit_output.stdout).trim(),
    ) {
        Ok(candidate) => candidate,
        Err(error) => {
            return Ok(uncommitted_operation_outcome(None, &operation_id, error));
        }
    };
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::InterruptAfterCommitTree {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            "injected interruption after commit-tree",
        ));
    }
    journal.candidate_commit_oid = Some(candidate.clone());
    if let Err(error) = crate::operation_journal::write_json(&journal_path, &journal) {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            error,
        ));
    }
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::InterruptAfterCandidateJournal {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            "injected interruption after candidate publication journal",
        ));
    }

    let worktree_unchanged =
        match worktree_matches(&root, &worktree_hashes, &desired_modes(&desired)) {
            Ok(unchanged) => unchanged,
            Err(error) => {
                return Ok(uncommitted_operation_outcome(
                    Some(&candidate),
                    &operation_id,
                    error,
                ));
            }
        };
    if !worktree_unchanged {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            "Vela worktree paths changed during publication planning",
        ));
    }
    if matches!(target_checkout, TargetCheckoutState::Current { .. }) {
        let current = match capture_index(&runner) {
            Ok(current) => current,
            Err(error) => {
                return Ok(uncommitted_operation_outcome(
                    Some(&candidate),
                    &operation_id,
                    error,
                ));
            }
        };
        if current != (original_index.clone(), original_index_sha256.clone()) {
            return Ok(uncommitted_operation_outcome(
                Some(&candidate),
                &operation_id,
                "the caller index changed during publication planning",
            ));
        }
    }
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::AdvanceRefBeforeFinalObservation
        && let Err(error) = inject_competing_ref_advance(
            &runner,
            &target_refname,
            &expected,
            &tree,
            &author_name,
            &author_email,
            &commit_date,
        )
    {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            error,
        ));
    }
    let checkout_at_cas = match target_checkout_state(&runner, &target_refname) {
        Ok(checkout) => checkout,
        Err(error) => {
            return Ok(uncommitted_operation_outcome(
                Some(&candidate),
                &operation_id,
                error,
            ));
        }
    };
    if checkout_at_cas != target_checkout {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            "target checkout identity changed during publication planning",
        ));
    }
    let actual_before = match runner.text(&["rev-parse", &target_refname.0]) {
        Ok(actual) => actual,
        Err(error) => {
            return Ok(uncommitted_operation_outcome(
                Some(&candidate),
                &operation_id,
                error,
            ));
        }
    };
    if actual_before != expected.hex {
        let _ = crate::operation_journal::remove(&journal_path);
        return Ok(PublicationOutcome {
            state: PublicationState::Stale {
                candidate: candidate.hex,
                expected: expected.hex,
                actual: actual_before,
            },
            recovery_command: None,
        });
    }
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::AdvanceRefAfterFinalObservation
        && let Err(error) = inject_competing_ref_advance(
            &runner,
            &target_refname,
            &expected,
            &tree,
            &author_name,
            &author_email,
            &commit_date,
        )
    {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            error,
        ));
    }

    let update = runner.run(
        &[
            OsString::from("update-ref"),
            OsString::from(&target_refname.0),
            OsString::from(&candidate.hex),
            OsString::from(&expected.hex),
        ],
        None,
        None,
        None,
    );
    let update = match update {
        Ok(update) => update,
        Err(error) => match runner.text(&["rev-parse", &target_refname.0]) {
            Ok(actual) if actual == candidate.hex => {
                return Ok(operation_recovery_outcome(&candidate, &operation_id));
            }
            Ok(actual) if actual != expected.hex => {
                return Ok(PublicationOutcome {
                    state: PublicationState::Stale {
                        candidate: candidate.hex,
                        expected: expected.hex,
                        actual,
                    },
                    recovery_command: None,
                });
            }
            Ok(_) => {
                return Ok(uncommitted_operation_outcome(
                    Some(&candidate),
                    &operation_id,
                    error,
                ));
            }
            Err(inspect_error) => {
                return Ok(PublicationOutcome {
                    state: PublicationState::Unknown {
                        reason: format!(
                            "Git ref update could not be observed: {error}; inspect ref: {inspect_error}"
                        ),
                    },
                    recovery_command: Some(format!(
                        "vela publication recover --operation {operation_id}"
                    )),
                });
            }
        },
    };
    if !update.status.success() {
        match runner.text(&["rev-parse", &target_refname.0]) {
            Ok(actual) if actual == candidate.hex => {
                return Ok(operation_recovery_outcome(&candidate, &operation_id));
            }
            Ok(actual) if actual != expected.hex => {
                let _ = crate::operation_journal::remove(&journal_path);
                return Ok(PublicationOutcome {
                    state: PublicationState::Stale {
                        candidate: candidate.hex,
                        expected: expected.hex,
                        actual,
                    },
                    recovery_command: None,
                });
            }
            Err(error) => {
                return Ok(PublicationOutcome {
                    state: PublicationState::Unknown {
                        reason: format!("compare-and-swap failed and ref is unreadable: {error}"),
                    },
                    recovery_command: Some(format!(
                        "vela publication recover --operation {operation_id}"
                    )),
                });
            }
            Ok(_) => {}
        }
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            format!(
                "compare-and-swap update of {} failed: {}",
                target_refname.0,
                String::from_utf8_lossy(&update.stderr).trim()
            ),
        ));
    }
    #[cfg(test)]
    if opts.test_step == PublicationTestStep::InterruptAfterRefCasBeforeMarker {
        return Ok(operation_recovery_outcome(&candidate, &operation_id));
    }
    journal.ref_moved = true;
    if crate::operation_journal::write_json(&journal_path, &journal).is_err() {
        return Ok(operation_recovery_outcome(&candidate, &operation_id));
    }

    let mut txn = GitPublicationTxn {
        target_refname: target_refname.clone(),
        target_checkout: target_checkout.clone(),
        expected_git_commit_oid: expected,
        candidate_tree_oid: tree,
        candidate_commit_oid: Some(candidate.clone()),
        lfs_objects: journal.lfs_objects.clone(),
    };
    if matches!(target_checkout, TargetCheckoutState::Current { .. })
        && let Err(_error) = reconcile_current_index(
            &runner,
            &desired,
            &original_index,
            &original_index_sha256,
            &worktree_hashes,
        )
    {
        return Ok(operation_recovery_outcome(&candidate, &operation_id));
    }
    txn.candidate_commit_oid = Some(candidate.clone());

    let (push_mode, _) = crate::config::settings::resolve("publish.git_push", Some(frontier));
    let outcome = if (opts.no_push || push_mode == "off") && !opts.force_push {
        PublicationOutcome {
            state: PublicationState::CommittedLocal {
                commit: candidate.hex.clone(),
            },
            recovery_command: Some(push_command(&runner, &txn)),
        }
    } else {
        push_and_verify(&runner, &txn, candidate.clone())
    };
    Ok(complete_publication(
        &operation_id,
        &candidate,
        &journal_path,
        &completed_dir,
        outcome,
        false,
        opts,
    ))
}

#[allow(clippy::too_many_arguments)]
fn exact_unchanged_outcome(
    runner: &GitRunner,
    frontier: &Path,
    target_refname: GitRefName,
    target_checkout: TargetCheckoutState,
    expected: GitOid,
    desired: &DesiredMap,
    specs: &[String],
    attribute_index: &Path,
    opts: &PublishOptions,
) -> Result<PublicationOutcome, String> {
    validate_candidate_attributes(runner, desired, specs, attribute_index)?;
    let tree = GitOid::parse(
        &expected.object_format,
        &runner.text(&["rev-parse", &format!("{}^{{tree}}", expected.hex)])?,
    )?;
    let txn = GitPublicationTxn {
        target_refname,
        target_checkout,
        expected_git_commit_oid: expected.clone(),
        candidate_tree_oid: tree,
        candidate_commit_oid: Some(expected.clone()),
        lfs_objects: desired
            .values()
            .filter_map(|entry| entry.as_ref())
            .filter(|entry| entry.content_mode == ContentMode::Lfs)
            .filter_map(|entry| parse_lfs_pointer(&entry.bytes).ok())
            .collect(),
    };
    let (push_mode, _) = crate::config::settings::resolve("publish.git_push", Some(frontier));
    if (opts.no_push || push_mode == "off") && !opts.force_push {
        return Ok(PublicationOutcome {
            state: PublicationState::Unchanged {
                commit: expected.hex,
            },
            recovery_command: Some(push_command(runner, &txn)),
        });
    }
    Ok(push_and_verify(runner, &txn, expected))
}

type TreeMap = BTreeMap<String, (String, GitOid)>;
type IndexMap = BTreeMap<String, String>;
type DesiredMap = BTreeMap<String, Option<DesiredEntry>>;
type WorktreeHashMap = BTreeMap<String, Option<String>>;

fn args_with_paths(mut args: Vec<OsString>, paths: &[String]) -> Vec<OsString> {
    args.extend(paths.iter().map(OsString::from));
    args
}

fn split_nul(bytes: &[u8]) -> Result<Vec<String>, String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| {
            String::from_utf8(part.to_vec()).map_err(|_| {
                "Git returned a non-UTF-8 Vela path; publication refuses ambiguity".to_string()
            })
        })
        .collect()
}

fn git_paths(
    runner: &GitRunner,
    args: Vec<OsString>,
    paths: &[String],
) -> Result<BTreeSet<String>, String> {
    let output = runner.checked(&args_with_paths(args, paths), None, None)?;
    Ok(split_nul(&output)?.into_iter().collect())
}

fn resolve_target_ref(runner: &GitRunner, explicit: Option<&str>) -> Result<GitRefName, String> {
    let current = runner.text(&["symbolic-ref", "-q", "HEAD"]).ok();
    let value = match explicit {
        Some(value) => value.to_string(),
        None => current.ok_or_else(|| {
            "detached HEAD requires an explicit local branch ref for publication".to_string()
        })?,
    };
    if !value.starts_with("refs/heads/") {
        return Err(format!(
            "publication target must be a full local branch ref, got `{value}`"
        ));
    }
    runner.text(&["check-ref-format", &value])?;
    Ok(GitRefName(value))
}

fn target_checkout_state(
    runner: &GitRunner,
    target: &GitRefName,
) -> Result<TargetCheckoutState, String> {
    let current = runner.text(&["symbolic-ref", "-q", "HEAD"]).ok();
    if current.as_deref() == Some(target.0.as_str()) {
        return Ok(TargetCheckoutState::Current {
            worktree: runner.root.display().to_string(),
        });
    }
    let listing = runner.text(&["worktree", "list", "--porcelain"])?;
    let mut worktree = None::<String>;
    for line in listing.lines().chain(std::iter::once("")) {
        if let Some(path) = line.strip_prefix("worktree ") {
            worktree = Some(path.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if branch == target.0 {
                return Ok(TargetCheckoutState::CheckedOutElsewhere {
                    worktree: worktree.clone().unwrap_or_default(),
                });
            }
        } else if line.is_empty() {
            worktree = None;
        }
    }
    Ok(TargetCheckoutState::UncheckedOut)
}

fn frontier_specs(frontier: &Path, root: &Path) -> Result<Vec<String>, String> {
    let prefix = frontier
        .strip_prefix(root)
        .map_err(|_| "frontier is outside the Git root".to_string())?;
    let mut names = vec![
        ".vela".to_string(),
        "frontier.json".to_string(),
        "frontier.yaml".to_string(),
        "vela.lock".to_string(),
        "proof".to_string(),
        "witnesses".to_string(),
        "records".to_string(),
    ];
    if let Some(manifest) = vela_protocol::frontier_repo::read_manifest(frontier)? {
        names.push(manifest.paths.state);
        names.push(manifest.paths.sources);
        names.push(manifest.paths.artifacts);
        names.push(manifest.paths.review);
        names.push(manifest.paths.proof);
    }
    let mut specs = BTreeSet::new();
    for name in names {
        let normalized_name = name.trim_end_matches(['/', '\\']);
        let relative = Path::new(normalized_name);
        let components = relative.components().collect::<Vec<_>>();
        let normalized = components
            .iter()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value),
                _ => None,
            })
            .collect::<PathBuf>();
        let unsafe_component = components.iter().any(|component| match component {
            std::path::Component::Normal(value) => value.to_str().is_none_or(|value| {
                value
                    .trim_end_matches([' ', '.'])
                    .eq_ignore_ascii_case(".git")
                    || value.starts_with(':')
                    || value.contains(['*', '?', '['])
            }),
            _ => true,
        });
        if normalized_name.trim().is_empty()
            || components.is_empty()
            || unsafe_component
            || normalized.as_os_str() != relative.as_os_str()
        {
            return Err(format!(
                "frontier manifest path is unsafe, root-reserved, or non-normalized: {name}"
            ));
        }
        let path = prefix.join(relative);
        let value = path
            .to_str()
            .ok_or_else(|| "non-UTF-8 frontier paths are not publishable".to_string())?
            .to_string();
        if !value.is_empty() {
            reject_symlink_ancestors(root, Path::new(&value))?;
            specs.insert(value);
        }
    }
    Ok(specs.into_iter().collect())
}

fn reject_symlink_ancestors(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (position, component) in components.iter().enumerate() {
        let std::path::Component::Normal(component) = component else {
            return Err(format!(
                "public Vela path is not normalized: {}",
                relative.display()
            ));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "public Vela path has a symlink ancestor: {}",
                    current.display()
                ));
            }
            Ok(metadata) if position + 1 < components.len() && !metadata.is_dir() => {
                return Err(format!(
                    "public Vela path has a non-directory ancestor: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "inspect public Vela path ancestor {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn is_private_vela_path(path: &str, specs: &[String]) -> bool {
    const PRIVATE: &[&str] = &[
        "agents",
        "keys",
        "operation-journals",
        "work",
        "tasks",
        "workspaces",
        "source-inbox",
        "artifact-blobs",
    ];
    specs.iter().any(|spec| {
        if !(spec == ".vela" || spec.ends_with("/.vela")) {
            return false;
        }
        path.strip_prefix(spec)
            .and_then(|suffix| suffix.strip_prefix('/'))
            .and_then(|suffix| suffix.split('/').next())
            .is_some_and(|component| PRIVATE.contains(&component))
    })
}

fn publication_delta_sha256(delta: &PublicationDelta) -> String {
    fn field(hasher: &mut Sha256, bytes: &[u8]) {
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }

    let mut hasher = Sha256::new();
    hasher.update(b"vela.git-publication-delta.v1\0");
    field(&mut hasher, delta.root.as_bytes());
    hasher.update((delta.entries.len() as u64).to_be_bytes());
    for entry in &delta.entries {
        field(&mut hasher, entry.path.as_bytes());
        match &entry.preimage_sha256 {
            Some(preimage) => {
                hasher.update([1]);
                field(&mut hasher, preimage.as_bytes());
            }
            None => hasher.update([0]),
        }
        match &entry.postimage {
            Some(postimage) => {
                hasher.update([1]);
                field(&mut hasher, postimage);
            }
            None => hasher.update([0]),
        }
        hasher.update([u8::from(entry.executable)]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn is_canonical_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn delta_paths(delta: &PublicationDelta) -> BTreeSet<String> {
    delta
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect()
}

fn path_is_in_specs(path: &str, specs: &[String]) -> bool {
    specs
        .iter()
        .any(|spec| path == spec || path.starts_with(&format!("{spec}/")))
}

fn validate_publication_delta(
    delta: &PublicationDelta,
    root: &Path,
    specs: &[String],
) -> Result<(), String> {
    if !is_canonical_sha256(&delta.root) {
        return Err(
            "exact publication delta root must be canonical sha256:<lower-hex>".to_string(),
        );
    }
    if delta.entries.is_empty() {
        return Err("exact publication delta has no public entries".to_string());
    }
    let mut previous = None::<&str>;
    let mut folded = BTreeMap::<String, &str>::new();
    for entry in &delta.entries {
        if let Some(previous) = previous {
            if entry.path == previous {
                return Err(format!(
                    "exact publication delta contains duplicate path {}",
                    entry.path
                ));
            }
            if entry.path.as_str() < previous {
                return Err(format!(
                    "exact publication delta is not strictly path-sorted at {}",
                    entry.path
                ));
            }
        }
        previous = Some(&entry.path);
        if entry.path.is_empty()
            || entry.path.contains('\\')
            || entry.path.chars().any(char::is_control)
        {
            return Err(format!(
                "exact publication path is empty, non-portable, or contains control bytes: {:?}",
                entry.path
            ));
        }
        let relative = Path::new(&entry.path);
        let components = relative.components().collect::<Vec<_>>();
        let normalized = components
            .iter()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value),
                _ => None,
            })
            .collect::<PathBuf>();
        let unsafe_component = components.iter().any(|component| match component {
            std::path::Component::Normal(value) => value.to_str().is_none_or(|value| {
                value
                    .trim_end_matches([' ', '.'])
                    .eq_ignore_ascii_case(".git")
                    || value.starts_with(':')
                    || value.contains(['*', '?', '['])
            }),
            _ => true,
        });
        if components.is_empty()
            || unsafe_component
            || normalized.as_os_str() != relative.as_os_str()
        {
            return Err(format!(
                "exact publication path is absolute, parent-bearing, Git-private, magic, or non-normalized: {}",
                entry.path
            ));
        }
        if !path_is_in_specs(&entry.path, specs) || is_private_vela_path(&entry.path, specs) {
            return Err(format!(
                "exact publication path is outside the resolved public frontier topology: {}",
                entry.path
            ));
        }
        reject_symlink_ancestors(root, relative)?;
        if let Some(preimage) = &entry.preimage_sha256
            && !is_canonical_sha256(preimage)
        {
            return Err(format!(
                "exact publication preimage digest is not canonical at {}",
                entry.path
            ));
        }
        if entry.postimage.is_none() && entry.executable {
            return Err(format!(
                "exact publication deletion carries an ambiguous executable bit at {}",
                entry.path
            ));
        }
        let case_key = entry.path.to_lowercase();
        if let Some(other) = folded.insert(case_key, &entry.path)
            && other != entry.path
        {
            return Err(format!(
                "exact publication delta contains case-colliding paths {other} and {}",
                entry.path
            ));
        }
    }
    Ok(())
}

fn reject_parent_case_collisions(
    delta_paths: &BTreeSet<String>,
    parent: &TreeMap,
) -> Result<(), String> {
    let folded_delta = delta_paths
        .iter()
        .map(|path| (path.to_lowercase(), path.as_str()))
        .collect::<BTreeMap<_, _>>();
    for path in parent.keys() {
        if let Some(delta_path) = folded_delta.get(&path.to_lowercase())
            && *delta_path != path
        {
            return Err(format!(
                "exact publication path {delta_path} case-collides with target-tree path {path}"
            ));
        }
    }
    Ok(())
}

fn reject_unrelated_public_dirt(
    runner: &GitRunner,
    expected: &GitOid,
    specs: &[String],
    allowed: &BTreeSet<String>,
    parent: &TreeMap,
    object_format: &str,
    attribute_index: &Path,
) -> Result<(), String> {
    let staged = git_paths(
        runner,
        vec![
            OsString::from("diff"),
            OsString::from("--cached"),
            OsString::from("--name-only"),
            OsString::from("-z"),
            OsString::from(&expected.hex),
            OsString::from("--"),
        ],
        specs,
    )?
    .into_iter()
    .filter(|path| !is_private_vela_path(path, specs))
    .collect::<BTreeSet<_>>();
    if !staged.is_empty() {
        return Err(format!(
            "exact publication refuses staged public frontier paths: {}",
            staged.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let mut unexpected = git_paths(
        runner,
        vec![
            OsString::from("ls-files"),
            OsString::from("-z"),
            OsString::from("--others"),
            OsString::from("--exclude-standard"),
            OsString::from("--"),
        ],
        specs,
    )?
    .into_iter()
    .filter(|path| !is_private_vela_path(path, specs) && !allowed.contains(path))
    .collect::<Vec<_>>();
    let cached = cached_index_stats(runner, specs)?;
    for (path, (parent_mode, parent_oid)) in parent {
        if allowed.contains(path) || is_private_vela_path(path, specs) {
            continue;
        }
        let metadata = match fs::symlink_metadata(runner.root.join(path)) {
            Ok(metadata) if metadata.file_type().is_file() => metadata,
            Ok(_) | Err(_) => {
                unexpected.push(path.clone());
                continue;
            }
        };
        if cached.get(path).is_some_and(|stat| stat.matches(&metadata)) {
            continue;
        }
        let bytes = fs::read(runner.root.join(path))
            .map_err(|error| format!("read possibly dirty public path {path}: {error}"))?;
        let actual = desired_entry_from_bytes(
            runner,
            path,
            &bytes,
            regular_file_mode(&metadata),
            specs,
            object_format,
            attribute_index,
        )?;
        if &actual.mode != parent_mode || &actual.oid != parent_oid {
            unexpected.push(path.clone());
        }
    }
    unexpected.sort();
    unexpected.dedup();
    if !unexpected.is_empty() {
        return Err(format!(
            "exact publication refuses unrelated public frontier dirt (pre-existing unstaged Vela edit): {}",
            unexpected.join(", ")
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct CachedIndexStat {
    #[cfg(unix)]
    ctime_seconds: u64,
    #[cfg(unix)]
    ctime_nanos: u32,
    mtime_seconds: u64,
    mtime_nanos: u32,
    size: u64,
}

impl CachedIndexStat {
    fn matches(&self, metadata: &fs::Metadata) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.ctime_seconds == metadata.ctime() as u64
                && self.ctime_nanos == metadata.ctime_nsec() as u32
                && self.mtime_seconds == metadata.mtime() as u64
                && self.mtime_nanos == metadata.mtime_nsec() as u32
                && self.size == metadata.size()
        }
        #[cfg(not(unix))]
        {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok());
            modified.is_some_and(|modified| {
                self.mtime_seconds == modified.as_secs()
                    && self.mtime_nanos == modified.subsec_nanos()
                    && self.size == metadata.len()
            })
        }
    }
}

fn cached_index_stats(
    runner: &GitRunner,
    specs: &[String],
) -> Result<BTreeMap<String, CachedIndexStat>, String> {
    let output = runner.checked(
        &args_with_paths(
            vec![
                OsString::from("ls-files"),
                OsString::from("--debug"),
                OsString::from("-z"),
                OsString::from("--"),
            ],
            specs,
        ),
        None,
        None,
    )?;
    let mut stats = BTreeMap::new();
    let mut cursor = 0usize;
    while cursor < output.len() {
        let nul = output[cursor..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| cursor + offset)
            .ok_or_else(|| "Git index debug output omitted a path terminator".to_string())?;
        let path = String::from_utf8(output[cursor..nul].to_vec())
            .map_err(|_| "Git index debug output contains a non-UTF-8 path".to_string())?;
        cursor = nul + 1;
        let debug_start = cursor;
        for _ in 0..5 {
            cursor = output[cursor..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| cursor + offset + 1)
                .ok_or_else(|| format!("Git index debug block is truncated at {path}"))?;
        }
        let debug = std::str::from_utf8(&output[debug_start..cursor])
            .map_err(|_| format!("Git index debug block is not UTF-8 at {path}"))?;
        let mut ctime = None;
        let mut mtime = None;
        let mut size = None;
        for line in debug.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("ctime: ") {
                ctime = Some(parse_index_debug_time(value, &path)?);
            } else if let Some(value) = line.strip_prefix("mtime: ") {
                mtime = Some(parse_index_debug_time(value, &path)?);
            } else if let Some(value) = line.strip_prefix("size: ") {
                let value = value.split_once('\t').map_or(value, |(value, _)| value);
                size = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("Git index debug size is malformed at {path}"))?,
                );
            }
        }
        let (ctime_seconds, ctime_nanos) =
            ctime.ok_or_else(|| format!("Git index debug block omitted ctime at {path}"))?;
        #[cfg(not(unix))]
        let _ = (ctime_seconds, ctime_nanos);
        let (mtime_seconds, mtime_nanos) =
            mtime.ok_or_else(|| format!("Git index debug block omitted mtime at {path}"))?;
        let size = size.ok_or_else(|| format!("Git index debug block omitted size at {path}"))?;
        stats.insert(
            path,
            CachedIndexStat {
                #[cfg(unix)]
                ctime_seconds,
                #[cfg(unix)]
                ctime_nanos,
                mtime_seconds,
                mtime_nanos,
                size,
            },
        );
    }
    Ok(stats)
}

fn parse_index_debug_time(value: &str, path: &str) -> Result<(u64, u32), String> {
    let (seconds, nanos) = value
        .split_once(':')
        .ok_or_else(|| format!("Git index debug time is malformed at {path}"))?;
    Ok((
        seconds
            .parse::<u64>()
            .map_err(|_| format!("Git index debug seconds are malformed at {path}"))?,
        nanos
            .parse::<u32>()
            .map_err(|_| format!("Git index debug nanoseconds are malformed at {path}"))?,
    ))
}

fn reject_unsupported_index(
    runner: &GitRunner,
    specs: &[String],
    object_format: &str,
) -> Result<(), String> {
    for key in [
        "core.splitIndex",
        "core.sparseCheckout",
        "core.sparseCheckoutCone",
    ] {
        if runner
            .text(&["config", "--bool", "--get", key])
            .is_ok_and(|value| value == "true")
        {
            return Err(format!("publication does not support index mode `{key}`"));
        }
    }
    let conflicts = runner.checked(
        &[
            OsString::from("ls-files"),
            OsString::from("--unmerged"),
            OsString::from("-z"),
        ],
        None,
        None,
    )?;
    if !conflicts.is_empty() {
        return Err("publication refuses a conflicted index".to_string());
    }
    let flagged = runner.checked(
        &args_with_paths(
            vec![
                OsString::from("ls-files"),
                OsString::from("-v"),
                OsString::from("-z"),
                OsString::from("--"),
            ],
            specs,
        ),
        None,
        None,
    )?;
    for row in split_nul(&flagged)? {
        let (flag, path) = row
            .split_once(' ')
            .ok_or_else(|| format!("malformed index flag row `{row}`"))?;
        if !is_private_vela_path(path, specs) && flag != "H" {
            return Err(format!(
                "publication refuses non-normal index flag `{flag}` at Vela path {path}"
            ));
        }
    }
    reject_unsupported_index_extensions(runner, specs, object_format)?;
    Ok(())
}

fn reject_unsupported_index_extensions(
    runner: &GitRunner,
    specs: &[String],
    object_format: &str,
) -> Result<(), String> {
    let index_path = git_private_path(runner, "index")?;
    let bytes = fs::read(&index_path)
        .map_err(|error| format!("read caller index {}: {error}", index_path.display()))?;
    let hash_len = match object_format {
        "sha1" => 20,
        "sha256" => 32,
        other => return Err(format!("unsupported Git object format `{other}`")),
    };
    if bytes.len() < 12 + hash_len || &bytes[..4] != b"DIRC" {
        return Err("caller index has an invalid header".to_string());
    }
    let version = u32::from_be_bytes(bytes[4..8].try_into().expect("four bytes"));
    if !matches!(version, 2 | 3) {
        return Err(format!(
            "publication does not support Git index version {version}"
        ));
    }
    let count = u32::from_be_bytes(bytes[8..12].try_into().expect("four bytes")) as usize;
    let content_end = bytes.len() - hash_len;
    let mut offset = 12usize;
    for _ in 0..count {
        let start = offset;
        let fixed_len = 40 + hash_len + 2;
        if start + fixed_len > content_end {
            return Err("caller index entry is truncated".to_string());
        }
        let flags_offset = start + 40 + hash_len;
        let flags = u16::from_be_bytes(
            bytes[flags_offset..flags_offset + 2]
                .try_into()
                .expect("two bytes"),
        );
        offset = start + fixed_len;
        let mut extended_flags = 0u16;
        if flags & 0x4000 != 0 {
            if version < 3 || offset + 2 > content_end {
                return Err("caller index has invalid extended flags".to_string());
            }
            extended_flags = u16::from_be_bytes(
                bytes[offset..offset + 2]
                    .try_into()
                    .expect("two extended-flag bytes"),
            );
            offset += 2;
        }
        let path_offset = offset;
        let encoded_len = usize::from(flags & 0x0fff);
        let nul = if encoded_len < 0x0fff {
            let nul = offset + encoded_len;
            if nul >= content_end || bytes[nul] != 0 {
                return Err("caller index pathname is malformed".to_string());
            }
            nul
        } else {
            bytes[offset..content_end]
                .iter()
                .position(|byte| *byte == 0)
                .map(|position| offset + position)
                .ok_or_else(|| "caller index pathname is unterminated".to_string())?
        };
        if extended_flags != 0 {
            let path = std::str::from_utf8(&bytes[path_offset..nul])
                .map_err(|_| "caller index has a non-UTF-8 extended-flag path".to_string())?;
            if specs
                .iter()
                .any(|spec| path == spec || path.starts_with(&format!("{spec}/")))
                && !is_private_vela_path(path, specs)
            {
                return Err(format!(
                    "publication refuses extended index flag 0x{extended_flags:04x} at Vela path {path}"
                ));
            }
        }
        let entry_len = nul + 1 - start;
        offset = start + ((entry_len + 7) & !7);
        if offset > content_end {
            return Err("caller index entry padding is truncated".to_string());
        }
    }
    while offset < content_end {
        if offset + 8 > content_end {
            return Err("caller index extension header is truncated".to_string());
        }
        let signature = &bytes[offset..offset + 4];
        let size = u32::from_be_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("four bytes"),
        ) as usize;
        offset += 8;
        if offset + size > content_end {
            return Err("caller index extension is truncated".to_string());
        }
        if !matches!(signature, b"TREE" | b"EOIE" | b"IEOT") {
            let name = String::from_utf8_lossy(signature);
            return Err(format!(
                "publication does not support caller index extension `{name}`"
            ));
        }
        offset += size;
    }
    Ok(())
}

fn tree_entries(runner: &GitRunner, tree: &GitOid, specs: &[String]) -> Result<TreeMap, String> {
    let output = runner.checked(
        &args_with_paths(
            vec![
                OsString::from("ls-tree"),
                OsString::from("-r"),
                OsString::from("-z"),
                OsString::from("--full-tree"),
                OsString::from(&tree.hex),
                OsString::from("--"),
            ],
            specs,
        ),
        None,
        None,
    )?;
    let mut entries = BTreeMap::new();
    for raw in output
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        let line = String::from_utf8(raw.to_vec())
            .map_err(|_| "Git tree contains a non-UTF-8 Vela path".to_string())?;
        let (header, path) = line
            .split_once('\t')
            .ok_or_else(|| format!("malformed ls-tree row `{line}`"))?;
        // Some pre-0.9 frontiers tracked artifact blobs below `.vela/` as
        // public evidence. Exact publication must preserve those immutable
        // parent entries so old artifact locators keep replaying. Private
        // paths still cannot appear in a PublicationDelta, and the candidate
        // index is built from this exact parent tree, so caller-local private
        // dirt cannot be swept into the commit.
        let mut fields = header.split_whitespace();
        let mode = fields
            .next()
            .ok_or_else(|| "ls-tree row missing mode".to_string())?;
        let kind = fields
            .next()
            .ok_or_else(|| "ls-tree row missing type".to_string())?;
        let oid = fields
            .next()
            .ok_or_else(|| "ls-tree row missing object id".to_string())?;
        if kind != "blob" || !matches!(mode, "100644" | "100755") {
            return Err(format!(
                "unsupported Vela tree entry {mode} {kind} at {path}"
            ));
        }
        entries.insert(
            path.to_string(),
            (mode.to_string(), GitOid::parse(&tree.object_format, oid)?),
        );
    }
    Ok(entries)
}

fn desired_entry_from_bytes(
    runner: &GitRunner,
    path: &str,
    worktree_bytes: &[u8],
    mode: String,
    specs: &[String],
    object_format: &str,
    attribute_index: &Path,
) -> Result<DesiredEntry, String> {
    let content_mode =
        effective_content_mode(runner, path, is_witness_path(path, specs), attribute_index)?;
    let blob_bytes = match content_mode {
        ContentMode::Raw => worktree_bytes.to_vec(),
        ContentMode::Lfs => prepare_lfs_content(runner, path, worktree_bytes)?,
    };
    let oid = hash_object(runner, object_format, &blob_bytes, false)?;
    Ok(DesiredEntry {
        mode,
        oid,
        bytes: blob_bytes,
        content_mode,
    })
}

fn validate_exact_preimages(
    runner: &GitRunner,
    delta: &PublicationDelta,
    parent: &TreeMap,
    specs: &[String],
    object_format: &str,
    attribute_index: &Path,
) -> Result<(), String> {
    for item in &delta.entries {
        let absolute = runner.root.join(&item.path);
        match (&item.preimage_sha256, fs::symlink_metadata(&absolute)) {
            (None, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                if parent.contains_key(&item.path) {
                    return Err(format!(
                        "exact publication declared an absent preimage for tracked path {}",
                        item.path
                    ));
                }
            }
            (None, Ok(_)) => {
                return Err(format!(
                    "exact publication expected an absent preimage at {}",
                    item.path
                ));
            }
            (None, Err(error)) => {
                return Err(format!(
                    "inspect exact publication preimage {}: {error}",
                    item.path
                ));
            }
            (Some(expected_hash), Ok(metadata)) if metadata.file_type().is_file() => {
                let bytes = fs::read(&absolute).map_err(|error| {
                    format!("read exact publication preimage {}: {error}", item.path)
                })?;
                if sha256(&bytes) != *expected_hash {
                    return Err(format!(
                        "exact publication preimage digest mismatch at {}",
                        item.path
                    ));
                }
                let entry = desired_entry_from_bytes(
                    runner,
                    &item.path,
                    &bytes,
                    regular_file_mode(&metadata),
                    specs,
                    object_format,
                    attribute_index,
                )?;
                let Some((parent_mode, parent_oid)) = parent.get(&item.path) else {
                    return Err(format!(
                        "exact publication declared a present preimage for untracked path {}",
                        item.path
                    ));
                };
                if parent_mode != &entry.mode || parent_oid != &entry.oid {
                    return Err(format!(
                        "exact publication preimage at {} does not match the target tree",
                        item.path
                    ));
                }
            }
            (Some(_), Ok(_)) => {
                return Err(format!(
                    "exact publication preimage is not a regular file at {}",
                    item.path
                ));
            }
            (Some(_), Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "exact publication preimage is missing at {}",
                    item.path
                ));
            }
            (Some(_), Err(error)) => {
                return Err(format!(
                    "inspect exact publication preimage {}: {error}",
                    item.path
                ));
            }
        }
    }
    Ok(())
}

fn utf8_repo_path(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| "Git publication path is not UTF-8".to_string())
}

fn commit_parents(
    runner: &GitRunner,
    commit: &GitOid,
    object_format: &str,
) -> Result<Vec<GitOid>, String> {
    let row = runner.checked(
        &[
            OsString::from("rev-list"),
            OsString::from("--parents"),
            OsString::from("-n"),
            OsString::from("1"),
            OsString::from(&commit.hex),
        ],
        None,
        None,
    )?;
    let row =
        String::from_utf8(row).map_err(|_| "Git commit-parent output is not UTF-8".to_string())?;
    let mut fields = row.split_whitespace();
    let reported = fields
        .next()
        .ok_or_else(|| "Git omitted the requested commit from parent output".to_string())?;
    if GitOid::parse(object_format, reported)? != *commit {
        return Err("Git returned parent data for a different commit".to_string());
    }
    fields
        .map(|parent| GitOid::parse(object_format, parent))
        .collect()
}

fn historical_exact_delta_matches(
    runner: &GitRunner,
    delta: &PublicationDelta,
    parent: &GitOid,
    commit: &GitOid,
    specs: &[String],
    temporary: &Path,
) -> Result<bool, String> {
    let parent_tree = tree_entries(runner, parent, specs)?;
    let commit_tree = tree_entries(runner, commit, specs)?;
    reject_parent_case_collisions(&delta_paths(delta), &parent_tree)?;
    let parent_index = temporary.join(format!("history-parent-{}.index", parent.hex));
    let commit_index = temporary.join(format!("history-commit-{}.index", commit.hex));
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&parent.hex)],
        None,
        Some(&parent_index),
    )?;
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&commit.hex)],
        None,
        Some(&commit_index),
    )?;
    if !tree_matches_exact_preimages(runner, delta, &parent_tree, specs, &parent_index)? {
        return Ok(false);
    }
    let desired = historical_exact_desired_entries(
        runner,
        delta,
        specs,
        &commit.object_format,
        &commit_index,
    )?;
    if !target_matches_exact_postimages(&commit_tree, &desired) {
        return Ok(false);
    }
    validate_candidate_attributes(runner, &desired, specs, &commit_index)?;
    let changed = git_paths(
        runner,
        vec![
            OsString::from("diff-tree"),
            OsString::from("--no-commit-id"),
            OsString::from("--name-only"),
            OsString::from("-r"),
            OsString::from("-z"),
            OsString::from(&parent.hex),
            OsString::from(&commit.hex),
            OsString::from("--"),
        ],
        &[],
    )?;
    if changed != delta_paths(delta) {
        return Ok(false);
    }
    for (path, entry) in &desired {
        if let Some(entry) = entry {
            let stored = runner.checked(
                &[
                    OsString::from("cat-file"),
                    OsString::from("blob"),
                    OsString::from(&entry.oid.hex),
                ],
                None,
                None,
            )?;
            if stored != entry.bytes {
                return Err(format!(
                    "historical exact-publication blob differs from expected bytes at {path}"
                ));
            }
        }
    }
    Ok(true)
}

fn historical_exact_desired_entries(
    runner: &GitRunner,
    delta: &PublicationDelta,
    specs: &[String],
    object_format: &str,
    attribute_index: &Path,
) -> Result<DesiredMap, String> {
    let mut desired = BTreeMap::new();
    for item in &delta.entries {
        let entry = match &item.postimage {
            Some(bytes) => {
                let content_mode = effective_content_mode(
                    runner,
                    &item.path,
                    is_witness_path(&item.path, specs),
                    attribute_index,
                )?;
                let stored = match content_mode {
                    ContentMode::Raw => bytes.clone(),
                    ContentMode::Lfs => canonical_lfs_storage_bytes(bytes),
                };
                let mode = if item.executable { "100755" } else { "100644" }.to_string();
                Some(DesiredEntry {
                    mode,
                    oid: hash_object(runner, object_format, &stored, false)?,
                    bytes: stored,
                    content_mode,
                })
            }
            None => None,
        };
        desired.insert(item.path.clone(), entry);
    }
    Ok(desired)
}

fn canonical_lfs_storage_bytes(worktree: &[u8]) -> Vec<u8> {
    if parse_lfs_pointer(worktree).is_ok() {
        return worktree.to_vec();
    }
    format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {}\n",
        hex::encode(Sha256::digest(worktree)),
        worktree.len()
    )
    .into_bytes()
}

fn tree_matches_exact_preimages(
    runner: &GitRunner,
    delta: &PublicationDelta,
    parent: &TreeMap,
    specs: &[String],
    attribute_index: &Path,
) -> Result<bool, String> {
    for item in &delta.entries {
        let Some(expected_hash) = &item.preimage_sha256 else {
            if parent.contains_key(&item.path) {
                return Ok(false);
            }
            continue;
        };
        let Some((_mode, oid)) = parent.get(&item.path) else {
            return Ok(false);
        };
        let stored = runner.checked(
            &[
                OsString::from("cat-file"),
                OsString::from("blob"),
                OsString::from(&oid.hex),
            ],
            None,
            None,
        )?;
        let matches = match effective_content_mode(
            runner,
            &item.path,
            is_witness_path(&item.path, specs),
            attribute_index,
        )? {
            ContentMode::Raw => sha256(&stored) == *expected_hash,
            ContentMode::Lfs => {
                let pointer = parse_lfs_pointer(&stored).map_err(|error| {
                    format!(
                        "historical LFS blob at {} is not a canonical pointer: {error}",
                        item.path
                    )
                })?;
                sha256(&stored) == *expected_hash
                    || expected_hash.strip_prefix("sha256:") == Some(pointer.oid.as_str())
            }
        };
        if !matches {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn historical_receipt_publication_matches(
    runner: &GitRunner,
    parent: &GitOid,
    commit: &GitOid,
    anchor_path: &str,
    receipt_bytes: &[u8],
    proposals_path: &str,
    receipt_path: &str,
    receipt_root: &str,
    operation_id: &str,
    temporary: &Path,
) -> Result<bool, String> {
    let parent_anchor = tree_entries(runner, parent, &[anchor_path.to_string()])?;
    if parent_anchor.contains_key(anchor_path) {
        return Ok(false);
    }
    let commit_anchor = tree_entries(runner, commit, &[anchor_path.to_string()])?;
    let Some((mode, oid)) = commit_anchor.get(anchor_path) else {
        return Ok(false);
    };
    if mode != "100644" {
        return Ok(false);
    }
    let stored = runner.checked(
        &[
            OsString::from("cat-file"),
            OsString::from("blob"),
            OsString::from(&oid.hex),
        ],
        None,
        None,
    )?;
    if stored != receipt_bytes {
        return Ok(false);
    }
    let commit_index = temporary.join(format!("receipt-commit-{}.index", commit.hex));
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&commit.hex)],
        None,
        Some(&commit_index),
    )?;
    if effective_content_mode(runner, anchor_path, false, &commit_index)? != ContentMode::Raw {
        return Ok(false);
    }

    let parent_links = matching_submission_links(
        runner,
        parent,
        proposals_path,
        receipt_path,
        receipt_root,
        operation_id,
    )?;
    if !parent_links.is_empty() {
        return Ok(false);
    }
    let commit_links = matching_submission_links(
        runner,
        commit,
        proposals_path,
        receipt_path,
        receipt_root,
        operation_id,
    )?;
    let [proposal_path] = commit_links.as_slice() else {
        return Ok(false);
    };
    let parent_proposals = tree_entries(runner, parent, &[proposals_path.to_string()])?;
    if parent_proposals.contains_key(proposal_path) {
        return Ok(false);
    }
    let changed = git_paths(
        runner,
        vec![
            OsString::from("diff-tree"),
            OsString::from("--no-commit-id"),
            OsString::from("--name-only"),
            OsString::from("-r"),
            OsString::from("-z"),
            OsString::from(&parent.hex),
            OsString::from(&commit.hex),
            OsString::from("--"),
        ],
        &[],
    )?;
    Ok(changed.contains(anchor_path) && changed.contains(proposal_path))
}

fn matching_submission_links(
    runner: &GitRunner,
    tree: &GitOid,
    proposals_path: &str,
    receipt_path: &str,
    receipt_root: &str,
    operation_id: &str,
) -> Result<Vec<String>, String> {
    let entries = tree_entries(runner, tree, &[proposals_path.to_string()])?;
    let mut matches = Vec::new();
    for (path, (_mode, oid)) in entries {
        let bytes = runner.checked(
            &[
                OsString::from("cat-file"),
                OsString::from("blob"),
                OsString::from(&oid.hex),
            ],
            None,
            None,
        )?;
        let Ok(proposal) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let links = proposal
            .get("payload")
            .and_then(|payload| payload.get("vela_submission"));
        let source_has_receipt = proposal
            .get("source_refs")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|refs| {
                refs.iter()
                    .any(|value| value.as_str() == Some(receipt_path))
            });
        if proposal.get("kind").and_then(serde_json::Value::as_str) == Some("finding.add")
            && links
                .and_then(|value| value.get("schema"))
                .and_then(serde_json::Value::as_str)
                == Some("vela.submission-links.internal.v1")
            && links
                .and_then(|value| value.get("receipt_path"))
                .and_then(serde_json::Value::as_str)
                == Some(receipt_path)
            && links
                .and_then(|value| value.get("receipt_root"))
                .and_then(serde_json::Value::as_str)
                == Some(receipt_root)
            && links
                .and_then(|value| value.get("operation_id"))
                .and_then(serde_json::Value::as_str)
                == Some(operation_id)
            && source_has_receipt
        {
            matches.push(path);
        }
    }
    Ok(matches)
}

/// Compare the target tree to the caller-bound raw preimages without relying
/// on the post-transaction worktree. Git stores LFS pointers, while the
/// frontier transaction binds the raw scientific bytes, so LFS entries are
/// resolved through the locally verified object store before hashing.
fn target_matches_exact_preimages(
    runner: &GitRunner,
    delta: &PublicationDelta,
    parent: &TreeMap,
    specs: &[String],
    attribute_index: &Path,
) -> Result<bool, String> {
    for item in &delta.entries {
        let Some(expected_hash) = &item.preimage_sha256 else {
            if parent.contains_key(&item.path) {
                return Ok(false);
            }
            continue;
        };
        let Some((_mode, oid)) = parent.get(&item.path) else {
            return Ok(false);
        };
        let stored = runner.checked(
            &[
                OsString::from("cat-file"),
                OsString::from("blob"),
                OsString::from(&oid.hex),
            ],
            None,
            None,
        )?;
        let raw = match effective_content_mode(
            runner,
            &item.path,
            is_witness_path(&item.path, specs),
            attribute_index,
        )? {
            ContentMode::Raw => stored,
            ContentMode::Lfs => {
                let pointer = parse_lfs_pointer(&stored).map_err(|error| {
                    format!(
                        "LFS availability failure: target blob at {} is not a canonical pointer: {error}",
                        item.path
                    )
                })?;
                read_verified_local_lfs_object(runner, &item.path, &pointer)?
            }
        };
        if sha256(&raw) != *expected_hash {
            return Ok(false);
        }
    }
    Ok(true)
}

fn target_matches_exact_postimages(parent: &TreeMap, desired: &DesiredMap) -> bool {
    desired
        .iter()
        .all(|(path, wanted)| match (parent.get(path), wanted) {
            (Some((mode, oid)), Some(wanted)) => mode == &wanted.mode && oid == &wanted.oid,
            (None, None) => true,
            _ => false,
        })
}

fn exact_desired_entries(
    runner: &GitRunner,
    delta: &PublicationDelta,
    root: &Path,
    specs: &[String],
    object_format: &str,
    attribute_index: &Path,
) -> Result<(DesiredMap, WorktreeHashMap), String> {
    let mut desired = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    for item in &delta.entries {
        let absolute = root.join(&item.path);
        match &item.postimage {
            Some(bytes) => {
                let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
                    format!("inspect exact publication postimage {}: {error}", item.path)
                })?;
                if !metadata.file_type().is_file() {
                    return Err(format!(
                        "exact publication postimage is not a regular file at {}",
                        item.path
                    ));
                }
                let actual = fs::read(&absolute).map_err(|error| {
                    format!("read exact publication postimage {}: {error}", item.path)
                })?;
                if actual != *bytes {
                    return Err(format!(
                        "installed public postimage differs from supplied bytes at {}",
                        item.path
                    ));
                }
                let mode = if item.executable { "100755" } else { "100644" }.to_string();
                if regular_file_mode(&metadata) != mode {
                    return Err(format!(
                        "installed public postimage mode differs from supplied mode at {}",
                        item.path
                    ));
                }
                let entry = desired_entry_from_bytes(
                    runner,
                    &item.path,
                    bytes,
                    mode,
                    specs,
                    object_format,
                    attribute_index,
                )?;
                hashes.insert(item.path.clone(), Some(sha256(bytes)));
                desired.insert(item.path.clone(), Some(entry));
            }
            None => match fs::symlink_metadata(&absolute) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    hashes.insert(item.path.clone(), None);
                    desired.insert(item.path.clone(), None);
                }
                Ok(_) => {
                    return Err(format!(
                        "exact publication deletion remains present at {}",
                        item.path
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "inspect exact publication deletion {}: {error}",
                        item.path
                    ));
                }
            },
        }
    }
    Ok((desired, hashes))
}

#[cfg(unix)]
fn regular_file_mode(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o111 == 0 {
        "100644".to_string()
    } else {
        "100755".to_string()
    }
}

#[cfg(not(unix))]
fn regular_file_mode(_metadata: &fs::Metadata) -> String {
    "100644".to_string()
}

fn is_witness_path(path: &str, specs: &[String]) -> bool {
    specs.iter().any(|spec| {
        (spec == "witnesses" || spec.ends_with("/witnesses"))
            && path
                .strip_prefix(spec)
                .is_some_and(|suffix| suffix.starts_with('/'))
    })
}

fn reject_local_attribute_overrides(runner: &GitRunner) -> Result<(), String> {
    let info = git_private_path(runner, "info/attributes")?;
    if info.is_file() {
        let content = fs::read_to_string(&info)
            .map_err(|error| format!("read local Git attributes {}: {error}", info.display()))?;
        if content
            .lines()
            .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        {
            return Err(
                "publication refuses repository-local .git/info/attributes overrides".to_string(),
            );
        }
    }
    Ok(())
}

fn validate_candidate_attributes(
    runner: &GitRunner,
    desired: &DesiredMap,
    specs: &[String],
    attribute_index: &Path,
) -> Result<(), String> {
    for (path, entry) in desired {
        if let Some(entry) = entry {
            let actual = effective_content_mode(
                runner,
                path,
                is_witness_path(path, specs),
                attribute_index,
            )?;
            if actual != entry.content_mode {
                return Err(format!(
                    "candidate Git attributes changed content semantics at {path}"
                ));
            }
        }
    }
    Ok(())
}

fn effective_content_mode(
    runner: &GitRunner,
    path: &str,
    witness_path: bool,
    attribute_index: &Path,
) -> Result<ContentMode, String> {
    let output = runner.checked(
        &[
            OsString::from("check-attr"),
            OsString::from("--cached"),
            OsString::from("-z"),
            OsString::from("--all"),
            OsString::from("--"),
            OsString::from(path),
        ],
        None,
        Some(attribute_index),
    )?;
    let fields = split_nul(&output)?;
    let attributes = fields
        .chunks_exact(3)
        .map(|triple| (triple[1].clone(), triple[2].clone()))
        .collect::<BTreeMap<_, _>>();
    let filter = attributes.get("filter").map(String::as_str);
    let lfs = filter == Some("lfs");
    if lfs && !witness_path {
        return Err(format!(
            "Git LFS is permitted only under the resolved witnesses path, not {path}"
        ));
    }
    for (attribute, value) in &attributes {
        let active = !matches!(value.as_str(), "unspecified" | "unset");
        let lfs_transport_attribute =
            lfs && matches!(attribute.as_str(), "filter" | "diff" | "merge") && value == "lfs";
        if active && lfs_transport_attribute {
            continue;
        }
        if active
            && matches!(
                attribute.as_str(),
                "filter" | "ident" | "working-tree-encoding" | "crlf" | "merge"
            )
        {
            return Err(format!(
                "unsafe effective Git attribute `{attribute}={value}` at {path}"
            ));
        }
        if attribute == "eol" && active && value != "lf" {
            return Err(format!(
                "unsafe effective Git attribute `eol={value}` at {path}"
            ));
        }
        if lfs && attribute == "text" && active {
            return Err(format!(
                "Git LFS witness must disable text conversion at {path}"
            ));
        }
    }
    Ok(if lfs {
        ContentMode::Lfs
    } else {
        ContentMode::Raw
    })
}

fn prepare_lfs_content(runner: &GitRunner, path: &str, worktree: &[u8]) -> Result<Vec<u8>, String> {
    let clean = runner
        .text(&["config", "--get", "filter.lfs.clean"])
        .map_err(|_| "LFS availability failure: filter.lfs.clean is not configured".to_string())?;
    let process = runner
        .text(&["config", "--get", "filter.lfs.process"])
        .map_err(|_| {
            "LFS availability failure: filter.lfs.process is not configured".to_string()
        })?;
    let required = runner
        .text(&["config", "--bool", "--get", "filter.lfs.required"])
        .unwrap_or_default();
    if !clean.starts_with("git-lfs clean")
        || !process.starts_with("git-lfs filter-process")
        || required != "true"
    {
        return Err(
            "LFS availability failure: Git LFS filters are not safely configured".to_string(),
        );
    }
    runner
        .text(&["lfs", "version"])
        .map_err(|error| format!("LFS availability failure: Git LFS is unavailable: {error}"))?;

    let pointer_bytes = if parse_lfs_pointer(worktree).is_ok() {
        worktree.to_vec()
    } else {
        runner
            .checked(
                &[
                    OsString::from("lfs"),
                    OsString::from("clean"),
                    OsString::from("--"),
                    OsString::from(path),
                ],
                Some(worktree),
                None,
            )
            .map_err(|error| format!("LFS availability failure: clean {path}: {error}"))?
    };
    let pointer = parse_lfs_pointer(&pointer_bytes)
        .map_err(|error| format!("LFS availability failure at {path}: {error}"))?;
    if parse_lfs_pointer(worktree).is_err()
        && (pointer.oid != hex::encode(Sha256::digest(worktree))
            || pointer.size != worktree.len() as u64)
    {
        return Err(format!(
            "LFS availability failure: pointer oid/size do not bind raw bytes at {path}"
        ));
    }
    verify_local_lfs_object(runner, path, &pointer)?;
    Ok(pointer_bytes)
}

fn parse_lfs_pointer(bytes: &[u8]) -> Result<LfsPointer, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "pointer is not UTF-8".to_string())?;
    let lines = text.lines().collect::<Vec<_>>();
    if !text.ends_with('\n')
        || lines.len() != 3
        || lines[0] != "version https://git-lfs.github.com/spec/v1"
    {
        return Err("pointer is not canonical Git LFS v1".to_string());
    }
    let oid = lines[1]
        .strip_prefix("oid sha256:")
        .ok_or_else(|| "pointer is missing sha256 oid".to_string())?;
    if oid.len() != 64 || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("pointer has an invalid sha256 oid".to_string());
    }
    let size = lines[2]
        .strip_prefix("size ")
        .ok_or_else(|| "pointer is missing size".to_string())?
        .parse::<u64>()
        .map_err(|_| "pointer has an invalid size".to_string())?;
    let canonical = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n",
        oid.to_ascii_lowercase()
    );
    if canonical.as_bytes() != bytes {
        return Err("pointer is not in canonical byte form".to_string());
    }
    Ok(LfsPointer {
        oid: oid.to_ascii_lowercase(),
        size,
    })
}

fn verify_local_lfs_object(
    runner: &GitRunner,
    witness_path: &str,
    pointer: &LfsPointer,
) -> Result<(), String> {
    read_verified_local_lfs_object(runner, witness_path, pointer).map(drop)
}

fn read_verified_local_lfs_object(
    runner: &GitRunner,
    witness_path: &str,
    pointer: &LfsPointer,
) -> Result<Vec<u8>, String> {
    let env = runner
        .checked(&[OsString::from("lfs"), OsString::from("env")], None, None)
        .map_err(|error| format!("LFS availability failure: inspect local storage: {error}"))?;
    let env = String::from_utf8_lossy(&env);
    let media_dir = env
        .lines()
        .find_map(|line| line.strip_prefix("LocalMediaDir="))
        .ok_or_else(|| "LFS availability failure: LocalMediaDir is unknown".to_string())?;
    let object = Path::new(media_dir)
        .join(&pointer.oid[..2])
        .join(&pointer.oid[2..4])
        .join(&pointer.oid);
    let bytes = fs::read(&object).map_err(|error| {
        format!(
            "LFS availability failure: local object for {witness_path} is missing at {}: {error}",
            object.display()
        )
    })?;
    if bytes.len() as u64 != pointer.size || hex::encode(Sha256::digest(&bytes)) != pointer.oid {
        return Err(format!(
            "LFS availability failure: local object for {witness_path} fails oid/size verification"
        ));
    }
    Ok(bytes)
}

fn hash_object(
    runner: &GitRunner,
    object_format: &str,
    bytes: &[u8],
    write: bool,
) -> Result<GitOid, String> {
    let mut args = vec![OsString::from("hash-object")];
    if write {
        args.push(OsString::from("-w"));
    }
    args.push(OsString::from("--no-filters"));
    args.push(OsString::from("--stdin"));
    let output = runner.checked(&args, Some(bytes), None)?;
    GitOid::parse(object_format, String::from_utf8_lossy(&output).trim())
}

fn mapping_matches(parent: &TreeMap, desired: &BTreeMap<String, Option<DesiredEntry>>) -> bool {
    parent.len() == desired.values().filter(|entry| entry.is_some()).count()
        && desired
            .iter()
            .all(|(path, wanted)| match (parent.get(path), wanted) {
                (Some((mode, oid)), Some(wanted)) => mode == &wanted.mode && oid == &wanted.oid,
                (None, None) => true,
                _ => false,
            })
}

fn inspect_exact_candidate(
    runner: &GitRunner,
    expected: &GitOid,
    tree: &GitOid,
    desired: &BTreeMap<String, Option<DesiredEntry>>,
) -> Result<(), String> {
    let changed = git_paths(
        runner,
        vec![
            OsString::from("diff-tree"),
            OsString::from("--no-commit-id"),
            OsString::from("--name-only"),
            OsString::from("-r"),
            OsString::from("-z"),
            OsString::from(&expected.hex),
            OsString::from(&tree.hex),
            OsString::from("--"),
        ],
        &[],
    )?;
    let supplied = desired.keys().cloned().collect::<BTreeSet<_>>();
    if changed != supplied {
        let missing = supplied.difference(&changed).cloned().collect::<Vec<_>>();
        let unexpected = changed.difference(&supplied).cloned().collect::<Vec<_>>();
        return Err(format!(
            "candidate diff does not equal the supplied public delta (missing: {}; unexpected: {})",
            missing.join(", "),
            unexpected.join(", ")
        ));
    }
    let exact_paths = supplied.into_iter().collect::<Vec<_>>();
    let actual = tree_entries(runner, tree, &exact_paths)?;
    if !mapping_matches(&actual, desired) {
        return Err("candidate tree does not equal the supplied public postimages".to_string());
    }
    for (path, entry) in desired {
        if let Some(entry) = entry {
            let bytes = runner.checked(
                &[
                    OsString::from("cat-file"),
                    OsString::from("blob"),
                    OsString::from(&entry.oid.hex),
                ],
                None,
                None,
            )?;
            if bytes != entry.bytes {
                return Err(format!(
                    "candidate authority blob differs from supplied bytes at {path}"
                ));
            }
        }
    }
    Ok(())
}

fn index_snapshot(runner: &GitRunner) -> Result<IndexMap, String> {
    index_snapshot_at(runner, None)
}

fn index_snapshot_at(runner: &GitRunner, index: Option<&Path>) -> Result<IndexMap, String> {
    let stage = runner.checked(
        &[
            OsString::from("ls-files"),
            OsString::from("--stage"),
            OsString::from("-z"),
        ],
        None,
        index,
    )?;
    let flags = runner.checked(
        &[
            OsString::from("ls-files"),
            OsString::from("-v"),
            OsString::from("-z"),
        ],
        None,
        index,
    )?;
    let mut map = BTreeMap::new();
    for row in split_nul(&stage)? {
        let (header, path) = row
            .split_once('\t')
            .ok_or_else(|| format!("malformed index row `{row}`"))?;
        map.insert(path.to_string(), header.to_string());
    }
    for row in split_nul(&flags)? {
        let (flag, path) = row
            .split_once(' ')
            .ok_or_else(|| format!("malformed index flag row `{row}`"))?;
        map.entry(path.to_string())
            .and_modify(|header| header.push_str(&format!("|{flag}")));
    }
    Ok(map)
}

fn capture_index(runner: &GitRunner) -> Result<(IndexMap, String), String> {
    let path = git_private_path(runner, "index")?;
    for _ in 0..3 {
        let before = fs::read(&path)
            .map_err(|error| format!("read caller index {}: {error}", path.display()))?;
        let snapshot = index_snapshot(runner)?;
        let after = fs::read(&path)
            .map_err(|error| format!("re-read caller index {}: {error}", path.display()))?;
        if before == after {
            return Ok((snapshot, sha256(&before)));
        }
    }
    Err("caller index changed while publication captured it".to_string())
}

fn atomically_reconcile_index(
    runner: &GitRunner,
    original: &IndexMap,
    original_sha256: &str,
    input: &[u8],
) -> Result<IndexMap, String> {
    let index_path = git_private_path(runner, "index")?;
    let (current, current_sha256) = capture_index(runner)?;
    if current != *original || current_sha256 != original_sha256 {
        return Err("caller index drifted before atomic reconciliation".to_string());
    }
    let original_bytes = fs::read(&index_path)
        .map_err(|error| format!("read real Git index {}: {error}", index_path.display()))?;
    if sha256(&original_bytes) != original_sha256 {
        return Err("real Git index bytes drifted before alternate-index build".to_string());
    }
    let parent = index_path
        .parent()
        .ok_or_else(|| "real Git index has no parent directory".to_string())?;
    let mut alternate = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create alternate Git index: {error}"))?;
    alternate
        .write_all(&original_bytes)
        .and_then(|()| alternate.as_file().sync_all())
        .map_err(|error| format!("seed alternate Git index: {error}"))?;
    runner.checked(
        &[
            OsString::from("update-index"),
            OsString::from("-z"),
            OsString::from("--index-info"),
        ],
        Some(input),
        Some(alternate.path()),
    )?;
    let reconciled = index_snapshot_at(runner, Some(alternate.path()))?;
    let replacement =
        fs::read(alternate.path()).map_err(|error| format!("read alternate Git index: {error}"))?;

    let lock = RealIndexLock::acquire(&index_path)?;
    let current_bytes = fs::read(&index_path)
        .map_err(|error| format!("re-read real Git index under lock: {error}"))?;
    if sha256(&current_bytes) != original_sha256 {
        return Err("real Git index changed while acquiring index.lock".to_string());
    }
    lock.install(&index_path, &replacement)?;
    Ok(reconciled)
}

fn reconcile_current_index(
    runner: &GitRunner,
    desired: &BTreeMap<String, Option<DesiredEntry>>,
    original: &IndexMap,
    original_sha256: &str,
    worktree_hashes: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    let (current, current_sha256) = capture_index(runner)?;
    if current != *original || current_sha256 != original_sha256 {
        return Err("caller index drifted before post-ref reconciliation".to_string());
    }
    if !worktree_matches(&runner.root, worktree_hashes, &desired_modes(desired))? {
        return Err("Vela worktree drifted before post-ref reconciliation".to_string());
    }
    let zeros = "0".repeat(
        desired
            .values()
            .find_map(|entry| entry.as_ref().map(|entry| entry.oid.hex.len()))
            .unwrap_or(40),
    );
    let mut input = Vec::new();
    for (path, entry) in desired {
        match entry {
            Some(entry) => input.extend_from_slice(
                format!("{} {}\t{}\0", entry.mode, entry.oid.hex, path).as_bytes(),
            ),
            None => input.extend_from_slice(format!("0 {zeros}\t{path}\0").as_bytes()),
        }
    }
    let after = atomically_reconcile_index(runner, original, original_sha256, &input)?;
    for (path, entry) in original {
        if !desired.contains_key(path) && after.get(path) != Some(entry) {
            return Err(format!("unrelated index entry changed at {path}"));
        }
    }
    for (path, entry) in desired {
        match entry {
            Some(entry) => {
                let expected = format!("{} {} 0|H", entry.mode, entry.oid.hex);
                if after.get(path) != Some(&expected) {
                    return Err(format!("post-ref index entry did not align at {path}"));
                }
            }
            None if after.contains_key(path) => {
                return Err(format!("deleted Vela path remains in the index at {path}"));
            }
            None => {}
        }
    }
    if !worktree_matches(&runner.root, worktree_hashes, &desired_modes(desired))? {
        return Err("publication changed Vela worktree bytes".to_string());
    }
    Ok(())
}

fn reconcile_journal_index(
    runner: &GitRunner,
    journal: &PublicationJournal,
) -> Result<(), JournalIndexReconcileError> {
    let frontier = Path::new(&journal.frontier);
    let specs =
        frontier_specs(frontier, &runner.root).map_err(JournalIndexReconcileError::Refused)?;
    reject_unsupported_index(
        runner,
        &specs,
        &journal.expected_git_commit_oid.object_format,
    )
    .map_err(JournalIndexReconcileError::Refused)?;
    let (current, current_sha256) =
        capture_index(runner).map_err(JournalIndexReconcileError::Retryable)?;
    let desired_paths = journal
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    let already_aligned = journal
        .entries
        .iter()
        .all(|entry| match (&entry.mode, &entry.oid) {
            (Some(mode), Some(oid)) => {
                current.get(&entry.path) == Some(&format!("{mode} {} 0|H", oid.hex))
            }
            (None, None) => !current.contains_key(&entry.path),
            _ => false,
        });
    let after = if !already_aligned {
        for entry in &journal.entries {
            let aligned = match (&entry.mode, &entry.oid) {
                (Some(mode), Some(oid)) => {
                    current.get(&entry.path) == Some(&format!("{mode} {} 0|H", oid.hex))
                }
                (None, None) => !current.contains_key(&entry.path),
                _ => {
                    return Err(JournalIndexReconcileError::Refused(format!(
                        "incomplete journal entry for {}",
                        entry.path
                    )));
                }
            };
            if !aligned && current.get(&entry.path) != journal.original_index.get(&entry.path) {
                return Err(JournalIndexReconcileError::Refused(
                    "publication recovery refuses Vela index drift".to_string(),
                ));
            }
        }
        let zeros = "0".repeat(journal.expected_git_commit_oid.hex.len());
        let mut input = Vec::new();
        for entry in &journal.entries {
            match (&entry.mode, &entry.oid) {
                (Some(mode), Some(oid)) => input
                    .extend_from_slice(format!("{mode} {}\t{}\0", oid.hex, entry.path).as_bytes()),
                (None, None) => {
                    input.extend_from_slice(format!("0 {zeros}\t{}\0", entry.path).as_bytes())
                }
                _ => {
                    return Err(JournalIndexReconcileError::Refused(format!(
                        "incomplete journal entry for {}",
                        entry.path
                    )));
                }
            }
        }
        atomically_reconcile_index(runner, &current, &current_sha256, &input)
            .map_err(JournalIndexReconcileError::Retryable)?
    } else {
        current.clone()
    };
    let unrelated_unchanged = current.iter().all(|(path, value)| {
        desired_paths.contains(path.as_str()) || after.get(path) == Some(value)
    }) && after
        .iter()
        .all(|(path, _)| desired_paths.contains(path.as_str()) || current.contains_key(path));
    if !unrelated_unchanged {
        return Err(JournalIndexReconcileError::Refused(
            "publication recovery changed an unrelated index entry".to_string(),
        ));
    }
    for entry in &journal.entries {
        match (&entry.mode, &entry.oid) {
            (Some(mode), Some(oid))
                if after.get(&entry.path) == Some(&format!("{mode} {} 0|H", oid.hex)) => {}
            (None, None) if !after.contains_key(&entry.path) => {}
            _ => {
                return Err(JournalIndexReconcileError::Refused(format!(
                    "journal recovery did not align {}",
                    entry.path
                )));
            }
        }
    }
    if !journal_worktree_matches(&runner.root, journal)
        .map_err(JournalIndexReconcileError::Refused)?
    {
        return Err(JournalIndexReconcileError::Refused(
            "publication recovery changed Vela worktree bytes".to_string(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn worktree_matches(
    root: &Path,
    expected: &BTreeMap<String, Option<String>>,
    modes: &BTreeMap<String, Option<String>>,
) -> Result<bool, String> {
    for (path, expected_hash) in expected {
        let absolute = root.join(path);
        let actual = match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if modes.get(path).and_then(Option::as_ref) != Some(&regular_file_mode(&metadata)) {
                    return Ok(false);
                }
                Some(sha256(&fs::read(&absolute).map_err(|error| {
                    format!("read Vela path {path}: {error}")
                })?))
            }
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("inspect Vela path {path}: {error}")),
        };
        if &actual != expected_hash {
            return Ok(false);
        }
    }
    Ok(true)
}

fn desired_modes(desired: &DesiredMap) -> BTreeMap<String, Option<String>> {
    desired
        .iter()
        .map(|(path, entry)| (path.clone(), entry.as_ref().map(|entry| entry.mode.clone())))
        .collect()
}

fn journal_worktree_matches(root: &Path, journal: &PublicationJournal) -> Result<bool, String> {
    let hashes = journal
        .entries
        .iter()
        .map(|entry| (entry.path.clone(), entry.worktree_sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    let modes = journal
        .entries
        .iter()
        .map(|entry| (entry.path.clone(), entry.mode.clone()))
        .collect::<BTreeMap<_, _>>();
    worktree_matches(root, &hashes, &modes)
}

fn publication_message(summary: &str, event_ids: &[String]) -> String {
    let mut message = summary
        .chars()
        .filter(|character| *character != '\0' && (*character == '\n' || !character.is_control()))
        .take(512)
        .collect::<String>();
    if !event_ids.is_empty() {
        message.push_str("\n\nsigned events:\n");
        for id in event_ids.iter().take(20) {
            let bounded = id
                .chars()
                .filter(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
                .take(128)
                .collect::<String>();
            message.push_str("  ");
            message.push_str(&bounded);
            message.push('\n');
        }
        if event_ids.len() > 20 {
            message.push_str(&format!("  +{} more\n", event_ids.len() - 20));
        }
    }
    message
}

fn identity_value(runner: &GitRunner, key: &str, fallback: &str) -> Result<String, String> {
    let value = runner
        .text(&["config", "--get", key])
        .unwrap_or_else(|_| fallback.to_string());
    if value.is_empty() || value.contains(['\n', '\r', '\0']) {
        return Err(format!("unsafe Git identity value for {key}"));
    }
    Ok(value)
}

fn git_private_path(runner: &GitRunner, name: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(runner.text(&["rev-parse", "--git-path", name])?);
    Ok(if path.is_absolute() {
        path
    } else {
        runner.root.join(path)
    })
}

fn acquire_publication_lock(runner: &GitRunner) -> Result<PublicationLock, PublicationLockError> {
    let path =
        git_private_path(runner, "vela/publication.lock").map_err(PublicationLockError::Failed)?;
    let parent = path.parent().ok_or_else(|| {
        PublicationLockError::Failed("publication lock path has no parent".to_string())
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        PublicationLockError::Failed(format!("create publication lock directory: {error}"))
    })?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| {
            PublicationLockError::Failed(format!(
                "open publication lock {}: {error}",
                path.display()
            ))
        })?;
    match file.try_lock() {
        Ok(()) => Ok(PublicationLock {
            _file: file,
            repository: runner.root.clone(),
        }),
        Err(std::fs::TryLockError::WouldBlock) => Err(PublicationLockError::Busy),
        Err(std::fs::TryLockError::Error(error)) => Err(PublicationLockError::Failed(format!(
            "lock Git publication {}: {error}",
            path.display()
        ))),
    }
}

fn valid_operation_id(operation_id: &str) -> bool {
    operation_id.strip_prefix("vop_").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn journal_entries(
    desired: &BTreeMap<String, Option<DesiredEntry>>,
    hashes: &BTreeMap<String, Option<String>>,
) -> Vec<JournalEntry> {
    desired
        .iter()
        .map(|(path, entry)| JournalEntry {
            path: path.clone(),
            mode: entry.as_ref().map(|entry| entry.mode.clone()),
            oid: entry.as_ref().map(|entry| entry.oid.clone()),
            worktree_sha256: hashes.get(path).cloned().flatten(),
        })
        .collect()
}

fn operation_recovery_outcome(candidate: &GitOid, operation_id: &str) -> PublicationOutcome {
    PublicationOutcome {
        state: PublicationState::CommittedLocal {
            commit: candidate.hex.clone(),
        },
        recovery_command: Some(format!(
            "vela publication recover --operation {operation_id}"
        )),
    }
}

fn uncommitted_operation_outcome(
    candidate: Option<&GitOid>,
    operation_id: &str,
    reason: impl Into<String>,
) -> PublicationOutcome {
    PublicationOutcome {
        state: PublicationState::Uncommitted {
            candidate: candidate.map(|candidate| candidate.hex.clone()),
            reason: reason.into(),
        },
        recovery_command: Some(format!(
            "vela publication recover --operation {operation_id}"
        )),
    }
}

fn complete_publication(
    operation_id: &str,
    candidate: &GitOid,
    journal_path: &Path,
    completed_dir: &Path,
    mut outcome: PublicationOutcome,
    complete_local: bool,
    _opts: &PublishOptions,
) -> PublicationOutcome {
    if !complete_local && !matches!(outcome.state, PublicationState::Pushed { .. }) {
        // A newly queued local publication or an indeterminate push remains
        // resumable. A deliberately local recovery sets `complete_local` and
        // closes the repair operation while retaining its direct push command.
        outcome.recovery_command = Some(format!(
            "vela publication recover --operation {operation_id} --push"
        ));
        return outcome;
    }
    let completed_path = crate::operation_journal::path(completed_dir, operation_id);
    let completed = CompletedPublication {
        schema: crate::operation_journal::JOURNAL_SCHEMA.to_string(),
        operation_id: operation_id.to_string(),
        outcome: outcome.clone(),
    };
    #[cfg(test)]
    if _opts.test_step == PublicationTestStep::FailCompletedRecordWrite {
        return operation_recovery_outcome(candidate, operation_id);
    }
    if crate::operation_journal::write_json(&completed_path, &completed).is_err() {
        return operation_recovery_outcome(candidate, operation_id);
    }
    #[cfg(test)]
    if _opts.test_step == PublicationTestStep::FailActiveJournalRemove {
        return operation_recovery_outcome(candidate, operation_id);
    }
    if crate::operation_journal::remove(journal_path).is_err() {
        return operation_recovery_outcome(candidate, operation_id);
    }
    #[cfg(test)]
    if _opts.test_step == PublicationTestStep::FailCompletedRecordPrune {
        return operation_recovery_outcome(candidate, operation_id);
    }
    if crate::operation_journal::prune_json(completed_dir, 64).is_err() {
        return operation_recovery_outcome(candidate, operation_id);
    }
    outcome
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn upstream_target(
    runner: &GitRunner,
    txn: &GitPublicationTxn,
) -> Result<Option<UpstreamTarget>, String> {
    upstream_target_for_ref(runner, &txn.target_refname)
}

fn upstream_target_for_ref(
    runner: &GitRunner,
    target_refname: &GitRefName,
) -> Result<Option<UpstreamTarget>, String> {
    let format = "%(upstream:remotename)%00%(upstream:remoteref)%00";
    let bytes = runner.checked(
        &[
            OsString::from("for-each-ref"),
            OsString::from(format!("--format={format}")),
            OsString::from("--"),
            OsString::from(&target_refname.0),
        ],
        None,
        None,
    )?;
    let mut fields = bytes.splitn(3, |byte| *byte == 0);
    let remote = fields.next().unwrap_or_default();
    let reference = fields.next().unwrap_or_default();
    let trailer = fields.next().unwrap_or_default();
    if !matches!(trailer, b"" | b"\n") {
        return Err("Git returned malformed upstream metadata".to_string());
    }
    if remote.is_empty() && reference.is_empty() {
        return Ok(None);
    }
    if remote.is_empty() || reference.is_empty() {
        return Err("Git returned incomplete upstream metadata".to_string());
    }
    let remote = String::from_utf8(remote.to_vec())
        .map_err(|_| "upstream remote name is not UTF-8".to_string())?;
    let reference = String::from_utf8(reference.to_vec())
        .map_err(|_| "upstream remote ref is not UTF-8".to_string())?;
    if remote.chars().any(char::is_control) {
        return Err("upstream remote name contains control characters".to_string());
    }
    runner.checked(
        &[
            OsString::from("check-ref-format"),
            OsString::from(&reference),
        ],
        None,
        None,
    )?;
    Ok(Some(UpstreamTarget { remote, reference }))
}

fn discovered_publication_outcome(
    runner: &GitRunner,
    target_refname: &GitRefName,
    commit: GitOid,
) -> PublicationOutcome {
    let pushed_remote = upstream_target_for_ref(runner, target_refname)
        .ok()
        .flatten()
        .filter(|upstream| remote_ref_provably_contains(runner, upstream, &commit));
    match pushed_remote {
        Some(upstream) => PublicationOutcome {
            state: PublicationState::Pushed {
                commit: commit.hex,
                remote: upstream.remote,
            },
            recovery_command: None,
        },
        None => PublicationOutcome {
            state: PublicationState::CommittedLocal { commit: commit.hex },
            recovery_command: None,
        },
    }
}

fn commit_is_ancestor(
    runner: &GitRunner,
    ancestor: &GitOid,
    descendant: &str,
) -> Result<bool, String> {
    let output = runner.run(
        &[
            OsString::from("merge-base"),
            OsString::from("--is-ancestor"),
            OsString::from(&ancestor.hex),
            OsString::from(descendant),
        ],
        None,
        None,
        None,
    )?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(format!(
        "inspect publication ancestry: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn remote_ref_provably_contains(
    runner: &GitRunner,
    upstream: &UpstreamTarget,
    commit: &GitOid,
) -> bool {
    let Ok(Some(tip)) = remote_ref_tip(runner, upstream, &commit.object_format) else {
        return false;
    };
    if tip == *commit {
        return true;
    }
    let Ok(present) = runner.run(
        &[
            OsString::from("cat-file"),
            OsString::from("-e"),
            OsString::from(format!("{}^{{commit}}", tip.hex)),
        ],
        None,
        None,
        None,
    ) else {
        return false;
    };
    if !present.status.success() {
        return false;
    }
    runner
        .run(
            &[
                OsString::from("merge-base"),
                OsString::from("--is-ancestor"),
                OsString::from(&commit.hex),
                OsString::from(&tip.hex),
            ],
            None,
            None,
            None,
        )
        .is_ok_and(|output| output.status.success())
}

fn remote_ref_tip(
    runner: &GitRunner,
    upstream: &UpstreamTarget,
    object_format: &str,
) -> Result<Option<GitOid>, String> {
    let output = runner.run(
        &[
            OsString::from("ls-remote"),
            OsString::from("--exit-code"),
            OsString::from("--"),
            OsString::from(&upstream.remote),
            OsString::from(&upstream.reference),
        ],
        None,
        None,
        None,
    )?;
    if output.status.code() == Some(2) && output.stdout.is_empty() && output.stderr.is_empty() {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "remote ref observation was not UTF-8".to_string())?;
    let rows = text.lines().collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(format!(
            "remote ref observation returned {} rows",
            rows.len()
        ));
    }
    let mut fields = rows[0].split_whitespace();
    let oid = fields
        .next()
        .ok_or_else(|| "remote ref observation omitted an oid".to_string())?;
    let reference = fields
        .next()
        .ok_or_else(|| "remote ref observation omitted a ref".to_string())?;
    if fields.next().is_some() || reference != upstream.reference {
        return Err("remote ref observation was malformed".to_string());
    }
    GitOid::parse(object_format, oid).map(Some)
}

fn diagnostic_upstream_command(txn: &GitPublicationTxn) -> String {
    format!(
        "git for-each-ref --format={} -- {}",
        shell_quote("%(refname) %(upstream:remotename) %(upstream:remoteref)"),
        shell_quote(&txn.target_refname.0)
    )
}

fn push_command_for(txn: &GitPublicationTxn, upstream: Option<&UpstreamTarget>) -> String {
    let _bound_plan = (
        &txn.target_checkout,
        &txn.expected_git_commit_oid,
        &txn.candidate_tree_oid,
        &txn.candidate_commit_oid,
    );
    let Some(upstream) = upstream else {
        return diagnostic_upstream_command(txn);
    };
    let source = txn
        .candidate_commit_oid
        .as_ref()
        .map_or(txn.target_refname.0.as_str(), |candidate| {
            candidate.hex.as_str()
        });
    let refspec = format!("{source}:{}", upstream.reference);
    let push = format!(
        "git push -- {} {}",
        shell_quote(&upstream.remote),
        shell_quote(&refspec)
    );
    if txn.lfs_objects.is_empty() {
        return push;
    }
    if upstream.remote.starts_with('-') {
        return diagnostic_upstream_command(txn);
    }
    let mut command = format!("git lfs push --object-id {}", shell_quote(&upstream.remote));
    let oids = txn
        .lfs_objects
        .iter()
        .map(|pointer| pointer.oid.as_str())
        .collect::<BTreeSet<_>>();
    for oid in oids {
        command.push(' ');
        command.push_str(&shell_quote(oid));
    }
    format!("{command} && {push}")
}

fn push_command(runner: &GitRunner, txn: &GitPublicationTxn) -> String {
    match upstream_target(runner, txn) {
        Ok(upstream) => push_command_for(txn, upstream.as_ref()),
        Err(_) => diagnostic_upstream_command(txn),
    }
}

fn upload_lfs_objects(
    runner: &GitRunner,
    remote: &str,
    pointers: &[LfsPointer],
) -> Result<(), String> {
    if pointers.is_empty() {
        return Ok(());
    }
    if remote.starts_with('-') {
        return Err("LFS upload refuses an option-like remote name".to_string());
    }
    runner.text(&["lfs", "version"])?;
    let mut input = pointers
        .iter()
        .map(|pointer| pointer.oid.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    input.push(b'\n');
    runner.checked(
        &[
            OsString::from("lfs"),
            OsString::from("push"),
            OsString::from("--object-id"),
            OsString::from(remote),
            OsString::from("--stdin"),
        ],
        Some(&input),
        None,
    )?;
    Ok(())
}

enum RemoteRefObservation {
    Candidate,
    Different,
    Unknown(String),
}

fn observe_remote_ref(
    runner: &GitRunner,
    upstream: &UpstreamTarget,
    candidate: &GitOid,
) -> RemoteRefObservation {
    let output = match runner.run(
        &[
            OsString::from("ls-remote"),
            OsString::from("--exit-code"),
            OsString::from("--"),
            OsString::from(&upstream.remote),
            OsString::from(&upstream.reference),
        ],
        None,
        None,
        None,
    ) {
        Ok(output) => output,
        Err(error) => return RemoteRefObservation::Unknown(error),
    };
    if output.status.code() == Some(2) && output.stdout.is_empty() && output.stderr.is_empty() {
        return RemoteRefObservation::Different;
    }
    if !output.status.success() {
        return RemoteRefObservation::Unknown(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        );
    }
    let text = match String::from_utf8(output.stdout) {
        Ok(text) => text,
        Err(_) => {
            return RemoteRefObservation::Unknown(
                "remote ref observation was not UTF-8".to_string(),
            );
        }
    };
    let rows = text.lines().collect::<Vec<_>>();
    if rows.len() != 1 {
        return RemoteRefObservation::Unknown(format!(
            "remote ref observation returned {} rows",
            rows.len()
        ));
    }
    let mut fields = rows[0].split_whitespace();
    let Some(oid) = fields.next() else {
        return RemoteRefObservation::Unknown("remote ref observation omitted an oid".to_string());
    };
    let Some(reference) = fields.next() else {
        return RemoteRefObservation::Unknown("remote ref observation omitted a ref".to_string());
    };
    if fields.next().is_some() || reference != upstream.reference {
        return RemoteRefObservation::Unknown("remote ref observation was malformed".to_string());
    }
    match GitOid::parse(&candidate.object_format, oid) {
        Ok(actual) if actual == *candidate => RemoteRefObservation::Candidate,
        Ok(_) => RemoteRefObservation::Different,
        Err(error) => RemoteRefObservation::Unknown(error),
    }
}

fn push_and_verify(
    runner: &GitRunner,
    txn: &GitPublicationTxn,
    candidate: GitOid,
) -> PublicationOutcome {
    let upstream = match upstream_target(runner, txn) {
        Ok(Some(upstream)) => upstream,
        Ok(None) | Err(_) => {
            return PublicationOutcome {
                state: PublicationState::CommittedLocal {
                    commit: candidate.hex,
                },
                recovery_command: Some(push_command(runner, txn)),
            };
        }
    };
    let recovery = push_command_for(txn, Some(&upstream));
    if let Err(upload_error) = upload_lfs_objects(runner, &upstream.remote, &txn.lfs_objects) {
        return match observe_remote_ref(runner, &upstream, &candidate) {
            RemoteRefObservation::Different => PublicationOutcome {
                state: PublicationState::CommittedLocal {
                    commit: candidate.hex,
                },
                recovery_command: Some(recovery),
            },
            RemoteRefObservation::Candidate => PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: format!(
                        "remote ref equals the candidate but exact LFS availability was not established: {upload_error}"
                    ),
                },
                recovery_command: Some(recovery),
            },
            RemoteRefObservation::Unknown(observation) => PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: format!(
                        "LFS upload failed ({upload_error}) and remote ref is unobservable: {observation}"
                    ),
                },
                recovery_command: Some(recovery),
            },
        };
    }
    if remote_ref_provably_contains(runner, &upstream, &candidate) {
        return PublicationOutcome {
            state: PublicationState::Pushed {
                commit: candidate.hex,
                remote: upstream.remote,
            },
            recovery_command: None,
        };
    }
    let refspec = format!("{}:{}", candidate.hex, upstream.reference);
    let push = runner.run(
        &[
            OsString::from("push"),
            OsString::from("-q"),
            OsString::from("--"),
            OsString::from(&upstream.remote),
            OsString::from(&refspec),
        ],
        None,
        None,
        None,
    );
    match observe_remote_ref(runner, &upstream, &candidate) {
        RemoteRefObservation::Candidate => PublicationOutcome {
            state: PublicationState::Pushed {
                commit: candidate.hex,
                remote: upstream.remote,
            },
            recovery_command: None,
        },
        RemoteRefObservation::Different => PublicationOutcome {
            state: PublicationState::CommittedLocal {
                commit: candidate.hex,
            },
            recovery_command: Some(recovery),
        },
        RemoteRefObservation::Unknown(observation) => {
            let push_detail = match push {
                Ok(output) if output.status.success() => "push reported success".to_string(),
                Ok(output) => format!(
                    "push reported failure: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
                Err(error) => format!("push observation failed: {error}"),
            };
            PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: format!(
                        "remote publication could not be established ({push_detail}); remote observation: {observation}"
                    ),
                },
                recovery_command: Some(recovery),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn all_commit_oids(dir: &Path) -> Vec<String> {
        let mut commits = sh(
            dir,
            &[
                "cat-file",
                "--batch-all-objects",
                "--batch-check=%(objecttype) %(objectname)",
            ],
        )
        .lines()
        .filter_map(|line| line.strip_prefix("commit ").map(str::to_string))
        .collect::<Vec<_>>();
        commits.sort();
        commits
    }

    fn frontier() -> tempfile::TempDir {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path();
        sh(path, &["init", "-q", "-b", "main"]);
        sh(path, &["config", "user.email", "test@example.test"]);
        sh(path, &["config", "user.name", "Vela Test"]);
        let project = vela_protocol::project::assemble("publication", vec![], 0, 0, "test");
        vela_protocol::repo::init_repo(path, &project).unwrap();
        vela_protocol::frontier_repo::materialize(path).unwrap();
        fs::write(path.join(".gitignore"), "/.vela/work/\n/records/\n").unwrap();
        fs::write(path.join("unrelated.txt"), "base\n").unwrap();
        sh(path, &["add", "-A"]);
        sh(path, &["commit", "-q", "-m", "base"]);
        temporary
    }

    fn operation_from(outcome: &PublicationOutcome) -> String {
        let parts = outcome
            .recovery_command
            .as_deref()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        parts
            .windows(2)
            .find_map(|pair| (pair[0] == "--operation").then_some(pair[1]))
            .unwrap()
            .to_string()
    }

    fn test_runner(path: &Path, temporary: &tempfile::TempDir) -> GitRunner {
        let hooks = temporary.path().join("hooks");
        let attributes = temporary.path().join("attributes");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(&attributes, []).unwrap();
        GitRunner {
            root: path.to_path_buf(),
            empty_hooks: hooks,
            empty_attributes: attributes,
            attribute_source: None,
        }
    }

    fn exact_delta(label: &str, entries: Vec<PublicationDeltaEntry>) -> PublicationDelta {
        PublicationDelta {
            root: sha256(label.as_bytes()),
            entries,
        }
    }

    fn exact_write(path: &Path, relative: &str, bytes: &[u8]) -> PublicationDeltaEntry {
        PublicationDeltaEntry {
            path: relative.to_string(),
            preimage_sha256: Some(sha256(&fs::read(path.join(relative)).unwrap())),
            postimage: Some(bytes.to_vec()),
            executable: false,
        }
    }

    fn exact_delete(path: &Path, relative: &str) -> PublicationDeltaEntry {
        PublicationDeltaEntry {
            path: relative.to_string(),
            preimage_sha256: Some(sha256(&fs::read(path.join(relative)).unwrap())),
            postimage: None,
            executable: false,
        }
    }

    fn active_operations(path: &Path) -> Vec<String> {
        let directory = publication_journal_dir(path).unwrap();
        let Ok(entries) = fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut operations = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
            .filter(|operation| valid_operation_id(operation))
            .collect::<Vec<_>>();
        operations.sort();
        operations
    }

    fn active_operation(path: &Path) -> String {
        let operations = active_operations(path);
        assert_eq!(operations.len(), 1, "expected exactly one active operation");
        operations.into_iter().next().unwrap()
    }

    fn publication_journal_path(path: &Path, operation: &str) -> PathBuf {
        crate::operation_journal::path(&publication_journal_dir(path).unwrap(), operation)
    }

    struct InterruptedPostRef {
        temporary: tempfile::TempDir,
        operation: String,
        expected: String,
        candidate: String,
        postimage: Vec<u8>,
    }

    fn interrupted_post_ref(label: &str) -> InterruptedPostRef {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let expected = sh(path, &["rev-parse", "refs/heads/main"]);
        let postimage = format!("[\n  {{\"actor_id\":\"agent:{label}\"}}\n]\n").into_bytes();
        let delta = exact_delta(
            label,
            vec![exact_write(path, ".vela/actors.json", &postimage)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), &postimage).unwrap();
        let index_lock = path.join(".git/index.lock");
        fs::write(&index_lock, "held by fixture\n").unwrap();
        let interrupted = publish_exact_delta(
            path,
            "post-ref recovery refusal fixture",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        fs::remove_file(index_lock).unwrap();
        let candidate = match &interrupted.state {
            PublicationState::CommittedLocal { commit } => commit.clone(),
            state => panic!("fixture must retain the moved ref, got {state:?}"),
        };
        let operation = operation_from(&interrupted);
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), candidate);
        InterruptedPostRef {
            temporary,
            operation,
            expected,
            candidate,
            postimage,
        }
    }

    #[test]
    fn publication_journal_root_is_git_private_with_no_public_fallback() {
        let temporary = frontier();
        let path = temporary.path();
        let resolved = publication_journal_dir(path).unwrap();
        assert_eq!(
            resolved,
            path.canonicalize()
                .unwrap()
                .join(".git/vela/operation-journals")
        );

        let non_git = tempfile::tempdir().unwrap();
        assert!(publication_journal_dir(non_git.path()).is_err());
        assert!(!non_git.path().join(".vela").exists());
    }

    #[test]
    fn publication_path_conversion_prefixes_nested_frontier_and_rejects_unsafe_inputs() {
        let repository = tempfile::tempdir().unwrap();
        sh(repository.path(), &["init", "-q", "-b", "main"]);
        let frontier = repository.path().join("projects/nested-frontier");
        fs::create_dir_all(frontier.join("sources")).unwrap();
        assert_eq!(
            publication_repo_relative_path(&frontier, "sources/input.json").unwrap(),
            "projects/nested-frontier/sources/input.json"
        );
        for unsafe_path in [
            "",
            "/absolute",
            "../parent",
            "sources/../escape",
            ".git/config",
            "sources\\windows",
            ":(top)",
        ] {
            assert!(
                publication_repo_relative_path(&frontier, unsafe_path).is_err(),
                "accepted unsafe frontier-relative path {unsafe_path:?}"
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            use std::os::unix::fs::symlink;
            let outside = tempfile::tempdir().unwrap();
            symlink(outside.path(), frontier.join("linked")).unwrap();
            assert!(publication_repo_relative_path(&frontier, "linked/file").is_err());

            let non_utf8 = repository
                .path()
                .join(std::ffi::OsString::from_vec(vec![b'f', 0xff]));
            assert!(publication_repo_relative_path(&non_utf8, "state.json").is_err());
        }
    }

    #[test]
    fn identical_exact_write_is_typed_unchanged_and_preserves_caller_staging() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let unchanged = fs::read(path.join(".vela/actors.json")).unwrap();
        let delta = exact_delta(
            "identical-write",
            vec![exact_write(path, ".vela/actors.json", &unchanged)],
        );
        let opts = PublishOptions::new(true);
        let head_before = sh(path, &["rev-parse", "HEAD"]);
        let status_before = sh(path, &["--no-optional-locks", "status", "--porcelain=v1"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let objects_before = sh(path, &["count-objects", "-v"]);

        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        let outcome =
            publish_exact_delta(path, "identical exact write", &[], &delta, preflight, &opts)
                .unwrap();

        match &outcome.state {
            PublicationState::Unchanged { commit } => assert_eq!(commit, &head_before),
            state => panic!("identical write must be typed unchanged, got {state:?}"),
        }
        assert_eq!(
            serde_json::to_value(&outcome).unwrap()["state"],
            "unchanged"
        );
        assert!(outcome.recovery_command.is_some());
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), head_before);
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
        assert_eq!(
            sh(path, &["--no-optional-locks", "status", "--porcelain=v1"]),
            status_before
        );
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), unchanged);
        assert_eq!(
            fs::read(path.join("unrelated.txt")).unwrap(),
            b"caller staged\n"
        );
    }

    #[test]
    fn identical_exact_write_can_push_without_a_local_git_delta() {
        let temporary = frontier();
        let path = temporary.path();
        let remote = tempfile::tempdir().unwrap();
        sh(remote.path(), &["init", "-q", "--bare"]);
        sh(
            path,
            &["remote", "add", "upstream", remote.path().to_str().unwrap()],
        );
        sh(
            path,
            &[
                "push",
                "-q",
                "-u",
                "upstream",
                "refs/heads/main:refs/heads/main",
            ],
        );

        let unchanged = fs::read(path.join(".vela/actors.json")).unwrap();
        let delta = exact_delta(
            "identical-write-push",
            vec![exact_write(path, ".vela/actors.json", &unchanged)],
        );
        let opts = PublishOptions::pushing();
        let head_before = sh(path, &["rev-parse", "HEAD"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let objects_before = sh(path, &["count-objects", "-v"]);

        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        let outcome = publish_exact_delta(
            path,
            "push identical exact write",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();

        match outcome.state {
            PublicationState::Pushed { commit, remote } => {
                assert_eq!(commit, head_before);
                assert_eq!(remote, "upstream");
            }
            state => panic!("explicit push of unchanged bytes must be verified, got {state:?}"),
        }
        assert!(outcome.recovery_command.is_none());
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), head_before);
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            head_before
        );
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), unchanged);
    }

    #[test]
    fn exact_publication_commits_only_supplied_writes_and_deletions() {
        let temporary = frontier();
        let path = temporary.path();
        let written = b"[\n  {\"actor_id\":\"agent:exact\"}\n]\n";
        let delta = exact_delta(
            "write-delete",
            vec![
                exact_write(path, ".vela/actors.json", written),
                exact_delete(path, "frontier.json"),
            ],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        fs::remove_file(path.join("frontier.json")).unwrap();

        let outcome = publish_exact_delta(
            path,
            "exact write and deletion",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(
            sh(
                path,
                &["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"]
            ),
            "M\t.vela/actors.json\nD\tfrontier.json"
        );
        assert_eq!(
            sh(path, &["show", "HEAD:.vela/actors.json"]),
            String::from_utf8_lossy(written).trim()
        );
        assert!(!path.join("frontier.json").exists());
    }

    #[test]
    fn exact_publication_preserves_legacy_tracked_artifact_blobs() {
        let temporary = frontier();
        let path = temporary.path();
        let legacy = ".vela/artifact-blobs/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        fs::create_dir_all(path.join(".vela/artifact-blobs/sha256")).unwrap();
        fs::write(path.join(legacy), b"legacy public evidence\n").unwrap();
        sh(path, &["add", "-f", "--", legacy]);
        sh(path, &["commit", "-q", "-m", "legacy artifact layout"]);

        let written = b"[\n  {\"actor_id\":\"agent:legacy-compatible\"}\n]\n";
        let delta = exact_delta(
            "legacy-tracked-artifact",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();

        let outcome = publish_exact_delta(
            path,
            "preserve legacy artifact layout",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(
            sh(
                path,
                &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
            ),
            ".vela/actors.json"
        );
        assert_eq!(
            sh(path, &["show", &format!("HEAD:{legacy}")]),
            "legacy public evidence"
        );
    }

    #[test]
    fn caller_index_drift_after_exact_preflight_is_zero_delta() {
        let temporary = frontier();
        let path = temporary.path();
        let postimage = b"[\n  {\"actor_id\":\"agent:index-drift\"}\n]\n";
        let delta = exact_delta(
            "caller-index-drift-after-preflight",
            vec![exact_write(path, ".vela/actors.json", postimage)],
        );
        let opts = PublishOptions::new(true);
        let expected = sh(path, &["rev-parse", "refs/heads/main"]);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();

        fs::write(path.join("unrelated.txt"), "staged after preflight\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        fs::write(path.join(".vela/actors.json"), postimage).unwrap();
        let status_after_drift = sh(path, &["--no-optional-locks", "status", "--porcelain=v1"]);
        let index_after_drift = fs::read(path.join(".git/index")).unwrap();
        let objects_after_drift = sh(path, &["count-objects", "-v"]);

        let outcome = publish_exact_delta(
            path,
            "caller index drift after exact preflight",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();

        match &outcome.state {
            PublicationState::Uncommitted { candidate, reason } => {
                assert!(candidate.is_none());
                assert!(
                    reason.contains("exact publication preflight refuses caller-index drift"),
                    "{reason}"
                );
            }
            state => panic!("expected caller-index-drift refusal, got {state:?}"),
        }
        assert!(outcome.recovery_command.is_none());
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), expected);
        assert_eq!(
            fs::read(path.join(".git/index")).unwrap(),
            index_after_drift
        );
        assert_eq!(sh(path, &["count-objects", "-v"]), objects_after_drift);
        assert_eq!(
            sh(path, &["--no-optional-locks", "status", "--porcelain=v1"]),
            status_after_drift
        );
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), postimage);
        assert!(active_operations(path).is_empty());
    }

    #[test]
    fn detached_head_requires_an_explicit_publication_target() {
        let temporary = frontier();
        let path = temporary.path();
        let head = sh(path, &["rev-parse", "HEAD"]);
        sh(path, &["checkout", "-q", "--detach", &head]);
        let delta = exact_delta(
            "detached-head",
            vec![exact_write(path, ".vela/actors.json", b"[\n]\n")],
        );
        let objects_before = sh(path, &["count-objects", "-v"]);

        let outcome = exact_publication_preflight(path, &delta, &PublishOptions::new(true))
            .expect_err("detached HEAD without an explicit branch must fail before publication");

        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => assert!(
                reason.contains("detached HEAD requires an explicit local branch ref"),
                "{reason}"
            ),
            state => panic!("expected detached-HEAD refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), head);
        assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
    }

    #[test]
    fn unchecked_target_publication_leaves_the_entire_caller_index_untouched() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["branch", "publication-target"]);
        let main_before = sh(path, &["rev-parse", "refs/heads/main"]);
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let written = b"[\n  {\"actor_id\":\"agent:unchecked\"}\n]\n";
        let delta = exact_delta(
            "unchecked-target",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let mut opts = PublishOptions::new(true);
        opts.target_refname = Some("refs/heads/publication-target".to_string());
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();

        let outcome = publish_exact_delta(
            path,
            "publish to unchecked target",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();

        let target_commit = match outcome.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected unchecked-target commit, got {state:?}"),
        };
        assert_eq!(sh(path, &["symbolic-ref", "HEAD"]), "refs/heads/main");
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), main_before);
        assert_eq!(
            sh(path, &["rev-parse", "refs/heads/publication-target"]),
            target_commit
        );
        assert_eq!(
            sh(
                path,
                &["show", "refs/heads/publication-target:.vela/actors.json"]
            ),
            String::from_utf8_lossy(written).trim()
        );
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
            ),
            "unrelated.txt"
        );
    }

    #[test]
    fn target_checked_out_in_another_worktree_is_rejected_before_construction() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["branch", "linked-target"]);
        let linked_parent = tempfile::tempdir().unwrap();
        let linked = linked_parent.path().join("linked-target");
        sh(
            path,
            &[
                "worktree",
                "add",
                "-q",
                linked.to_str().unwrap(),
                "linked-target",
            ],
        );
        let target_before = sh(path, &["rev-parse", "refs/heads/linked-target"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let objects_before = sh(path, &["count-objects", "-v"]);
        let delta = exact_delta(
            "linked-worktree",
            vec![exact_write(path, ".vela/actors.json", b"[\n]\n")],
        );
        let mut opts = PublishOptions::new(true);
        opts.target_refname = Some("refs/heads/linked-target".to_string());

        let outcome = exact_publication_preflight(path, &delta, &opts)
            .expect_err("a target checked out elsewhere must fail before object construction");

        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(
                    reason.contains("checked out in another worktree"),
                    "{reason}"
                );
                assert!(reason.contains(linked.to_str().unwrap()), "{reason}");
            }
            state => panic!("expected linked-worktree refusal, got {state:?}"),
        }
        assert_eq!(
            sh(path, &["rev-parse", "refs/heads/linked-target"]),
            target_before
        );
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
        sh(
            path,
            &["worktree", "remove", "--force", linked.to_str().unwrap()],
        );
    }

    #[test]
    fn pre_ref_journal_interruptions_recover_exactly_once() {
        for step in [
            PublicationTestStep::InterruptAfterPreparedJournal,
            PublicationTestStep::InterruptAfterCommitTree,
            PublicationTestStep::InterruptAfterCandidateJournal,
        ] {
            let temporary = frontier();
            let path = temporary.path();
            fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
            sh(path, &["add", "--", "unrelated.txt"]);
            let expected = sh(path, &["rev-parse", "refs/heads/main"]);
            let postimage = format!("[\n  {{\"actor_id\":\"agent:{step:?}\"}}\n]\n").into_bytes();
            let delta = exact_delta(
                &format!("pre-ref-journal-{step:?}"),
                vec![exact_write(path, ".vela/actors.json", &postimage)],
            );
            let opts = PublishOptions::new(true).at_test_step(step);
            let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
            fs::write(path.join(".vela/actors.json"), &postimage).unwrap();
            let index_before = fs::read(path.join(".git/index")).unwrap();

            let interrupted = publish_exact_delta(
                path,
                "pre-ref publication interruption",
                &[],
                &delta,
                preflight,
                &opts,
            )
            .unwrap();
            match &interrupted.state {
                PublicationState::Uncommitted { candidate, reason } => {
                    assert!(reason.contains("injected interruption"), "{reason}");
                    assert_eq!(
                        candidate.is_some(),
                        step != PublicationTestStep::InterruptAfterPreparedJournal,
                        "candidate visibility at {step:?}"
                    );
                }
                state => panic!("expected injected interruption at {step:?}, got {state:?}"),
            }
            let operation = operation_from(&interrupted);
            let journal: PublicationJournal =
                crate::operation_journal::read_json(&publication_journal_path(path, &operation))
                    .unwrap();
            assert_eq!(
                journal.candidate_commit_oid.is_some(),
                step == PublicationTestStep::InterruptAfterCandidateJournal,
                "durable candidate journal state at {step:?}"
            );
            assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), expected);
            assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);

            let recovered = recover_publication(path, &operation, &PublishOptions::new(true));
            let commit = match &recovered.state {
                PublicationState::CommittedLocal { commit } => commit.clone(),
                state => panic!("expected local recovery at {step:?}, got {state:?}"),
            };
            assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), commit);
            assert_eq!(sh(path, &["rev-parse", "HEAD^"]), expected);
            assert!(
                sh(
                    path,
                    &["diff", "--cached", "--name-only", "--", ".vela/actors.json"]
                )
                .is_empty(),
                "recovery at {step:?} must reconcile the published Vela entry"
            );
            assert_eq!(
                sh(
                    path,
                    &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
                ),
                "unrelated.txt"
            );
            let commit_count = sh(path, &["rev-list", "--count", "refs/heads/main"]);
            let repeated = recover_publication(path, &operation, &PublishOptions::new(true));
            assert_eq!(repeated.state, recovered.state);
            assert_eq!(
                sh(path, &["rev-list", "--count", "refs/heads/main"]),
                commit_count
            );
            assert!(active_operations(path).is_empty());
        }
    }

    #[test]
    fn index_lock_after_ref_move_retains_commit_and_recovers_exactly_once() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let written = b"[\n  {\"actor_id\":\"agent:recover-index\"}\n]\n";
        let delta = exact_delta(
            "post-ref-index-lock",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        let index_lock = path.join(".git/index.lock");
        fs::write(&index_lock, "held by fixture\n").unwrap();

        let interrupted = publish_exact_delta(
            path,
            "post-ref index-lock recovery",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        let commit = match &interrupted.state {
            PublicationState::CommittedLocal { commit } => commit.clone(),
            state => panic!("ref movement must be reported as retained, got {state:?}"),
        };
        let operation = operation_from(&interrupted);
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), commit);
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);

        fs::remove_file(index_lock).unwrap();
        let recovered = recover_publication(path, &operation, &opts);
        assert_eq!(
            recovered.state,
            PublicationState::CommittedLocal {
                commit: commit.clone()
            }
        );
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), commit);
        assert!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", ".vela/actors.json"]
            )
            .is_empty(),
            "recovery must reconcile only the published Vela index entry"
        );
        assert_eq!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
            ),
            "unrelated.txt"
        );
        let commit_count = sh(path, &["rev-list", "--count", "refs/heads/main"]);
        let repeated = recover_publication(path, &operation, &opts);
        assert_eq!(repeated.state, recovered.state);
        assert_eq!(
            sh(path, &["rev-list", "--count", "refs/heads/main"]),
            commit_count
        );
    }

    #[test]
    fn crash_after_ref_cas_before_marker_recovers_exactly_once() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let postimage = b"[\n  {\"actor_id\":\"agent:ref-cas-crash\"}\n]\n";
        let delta = exact_delta(
            "after-ref-cas-before-marker",
            vec![exact_write(path, ".vela/actors.json", postimage)],
        );
        let opts = PublishOptions::new(true)
            .at_test_step(PublicationTestStep::InterruptAfterRefCasBeforeMarker);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), postimage).unwrap();

        let interrupted = publish_exact_delta(
            path,
            "crash after ref CAS before marker",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        let commit = match &interrupted.state {
            PublicationState::CommittedLocal { commit } => commit.clone(),
            state => panic!("moved ref must be retained, got {state:?}"),
        };
        let operation = operation_from(&interrupted);
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), commit);
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);

        let recover_opts = PublishOptions::new(true);
        let recovered = recover_publication(path, &operation, &recover_opts);
        assert_eq!(
            recovered.state,
            PublicationState::CommittedLocal {
                commit: commit.clone()
            }
        );
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), commit);
        assert!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", ".vela/actors.json"]
            )
            .is_empty()
        );
        assert_eq!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
            ),
            "unrelated.txt"
        );
        let commit_count = sh(path, &["rev-list", "--count", "refs/heads/main"]);
        let repeated = recover_publication(path, &operation, &recover_opts);
        assert_eq!(repeated.state, recovered.state);
        assert_eq!(
            sh(path, &["rev-list", "--count", "refs/heads/main"]),
            commit_count
        );
    }

    #[test]
    fn post_ref_recovery_preserves_unrelated_staging_and_refuses_unsafe_drift() {
        #[derive(Clone, Copy, Debug)]
        enum Drift {
            Checkout,
            VelaWorktree,
            UnrelatedIndex,
            VelaIndex,
            RefRollback,
            UnrelatedRefAdvance,
        }

        for drift in [
            Drift::Checkout,
            Drift::VelaWorktree,
            Drift::UnrelatedIndex,
            Drift::VelaIndex,
            Drift::RefRollback,
            Drift::UnrelatedRefAdvance,
        ] {
            let label = format!("recovery-drift-{drift:?}").to_lowercase();
            let fixture = interrupted_post_ref(&label);
            let path = fixture.temporary.path();

            match drift {
                Drift::Checkout => {
                    sh(path, &["branch", "other", &fixture.candidate]);
                    sh(path, &["symbolic-ref", "HEAD", "refs/heads/other"]);
                }
                Drift::VelaWorktree => {
                    fs::write(path.join(".vela/actors.json"), "caller worktree drift\n").unwrap();
                }
                Drift::UnrelatedIndex => {
                    fs::write(path.join("after-ref.txt"), "new caller staging\n").unwrap();
                    sh(path, &["add", "--", "after-ref.txt"]);
                }
                Drift::VelaIndex => {
                    fs::write(path.join(".vela/actors.json"), "caller index drift\n").unwrap();
                    sh(path, &["add", "--", ".vela/actors.json"]);
                    fs::write(path.join(".vela/actors.json"), &fixture.postimage).unwrap();
                }
                Drift::RefRollback => {
                    sh(
                        path,
                        &[
                            "update-ref",
                            "refs/heads/main",
                            &fixture.expected,
                            &fixture.candidate,
                        ],
                    );
                }
                Drift::UnrelatedRefAdvance => {
                    let treeish = format!("{}^{{tree}}", fixture.expected);
                    let tree = sh(path, &["rev-parse", &treeish]);
                    let sibling = sh(
                        path,
                        &[
                            "commit-tree",
                            &tree,
                            "-p",
                            &fixture.expected,
                            "-m",
                            "unrelated ref advance",
                        ],
                    );
                    sh(
                        path,
                        &[
                            "update-ref",
                            "refs/heads/main",
                            &sibling,
                            &fixture.candidate,
                        ],
                    );
                }
            }

            let journal_path = publication_journal_path(path, &fixture.operation);
            let completed_path = journal_path
                .parent()
                .unwrap()
                .join("completed")
                .join(journal_path.file_name().unwrap());
            let ref_before = sh(path, &["rev-parse", "refs/heads/main"]);
            let head_before = sh(path, &["symbolic-ref", "-q", "HEAD"]);
            let unrelated_index_before = sh(
                path,
                &[
                    "ls-files",
                    "--stage",
                    "--",
                    "unrelated.txt",
                    "after-ref.txt",
                ],
            );
            let index_before = fs::read(path.join(".git/index")).unwrap();
            let actors_before = fs::read(path.join(".vela/actors.json")).unwrap();
            let unrelated_before = fs::read(path.join("unrelated.txt")).unwrap();
            let after_ref_before = fs::read(path.join("after-ref.txt")).ok();
            let journal_before = fs::read(&journal_path).unwrap();
            let objects_before = sh(path, &["count-objects", "-v"]);
            assert!(!completed_path.exists());

            let outcome = recover_publication(path, &fixture.operation, &PublishOptions::new(true));
            match (drift, &outcome.state) {
                (Drift::Checkout, PublicationState::Unknown { reason }) => {
                    assert!(reason.contains("checkout identity drift"), "{reason}");
                }
                (Drift::VelaWorktree, PublicationState::Unknown { reason }) => {
                    assert!(reason.contains("Vela worktree drift"), "{reason}");
                }
                (Drift::UnrelatedIndex, PublicationState::CommittedLocal { commit }) => {
                    assert_eq!(commit, &fixture.candidate);
                }
                (Drift::VelaIndex, PublicationState::Unknown { reason }) => {
                    assert!(reason.contains("Vela index drift"), "{reason}");
                }
                (Drift::RefRollback, PublicationState::Unknown { reason }) => {
                    assert!(reason.contains("ref rolled back"), "{reason}");
                }
                (
                    Drift::UnrelatedRefAdvance,
                    PublicationState::Stale {
                        candidate,
                        expected,
                        actual,
                    },
                ) => {
                    assert_eq!(candidate, &fixture.candidate);
                    assert_eq!(expected, &fixture.expected);
                    assert_eq!(actual, &ref_before);
                }
                (_, state) => panic!("unexpected {drift:?} recovery state: {state:?}"),
            }

            assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), ref_before);
            assert_eq!(sh(path, &["symbolic-ref", "-q", "HEAD"]), head_before);
            assert_eq!(
                fs::read(path.join(".vela/actors.json")).unwrap(),
                actors_before
            );
            assert_eq!(
                fs::read(path.join("unrelated.txt")).unwrap(),
                unrelated_before
            );
            assert_eq!(fs::read(path.join("after-ref.txt")).ok(), after_ref_before);
            assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
            if matches!(drift, Drift::UnrelatedIndex) {
                assert_eq!(
                    sh(
                        path,
                        &[
                            "ls-files",
                            "--stage",
                            "--",
                            "unrelated.txt",
                            "after-ref.txt",
                        ]
                    ),
                    unrelated_index_before
                );
                assert!(
                    sh(
                        path,
                        &["diff", "--cached", "--name-only", "--", ".vela/actors.json"]
                    )
                    .is_empty()
                );
                assert_eq!(
                    sh(
                        path,
                        &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
                    ),
                    "unrelated.txt"
                );
                assert_eq!(
                    sh(
                        path,
                        &["diff", "--cached", "--name-only", "--", "after-ref.txt"]
                    ),
                    "after-ref.txt"
                );
                assert!(!journal_path.exists());
                assert!(completed_path.exists());
                let repeated =
                    recover_publication(path, &fixture.operation, &PublishOptions::new(true));
                assert_eq!(repeated.state, outcome.state);
            } else {
                assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
                assert_eq!(fs::read(&journal_path).unwrap(), journal_before);
                assert!(!completed_path.exists());
            }
        }
    }

    #[test]
    fn exact_publication_resume_after_scientific_commit_publishes_bound_delta() {
        let temporary = frontier();
        let path = temporary.path();
        let written = b"[\n  {\"actor_id\":\"agent:resumed\"}\n]\n";
        let delta = exact_delta(
            "scientific-commit-crash",
            vec![
                exact_write(path, ".vela/actors.json", written),
                exact_delete(path, "frontier.json"),
            ],
        );
        let opts = PublishOptions::new(true);

        // Simulate a process dying after FrontierTxn reached Completed but
        // before it obtained or used a Git publication lease.
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        fs::remove_file(path.join("frontier.json")).unwrap();
        let ordinary = exact_publication_preflight(path, &delta, &opts)
            .expect_err("ordinary preflight still requires worktree preimages");
        assert!(matches!(
            ordinary.state,
            PublicationState::Uncommitted { .. }
        ));

        let preflight = exact_publication_resume_preflight(path, &delta, &opts).unwrap();
        let outcome = publish_exact_delta(
            path,
            "resume completed scientific transaction",
            &[],
            &delta,
            preflight,
            &opts,
        )
        .unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(
            sh(
                path,
                &["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"]
            ),
            "M\t.vela/actors.json\nD\tfrontier.json"
        );
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), written);
        assert!(!path.join("frontier.json").exists());
    }

    #[test]
    fn exact_publication_resume_recognizes_already_published_target_idempotently() {
        let temporary = frontier();
        let path = temporary.path();
        let written = b"[\n  {\"actor_id\":\"agent:idempotent\"}\n]\n";
        let delta = exact_delta(
            "already-published",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let initial = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        let first =
            publish_exact_delta(path, "first publication", &[], &delta, initial, &opts).unwrap();
        let published = match first.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected first local commit, got {state:?}"),
        };
        let commit_count = sh(path, &["rev-list", "--count", "HEAD"]);

        let resumed = exact_publication_resume_preflight(path, &delta, &opts).unwrap();
        let second =
            publish_exact_delta(path, "retry after publication", &[], &delta, resumed, &opts)
                .unwrap();
        match second.state {
            PublicationState::Unchanged { commit } => assert_eq!(commit, published),
            state => panic!("expected idempotent local outcome, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), published);
        assert_eq!(sh(path, &["rev-list", "--count", "HEAD"]), commit_count);
    }

    #[test]
    fn exact_publication_resume_resolves_lfs_parent_to_raw_preimage() {
        if !Command::new("git")
            .args(["lfs", "version"])
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return;
        }
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["lfs", "install", "--local"]);
        fs::write(
            path.join(".gitattributes"),
            "witnesses/** filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        fs::create_dir_all(path.join("witnesses")).unwrap();
        let before = b"raw LFS preimage bytes\n";
        fs::write(path.join("witnesses/resume.bin"), before).unwrap();
        sh(
            path,
            &["add", "--", ".gitattributes", "witnesses/resume.bin"],
        );
        sh(path, &["commit", "-q", "-m", "LFS parent"]);
        assert!(
            sh(path, &["show", "HEAD:witnesses/resume.bin"])
                .starts_with("version https://git-lfs.github.com/spec/v1\n")
        );

        let after = b"raw LFS postimage bytes\n";
        let delta = exact_delta(
            "lfs-parent-resume",
            vec![exact_write(path, "witnesses/resume.bin", after)],
        );
        fs::write(path.join("witnesses/resume.bin"), after).unwrap();
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_resume_preflight(path, &delta, &opts).unwrap();
        let outcome =
            publish_exact_delta(path, "resume LFS parent", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let pointer = sh(path, &["show", "HEAD:witnesses/resume.bin"]);
        assert!(pointer.contains(&format!(
            "oid sha256:{}",
            hex::encode(Sha256::digest(after))
        )));
        assert_eq!(fs::read(path.join("witnesses/resume.bin")).unwrap(), after);
    }

    #[test]
    fn exact_publication_discovery_preserves_a_lineage_after_b_and_in_clean_clone() {
        fn landing_delta(
            path: &Path,
            label: &str,
            operation_id: &str,
            receipt_bytes: &[u8],
            actor_bytes: &[u8],
        ) -> (PublicationDelta, String, String) {
            let receipt_root = sha256(receipt_bytes);
            let receipt_path = format!(
                "records/receipts/sha256/{}.json",
                receipt_root.strip_prefix("sha256:").unwrap()
            );
            let proposal_path = format!(".vela/proposals/vsp_{label}.json");
            let proposal = serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "vela.proposal.v1",
                "id": format!("vsp_{label}"),
                "kind": "finding.add",
                "status": "pending_review",
                "payload": {
                    "vela_submission": {
                        "schema": "vela.submission-links.internal.v1",
                        "receipt_root": receipt_root,
                        "receipt_path": receipt_path,
                        "record_id": format!("vrc_{}", &"0".repeat(16)),
                        "operation_id": operation_id,
                    }
                },
                "source_refs": [receipt_path],
            }))
            .unwrap();
            let mut entries = vec![
                exact_write(path, ".vela/actors.json", actor_bytes),
                PublicationDeltaEntry {
                    path: proposal_path,
                    preimage_sha256: None,
                    postimage: Some(proposal),
                    executable: false,
                },
                PublicationDeltaEntry {
                    path: receipt_path.clone(),
                    preimage_sha256: None,
                    postimage: Some(receipt_bytes.to_vec()),
                    executable: false,
                },
            ];
            entries.sort_by(|left, right| left.path.cmp(&right.path));
            (
                exact_delta(&format!("landing-{label}"), entries),
                receipt_root,
                receipt_path,
            )
        }

        fn install_delta(path: &Path, delta: &PublicationDelta) {
            for entry in &delta.entries {
                let absolute = path.join(&entry.path);
                match &entry.postimage {
                    Some(bytes) => {
                        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
                        fs::write(absolute, bytes).unwrap();
                    }
                    None => fs::remove_file(absolute).unwrap(),
                }
            }
        }

        let temporary = frontier();
        let path = temporary.path();
        let operation_a = format!("vop_{}", "a".repeat(64));
        let receipt_a = b"{\"claim\":\"publication A\",\"schema\":\"vela.receipt.v1\"}\n";
        let (delta_a, root_a, anchor_a) = landing_delta(
            path,
            "a",
            &operation_a,
            receipt_a,
            b"[\n  {\"actor_id\":\"agent:a\"}\n]\n",
        );
        let opts = PublishOptions::new(true);
        let preflight_a = exact_publication_preflight(path, &delta_a, &opts).unwrap();
        install_delta(path, &delta_a);
        let published_a =
            publish_exact_delta(path, "publish A", &[], &delta_a, preflight_a, &opts).unwrap();
        let commit_a = match published_a.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected publication A commit, got {state:?}"),
        };

        let operation_b = format!("vop_{}", "b".repeat(64));
        let receipt_b = b"{\"claim\":\"publication B\",\"schema\":\"vela.receipt.v1\"}\n";
        let (delta_b, _root_b, _anchor_b) = landing_delta(
            path,
            "b",
            &operation_b,
            receipt_b,
            b"[\n  {\"actor_id\":\"agent:b\"}\n]\n",
        );
        let preflight_b = exact_publication_preflight(path, &delta_b, &opts).unwrap();
        install_delta(path, &delta_b);
        let published_b =
            publish_exact_delta(path, "publish B", &[], &delta_b, preflight_b, &opts).unwrap();
        let commit_b = match published_b.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected publication B commit, got {state:?}"),
        };
        assert_ne!(commit_a, commit_b);

        let found_with_delta =
            discover_exact_publication(path, &delta_a, &anchor_a, &opts).unwrap();
        assert!(matches!(
            found_with_delta.map(|outcome| outcome.state),
            Some(PublicationState::CommittedLocal { commit }) if commit == commit_a
        ));
        let found_from_receipt =
            discover_receipt_publication(path, receipt_a, &root_a, &operation_a, &opts).unwrap();
        assert!(matches!(
            found_from_receipt.map(|outcome| outcome.state),
            Some(PublicationState::CommittedLocal { commit }) if commit == commit_a
        ));
        let wrong_operation = format!("vop_{}", "c".repeat(64));
        assert!(
            discover_receipt_publication(path, receipt_a, &root_a, &wrong_operation, &opts)
                .unwrap()
                .is_none()
        );

        let clone_parent = tempfile::tempdir().unwrap();
        let clone = clone_parent.path().join("clean-clone");
        sh(
            clone_parent.path(),
            &[
                "clone",
                "-q",
                "--local",
                path.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        assert!(
            !clone.join(".git/vela/operation-journals").exists(),
            "clean-clone discovery must not depend on private journals"
        );
        let clone_opts = PublishOptions::new(true);
        let clean_found =
            discover_receipt_publication(&clone, receipt_a, &root_a, &operation_a, &clone_opts)
                .unwrap();
        assert!(matches!(
            clean_found.map(|outcome| outcome.state),
            Some(PublicationState::Pushed { commit, remote })
                if commit == commit_a && remote == "origin"
        ));
        let clean_delta_found =
            discover_exact_publication(&clone, &delta_a, &anchor_a, &clone_opts).unwrap();
        assert!(matches!(
            clean_delta_found.map(|outcome| outcome.state),
            Some(PublicationState::Pushed { commit, remote })
                if commit == commit_a && remote == "origin"
        ));
        assert_eq!(sh(&clone, &["rev-parse", "HEAD"]), commit_b);
    }

    #[test]
    fn exact_publication_reports_delta_mutation_as_typed_error() {
        let temporary = frontier();
        let path = temporary.path();
        let mut delta = exact_delta(
            "mutation",
            vec![exact_write(path, ".vela/actors.json", b"[\n]\n")],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        delta.entries[0].postimage = Some(b"mutated after preflight\n".to_vec());

        let error = publish_exact_delta(path, "mutated", &[], &delta, preflight, &opts)
            .expect_err("delta mutation must be typed");
        assert!(matches!(error, ExactPublicationError::DeltaChanged { .. }));
    }

    #[test]
    fn exact_publication_rejects_non_normal_duplicate_and_case_colliding_paths() {
        fn new_file(path: &str) -> PublicationDeltaEntry {
            PublicationDeltaEntry {
                path: path.to_string(),
                preimage_sha256: None,
                postimage: Some(b"new\n".to_vec()),
                executable: false,
            }
        }

        let temporary = frontier();
        let path = temporary.path();
        let invalid = vec![
            exact_delta("absolute", vec![new_file("/.vela/actors.json")]),
            exact_delta("parent", vec![new_file("sources/../escape")]),
            exact_delta("git-private", vec![new_file(".git/config")]),
            exact_delta(
                "duplicate",
                vec![new_file("sources/same"), new_file("sources/same")],
            ),
            exact_delta(
                "case-collision",
                vec![new_file("sources/Case"), new_file("sources/case")],
            ),
            exact_delta(
                "unsorted",
                vec![new_file("sources/z"), new_file("sources/a")],
            ),
        ];
        for delta in invalid {
            let before = sh(path, &["count-objects", "-v"]);
            let outcome = exact_publication_preflight(path, &delta, &PublishOptions::new(true))
                .expect_err("unsafe exact delta must fail before object construction");
            assert!(matches!(
                outcome.state,
                PublicationState::Uncommitted { .. }
            ));
            assert_eq!(sh(path, &["count-objects", "-v"]), before);
        }
    }

    #[cfg(unix)]
    #[test]
    fn exact_publication_never_executes_hostile_worktree_filter() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = frontier();
        let path = temporary.path();
        // Force one unchanged tracked entry into Git's racily-clean window.
        // Reconciliation copies this stat data into its alternate index, so
        // writing that index must verify the worktree bytes. Without a pinned
        // attribute source, that implicit check would execute the hostile
        // worktree filter below.
        File::open(path.join(".vela/config.toml"))
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(86_400))
            .unwrap();
        sh(
            path,
            &["update-index", "--refresh", "--", ".vela/config.toml"],
        );
        let marker = path.join("EXACT_FILTER_EXECUTED");
        let script = path.join("exact-evil-filter.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\n: > '{}'\ncat\n",
                marker.display().to_string().replace('\'', "'\"'\"'")
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        sh(
            path,
            &[
                "config",
                "filter.exact-evil.clean",
                script.to_str().unwrap(),
            ],
        );
        sh(path, &["config", "filter.exact-evil.required", "true"]);
        fs::write(path.join(".gitattributes"), ".vela/** filter=exact-evil\n").unwrap();

        let written = b"[\n]\n";
        let delta = exact_delta(
            "hostile-filter",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        assert!(!marker.exists(), "exact preflight executed hostile filter");
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        let outcome =
            publish_exact_delta(path, "hostile filter", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert!(
            !marker.exists(),
            "exact publication executed hostile filter"
        );
    }

    #[test]
    fn exact_publication_rejects_unrelated_public_dirt_after_preflight() {
        let temporary = frontier();
        let path = temporary.path();
        let written = b"[\n]\n";
        let delta = exact_delta(
            "unexpected-dirt",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        fs::write(path.join(".vela/artifacts.json"), "tracked caller dirt\n").unwrap();
        fs::create_dir_all(path.join("sources")).unwrap();
        fs::write(path.join("sources/unexpected.json"), "{}\n").unwrap();
        let before = sh(path, &["rev-parse", "HEAD"]);

        let outcome =
            publish_exact_delta(path, "unexpected dirt", &[], &delta, preflight, &opts).unwrap();
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(
                    reason.contains("unrelated public frontier dirt"),
                    "{reason}"
                );
                assert!(reason.contains(".vela/artifacts.json"), "{reason}");
                assert!(reason.contains("sources/unexpected.json"), "{reason}");
            }
            state => panic!("expected unrelated-dirt refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
        assert_eq!(
            fs::read_to_string(path.join("sources/unexpected.json")).unwrap(),
            "{}\n"
        );
        assert_eq!(
            fs::read_to_string(path.join(".vela/artifacts.json")).unwrap(),
            "tracked caller dirt\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn exact_publication_never_sweeps_ignored_public_roots_for_desired_state() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join("records")).unwrap();
        let ignored = path.join("records/unreadable-private-input");
        fs::write(&ignored, "must not be read or published\n").unwrap();
        let mut permissions = fs::metadata(&ignored).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&ignored, permissions).unwrap();

        let written = b"[\n]\n";
        let delta = exact_delta(
            "no-sweep",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        let outcome = publish_exact_delta(path, "no sweep", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let changed = sh(
            path,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
        );
        assert_eq!(changed, ".vela/actors.json");
        assert!(sh(path, &["ls-tree", "-r", "--name-only", "HEAD", "records"]).is_empty());

        let mut permissions = fs::metadata(&ignored).unwrap().permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&ignored, permissions).unwrap();
    }

    #[test]
    fn exact_publication_ref_race_preserves_caller_index_and_worktree() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let written = b"[\n]\n";
        let delta = exact_delta(
            "ref-race",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::new(true)
            .at_test_step(PublicationTestStep::AdvanceRefBeforeFinalObservation);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();

        let outcome =
            publish_exact_delta(path, "exact race", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(outcome.state, PublicationState::Stale { .. }));
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), written);
        assert_eq!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
            ),
            "unrelated.txt"
        );
    }

    #[test]
    fn actual_cas_loss_after_final_ref_observation_is_stale_and_non_destructive() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let expected = sh(path, &["rev-parse", "refs/heads/main"]);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        let postimage = b"[\n  {\"actor_id\":\"agent:actual-cas-loss\"}\n]\n";
        let delta = exact_delta(
            "actual-cas-loss",
            vec![exact_write(path, ".vela/actors.json", postimage)],
        );
        let opts = PublishOptions::new(true)
            .at_test_step(PublicationTestStep::AdvanceRefAfterFinalObservation);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), postimage).unwrap();

        let outcome =
            publish_exact_delta(path, "actual CAS loss", &[], &delta, preflight, &opts).unwrap();
        let actual = match &outcome.state {
            PublicationState::Stale {
                candidate,
                expected: observed_expected,
                actual,
            } => {
                assert_eq!(observed_expected, &expected);
                assert_ne!(candidate, actual);
                actual.clone()
            }
            state => panic!("expected a real compare-and-swap loss, got {state:?}"),
        };
        assert!(outcome.recovery_command.is_none());
        assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), actual);
        assert_eq!(sh(path, &["rev-parse", "HEAD^"]), expected);
        assert_eq!(
            sh(path, &["show", "--format=%s", "--no-patch", "HEAD"]),
            "competing publication"
        );
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(fs::read(path.join(".vela/actors.json")).unwrap(), postimage);
        assert_eq!(
            sh(
                path,
                &["diff", "--cached", "--name-only", "--", "unrelated.txt"]
            ),
            "unrelated.txt"
        );
        assert!(active_operations(path).is_empty());
    }

    #[test]
    fn pushed_completion_failures_recover_without_new_commit_or_ref_move() {
        for step in [
            PublicationTestStep::FailCompletedRecordWrite,
            PublicationTestStep::FailActiveJournalRemove,
            PublicationTestStep::FailCompletedRecordPrune,
        ] {
            let temporary = frontier();
            let path = temporary.path();
            let remote = tempfile::tempdir().unwrap();
            sh(remote.path(), &["init", "-q", "--bare"]);
            sh(remote.path(), &["config", "core.logAllRefUpdates", "true"]);
            sh(
                path,
                &["remote", "add", "upstream", remote.path().to_str().unwrap()],
            );
            sh(
                path,
                &[
                    "push",
                    "-q",
                    "-u",
                    "upstream",
                    "refs/heads/main:refs/heads/main",
                ],
            );
            fs::write(path.join("unrelated.txt"), "caller staged\n").unwrap();
            sh(path, &["add", "--", "unrelated.txt"]);
            let expected = sh(path, &["rev-parse", "refs/heads/main"]);
            let postimage = format!("[\n  {{\"actor_id\":\"agent:{step:?}\"}}\n]\n").into_bytes();
            let delta = exact_delta(
                &format!("pushed-completion-{step:?}"),
                vec![exact_write(path, ".vela/actors.json", &postimage)],
            );
            let opts = PublishOptions::pushing().at_test_step(step);
            let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
            fs::write(path.join(".vela/actors.json"), &postimage).unwrap();

            let interrupted = publish_exact_delta(
                path,
                "pushed completion failure",
                &[],
                &delta,
                preflight,
                &opts,
            )
            .unwrap();
            let candidate = match &interrupted.state {
                PublicationState::CommittedLocal { commit } => commit.clone(),
                state => panic!("expected resumable completion failure at {step:?}, got {state:?}"),
            };
            let operation = operation_from(&interrupted);
            let journal_path = publication_journal_path(path, &operation);
            let completed_path = journal_path
                .parent()
                .unwrap()
                .join("completed")
                .join(journal_path.file_name().unwrap());
            match step {
                PublicationTestStep::FailCompletedRecordWrite => {
                    assert!(journal_path.is_file());
                    assert!(!completed_path.exists());
                }
                PublicationTestStep::FailActiveJournalRemove => {
                    assert!(journal_path.is_file());
                    assert!(completed_path.is_file());
                }
                PublicationTestStep::FailCompletedRecordPrune => {
                    assert!(!journal_path.exists());
                    assert!(completed_path.is_file());
                }
                _ => unreachable!(),
            }
            assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), candidate);
            assert_eq!(sh(path, &["rev-parse", "HEAD^"]), expected);
            assert_eq!(
                sh(remote.path(), &["rev-parse", "refs/heads/main"]),
                candidate,
                "the remote push must complete before {step:?}"
            );
            let local_commits = all_commit_oids(path);
            let remote_commits = all_commit_oids(remote.path());
            let local_reflog = sh(path, &["reflog", "show", "--format=%H", "refs/heads/main"]);
            let remote_reflog = sh(
                remote.path(),
                &["reflog", "show", "--format=%H", "refs/heads/main"],
            );
            let index_after_failure = fs::read(path.join(".git/index")).unwrap();

            let recovered = recover_publication(path, &operation, &PublishOptions::pushing());
            assert_eq!(
                recovered.state,
                PublicationState::Pushed {
                    commit: candidate.clone(),
                    remote: "upstream".to_string(),
                }
            );
            assert!(recovered.recovery_command.is_none());
            assert_eq!(sh(path, &["rev-parse", "refs/heads/main"]), candidate);
            assert_eq!(
                sh(remote.path(), &["rev-parse", "refs/heads/main"]),
                candidate
            );
            assert_eq!(all_commit_oids(path), local_commits);
            assert_eq!(all_commit_oids(remote.path()), remote_commits);
            assert_eq!(
                sh(path, &["reflog", "show", "--format=%H", "refs/heads/main"]),
                local_reflog
            );
            assert_eq!(
                sh(
                    remote.path(),
                    &["reflog", "show", "--format=%H", "refs/heads/main"]
                ),
                remote_reflog
            );
            assert_eq!(
                fs::read(path.join(".git/index")).unwrap(),
                index_after_failure
            );
            assert!(active_operations(path).is_empty());
            assert!(completed_path.is_file());

            let repeated = recover_publication(path, &operation, &PublishOptions::pushing());
            assert_eq!(repeated, recovered);
            assert_eq!(all_commit_oids(path), local_commits);
            assert_eq!(all_commit_oids(remote.path()), remote_commits);
        }
    }

    #[test]
    fn exact_publication_push_failure_retains_recovery_journal() {
        let temporary = frontier();
        let path = temporary.path();
        let remote = tempfile::tempdir().unwrap();
        sh(remote.path(), &["init", "-q", "--bare"]);
        sh(
            path,
            &["remote", "add", "upstream", remote.path().to_str().unwrap()],
        );
        sh(
            path,
            &[
                "push",
                "-q",
                "-u",
                "upstream",
                "refs/heads/main:refs/heads/main",
            ],
        );
        sh(
            path,
            &[
                "remote",
                "set-url",
                "upstream",
                "/definitely/missing/vela.git",
            ],
        );

        let written = b"[\n]\n";
        let delta = exact_delta(
            "push-failure",
            vec![exact_write(path, ".vela/actors.json", written)],
        );
        let opts = PublishOptions::pushing();
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::write(path.join(".vela/actors.json"), written).unwrap();
        let outcome =
            publish_exact_delta(path, "push failure", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. } | PublicationState::Unknown { .. }
        ));
        let operation = active_operation(path);
        assert_eq!(
            outcome.recovery_command.as_deref(),
            Some(format!("vela publication recover --operation {operation} --push").as_str())
        );
        sh(
            path,
            &[
                "remote",
                "set-url",
                "upstream",
                remote.path().to_str().unwrap(),
            ],
        );
        let recovered = recover_publication(path, &operation, &PublishOptions::pushing());
        assert!(matches!(recovered.state, PublicationState::Pushed { .. }));
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            sh(path, &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn earlier_local_publication_recovers_after_a_later_descendant_without_local_rewrites() {
        let temporary = frontier();
        let path = temporary.path();
        let remote = tempfile::tempdir().unwrap();
        sh(remote.path(), &["init", "-q", "--bare"]);
        sh(
            path,
            &["remote", "add", "upstream", remote.path().to_str().unwrap()],
        );
        sh(
            path,
            &[
                "push",
                "-q",
                "-u",
                "upstream",
                "refs/heads/main:refs/heads/main",
            ],
        );

        let actors_a = b"[\n  {\"id\": \"agent:a\"}\n]\n";
        let delta_a = exact_delta(
            "queued-a",
            vec![exact_write(path, ".vela/actors.json", actors_a)],
        );
        let local_opts = PublishOptions::new(true);
        let preflight_a = exact_publication_preflight(path, &delta_a, &local_opts).unwrap();
        fs::write(path.join(".vela/actors.json"), actors_a).unwrap();
        let published_a =
            publish_exact_delta(path, "queued A", &[], &delta_a, preflight_a, &local_opts).unwrap();
        let operation_a = operation_from(&published_a);
        let commit_a = match published_a.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected local publication A, got {state:?}"),
        };

        let actors_b = b"[\n  {\"id\": \"agent:a\"},\n  {\"id\": \"agent:b\"}\n]\n";
        let delta_b = exact_delta(
            "queued-b",
            vec![exact_write(path, ".vela/actors.json", actors_b)],
        );
        let preflight_b = exact_publication_preflight(path, &delta_b, &local_opts).unwrap();
        fs::write(path.join(".vela/actors.json"), actors_b).unwrap();
        let published_b =
            publish_exact_delta(path, "queued B", &[], &delta_b, preflight_b, &local_opts).unwrap();
        let commit_b = match published_b.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected local publication B, got {state:?}"),
        };
        assert_ne!(commit_a, commit_b);

        fs::write(path.join("unrelated.txt"), "caller staged after B\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let local_ref_before = sh(path, &["rev-parse", "refs/heads/main"]);
        let actors_before = fs::read(path.join(".vela/actors.json")).unwrap();
        let index_before = fs::read(path.join(".git/index")).unwrap();

        let recovered = recover_publication(path, &operation_a, &PublishOptions::pushing());
        assert_eq!(
            recovered.state,
            PublicationState::Pushed {
                commit: commit_a.clone(),
                remote: "upstream".to_string(),
            }
        );
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            commit_a
        );
        assert_eq!(
            sh(path, &["rev-parse", "refs/heads/main"]),
            local_ref_before
        );
        assert_eq!(
            fs::read(path.join(".vela/actors.json")).unwrap(),
            actors_before
        );
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
    }

    #[test]
    fn earlier_local_publication_recovery_accepts_a_remote_descendant_without_local_rewrites() {
        let temporary = frontier();
        let path = temporary.path();
        let remote = tempfile::tempdir().unwrap();
        sh(remote.path(), &["init", "-q", "--bare"]);
        sh(
            path,
            &["remote", "add", "upstream", remote.path().to_str().unwrap()],
        );
        sh(
            path,
            &[
                "push",
                "-q",
                "-u",
                "upstream",
                "refs/heads/main:refs/heads/main",
            ],
        );

        let actors_a = b"[\n  {\"id\": \"agent:a\"}\n]\n";
        let delta_a = exact_delta(
            "queued-a",
            vec![exact_write(path, ".vela/actors.json", actors_a)],
        );
        let local_opts = PublishOptions::new(true);
        let preflight_a = exact_publication_preflight(path, &delta_a, &local_opts).unwrap();
        fs::write(path.join(".vela/actors.json"), actors_a).unwrap();
        let published_a =
            publish_exact_delta(path, "queued A", &[], &delta_a, preflight_a, &local_opts).unwrap();
        let operation_a = operation_from(&published_a);
        let commit_a = match published_a.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected local publication A, got {state:?}"),
        };

        let actors_b = b"[\n  {\"id\": \"agent:a\"},\n  {\"id\": \"agent:b\"}\n]\n";
        let delta_b = exact_delta(
            "queued-b",
            vec![exact_write(path, ".vela/actors.json", actors_b)],
        );
        let preflight_b = exact_publication_preflight(path, &delta_b, &local_opts).unwrap();
        fs::write(path.join(".vela/actors.json"), actors_b).unwrap();
        let published_b =
            publish_exact_delta(path, "queued B", &[], &delta_b, preflight_b, &local_opts).unwrap();
        let commit_b = match published_b.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("expected local publication B, got {state:?}"),
        };
        let descendant_refspec = format!("{commit_b}:refs/heads/main");
        sh(
            path,
            &["push", "-q", "upstream", descendant_refspec.as_str()],
        );

        fs::write(path.join("unrelated.txt"), "caller staged after B\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let local_ref_before = sh(path, &["rev-parse", "refs/heads/main"]);
        let remote_ref_before = sh(remote.path(), &["rev-parse", "refs/heads/main"]);
        let actors_before = fs::read(path.join(".vela/actors.json")).unwrap();
        let index_before = fs::read(path.join(".git/index")).unwrap();

        let recovered = recover_publication(path, &operation_a, &PublishOptions::pushing());
        assert_eq!(
            recovered.state,
            PublicationState::Pushed {
                commit: commit_a.clone(),
                remote: "upstream".to_string(),
            }
        );
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            remote_ref_before
        );
        assert_eq!(remote_ref_before, commit_b);
        assert_eq!(
            sh(path, &["rev-parse", "refs/heads/main"]),
            local_ref_before
        );
        assert_eq!(
            fs::read(path.join(".vela/actors.json")).unwrap(),
            actors_before
        );
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);

        let recovered_again = recover_publication(path, &operation_a, &PublishOptions::pushing());
        assert_eq!(recovered_again, recovered);
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
    }

    #[test]
    fn exact_publication_preserves_verified_lfs_transport() {
        if !Command::new("git")
            .args(["lfs", "version"])
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return;
        }
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["lfs", "install", "--local"]);
        fs::write(
            path.join(".gitattributes"),
            "witnesses/** filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        sh(path, &["add", "--", ".gitattributes"]);
        sh(path, &["commit", "-q", "-m", "exact LFS attributes"]);

        let raw = b"exact public witness bytes\n";
        let delta = exact_delta(
            "exact-lfs",
            vec![PublicationDeltaEntry {
                path: "witnesses/exact.bin".to_string(),
                preimage_sha256: None,
                postimage: Some(raw.to_vec()),
                executable: false,
            }],
        );
        let opts = PublishOptions::new(true);
        let preflight = exact_publication_preflight(path, &delta, &opts).unwrap();
        fs::create_dir_all(path.join("witnesses")).unwrap();
        fs::write(path.join("witnesses/exact.bin"), raw).unwrap();
        let outcome =
            publish_exact_delta(path, "exact LFS", &[], &delta, preflight, &opts).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let pointer = sh(path, &["show", "HEAD:witnesses/exact.bin"]);
        assert!(pointer.starts_with("version https://git-lfs.github.com/spec/v1\n"));
        assert!(pointer.contains(&format!("oid sha256:{}", hex::encode(Sha256::digest(raw)))));
        assert_eq!(fs::read(path.join("witnesses/exact.bin")).unwrap(), raw);
    }

    #[test]
    fn git_runner_forces_literal_pathspecs() {
        let temporary = frontier();
        let path = temporary.path();
        let runner_temp = tempfile::tempdir().unwrap();
        let runner = test_runner(path, &runner_temp);
        for hostile in ["*", ":(top)"] {
            let paths = git_paths(
                &runner,
                vec![
                    OsString::from("ls-files"),
                    OsString::from("-z"),
                    OsString::from("--"),
                ],
                &[hostile.to_string()],
            )
            .unwrap();
            assert!(paths.is_empty(), "{hostile} broadened to {paths:?}");
        }
    }

    #[test]
    fn recovery_operation_id_requires_exact_lowercase_sha256_shape() {
        let temporary = frontier();
        let path = temporary.path();
        for invalid in [
            "vop_",
            "vop_a",
            "vop_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "vop_gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let outcome = recover_publication(path, invalid, &PublishOptions::new(true));
            match outcome.state {
                PublicationState::Unknown { reason } => {
                    assert!(
                        reason.contains("invalid publication operation id"),
                        "{reason}"
                    )
                }
                state => panic!("expected invalid-id refusal, got {state:?}"),
            }
        }
    }

    #[test]
    fn recovery_push_command_shell_quotes_untrusted_names() {
        let oid = GitOid::parse("sha1", &"0".repeat(40)).unwrap();
        let txn = GitPublicationTxn {
            target_refname: GitRefName("refs/heads/main".to_string()),
            target_checkout: TargetCheckoutState::UncheckedOut,
            expected_git_commit_oid: oid.clone(),
            candidate_tree_oid: oid.clone(),
            candidate_commit_oid: Some(oid),
            lfs_objects: Vec::new(),
        };
        let upstream = UpstreamTarget {
            remote: "origin'; touch /tmp/pwn; echo '".to_string(),
            reference: "refs/heads/main;$(touch${IFS}/tmp/pwn)".to_string(),
        };
        let command = push_command_for(&txn, Some(&upstream));
        assert_eq!(
            command,
            "git push -- 'origin'\"'\"'; touch /tmp/pwn; echo '\"'\"'' '0000000000000000000000000000000000000000:refs/heads/main;$(touch${IFS}/tmp/pwn)'"
        );
    }
}
