//! Cross-surface regressions for ADR 0003's task-first trust boundary.

use std::path::Path;
use std::process::{Command, Output};

fn vela_bin() -> &'static str {
    env!("CARGO_BIN_EXE_vela")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    run_with_env(dir, args, &[])
}

fn run_with_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(vela_bin());
    command
        .current_dir(dir)
        .args(args)
        .env("HOME", dir)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    for (key, _) in std::env::vars() {
        if key.starts_with("VELA_") && key != "VELA_ADVICE" {
            command.env_remove(key);
        }
    }
    command.envs(env.iter().copied());
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

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git(dir, args);
    assert_success(&output, &format!("git {}", args.join(" ")));
    String::from_utf8(output.stdout).unwrap()
}

fn init_git_frontier(dir: &Path) {
    assert_success(
        &run(dir, &["init", ".", "--name", "task-first", "--json"]),
        "init frontier",
    );
    assert_success(
        &run(dir, &["id", "create", "--handle", "t"]),
        "create test identity",
    );
    assert_success(
        &git(dir, &["config", "user.email", "test@vela.invalid"]),
        "git email",
    );
    assert_success(&git(dir, &["config", "user.name", "Vela Test"]), "git name");
    assert_success(&git(dir, &["add", "-A"]), "stage baseline");
    assert_success(&git(dir, &["commit", "-qm", "baseline"]), "commit baseline");
}

fn write_receipt(dir: &Path, filename: &str, claim: &str) {
    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    std::fs::write(dir.join("artifacts/w.json"), r#"{"witness":true}"#).unwrap();
    let receipt = serde_json::json!({
        "schema": "vela.receipt.v1",
        "claim": claim,
        "type": "computational",
        "replayability": "exact",
        "artifacts": [{"path": "artifacts/w.json", "kind": "witness"}],
        "caveats": ["fixture evidence only"],
        "verifier_runs": [{"method": "fixture", "outcome": "pass", "log": "ok"}],
    });
    std::fs::write(
        dir.join(filename),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

fn one_json_object(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON value: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn snapshot_scientific_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        if !path.exists() {
            return;
        }
        if path.is_dir() {
            let mut entries = std::fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for entry in entries {
                collect(root, &entry, out);
            }
        } else {
            out.push((
                path.strip_prefix(root).unwrap().display().to_string(),
                std::fs::read(path).unwrap(),
            ));
        }
    }

    let mut out = Vec::new();
    for relative in [".vela", "records", "frontier.json", "vela.lock", "proof"] {
        collect(dir, &dir.join(relative), &mut out);
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

#[test]
fn json_mode_writes_one_object_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "one-object JSON publication regression",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land with publication");
    let value = one_json_object(&output);
    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "land");
    assert!(value["operation_id"].as_str().is_some(), "{value}");
    assert!(value.get("publication").is_some(), "{value}");
}

#[test]
fn invalid_land_human_output_reports_zero_delta_and_safe_next_command() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run_with_env(
        tmp.path(),
        &[
            "land",
            "--claim",
            "invalid artifact input changes nothing",
            "--artifact",
            "artifacts/missing.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
        ],
        &[("VELA_ADVICE", "1")],
    );
    assert!(!output.status.success(), "missing artifact must fail");
    assert!(
        output.stdout.is_empty(),
        "human error stdout must stay empty"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unchanged"), "{stderr}");
    assert!(
        stderr.contains("canonical Vela state, Git refs, index, and worktree"),
        "{stderr}"
    );
    assert!(stderr.contains("retained"), "{stderr}");
    assert!(stderr.contains("vop_"), "{stderr}");
    assert!(stderr.contains("next"), "{stderr}");
    assert!(stderr.contains("vela land --help"), "{stderr}");
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn concurrent_publication_owner_blocks_scientific_mutation() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "busy publication must not race a scientific write",
    );

    let lock_path = tmp.path().join(".git/vela/publication.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let publication_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    publication_lock.lock().unwrap();

    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert!(!output.status.success(), "busy land must retry, not mutate");
    let value = one_json_object(&output);
    assert_eq!(value["ok"], false, "{value}");
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("scientific state was not changed")),
        "{value}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn failed_push_reports_committed_local() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "failed push retains local publication",
    );

    let output = run(
        tmp.path(),
        &[
            "land",
            "receipt.json",
            "--as",
            "agent:t",
            "--push",
            "--json",
        ],
    );
    assert_success(&output, "land despite push failure");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");
    assert!(value["publication"]["commit"].as_str().is_some(), "{value}");
    assert!(
        value["publication"]["recovery_command"]
            .as_str()
            .is_some_and(|command| {
                command.starts_with("vela publication recover --operation vop_")
                    && command.ends_with(" --push")
            }),
        "{value}"
    );
}

#[test]
fn failed_push_human_output_separates_request_from_publication_recovery() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "failed push human output has distinct identities",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--push"],
    );
    assert_success(&output, "human land despite push failure");
    assert!(output.stderr.is_empty(), "unexpected stderr");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let request = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("request"))
        .expect("request line");
    let retained = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("retained"))
        .expect("retained line");
    let remote = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("remote"))
        .expect("remote line");
    let next = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("next"))
        .expect("next line");
    assert!(request.contains("vop_"), "{stdout}");
    assert!(retained.contains("local commit"), "{stdout}");
    assert!(!retained.contains("vop_"), "{stdout}");
    assert!(remote.contains("unverified"), "{stdout}");
    assert!(
        next.contains("vela publication recover --operation vop_") && next.ends_with(" --push"),
        "{stdout}"
    );
}

#[test]
fn publication_never_commits_callers_index() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::write(tmp.path().join("notes.txt"), "baseline\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage notes baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "notes baseline"]),
        "commit notes baseline",
    );

    std::fs::write(tmp.path().join("notes.txt"), "caller staged bytes\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage caller bytes",
    );
    write_receipt(tmp.path(), "receipt.json", "path-scoped publication");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "path-scoped land");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");

    let committed_paths = git_stdout(
        tmp.path(),
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(
        !committed_paths.lines().any(|path| path == "notes.txt"),
        "publication captured the caller's staged file: {committed_paths}"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["show", ":notes.txt"]),
        "caller staged bytes\n",
        "publication changed the caller's staged entry"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["show", "HEAD:notes.txt"]),
        "baseline\n",
        "publication committed unrelated staged bytes"
    );
}

#[test]
fn publication_preserves_unstaged_work() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::write(tmp.path().join("notes.txt"), "baseline\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage notes baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "notes baseline"]),
        "commit notes baseline",
    );

    std::fs::write(tmp.path().join("notes.txt"), "caller unstaged bytes\n").unwrap();
    std::fs::write(tmp.path().join("scratch.txt"), "caller untracked bytes\n").unwrap();
    write_receipt(tmp.path(), "receipt.json", "preserve unstaged publication");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land around unstaged work");
    one_json_object(&output);

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("notes.txt")).unwrap(),
        "caller unstaged bytes\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("scratch.txt")).unwrap(),
        "caller untracked bytes\n"
    );
    let status = git_stdout(tmp.path(), &["status", "--porcelain=v1"]);
    assert!(
        status.lines().any(|line| line == " M notes.txt"),
        "{status}"
    );
    assert!(
        status.lines().any(|line| line == "?? scratch.txt"),
        "{status}"
    );
}

#[test]
fn publication_refuses_overlapping_vela_edits() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("sources")).unwrap();
    let caller_source = tmp.path().join("sources/preexisting.txt");
    std::fs::write(&caller_source, "baseline source\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "sources/preexisting.txt"]),
        "stage source baseline",
    );
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "source baseline"]),
        "commit source baseline",
    );
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    std::fs::write(
        &caller_source,
        "baseline source\ncaller-owned pre-existing edit\n",
    )
    .unwrap();
    write_receipt(
        tmp.path(),
        "receipt.json",
        "scientific write survives publication overlap refusal",
    );

    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "land despite publication overlap refusal");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "uncommitted", "{value}");
    assert!(
        value["publication"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("pre-existing unstaged Vela edit")),
        "{value}"
    );
    assert_eq!(
        git_stdout(tmp.path(), &["rev-parse", "HEAD"]),
        head_before,
        "preflight refusal must leave HEAD unchanged"
    );
    let proposal_id = value["proposal_id"].as_str().unwrap();
    assert!(
        tmp.path()
            .join(".vela/proposals")
            .join(format!("{proposal_id}.json"))
            .is_file(),
        "scientific landing must remain durable when Git publication refuses"
    );
    assert!(
        std::fs::read_to_string(caller_source)
            .unwrap()
            .contains("caller-owned pre-existing edit")
    );
}

#[cfg(unix)]
#[test]
fn publication_bypasses_all_repository_hooks() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let hooks = tmp.path().join(".git/hooks");
    let marker = tmp.path().join("hook-ran.marker");
    let quoted_marker = marker.display().to_string().replace('\'', "'\\''");
    let body = format!("#!/bin/sh\nprintf hook-ran > '{quoted_marker}'\nexit 91\n");
    for name in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "reference-transaction",
        "post-rewrite",
    ] {
        let path = hooks.join(name);
        std::fs::write(&path, &body).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    write_receipt(tmp.path(), "receipt.json", "hooks are not authority");
    let output = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&output, "hook-bypassing publication");
    one_json_object(&output);
    assert!(
        !marker.exists(),
        "repository hook ran during Vela publication"
    );
}

#[cfg(unix)]
#[test]
fn publication_scrubs_inherited_git_config_injection() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let injected_hooks = tmp.path().join("injected-hooks");
    std::fs::create_dir(&injected_hooks).unwrap();
    let marker = tmp.path().join("injected-hook-ran.marker");
    let quoted_marker = marker.display().to_string().replace('\'', "'\\''");
    let hook = injected_hooks.join("reference-transaction");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\nprintf hook-ran > '{quoted_marker}'\nexit 91\n"),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();

    write_receipt(
        tmp.path(),
        "receipt.json",
        "inherited Git config is not publication authority",
    );
    let hooks_value = injected_hooks.to_string_lossy().into_owned();
    let output = run_with_env(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
        &[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.hooksPath"),
            ("GIT_CONFIG_VALUE_0", hooks_value.as_str()),
        ],
    );
    assert_success(&output, "publication under hostile inherited Git config");
    let value = one_json_object(&output);
    assert_eq!(value["publication"]["state"], "committed_local", "{value}");
    assert!(
        !marker.exists(),
        "an inherited GIT_CONFIG_* hook ran during publication"
    );
}

#[test]
fn mcp_profiles_expose_no_finalizer() {
    use vela_edge::tool_registry::{McpProfile, get_tool, tools_for_profile};

    assert!(get_tool("decide").is_none());
    for profile in [
        McpProfile::ReadOnly,
        McpProfile::Draft,
        McpProfile::Maintainer,
    ] {
        assert!(
            tools_for_profile(profile)
                .iter()
                .all(|tool| tool.name != "decide"),
            "removed finalizer leaked into {} MCP discovery",
            profile.as_str()
        );
    }
    let draft = tools_for_profile(McpProfile::Draft)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    let maintainer = tools_for_profile(McpProfile::Maintainer)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    assert_eq!(maintainer, draft, "maintainer must only alias draft");
}

#[test]
fn untrusted_terminal_text_is_escaped() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_success(
        &run(
            tmp.path(),
            &["init", ".", "--name", "safe-text", "--no-git", "--json"],
        ),
        "init frontier",
    );
    let receipt = serde_json::json!({
        "schema": "vela.receipt.v1",
        "claim": "safe failure path",
        "type": "computational",
        "replayability": "bad\u{001b}]8;;https://bad.example\u{0007}\u{202e}",
        "caveats": ["fixture"],
    });
    std::fs::write(
        tmp.path().join("receipt.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();

    let output = run(tmp.path(), &["land", "receipt.json", "--as", "agent:t"]);
    assert!(!output.status.success(), "hostile receipt must be rejected");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!rendered.contains('\u{001b}'), "{rendered:?}");
    assert!(!rendered.contains('\u{0007}'), "{rendered:?}");
    assert!(!rendered.contains('\u{202e}'), "{rendered:?}");
    assert!(rendered.contains("\\u{001B}"), "{rendered:?}");
    assert!(rendered.contains("\\u{202E}"), "{rendered:?}");
}
