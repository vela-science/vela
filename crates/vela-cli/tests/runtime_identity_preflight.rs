//! Linux/container runtime identity failures are structured, actionable, and
//! rejected before the empty initialization target changes.

#![cfg(all(feature = "test-support", target_os = "linux"))]

use std::path::Path;
use std::process::{Command, Output};

fn run(cwd: &Path, machine_id: &Path, target: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args([
            "init",
            &target.to_string_lossy(),
            "--name",
            "Container identity regression",
            "--scope",
            "Refuse an unavailable local runtime identity before any bootstrap byte.",
            "--json",
        ])
        .env("VELA_TEST_MACHINE_ID_PATH", machine_id)
        .env("SSH_AUTH_SOCK", cwd.join("missing-agent.sock"))
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run vela init")
}

fn diagnostic(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode diagnostic: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_zero_delta(target: &Path, output: &Output, code: &str) {
    assert_eq!(output.status.code(), Some(1));
    let value = diagnostic(output);
    assert_eq!(value["schema"], "vela.error.v1");
    assert_eq!(value["command"], "init");
    assert_eq!(value["changed"], false);
    assert_eq!(value["retained"]["transaction_marker"], false);
    assert_eq!(value["error"]["code"], code);
    assert!(
        value["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("container-local")
                && hint.contains("/etc/machine-id")
                && hint.contains("do not mount the host")),
        "diagnostic must carry the container-safe remediation: {value}"
    );
    assert_eq!(
        std::fs::read_dir(target)
            .expect("read unchanged target")
            .count(),
        0,
        "identity refusal must precede Profile, Git, journal, or trust state"
    );
}

#[test]
fn missing_and_malformed_machine_ids_are_stable_zero_delta_errors() {
    let temporary = tempfile::tempdir().expect("temporary directory");

    let missing_target = temporary.path().join("missing-target");
    std::fs::create_dir(&missing_target).expect("missing target");
    let missing_path = temporary.path().join("absent-machine-id");
    let alternate = temporary.path().join("var/lib/dbus/machine-id");
    std::fs::create_dir_all(alternate.parent().expect("alternate parent"))
        .expect("alternate directory");
    std::fs::write(&alternate, b"0123456789abcdef0123456789abcdef\n").expect("alternate identity");
    let missing = run(temporary.path(), &missing_path, &missing_target);
    assert_zero_delta(&missing_target, &missing, "runtime_identity_missing");
    let missing_repeat = run(temporary.path(), &missing_path, &missing_target);
    assert_eq!(
        missing.stdout, missing_repeat.stdout,
        "the same missing identity request has a stable structured diagnostic"
    );

    for (case, bytes) in [
        ("non-hex", b"not-a-machine-id\n".as_slice()),
        (
            "uppercase",
            b"0123456789ABCDEF0123456789ABCDEF\n".as_slice(),
        ),
        (
            "missing-newline",
            b"0123456789abcdef0123456789abcdef".as_slice(),
        ),
        (
            "leading-space",
            b" 0123456789abcdef0123456789abcdef\n".as_slice(),
        ),
        ("all-zero", b"00000000000000000000000000000000\n".as_slice()),
    ] {
        let malformed_target = temporary.path().join(format!("{case}-target"));
        std::fs::create_dir(&malformed_target).expect("malformed target");
        let malformed_path = temporary.path().join(format!("{case}-machine-id"));
        std::fs::write(&malformed_path, bytes).expect("malformed machine ID");
        let malformed = run(temporary.path(), &malformed_path, &malformed_target);
        assert_zero_delta(&malformed_target, &malformed, "runtime_identity_malformed");
    }
}
