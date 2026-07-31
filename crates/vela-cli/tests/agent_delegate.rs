#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn bun_runtime() -> std::path::PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("bun"))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .expect("Bun must be available for the Agent delegator integration test")
}

fn helper(root: &std::path::Path) -> std::path::PathBuf {
    let path = root.join("vela-agent-test-helper");
    fs::write(
        &path,
        r#"import { writeFileSync } from "node:fs";
const args = process.argv.slice(2);
const environment = (name) => process.env[name] ?? "unset";
writeFileSync(process.env.VELA_AGENT_TEST_CAPTURE, [
  `cwd=${process.cwd()}`,
  `vela=${environment("VELA_BIN")}`,
  `no_key=${environment("VELA_NO_KEY_ACCESS")}`,
  `ssh=${environment("SSH_AUTH_SOCK")}`,
  `key=${environment("VELA_KEY_PATH")}`,
  `authority=${environment("VELA_REPOSITORY_AUTHORITY_TEST")}`,
  `argc=${args.length}`,
  ...args.map((argument) => `arg=${argument}`),
  "",
].join("\n"));
if (args[0] === "replay") process.exit(17);
"#,
    )
    .expect("write helper");
    let mut permissions = fs::metadata(&path).expect("helper metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("make helper executable");
    fs::canonicalize(path).expect("canonical helper")
}

#[test]
fn delegates_only_to_explicit_helper_and_scrubs_authority_environment() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let helper = helper(temporary.path());
    let helper_link = temporary.path().join("vela-agent");
    std::os::unix::fs::symlink(&helper, &helper_link).expect("link helper");
    let capture = temporary.path().join("capture.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "doctor", "--help", "--profile", "generic"])
        .current_dir(temporary.path())
        .env("VELA_AGENT_BIN", &helper_link)
        .env("VELA_AGENT_RUNTIME", bun_runtime())
        .env("VELA_AGENT_TEST_CAPTURE", &capture)
        .env("SSH_AUTH_SOCK", "/tmp/authority.sock")
        .env("VELA_KEY_PATH", "/tmp/human.key")
        .env("VELA_REPOSITORY_AUTHORITY_TEST", "secret")
        .output()
        .expect("run vela agent");
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let observed = fs::read_to_string(capture).expect("read helper capture");
    let expected_cwd = fs::canonicalize(temporary.path()).expect("canonical working directory");
    assert!(observed.contains(&format!("cwd={}", expected_cwd.display())));
    assert!(observed.contains("no_key=1"));
    assert!(observed.contains("ssh=unset"));
    assert!(observed.contains("key=unset"));
    assert!(observed.contains("authority=unset"));
    assert!(observed.contains("argc=4"));
    assert!(observed.contains("arg=doctor"));
    assert!(observed.contains("arg=--help"));
    assert!(observed.contains("arg=--profile"));
    assert!(observed.contains("arg=generic"));
    let expected_vela = fs::canonicalize(env!("CARGO_BIN_EXE_vela")).expect("canonical Vela");
    assert!(observed.contains(&format!("vela={}", expected_vela.display())));
}

#[test]
fn propagates_helper_exit_status() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let helper = helper(temporary.path());
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "replay", "run.json"])
        .env("VELA_AGENT_BIN", helper)
        .env("VELA_AGENT_RUNTIME", bun_runtime())
        .env(
            "VELA_AGENT_TEST_CAPTURE",
            temporary.path().join("capture.txt"),
        )
        .output()
        .expect("run vela agent");
    assert_eq!(output.status.code(), Some(17));
}

#[test]
fn rejects_missing_relative_and_authority_actions_before_spawn() {
    let missing = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "doctor"])
        .env_remove("VELA_AGENT_BIN")
        .output()
        .expect("run without helper");
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("VELA_AGENT_BIN is not set"));

    let relative = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "doctor"])
        .env("VELA_AGENT_BIN", "relative/helper")
        .output()
        .expect("run with relative helper");
    assert_eq!(relative.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&relative.stderr).contains("must be an absolute path"));

    let authority = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "review"])
        .output()
        .expect("parse forbidden action");
    assert_eq!(authority.status.code(), Some(2));
}

#[test]
fn namespace_help_remains_available_without_a_helper() {
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(["agent", "--help"])
        .env_remove("VELA_AGENT_BIN")
        .output()
        .expect("show Vela Agent help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for action in ["doctor", "run", "show", "replay", "export"] {
        assert!(stdout.contains(action), "missing {action} in {stdout}");
    }
    assert!(!stdout.contains("submit"));
    assert!(!stdout.contains("review"));
}
