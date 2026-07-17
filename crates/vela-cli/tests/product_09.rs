//! Focused product regressions for ADR 0010.

use std::collections::BTreeMap;
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
        "sign",
        "check",
        "reproduce",
        "log",
        "doctor",
        "migrate",
    ] {
        assert!(help.contains(command), "missing {command}: {help}");
    }
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
    assert!(value.get("inbox").is_none());
    assert!(value.get("testing").is_none());

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
    let retired_validate = run(temp.path(), temp.path(), &["review", "validate", "--help"]);
    assert_eq!(retired_validate.status.code(), Some(2));

    for (retired, replacement) in [
        ("proposals", "vela review"),
        ("state", "vela finding show"),
        ("hub", "vela-hub"),
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

fn canonical_bytes(frontier: &Path) -> BTreeMap<String, Vec<u8>> {
    fn collect(frontier: &Path, path: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect(frontier, &path, files);
            } else if path.is_file() {
                let name = path
                    .strip_prefix(frontier)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                files.insert(name, std::fs::read(path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    for relative in [
        ".vela/actors.json",
        ".vela/events",
        ".vela/proposals",
        ".vela/artifacts",
    ] {
        let path = frontier.join(relative);
        if path.is_file() {
            files.insert(relative.to_string(), std::fs::read(path).unwrap());
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        collect(frontier, &path, &mut files);
    }
    files
}

#[test]
fn migration_previews_exact_files_preserves_roots_and_refuses_dirty_input() {
    let temp = tempfile::tempdir().unwrap();
    let frontier = temp.path().join("legacy");
    vela_protocol::frontier_repo::initialize(
        &frontier,
        vela_protocol::frontier_repo::InitOptions {
            name: "legacy-migration",
            initialize_git: true,
        },
    )
    .unwrap();
    let manifest_path = frontier.join("frontier.yaml");
    let mut manifest = std::fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("carina:\n  kernel: carina@0.1.0\n");
    std::fs::write(&manifest_path, manifest).unwrap();
    commit_all(&frontier);
    let canonical_before = canonical_bytes(&frontier);
    let git_before = String::from_utf8(git(&frontier, &["rev-parse", "HEAD^{tree}"]).stdout)
        .unwrap()
        .trim()
        .to_string();

    let check = run(
        temp.path(),
        temp.path(),
        &[
            "migrate",
            frontier.to_str().unwrap(),
            "--to",
            "0.900",
            "--check",
            "--json",
        ],
    );
    assert_success(&check);
    let check_value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check_value["schema"], "vela.migration.v1");
    assert_eq!(
        check_value["roots"]["before"],
        check_value["roots"]["after"]
    );
    assert!(
        check_value["touched"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "frontier.yaml")
    );
    assert!(
        std::fs::read_to_string(&manifest_path)
            .unwrap()
            .contains("carina:")
    );

    let apply = run(
        temp.path(),
        temp.path(),
        &[
            "migrate",
            frontier.to_str().unwrap(),
            "--to",
            "0.900",
            "--apply",
            "--json",
        ],
    );
    assert_success(&apply);
    let apply_value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(
        apply_value["roots"]["before"],
        apply_value["roots"]["after"]
    );
    assert_eq!(canonical_bytes(&frontier), canonical_before);
    assert!(
        !std::fs::read_to_string(&manifest_path)
            .unwrap()
            .contains("carina:")
    );
    let git_after = String::from_utf8(git(&frontier, &["rev-parse", "HEAD^{tree}"]).stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(git_after, git_before, "migration must not move Git HEAD");

    std::fs::write(frontier.join("unrelated.txt"), "untracked\n").unwrap();
    let refused = run(
        temp.path(),
        temp.path(),
        &[
            "migrate",
            frontier.to_str().unwrap(),
            "--to",
            "0.900",
            "--check",
            "--json",
        ],
    );
    assert_eq!(refused.status.code(), Some(1));
    assert!(text(&refused).contains("requires a clean checkout"));
}
