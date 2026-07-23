//! Fixed Git subprocess environment for trust-bound repository operations.
//!
//! `git -C <repo>` does not select `repo` when inherited `GIT_*` variables
//! redirect repository discovery, object lookup, the index, replacement refs,
//! shallow state, or configuration. Trust-bound callers must start from this
//! command and add only their explicit arguments and narrowly scoped
//! operation-owned variables.

use std::path::Path;
use std::process::{Command, Output};

#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

/// Construct a Git command that can inspect only the repository named by
/// `repo`, subject to that repository's ordinary on-disk object database.
pub(crate) fn command(repo: &Path) -> Command {
    let mut command = Command::new("git");

    // Remove every inherited Git variable, including indexed
    // GIT_CONFIG_KEY_n / GIT_CONFIG_VALUE_n entries and future redirection
    // variables not yet known to Vela. Callers may add back only values they
    // own explicitly (for example a transaction-private index).
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
        .arg(repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("LC_ALL", "C");
    command
}

pub(crate) fn output(repo: &Path, args: &[&str]) -> Result<Output, String> {
    command(repo)
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))
}

pub(crate) fn text(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = output(repo, args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {} failed with {}", args.join(" "), output.status)
        } else {
            stderr
        });
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("git {} output was not UTF-8: {error}", args.join(" ")))
}
