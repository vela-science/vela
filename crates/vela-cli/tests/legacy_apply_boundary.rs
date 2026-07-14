//! Regression tests for the retired keyless finding finalizer and proposal
//! import trust boundary. Parser compatibility is intentional; authority is
//! not.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(dir)
        .args(args)
        .env("HOME", dir)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "1");
    for (key, _) in std::env::vars() {
        if key.starts_with("VELA_") && key != "VELA_ADVICE" {
            command.env_remove(key);
        }
    }
    command.output().expect("run vela")
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("HOME", dir)
        .output()
        .expect("run git")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: status={:?}\n{}",
        output.status.code(),
        output_text(output)
    );
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git(dir, args);
    assert_success(&output, &format!("git {}", args.join(" ")));
    String::from_utf8(output.stdout).unwrap()
}

fn init_frontier(dir: &Path) {
    assert_success(
        &run(dir, &["init", ".", "--name", "legacy-boundary", "--json"]),
        "init frontier",
    );
    assert_success(
        &git(dir, &["config", "user.email", "test@vela.invalid"]),
        "configure git email",
    );
    assert_success(
        &git(dir, &["config", "user.name", "Vela Test"]),
        "configure git name",
    );
    assert_success(&git(dir, &["add", "-A"]), "stage baseline");
    assert_success(&git(dir, &["commit", "-qm", "baseline"]), "commit baseline");
}

fn snapshot_files(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, out);
            } else if path.is_file() {
                out.push((
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(&path).unwrap(),
                ));
            }
        }
    }

    let mut files = Vec::new();
    collect(root, root, &mut files);
    files
}

fn assert_zero_frontier_and_git_delta(
    dir: &Path,
    before_files: &[(String, Vec<u8>)],
    before_head: &str,
    before_status: &str,
    context: &str,
) {
    assert_eq!(
        snapshot_files(dir),
        before_files,
        "{context}: managed bytes changed"
    );
    assert_eq!(
        git_stdout(dir, &["rev-parse", "HEAD"]),
        before_head,
        "{context}: Git HEAD changed"
    );
    assert_eq!(
        git_stdout(dir, &["status", "--porcelain=v1"]),
        before_status,
        "{context}: Git worktree/index changed"
    );
}

fn pending_finding_proposal() -> vela_protocol::proposals::StateProposal {
    vela_protocol::state::build_add_finding_proposal_at(
        vela_protocol::state::FindingDraftOptions {
            text: "A bounded imported finding".to_string(),
            assertion_type: "computational".to_string(),
            source: "hostile import fixture".to_string(),
            source_type: "researcher_notes".to_string(),
            author: "agent:fixture".to_string(),
            confidence: 0.3,
            evidence_type: "computational".to_string(),
            doi: None,
            year: Some(2026),
            url: None,
            source_authors: Vec::new(),
            source_refs: Vec::new(),
            conditions_text: Some("fixture scope only".to_string()),
            evidence_spans: Vec::new(),
            gap: false,
            negative_space: false,
            replication_attestation: None,
        },
        "2026-07-14T00:00:00Z",
    )
    .unwrap()
}

#[test]
fn every_legacy_finding_apply_spelling_refuses_before_managed_or_git_change() {
    let frontier = tempfile::tempdir().unwrap();
    init_frontier(frontier.path());

    let cases = [
        vec![
            "finding",
            "add",
            ".",
            "--assertion",
            "fixture",
            "--author",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "supersede",
            ".",
            "vf_missing",
            "--assertion",
            "fixture",
            "--author",
            "reviewer:ghost",
            "--reason",
            "fixture",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "note",
            ".",
            "vf_missing",
            "--text",
            "fixture",
            "--author",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "caveat",
            ".",
            "vf_missing",
            "--text",
            "fixture",
            "--author",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "revise",
            ".",
            "vf_missing",
            "--confidence",
            "0.5",
            "--reason",
            "fixture",
            "--as",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "reject",
            ".",
            "vf_missing",
            "--reason",
            "fixture",
            "--as",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "review",
            ".",
            "vf_missing",
            "--status",
            "accepted",
            "--as",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "contribution",
            ".",
            "vf_missing",
            "--unit",
            "whole",
            "--agent-kind",
            "human",
            "--agent-id",
            "reviewer:ghost",
            "--role",
            "reviewed",
            "--as",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
        vec![
            "finding",
            "retract",
            ".",
            "vf_missing",
            "--reason",
            "fixture",
            "--as",
            "reviewer:ghost",
            "--apply",
            "--json",
        ],
    ];

    for args in cases {
        let before_files = snapshot_files(frontier.path());
        let before_head = git_stdout(frontier.path(), &["rev-parse", "HEAD"]);
        let before_status = git_stdout(frontier.path(), &["status", "--porcelain=v1"]);
        let output = run(frontier.path(), &args);
        assert_eq!(
            output.status.code(),
            Some(4),
            "{} did not return custody refusal\n{}",
            args.join(" "),
            output_text(&output)
        );
        let body = output_text(&output);
        assert!(
            body.contains("cannot finalize a finding proposal"),
            "{body}"
        );
        assert!(body.contains("without `--apply`"), "{body}");
        assert!(body.contains("vela sign"), "{body}");
        assert_zero_frontier_and_git_delta(
            frontier.path(),
            &before_files,
            &before_head,
            &before_status,
            &args.join(" "),
        );
    }

    // Parser compatibility did not turn the family read-only: omitting the
    // retired flag still records an ordinary pending proposal and no event.
    let draft = run(
        frontier.path(),
        &[
            "finding",
            "add",
            ".",
            "--assertion",
            "ordinary pending draft",
            "--author",
            "agent:fixture",
            "--json",
        ],
    );
    assert_success(&draft, "create pending finding draft");
    let project = vela_protocol::repo::load_from_path(frontier.path()).unwrap();
    assert_eq!(project.proposals.len(), 1);
    assert_eq!(project.proposals[0].status, "pending_review");
    assert!(project.events.is_empty());
    assert!(project.findings.is_empty());
}

#[test]
fn proposal_import_is_pending_only_and_decided_inputs_have_zero_delta() {
    let frontier = tempfile::tempdir().unwrap();
    let sources = tempfile::tempdir().unwrap();
    init_frontier(frontier.path());
    let pending = pending_finding_proposal();

    let mut decided_cases = Vec::new();
    for status in ["accepted", "applied", "rejected"] {
        let mut proposal = pending.clone();
        proposal.status = status.to_string();
        proposal.reviewed_by = Some("reviewer:ghost".to_string());
        proposal.reviewed_at = Some("2026-07-14T00:01:00Z".to_string());
        proposal.decision_reason = Some("unverified imported verdict".to_string());
        if status == "applied" {
            proposal.applied_event_id = Some("vse_unverified".to_string());
        }
        decided_cases.push((status.to_string(), proposal));
    }
    let mut metadata_only = pending.clone();
    metadata_only.reviewed_by = Some("reviewer:ghost".to_string());
    metadata_only.decision_reason = Some("hidden verdict on pending status".to_string());
    decided_cases.push(("pending-with-decision-metadata".to_string(), metadata_only));

    for (label, proposal) in decided_cases {
        let source: PathBuf = sources.path().join(format!("{label}.json"));
        std::fs::write(&source, serde_json::to_vec_pretty(&proposal).unwrap()).unwrap();
        let before_files = snapshot_files(frontier.path());
        let before_head = git_stdout(frontier.path(), &["rev-parse", "HEAD"]);
        let before_status = git_stdout(frontier.path(), &["status", "--porcelain=v1"]);
        let output = run(
            frontier.path(),
            &[
                "proposals",
                "import",
                ".",
                source.to_str().unwrap(),
                "--json",
            ],
        );
        assert!(
            !output.status.success(),
            "decided import {label} unexpectedly succeeded"
        );
        let body = output_text(&output);
        assert!(
            body.contains("pending_review records only"),
            "{label}: {body}"
        );
        assert!(
            body.contains("signed-authority/event import"),
            "{label}: {body}"
        );
        assert_zero_frontier_and_git_delta(
            frontier.path(),
            &before_files,
            &before_head,
            &before_status,
            &format!("decided proposal import {label}"),
        );
    }

    let source = sources.path().join("pending.json");
    std::fs::write(&source, serde_json::to_vec_pretty(&pending).unwrap()).unwrap();
    let output = run(
        frontier.path(),
        &[
            "proposals",
            "import",
            ".",
            source.to_str().unwrap(),
            "--json",
        ],
    );
    assert_success(&output, "import ordinary pending proposal");
    let project = vela_protocol::repo::load_from_path(frontier.path()).unwrap();
    assert_eq!(project.proposals.len(), 1);
    assert_eq!(project.proposals[0].id, pending.id);
    assert_eq!(project.proposals[0].status, "pending_review");
    assert!(project.proposals[0].reviewed_by.is_none());
    assert!(project.events.is_empty());
    assert!(project.findings.is_empty());
}
