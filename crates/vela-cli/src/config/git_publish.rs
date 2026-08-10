//! Exact local Git commits for installed repository transactions.
//!
//! Vela owns the scientific transaction. Git owns repository history and
//! network publication. This module therefore does one small job: turn the
//! exact public delta already checked by Vela into one local Git commit without
//! touching the caller's index or unrelated worktree changes.
//!
//! The implementation uses Git's native plumbing:
//! `read-tree` builds an isolated index, `commit-tree` creates the commit, and
//! `update-ref <new> <old>` advances the checked-out branch with compare-and-
//! swap semantics. Pushing, authentication, retries, and remote policy remain
//! ordinary Git concerns.
//!
//! The integrity assets are the intended worktree, Git/common/object
//! directories, caller index, target ref, and unrelated caller changes. Treat
//! inherited process configuration, hooks, attributes and filters, object
//! alternates, replacement refs, prompts, pagers, and locale as hostile. Bind
//! repository storage at preflight, ignore ambient redirection, and retain the
//! ref compare-and-swap for concurrent drift. This local publisher performs no
//! network operation and grants no scientific authority.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NULL_DEVICE: &str = "/dev/null";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum PublicationState {
    Unchanged {
        commit: String,
    },
    Uncommitted {
        candidate: Option<String>,
        reason: String,
    },
    CommittedLocal {
        commit: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PublicationOutcome {
    #[serde(flatten)]
    pub state: PublicationState,
}

impl PublicationOutcome {
    fn uncommitted(reason: impl Into<String>) -> Self {
        Self {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: reason.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicationDeltaEntry {
    pub path: String,
    pub preimage_sha256: Option<String>,
    pub postimage: Option<Vec<u8>>,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublicationDelta {
    pub root: String,
    pub entries: Vec<PublicationDeltaEntry>,
}

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

#[derive(Debug, Clone)]
pub(crate) struct PublishOptions {
    preflight_inputs: Vec<PathBuf>,
}

impl PublishOptions {
    pub(crate) fn local() -> Self {
        Self {
            preflight_inputs: Vec::new(),
        }
    }

    pub(crate) fn with_preflight_inputs(mut self, paths: Vec<PathBuf>) -> Self {
        self.preflight_inputs = paths;
        self
    }
}

#[derive(Debug)]
pub(crate) struct ExactPublicationPreflight {
    repository_root: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
    object_dir: PathBuf,
    object_format: String,
    target_ref: String,
    expected_head: String,
    delta_sha256: String,
    input_hashes: Vec<(PathBuf, String)>,
}

pub(crate) fn publication_repo_relative_path(
    repository_path: &Path,
    repository_relative: &str,
) -> Result<String, String> {
    validate_relative_path(repository_relative)?;
    let root = resolve_git_repository(repository_path)?.worktree;
    let repository_path = repository_path
        .canonicalize()
        .map_err(|error| format!("canonicalize repository: {error}"))?;
    let prefix = repository_path
        .strip_prefix(&root)
        .map_err(|_| "repository is outside the resolved Git worktree".to_string())?;
    let path = prefix.join(repository_relative);
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| "non-UTF-8 repository paths are not publishable".to_string())
}

pub(crate) fn exact_publication_preflight(
    repository_path: &Path,
    delta: &PublicationDelta,
    options: &PublishOptions,
) -> Result<ExactPublicationPreflight, PublicationOutcome> {
    preflight(repository_path, delta, options).map_err(PublicationOutcome::uncommitted)
}

pub(crate) fn publish_exact_delta(
    repository_path: &Path,
    summary: &str,
    object_ids: &[String],
    delta: &PublicationDelta,
    preflight: ExactPublicationPreflight,
) -> Result<PublicationOutcome, ExactPublicationError> {
    let actual_sha256 = publication_delta_sha256(delta);
    if actual_sha256 != preflight.delta_sha256 {
        return Err(ExactPublicationError::DeltaChanged {
            expected_sha256: preflight.delta_sha256,
            actual_sha256,
        });
    }
    Ok(
        publish(repository_path, summary, object_ids, delta, &preflight)
            .unwrap_or_else(PublicationOutcome::uncommitted),
    )
}

fn preflight(
    repository_path: &Path,
    delta: &PublicationDelta,
    options: &PublishOptions,
) -> Result<ExactPublicationPreflight, String> {
    validate_delta(delta)?;
    let repository = resolve_git_repository(repository_path)?;
    let target_ref = git_text_in(&repository, &["symbolic-ref", "-q", "HEAD"]).map_err(|_| {
        "Vela requires a checked-out Git branch; detached HEAD is read-only".to_string()
    })?;
    let expected_head = git_text_in(&repository, &["rev-parse", "HEAD^{commit}"])?;

    ensure_delta_paths_unstaged(&repository, delta)?;

    for entry in &delta.entries {
        let expected = entry.preimage_sha256.as_deref();
        let observed = git_blob_sha256(&repository, &expected_head, &entry.path)?;
        if observed.as_deref() != expected {
            return Err(format!(
                "Git preimage changed for {} (expected {}, observed {})",
                entry.path,
                expected.unwrap_or("absent"),
                observed.as_deref().unwrap_or("absent")
            ));
        }
        let worktree = file_sha256_if_regular(&repository.worktree.join(&entry.path))?;
        if worktree.as_deref() != expected {
            return Err(format!(
                "worktree preimage changed for {} (expected {}, observed {})",
                entry.path,
                expected.unwrap_or("absent"),
                worktree.as_deref().unwrap_or("absent")
            ));
        }
    }

    let input_hashes = options
        .preflight_inputs
        .iter()
        .map(|path| {
            file_sha256(path)
                .map(|digest| (path.clone(), digest))
                .map_err(|error| format!("preflight input {}: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ExactPublicationPreflight {
        repository_root: repository.worktree,
        git_dir: repository.git_dir,
        git_common_dir: repository.git_common_dir,
        object_dir: repository.object_dir,
        object_format: repository.object_format,
        target_ref,
        expected_head,
        delta_sha256: publication_delta_sha256(delta),
        input_hashes,
    })
}

fn publish(
    repository_path: &Path,
    summary: &str,
    object_ids: &[String],
    delta: &PublicationDelta,
    preflight: &ExactPublicationPreflight,
) -> Result<PublicationOutcome, String> {
    let repository = resolve_git_repository(repository_path)?;
    if repository.worktree != preflight.repository_root
        || repository.git_dir != preflight.git_dir
        || repository.git_common_dir != preflight.git_common_dir
        || repository.object_dir != preflight.object_dir
        || repository.object_format != preflight.object_format
    {
        return Err("Git repository storage changed after Vela preflight".to_string());
    }
    let root = &repository.worktree;
    let target_ref = git_text_in(&repository, &["symbolic-ref", "-q", "HEAD"])?;
    if target_ref != preflight.target_ref {
        return Err("checked-out Git branch changed after Vela preflight".to_string());
    }
    let current_head = git_text_in(&repository, &["rev-parse", "HEAD^{commit}"])?;
    if current_head != preflight.expected_head {
        return Err(format!(
            "Git HEAD changed after Vela preflight (expected {}, observed {current_head})",
            preflight.expected_head
        ));
    }
    ensure_delta_paths_unstaged(&repository, delta)?;
    for (path, expected) in &preflight.input_hashes {
        let observed = file_sha256(path)?;
        if &observed != expected {
            return Err(format!("preflight input {} changed", path.display()));
        }
    }
    for entry in &delta.entries {
        let observed = file_sha256_if_regular(&root.join(&entry.path))?;
        let expected = entry.postimage.as_deref().map(sha256_bytes);
        if observed != expected {
            return Err(format!("installed postimage changed for {}", entry.path));
        }
    }

    let temporary = tempfile::tempdir().map_err(|error| format!("Git index tempdir: {error}"))?;
    let index = temporary.path().join("index");
    git_with_index(
        &repository,
        &index,
        &["read-tree", &preflight.expected_head],
        None,
    )?;

    let mut caller_index_updates = Vec::with_capacity(delta.entries.len());
    for entry in &delta.entries {
        match &entry.postimage {
            Some(bytes) => {
                let blob = git_with_index(
                    &repository,
                    &index,
                    &["hash-object", "-w", "--stdin"],
                    Some(bytes),
                )?;
                let mode = if entry.executable { "100755" } else { "100644" };
                git_with_index(
                    &repository,
                    &index,
                    &[
                        "update-index",
                        "--add",
                        "--cacheinfo",
                        mode,
                        &blob,
                        &entry.path,
                    ],
                    None,
                )?;
                caller_index_updates.push((entry.path.clone(), Some((mode.to_string(), blob))));
            }
            None => {
                git_with_index(
                    &repository,
                    &index,
                    &["update-index", "--force-remove", "--", &entry.path],
                    None,
                )?;
                caller_index_updates.push((entry.path.clone(), None));
            }
        }
    }
    let tree = git_with_index(&repository, &index, &["write-tree"], None)?;
    let parent_tree = git_text_in(&repository, &["rev-parse", "HEAD^{tree}"])?;
    if tree == parent_tree {
        return Ok(PublicationOutcome {
            state: PublicationState::Unchanged {
                commit: preflight.expected_head.clone(),
            },
        });
    }

    let mut message = format!("vela: {summary}");
    if !object_ids.is_empty() {
        message.push_str("\n\nObjects: ");
        message.push_str(&object_ids.join(", "));
    }
    message.push_str("\n\nDelta-root: ");
    message.push_str(&delta.root);
    let commit = git_with_index(
        &repository,
        &index,
        &["commit-tree", &tree, "-p", &preflight.expected_head],
        Some(message.as_bytes()),
    )?;
    git_text_in(
        &repository,
        &[
            "update-ref",
            "-m",
            summary,
            &preflight.target_ref,
            &commit,
            &preflight.expected_head,
        ],
    )?;
    for (path, update) in caller_index_updates {
        let result = match update {
            Some((mode, blob)) => git_text_in(
                &repository,
                &["update-index", "--add", "--cacheinfo", &mode, &blob, &path],
            ),
            None => git_text_in(
                &repository,
                &["update-index", "--force-remove", "--", &path],
            ),
        };
        result.map_err(|error| {
            format!(
                "created local Vela commit {commit}, but could not refresh the caller index for {path}: {error}"
            )
        })?;
    }
    Ok(PublicationOutcome {
        state: PublicationState::CommittedLocal { commit },
    })
}

fn ensure_delta_paths_unstaged(
    repository: &GitRepository,
    delta: &PublicationDelta,
) -> Result<(), String> {
    let mut args = vec![
        "diff",
        "--cached",
        "--quiet",
        "--no-ext-diff",
        "--no-textconv",
        "HEAD",
        "--",
    ];
    args.extend(delta.entries.iter().map(|entry| entry.path.as_str()));
    let output = git_output_in(repository, &args, None)?;
    match output.status.code() {
        Some(0) => Ok(()),
        Some(1) => Err(
            "Git index has staged changes on Vela transaction paths; commit or unstage them before retrying"
                .to_string(),
        ),
        _ => Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

fn validate_delta(delta: &PublicationDelta) -> Result<(), String> {
    if delta.entries.is_empty() {
        return Err("publication delta is empty".to_string());
    }
    let mut previous: Option<&str> = None;
    let mut folded = BTreeSet::new();
    for entry in &delta.entries {
        validate_relative_path(&entry.path)?;
        if previous.is_some_and(|value| value >= entry.path.as_str()) {
            return Err("publication paths must be strictly sorted and unique".to_string());
        }
        let key = entry.path.to_lowercase();
        if !folded.insert(key) {
            return Err("publication paths collide under case folding".to_string());
        }
        previous = Some(&entry.path);
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || value.contains('\\') || value.chars().any(char::is_control) {
        return Err(format!("invalid publication path: {value}"));
    }
    for component in path.components() {
        match component {
            Component::Normal(value)
                if value.to_str().is_some_and(|value| {
                    !value.eq_ignore_ascii_case(".git")
                        && !value.starts_with(':')
                        && !value.contains(['*', '?', '['])
                        && value.trim_end_matches([' ', '.']) == value
                }) => {}
            _ => {
                return Err(format!(
                    "unsafe or non-normalized publication path: {value}"
                ));
            }
        }
    }
    Ok(())
}

fn publication_delta_sha256(delta: &PublicationDelta) -> String {
    let mut digest = Sha256::new();
    digest.update(b"vela.git-publication-delta.v1\0");
    digest.update(delta.root.as_bytes());
    for entry in &delta.entries {
        digest.update([0]);
        digest.update(entry.path.as_bytes());
        digest.update([u8::from(entry.executable)]);
        if let Some(preimage) = &entry.preimage_sha256 {
            digest.update(preimage.as_bytes());
        }
        digest.update([0]);
        if let Some(postimage) = &entry.postimage {
            digest.update(postimage);
        }
    }
    format!("sha256:{:x}", digest.finalize())
}

fn git_blob_sha256(
    repository: &GitRepository,
    commit: &str,
    path: &str,
) -> Result<Option<String>, String> {
    let spec = format!("{commit}:{path}");
    let output = git_output_in(repository, &["show", &spec], None)?;
    if output.status.success() {
        return Ok(Some(sha256_bytes(&output.stdout)));
    }
    let missing = git_output_in(repository, &["cat-file", "-e", &spec], None)?;
    if !missing.status.success() {
        return Ok(None);
    }
    Err(format!("Git object {spec} is not a regular blob"))
}

fn file_sha256_if_regular(path: &Path) -> Result<Option<String>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => file_sha256(path).map(Some),
        Ok(_) => Err(format!("{} is not a regular file", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("inspect {}: {error}", path.display())),
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitRepository {
    worktree: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
    object_dir: PathBuf,
    object_format: String,
}

fn resolve_git_repository(path: &Path) -> Result<GitRepository, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("canonicalize repository path {}: {error}", path.display()))?;
    if !path.is_dir() {
        return Err(format!(
            "repository path is not a directory: {}",
            path.display()
        ));
    }
    let worktree = path
        .ancestors()
        .find(|ancestor| {
            std::fs::symlink_metadata(ancestor.join(".git"))
                .is_ok_and(|metadata| metadata.is_dir() || metadata.is_file())
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("not a Git worktree: {}", path.display()))?;

    let output = run_git_discovery(&worktree, &["rev-parse", "--absolute-git-dir"])?;
    let git_dir = successful_git_text(output, &["rev-parse", "--absolute-git-dir"])?;
    let git_dir = PathBuf::from(git_dir)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git directory: {error}"))?;
    let mut repository = GitRepository {
        worktree,
        git_common_dir: git_dir.clone(),
        object_dir: git_dir.join("objects"),
        git_dir,
        object_format: String::new(),
    };
    let observed_worktree =
        PathBuf::from(git_text_in(&repository, &["rev-parse", "--show-toplevel"])?)
            .canonicalize()
            .map_err(|error| format!("canonicalize Git worktree: {error}"))?;
    if observed_worktree != repository.worktree {
        return Err(format!(
            "resolved Git worktree changed (expected {}, observed {})",
            repository.worktree.display(),
            observed_worktree.display()
        ));
    }

    reject_local_config_includes(&repository)?;
    repository.git_common_dir = PathBuf::from(git_text_in(
        &repository,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?)
    .canonicalize()
    .map_err(|error| format!("canonicalize Git common directory: {error}"))?;
    repository.object_dir = PathBuf::from(git_text_in(
        &repository,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
    )?)
    .canonicalize()
    .map_err(|error| format!("canonicalize Git object directory: {error}"))?;
    if repository.object_dir.parent() != Some(repository.git_common_dir.as_path()) {
        return Err(format!(
            "Git object directory is outside the common repository directory: {}",
            repository.object_dir.display()
        ));
    }
    let alternates = repository.object_dir.join("info").join("alternates");
    match std::fs::symlink_metadata(&alternates) {
        Ok(_) => {
            return Err(format!(
                "Git object alternates are unsupported for exact publication: {}",
                alternates.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "inspect Git object alternates {}: {error}",
                alternates.display()
            ));
        }
    }
    repository.object_format = git_text_in(&repository, &["rev-parse", "--show-object-format"])?;
    Ok(repository)
}

fn reject_local_config_includes(repository: &GitRepository) -> Result<(), String> {
    reject_config_includes(repository, "--local")?;
    let worktree_config = git_output_in(
        repository,
        &[
            "config",
            "--local",
            "--no-includes",
            "--type=bool",
            "--get",
            "extensions.worktreeConfig",
        ],
        None,
    )?;
    match worktree_config.status.code() {
        Some(0) if String::from_utf8_lossy(&worktree_config.stdout).trim() == "true" => {
            reject_config_includes(repository, "--worktree")
        }
        Some(0 | 1) => Ok(()),
        _ => Err(format!(
            "inspect Git worktree configuration: {}",
            String::from_utf8_lossy(&worktree_config.stderr).trim()
        )),
    }
}

fn reject_config_includes(repository: &GitRepository, scope: &str) -> Result<(), String> {
    let output = git_output_in(
        repository,
        &["config", scope, "--no-includes", "--list", "--name-only"],
        None,
    )?;
    let names = successful_git_text(output, &["config", scope, "--no-includes", "--list"])?;
    if let Some(name) = names.lines().find(|name| {
        let name = name.to_ascii_lowercase();
        name.starts_with("include.") || name.starts_with("includeif.")
    }) {
        return Err(format!(
            "Git {scope} config includes are unsupported for exact publication: {name}"
        ));
    }
    Ok(())
}

#[cfg(test)]
fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let repository = resolve_git_repository(root)?;
    git_text_in(&repository, args)
}

fn git_text_in(repository: &GitRepository, args: &[&str]) -> Result<String, String> {
    let output = git_output_in(repository, args, None)?;
    successful_git_text(output, args)
}

fn successful_git_text(output: std::process::Output, args: &[&str]) -> Result<String, String> {
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("git {} returned non-UTF-8 output: {error}", args.join(" ")))
}

fn git_with_index(
    repository: &GitRepository,
    index: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<String, String> {
    let output = git_output_with_index(repository, index, args, stdin)?;
    successful_git_text(output, args)
}

fn git_output_in(
    repository: &GitRepository,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    run_git(repository, None, args, stdin)
}

fn git_output_with_index(
    repository: &GitRepository,
    index: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    run_git(repository, Some(index), args, stdin)
}

fn run_git(
    repository: &GitRepository,
    index: Option<&Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    let mut command =
        isolated_git_command(repository, index, std::env::vars_os().map(|(name, _)| name));
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .ok_or_else(|| "Git stdin unavailable".to_string())?
            .write_all(input)
            .map_err(|error| format!("write git stdin: {error}"))?;
    }
    child
        .wait_with_output()
        .map_err(|error| format!("wait for git {}: {error}", args.join(" ")))
}

fn run_git_discovery(worktree: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let repository = GitRepository {
        worktree: worktree.to_path_buf(),
        git_dir: worktree.join(".git"),
        git_common_dir: worktree.join(".git"),
        object_dir: worktree.join(".git").join("objects"),
        object_format: String::new(),
    };
    let mut command =
        isolated_git_command(&repository, None, std::env::vars_os().map(|(name, _)| name));
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

fn isolated_git_command(
    repository: &GitRepository,
    index: Option<&Path>,
    inherited_environment: impl IntoIterator<Item = OsString>,
) -> Command {
    let mut command = Command::new("git");

    // Git grows new repository and configuration environment variables over
    // time. Remove the namespace rather than trying to maintain a denylist;
    // this also covers indexed GIT_CONFIG_KEY_n / GIT_CONFIG_VALUE_n entries.
    for name in inherited_environment {
        if name.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(name);
        }
    }

    command
        .current_dir(&repository.worktree)
        .arg("--no-pager")
        .arg("--no-optional-locks")
        .arg("--no-replace-objects")
        .arg("--literal-pathspecs")
        .arg("--work-tree")
        .arg(&repository.worktree);
    if repository.git_dir != repository.worktree.join(".git") || repository.git_dir.is_dir() {
        command.arg("--git-dir").arg(&repository.git_dir);
    }
    command
        .arg("-c")
        .arg("core.bare=false")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg(format!("core.hooksPath={NULL_DEVICE}"))
        .arg("-c")
        .arg(format!("core.attributesFile={NULL_DEVICE}"))
        .arg("-c")
        .arg(format!("core.excludesFile={NULL_DEVICE}"))
        .arg("-c")
        .arg("diff.external=")
        .arg("-c")
        .arg("submodule.recurse=false")
        .arg("-c")
        .arg("protocol.file.allow=never")
        .arg("-c")
        .arg("commit.gpgSign=false")
        .arg("-C")
        .arg(&repository.worktree)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", NULL_DEVICE)
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("LC_ALL", "C");
    if repository.git_dir == repository.worktree.join(".git") && !repository.git_dir.is_dir() {
        command.env("GIT_CEILING_DIRECTORIES", &repository.worktree);
    }
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::OsStr;

    const HOSTILE_CHILD: &str = "VELA_GIT_PUBLISH_HOSTILE_CHILD";
    const HOSTILE_REPOSITORY: &str = "VELA_GIT_PUBLISH_HOSTILE_REPOSITORY";

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fixture_git_text(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn setup_repository(root: &Path) {
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Vela Test"]);
        git(root, &["config", "user.email", "vela@example.invalid"]);
    }

    fn one_file_delta(path: &str) -> PublicationDelta {
        PublicationDelta {
            root: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            entries: vec![PublicationDeltaEntry {
                path: path.into(),
                preimage_sha256: Some(sha256_bytes(b"before")),
                postimage: Some(b"after".to_vec()),
                executable: false,
            }],
        }
    }

    fn snapshot_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(base: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    visit(base, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(base).unwrap().to_path_buf(),
                        std::fs::read(path).unwrap(),
                    );
                }
            }
        }

        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, contents).unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn write_command_removes_all_git_environment_and_restores_only_owned_values() {
        let temp = tempfile::tempdir().unwrap();
        setup_repository(temp.path());
        let repository = resolve_git_repository(temp.path()).unwrap();
        let index = temp.path().join("owned-index");
        let inherited = [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_REPLACE_REF_BASE",
            "GIT_NO_LAZY_FETCH",
            "GIT_NAMESPACE",
            "GIT_SHALLOW_FILE",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_KEY_99",
            "GIT_CONFIG_VALUE_99",
            "GIT_CONFIG_PARAMETERS",
            "GIT_ASKPASS",
            "GIT_EDITOR",
            "GIT_PAGER",
            "GIT_ATTR_SOURCE",
            "GIT_ALLOW_PROTOCOL",
            "GIT_PROTOCOL_FROM_USER",
        ];
        let command = isolated_git_command(
            &repository,
            Some(&index),
            inherited.iter().map(OsString::from),
        );
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<BTreeMap<_, _>>();
        let owned = [
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", NULL_DEVICE),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_SYSTEM", NULL_DEVICE),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_PAGER", "cat"),
            ("GIT_TERMINAL_PROMPT", "0"),
        ];
        for (name, value) in &environment {
            if value.is_some() && name.to_str().is_some_and(|name| name.starts_with("GIT_")) {
                assert!(
                    name == OsStr::new("GIT_INDEX_FILE")
                        || name == OsStr::new("GIT_CEILING_DIRECTORIES")
                        || owned.iter().any(|(owned, _)| name == OsStr::new(owned)),
                    "unowned Git environment was restored: {name:?}"
                );
            }
        }
        for name in inherited {
            if name != "GIT_INDEX_FILE" && !owned.iter().any(|(owned, _)| *owned == name) {
                assert_eq!(
                    environment.get(OsStr::new(name)),
                    Some(&None),
                    "inherited Git environment was not removed: {name}"
                );
            }
        }
        for (name, value) in owned {
            assert_eq!(
                environment.get(OsStr::new(name)),
                Some(&Some(OsString::from(value))),
                "missing owned Git environment: {name}"
            );
        }
        assert_eq!(
            environment.get(OsStr::new("GIT_INDEX_FILE")),
            Some(&Some(index.into_os_string()))
        );
        assert_eq!(
            environment.get(OsStr::new("PAGER")),
            Some(&Some(OsString::from("cat")))
        );
        assert_eq!(
            environment.get(OsStr::new("LC_ALL")),
            Some(&Some(OsString::from("C")))
        );
        for required in [
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "--literal-pathspecs",
            "core.hooksPath=/dev/null",
            "core.attributesFile=/dev/null",
            "core.excludesFile=/dev/null",
            "protocol.file.allow=never",
        ] {
            assert!(
                command.get_args().any(|arg| arg == required),
                "missing Git argument: {required}"
            );
        }
    }

    #[test]
    fn on_disk_object_alternates_fail_preflight() {
        let intended = tempfile::tempdir().unwrap();
        let sentinel = tempfile::tempdir().unwrap();
        setup_repository(intended.path());
        setup_repository(sentinel.path());
        std::fs::write(intended.path().join("a.txt"), b"before").unwrap();
        git(intended.path(), &["add", "."]);
        git(intended.path(), &["commit", "-m", "initial"]);
        let sentinel_objects = snapshot_files(&sentinel.path().join(".git/objects"));
        let info = intended.path().join(".git/objects/info");
        std::fs::create_dir_all(&info).unwrap();
        std::fs::write(
            info.join("alternates"),
            format!("{}\n", sentinel.path().join(".git/objects").display()),
        )
        .unwrap();

        let error = exact_publication_preflight(
            intended.path(),
            &one_file_delta("a.txt"),
            &PublishOptions::local(),
        )
        .expect_err("on-disk object alternates must fail closed");
        assert!(matches!(
            error.state,
            PublicationState::Uncommitted { reason, .. }
                if reason.contains("object alternates are unsupported")
        ));
        assert_eq!(
            snapshot_files(&sentinel.path().join(".git/objects")),
            sentinel_objects
        );
    }

    #[test]
    fn local_config_include_cannot_redirect_discovery() {
        let intended = tempfile::tempdir().unwrap();
        let sentinel = tempfile::tempdir().unwrap();
        setup_repository(intended.path());
        std::fs::write(intended.path().join("a.txt"), b"before").unwrap();
        git(intended.path(), &["add", "."]);
        git(intended.path(), &["commit", "-m", "initial"]);
        let sentinel_files = snapshot_files(sentinel.path());
        let hostile_config = intended.path().join("hostile-include.gitconfig");
        std::fs::write(
            &hostile_config,
            format!(
                "[core]\n\tworktree = {}\n[extensions]\n\tobjectFormat = sha256\n",
                sentinel.path().display()
            ),
        )
        .unwrap();
        git(
            intended.path(),
            &[
                "config",
                "--local",
                "include.path",
                hostile_config.to_str().unwrap(),
            ],
        );

        let error = exact_publication_preflight(
            intended.path(),
            &one_file_delta("a.txt"),
            &PublishOptions::local(),
        )
        .expect_err("local includes must not redirect exact publication");
        assert!(matches!(
            error.state,
            PublicationState::Uncommitted { reason, .. }
                if reason.contains("config includes are unsupported")
        ));
        assert_eq!(snapshot_files(sentinel.path()), sentinel_files);
    }

    #[cfg(unix)]
    #[test]
    fn hostile_ambient_git_state_cannot_redirect_publication() {
        if std::env::var_os(HOSTILE_CHILD).is_some() {
            let root = PathBuf::from(std::env::var_os(HOSTILE_REPOSITORY).unwrap());
            let repository_path = root.join("repository");
            let delta = one_file_delta("repository/a.txt");
            let preflight =
                exact_publication_preflight(&repository_path, &delta, &PublishOptions::local())
                    .unwrap();
            std::fs::write(root.join("repository/a.txt"), b"after").unwrap();
            let outcome = publish_exact_delta(
                &repository_path,
                "hostile ambient Git test",
                &[],
                &delta,
                preflight,
            )
            .unwrap();
            assert!(matches!(
                outcome.state,
                PublicationState::CommittedLocal { .. }
            ));
            return;
        }

        let intended = tempfile::tempdir().unwrap();
        let sentinel = tempfile::tempdir().unwrap();
        let hostile = tempfile::tempdir().unwrap();
        setup_repository(intended.path());
        setup_repository(sentinel.path());
        std::fs::create_dir(intended.path().join("repository")).unwrap();
        std::fs::write(intended.path().join("repository/a.txt"), b"before").unwrap();
        std::fs::write(intended.path().join("unrelated.txt"), b"clean").unwrap();
        std::fs::write(sentinel.path().join("sentinel.txt"), b"untouched").unwrap();
        git(intended.path(), &["add", "."]);
        git(intended.path(), &["commit", "-m", "initial"]);
        git(sentinel.path(), &["add", "."]);
        git(sentinel.path(), &["commit", "-m", "sentinel"]);
        std::fs::write(intended.path().join("unrelated.txt"), b"staged").unwrap();
        git(intended.path(), &["add", "unrelated.txt"]);
        std::fs::write(intended.path().join("unrelated.txt"), b"dirty-after-stage").unwrap();

        let sentinel_index = hostile.path().join("sentinel-index");
        let output = Command::new("git")
            .current_dir(sentinel.path())
            .args(["read-tree", "HEAD"])
            .env("GIT_INDEX_FILE", &sentinel_index)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let sentinel_index_before = std::fs::read(&sentinel_index).unwrap();
        let sentinel_objects_before = snapshot_files(&sentinel.path().join(".git/objects"));
        let sentinel_head = fixture_git_text(sentinel.path(), &["rev-parse", "HEAD"]);
        let intended_head = fixture_git_text(intended.path(), &["rev-parse", "HEAD"]);

        let side_effect = hostile.path().join("git-side-effect");
        let helper = hostile.path().join("hostile-helper");
        let helper_source = format!("#!/bin/sh\n: > '{}'\ncat\n", side_effect.display());
        write_executable(&helper, &helper_source);
        let hooks = hostile.path().join("hooks");
        std::fs::create_dir(&hooks).unwrap();
        let hook_script = hooks.join("reference-transaction");
        write_executable(&hook_script, &helper_source);
        write_executable(
            &intended.path().join(".git/hooks/reference-transaction"),
            &helper_source,
        );
        std::fs::create_dir_all(intended.path().join(".git/info")).unwrap();
        std::fs::write(
            intended.path().join(".git/info/attributes"),
            b"repository/a.txt filter=hostile\n",
        )
        .unwrap();
        git(
            intended.path(),
            &["config", "filter.hostile.clean", helper.to_str().unwrap()],
        );
        git(
            intended.path(),
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );
        git(
            intended.path(),
            &["config", "core.worktree", sentinel.path().to_str().unwrap()],
        );

        let attributes = hostile.path().join("attributes");
        let excludes = hostile.path().join("excludes");
        std::fs::write(&attributes, b"repository/a.txt filter=hostile\n").unwrap();
        std::fs::write(&excludes, b"repository/a.txt\n").unwrap();
        let config_contents = format!(
            "[core]\n\tworktree = {}\n\thooksPath = {}\n\tattributesFile = {}\n\texcludesFile = {}\n[filter \"hostile\"]\n\tclean = {}\n[protocol \"file\"]\n\tallow = always\n",
            sentinel.path().display(),
            hooks.display(),
            attributes.display(),
            excludes.display(),
            helper.display(),
        );
        let global_config = hostile.path().join("global.gitconfig");
        let system_config = hostile.path().join("system.gitconfig");
        let hostile_home = hostile.path().join("home");
        std::fs::create_dir(&hostile_home).unwrap();
        for config in [
            &global_config,
            &system_config,
            &hostile_home.join(".gitconfig"),
        ] {
            std::fs::write(config, &config_contents).unwrap();
        }

        let test_name =
            "config::git_publish::tests::hostile_ambient_git_state_cannot_redirect_publication";
        let ambient = [
            ("GIT_DIR", sentinel.path().join(".git").into_os_string()),
            ("GIT_WORK_TREE", sentinel.path().as_os_str().to_os_string()),
            ("GIT_INDEX_FILE", sentinel_index.as_os_str().to_os_string()),
            (
                "GIT_COMMON_DIR",
                sentinel.path().join(".git").into_os_string(),
            ),
            (
                "GIT_OBJECT_DIRECTORY",
                sentinel.path().join(".git/objects").into_os_string(),
            ),
            (
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                intended.path().join(".git/objects").into_os_string(),
            ),
            ("GIT_NAMESPACE", OsString::from("hostile")),
            (
                "GIT_SHALLOW_FILE",
                hostile.path().join("missing-shallow").into_os_string(),
            ),
            (
                "GIT_REPLACE_REF_BASE",
                OsString::from("refs/hostile-replacements/"),
            ),
            ("GIT_NO_LAZY_FETCH", OsString::from("0")),
            (
                "GIT_CONFIG_GLOBAL",
                global_config.as_os_str().to_os_string(),
            ),
            (
                "GIT_CONFIG_SYSTEM",
                system_config.as_os_str().to_os_string(),
            ),
            ("GIT_CONFIG_NOSYSTEM", OsString::from("0")),
            ("GIT_ATTR_NOSYSTEM", OsString::from("0")),
            ("GIT_ATTR_SOURCE", OsString::from("HEAD")),
            ("GIT_ALLOW_PROTOCOL", OsString::from("file")),
            ("GIT_PROTOCOL_FROM_USER", OsString::from("1")),
            ("GIT_TERMINAL_PROMPT", OsString::from("1")),
            ("GIT_ASKPASS", helper.as_os_str().to_os_string()),
            ("GIT_PAGER", helper.as_os_str().to_os_string()),
            ("GIT_EDITOR", helper.as_os_str().to_os_string()),
            ("GIT_TRACE", side_effect.as_os_str().to_os_string()),
        ];
        let injected = [
            ("core.worktree", sentinel.path().as_os_str()),
            ("core.hooksPath", hooks.as_os_str()),
            ("core.attributesFile", attributes.as_os_str()),
            ("core.excludesFile", excludes.as_os_str()),
            ("filter.hostile.clean", helper.as_os_str()),
            ("protocol.file.allow", OsStr::new("always")),
            ("core.fsmonitor", hook_script.as_os_str()),
            ("core.repositoryFormatVersion", OsStr::new("1")),
            ("extensions.objectFormat", OsStr::new("sha256")),
        ];
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", test_name, "--nocapture"])
            .env(HOSTILE_CHILD, "1")
            .env(HOSTILE_REPOSITORY, intended.path())
            .env("HOME", &hostile_home)
            .env("LC_ALL", "zz_ZZ")
            .env("PAGER", &helper)
            .env("GIT_CONFIG_COUNT", injected.len().to_string())
            .envs(ambient);
        for (index, (key, value)) in injected.into_iter().enumerate() {
            child
                .env(format!("GIT_CONFIG_KEY_{index}"), key)
                .env(format!("GIT_CONFIG_VALUE_{index}"), value);
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "hostile child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert_ne!(
            git_text(intended.path(), &["rev-parse", "HEAD"]).unwrap(),
            intended_head
        );
        assert_eq!(
            git_text(intended.path(), &["show", "HEAD:repository/a.txt"]).unwrap(),
            "after"
        );
        assert_eq!(
            git_text(intended.path(), &["show", "HEAD:unrelated.txt"]).unwrap(),
            "clean"
        );
        assert_eq!(
            git_text(intended.path(), &["show", ":unrelated.txt"]).unwrap(),
            "staged"
        );
        assert_eq!(
            std::fs::read(intended.path().join("unrelated.txt")).unwrap(),
            b"dirty-after-stage"
        );
        assert_eq!(
            fixture_git_text(sentinel.path(), &["rev-parse", "HEAD"]),
            sentinel_head
        );
        assert_eq!(
            std::fs::read(sentinel.path().join("sentinel.txt")).unwrap(),
            b"untouched"
        );
        assert_eq!(
            std::fs::read(&sentinel_index).unwrap(),
            sentinel_index_before
        );
        assert_eq!(
            snapshot_files(&sentinel.path().join(".git/objects")),
            sentinel_objects_before
        );
        assert!(
            !side_effect.exists(),
            "hostile Git helper or tracing executed"
        );
    }

    #[test]
    fn replacement_refs_are_ignored_for_preflight_and_commit_parent() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        setup_repository(root);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);
        let initial = fixture_git_text(root, &["rev-parse", "HEAD"]);
        std::fs::write(root.join("a.txt"), b"replacement").unwrap();
        git(root, &["add", "a.txt"]);
        git(root, &["commit", "-m", "replacement object"]);
        let replacement = fixture_git_text(root, &["rev-parse", "HEAD"]);
        git(root, &["reset", "--hard", &initial]);
        git(root, &["replace", &initial, &replacement]);
        assert_eq!(
            fixture_git_text(root, &["show", "HEAD:a.txt"]),
            "replacement"
        );

        let delta = one_file_delta("a.txt");
        let preflight =
            exact_publication_preflight(root, &delta, &PublishOptions::local()).unwrap();
        std::fs::write(root.join("a.txt"), b"after").unwrap();
        let outcome =
            publish_exact_delta(root, "ignore replacements", &[], &delta, preflight).unwrap();
        let commit = match outcome.state {
            PublicationState::CommittedLocal { commit } => commit,
            state => panic!("unexpected publication state: {state:?}"),
        };
        assert_eq!(
            fixture_git_text(root, &["--no-replace-objects", "show", "HEAD:a.txt"]),
            "after"
        );
        assert_eq!(
            fixture_git_text(
                root,
                &["--no-replace-objects", "rev-parse", &format!("{commit}^")]
            ),
            initial
        );
        assert_eq!(
            fixture_git_text(root, &["rev-parse", &format!("refs/replace/{initial}")]),
            replacement
        );
    }

    #[test]
    fn linked_worktree_publication_uses_the_bound_common_object_directory() {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("primary");
        let linked = temp.path().join("linked");
        std::fs::create_dir(&primary).unwrap();
        setup_repository(&primary);
        std::fs::write(primary.join("a.txt"), b"before").unwrap();
        git(&primary, &["add", "."]);
        git(&primary, &["commit", "-m", "initial"]);
        let primary_head = fixture_git_text(&primary, &["rev-parse", "HEAD"]);
        git(
            &primary,
            &[
                "worktree",
                "add",
                "-b",
                "publication-test",
                linked.to_str().unwrap(),
            ],
        );

        let delta = one_file_delta("a.txt");
        let preflight = exact_publication_preflight(&linked, &delta, &PublishOptions::local())
            .expect("linked worktree preflight");
        assert_ne!(preflight.git_dir, preflight.git_common_dir);
        assert_eq!(
            preflight.object_dir,
            preflight.git_common_dir.join("objects")
        );
        std::fs::write(linked.join("a.txt"), b"after").unwrap();
        let outcome = publish_exact_delta(&linked, "linked worktree", &[], &delta, preflight)
            .expect("linked worktree publication");
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(git_text(&linked, &["show", "HEAD:a.txt"]).unwrap(), "after");
        assert_eq!(
            fixture_git_text(&primary, &["rev-parse", "HEAD"]),
            primary_head
        );
    }

    #[test]
    fn staged_change_on_transaction_path_fails_preflight() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        setup_repository(root);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);

        std::fs::write(root.join("a.txt"), b"staged").unwrap();
        git(root, &["add", "a.txt"]);
        std::fs::write(root.join("a.txt"), b"before").unwrap();

        let error =
            exact_publication_preflight(root, &one_file_delta("a.txt"), &PublishOptions::local())
                .expect_err("staged transaction path must fail closed");
        assert!(matches!(
            error.state,
            PublicationState::Uncommitted { reason, .. }
                if reason.contains("staged changes on Vela transaction paths")
        ));
        assert_eq!(git_text(root, &["show", ":a.txt"]).unwrap(), "staged");
    }

    #[test]
    fn ref_change_after_preflight_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        setup_repository(root);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);
        let delta = one_file_delta("a.txt");
        let preflight =
            exact_publication_preflight(root, &delta, &PublishOptions::local()).unwrap();
        std::fs::write(root.join("other.txt"), b"advance").unwrap();
        git(root, &["add", "other.txt"]);
        git(root, &["commit", "-m", "advance"]);
        let advanced_head = fixture_git_text(root, &["rev-parse", "HEAD"]);
        std::fs::write(root.join("a.txt"), b"after").unwrap();
        let outcome = publish_exact_delta(root, "test", &[], &delta, preflight).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { reason, .. }
                if reason.contains("Git HEAD changed after Vela preflight")
        ));
        assert_eq!(
            fixture_git_text(root, &["rev-parse", "HEAD"]),
            advanced_head
        );
        assert_eq!(fixture_git_text(root, &["show", "HEAD:a.txt"]), "before");
    }
}
