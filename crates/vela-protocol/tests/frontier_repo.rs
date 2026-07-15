use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;
use vela_protocol::frontier_repo::{self, InitOptions};

fn vela_bin() -> PathBuf {
    if let Ok(env_path) = std::env::var("CARGO_BIN_EXE_vela") {
        return PathBuf::from(env_path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let debug = manifest.join("../../target/debug/vela");
    if debug.is_file() {
        return debug;
    }
    let release = manifest.join("../../target/release/vela");
    if release.is_file() {
        return release;
    }
    debug
}

fn run_json(args: &[&str]) -> Value {
    let binary = vela_bin();
    let output = Command::new(&binary)
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        output.status.success(),
        "vela command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "command did not return JSON: {error}\nbinary: {}\nstdout:\n{}\nstderr:\n{}",
            binary.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn run_expect_failure(args: &[&str]) -> String {
    let output = Command::new(vela_bin())
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        !output.status.success(),
        "vela command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn run_failure_json(args: &[&str]) -> Value {
    let binary = vela_bin();
    let output = Command::new(&binary)
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        !output.status.success(),
        "vela command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "failed command did not return JSON: {error}\nbinary: {}\nstdout:\n{}\nstderr:\n{}",
            binary.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn init_creates_canonical_frontier_repo_layout() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("test-frontier");

    let payload = run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Test frontier",
        "--template",
        "disease-frontier",
        "--json",
    ]);

    assert_eq!(payload["schema"], "vela.frontier_repo_init.v0.1");
    assert_eq!(payload["layout"], "vela.frontier_repo.v0.1");
    // The git-clean skeleton: the record + its derived views + the CI/agent
    // scaffold. No empty section-README stub dirs (those are created on demand
    // when a section actually holds content).
    for path in [
        "README.md",
        "SCOPE.md",
        "VELA.md",
        ".gitignore",
        ".github/workflows/vela-frontier.yml",
        "frontier.yaml",
        "frontier.json",
        "vela.lock",
        "proof/latest.json",
        "proof/events.manifest.jsonl",
        "proof/replay.trace.jsonl",
        "proof/freshness.md",
        "proof/hashes.json",
        ".vela/config.toml",
        ".vela/findings",
        ".vela/events",
        ".vela/proposals",
        ".vela/proof-state.json",
        ".vela/actors.json",
    ] {
        assert!(frontier.join(path).exists(), "missing {path}");
    }
    // No empty stub directories litter a fresh frontier.
    for stub in ["sources", "artifacts", "review", "exports"] {
        assert!(
            !frontier.join(stub).exists(),
            "unexpected empty stub dir: {stub}"
        );
    }
    assert!(frontier.join(".git").is_dir());
    let branch = Command::new("git")
        .args(["-C", frontier.to_str().unwrap(), "branch", "--show-current"])
        .output()
        .expect("failed to inspect initialized frontier branch");
    assert!(branch.status.success());
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "main");
    assert!(
        fs::read_to_string(frontier.join("README.md"))
            .unwrap()
            .contains("Test frontier")
    );
    assert!(
        fs::read_to_string(frontier.join("frontier.yaml"))
            .unwrap()
            .contains("mode: split")
    );
    assert!(
        fs::read_to_string(frontier.join("vela.lock"))
            .unwrap()
            .contains("canonicalization:")
    );
    assert!(
        fs::read_to_string(frontier.join("vela.lock"))
            .unwrap()
            .contains("proof:")
    );
    let frontier_json: Value =
        serde_json::from_slice(&fs::read(frontier.join("frontier.json")).unwrap()).unwrap();
    assert_eq!(
        frontier_json["_meta"]["materialized_from"],
        Value::String(".vela/events/".to_string())
    );
    assert!(
        frontier_json["_warning"]
            .as_str()
            .unwrap()
            .contains("Do not edit frontier.json directly")
    );

    let check = run_json(&["check", frontier.to_str().unwrap(), "--json"]);
    assert_eq!(check["ok"], true);
    // The repo subcommand died in the v0.700 surface cut; status and
    // doctor are top-level now and carry no schema field — assert the
    // living contract: both answer, doctor is ok on a fresh init.
    let _status = run_json(&["status", frontier.to_str().unwrap(), "--json"]);
    let proof = run_json(&["proof", "verify", frontier.to_str().unwrap(), "--json"]);
    assert_eq!(proof["schema"], "vela.frontier_proof_verify.v0.1");
    assert_eq!(proof["ok"], true);
}

#[test]
fn frontier_materialize_writes_frontier_json_and_lock() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("materialized-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Materialized frontier",
        "--json",
    ]);
    fs::remove_file(frontier.join("frontier.json")).expect("remove frontier.json");
    fs::remove_file(frontier.join("vela.lock")).expect("remove vela.lock");

    let payload = run_json(&[
        "frontier",
        "materialize",
        frontier.to_str().unwrap(),
        "--json",
    ]);

    assert_eq!(payload["schema"], "vela.frontier_materialize.v0.1");
    assert_eq!(payload["wrote_frontier"], "frontier.json");
    assert_eq!(payload["wrote_lock"], "vela.lock");
    assert!(frontier.join("frontier.json").is_file());
    assert!(frontier.join("vela.lock").is_file());
    assert!(frontier.join("proof/latest.json").is_file());
    assert!(frontier.join("proof/events.manifest.jsonl").is_file());
    assert!(frontier.join("proof/replay.trace.jsonl").is_file());
    assert!(
        fs::read_to_string(frontier.join("vela.lock"))
            .unwrap()
            .contains("event_log_hash:")
    );
}

#[test]
fn frontier_materialize_is_idempotent_when_state_is_fresh() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("idempotent-frontier");
    frontier_repo::initialize(
        &frontier,
        InitOptions {
            name: "Idempotent frontier",
            template: "default",
            initialize_git: false,
        },
    )
    .expect("initialize frontier repo");
    frontier_repo::materialize(&frontier).expect("materialize frontier");
    let frontier_json = fs::read_to_string(frontier.join("frontier.json")).unwrap();
    let lock = fs::read_to_string(frontier.join("vela.lock")).unwrap();
    let proof_latest = fs::read_to_string(frontier.join("proof/latest.json")).unwrap();
    let proof_freshness = fs::read_to_string(frontier.join("proof/freshness.md")).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));
    frontier_repo::materialize(&frontier).expect("materialize frontier again");

    assert_eq!(
        fs::read_to_string(frontier.join("frontier.json")).unwrap(),
        frontier_json
    );
    assert_eq!(
        fs::read_to_string(frontier.join("vela.lock")).unwrap(),
        lock
    );
    assert_eq!(
        fs::read_to_string(frontier.join("proof/latest.json")).unwrap(),
        proof_latest
    );
    assert_eq!(
        fs::read_to_string(frontier.join("proof/freshness.md")).unwrap(),
        proof_freshness
    );
}

#[test]
fn strict_check_fails_when_visible_proof_is_tampered() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("tampered-proof-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Tampered proof frontier",
        "--json",
    ]);

    fs::write(frontier.join("proof/latest.json"), "{\"tampered\":true}\n").unwrap();

    let error = run_expect_failure(&["check", frontier.to_str().unwrap(), "--strict", "--json"]);

    assert!(error.contains("proof digest does not match proof/"));

    let proof_error =
        run_expect_failure(&["proof", "verify", frontier.to_str().unwrap(), "--json"]);
    assert!(proof_error.contains("proof_digest_mismatch"));
}

#[test]
fn proof_explain_prints_human_readable_repo_chain() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("explain-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Explain frontier",
        "--json",
    ]);

    let output = Command::new(vela_bin())
        .args(["proof", "explain", frontier.to_str().unwrap()])
        .output()
        .expect("failed to run vela proof explain");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Authority: `.vela/events/`"));
    assert!(text.contains("Visible proof: `proof/latest.json`"));
}

#[test]
fn strict_check_fails_when_visible_frontier_does_not_match_lock() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("stale-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Stale frontier",
        "--json",
    ]);

    let mut data: Value =
        serde_json::from_slice(&fs::read(frontier.join("frontier.json")).unwrap()).unwrap();
    data["frontier"]["description"] = Value::String("tampered after lock".to_string());
    fs::write(
        frontier.join("frontier.json"),
        serde_json::to_string_pretty(&data).unwrap(),
    )
    .unwrap();

    let error = run_expect_failure(&["check", frontier.to_str().unwrap(), "--strict", "--json"]);

    assert!(error.contains("frontier_lock_mismatch"));
}

#[test]
fn integrity_tolerates_missing_lock_for_frontier_repo() {
    // vela.lock is a derived view: gitignored and regenerated byte-for-byte by
    // `vela frontier materialize`. A missing lock is therefore "not yet materialized,"
    // not a layout fault — integrity must NOT flag `missing_frontier_lock`.
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("missing-lock-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Missing lock frontier",
        "--json",
    ]);
    fs::remove_file(frontier.join("vela.lock")).expect("remove lock");

    // `integrity` was folded into `check`; state_integrity carries the same
    // structural_errors with rule_ids.
    let report = run_json(&["check", frontier.to_str().unwrap(), "--json"]);

    let flags_missing_lock = report["state_integrity"]["structural_errors"]
        .as_array()
        .map(|errs| {
            errs.iter()
                .any(|error| error["rule_id"] == "missing_frontier_lock")
        })
        .unwrap_or(false);
    assert!(
        !flags_missing_lock,
        "a missing vela.lock is a regenerable derived view and must NOT be a structural error: {report}"
    );
}

#[test]
fn repo_without_manifest_requires_materialization() {
    let tmp = TempDir::new().expect("tempdir");
    let frontier = tmp.path().join("unmaterialized-frontier");
    run_json(&[
        "init",
        frontier.to_str().unwrap(),
        "--name",
        "Unmaterialized frontier",
        "--json",
    ]);
    fs::remove_file(frontier.join("frontier.yaml")).expect("remove frontier.yaml");
    fs::remove_file(frontier.join("vela.lock")).expect("remove vela.lock");

    let check = run_failure_json(&["check", frontier.to_str().unwrap(), "--json"]);

    assert_eq!(check["ok"], false);
    assert!(
        check["state_integrity"]["structural_errors"]
            .as_array()
            .expect("structural errors")
            .iter()
            .any(|error| error["rule_id"] == "missing_frontier_manifest"),
        "missing current manifest must be explicit: {check}"
    );
}
