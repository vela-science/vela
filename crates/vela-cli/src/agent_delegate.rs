//! Experimental porcelain for one removable bounded-evidence executor.
//!
//! The helper remains a separate process so none of its runner, model, or
//! verifier dependencies enter deterministic Standing replay. This delegator
//! deliberately exposes no Submission registration, Verification import,
//! review, repository authority, or campaign-host operation.

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

const HELPER_ENV: &str = "VELA_AGENT_BIN";
const RUNTIME_ENV: &str = "VELA_AGENT_RUNTIME";
const BUN_VERSION: &str = "1.3.12";
const MAX_RUNTIME_BYTES: u64 = 512 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_HELPER_RUN_OUTPUT_BYTES: u64 = 64 * 1024;
const ALLOWED_ACTIONS: [&str; 5] = ["doctor", "run", "show", "replay", "export"];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct AgentHelperBuild {
    schema: &'static str,
    platform: String,
    runtime: AgentRuntimeFile,
    bundle: AgentBundleFile,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AgentRuntimeFile {
    kind: &'static str,
    version: &'static str,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AgentBundleFile {
    format: &'static str,
    size: u64,
    sha256: String,
}

impl AgentHelperBuild {
    pub(crate) fn root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(self)?
        ))
    }
}

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
                | "VELA_AGENT_RUNTIME"
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
    let vela = std::fs::canonicalize(current_exe)
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    if helper == vela {
        return Err(format!("{HELPER_ENV} cannot point back to the Vela binary"));
    }
    Ok(helper)
}

fn executable_from_path(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return std::fs::canonicalize(&candidate).map_err(|error| {
                format!("resolve Agent runtime {}: {error}", candidate.display())
            });
        }
    }
    Err(format!(
        "{RUNTIME_ENV} is not set and {name} was not found on PATH"
    ))
}

fn resolve_runtime(raw: Option<OsString>) -> Result<PathBuf, String> {
    let runtime = match raw {
        Some(raw) => {
            let supplied = PathBuf::from(raw);
            if !supplied.is_absolute() {
                return Err(format!("{RUNTIME_ENV} must be an absolute path"));
            }
            std::fs::canonicalize(&supplied)
                .map_err(|error| format!("resolve {RUNTIME_ENV} {}: {error}", supplied.display()))?
        }
        None => executable_from_path(if cfg!(windows) { "bun.exe" } else { "bun" })?,
    };
    let metadata = std::fs::metadata(&runtime)
        .map_err(|error| format!("inspect Agent runtime {}: {error}", runtime.display()))?;
    if !metadata.is_file() {
        return Err(format!("{RUNTIME_ENV} must name a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!("{RUNTIME_ENV} is not executable"));
        }
    }
    Ok(runtime)
}

fn file_identity(path: &Path, max_bytes: u64, label: &str) -> Result<(u64, String), String> {
    let size = std::fs::metadata(path)
        .map_err(|error| format!("inspect Agent {label} {}: {error}", path.display()))?
        .len();
    if size == 0 {
        return Err(format!("Agent {label} {} is empty", path.display()));
    }
    if size > max_bytes {
        return Err(format!(
            "Agent {label} {} is {size} bytes; limit is {max_bytes}",
            path.display()
        ));
    }
    let sha256 = crate::authority_transaction::execution_binary_sha256(path)?;
    Ok((size, sha256))
}

fn bun_platform(os: &str, architecture: &str) -> String {
    let os = match os {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let architecture = match architecture {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        "x86" => "ia32",
        other => other,
    };
    format!("{os}-{architecture}")
}

fn platform() -> String {
    bun_platform(std::env::consts::OS, std::env::consts::ARCH)
}

fn helper_build(runtime: &Path, helper: &Path) -> Result<AgentHelperBuild, String> {
    let output = Command::new(runtime)
        .arg("--version")
        .output()
        .map_err(|error| format!("probe Agent Bun runtime {}: {error}", runtime.display()))?;
    let observed = String::from_utf8(output.stdout)
        .map_err(|error| format!("Agent Bun version output is not UTF-8: {error}"))?;
    if !output.status.success() || !output.stderr.is_empty() || observed.trim() != BUN_VERSION {
        return Err(format!(
            "Vela Agent requires Bun {BUN_VERSION}; {} reported stdout={:?}",
            runtime.display(),
            observed.trim()
        ));
    }
    let (runtime_size, runtime_sha256) = file_identity(runtime, MAX_RUNTIME_BYTES, "Bun runtime")?;
    let (bundle_size, bundle_sha256) = file_identity(helper, MAX_BUNDLE_BYTES, "helper bundle")?;
    Ok(AgentHelperBuild {
        schema: "vela.agent-helper-build.v1",
        platform: platform(),
        runtime: AgentRuntimeFile {
            kind: "bun",
            version: BUN_VERSION,
            size: runtime_size,
            sha256: runtime_sha256,
        },
        bundle: AgentBundleFile {
            format: "esm",
            size: bundle_size,
            sha256: bundle_sha256,
        },
    })
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

fn helper_command(runtime: &Path, helper: &Path, vela: &Path) -> Command {
    let mut command = Command::new(runtime);
    command.arg(helper);
    // This avoids accidental credential forwarding; it is not an OS sandbox.
    // The optional helper is a trusted local controller. Its worker and
    // verifier enforce the actual filesystem/network custody boundary.
    for (name, _) in std::env::vars_os() {
        if is_sensitive_environment(&name) {
            command.env_remove(name);
        }
    }
    command.env("VELA_BIN", vela).env("VELA_NO_KEY_ACCESS", "1");
    command
}

fn helper_runtime_and_vela() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    let helper = resolve_helper(std::env::var_os(HELPER_ENV), &current_exe)?;
    let runtime = resolve_runtime(std::env::var_os(RUNTIME_ENV))?;
    let vela = std::fs::canonicalize(&current_exe)
        .map_err(|error| format!("resolve invoking Vela binary: {error}"))?;
    Ok((helper, runtime, vela))
}

fn run(action: &str, args: &[OsString]) -> Result<i32, String> {
    if !is_allowed_action(action) {
        return Err(format!(
            "unsupported Vela Agent action '{action}'; expected {}",
            ALLOWED_ACTIONS.join(", ")
        ));
    }
    let (helper, runtime, vela) = helper_runtime_and_vela()?;
    let mut command = helper_command(&runtime, &helper, &vela);
    command.arg(action).args(args);
    let status = command
        .status()
        .map_err(|error| format!("launch Vela Agent helper {}: {error}", helper.display()))?;
    Ok(status_code(status))
}

fn forward_helper_run_output(
    writer: &mut dyn Write,
    output: &[u8],
    receipt: Option<Result<(), String>>,
) -> Result<(), String> {
    writer
        .write_all(output)
        .map_err(|error| format!("write Vela Agent helper output: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("flush Vela Agent helper output: {error}"))?;
    if let Some(receipt) = receipt {
        receipt?;
    }
    Ok(())
}

fn run_attempt(frontier: &Path, attempt: &str, output: Option<&Path>) -> Result<i32, String> {
    let (helper, runtime, vela) = helper_runtime_and_vela()?;
    let build = helper_build(&runtime, &helper)?;
    let request = crate::current_work::agent_run_request(frontier, attempt, &build, output)?;
    let mut command = helper_command(&runtime, &helper, &vela);
    command
        .arg("run")
        .arg("--request-stdin")
        .current_dir(frontier)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command
        .spawn()
        .map_err(|error| format!("launch Vela Agent helper {}: {error}", helper.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Vela Agent helper stdin was unavailable".to_string())?;
    if let Err(error) = stdin.write_all(&request.bytes) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("write private Agent run request: {error}"));
    }
    drop(stdin);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Vela Agent helper stdout was unavailable".to_string())?;
    let mut output = Vec::new();
    if let Err(error) = stdout
        .take(MAX_HELPER_RUN_OUTPUT_BYTES + 1)
        .read_to_end(&mut output)
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("read Vela Agent helper output: {error}"));
    }
    if output.len() as u64 > MAX_HELPER_RUN_OUTPUT_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "Vela Agent helper output exceeds {MAX_HELPER_RUN_OUTPUT_BYTES} bytes"
        ));
    }
    let status = child
        .wait()
        .map_err(|error| format!("wait for Vela Agent helper: {error}"))?;
    let receipt = status.success().then(|| {
        crate::current_work::record_agent_run_receipt(
            frontier,
            attempt,
            &request.request_root,
            &output,
        )
    });
    forward_helper_run_output(&mut std::io::stdout(), &output, receipt)?;
    Ok(status_code(status))
}

pub(crate) fn cmd_agent(action: &str, args: &[OsString]) -> ! {
    match run(action, args) {
        Ok(code) => std::process::exit(code),
        Err(error) => crate::cli::fail(&error),
    }
}

pub(crate) fn cmd_agent_run(frontier: &Path, attempt: &str, output: Option<&Path>) -> ! {
    match run_attempt(frontier, attempt, output) {
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

    #[test]
    fn helper_platform_matches_bun_process_platform_and_arch() {
        assert_eq!(bun_platform("macos", "aarch64"), "darwin-arm64");
        assert_eq!(bun_platform("macos", "x86_64"), "darwin-x64");
        assert_eq!(bun_platform("linux", "x86_64"), "linux-x64");
        assert_eq!(bun_platform("windows", "x86_64"), "win32-x64");
    }

    #[test]
    fn successful_helper_output_survives_private_receipt_failure() {
        let output = br#"{"schema":"vela.agent-run-result.v1","run":{"path":"/tmp/run.json"}}"#;
        let mut recovered = Vec::new();
        let error = forward_helper_run_output(
            &mut recovered,
            output,
            Some(Err("private receipt unavailable".to_string())),
        )
        .unwrap_err();
        assert_eq!(recovered, output);
        assert!(error.contains("private receipt unavailable"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn helper_build_binds_exact_runtime_and_bundle_bytes() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().unwrap();
        let runtime = temporary.path().join("bun");
        let helper = temporary.path().join("vela-agent");
        std::fs::write(
            &runtime,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '{}\\n'; exit 0; fi\nexit 1\n",
                BUN_VERSION
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&runtime).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&runtime, permissions).unwrap();
        std::fs::write(&helper, b"console.log('one');\n").unwrap();

        let first = helper_build(&runtime, &helper).unwrap();
        assert_eq!(first.schema, "vela.agent-helper-build.v1");
        assert_eq!(first.runtime.kind, "bun");
        assert_eq!(first.runtime.version, BUN_VERSION);
        assert_eq!(
            first.runtime.size,
            std::fs::metadata(&runtime).unwrap().len()
        );
        assert_eq!(first.bundle.size, std::fs::metadata(&helper).unwrap().len());
        assert_eq!(first.bundle.format, "esm");

        std::fs::write(&helper, b"console.log('two');\n").unwrap();
        let changed_bundle = helper_build(&runtime, &helper).unwrap();
        assert_eq!(first.runtime.sha256, changed_bundle.runtime.sha256);
        assert_ne!(first.bundle.sha256, changed_bundle.bundle.sha256);
        assert_ne!(first.root().unwrap(), changed_bundle.root().unwrap());
    }
}
