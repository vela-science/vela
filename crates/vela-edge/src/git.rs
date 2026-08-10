//! Shared Git process isolation and read-only repository inspection.
//!
//! An ordinary Git subprocess is not a trust boundary: inherited `GIT_*`
//! variables and process configuration can redirect repository discovery,
//! object lookup, the index, replacement refs, shallow state, or hooks. The
//! write-neutral [`isolate_ambient`] helper removes that shared ambient state;
//! [`command`] adds the repository binding required by read-only callers.
//!
//! Repository initialization and publication remain explicit product
//! operations. This module does not choose or execute either operation.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

const NULL_DEVICE: &str = "/dev/null";

/// Remove hostile ambient Git state and install the process settings shared
/// by read and write callers.
pub fn isolate_ambient(command: &mut Command) {
    isolate_ambient_from(command, std::env::vars_os().map(|(name, _)| name));
}

fn isolate_ambient_from(
    command: &mut Command,
    inherited_environment: impl IntoIterator<Item = OsString>,
) {
    for name in inherited_environment {
        if name.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(name);
        }
    }
    command
        .args([
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "-c",
            "core.bare=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "diff.external=",
            "-c",
            "submodule.recurse=false",
            "-c",
            "protocol.file.allow=never",
        ])
        .envs([
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_SYSTEM", NULL_DEVICE),
            ("GIT_CONFIG_GLOBAL", NULL_DEVICE),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_PAGER", "cat"),
            ("PAGER", "cat"),
            ("LC_ALL", "C"),
        ]);
}

/// Construct a read-only Git command isolated from ambient Git configuration
/// and repository-redirection variables.
pub(crate) fn command(repo: &Path) -> Result<Command, String> {
    let repo = std::fs::canonicalize(repo)
        .map_err(|error| format!("resolve Git repository {}: {error}", repo.display()))?;
    let worktree = repo
        .to_str()
        .ok_or_else(|| format!("Git repository path is not UTF-8: {}", repo.display()))?;
    let mut command = Command::new("git");
    isolate_ambient(&mut command);
    command
        .arg("-c")
        .arg(format!("core.worktree={worktree}"))
        .arg("-C")
        .arg(&repo);
    Ok(command)
}

/// Run an explicitly supplied read-only Git operation.
pub fn output(repo: &Path, args: &[&str]) -> Result<Output, String> {
    command(repo)?
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

/// Return stdout for a successful read-only Git operation.
pub(crate) fn bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
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

/// Return trimmed UTF-8 stdout for a successful read-only Git operation.
pub fn text(repo: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(bytes(repo, args)?)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("git {} output was not UTF-8: {error}", args.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};

    fn run(mut command: Command) {
        let output = command.output().expect("run Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn ambient_isolation_removes_the_git_namespace_and_sets_only_common_state() {
        let mut inherited = vec![
            OsString::from("GIT_DIR"),
            OsString::from("GIT_WORK_TREE"),
            OsString::from("GIT_INDEX_FILE"),
            OsString::from("GIT_CONFIG_KEY_0"),
            OsString::from("GIT_NO_LAZY_FETCH"),
            OsString::from("GIT_EDITOR"),
        ];
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;

            let name = OsString::from_vec(b"GIT_\xff".to_vec());
            inherited.push(name);
        }
        let mut command = Command::new("git");
        isolate_ambient_from(&mut command, inherited.iter().cloned());
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<BTreeMap<_, _>>();

        for name in inherited {
            assert_eq!(
                environment.get(&name),
                Some(&None),
                "inherited Git environment was not removed: {name:?}"
            );
        }
        for (name, value) in [
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_PAGER", "cat"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("LC_ALL", "C"),
            ("PAGER", "cat"),
        ] {
            assert_eq!(
                environment.get(OsStr::new(name)),
                Some(&Some(OsString::from(value))),
                "missing common Git environment: {name}"
            );
        }
        for required in [
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "core.bare=false",
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "core.attributesFile=/dev/null",
            "core.excludesFile=/dev/null",
            "diff.external=",
            "submodule.recurse=false",
            "protocol.file.allow=never",
        ] {
            assert!(
                command.get_args().any(|arg| arg == required),
                "missing common Git argument: {required}"
            );
        }
        assert!(!command.get_args().any(|arg| arg == "--literal-pathspecs"));
        assert!(!command.get_args().any(|arg| arg == "commit.gpgSign=false"));
    }

    #[test]
    fn reads_only_the_explicit_initialized_repository() {
        let repo = tempfile::tempdir().expect("temporary repository");
        let unrelated = tempfile::tempdir().expect("unrelated repository");
        run({
            let mut command = Command::new("git");
            command.args(["init", "--quiet"]).current_dir(repo.path());
            command
        });
        run({
            let mut command = Command::new("git");
            command
                .args(["init", "--quiet"])
                .current_dir(unrelated.path());
            command
        });

        let resolved = text(repo.path(), &["rev-parse", "--show-toplevel"])
            .expect("inspect initialized repository");
        assert_eq!(
            std::fs::canonicalize(resolved).expect("canonical Git output"),
            std::fs::canonicalize(repo.path()).expect("canonical repository")
        );
    }

    #[test]
    fn does_not_implicitly_initialize_a_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let error = text(directory.path(), &["rev-parse", "--git-dir"])
            .expect_err("plain directory must not become a repository");
        assert!(error.contains("not a git repository"), "{error}");
        assert!(!directory.path().join(".git").exists());
    }
}
