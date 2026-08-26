//! Independent VELA-RC-1 R1 requalification of the shipped trusted-read CLI.
//!
//! This audit-only test challenges the repair with a frozen governed history.
//! It is deliberately separate from the repair's genesis regression.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const REPOSITORY_ID: &str = "ced420cb-454a-42fb-b7d2-d62422c794b7";
const SEQUENCE_ONE_ROOT: &str =
    "sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753";
const CLAIM_ID: &str = "vcl_24df07004f63ce0c92a4fe12b06a08d0b777714642f4e9d613a92d8b3bdbb94b";
const PROPOSAL_ID: &str = "vpr_af87deb2a3f1fc1c";

struct RestoreAnchor {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

impl Drop for RestoreAnchor {
    fn drop(&mut self) {
        match &self.original {
            Some(bytes) => {
                let _ = fs::write(&self.path, bytes);
                let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
            }
            None => {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

fn run(repository: &Path, home: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(repository)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("SSH_AUTH_SOCK", repository.join("missing-agent.sock"));
    if let Some(home) = home {
        command.env("HOME", home);
    }
    command.output().expect("run shipped vela CLI")
}

fn output_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn governed_reads() -> Vec<Vec<&'static str>> {
    vec![
        vec!["replay", ".", "--json"],
        vec!["status", ".", "--json"],
        vec!["claims", ".", "--json"],
        vec!["show", ".", CLAIM_ID, "--json"],
        vec!["why", ".", CLAIM_ID, "--json"],
        vec!["log", ".", "--json"],
        vec!["review", "list", ".", "--status", "all", "--json"],
        vec!["review", "show", ".", PROPOSAL_ID, "--json"],
        vec!["review", "inbox", ".", "--json"],
        vec!["projection", ".", "--json"],
        vec!["correction", "impact", ".", CLAIM_ID, "--json"],
    ]
}

fn assert_all_reach_selected_history(repository: &Path, home: Option<&Path>) {
    for command in governed_reads() {
        let output = run(repository, home, &command);
        if command.starts_with(&["correction", "impact"]) {
            assert_eq!(output.status.code(), Some(2));
            let value = output_json(&output);
            assert!(
                value["error"]["message"]
                    .as_str()
                    .is_some_and(|message| message
                        .contains("carries no `corrects` or `supersedes` relation")),
                "correct pin did not reach correction semantics: {value}"
            );
            continue;
        }
        assert!(
            output.status.success(),
            "correctly pinned governed read failed: {command:?}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let value = output_json(&output);
        assert_ne!(
            value["ok"], false,
            "governed read returned an error: {value}"
        );
    }
}

fn assert_all_refuse(repository: &Path, home: Option<&Path>, expected: &str) {
    for command in governed_reads() {
        let output = run(repository, home, &command);
        assert_eq!(
            output.status.code(),
            Some(1),
            "governed read did not fail closed: {command:?}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let value = output_json(&output);
        assert_eq!(value["ok"], false);
        assert!(
            value["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(expected)),
            "unexpected refusal for {command:?}: {value}"
        );
    }
}

fn write_anchor(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("encode trust anchor");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write trust anchor");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .expect("set trust-anchor permissions");
}

fn repository_snapshot(root: &Path) -> Vec<(PathBuf, u32, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<(PathBuf, u32, Vec<u8>)>) {
        for entry in fs::read_dir(path).expect("read Repository snapshot") {
            let path = entry.expect("Repository snapshot entry").path();
            let metadata = fs::symlink_metadata(&path).expect("Repository snapshot metadata");
            if metadata.is_dir() {
                visit(root, &path, entries);
            } else {
                entries.push((
                    path.strip_prefix(root)
                        .expect("snapshot path under Repository")
                        .to_path_buf(),
                    metadata.permissions().mode(),
                    fs::read(path).expect("Repository snapshot bytes"),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

#[test]
fn shipped_cli_enforces_the_independent_sequence_one_selection_on_every_governed_read() {
    let temporary = tempfile::tempdir().expect("audit temporary directory");
    let repository = temporary.path().join("repository");
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/neutral-replay/neutral-replay.git.bundle");
    let cloned = Command::new("git")
        .args(["clone", "-q", "-b", "valid"])
        .arg(bundle)
        .arg(&repository)
        .output()
        .expect("clone neutral replay fixture");
    assert!(
        cloned.status.success(),
        "clone fixture: {}",
        String::from_utf8_lossy(&cloned.stderr)
    );

    let pinned = run(
        &repository,
        None,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            SEQUENCE_ONE_ROOT,
            "--json",
        ],
    );
    assert!(
        pinned.status.success(),
        "install fixture pin: {}{}",
        String::from_utf8_lossy(&pinned.stdout),
        String::from_utf8_lossy(&pinned.stderr)
    );
    let pinned = output_json(&pinned);
    let anchor_path = PathBuf::from(
        pinned["authority_trust_anchor_path"]
            .as_str()
            .expect("trust-anchor path"),
    );
    let installed_by_test = pinned["operation"] == "installed";
    let correct_bytes = fs::read(&anchor_path).expect("read correct trust anchor");
    let _restore = RestoreAnchor {
        path: anchor_path.clone(),
        original: (!installed_by_test).then_some(correct_bytes.clone()),
    };
    let repository_before = repository_snapshot(&repository.join(".vela"));

    assert_all_reach_selected_history(&repository, None);

    fs::remove_file(&anchor_path).expect("remove trust anchor");
    assert_all_refuse(&repository, None, "independent sequence-one pin");

    let hostile_home = temporary.path().join("hostile-home");
    let hostile_anchor = hostile_home
        .join(".vela/trust/authorities")
        .join(format!("{REPOSITORY_ID}.json"));
    fs::create_dir_all(hostile_anchor.parent().expect("hostile trust parent"))
        .expect("create hostile trust directory");
    write_anchor(
        &hostile_anchor,
        &json!({
            "schema": "vela.authority-trust-anchor.v1",
            "repository_id": REPOSITORY_ID,
            "first_authority_record_root": SEQUENCE_ONE_ROOT,
        }),
    );
    let hostile_missing = run(&repository, Some(&hostile_home), &["replay", ".", "--json"]);
    assert_eq!(hostile_missing.status.code(), Some(1));
    assert!(
        output_json(&hostile_missing)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("independent sequence-one pin")),
        "hostile HOME redirected a missing OS-account pin"
    );

    write_anchor(
        &anchor_path,
        &json!({
            "schema": "vela.authority-trust-anchor.v1",
            "repository_id": REPOSITORY_ID,
            "first_authority_record_root": format!("sha256:{}", "0".repeat(64)),
        }),
    );
    assert_all_refuse(
        &repository,
        None,
        "installed authority trust anchor selects",
    );

    fs::write(&anchor_path, b"{not-json\n").expect("write malformed trust anchor");
    fs::set_permissions(&anchor_path, fs::Permissions::from_mode(0o600))
        .expect("set malformed trust-anchor permissions");
    assert_all_refuse(
        &repository,
        None,
        "could not load the independent authority trust anchor",
    );

    fs::write(&anchor_path, &correct_bytes).expect("restore correct trust anchor");
    fs::set_permissions(&anchor_path, fs::Permissions::from_mode(0o600))
        .expect("restore trust-anchor permissions");
    write_anchor(
        &hostile_anchor,
        &json!({
            "schema": "vela.authority-trust-anchor.v1",
            "repository_id": REPOSITORY_ID,
            "first_authority_record_root": format!("sha256:{}", "0".repeat(64)),
        }),
    );
    assert_all_reach_selected_history(&repository, Some(&hostile_home));
    assert_eq!(
        repository_snapshot(&repository.join(".vela")),
        repository_before,
        "governed reads and their trust refusals must not mutate Repository state"
    );
}
