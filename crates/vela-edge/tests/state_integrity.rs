use serde_json::json;
use vela_edge::state_integrity;
use vela_protocol::bundle::{
    Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
    FindingBundle, Flags, Provenance,
};
use vela_protocol::events::{self, FindingEventInput, NULL_HASH};
use vela_protocol::project::{self, Project};
use vela_protocol::proposals::{
    ProofPacketRecord, StateProposal, new_proposal, record_proof_export,
};
use vela_protocol::repo;
fn finding(id_text: &str) -> FindingBundle {
    let assertion = Assertion {
        text: format!("BBB integrity test finding {id_text}"),
        assertion_type: "mechanism".to_string(),
        entities: Vec::new(),
        relation: None,
        direction: None,
        causal_claim: None,
        causal_evidence_grade: None,
    };
    let provenance = Provenance {
        source_type: "published_paper".to_string(),
        doi: Some(format!("10.0000/integrity.{id_text}")),
        url: Some(format!("https://example.org/{id_text}")),
        title: format!("Integrity fixture {id_text}"),
        authors: Vec::new(),
        year: Some(2026),
        license: None,
        publisher: None,
        funders: Vec::new(),
        extraction: Extraction::default(),
        review: None,
        contributions: Vec::new(),
    };
    let mut bundle = FindingBundle::new(
        assertion,
        Evidence {
            evidence_type: "experimental".to_string(),
            model_system: "human".to_string(),
            method: "manual".to_string(),
            replicated: false,
            replication_count: None,
            evidence_spans: Vec::new(),
        },
        Conditions {
            text: "human BBB context".to_string(),
            duration: None,
        },
        Confidence {
            kind: ConfidenceKind::FrontierEpistemic,
            score: 0.5,
            basis: "fixture".to_string(),
            method: ConfidenceMethod::ExpertJudgment,
            extraction_confidence: 1.0,
        },
        provenance,
        Flags::default(),
    );
    bundle.created = "2026-05-07T00:00:00Z".to_string();
    bundle
}

fn frontier_with_one_finding() -> Project {
    let finding = finding("one");
    let mut frontier = project::assemble("integrity frontier", vec![finding.clone()], 0, 0, "test");
    frontier.frontier_id = Some("vfr_integrity_test".to_string());
    frontier
        .events
        .push(events::new_finding_event(FindingEventInput {
            kind: "finding.asserted",
            finding_id: &finding.id,
            actor_id: "reviewer:test",
            actor_type: "human",
            reason: "fixture genesis",
            before_hash: NULL_HASH,
            after_hash: &events::finding_hash(&finding),
            payload: json!({"proposal_id": "vpr_fixture", "finding": finding}),
            caveats: Vec::new(),
            timestamp: None,
        }));
    frontier
}

#[test]
fn state_integrity_reports_duplicate_events_as_structural_failure() {
    let mut frontier = frontier_with_one_finding();
    frontier.events.push(frontier.events[0].clone());

    let report = state_integrity::analyze(&frontier);

    assert_eq!(report.schema, "vela.state_integrity_report.v0.1");
    assert_eq!(report.status, "fail");
    assert!(
        report
            .structural_errors
            .iter()
            .any(|error| error.rule_id == "duplicate_event_id")
    );
    assert_eq!(report.proof_freshness, "unknown");
}

#[test]
fn state_integrity_reports_applied_proposal_without_event() {
    let mut frontier = frontier_with_one_finding();
    let proposal = StateProposal {
        status: "applied".to_string(),
        applied_event_id: None,
        reviewed_by: Some("reviewer:test".to_string()),
        reviewed_at: Some("2026-05-07T00:00:00Z".to_string()),
        decision_reason: Some("fixture".to_string()),
        ..new_proposal(
            "finding.note",
            events::StateTarget {
                r#type: "finding".to_string(),
                id: frontier.findings[0].id.clone(),
            },
            "reviewer:test",
            "human",
            "fixture",
            json!({"text": "reviewed note"}),
            Vec::new(),
            Vec::new(),
        )
    };
    frontier.proposals.push(proposal);

    let report = state_integrity::analyze(&frontier);

    assert_eq!(report.status, "fail");
    assert!(
        report
            .structural_errors
            .iter()
            .any(|error| error.rule_id == "applied_proposal_missing_event")
    );
}

#[test]
fn state_integrity_reports_stale_proof_after_accepted_event() {
    let mut frontier = frontier_with_one_finding();
    let snapshot_hash = events::snapshot_hash(&frontier);
    let event_log_hash = events::event_log_hash(&frontier.events);
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-05-07T00:00:00Z".to_string(),
            snapshot_hash,
            event_log_hash,
            packet_manifest_hash:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
        },
    );
    frontier
        .events
        .push(events::new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &frontier.findings[0].id,
            actor_id: "reviewer:test",
            actor_type: "human",
            reason: "new event after proof export",
            before_hash: &events::finding_hash(&frontier.findings[0]),
            after_hash: &events::finding_hash(&frontier.findings[0]),
            payload: json!({"proposal_id": "vpr_after_proof", "status": "accepted"}),
            caveats: Vec::new(),
            timestamp: None,
        }));

    let report = state_integrity::analyze(&frontier);

    assert_eq!(report.status, "fail");
    assert_eq!(report.proof_freshness, "stale");
    assert!(
        report
            .structural_errors
            .iter()
            .any(|error| error.rule_id == "stale_proof_packet")
    );
}

fn append_work_lease(frontier: &mut Project, obligation_id: &str, actor_id: &str, at: &str) {
    let event = events::new_finding_event(FindingEventInput {
        kind: events::EVENT_KIND_ATTEMPT_CLAIMED,
        finding_id: obligation_id,
        actor_id,
        actor_type: "agent",
        reason: "proof freshness coordination fixture",
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({
            "obligation_id": obligation_id,
            "lease_ttl_seconds": 86_400,
            "claimant_actor": actor_id,
            "claimant_pubkey": "11".repeat(32),
        }),
        caveats: Vec::new(),
        timestamp: Some(at),
    });
    vela_protocol::reducer::apply_event(frontier, &event).expect("apply work lease");
    frontier.events.push(event);
    project::recompute_stats(frontier);
}

#[test]
fn work_lease_does_not_stale_an_explicit_nonlease_proof_commitment() {
    let mut frontier = frontier_with_one_finding();
    let snapshot_hash = events::snapshot_hash(&frontier);
    let event_log_hash = events::event_log_hash(&frontier.events);
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-07-18T00:00:00Z".to_string(),
            snapshot_hash,
            event_log_hash: event_log_hash.clone(),
            packet_manifest_hash: "aa".repeat(32),
        },
    );
    assert_eq!(
        frontier
            .proof_state
            .latest_packet
            .nonlease_event_log_hash
            .as_deref(),
        Some(event_log_hash.as_str())
    );

    let target = frontier.findings[0].id.clone();
    append_work_lease(
        &mut frontier,
        &target,
        "agent:proof-freshness",
        "2026-07-18T00:00:01Z",
    );
    let report = state_integrity::analyze(&frontier);

    assert_eq!(report.proof_freshness, "fresh");
    assert!(
        report
            .structural_errors
            .iter()
            .all(|issue| issue.rule_id != "stale_proof_packet")
    );
}

#[test]
fn legacy_proof_root_exempts_only_lease_drift() {
    let mut frontier = frontier_with_one_finding();
    let snapshot_hash = events::snapshot_hash(&frontier);
    let event_log_hash = events::event_log_hash(&frontier.events);
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-07-18T00:00:00Z".to_string(),
            snapshot_hash,
            event_log_hash,
            packet_manifest_hash: "bb".repeat(32),
        },
    );
    frontier.proof_state.latest_packet.nonlease_event_log_hash = None;
    let target = frontier.findings[0].id.clone();
    append_work_lease(
        &mut frontier,
        &target,
        "agent:legacy-proof",
        "2026-07-18T00:00:01Z",
    );

    assert_eq!(
        state_integrity::analyze(&frontier).proof_freshness,
        "fresh",
        "an exact historical full root may match the current non-lease event set"
    );

    frontier
        .events
        .push(events::new_finding_event(FindingEventInput {
            kind: "research_trace.review",
            finding_id: &target,
            actor_id: "agent:legacy-proof",
            actor_type: "agent",
            reason: "unknown non-lease event must remain committed",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({}),
            caveats: Vec::new(),
            timestamp: Some("2026-07-18T00:00:02Z"),
        }));
    assert_eq!(state_integrity::analyze(&frontier).proof_freshness, "stale");
}

#[test]
fn invalid_nonlease_proof_root_grants_no_exemption() {
    let mut frontier = frontier_with_one_finding();
    let snapshot_hash = events::snapshot_hash(&frontier);
    let event_log_hash = events::event_log_hash(&frontier.events);
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-07-18T00:00:00Z".to_string(),
            snapshot_hash,
            event_log_hash,
            packet_manifest_hash: "cc".repeat(32),
        },
    );
    frontier.proof_state.latest_packet.nonlease_event_log_hash = Some("00".repeat(32));
    let target = frontier.findings[0].id.clone();
    append_work_lease(
        &mut frontier,
        &target,
        "agent:invalid-proof-root",
        "2026-07-18T00:00:01Z",
    );

    let report = state_integrity::analyze(&frontier);
    assert_eq!(report.proof_freshness, "stale");
    assert!(
        report
            .structural_errors
            .iter()
            .any(|issue| issue.rule_id == "stale_proof_packet")
    );
}

#[test]
fn integrity_cli_json_reports_state_integrity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("frontier.json");
    let mut frontier = frontier_with_one_finding();
    frontier.events.push(frontier.events[0].clone());
    repo::save_to_path(&path, &frontier).expect("save frontier");

    let report = state_integrity::analyze_path(&path).expect("integrity report");

    assert_eq!(report.status, "fail");
    assert!(
        report
            .structural_errors
            .iter()
            .any(|error| error.rule_id == "duplicate_event_id")
    );
}
