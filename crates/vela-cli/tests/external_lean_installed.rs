use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use vela_cli::external_lean;

fn executable(path: &str) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|error| panic!("missing test executable {path}: {error}"))
}

fn request(
    executable: &Path,
    args: &[&str],
    read_root: &Path,
    write_root: &Path,
) -> serde_json::Value {
    json!({
        "schema": "vela.external_lean_sandbox_request.v1",
        "command": std::iter::once(executable.to_string_lossy().to_string())
            .chain(args.iter().map(|value| (*value).to_string()))
            .collect::<Vec<_>>(),
        "cwd": read_root,
        "read_roots": [read_root],
        "write_root": write_root,
        "allowed_executables": [executable],
        "limits": {
            "wall_seconds": 5,
            "output_bytes": 4096,
            "disk_bytes": 1048576,
            "memory_bytes": 1073741824,
            "processes": 16,
            "cpu_seconds": 5,
            "open_files": 64,
            "single_file_bytes": 1048576
        }
    })
}

#[test]
fn embedded_driver_runs_outside_a_campaign_checkout() {
    let directory = tempfile::tempdir().unwrap();
    let read_root = directory.path().join("pinned-source");
    let write_root = directory.path().join("bounded-output");
    fs::create_dir(&read_root).unwrap();
    fs::create_dir(&write_root).unwrap();
    let echo = executable("/bin/echo");

    let result = external_lean::run_sandbox_request(&request(
        &echo,
        &["installed-driver-ok"],
        &read_root,
        &write_root,
    ))
    .unwrap();

    if cfg!(target_os = "macos") {
        assert_eq!(result["ok"], true, "{result:#}");
        assert_eq!(result["sandbox"]["backend"], "sandbox-exec");
        assert_eq!(result["stdout"]["rendered"], "installed-driver-ok\n");
    } else {
        assert_eq!(result["ok"], false);
        assert_eq!(result["error"]["code"], "sandbox_unavailable");
    }
    assert_eq!(result["sandbox"]["fail_closed"], true);
    assert!(external_lean::embedded_driver_root().starts_with("sha256:"));
}

#[test]
fn parsed_installed_command_never_discovers_a_checkout_driver() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home");
    let cache = directory.path().join("external-lean-cache");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&cache).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(directory.path())
        .env("HOME", &home)
        .env("VELA_EXTERNAL_LEAN_CACHE", &cache)
        .env_remove("VELA_WORKSPACE_ROOT")
        .args([
            "reproduce-external",
            "https://github.com/example/fixture",
            "../not-a-commit",
            "Fixture.theorem",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut values =
        serde_json::Deserializer::from_slice(&output.stdout).into_iter::<serde_json::Value>();
    let value = values.next().unwrap().unwrap();
    assert!(
        values.next().is_none(),
        "command emitted more than one JSON value"
    );
    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "reproduce-external");
    assert_eq!(value["verdict"], "skipped_with_reason");
    assert_eq!(value["installed_onramp"]["embedded"], true);
    assert_eq!(value["installed_onramp"]["checkout_discovery"], false);
    assert_eq!(value["installed_onramp"]["artifact_retention"], "ephemeral");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("driver not found"));
}
