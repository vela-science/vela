//! Product-boundary recovery regressions.
//!
//! The transaction crate creates exact crash states; every recovery action is
//! then exercised through the real `vela` binary. Test authorization is local,
//! in-memory, exact-plan-bound, and carries no Vela policy or credential.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use vela_protocol::repository::{
    REPOSITORY_PROFILE_SCHEMA_V1, RepositoryProfileLicenseV1, RepositoryProfileScopeV1,
    RepositoryProfileV1,
};
use vela_repository::{
    ContentDigest, DeltaDraft, OperationId, OperationKind, PlannedWrite, RepoPath,
    RepositoryBinding, RepositoryTxn, RepositoryTxnError, RepositoryTxnPlan, RepositoryTxnPlanSpec,
    RepositoryTxnStep, TransactionAuthorization, TransactionAuthorizationContext, WriteClass,
};

mod support;
use support::{EphemeralAgent, RemoveAnchorOnDrop};

#[derive(Debug, Default)]
struct ExactTestAuthorization {
    plan_root: Option<ContentDigest>,
}

impl TransactionAuthorization for ExactTestAuthorization {
    fn bind_plan(
        &mut self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        self.plan_root = Some(context.plan_root().clone());
        Ok(())
    }

    fn revalidate_for_marker(
        &self,
        context: &mut TransactionAuthorizationContext<'_>,
    ) -> Result<(), RepositoryTxnError> {
        let expected = self.plan_root.as_ref().ok_or_else(|| {
            RepositoryTxnError::WriteAuthorization("unbound recovery test capability".into())
        })?;
        if expected != context.plan_root() {
            return Err(RepositoryTxnError::StaleWriteAuthorization {
                expected: expected.clone(),
                actual: context.plan_root().clone(),
            });
        }
        Ok(())
    }
}

struct RepoFixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    repository_id: String,
}

impl RepoFixture {
    fn new(label: &str) -> Self {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("repository");
        let home = temporary.path().join("home");
        fs::create_dir_all(root.join(".vela")).expect("private repository directory");
        fs::create_dir_all(&home).expect("isolated home");
        git(None, &["init", "-q", root.to_str().expect("UTF-8 root")]);
        git(Some(&root), &["config", "user.name", "Vela Recovery Test"]);
        git(
            Some(&root),
            &["config", "user.email", "recovery@example.invalid"],
        );
        fs::write(root.join(".gitignore"), ".vela/operation-journals/\n")
            .expect("private journal ignore");
        fs::write(root.join("base.txt"), b"before\n").expect("base preimage");
        let repository_id = "01234567-89ab-4def-8123-456789abcdef".to_string();
        let profile = RepositoryProfileV1 {
            schema: REPOSITORY_PROFILE_SCHEMA_V1.into(),
            repository_id: repository_id.clone(),
            name: format!("Recovery fixture {label}"),
            summary: "Product-boundary repository recovery fixture.".into(),
            scope: RepositoryProfileScopeV1 {
                question: "Does exact transaction recovery fail closed?".into(),
                includes: Vec::new(),
                excludes: Vec::new(),
            },
            maintainers: Vec::new(),
            license: RepositoryProfileLicenseV1 {
                content: "CC-BY-4.0".into(),
                code: "Apache-2.0".into(),
                data: "NOASSERTION".into(),
            },
        };
        fs::write(
            root.join("vela.toml"),
            toml::to_string_pretty(&profile).expect("serialize fixture profile"),
        )
        .expect("write fixture profile");
        git(Some(&root), &["add", ".gitignore", "base.txt", "vela.toml"]);
        git(Some(&root), &["commit", "-qm", "fixture base"]);
        let root = root.canonicalize().expect("canonical fixture root");
        Self {
            _temporary: temporary,
            root,
            home,
            repository_id,
        }
    }

    fn journal_dir(&self) -> PathBuf {
        self.root.join(".vela/operation-journals")
    }

    fn prepare(&self, label: &str, writes: Vec<PlannedWrite>) -> (OperationId, RepositoryTxn) {
        prepare_transaction(
            &self.root,
            &self.journal_dir(),
            &self.repository_id,
            label,
            writes,
        )
    }

    fn recover(&self, operation_id: &OperationId) -> Output {
        run_recover(&self.root, &self.home, operation_id, true)
    }
}

fn prepare_transaction(
    root: &Path,
    journal_dir: &Path,
    repository_id: &str,
    label: &str,
    writes: Vec<PlannedWrite>,
) -> (OperationId, RepositoryTxn) {
    let draft = DeltaDraft::prepare(root, writes).expect("prepare exact recovery delta");
    let operation_id = OperationId::derive("recovery_fixture", label.as_bytes());
    let plan = RepositoryTxnPlan::new(
        RepositoryTxnPlanSpec {
            kind: OperationKind::new("recovery_fixture").expect("fixture operation kind"),
            operation_id: operation_id.clone(),
            request_root: ContentDigest::hash(label.as_bytes()),
            repository: RepositoryBinding::new(root, repository_id)
                .expect("fixture repository binding"),
            fixed_time: "2026-08-10T12:00:00Z".into(),
            read_set: Vec::new(),
            result: json!({"fixture": label}),
        },
        draft.delta.clone(),
    )
    .expect("fixture transaction plan");
    let barrier = RepositoryTxn::acquire_recovery_barrier(root, journal_dir)
        .expect("fixture recovery barrier")
        .authorize(Box::<ExactTestAuthorization>::default());
    let transaction =
        RepositoryTxn::prepare_with_barrier(barrier, plan, draft).expect("prepare fixture journal");
    (operation_id, transaction)
}

fn run_recover(root: &Path, home: &Path, operation_id: &OperationId, json: bool) -> Output {
    let mut arguments = vec![
        "recover".to_string(),
        "--repo".to_string(),
        root.to_string_lossy().into_owned(),
        operation_id.as_str().to_string(),
    ];
    if json {
        arguments.push("--json".into());
    }
    run(
        root,
        home,
        &arguments.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn run(cwd: &Path, home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(arguments)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "1")
        .env("SSH_AUTH_SOCK", cwd.join("missing-recovery-agent.sock"))
        .env_remove("VELA_AGENT_KEY_HEX")
        .output()
        .expect("run vela")
}

#[cfg(feature = "test-support")]
fn run_with_test_env(
    cwd: &Path,
    home: &Path,
    arguments: &[&str],
    name: &str,
    value: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(cwd)
        .args(arguments)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "1")
        .env("SSH_AUTH_SOCK", cwd.join("missing-recovery-agent.sock"))
        .env_remove("VELA_AGENT_KEY_HEX")
        .env(name, value)
        .output()
        .expect("run failpoint-enabled vela")
}

fn json_output(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "decode Vela JSON: {error}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn successful_recovery(output: &Output, operation_id: &OperationId, outcome: &str) -> Value {
    assert!(
        output.status.success(),
        "recovery failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_output(output);
    assert_eq!(payload["schema"], "vela.recover-result.v1");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["command"], "recover");
    assert_eq!(payload["operation_id"], operation_id.as_str());
    assert_eq!(payload["outcome"], outcome);
    let mut keys = payload
        .as_object()
        .expect("recover result object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    let mut expected = vec![
        "command",
        "ok",
        "operation_id",
        "outcome",
        "prior_recovery_state",
        "repository_blocked_after",
        "repository_id",
        "repository_path",
        "schema",
    ];
    if matches!(outcome, "completed" | "already_completed")
        || payload["repository_blocked_after"] == true
    {
        expected.push("next_command");
        expected.sort_unstable();
    }
    assert_eq!(keys, expected, "recover-result.v1 key set drifted");
    payload
}

fn failed_recovery(output: &Output) -> Value {
    assert!(
        !output.status.success(),
        "recovery unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let payload = json_output(output);
    assert_eq!(payload["schema"], "vela.error.v1");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["command"], "recover");
    payload
}

fn git(cwd: Option<&Path>, arguments: &[&str]) -> Vec<u8> {
    let mut command = Command::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .args(arguments)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git {}: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git_publication_snapshot(root: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        git(Some(root), &["rev-parse", "HEAD"]),
        git(
            Some(root),
            &["for-each-ref", "--format=%(refname)%00%(objectname)"],
        ),
        fs::read(root.join(".git/index")).expect("Git index bytes"),
    )
}

fn plan_path(journal_dir: &Path, operation_id: &OperationId) -> PathBuf {
    journal_dir
        .join("repository")
        .join(format!("{}.json", operation_id.as_str()))
}

fn marker_path(journal_dir: &Path, operation_id: &OperationId) -> PathBuf {
    journal_dir
        .join("repository/committed")
        .join(format!("{}.json", operation_id.as_str()))
}

fn blob_paths(journal_dir: &Path) -> Vec<PathBuf> {
    let directory = journal_dir.join("repository/blobs");
    let mut paths = fs::read_dir(directory)
        .expect("read recovery blobs")
        .map(|entry| entry.expect("recovery blob entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn tree_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, path: &Path, snapshot: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = match fs::read_dir(path) {
            Ok(entries) => entries
                .collect::<Result<Vec<_>, _>>()
                .expect("snapshot directory entries"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("snapshot {}: {error}", path.display()),
        };
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .expect("snapshot path under base")
                .to_string_lossy()
                .into_owned();
            let metadata = fs::symlink_metadata(&path).expect("snapshot metadata");
            if metadata.file_type().is_symlink() {
                snapshot.insert(
                    relative,
                    fs::read_link(&path)
                        .expect("snapshot symlink")
                        .to_string_lossy()
                        .as_bytes()
                        .to_vec(),
                );
            } else if metadata.is_dir() {
                visit(base, &path, snapshot);
            } else {
                snapshot.insert(relative, fs::read(&path).expect("snapshot file"));
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

fn recovery_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    tree_snapshot(root)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("repository-locks/"))
        .collect()
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied root");
    let mut entries = fs::read_dir(source)
        .expect("read copied tree")
        .collect::<Result<Vec<_>, _>>()
        .expect("copied tree entries");
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).expect("copied entry metadata");
        if metadata.file_type().is_symlink() {
            symlink(
                fs::read_link(&source_path).expect("copied symlink target"),
                &destination_path,
            )
            .expect("copy symlink");
        } else if metadata.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).expect("copy file");
        }
    }
}

fn write_journal_state(path: &Path, state: &str) {
    let mut value: Value = serde_json::from_slice(&fs::read(path).expect("read journal state"))
        .expect("parse journal state");
    value["recovery"] = json!({"state": state});
    let mut bytes = serde_json::to_vec_pretty(&value).expect("serialize journal state");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("replace journal state");
}

#[cfg(feature = "test-support")]
fn operation_in_state(journal_dir: &Path, state: &str) -> OperationId {
    let mut matches = fs::read_dir(journal_dir.join("repository"))
        .expect("read repository journals")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| {
            let value: Value = serde_json::from_slice(&fs::read(entry.path()).ok()?).ok()?;
            (value["recovery"]["state"] == state).then(|| {
                OperationId::parse(
                    entry
                        .path()
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .expect("UTF-8 operation journal name")
                        .to_string(),
                )
                .expect("valid operation journal name")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one transaction in {state}, found {matches:?}"
    );
    matches.remove(0)
}

fn standard_writes() -> Vec<PlannedWrite> {
    vec![
        PlannedWrite::write(
            RepoPath::parse("records/recovered.json").expect("record path"),
            WriteClass::CanonicalEvidence,
            b"{\"recovered\":true}\n".to_vec(),
        ),
        PlannedWrite::write(
            RepoPath::parse("base.txt").expect("base path"),
            WriteClass::Authority,
            b"after\n".to_vec(),
        ),
    ]
}

fn committed_fixture(label: &str) -> (RepoFixture, OperationId) {
    let fixture = RepoFixture::new(label);
    let (operation_id, mut transaction) = fixture.prepare(label, standard_writes());
    transaction.mark_committed().expect("durable commit marker");
    drop(transaction);
    (fixture, operation_id)
}

fn before_write(index: usize) -> RepositoryTxnStep {
    RepositoryTxnStep::BeforeInstallWrite { index }
}

fn after_write(index: usize) -> RepositoryTxnStep {
    RepositoryTxnStep::AfterInstallWrite { index }
}

fn before_progress(index: usize) -> RepositoryTxnStep {
    RepositoryTxnStep::BeforeInstallingJournalWrite { index }
}

fn after_progress(index: usize) -> RepositoryTxnStep {
    RepositoryTxnStep::AfterInstallingJournalWrite { index }
}

#[test]
fn prepared_recovery_is_explicit_read_only_safe_and_unblocks_a_real_write() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let home = temporary.path().join("home");
    fs::create_dir_all(&home).expect("isolated home");
    let daily_help = run(temporary.path(), &home, &["--help"]);
    assert!(daily_help.status.success());
    assert!(!String::from_utf8_lossy(&daily_help.stdout).contains("  recover"));
    let advanced_help = run(temporary.path(), &home, &["help", "advanced"]);
    assert!(advanced_help.status.success());
    assert!(String::from_utf8_lossy(&advanced_help.stdout).contains("  recover"));
    let agent = EphemeralAgent::start(temporary.path(), "vela recovery product test");
    let repository = temporary.path().join("repository");
    let repository_text = repository.to_string_lossy().into_owned();
    let initialized = Command::new(env!("CARGO_BIN_EXE_vela"))
        .current_dir(temporary.path())
        .args([
            "init",
            repository_text.as_str(),
            "--name",
            &format!(
                "Recovery product fixture {}",
                temporary
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unique")
            ),
            "--scope",
            "Exercise explicit repository transaction recovery.",
            "--json",
        ])
        .env("HOME", &home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "1")
        .env("SSH_AUTH_SOCK", agent.socket())
        .output()
        .expect("initialize recovery fixture");
    assert!(
        initialized.status.success(),
        "init: stdout={} stderr={}",
        String::from_utf8_lossy(&initialized.stdout),
        String::from_utf8_lossy(&initialized.stderr)
    );
    let initialized_json = json_output(&initialized);
    let _anchor = RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized.stdout))
        .expect("init trust anchor guard");
    let repository_id = initialized_json["repository_id"]
        .as_str()
        .expect("repository id");
    let canonical_repository_text = repository
        .canonicalize()
        .expect("canonical initialized repository")
        .to_string_lossy()
        .into_owned();
    git(
        Some(&repository),
        &["config", "user.name", "Vela Recovery Test"],
    );
    git(
        Some(&repository),
        &["config", "user.email", "recovery@example.invalid"],
    );
    fs::write(
        repository.join("witness.json"),
        b"{\"schema\":\"vela.recovery-witness.v1\",\"ok\":true}\n",
    )
    .expect("submission witness");
    let submit_arguments = [
        "submit",
        "--repo",
        repository_text.as_str(),
        "--claim",
        "Explicit recovery leaves the next write unblocked.",
        "--type",
        "computational",
        "--replayability",
        "exact",
        "--artifact",
        "witness.json:witness",
        "--caveat",
        "Recovery integration fixture only.",
        "--as",
        "agent:recovery-fixture",
        "--json",
    ];

    let journal_dir = repository.join(".vela/operation-journals");
    let (operation_id, transaction) = prepare_transaction(
        &repository,
        &journal_dir,
        repository_id,
        "prepared product boundary",
        vec![PlannedWrite::write(
            RepoPath::parse("scratch/prepared.json").expect("prepared path"),
            WriteClass::PublicReview,
            b"{\"must_not_install\":true}\n".to_vec(),
        )],
    );
    let held_snapshot = tree_snapshot(&journal_dir);
    let busy = run(temporary.path(), &home, &submit_arguments);
    assert_eq!(busy.status.code(), Some(1));
    let busy = json_output(&busy);
    assert_eq!(busy["error"]["kind"], "domain");
    assert!(busy["error"]["code"].is_null());
    assert_eq!(
        busy["error"]["message"],
        "another repository transaction holds the write lock"
    );
    assert_eq!(
        busy["error"]["hint"],
        "wait for the active repository writer to exit, then rerun the same command"
    );
    assert!(
        !busy["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("vela recover")
    );
    assert_eq!(tree_snapshot(&journal_dir), held_snapshot);

    let busy_recovery = run_recover(&repository, &home, &operation_id, true);
    assert_eq!(busy_recovery.status.code(), Some(1));
    let busy_recovery = failed_recovery(&busy_recovery);
    assert_eq!(busy_recovery["error"]["kind"], "domain");
    assert!(busy_recovery["error"]["code"].is_null());
    let busy_hint = busy_recovery["error"]["hint"]
        .as_str()
        .expect("busy recovery retry hint");
    assert!(busy_hint.contains("wait for the active repository writer"));
    assert!(busy_hint.contains(&format!(
        "vela recover --repo '{}' {} --json",
        canonical_repository_text,
        operation_id.as_str()
    )));
    assert_eq!(tree_snapshot(&journal_dir), held_snapshot);
    drop(transaction);

    let before_reads = tree_snapshot(&journal_dir);
    let missing_claim = format!("vcl_{}", "a".repeat(64));
    let read_commands = [
        vec!["status", "--repo", repository_text.as_str(), "--json"],
        vec!["replay", "--repo", repository_text.as_str(), "--json"],
        vec![
            "show",
            "--repo",
            repository_text.as_str(),
            missing_claim.as_str(),
            "--json",
        ],
        vec![
            "why",
            "--repo",
            repository_text.as_str(),
            missing_claim.as_str(),
            "--json",
        ],
        vec!["log", "--repo", repository_text.as_str(), "--json"],
        vec!["reproduce", repository_text.as_str(), "--json"],
    ];
    for arguments in read_commands {
        let output = run(temporary.path(), &home, &arguments);
        assert!(
            output.status.code().is_some(),
            "read command did not return: {arguments:?}"
        );
    }
    assert_eq!(
        tree_snapshot(&journal_dir),
        before_reads,
        "read-only commands changed recovery files"
    );

    let blocked = run(temporary.path(), &home, &submit_arguments);
    assert_eq!(blocked.status.code(), Some(1));
    let blocked_json = json_output(&blocked);
    assert_eq!(blocked_json["error"]["code"], "repository_incomplete");
    let next = format!(
        "vela recover --repo '{}' {} --json",
        canonical_repository_text,
        operation_id.as_str()
    );
    assert_eq!(blocked_json["error"]["hint"], next);

    let human_submit = submit_arguments[..submit_arguments.len() - 1].to_vec();
    let blocked_human = run(temporary.path(), &home, &human_submit);
    assert_eq!(blocked_human.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&blocked_human.stderr).contains(&next),
        "human RecoveryRequired did not name exact action: {}",
        String::from_utf8_lossy(&blocked_human.stderr)
    );
    let recovered = run_recover(&repository, &home, &operation_id, true);
    let recovered = successful_recovery(&recovered, &operation_id, "aborted_prepared");
    assert_eq!(recovered["prior_recovery_state"], "prepared");
    assert_eq!(recovered["repository_id"], repository_id);
    assert_eq!(recovered["repository_path"], canonical_repository_text);
    assert_eq!(recovered["repository_blocked_after"], false);
    assert!(recovered["next_command"].is_null());
    assert!(!repository.join("scratch/prepared.json").exists());

    let repeated = run_recover(&repository, &home, &operation_id, true);
    let repeated = successful_recovery(&repeated, &operation_id, "already_aborted");
    assert_eq!(repeated["prior_recovery_state"], "aborted");
    assert!(!repeated["repository_blocked_after"].as_bool().unwrap());

    #[cfg(feature = "test-support")]
    {
        let publication_before = git_publication_snapshot(&repository);
        let interrupted = run_with_test_env(
            temporary.path(),
            &home,
            &submit_arguments,
            "VELA_TEST_INTERRUPT_SUBMIT_AFTER_INSTALL",
            "1",
        );
        assert_eq!(interrupted.status.code(), Some(1));
        let submitted_operation = operation_in_state(&journal_dir, "installed");
        let expected_hint = format!(
            "vela recover --repo '{}' {} --json",
            canonical_repository_text,
            submitted_operation.as_str()
        );
        let interrupted = json_output(&interrupted);
        assert_eq!(interrupted["error"]["code"], "repository_incomplete");
        assert_eq!(interrupted["error"]["hint"], expected_hint);
        let installed_snapshot = recovery_snapshot(&journal_dir);

        // The exact semantic result is already present, but the durable
        // transaction is not Completed. The command must surface recovery
        // before its idempotent-current-state shortcut can return success.
        let blocked_repeat = run(temporary.path(), &home, &submit_arguments);
        assert_eq!(blocked_repeat.status.code(), Some(1));
        let blocked_repeat = json_output(&blocked_repeat);
        assert_eq!(blocked_repeat["error"]["code"], "repository_incomplete");
        assert_eq!(blocked_repeat["error"]["hint"], expected_hint);
        assert_eq!(recovery_snapshot(&journal_dir), installed_snapshot);

        let finalized = run_recover(&repository, &home, &submitted_operation, true);
        let finalized = successful_recovery(&finalized, &submitted_operation, "completed");
        assert_eq!(finalized["prior_recovery_state"], "installed");
        assert_eq!(git_publication_snapshot(&repository), publication_before);

        // Recovery intentionally stops before Git publication. The operator
        // inspects and publishes those exact recovered bytes separately; only
        // then can the original semantic command return its idempotent result.
        git(Some(&repository), &["add", "-A"]);
        git(
            Some(&repository),
            &["commit", "-qm", "Publish explicitly recovered transaction"],
        );
        let idempotent_submit = run(temporary.path(), &home, &submit_arguments);
        assert!(
            idempotent_submit.status.success(),
            "same semantic write did not become idempotent after explicit recovery: stdout={} stderr={}",
            String::from_utf8_lossy(&idempotent_submit.stdout),
            String::from_utf8_lossy(&idempotent_submit.stderr)
        );
    }

    #[cfg(not(feature = "test-support"))]
    {
        let submitted = run(temporary.path(), &home, &submit_arguments);
        assert!(
            submitted.status.success(),
            "write after explicit abort remained blocked: stdout={} stderr={}",
            String::from_utf8_lossy(&submitted.stdout),
            String::from_utf8_lossy(&submitted.stderr)
        );
    }
}

#[test]
fn every_install_crash_position_recovers_exactly_without_git_publication() {
    let cases = vec![
        ("marker-before-install", None, "committed"),
        ("marker-durable-prepared-journal", None, "prepared"),
        ("before-write-0", Some(before_write(0)), "committed"),
        ("after-write-0", Some(after_write(0)), "committed"),
        ("before-progress-0", Some(before_progress(0)), "committed"),
        ("after-progress-0", Some(after_progress(0)), "installing"),
        ("before-write-1", Some(before_write(1)), "installing"),
        ("after-write-1", Some(after_write(1)), "installing"),
        ("before-progress-1", Some(before_progress(1)), "installing"),
        ("after-progress-1", Some(after_progress(1)), "installing"),
        (
            "before-installed",
            Some(RepositoryTxnStep::BeforeInstalledJournalWrite),
            "installing",
        ),
        (
            "after-installed",
            Some(RepositoryTxnStep::AfterInstalledJournalWrite),
            "installed",
        ),
    ];

    for (label, failpoint, prior_state) in cases {
        let fixture = RepoFixture::new(label);
        let (operation_id, mut transaction) = fixture.prepare(label, standard_writes());
        transaction.mark_committed().expect("durable commit marker");
        if label == "marker-durable-prepared-journal" {
            // The marker write and the subsequent Committed journal rewrite
            // are separate durability boundaries. Recovery must trust the
            // exact marker even when the older Prepared journal survived.
            write_journal_state(
                &plan_path(&fixture.journal_dir(), &operation_id),
                "prepared",
            );
        }
        if let Some(step) = failpoint {
            assert!(matches!(
                transaction.install_at_failpoint(step),
                Err(RepositoryTxnError::InjectedFailure { step: actual }) if actual == step
            ));
        }
        drop(transaction);

        let publication = git_publication_snapshot(&fixture.root);
        let marker = fs::read(marker_path(&fixture.journal_dir(), &operation_id))
            .expect("commit marker bytes");
        let names = recovery_snapshot(&fixture.journal_dir())
            .into_keys()
            .collect::<Vec<_>>();
        assert!(!blob_paths(&fixture.journal_dir()).is_empty());

        let recovered = fixture.recover(&operation_id);
        let recovered = successful_recovery(&recovered, &operation_id, "completed");
        assert_eq!(recovered["prior_recovery_state"], prior_state, "{label}");
        assert_eq!(recovered["repository_id"], fixture.repository_id, "{label}");
        assert_eq!(recovered["repository_blocked_after"], false, "{label}");
        assert_eq!(
            recovered["next_command"],
            format!("git -C '{}' status --short", fixture.root.display()),
            "{label}"
        );
        assert_eq!(
            fs::read(fixture.root.join("records/recovered.json")).expect("recovered record"),
            b"{\"recovered\":true}\n",
            "{label}"
        );
        assert_eq!(fs::read(fixture.root.join("base.txt")).unwrap(), b"after\n");
        assert_eq!(
            git_publication_snapshot(&fixture.root),
            publication,
            "{label}"
        );
        assert_eq!(
            fs::read(marker_path(&fixture.journal_dir(), &operation_id)).unwrap(),
            marker,
            "recovery rewrote the durable marker at {label}"
        );
        assert_eq!(
            recovery_snapshot(&fixture.journal_dir())
                .into_keys()
                .collect::<Vec<_>>(),
            names,
            "recovery created another operation or publication record at {label}"
        );
        assert!(!blob_paths(&fixture.journal_dir()).is_empty(), "{label}");

        let completed_snapshot = recovery_snapshot(&fixture.journal_dir());
        // Every failpoint above interrupts the same `install_with_failpoints`
        // routine production recovery invokes. This second real CLI call is
        // therefore the product-boundary retry after an interrupted recovery,
        // and must not change a byte once completion is durable.
        let repeated = fixture.recover(&operation_id);
        let repeated = successful_recovery(&repeated, &operation_id, "already_completed");
        assert_eq!(repeated["prior_recovery_state"], "completed", "{label}");
        assert_eq!(
            recovery_snapshot(&fixture.journal_dir()),
            completed_snapshot,
            "completed retry changed durable bytes at {label}"
        );
        if label == "marker-before-install" {
            let human = run_recover(&fixture.root, &fixture.home, &operation_id, false);
            assert!(human.status.success());
            let human = String::from_utf8(human.stdout).expect("UTF-8 human recovery output");
            assert!(human.contains("ALREADY_COMPLETED"));
            assert!(human.contains("prior state"));
            assert!(human.contains("completed"));
            assert!(human.contains("Git publication"));
            assert!(human.contains("not attempted"));
            assert!(human.contains("next"));
            assert!(human.contains("git -C"));
            assert!(human.contains("status --short"));
        }
    }

    #[cfg(feature = "test-support")]
    {
        let (fixture, operation_id) = committed_fixture("interrupted-real-recovery");
        let repository = fixture.root.to_string_lossy().into_owned();
        let interrupted = run_with_test_env(
            &fixture.root,
            &fixture.home,
            &[
                "recover",
                "--repo",
                repository.as_str(),
                operation_id.as_str(),
                "--json",
            ],
            "VELA_TEST_INTERRUPT_RECOVERY_AFTER_INSTALLED",
            "1",
        );
        assert_eq!(interrupted.status.code(), Some(1));
        let interrupted = failed_recovery(&interrupted);
        assert_eq!(interrupted["error"]["code"], "repository_incomplete");
        assert_eq!(
            operation_in_state(&fixture.journal_dir(), "installed"),
            operation_id
        );
        assert_eq!(fs::read(fixture.root.join("base.txt")).unwrap(), b"after\n");
        assert_eq!(
            fs::read(fixture.root.join("records/recovered.json")).unwrap(),
            b"{\"recovered\":true}\n"
        );

        let recovered =
            successful_recovery(&fixture.recover(&operation_id), &operation_id, "completed");
        assert_eq!(recovered["prior_recovery_state"], "installed");
        let completed = recovery_snapshot(&fixture.journal_dir());
        successful_recovery(
            &fixture.recover(&operation_id),
            &operation_id,
            "already_completed",
        );
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), completed);
    }
}

#[test]
fn corrupt_markers_and_blobs_fail_closed_at_the_real_command() {
    {
        let (fixture, operation_id) = committed_fixture("malformed-marker");
        let marker = marker_path(&fixture.journal_dir(), &operation_id);
        fs::write(&marker, b"{not-json\n").expect("malformed marker");
        let before = recovery_snapshot(&fixture.journal_dir());
        let output = fixture.recover(&operation_id);
        assert_eq!(output.status.code(), Some(1));
        let error = failed_recovery(&output);
        assert_eq!(error["error"]["kind"], "domain");
        assert_eq!(error["error"]["code"], "repository_incomplete");
        let hint = error["error"]["hint"]
            .as_str()
            .expect("actionable corrupt-marker hint");
        assert!(hint.contains("trusted backup"));
        assert!(hint.contains("never delete or reinterpret"));
        assert!(hint.contains(&format!(
            "vela recover --repo '{}' {} --json",
            fixture.root.display(),
            operation_id.as_str()
        )));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
    }

    {
        let (fixture, operation_id) = committed_fixture("unreadable-marker");
        let marker = marker_path(&fixture.journal_dir(), &operation_id);
        let original_mode = fs::metadata(&marker)
            .expect("marker metadata")
            .permissions()
            .mode();
        let mut permissions = fs::metadata(&marker).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&marker, permissions).expect("make marker unreadable");
        let output = fixture.recover(&operation_id);
        let mut restore = fs::metadata(&marker).unwrap().permissions();
        restore.set_mode(original_mode);
        fs::set_permissions(&marker, restore).expect("restore marker permissions");
        failed_recovery(&output);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
    }

    {
        let (fixture, operation_id) = committed_fixture("missing-blob");
        let blob = blob_paths(&fixture.journal_dir())
            .into_iter()
            .next()
            .expect("recovery blob");
        fs::remove_file(blob).expect("remove recovery blob");
        let before = recovery_snapshot(&fixture.journal_dir());
        failed_recovery(&fixture.recover(&operation_id));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
    }

    {
        let (fixture, operation_id) = committed_fixture("corrupt-blob");
        let blob = blob_paths(&fixture.journal_dir())
            .into_iter()
            .next()
            .expect("recovery blob");
        let mut value: Value = serde_json::from_slice(&fs::read(&blob).unwrap()).unwrap();
        value["bytes"] = json!([0]);
        let mut bytes = serde_json::to_vec_pretty(&value).unwrap();
        bytes.push(b'\n');
        fs::write(blob, bytes).expect("corrupt recovery blob bytes");
        let before = recovery_snapshot(&fixture.journal_dir());
        failed_recovery(&fixture.recover(&operation_id));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
    }

    {
        let (fixture, operation_id) = committed_fixture("missing-marker");
        fs::remove_file(marker_path(&fixture.journal_dir(), &operation_id))
            .expect("remove commit marker");
        let before = recovery_snapshot(&fixture.journal_dir());
        failed_recovery(&fixture.recover(&operation_id));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
    }

    {
        let (fixture, operation_id) = committed_fixture("orphan-marker");
        let orphan_id = OperationId::derive("recovery_fixture", b"orphan marker name");
        fs::rename(
            marker_path(&fixture.journal_dir(), &operation_id),
            marker_path(&fixture.journal_dir(), &orphan_id),
        )
        .expect("orphan the exact marker under a different valid name");
        let before = recovery_snapshot(&fixture.journal_dir());
        failed_recovery(&fixture.recover(&operation_id));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
    }
}

#[test]
fn conflicts_path_attacks_wrong_ids_and_copied_roots_never_redirect_recovery() {
    {
        let (fixture, operation_id) = committed_fixture("postimage-conflict");
        fs::write(fixture.root.join("base.txt"), b"third-party drift\n")
            .expect("post-marker drift");
        failed_recovery(&fixture.recover(&operation_id));
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"third-party drift\n"
        );
        fs::write(fixture.root.join("base.txt"), b"before\n").expect("repair preimage");
        let recovered = fixture.recover(&operation_id);
        let recovered = successful_recovery(&recovered, &operation_id, "completed");
        assert_eq!(recovered["prior_recovery_state"], "committed");
        assert_eq!(fs::read(fixture.root.join("base.txt")).unwrap(), b"after\n");
    }

    {
        let fixture = RepoFixture::new("symlink-target");
        let outside = fixture.root.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).expect("outside sentinel directory");
        let (operation_id, mut transaction) = fixture.prepare(
            "symlink-target",
            vec![PlannedWrite::write(
                RepoPath::parse("records/recovered.json").unwrap(),
                WriteClass::CanonicalEvidence,
                b"outside must stay untouched\n".to_vec(),
            )],
        );
        transaction.mark_committed().unwrap();
        drop(transaction);
        symlink(&outside, fixture.root.join("records")).expect("hostile target symlink");
        failed_recovery(&fixture.recover(&operation_id));
        assert!(!outside.join("recovered.json").exists());
        fs::remove_file(fixture.root.join("records")).expect("remove target symlink");
        successful_recovery(&fixture.recover(&operation_id), &operation_id, "completed");
        assert!(!outside.join("recovered.json").exists());
    }

    {
        let (fixture, operation_id) = committed_fixture("wrong-operation-id");
        let wrong = OperationId::derive("recovery_fixture", b"different exact operation");
        assert_ne!(wrong, operation_id);
        let before = recovery_snapshot(&fixture.journal_dir());
        let error = failed_recovery(&fixture.recover(&wrong));
        assert_eq!(error["error"]["kind"], "not_found");
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
    }

    {
        let (fixture, operation_id) = committed_fixture("copied-root");
        let copied = fixture.root.parent().unwrap().join("copied-repository");
        copy_tree(&fixture.root, &copied);
        let copied_journals = copied.join(".vela/operation-journals");
        let before = recovery_snapshot(&copied_journals);
        failed_recovery(&run_recover(&copied, &fixture.home, &operation_id, true));
        assert_eq!(recovery_snapshot(&copied_journals), before);
        assert_eq!(fs::read(copied.join("base.txt")).unwrap(), b"before\n");
    }

    {
        let (fixture, operation_id) = committed_fixture("same-root-wrong-identity");
        let profile_path = fixture.root.join("vela.toml");
        let original_profile = fs::read_to_string(&profile_path).expect("fixture profile");
        let mut profile = RepositoryProfileV1::from_toml_str(&original_profile).unwrap();
        profile.repository_id = "fedcba98-7654-4abc-8123-456789abcdef".into();
        fs::write(&profile_path, toml::to_string_pretty(&profile).unwrap())
            .expect("replace current profile identity");
        let before = recovery_snapshot(&fixture.journal_dir());
        let error = failed_recovery(&fixture.recover(&operation_id));
        assert!(
            error["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("repository identity mismatch")),
            "same-root identity transplant was not rejected: {error}"
        );
        assert_eq!(error["error"]["code"], "repository_incomplete");
        let hint = error["error"]["hint"]
            .as_str()
            .expect("identity mismatch hint");
        assert!(hint.contains("fedcba98-7654-4abc-8123-456789abcdef"));
        assert!(hint.contains("01234567-89ab-4def-8123-456789abcdef"));
        assert!(hint.contains("never rewrite either identity"));
        assert_eq!(recovery_snapshot(&fixture.journal_dir()), before);
        assert_eq!(
            fs::read(fixture.root.join("base.txt")).unwrap(),
            b"before\n"
        );
        fs::write(&profile_path, original_profile).expect("restore fixture profile");
        successful_recovery(&fixture.recover(&operation_id), &operation_id, "completed");
    }
}

#[test]
fn multiple_incomplete_journals_fail_before_mutation_and_terminal_retry_names_one_next() {
    let fixture = RepoFixture::new("multiple-incomplete");
    let (first_id, first) = fixture.prepare(
        "first incomplete",
        vec![PlannedWrite::write(
            RepoPath::parse("records/first.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"first\n".to_vec(),
        )],
    );
    drop(first);
    let first_plan = plan_path(&fixture.journal_dir(), &first_id);
    let first_prepared_bytes = fs::read(&first_plan).expect("first Prepared bytes");
    write_journal_state(&first_plan, "aborted");

    let (second_id, second) = fixture.prepare(
        "second incomplete",
        vec![PlannedWrite::write(
            RepoPath::parse("records/second.json").unwrap(),
            WriteClass::CanonicalEvidence,
            b"second\n".to_vec(),
        )],
    );
    drop(second);
    fs::write(&first_plan, &first_prepared_bytes).expect("restore first Prepared bytes");

    let impossible = recovery_snapshot(&fixture.journal_dir());
    let error = failed_recovery(&fixture.recover(&first_id));
    assert!(
        error["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains(second_id.as_str())),
        "ambiguous recovery did not name the other operation: {error}"
    );
    assert_eq!(recovery_snapshot(&fixture.journal_dir()), impossible);
    assert!(!fixture.root.join("records/first.json").exists());
    assert!(!fixture.root.join("records/second.json").exists());

    write_journal_state(&first_plan, "aborted");
    let terminal = fixture.recover(&first_id);
    let terminal = successful_recovery(&terminal, &first_id, "already_aborted");
    assert_eq!(terminal["repository_blocked_after"], true);
    assert_eq!(
        terminal["next_command"],
        format!(
            "vela recover --repo '{}' {} --json",
            fixture.root.display(),
            second_id.as_str()
        )
    );
    let recovered_second = fixture.recover(&second_id);
    let recovered_second = successful_recovery(&recovered_second, &second_id, "aborted_prepared");
    assert_eq!(recovered_second["repository_blocked_after"], false);
}
