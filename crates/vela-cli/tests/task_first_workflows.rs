//! Cross-surface regressions for ADR 0003's task-first trust boundary.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

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

fn run_mcp_tool(
    dir: &Path,
    profile: &str,
    name: &str,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    });
    let mut command = Command::new(vela_bin());
    command
        .current_dir(dir)
        .args(["serve", ".", "--profile", profile])
        .env("HOME", dir)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars() {
        if key.starts_with("VELA_") && key != "VELA_ADVICE" {
            command.env_remove(key);
        }
    }
    let mut server = command.spawn().expect("start MCP server");
    writeln!(server.stdin.as_mut().unwrap(), "{request}").unwrap();
    drop(server.stdin.take());
    let output = server.wait_with_output().expect("wait for MCP server");
    assert_success(&output, &format!("MCP {name}"));
    let rpc = one_json_object(&output);
    assert_eq!(rpc["result"]["isError"], false, "{rpc}");
    serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap())
        .expect("MCP content is one JSON envelope")
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

fn work_session_path(work: &serde_json::Value) -> std::path::PathBuf {
    std::path::PathBuf::from(
        work["session"]["path"]
            .as_str()
            .expect("compact work response has session.path"),
    )
}

fn load_work_session(work: &serde_json::Value) -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(work_session_path(work)).expect("read private work session"),
    )
    .expect("parse private work session")
}

fn init_git_frontier(dir: &Path) {
    assert_success(
        &run(
            dir,
            &[
                "init",
                ".",
                "--name",
                "task-first",
                "--scope",
                "Exercise the bounded task-first fixture.",
                "--json",
            ],
        ),
        "init frontier",
    );
    assert_success(
        &run(dir, &["id", "create", "--handle", "t", "--agent"]),
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

fn register_deterministic_reviewer(dir: &Path, seed: u8) -> std::path::PathBuf {
    use ed25519_dalek::SigningKey;
    use vela_protocol::sign::ActorRecord;

    let key = SigningKey::from_bytes(&[seed; 32]);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    let identity_path = dir.join(".vela/identity.json");
    let mut identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&identity_path).unwrap()).unwrap();
    let key_path = dir.join(".vela/keys/t/private.key");
    std::fs::write(&key_path, hex::encode(key.to_bytes())).unwrap();
    identity["actor_id"] = "reviewer:t".into();
    identity["actor_type"] = "human".into();
    identity["pubkey"] = public_key.clone().into();
    identity["key_path"] = key_path.display().to_string().into();
    std::fs::write(
        &identity_path,
        format!("{}\n", serde_json::to_string_pretty(&identity).unwrap()),
    )
    .unwrap();

    let mut project = vela_protocol::repo::load_from_path(dir).unwrap();
    project.actors.retain(|actor| actor.id != "reviewer:t");
    project.actors.push(ActorRecord {
        id: "reviewer:t".to_string(),
        public_key,
        algorithm: "ed25519".to_string(),
        created_at: "2026-07-13T00:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    });
    vela_protocol::repo::save_to_path(dir, &project).unwrap();
    key_path
}

#[test]
fn work_lease_preserves_strict_freshness_of_a_recorded_proof() {
    let tmp = tempfile::tempdir().unwrap();
    let packet = tempfile::tempdir().unwrap();
    init_git_frontier(tmp.path());

    let proof = run(
        tmp.path(),
        &[
            "proof",
            ".",
            "--out",
            packet.path().to_str().unwrap(),
            "--record-proof-state",
            "--json",
        ],
    );
    assert_success(&proof, "record exact proof state");
    let mut historical = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert!(
        historical
            .proof_state
            .latest_packet
            .nonlease_event_log_hash
            .is_some(),
        "new proof exports must record the explicit non-lease root"
    );
    // Model the exact pre-fix Sidon record. The work path may backfill this
    // optional derived field only while the historical full event root still
    // proves that the non-lease event set is unchanged.
    historical.proof_state.latest_packet.nonlease_event_log_hash = None;
    vela_protocol::repo::save_to_path(tmp.path(), &historical).unwrap();
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage proof state");
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "record proof state"]),
        "commit proof state",
    );
    let before = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let before_event_root = vela_protocol::events::event_log_hash(&before.events);
    assert!(
        before
            .proof_state
            .latest_packet
            .nonlease_event_log_hash
            .is_none(),
        "fixture must reach work with the historical proof-state shape"
    );

    let key = "64".repeat(32);
    let work = run_with_env(
        tmp.path(),
        &["work", "seed:proof-freshness", "--as", "agent:t", "--json"],
        &[("VELA_AGENT_KEY_HEX", key.as_str())],
    );
    assert_success(&work, "claim work without staling proof");

    let check = run(tmp.path(), &["check", ".", "--strict", "--json"]);
    assert_success(&check, "strict check after coordination lease");
    let check = one_json_object(&check);
    assert_eq!(check["state_integrity"]["proof_freshness"], "fresh");

    let after = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(
        after
            .proof_state
            .latest_packet
            .nonlease_event_log_hash
            .as_deref(),
        Some(before_event_root.as_str()),
        "work must backfill the exact non-lease commitment before appending its lease"
    );
    assert_ne!(
        vela_protocol::events::event_log_hash(&after.events),
        before_event_root,
        "the test must append a real signed work lease"
    );
    assert_eq!(
        vela_protocol::events::nonlease_event_log_hash(&after.events),
        vela_protocol::events::nonlease_event_log_hash(&before.events),
        "the lease must be the only event-set change"
    );
}

#[test]
fn temporal_actor_registration_strict_check_preserves_legacy_history() {
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use vela_protocol::actor_registration::ACTOR_REGISTRATION_BOUNDARY_SCHEMA;
    use vela_protocol::events::{
        EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED, EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent,
        StateTarget, compute_event_id,
    };

    let tmp = tempfile::tempdir().unwrap();
    init_git_frontier(tmp.path());
    register_deterministic_reviewer(tmp.path(), 0x55);
    let key = SigningKey::from_bytes(&[0x55; 32]);
    let public_key = hex::encode(key.verifying_key().to_bytes());

    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let mut legacy = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: "research_trace.review".into(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: project.frontier_id(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: "reviewer:t".to_string(),
        },
        timestamp: "2026-07-01T00:00:00Z".to_string(),
        reason: "Unsigned legacy review record.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({}),
        caveats: vec![],
        signature: None,
    };
    legacy.id = compute_event_id(&legacy);
    project.events.push(legacy);
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage anchor");
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "actor registration anchor"]),
        "commit anchor",
    );
    let anchor_commit = git_stdout(tmp.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    let anchor_tree = git_stdout(tmp.path(), &["show", "-s", "--format=%T", "HEAD"])
        .trim()
        .to_string();
    let registry_bytes = std::fs::read(tmp.path().join(".vela/actors.json")).unwrap();
    let registry_root = format!("sha256:{}", hex::encode(Sha256::digest(registry_bytes)));
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );

    let mut activation = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED.into(),
        target: StateTarget {
            r#type: "actor".to_string(),
            id: "reviewer:t".to_string(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: "reviewer:t".to_string(),
        },
        timestamp: "2026-07-16T00:00:00Z".to_string(),
        reason: "Activate temporal actor registration.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "schema": ACTOR_REGISTRATION_BOUNDARY_SCHEMA,
            "mode": "temporalize_existing",
            "frontier_id": project.frontier_id(),
            "actor_id": "reviewer:t",
            "public_key": public_key,
            "algorithm": "ed25519",
            "anchor": {
                "git_object_format": "sha1",
                "git_commit": anchor_commit,
                "git_tree": anchor_tree,
                "event_log_root": event_root,
                "event_count": project.events.len(),
                "actor_registry_root": registry_root
            }
        }),
        caveats: vec!["Unsigned anchor members remain unauthenticated.".to_string()],
        signature: None,
    };
    activation.id = compute_event_id(&activation);
    activation.signature = Some(vela_protocol::sign::sign_event(&activation, &key).unwrap());
    project.events.push(activation);
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    assert_success(
        &run(tmp.path(), &["frontier", "materialize", ".", "--json"]),
        "materialize temporal registration fixture",
    );
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage activation");
    assert_success(
        &git(
            tmp.path(),
            &["commit", "-qm", "activate actor registration"],
        ),
        "commit activation",
    );

    let checked = run(tmp.path(), &["check", ".", "--strict", "--json"]);
    assert_success(&checked, "strict temporal actor registration");
    let payload: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    let kinds = payload["signals"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|signal| signal["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"pre_registration_unsigned_actor_event"));
    assert!(!kinds.contains(&"unsigned_registered_actor"));
}

#[test]
fn temporal_actor_registration_requires_a_pinned_repository_boundary_before_key_use() {
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use vela_protocol::events::{
        EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
    };

    let tmp = tempfile::tempdir().unwrap();
    init_git_frontier(tmp.path());
    register_deterministic_reviewer(tmp.path(), 0x66);
    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let mut legacy = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: "research_trace.review".into(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: project.frontier_id(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: "reviewer:t".to_string(),
        },
        timestamp: "2026-07-01T00:00:00Z".to_string(),
        reason: "Immutable unsigned legacy event.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({}),
        caveats: vec![],
        signature: None,
    };
    legacy.id = compute_event_id(&legacy);
    project.events.push(legacy);
    let before = project.events.len();
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    let legacy_path = tmp
        .path()
        .join(".vela/events")
        .join(format!("{}.json", project.events.last().unwrap().id));
    let mut explicit_null = serde_json::to_value(project.events.last().unwrap()).unwrap();
    explicit_null["signature"] = serde_json::Value::Null;
    std::fs::write(
        &legacy_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&explicit_null).unwrap()
        ),
    )
    .unwrap();
    let before_event_bytes = std::fs::read_dir(tmp.path().join(".vela/events"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage anchor");
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "temporal activation anchor"]),
        "commit anchor",
    );
    let anchor = git_stdout(tmp.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_string();

    let preview = run(
        tmp.path(),
        &[
            "actor",
            "activate",
            ".",
            "--anchor",
            &anchor,
            "--preview",
            "--json",
        ],
    );
    assert_success(&preview, "actor activation preview");
    let preview_json: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview_json["command"], "actor.activate.preview");
    assert_eq!(preview_json["counts"]["anchored_unsigned"], 1);
    let preview_root = preview_json["preview_root"].as_str().unwrap().to_string();
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before
    );

    let unconfirmed = run(
        tmp.path(),
        &["actor", "activate", ".", "--anchor", &anchor, "--json"],
    );
    assert!(!unconfirmed.status.success());
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before
    );

    let refused = run_with_env(
        tmp.path(),
        &[
            "actor",
            "activate",
            ".",
            "--anchor",
            &anchor,
            "--actor",
            "reviewer:t",
            "--yes",
            "--confirm-root",
            &preview_root,
            "--json",
        ],
        &[("VELA_ACTOR_ID", "agent:attempted-custody")],
    );
    assert_eq!(refused.status.code(), Some(4));
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before
    );

    let stale = run(
        tmp.path(),
        &[
            "actor",
            "activate",
            ".",
            "--anchor",
            &anchor,
            "--yes",
            "--confirm-root",
            &format!("sha256:{}", "0".repeat(64)),
            "--json",
        ],
    );
    assert!(!stale.status.success());
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before
    );

    let wrong_key = SigningKey::from_bytes(&[0x67; 32]);
    std::fs::write(
        tmp.path().join(".vela/keys/t/private.key"),
        hex::encode(wrong_key.to_bytes()),
    )
    .unwrap();
    let wrong_key_attempt = run(
        tmp.path(),
        &[
            "actor",
            "activate",
            ".",
            "--anchor",
            &anchor,
            "--yes",
            "--confirm-root",
            &preview_root,
            "--json",
        ],
    );
    assert!(!wrong_key_attempt.status.success());
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before
    );
    let correct_key = SigningKey::from_bytes(&[0x66; 32]);
    std::fs::write(
        tmp.path().join(".vela/keys/t/private.key"),
        hex::encode(correct_key.to_bytes()),
    )
    .unwrap();
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();

    let blocked_without_repository_boundary = run(
        tmp.path(),
        &[
            "actor",
            "activate",
            ".",
            "--anchor",
            &anchor,
            "--yes",
            "--confirm-root",
            &preview_root,
            "--json",
        ],
    );
    assert!(!blocked_without_repository_boundary.status.success());
    assert!(
        String::from_utf8_lossy(&blocked_without_repository_boundary.stderr)
            .contains("repository_write_intent_denied")
    );
    let reloaded = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(reloaded.events.len(), before);
    for (name, bytes) in &before_event_bytes {
        assert_eq!(
            std::fs::read(tmp.path().join(".vela/events").join(name)).unwrap(),
            *bytes,
            "blocked actor activation rewrote immutable event file {name}"
        );
    }
    assert!(
        reloaded
            .events
            .iter()
            .all(|event| event.kind.as_str() != "actor.registration_activated")
    );
}

#[test]
fn work_and_land_preserve_all_preexisting_event_bytes() {
    use serde_json::json;
    use vela_protocol::events::{
        EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
    };

    let tmp = tempfile::tempdir().unwrap();
    init_git_frontier(tmp.path());
    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let mut legacy = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: "research_trace.review".into(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: project.frontier_id(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: "reviewer:legacy".to_string(),
        },
        timestamp: "2026-07-01T00:00:00Z".to_string(),
        reason: "Immutable pre-registration history.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({"fixture": "cold-use"}),
        caveats: Vec::new(),
        signature: None,
    };
    legacy.id = compute_event_id(&legacy);
    project.events.push(legacy.clone());
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    let legacy_path = tmp
        .path()
        .join(".vela/events")
        .join(format!("{}.json", legacy.id));
    let mut explicit_null = serde_json::to_value(&legacy).unwrap();
    explicit_null["signature"] = serde_json::Value::Null;
    let legacy_bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(&explicit_null).unwrap()
    )
    .into_bytes();
    std::fs::write(&legacy_path, &legacy_bytes).unwrap();
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    std::fs::write(
        tmp.path().join("artifacts/byte-preservation.json"),
        br#"{"preserve":true}"#,
    )
    .unwrap();
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage byte fixture");
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "freeze byte fixture"]),
        "commit byte fixture",
    );

    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let work = run_with_env(
        tmp.path(),
        &[
            "work",
            "erdos:byte-preservation",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&work, "open byte-preserving work session");
    assert_eq!(
        std::fs::read(&legacy_path).unwrap(),
        legacy_bytes,
        "work rewrote immutable legacy event bytes"
    );
    let after_work = std::fs::read_dir(tmp.path().join(".vela/events"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let land = run_with_env(
        tmp.path(),
        &[
            "land",
            "--work",
            "erdos:byte-preservation",
            "--claim",
            "The transaction preserves every preexisting event byte.",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/byte-preservation.json:witness",
            "--caveat",
            "Fixture evidence only.",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&land, "land byte-preserving receipt");
    for (name, bytes) in after_work {
        assert_eq!(
            std::fs::read(tmp.path().join(".vela/events").join(&name)).unwrap(),
            bytes,
            "land rewrote immutable event file {name}"
        );
    }
}

fn write_receipt(dir: &Path, filename: &str, claim: &str) {
    write_receipt_with_artifact(dir, filename, claim, "w.json", br#"{"witness":true}"#);
}

fn write_receipt_with_artifact(
    dir: &Path,
    filename: &str,
    claim: &str,
    artifact_name: &str,
    artifact: &[u8],
) {
    write_receipt_with_artifact_as(
        dir,
        filename,
        claim,
        artifact_name,
        artifact,
        "agent:t",
        0x42,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_receipt_with_artifact_as(
    dir: &Path,
    filename: &str,
    claim: &str,
    artifact_name: &str,
    artifact: &[u8],
    actor: &str,
    key_seed: u8,
) {
    use ed25519_dalek::SigningKey;
    use sha2::{Digest, Sha256};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{
        ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput,
    };

    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    let artifact_path = format!("artifacts/{artifact_name}");
    std::fs::write(dir.join(&artifact_path), artifact).unwrap();
    let digest = hex::encode(Sha256::digest(artifact));
    let project = vela_protocol::repo::load_from_path(dir).unwrap();
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let operation_id = format!(
        "vop_{}",
        hex::encode(Sha256::digest(
            format!("{actor}\0{filename}\0{claim}\0{artifact_path}\0{digest}").as_bytes()
        ))
    );
    let at = "2026-07-13T12:00:00Z";
    let key = SigningKey::from_bytes(&[key_seed; 32]);
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            created_at: at.to_string(),
        },
        &key,
    )
    .unwrap();
    let input = ReceiptInput::new(
        claim.to_string(),
        "computational".to_string(),
        "exact".to_string(),
        vec![ArtifactInput::new(artifact_path, "witness".to_string(), Some(digest), None).unwrap()],
        vec!["fixture evidence only".to_string()],
        vec![
            ProducerReportedRun::producer_reported("fixture".to_string(), "pass".to_string())
                .unwrap(),
        ],
        actor.to_string(),
        at.to_string(),
        event_root,
        ".".to_string(),
        operation_id,
        "urn:vela:policy:none".to_string(),
    )
    .unwrap();
    let receipt = ReceiptBuilder::build(input, &identity).unwrap();
    std::fs::write(dir.join(filename), receipt.canonical_bytes().unwrap()).unwrap();
}

fn write_active_deny_policy(dir: &Path) {
    write_active_deny_policy_with_expiry(dir, "2099-12-31T23:59:59Z");
}

fn write_active_deny_policy_with_expiry(dir: &Path, expires_at: &str) {
    use ed25519_dalek::{Signer, SigningKey};
    use vela_protocol::acceptance_policy::{
        AcceptancePolicy, Outcome, PolicySignatureRecord, Quorum,
    };

    let frontier = vela_protocol::repo::load_from_path(dir).unwrap();
    let mut policy = AcceptancePolicy {
        schema: "vela.acceptance_policy.v0.1".to_string(),
        id: String::new(),
        frontier_id: frontier.frontier_id().to_string(),
        epoch: 1,
        issued_by: vec!["reviewer:deny-fixture".to_string()],
        quorum: Quorum {
            threshold: 1,
            eligible_roles: vec!["reviewer".to_string()],
        },
        rules: Vec::new(),
        default: Outcome::Deny,
        expires_at: expires_at.to_string(),
        revocation_ref: None,
    };
    policy.id = policy.content_address();
    let key = SigningKey::from_bytes(&[0x24; 32]);
    let signed_at = "2026-07-12T00:00:00Z";
    let signature = key.sign(
        &vela_protocol::acceptance_policy::policy_signature_preimage(&policy, signed_at).unwrap(),
    );
    let policy_dir = dir.join(".vela/policies");
    std::fs::create_dir_all(&policy_dir).unwrap();
    std::fs::write(
        policy_dir.join("active.json"),
        serde_json::to_vec_pretty(&policy).unwrap(),
    )
    .unwrap();
    std::fs::write(
        policy_dir.join("active.sig.json"),
        serde_json::to_vec_pretty(&PolicySignatureRecord {
            policy_id: policy.id,
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(signature.to_bytes()),
            signed_at: signed_at.to_string(),
        })
        .unwrap(),
    )
    .unwrap();
}

fn break_active_policy_content_address(dir: &Path) {
    let path = dir.join(".vela/policies/active.json");
    let mut policy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    policy["expires_at"] = "2098-12-31T23:59:59Z".into();
    std::fs::write(path, serde_json::to_vec_pretty(&policy).unwrap()).unwrap();
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

fn strip_cli_land_envelope(mut value: serde_json::Value) -> serde_json::Value {
    let fields = value.as_object_mut().expect("CLI land output is an object");
    assert_eq!(fields.remove("ok"), Some(serde_json::json!(true)));
    assert_eq!(fields.remove("command"), Some(serde_json::json!("land")));
    let request_id = fields.remove("request_id").expect("CLI request id");
    assert_eq!(request_id, fields["operation_id"]);
    value
}

fn assert_land_rejected_without_git_change(
    dir: &Path,
    filename: &str,
    receipt: &serde_json::Value,
    context: &str,
) {
    let before_head = git_stdout(dir, &["rev-parse", "HEAD"]);
    let before_status = git_stdout(dir, &["status", "--porcelain"]);
    let path = dir.join(filename);
    std::fs::write(&path, serde_json::to_vec(receipt).unwrap()).unwrap();
    let landed = run(dir, &["land", filename, "--as", "agent:t", "--json"]);
    assert!(
        !landed.status.success(),
        "{context} unexpectedly landed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&landed.stdout),
        String::from_utf8_lossy(&landed.stderr)
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(git_stdout(dir, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(
        git_stdout(dir, &["status", "--porcelain"]),
        before_status,
        "{context} changed the frontier before rejection"
    );
}

fn refresh_receipt_binding(receipt: &mut serde_json::Value) {
    use serde_json::json;

    fn base64_standard(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            encoded.push(TABLE[(a >> 2) as usize] as char);
            encoded.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
            encoded.push(if chunk.len() > 1 {
                TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            encoded.push(if chunk.len() > 2 {
                TABLE[(c & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        encoded
    }

    let mut body = receipt.as_object().unwrap().clone();
    body.remove("attestation");
    let body_root =
        vela_protocol::canonical::sha256_canonical(&serde_json::Value::Object(body)).unwrap();
    let machine = receipt["machine"].clone();
    let statement = json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": machine["subject"].clone(),
        "predicateType": "https://vela.science/receipt/v1",
        "predicate": {
            "schema": "vela.receipt.predicate.v1",
            "machine": machine,
            "acceptance": receipt["acceptance"].clone(),
            "distillation": receipt["distillation"].clone(),
            "lineage": receipt["lineage"].clone(),
            "contributors": receipt["contributors"].clone(),
            "signature_identities": receipt["signature_identities"].clone(),
            "provenance": receipt["provenance"].clone(),
            "vela:receipt_body": {"sha256": body_root},
        }
    });
    let payload = vela_protocol::canonical::to_canonical_bytes(&statement).unwrap();
    receipt["attestation"]["statement"] = statement;
    receipt["attestation"]["dsse_envelope"]["payload"] =
        serde_json::Value::String(base64_standard(&payload));
}

fn snapshot_scientific_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        if !path.exists() {
            return;
        }
        let relative = path.strip_prefix(root).unwrap();
        let mut components = relative.components();
        if components
            .next()
            .is_some_and(|part| part.as_os_str() == ".vela")
            && components.next().is_some_and(|part| {
                matches!(
                    part.as_os_str().to_str(),
                    Some(
                        "agents"
                            | "keys"
                            | "operation-journals"
                            | "tasks"
                            | "work"
                            | "workspaces"
                            | "source-inbox"
                            | "artifact-blobs"
                    )
                )
            })
        {
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
            out.push((relative.display().to_string(), std::fs::read(path).unwrap()));
        }
    }

    let mut out = Vec::new();
    for relative in [".vela", "records", "frontier.json", "vela.lock", "proof"] {
        collect(dir, &dir.join(relative), &mut out);
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

fn snapshot_exact_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, path: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            return;
        };
        if metadata.is_dir() {
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
    collect(root, root, &mut out);
    out
}

#[derive(Debug, PartialEq, Eq)]
struct GitPrivateSnapshot {
    object_store: String,
    publication_state: Vec<(String, Vec<u8>)>,
}

fn snapshot_git_private_state(dir: &Path) -> GitPrivateSnapshot {
    let git_dir = std::path::PathBuf::from(git_stdout(dir, &["rev-parse", "--git-dir"]).trim());
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        dir.join(git_dir)
    };
    GitPrivateSnapshot {
        object_store: git_stdout(dir, &["count-objects", "-v"]),
        publication_state: snapshot_exact_tree(&git_dir.join("vela")),
    }
}

fn valid_ustar_archive(path: &str, contents: &[u8]) -> Vec<u8> {
    fn write_octal(field: &mut [u8], value: u64) {
        let octal = format!("{value:o}");
        assert!(octal.len() < field.len());
        field.fill(b'0');
        let start = field.len() - octal.len() - 1;
        field[start..start + octal.len()].copy_from_slice(octal.as_bytes());
        field[field.len() - 1] = 0;
    }

    assert!(
        path.len() <= 100,
        "ustar fixture path must fit the name field"
    );
    let mut header = [0_u8; 512];
    header[..path.len()].copy_from_slice(path.as_bytes());
    write_octal(&mut header[100..108], 0o644);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], contents.len() as u64);
    write_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    header[265..269].copy_from_slice(b"vela");
    header[297..301].copy_from_slice(b"vela");
    let checksum = header.iter().map(|byte| u64::from(*byte)).sum::<u64>();
    let checksum = format!("{checksum:06o}\0 ");
    assert_eq!(checksum.len(), 8);
    header[148..156].copy_from_slice(checksum.as_bytes());

    let mut archive = header.to_vec();
    archive.extend_from_slice(contents);
    let padding = (512 - archive.len() % 512) % 512;
    archive.resize(archive.len() + padding + 1024, 0);
    archive
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
    assert!(value["original_route"].is_null(), "{value}");
    assert_eq!(
        value["accepted_event_count_before"], value["accepted_event_count_after"],
        "a deferred landing must not append an accepted event: {value}"
    );
    assert_eq!(value["accepted_event_delta"], 0, "{value}");
}

#[test]
fn exact_receipt_retry_is_idempotent_across_frontier_and_git() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "an exact normalized receipt retries without a second transition",
    );

    let first = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "first exact landing");
    let first = one_json_object(&first);
    assert_eq!(first["route"], "deferred", "{first}");
    assert!(first["original_route"].is_null(), "{first}");
    assert_eq!(first["accepted_event_delta"], 0, "{first}");

    let frontier_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let retry = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&retry, "exact retry");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    assert_eq!(retry["original_route"], "deferred", "{retry}");
    assert_eq!(
        retry["accepted_event_count_before"], first["accepted_event_count_before"],
        "exact retry changed the recorded transaction preimage count"
    );
    assert_eq!(
        retry["accepted_event_count_after"], first["accepted_event_count_after"],
        "exact retry changed the recorded transaction postimage count"
    );
    assert_eq!(retry["accepted_event_delta"], 0, "{retry}");
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(retry[key], first[key], "exact retry changed {key}");
    }
    assert_eq!(
        retry["publication"]["state"], "committed_local",
        "exact retry must recover or recognize the original publication: {retry}"
    );
    assert_eq!(
        retry["publication"]["commit"], first["publication"]["commit"],
        "exact retry minted a second publication commit: {retry}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), frontier_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
}

#[test]
fn land_wire_parity_is_exact_between_cli_and_mcp_retries() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "CLI and MCP expose one transport-neutral durable landing result",
    );
    let receipt =
        String::from_utf8(std::fs::read(tmp.path().join("receipt.json")).unwrap()).unwrap();

    let first = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "initial deferred landing");
    let first = one_json_object(&first);
    assert_eq!(first["route"], "deferred", "{first}");
    assert!(first["original_route"].is_null(), "{first}");
    assert_eq!(first["accepted_event_delta"], 0, "{first}");

    let frontier_path = tmp.path().canonicalize().unwrap();
    let mcp = run_mcp_tool(
        tmp.path(),
        "draft",
        "work",
        serde_json::json!({
            "action": "land",
            "frontier_path": frontier_path,
            "agent_actor": "agent:t",
            "receipt": receipt,
        }),
    );
    assert_eq!(mcp["tool"], "work", "{mcp}");
    assert_eq!(mcp["ok"], true, "{mcp}");
    let mcp_wire = mcp["data"].clone();
    assert_eq!(mcp_wire["route"], "exact_retry", "{mcp}");
    assert_eq!(mcp_wire["original_route"], "deferred", "{mcp}");

    let cli = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&cli, "CLI exact retry after MCP exact retry");
    assert_eq!(strip_cli_land_envelope(one_json_object(&cli)), mcp_wire);
}

#[test]
fn exact_retry_with_no_vela_git_delta_preserves_nonempty_caller_index() {
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
    write_receipt(
        tmp.path(),
        "receipt.json",
        "an exact retry with no Vela Git delta preserves caller staging",
    );

    let first = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "first landing before nonempty-index retry");
    let first = one_json_object(&first);
    assert_eq!(first["route"], "deferred", "{first}");

    std::fs::write(tmp.path().join("notes.txt"), "caller staged bytes\n").unwrap();
    assert_success(
        &git(tmp.path(), &["add", "notes.txt"]),
        "stage caller bytes",
    );
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let commits_before = git_stdout(tmp.path(), &["rev-list", "--count", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let objects_before = git_stdout(tmp.path(), &["count-objects", "-v"]);
    let scientific_before = snapshot_scientific_tree(tmp.path());
    let staged_before = git_stdout(tmp.path(), &["show", ":notes.txt"]);
    let committed_before = git_stdout(tmp.path(), &["show", "HEAD:notes.txt"]);
    // Capture raw index bytes after all Git reads above; `git status` may
    // legitimately refresh stat-cache fields while preserving logical entries.
    let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

    let retry = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&retry, "exact retry with nonempty caller index");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    assert_eq!(retry["publication"]["state"], "committed_local", "{retry}");
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(retry[key], first[key], "exact retry changed {key}");
    }
    assert_eq!(
        retry["publication"]["commit"],
        first["publication"]["commit"]
    );
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(tmp.path(), &["rev-list", "--count", "HEAD"]),
        commits_before
    );
    assert_eq!(
        std::fs::read(tmp.path().join(".git/index")).unwrap(),
        index_before
    );
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
    assert_eq!(
        git_stdout(tmp.path(), &["count-objects", "-v"]),
        objects_before
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(
        git_stdout(tmp.path(), &["show", ":notes.txt"]),
        staged_before
    );
    assert_eq!(
        git_stdout(tmp.path(), &["show", "HEAD:notes.txt"]),
        committed_before
    );
}

#[test]
fn completed_scientific_retry_resumes_git_publication() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "a completed scientific transaction retains resumable publication intent",
    );
    let baseline = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let scientific_only = run_with_env(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
        &[("VELA_NO_PUBLISH", "1")],
    );
    assert_success(&scientific_only, "scientific-only landing");
    let scientific_only = one_json_object(&scientific_only);
    assert_eq!(scientific_only["publication"]["state"], "uncommitted");
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), baseline);

    let resumed = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&resumed, "resume exact publication");
    let resumed = one_json_object(&resumed);
    assert_eq!(resumed["route"], "exact_retry", "{resumed}");
    assert_eq!(resumed["publication"]["state"], "committed_local");
    let published = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    assert_ne!(published, baseline);
    assert_eq!(
        resumed["publication"]["commit"].as_str().unwrap(),
        published.trim()
    );

    let idempotent = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&idempotent, "recognize resumed publication");
    let idempotent = one_json_object(&idempotent);
    assert_eq!(
        idempotent["publication"]["commit"],
        resumed["publication"]["commit"]
    );
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), published);
}

#[test]
fn old_exact_retry_recovers_original_commit_after_later_land_and_in_clean_clone() {
    let producer = tempfile::TempDir::new().unwrap();
    init_git_frontier(producer.path());
    write_receipt_with_artifact(
        producer.path(),
        "receipt-a.json",
        "first exact publication remains attributable after later frontier state",
        "w-a.json",
        br#"{"witness":"a"}"#,
    );
    let receipt_a_bytes = std::fs::read(producer.path().join("receipt-a.json")).unwrap();
    let first = run(
        producer.path(),
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&first, "land historical operation A");
    let first = one_json_object(&first);
    let commit_a = first["publication"]["commit"]
        .as_str()
        .expect("A publication commit")
        .to_string();
    std::fs::remove_file(producer.path().join("receipt-a.json")).unwrap();
    std::fs::remove_file(producer.path().join("artifacts/w-a.json")).unwrap();

    write_receipt_with_artifact(
        producer.path(),
        "receipt-b.json",
        "a later exact publication advances shared frontier projections",
        "w-b.json",
        br#"{"witness":"b"}"#,
    );
    let second = run(
        producer.path(),
        &["land", "receipt-b.json", "--as", "agent:t", "--json"],
    );
    assert_success(&second, "land later operation B");
    let second = one_json_object(&second);
    let commit_b = second["publication"]["commit"]
        .as_str()
        .unwrap_or_else(|| panic!("B publication commit: {second}"))
        .to_string();
    assert_ne!(commit_a, commit_b);
    std::fs::write(producer.path().join("receipt-a.json"), &receipt_a_bytes).unwrap();

    let retry = run(
        producer.path(),
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&retry, "retry historical A after B");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    assert_eq!(retry["publication"]["commit"], commit_a, "{retry}");
    assert_eq!(
        git_stdout(producer.path(), &["rev-parse", "HEAD"]).trim(),
        commit_b
    );

    let clone_parent = tempfile::TempDir::new().unwrap();
    let clone = clone_parent.path().join("clone");
    let clone_output = Command::new("git")
        .args(["clone", "-q", "--no-local"])
        .arg(producer.path())
        .arg(&clone)
        .output()
        .unwrap();
    assert_success(&clone_output, "clone journal-free frontier");
    assert!(!clone.join(".vela/operation-journals").exists());
    assert!(!clone.join(".git/vela/operation-journals").exists());
    std::fs::write(clone.join("receipt-a.json"), receipt_a_bytes).unwrap();
    let clone_before = snapshot_scientific_tree(&clone);

    let clone_retry = run(
        &clone,
        &["land", "receipt-a.json", "--as", "agent:t", "--json"],
    );
    assert_success(&clone_retry, "clean-clone retry of A");
    let clone_retry = one_json_object(&clone_retry);
    assert_eq!(clone_retry["route"], "exact_retry", "{clone_retry}");
    assert_eq!(clone_retry["publication"]["commit"], commit_a);
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(clone_retry[key], first[key], "clean clone changed {key}");
    }
    assert_eq!(snapshot_scientific_tree(&clone), clone_before);
    assert_eq!(git_stdout(&clone, &["rev-parse", "HEAD"]).trim(), commit_b);
}

#[test]
fn deny_has_zero_canonical_and_git_delta() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "receipt.json",
        "a signed deny route must leave every durable boundary unchanged",
    );
    write_active_deny_policy(tmp.path());

    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let index_before = git_stdout(tmp.path(), &["write-tree"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );

    let denied = run(
        tmp.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert!(!denied.status.success(), "Deny must not report success");
    let denied = one_json_object(&denied);
    assert_eq!(denied["ok"], false, "{denied}");
    assert!(
        denied["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("policy denies")),
        "{denied}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git_stdout(tmp.path(), &["write-tree"]), index_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
}

#[test]
fn external_public_artifact_requires_complete_descriptor() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "external.json",
        "an external public artifact needs enough metadata to remain inspectable offline",
    );
    let receipt_path = tmp.path().join("external.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let artifact = &mut receipt["artifacts"][0];
    artifact["path"] = serde_json::json!("https://example.invalid/result.bin");
    artifact["uri"] = serde_json::json!("https://example.invalid/result.bin");
    artifact.as_object_mut().unwrap().remove("size_bytes");
    artifact.as_object_mut().unwrap().remove("media_type");
    refresh_receipt_binding(&mut receipt);
    std::fs::write(
        &receipt_path,
        vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run(
        tmp.path(),
        &["land", "external.json", "--as", "agent:t", "--json"],
    );
    assert!(!output.status.success(), "incomplete descriptor must fail");
    let output = one_json_object(&output);
    assert!(
        output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("explicit size_bytes descriptor")),
        "{output}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn foreign_receipt_read_is_bounded_symlink_safe_and_zero_delta() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());

    let oversized = tmp.path().join("oversized-receipt.json");
    let file = std::fs::File::create(&oversized).unwrap();
    file.set_len(8 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let git_private_before = snapshot_git_private_state(tmp.path());
    let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

    let rejected = run(
        tmp.path(),
        &[
            "land",
            "oversized-receipt.json",
            "--as",
            "agent:t",
            "--json",
        ],
    );
    assert!(!rejected.status.success(), "oversized Receipt must fail");
    let rejected = one_json_object(&rejected);
    assert!(
        rejected["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("exceeds the 8388608-byte limit")),
        "{rejected}"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
    assert_eq!(
        std::fs::read(tmp.path().join(".git/index")).unwrap(),
        index_before
    );
    assert_eq!(snapshot_git_private_state(tmp.path()), git_private_before);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        write_receipt(
            tmp.path(),
            "real-receipt.json",
            "a foreign Receipt symlink must not cross the write edge",
        );
        symlink("real-receipt.json", tmp.path().join("linked-receipt.json")).unwrap();
        let scientific_before = snapshot_scientific_tree(tmp.path());
        let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
        let status_before = git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let git_private_before = snapshot_git_private_state(tmp.path());
        let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

        let rejected = run(
            tmp.path(),
            &["land", "linked-receipt.json", "--as", "agent:t", "--json"],
        );
        assert!(!rejected.status.success(), "symlinked Receipt must fail");
        let rejected = one_json_object(&rejected);
        assert!(
            rejected["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("symlink")),
            "{rejected}"
        );
        assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
        assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            git_stdout(
                tmp.path(),
                &["status", "--porcelain=v1", "--untracked-files=all"]
            ),
            status_before
        );
        assert_eq!(
            std::fs::read(tmp.path().join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(snapshot_git_private_state(tmp.path()), git_private_before);
    }
}

#[test]
fn local_artifact_reads_are_bounded_symlink_safe_and_zero_delta() {
    {
        let tmp = tempfile::TempDir::new().unwrap();
        init_git_frontier(tmp.path());
        write_receipt_with_artifact(
            tmp.path(),
            "oversized-artifact-receipt.json",
            "an oversized local artifact must not cross the write edge",
            "oversized.bin",
            b"seed",
        );
        let artifact = tmp.path().join("artifacts/oversized.bin");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&artifact)
            .unwrap();
        file.set_len(8 * 1024 * 1024 + 1).unwrap();
        drop(file);
        let scientific_before = snapshot_scientific_tree(tmp.path());
        let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
        let status_before = git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let git_private_before = snapshot_git_private_state(tmp.path());
        let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

        let rejected = run(
            tmp.path(),
            &[
                "land",
                "oversized-artifact-receipt.json",
                "--as",
                "agent:t",
                "--json",
            ],
        );
        assert!(!rejected.status.success(), "oversized artifact must fail");
        let rejected = one_json_object(&rejected);
        assert!(
            rejected["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("exceeds the 8388608-byte limit")),
            "{rejected}"
        );
        assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
        assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            git_stdout(
                tmp.path(),
                &["status", "--porcelain=v1", "--untracked-files=all"]
            ),
            status_before
        );
        assert_eq!(
            std::fs::read(tmp.path().join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(snapshot_git_private_state(tmp.path()), git_private_before);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        init_git_frontier(tmp.path());
        write_receipt_with_artifact(
            tmp.path(),
            "symlinked-artifact-receipt.json",
            "a symlinked artifact ancestor must not cross the write edge",
            "w.json",
            br#"{"witness":"inside"}"#,
        );
        std::fs::rename(
            tmp.path().join("artifacts"),
            tmp.path().join("real-artifacts"),
        )
        .unwrap();
        symlink("real-artifacts", tmp.path().join("artifacts")).unwrap();
        let scientific_before = snapshot_scientific_tree(tmp.path());
        let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
        let status_before = git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let git_private_before = snapshot_git_private_state(tmp.path());
        let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

        let rejected = run(
            tmp.path(),
            &[
                "land",
                "symlinked-artifact-receipt.json",
                "--as",
                "agent:t",
                "--json",
            ],
        );
        assert!(!rejected.status.success(), "symlinked artifact must fail");
        let rejected = one_json_object(&rejected);
        assert!(
            rejected["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("path traverses a symlink")),
            "{rejected}"
        );
        assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
        assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            git_stdout(
                tmp.path(),
                &["status", "--porcelain=v1", "--untracked-files=all"]
            ),
            status_before
        );
        assert_eq!(
            std::fs::read(tmp.path().join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(snapshot_git_private_state(tmp.path()), git_private_before);
    }
}

#[test]
fn flag_authored_artifact_read_is_bounded_and_preserves_work_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    let artifact = tmp.path().join("artifacts/oversized.bin");
    let file = std::fs::File::create(&artifact).unwrap();
    file.set_len(8 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let work = run_with_env(
        tmp.path(),
        &[
            "work",
            "erdos:bounded-artifact",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&work, "open bounded-artifact work session");
    let work = one_json_object(&work);
    let session_path = work_session_path(&work);
    let session_before = std::fs::read(&session_path).unwrap();
    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let git_private_before = snapshot_git_private_state(tmp.path());
    let index_before = std::fs::read(tmp.path().join(".git/index")).unwrap();

    let rejected = run_with_env(
        tmp.path(),
        &[
            "land",
            "--work",
            "erdos:bounded-artifact",
            "--claim",
            "an oversized flag-authored artifact must not cross the write edge",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/oversized.bin:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert!(!rejected.status.success(), "oversized artifact must fail");
    let rejected = one_json_object(&rejected);
    assert!(
        rejected["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("exceeds the 8388608-byte limit")),
        "{rejected}"
    );
    assert_eq!(std::fs::read(&session_path).unwrap(), session_before);
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
    assert_eq!(
        std::fs::read(tmp.path().join(".git/index")).unwrap(),
        index_before
    );
    assert_eq!(snapshot_git_private_state(tmp.path()), git_private_before);
}

#[test]
fn archive_artifact_is_retained_as_opaque_bytes_and_never_expanded() {
    use sha2::{Digest, Sha256};

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let sentinel = format!(
        "VELA_VALID_ARCHIVE_WAS_EXPANDED_{}_{}",
        std::process::id(),
        tmp.path().file_name().unwrap().to_string_lossy()
    );
    let archive = valid_ustar_archive(&format!("../{sentinel}"), b"expanded fixture bytes");
    assert_eq!(&archive[257..263], b"ustar\0");
    assert_eq!(archive.len() % 512, 0);
    assert!(archive.ends_with(&[0_u8; 1024]));
    write_receipt_with_artifact(
        tmp.path(),
        "archive-receipt.json",
        "archive-like evidence remains opaque until an explicit verifier opens it",
        "bundle.tar",
        &archive,
    );

    let landed = run(
        tmp.path(),
        &["land", "archive-receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&landed, "land opaque archive artifact");
    let landed = one_json_object(&landed);
    let digest = hex::encode(Sha256::digest(&archive));
    assert_eq!(
        std::fs::read(
            tmp.path()
                .join(format!("records/artifacts/sha256/{digest}"))
        )
        .unwrap(),
        archive
    );
    let sentinel_paths = [
        tmp.path().join("artifacts").join(&sentinel),
        tmp.path().join(&sentinel),
        tmp.path().parent().unwrap().join(&sentinel),
    ];
    for path in &sentinel_paths {
        assert!(
            !path.exists(),
            "archive traversal created {}",
            path.display()
        );
    }

    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let proposal_id = landed["proposal_id"].as_str().unwrap();
    let frontier = tmp.path().to_str().unwrap();
    let preview = run(tmp.path(), &["review", "preview", frontier, proposal_id]);
    assert_success(&preview, "preview opaque archive proposal");
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    for path in &sentinel_paths {
        assert!(!path.exists(), "review created {}", path.display());
    }
}

#[test]
fn same_claim_new_evidence_is_not_an_exact_retry() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let claim = "the same scoped claim has an independent evidence submission";
    write_receipt_with_artifact_as(
        tmp.path(),
        "receipt-a.json",
        claim,
        "w-a.json",
        br#"{"witness":"first"}"#,
        "agent:replicator-a",
        0x31,
    );
    let first = run(
        tmp.path(),
        &[
            "land",
            "receipt-a.json",
            "--as",
            "agent:replicator-a",
            "--json",
        ],
    );
    assert_success(&first, "first evidence landing");
    let first = one_json_object(&first);

    write_receipt_with_artifact_as(
        tmp.path(),
        "receipt-b.json",
        claim,
        "w-b.json",
        br#"{"witness":"second"}"#,
        "agent:replicator-b",
        0x32,
    );
    let second = run(
        tmp.path(),
        &[
            "land",
            "receipt-b.json",
            "--as",
            "agent:replicator-b",
            "--json",
        ],
    );
    assert_success(&second, "second evidence landing");
    let second = one_json_object(&second);

    assert_ne!(second["route"], "exact_retry", "{second}");
    for key in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_ne!(
            second[key], first[key],
            "different evidence collapsed {key}: first={first}, second={second}"
        );
    }
    let first_proposal_id = first["proposal_id"].as_str().unwrap();
    let first_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            tmp.path()
                .join(".vela/proposals")
                .join(format!("{first_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(first_proposal["actor"]["id"], "agent:replicator-a");

    let second_proposal_id = second["proposal_id"].as_str().unwrap();
    let second_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            tmp.path()
                .join(".vela/proposals")
                .join(format!("{second_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second_proposal["actor"]["id"], "agent:replicator-b");
    let related = second_proposal["payload"]["vela_submission"]["same_claim_findings"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("second proposal must retain the same-claim relation: {second_proposal}")
        });
    assert_eq!(
        related,
        &[serde_json::Value::String(
            first["finding_id"].as_str().unwrap().to_string()
        )],
        "second evidence did not point back to the related first finding: {second_proposal}"
    );
}

#[test]
fn clean_clone_rebuilds_public_review_root_without_restricted_bytes() {
    use sha2::{Digest, Sha256};

    let producer = tempfile::TempDir::new().unwrap();
    init_git_frontier(producer.path());
    write_receipt_with_artifact(
        producer.path(),
        "privacy-receipt.json",
        "a mixed-disclosure receipt keeps its public review packet portable",
        "public-witness.json",
        br#"{"public":true}"#,
    );
    let restricted_bytes = b"VELA-RESTRICTED-OPENING-7f75f37b";
    let restricted_path = producer.path().join(".vela/work/privacy/secret.bin");
    std::fs::create_dir_all(restricted_path.parent().unwrap()).unwrap();
    std::fs::write(&restricted_path, restricted_bytes).unwrap();

    let receipt_path = producer.path().join("privacy-receipt.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    receipt["artifacts"].as_array_mut().unwrap().extend([
        serde_json::json!({
            "path": "https://example.invalid/frozen-dataset.bin",
            "uri": "https://example.invalid/frozen-dataset.bin",
            "kind": "dataset",
            "sha256": "9".repeat(64),
            "size_bytes": 128,
            "media_type": "application/octet-stream",
            "locator_integrity": "immutable",
            "availability": "unknown"
        }),
        serde_json::json!({
            "path": "opaque:custodian-fixture-7",
            "kind": "restricted_witness",
            "disclosure": "restricted",
            "locator_integrity": "unknown",
            "availability": "available"
        }),
    ]);
    receipt["x:portable-review"] = serde_json::json!({
        "distiller": "outside-producer-fixture",
        "belief": {"value": 0.61, "attributed_to": "agent:t"}
    });
    receipt["machine"]["subject"] = serde_json::Value::Array(
        receipt["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|artifact| {
                let mut subject = serde_json::Map::new();
                subject.insert("name".to_string(), artifact["path"].clone());
                if artifact
                    .get("disclosure")
                    .and_then(serde_json::Value::as_str)
                    != Some("restricted")
                {
                    if let Some(digest) = artifact.get("sha256") {
                        subject.insert("digest".to_string(), serde_json::json!({"sha256": digest}));
                    }
                    if let Some(uri) = artifact.get("uri") {
                        subject.insert("uri".to_string(), uri.clone());
                    }
                }
                serde_json::Value::Object(subject)
            })
            .collect(),
    );
    refresh_receipt_binding(&mut receipt);

    let restricted_digest = hex::encode(Sha256::digest(restricted_bytes));
    let restricted_location = restricted_path.to_string_lossy().into_owned();
    let mut descriptor_leak = receipt.clone();
    descriptor_leak["artifacts"][2]["sha256"] =
        serde_json::Value::String(restricted_digest.clone());
    refresh_receipt_binding(&mut descriptor_leak);
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-descriptor.receipt.json",
        &descriptor_leak,
        "restricted descriptor digest leak",
    );

    let mut mirror_leak = receipt.clone();
    mirror_leak["machine"]["subject"][2]["digest"] =
        serde_json::json!({"sha256": restricted_digest});
    mirror_leak["machine"]["subject"][2]["uri"] =
        serde_json::Value::String(restricted_location.clone());
    refresh_receipt_binding(&mut mirror_leak);
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-subject.receipt.json",
        &mirror_leak,
        "restricted subject mirror leak",
    );

    let mut prov_leak = receipt.clone();
    prov_leak["attestation"]["prov"] = serde_json::json!({
        "entity": {
            "artifact:opaque:custodian-fixture-7": {
                "prov:type": "vela:artifact",
                "vela:kind": "restricted_witness",
                "opening": "VELA-RESTRICTED-OPENING-7f75f37b",
                "location": restricted_location,
            }
        }
    });
    assert_land_rejected_without_git_change(
        producer.path(),
        "malicious-prov.receipt.json",
        &prov_leak,
        "restricted PROV mirror leak",
    );

    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    std::fs::write(&receipt_path, receipt.canonical_bytes().unwrap()).unwrap();

    let landed = run(
        producer.path(),
        &["land", "privacy-receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&landed, "mixed-disclosure landing");
    let landed = one_json_object(&landed);
    assert_eq!(landed["route"], "deferred", "{landed}");
    assert_eq!(
        landed["publication"]["state"], "committed_local",
        "{landed}"
    );

    let record_id = landed["record_id"].as_str().unwrap();
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join("records")
                .join(format!("{record_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let artifacts = record["artifacts"].as_array().unwrap();
    assert!(artifacts.iter().any(|artifact| {
        artifact["disclosure"] == "restricted"
            && artifact["locator"] == "opaque:custodian-fixture-7"
            && artifact.get("sha256").is_none()
    }));
    assert!(artifacts.iter().any(|artifact| {
        artifact["kind"] == "dataset"
            && artifact["locator_integrity"] == "immutable"
            && artifact.get("availability").is_none()
    }));
    let proposal_id = landed["proposal_id"].as_str().unwrap();
    let proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join(".vela/proposals")
                .join(format!("{proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let review_material_path = proposal["payload"]["vela_submission"]["review_material_path"]
        .as_str()
        .expect("proposal must retain its public review material path");
    let review_material_bytes = std::fs::read(producer.path().join(review_material_path)).unwrap();
    let review_material: serde_json::Value =
        serde_json::from_slice(&review_material_bytes).unwrap();
    assert_eq!(review_material["proposal_id"], proposal_id);
    assert_eq!(review_material["receipt_root"], landed["receipt_root"]);
    assert_eq!(review_material["route"]["policy_state"], "absent");
    assert_eq!(review_material["route"]["permit_readiness"], "human_only");
    assert_eq!(
        review_material["route"]["reason_codes"],
        serde_json::json!(["policy_absent"])
    );
    assert!(review_material["route"]["policy_decision"].is_null());
    assert_eq!(review_material["route"]["engine_gate"]["strict"], true);
    let review_material_root_before =
        vela_protocol::canonical::sha256_canonical(&review_material).unwrap();
    let review_root_before = vela_protocol::canonical::sha256_canonical(&serde_json::json!({
        "receipt_root": landed["receipt_root"],
        "artifacts": artifacts,
    }))
    .unwrap();

    // A genuinely independent producer may submit the same claim with new
    // evidence. Both public receipt/review roots must survive the clone; this
    // is replication, never exact-retry deduplication.
    std::fs::remove_file(producer.path().join("artifacts/public-witness.json")).unwrap();
    let replicated_claim = "a mixed-disclosure receipt keeps its public review packet portable";
    write_receipt_with_artifact_as(
        producer.path(),
        "replication-receipt.json",
        replicated_claim,
        "replication-witness.json",
        br#"{"public":"independent replication"}"#,
        "agent:replicator-b",
        0x52,
    );
    let replicated = run(
        producer.path(),
        &[
            "land",
            "replication-receipt.json",
            "--as",
            "agent:replicator-b",
            "--json",
        ],
    );
    assert_success(&replicated, "independent replication landing");
    let replicated = one_json_object(&replicated);
    assert_ne!(replicated["route"], "exact_retry", "{replicated}");
    assert_ne!(replicated["receipt_root"], landed["receipt_root"]);
    assert_ne!(replicated["proposal_id"], landed["proposal_id"]);
    let replicated_proposal_id = replicated["proposal_id"].as_str().unwrap();
    let replicated_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            producer
                .path()
                .join(".vela/proposals")
                .join(format!("{replicated_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(replicated_proposal["actor"]["id"], "agent:replicator-b");
    assert_eq!(
        replicated_proposal["payload"]["vela_submission"]["same_claim_findings"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    let replicated_review_path =
        replicated_proposal["payload"]["vela_submission"]["review_material_path"]
            .as_str()
            .unwrap()
            .to_string();
    let replicated_review_root = vela_protocol::canonical::sha256_canonical(
        &serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(producer.path().join(&replicated_review_path)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let clone_parent = tempfile::TempDir::new().unwrap();
    let clone = clone_parent.path().join("clone");
    let clone_output = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--no-local",
            producer.path().to_str().unwrap(),
            clone.to_str().unwrap(),
        ])
        .env("HOME", clone_parent.path())
        .output()
        .unwrap();
    assert_success(&clone_output, "clean clone");
    std::fs::remove_dir_all(producer.path()).unwrap();

    let receipt_hex = landed["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let cloned_receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &std::fs::read(
            clone
                .join("records/receipts/sha256")
                .join(format!("{receipt_hex}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        cloned_receipt.canonical_root().unwrap(),
        landed["receipt_root"].as_str().unwrap()
    );
    let cloned_record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(clone.join("records").join(format!("{record_id}.json"))).unwrap(),
    )
    .unwrap();
    let cloned_review_material: serde_json::Value =
        serde_json::from_slice(&std::fs::read(clone.join(review_material_path)).unwrap()).unwrap();
    assert_eq!(
        vela_protocol::canonical::sha256_canonical(&cloned_review_material).unwrap(),
        review_material_root_before,
        "clean clone changed the staged decision and gate facts"
    );
    let review_root_after = vela_protocol::canonical::sha256_canonical(&serde_json::json!({
        "receipt_root": cloned_receipt.canonical_root().unwrap(),
        "artifacts": cloned_record["artifacts"],
    }))
    .unwrap();
    assert_eq!(review_root_after, review_root_before);
    let replicated_hex = replicated["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let replicated_receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &std::fs::read(
            clone
                .join("records/receipts/sha256")
                .join(format!("{replicated_hex}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        replicated_receipt.canonical_root().unwrap(),
        replicated["receipt_root"].as_str().unwrap()
    );
    let cloned_replicated_proposal: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            clone
                .join(".vela/proposals")
                .join(format!("{replicated_proposal_id}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        cloned_replicated_proposal["actor"]["id"],
        "agent:replicator-b"
    );
    let cloned_replicated_review: serde_json::Value =
        serde_json::from_slice(&std::fs::read(clone.join(&replicated_review_path)).unwrap())
            .unwrap();
    assert_eq!(
        vela_protocol::canonical::sha256_canonical(&cloned_replicated_review).unwrap(),
        replicated_review_root
    );

    let objects = Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args([
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname) %(objecttype)",
        ])
        .output()
        .unwrap();
    assert_success(&objects, "enumerate clone objects");
    let forbidden_markers = [
        restricted_bytes.as_slice(),
        restricted_digest.as_bytes(),
        restricted_location.as_bytes(),
        b".vela/work/privacy/secret.bin".as_slice(),
    ];
    for line in String::from_utf8(objects.stdout).unwrap().lines() {
        let mut fields = line.split_whitespace();
        let Some(oid) = fields.next() else { continue };
        if fields.next() != Some("blob") {
            continue;
        }
        let blob = Command::new("git")
            .arg("-C")
            .arg(&clone)
            .args(["cat-file", "blob", oid])
            .output()
            .unwrap();
        assert_success(&blob, "read clone blob");
        for marker in forbidden_markers {
            assert!(
                !blob
                    .stdout
                    .windows(marker.len())
                    .any(|window| window == marker),
                "restricted bytes, digest, opening, or location entered Git object {oid}"
            );
        }
    }
}

#[test]
fn isolated_training_frontier_lands_pending_and_reproduces_from_a_clean_clone() {
    let tmp = tempfile::TempDir::new().unwrap();
    let producer = tmp.path().join("producer");
    let clone = tmp.path().join("clone");
    std::fs::create_dir(&producer).unwrap();
    init_git_frontier(&producer);

    // The ordinary contribution path must not depend on a human signing key.
    // Keep only the explicit fixture agent key used to sign coordination and
    // producer provenance below.
    let human_key = producer.join(".vela/keys/t/private.key");
    std::fs::remove_file(&human_key).unwrap();
    assert!(!human_key.exists());

    std::fs::write(
        producer.join("campaign.yaml"),
        r#"
batches:
  - name: bounded training contribution
    state: open
    problems:
      - id: seed:training-golomb
        title: Reproduce a bounded Golomb witness
        why: Exercise the real pending-contribution path without authority
"#,
    )
    .unwrap();
    std::fs::create_dir_all(producer.join("witnesses")).unwrap();
    std::fs::write(
        producer.join("witnesses/training-golomb.witness.json"),
        br#"{"kind":"golomb","length":6,"marks":[0,1,4,6]}"#,
    )
    .unwrap();
    let legacy_blob = ".vela/artifact-blobs/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    std::fs::create_dir_all(producer.join(".vela/artifact-blobs/sha256")).unwrap();
    std::fs::write(producer.join(legacy_blob), b"legacy public evidence\n").unwrap();
    assert_success(
        &git(&producer, &["add", "campaign.yaml", "witnesses"]),
        "stage training frontier",
    );
    assert_success(
        &git(&producer, &["add", "-f", "--", legacy_blob]),
        "stage legacy artifact layout",
    );
    assert_success(
        &git(&producer, &["commit", "-qm", "add bounded training target"]),
        "commit training frontier",
    );

    let next = run(&producer, &["next", ".", "--json"]);
    assert_success(&next, "rank training target");
    let next = one_json_object(&next);
    let offered = next["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["target_id"] == "seed:training-golomb")
        .expect("training target appears in the real next offer");
    assert_eq!(offered["next_command"], "vela work seed:training-golomb");

    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let work = run_with_env(
        &producer,
        &[
            "work",
            "seed:training-golomb",
            "--as",
            "agent:training-fixture",
            "--json",
        ],
        &env,
    );
    assert_success(&work, "open training work session");
    let work = one_json_object(&work);
    assert_eq!(work["target_id"], "seed:training-golomb", "{work}");
    assert_eq!(
        git_stdout(
            &producer,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "the real work step did not publish a clean exact lease"
    );
    assert_eq!(
        git_stdout(&producer, &["show", &format!("HEAD:{legacy_blob}")]),
        "legacy public evidence\n",
        "work publication dropped pre-0.9 artifact evidence"
    );
    let session_path = work_session_path(&work);
    assert!(session_path.is_file());

    let reproduced = run(&producer, &["reproduce", ".", "--json"]);
    assert_success(&reproduced, "verify training witness before landing");
    let reproduced = one_json_object(&reproduced);
    assert_eq!(reproduced["passed"], 1, "{reproduced}");
    assert_eq!(reproduced["failed"], 0, "{reproduced}");

    let before_land = vela_protocol::repo::load_from_path(&producer).unwrap();
    let accepted_root_before = vela_protocol::events::event_log_hash(&before_land.events);
    let land = run_with_env(
        &producer,
        &[
            "land",
            "--work",
            "seed:training-golomb",
            "--claim",
            "the frozen verifier confirms the bounded training Golomb witness",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "witnesses/training-golomb.witness.json:witness",
            "--caveat",
            "training evidence remains pending human review",
            "--predicted-observable",
            "The frozen verifier reports six distinct pairwise differences.",
            "--performed-test",
            "Ran vela reproduce against the committed witness.",
            "--result",
            "The frozen Golomb verifier passed.",
            "--evidence",
            "witnesses/training-golomb.witness.json",
            "--as",
            "agent:training-fixture",
            "--json",
        ],
        &env,
    );
    assert_success(&land, "land training receipt");
    let land = one_json_object(&land);
    assert_eq!(land["route"], "deferred", "{land}");
    assert_eq!(land["accepted_event_delta"], 0, "{land}");
    assert_eq!(
        land["publication"]["state"], "committed_local",
        "training landing was not committed for portable review: {land}"
    );
    assert_eq!(
        land["accepted_event_count_before"], land["accepted_event_count_after"],
        "pending training evidence changed accepted state: {land}"
    );
    assert!(
        !session_path.exists(),
        "successful landing kept its private session"
    );

    let after_land = vela_protocol::repo::load_from_path(&producer).unwrap();
    assert_eq!(
        vela_protocol::events::event_log_hash(&after_land.events),
        accepted_root_before,
        "Deferred landing changed the accepted event root"
    );
    let proposal_id = land["proposal_id"].as_str().unwrap();
    assert!(
        producer
            .join(".vela/proposals")
            .join(format!("{proposal_id}.json"))
            .is_file(),
        "Deferred route did not retain its pending proposal"
    );
    let proposal_path = format!(".vela/proposals/{proposal_id}.json");
    let published_tree = git_stdout(&producer, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        published_tree.lines().any(|path| path == proposal_path),
        "pending proposal was not included in the landing commit:\n{published_tree}"
    );

    let cloned = Command::new("git")
        .args(["clone", "-q", "--no-local"])
        .arg(&producer)
        .arg(&clone)
        .output()
        .unwrap();
    assert_success(&cloned, "clone landed training frontier");
    assert!(
        !clone.join(".vela/keys").exists(),
        "a private key entered the portable frontier"
    );
    assert!(
        clone
            .join(".vela/proposals")
            .join(format!("{proposal_id}.json"))
            .is_file(),
        "clean clone lost the pending proposal; clone tree:\n{}",
        git_stdout(&clone, &["ls-tree", "-r", "--name-only", "HEAD"])
    );

    let review_show = run(&clone, &["review", "show", ".", proposal_id, "--json"]);
    assert_success(&review_show, "show reproducible pending proposal");
    let review_show = one_json_object(&review_show);
    assert!(
        review_show["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["kind"] == "reproduce_pending_artifact"),
        "a pending proposal with a retained witness must advertise scoped reproduction: {review_show}"
    );
    let proposal_reproduced = run(
        &clone,
        &["reproduce", ".", "--proposal", proposal_id, "--json"],
    );
    assert_success(
        &proposal_reproduced,
        "reproduce only the pending proposal witness",
    );
    let proposal_reproduced = one_json_object(&proposal_reproduced);
    assert_eq!(proposal_reproduced["scope"], "pending_proposal");
    assert_eq!(proposal_reproduced["proposal_id"], proposal_id);
    assert_eq!(proposal_reproduced["passed"], 1, "{proposal_reproduced}");
    assert_eq!(proposal_reproduced["failed"], 0, "{proposal_reproduced}");

    let strict = run(&clone, &["check", ".", "--strict", "--json"]);
    assert_success(&strict, "strict replay in clean clone");
    let cloned_frontier = vela_protocol::repo::load_from_path(&clone).unwrap();
    assert_eq!(
        vela_protocol::events::event_log_hash(&cloned_frontier.events),
        accepted_root_before,
        "clean clone replay changed the accepted root"
    );
    let reproduced = run(&clone, &["reproduce", ".", "--json"]);
    assert_success(&reproduced, "reproduce training witness in clean clone");
    let reproduced = one_json_object(&reproduced);
    assert_eq!(reproduced["passed"], 1, "{reproduced}");
    assert_eq!(reproduced["failed"], 0, "{reproduced}");
    assert_eq!(
        git_stdout(
            &clone,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "strict replay or reproduction dirtied the clean clone"
    );
}

#[test]
fn rich_campaign_target_is_consistent_across_next_and_prelease_work() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::write(
        tmp.path().join("campaign.yaml"),
        r#"
batches:
  - name: external reproduction
    state: open
    problems:
      - 443
      - id: seed:prepared-target
        title: Reproduce the prepared declaration
        why: Prepared external verifier exercise
        task:
          kind: external_lean_reproduction
          source:
            repo_url: https://github.com/example/prepared
            commit: 0123456789abcdef0123456789abcdef01234567
            declaration: prepared-target
            source_path: Prepared.lean
          verifier:
            command: vela reproduce-external
          fixed_base:
            frontier_id: campaign-must-not-pin-state
            event_log_root: sha256:campaign-must-not-pin-state
          constraints:
            - Use the exact pinned source.
          allowed_actions:
            - sign with a human key
          authority_ceiling: campaign may accept results
"#,
    )
    .unwrap();
    let before = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let expected_base = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&before.events)
    );
    let expected_git_commit = git_stdout(tmp.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_string();

    let next = run(tmp.path(), &["next", ".", "--limit", "100", "--json"]);
    assert_success(&next, "list rich campaign target");
    let next = one_json_object(&next);
    let targets = next["targets"].as_array().unwrap();
    assert!(
        targets
            .iter()
            .any(|target| target["target_id"] == "seed:443"),
        "legacy scalar seed changed shape: {next}"
    );
    let prepared = targets
        .iter()
        .find(|target| target["target_id"] == "seed:prepared-target")
        .expect("rich target keeps its explicit id");
    assert_eq!(prepared["next_command"], "vela work seed:prepared-target");
    assert!(prepared["objective"].as_str().is_some(), "{prepared}");
    assert!(
        prepared.get("task").is_none(),
        "compact offer leaked task body: {prepared}"
    );

    let agent_key = "42".repeat(32);
    let work = run_with_env(
        tmp.path(),
        &["work", "seed:prepared-target", "--as", "agent:t", "--json"],
        &[("VELA_AGENT_KEY_HEX", agent_key.as_str())],
    );
    assert_success(&work, "open rich campaign work session");
    let work = one_json_object(&work);
    assert_eq!(work["starting_roots"]["event_log"], expected_base);
    assert_eq!(work["starting_roots"]["git_commit"], expected_git_commit);
    let session = load_work_session(&work);
    assert_eq!(session["base_event_log_root"], expected_base);
    assert_eq!(session["source_git_commit_oid"], expected_git_commit);
    assert_eq!(
        session["task_contract"]["authority_ceiling"],
        "Producer evidence only. The session can create a receipt and proposal; it cannot create human acceptance."
    );
    assert!(
        session["task_contract"]["forbidden_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item
                .as_str()
                .is_some_and(|text| text.contains("human signing key"))),
        "restrictive Vela task contract was not retained: {work}"
    );
    let after = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(work["target_id"], "seed:prepared-target");
    assert_ne!(
        format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&after.events)
        ),
        expected_base,
        "lease fixture did not add its coordination event"
    );

    let checked = run(tmp.path(), &["check", ".", "--strict", "--json"]);
    assert_success(
        &checked,
        "immediate strict check after the published work claim",
    );
    let checked = one_json_object(&checked);
    assert_eq!(checked["ok"], true, "{checked}");
    assert_eq!(
        checked["state_integrity"]["structural_errors"],
        serde_json::json!([]),
        "work left frontier.json or vela.lock at the pre-claim snapshot: {checked}"
    );
}

#[test]
fn repeated_work_claim_returns_the_exact_active_session_without_a_second_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let args = [
        "work",
        "erdos:retry-safe",
        "--as",
        "agent:retry-safe",
        "--json",
    ];

    let first = run_with_env(tmp.path(), &args, &env);
    assert_success(&first, "open first retry-safe session");
    let first = one_json_object(&first);
    assert_eq!(first["idempotent"], false, "{first}");
    let session_path = work_session_path(&first);
    let session_bytes = std::fs::read(&session_path).unwrap();
    let after_first = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let event_root = vela_protocol::events::event_log_hash(&after_first.events);
    let event_count = after_first.events.len();
    let head = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let retry = run_with_env(tmp.path(), &args, &env);
    assert_success(&retry, "retry exact active work session");
    let retry = one_json_object(&retry);
    assert_eq!(retry["idempotent"], true, "{retry}");
    assert_eq!(retry["session"]["id"], first["session"]["id"]);
    assert_eq!(retry["session"]["path"], first["session"]["path"]);
    assert_eq!(std::fs::read(&session_path).unwrap(), session_bytes);

    let after_retry = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(after_retry.events.len(), event_count);
    assert_eq!(
        vela_protocol::events::event_log_hash(&after_retry.events),
        event_root
    );
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        ""
    );
}

#[test]
fn invalid_campaign_bytes_targets_and_duplicates_fail_before_lease() {
    let cases = [
        (
            "unsafe-id",
            b"batches:\n  - problems:\n      - foo;id\n".to_vec(),
            "invalid campaign target",
        ),
        (
            "duplicate-id",
            b"batches:\n  - problems:\n      - 443\n      - id: seed:443\n".to_vec(),
            "duplicate resolved campaign target",
        ),
        (
            "oversized-file",
            vec![b' '; 1024 * 1024 + 1],
            "exceeds the 1048576-byte limit",
        ),
    ];
    for (label, campaign, expected_error) in cases {
        let tmp = tempfile::TempDir::new().unwrap();
        init_git_frontier(tmp.path());
        std::fs::write(tmp.path().join("campaign.yaml"), campaign).unwrap();
        let before = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
        let agent_key = "42".repeat(32);
        let work = run_with_env(
            tmp.path(),
            &["work", "seed:probe", "--as", "agent:t", "--json"],
            &[("VELA_AGENT_KEY_HEX", agent_key.as_str())],
        );
        assert!(
            !work.status.success(),
            "{label} unexpectedly claimed work: {}",
            String::from_utf8_lossy(&work.stdout)
        );
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&work.stdout),
            String::from_utf8_lossy(&work.stderr)
        );
        assert!(output.contains(expected_error), "{label}: {output}");
        let after = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
        assert_eq!(after.attempt_claims, before.attempt_claims, "{label}");
        assert_eq!(
            vela_protocol::events::event_log_hash(&after.events),
            vela_protocol::events::event_log_hash(&before.events),
            "{label} changed the event log"
        );
    }
}

#[test]
fn flag_authored_land_closes_only_its_exact_private_work_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    std::fs::write(
        tmp.path().join("artifacts/session-witness.json"),
        br#"{"session":true}"#,
    )
    .unwrap();
    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];

    let work = run_with_env(
        tmp.path(),
        &["work", "erdos:session-close", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&work, "open work session");
    let work = one_json_object(&work);
    assert_eq!(work["target_id"], "erdos:session-close", "{work}");
    let session_record = work_session_path(&work);
    let session_dir = session_record.parent().unwrap();
    assert!(session_record.is_file());
    let session = load_work_session(&work);
    assert_eq!(session["schema"], "vela.work-session.internal.v1");
    assert_eq!(session["target"], "erdos:session-close");
    assert_eq!(session["actor"], "agent:t");
    assert!(
        work["session"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("vws_") && id.len() == 68)
    );
    let task_contract_root = work["starting_roots"]["task_contract"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(task_contract_root.starts_with("sha256:"));
    assert_eq!(
        session_dir.file_name().unwrap().to_string_lossy().len(),
        "erdos-session-close".len() + 2 + 64,
        "session directory must retain a collision-safe full target digest"
    );
    std::fs::write(
        session_dir.join("producer-notes.txt"),
        "keep this scratch\n",
    )
    .unwrap();

    let land = run_with_env(
        tmp.path(),
        &[
            "land",
            "--work",
            "erdos:session-close",
            "--claim",
            "the exact work-session receipt closes its private session",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/session-witness.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&land, "land flag-authored work-session receipt");
    let land = one_json_object(&land);
    assert!(
        !session_record.exists(),
        "the typed session must close only after the landing installs"
    );
    assert_eq!(
        std::fs::read_to_string(session_dir.join("producer-notes.txt")).unwrap(),
        "keep this scratch\n",
        "landing must preserve unrelated producer scratch"
    );
    let receipt_hex = land["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            tmp.path()
                .join("records/receipts/sha256")
                .join(format!("{receipt_hex}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        receipt["environment"]["vela:producer_context"]["task_contract_root"], task_contract_root,
        "portable receipt provenance must bind the private task contract"
    );

    let committed_paths = git_stdout(
        tmp.path(),
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(
        !committed_paths
            .lines()
            .any(|path| path.starts_with(".vela/work/")),
        "private work coordination entered the Git publication: {committed_paths}"
    );
}

#[test]
fn flag_authoring_and_file_input_share_canonical_receipt_bytes() {
    const CLAIM: &str = "flag and file inputs have one canonical Receipt v1";
    const CAVEAT: &str = "fixture evidence only";
    const PREDICTION: &str = "The exact replay emits the same witness checksum.";
    const PERFORMED_TEST: &str = "Re-ran the frozen fixture verifier.";
    const RESULT: &str = "The verifier passed with the expected checksum.";
    const PACKET_ROOT: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const PROFILE_ROOT: &str =
        "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const CAPSULE_ROOT: &str =
        "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    const RESULT_CONTRACT_ROOT: &str =
        "sha256:4444444444444444444444444444444444444444444444444444444444444444";

    let tmp = tempfile::TempDir::new().unwrap();
    let base = tmp.path().join("base");
    std::fs::create_dir(&base).unwrap();
    init_git_frontier(&base);
    std::fs::create_dir_all(base.join("artifacts")).unwrap();
    let artifact_bytes = br#"{"input_parity":true}"#;
    std::fs::write(base.join("artifacts/input-parity.json"), artifact_bytes).unwrap();
    assert_success(&git(&base, &["add", "-A"]), "stage parity artifact");
    assert_success(
        &git(&base, &["commit", "-qm", "add parity artifact"]),
        "commit parity artifact",
    );

    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let opened = run_with_env(
        &base,
        &["work", "erdos:input-parity", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&opened, "open parity work session");
    let opened = one_json_object(&opened);
    assert_eq!(
        git_stdout(
            &base,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "work must publish its exact lease before a portable landing"
    );
    let session = work_session_path(&opened);
    let relative_session = session
        .strip_prefix(base.canonicalize().unwrap())
        .unwrap()
        .to_path_buf();
    assert_eq!(
        git_stdout(
            &base,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "published work claim left public frontier dirt"
    );

    let flags_frontier = tmp.path().join("flags");
    let file_frontier = tmp.path().join("file");
    for destination in [&flags_frontier, &file_frontier] {
        let cloned = Command::new("git")
            .args(["clone", "-q", "--no-local"])
            .arg(&base)
            .arg(destination)
            .output()
            .unwrap();
        assert_success(&cloned, "clone common parity preimage");
        let destination_session = destination.join(&relative_session);
        std::fs::create_dir_all(destination_session.parent().unwrap()).unwrap();
        std::fs::copy(&session, destination_session).unwrap();
    }

    let from_flags = run_with_env(
        &flags_frontier,
        &[
            "land",
            "--work",
            "erdos:input-parity",
            "--claim",
            CLAIM,
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/input-parity.json:witness",
            "--caveat",
            CAVEAT,
            "--predicted-observable",
            PREDICTION,
            "--performed-test",
            PERFORMED_TEST,
            "--result",
            RESULT,
            "--evidence",
            "artifacts/input-parity.json",
            "--counterevidence",
            "records/attempts/prior-mismatch.json",
            "--packet-root",
            PACKET_ROOT,
            "--profile-root",
            PROFILE_ROOT,
            "--verifier-capsule-root",
            CAPSULE_ROOT,
            "--result-contract-root",
            RESULT_CONTRACT_ROOT,
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert_success(&from_flags, "land flag-authored receipt");
    let from_flags = one_json_object(&from_flags);
    let receipt_hex = from_flags["receipt_root"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let receipt_bytes = std::fs::read(
        flags_frontier
            .join("records/receipts/sha256")
            .join(format!("{receipt_hex}.json")),
    )
    .unwrap();
    let authored: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
    assert_eq!(
        authored["environment"]["vela:scientific_chain"],
        serde_json::json!({
            "schema": "vela.scientific-chain.producer.v1",
            "authority": "producer",
            "predicted_observable": PREDICTION,
            "not_applicable": false,
            "performed_test": PERFORMED_TEST,
            "result": RESULT,
            "evidence": ["artifacts/input-parity.json"],
            "counterevidence": ["records/attempts/prior-mismatch.json"],
        })
    );
    assert_eq!(
        authored["environment"]["vela:execution_binding"],
        serde_json::json!({
            "schema": "vela.execution-binding.v1",
            "packet_root": PACKET_ROOT,
            "profile_root": PROFILE_ROOT,
            "verifier_capsule_root": CAPSULE_ROOT,
            "result_contract_root": RESULT_CONTRACT_ROOT,
        })
    );

    std::fs::write(flags_frontier.join("portable-receipt.json"), &receipt_bytes).unwrap();
    let retry = run_with_env(
        &flags_frontier,
        &["land", "portable-receipt.json", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&retry, "retry flag-authored Receipt through file input");
    let retry = one_json_object(&retry);
    assert_eq!(retry["route"], "exact_retry", "{retry}");
    for field in [
        "operation_id",
        "receipt_root",
        "record_id",
        "proposal_id",
        "finding_id",
    ] {
        assert_eq!(
            retry[field], from_flags[field],
            "byte-identical file retry changed {field}"
        );
    }
    assert_eq!(
        retry["publication"]["commit"], from_flags["publication"]["commit"],
        "byte-identical file retry changed the publication commit"
    );

    std::fs::write(file_frontier.join("portable-receipt.json"), &receipt_bytes).unwrap();
    let from_file = run_with_env(
        &file_frontier,
        &["land", "portable-receipt.json", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&from_file, "land imported Receipt v1");
    let from_file = one_json_object(&from_file);

    assert_eq!(
        from_file["receipt_root"], from_flags["receipt_root"],
        "file input changed the canonical Receipt v1 root"
    );
    assert_eq!(
        from_file["route"], from_flags["route"],
        "both fresh landings should reach the same no-policy route"
    );
    assert!(
        !flags_frontier.join(&relative_session).exists()
            && !file_frontier.join(&relative_session).exists(),
        "both surfaces must close the same receipt-bound private session"
    );
    assert_eq!(
        std::fs::read(
            file_frontier
                .join("records/receipts/sha256")
                .join(format!("{receipt_hex}.json")),
        )
        .unwrap(),
        receipt_bytes,
        "file import changed the canonical Receipt v1 bytes"
    );

    // The flag surface authors the complete Receipt; the file surface imports
    // those canonical bytes without reinterpretation. Record, proposal, and
    // publication identities intentionally include the fresh landing time, so
    // independent landings need not share those downstream IDs.
}

#[test]
fn receipt_with_a_different_key_cannot_close_an_agents_work_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let lease_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", lease_key.as_str())];
    let work = run_with_env(
        tmp.path(),
        &["work", "erdos:session-owner", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&work, "open protected work session");
    let work = one_json_object(&work);
    let session_record = work_session_path(&work);
    let session_relative = session_record
        .parent()
        .unwrap()
        .strip_prefix(tmp.path().canonicalize().unwrap())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let task_contract_root = work["starting_roots"]["task_contract"].clone();

    write_receipt_with_artifact_as(
        tmp.path(),
        "wrong-key.json",
        "a self-signed receipt cannot retire another key's coordination state",
        "wrong-key-witness.json",
        br#"{"wrong_key":true}"#,
        "agent:t",
        0x43,
    );
    let receipt_path = tmp.path().join("wrong-key.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    receipt["environment"]["vela:producer_context"]["base_path"] =
        serde_json::json!(session_relative);
    receipt["environment"]["vela:producer_context"]["task_contract_root"] = task_contract_root;
    refresh_receipt_binding(&mut receipt);
    std::fs::write(
        &receipt_path,
        vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);

    let output = run_with_env(
        tmp.path(),
        &["land", "wrong-key.json", "--as", "agent:t", "--json"],
        &env,
    );
    assert!(!output.status.success(), "wrong-key close must fail");
    let output = one_json_object(&output);
    assert!(
        output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("does not match the target lease key")),
        "{output}"
    );
    assert!(
        session_record.is_file(),
        "failed close must preserve the active session"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
}

#[test]
fn drop_records_a_signed_exact_release_before_removing_private_scratch() {
    use ed25519_dalek::SigningKey;

    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let owner_key = "42".repeat(32);
    let other_key = "43".repeat(32);
    let owner_env = [("VELA_AGENT_KEY_HEX", owner_key.as_str())];
    let other_env = [("VELA_AGENT_KEY_HEX", other_key.as_str())];
    let target = "erdos:signed-drop";

    let opened = run_with_env(
        tmp.path(),
        &["work", target, "--as", "agent:owner", "--json"],
        &owner_env,
    );
    assert_success(&opened, "open lease for signed drop");
    let opened = one_json_object(&opened);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "work claim was not committed before private session handoff"
    );
    let opened_session = load_work_session(&opened);
    let first_claim_event_id = opened_session["lease"]["claim_event_id"]
        .as_str()
        .unwrap()
        .to_string();
    let session_record = work_session_path(&opened);
    let before_wrong_owner = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let release_state_root_before = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&before_wrong_owner.events)
    );

    let denied = run_with_env(
        tmp.path(),
        &[
            "work",
            target,
            "--drop",
            "--reason",
            "not my lease",
            "--as",
            "agent:other",
            "--json",
        ],
        &other_env,
    );
    assert!(!denied.status.success(), "a non-owner released the lease");
    assert!(session_record.is_file(), "failed release removed scratch");
    assert_eq!(
        vela_protocol::repo::load_from_path(tmp.path())
            .unwrap()
            .events
            .len(),
        before_wrong_owner.events.len(),
        "failed release appended a frontier event"
    );

    let released = run_with_env(
        tmp.path(),
        &[
            "work",
            target,
            "--drop",
            "--reason",
            "switching to a better route",
            "--as",
            "agent:owner",
            "--json",
        ],
        &owner_env,
    );
    assert_success(&released, "owner signed exact release");
    let released = one_json_object(&released);
    let release_event_id = released["release"]["claim_event_id"].as_str().unwrap();
    assert_eq!(
        released["release"]["prior_claim_event_id"],
        first_claim_event_id
    );
    assert_eq!(
        released["release"]["state_root_before"],
        release_state_root_before
    );
    assert_eq!(released["release"]["ttl_seconds"], 0);
    assert_eq!(
        released["release"]["publication"]["state"], "committed_local",
        "signed lease release was not committed: {released}"
    );
    assert!(
        !session_record.exists(),
        "scratch was not removed after the signed release committed"
    );

    let after_release = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(
        released["release"]["state_root_after"],
        format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&after_release.events)
        )
    );
    let release_event = after_release
        .events
        .iter()
        .find(|event| event.id == release_event_id)
        .unwrap();
    assert_eq!(release_event.kind, "attempt.claimed");
    assert_eq!(
        release_event.payload["prior_claim_event_id"],
        first_claim_event_id
    );
    assert_eq!(
        release_event.payload["release_reason"],
        "switching to a better route"
    );
    let owner_signing_key = SigningKey::from_bytes(&[0x42; 32]);
    let owner_pubkey = hex::encode(owner_signing_key.verifying_key().to_bytes());
    assert!(vela_protocol::sign::verify_event_signature(release_event, &owner_pubkey).unwrap());
    let released_lease = after_release
        .attempt_claims
        .iter()
        .find(|claim| claim.obligation_id == target)
        .unwrap();
    assert_eq!(released_lease.lease_ttl_seconds, 0);
    assert_eq!(
        released_lease.claim_event_id.as_deref(),
        Some(release_event_id)
    );

    let reclaimed = run_with_env(
        tmp.path(),
        &["work", target, "--as", "agent:other", "--json"],
        &other_env,
    );
    assert_success(&reclaimed, "immediate reclaim after signed release");
    let reclaimed = one_json_object(&reclaimed);
    assert_eq!(reclaimed["session"]["actor"], "agent:other");
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        "",
        "reclaimed lease was not committed"
    );
}

#[test]
fn land_inference_filters_by_actor_and_requires_exactly_one_owned_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    std::fs::write(
        tmp.path().join("artifacts/inference.json"),
        br#"{"actor_filtered":true}"#,
    )
    .unwrap();
    let owner_key = "42".repeat(32);
    let other_key = "43".repeat(32);
    let owner_env = [("VELA_AGENT_KEY_HEX", owner_key.as_str())];
    let other_env = [("VELA_AGENT_KEY_HEX", other_key.as_str())];

    let mut owner_sessions = Vec::new();
    for target in ["erdos:owned-one", "erdos:owned-two"] {
        let opened = run_with_env(
            tmp.path(),
            &["work", target, "--as", "agent:owner", "--json"],
            &owner_env,
        );
        assert_success(&opened, "open owner session");
        owner_sessions.push(work_session_path(&one_json_object(&opened)));
    }
    let other = run_with_env(
        tmp.path(),
        &["work", "erdos:other-actor", "--as", "agent:other", "--json"],
        &other_env,
    );
    assert_success(&other, "open other actor session");
    let other_session = work_session_path(&one_json_object(&other));

    let ambiguous = run_with_env(
        tmp.path(),
        &[
            "land",
            "--claim",
            "actor-filtered inference must still be exact",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/inference.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:owner",
            "--json",
        ],
        &owner_env,
    );
    assert!(
        !ambiguous.status.success(),
        "ambiguous actor sessions inferred one"
    );
    let ambiguous = one_json_object(&ambiguous);
    let message = ambiguous["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("agent:owner has 2 active work sessions"),
        "{ambiguous}"
    );
    assert!(message.contains("--work <target>"), "{ambiguous}");
    assert!(owner_sessions.iter().all(|path| path.is_file()));
    assert!(other_session.is_file());

    let explicit = run_with_env(
        tmp.path(),
        &[
            "land",
            "--work",
            "erdos:owned-one",
            "--claim",
            "explicit selection closes exactly one owned session",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/inference.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:owner",
            "--json",
        ],
        &owner_env,
    );
    assert_success(&explicit, "explicit actor-owned session landing");
    assert!(!owner_sessions[0].exists());
    assert!(owner_sessions[1].is_file());
    assert!(other_session.is_file());
}

#[test]
fn denied_work_landing_preserves_the_exact_session_and_all_durable_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
    std::fs::write(
        tmp.path().join("artifacts/denied-session.json"),
        br#"{"deny":true}"#,
    )
    .unwrap();
    let agent_key = "42".repeat(32);
    let env = [("VELA_AGENT_KEY_HEX", agent_key.as_str())];
    let opened = run_with_env(
        tmp.path(),
        &["work", "erdos:deny-session", "--as", "agent:t", "--json"],
        &env,
    );
    assert_success(&opened, "open deny-route session");
    let session_record = work_session_path(&one_json_object(&opened));
    write_active_deny_policy(tmp.path());
    let scientific_before = snapshot_scientific_tree(tmp.path());
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let index_before = git_stdout(tmp.path(), &["write-tree"]);

    let denied = run_with_env(
        tmp.path(),
        &[
            "land",
            "--work",
            "erdos:deny-session",
            "--claim",
            "a denied work result cannot consume its authoring session",
            "--type",
            "computational",
            "--replayability",
            "exact",
            "--artifact",
            "artifacts/denied-session.json:witness",
            "--caveat",
            "fixture evidence only",
            "--as",
            "agent:t",
            "--json",
        ],
        &env,
    );
    assert!(!denied.status.success(), "Deny consumed a work result");
    let denied = one_json_object(&denied);
    assert!(
        denied["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("policy denies")),
        "{denied}"
    );
    assert!(
        session_record.is_file(),
        "Deny retired the authoring session"
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git_stdout(tmp.path(), &["write-tree"]), index_before);
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
    for profile in [McpProfile::ReadOnly, McpProfile::Draft] {
        assert!(
            tools_for_profile(profile)
                .iter()
                .all(|tool| tool.name != "decide"),
            "removed finalizer leaked into {} MCP discovery",
            profile.as_str()
        );
    }
    assert!(McpProfile::parse("maintainer").is_err());
}

#[test]
fn untrusted_terminal_text_is_escaped() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_success(
        &run(
            tmp.path(),
            &[
                "init",
                ".",
                "--name",
                "safe-text",
                "--scope",
                "Exercise safe rendering.",
                "--json",
            ],
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

#[test]
fn broken_active_policy_fails_ordinary_check_and_status() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_active_deny_policy(tmp.path());
    break_active_policy_content_address(tmp.path());

    let json_check = run(tmp.path(), &["check", ".", "--json"]);
    assert!(
        !json_check.status.success(),
        "ordinary check must fail when present active-policy bytes are broken"
    );
    let check = one_json_object(&json_check);
    assert_eq!(check["ok"], false, "{check}");
    let active_policy = check["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "active_policy")
        .expect("active_policy check entry");
    assert_eq!(active_policy["status"], "fail", "{check}");
    assert_eq!(active_policy["failed"], 1, "{check}");
    assert!(
        check["diagnostics"].as_array().unwrap().iter().any(|item| {
            item["rule_id"] == "active_policy_integrity"
                && item["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("id does not re-derive"))
        }),
        "{check}"
    );

    let human_check = run(tmp.path(), &["check", "."]);
    assert!(
        !human_check.status.success(),
        "human check must fail when present active-policy bytes are broken"
    );
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&human_check.stdout),
        String::from_utf8_lossy(&human_check.stderr)
    );
    assert!(
        rendered.contains("active policy: broken · Permit blocked"),
        "{rendered}"
    );
    assert!(rendered.contains("id does not re-derive"), "{rendered}");

    let json_status = run(tmp.path(), &["status", ".", "--json"]);
    assert!(
        !json_status.status.success(),
        "JSON status must return failure for a broken active policy"
    );
    let status = one_json_object(&json_status);
    assert_eq!(status["ok"], false, "{status}");
    assert_eq!(status["policy"]["state"], "broken", "{status}");
    assert_eq!(status["policy"]["permit_readiness"], "blocked", "{status}");
    assert!(
        status["policy"]["error"]
            .as_str()
            .is_some_and(|message| message.contains("id does not re-derive")),
        "{status}"
    );

    let human_status = run(tmp.path(), &["status", "."]);
    assert!(
        !human_status.status.success(),
        "human status must return failure for a broken active policy"
    );
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&human_status.stdout),
        String::from_utf8_lossy(&human_status.stderr)
    );
    assert!(rendered.contains("broken · Permit blocked"), "{rendered}");
    assert!(rendered.contains("id does not re-derive"), "{rendered}");
}

#[test]
fn orphan_and_invalid_signature_active_policy_pairs_fail_cli_surfaces() {
    let assert_broken = |dir: &Path, expected: &str| {
        let check = run(dir, &["check", ".", "--json"]);
        assert!(!check.status.success(), "broken pair passed check");
        let check = one_json_object(&check);
        let active_policy = check["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "active_policy")
            .expect("active_policy check entry");
        assert!(
            active_policy["errors"][0]
                .as_str()
                .is_some_and(|message| message.contains(expected)),
            "{check}"
        );

        let status = run(dir, &["status", ".", "--json"]);
        assert!(!status.status.success(), "broken pair passed status");
        let status = one_json_object(&status);
        assert_eq!(status["ok"], false, "{status}");
        assert_eq!(status["policy"]["state"], "broken", "{status}");
        assert_eq!(status["policy"]["permit_readiness"], "blocked", "{status}");
        assert!(
            status["policy"]["error"]
                .as_str()
                .is_some_and(|message| message.contains(expected)),
            "{status}"
        );
    };

    let orphan = tempfile::TempDir::new().unwrap();
    init_git_frontier(orphan.path());
    write_active_deny_policy(orphan.path());
    std::fs::remove_file(orphan.path().join(".vela/policies/active.json")).unwrap();
    assert_broken(orphan.path(), "signature exists without");

    let invalid_signature = tempfile::TempDir::new().unwrap();
    init_git_frontier(invalid_signature.path());
    write_active_deny_policy(invalid_signature.path());
    let signature_path = invalid_signature
        .path()
        .join(".vela/policies/active.sig.json");
    let mut signature: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&signature_path).unwrap()).unwrap();
    signature["signature"] = "00".into();
    std::fs::write(
        signature_path,
        serde_json::to_vec_pretty(&signature).unwrap(),
    )
    .unwrap();
    assert_broken(invalid_signature.path(), "signature must be 64 bytes");
}

#[test]
fn malformed_policy_head_is_a_separate_blocked_readiness_check() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_active_deny_policy(tmp.path());
    let mut frontier = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    frontier.events.push(
        vela_protocol::events::new_review_decision_event(
            "vpr_missing_policy_head",
            vela_protocol::proposals::policy_accept::POLICY_HEAD_PROPOSAL_KIND,
            "accepted",
            None,
            "reviewer:unregistered",
            "malformed head fixture",
            Some("2026-07-14T00:00:00Z"),
        )
        .unwrap(),
    );
    vela_protocol::repo::save_to_path(tmp.path(), &frontier).unwrap();

    let check = run(tmp.path(), &["check", ".", "--json"]);
    assert!(!check.status.success(), "invalid head passed check");
    let check = one_json_object(&check);
    let checks = check["checks"].as_array().unwrap();
    let active_pair = checks
        .iter()
        .find(|item| item["id"] == "active_policy")
        .unwrap();
    assert_eq!(active_pair["status"], "pass", "{check}");
    let readiness = checks
        .iter()
        .find(|item| item["id"] == "policy_readiness")
        .unwrap();
    assert_eq!(readiness["status"], "fail", "{check}");
    assert_eq!(readiness["state"], "active", "{check}");
    assert_eq!(readiness["permit_readiness"], "blocked", "{check}");
    assert_eq!(
        readiness["reason_codes"],
        serde_json::json!(["policy_head_invalid"]),
        "{check}"
    );
    assert!(check["diagnostics"].as_array().unwrap().iter().any(|item| {
        item["rule_id"] == "policy_head_integrity" && item["check"] == "policy_readiness"
    }));

    let status = run(tmp.path(), &["status", ".", "--json"]);
    assert!(!status.status.success(), "invalid head passed status");
    let status = one_json_object(&status);
    assert_eq!(status["policy"]["state"], "active", "{status}");
    assert_eq!(status["policy"]["permit_readiness"], "blocked", "{status}");
    assert_eq!(
        status["policy"]["reason_codes"],
        serde_json::json!(["policy_head_invalid"]),
        "{status}"
    );

    let show_json = run(tmp.path(), &["policy", "show", ".", "--json"]);
    assert!(
        !show_json.status.success(),
        "invalid head passed policy show"
    );
    let show_json = one_json_object(&show_json);
    assert_eq!(show_json["ok"], false, "{show_json}");
    assert_eq!(show_json["state"], "active", "{show_json}");
    assert_eq!(show_json["permit_readiness"], "blocked", "{show_json}");
    assert_eq!(
        show_json["reason_codes"],
        serde_json::json!(["policy_head_invalid"]),
        "{show_json}"
    );
    assert!(show_json["policy"].is_object(), "{show_json}");

    let show_human = run(tmp.path(), &["policy", "show", "."]);
    assert!(
        !show_human.status.success(),
        "invalid head passed human policy show"
    );
    let show_human = String::from_utf8_lossy(&show_human.stdout);
    assert!(
        show_human.contains("active · Permit blocked"),
        "{show_human}"
    );
    assert!(show_human.contains("policy_head_invalid"), "{show_human}");
}

#[test]
fn doctor_names_the_supported_missing_policy_head_recovery() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_active_deny_policy_with_expiry(
        tmp.path(),
        vela_protocol::proposals::policy_accept::CAUSALLY_UNBOUNDED_POLICY_EXPIRY,
    );

    let doctor = run(tmp.path(), &["doctor", ".", "--all", "--json"]);
    assert_success(
        &doctor,
        "doctor with an active pair missing its causal head",
    );
    let doctor = one_json_object(&doctor);
    let policy = doctor["setup"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "policy")
        .expect("doctor policy row");
    assert_eq!(policy["status"], "warn", "{doctor}");
    assert!(
        policy["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("policy_head_missing")),
        "{doctor}"
    );
    assert!(
        policy["next"]
            .as_str()
            .is_some_and(|fix| fix.contains("policy draft <template> --replace")
                && fix.contains("policy decide . --rotate")),
        "{doctor}"
    );
}

#[test]
fn canonical_policy_states_and_readiness_keep_their_routes() {
    let absent = tempfile::TempDir::new().unwrap();
    init_git_frontier(absent.path());

    let absent_check = run(absent.path(), &["check", ".", "--json"]);
    assert_success(&absent_check, "ordinary check without an active policy");
    let absent_check = one_json_object(&absent_check);
    let active_policy = absent_check["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "active_policy")
        .expect("active_policy check entry");
    assert_eq!(active_policy["status"], "pass", "{absent_check}");
    assert_eq!(active_policy["state"], "absent", "{absent_check}");
    assert_eq!(
        active_policy["permit_readiness"], "human_only",
        "{absent_check}"
    );

    let absent_status = run(absent.path(), &["status", ".", "--json"]);
    assert_success(&absent_status, "status without an active policy");
    let absent_status = one_json_object(&absent_status);
    assert_eq!(absent_status["ok"], true, "{absent_status}");
    assert_eq!(
        absent_status["policy"]["state"], "absent",
        "{absent_status}"
    );
    assert_eq!(
        absent_status["policy"]["permit_readiness"], "human_only",
        "{absent_status}"
    );

    let absent_file_status = run(absent.path(), &["status", "frontier.json", "--json"]);
    assert_success(
        &absent_file_status,
        "file-source status without an active policy",
    );
    let absent_file_status = one_json_object(&absent_file_status);
    assert_eq!(absent_file_status["ok"], true, "{absent_file_status}");
    assert_eq!(
        absent_file_status["policy"]["state"], "absent",
        "{absent_file_status}"
    );
    assert_eq!(
        absent_file_status["policy"]["permit_readiness"], "human_only",
        "{absent_file_status}"
    );

    write_receipt(
        absent.path(),
        "receipt.json",
        "an absent policy retains the conservative deferred route",
    );
    let deferred = run(
        absent.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&deferred, "land without an active policy");
    assert_eq!(one_json_object(&deferred)["route"], "deferred");

    let staged = tempfile::TempDir::new().unwrap();
    init_git_frontier(staged.path());
    write_active_deny_policy(staged.path());
    std::fs::remove_file(staged.path().join(".vela/policies/active.sig.json")).unwrap();

    let staged_check = run(staged.path(), &["check", ".", "--json"]);
    assert_success(
        &staged_check,
        "ordinary check with a valid staged unsigned policy",
    );
    let staged_check = one_json_object(&staged_check);
    let active_policy = staged_check["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "active_policy")
        .expect("active_policy check entry");
    assert_eq!(active_policy["status"], "pass", "{staged_check}");
    assert_eq!(active_policy["state"], "staged_unsigned", "{staged_check}");
    assert_eq!(
        active_policy["permit_readiness"], "human_only",
        "{staged_check}"
    );

    let staged_status = run(staged.path(), &["status", ".", "--json"]);
    assert_success(&staged_status, "status with a valid staged unsigned policy");
    let staged_status = one_json_object(&staged_status);
    assert_eq!(staged_status["ok"], true, "{staged_status}");
    assert_eq!(
        staged_status["policy"]["state"], "staged_unsigned",
        "{staged_status}"
    );
    assert_eq!(
        staged_status["policy"]["permit_readiness"], "human_only",
        "{staged_status}"
    );

    let staged_log = run(staged.path(), &["policy", "log", ".", "--json"]);
    assert_success(&staged_log, "policy log with staged unsigned bytes");
    let staged_log = one_json_object(&staged_log);
    assert_eq!(
        staged_log["policy_state"], "staged_unsigned",
        "{staged_log}"
    );
    assert_eq!(staged_log["permit_readiness"], "human_only", "{staged_log}");
    assert!(staged_log["current_policy_id"].is_string(), "{staged_log}");
    assert!(
        !staged_log.to_string().contains("\"active\":"),
        "{staged_log}"
    );

    write_receipt(
        staged.path(),
        "receipt.json",
        "a staged unsigned policy retains the conservative deferred route",
    );
    let deferred = run(
        staged.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert_success(&deferred, "land with a valid staged unsigned policy");
    assert_eq!(one_json_object(&deferred)["route"], "deferred");

    let valid = tempfile::TempDir::new().unwrap();
    init_git_frontier(valid.path());
    write_active_deny_policy(valid.path());

    let valid_check = run(valid.path(), &["check", ".", "--json"]);
    assert_success(&valid_check, "ordinary check with a valid active policy");
    let valid_check = one_json_object(&valid_check);
    let active_policy = valid_check["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "active_policy")
        .expect("active_policy check entry");
    assert_eq!(active_policy["status"], "pass", "{valid_check}");
    assert_eq!(active_policy["state"], "active", "{valid_check}");
    assert_eq!(
        active_policy["permit_readiness"], "human_only",
        "{valid_check}"
    );

    let valid_status = run(valid.path(), &["status", ".", "--json"]);
    assert_success(&valid_status, "status with a valid active policy");
    let valid_status = one_json_object(&valid_status);
    assert_eq!(valid_status["ok"], true, "{valid_status}");
    assert_eq!(valid_status["policy"]["state"], "active", "{valid_status}");
    assert_eq!(
        valid_status["policy"]["permit_readiness"], "human_only",
        "{valid_status}"
    );

    let valid_file_status = run(valid.path(), &["status", "frontier.json", "--json"]);
    assert_success(
        &valid_file_status,
        "file-source status with a valid active policy",
    );
    let valid_file_status = one_json_object(&valid_file_status);
    assert_eq!(valid_file_status["ok"], true, "{valid_file_status}");
    assert_eq!(
        valid_file_status["policy"]["state"], "active",
        "{valid_file_status}"
    );
    assert_eq!(
        valid_file_status["policy"]["permit_readiness"], "human_only",
        "{valid_file_status}"
    );

    let policy_show = run(valid.path(), &["policy", "show", ".", "--json"]);
    assert_success(&policy_show, "policy show with a valid active pair");
    let policy_show = one_json_object(&policy_show);
    assert_eq!(policy_show["state"], "active", "{policy_show}");
    assert_eq!(
        policy_show["permit_readiness"], "human_only",
        "{policy_show}"
    );
    assert!(
        policy_show.get("auto_permit_enabled").is_none(),
        "{policy_show}"
    );

    let policy_test = run(valid.path(), &["policy", "test", ".", "--json"]);
    assert_success(&policy_test, "policy test with human-only active bytes");
    let policy_test = one_json_object(&policy_test);
    assert_eq!(policy_test["state"], "active", "{policy_test}");
    assert_eq!(
        policy_test["permit_readiness"], "human_only",
        "{policy_test}"
    );
    assert!(policy_test.get("lane_open").is_none(), "{policy_test}");
    assert!(policy_test.get("mode").is_none(), "{policy_test}");

    let doctor = run(valid.path(), &["doctor", ".", "--all", "--json"]);
    assert_success(&doctor, "doctor with a valid active pair");
    let doctor = one_json_object(&doctor);
    let policy_check = doctor["setup"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "policy")
        .expect("doctor policy row");
    assert_eq!(policy_check["status"], "warn", "{doctor}");
    assert!(
        policy_check["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("active · Permit human_only")
                && detail.contains("policy_wall_clock_expiry_unanchored")),
        "{doctor}"
    );

    write_receipt(
        valid.path(),
        "receipt.json",
        "a valid deny policy retains its existing route",
    );
    let denied = run(
        valid.path(),
        &["land", "receipt.json", "--as", "agent:t", "--json"],
    );
    assert!(
        !denied.status.success(),
        "valid deny policy must still deny"
    );
    let denied = one_json_object(&denied);
    assert!(
        denied["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("policy denies")),
        "{denied}"
    );
}

#[test]
fn broken_active_policy_fails_every_check_mode_except_schema_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_active_deny_policy(tmp.path());
    break_active_policy_content_address(tmp.path());

    let conformance_dir = tmp.path().join("conformance-fixture");
    std::fs::create_dir_all(&conformance_dir).unwrap();
    std::fs::write(
        conformance_dir.join("noop.json"),
        br#"{"suite":"noop","cases":[]}"#,
    )
    .unwrap();
    let conformance = conformance_dir.to_string_lossy().into_owned();

    let human_modes = [
        vec!["check", ".", "--schema"],
        vec!["check", ".", "--stats"],
        vec![
            "check",
            ".",
            "--conformance",
            "--conformance-dir",
            conformance.as_str(),
        ],
    ];
    for args in human_modes {
        let output = run(tmp.path(), &args);
        assert!(
            !output.status.success(),
            "broken policy passed human mode: {}",
            args.join(" ")
        );
        let rendered = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            rendered.contains("active policy: broken · Permit blocked")
                && rendered.contains("id does not re-derive"),
            "{}: {rendered}",
            args.join(" ")
        );
    }

    let json_modes = [
        vec!["check", ".", "--schema", "--json"],
        vec!["check", ".", "--stats", "--json"],
        vec![
            "check",
            ".",
            "--conformance",
            "--conformance-dir",
            conformance.as_str(),
            "--json",
        ],
    ];
    for args in json_modes {
        let output = run(tmp.path(), &args);
        assert!(
            !output.status.success(),
            "broken policy passed JSON mode: {}",
            args.join(" ")
        );
        let payload = one_json_object(&output);
        let active_policy = payload["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "active_policy")
            .expect("active_policy check entry");
        assert_eq!(active_policy["status"], "fail", "{payload}");
    }

    let schema_only = run(tmp.path(), &["check", ".", "--schema-only"]);
    assert_success(&schema_only, "explicit human schema-only check");
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&schema_only.stdout),
        String::from_utf8_lossy(&schema_only.stderr)
    );
    assert!(!rendered.contains("active policy:"), "{rendered}");

    let schema_only = run(tmp.path(), &["check", ".", "--schema-only", "--json"]);
    assert_success(&schema_only, "explicit JSON schema-only check");
    let payload = one_json_object(&schema_only);
    let active_policy = payload["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "active_policy")
        .expect("active_policy check entry");
    assert_eq!(active_policy["skipped"], true, "{payload}");
}

#[test]
fn strict_check_surfaces_policy_lane_replay_in_human_and_json_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();
    let mut frontier = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    frontier
        .events
        .push(vela_protocol::events::new_finding_event(
            vela_protocol::events::FindingEventInput {
                kind: vela_protocol::events::EVENT_KIND_ATTESTATION_RECORDED,
                finding_id: "vf_policy_lane_fixture",
                actor_id: "agent:t",
                actor_type: "agent",
                reason: "malformed strict-check fixture",
                before_hash: vela_protocol::events::NULL_HASH,
                after_hash: vela_protocol::events::NULL_HASH,
                payload: serde_json::json!({
                    vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY: {
                        "schema": "vela.policy-lane.v2",
                        "policy_id": "vap_forged"
                    }
                }),
                caveats: Vec::new(),
                timestamp: Some("2026-07-14T00:00:00Z"),
            },
        ));
    vela_protocol::repo::save_to_path(tmp.path(), &frontier).unwrap();

    let json_output = run(tmp.path(), &["check", ".", "--strict", "--json"]);
    assert!(
        !json_output.status.success(),
        "malformed policy lane must fail strict JSON check"
    );
    let payload = one_json_object(&json_output);
    let policy_lane = payload["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "policy_lane")
        .expect("policy_lane check entry");
    assert_eq!(policy_lane["status"], "fail", "{payload}");
    assert_eq!(policy_lane["failed"], 1, "{payload}");
    assert!(
        payload["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["rule_id"] == "policy_lane_replay"
                    && item["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("malformed or open-ended"))
            })
    );

    let human_output = run(tmp.path(), &["check", ".", "--strict"]);
    assert!(
        !human_output.status.success(),
        "malformed policy lane must fail strict human check"
    );
    let rendered = format!(
        "{}{}",
        String::from_utf8_lossy(&human_output.stdout),
        String::from_utf8_lossy(&human_output.stderr)
    );
    assert!(
        rendered.contains("policy-lane replay: 1 conflict(s)"),
        "{rendered}"
    );
    assert!(rendered.contains("malformed or open-ended"), "{rendered}");
}

fn review_surface_contract(review: &serde_json::Value) -> serde_json::Value {
    let action_eligibility = |name: &str| {
        review["brief"]["authority"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["action"] == name)
            .and_then(|action| action["eligibility"].as_str())
            .unwrap()
            .to_string()
    };
    let missing_codes = review["brief"]["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|missing| {
            serde_json::json!({
                "field": missing["field"],
                "reason": missing["reason"],
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "proposal_id": review["brief"]["audit"]["proposal_id"],
        "sort_key_proposal_id": review["sort_key"]["proposal_id"],
        "event_log_root": review["event_log_root"],
        "fixed_base_event_log_root": review["brief"]["change"]["fixed_base"]["event_log_root"],
        "accept_eligibility": action_eligibility("accept"),
        "reject_eligibility": action_eligibility("reject"),
        "missing_codes": missing_codes,
        "decision_facts_root": review["brief"]["audit"]["decision_facts_root"],
    })
}

fn normalized_review_snapshot(review: &serde_json::Value) -> serde_json::Value {
    let mut normalized = review.clone();
    assert_eq!(
        normalized["observed_at"], normalized["brief"]["audit"]["observed_at"],
        "each surface must keep its observation timestamp coherent"
    );
    // Every command takes its own read-only observation. Wall-clock bytes are
    // visible by design in the testing projection but are not scientific
    // facts or a signing input, so normalize only those two mirrored fields
    // before asserting complete Decision Brief equality across transports.
    normalized["observed_at"] = "<surface-observation>".into();
    normalized["brief"]["audit"]["observed_at"] = "<surface-observation>".into();
    normalized
}

#[test]
fn decision_brief_read_surfaces_share_the_same_review_contract() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    // A pending proposal may legitimately arrive without its retained receipt
    // (for example after a partial legacy import). Build that state directly
    // instead of corrupting a completed landing transaction's postimage.
    let finding_id = "vf_decision_brief_missing_receipt";
    let proposal = vela_protocol::proposals::new_proposal_at(
        "finding.add",
        vela_protocol::events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        "agent:t",
        "agent",
        "all read-only review surfaces expose one decision brief",
        serde_json::json!({
            "finding": {
                "id": finding_id,
                "assertion": {
                    "text": "all read-only review surfaces expose one decision brief",
                    "type": "computational",
                },
                "conditions": {"text": "integration fixture"},
                "confidence": {"score": 0.2},
                "flags": {"contested": false},
            },
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": format!("sha256:{}", "a".repeat(64)),
                "receipt_path": "records/receipts/sha256/missing.json",
                "record_id": "vrc_missing_review_fixture",
                "operation_id": format!("vop_{}", "b".repeat(64)),
            }
        }),
        Vec::new(),
        vec!["fixture receipt is intentionally unavailable".to_string()],
        "2026-07-14T12:00:00Z",
    );
    let proposal_id = proposal.id.clone();
    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    project.proposals.push(proposal);
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let frontier = tmp.path().to_str().unwrap();

    let review_show = run(
        tmp.path(),
        &["review", "show", frontier, &proposal_id, "--json"],
    );
    assert_success(&review_show, "decision_brief review show");
    let review_show = one_json_object(&review_show);
    assert_eq!(review_show["proposal_id"], proposal_id);
    assert!(
        review_show["next_actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| action["kind"] != "reproduce_pending_artifact"),
        "a proposal with no frontier-local replay input must not advertise a broken reproduce command: {review_show}"
    );

    let review_preview = run(
        tmp.path(),
        &["review", "preview", frontier, &proposal_id, "--json"],
    );
    assert_success(&review_preview, "decision_brief review preview");
    let review_preview = one_json_object(&review_preview);

    let review_list = run(tmp.path(), &["review", "list", frontier, "--json"]);
    assert_success(&review_list, "compact review list");
    let review_list = one_json_object(&review_list);
    assert_eq!(review_list["items"].as_array().unwrap().len(), 1);
    assert!(review_list["items"][0].get("brief").is_none());

    let sign_preview = run(
        tmp.path(),
        &[
            "sign",
            "--preview",
            "--frontier",
            frontier,
            "--limit",
            "100",
            "--json",
        ],
    );
    assert_success(&sign_preview, "decision_brief sign preview");
    let sign_preview = one_json_object(&sign_preview);
    let sign_frontier = &sign_preview["frontiers"][0];
    assert_eq!(sign_frontier["items"].as_array().unwrap().len(), 1);

    let status = run(tmp.path(), &["status", frontier, "--json"]);
    assert!(
        !status.status.success(),
        "the intentionally incomplete legacy fixture must not receive a false strict pass"
    );
    let status = one_json_object(&status);
    assert_eq!(status["ok"], false);
    assert_eq!(status["integrity"]["strict"], "blocked");
    assert!(
        status["integrity"]["blockers_by_code"]["state_integrity"]
            .as_u64()
            .is_some_and(|count| count >= 1),
        "{status}"
    );
    assert_eq!(status["counts"]["pending_review"], 1);
    assert!(status.get("inbox").is_none());

    let contracts = [
        review_surface_contract(&review_show["review"]),
        review_surface_contract(&review_preview["review"]),
        review_surface_contract(&sign_frontier["items"][0]),
    ];
    for contract in &contracts[1..] {
        assert_eq!(contract, &contracts[0]);
    }
    let normalized = [
        normalized_review_snapshot(&review_show["review"]),
        normalized_review_snapshot(&review_preview["review"]),
        normalized_review_snapshot(&sign_frontier["items"][0]),
    ];
    for snapshot in &normalized[1..] {
        assert_eq!(
            snapshot, &normalized[0],
            "all read surfaces must expose one complete Decision Brief"
        );
    }
    assert_eq!(contracts[0]["proposal_id"], proposal_id);
    assert_eq!(contracts[0]["sort_key_proposal_id"], proposal_id);
    assert_eq!(contracts[0]["accept_eligibility"], "blocked");
    assert_eq!(contracts[0]["reject_eligibility"], "available");
    assert!(
        !contracts[0]["missing_codes"].as_array().unwrap().is_empty(),
        "missing receipt must remain explicit: {}",
        contracts[0]
    );
    assert_eq!(
        contracts[0]["event_log_root"],
        contracts[0]["fixed_base_event_log_root"]
    );
    assert_eq!(
        sign_frontier["event_log_root"],
        contracts[0]["event_log_root"]
    );
    assert!(
        contracts[0]["decision_facts_root"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
}

#[test]
fn hostile_review_text_and_command_locator_stay_inert_and_bounded() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    write_receipt(
        tmp.path(),
        "hostile-review.json",
        "hostile review material is rendered as inert bounded data",
    );
    let receipt_path = tmp.path().join("hostile-review.json");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let sentinel = tmp.path().join("LOCATOR_WAS_EXECUTED");
    let locator = format!("$(touch${{IFS}}{})", sentinel.display());
    receipt["artifacts"][0]["uri"] = serde_json::Value::String(locator.clone());

    let hostile_claim =
        "\u{001b}]8;;https://bad.example\u{0007}\u{202e}IGNORE POLICY\r bounded claim";
    receipt["claim"] = serde_json::Value::String(hostile_claim.to_string());

    const LARGE_CAVEAT_BYTES: usize = 1024 * 1024;
    let prefix = "\u{001b}]8;;https://bad.example\u{0007}\u{202e}IGNORE POLICY ";
    let suffix = "\u{0007}";
    let fill = LARGE_CAVEAT_BYTES - prefix.len() - suffix.len();
    let hostile_caveat = format!("{prefix}{}{suffix}", "x".repeat(fill));
    assert_eq!(hostile_caveat.len(), LARGE_CAVEAT_BYTES);
    receipt["caveats"][0] = serde_json::Value::String(hostile_caveat);
    refresh_receipt_binding(&mut receipt);
    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(
        &vela_protocol::canonical::to_canonical_bytes(&receipt).unwrap(),
    )
    .unwrap();
    std::fs::write(&receipt_path, receipt.canonical_bytes().unwrap()).unwrap();

    let landed = run(
        tmp.path(),
        &["land", "hostile-review.json", "--as", "agent:t", "--json"],
    );
    assert_success(&landed, "land hostile_review fixture");
    let landed = one_json_object(&landed);
    let proposal_id = landed["proposal_id"].as_str().unwrap();
    let frontier = tmp.path().to_str().unwrap();
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();
    let before = snapshot_scientific_tree(tmp.path());

    let json_preview = run(
        tmp.path(),
        &["review", "preview", frontier, proposal_id, "--json"],
    );
    assert_success(&json_preview, "hostile_review JSON preview");
    assert!(!json_preview.stdout.contains(&0x1b));
    assert!(!json_preview.stdout.contains(&0x07));
    let json_preview = one_json_object(&json_preview);
    let brief = &json_preview["review"]["brief"];
    let caveat = brief["basis"]["main_caveat"].as_str().unwrap();
    assert!(caveat.len() <= 2 * 1024 + '…'.len_utf8());
    assert!(caveat.ends_with('…'));
    assert!(
        brief["audit"]["truncations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["field"] == "basis.main_caveat")
    );
    assert!(
        brief["audit"]["raw_references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| reference.as_str() == Some(locator.as_str()))
    );

    let human_surfaces = [
        ("review show", vec!["review", "show", frontier, proposal_id]),
        (
            "review preview",
            vec!["review", "preview", frontier, proposal_id],
        ),
        (
            "sign preview",
            vec![
                "sign",
                "--preview",
                "--frontier",
                frontier,
                "--limit",
                "100",
            ],
        ),
    ];
    for (label, args) in human_surfaces {
        let output = run(tmp.path(), &args);
        assert_success(&output, label);
        let rendered = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!rendered.contains('\u{001b}'), "{label}: {rendered:?}");
        assert!(!rendered.contains('\u{0007}'), "{label}: {rendered:?}");
        assert!(!rendered.contains('\u{202e}'), "{label}: {rendered:?}");
        assert!(!rendered.contains('\r'), "{label}: {rendered:?}");
        assert!(rendered.contains("\\u{001B}"), "{label}: {rendered:?}");
        assert!(rendered.contains("\\u{202E}"), "{label}: {rendered:?}");
        assert!(
            rendered.len() < 16 * 1024,
            "{label} rendered {} bytes",
            rendered.len()
        );
    }
    assert!(!sentinel.exists(), "command-like locator was executed");
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
}

#[test]
fn ai_volume_preview_has_stable_bounded_keyset_pages_and_never_fetches_locators() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let locator = format!(
        "http://{}/must-not-be-fetched",
        listener.local_addr().unwrap()
    );

    let mut proposals = (0..125)
        .map(|index| {
            let finding_id = format!("vf_ai_volume_{index:04}");
            let finding = serde_json::json!({
                "id": finding_id,
                "assertion": {
                    "text": format!("AI volume claim {index:04}"),
                    "type": "computational",
                },
                "conditions": {"text": "synthetic volume fixture"},
                "confidence": {"score": 0.2},
                "flags": {"contested": false},
            });
            let created_at = format!("2026-07-14T{:02}:{:02}:00Z", 10 + index / 60, index % 60);
            vela_protocol::proposals::new_proposal_at(
                "finding.add",
                vela_protocol::events::StateTarget {
                    r#type: "finding".to_string(),
                    id: finding_id,
                },
                "agent:volume",
                "agent",
                format!("volume fixture {index:04}"),
                serde_json::json!({"finding": finding}),
                vec![locator.clone()],
                vec!["synthetic volume fixture only".to_string()],
                created_at,
            )
        })
        .collect::<Vec<_>>();
    let mut expected = proposals
        .iter()
        .map(|proposal| (proposal.created_at.clone(), proposal.id.clone()))
        .collect::<Vec<_>>();
    expected.sort();
    let expected = expected
        .into_iter()
        .map(|(_, proposal_id)| proposal_id)
        .collect::<Vec<_>>();
    proposals.reverse();
    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    project.proposals.extend(proposals);
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    std::fs::remove_file(tmp.path().join(".vela/keys/t/private.key")).unwrap();
    let before = snapshot_scientific_tree(tmp.path());
    let frontier = tmp.path().to_str().unwrap();

    let first = run(
        tmp.path(),
        &[
            "sign",
            "--preview",
            "--frontier",
            frontier,
            "--limit",
            "100",
            "--json",
        ],
    );
    assert_success(&first, "ai_volume first preview page");
    let first = one_json_object(&first);
    let first_page = &first["frontiers"][0];
    assert_eq!(first_page["total"], 125);
    assert_eq!(first_page["returned"], 100);
    let first_ids = first_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            item["sort_key"]["proposal_id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(first_ids, expected[..100]);
    let cursor = first_page["next_cursor"].as_str().unwrap().to_string();

    let second = run(
        tmp.path(),
        &[
            "sign",
            "--preview",
            "--frontier",
            frontier,
            "--limit",
            "100",
            "--cursor",
            &cursor,
            "--json",
        ],
    );
    assert_success(&second, "ai_volume continuation page");
    let second = one_json_object(&second);
    let second_page = &second["frontiers"][0];
    assert_eq!(second_page["returned"], 25);
    assert!(second_page["next_cursor"].is_null());
    let second_ids = second_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            item["sort_key"]["proposal_id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(second_ids, expected[100..]);

    let repeated = run(
        tmp.path(),
        &[
            "sign",
            "--preview",
            "--frontier",
            frontier,
            "--limit",
            "100",
            "--json",
        ],
    );
    assert_success(&repeated, "ai_volume repeated first page");
    let repeated = one_json_object(&repeated);
    let repeated_ids = repeated["frontiers"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            item["sort_key"]["proposal_id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(repeated_ids, first_ids);

    let over_limit = run(
        tmp.path(),
        &[
            "sign",
            "--preview",
            "--frontier",
            frontier,
            "--limit",
            "101",
            "--json",
        ],
    );
    assert!(
        !over_limit.status.success(),
        "page size above 100 must fail"
    );
    match listener.accept() {
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
        Ok((_, peer)) => panic!("review fetched locator from {peer}"),
        Err(error) => panic!("inspect locator listener: {error}"),
    }
    assert_eq!(snapshot_scientific_tree(tmp.path()), before);
}

#[test]
fn decision_plan_direct_sign_requires_a_pinned_repository_boundary_before_key_use() {
    let tmp = tempfile::TempDir::new().unwrap();
    init_git_frontier(tmp.path());
    let key_path = register_deterministic_reviewer(tmp.path(), 0x56);
    assert_success(&git(tmp.path(), &["add", "-A"]), "stage direct sign actor");
    assert_success(
        &git(tmp.path(), &["commit", "-qm", "direct sign actor"]),
        "commit direct sign actor",
    );
    write_receipt(
        tmp.path(),
        "direct-sign-receipt.json",
        "a direct sign acceptance uses exact two-phase confirmation",
    );
    let landed = run(
        tmp.path(),
        &[
            "land",
            "direct-sign-receipt.json",
            "--as",
            "agent:t",
            "--json",
        ],
    );
    assert_success(&landed, "land direct sign fixture");
    let landed = one_json_object(&landed);
    let landed_proposal_id = landed["proposal_id"].as_str().unwrap().to_string();
    let finding_id = landed["finding_id"].as_str().unwrap().to_string();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[0x56; 32]);
    let decided_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let mut project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    let mut seed_accept = vela_protocol::proposals::prepare_proposal_accept_in_memory_at(
        &mut project,
        &landed_proposal_id,
        "reviewer:t",
        "seed the accepted finding used by this command-surface fixture",
        None,
        &decided_at,
    )
    .unwrap();
    vela_protocol::proposals::bind_decision_root_to_prepared(
        &mut project,
        &mut seed_accept,
        &format!("sha256:{}", "d".repeat(64)),
    )
    .unwrap();
    vela_protocol::proposals::sign_prepared_decision_events(
        &mut project,
        &seed_accept,
        "reviewer:t",
        &signing_key,
    )
    .unwrap();
    vela_protocol::project::recompute_stats(&mut project);
    let note = vela_protocol::proposals::new_proposal_at(
        "finding.note",
        vela_protocol::events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id,
        },
        "agent:fixture",
        "agent",
        "record the bounded scope",
        serde_json::json!({
            "text": "This accepted observation applies only under the fixture conditions."
        }),
        Vec::new(),
        Vec::new(),
        &decided_at,
    );
    let proposal_id = note.id.clone();
    project.proposals.push(note);
    vela_protocol::repo::save_to_path(tmp.path(), &project).unwrap();
    assert_success(
        &git(tmp.path(), &["add", "-A"]),
        "stage direct sign accepted-finding fixture",
    );
    assert_success(
        &git(
            tmp.path(),
            &["commit", "-qm", "direct sign accepted-finding fixture"],
        ),
        "commit direct sign accepted-finding fixture",
    );
    let frontier = tmp.path().to_str().unwrap();
    let reason = "Receipt, evidence, and scope checked";
    let missing_key = tmp.path().join("direct-sign-preview-must-not-read.key");
    let scientific_before = snapshot_scientific_tree(tmp.path());
    let journal_dir = tmp.path().join(".vela/operation-journals");
    let journal_before = snapshot_exact_tree(&journal_dir);
    let head_before = git_stdout(tmp.path(), &["rev-parse", "HEAD"]);
    let status_before = git_stdout(
        tmp.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );

    let preview = run(
        tmp.path(),
        &[
            "sign",
            &proposal_id,
            "--frontier",
            frontier,
            "--reason",
            reason,
            "--yes",
            "--key",
            missing_key.to_str().unwrap(),
            "--json",
        ],
    );
    assert_success(&preview, "direct sign preview");
    let preview = one_json_object(&preview);
    assert_eq!(preview["command"], "sign.preview");
    assert_eq!(preview["signed"], false);
    assert_eq!(preview["key_read"], false);
    assert!(preview.get("next").is_none(), "{preview}");
    let decision_root = preview["confirmation"]["root"]
        .as_str()
        .unwrap()
        .to_string();
    let confirm_at = preview["confirmation"]["at"].as_str().unwrap().to_string();
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(snapshot_exact_tree(&journal_dir), journal_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );

    std::thread::sleep(std::time::Duration::from_millis(25));
    std::fs::remove_file(&key_path).unwrap();
    let blocked_without_repository_boundary = run(
        tmp.path(),
        &[
            "sign",
            &proposal_id,
            "--frontier",
            frontier,
            "--reason",
            reason,
            "--yes",
            "--confirm-root",
            &decision_root,
            "--confirm-at",
            &confirm_at,
            "--key",
            key_path.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(!blocked_without_repository_boundary.status.success());
    assert!(
        String::from_utf8_lossy(&blocked_without_repository_boundary.stdout)
            .contains("repository_write_intent_denied")
    );
    assert_eq!(snapshot_scientific_tree(tmp.path()), scientific_before);
    assert_eq!(snapshot_exact_tree(&journal_dir), journal_before);
    assert_eq!(git_stdout(tmp.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_stdout(
            tmp.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        status_before
    );
    let project = vela_protocol::repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(
        project
            .proposals
            .iter()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap()
            .status,
        "pending_review"
    );
}

#[test]
fn decision_plan_confirm_root_flags_are_visible_on_sign() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = run(tmp.path(), &["sign", "--help"]);
    assert_success(&output, "sign --help");
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--confirm-root"), "{help}");
    assert!(help.contains("--confirm-at"), "{help}");
}
