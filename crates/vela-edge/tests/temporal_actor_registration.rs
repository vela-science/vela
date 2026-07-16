use std::fs;
use std::path::Path;
use std::process::Command;

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use vela_edge::signals;
use vela_protocol::actor_registration::ACTOR_REGISTRATION_BOUNDARY_SCHEMA;
use vela_protocol::events::{
    EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED, EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent,
    StateTarget, compute_event_id,
};
use vela_protocol::project::{self, Project};
use vela_protocol::repo::{self, VelaSource};
use vela_protocol::sign::{self, ActorRecord};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn commit_all(repo: &Path, message: &str) -> String {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn save(repo_dir: &Path, project: &Project) {
    fs::create_dir_all(repo_dir.join(".vela")).unwrap();
    repo::save(&VelaSource::VelaRepo(repo_dir.to_path_buf()), project).unwrap();
}

fn actor_event(actor: &str, ordinal: u8, key: Option<&SigningKey>) -> StateEvent {
    let mut event = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: "research_trace.review".into(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: "vfr_test".to_string(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: actor.to_string(),
        },
        timestamp: format!("2026-07-{ordinal:02}T00:00:00Z"),
        reason: format!("legacy actor event {ordinal}"),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({}),
        caveats: vec![],
        signature: None,
    };
    event.id = compute_event_id(&event);
    if let Some(key) = key {
        event.signature = Some(sign::sign_event(&event, key).unwrap());
    }
    event
}

struct Fixture {
    dir: TempDir,
    key: SigningKey,
    project: Project,
    unsigned_anchor_id: String,
    signed_anchor_id: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Vela Test"]);
    git(
        dir.path(),
        &["config", "user.email", "vela-test@example.invalid"],
    );

    let key = SigningKey::generate(&mut OsRng);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    let actor_id = "reviewer:temporal-test";
    let mut project = project::assemble("temporal-test", vec![], 0, 0, "test");
    project.actors.push(ActorRecord {
        id: actor_id.to_string(),
        public_key: public_key.clone(),
        algorithm: "ed25519".to_string(),
        created_at: "2026-07-10T00:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    });
    let unsigned = actor_event(actor_id, 1, None);
    let signed = actor_event(actor_id, 2, Some(&key));
    let unsigned_anchor_id = unsigned.id.clone();
    let signed_anchor_id = signed.id.clone();
    project.events.push(unsigned);
    project.events.push(signed);
    save(dir.path(), &project);
    let anchor_commit = commit_all(dir.path(), "anchor");
    let anchor_tree = git(dir.path(), &["show", "-s", "--format=%T", &anchor_commit]);
    let registry_bytes = fs::read(dir.path().join(".vela/actors.json")).unwrap();
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
            id: actor_id.to_string(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: actor_id.to_string(),
        },
        timestamp: "2026-07-10T00:01:00Z".to_string(),
        reason: "Activate exact-root signature enforcement.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "schema": ACTOR_REGISTRATION_BOUNDARY_SCHEMA,
            "mode": "temporalize_existing",
            "frontier_id": project.frontier_id(),
            "actor_id": actor_id,
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
    activation.signature = Some(sign::sign_event(&activation, &key).unwrap());
    project.events.push(activation);
    save(dir.path(), &project);
    commit_all(dir.path(), "activate");

    Fixture {
        dir,
        key,
        project,
        unsigned_anchor_id,
        signed_anchor_id,
    }
}

#[test]
fn temporal_actor_registration_exempts_only_exact_anchor_members() {
    let mut fixture = fixture();
    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "pre_registration_unsigned_actor_event"
            && signal.target.id == fixture.unsigned_anchor_id
            && signal.blocks.is_empty()
    }));
    assert!(!report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));

    let mut backdated = actor_event("reviewer:temporal-test", 1, None);
    backdated.reason = "post-anchor but backdated".to_string();
    backdated.id = compute_event_id(&backdated);
    let backdated_id = backdated.id.clone();
    fixture.project.events.push(backdated);
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == backdated_id
    }));
}

#[test]
fn temporal_actor_registration_preserves_anchor_signatures() {
    let mut fixture = fixture();
    let event = fixture
        .project
        .events
        .iter_mut()
        .find(|event| event.id == fixture.signed_anchor_id)
        .unwrap();
    event.signature = None;
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "pre_registration_signature_lost"
            && signal.target.id == fixture.signed_anchor_id
            && signal.blocks.iter().any(|block| block == "strict_check")
    }));
}

#[test]
fn temporal_actor_registration_invalid_anchor_grants_no_exemption() {
    let mut fixture = fixture();
    let activation = fixture
        .project
        .events
        .iter_mut()
        .find(|event| event.kind.as_str() == EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .unwrap();
    activation.payload["anchor"]["event_log_root"] = json!(format!("sha256:{}", "0".repeat(64)));
    activation.id = compute_event_id(activation);
    activation.signature = Some(sign::sign_event(activation, &fixture.key).unwrap());
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(
        report
            .signals
            .iter()
            .any(|signal| signal.kind == "actor_registration_anchor_invalid")
    );
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));
}

#[test]
fn temporal_actor_registration_unsigned_anchor_may_gain_valid_signature() {
    let mut fixture = fixture();
    let event = fixture
        .project
        .events
        .iter_mut()
        .find(|event| event.id == fixture.unsigned_anchor_id)
        .unwrap();
    event.signature = Some(sign::sign_event(event, &fixture.key).unwrap());
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(!report.signals.iter().any(|signal| {
        signal.target.id == fixture.unsigned_anchor_id
            && matches!(
                signal.kind.as_str(),
                "pre_registration_unsigned_actor_event"
                    | "unsigned_registered_actor"
                    | "pre_registration_signature_lost"
            )
    }));
}

#[test]
fn temporal_actor_registration_duplicate_boundaries_fail_closed() {
    let mut fixture = fixture();
    let mut duplicate = fixture
        .project
        .events
        .iter()
        .find(|event| event.kind.as_str() == EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .unwrap()
        .clone();
    duplicate.reason = "Duplicate boundary must fail closed.".to_string();
    duplicate.id = compute_event_id(&duplicate);
    duplicate.signature = Some(sign::sign_event(&duplicate, &fixture.key).unwrap());
    fixture.project.events.push(duplicate);
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(
        report
            .signals
            .iter()
            .any(|signal| signal.kind == "actor_registration_anchor_invalid"
                && signal.reason.contains("duplicate"))
    );
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));
}

#[test]
fn temporal_actor_registration_registry_tamper_fails_closed() {
    let mut fixture = fixture();
    fixture.project.actors[0].public_key = "0".repeat(64);
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(
        report
            .signals
            .iter()
            .any(|signal| signal.kind == "actor_registration_anchor_invalid"
                && signal.reason.contains("current actor registry"))
    );
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));
}

#[test]
fn temporal_actor_registration_missing_anchor_is_unavailable_and_timeless() {
    let mut fixture = fixture();
    let activation = fixture
        .project
        .events
        .iter_mut()
        .find(|event| event.kind.as_str() == EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .unwrap();
    activation.payload["anchor"]["git_commit"] = json!("0".repeat(40));
    activation.id = compute_event_id(activation);
    activation.signature = Some(sign::sign_event(activation, &fixture.key).unwrap());
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(
        report
            .signals
            .iter()
            .any(|signal| signal.kind == "actor_registration_anchor_unavailable")
    );
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));
}

#[test]
fn temporal_actor_registration_and_registry_deletion_is_detected_from_history() {
    let mut fixture = fixture();
    fixture
        .project
        .events
        .retain(|event| event.kind.as_str() != EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED);
    fixture.project.actors.clear();
    save(fixture.dir.path(), &fixture.project);
    commit_all(fixture.dir.path(), "delete activation and registry");

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "actor_registration_anchor_invalid"
            && signal
                .reason
                .contains("removed from the checked descendant")
    }));
}

#[test]
fn temporal_actor_registration_anchor_event_deletion_and_mutation_fail_closed() {
    let mut deleted = fixture();
    deleted
        .project
        .events
        .retain(|event| event.id != deleted.unsigned_anchor_id);
    save(deleted.dir.path(), &deleted.project);
    let report = signals::analyze_at(&deleted.project, &[], Some(deleted.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "actor_registration_anchor_invalid"
            && signal.reason.contains("anchored event")
            && signal.reason.contains("missing")
    }));

    let mut mutated = fixture();
    let event = mutated
        .project
        .events
        .iter_mut()
        .find(|event| event.id == mutated.unsigned_anchor_id)
        .unwrap();
    event.reason = "mutated historical meaning".to_string();
    save(mutated.dir.path(), &mutated.project);
    let report = signals::analyze_at(&mutated.project, &[], Some(mutated.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "actor_registration_anchor_invalid"
            && signal.reason.contains("changed canonical content")
    }));
}

#[test]
fn temporal_actor_registration_non_ancestor_anchor_fails_closed() {
    let mut fixture = fixture();
    let tree = git(fixture.dir.path(), &["show", "-s", "--format=%T", "HEAD"]);
    let orphan = git(
        fixture.dir.path(),
        &["commit-tree", &tree, "-m", "unrelated anchor"],
    );
    let activation = fixture
        .project
        .events
        .iter_mut()
        .find(|event| event.kind.as_str() == EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .unwrap();
    activation.payload["anchor"]["git_commit"] = json!(orphan);
    activation.payload["anchor"]["git_tree"] = json!(tree);
    activation.id = compute_event_id(activation);
    activation.signature = Some(sign::sign_event(activation, &fixture.key).unwrap());
    save(fixture.dir.path(), &fixture.project);

    let report = signals::analyze_at(&fixture.project, &[], Some(fixture.dir.path()));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "actor_registration_anchor_invalid"
            && signal.reason.contains("not an ancestor")
    }));
    assert!(report.signals.iter().any(|signal| {
        signal.kind == "unsigned_registered_actor" && signal.target.id == fixture.unsigned_anchor_id
    }));
}

#[test]
fn temporal_actor_registration_bootstrap_requires_empty_registry_and_proof_of_possession() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.name", "Vela Test"]);
    git(
        dir.path(),
        &["config", "user.email", "vela-test@example.invalid"],
    );
    let mut project = project::assemble("bootstrap-test", vec![], 0, 0, "test");
    save(dir.path(), &project);
    let anchor_commit = commit_all(dir.path(), "empty actor registry");
    let anchor_tree = git(dir.path(), &["show", "-s", "--format=%T", &anchor_commit]);
    let registry_bytes = fs::read(dir.path().join(".vela/actors.json")).unwrap();
    let registry_root = format!("sha256:{}", hex::encode(Sha256::digest(registry_bytes)));
    let event_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );

    let key = SigningKey::generate(&mut OsRng);
    let public_key = hex::encode(key.verifying_key().to_bytes());
    project.actors.push(ActorRecord {
        id: "reviewer:bootstrap".to_string(),
        public_key: public_key.clone(),
        algorithm: "ed25519".to_string(),
        created_at: "2026-07-16T00:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    });
    let mut activation = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED.into(),
        target: StateTarget {
            r#type: "actor".to_string(),
            id: "reviewer:bootstrap".to_string(),
        },
        actor: StateActor {
            r#type: "human".to_string(),
            id: "reviewer:bootstrap".to_string(),
        },
        timestamp: "2026-07-16T00:00:00Z".to_string(),
        reason: "Bootstrap the first actor with proof of possession.".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "schema": ACTOR_REGISTRATION_BOUNDARY_SCHEMA,
            "mode": "bootstrap",
            "frontier_id": project.frontier_id(),
            "actor_id": "reviewer:bootstrap",
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
        caveats: vec!["Bootstrap grants no scientific authority.".to_string()],
        signature: None,
    };
    activation.id = compute_event_id(&activation);
    activation.signature = Some(sign::sign_event(&activation, &key).unwrap());
    project.events.push(activation);
    save(dir.path(), &project);
    commit_all(dir.path(), "bootstrap actor");

    let report = signals::analyze_at(&project, &[], Some(dir.path()));
    assert!(!report.signals.iter().any(|signal| {
        matches!(
            signal.kind.as_str(),
            "actor_registration_anchor_invalid" | "actor_registration_anchor_unavailable"
        )
    }));

    let mut tampered: Project =
        serde_json::from_value(serde_json::to_value(&project).unwrap()).unwrap();
    let activation = tampered
        .events
        .iter_mut()
        .find(|event| event.kind.as_str() == EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED)
        .unwrap();
    activation.signature = Some(format!("v1:{}", "0".repeat(128)));
    let report = signals::analyze_at(&tampered, &[], Some(dir.path()));
    assert!(
        report
            .signals
            .iter()
            .any(|signal| signal.kind == "actor_registration_anchor_invalid")
    );
}
