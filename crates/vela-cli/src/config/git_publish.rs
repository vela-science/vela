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

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    let root = git_text(repository_path, &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
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
    let repository_root = PathBuf::from(git_text(
        repository_path,
        &["rev-parse", "--show-toplevel"],
    )?);
    let repository_root = repository_root
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    let target_ref = git_text(&repository_root, &["symbolic-ref", "-q", "HEAD"]).map_err(|_| {
        "Vela requires a checked-out Git branch; detached HEAD is read-only".to_string()
    })?;
    let expected_head = git_text(&repository_root, &["rev-parse", "HEAD^{commit}"])?;

    ensure_delta_paths_unstaged(&repository_root, delta)?;

    for entry in &delta.entries {
        let expected = entry.preimage_sha256.as_deref();
        let observed = git_blob_sha256(&repository_root, &expected_head, &entry.path)?;
        if observed.as_deref() != expected {
            return Err(format!(
                "Git preimage changed for {} (expected {}, observed {})",
                entry.path,
                expected.unwrap_or("absent"),
                observed.as_deref().unwrap_or("absent")
            ));
        }
        let worktree = file_sha256_if_regular(&repository_root.join(&entry.path))?;
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
        repository_root,
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
    let root = PathBuf::from(git_text(
        repository_path,
        &["rev-parse", "--show-toplevel"],
    )?);
    let root = root
        .canonicalize()
        .map_err(|error| format!("canonicalize Git root: {error}"))?;
    if root != preflight.repository_root {
        return Err("Git worktree changed after Vela preflight".to_string());
    }
    let target_ref = git_text(&root, &["symbolic-ref", "-q", "HEAD"])?;
    if target_ref != preflight.target_ref {
        return Err("checked-out Git branch changed after Vela preflight".to_string());
    }
    let current_head = git_text(&root, &["rev-parse", "HEAD^{commit}"])?;
    if current_head != preflight.expected_head {
        return Err(format!(
            "Git HEAD changed after Vela preflight (expected {}, observed {current_head})",
            preflight.expected_head
        ));
    }
    ensure_delta_paths_unstaged(&root, delta)?;
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
        &root,
        &index,
        &["read-tree", &preflight.expected_head],
        None,
    )?;

    for entry in &delta.entries {
        match &entry.postimage {
            Some(bytes) => {
                let blob = git_with_index(
                    &root,
                    &index,
                    &["hash-object", "-w", "--stdin"],
                    Some(bytes),
                )?;
                let mode = if entry.executable { "100755" } else { "100644" };
                git_with_index(
                    &root,
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
            }
            None => {
                git_with_index(
                    &root,
                    &index,
                    &["update-index", "--force-remove", "--", &entry.path],
                    None,
                )?;
            }
        }
    }
    let tree = git_with_index(&root, &index, &["write-tree"], None)?;
    let parent_tree = git_text(&root, &["rev-parse", "HEAD^{tree}"])?;
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
        &root,
        &index,
        &["commit-tree", &tree, "-p", &preflight.expected_head],
        Some(message.as_bytes()),
    )?;
    git_text(
        &root,
        &[
            "update-ref",
            "-m",
            summary,
            &preflight.target_ref,
            &commit,
            &preflight.expected_head,
        ],
    )?;
    let mut reset_args = vec!["reset", "--quiet", "HEAD", "--"];
    reset_args.extend(delta.entries.iter().map(|entry| entry.path.as_str()));
    git_text(&root, &reset_args).map_err(|error| {
        format!(
            "created local Vela commit {commit}, but could not refresh the caller index: {error}"
        )
    })?;
    Ok(PublicationOutcome {
        state: PublicationState::CommittedLocal { commit },
    })
}

fn ensure_delta_paths_unstaged(root: &Path, delta: &PublicationDelta) -> Result<(), String> {
    let mut args = vec!["diff", "--cached", "--quiet", "HEAD", "--"];
    args.extend(delta.entries.iter().map(|entry| entry.path.as_str()));
    let output = git_output(root, &args, None)?;
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

fn git_blob_sha256(root: &Path, commit: &str, path: &str) -> Result<Option<String>, String> {
    let spec = format!("{commit}:{path}");
    let output = git_output(root, &["show", &spec], None)?;
    if output.status.success() {
        return Ok(Some(sha256_bytes(&output.stdout)));
    }
    let missing = git_output(root, &["cat-file", "-e", &spec], None)?;
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

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_output(root, args, None)?;
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
    root: &Path,
    index: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<String, String> {
    let output = git_output_with_index(root, index, args, stdin)?;
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

fn git_output(
    root: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    run_git(root, None, args, stdin)
}

fn git_output_with_index(
    root: &Path,
    index: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    run_git(root, Some(index), args, stdin)
}

fn run_git(
    root: &Path,
    index: Option<&Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("-c")
        .arg("core.hooksPath=/dev/null")
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn exact_commit_uses_isolated_index_and_preserves_unrelated_dirt() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Vela Test"]);
        git(root, &["config", "user.email", "vela@example.invalid"]);
        std::fs::create_dir(root.join("repository")).unwrap();
        std::fs::write(root.join("repository/a.txt"), b"before").unwrap();
        std::fs::write(root.join("unrelated.txt"), b"clean").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);

        std::fs::write(root.join("unrelated.txt"), b"staged").unwrap();
        git(root, &["add", "unrelated.txt"]);

        let delta = PublicationDelta {
            root: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            entries: vec![PublicationDeltaEntry {
                path: "repository/a.txt".into(),
                preimage_sha256: Some(sha256_bytes(b"before")),
                postimage: Some(b"after".to_vec()),
                executable: false,
            }],
        };
        let options = PublishOptions::local();
        let preflight =
            exact_publication_preflight(&root.join("repository"), &delta, &options).unwrap();
        std::fs::write(root.join("repository/a.txt"), b"after").unwrap();
        std::fs::write(root.join("unrelated.txt"), b"dirty-after-stage").unwrap();
        let outcome = publish_exact_delta(
            &root.join("repository"),
            "retain Submission",
            &["vsb_test".into()],
            &delta,
            preflight,
        )
        .unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::CommittedLocal { .. }
        ));
        assert_eq!(
            git_text(root, &["show", "HEAD:repository/a.txt"]).unwrap(),
            "after"
        );
        assert_eq!(
            git_text(root, &["show", "HEAD:unrelated.txt"]).unwrap(),
            "clean"
        );
        assert_eq!(
            std::fs::read(root.join("unrelated.txt")).unwrap(),
            b"dirty-after-stage"
        );
        assert_eq!(
            git_text(root, &["show", ":unrelated.txt"]).unwrap(),
            "staged"
        );
        assert_eq!(
            git_text(root, &["diff", "--cached", "--name-only"]).unwrap(),
            "unrelated.txt"
        );
    }

    #[test]
    fn staged_change_on_transaction_path_fails_preflight() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Vela Test"]);
        git(root, &["config", "user.email", "vela@example.invalid"]);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);

        std::fs::write(root.join("a.txt"), b"staged").unwrap();
        git(root, &["add", "a.txt"]);
        std::fs::write(root.join("a.txt"), b"before").unwrap();

        let delta = PublicationDelta {
            root: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            entries: vec![PublicationDeltaEntry {
                path: "a.txt".into(),
                preimage_sha256: Some(sha256_bytes(b"before")),
                postimage: Some(b"after".to_vec()),
                executable: false,
            }],
        };
        let error = exact_publication_preflight(root, &delta, &PublishOptions::local())
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
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.name", "Vela Test"]);
        git(root, &["config", "user.email", "vela@example.invalid"]);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);
        let delta = PublicationDelta {
            root: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            entries: vec![PublicationDeltaEntry {
                path: "a.txt".into(),
                preimage_sha256: Some(sha256_bytes(b"before")),
                postimage: Some(b"after".to_vec()),
                executable: false,
            }],
        };
        let options = PublishOptions::local();
        let preflight = exact_publication_preflight(root, &delta, &options).unwrap();
        std::fs::write(root.join("other.txt"), b"advance").unwrap();
        git(root, &["add", "other.txt"]);
        git(root, &["commit", "-m", "advance"]);
        std::fs::write(root.join("a.txt"), b"after").unwrap();
        let outcome = publish_exact_delta(root, "test", &[], &delta, preflight).unwrap();
        assert!(matches!(
            outcome.state,
            PublicationState::Uncommitted { .. }
        ));
    }
}
