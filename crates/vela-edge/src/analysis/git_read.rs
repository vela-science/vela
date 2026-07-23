//! Shared, read-only Git process boundary for derived repository checks.
//!
//! Repository verification must not inherit a caller's Git redirects,
//! executable helpers, replacement refs, or mutable global configuration.
//! Callers still choose the exact read-only plumbing command and validate its
//! output; this module supplies only the common process isolation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

pub(crate) fn hardened_command(repo: &Path, label: &str) -> Result<Command, String> {
    #[cfg(windows)]
    const NULL_DEVICE: &str = "NUL";
    #[cfg(not(windows))]
    const NULL_DEVICE: &str = "/dev/null";

    let repo = std::fs::canonicalize(repo).map_err(|error| format!("resolve {label}: {error}"))?;
    let worktree = repo
        .to_str()
        .ok_or_else(|| format!("{label} path is not UTF-8"))?;
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(|name| name.starts_with("GIT_")) {
            command.env_remove(name);
        }
    }
    command
        .arg("--no-pager")
        .arg("--no-optional-locks")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.worktree={worktree}"))
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
        .arg("-C")
        .arg(&repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", NULL_DEVICE)
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("LC_ALL", "C");
    Ok(command)
}

fn output(repo: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    hardened_command(repo, "Git repository")?
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

fn bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = output(repo, args)?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    mode: String,
    object: String,
    path: String,
}

fn validate_path(path: &str) -> Result<(), String> {
    let parsed = Path::new(path);
    if path.is_empty()
        || path.len() > 4_096
        || path.contains(['\n', '\r', '\0', '\\'])
        || path.chars().any(char::is_control)
        || parsed.is_absolute()
        || parsed
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "Git dirt check refuses non-normalized or unsafe path {path:?}"
        ));
    }
    Ok(())
}

fn parse_tree_entry(raw: &[u8]) -> Result<Entry, String> {
    let record = std::str::from_utf8(raw)
        .map_err(|error| format!("Git tree contains a non-UTF-8 path: {error}"))?;
    let (metadata, path) = record
        .split_once('\t')
        .ok_or_else(|| format!("malformed Git tree record {record:?}"))?;
    validate_path(path)?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(format!("malformed Git tree metadata {metadata:?}"));
    }
    Ok(Entry {
        mode: fields[0].to_string(),
        object: fields[2].to_string(),
        path: path.to_string(),
    })
}

fn parse_index_entry(raw: &[u8]) -> Result<Entry, String> {
    let record = std::str::from_utf8(raw)
        .map_err(|error| format!("Git index contains a non-UTF-8 path: {error}"))?;
    let (metadata, path) = record
        .split_once('\t')
        .ok_or_else(|| format!("malformed Git index record {record:?}"))?;
    validate_path(path)?;
    let fields = metadata.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 3 || fields[2] != "0" {
        return Err(format!(
            "Git index path {path:?} is conflicted or not an exact stage-0 entry"
        ));
    }
    Ok(Entry {
        mode: fields[0].to_string(),
        object: fields[1].to_string(),
        path: path.to_string(),
    })
}

fn entries(
    repo: &Path,
    args: &[&str],
    parser: fn(&[u8]) -> Result<Entry, String>,
) -> Result<BTreeMap<String, Entry>, String> {
    let mut parsed = BTreeMap::new();
    for raw in bytes(repo, args)?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let entry = parser(raw)?;
        if parsed.insert(entry.path.clone(), entry).is_some() {
            return Err("Git repository contains duplicate paths".to_string());
        }
    }
    Ok(parsed)
}

fn real_ancestors(repo: &Path, relative: &str) -> Result<bool, String> {
    let mut cursor = repo.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        cursor.push(component.as_os_str());
        match std::fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "inspect tracked worktree ancestor {}: {error}",
                    cursor.display()
                ));
            }
        }
    }
    Ok(true)
}

fn hash_regular_paths(repo: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = hardened_command(repo, "Git repository")?
        .args(["hash-object", "--no-filters", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("run filter-free worktree hashing: {error}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "open filter-free worktree hashing input".to_string())?;
        for path in paths {
            writeln!(stdin, "{path}")
                .map_err(|error| format!("write filter-free Git path {path:?}: {error}"))?;
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for filter-free worktree hashing: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("filter-free worktree hashing failed with {}", output.status)
        } else {
            stderr
        });
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("filter-free worktree hashes were not UTF-8: {error}"))?;
    let hashes = stdout.lines().map(str::to_string).collect::<Vec<_>>();
    if hashes.len() != paths.len() {
        return Err(format!(
            "filter-free worktree hashing returned {} roots for {} paths",
            hashes.len(),
            paths.len()
        ));
    }
    Ok(hashes)
}

#[cfg(unix)]
fn symlink_target_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    std::fs::read_link(path)
        .map(|target| target.as_os_str().as_bytes().to_vec())
        .map_err(|error| format!("read tracked symlink {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn symlink_target_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let target = std::fs::read_link(path)
        .map_err(|error| format!("read tracked symlink {}: {error}", path.display()))?;
    target
        .to_str()
        .map(|target| target.as_bytes().to_vec())
        .ok_or_else(|| format!("tracked symlink {} is not UTF-8", path.display()))
}

/// Return every tracked/index/worktree mismatch and, when requested, every
/// ignored-aware untracked path without invoking `git status`, fsmonitor, or
/// repository-configured clean filters.
pub fn dirty_worktree_paths(repo: &Path, include_untracked: bool) -> Result<Vec<String>, String> {
    let repo = std::fs::canonicalize(repo)
        .map_err(|error| format!("resolve Git repository for dirt check: {error}"))?;
    let head = entries(
        &repo,
        &["ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        parse_tree_entry,
    )?;
    let index = entries(&repo, &["ls-files", "--stage", "-z"], parse_index_entry)?;
    let mut dirty = BTreeSet::new();
    for path in head.keys().chain(index.keys()) {
        if head.get(path) != index.get(path) {
            dirty.insert(path.clone());
        }
    }

    let mut regular = Vec::new();
    for entry in index.values() {
        if !real_ancestors(&repo, &entry.path)? {
            dirty.insert(entry.path.clone());
            continue;
        }
        let path = repo.join(&entry.path);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                dirty.insert(entry.path.clone());
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "inspect tracked worktree path {}: {error}",
                    path.display()
                ));
            }
        };
        match entry.mode.as_str() {
            "100644" | "100755" if metadata.is_file() && !metadata.file_type().is_symlink() => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let actual = if metadata.permissions().mode() & 0o111 == 0 {
                        "100644"
                    } else {
                        "100755"
                    };
                    if actual != entry.mode {
                        dirty.insert(entry.path.clone());
                        continue;
                    }
                }
                regular.push(entry.path.clone());
            }
            "120000" if metadata.file_type().is_symlink() => {
                let expected = bytes(&repo, &["cat-file", "blob", &entry.object])?;
                if symlink_target_bytes(&path)? != expected {
                    dirty.insert(entry.path.clone());
                }
            }
            _ => {
                // Gitlinks and special/replaced filesystem objects are never
                // accepted as a clean trust-bound repository checkout.
                dirty.insert(entry.path.clone());
            }
        }
    }
    for (path, object) in regular.iter().zip(hash_regular_paths(&repo, &regular)?) {
        if index.get(path).is_none_or(|entry| entry.object != object) {
            dirty.insert(path.clone());
        }
    }

    if include_untracked {
        for raw in bytes(
            &repo,
            &["ls-files", "--others", "--exclude-standard", "-z", "--"],
        )?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        {
            let path = std::str::from_utf8(raw)
                .map_err(|_| "Git dirt check refuses non-UTF-8 untracked paths".to_string())?;
            validate_path(path)?;
            dirty.insert(path.to_string());
        }
    }
    Ok(dirty.into_iter().collect())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn run(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn dirt_check_never_executes_repository_helpers_and_detects_exact_changes() {
        let repo = tempfile::tempdir().unwrap();
        run(repo.path(), &["init", "-q", "-b", "main"]);
        run(repo.path(), &["config", "user.name", "Vela Test"]);
        run(
            repo.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        std::fs::write(
            repo.path().join(".gitattributes"),
            "tracked.txt filter=hostile\n",
        )
        .unwrap();
        std::fs::write(repo.path().join("tracked.txt"), "exact\n").unwrap();
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-qm", "fixture"]);

        let helpers = tempfile::tempdir().unwrap();
        let fsmonitor_canary = helpers.path().join("fsmonitor-ran");
        let filter_canary = helpers.path().join("filter-ran");
        let fsmonitor = helpers.path().join("fsmonitor.sh");
        let filter = helpers.path().join("filter.sh");
        std::fs::write(
            &fsmonitor,
            format!(
                "#!/bin/sh\ntouch '{}'\nprintf '0\\n'\n",
                fsmonitor_canary.display()
            ),
        )
        .unwrap();
        std::fs::write(
            &filter,
            format!("#!/bin/sh\ntouch '{}'\ncat\n", filter_canary.display()),
        )
        .unwrap();
        std::fs::set_permissions(&fsmonitor, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&filter, std::fs::Permissions::from_mode(0o755)).unwrap();
        run(
            repo.path(),
            &["config", "core.fsmonitor", fsmonitor.to_str().unwrap()],
        );
        run(
            repo.path(),
            &["config", "filter.hostile.clean", filter.to_str().unwrap()],
        );

        assert!(dirty_worktree_paths(repo.path(), true).unwrap().is_empty());
        assert!(!fsmonitor_canary.exists());
        assert!(!filter_canary.exists());

        std::fs::write(repo.path().join("tracked.txt"), "changed\n").unwrap();
        std::fs::write(repo.path().join("untracked.txt"), "new\n").unwrap();
        assert_eq!(
            dirty_worktree_paths(repo.path(), true).unwrap(),
            vec!["tracked.txt".to_string(), "untracked.txt".to_string()]
        );
        assert_eq!(
            dirty_worktree_paths(repo.path(), false).unwrap(),
            vec!["tracked.txt".to_string()]
        );
        assert!(!fsmonitor_canary.exists());
        assert!(!filter_canary.exists());
    }
}
