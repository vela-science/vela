//! Shared, read-only Git subprocess boundary for repository inspection.
//!
//! `git -C <repo>` alone is not a trust boundary: inherited `GIT_*`
//! variables can redirect repository discovery, object lookup, the index,
//! replacement refs, shallow state, or configuration. Read-only callers must
//! start from [`command`] and add only explicit arguments.
//!
//! This module never initializes a repository. Repository creation remains an
//! explicit product operation; this boundary only inspects an existing path.

use std::path::Path;
use std::process::{Command, Output};

const NULL_DEVICE: &str = "/dev/null";

/// Construct a read-only Git command isolated from ambient Git configuration
/// and repository-redirection variables.
pub fn command(repo: &Path) -> Result<Command, String> {
    let repo = std::fs::canonicalize(repo)
        .map_err(|error| format!("resolve Git repository {}: {error}", repo.display()))?;
    let worktree = repo
        .to_str()
        .ok_or_else(|| format!("Git repository path is not UTF-8: {}", repo.display()))?;
    let mut command = Command::new("git");

    // Remove every inherited Git variable, including indexed
    // GIT_CONFIG_KEY_n / GIT_CONFIG_VALUE_n entries and future redirection
    // variables not yet known to Vela. Callers may add back only values they
    // own explicitly.
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

/// Run an explicitly supplied read-only Git operation.
pub fn output(repo: &Path, args: &[&str]) -> Result<Output, String> {
    command(repo)?
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

/// Return stdout for a successful read-only Git operation.
pub fn bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
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

    fn run(mut command: Command) {
        let output = command.output().expect("run Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
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
