//! Cold-start CLI contract for a native repository before repository authority.

use std::collections::BTreeMap;
use std::path::Path;
#[cfg(feature = "test-support")]
use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

mod support;
use support::EphemeralAgent;

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn unique_name(prefix: &str, temporary: &tempfile::TempDir) -> String {
    format!(
        "{} {}",
        prefix,
        temporary
            .path()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("unique")
    )
}

fn run(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    run_with_advice_setting(cwd, socket, args, "0")
}

fn run_with_advice(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Output {
    run_with_advice_setting(cwd, socket, args, "1")
}

fn run_with_advice_setting(
    cwd: &Path,
    socket: Option<&Path>,
    args: &[&str],
    advice: &str,
) -> Output {
    let mut command = vela_command(cwd, socket, args);
    command.env("VELA_ADVICE", advice);
    command.output().expect("run vela")
}

fn vela_command(cwd: &Path, socket: Option<&Path>, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0");
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    } else {
        command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock"));
    }
    command
}

#[cfg(feature = "test-support")]
fn run_with_test_failpoint(
    cwd: &Path,
    socket: Option<&Path>,
    args: &[&str],
    failpoint: &str,
) -> Output {
    vela_command(cwd, socket, args)
        .env(failpoint, "1")
        .output()
        .expect("run failpoint-injected vela")
}

#[cfg(feature = "test-support")]
fn git_output(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("run fixture Git")
}

#[cfg(feature = "test-support")]
fn git_text(root: &Path, args: &[&str]) -> String {
    let output = git_output(root, args);
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git text is UTF-8")
        .trim()
        .to_string()
}

#[cfg(feature = "test-support")]
fn operation_id(repository: &Path) -> String {
    let journals = repository.join(".vela/operation-journals/repository");
    let mut operations = std::fs::read_dir(journals)
        .expect("read operation journals")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let name = entry.file_name().into_string().ok()?;
            (metadata.is_file() && name.starts_with("vop_") && name.ends_with(".json"))
                .then(|| name.trim_end_matches(".json").to_string())
        })
        .collect::<Vec<_>>();
    operations.sort();
    assert_eq!(operations.len(), 1, "one exact native genesis operation");
    operations.remove(0)
}

#[cfg(feature = "test-support")]
fn agent_fingerprint(agent_root: &Path) -> String {
    let output = Command::new("ssh-keygen")
        .args(["-lf", "-", "-E", "sha256"])
        .stdin(std::fs::File::open(agent_root.join("repository_authority.pub")).unwrap())
        .output()
        .expect("fingerprint disposable authority key");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .nth(1)
        .expect("ssh-keygen fingerprint")
        .to_string()
}

#[cfg(feature = "test-support")]
fn arm_anchor(payload: &Value) -> RemoveOnDrop {
    RemoveOnDrop(PathBuf::from(
        payload["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ))
}

#[cfg(feature = "test-support")]
fn expected_anchor_path(repository: &Path) -> PathBuf {
    let profile = vela_protocol::repository::RepositoryProfileV1::from_toml_str(
        &std::fs::read_to_string(repository.join("vela.toml")).unwrap(),
    )
    .unwrap();
    PathBuf::from(std::env::var_os("HOME").expect("test HOME"))
        .join(".vela/trust/authorities")
        .join(format!("{}.json", profile.repository_id))
}

#[cfg(all(feature = "test-support", unix))]
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(all(feature = "test-support", unix))]
fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    chmod(path, 0o755);
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode Vela JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn directory_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).expect("snapshot metadata");
            if metadata.is_dir() {
                visit(base, &path, files);
            } else if metadata.is_file() {
                files.insert(
                    path.strip_prefix(base)
                        .expect("snapshot path under root")
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(path).expect("snapshot file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn replay_is_the_only_repository_replay_verb() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let help = run(temporary.path(), None, &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("replay"));
    assert!(!help.contains("check"));

    let retired = run(temporary.path(), None, &["check", "--json"]);
    assert_eq!(retired.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&retired.stderr).contains("unrecognized subcommand 'check'"));

    let verification_help = run(
        temporary.path(),
        None,
        &["verification", "record", "--help"],
    );
    assert!(verification_help.status.success());
    let verification_help = String::from_utf8_lossy(&verification_help.stdout);
    assert!(verification_help.contains("agent:<name>, ci:<name>, or verifier:<name>"));
    assert!(!verification_help.contains("reviewer:<you>"));
}

#[test]
fn bootstrap_discovery_and_blocked_commands_name_the_one_valid_next_action() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let name = unique_name("Cold-start UX", &temporary);
    let initialized = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--name",
            &name,
            "--scope",
            "Exercise phase-aware CLI diagnostics.",
            "--json",
        ],
    );
    assert_eq!(initialized.status.code(), Some(1));
    let initialized = json(&initialized);
    assert_eq!(initialized["command"], "init");
    assert!(
        initialized["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("signing could not complete"))
    );
    let init_hint = initialized["error"]["hint"]
        .as_str()
        .expect("init recovery hint");
    assert!(init_hint.contains("ssh-add /path/to/private-key"));
    assert!(init_hint.contains("start ssh-agent first on Linux"));
    assert!(init_hint.contains("docs/QUICKSTART.md#first-time-authority-key-setup"));
    assert!(init_hint.contains(&format!("vela init '{repository_path_text}'")));
    assert!(init_hint.ends_with("first-time-authority-key-setup"));

    let nested = repository_path.join("notes/drafts");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let status = run(&nested, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    /* One command, one document: a cold repository and a replaying one both
    answer `vela.status.v4`, and the phase is a value inside it. */
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["actions"]["work"]["mode"], "authority_uninitialized");
    assert!(
        status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela init "))
    );
    assert_eq!(status["actions"]["review"], serde_json::Value::Null);
    assert_eq!(status["integrity"]["replay"], "not_initialized");
    assert_eq!(status["integrity"]["strict"], "blocked");
    assert_eq!(
        status["integrity"]["blockers_by_code"]["repository_authority_uninitialized"],
        1
    );

    for args in [
        vec!["replay", "--json"],
        vec!["review", "inbox", &repository_path_text, "--json"],
        vec!["show", &repository_path_text, "vcl_missing", "--json"],
    ] {
        let blocked = run(&nested, None, &args);
        assert_eq!(blocked.status.code(), Some(1), "args={args:?}");
        let blocked = json(&blocked);
        assert_eq!(blocked["ok"], false, "args={args:?}");
        assert_eq!(blocked["error"]["kind"], "domain", "args={args:?}");
        assert_eq!(
            blocked["error"]["message"], "repository authority is not initialized",
            "args={args:?}"
        );
        assert!(
            blocked["error"]["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("vela init")),
            "args={args:?}"
        );
    }

    std::fs::remove_dir_all(repository_path.join("notes"))
        .expect("remove nested read-only discovery fixture before exact genesis publication");
    let agent = EphemeralAgent::start(temporary.path(), "vela resumable init test");
    let resumed = run(
        temporary.path(),
        Some(agent.socket()),
        &["init", &repository_path_text, "--json"],
    );
    assert!(resumed.status.success());
    let resumed = json(&resumed);
    assert_eq!(resumed["schema"], "vela.repository-init.v1");
    assert_eq!(resumed["resumed"], true);
    assert_eq!(resumed["authority"]["state"], "initialized");
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        resumed["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));
}

#[test]
fn human_init_recovery_keeps_the_resume_command_human_readable() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let initialized = run_with_advice(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--name",
            "Human recovery",
            "--scope",
            "Explain first-time key setup without switching output modes.",
        ],
    );

    assert_eq!(initialized.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&initialized.stderr);
    assert!(stderr.contains("ssh-add /path/to/private-key"));
    assert!(stderr.contains("key setup: https://github.com/vela-science/vela/"));
    assert!(stderr.contains(&format!("vela init '{repository_path_text}'")));
    assert!(!stderr.contains("--json"));
}

#[test]
fn init_creates_a_signed_ready_repository_in_one_command() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela one-step init test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let name = unique_name("Ready repository", &temporary);
    let initialized = run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &name,
            "--scope",
            "Exercise one-command initialization.",
            "--json",
        ],
    );
    assert!(initialized.status.success());
    let initialized = json(&initialized);
    assert_eq!(initialized["schema"], "vela.repository-init.v1");
    assert_eq!(initialized["authority"]["state"], "initialized");
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        initialized["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));
    assert!(
        initialized["repository"]["repository_root"]
            .as_str()
            .is_some_and(|root| root.starts_with("sha256:"))
    );
    assert!(initialized["repository"]["git_commit"].as_str().is_some());

    let readme =
        std::fs::read_to_string(repository_path.join("README.md")).expect("repository README");
    assert!(readme.contains("## Operator loop"));
    assert!(readme.contains("git add -- verification/method.json"));
    assert!(readme.contains("vela verification record"));
    assert!(readme.contains("vela review inbox"));
    assert!(readme.contains("vela review accept"));
    assert!(readme.contains("--if-entry-root"));
    let agent_charter = std::fs::read_to_string(repository_path.join("AGENTS.md"))
        .expect("repository agent charter");
    assert!(agent_charter.contains("tracked, clean, and retained"));
    assert!(agent_charter.contains("vela verification record"));
    assert!(agent_charter.contains("vela review inbox"));
    assert!(agent_charter.contains("do not decide it yourself"));

    let status = run(&repository_path, None, &["status", "--json"]);
    assert!(status.status.success());
    let status = json(&status);
    /* The same literal the cold-start test asserts, so the two branches cannot
    drift back into two documents without one of them failing here. */
    assert_eq!(status["schema"], "vela.status.v4");
    assert_eq!(status["integrity"]["strict"], "pass");
    assert_eq!(status["actions"]["work"]["mode"], "direct_submission");
    assert!(
        status["actions"]["work"]["command"]
            .as_str()
            .is_some_and(|command| command.starts_with("vela submit "))
    );
}

#[test]
fn completed_native_genesis_finishes_git_and_trust_without_a_signer() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela completed genesis continuation");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let reason = "Establish one resumable native genesis.";
    let first = run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &unique_name("Resumable genesis", &temporary),
            "--scope",
            "Finish only the post-transaction Git and trust tail.",
            "--reason",
            reason,
            "--json",
        ],
    );
    assert!(
        first.status.success(),
        "initial init: stdout={} stderr={}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first = json(&first);
    let operation_id = first["operation_id"]
        .as_str()
        .expect("native genesis operation id")
        .to_string();
    assert!(operation_id.starts_with("vop_"));
    let fingerprint = first["authority"]["key_fingerprint"]
        .as_str()
        .expect("repository key fingerprint")
        .to_string();
    let anchor_path = std::path::PathBuf::from(
        first["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    );
    let _anchor = RemoveOnDrop(anchor_path.clone());
    let authority_before = directory_snapshot(&repository_path.join(".vela/authority"));
    let journals_before = directory_snapshot(&repository_path.join(".vela/operation-journals"));
    let initial_commit = first["repository"]["git_commit"]
        .as_str()
        .expect("genesis commit")
        .to_string();

    std::fs::remove_file(&anchor_path).expect("remove post-genesis trust anchor");
    let deleted = Command::new("git")
        .current_dir(&repository_path)
        .args(["update-ref", "-d", "refs/heads/main"])
        .output()
        .expect("remove genesis ref");
    assert!(deleted.status.success());
    let continued = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            &fingerprint,
            "--reason",
            reason,
            "--json",
        ],
    );
    assert!(
        continued.status.success(),
        "continued init: stdout={} stderr={}",
        String::from_utf8_lossy(&continued.stdout),
        String::from_utf8_lossy(&continued.stderr)
    );
    let continued = json(&continued);
    assert_eq!(continued["schema"], "vela.repository-init.v1");
    assert_eq!(continued["resumed"], true);
    assert_eq!(continued["operation_id"], operation_id);
    assert_eq!(continued["repository"]["git_commit"], initial_commit);
    assert_eq!(
        continued["repository"]["repository_root"],
        first["repository"]["repository_root"]
    );
    assert_eq!(
        directory_snapshot(&repository_path.join(".vela/authority")),
        authority_before,
        "continuation must not sign or rewrite authority"
    );
    assert_eq!(
        directory_snapshot(&repository_path.join(".vela/operation-journals")),
        journals_before,
        "continuation must not create or rewrite a transaction"
    );

    // Crash after the exact commit but before trust: only the pin is missing.
    std::fs::remove_file(&anchor_path).expect("remove trust anchor after exact commit");
    let trust_only = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            &fingerprint,
            "--reason",
            reason,
            "--json",
        ],
    );
    assert!(trust_only.status.success());
    let trust_only = json(&trust_only);
    assert_eq!(trust_only["operation_id"], operation_id);
    assert_eq!(trust_only["repository"]["git_commit"], initial_commit);

    // Crash after trust (or a lost response): the exact command is idempotent.
    let idempotent = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            &fingerprint,
            "--reason",
            reason,
            "--json",
        ],
    );
    assert!(idempotent.status.success());
    let idempotent = json(&idempotent);
    assert_eq!(idempotent["operation_id"], operation_id);
    assert_eq!(idempotent["repository"]["git_commit"], initial_commit);
    assert_eq!(
        directory_snapshot(&repository_path.join(".vela/authority")),
        authority_before
    );
    assert_eq!(
        directory_snapshot(&repository_path.join(".vela/operation-journals")),
        journals_before
    );

    let git_before_wrong_key = directory_snapshot(&repository_path.join(".git"));
    let anchor_before_wrong_key = std::fs::read(&anchor_path).expect("read exact trust anchor");
    let wrong_key = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "--reason",
            reason,
            "--json",
        ],
    );
    assert_eq!(wrong_key.status.code(), Some(1));
    assert!(
        json(&wrong_key)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--key does not match"))
    );
    assert_eq!(
        directory_snapshot(&repository_path.join(".git")),
        git_before_wrong_key,
        "a mismatched retained key must not rewrite Git"
    );
    assert_eq!(
        std::fs::read(&anchor_path).expect("reread exact trust anchor"),
        anchor_before_wrong_key,
        "a mismatched retained key must not rewrite trust"
    );

    let removed_ref = Command::new("git")
        .current_dir(&repository_path)
        .args(["update-ref", "-d", "refs/heads/main"])
        .output()
        .expect("remove genesis ref with exact trust retained");
    assert!(removed_ref.status.success());
    let git_without_ref = directory_snapshot(&repository_path.join(".git"));
    let exact_pin_without_ref = std::fs::read(&anchor_path).expect("read exact retained pin");
    let inconsistent_tail = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            &fingerprint,
            "--reason",
            reason,
            "--json",
        ],
    );
    assert_eq!(inconsistent_tail.status.code(), Some(1));
    assert_eq!(
        directory_snapshot(&repository_path.join(".git")),
        git_without_ref,
        "an installed exact pin must not authorize recreating a missing Git ref"
    );
    assert_eq!(
        std::fs::read(&anchor_path).expect("reread exact retained pin"),
        exact_pin_without_ref,
        "an inconsistent Git tail must not rewrite the exact pin"
    );
    let restored_ref = Command::new("git")
        .current_dir(&repository_path)
        .args(["update-ref", "refs/heads/main", &initial_commit])
        .output()
        .expect("restore exact genesis ref for remaining assertions");
    assert!(restored_ref.status.success());

    let wrong_reason = run(
        temporary.path(),
        None,
        &[
            "init",
            &repository_path_text,
            "--key",
            &fingerprint,
            "--reason",
            "Different reason.",
            "--json",
        ],
    );
    assert_eq!(wrong_reason.status.code(), Some(1));
    assert!(
        json(&wrong_reason)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--reason does not match"))
    );
}

#[cfg(feature = "test-support")]
#[test]
fn hard_exit_before_git_init_resumes_the_retained_profile_and_repository_identity() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent_root = temporary.path().join("agent");
    std::fs::create_dir(&agent_root).unwrap();
    let agent = EphemeralAgent::start(&agent_root, "vela staged bootstrap hard exit");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let name = unique_name("Retained staged identity", &temporary);
    let scope = "Resume one exact crash-retained bootstrap Profile.";
    let reason = "Establish the retained staged repository.";
    let args = [
        "init",
        &repository_text,
        "--name",
        &name,
        "--scope",
        scope,
        "--reason",
        reason,
        "--json",
    ];
    let interrupted = run_with_test_failpoint(
        temporary.path(),
        Some(agent.socket()),
        &args,
        "VELA_TEST_INTERRUPT_INIT_BEFORE_GIT",
    );
    assert_eq!(interrupted.status.code(), Some(86));
    assert!(!repository.join(".git").exists());
    let mut staging = std::fs::read_dir(&repository)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".vela-init-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(staging.len(), 1);
    let profile = vela_protocol::repository::RepositoryProfileV1::from_toml_str(
        &std::fs::read_to_string(staging.remove(0).path().join("vela.toml")).unwrap(),
    )
    .unwrap();
    let retained_repository_id = profile.repository_id;

    let resumed = run(temporary.path(), Some(agent.socket()), &args);
    assert!(
        resumed.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed_json = json(&resumed);
    let _anchor = arm_anchor(&resumed_json);
    assert_eq!(resumed_json["repository_id"], retained_repository_id);
    assert_eq!(resumed_json["resumed"], true);
    let first_commit = resumed_json["repository"]["git_commit"].clone();
    let first_operation = resumed_json["operation_id"].clone();

    let idempotent = run(temporary.path(), None, &args);
    assert!(idempotent.status.success());
    let idempotent = json(&idempotent);
    assert_eq!(idempotent["repository_id"], retained_repository_id);
    assert_eq!(idempotent["repository"]["git_commit"], first_commit);
    assert_eq!(idempotent["operation_id"], first_operation);
}

#[cfg(feature = "test-support")]
#[test]
fn hard_exit_before_genesis_ref_rejects_private_residue_then_converges_exactly() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent_root = temporary.path().join("agent");
    std::fs::create_dir(&agent_root).unwrap();
    let agent = EphemeralAgent::start(&agent_root, "vela pre-ref genesis hard exit");
    let fingerprint = agent_fingerprint(&agent_root);
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let name = unique_name("Pre-ref genesis", &temporary);
    let scope = "Converge from unreferenced exact native-genesis objects.";
    let reason = "Establish one pre-ref crash fixture.";
    let args = [
        "init",
        &repository_text,
        "--name",
        &name,
        "--scope",
        scope,
        "--key",
        &fingerprint,
        "--reason",
        reason,
        "--json",
    ];
    let interrupted = run_with_test_failpoint(
        temporary.path(),
        Some(agent.socket()),
        &args,
        "VELA_TEST_INTERRUPT_INIT_BEFORE_GENESIS_REF",
    );
    assert_eq!(interrupted.status.code(), Some(86));
    let operation = operation_id(&repository);
    assert!(git_text(&repository, &["for-each-ref", "--format=%(refname)"]).is_empty());
    assert!(git_text(&repository, &["ls-files", "--stage"]).is_empty());
    let fsck = git_output(
        &repository,
        &["fsck", "--no-reflogs", "--unreachable", "--no-progress"],
    );
    assert!(fsck.status.success());
    let fsck = format!(
        "{}{}",
        String::from_utf8_lossy(&fsck.stdout),
        String::from_utf8_lossy(&fsck.stderr)
    );
    let candidate = fsck
        .lines()
        .find_map(|line| {
            line.strip_prefix("unreachable commit ")
                .or_else(|| line.strip_prefix("dangling commit "))
        })
        .unwrap_or_else(|| panic!("pre-ref crash retained no unreferenced commit:\n{fsck}"))
        .to_string();
    let candidate_bytes = git_output(&repository, &["cat-file", "commit", &candidate]).stdout;
    let authority_before = directory_snapshot(&repository.join(".vela/authority"));
    let journals_before = directory_snapshot(&repository.join(".vela/operation-journals"));

    let profile = vela_protocol::repository::RepositoryProfileV1::from_toml_str(
        &std::fs::read_to_string(repository.join("vela.toml")).unwrap(),
    )
    .unwrap();
    let conflicting_path = expected_anchor_path(&repository);
    assert!(!conflicting_path.exists());
    let authorities = conflicting_path.parent().unwrap();
    std::fs::create_dir_all(authorities).unwrap();
    #[cfg(unix)]
    for directory in [authorities, authorities.parent().unwrap()] {
        chmod(directory, 0o700);
    }
    let conflicting_pin_before = format!(
        "{{\n  \"schema\": \"vela.authority-trust-anchor.v1\",\n  \"repository_id\": \"{}\",\n  \"first_authority_record_root\": \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n}}\n",
        profile.repository_id
    )
    .into_bytes();
    std::fs::write(&conflicting_path, &conflicting_pin_before).unwrap();
    #[cfg(unix)]
    chmod(&conflicting_path, 0o600);
    let git_before_collision = directory_snapshot(&repository.join(".git"));
    let collision = run(temporary.path(), None, &args);
    assert_eq!(collision.status.code(), Some(1));
    assert!(
        json(&collision)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("local trust pin"))
    );
    assert_eq!(
        directory_snapshot(&repository.join(".git")),
        git_before_collision
    );
    assert_eq!(
        std::fs::read(&conflicting_path).unwrap(),
        conflicting_pin_before
    );
    std::fs::remove_file(&conflicting_path).unwrap();

    let evil = repository.join(".vela/operation-journals/evil");
    std::fs::write(&evil, b"not runtime-owned\n").unwrap();
    let git_before_rejection = directory_snapshot(&repository.join(".git"));
    let rejected = run(temporary.path(), None, &args);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(
        json(&rejected)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("journal-root entry"))
    );
    assert_eq!(
        directory_snapshot(&repository.join(".git")),
        git_before_rejection
    );
    assert!(git_text(&repository, &["for-each-ref", "--format=%(refname)"]).is_empty());
    std::fs::remove_file(&evil).unwrap();

    let resumed = run(temporary.path(), None, &args);
    assert!(
        resumed.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed_json = json(&resumed);
    let _anchor = arm_anchor(&resumed_json);
    assert_eq!(resumed_json["operation_id"], operation);
    assert_eq!(resumed_json["repository"]["git_commit"], candidate);
    assert_eq!(
        git_output(&repository, &["cat-file", "commit", &candidate]).stdout,
        candidate_bytes
    );
    assert_eq!(
        directory_snapshot(&repository.join(".vela/authority")),
        authority_before
    );
    assert_eq!(
        directory_snapshot(&repository.join(".vela/operation-journals")),
        journals_before
    );
    let idempotent = run(temporary.path(), None, &args);
    assert!(idempotent.status.success());
    assert_eq!(idempotent.stdout, resumed.stdout);
}

#[cfg(all(feature = "test-support", unix))]
#[test]
fn completed_genesis_rejects_missing_ambiguous_and_aliased_state_before_git_or_pin() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent_root = temporary.path().join("agent");
    std::fs::create_dir(&agent_root).unwrap();
    let agent = EphemeralAgent::start(&agent_root, "vela pre-object closed-state test");
    let fingerprint = agent_fingerprint(&agent_root);
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let name = unique_name("Closed completed genesis", &temporary);
    let reason = "Reject every non-native byte before Git or trust.";
    let args = [
        "init",
        &repository_text,
        "--name",
        &name,
        "--scope",
        "Close the native genesis public and private path census.",
        "--key",
        &fingerprint,
        "--reason",
        reason,
        "--json",
    ];
    let interrupted = run_with_test_failpoint(
        temporary.path(),
        Some(agent.socket()),
        &args,
        "VELA_TEST_INTERRUPT_INIT_AFTER_COMPLETED",
    );
    assert_eq!(interrupted.status.code(), Some(86));
    let anchor = expected_anchor_path(&repository);
    assert!(!anchor.exists());
    let git_before = directory_snapshot(&repository.join(".git"));
    assert!(git_text(&repository, &["for-each-ref", "--format=%(refname)"]).is_empty());
    assert!(
        git_text(&repository, &["count-objects", "-v"])
            .lines()
            .all(|line| !line.starts_with("count: ") || line == "count: 0")
    );
    let operation = operation_id(&repository);
    let journal = repository
        .join(".vela/operation-journals/repository")
        .join(format!("{operation}.json"));

    let sentinel = temporary.path().join("trust-pin-symlink-sentinel");
    std::fs::write(&sentinel, b"must remain untouched\n").unwrap();
    std::fs::create_dir_all(anchor.parent().expect("trust pin parent")).unwrap();
    std::os::unix::fs::symlink(&sentinel, &anchor).unwrap();
    let _anchor_cleanup = RemoveOnDrop(anchor.clone());
    let nonregular_pin = run(temporary.path(), None, &args);
    assert_eq!(nonregular_pin.status.code(), Some(1));
    let nonregular_message = json(&nonregular_pin)["error"]["message"]
        .as_str()
        .expect("nonregular trust-pin error")
        .to_string();
    assert!(
        nonregular_message.contains("Completed native genesis")
            && nonregular_message.contains("symlink")
            && !nonregular_message.contains("signing could not complete"),
        "{nonregular_message}"
    );
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"must remain untouched\n"
    );
    assert!(
        std::fs::symlink_metadata(&anchor)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    std::fs::remove_file(&anchor).unwrap();

    let detached = temporary.path().join("detached-journal.json");
    std::fs::rename(&journal, &detached).unwrap();
    let missing = run(temporary.path(), None, &args);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        json(&missing)["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Completed native genesis"))
    );
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert!(!anchor.exists());
    std::fs::rename(&detached, &journal).unwrap();

    let duplicate = repository
        .join(".vela/operation-journals/repository")
        .join("vop_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json");
    std::fs::copy(&journal, &duplicate).unwrap();
    let ambiguous = run(temporary.path(), None, &args);
    assert_eq!(ambiguous.status.code(), Some(1));
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert!(!anchor.exists());
    std::fs::remove_file(&duplicate).unwrap();

    let readme = repository.join("README.md");
    let temporary_case = repository.join("README.case-transition");
    let lower = repository.join("readme.md");
    std::fs::rename(&readme, &temporary_case).unwrap();
    std::fs::rename(&temporary_case, &lower).unwrap();
    let case_alias = run(temporary.path(), None, &args);
    assert_eq!(case_alias.status.code(), Some(1));
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert!(!anchor.exists());
    std::fs::rename(&lower, &temporary_case).unwrap();
    std::fs::rename(&temporary_case, &readme).unwrap();

    let unicode_extra = repository.join("e\u{301}.txt");
    std::fs::write(&unicode_extra, b"normalization alias residue\n").unwrap();
    let unicode = run(temporary.path(), None, &args);
    assert_eq!(unicode.status.code(), Some(1));
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert!(!anchor.exists());
    std::fs::remove_file(&unicode_extra).unwrap();

    let readme_bytes = std::fs::read(&readme).unwrap();
    std::fs::write(&readme, b"drifted expected scaffold bytes\n").unwrap();
    let scaffold_drift = run(temporary.path(), None, &args);
    assert_eq!(scaffold_drift.status.code(), Some(1));
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert!(!anchor.exists());

    let journals_before_blocked_advisory =
        directory_snapshot(&repository.join(".vela/operation-journals"));
    let recover_with_blocked_continuation = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation, "--json"],
    );
    assert!(recover_with_blocked_continuation.status.success());
    let blocked_advisory = json(&recover_with_blocked_continuation);
    assert_eq!(blocked_advisory["schema"], "vela.recover-result.v1");
    assert_eq!(blocked_advisory["ok"], true);
    assert_eq!(blocked_advisory["outcome"], "already_completed");
    assert_eq!(blocked_advisory["prior_recovery_state"], "completed");
    assert_eq!(blocked_advisory["repository_blocked_after"], false);
    assert_eq!(blocked_advisory["continuation_status"], "blocked");
    assert_eq!(
        blocked_advisory["continuation_code"],
        "native_genesis_continuation_unverified"
    );
    assert_eq!(
        blocked_advisory["continuation_diagnostic"],
        "filesystem recovery succeeded, but the exact native-genesis init continuation could not be verified; preserve the Completed journal and repository bytes before repair"
    );
    assert!(blocked_advisory.get("next_command").is_none());
    assert_eq!(directory_snapshot(&repository.join(".git")), git_before);
    assert_eq!(
        directory_snapshot(&repository.join(".vela/operation-journals")),
        journals_before_blocked_advisory,
        "blocked continuation advice must not mutate the Completed journal inventory"
    );
    assert!(!anchor.exists());
    let blocked_retry = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation, "--json"],
    );
    assert!(blocked_retry.status.success());
    assert_eq!(
        blocked_retry.stdout, recover_with_blocked_continuation.stdout,
        "a terminal blocked advisory must be byte-stable on retry"
    );
    std::fs::write(&readme, readme_bytes).unwrap();

    let resumed = run(temporary.path(), None, &args);
    assert!(
        resumed.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed = json(&resumed);
    let _anchor = arm_anchor(&resumed);
}

#[cfg(feature = "test-support")]
#[test]
fn post_ref_and_post_pin_hard_exits_return_the_same_init_result_without_a_signer() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let mut anchors = Vec::new();
    for (label, failpoint) in [
        ("post-ref", "VELA_TEST_INTERRUPT_INIT_AFTER_GIT"),
        ("post-pin", "VELA_TEST_INTERRUPT_INIT_AFTER_TRUST"),
    ] {
        let agent_root = temporary.path().join(format!("{label}-agent"));
        std::fs::create_dir(&agent_root).unwrap();
        let agent = EphemeralAgent::start(&agent_root, &format!("vela {label} hard exit"));
        let fingerprint = agent_fingerprint(&agent_root);
        let repository = temporary.path().join(format!("{label}-repository"));
        let repository_text = repository.to_string_lossy().into_owned();
        let name = unique_name(&format!("{label} native genesis"), &temporary);
        let reason = format!("Establish the {label} hard-exit fixture.");
        let args = [
            "init",
            &repository_text,
            "--name",
            &name,
            "--scope",
            "Resume the exact deterministic Git and trust tail.",
            "--key",
            &fingerprint,
            "--reason",
            &reason,
            "--json",
        ];
        let interrupted =
            run_with_test_failpoint(temporary.path(), Some(agent.socket()), &args, failpoint);
        assert_eq!(interrupted.status.code(), Some(86), "{label}");
        let commit = git_text(&repository, &["rev-parse", "HEAD^{commit}"]);
        let operation = operation_id(&repository);
        let resumed = run(temporary.path(), None, &args);
        assert!(
            resumed.status.success(),
            "{label}: stdout={} stderr={}",
            String::from_utf8_lossy(&resumed.stdout),
            String::from_utf8_lossy(&resumed.stderr)
        );
        let resumed_json = json(&resumed);
        anchors.push(arm_anchor(&resumed_json));
        assert_eq!(resumed_json["operation_id"], operation);
        assert_eq!(resumed_json["repository"]["git_commit"], commit);
        let idempotent = run(temporary.path(), None, &args);
        assert!(idempotent.status.success());
        assert_eq!(idempotent.stdout, resumed.stdout, "{label}");
    }
}

#[cfg(feature = "test-support")]
#[test]
fn installed_native_genesis_requires_explicit_recovery_then_policy_free_init_tail() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent_root = temporary.path().join("agent");
    std::fs::create_dir(&agent_root).unwrap();
    let agent = EphemeralAgent::start(&agent_root, "vela installed recovery hard exit");
    let fingerprint = agent_fingerprint(&agent_root);
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let name = unique_name("Installed recovery genesis", &temporary);
    let reason = "Recover Installed genesis before its Git and trust tail.";
    let args = [
        "init",
        &repository_text,
        "--name",
        &name,
        "--scope",
        "Prove recovery stops before post-transaction publication.",
        "--key",
        &fingerprint,
        "--reason",
        reason,
        "--json",
    ];
    let interrupted = run_with_test_failpoint(
        temporary.path(),
        Some(agent.socket()),
        &args,
        "VELA_TEST_INTERRUPT_INIT_AFTER_INSTALLED",
    );
    assert_eq!(interrupted.status.code(), Some(86));
    let operation = operation_id(&repository);
    assert!(git_text(&repository, &["for-each-ref", "--format=%(refname)"]).is_empty());

    let blocked = run(temporary.path(), None, &args);
    assert_eq!(blocked.status.code(), Some(1));
    let blocked = json(&blocked);
    assert_eq!(blocked["error"]["code"], "repository_incomplete");
    assert!(
        blocked["error"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains(&operation) && hint.contains("vela recover"))
    );

    // Recovery has a complete filesystem transaction to finalize, but the
    // native-genesis continuation is only advisory. A scaffold defect must
    // not turn that successful recovery into a command failure.
    let readme = repository.join("README.md");
    let readme_bytes = std::fs::read(&readme).unwrap();
    std::fs::write(&readme, b"drifted after Installed recovery state\n").unwrap();
    let git_before_recovery = directory_snapshot(&repository.join(".git"));
    let authority_before_recovery = directory_snapshot(&repository.join(".vela/authority"));
    let anchor = expected_anchor_path(&repository);
    assert!(!anchor.exists());

    let recovered = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation, "--json"],
    );
    assert!(
        recovered.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&recovered.stdout),
        String::from_utf8_lossy(&recovered.stderr)
    );
    let recovered = json(&recovered);
    assert_eq!(recovered["outcome"], "completed");
    assert_eq!(recovered["prior_recovery_state"], "installed");
    assert_eq!(recovered["repository_blocked_after"], false);
    assert_eq!(recovered["continuation_status"], "blocked");
    assert_eq!(
        recovered["continuation_code"],
        "native_genesis_continuation_unverified"
    );
    assert!(recovered.get("next_command").is_none());
    assert_eq!(
        directory_snapshot(&repository.join(".git")),
        git_before_recovery
    );
    assert_eq!(
        directory_snapshot(&repository.join(".vela/authority")),
        authority_before_recovery
    );
    assert!(!anchor.exists());
    let journals_after_recovery = directory_snapshot(&repository.join(".vela/operation-journals"));

    let repeated = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation, "--json"],
    );
    assert!(repeated.status.success());
    let repeated = json(&repeated);
    assert_eq!(repeated["outcome"], "already_completed");
    assert_eq!(repeated["prior_recovery_state"], "completed");
    assert_eq!(repeated["repository_blocked_after"], false);
    assert_eq!(repeated["continuation_status"], "blocked");
    assert!(repeated.get("next_command").is_none());
    assert_eq!(
        directory_snapshot(&repository.join(".vela/operation-journals")),
        journals_after_recovery
    );
    assert_eq!(
        directory_snapshot(&repository.join(".git")),
        git_before_recovery
    );
    assert!(!anchor.exists());

    let blocked_human = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation],
    );
    assert!(blocked_human.status.success());
    let blocked_human = String::from_utf8(blocked_human.stdout).unwrap();
    assert!(blocked_human.contains("continuation"));
    assert!(blocked_human.contains("blocked"));
    assert!(blocked_human.contains("native_genesis_continuation_unverified"));
    assert!(blocked_human.contains("filesystem recovery succeeded"));
    assert_eq!(
        directory_snapshot(&repository.join(".git")),
        git_before_recovery
    );
    assert_eq!(
        directory_snapshot(&repository.join(".vela/operation-journals")),
        journals_after_recovery
    );
    assert!(!anchor.exists());

    std::fs::write(&readme, readme_bytes).unwrap();
    let available = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation, "--json"],
    );
    assert!(available.status.success());
    let available = json(&available);
    assert_eq!(available["outcome"], "already_completed");
    assert_eq!(available["continuation_status"], "exact_init_available");
    assert!(available.get("continuation_code").is_none());
    assert!(available.get("continuation_diagnostic").is_none());
    let next = available["next_command"]
        .as_str()
        .expect("exact init continuation");
    assert!(next.contains("vela init"));
    assert!(next.contains(&fingerprint));
    assert!(next.contains(reason));
    assert!(git_text(&repository, &["for-each-ref", "--format=%(refname)"]).is_empty());
    let authority_after_recovery = directory_snapshot(&repository.join(".vela/authority"));

    let recovered_human = run(
        temporary.path(),
        None,
        &["recover", "--repo", &repository_text, &operation],
    );
    assert!(recovered_human.status.success());
    assert!(
        String::from_utf8_lossy(&recovered_human.stdout).contains(next),
        "human recovery must render the same exact continuation: {}",
        String::from_utf8_lossy(&recovered_human.stdout)
    );

    let resumed = run(temporary.path(), None, &args);
    assert!(resumed.status.success());
    let resumed_json = json(&resumed);
    let _anchor = arm_anchor(&resumed_json);
    assert_eq!(resumed_json["operation_id"], operation);
    assert_eq!(
        directory_snapshot(&repository.join(".vela/authority")),
        authority_after_recovery
    );
    assert_eq!(
        directory_snapshot(&repository.join(".vela/operation-journals")),
        journals_after_recovery
    );
    let idempotent = run(temporary.path(), None, &args);
    assert!(idempotent.status.success());
    assert_eq!(idempotent.stdout, resumed.stdout);
}

#[cfg(all(feature = "test-support", unix))]
#[test]
fn real_init_ignores_hostile_ambient_git_and_leaves_every_sentinel_untouched() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent_root = temporary.path().join("agent");
    std::fs::create_dir(&agent_root).unwrap();
    let agent = EphemeralAgent::start(&agent_root, "vela hostile product init");
    let sentinel = temporary.path().join("sentinel");
    std::fs::create_dir(&sentinel).unwrap();
    assert!(
        git_output(&sentinel, &["init", "-b", "main"])
            .status
            .success()
    );
    assert!(
        git_output(&sentinel, &["config", "user.name", "Sentinel"])
            .status
            .success()
    );
    assert!(
        git_output(
            &sentinel,
            &["config", "user.email", "sentinel@example.invalid"]
        )
        .status
        .success()
    );
    std::fs::write(sentinel.join("sentinel.txt"), b"untouched\n").unwrap();
    assert!(
        git_output(&sentinel, &["add", "sentinel.txt"])
            .status
            .success()
    );
    assert!(
        git_output(&sentinel, &["commit", "-m", "sentinel"])
            .status
            .success()
    );

    let hostile = temporary.path().join("hostile");
    std::fs::create_dir(&hostile).unwrap();
    let sentinel_index = hostile.join("sentinel-index");
    assert!(
        Command::new("git")
            .current_dir(&sentinel)
            .args(["read-tree", "HEAD"])
            .env("GIT_INDEX_FILE", &sentinel_index)
            .output()
            .unwrap()
            .status
            .success()
    );
    let side_effect = hostile.join("side-effect");
    let helper = hostile.join("helper");
    write_executable(
        &helper,
        &format!("#!/bin/sh\n: > '{}'\ncat\n", side_effect.display()),
    );
    let hooks = hostile.join("hooks");
    let templates = hostile.join("templates");
    std::fs::create_dir(&hooks).unwrap();
    std::fs::create_dir_all(templates.join("hooks")).unwrap();
    write_executable(&hooks.join("reference-transaction"), "#!/bin/sh\nexit 97\n");
    write_executable(
        &templates.join("hooks/post-checkout"),
        "#!/bin/sh\nexit 98\n",
    );
    std::fs::write(templates.join("hostile-template-byte"), b"must not copy\n").unwrap();
    let global = hostile.join("global.gitconfig");
    let system = hostile.join("system.gitconfig");
    let config = format!(
        "[core]\n\tworktree = {}\n\thooksPath = {}\n[filter \"hostile\"]\n\tclean = {}\n[init]\n\ttemplateDir = {}\n",
        sentinel.display(),
        hooks.display(),
        helper.display(),
        templates.display()
    );
    std::fs::write(&global, &config).unwrap();
    std::fs::write(&system, &config).unwrap();
    let hostile_home = hostile.join("home");
    std::fs::create_dir(&hostile_home).unwrap();
    std::fs::write(hostile_home.join(".gitconfig"), &config).unwrap();
    let sentinel_before = directory_snapshot(&sentinel);
    let sentinel_index_before = std::fs::read(&sentinel_index).unwrap();

    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let name = unique_name("Hostile Git product init", &temporary);
    let args = [
        "init",
        &repository_text,
        "--name",
        &name,
        "--scope",
        "Bind every Git byte to the intended native repository.",
        "--json",
    ];
    let injected = [
        ("core.worktree", sentinel.as_os_str()),
        ("core.hooksPath", hooks.as_os_str()),
        ("filter.hostile.clean", helper.as_os_str()),
        ("init.templateDir", templates.as_os_str()),
        ("extensions.objectFormat", std::ffi::OsStr::new("sha256")),
    ];
    let mut command = vela_command(temporary.path(), Some(agent.socket()), &args);
    command
        .env("HOME", &hostile_home)
        .env("GIT_DIR", sentinel.join(".git"))
        .env("GIT_WORK_TREE", &sentinel)
        .env("GIT_COMMON_DIR", sentinel.join(".git"))
        .env("GIT_INDEX_FILE", &sentinel_index)
        .env("GIT_OBJECT_DIRECTORY", sentinel.join(".git/objects"))
        .env(
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            repository.join("objects"),
        )
        .env("GIT_NAMESPACE", "hostile")
        .env("GIT_DEFAULT_HASH", "sha256")
        .env("GIT_TEMPLATE_DIR", &templates)
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_CONFIG_SYSTEM", &system)
        .env("GIT_CONFIG_NOSYSTEM", "0")
        .env("GIT_ATTR_NOSYSTEM", "0")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_PROTOCOL_FROM_USER", "1")
        .env("GIT_TERMINAL_PROMPT", "1")
        .env("GIT_ASKPASS", &helper)
        .env("GIT_PAGER", &helper)
        .env("GIT_EDITOR", &helper)
        .env("GIT_TRACE", &side_effect)
        .env("GIT_CONFIG_COUNT", injected.len().to_string());
    for (index, (key, value)) in injected.into_iter().enumerate() {
        command
            .env(format!("GIT_CONFIG_KEY_{index}"), key)
            .env(format!("GIT_CONFIG_VALUE_{index}"), value);
    }
    let initialized = command.output().expect("run hostile product init");
    assert!(
        initialized.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&initialized.stdout),
        String::from_utf8_lossy(&initialized.stderr)
    );
    let initialized = json(&initialized);
    let _anchor = arm_anchor(&initialized);
    assert!(repository.join(".git").is_dir());
    assert!(!repository.join(".git/hostile-template-byte").exists());
    assert_eq!(directory_snapshot(&sentinel), sentinel_before);
    assert_eq!(
        std::fs::read(&sentinel_index).unwrap(),
        sentinel_index_before
    );
    assert!(
        !side_effect.exists(),
        "hostile Git helper or tracing executed"
    );
}

#[test]
fn review_decision_preflight_keeps_json_error_contract() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    assert!(
        run(
            temporary.path(),
            None,
            &[
                "init",
                &repository_path_text,
                "--name",
                "Decision UX",
                "--scope",
                "Keep review errors machine-readable.",
                "--json",
            ],
        )
        .status
        .code()
            == Some(1)
    );

    let blocked = run(
        temporary.path(),
        None,
        &[
            "review",
            "accept",
            &repository_path_text,
            "vpr_missing",
            "--reason",
            "Inspect the JSON contract.",
            "--json",
        ],
    );
    assert_eq!(blocked.status.code(), Some(1));
    let blocked = json(&blocked);
    assert_eq!(blocked["command"], "review.accept");
    assert_eq!(
        blocked["error"]["message"],
        "repository authority is not initialized"
    );
}

#[test]
fn a_colliding_trust_pin_is_not_reported_as_a_signing_failure() {
    // The pin lives under the OS account home and keys on repository_id, so a
    // second authority initialization of the same retained Profile, with a
    // different key, targets the same pin path with a different record root.
    // That is not a signing failure and no key operation can clear it. Two
    // separately created repositories can no longer provoke this, because each
    // draws its own genesis identity; a copied bootstrap still can.
    let temporary = tempfile::tempdir().expect("temporary directory");
    let name = unique_name("Pin collision", &temporary);
    let scope = "Prove a colliding pin is classified apart from signing.";

    let first_root = temporary.path().join("first");
    std::fs::create_dir_all(&first_root).expect("first agent root");
    let first_agent = EphemeralAgent::start(&first_root, "vela pin collision first");
    let first_repository = temporary.path().join("first/repository");
    let first_repository_text = first_repository.to_string_lossy().into_owned();
    let established = run(
        temporary.path(),
        Some(first_agent.socket()),
        &[
            "init",
            &first_repository_text,
            "--name",
            &name,
            "--scope",
            scope,
            "--json",
        ],
    );
    assert!(established.status.success());
    let established = json(&established);
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        established["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    // The second init has no usable agent. Detecting the pre-existing pin must
    // happen before identity selection or signing, not merely be relabeled
    // after a cryptographic failure.
    let second_repository = temporary.path().join("second/repository");
    let second_repository_text = second_repository.to_string_lossy().into_owned();
    // Copy the first repository's retained bootstrap, which carries its exact
    // repository_id, and resume `vela init` there against the second key.
    std::fs::create_dir_all(second_repository.join(".vela")).expect("second bootstrap .vela");
    for retained in [
        "vela.toml",
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        ".gitignore",
        ".gitattributes",
    ] {
        std::fs::copy(
            first_repository.join(retained),
            second_repository.join(retained),
        )
        .unwrap_or_else(|error| panic!("copy retained bootstrap {retained}: {error}"));
    }
    let git = Command::new("git")
        .args(["init", "--quiet", "-b", "main"])
        .arg(&second_repository)
        .output()
        .expect("git init the copied bootstrap");
    assert!(
        git.status.success(),
        "{}",
        String::from_utf8_lossy(&git.stderr)
    );
    let pin_path = std::path::PathBuf::from(
        established["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("first trust pin path"),
    );
    let pin_before = std::fs::read(&pin_path).expect("read established trust pin");
    let git_and_bootstrap_before = directory_snapshot(&second_repository);
    let collided = run(
        temporary.path(),
        None,
        &["init", &second_repository_text, "--json"],
    );
    assert_eq!(collided.status.code(), Some(1));
    let collided = json(&collided);
    assert_eq!(collided["command"], "init");
    assert_eq!(collided["error"]["kind"], "domain");
    let message = collided["error"]["message"]
        .as_str()
        .expect("collision message");
    assert!(!message.contains("signing could not complete"), "{message}");
    assert!(message.contains("local trust pin"), "{message}");
    let hint = collided["error"]["hint"].as_str().expect("collision hint");
    assert!(!hint.contains("ssh-add"), "{hint}");
    assert!(hint.contains("--previous-record-root"), "{hint}");
    assert!(hint.contains("retained Profile UUID"), "{hint}");
    assert!(
        hint.contains("changing --name or --scope never changes"),
        "{hint}"
    );
    assert_eq!(
        directory_snapshot(&second_repository),
        git_and_bootstrap_before,
        "a blocking trust pin must leave Git and bootstrap bytes exact"
    );
    assert_eq!(
        std::fs::read(&pin_path).expect("reread blocking trust pin"),
        pin_before,
        "a blocking trust pin must not rewrite itself"
    );
    assert!(!second_repository.join(".vela/origin.json").exists());
    assert!(!second_repository.join(".vela/repository.json").exists());
    assert!(!second_repository.join(".vela/authority").exists());
    assert!(!second_repository.join(".vela/operation-journals").exists());
}

#[test]
fn two_repositories_on_the_same_question_receive_different_identities() {
    // A Vela repository is one independently clonable Git repository, so identity
    // must distinguish repositories rather than questions. Two groups may open
    // repositories on the same bounded question with the same wording; neither
    // may take the other's identity or its user-local trust anchor.
    let temporary = tempfile::tempdir().expect("temporary directory");
    let name = unique_name("Same question", &temporary);
    let scope = "Prove independent repositories keep independent identities.";

    let mut identities = Vec::new();
    let mut anchors = Vec::new();
    for label in ["first", "second"] {
        let agent_root = temporary.path().join(label);
        std::fs::create_dir_all(&agent_root).expect("agent root");
        let agent = EphemeralAgent::start(&agent_root, &format!("vela same question {label}"));
        let repository_path = agent_root.join("repository_path");
        let repository_path_text = repository_path.to_string_lossy().into_owned();
        let created = run(
            temporary.path(),
            Some(agent.socket()),
            &[
                "init",
                &repository_path_text,
                "--name",
                &name,
                "--scope",
                scope,
                "--json",
            ],
        );
        assert!(
            created.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&created.stderr)
        );
        let created = json(&created);
        anchors.push(RemoveOnDrop(std::path::PathBuf::from(
            created["authority"]["local_trust"]["anchor_path"]
                .as_str()
                .expect("local trust anchor path"),
        )));
        identities.push(
            created["repository_id"]
                .as_str()
                .expect("repository_id")
                .to_string(),
        );
    }

    assert_ne!(identities[0], identities[1]);
    assert_ne!(anchors[0].0, anchors[1].0);
}

/// Content-addressed paths must never be end-of-line normalized.
///
/// A record's root is sha256 over the exact bytes Git holds. `text` or `eol=`
/// on such a path rewrites them on checkout, and replay then reads a file whose
/// digest is not the one the manifest binds — so the scaffold shipping
/// the record family with `text eol=lf` meant every repository had to
/// hand-correct its own `.gitattributes` before it could verify. All four did,
/// independently.
/// `REPOSITORY_PROFILE.md` states the rule; nothing held the scaffold
/// to it.
#[test]
fn the_scaffold_never_normalizes_a_content_addressed_path() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let agent = EphemeralAgent::start(temporary.path(), "vela gitattributes scaffold test");
    let repository_path = temporary.path().join("repository_path");
    let repository_path_text = repository_path.to_string_lossy().into_owned();
    let initialized = run(
        temporary.path(),
        Some(agent.socket()),
        &[
            "init",
            &repository_path_text,
            "--name",
            &unique_name("Byte stability", &temporary),
            "--scope",
            "Hold content-addressed paths out of every checkout filter.",
            "--json",
        ],
    );
    assert!(
        initialized.status.success(),
        "the fixture repository must initialize"
    );
    let _anchor = RemoveOnDrop(std::path::PathBuf::from(
        json(&initialized)["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .expect("local trust anchor path"),
    ));

    let attributes = std::fs::read_to_string(repository_path.join(".gitattributes"))
        .expect("scaffolded .gitattributes");
    for family in [".vela/**", "records/**", "artifacts/**"] {
        let line = attributes
            .lines()
            .find(|line| line.starts_with(family))
            .unwrap_or_else(|| panic!("the scaffold must declare {family}"));
        assert!(
            line.contains("-text"),
            "{family} must be -text so its bytes survive checkout: {line}"
        );
        assert!(
            !line.contains("eol="),
            "{family} must not be end-of-line normalized: {line}"
        );
    }

    /* The other half of the same failure. The profile line said
    `frontier.toml` while `vela init` wrote `vela.toml`, so the one file here a
    human is expected to edit was declared under a name that never existed and
    the rule matched nothing. */
    let profile = attributes
        .lines()
        .find(|line| line.starts_with("vela.toml "))
        .expect("the scaffold must declare the profile it writes");
    assert!(
        profile.contains("text eol=lf"),
        "the profile is human-edited configuration and keeps eol normalization: {profile}"
    );
    assert!(
        !attributes.contains("frontier.toml"),
        "the scaffold declares a profile filename `vela init` does not write:\n{attributes}"
    );
}
