//! v0.57: Integration tests for the finding-level span-repair primitive.

use serde_json::json;
use vela_protocol::bundle::{
    Assertion, Conditions, Confidence, Evidence, Extraction, FindingBundle, Flags, Provenance,
};
use vela_protocol::project::{self, Project};
use vela_protocol::{events, repo, state};

fn fixture_finding() -> FindingBundle {
    FindingBundle::new(
        Assertion {
            text: "Span-repair fixture finding".to_string(),
            assertion_type: "mechanism".to_string(),
            entities: Vec::new(),
            relation: None,
            direction: None,
            causal_claim: None,
            causal_evidence_grade: None,
        },
        Evidence {
            evidence_type: "experimental".to_string(),
            model_system: "human".to_string(),
            method: "manual".to_string(),
            replicated: false,
            replication_count: None,
            evidence_spans: Vec::new(),
        },
        Conditions {
            text: "fixture context".to_string(),
            duration: None,
        },
        Confidence::raw(0.5, "fixture", 0.8),
        Provenance {
            source_type: "published_paper".to_string(),
            doi: Some("10.1/test-span-repair".to_string()),
            url: None,
            title: "Span-repair fixture source".to_string(),
            authors: Vec::new(),
            year: Some(2026),
            license: None,
            publisher: None,
            funders: Vec::new(),
            extraction: Extraction::default(),
            review: None,
            contributions: Vec::new(),
        },
        Flags {
            gap: false,
            negative_space: false,
            contested: false,
            retracted: false,
            declining: false,
            gravity_well: false,
            review_state: None,
            superseded: false,
            signature_threshold: None,
            jointly_accepted: false,
        },
    )
}

fn frontier_with_one_finding_no_spans() -> Project {
    project::assemble("span-repair-fixture", vec![fixture_finding()], 0, 0, "test")
}

fn accept_pending_via_decision_plan(
    path: &std::path::Path,
    proposal_id: &str,
) -> Result<(), String> {
    const REVIEWER: &str = "reviewer:repair-fixture";
    let key = ed25519_dalek::SigningKey::from_bytes(&[47_u8; 32]);
    let mut frontier = repo::load_from_path(path)?;
    frontier.actors.push(vela_protocol::sign::ActorRecord {
        id: REVIEWER.to_string(),
        public_key: vela_protocol::sign::pubkey_hex(&key),
        algorithm: "ed25519".to_string(),
        created_at: "2020-01-01T00:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    });
    let decided_at = chrono::Utc::now().to_rfc3339();
    let mut prepared = vela_protocol::proposals::prepare_proposal_accept_in_memory_at(
        &mut frontier,
        proposal_id,
        REVIEWER,
        "repair fixture Decision Plan",
        None,
        &decided_at,
    )?;
    vela_protocol::proposals::bind_decision_root_to_prepared(
        &mut frontier,
        &mut prepared,
        &format!("sha256:{}", "2".repeat(64)),
    )?;
    vela_protocol::proposals::sign_prepared_decision_events(
        &mut frontier,
        &prepared,
        REVIEWER,
        &key,
    )?;
    repo::save_to_path(path, &frontier)?;
    Ok(())
}

#[test]
fn span_repair_apply_appends_span_and_emits_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontier.json");
    let frontier = frontier_with_one_finding_no_spans();
    repo::save_to_path(&path, &frontier).expect("save frontier");
    let finding_id = frontier.findings[0].id.clone();

    let report = state::repair_finding_span(
        &path,
        &finding_id,
        "abstract",
        "Bounded human evidence span body.",
        "reviewer:test",
        "Mechanical span repair",
        false,
    )
    .expect("repair proposal recorded");

    assert_eq!(report.command, "span-repair");
    assert_eq!(report.proposal_status, "pending_review");
    assert!(report.applied_event_id.is_none());
    accept_pending_via_decision_plan(&path, &report.proposal_id).unwrap();

    let reloaded = repo::load_from_path(&path).expect("reload");
    let f = reloaded
        .findings
        .iter()
        .find(|f| f.id == finding_id)
        .unwrap();
    let spans = &f.evidence.evidence_spans;
    assert!(spans.iter().any(
        |s| s.get("section").and_then(|v| v.as_str()) == Some("abstract")
            && s.get("text").and_then(|v| v.as_str()) == Some("Bounded human evidence span body.")
    ));
    let event_count = reloaded
        .events
        .iter()
        .filter(|e| e.kind == "finding.span_repaired")
        .count();
    assert_eq!(event_count, 1);
}

#[test]
fn span_repair_refuses_duplicate_span() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontier.json");
    let mut frontier = frontier_with_one_finding_no_spans();
    let finding_id = frontier.findings[0].id.clone();
    frontier.findings[0]
        .evidence
        .evidence_spans
        .push(json!({"section": "abstract", "text": "already there"}));
    repo::save_to_path(&path, &frontier).expect("save frontier");

    let report = state::repair_finding_span(
        &path,
        &finding_id,
        "abstract",
        "already there",
        "reviewer:test",
        "duplicate attempt",
        false,
    )
    .unwrap();
    let error = accept_pending_via_decision_plan(&path, &report.proposal_id).unwrap_err();
    assert!(error.contains("already carries an identical"));
}

#[test]
fn span_repair_legacy_apply_is_atomic_refusal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontier.json");
    let frontier = frontier_with_one_finding_no_spans();
    let finding_id = frontier.findings[0].id.clone();
    repo::save_to_path(&path, &frontier).expect("save frontier");
    let before = std::fs::read(&path).unwrap();

    let error = state::repair_finding_span(
        &path,
        &finding_id,
        "abstract",
        "bounded evidence",
        "agent:repair",
        "legacy apply probe",
        true,
    )
    .unwrap_err();
    assert!(error.contains("retired"), "unexpected error: {error}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn span_repair_refuses_when_finding_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontier.json");
    let frontier = frontier_with_one_finding_no_spans();
    repo::save_to_path(&path, &frontier).expect("save frontier");

    let result = state::repair_finding_span(
        &path,
        "vf_does_not_exist",
        "abstract",
        "text",
        "reviewer:test",
        "missing finding",
        false,
    );
    assert!(result.is_err());
}

#[test]
fn span_repair_event_validates() {
    let payload_ok = json!({
        "proposal_id": "vpr_test",
        "section": "abstract",
        "text": "real text",
    });
    events::validate_event_payload("finding.span_repaired", &payload_ok).expect("ok");

    let payload_missing_text = json!({
        "proposal_id": "vpr_test",
        "section": "abstract",
    });
    let r = events::validate_event_payload("finding.span_repaired", &payload_missing_text);
    assert!(r.is_err());
}
