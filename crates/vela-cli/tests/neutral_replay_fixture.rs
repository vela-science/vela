//! Frozen neutral replay history and required-Artifact fail-closed regression.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const BUNDLE_ROOT: &str = "sha256:7edc8297e79c995864b7a3e02bb046fdb47aed11a767a003c14395f5eaf4131c";
const VALID_COMMIT: &str = "0bd019a846902c8e3e7802d6150063b475f144dc";
const VALID_TREE: &str = "0983f52ac18e11897225087cf7aa919d459823cd";
const REPOSITORY_ROOT: &str =
    "sha256:6e7c2d797352a70b9d102f79baa9f3431631aa6ca240233f3dcd37d13f938e6a";
const STANDING_COMMITMENT: &str =
    "sha256:87e6791ebd481d977a0789b71f5fe523a1fe2799fb1015eb852f1f57da79ace1";
const CLAIM_ID: &str = "vcl_24df07004f63ce0c92a4fe12b06a08d0b777714642f4e9d613a92d8b3bdbb94b";
const SEQUENCE_ONE_RECORD_ROOT: &str =
    "sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753";
const CORRUPT_COMMIT: &str = "1712f8189c66d49415ab3ab54a8ae96e605e505c";
const MISSING_ARTIFACT_ERROR: &str = "read object records/artifacts/sha256/39feb3b6928d9d1ccf52fb14ad584c45d515cc3800f011388e7ca77c3dc6e1cb: No such file or directory (os error 2)";

fn reference_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/neutral-replay")
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

#[test]
fn clean_clone_replays_and_missing_required_artifact_fails_closed() {
    let reference = reference_directory();
    let bundle = reference.join("neutral-replay.git.bundle");
    assert_eq!(
        vela_protocol::canonical::sha256_root(
            &std::fs::read(&bundle).expect("neutral replay Git bundle")
        ),
        BUNDLE_ROOT
    );

    let temporary = tempfile::tempdir().expect("neutral replay temporary directory");
    let valid = temporary.path().join("valid");
    clone_branch(&bundle, "valid", &valid);
    assert_eq!(git(&valid, &["rev-parse", "HEAD"]), VALID_COMMIT);
    assert_eq!(git(&valid, &["rev-parse", "HEAD^{tree}"]), VALID_TREE);
    assert!(git(&valid, &["status", "--short"]).is_empty());
    assert_eq!(git(&valid, &["rev-list", "--count", "HEAD"]), "5");

    let pin = success_json(run(
        &valid,
        &[
            "authority",
            "trust",
            "pin",
            ".",
            "--record-root",
            SEQUENCE_ONE_RECORD_ROOT,
            "--json",
        ],
    ));
    assert_eq!(pin["first_authority_record_root"], SEQUENCE_ONE_RECORD_ROOT);
    let _pin = RemoveInstalledPin((pin["operation"] == "installed").then(|| {
        PathBuf::from(
            pin["authority_trust_anchor_path"]
                .as_str()
                .expect("installed trust pin path"),
        )
    }));

    let replay = success_json(run(&valid, &["replay", ".", "--json"]));
    assert_eq!(replay["repository_root"], REPOSITORY_ROOT);
    assert_eq!(replay["counts"]["accepted_claims"], 1);
    assert_eq!(replay["counts"]["pending_claims"], 0);
    assert_eq!(replay["counts"]["submissions"], 1);
    assert_eq!(replay["counts"]["verifications"], 1);

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(valid.join(".vela/repository.json")).expect("repository manifest"),
    )
    .expect("repository manifest JSON");
    assert_eq!(
        format!(
            "sha256:{}",
            vela_protocol::canonical::sha256_canonical(&manifest["accepted_claims"])
                .expect("fixture-local accepted Standing commitment")
        ),
        STANDING_COMMITMENT
    );

    let why = success_json(run(&valid, &["why", ".", CLAIM_ID, "--json"]));
    assert_eq!(why["standing"], "accepted");
    assert_eq!(why["proposal_status"], "accepted");
    assert_eq!(
        why["chain"]["verification_records"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        why["chain"]["authority_events"].as_array().map(Vec::len),
        Some(2)
    );
    let log = success_json(run(&valid, &["log", ".", "--json"]));
    let event_kinds = log["events"]
        .as_array()
        .expect("authority events")
        .iter()
        .map(|event| event["kind"].as_str().expect("Event kind"))
        .collect::<Vec<_>>();
    assert!(event_kinds.contains(&"authority.initialized"));
    assert!(event_kinds.contains(&"review.accepted"));
    assert!(event_kinds.contains(&"claim.asserted"));

    let corrupt = temporary.path().join("corrupt");
    clone_branch(&bundle, "corrupt-artifact", &corrupt);
    assert_eq!(git(&corrupt, &["rev-parse", "HEAD"]), CORRUPT_COMMIT);
    assert!(git(&corrupt, &["status", "--short"]).is_empty());
    let failed = run(&corrupt, &["replay", ".", "--json"]);
    assert_eq!(failed.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&failed.stdout).expect("Vela error JSON");
    assert_eq!(error["schema"], "vela.error.v1");
    assert_eq!(error["ok"], false);
    assert_eq!(error["command"], "replay");
    assert_eq!(error["error"]["kind"], "domain");
    assert_eq!(error["error"]["message"], MISSING_ARTIFACT_ERROR);
}
