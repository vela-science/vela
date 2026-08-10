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

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::canonical::sha256_root;
use vela_repository::{ValidatedPrivateResidue, ValidatedPrivateResidueKind};

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
) -> Result<PublicationOutcome, String> {
    let actual_sha256 = publication_delta_sha256(delta);
    if actual_sha256 != preflight.delta_sha256 {
        return Err(format!(
            "exact publication delta changed after preflight (expected {}, got {actual_sha256})",
            preflight.delta_sha256
        ));
    }
    Ok(
        publish(repository_path, summary, object_ids, delta, &preflight)
            .unwrap_or_else(PublicationOutcome::uncommitted),
    )
}

pub(crate) fn initialize_native_git_repository(path: &Path) -> Result<(), String> {
    let root = path
        .canonicalize()
        .map_err(|error| format!("canonicalize native Git target: {error}"))?;
    if !root.is_dir() || root.join(".git").exists() {
        return Err("native Git initialization requires a real uninitialized directory".into());
    }
    let templates = tempfile::tempdir()
        .map_err(|error| format!("create empty trusted Git template directory: {error}"))?;
    let mut command = Command::new("git");
    vela_edge::git::isolate_ambient(&mut command);
    command
        .current_dir(&root)
        .arg("--literal-pathspecs")
        .arg("-c")
        .arg("commit.gpgSign=false")
        .arg("-c")
        .arg(format!("init.templateDir={}", templates.path().display()))
        .args(["init", "--quiet", "--object-format=sha1", "--template"])
        .arg(templates.path())
        .args(["-b", "main", "--"])
        .arg(&root)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_CEILING_DIRECTORIES", &root);
    let output = command
        .output()
        .map_err(|error| format!("run isolated git init: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "isolated git init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    verify_empty_native_git_repository(&root)
}

pub(crate) fn verify_empty_native_git_repository(path: &Path) -> Result<(), String> {
    let root = path
        .canonicalize()
        .map_err(|error| format!("canonicalize native Git target: {error}"))?;
    let metadata = std::fs::symlink_metadata(root.join(".git"))
        .map_err(|error| format!("inspect initialized Git directory: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("isolated git init did not create one ordinary .git directory".into());
    }
    let repository = resolve_git_repository(&root)?;
    if repository.worktree != root
        || repository.git_dir != root.join(".git")
        || repository.git_common_dir != repository.git_dir
        || repository.object_dir != repository.git_dir.join("objects")
        || repository.object_format != "sha1"
        || git_text_in(&repository, &["symbolic-ref", "-q", "HEAD"])? != "refs/heads/main"
        || !git_text_in(
            &repository,
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        )?
        .is_empty()
        || !git_output_in(&repository, &["ls-files", "--stage", "-z"], None)?
            .stdout
            .is_empty()
    {
        return Err("isolated git init produced an unexpected repository shape".into());
    }
    let config = git_text_in(
        &repository,
        &["config", "--local", "--no-includes", "--list"],
    )?;
    for entry in config.lines() {
        let Some((name, value)) = entry.split_once('=') else {
            return Err("isolated git init produced malformed local configuration".into());
        };
        let valid = match name.to_ascii_lowercase().as_str() {
            "core.repositoryformatversion" => value == "0",
            "core.filemode" => value == "true",
            "core.bare" => value == "false",
            "core.logallrefupdates" => value == "true",
            "core.ignorecase" | "core.precomposeunicode" => value == "true" || value == "false",
            _ => false,
        };
        if !valid {
            return Err(format!(
                "isolated git init produced unexpected local configuration {name}={value}"
            ));
        }
    }
    for relative in ["HEAD", "config"] {
        let metadata = std::fs::symlink_metadata(repository.git_dir.join(relative))
            .map_err(|error| format!("inspect initialized Git {relative}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "isolated git init {relative} must be a regular non-symlink file"
            ));
        }
    }
    let count = git_text_in(&repository, &["count-objects", "-v"])?;
    let no_objects = count.lines().all(|line| {
        let Some((name, value)) = line.split_once(": ") else {
            return false;
        };
        matches!(
            name,
            "count"
                | "size"
                | "in-pack"
                | "packs"
                | "size-pack"
                | "prune-packable"
                | "garbage"
                | "size-garbage"
        ) && value == "0"
    });
    if !no_objects {
        return Err("isolated git init contains unexpected Git objects or garbage".into());
    }
    Ok(())
}

/// Exact local publication of the one parentless native-repository genesis.
///
/// Unlike an ordinary repository transaction, genesis has no parent commit to
/// preflight. The caller supplies the complete closed path set retained by the
/// recovered sequence-one record. This function builds the tree through raw
/// Git plumbing, so checkout attributes, filters, hooks, and ambient Git
/// configuration never get to rewrite canonical bytes or redirect the ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeGenesisPublication {
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeGenesisFile {
    path: String,
    bytes: Vec<u8>,
}

fn validate_native_genesis_files(
    root: &Path,
    paths: &[String],
) -> Result<Vec<NativeGenesisFile>, String> {
    let mut previous: Option<&str> = None;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        validate_relative_path(path)?;
        if previous.is_some_and(|previous| previous >= path.as_str()) {
            return Err("native genesis paths must be strictly sorted and unique".into());
        }
        previous = Some(path);
        let absolute = root.join(path);
        let metadata = std::fs::symlink_metadata(&absolute)
            .map_err(|error| format!("inspect native genesis path {path}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "native genesis path {path} must be a regular non-symlink file"
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 != 0 {
                return Err(format!(
                    "native genesis path {path} must have regular mode 100644"
                ));
            }
        }
        files.push(NativeGenesisFile {
            path: path.clone(),
            bytes: std::fs::read(&absolute)
                .map_err(|error| format!("read native genesis path {path}: {error}"))?,
        });
    }
    Ok(files)
}

fn build_native_genesis_tree(
    repository: &GitRepository,
    index: &Path,
    object_directory: Option<&Path>,
    files: &[NativeGenesisFile],
) -> Result<(String, Vec<u8>), String> {
    git_with_index_and_objects(
        repository,
        index,
        object_directory,
        &["read-tree", "--empty"],
        None,
    )?;
    for file in files {
        let blob = git_with_index_and_objects(
            repository,
            index,
            object_directory,
            &["hash-object", "-w", "--stdin"],
            Some(&file.bytes),
        )?;
        git_with_index_and_objects(
            repository,
            index,
            object_directory,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "100644",
                &blob,
                &file.path,
            ],
            None,
        )?;
    }
    let tree =
        git_with_index_and_objects(repository, index, object_directory, &["write-tree"], None)?;
    let expected_index = run_git_owned(
        repository,
        Some(index),
        object_directory,
        &["ls-files", "--stage", "-z"],
        None,
    )?;
    if !expected_index.status.success() {
        return Err("read exact native genesis temporary index failed".into());
    }
    Ok((tree, expected_index.stdout))
}

pub(crate) fn publish_native_genesis(
    repository_path: &Path,
    paths: &[String],
    private_residue: &[ValidatedPrivateResidue],
    recorded_at: &str,
    create_if_missing: bool,
) -> Result<NativeGenesisPublication, String> {
    let supplied_time = recorded_at;
    let recorded_at = chrono::DateTime::parse_from_rfc3339(supplied_time)
        .map_err(|error| format!("native genesis time is invalid: {error}"))?
        .with_timezone(&chrono::Utc);
    let canonical_time = recorded_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if canonical_time != supplied_time {
        return Err("native genesis time is not canonical to whole seconds".into());
    }
    let repository = resolve_git_repository(repository_path)?;
    let requested_root = repository_path
        .canonicalize()
        .map_err(|error| format!("canonicalize native genesis repository: {error}"))?;
    if repository.worktree != requested_root
        || repository.git_dir != requested_root.join(".git")
        || repository.git_common_dir != repository.git_dir
    {
        return Err(
            "native genesis requires one ordinary Git worktree rooted at the repository".into(),
        );
    }
    if git_text_in(&repository, &["symbolic-ref", "-q", "HEAD"])? != "refs/heads/main" {
        return Err("native genesis requires the unborn or exact checked-out main branch".into());
    }

    ensure_closed_native_genesis_worktree(&requested_root, paths, private_residue)?;
    let files = validate_native_genesis_files(&requested_root, paths)?;
    let temporary =
        tempfile::tempdir().map_err(|error| format!("Git preflight tempdir: {error}"))?;
    let object_directory = temporary.path().join("objects");
    std::fs::create_dir_all(object_directory.join("info"))
        .and_then(|_| std::fs::create_dir_all(object_directory.join("pack")))
        .map_err(|error| format!("create isolated Git object preflight: {error}"))?;
    let expected_index_path = temporary.path().join("native-genesis-expected-index");
    let (tree, expected_index) = build_native_genesis_tree(
        &repository,
        &expected_index_path,
        Some(&object_directory),
        &files,
    )?;

    let timestamp = recorded_at.timestamp();
    let commit_bytes = format!(
        "tree {tree}\nauthor Vela Agent <agent@vela.space> {timestamp} +0000\ncommitter Vela Agent <agent@vela.space> {timestamp} +0000\n\nInitialize current Vela repository\n"
    );
    let expected_commit = git_text_in_with_input_and_objects(
        &repository,
        Some(&object_directory),
        &["hash-object", "-t", "commit", "--stdin"],
        commit_bytes.as_bytes(),
    )?;
    let refs = git_text_in(
        &repository,
        &["for-each-ref", "--format=%(refname)%00%(objectname)"],
    )?;
    let expected_ref = format!("refs/heads/main\0{expected_commit}");
    let current_index = git_output_in(&repository, &["ls-files", "--stage", "-z"], None)?;
    if !current_index.status.success() {
        return Err("native genesis Git index contains conflicting staged state".into());
    }
    let created = if refs.is_empty() {
        if !create_if_missing {
            return Err("native genesis trust exists but its exact Git commit is absent".into());
        }
        if !current_index.stdout.is_empty()
            && current_index.stdout.as_slice() != expected_index.as_slice()
        {
            return Err("native genesis Git index contains conflicting staged state".into());
        }
        if validate_native_genesis_files(&requested_root, paths)? != files {
            return Err("native genesis file bytes changed during preflight".into());
        }
        ensure_closed_native_genesis_worktree(&requested_root, paths, private_residue)?;
        if !git_text_in(
            &repository,
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        )?
        .is_empty()
        {
            return Err("native genesis Git refs changed during publication".into());
        }
        let actual_index_path = temporary.path().join("native-genesis-write-index");
        let (written_tree, written_index) =
            build_native_genesis_tree(&repository, &actual_index_path, None, &files)?;
        if written_tree != tree || written_index != expected_index {
            return Err("native genesis Git objects changed while writing".into());
        }
        let written = git_text_in_with_input(
            &repository,
            &["hash-object", "-w", "-t", "commit", "--stdin"],
            commit_bytes.as_bytes(),
        )?;
        if written != expected_commit {
            return Err("native genesis commit object changed while writing".into());
        }
        if validate_native_genesis_files(&requested_root, paths)? != files {
            return Err("native genesis file bytes changed before ref publication".into());
        }
        ensure_closed_native_genesis_worktree(&requested_root, paths, private_residue)?;
        let observed_index = git_output_in(&repository, &["ls-files", "--stage", "-z"], None)?;
        if !git_text_in(
            &repository,
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        )?
        .is_empty()
            || !observed_index.status.success()
            || observed_index.stdout.as_slice() != current_index.stdout.as_slice()
        {
            return Err("native genesis Git ref or index changed before publication".into());
        }
        #[cfg(feature = "test-support")]
        if std::env::var_os("VELA_TEST_INTERRUPT_INIT_BEFORE_GENESIS_REF").is_some() {
            std::process::exit(86);
        }
        let zero = "0".repeat(expected_commit.len());
        git_text_in(
            &repository,
            &[
                "update-ref",
                "-m",
                "Initialize current Vela repository",
                "refs/heads/main",
                &expected_commit,
                &zero,
            ],
        )?;
        if current_index.stdout.is_empty() {
            git_text_in(&repository, &["read-tree", &tree])?;
        }
        true
    } else if refs == expected_ref {
        if !current_index.stdout.is_empty() && current_index.stdout != expected_index {
            return Err("native genesis Git index contains conflicting staged state".into());
        }
        if current_index.stdout.is_empty() {
            if !create_if_missing {
                return Err("native genesis trust exists but its exact Git index is absent".into());
            }
            git_text_in(&repository, &["read-tree", &tree])?;
        }
        false
    } else {
        return Err("native genesis Git refs do not equal the one exact main-branch commit".into());
    };

    verify_native_genesis_git_state(
        &repository,
        &tree,
        &expected_commit,
        commit_bytes.as_bytes(),
        &expected_index,
        paths,
        private_residue,
    )?;
    Ok(NativeGenesisPublication {
        commit: expected_commit,
        tree,
        created,
    })
}

fn verify_native_genesis_git_state(
    repository: &GitRepository,
    expected_tree: &str,
    expected_commit: &str,
    expected_commit_bytes: &[u8],
    expected_index: &[u8],
    expected_paths: &[String],
    private_residue: &[ValidatedPrivateResidue],
) -> Result<(), String> {
    if git_text_in(repository, &["symbolic-ref", "-q", "HEAD"])? != "refs/heads/main"
        || git_text_in(repository, &["rev-parse", "HEAD^{commit}"])? != expected_commit
        || git_text_in(repository, &["rev-parse", "HEAD^{tree}"])? != expected_tree
    {
        return Err("native genesis Git HEAD, tree, or index is not exact".into());
    }
    let index = git_output_in(repository, &["ls-files", "--stage", "-z"], None)?;
    if !index.status.success() || index.stdout != expected_index {
        return Err("native genesis Git index is not exact".into());
    }
    let commit = git_output_in(repository, &["cat-file", "commit", expected_commit], None)?;
    if !commit.status.success() || commit.stdout != expected_commit_bytes {
        return Err("native genesis commit parents or metadata are not exact".into());
    }
    let refs = git_text_in(
        repository,
        &["for-each-ref", "--format=%(refname)%00%(objectname)"],
    )?;
    if refs != format!("refs/heads/main\0{expected_commit}") {
        return Err("native genesis Git refs do not equal the one exact main-branch commit".into());
    }
    ensure_closed_native_genesis_worktree(&repository.worktree, expected_paths, private_residue)
}

fn ensure_closed_native_genesis_worktree(
    root: &Path,
    paths: &[String],
    private_residue: &[ValidatedPrivateResidue],
) -> Result<(), String> {
    let expected = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut expected_private = BTreeMap::new();
    for residue in private_residue {
        let relative = format!(".vela/operation-journals/{}", residue.path().as_str());
        if expected.contains(relative.as_str())
            || expected_private
                .insert(relative.clone(), residue.kind())
                .is_some()
        {
            return Err(format!(
                "native genesis private recovery census repeats or overlaps {relative}"
            ));
        }
    }
    let mut seen_private = BTreeSet::new();
    fn visit(
        root: &Path,
        directory: &Path,
        expected: &BTreeSet<&str>,
        expected_private: &BTreeMap<String, ValidatedPrivateResidueKind>,
        seen_private: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        let mut entries = std::fs::read_dir(directory)
            .map_err(|error| {
                format!(
                    "read native genesis directory {}: {error}",
                    directory.display()
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read native genesis directory entry: {error}"))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "native genesis path escaped its repository".to_string())?
                .to_str()
                .ok_or_else(|| "native genesis contains a non-UTF-8 path".to_string())?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| format!("inspect native genesis path {relative}: {error}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "native genesis path {relative} must not be a symlink"
                ));
            }
            if relative == ".git" {
                if !metadata.is_dir() {
                    return Err("native genesis .git must be a real directory".into());
                }
                continue;
            }
            let private_kind = expected_private.get(&relative).copied();
            let private_root = relative == ".vela/operation-journals";
            if metadata.is_dir() {
                let prefix = format!("{relative}/");
                if matches!(private_kind, Some(ValidatedPrivateResidueKind::RegularFile)) {
                    return Err(format!(
                        "native genesis private recovery path {relative} changed filesystem kind"
                    ));
                }
                if private_kind == Some(ValidatedPrivateResidueKind::Directory) {
                    seen_private.insert(relative.clone());
                } else if !private_root
                    && !expected.iter().any(|path| path.starts_with(&prefix))
                    && !expected_private
                        .keys()
                        .any(|path| path.starts_with(&prefix))
                {
                    return Err(format!(
                        "native genesis contains unexpected directory {relative}"
                    ));
                }
                visit(root, &path, expected, expected_private, seen_private)?;
            } else if !metadata.is_file() {
                return Err(format!(
                    "native genesis path {relative} must be a regular file"
                ));
            } else if matches!(private_kind, Some(ValidatedPrivateResidueKind::Directory)) {
                return Err(format!(
                    "native genesis private recovery path {relative} changed filesystem kind"
                ));
            } else if private_kind == Some(ValidatedPrivateResidueKind::RegularFile) {
                seen_private.insert(relative.clone());
            } else if !expected.contains(relative.as_str()) {
                return Err(format!(
                    "native genesis contains unexpected file {relative}"
                ));
            }
        }
        Ok(())
    }
    visit(root, root, &expected, &expected_private, &mut seen_private)?;
    let expected_private_paths = expected_private.keys().cloned().collect::<BTreeSet<_>>();
    if seen_private != expected_private_paths {
        return Err("native genesis private recovery census changed after validation".into());
    }
    Ok(())
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
        let expected = entry.postimage.as_deref().map(sha256_root);
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
        return Ok(Some(sha256_root(&output.stdout)));
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
    Ok(sha256_root(&bytes))
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

fn git_text_in_with_input(
    repository: &GitRepository,
    args: &[&str],
    input: &[u8],
) -> Result<String, String> {
    let output = git_output_in(repository, args, Some(input))?;
    successful_git_text(output, args)
}

fn git_text_in_with_input_and_objects(
    repository: &GitRepository,
    object_directory: Option<&Path>,
    args: &[&str],
    input: &[u8],
) -> Result<String, String> {
    let output = run_git_owned(repository, None, object_directory, args, Some(input))?;
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
    let output = run_git_owned(repository, Some(index), None, args, stdin)?;
    successful_git_text(output, args)
}

fn git_output_in(
    repository: &GitRepository,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    run_git_owned(repository, None, None, args, stdin)
}

fn git_with_index_and_objects(
    repository: &GitRepository,
    index: &Path,
    object_directory: Option<&Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<String, String> {
    let output = run_git_owned(repository, Some(index), object_directory, args, stdin)?;
    successful_git_text(output, args)
}

fn run_git_owned(
    repository: &GitRepository,
    index: Option<&Path>,
    object_directory: Option<&Path>,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    let mut command = isolated_git_command(repository, index);
    if let Some(object_directory) = object_directory {
        command.env("GIT_OBJECT_DIRECTORY", object_directory);
    }
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
    let mut command = isolated_git_command(&repository, None);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

fn isolated_git_command(repository: &GitRepository, index: Option<&Path>) -> Command {
    let mut command = Command::new("git");
    vela_edge::git::isolate_ambient(&mut command);

    command
        .current_dir(&repository.worktree)
        .arg("--literal-pathspecs")
        .arg("--work-tree")
        .arg(&repository.worktree);
    if repository.git_dir != repository.worktree.join(".git") || repository.git_dir.is_dir() {
        command.arg("--git-dir").arg(&repository.git_dir);
    }
    command
        .arg("-c")
        .arg("commit.gpgSign=false")
        .arg("-C")
        .arg(&repository.worktree)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_CEILING_DIRECTORIES", &repository.worktree);
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};

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

    fn setup_native_genesis(root: &Path) -> Vec<String> {
        initialize_native_git_repository(root).unwrap();
        std::fs::write(root.join(".gitignore"), b"ignored\n").unwrap();
        std::fs::write(root.join("a.txt"), b"exact native genesis\n").unwrap();
        vec![".gitignore".into(), "a.txt".into()]
    }

    fn native_state(root: &Path) -> (String, Option<Vec<u8>>, BTreeMap<PathBuf, Vec<u8>>) {
        let refs = fixture_git_text(
            root,
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        );
        let index = std::fs::read(root.join(".git/index")).ok();
        let files = snapshot_files(root);
        (refs, index, files)
    }

    fn one_file_delta(path: &str) -> PublicationDelta {
        PublicationDelta {
            root: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            entries: vec![PublicationDeltaEntry {
                path: path.into(),
                preimage_sha256: Some(sha256_root(b"before")),
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

    #[test]
    fn native_genesis_commit_is_parentless_deterministic_and_idempotent() {
        let temporary = tempfile::tempdir().unwrap();
        let paths = setup_native_genesis(temporary.path());
        let first =
            publish_native_genesis(temporary.path(), &paths, &[], "2026-08-10T12:34:56Z", true)
                .unwrap();
        assert!(first.created);
        assert_eq!(
            fixture_git_text(temporary.path(), &["rev-parse", "HEAD"]),
            first.commit
        );
        assert_eq!(
            fixture_git_text(temporary.path(), &["rev-parse", "HEAD^{tree}"]),
            first.tree
        );
        let parents = fixture_git_text(temporary.path(), &["show", "-s", "--format=%P", "HEAD"]);
        assert!(parents.is_empty());
        let raw = Command::new("git")
            .current_dir(temporary.path())
            .args(["cat-file", "commit", "HEAD"])
            .output()
            .unwrap();
        assert!(raw.status.success());
        assert_eq!(
            raw.stdout,
            format!(
                "tree {}\nauthor Vela Agent <agent@vela.space> 1786365296 +0000\ncommitter Vela Agent <agent@vela.space> 1786365296 +0000\n\nInitialize current Vela repository\n",
                first.tree
            )
            .into_bytes()
        );
        let before = snapshot_files(temporary.path());
        let second =
            publish_native_genesis(temporary.path(), &paths, &[], "2026-08-10T12:34:56Z", false)
                .unwrap();
        assert_eq!(
            second,
            NativeGenesisPublication {
                commit: first.commit,
                tree: first.tree,
                created: false,
            }
        );
        assert_eq!(snapshot_files(temporary.path()), before);
    }

    #[cfg(unix)]
    #[test]
    fn native_genesis_refuses_mode_index_ref_and_closed_tree_drift_before_publication() {
        use std::os::unix::fs::PermissionsExt;

        let executable = tempfile::tempdir().unwrap();
        let paths = setup_native_genesis(executable.path());
        let mut permissions = std::fs::metadata(executable.path().join("a.txt"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(executable.path().join("a.txt"), permissions).unwrap();
        let before = native_state(executable.path());
        assert!(
            publish_native_genesis(executable.path(), &paths, &[], "2026-08-10T12:34:56Z", true,)
                .unwrap_err()
                .contains("100644")
        );
        assert_eq!(native_state(executable.path()), before);

        let ignored_extra = tempfile::tempdir().unwrap();
        let paths = setup_native_genesis(ignored_extra.path());
        std::fs::write(ignored_extra.path().join(".gitignore"), b"*\n").unwrap();
        std::fs::write(ignored_extra.path().join("hidden.txt"), b"must fail\n").unwrap();
        let before = native_state(ignored_extra.path());
        assert!(
            publish_native_genesis(
                ignored_extra.path(),
                &paths,
                &[],
                "2026-08-10T12:34:56Z",
                true,
            )
            .unwrap_err()
            .contains("unexpected file")
        );
        assert_eq!(native_state(ignored_extra.path()), before);

        let index_conflict = tempfile::tempdir().unwrap();
        let paths = setup_native_genesis(index_conflict.path());
        std::fs::write(index_conflict.path().join("a.txt"), b"staged conflict\n").unwrap();
        git(index_conflict.path(), &["add", "a.txt"]);
        std::fs::write(
            index_conflict.path().join("a.txt"),
            b"exact native genesis\n",
        )
        .unwrap();
        let before = native_state(index_conflict.path());
        assert!(
            publish_native_genesis(
                index_conflict.path(),
                &paths,
                &[],
                "2026-08-10T12:34:56Z",
                true,
            )
            .unwrap_err()
            .contains("conflicting staged state")
        );
        assert_eq!(native_state(index_conflict.path()), before);

        let ref_conflict = tempfile::tempdir().unwrap();
        let paths = setup_native_genesis(ref_conflict.path());
        git(ref_conflict.path(), &["config", "user.name", "Hostile"]);
        git(
            ref_conflict.path(),
            &["config", "user.email", "hostile@example.invalid"],
        );
        git(
            ref_conflict.path(),
            &["commit", "--allow-empty", "-m", "hostile"],
        );
        let before = native_state(ref_conflict.path());
        assert!(
            publish_native_genesis(
                ref_conflict.path(),
                &paths,
                &[],
                "2026-08-10T12:34:56Z",
                true,
            )
            .unwrap_err()
            .contains("refs do not equal")
        );
        assert_eq!(native_state(ref_conflict.path()), before);
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
    fn write_command_retains_only_cli_owned_extensions() {
        let temp = tempfile::tempdir().unwrap();
        setup_repository(temp.path());
        let repository = resolve_git_repository(temp.path()).unwrap();
        let index = temp.path().join("owned-index");
        let command = isolated_git_command(&repository, Some(&index));
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<BTreeMap<_, _>>();
        for (name, value) in [
            ("GIT_EDITOR", "true"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_SEQUENCE_EDITOR", "true"),
        ] {
            assert_eq!(
                environment.get(OsStr::new(name)),
                Some(&Some(OsString::from(value))),
                "missing CLI-owned Git environment: {name}"
            );
        }
        assert_eq!(
            environment.get(OsStr::new("GIT_INDEX_FILE")),
            Some(&Some(index.into_os_string()))
        );
        assert_eq!(
            environment.get(OsStr::new("GIT_CEILING_DIRECTORIES")),
            Some(&Some(repository.worktree.into_os_string()))
        );
        for required in ["--literal-pathspecs", "commit.gpgSign=false", "--work-tree"] {
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
            let hostile_init = root.join("hostile-init");
            initialize_native_git_repository(&hostile_init).unwrap();
            verify_empty_native_git_repository(&hostile_init).unwrap();
            let native = root.join("native-genesis");
            let paths = vec![".gitignore".into(), "a.txt".into()];
            publish_native_genesis(&native, &paths, &[], "2026-08-10T12:34:56Z", true).unwrap();
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
        std::fs::create_dir(intended.path().join("hostile-init")).unwrap();
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

        let native = intended.path().join("native-genesis");
        std::fs::create_dir(&native).unwrap();
        let native_paths = setup_native_genesis(&native);
        assert_eq!(native_paths, vec![".gitignore", "a.txt"]);
        std::fs::create_dir_all(native.join(".git/info")).unwrap();
        std::fs::create_dir_all(native.join(".git/hooks")).unwrap();
        std::fs::write(
            native.join(".git/info/attributes"),
            b"a.txt filter=hostile\n",
        )
        .unwrap();
        write_executable(
            &native.join(".git/hooks/reference-transaction"),
            &helper_source,
        );
        git(
            &native,
            &["config", "filter.hostile.clean", helper.to_str().unwrap()],
        );
        git(
            &native,
            &["config", "core.hooksPath", hooks.to_str().unwrap()],
        );
        git(
            &native,
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
        verify_empty_native_git_repository(&intended.path().join("hostile-init")).unwrap();
        assert_eq!(
            fixture_git_text(&native, &["show", "HEAD:a.txt"]),
            "exact native genesis"
        );
        assert!(fixture_git_text(&native, &["show", "-s", "--format=%P", "HEAD"]).is_empty());
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

    #[cfg(unix)]
    #[test]
    fn delta_transplant_after_preflight_leaves_git_and_worktree_byte_exact() {
        use std::os::unix::fs::MetadataExt;
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        setup_repository(root);
        std::fs::write(root.join("a.txt"), b"before").unwrap();
        write_executable(&root.join("unrelated.txt"), "sentinel");
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "initial"]);
        let mut delta = one_file_delta("a.txt");
        let preflight =
            exact_publication_preflight(root, &delta, &PublishOptions::local()).unwrap();
        delta.root = sha256_root(b"transplanted root");
        std::fs::write(root.join("a.txt"), b"after").unwrap();
        let mode = |path| std::fs::metadata(root.join(path)).unwrap().mode();
        let state = || (native_state(root), mode("a.txt"), mode("unrelated.txt"));
        let before = state();
        let error = publish_exact_delta(root, "transplant", &[], &delta, preflight).unwrap_err();
        assert_eq!(
            error,
            "exact publication delta changed after preflight (expected sha256:2f808e15404bd334942aac2328ffcde194a6c337a97da172f04e5333207e89b6, got sha256:4d504504e69689b5c843ad647f79045cf4b735e986bd697eaff9c66a15b58ef8)"
        );
        assert_eq!(state(), before);
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
