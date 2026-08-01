//! Focused ADR 0016 target-index porcelain regressions.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

mod support;
use support::EphemeralAgent;

fn run(home: &Path, cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_ACTOR_ID")
        .env_remove("VELA_AGENT_KEY_HEX")
        .env_remove("VELA_KEY_PATH");
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    } else {
        command.env_remove("SSH_AUTH_SOCK");
    }
    command.output().expect("run vela")
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn success_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "status={:?}\n{}",
        output.status.code(),
        combined(output)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("decode JSON: {error}\n{}", combined(output)))
}

fn failure_json(output: &Output) -> Value {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("decode failure JSON: {error}\n{}", combined(output)))
}

fn visible_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(current)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path == root.join(".git") {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, files);
            } else if path.is_file() {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

struct Fixture {
    directory: tempfile::TempDir,
    home: tempfile::TempDir,
    agent: EphemeralAgent,
    trust_anchor: std::path::PathBuf,
    frontier_id: String,
    candidate: Value,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.trust_anchor);
    }
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let agent = EphemeralAgent::start(home.path(), "vela target index test");
        let frontier = directory.path();
        success_json(&run(
            home.path(),
            frontier,
            None,
            &[
                "init",
                ".",
                "--name",
                &format!(
                    "Target index fixture {}",
                    directory
                        .path()
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("unique")
                ),
                "--scope",
                "Can one domain candidate become an exact derived offer?",
                "--json",
            ],
        ));
        let authority = success_json(&run(
            home.path(),
            frontier,
            Some(agent.socket()),
            &[
                "authority",
                "init",
                ".",
                "--reason",
                "Establish target-index fixture authority.",
                "--json",
            ],
        ));
        let trust_anchor = std::path::PathBuf::from(
            authority["local_trust"]["anchor_path"]
                .as_str()
                .expect("local trust anchor path"),
        );
        git(frontier, &["config", "user.name", "Vela Test"]);
        git(frontier, &["config", "user.email", "vela@example.invalid"]);
        write(&frontier.join("domain/source.json"), br#"{"open":[1056]}"#);
        git(frontier, &["add", "-A"]);
        git(frontier, &["commit", "-qm", "source"]);
        let source_commit = git(frontier, &["rev-parse", "HEAD^{commit}"]);
        let profile_source = std::fs::read_to_string(frontier.join("frontier.yaml")).unwrap();
        let frontier_id =
            vela_protocol::current_repository::CurrentFrontierProfileV2::from_yaml_str(
                &profile_source,
            )
            .unwrap()
            .frontier_id;
        write(
            &frontier.join("site/problems/1056.json"),
            br#"{"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#,
        );
        let candidate = json!({
            "schema": "vela.target-index-candidate.v1",
            "frontier_id": frontier_id,
            "source": {
                "git_commit": source_commit,
                "input_paths": ["domain/source.json"]
            },
            "targets": [{
                "id": "erdos:1056",
                "title": "Erdős 1056",
                "why": "First exact bounded target.",
                "state": "open",
                "rank": 7,
                "objective": "Produce one bounded artifact.",
                "labels": ["erdos", "open"],
                "packet": {
                    "schema": "erdos-frontier.problem-work.v1",
                    "path": "site/problems/1056.json"
                }
            }]
        });
        write(
            &frontier.join(".vela/tmp/target-index-candidate.json"),
            serde_json::to_vec_pretty(&candidate).unwrap(),
        );
        Self {
            directory,
            home,
            agent,
            trust_anchor,
            frontier_id,
            candidate,
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn command(&self, args: &[&str]) -> Output {
        run(
            self.home.path(),
            self.path(),
            Some(self.agent.socket()),
            args,
        )
    }

    fn check_args(&self) -> Vec<&str> {
        vec![
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--check",
            "--json",
        ]
    }

    fn apply(&self) -> Value {
        success_json(&self.command(&[
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--apply",
            "--json",
        ]))
    }
}

#[test]
fn seal_check_is_zero_writes_and_reports_exact_read_and_touch_sets() {
    let fixture = Fixture::new();
    let before = visible_files(fixture.path());
    let head_before = git(fixture.path(), &["rev-parse", "HEAD"]);
    let cached_before = git(fixture.path(), &["diff", "--cached", "--name-only"]);
    let output = success_json(&fixture.command(&fixture.check_args()));

    assert_eq!(output["schema"], "vela.target-index-seal.v1");
    assert_eq!(output["mode"], "check");
    assert_eq!(output["changed"], false);
    assert_eq!(output["wrote"], json!([]));
    assert_eq!(output["plan"]["frontier_id"], fixture.frontier_id);
    assert_eq!(output["plan"]["input_paths"], json!(["domain/source.json"]));
    assert_eq!(
        output["plan"]["packet_paths"],
        json!(["site/problems/1056.json"])
    );
    assert_eq!(output["plan"]["touched_paths"], json!(["targets.json"]));
    assert!(output["plan"]["canonical_json"].as_str().is_some());
    assert_eq!(visible_files(fixture.path()), before);
    assert_eq!(git(fixture.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(fixture.path(), &["diff", "--cached", "--name-only"]),
        cached_before
    );
    assert!(!fixture.path().join("targets.json").exists());
}

#[test]
fn seal_apply_atomically_writes_only_targets_json_without_staging() {
    let fixture = Fixture::new();
    let before = visible_files(fixture.path());
    let packet_before = std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap();
    let candidate_before =
        std::fs::read(fixture.path().join(".vela/tmp/target-index-candidate.json")).unwrap();
    let output = fixture.apply();

    assert_eq!(output["mode"], "apply");
    assert_eq!(output["changed"], true);
    assert_eq!(output["wrote"], json!(["targets.json"]));
    let mut after = visible_files(fixture.path());
    let target_bytes = after.remove("targets.json").expect("sealed target index");
    assert_eq!(after, before);
    assert_eq!(
        target_bytes,
        output["plan"]["canonical_json"]
            .as_str()
            .unwrap()
            .as_bytes()
    );
    assert_eq!(
        std::fs::read(fixture.path().join("site/problems/1056.json")).unwrap(),
        packet_before
    );
    assert_eq!(
        std::fs::read(fixture.path().join(".vela/tmp/target-index-candidate.json")).unwrap(),
        candidate_before
    );
    assert_eq!(
        git(fixture.path(), &["diff", "--cached", "--name-only"]),
        ""
    );
}

#[test]
fn candidate_is_closed_and_cannot_supply_seal_owned_fields() {
    for (field, value) in [
        ("index_root", json!(format!("sha256:{}", "0".repeat(64)))),
        ("generated_by", json!({"program":"vela","version":"0.0.0"})),
        ("roots", json!({})),
        ("unknown", json!(true)),
    ] {
        let fixture = Fixture::new();
        let mut candidate = fixture.candidate.clone();
        candidate
            .as_object_mut()
            .unwrap()
            .insert(field.to_string(), value);
        write(
            &fixture.path().join(".vela/tmp/target-index-candidate.json"),
            serde_json::to_vec(&candidate).unwrap(),
        );
        let output = fixture.command(&fixture.check_args());
        let payload = failure_json(&output);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(payload["command"], "target-index.seal");
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown field")
        );
        assert!(!fixture.path().join("targets.json").exists());
    }
}

#[test]
fn candidate_cannot_duplicate_the_mutable_repository_binding_as_an_input() {
    let fixture = Fixture::new();
    let mut candidate = fixture.candidate.clone();
    candidate["source"]["input_paths"] = json!([".vela/repository.json", "domain/source.json"]);
    write(
        &fixture.path().join(".vela/tmp/target-index-candidate.json"),
        serde_json::to_vec(&candidate).unwrap(),
    );

    let output = fixture.command(&fixture.check_args());
    let payload = failure_json(&output);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap()
            .contains("duplicates the Target Index repository binding")
    );
    assert!(!fixture.path().join("targets.json").exists());
}

#[test]
fn repair_is_read_only_and_stale_exact_id_inspection_is_never_actionable() {
    let fixture = Fixture::new();
    fixture.apply();
    git(
        fixture.path(),
        &["add", "targets.json", "site/problems/1056.json"],
    );
    git(fixture.path(), &["commit", "-qm", "sealed index"]);
    write(
        &fixture.path().join("site/problems/1056.json"),
        br#"{"changed":true,"problem":1056,"schema":"erdos-frontier.problem-work.v1"}"#,
    );
    let before = visible_files(fixture.path());
    let repair = success_json(&fixture.command(&["target-index", "repair", ".", "--json"]));
    assert_eq!(repair["report"]["schema"], "vela.target-index-repair.v1");
    assert_eq!(
        repair["report"]["changed_declared_paths"],
        json!(["site/problems/1056.json"])
    );
    assert_eq!(
        repair["report"]["repair_command"],
        "vela target-index seal . --candidate .vela/tmp/target-index-candidate.json --check --json"
    );
    assert_eq!(visible_files(fixture.path()), before);

    let inspection =
        success_json(&fixture.command(&["target-index", "inspect", ".", "erdos:1056", "--json"]));
    assert_eq!(inspection["target"]["target_id"], "erdos:1056");
    assert_eq!(inspection["target"]["actionable"], false);
    assert!(
        inspection["target"]["codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "target_index_output_not_tracked")
    );
}

#[test]
fn inspect_requires_an_exact_valid_full_target_id_and_has_no_bypass_flags() {
    let fixture = Fixture::new();
    fixture.apply();
    for (target, kind) in [("invalid", "usage"), ("erdos:10", "not_found")] {
        let output = fixture.command(&["target-index", "inspect", ".", target, "--json"]);
        let payload = failure_json(&output);
        assert_eq!(payload["error"]["kind"], kind);
        assert!(!combined(&output).contains("Erdős 1056"));
    }

    for args in [
        vec![
            "target-index",
            "seal",
            ".",
            "--candidate",
            ".vela/tmp/target-index-candidate.json",
            "--check",
            "--force",
            "--json",
        ],
        vec![
            "target-index",
            "inspect",
            ".",
            "erdos:1056",
            "--non-strict",
            "--json",
        ],
    ] {
        let output = fixture.command(&args);
        assert_eq!(output.status.code(), Some(2), "{}", combined(&output));
    }
}

#[test]
fn seal_refuses_unrelated_dirty_files_and_invalid_packet_outputs() {
    let fixture = Fixture::new();
    write(
        &fixture.path().join("unrelated.txt"),
        b"not part of the seal",
    );
    let output = fixture.command(&fixture.check_args());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        failure_json(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unrelated worktree dirt")
    );
    assert!(!fixture.path().join("targets.json").exists());

    std::fs::remove_file(fixture.path().join("unrelated.txt")).unwrap();
    write(
        &fixture.path().join("site/problems/1056.json"),
        br#"{"schema":"wrong.packet.v1"}"#,
    );
    let output = fixture.command(&fixture.check_args());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        failure_json(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("target_index_packet_mismatch")
    );
    assert!(!fixture.path().join("targets.json").exists());
}
