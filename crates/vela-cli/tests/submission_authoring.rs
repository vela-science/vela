//! Focused current Submission authoring regressions.

#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output};

use vela_protocol::submission::SubmissionRecordV2;

mod support;
use support::{
    EphemeralAgent, RemoveAnchorOnDrop, configure_git_identity, run_with_isolated_home,
    success_json,
};

fn run(home: &Path, repository_path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .arg("submit")
        .args(args)
        .args([
            "--claim",
            "Exact bounded fixture.",
            "--type",
            "theoretical",
            "--replayability",
            "exact",
            "--artifact",
            "missing.json:source-diff",
            "--caveat",
            "Exact fixture only.",
            "--as",
            "agent:fixture",
            "--repo",
            repository_path.to_str().expect("utf-8 repository"),
            "--json",
        ])
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("SSH_AUTH_SOCK")
        .output()
        .expect("run vela submit")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn new_claim_authoring_does_not_require_a_source_run() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(directory.path(), directory.path(), &[]);
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --source-run"));
    assert!(message.contains("artifact 0"));
}

#[test]
fn exact_supersession_authoring_does_not_require_a_source_run() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let output = run(
        directory.path(),
        directory.path(),
        &[
            "--supersedes",
            &format!("vcl_{}", "a".repeat(64)),
            "--target-root",
            &format!("sha256:{}", "b".repeat(64)),
        ],
    );
    let message = combined(&output);

    assert!(!output.status.success());
    assert!(!message.contains("requires --source-run"));
    assert!(message.contains("artifact 0"));
}

#[test]
fn execution_binding_is_all_or_none_and_survives_signed_submission_authoring() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let roots = [
        ("--packet-root", format!("sha256:{}", "1".repeat(64))),
        ("--profile-root", format!("sha256:{}", "2".repeat(64))),
        (
            "--verifier-capsule-root",
            format!("sha256:{}", "3".repeat(64)),
        ),
        (
            "--result-contract-root",
            format!("sha256:{}", "4".repeat(64)),
        ),
    ];

    for omitted in 0..roots.len() {
        let mut flags = Vec::new();
        for (index, (name, value)) in roots.iter().enumerate() {
            if index != omitted {
                flags.push(*name);
                flags.push(value.as_str());
            }
        }
        let output = run(temporary.path(), temporary.path(), &flags);
        let message = combined(&output);
        assert!(!output.status.success());
        assert!(
            message.contains(roots[omitted].0),
            "partial binding did not require {}:\n{message}",
            roots[omitted].0
        );
        assert!(
            !message.contains("artifact 0"),
            "partial binding reached repository authoring before CLI refusal:\n{message}"
        );
    }

    let home = temporary.path().join("home");
    std::fs::create_dir_all(&home).expect("isolated home");
    let agent = EphemeralAgent::start(temporary.path(), "execution binding CLI test");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let initialized = success_json(&run_with_isolated_home(
        temporary.path(),
        Some(agent.socket()),
        &home,
        &[
            "init",
            &repository_text,
            "--name",
            "Execution binding fixture",
            "--scope",
            "Preserve one exact producer execution binding.",
            "--json",
        ],
    ));
    let _anchor = RemoveAnchorOnDrop(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .map(Into::into)
            .expect("local trust anchor path"),
    );
    configure_git_identity(&repository);
    std::fs::write(repository.join("evidence.json"), b"{\"bounded\":true}\n")
        .expect("fixture evidence");
    for args in [
        &["add", "evidence.json"][..],
        &["commit", "-qm", "Retain execution binding fixture evidence"][..],
    ] {
        let output = Command::new("git")
            .current_dir(&repository)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut arguments = vec![
        "submit",
        "--repo",
        ".",
        "--claim",
        "The exact execution binding fixture produced the retained evidence.",
        "--type",
        "computational",
        "--replayability",
        "exact",
        "--artifact",
        "evidence.json:source-diff",
        "--caveat",
        "This fixture establishes only exact binding preservation.",
        "--as",
        "agent:execution-binding-fixture",
    ];
    for (name, value) in &roots {
        arguments.push(*name);
        arguments.push(value);
    }
    arguments.push("--json");
    let submitted = success_json(&run_with_isolated_home(
        &repository,
        None,
        &home,
        &arguments,
    ));
    let repository_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json"))
            .expect("current repository manifest"),
    )
    .expect("decode current repository manifest");
    let reference = repository_manifest["submissions"]
        .as_array()
        .and_then(|submissions| {
            submissions
                .iter()
                .find(|reference| reference["id"] == submitted["submission_id"])
        })
        .expect("retained Submission reference");
    let retained = SubmissionRecordV2::parse(
        &std::fs::read(
            repository.join(
                reference["path"]
                    .as_str()
                    .expect("retained Submission path"),
            ),
        )
        .expect("retained Submission bytes"),
    )
    .expect("parse retained signed Submission");
    let binding = retained
        .submission
        .execution_binding
        .expect("exact execution binding");
    assert_eq!(binding.schema, "vela.execution-binding.v1");
    assert_eq!(binding.packet_root, roots[0].1);
    assert_eq!(binding.profile_root, roots[1].1);
    assert_eq!(binding.verifier_capsule_root, roots[2].1);
    assert_eq!(binding.result_contract_root, roots[3].1);
}
