//! Focused product regressions for ADR 0010.

use std::path::Path;
use std::process::{Command, Output};

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_ACTOR_ID")
        .env_remove("VELA_KEY_PATH")
        .output()
        .expect("run vela")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={:?}\n{}",
        output.status.code(),
        text(output)
    );
}

fn git(path: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git")
}

fn commit_all(path: &Path) {
    assert_success(&git(path, &["config", "user.email", "test@vela.invalid"]));
    assert_success(&git(path, &["config", "user.name", "Vela Test"]));
    assert_success(&git(path, &["add", "-A"]));
    assert_success(&git(path, &["commit", "-qm", "fixture"]));
}

#[test]
fn compact_contract_exposes_only_the_daily_surface_and_bounded_status() {
    let temp = tempfile::tempdir().unwrap();
    let frontier = temp.path().join("frontier");
    let init = run(
        temp.path(),
        temp.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            "compact-contract",
            "--scope",
            "Does the compact contract preserve exact state?",
            "--json",
        ],
    );
    assert_success(&init);

    let help = run(temp.path(), temp.path(), &["--help"]);
    assert_success(&help);
    let help = text(&help);
    for command in [
        "init",
        "status",
        "next",
        "work",
        "land",
        "review",
        "check",
        "reproduce",
        "verify",
        "log",
        "doctor",
        "migrate",
    ] {
        assert!(help.contains(command), "missing {command}: {help}");
    }
    assert!(!help.contains("  sign"), "default help leaked sign: {help}");
    let advanced = run(temp.path(), temp.path(), &["help", "advanced"]);
    assert_success(&advanced);
    assert!(text(&advanced).contains("historical batch and detached-file signing compatibility"));
    assert!(text(&advanced).contains("target-index  inspect, diagnose, or seal"));
    for retired in ["proposals", "foundry", "atlas", "hub", "publication"] {
        assert!(
            !help.contains(retired),
            "default help leaked {retired}: {help}"
        );
    }

    let status = run(
        temp.path(),
        temp.path(),
        &["status", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&status);
    assert!(status.stdout.len() <= 16 * 1024);
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["schema"], "vela.status.v1");
    assert_eq!(value["integrity"]["replay"], "reproduced");
    assert_eq!(value["integrity"]["strict"], "pass");
    assert!(
        value["roots"]["event_log"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["roots"]["scientific_state_root"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(
        value["roots"]["legacy_snapshot_root"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(value["roots"].get("snapshot").is_none());
    assert_eq!(
        value["integrity"]["repository_context"]["generation"],
        "profile_v1"
    );
    assert_eq!(value["integrity"]["repository_context"]["valid"], true);
    assert_eq!(
        value["roots"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "actor_registry",
            "artifacts",
            "event_log",
            "legacy_snapshot_root",
            "proposals",
            "scientific_state_root",
        ])
    );
    assert!(value.get("inbox").is_none());
    assert!(value.get("testing").is_none());
    assert_eq!(value["counts"]["open_work"], 1);
    assert_eq!(value["counts"]["available_work"], 1);
    assert_eq!(value["counts"]["leased_work"], 0);

    let next = run(
        temp.path(),
        temp.path(),
        &["next", frontier.to_str().unwrap(), "--limit", "1", "--json"],
    );
    assert_success(&next);
    let next: serde_json::Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_eq!(next["schema"], "vela.offer.v1");
    assert_eq!(
        next["availability"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "available",
            "configured",
            "leased",
            "repair_command",
            "returned",
            "stale",
        ])
    );
    assert_eq!(next["availability"]["configured"], 1);
    assert_eq!(next["availability"]["stale"], 0);
    assert_eq!(next["availability"]["available"], 1);
    assert_eq!(next["availability"]["leased"], 0);
    assert_eq!(next["availability"]["returned"], 1);
    assert_eq!(
        next["availability"]["repair_command"],
        format!("vela target-index repair {} --json", frontier.display())
    );
    assert_eq!(next["targets"][0]["target_id"], "seed:first");
    assert_eq!(next["leased_targets"], serde_json::json!([]));

    let graph_rank = run(
        temp.path(),
        temp.path(),
        &["frontier", "rank", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&graph_rank);
    let graph_rank: serde_json::Value = serde_json::from_slice(&graph_rank.stdout).unwrap();
    assert_eq!(graph_rank["ranking_kind"], "structural_opportunity");
    assert_eq!(graph_rank["authority"], "advice_only");
    assert_eq!(graph_rank["work_queue"], false);
    assert_eq!(graph_rank["producer_work_command"], "vela next . --json");

    let review = run(
        temp.path(),
        temp.path(),
        &["review", "list", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&review);
    let review: serde_json::Value = serde_json::from_slice(&review.stdout).unwrap();
    assert_eq!(review["schema"], "vela.review.v1");
    assert_eq!(review["returned"], 0);
    assert!(review.get("review").is_none());

    let mut project = vela_protocol::repo::load_from_path(&frontier).unwrap();
    for (index, created_at) in [
        "2026-07-17T12:00:00Z",
        "2026-07-17T12:02:00Z",
        "2026-07-17T12:01:00Z",
    ]
    .into_iter()
    .enumerate()
    {
        let mut proposal = vela_protocol::proposals::new_proposal_at(
            "finding.add",
            vela_protocol::events::StateTarget {
                r#type: "finding".to_string(),
                id: format!("vf_compact_{index}"),
            },
            "agent:compact",
            "agent",
            format!("compact review fixture {index}"),
            serde_json::json!({
                "finding": {
                    "id": format!("vf_compact_{index}"),
                    "assertion": {"text": format!("claim {index}"), "type": "computational"},
                    "conditions": {"text": "fixture"},
                    "confidence": {"score": 0.1},
                    "flags": {"contested": false}
                }
            }),
            Vec::new(),
            vec!["fixture only".to_string()],
            created_at.to_string(),
        );
        if index == 1 {
            proposal.payload["claim"] = serde_json::json!("misleading sibling claim");
        }
        project.proposals.push(proposal);
    }
    vela_protocol::repo::save_to_path(&frontier, &project).unwrap();

    let first = run(
        temp.path(),
        temp.path(),
        &[
            "review",
            "list",
            frontier.to_str().unwrap(),
            "--limit",
            "2",
            "--json",
        ],
    );
    assert_success(&first);
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["order"], "created_at_desc_then_proposal_id");
    assert_eq!(first["items"][0]["created_at"], "2026-07-17T12:02:00Z");
    assert_eq!(first["items"][0]["claim"], "claim 1");
    assert_eq!(first["items"][1]["created_at"], "2026-07-17T12:01:00Z");
    assert_eq!(first["items"][1]["claim"], "claim 2");
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = run(
        temp.path(),
        temp.path(),
        &[
            "review",
            "list",
            frontier.to_str().unwrap(),
            "--limit",
            "2",
            "--cursor",
            cursor,
            "--json",
        ],
    );
    assert_success(&second);
    let second: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second["items"][0]["created_at"], "2026-07-17T12:00:00Z");
    assert_eq!(second["next_cursor"], serde_json::Value::Null);

    let retired_validate = run(temp.path(), temp.path(), &["review", "validate", "--help"]);
    assert_eq!(retired_validate.status.code(), Some(2));

    for (retired, replacement) in [
        ("proposals", "vela review list"),
        ("state", "vela finding show"),
        ("hub", "vela serve"),
        ("atlas", "Canopus"),
    ] {
        let output = run(temp.path(), temp.path(), &[retired]);
        assert_eq!(output.status.code(), Some(2));
        let output = text(&output);
        assert!(output.contains("retired"), "{retired}: {output}");
        assert!(output.contains(replacement), "{retired}: {output}");
    }
}

#[test]
fn nested_frontier_failures_keep_stable_dotted_command_ids() {
    let temp = tempfile::tempdir().unwrap();

    let bind = run(
        temp.path(),
        temp.path(),
        &[
            "frontier",
            "bind",
            temp.path().to_str().unwrap(),
            "--reason",
            "exercise the failure envelope",
            "--json",
        ],
    );
    assert!(!bind.status.success(), "{}", text(&bind));
    assert!(bind.stderr.is_empty(), "{}", text(&bind));
    let bind: serde_json::Value = serde_json::from_slice(&bind.stdout).unwrap();
    assert_eq!(bind["ok"], false);
    assert_eq!(bind["command"], "frontier.bind");

    let recover = run(
        temp.path(),
        temp.path(),
        &[
            "frontier",
            "recover-publication",
            "--operation",
            "vop_0000000000000000000000000000000000000000000000000000000000000000",
            "--json",
        ],
    );
    assert!(!recover.status.success(), "{}", text(&recover));
    assert!(recover.stderr.is_empty(), "{}", text(&recover));
    let recover: serde_json::Value = serde_json::from_slice(&recover.stdout).unwrap();
    assert_eq!(recover["ok"], false);
    assert_eq!(recover["command"], "frontier.recover-publication");
}

#[cfg(unix)]
#[test]
fn compact_status_ignores_hostile_repository_git_configuration() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let frontier = temp.path().join("frontier");
    let init = run(
        temp.path(),
        temp.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            "hostile-git-status",
            "--scope",
            "Can repository Git configuration influence compact status?",
            "--json",
        ],
    );
    assert_success(&init);
    commit_all(&frontier);

    let git_dir = frontier.join(".git");
    let canary = git_dir.join("status-fsmonitor-ran");
    let helper = git_dir.join("status-fsmonitor");
    std::fs::write(
        &helper,
        format!(
            "#!/bin/sh\nprintf ran > '{}'\nprintf '0\\n'\n",
            canary.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_success(&git(
        &frontier,
        &["config", "core.fsmonitor", helper.to_str().unwrap()],
    ));

    let status = run(
        temp.path(),
        temp.path(),
        &["status", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&status);
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["git"]["clean"], true);
    assert_eq!(value["integrity"]["repository_context"]["valid"], true);
    assert!(
        !canary.exists(),
        "compact status executed repository-configured fsmonitor code"
    );
}

#[cfg(unix)]
#[test]
fn compact_read_projections_work_without_checkout_write_access() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let frontier = temp.path().join("read-only");
    let init = run(
        temp.path(),
        temp.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            "read-only",
            "--scope",
            "Can an exact frontier be inspected without write access?",
            "--json",
        ],
    );
    assert_success(&init);
    let journals = frontier.join(".vela/operation-journals");
    if journals.exists() {
        std::fs::remove_dir_all(&journals).unwrap();
    }

    fn set_tree_mode(path: &Path, mode: u32) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                set_tree_mode(&entry.unwrap().path(), mode);
            }
        }
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(path, permissions).unwrap();
    }
    set_tree_mode(&frontier, 0o555);

    let status = run(
        temp.path(),
        temp.path(),
        &["status", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&status);
    let missing = run(
        temp.path(),
        temp.path(),
        &[
            "review",
            "show",
            frontier.to_str().unwrap(),
            "vpr_0000000000000000",
            "--json",
        ],
    );
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        !text(&missing).contains("frontier lock"),
        "{}",
        text(&missing)
    );

    set_tree_mode(&frontier, 0o755);
    assert!(
        !journals.exists(),
        "read projections must not create operational state"
    );
}

#[test]
fn init_minimal_requires_bounded_inputs_and_omits_optional_scaffolding() {
    let temp = tempfile::tempdir().unwrap();
    let refused = run(
        temp.path(),
        temp.path(),
        &["init", "missing-scope", "--name", "fixture", "--json"],
    );
    assert_eq!(refused.status.code(), Some(2));
    assert!(text(&refused).contains("requires --scope"));

    let frontier = temp.path().join("minimal");
    let initialized = run(
        temp.path(),
        temp.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            "minimal",
            "--scope",
            "Can an empty frontier replay without optional integrations?",
            "--json",
        ],
    );
    assert_success(&initialized);
    for absent in [".mcp.json", ".github/workflows/vela-frontier.yml", "proof"] {
        assert!(!frontier.join(absent).exists(), "unexpected {absent}");
    }
    for present in [
        ".vela/actors.json",
        "README.md",
        "SCOPE.md",
        "VELA.md",
        "frontier.yaml",
        "frontier.json",
        "vela.lock",
        ".gitignore",
        ".gitattributes",
    ] {
        assert!(frontier.join(present).exists(), "missing {present}");
    }
    let check = run(
        temp.path(),
        temp.path(),
        &["check", frontier.to_str().unwrap(), "--json"],
    );
    assert_success(&check);
}
