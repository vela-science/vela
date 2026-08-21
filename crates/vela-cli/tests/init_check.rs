//! `vela init --check` exercises the real target, runtime-identity, and signer
//! preconditions without creating bootstrap, Git, trust, or scientific state.

#![cfg(unix)]

#[cfg(feature = "test-support")]
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(feature = "test-support")]
use std::path::PathBuf;
use std::process::{Command, Output};

mod support;
use support::EphemeralAgent;
#[cfg(feature = "test-support")]
use support::RemoveAnchorOnDrop;

fn run(cwd: &Path, socket: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("SSH_AUTH_SOCK", socket)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run vela init --check")
}

#[cfg(feature = "test-support")]
fn run_with_failpoint(cwd: &Path, socket: &Path, args: &[&str], failpoint: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(args)
        .env("SSH_AUTH_SOCK", socket)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env(failpoint, "1")
        .output()
        .expect("run interrupted vela init")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[cfg(feature = "test-support")]
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = std::fs::read_dir(current)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).expect("snapshot metadata");
            let relative = path.strip_prefix(base).expect("relative snapshot path");
            if metadata.is_dir() {
                visit(base, &path, files);
            } else {
                files.insert(
                    relative.to_path_buf(),
                    std::fs::read(&path).expect("snapshot file"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    if root.is_dir() {
        visit(root, root, &mut files);
    }
    files
}

#[test]
fn init_check_accepts_an_absent_target_without_creating_it() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela init check test");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let checked = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &repository_text,
            "--name",
            "Init preflight fixture",
            "--scope",
            "Prove setup readiness without changing any state.",
            "--check",
            "--json",
        ],
    );
    assert!(
        checked.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    let checked = json(&checked);
    assert_eq!(checked["schema"], "vela.init-preflight.v1");
    assert_eq!(checked["command"], "init.check");
    assert_eq!(checked["changed"], false);
    assert_eq!(checked["authority_effect"], "none");
    assert_eq!(checked["target_state"], "absent");
    assert_eq!(checked["runtime_identity"]["available"], true);
    assert_eq!(
        checked["runtime_identity"]["device_identifier_exposed"],
        false
    );
    assert_eq!(checked["authority_signer"]["available"], true);
    assert!(
        !repository.exists(),
        "preflight must not create even the absent target directory"
    );
}

#[test]
fn fresh_preflight_refuses_missing_or_blank_profile_inputs_without_creating_a_target() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let missing_socket = temporary.path().join("missing-agent.sock");
    for args in [
        vec!["init", &repository_text, "--check", "--json"],
        vec![
            "init",
            &repository_text,
            "--name",
            " ",
            "--scope",
            " ",
            "--check",
            "--json",
        ],
    ] {
        let refused = run(temporary.path(), &missing_socket, &args);
        assert_eq!(refused.status.code(), Some(1));
        let refused = json(&refused);
        assert_eq!(refused["schema"], "vela.error.v1");
        assert_eq!(refused["changed"], false);
        assert_eq!(refused["retained"]["transaction_marker"], false);
        assert!(!repository.exists());
    }
}

#[cfg(feature = "test-support")]
#[test]
fn preflight_reads_staged_bootstrap_bootstrap_initialized_and_hostile_targets_exactly() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela init state preflight test");
    let missing_socket = temporary.path().join("missing-agent.sock");

    let empty = temporary.path().join("empty");
    std::fs::create_dir(&empty).expect("empty target");
    let empty_text = empty.to_string_lossy().into_owned();
    let empty_checked = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &empty_text,
            "--name",
            "Empty preflight",
            "--scope",
            "Validate one empty target.",
            "--check",
            "--json",
        ],
    );
    assert_eq!(json(&empty_checked)["target_state"], "empty");
    assert_eq!(snapshot(&empty), BTreeMap::new());

    let staged = temporary.path().join("staged");
    let staged_text = staged.to_string_lossy().into_owned();
    let staged_init = [
        "init",
        &staged_text,
        "--name",
        "Staged preflight",
        "--scope",
        "Validate one crash-retained staged bootstrap.",
        "--json",
    ];
    let interrupted = run_with_failpoint(
        temporary.path(),
        agent.socket(),
        &staged_init,
        "VELA_TEST_INTERRUPT_INIT_BEFORE_GIT",
    );
    assert_eq!(interrupted.status.code(), Some(86));
    let before = snapshot(&staged);
    let staged_checked = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &staged_text,
            "--name",
            "Staged preflight",
            "--scope",
            "Validate one crash-retained staged bootstrap.",
            "--check",
            "--json",
        ],
    );
    assert_eq!(json(&staged_checked)["target_state"], "staged_bootstrap");
    assert_eq!(snapshot(&staged), before);

    let bootstrap = temporary.path().join("bootstrap");
    let bootstrap_text = bootstrap.to_string_lossy().into_owned();
    let bootstrap_args = [
        "init",
        &bootstrap_text,
        "--name",
        "Bootstrap preflight",
        "--scope",
        "Validate one retained bootstrap before authority.",
        "--json",
    ];
    let unsigned = run(temporary.path(), &missing_socket, &bootstrap_args);
    assert_eq!(unsigned.status.code(), Some(1));
    let before = snapshot(&bootstrap);
    let bootstrap_checked = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &bootstrap_text,
            "--name",
            "Bootstrap preflight",
            "--scope",
            "Validate one retained bootstrap before authority.",
            "--check",
            "--json",
        ],
    );
    assert_eq!(json(&bootstrap_checked)["target_state"], "bootstrap");
    assert_eq!(snapshot(&bootstrap), before);

    let initialized = temporary.path().join("initialized");
    let initialized_text = initialized.to_string_lossy().into_owned();
    let initialized_output = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &initialized_text,
            "--name",
            "Initialized preflight",
            "--scope",
            "Validate one complete repository without a signer.",
            "--json",
        ],
    );
    assert!(initialized_output.status.success());
    let _anchor =
        RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized_output.stdout))
            .expect("initialized trust anchor");
    let before = snapshot(&initialized);
    let initialized_checked = run(
        temporary.path(),
        &missing_socket,
        &["init", &initialized_text, "--check", "--json"],
    );
    let initialized_checked = json(&initialized_checked);
    assert_eq!(initialized_checked["target_state"], "initialized");
    assert!(initialized_checked["authority_signer"]["available"].is_null());
    assert_eq!(snapshot(&initialized), before);

    let hostile = temporary.path().join("nonempty");
    std::fs::create_dir(&hostile).expect("hostile target");
    std::fs::write(hostile.join("packet.json"), b"{}\n").expect("hostile packet");
    let before = snapshot(&hostile);
    let hostile_text = hostile.to_string_lossy().into_owned();
    let refused = run(
        temporary.path(),
        agent.socket(),
        &[
            "init",
            &hostile_text,
            "--name",
            "Nonempty preflight",
            "--scope",
            "Preserve the empty-directory invariant.",
            "--check",
            "--json",
        ],
    );
    assert_eq!(refused.status.code(), Some(1));
    let refused = json(&refused);
    assert_eq!(refused["changed"], false);
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("non-empty directory"))
    );
    assert_eq!(snapshot(&hostile), before);
}
