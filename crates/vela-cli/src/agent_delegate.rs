//! Experimental porcelain for one removable bounded-evidence executor.
//!
//! The helper remains a separate process so none of its runner, model, or
//! verifier dependencies enter deterministic Standing replay. This delegator
//! deliberately exposes no Submission registration, Verification import,
//! review, repository authority, or campaign-host operation.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

const HELPER_ENV: &str = "VELA_AGENT_BIN";
const ALLOWED_ACTIONS: [&str; 5] = ["doctor", "run", "show", "replay", "export"];

fn is_allowed_action(action: &str) -> bool {
    ALLOWED_ACTIONS.contains(&action)
}

fn is_sensitive_environment(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name.starts_with("SSH_")
        || name.starts_with("VELA_REPOSITORY_AUTHORITY")
        || matches!(
            name,
            "VELA_AGENT_BIN"
                | "VELA_AGENT_KEY_HEX"
                | "VELA_KEY_PATH"
                | "VELA_AUTHORITY_KEY"
                | "VELA_HUMAN_KEY"
        )
}

fn resolve_helper(raw: Option<OsString>, current_exe: &Path) -> Result<PathBuf, String> {
    let raw = raw.ok_or_else(|| {
        format!(
            "{HELPER_ENV} is not set; point it to the canonical absolute path of the optional Vela Agent helper"
        )
    })?;
    let supplied = PathBuf::from(raw);
    if !supplied.is_absolute() {
        return Err(format!("{HELPER_ENV} must be an absolute path"));
    }
    let helper = std::fs::canonicalize(&supplied)
        .map_err(|error| format!("resolve {HELPER_ENV} {}: {error}", supplied.display()))?;
    let metadata = std::fs::metadata(&helper)
        .map_err(|error| format!("inspect {HELPER_ENV} {}: {error}", helper.display()))?;
    if !metadata.is_file() {
        return Err(format!("{HELPER_ENV} must name a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!("{HELPER_ENV} helper is not executable"));
        }
    }
    #[cfg(windows)]
    if !helper
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(format!(
            "{HELPER_ENV} must name a native .exe on Windows; shell-backed npm and Bun shims are not accepted"
        ));
    }
    let vela = std::fs::canonicalize(current_exe)
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    if helper == vela {
        return Err(format!("{HELPER_ENV} cannot point back to the Vela binary"));
    }
    Ok(helper)
}

fn status_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

fn run(action: &str, args: &[OsString]) -> Result<i32, String> {
    if !is_allowed_action(action) {
        return Err(format!(
            "unsupported Vela Agent action '{action}'; expected {}",
            ALLOWED_ACTIONS.join(", ")
        ));
    }
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    let helper = resolve_helper(std::env::var_os(HELPER_ENV), &current_exe)?;
    let vela = std::fs::canonicalize(&current_exe)
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    let mut command = Command::new(&helper);
    command.arg(action).args(args);
    // This avoids accidental credential forwarding; it is not an OS sandbox.
    // The optional helper is a trusted local controller. Its worker and
    // verifier must enforce the actual filesystem/network custody boundary.
    for (name, _) in std::env::vars_os() {
        if is_sensitive_environment(&name) {
            command.env_remove(name);
        }
    }
    command.env("VELA_BIN", vela).env("VELA_NO_KEY_ACCESS", "1");
    let status = command
        .status()
        .map_err(|error| format!("launch Vela Agent helper {}: {error}", helper.display()))?;
    Ok(status_code(status))
}

pub(crate) fn cmd_agent(action: &str, args: &[OsString]) -> ! {
    match run(action, args) {
        Ok(code) => std::process::exit(code),
        Err(error) => crate::cli::fail(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_allowlist_excludes_authority_operations() {
        for action in ALLOWED_ACTIONS {
            assert!(is_allowed_action(action));
        }
        for action in [
            "submit",
            "verification",
            "review",
            "accept",
            "reject",
            "campaign",
        ] {
            assert!(!is_allowed_action(action));
        }
    }

    #[test]
    fn sensitive_environment_is_narrow_and_explicit() {
        assert!(is_sensitive_environment(OsStr::new("SSH_AUTH_SOCK")));
        assert!(is_sensitive_environment(OsStr::new(
            "VELA_REPOSITORY_AUTHORITY_SOCKET"
        )));
        assert!(is_sensitive_environment(OsStr::new("VELA_KEY_PATH")));
        assert!(!is_sensitive_environment(OsStr::new("CODEX_HOME")));
        assert!(!is_sensitive_environment(OsStr::new("PATH")));
    }
}
