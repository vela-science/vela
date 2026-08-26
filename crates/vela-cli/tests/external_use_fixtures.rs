//! Frozen R4 external-use histories, native checks, and fail-closed branches.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn examples_directory(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn metadata(directory: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(directory.join("expected.json")).expect("fixture metadata"),
    )
    .expect("fixture metadata JSON")
}

fn git(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository)
        .args(args)
        .output()
        .expect("run Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git output is UTF-8")
        .trim()
        .to_string()
}

fn clone_branch(bundle: &Path, branch: &str, destination: &Path) {
    let output = Command::new("git")
        .args(["clone", "-q", "-b", branch])
        .arg(bundle)
        .arg(destination)
        .output()
        .expect("clone frozen fixture branch");
    assert!(
        output.status.success(),
        "git clone {branch}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run(repository: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(repository)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .output()
        .expect("run Vela")
}

fn success_json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Vela success JSON")
}

struct RemoveInstalledPin(Option<PathBuf>);

impl Drop for RemoveInstalledPin {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn pin(repository: &Path, expected: &Value) -> RemoveInstalledPin {
    let root = expected["authority"]["sequence_one_record_root"]
        .as_str()
        .expect("sequence-one root");
    let pinned = success_json(run(
        repository,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            root,
            "--json",
        ],
    ));
    assert_eq!(pinned["first_authority_record_root"], root);
    RemoveInstalledPin((pinned["operation"] == "installed").then(|| {
        PathBuf::from(
            pinned["authority_trust_anchor_path"]
                .as_str()
                .expect("installed trust pin path"),
        )
    }))
}

fn assert_bundle(directory: &Path, expected: &Value, name: &str) -> PathBuf {
    let bundle = directory.join(name);
    assert_eq!(
        vela_protocol::canonical::sha256_root(
            &std::fs::read(&bundle).expect("external-use Git bundle")
        ),
        expected["bundle_root"].as_str().expect("bundle root")
    );
    bundle
}

fn assert_accepted_set(repository: &Path, expected: &Value) {
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json")).expect("repository manifest"),
    )
    .expect("repository manifest JSON");
    assert_eq!(
        format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&manifest["accepted_claims"])
                .expect("fixture-local accepted-set commitment")
        ),
        expected["branches"]["valid"]["accepted_set_fixture_commitment"]
    );
}

#[test]
fn failed_formal_proposal_is_rejected_and_corrected_proposal_replays() {
    let reference = examples_directory("external-formal-verifier");
    let expected = metadata(&reference);
    let bundle = assert_bundle(&reference, &expected, "formal-verifier.git.bundle");
    let temporary = tempfile::tempdir().expect("formal fixture temporary directory");

    for (statement, report, outcome) in [
        ("bad-statement.json", "bad-report.json", "fail"),
        ("corrected-statement.json", "corrected-report.json", "pass"),
    ] {
        let generated = temporary.path().join(format!("generated-{report}"));
        let output = Command::new("python3")
            .arg(reference.join("verifier.py"))
            .arg(reference.join(statement))
            .args(["--output", generated.to_str().expect("temporary path")])
            .args(["--expect-outcome", outcome])
            .output()
            .expect("run finite Boolean verifier");
        assert!(output.status.success(), "native verifier: {output:?}");
        assert_eq!(
            std::fs::read(generated).expect("generated verifier report"),
            std::fs::read(reference.join(report)).expect("retained verifier report")
        );
    }

    let valid = temporary.path().join("valid");
    clone_branch(&bundle, "valid", &valid);
    assert_eq!(
        git(&valid, &["rev-parse", "HEAD"]),
        expected["branches"]["valid"]["git_commit"]
    );
    assert_eq!(
        git(&valid, &["rev-parse", "HEAD^{tree}"]),
        expected["branches"]["valid"]["git_tree"]
    );
    assert!(git(&valid, &["status", "--short"]).is_empty());
    let _pin = pin(&valid, &expected);

    let replay = success_json(run(&valid, &["replay", ".", "--json"]));
    assert_eq!(
        replay["repository_root"],
        expected["branches"]["valid"]["repository_root"]
    );
    assert_eq!(replay["counts"]["accepted_claims"], 1);
    assert_eq!(replay["counts"]["pending_claims"], 0);
    assert_eq!(replay["counts"]["submissions"], 2);
    assert_eq!(replay["counts"]["verifications"], 2);
    assert_accepted_set(&valid, &expected);

    let bad_proposal = expected["objects"]["bad"]["proposal_id"]
        .as_str()
        .expect("bad Proposal id");
    let bad = success_json(run(
        &valid,
        &["review", "show", ".", bad_proposal, "--json"],
    ));
    assert_eq!(bad["status"], "rejected");
    assert_eq!(bad["verification_records"][0]["record"]["outcome"], "fail");

    for (object, standing, status) in [
        ("bad", "unassessed", "rejected"),
        ("corrected", "accepted", "accepted"),
    ] {
        let claim = expected["objects"][object]["claim_id"]
            .as_str()
            .expect("Claim id");
        let why = success_json(run(&valid, &["why", ".", claim, "--json"]));
        assert_eq!(why["standing"], standing);
        assert_eq!(why["proposal_status"], status);
    }

    let failed = temporary.path().join("failed");
    clone_branch(&bundle, "failed-proposal", &failed);
    let inbox = success_json(run(&failed, &["review", "inbox", ".", "--json"]));
    assert_eq!(
        inbox["repository_root"],
        expected["branches"]["failed_proposal"]["repository_root"]
    );
    assert_eq!(inbox["entries"][0]["readiness"]["protocol_gate"], "blocked");
    assert_eq!(
        inbox["entries"][0]["verification_records"][0]["outcome"],
        "fail"
    );

    let missing = temporary.path().join("missing");
    clone_branch(&bundle, "missing-artifact", &missing);
    let refused = run(&missing, &["replay", ".", "--json"]);
    assert_eq!(refused.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&refused.stdout).expect("Vela error JSON");
    assert_eq!(error["ok"], false);
    assert_eq!(
        error["error"]["message"],
        expected["branches"]["missing_artifact"]["error_message"]
    );
}

#[test]
fn heterogeneous_evidence_requires_both_scoped_checks_and_replays() {
    let reference = examples_directory("external-heterogeneous-evidence");
    let expected = metadata(&reference);
    let bundle = assert_bundle(&reference, &expected, "heterogeneous-evidence.git.bundle");
    let temporary = tempfile::tempdir().expect("evidence fixture temporary directory");

    let native = Command::new("python3")
        .arg(reference.join("analysis.py"))
        .arg(reference.join("observations.csv"))
        .arg("--check")
        .arg(reference.join("result.json"))
        .output()
        .expect("run exact tabular analysis");
    assert!(native.status.success(), "native analysis: {native:?}");

    let valid = temporary.path().join("valid");
    clone_branch(&bundle, "valid", &valid);
    assert_eq!(
        git(&valid, &["rev-parse", "HEAD"]),
        expected["branches"]["valid"]["git_commit"]
    );
    assert_eq!(
        git(&valid, &["rev-parse", "HEAD^{tree}"]),
        expected["branches"]["valid"]["git_tree"]
    );
    assert!(git(&valid, &["status", "--short"]).is_empty());
    let _pin = pin(&valid, &expected);

    let replay = success_json(run(&valid, &["replay", ".", "--json"]));
    assert_eq!(
        replay["repository_root"],
        expected["branches"]["valid"]["repository_root"]
    );
    assert_eq!(replay["counts"]["accepted_claims"], 1);
    assert_eq!(replay["counts"]["pending_claims"], 0);
    assert_eq!(replay["counts"]["submissions"], 1);
    assert_eq!(replay["counts"]["verifications"], 2);
    assert_accepted_set(&valid, &expected);

    let claim = expected["objects"]["claim_id"]
        .as_str()
        .expect("evidence Claim id");
    let why = success_json(run(&valid, &["why", ".", claim, "--json"]));
    assert_eq!(why["standing"], "accepted");
    assert_eq!(why["proposal_status"], "accepted");
    assert_eq!(
        why["chain"]["verification_records"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let incomplete = temporary.path().join("incomplete");
    clone_branch(&bundle, "incomplete-review", &incomplete);
    let inbox = success_json(run(&incomplete, &["review", "inbox", ".", "--json"]));
    assert_eq!(
        inbox["repository_root"],
        expected["branches"]["incomplete_review"]["repository_root"]
    );
    assert_eq!(inbox["entries"][0]["readiness"]["protocol_gate"], "blocked");
    assert_eq!(
        inbox["entries"][0]["verification_records"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        inbox["entries"][0]["readiness"]["blockers"][0]["subject"],
        "evidence_scope_review"
    );

    let missing = temporary.path().join("missing");
    clone_branch(&bundle, "missing-artifact", &missing);
    let refused = run(&missing, &["replay", ".", "--json"]);
    assert_eq!(refused.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&refused.stdout).expect("Vela error JSON");
    assert_eq!(error["ok"], false);
    assert_eq!(
        error["error"]["message"],
        expected["branches"]["missing_artifact"]["error_message"]
    );
}
