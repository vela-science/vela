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
    specs: Vec<String>,
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

#[derive(Debug)]
pub(crate) struct PublicationPreflight {
    repository: PathBuf,
    frontier: PathBuf,
    target_refname: GitRefName,
    target_checkout: TargetCheckoutState,
    expected_git_commit_oid: GitOid,
    original_index: IndexMap,
    original_index_sha256: String,
    allowed_input_hashes: BTreeMap<String, String>,
    publication_lock: PublicationLock,
}

struct GitRunner {
    root: PathBuf,
    empty_hooks: PathBuf,
    empty_attributes: PathBuf,
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
}

pub(crate) struct PublishOptions {
    pub no_commit: bool,
    pub no_push: bool,
    /// Push even when config resolves `publish.git_push` to "off" — the explicit
    /// `--push` flag. Ordinary calls leave this false, so the default
    /// (commit locally, do not push) holds and publishing stays deliberate.
    pub force_push: bool,
    /// An explicit local branch is required when HEAD is detached. Keeping the
    /// target in the options also makes un-checked-out publication testable
    /// without teaching the ordinary CLI a second default.
    pub target_refname: Option<String>,
    pub preflight: Option<PublicationPreflight>,
    pub preflight_inputs: Vec<PathBuf>,
    #[cfg(test)]
    fail_after_ref_move: bool,
    #[cfg(test)]
    fail_before_ref_move: bool,
    #[cfg(test)]
    race_ref_before_cas: bool,
    #[cfg(test)]
    pause_after_journal: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
}

impl PublishOptions {
    pub(crate) fn new(no_commit: bool, no_push: bool) -> Self {
        Self {
            no_commit,
            no_push,
            force_push: false,
            target_refname: None,
            preflight: None,
            preflight_inputs: Vec::new(),
            #[cfg(test)]
            fail_after_ref_move: false,
            #[cfg(test)]
            fail_before_ref_move: false,
            #[cfg(test)]
            race_ref_before_cas: false,
            #[cfg(test)]
            pause_after_journal: None,
        }
    }

    /// Explicit publish: commit locally and push regardless of config.
    pub(crate) fn pushing() -> Self {
        Self {
            no_commit: false,
            no_push: false,
            force_push: true,
            target_refname: None,
            preflight: None,
            preflight_inputs: Vec::new(),
            #[cfg(test)]
            fail_after_ref_move: false,
            #[cfg(test)]
            fail_before_ref_move: false,
            #[cfg(test)]
            race_ref_before_cas: false,
            #[cfg(test)]
            pause_after_journal: None,
        }
    }

    #[cfg(test)]
    fn targeting(mut self, target: &str) -> Self {
        self.target_refname = Some(target.to_string());
        self
    }

    pub(crate) fn with_preflight(mut self, preflight: PublicationPreflight) -> Self {
        self.preflight = Some(preflight);
        self
    }

    pub(crate) fn with_preflight_inputs(mut self, paths: Vec<PathBuf>) -> Self {
        self.preflight_inputs = paths;
        self
    }

    #[cfg(test)]
    fn failing_after_ref_move(mut self) -> Self {
        self.fail_after_ref_move = true;
        self
    }

    #[cfg(test)]
    fn failing_before_ref_move(mut self) -> Self {
        self.fail_before_ref_move = true;
        self
    }

    #[cfg(test)]
    fn racing_ref_before_cas(mut self) -> Self {
        self.race_ref_before_cas = true;
        self
    }

    #[cfg(test)]
    fn pausing_after_journal(
        mut self,
        reached: std::sync::mpsc::Sender<()>,
        resume: std::sync::mpsc::Receiver<()>,
    ) -> Self {
        self.pause_after_journal = Some((reached, resume));
        self
    }
}

/// Capture the clean Git/Vela boundary before a scientific mutation. The
/// returned token is private process plumbing; it is not serialized into a
/// receipt, event, or authority object.
pub(crate) fn publication_preflight(
    frontier: &Path,
    opts: &PublishOptions,
) -> Result<PublicationPreflight, PublicationOutcome> {
    publication_preflight_inner(frontier, opts).map_err(PublicationOutcome::uncommitted)
}

fn publication_preflight_inner(
    frontier: &Path,
    opts: &PublishOptions,
) -> Result<PublicationPreflight, String> {
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
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
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
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let expected = GitOid::parse(
        &object_format,
        &runner.text(&["rev-parse", &format!("{}^{{commit}}", target_refname.0)])?,
    )?;
    reject_unsupported_index(&runner, &specs, &object_format)?;
    let mut allowed_input_hashes = BTreeMap::new();
    for input in &opts.preflight_inputs {
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
        if absolute != lexical || !absolute.starts_with(&frontier_abs) {
            return Err(format!(
                "preflight input has symlink or containment ambiguity: {}",
                input.display()
            ));
        }
        let relative = absolute
            .strip_prefix(&root)
            .map_err(|_| "preflight input is outside the Git worktree".to_string())?
            .to_str()
            .ok_or_else(|| "preflight input path is not UTF-8".to_string())?
            .to_string();
        let bytes = fs::read(&absolute).map_err(|error| {
            format!("read explicit preflight input {}: {error}", input.display())
        })?;
        allowed_input_hashes.insert(relative, sha256(&bytes));
    }
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
    if let Some(path) = staged
        .iter()
        .find(|path| allowed_input_hashes.contains_key(*path))
    {
        return Err(format!(
            "explicit preflight input {path} is staged; unstage it so publication can prove the bound worktree bytes without consuming caller-owned index state"
        ));
    }
    let parent = tree_entries(&runner, &expected, &specs)?;
    let attribute_index = temporary.path().join("preflight.index");
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&expected.hex)],
        None,
        Some(&attribute_index),
    )?;
    let (desired, _) = desired_entries(
        &runner,
        &root,
        &specs,
        &parent,
        &object_format,
        &attribute_index,
    )?;
    let changed = desired
        .iter()
        .filter_map(|(path, entry)| {
            let matches = match entry {
                Some(entry) => parent
                    .get(path)
                    .is_some_and(|(mode, oid)| mode == &entry.mode && oid == &entry.oid),
                None => !parent.contains_key(path),
            };
            (!matches).then_some(path)
        })
        .collect::<BTreeSet<_>>();
    if let Some(path) = changed
        .iter()
        .find(|path| !allowed_input_hashes.contains_key(path.as_str()))
    {
        return Err(format!(
            "preflight refuses pre-existing unstaged Vela edit at {path}"
        ));
    }
    let (original_index, original_index_sha256) = capture_index(&runner)?;
    Ok(PublicationPreflight {
        repository: root,
        frontier: frontier_abs,
        target_refname,
        target_checkout,
        expected_git_commit_oid: expected,
        original_index,
        original_index_sha256,
        allowed_input_hashes,
        publication_lock,
    })
}

/// Publish a signed decision: materialize derived views, stage the
/// frontier's store paths, commit with a canonical message binding the
/// event ids, and push. Config: identity `git_commit` / `git_push`
/// ("auto" default, "off" opts out); `VELA_NO_PUBLISH=1` disables
/// globally (gates, tests); per-call flags override.
pub(crate) fn publish_decision(
    frontier: &Path,
    summary: &str,
    event_ids: &[String],
    opts: &PublishOptions,
) -> PublicationOutcome {
    // cfg!(test): the unit test below exercises the publish path itself
    // and must not be muted by the conformance gate's own guard.
    if !cfg!(test) && std::env::var("VELA_NO_PUBLISH").is_ok_and(|v| v == "1") {
        return PublicationOutcome::uncommitted("publication disabled by VELA_NO_PUBLISH");
    }
    // Settings resolution (env > frontier-narrowing > user config >
    // legacy identity field > default): a frontier may force "off",
    // never "auto" — a clone can stop publication, not start it.
    let (commit_mode, _) = crate::config::settings::resolve("publish.git_commit", Some(frontier));
    if opts.no_commit || commit_mode == "off" {
        return PublicationOutcome::uncommitted("Git commit publication is disabled");
    }

    publish_inner(frontier, summary, event_ids, opts)
        .unwrap_or_else(PublicationOutcome::uncommitted)
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
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
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
    let checkout = target_checkout_state(&runner, &journal.target_refname)?;
    if checkout != journal.target_checkout {
        return Err("publication recovery refuses checkout identity drift".to_string());
    }
    if !journal_worktree_matches(&root, &journal)? {
        return Err("publication recovery refuses Vela worktree drift".to_string());
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

    if matches!(checkout, TargetCheckoutState::Current { .. })
        && reconcile_journal_index(&runner, &journal).is_err()
    {
        return Ok(operation_recovery_outcome(&candidate, operation_id));
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
        operation_id,
        &candidate,
        &journal_path,
        &completed_dir,
        outcome,
    ))
}

fn publish_inner(
    frontier: &Path,
    summary: &str,
    event_ids: &[String],
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
    };
    let root = PathBuf::from(bootstrap.text(&["rev-parse", "--show-toplevel"])?)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let runner = GitRunner {
        root: root.clone(),
        empty_hooks,
        empty_attributes,
    };
    let owned_publication_lock = if opts.preflight.is_none() {
        match acquire_publication_lock(&runner) {
            Ok(lock) => Some(lock),
            Err(PublicationLockError::Busy) => {
                return Ok(publication_busy_outcome(&runner));
            }
            Err(PublicationLockError::Failed(reason)) => return Err(reason),
        }
    } else {
        None
    };
    let publication_lock = opts
        .preflight
        .as_ref()
        .map(|preflight| &preflight.publication_lock)
        .or(owned_publication_lock.as_ref())
        .expect("preflight or internally-owned publication lock");
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

    // Validate every manifest-derived path before materialization is allowed
    // to read or write it. The store must never be committed ahead of its
    // derived views: the vela-check Action holds committed views to
    // replayed-state hash parity, so store-without-views is red by design.
    vela_protocol::frontier_repo::materialize(frontier)
        .map_err(|error| format!("materialize before publication: {error}"))?;
    let object_format = runner.text(&["rev-parse", "--show-object-format"])?;
    let expected = GitOid::parse(
        &object_format,
        &runner.text(&["rev-parse", &format!("{}^{{commit}}", target_refname.0)])?,
    )?;

    reject_unsupported_index(&runner, &specs, &object_format)?;
    if let Some(preflight) = &opts.preflight {
        if preflight.repository != root
            || preflight.frontier != frontier_abs
            || preflight.target_refname != target_refname
            || preflight.target_checkout != target_checkout
            || preflight.expected_git_commit_oid != expected
        {
            return Err("publication preflight identity is stale".to_string());
        }
        let (current_index, current_index_sha256) = capture_index(&runner)?;
        if current_index != preflight.original_index
            || current_index_sha256 != preflight.original_index_sha256
        {
            return Err("publication preflight refuses caller-index drift".to_string());
        }
        for (path, expected_hash) in &preflight.allowed_input_hashes {
            let bytes = fs::read(root.join(path))
                .map_err(|error| format!("re-read explicit preflight input {path}: {error}"))?;
            if sha256(&bytes) != *expected_hash {
                return Err(format!(
                    "publication preflight refuses explicit-input drift at {path}"
                ));
            }
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
    let index_path = temporary.path().join("publication.index");
    runner.checked(
        &[OsString::from("read-tree"), OsString::from(&expected.hex)],
        None,
        Some(&index_path),
    )?;
    let (desired, worktree_hashes) =
        desired_entries(&runner, &root, &specs, &parent, &object_format, &index_path)?;
    if mapping_matches(&parent, &desired) {
        return Ok(PublicationOutcome::uncommitted(
            "resolved Vela paths already match the target tree",
        ));
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
    inspect_candidate(&runner, &expected, &tree, &desired, &specs)?;

    let message = publication_message(summary, event_ids);
    let planning_identity = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        root.display(),
        frontier_abs.display(),
        target_refname.0,
        expected.hex,
        tree.hex,
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
        specs: specs.clone(),
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
    if let Some((reached, resume)) = &opts.pause_after_journal
        && (reached.send(()).is_err() || resume.recv().is_err())
    {
        return Ok(uncommitted_operation_outcome(
            None,
            &operation_id,
            "publication test pause channel disappeared",
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
    journal.candidate_commit_oid = Some(candidate.clone());
    if let Err(error) = crate::operation_journal::write_json(&journal_path, &journal) {
        return Ok(uncommitted_operation_outcome(
            Some(&candidate),
            &operation_id,
            error,
        ));
    }

    #[cfg(test)]
    if opts.fail_before_ref_move {
        return Ok(PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: Some(candidate.hex),
                reason: "injected stop before ref movement".to_string(),
            },
            recovery_command: Some(format!(
                "vela publication recover --operation {operation_id}"
            )),
        });
    }

    let worktree_unchanged =
        match worktree_matches(&root, &specs, &worktree_hashes, &desired_modes(&desired)) {
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
    if opts.race_ref_before_cas {
        let competing = match runner.run(
            &[
                OsString::from("commit-tree"),
                OsString::from(&tree.hex),
                OsString::from("-p"),
                OsString::from(&expected.hex),
            ],
            Some(b"competing publication\n"),
            None,
            Some((&author_name, &author_email, &commit_date)),
        ) {
            Ok(output) => output,
            Err(error) => {
                return Ok(uncommitted_operation_outcome(
                    Some(&candidate),
                    &operation_id,
                    error,
                ));
            }
        };
        if !competing.status.success() {
            return Ok(uncommitted_operation_outcome(
                Some(&candidate),
                &operation_id,
                "could not construct injected competing commit",
            ));
        }
        let competing = String::from_utf8_lossy(&competing.stdout)
            .trim()
            .to_string();
        if let Err(error) = runner.checked(
            &[
                OsString::from("update-ref"),
                OsString::from(&target_refname.0),
                OsString::from(competing),
                OsString::from(&expected.hex),
            ],
            None,
            None,
        ) {
            return Ok(uncommitted_operation_outcome(
                Some(&candidate),
                &operation_id,
                error,
            ));
        }
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
    journal.ref_moved = true;
    if crate::operation_journal::write_json(&journal_path, &journal).is_err() {
        return Ok(operation_recovery_outcome(&candidate, &operation_id));
    }

    #[cfg(test)]
    if opts.fail_after_ref_move {
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
            &specs,
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
    ))
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

fn filesystem_public_paths(root: &Path, specs: &[String]) -> Result<BTreeSet<String>, String> {
    fn visit(
        root: &Path,
        path: &Path,
        specs: &[String],
        output: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "public path escaped the Git root".to_string())?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "non-UTF-8 public Vela path is not publishable".to_string())?
            .to_string();
        if is_private_vela_path(&relative, specs) {
            return Ok(());
        }
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("inspect public path {}: {error}", path.display())),
        };
        if metadata.file_type().is_dir() {
            let mut children = fs::read_dir(path)
                .map_err(|error| format!("read public directory {}: {error}", path.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                visit(root, &child, specs, output)?;
            }
        } else {
            output.insert(relative);
        }
        Ok(())
    }

    let mut output = BTreeSet::new();
    for spec in specs {
        reject_symlink_ancestors(root, Path::new(spec))?;
        visit(root, &root.join(spec), specs, &mut output)?;
    }
    Ok(output)
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
        if is_private_vela_path(path, specs) {
            return Err(format!(
                "expected tree contains tracked private Vela scratch path {path}; remove it explicitly before publication"
            ));
        }
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

fn desired_entries(
    runner: &GitRunner,
    root: &Path,
    specs: &[String],
    parent: &TreeMap,
    object_format: &str,
    attribute_index: &Path,
) -> Result<(DesiredMap, WorktreeHashMap), String> {
    let mut listed = git_paths(
        runner,
        vec![
            OsString::from("ls-files"),
            OsString::from("-z"),
            OsString::from("--cached"),
            OsString::from("--others"),
            OsString::from("--exclude-standard"),
            OsString::from("--"),
        ],
        specs,
    )?;
    listed.extend(filesystem_public_paths(root, specs)?);
    let paths = parent
        .keys()
        .cloned()
        .chain(listed)
        .collect::<BTreeSet<_>>();
    let mut desired = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    for path in paths {
        let absolute = root.join(&path);
        match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let worktree_bytes = fs::read(&absolute)
                    .map_err(|error| format!("read Vela path {path}: {error}"))?;
                let content_mode = effective_content_mode(
                    runner,
                    &path,
                    is_witness_path(&path, specs),
                    attribute_index,
                )?;
                let blob_bytes = match content_mode {
                    ContentMode::Raw => worktree_bytes.clone(),
                    ContentMode::Lfs => prepare_lfs_content(runner, &path, &worktree_bytes)?,
                };
                let oid = hash_object(runner, object_format, &blob_bytes, false)?;
                let mode = regular_file_mode(&metadata);
                hashes.insert(path.clone(), Some(sha256(&worktree_bytes)));
                desired.insert(
                    path,
                    Some(DesiredEntry {
                        mode,
                        oid,
                        bytes: blob_bytes,
                        content_mode,
                    }),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hashes.insert(path.clone(), None);
                desired.insert(path, None);
            }
            Ok(_) => return Err(format!("Vela path is not a regular file: {path}")),
            Err(error) => return Err(format!("inspect Vela path {path}: {error}")),
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
    Ok(())
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

fn inspect_candidate(
    runner: &GitRunner,
    expected: &GitOid,
    tree: &GitOid,
    desired: &BTreeMap<String, Option<DesiredEntry>>,
    specs: &[String],
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
        specs,
    )?;
    let allowlist = desired.keys().cloned().collect::<BTreeSet<_>>();
    if !changed.is_subset(&allowlist) {
        return Err("candidate tree changes a path outside the exact Vela allowlist".to_string());
    }
    let actual = tree_entries(runner, tree, specs)?;
    if !mapping_matches(&actual, desired) {
        return Err("candidate tree does not equal the resolved Vela mapping".to_string());
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
                    "candidate authority blob differs from canonical bytes at {path}"
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
    specs: &[String],
    original: &IndexMap,
    original_sha256: &str,
    worktree_hashes: &BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    let (current, current_sha256) = capture_index(runner)?;
    if current != *original || current_sha256 != original_sha256 {
        return Err("caller index drifted before post-ref reconciliation".to_string());
    }
    if !worktree_matches(
        &runner.root,
        specs,
        worktree_hashes,
        &desired_modes(desired),
    )? {
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
    if !worktree_matches(
        &runner.root,
        specs,
        worktree_hashes,
        &desired_modes(desired),
    )? {
        return Err("publication changed Vela worktree bytes".to_string());
    }
    Ok(())
}

fn reconcile_journal_index(runner: &GitRunner, journal: &PublicationJournal) -> Result<(), String> {
    let current = index_snapshot(runner)?;
    let desired_paths = journal
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    let unrelated_unchanged = journal.original_index.iter().all(|(path, value)| {
        desired_paths.contains(path.as_str()) || current.get(path) == Some(value)
    }) && current.iter().all(|(path, _)| {
        desired_paths.contains(path.as_str()) || journal.original_index.contains_key(path)
    });
    if !unrelated_unchanged {
        return Err("publication recovery refuses unrelated index drift".to_string());
    }
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
        let vela_original = journal
            .entries
            .iter()
            .all(|entry| current.get(&entry.path) == journal.original_index.get(&entry.path));
        if !vela_original {
            return Err("publication recovery refuses Vela index drift".to_string());
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
                _ => return Err(format!("incomplete journal entry for {}", entry.path)),
            }
        }
        atomically_reconcile_index(
            runner,
            &journal.original_index,
            &journal.original_index_sha256,
            &input,
        )?
    } else {
        current.clone()
    };
    for entry in &journal.entries {
        match (&entry.mode, &entry.oid) {
            (Some(mode), Some(oid))
                if after.get(&entry.path) == Some(&format!("{mode} {} 0|H", oid.hex)) => {}
            (None, None) if !after.contains_key(&entry.path) => {}
            _ => return Err(format!("journal recovery did not align {}", entry.path)),
        }
    }
    if !journal_worktree_matches(&runner.root, journal)? {
        return Err("publication recovery changed Vela worktree bytes".to_string());
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn worktree_matches(
    root: &Path,
    specs: &[String],
    expected: &BTreeMap<String, Option<String>>,
    modes: &BTreeMap<String, Option<String>>,
) -> Result<bool, String> {
    let actual_paths = filesystem_public_paths(root, specs)?;
    let expected_paths = expected
        .iter()
        .filter_map(|(path, hash)| hash.as_ref().map(|_| path.clone()))
        .collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        return Ok(false);
    }
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
    worktree_matches(root, &journal.specs, &hashes, &modes)
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

fn publication_busy_outcome(runner: &GitRunner) -> PublicationOutcome {
    let journal_dir = git_private_path(runner, "vela/operation-journals").ok();
    let operation = journal_dir
        .and_then(|dir| fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
        .find(|operation| valid_operation_id(operation));
    PublicationOutcome {
        state: PublicationState::Uncommitted {
            candidate: None,
            reason: PUBLICATION_BUSY_REASON.to_string(),
        },
        recovery_command: operation
            .map(|operation| format!("vela publication recover --operation {operation}")),
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
) -> PublicationOutcome {
    if !matches!(outcome.state, PublicationState::Pushed { .. }) {
        // A local-only or remotely indeterminate publication is resumable.
        // Keep the full transaction journal so `recover --push` can advance
        // it; an outcome-only tombstone would permanently strand the push.
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
    if crate::operation_journal::write_json(&completed_path, &completed).is_err()
        || crate::operation_journal::remove(journal_path).is_err()
        || crate::operation_journal::prune_json(completed_dir, 64).is_err()
    {
        operation_recovery_outcome(candidate, operation_id)
    } else {
        outcome
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn upstream_target(
    runner: &GitRunner,
    txn: &GitPublicationTxn,
) -> Result<Option<UpstreamTarget>, String> {
    let format = "%(upstream:remotename)%00%(upstream:remoteref)%00";
    let bytes = runner.checked(
        &[
            OsString::from("for-each-ref"),
            OsString::from(format!("--format={format}")),
            OsString::from("--"),
            OsString::from(&txn.target_refname.0),
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
    let refspec = format!("{}:{}", txn.target_refname.0, upstream.reference);
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
    let refspec = format!("{}:{}", txn.target_refname.0, upstream.reference);
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

    fn change_vela(path: &Path) {
        fs::write(path.join(".vela/actors.json"), "[\n]\n").unwrap();
    }

    fn operation_from(outcome: &PublicationOutcome) -> String {
        outcome
            .recovery_command
            .as_deref()
            .unwrap()
            .split_whitespace()
            .last()
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
        }
    }

    fn active_operation(path: &Path) -> String {
        let git_dir = PathBuf::from(sh(path, &["rev-parse", "--git-dir"]));
        let git_dir = if git_dir.is_absolute() {
            git_dir
        } else {
            path.join(git_dir)
        };
        fs::read_dir(git_dir.join("vela/operation-journals"))
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
            .find(|operation| valid_operation_id(operation))
            .unwrap()
    }

    #[test]
    fn publication_never_commits_callers_index() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "staged by caller\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        change_vela(path);

        let outcome = publish_decision(
            path,
            "accept: 1 proposal",
            &["vev_x".to_string()],
            &PublishOptions::new(false, true),
        );
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let body = sh(path, &["log", "-1", "--format=%B"]);
        assert!(body.contains("accept: 1 proposal"), "{body}");
        assert!(body.contains("vev_x"), "{body}");
        assert_eq!(
            sh(path, &["diff", "--cached", "--name-only"]),
            "unrelated.txt"
        );
        assert!(
            !sh(
                path,
                &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
            )
            .lines()
            .any(|path| path == "unrelated.txt")
        );
    }

    #[test]
    fn publication_noop_ignores_unrelated_staging() {
        let temporary = frontier();
        let path = temporary.path();
        let before = sh(path, &["rev-parse", "HEAD"]);
        fs::write(path.join("unrelated.txt"), "staged by caller\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let outcome = publish_decision(path, "noop", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
        assert_eq!(
            sh(path, &["diff", "--cached", "--name-only"]),
            "unrelated.txt"
        );
    }

    #[test]
    fn publication_includes_public_manifest_paths_but_not_private_work() {
        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join("records")).unwrap();
        fs::create_dir_all(path.join("sources")).unwrap();
        fs::create_dir_all(path.join("exports")).unwrap();
        fs::create_dir_all(path.join(".vela/work/session")).unwrap();
        fs::write(path.join("records/public.json"), "{}\n").unwrap();
        fs::write(path.join("sources/source.txt"), "source\n").unwrap();
        fs::write(path.join("exports/derived.txt"), "derived\n").unwrap();
        fs::write(path.join(".vela/work/session/private.txt"), "private\n").unwrap();
        let outcome =
            publish_decision(path, "public paths", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let changed = sh(
            path,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
        );
        assert!(
            changed.lines().any(|item| item == "records/public.json"),
            "{changed}"
        );
        assert!(
            changed.lines().any(|item| item == "sources/source.txt"),
            "{changed}"
        );
        assert!(!changed.contains(".vela/work/"), "{changed}");
        assert!(!changed.contains("exports/"), "{changed}");
        assert_eq!(
            fs::read_to_string(path.join(".vela/work/session/private.txt")).unwrap(),
            "private\n"
        );
    }

    #[test]
    fn publication_converts_witness_to_verified_lfs_pointer() {
        let has_lfs = Command::new("git")
            .args(["lfs", "version"])
            .output()
            .is_ok_and(|output| output.status.success());
        if !has_lfs {
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
        sh(path, &["commit", "-q", "-m", "configure lfs"]);
        fs::create_dir_all(path.join("witnesses")).unwrap();
        let raw = b"{\"kind\":\"computational\",\"payload\":\"large-enough-for-transport\"}\n";
        fs::write(path.join("witnesses/run.witness.json"), raw).unwrap();

        let outcome = publish_decision(path, "lfs", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        let pointer = sh(path, &["show", "HEAD:witnesses/run.witness.json"]);
        assert!(
            pointer.starts_with("version https://git-lfs.github.com/spec/v1\n"),
            "{pointer}"
        );
        assert!(pointer.contains(&format!("oid sha256:{}", hex::encode(Sha256::digest(raw)))));
        assert!(pointer.contains(&format!("size {}", raw.len())));
        assert_eq!(
            fs::read(path.join("witnesses/run.witness.json")).unwrap(),
            raw
        );
    }

    #[test]
    fn publication_reports_missing_lfs_object_as_availability_failure() {
        let has_lfs = Command::new("git")
            .args(["lfs", "version"])
            .output()
            .is_ok_and(|output| output.status.success());
        if !has_lfs {
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
        sh(path, &["commit", "-q", "-m", "configure lfs"]);
        fs::create_dir_all(path.join("witnesses")).unwrap();
        let missing = "f".repeat(64);
        fs::write(
            path.join("witnesses/missing.witness.json"),
            format!("version https://git-lfs.github.com/spec/v1\noid sha256:{missing}\nsize 42\n"),
        )
        .unwrap();
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome = publish_decision(path, "missing lfs", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("LFS availability failure"), "{reason}")
            }
            state => panic!("expected LFS availability failure, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
    }

    #[test]
    fn publication_refuses_overlapping_vela_staging() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        sh(path, &["add", "--", ".vela/actors.json"]);
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome = publish_decision(path, "overlap", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("staged Vela"), "{reason}")
            }
            state => panic!("expected staged-path refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
        assert_eq!(
            sh(path, &["diff", "--cached", "--name-only"]),
            ".vela/actors.json"
        );
    }

    #[test]
    fn publication_refuses_overlapping_vela_edits_in_preflight() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let outcome = publication_preflight(path, &PublishOptions::new(false, true)).unwrap_err();
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(
                    reason.contains("pre-existing unstaged Vela edit"),
                    "{reason}"
                )
            }
            state => panic!("expected dirty-preflight refusal, got {state:?}"),
        }
    }

    #[test]
    fn publication_accepts_clean_preflight_then_exact_mutation() {
        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join("witnesses")).unwrap();
        fs::write(path.join("witnesses/input.json"), "{}\n").unwrap();
        let base = PublishOptions::new(false, true)
            .with_preflight_inputs(vec![PathBuf::from("witnesses/input.json")]);
        let preflight = publication_preflight(path, &base).unwrap();
        change_vela(path);
        let outcome = publish_decision(path, "preflight", &[], &base.with_preflight(preflight));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
    }

    #[test]
    fn publication_preflight_rejects_parent_and_symlink_inputs() {
        let temporary = frontier();
        let path = temporary.path();
        let parent = PublishOptions::new(false, true)
            .with_preflight_inputs(vec![PathBuf::from("../outside")]);
        let outcome = publication_preflight(path, &parent).unwrap_err();
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::write(path.join("real-input"), "input\n").unwrap();
            symlink(path.join("real-input"), path.join("linked-input")).unwrap();
            let linked = PublishOptions::new(false, true)
                .with_preflight_inputs(vec![PathBuf::from("linked-input")]);
            let outcome = publication_preflight(path, &linked).unwrap_err();
            assert!(matches!(
                outcome.state,
                PublicationState::Uncommitted { .. }
            ));
        }
    }

    #[test]
    fn publication_refuses_non_normal_vela_index_flags() {
        let temporary = frontier();
        let path = temporary.path();
        sh(
            path,
            &["update-index", "--assume-unchanged", ".vela/actors.json"],
        );
        change_vela(path);
        let before = sh(path, &["count-objects", "-v"]);
        let outcome = publish_decision(path, "flags", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("index flag"), "{reason}")
            }
            state => panic!("expected index-flag refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["count-objects", "-v"]), before);
    }

    #[test]
    fn publication_refuses_unsupported_index_extensions() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["update-index", "--untracked-cache"]);
        fs::write(path.join("untracked-for-cache"), "x\n").unwrap();
        sh(path, &["status", "--porcelain"]);
        change_vela(path);
        let before = sh(path, &["count-objects", "-v"]);
        let outcome = publish_decision(path, "extension", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("index extension"), "{reason}")
            }
            state => panic!("expected extension refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["count-objects", "-v"]), before);
    }

    #[test]
    fn publication_preserves_unstaged_work_and_vela_deletion() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join("unrelated.txt"), "unstaged by caller\n").unwrap();
        fs::remove_file(path.join(".vela/actors.json")).unwrap();
        let outcome = publish_decision(path, "delete", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(
            fs::read_to_string(path.join("unrelated.txt")).unwrap(),
            "unstaged by caller\n"
        );
        assert_eq!(
            sh(path, &["status", "--short", "--", "unrelated.txt"]),
            "M unrelated.txt"
        );
        assert!(!path.join(".vela/actors.json").exists());
        assert_eq!(
            sh(
                path,
                &[
                    "diff-tree",
                    "--no-commit-id",
                    "--name-status",
                    "-r",
                    "HEAD",
                    "--",
                    ".vela/actors.json"
                ]
            ),
            "D\t.vela/actors.json"
        );
    }

    #[test]
    fn publication_bypasses_all_repository_hooks() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = frontier();
        let path = temporary.path();
        let marker = path.join("hook-ran");
        for hook in [
            "pre-commit",
            "post-commit",
            "post-index-change",
            "reference-transaction",
        ] {
            let hook_path = path.join(".git/hooks").join(hook);
            fs::write(
                &hook_path,
                format!("#!/bin/sh\ntouch '{}'\nexit 91\n", marker.display()),
            )
            .unwrap();
            let mut permissions = fs::metadata(&hook_path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&hook_path, permissions).unwrap();
        }
        change_vela(path);
        let outcome = publish_decision(path, "hooks", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert!(!marker.exists(), "repository hook was invoked");
    }

    #[test]
    fn publication_rejects_effective_worktree_filters() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(
            path.join(".git/info/attributes"),
            ".vela/** filter=hostile\n",
        )
        .unwrap();
        change_vela(path);
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome = publish_decision(path, "attrs", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("attribute"), "{reason}")
            }
            state => panic!("expected attribute refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
    }

    #[test]
    fn publication_detached_head_requires_explicit_ref() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["switch", "--detach", "-q"]);
        change_vela(path);
        let outcome = publish_decision(path, "detached", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));
    }

    #[test]
    fn publication_noncheckedout_ref_leaves_caller_index_untouched() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["branch", "publication-target"]);
        fs::write(path.join("unrelated.txt"), "staged by caller\n").unwrap();
        sh(path, &["add", "--", "unrelated.txt"]);
        let before_index = fs::read(path.join(".git/index")).unwrap();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "other branch",
            &[],
            &PublishOptions::new(false, true).targeting("refs/heads/publication-target"),
        );
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), before_index);
        assert_ne!(
            sh(path, &["rev-parse", "main"]),
            sh(path, &["rev-parse", "publication-target"])
        );
    }

    #[test]
    fn publication_ref_checked_out_elsewhere_fails_before_objects() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["branch", "elsewhere"]);
        let linked_parent = tempfile::tempdir().unwrap();
        let linked = linked_parent.path().join("linked");
        sh(
            path,
            &[
                "worktree",
                "add",
                "-q",
                linked.to_str().unwrap(),
                "elsewhere",
            ],
        );
        change_vela(path);
        let before = sh(path, &["count-objects", "-v"]);
        let outcome = publish_decision(
            path,
            "elsewhere",
            &[],
            &PublishOptions::new(false, true).targeting("refs/heads/elsewhere"),
        );
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));
        assert_eq!(sh(path, &["count-objects", "-v"]), before);
    }

    #[test]
    fn publication_git_ref_race_returns_stale_without_merge() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "race",
            &[],
            &PublishOptions::new(false, true).racing_ref_before_cas(),
        );
        let (candidate, actual) = match outcome.state {
            PublicationState::Stale {
                candidate, actual, ..
            } => (candidate, actual),
            state => panic!("expected stale publication, got {state:?}"),
        };
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), actual);
        assert_ne!(candidate, actual);
        assert_eq!(sh(path, &["rev-list", "--count", "HEAD^..HEAD"]), "1");
    }

    #[test]
    fn publication_crash_retry_reuses_candidate_commit() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "reuse",
            &[],
            &PublishOptions::new(false, true).failing_before_ref_move(),
        );
        let candidate = match outcome.state {
            PublicationState::Uncommitted {
                candidate: Some(candidate),
                ..
            } => candidate,
            state => panic!("expected candidate-ready outcome, got {state:?}"),
        };
        let recovered = publish_decision(path, "reuse", &[], &PublishOptions::new(false, true));
        assert_eq!(
            recovered.state,
            PublicationState::CommittedLocal {
                commit: candidate.clone()
            }
        );
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), candidate);
    }

    #[test]
    fn publication_post_ref_recovery_is_idempotent() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "recover",
            &[],
            &PublishOptions::new(false, true).failing_after_ref_move(),
        );
        let operation = operation_from(&outcome);
        assert!(!sh(path, &["status", "--porcelain", "--", ".vela"]).is_empty());

        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        assert!(matches!(
            recovered.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert!(sh(path, &["status", "--porcelain", "--", ".vela"]).is_empty());
        let second = recover_publication(path, &operation, &PublishOptions::new(false, true));
        assert_eq!(second, recovered);
    }

    #[test]
    fn publication_post_ref_recovery_refuses_vela_worktree_drift() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "drift",
            &[],
            &PublishOptions::new(false, true).failing_after_ref_move(),
        );
        let operation = operation_from(&outcome);
        fs::write(path.join(".vela/actors.json"), "[{}, {}]\n").unwrap();
        let before = sh(path, &["rev-parse", "HEAD"]);
        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        match recovered.state {
            PublicationState::Unknown { reason } => {
                assert!(reason.contains("worktree drift"), "{reason}")
            }
            state => panic!("expected drift refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
    }

    #[test]
    fn publication_post_ref_recovery_refuses_checkout_drift() {
        let temporary = frontier();
        let path = temporary.path();
        sh(path, &["branch", "other"]);
        change_vela(path);
        let outcome = publish_decision(
            path,
            "checkout drift",
            &[],
            &PublishOptions::new(false, true).failing_after_ref_move(),
        );
        let operation = operation_from(&outcome);
        sh(path, &["symbolic-ref", "HEAD", "refs/heads/other"]);
        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        match recovered.state {
            PublicationState::Unknown { reason } => {
                assert!(reason.contains("checkout identity drift"), "{reason}")
            }
            state => panic!("expected checkout refusal, got {state:?}"),
        }
    }

    #[test]
    fn publication_preflight_guard_excludes_a_second_writer() {
        let temporary = frontier();
        let path = temporary.path();
        let first = publication_preflight(path, &PublishOptions::new(false, true)).unwrap();
        let second = publication_preflight(path, &PublishOptions::new(false, true)).unwrap_err();
        match second.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("another Git publication"), "{reason}")
            }
            state => panic!("expected busy preflight, got {state:?}"),
        }
        change_vela(path);
        let outcome = publish_decision(
            path,
            "guarded mutation",
            &[],
            &PublishOptions::new(false, true).with_preflight(first),
        );
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
    }

    #[test]
    fn concurrent_identical_publisher_cannot_overwrite_recovery_journal() {
        use std::sync::mpsc;
        use std::time::Duration;

        let temporary = frontier();
        let path = temporary.path().to_path_buf();
        change_vela(&path);
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let first_path = path.clone();
        let first = std::thread::spawn(move || {
            publish_decision(
                &first_path,
                "identical",
                &[],
                &PublishOptions::new(false, true)
                    .pausing_after_journal(reached_tx, resume_rx)
                    .failing_after_ref_move(),
            )
        });
        reached_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let busy = publish_decision(&path, "identical", &[], &PublishOptions::new(false, true));
        assert!(matches!(busy.state, PublicationState::Uncommitted { .. }));
        resume_tx.send(()).unwrap();
        let stopped = first.join().unwrap();
        let operation = operation_from(&stopped);
        assert_eq!(operation_from(&busy), operation);
        let recovered = recover_publication(&path, &operation, &PublishOptions::new(false, true));
        assert!(matches!(
            recovered.state,
            PublicationState::CommittedLocal { .. }
        ));
    }

    #[test]
    fn publication_rechecks_checkout_identity_at_cas_boundary() {
        use std::sync::mpsc;
        use std::time::Duration;

        let temporary = frontier();
        let path = temporary.path().to_path_buf();
        sh(&path, &["branch", "other"]);
        change_vela(&path);
        let main_before = sh(&path, &["rev-parse", "refs/heads/main"]);
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let thread_path = path.clone();
        let publisher = std::thread::spawn(move || {
            publish_decision(
                &thread_path,
                "checkout race",
                &[],
                &PublishOptions::new(false, true).pausing_after_journal(reached_tx, resume_rx),
            )
        });
        reached_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        sh(&path, &["symbolic-ref", "HEAD", "refs/heads/other"]);
        resume_tx.send(()).unwrap();
        let outcome = publisher.join().unwrap();
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("checkout identity changed"), "{reason}")
            }
            state => panic!("expected checkout-race refusal, got {state:?}"),
        }
        assert_eq!(sh(&path, &["rev-parse", "refs/heads/main"]), main_before);
    }

    #[test]
    fn recovery_refuses_ref_rollback_after_recorded_movement() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let stopped = publish_decision(
            path,
            "rollback",
            &[],
            &PublishOptions::new(false, true).failing_after_ref_move(),
        );
        let operation = operation_from(&stopped);
        let candidate = sh(path, &["rev-parse", "HEAD"]);
        let expected = sh(path, &["rev-parse", "HEAD^"]);
        sh(
            path,
            &["update-ref", "refs/heads/main", &expected, &candidate],
        );
        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        match recovered.state {
            PublicationState::Unknown { reason } => {
                assert!(reason.contains("rolled back"), "{reason}")
            }
            state => panic!("expected rollback refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), expected);
    }

    #[test]
    fn publication_rejects_git_private_and_magic_manifest_paths_before_materialize() {
        for hostile in [".git/config", ":(top)", "*"] {
            let temporary = frontier();
            let path = temporary.path();
            let manifest = fs::read_to_string(path.join("frontier.yaml")).unwrap();
            let manifest = manifest.replace("state: frontier.json", &format!("state: '{hostile}'"));
            fs::write(path.join("frontier.yaml"), manifest).unwrap();
            let config_before = fs::read(path.join(".git/config")).unwrap();
            let objects_before = sh(path, &["count-objects", "-v"]);
            let outcome = publish_decision(
                path,
                "hostile manifest",
                &[],
                &PublishOptions::new(false, true),
            );
            match outcome.state {
                PublicationState::Uncommitted { reason, .. } => {
                    assert!(reason.contains("manifest path"), "{hostile}: {reason}")
                }
                state => panic!("expected manifest refusal for {hostile}, got {state:?}"),
            }
            assert_eq!(fs::read(path.join(".git/config")).unwrap(), config_before);
            assert_eq!(sh(path, &["count-objects", "-v"]), objects_before);
        }
    }

    #[cfg(unix)]
    #[test]
    fn publication_rejects_nested_manifest_symlink_ancestor() {
        use std::os::unix::fs::symlink;

        let temporary = frontier();
        let path = temporary.path();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(outside.path().join("sources")).unwrap();
        fs::write(outside.path().join("sources/secret"), "outside\n").unwrap();
        symlink(outside.path(), path.join("nested")).unwrap();
        let manifest = fs::read_to_string(path.join("frontier.yaml")).unwrap();
        let manifest = manifest.replace("sources: sources/", "sources: nested/sources/");
        fs::write(path.join("frontier.yaml"), manifest).unwrap();
        let outcome = publish_decision(
            path,
            "ancestor containment",
            &[],
            &PublishOptions::new(false, true),
        );
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("symlink ancestor"), "{reason}")
            }
            state => panic!("expected ancestor-symlink refusal, got {state:?}"),
        }
        assert_eq!(
            fs::read_to_string(outside.path().join("sources/secret")).unwrap(),
            "outside\n"
        );
    }

    #[test]
    fn publication_rejects_private_scratch_already_tracked_at_expected_tip() {
        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join(".vela/work")).unwrap();
        fs::write(path.join(".vela/work/secret"), "private\n").unwrap();
        sh(path, &["add", "-f", "--", ".vela/work/secret"]);
        sh(path, &["commit", "-q", "-m", "tracked private fixture"]);
        change_vela(path);
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome =
            publish_decision(path, "private path", &[], &PublishOptions::new(false, true));
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("tracked private Vela scratch"), "{reason}")
            }
            state => panic!("expected tracked-private refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
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
    fn explicit_preflight_input_reports_staged_refusal_precisely() {
        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join("witnesses")).unwrap();
        fs::write(path.join("witnesses/input.json"), "{}\n").unwrap();
        sh(path, &["add", "--", "witnesses/input.json"]);
        let outcome = publication_preflight(
            path,
            &PublishOptions::new(false, true)
                .with_preflight_inputs(vec![PathBuf::from("witnesses/input.json")]),
        )
        .unwrap_err();
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("is staged; unstage it"), "{reason}")
            }
            state => panic!("expected staged-input refusal, got {state:?}"),
        }
    }

    #[test]
    fn explicit_preflight_input_rejects_intent_to_add_index_entry() {
        let temporary = frontier();
        let path = temporary.path();
        fs::create_dir_all(path.join("sources")).unwrap();
        fs::write(path.join("sources/input.json"), "{}\n").unwrap();
        sh(path, &["add", "-N", "--", "sources/input.json"]);
        let outcome = publication_preflight(
            path,
            &PublishOptions::new(false, true)
                .with_preflight_inputs(vec![PathBuf::from("sources/input.json")]),
        )
        .unwrap_err();
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(reason.contains("index flag"), "{reason}")
            }
            state => panic!("expected intent-to-add refusal, got {state:?}"),
        }
    }

    #[test]
    fn candidate_attributes_come_from_expected_tree_not_unstaged_mask() {
        let temporary = frontier();
        let path = temporary.path();
        fs::write(path.join(".gitattributes"), ".vela/** filter=hostile\n").unwrap();
        sh(path, &["add", "--", ".gitattributes"]);
        sh(path, &["commit", "-q", "-m", "hostile tracked attributes"]);
        fs::write(
            path.join(".gitattributes"),
            "# safe-looking worktree mask\n",
        )
        .unwrap();
        change_vela(path);
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome = publish_decision(
            path,
            "attribute provenance",
            &[],
            &PublishOptions::new(false, true),
        );
        match outcome.state {
            PublicationState::Uncommitted { reason, .. } => {
                assert!(
                    reason.contains("unsafe effective Git attribute"),
                    "{reason}"
                )
            }
            state => panic!("expected candidate-attribute refusal, got {state:?}"),
        }
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
    }

    #[cfg(unix)]
    #[test]
    fn hostile_attribute_filter_never_executes_in_preflight_or_reconciliation() {
        use std::os::unix::fs::PermissionsExt;

        fn install_filter(path: &Path) -> PathBuf {
            let marker = path.join("FILTER_EXECUTED");
            let script = path.join("evil-filter.sh");
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
                &["config", "filter.evil.clean", script.to_str().unwrap()],
            );
            sh(path, &["config", "filter.evil.required", "true"]);
            marker
        }

        let with_info = frontier();
        let info_path = with_info.path();
        let marker = install_filter(info_path);
        fs::create_dir_all(info_path.join(".git/info")).unwrap();
        fs::write(
            info_path.join(".git/info/attributes"),
            ".vela/** filter=evil\n",
        )
        .unwrap();
        let preflight =
            publication_preflight(info_path, &PublishOptions::new(false, true)).unwrap_err();
        assert!(matches!(
            preflight.state,
            PublicationState::Uncommitted { .. }
        ));
        assert!(!marker.exists(), "preflight executed hostile info filter");
        change_vela(info_path);
        let planned = publish_decision(
            info_path,
            "hostile info",
            &[],
            &PublishOptions::new(false, true),
        );
        assert!(matches!(
            planned.state,
            PublicationState::Uncommitted { .. }
        ));
        assert!(!marker.exists(), "planning executed hostile info filter");

        let with_unstaged_attributes = frontier();
        let path = with_unstaged_attributes.path();
        let marker = install_filter(path);
        fs::write(path.join(".gitattributes"), ".vela/** filter=evil\n").unwrap();
        change_vela(path);
        let outcome = publish_decision(
            path,
            "ignore unstaged filter mask",
            &[],
            &PublishOptions::new(false, true),
        );
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert!(
            !marker.exists(),
            "planning or index reconciliation executed hostile worktree filter"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_rejects_symlink_and_new_public_path_drift() {
        use std::os::unix::fs::symlink;

        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let stopped = publish_decision(
            path,
            "exact worktree",
            &[],
            &PublishOptions::new(false, true).failing_after_ref_move(),
        );
        let operation = operation_from(&stopped);
        let bytes = fs::read(path.join(".vela/actors.json")).unwrap();
        fs::write(path.join("outside-identical"), bytes).unwrap();
        fs::remove_file(path.join(".vela/actors.json")).unwrap();
        symlink(
            path.join("outside-identical"),
            path.join(".vela/actors.json"),
        )
        .unwrap();
        fs::create_dir_all(path.join("records")).unwrap();
        fs::write(path.join("records/appeared.json"), "{}\n").unwrap();
        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        match recovered.state {
            PublicationState::Unknown { reason } => {
                assert!(reason.contains("worktree drift"), "{reason}")
            }
            state => panic!("expected exact-worktree refusal, got {state:?}"),
        }
    }

    #[test]
    fn real_index_lock_prevents_post_ref_overwrite_and_recovery_resumes() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let index_before = fs::read(path.join(".git/index")).unwrap();
        fs::write(path.join(".git/index.lock"), "external owner\n").unwrap();
        let outcome = publish_decision(path, "index lock", &[], &PublishOptions::new(false, true));
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(fs::read(path.join(".git/index")).unwrap(), index_before);
        assert_eq!(
            fs::read_to_string(path.join(".git/index.lock")).unwrap(),
            "external owner\n"
        );
        fs::remove_file(path.join(".git/index.lock")).unwrap();
        let operation = active_operation(path);
        let recovered = recover_publication(path, &operation, &PublishOptions::new(false, true));
        assert!(matches!(
            recovered.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert!(sh(path, &["status", "--porcelain", "--", ".vela"]).is_empty());
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
            let outcome = recover_publication(path, invalid, &PublishOptions::new(false, true));
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
    fn push_uses_remote_branch_ref_and_committed_journal_can_resume() {
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
        change_vela(path);
        let pushed = publish_decision(path, "push", &[], &PublishOptions::pushing());
        assert!(matches!(pushed.state, PublicationState::Pushed { .. }));
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            sh(path, &["rev-parse", "HEAD"])
        );
        assert!(
            sh(
                remote.path(),
                &["for-each-ref", "--format=%(refname)", "refs/remotes"]
            )
            .is_empty()
        );

        fs::create_dir_all(path.join("records")).unwrap();
        fs::write(path.join("records/resume.json"), "{}\n").unwrap();
        let local = publish_decision(path, "resume push", &[], &PublishOptions::new(false, true));
        let operation = active_operation(path);
        let expected_recovery = format!("vela publication recover --operation {operation} --push");
        assert_eq!(
            local.recovery_command.as_deref(),
            Some(expected_recovery.as_str())
        );
        let resumed = recover_publication(path, &operation, &PublishOptions::pushing());
        assert!(matches!(resumed.state, PublicationState::Pushed { .. }));
        assert_eq!(
            sh(remote.path(), &["rev-parse", "refs/heads/main"]),
            sh(path, &["rev-parse", "HEAD"])
        );
    }

    #[test]
    fn push_explicitly_uploads_exact_lfs_objects_with_hooks_disabled() {
        if !Command::new("git")
            .args(["lfs", "version"])
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return;
        }
        let temporary = frontier();
        let path = temporary.path();
        let remote = tempfile::tempdir().unwrap();
        sh(remote.path(), &["init", "-q", "--bare"]);
        sh(path, &["lfs", "install", "--local"]);
        fs::write(
            path.join(".gitattributes"),
            "witnesses/** filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        sh(path, &["add", "--", ".gitattributes"]);
        sh(path, &["commit", "-q", "-m", "witness LFS attributes"]);
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
        fs::create_dir_all(path.join("witnesses")).unwrap();
        fs::write(
            path.join("witnesses/upload.bin"),
            b"large witness bytes transported explicitly\n",
        )
        .unwrap();
        let outcome = publish_decision(path, "LFS push", &[], &PublishOptions::pushing());
        assert!(matches!(outcome.state, PublicationState::Pushed { .. }));
        let pointer = sh(
            remote.path(),
            &["show", "refs/heads/main:witnesses/upload.bin"],
        );
        let attributes = sh(
            path,
            &[
                "check-attr",
                "--cached",
                "--all",
                "--",
                "witnesses/upload.bin",
            ],
        );
        let oid = pointer
            .lines()
            .find_map(|line| line.strip_prefix("oid sha256:"))
            .unwrap_or_else(|| {
                panic!(
                    "remote blob was not an LFS pointer: {pointer:?}; attributes: {attributes:?}"
                )
            });
        assert!(
            remote
                .path()
                .join("lfs/objects")
                .join(&oid[..2])
                .join(&oid[2..4])
                .join(oid)
                .is_file(),
            "bare remote is missing explicitly uploaded LFS object {oid}"
        );
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
            "git push -- 'origin'\"'\"'; touch /tmp/pwn; echo '\"'\"'' 'refs/heads/main:refs/heads/main;$(touch${IFS}/tmp/pwn)'"
        );
    }

    #[test]
    fn no_commit_returns_structured_uncommitted_without_writing_git() {
        let temporary = frontier();
        let path = temporary.path();
        change_vela(path);
        let before = sh(path, &["rev-parse", "HEAD"]);
        let outcome = publish_decision(path, "x", &[], &PublishOptions::new(true, false));
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));
        assert_eq!(sh(path, &["rev-parse", "HEAD"]), before);
    }
}
