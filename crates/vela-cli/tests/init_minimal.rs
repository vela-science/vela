//! Focused Profile v1 initialization regressions for ADR 0016.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use vela_protocol::events::StateEvent;
use vela_protocol::frontier_profile::{
    EffectiveFrontierAuthorityV1, FRONTIER_CREATED_SCHEMA_V1, FRONTIER_PROFILE_SCHEMA_V1,
};
use vela_protocol::frontier_repo::{FrontierLockFile, FrontierProfileFile};
use vela_protocol::frontier_settings::{FRONTIER_SETTINGS_SCHEMA, FrontierSettingsV1};

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vela"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env_remove("VELA_ACTOR_ID")
        .env_remove("VELA_KEY_PATH")
        .env_remove("VELA_NO_PUBLISH")
        .output()
        .expect("run vela")
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
        .unwrap_or_else(|error| panic!("decode init JSON: {error}\n{}", combined(output)))
}

fn collect_files(root: &Path, current: &Path, files: &mut BTreeSet<String>) {
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
            collect_files(root, &path, files);
        } else if path.is_file() {
            files.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn init_minimal_profile_v1_binds_genesis_and_reports_exact_writes() {
    let temporary = tempfile::tempdir().unwrap();
    let frontier = temporary.path().join("bounded-frontier");
    let name = "Bounded frontier";
    let scope = "Does one structural genesis reproduce an empty Frontier?";
    let payload = success_json(&run(
        temporary.path(),
        temporary.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            name,
            "--scope",
            scope,
            "--json",
        ],
    ));

    assert_eq!(payload["schema"], "vela.frontier_repo_init.v1");
    assert_eq!(payload["layout"], "vela.frontier_repo.v1");
    assert_eq!(payload["name"], name);
    assert_eq!(payload["scope"], scope);

    let event_id = payload["frontier_created_event_id"]
        .as_str()
        .expect("event id");
    let event_path = format!(".vela/events/{event_id}.json");
    let expected_writes = [
        "README.md".to_string(),
        "SCOPE.md".to_string(),
        "frontier.yaml".to_string(),
        "frontier.json".to_string(),
        "vela.lock".to_string(),
        ".gitignore".to_string(),
        ".gitattributes".to_string(),
        "VELA.md".to_string(),
        ".vela/settings.toml".to_string(),
        event_path.clone(),
        ".vela/proof-state.json".to_string(),
        ".vela/actors.json".to_string(),
    ];
    assert_eq!(
        payload["wrote"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        expected_writes
    );
    let mut actual_files = BTreeSet::new();
    collect_files(&frontier, &frontier, &mut actual_files);
    assert_eq!(
        actual_files,
        expected_writes.into_iter().collect::<BTreeSet<_>>()
    );

    let event: StateEvent =
        serde_json::from_slice(&std::fs::read(frontier.join(&event_path)).unwrap()).unwrap();
    assert_eq!(event.kind, "frontier.created");
    assert_eq!(event.id, event_id);
    assert_eq!(event.id, vela_protocol::events::compute_event_id(&event));
    assert_eq!(event.payload["schema"], FRONTIER_CREATED_SCHEMA_V1);
    assert_eq!(event.payload["name_at_creation"], name);
    assert_eq!(event.payload["creator"], event.actor.id);
    assert_eq!(event.payload["created_at"], event.timestamp);
    assert!(event.signature.is_none());
    assert!(event.caveats.is_empty());

    let project = vela_protocol::repo::load_from_path(&frontier).unwrap();
    assert_eq!(project.events.len(), 1);
    assert_eq!(project.frontier_id(), payload["frontier_id"]);
    assert!(vela_protocol::reducer::verify_replay(&project).ok);
    let authority = EffectiveFrontierAuthorityV1::from_events(&project.events).unwrap();
    assert_eq!(authority.frontier_id, project.frontier_id());
    assert_eq!(authority.dependency_root, payload["dependency_root"]);
    assert_eq!(
        authority.dependency_root,
        vela_protocol::frontier_repository::exact_dependency_root(&[]).unwrap()
    );

    let profile = match vela_protocol::frontier_repo::read_repository_profile(&frontier).unwrap() {
        Some(FrontierProfileFile::V1(profile)) => profile,
        other => panic!("expected Profile v1, got {other:?}"),
    };
    assert_eq!(profile.schema, FRONTIER_PROFILE_SCHEMA_V1);
    assert_eq!(profile.frontier_id, project.frontier_id());
    assert_eq!(profile.name, name);
    assert_eq!(profile.summary, scope);
    assert_eq!(profile.scope.question, scope);
    assert!(profile.scope.includes.is_empty());
    assert!(profile.scope.excludes.is_empty());
    assert_eq!(profile.profile_root().unwrap(), payload["profile_root"]);

    let settings = FrontierSettingsV1::from_toml(
        &std::fs::read_to_string(frontier.join(".vela/settings.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings.schema, FRONTIER_SETTINGS_SCHEMA);
    assert!(settings.publish.is_none());
    assert!(settings.work.is_none());
    assert!(settings.mcp.is_none());

    let lock = match vela_protocol::frontier_repo::read_repository_lock(&frontier).unwrap() {
        Some(FrontierLockFile::V1(lock)) => lock,
        other => panic!("expected Profile v1 lock, got {other:?}"),
    };
    assert_eq!(lock.frontier_id, project.frontier_id());
    assert_eq!(lock.event_count, 1);
    assert_eq!(lock.profile_root, profile.profile_root().unwrap());
    assert_eq!(lock.dependency_root, authority.dependency_root);
    assert!(lock.dependencies.is_empty());
    assert_eq!(lock.proof_freshness, "not_materialized");
}

#[test]
fn init_minimal_profile_v1_omits_integrations_and_materializes_without_history_drift() {
    let temporary = tempfile::tempdir().unwrap();
    let frontier = temporary.path().join("minimal-frontier");
    success_json(&run(
        temporary.path(),
        temporary.path(),
        &[
            "init",
            frontier.to_str().unwrap(),
            "--name",
            "Minimal frontier",
            "--scope",
            "Can Profile v1 initialize without optional integration surfaces?",
            "--json",
        ],
    ));

    for absent in [
        ".vela/config.toml",
        ".vela/hooks",
        ".vela/tasks",
        ".vela/workspaces",
        ".vela/source-inbox",
        ".mcp.json",
        ".github",
        ".vscode",
        "AGENTS.md",
        "CLAUDE.md",
        "proof",
        "targets.json",
    ] {
        assert!(!frontier.join(absent).exists(), "unexpected {absent}");
    }
    assert!(frontier.join(".vela/events").is_dir());
    assert!(frontier.join(".vela/findings").is_dir());
    assert!(frontier.join(".vela/proposals").is_dir());
    assert!(frontier.join(".vela/artifacts").is_dir());
    assert!(frontier.join(".git").is_dir());
    assert!(!temporary.path().join(".vela/frontiers.json").exists());

    let hooks_path = Command::new("git")
        .args([
            "-C",
            frontier.to_str().unwrap(),
            "config",
            "--get",
            "core.hooksPath",
        ])
        .output()
        .unwrap();
    assert!(!hooks_path.status.success());

    let event_path = std::fs::read_dir(frontier.join(".vela/events"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let event_before = std::fs::read(&event_path).unwrap();
    let profile_before = std::fs::read(frontier.join("frontier.yaml")).unwrap();

    let materialized = success_json(&run(
        temporary.path(),
        temporary.path(),
        &[
            "frontier",
            "materialize",
            frontier.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(materialized["wrote_frontier"], "frontier.json");
    assert_eq!(materialized["wrote_lock"], "vela.lock");
    assert!(frontier.join("proof/latest.json").is_file());
    assert_eq!(std::fs::read(&event_path).unwrap(), event_before);
    assert_eq!(
        std::fs::read(frontier.join("frontier.yaml")).unwrap(),
        profile_before
    );

    let reloaded = vela_protocol::repo::load_from_path(&frontier).unwrap();
    assert_eq!(reloaded.events.len(), 1);
    assert!(vela_protocol::reducer::verify_replay(&reloaded).ok);
    let check = success_json(&run(
        temporary.path(),
        temporary.path(),
        &["check", frontier.to_str().unwrap(), "--strict", "--json"],
    ));
    assert_eq!(check["ok"], true);
}

#[test]
fn init_minimal_json_still_requires_name_and_scope_before_writing() {
    let temporary = tempfile::tempdir().unwrap();
    for (directory, args, message) in [
        (
            "missing-name",
            vec![
                "init",
                "missing-name",
                "--scope",
                "Bounded question",
                "--json",
            ],
            "requires --name",
        ),
        (
            "missing-scope",
            vec!["init", "missing-scope", "--name", "Bounded name", "--json"],
            "requires --scope",
        ),
    ] {
        let output = run(temporary.path(), temporary.path(), &args);
        assert_eq!(output.status.code(), Some(2), "{}", combined(&output));
        assert!(combined(&output).contains(message), "{}", combined(&output));
        assert!(!temporary.path().join(directory).exists());
    }
}

#[test]
fn init_refuses_an_existing_nonempty_repository_without_overwriting_it() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path().join("existing");
    std::fs::create_dir(&repository).unwrap();
    let sentinel = b"user-authored repository readme\n";
    std::fs::write(repository.join("README.md"), sentinel).unwrap();
    std::fs::write(repository.join("frontier.yaml"), b"not a Vela profile\n").unwrap();

    let output = run(
        temporary.path(),
        temporary.path(),
        &[
            "init",
            repository.to_str().unwrap(),
            "--name",
            "Must not overwrite",
            "--scope",
            "Can initialization preserve an existing repository?",
            "--json",
        ],
    );
    assert!(!output.status.success(), "{}", combined(&output));
    assert!(
        combined(&output).contains("refusing to initialize non-empty directory"),
        "{}",
        combined(&output)
    );
    assert_eq!(
        std::fs::read(repository.join("README.md")).unwrap(),
        sentinel
    );
    assert_eq!(
        std::fs::read(repository.join("frontier.yaml")).unwrap(),
        b"not a Vela profile\n"
    );
    assert!(!repository.join(".vela").exists());
    assert!(!repository.join("vela.lock").exists());
}
