use super::*;
use crate::bundle::{
    Artifact, Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Entity,
    Evidence, Extraction, Flags, Provenance,
};
use crate::project;
use tempfile::TempDir;

use crate::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
use crate::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};

fn durable_attachment(
    finding: &FindingBundle,
    actor: &str,
    method: crate::verifier_attachment::VerifierMethod,
) -> crate::verifier_attachment::VerifierAttachment {
    use crate::verifier_attachment::{
        AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, ProbeKind, ProbeResult,
        VerifierAttachment,
    };
    let root = format!("sha256:{}", "a".repeat(64));
    VerifierAttachment::build(AttachmentDraft {
        target: finding.id.clone(),
        claim_digest: crate::verifier_attachment::claim_digest(&finding.assertion.text),
        verifier_method: method,
        solver_id: "frozen-test-solver".to_string(),
        independent_of: Vec::new(),
        match_to_claim: MatchToClaim {
            matches: true,
            checker_actor: actor.to_string(),
        },
        adversarial_probes: vec![AdversarialProbe {
            kind: ProbeKind::CounterexampleSearch,
            result: ProbeResult::Survived,
            note: "bounded negative probe".to_string(),
            evidence_root: root.clone(),
        }],
        outcome: AttachmentOutcome::Passed,
        verifier_actor: actor.to_string(),
        note: "independent frozen re-check".to_string(),
    })
    .unwrap()
    .with_claim_root(&format!(
        "sha256:{}",
        hex::encode(Sha256::digest(finding.assertion.text.trim().as_bytes()))
    ))
    .unwrap()
    .with_method_integrity(crate::verifier_attachment::MethodIntegrity::Sound)
    .unwrap()
    .with_implementation_id("impl:frozen-test")
    .unwrap()
    .with_execution_evidence_roots(vec![root])
    .unwrap()
}

fn copy_project(project: &Project) -> Project {
    serde_json::from_value(serde_json::to_value(project).unwrap()).unwrap()
}

#[test]
fn proposal_verifier_attachment_is_evidence_only_and_fails_closed() {
    use crate::verifier_attachment::VerifierMethod;
    let actor = "verifier:test";
    let key = ed25519_dalek::SigningKey::from_bytes(&[29_u8; 32]);
    let finding = finding("vf_0123456789abcdef");
    let proposal = new_proposal_at(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: finding.id.clone(),
        },
        "agent:producer",
        "agent",
        "bounded candidate",
        json!({ "finding": finding }),
        Vec::new(),
        vec!["pending human review".to_string()],
        "2026-07-20T00:00:00Z",
    );
    let finding: FindingBundle =
        serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
    let mut project = project::assemble("attachment-test", Vec::new(), 0, 0, "test");
    project.proposals.push(proposal.clone());
    let attachment = durable_attachment(&finding, actor, VerifierMethod::ComputationalSearch);
    let event = append_proposal_verifier_attachment(
        &mut project,
        &proposal.id,
        attachment.clone(),
        actor,
        "2026-07-20T00:01:00Z",
        &key,
    )
    .unwrap();
    assert_eq!(
        project.findings.len(),
        0,
        "evidence must not accept the finding"
    );
    assert_eq!(project.proposals[0].status, "pending_review");
    assert_eq!(project.verifier_attachments, vec![attachment.clone()]);
    assert!(event.signature.is_some());

    let mut stale = durable_attachment(&finding, actor, VerifierMethod::ExactArithmeticRecompute);
    stale.claim_digest = "0000000000000000".to_string();
    stale.id = stale.derive_id().unwrap();
    let mut stale_project = copy_project(&project);
    assert!(
        append_proposal_verifier_attachment(
            &mut stale_project,
            &proposal.id,
            stale,
            actor,
            "2026-07-20T00:02:00Z",
            &key,
        )
        .unwrap_err()
        .contains("claim_digest is stale")
    );

    let mut forged = durable_attachment(&finding, actor, VerifierMethod::ExactArithmeticRecompute);
    forged.id = "vva_forged00000000".to_string();
    let mut forged_project = copy_project(&project);
    assert!(
        append_proposal_verifier_attachment(
            &mut forged_project,
            &proposal.id,
            forged,
            actor,
            "2026-07-20T00:02:00Z",
            &key,
        )
        .unwrap_err()
        .contains("id mismatch")
    );

    let mut missing = durable_attachment(&finding, actor, VerifierMethod::ExactArithmeticRecompute);
    missing.execution_evidence_roots.clear();
    missing.id = missing.derive_id().unwrap();
    let mut missing_project = copy_project(&project);
    assert!(
        append_proposal_verifier_attachment(
            &mut missing_project,
            &proposal.id,
            missing,
            actor,
            "2026-07-20T00:02:00Z",
            &key,
        )
        .unwrap_err()
        .contains("execution_evidence_roots")
    );

    let first = project.verifier_attachments[0]
        .clone()
        .with_lineage_couplings(vec!["code:shared".to_string()])
        .unwrap();
    project.verifier_attachments[0] = first.clone();
    let mut shared = durable_attachment(&finding, actor, VerifierMethod::ExactArithmeticRecompute)
        .with_lineage_couplings(vec!["code:shared".to_string()])
        .unwrap();
    shared.independent_of = vec![first.id.clone()];
    shared.id = shared.derive_id().unwrap();
    assert!(
        append_proposal_verifier_attachment(
            &mut project,
            &proposal.id,
            shared,
            actor,
            "2026-07-20T00:03:00Z",
            &key,
        )
        .unwrap_err()
        .contains("shared failure domain")
    );
}

pub(crate) fn finding(id: &str) -> FindingBundle {
    FindingBundle {
        id: id.to_string(),
        version: 1,
        previous_version: None,
        assertion: Assertion {
            text: format!("Test finding {id}"),
            assertion_type: "mechanism".to_string(),
            entities: vec![Entity {
                name: "LRP1".to_string(),
                entity_type: "protein".to_string(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: None,
                resolution_confidence: 1.0,
                resolution_method: None,
                species_context: None,
                needs_review: false,
            }],
            relation: None,
            direction: None,
            causal_claim: None,
            causal_evidence_grade: None,
        },
        evidence: Evidence {
            evidence_type: "experimental".to_string(),
            model_system: String::new(),
            method: "manual".to_string(),
            replicated: false,
            replication_count: None,
            evidence_spans: Vec::new(),
        },
        conditions: Conditions {
            text: "mouse".to_string(),
            duration: None,
        },
        confidence: Confidence {
            kind: ConfidenceKind::FrontierEpistemic,
            score: 0.7,
            basis: "test".to_string(),
            method: ConfidenceMethod::ExpertJudgment,
            extraction_confidence: 1.0,
        },
        provenance: Provenance {
            source_type: "published_paper".to_string(),
            doi: None,
            url: None,
            title: "Test".to_string(),
            authors: Vec::new(),
            year: Some(2024),
            license: None,
            publisher: None,
            funders: Vec::new(),
            extraction: Extraction::default(),
            review: None,
            contributions: Vec::new(),
        },
        flags: Flags {
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
        links: Vec::new(),
        annotations: Vec::new(),
        attachments: Vec::new(),
        created: "2026-04-23T00:00:00Z".to_string(),
        updated: None,

        access_tier: crate::access_tier::AccessTier::Public,
    }
}

fn proposal_withdrawal_fixture() -> (TempDir, Project, ed25519_dalek::SigningKey, String) {
    let tmp = TempDir::new().unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[17_u8; 32]);
    let identity = IdentityBinding::build(
        IdentityBindingDraft {
            actor_id: "agent:withdrawal-test".to_string(),
            actor_class: ActorClass::Agent,
            created_at: "2026-07-17T00:00:00Z".to_string(),
        },
        &key,
    )
    .unwrap();
    let mut project = project::assemble("withdrawal-test", Vec::new(), 0, 0, "test");
    repo::init_repo(tmp.path(), &project).unwrap();
    let receipt = ReceiptBuilder::build(
        ReceiptInput::new(
            "bounded negative search".to_string(),
            "computational".to_string(),
            "exact".to_string(),
            vec![
                ArtifactInput::new(
                    "artifact.json".to_string(),
                    "witness".to_string(),
                    Some("a".repeat(64)),
                    None,
                )
                .unwrap(),
            ],
            vec!["bounded only".to_string()],
            Vec::new(),
            identity.actor_id.clone(),
            "2026-07-17T00:00:01Z".to_string(),
            format!("sha256:{}", events::event_log_hash(&project.events)),
            ".".to_string(),
            format!("vop_{}", "b".repeat(64)),
            "urn:vela:policy:none".to_string(),
        )
        .unwrap(),
        &identity,
    )
    .unwrap();
    let receipt_root = receipt.canonical_root().unwrap();
    let receipt_path = format!(
        "records/receipts/sha256/{}.json",
        receipt_root.strip_prefix("sha256:").unwrap()
    );
    std::fs::create_dir_all(tmp.path().join("records/receipts/sha256")).unwrap();
    std::fs::write(
        tmp.path().join(&receipt_path),
        receipt.canonical_bytes().unwrap(),
    )
    .unwrap();
    let proposal = new_proposal_at(
        "finding.review",
        crate::events::StateTarget {
            r#type: "finding".to_string(),
            id: "vf_target".to_string(),
        },
        identity.actor_id.clone(),
        "agent",
        "land bounded result",
        json!({
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": receipt_root,
                "receipt_path": receipt_path,
                "record_id": "vrc_0123456789abcdef",
                "operation_id": format!("vop_{}", "b".repeat(64)),
                "review_material_path": "records/review/sha256/test.json",
            }
        }),
        Vec::new(),
        Vec::new(),
        "2026-07-17T00:00:02Z",
    );
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);
    repo::save_to_path(tmp.path(), &project).unwrap();
    (tmp, project, key, proposal_id)
}

#[test]
fn proposal_withdrawal_is_receipt_bound_signed_and_scientifically_noop() {
    let (tmp, mut project, key, proposal_id) = proposal_withdrawal_fixture();
    let finding_root_before = crate::canonical::sha256_canonical(&project.findings).unwrap();
    let artifact_root_before = crate::canonical::sha256_canonical(&project.artifacts).unwrap();
    let event = apply_proposal_withdrawal(
        tmp.path(),
        &mut project,
        &proposal_id,
        "agent:withdrawal-test",
        "superseded bounded run",
        "2026-07-17T00:00:03Z",
        &key,
    )
    .unwrap();
    assert_eq!(event.kind, events::EVENT_KIND_PROPOSAL_WITHDRAWN);
    assert_eq!(event.before_hash, events::NULL_HASH);
    assert_eq!(event.after_hash, events::NULL_HASH);
    assert_eq!(project.proposals[0].status, "withdrawn");
    assert_eq!(
        crate::canonical::sha256_canonical(&project.findings).unwrap(),
        finding_root_before
    );
    assert_eq!(
        crate::canonical::sha256_canonical(&project.artifacts).unwrap(),
        artifact_root_before
    );
    assert!(verify_proposal_withdrawals(tmp.path(), &project).is_empty());
    assert!(verify_proposal_decision_parity(&project).is_empty());
}

#[test]
fn proposal_withdrawal_rejects_wrong_key_and_terminal_decision() {
    let (tmp, mut project, _key, proposal_id) = proposal_withdrawal_fixture();
    let wrong = ed25519_dalek::SigningKey::from_bytes(&[19_u8; 32]);
    assert!(
        apply_proposal_withdrawal(
            tmp.path(),
            &mut project,
            &proposal_id,
            "agent:withdrawal-test",
            "wrong key",
            "2026-07-17T00:00:03Z",
            &wrong,
        )
        .unwrap_err()
        .contains("producer key")
    );
    project.proposals[0].status = "rejected".to_string();
    assert!(
        apply_proposal_withdrawal(
            tmp.path(),
            &mut project,
            &proposal_id,
            "agent:withdrawal-test",
            "too late",
            "2026-07-17T00:00:04Z",
            &wrong,
        )
        .unwrap_err()
        .contains("not pending_review")
    );
}

#[test]
fn proposal_withdrawal_tampering_fails_closed_and_retry_is_idempotent() {
    let (tmp, mut project, key, proposal_id) = proposal_withdrawal_fixture();
    apply_proposal_withdrawal(
        tmp.path(),
        &mut project,
        &proposal_id,
        "agent:withdrawal-test",
        "abandoned",
        "2026-07-17T00:00:03Z",
        &key,
    )
    .unwrap();
    let existing = existing_proposal_withdrawal(tmp.path(), &project, &proposal_id)
        .unwrap()
        .unwrap();
    assert_eq!(existing.id, project.events.last().unwrap().id);
    repo::save_to_path(tmp.path(), &project).unwrap();
    let event_path = tmp
        .path()
        .join(format!(".vela/events/{}.json", existing.id));
    let mut event_value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&event_path).unwrap()).unwrap();
    event_value["reason"] = json!("tampered");
    std::fs::write(
        &event_path,
        serde_json::to_vec_pretty(&event_value).unwrap(),
    )
    .unwrap();
    project.events.last_mut().unwrap().reason = "tampered".to_string();
    assert!(!verify_proposal_withdrawals(tmp.path(), &project).is_empty());
    assert!(
        existing_proposal_withdrawal(tmp.path(), &project, &proposal_id)
            .unwrap_err()
            .contains("does not match")
    );
    let loaded = repo::load_from_path(tmp.path()).unwrap();
    assert_eq!(loaded.proposals[0].status, "pending_review");
    assert!(!verify_proposal_withdrawals(tmp.path(), &loaded).is_empty());
}

#[test]
fn proposal_withdrawal_conflicts_with_duplicates_or_human_decisions() {
    let (tmp, mut project, key, proposal_id) = proposal_withdrawal_fixture();
    let withdrawal = apply_proposal_withdrawal(
        tmp.path(),
        &mut project,
        &proposal_id,
        "agent:withdrawal-test",
        "abandoned",
        "2026-07-17T00:00:03Z",
        &key,
    )
    .unwrap();

    project.events.push(withdrawal.clone());
    assert!(
        verify_proposal_withdrawals(tmp.path(), &project)
            .iter()
            .any(|error| error.contains("exactly one withdrawal event"))
    );
    project.events.pop();

    let mut review = events::new_review_decision_event(
        &proposal_id,
        "finding.review",
        "rejected",
        None,
        "reviewer:fixture",
        "rejected independently",
        Some("2026-07-17T00:00:04Z"),
    )
    .unwrap();
    review.signature = Some(format!("v1:{}", "0".repeat(128)));
    project.events.push(review);
    assert!(
        verify_proposal_withdrawals(tmp.path(), &project)
            .iter()
            .any(|error| error.contains("conflicts with a human decision"))
    );
}

fn artifact(id: &str) -> Artifact {
    Artifact {
        id: id.to_string(),
        kind: "code".to_string(),
        name: "Pinned proof source".to_string(),
        content_hash: format!("sha256:{}", "a".repeat(64)),
        size_bytes: None,
        media_type: Some("text/plain".to_string()),
        storage_mode: "remote".to_string(),
        disclosure: crate::bundle::ArtifactDisclosure::Unknown,
        locator_integrity: crate::bundle::LocatorIntegrity::Unknown,
        availability: crate::bundle::ArtifactAvailability::Unknown,
        locator: Some("https://example.test/proof.lean".to_string()),
        source_url: Some("https://example.test/proof.lean".to_string()),
        license: Some("MIT".to_string()),
        target_findings: vec!["vf_test".to_string()],
        source_id: None,
        provenance: Provenance {
            source_type: "data_release".to_string(),
            doi: None,
            url: Some("https://example.test/proof.lean".to_string()),
            title: "Pinned proof source".to_string(),
            authors: Vec::new(),
            year: Some(2026),
            license: Some("MIT".to_string()),
            publisher: None,
            funders: Vec::new(),
            extraction: Extraction::default(),
            review: None,
            contributions: Vec::new(),
        },
        metadata: std::collections::BTreeMap::from([("commit".to_string(), json!("b".repeat(40)))]),
        review_state: None,
        retracted: false,
        access_tier: crate::access_tier::AccessTier::Public,
        created: "2026-07-12T00:00:00Z".to_string(),
    }
}

fn artifact_retract_proposal(actor: &str) -> StateProposal {
    new_proposal(
        "artifact.retract",
        StateTarget {
            r#type: "artifact".to_string(),
            id: "va_1111111111111111".to_string(),
        },
        actor,
        if actor.starts_with("agent:") {
            "agent"
        } else {
            "human"
        },
        "Legacy pointer has no immutable source pin",
        json!({}),
        Vec::new(),
        Vec::new(),
    )
}

/// Domain tests exercise accepted mutations through the same structural seam
/// as `vela sign`: pending insertion, fixed-time Decision Plan preparation,
/// decision-root binding, and signatures from a registered human key. This is
/// intentionally test-only; no Boolean or typed reviewer name can create a
/// decision in production.
fn create_and_accept_via_decision_plan(
    path: &std::path::Path,
    proposal: StateProposal,
) -> Result<CreateProposalResult, String> {
    const REVIEWER: &str = "reviewer:test-fixture";
    const DECIDED_AT: &str = "2099-07-14T12:34:56Z";
    let key = ed25519_dalek::SigningKey::from_bytes(&[41_u8; 32]);
    let mut frontier = repo::load_from_path(path)?;
    if !frontier.actors.iter().any(|actor| actor.id == REVIEWER) {
        frontier.actors.push(crate::sign::ActorRecord {
            id: REVIEWER.to_string(),
            public_key: crate::sign::pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2020-01-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
    }
    let inserted = insert_pending_in_frontier(&mut frontier, proposal)?;
    if inserted.status == "applied" {
        return Ok(inserted);
    }
    let mut prepared = prepare_proposal_accept_in_memory_at(
        &mut frontier,
        &inserted.proposal_id,
        REVIEWER,
        "test Decision Plan",
        None,
        DECIDED_AT,
    )?;
    bind_decision_root_to_prepared(
        &mut frontier,
        &mut prepared,
        &format!("sha256:{}", "d".repeat(64)),
    )?;
    sign_prepared_decision_events(&mut frontier, &prepared, REVIEWER, &key)?;
    project::recompute_stats(&mut frontier);
    let applied = frontier
        .proposals
        .iter()
        .find(|candidate| candidate.id == inserted.proposal_id)
        .ok_or_else(|| "accepted proposal disappeared".to_string())?;
    let result = CreateProposalResult {
        proposal_id: applied.id.clone(),
        finding_id: applied.target.id.clone(),
        status: applied.status.clone(),
        applied_event_id: applied.applied_event_id.clone(),
    };
    repo::save_to_path(path, &frontier)?;
    Ok(result)
}

#[test]
fn artifact_retract_stays_pending_until_human_acceptance() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    frontier.artifacts.push(artifact("va_1111111111111111"));
    repo::save_to_path(&path, &frontier).unwrap();

    let result =
        insert_pending_at_path(&path, artifact_retract_proposal("agent:legacy-cleanup")).unwrap();
    assert_eq!(result.status, "pending_review");
    let loaded = repo::load_from_path(&path).unwrap();
    assert!(!loaded.artifacts[0].retracted);
    assert_eq!(loaded.events.len(), 1);
}

#[test]
fn human_applied_artifact_retract_emits_existing_lifecycle_event() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    frontier.artifacts.push(artifact("va_1111111111111111"));
    repo::save_to_path(&path, &frontier).unwrap();

    let result =
        create_and_accept_via_decision_plan(&path, artifact_retract_proposal("reviewer:human"))
            .unwrap();
    assert_eq!(result.status, "applied");
    let loaded = repo::load_from_path(&path).unwrap();
    assert!(loaded.artifacts[0].retracted);
    let retracted = loaded
        .events
        .iter()
        .find(|event| event.kind == "artifact.retracted")
        .unwrap();
    assert_eq!(retracted.kind, "artifact.retracted");
    assert_eq!(retracted.target.id, "va_1111111111111111");
}

#[test]
fn artifact_retract_rejects_unknown_wrong_type_and_repeat() {
    let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    frontier.artifacts.push(artifact("va_1111111111111111"));
    let unknown = new_proposal(
        "artifact.retract",
        StateTarget {
            r#type: "artifact".to_string(),
            id: "va_2222222222222222".to_string(),
        },
        "agent:test",
        "agent",
        "retire unknown",
        json!({}),
        Vec::new(),
        Vec::new(),
    );
    assert!(validate_new_proposal(&frontier, &unknown).is_err());

    let mut wrong_type = artifact_retract_proposal("agent:test");
    wrong_type.target.r#type = "finding".to_string();
    assert!(validate_new_proposal(&frontier, &wrong_type).is_err());

    frontier.artifacts[0].retracted = true;
    assert!(validate_new_proposal(&frontier, &artifact_retract_proposal("agent:test")).is_err());
}

#[test]
fn pending_review_proposal_does_not_mutate_frontier() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let proposal = new_proposal(
        "finding.review",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        },
        "reviewer:test",
        "human",
        "Mouse-only evidence",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    insert_pending_at_path(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(loaded.events.len(), 1); // genesis only (proposal pending)
    assert_eq!(loaded.proposals.len(), 1);
    assert!(!loaded.findings[0].flags.contested);
}

#[test]
fn applied_proposal_emits_event_and_stales_proof() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-04-23T00:00:00Z".to_string(),
            snapshot_hash: "a".repeat(64),
            event_log_hash: "b".repeat(64),
            packet_manifest_hash: "c".repeat(64),
        },
    );
    repo::save_to_path(&path, &frontier).unwrap();
    let proposal = new_proposal(
        "finding.review",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        },
        "reviewer:test",
        "human",
        "Mouse-only evidence",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    create_and_accept_via_decision_plan(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(loaded.events.len(), 3); // genesis + domain + review.accepted
    assert!(loaded.findings[0].flags.contested);
    assert_eq!(loaded.proposals[0].status, "applied");
    assert_eq!(loaded.proof_state.latest_packet.status, "stale");
}

// ── Typed process authority boundaries ────────────────────────────

#[test]
fn preview_reports_changed_objects_and_event_kind_without_mutation() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let proposal = new_proposal(
        "finding.review",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        },
        "reviewer:test",
        "human",
        "Mouse-only evidence",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    let proposal_id = insert_pending_at_path(&path, proposal).unwrap().proposal_id;

    let preview = preview_at_path(&path, &proposal_id, "reviewer:test").unwrap();

    assert_eq!(preview.changed_findings, vec!["vf_test"]);
    assert!(preview.changed_artifacts.is_empty());
    assert_eq!(preview.event_kinds, vec!["finding.reviewed"]);
    assert_eq!(
        preview.new_event_ids,
        vec![preview.applied_event_id.clone()]
    );
    assert_eq!(preview.events_delta, 1);
    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(loaded.events.len(), 1, "preview must not mutate events");
    assert_eq!(
        loaded.proposals[0].status, "pending_review",
        "preview must not accept the proposal"
    );
}

#[test]
fn pending_note_proposal_does_not_mutate_annotations() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let proposal = new_proposal(
        "finding.note",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        },
        "reviewer:test",
        "human",
        "Track mouse-only evidence",
        json!({"text": "Track mouse-only evidence"}),
        Vec::new(),
        Vec::new(),
    );
    insert_pending_at_path(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(loaded.events.len(), 1); // genesis only
    assert_eq!(loaded.proposals.len(), 1);
    assert!(loaded.findings[0].annotations.is_empty());
    assert_eq!(loaded.proposals[0].kind, "finding.note");
}

#[test]
fn applied_note_emits_noted_event_and_stales_proof() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    record_proof_export(
        &mut frontier,
        ProofPacketRecord {
            generated_at: "2026-04-23T00:00:00Z".to_string(),
            snapshot_hash: "a".repeat(64),
            event_log_hash: "b".repeat(64),
            packet_manifest_hash: "c".repeat(64),
        },
    );
    repo::save_to_path(&path, &frontier).unwrap();
    let proposal = new_proposal(
        "finding.note",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        },
        "reviewer:test",
        "human",
        "Track mouse-only evidence",
        json!({"text": "Track mouse-only evidence"}),
        Vec::new(),
        Vec::new(),
    );
    let result = create_and_accept_via_decision_plan(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(loaded.events.len(), 3); // genesis + finding.noted + review.accepted
    assert_eq!(loaded.events[1].kind, "finding.noted");
    assert_eq!(loaded.findings[0].annotations.len(), 1);
    assert_eq!(loaded.proposals[0].status, "applied");
    assert_eq!(
        loaded.proposals[0].applied_event_id,
        result.applied_event_id
    );
    assert_eq!(loaded.proof_state.latest_packet.status, "stale");
}

#[test]
fn retract_emits_per_dependent_cascade_events() {
    // Phase L: a retraction must emit one canonical
    // `finding.dependency_invalidated` event per affected dependent
    // in BFS depth order. Build a tiny dependency chain:
    //   src  <-supports- dep1  <-depends- dep2
    // and assert that retracting `src` produces three events:
    // [retracted(src), dep_invalidated(dep1, depth=1),
    //  dep_invalidated(dep2, depth=2)] all carrying the source's
    // canonical event ID as `upstream_event_id`.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut src = finding("vf_src");
    let mut dep1 = finding("vf_dep1");
    let mut dep2 = finding("vf_dep2");
    src.assertion.text = "src finding".into();
    dep1.assertion.text = "dep1 finding".into();
    dep2.assertion.text = "dep2 finding".into();
    // BFS edges flow from dependent → upstream via `target`.
    dep1.add_link("vf_src", "supports", "");
    dep2.add_link("vf_dep1", "depends", "");
    let frontier = project::assemble("test", vec![src, dep1, dep2], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();

    let proposal = new_proposal(
        "finding.retract",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_src".to_string(),
        },
        "reviewer:test",
        "human",
        "Source paper retracted by publisher",
        json!({}),
        Vec::new(),
        Vec::new(),
    );
    create_and_accept_via_decision_plan(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();

    // genesis + source retract + 2 cascade events + review.accepted.
    assert_eq!(loaded.events.len(), 5, "{:?}", loaded.events);
    let kinds: Vec<&str> = loaded.events.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(kinds[0], "frontier.created");
    assert_eq!(kinds[1], "finding.retracted");
    assert_eq!(kinds[2], "finding.dependency_invalidated");
    assert_eq!(kinds[3], "finding.dependency_invalidated");
    assert_eq!(kinds[4], "review.accepted");

    let source_event_id = loaded.events[1].id.clone();
    let dep1_event = &loaded.events[2];
    let dep2_event = &loaded.events[3];
    assert_eq!(dep1_event.target.id, "vf_dep1");
    assert_eq!(dep2_event.target.id, "vf_dep2");
    assert_eq!(
        dep1_event
            .payload
            .get("upstream_event_id")
            .and_then(|v| v.as_str()),
        Some(source_event_id.as_str())
    );
    assert_eq!(
        dep1_event.payload.get("depth").and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        dep2_event.payload.get("depth").and_then(|v| v.as_u64()),
        Some(2)
    );
    // Both dependents must end up contested in materialized state.
    let dep1 = loaded.findings.iter().find(|f| f.id == "vf_dep1").unwrap();
    let dep2 = loaded.findings.iter().find(|f| f.id == "vf_dep2").unwrap();
    assert!(dep1.flags.contested);
    assert!(dep2.flags.contested);
    let src = loaded.findings.iter().find(|f| f.id == "vf_src").unwrap();
    assert!(src.flags.retracted);
}

#[test]
fn proposal_id_is_content_addressed_independent_of_created_at() {
    // Phase P (v0.5): identical logical proposals constructed at different
    // times must produce the same `vpr_…`. This is the substrate property
    // that makes agent retries idempotent.
    let target = StateTarget {
        r#type: "finding".to_string(),
        id: "vf_test".to_string(),
    };
    let mut a = new_proposal(
        "finding.review",
        target.clone(),
        "reviewer:test",
        "human",
        "scope narrower than claim",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    let mut b = new_proposal(
        "finding.review",
        target,
        "reviewer:test",
        "human",
        "scope narrower than claim",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    // Force divergent timestamps; the IDs must still match.
    a.created_at = "2026-04-25T00:00:00Z".to_string();
    b.created_at = "2026-09-12T17:32:00Z".to_string();
    a.id = proposal_id(&a);
    b.id = proposal_id(&b);
    assert_eq!(a.id, b.id, "vpr_… must not depend on created_at");
}

#[test]
fn pending_insert_is_idempotent_under_repeated_calls() {
    // Phase P: inserting twice with identical content must
    // not duplicate the proposal nor emit two events. The second call
    // returns the same proposal_id and applied_event_id as the first.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();

    let make = || {
        new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "agent retry test",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        )
    };

    let first = create_and_accept_via_decision_plan(&path, make()).unwrap();
    let second = create_and_accept_via_decision_plan(&path, make()).unwrap();

    assert_eq!(first.proposal_id, second.proposal_id);
    assert_eq!(first.applied_event_id, second.applied_event_id);

    let loaded = repo::load_from_path(&path).unwrap();
    assert_eq!(
        loaded.proposals.len(),
        1,
        "second insertion must not insert a duplicate proposal"
    );
    // genesis + one domain event + one review.accepted event; no retry event.
    assert_eq!(
        loaded.events.len(),
        3,
        "second insertion must not emit a duplicate event"
    );
}

#[test]
fn verifier_attach_accepts_and_derives_verified() {
    use crate::verifier_attachment::{
        AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, ProbeKind, ProbeResult,
        VerifierAttachment, VerifierMethod, derive_gate_status,
    };
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let cd = crate::verifier_attachment::claim_digest("Test finding");
    let mk = |method: VerifierMethod, solver: &str, indep: Vec<String>| {
        VerifierAttachment::build(AttachmentDraft {
            target: "vf_test".to_string(),
            claim_digest: cd.clone(),
            verifier_method: method,
            solver_id: solver.to_string(),
            independent_of: indep,
            match_to_claim: MatchToClaim {
                matches: true,
                checker_actor: "opus".to_string(),
            },
            adversarial_probes: vec![AdversarialProbe {
                kind: ProbeKind::CounterexampleSearch,
                result: ProbeResult::Survived,
                note: String::new(),
                evidence_root: String::new(),
            }],
            outcome: AttachmentOutcome::Passed,
            verifier_actor: "opus".to_string(),
            note: String::new(),
        })
        .unwrap()
    };
    let a1 = mk(VerifierMethod::ExactArithmeticRecompute, "solver-a", vec![]);
    let a2 = mk(
        VerifierMethod::LiteratureCorroboration,
        "solver-b",
        vec![a1.id.clone()],
    );
    for att in [&a1, &a2] {
        let proposal = new_proposal(
            "verifier.attach",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "attach verifier evidence",
            json!({ "attachment": att }),
            Vec::new(),
            Vec::new(),
        );
        create_and_accept_via_decision_plan(&path, proposal).unwrap();
    }
    let reloaded = repo::load_from_path(&path).unwrap();
    assert_eq!(
        reloaded.verifier_attachments.len(),
        2,
        "both attachments stored in the sidecar collection"
    );
    // Per-finding gate status is DERIVED on read, never stored.
    let outcome = derive_gate_status(&cd, &reloaded.verifier_attachments);
    assert!(
        outcome.is_verified(),
        "two independent matched surviving-probe attachments must derive Verified"
    );
}

// ---- exact-lane proposal-level wrapper (Phase 1A) ----

fn admit_ready_fixture() -> (
    StateProposal,
    crate::bundle::FindingBundle,
    Vec<crate::verifier_attachment::VerifierAttachment>,
) {
    use crate::verifier_attachment::{
        AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, MethodIntegrity,
        ProbeKind, ProbeResult, VerifierAttachment, VerifierMethod,
    };
    // A finding whose id is its real content-address (the drift-pin passes).
    let mut finding = crate::test_support::make_finding("vf_placeholder", 1.0, "measurement");
    finding.id =
        crate::bundle::FindingBundle::content_address(&finding.assertion, &finding.provenance);
    let cd = crate::verifier_attachment::claim_digest(&finding.assertion.text);
    // Build genuinely id-valid attachments: integrity and implementation_id
    // are set through the re-deriving builders (post-build field mutation
    // would leave the stored id no longer content-addressing the body, which
    // the gate's G4 id-integrity check now excludes). Independence is
    // one-directional (a2 names a1); a mutual 2-cycle is unconstructable.
    let mk = |method: VerifierMethod, solver: &str, impl_id: &str, independent_of: Vec<String>| {
        VerifierAttachment::build(AttachmentDraft {
            target: finding.id.clone(),
            claim_digest: cd.clone(),
            verifier_method: method,
            solver_id: solver.to_string(),
            independent_of,
            match_to_claim: MatchToClaim {
                matches: true,
                checker_actor: "checker".to_string(),
            },
            adversarial_probes: vec![AdversarialProbe {
                kind: ProbeKind::FormalismFidelity,
                result: ProbeResult::Survived,
                note: String::new(),
                evidence_root: String::new(),
            }],
            outcome: AttachmentOutcome::Passed,
            verifier_actor: "verifier:vela-verify".to_string(),
            note: String::new(),
        })
        .unwrap()
        .with_method_integrity(MethodIntegrity::Sound)
        .unwrap()
        .with_implementation_id(impl_id)
        .unwrap()
    };
    // a1 is built first so its id is final before a2 references it.
    let a1 = mk(
        VerifierMethod::ComputationalSearch,
        "cp-sat",
        "impl-a",
        vec![],
    );
    let a2 = mk(
        VerifierMethod::ExactArithmeticRecompute,
        "pari",
        "impl-b",
        vec![a1.id.clone()],
    );
    let proposal = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: finding.id.clone(),
        },
        "producer:campaign", // != every verifier_actor
        "agent",
        "campaign finding",
        json!({ "finding": finding.clone() }),
        Vec::new(),
        Vec::new(),
    );
    (proposal, finding, vec![a1, a2])
}

#[test]
fn exact_lane_wrapper_happy_path() {
    let (p, f, atts) = admit_ready_fixture();
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(admit, "should admit, refused for: {reasons:?}");
}

#[test]
fn exact_lane_wrapper_rejects_wrong_kind() {
    let (mut p, f, atts) = admit_ready_fixture();
    p.kind = "verifier.attach".to_string();
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
    assert!(reasons.iter().any(|r| r.contains("finding.add")));
}

#[test]
fn exact_lane_wrapper_rejects_target_mismatch() {
    let (mut p, f, atts) = admit_ready_fixture();
    p.target.id = "vf_other".to_string();
    let (admit, _r) = exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
}

// ATTACK: the assertion text is edited after the id was minted.
#[test]
fn exact_lane_wrapper_rejects_content_address_drift() {
    let (p, mut f, atts) = admit_ready_fixture();
    f.assertion.text = "a tampered, inflated claim".to_string();
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
    assert!(reasons.iter().any(|r| r.contains("drift")));
}

#[test]
fn exact_lane_wrapper_rejects_retracted_or_superseded() {
    let (p, mut f, atts) = admit_ready_fixture();
    f.flags.retracted = true;
    let (admit, _r) = exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
    let (p2, mut f2, atts2) = admit_ready_fixture();
    f2.flags.superseded = true;
    let (admit2, _r2) =
        exact_lane_eligible(&p2, &f2, &atts2, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit2);
}

#[test]
fn exact_lane_wrapper_rejects_synthetic_signal() {
    let (p, f, atts) = admit_ready_fixture();
    let synthetic = BTreeSet::from([f.id.clone()]);
    let (admit, reasons) = exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &synthetic, false);
    assert!(!admit);
    assert!(reasons.iter().any(|r| r.contains("synthetic")));
}

#[test]
fn exact_lane_wrapper_rejects_open_contradiction() {
    let (p, f, atts) = admit_ready_fixture();
    let contradictions = BTreeSet::from([f.id.clone()]);
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &atts, &contradictions, &BTreeSet::new(), false);
    assert!(!admit);
    assert!(reasons.iter().any(|r| r.contains("contradiction")));
}

// ATTACK: the producer is also a corroborator (same actor).
#[test]
fn exact_lane_wrapper_rejects_producer_equals_verifier() {
    let (p, f, mut atts) = admit_ready_fixture();
    atts[0].verifier_actor = "producer:campaign".to_string(); // == proposal.actor.id
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &atts, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
    assert!(reasons.iter().any(|r| r.contains("corroborate itself")));
}

// The attachment predicate still gates: a single attachment fails.
#[test]
fn exact_lane_wrapper_delegates_to_attachment_predicate() {
    let (p, f, atts) = admit_ready_fixture();
    let single = vec![atts[0].clone()];
    let (admit, _r) =
        exact_lane_eligible(&p, &f, &single, &BTreeSet::new(), &BTreeSet::new(), false);
    assert!(!admit);
}

// floor_sufficient: the exact-lane FLOOR is the proof, so the lane admits
// on the floor alone (NO attachments) — the >=2-attachment bar is waived.
#[test]
fn exact_lane_wrapper_floor_sufficient_admits_without_attachments() {
    let (p, f, _atts) = admit_ready_fixture();
    let (admit, reasons) =
        exact_lane_eligible(&p, &f, &[], &BTreeSet::new(), &BTreeSet::new(), true);
    assert!(
        admit,
        "floor-sufficient should admit with no attachments: {reasons:?}"
    );
}

// ...but floor_sufficient never bypasses the proposal-level guards.
#[test]
fn exact_lane_wrapper_floor_sufficient_still_honors_guards() {
    let (p, mut f, _atts) = admit_ready_fixture();
    f.flags.retracted = true;
    let (admit, _r) = exact_lane_eligible(&p, &f, &[], &BTreeSet::new(), &BTreeSet::new(), true);
    assert!(
        !admit,
        "retracted finding refuses even when floor-sufficient"
    );

    let (p2, f2, _) = admit_ready_fixture();
    let synthetic = BTreeSet::from([f2.id.clone()]);
    let (admit2, _r2) = exact_lane_eligible(&p2, &f2, &[], &BTreeSet::new(), &synthetic, true);
    assert!(
        !admit2,
        "synthetic source refuses even when floor-sufficient"
    );
}

// ---- derive_trust_tier projection ----

fn policy_admit_event(proposal_id: &str) -> StateEvent {
    StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: "vev_test_admit".to_string(),
        kind: events::EVENT_KIND_POLICY_AUTO_ADMITTED.into(),
        target: StateTarget {
            r#type: "proposal".to_string(),
            id: proposal_id.to_string(),
        },
        actor: StateActor {
            id: "policy:exact-lane".to_string(),
            r#type: "agent".to_string(),
        },
        timestamp: "2026-06-19T00:00:00Z".to_string(),
        reason: "exact-lane auto-admit".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({ "proposal_id": proposal_id }),
        caveats: vec![],
        signature: None,
    }
}

#[test]
fn trust_tier_accepted_when_landed() {
    let (_p, f, _a) = admit_ready_fixture();
    let frontier = project::assemble("t", vec![f.clone()], 0, 0, "t");
    assert_eq!(derive_trust_tier(&frontier, &f.id), TrustTier::Accepted);
}

#[test]
fn trust_tier_candidate_when_retracted() {
    let (_p, mut f, _a) = admit_ready_fixture();
    f.flags.retracted = true;
    let frontier = project::assemble("t", vec![f.clone()], 0, 0, "t");
    assert_eq!(derive_trust_tier(&frontier, &f.id), TrustTier::Candidate);
}

#[test]
fn trust_tier_machine_verified_for_pending_auto_admitted() {
    let (p, f, atts) = admit_ready_fixture();
    let mut frontier = project::assemble("t", vec![], 0, 0, "t");
    frontier.verifier_attachments = atts;
    frontier.events.push(policy_admit_event(&p.id));
    frontier.proposals.push(p);
    assert_eq!(
        derive_trust_tier(&frontier, &f.id),
        TrustTier::MachineVerified
    );
}

// A pending finding with passing attachments but no auto-admit marker is
// only schema_checked — never machine_verified.
#[test]
fn trust_tier_schema_checked_without_admit_marker() {
    let (p, f, atts) = admit_ready_fixture();
    let mut frontier = project::assemble("t", vec![], 0, 0, "t");
    frontier.verifier_attachments = atts;
    frontier.proposals.push(p); // pending, NO policy.auto_admitted event
    assert_eq!(
        derive_trust_tier(&frontier, &f.id),
        TrustTier::SchemaChecked
    );
}

#[test]
fn trust_tier_candidate_when_unknown() {
    let frontier = project::assemble("t", vec![], 0, 0, "t");
    assert_eq!(
        derive_trust_tier(&frontier, "vf_nothing"),
        TrustTier::Candidate
    );
}

#[test]
fn engine_preview_reports_new_review_warning() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();

    // A sparse finding (no evidence span) introduces a review warning
    // on accept — the deterministic signal the Engine reads.
    let f = finding("vf_engine_gate");
    let proposal = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: f.id.clone(),
        },
        "reviewer:test",
        "human",
        "add a sparse finding",
        json!({ "finding": f }),
        Vec::new(),
        Vec::new(),
    );
    let created = insert_pending_at_path(&path, proposal).unwrap();
    let vpr = created.proposal_id.clone();

    // Prospective verdict: warns (new review warning), would not block.
    let preview = preview_engine_verdict(&path, &vpr).unwrap();
    assert_eq!(preview.status, "warn");
    assert!(!preview.new_warnings.is_empty());
}

#[test]
fn math_profile_skips_study_design_checks_for_theoretical_findings() {
    use crate::evidence_ci::{self, EvidenceCiClassification};

    fn warn_ids(report: &evidence_ci::EvidenceCiReport) -> std::collections::BTreeSet<String> {
        report
            .checks
            .iter()
            .filter(|c| c.classification == EvidenceCiClassification::ReviewWarning)
            .map(|c| c.id.clone())
            .collect()
    }
    const STUDY_DESIGN: &[&str] = &[
        "condition.population",
        "condition.comparator_or_baseline",
        "condition.endpoint",
        "trial.registry_reference",
    ];

    // A theoretical claim (Erdős-style open question, no empirical signal)
    // must NOT raise the clinical study-design warnings — they are a
    // category error on a formal claim.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut theo = finding("vf_theo");
    theo.assertion.assertion_type = "open_question".to_string();
    theo.evidence.evidence_type = "theoretical".to_string();
    theo.conditions.text = "Erdős problem statement".to_string();
    let frontier = project::assemble("math", vec![theo], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let report = evidence_ci::run_project(&repo::load_from_path(&path).unwrap(), &path);
    let theo_warns = warn_ids(&report);
    for id in STUDY_DESIGN {
        assert!(
            !theo_warns.contains(*id),
            "theoretical finding should not warn on {id}, got {theo_warns:?}"
        );
    }

    // The default empirical finding (mechanism / experimental, in_vivo)
    // still gets the study-design checks — the gate stays meaningful where
    // a study-design dimension actually exists.
    let tmp2 = TempDir::new().unwrap();
    let path2 = tmp2.path().join("frontier.json");
    let emp = finding("vf_emp"); // assertion mechanism, evidence experimental
    let frontier2 = project::assemble("bio", vec![emp], 0, 0, "test");
    repo::save_to_path(&path2, &frontier2).unwrap();
    let report2 = evidence_ci::run_project(&repo::load_from_path(&path2).unwrap(), &path2);
    let emp_warns = warn_ids(&report2);
    assert!(
        emp_warns.contains("condition.comparator_or_baseline")
            || emp_warns.contains("condition.endpoint"),
        "empirical finding should still raise a study-design warning, got {emp_warns:?}"
    );
}

#[test]
fn v0_13_apply_materializes_source_records_inline() {
    // Pre-v0.13: vela check --strict on a CLI-built frontier flagged
    // `missing_source_record` because source_records weren't populated
    // until vela normalize --write — and normalize refuses on event-ful
    // frontiers. v0.13 materializes inline at apply time so source_records
    // grow in lockstep with findings.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    // Add a finding via the standard finding.add proposal flow.
    let f = finding("vf_v013_inline_src");
    let proposal = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: f.id.clone(),
        },
        "reviewer:test",
        "human",
        "Manual finding for v0.13 source-record materialization test",
        json!({"finding": f}),
        Vec::new(),
        Vec::new(),
    );
    create_and_accept_via_decision_plan(&path, proposal).unwrap();
    let loaded = repo::load_from_path(&path).unwrap();
    // Source records, evidence atoms, and condition records should all
    // be materialized — without any explicit normalize call.
    assert!(
        !loaded.sources.is_empty(),
        "v0.13: source_records should materialize inline at apply time"
    );
    assert!(
        !loaded.evidence_atoms.is_empty(),
        "v0.13: evidence_atoms should materialize inline at apply time"
    );
    assert!(
        !loaded.condition_records.is_empty(),
        "v0.13: condition_records should materialize inline at apply time"
    );
    // Sanity: stats reflect the new source registry.
    assert_eq!(loaded.stats.source_count, loaded.sources.len());
    // Suppress unused-mut warning when frontier isn't reused below.
    let _ = &mut frontier;
}

fn make_supersede_payload(old_id: &str, new_text: &str) -> (FindingBundle, Value) {
    let mut new_finding = finding("vf_supersede_new");
    new_finding.assertion.text = new_text.to_string();
    // Re-derive id from the new assertion text + provenance. For the
    // test we just hand-pick a distinct id; the real CLI uses
    // `build_finding_bundle` which content-addresses correctly.
    new_finding.id = format!(
        "vf_{:0>16}",
        old_id
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_add(b as u64))
    );
    let payload = json!({"new_finding": new_finding.clone()});
    (new_finding, payload)
}

#[test]
fn v0_14_supersede_creates_new_finding_and_marks_old() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut frontier = project::assemble("test", vec![finding("vf_old")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let (new_finding, payload) = make_supersede_payload("vf_old", "Newer claim");
    let proposal = new_proposal(
        "finding.supersede",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_old".to_string(),
        },
        "reviewer:test",
        "human",
        "Newer evidence updates the wording",
        payload,
        Vec::new(),
        Vec::new(),
    );
    let result = create_and_accept_via_decision_plan(&path, proposal).unwrap();
    assert!(result.applied_event_id.is_some());
    let loaded = repo::load_from_path(&path).unwrap();
    // Old finding now flagged superseded.
    let old = loaded.findings.iter().find(|f| f.id == "vf_old").unwrap();
    assert!(
        old.flags.superseded,
        "old finding should be flagged superseded"
    );
    // New finding present, with auto-injected supersedes link back to old.
    let new_f = loaded
        .findings
        .iter()
        .find(|f| f.id == new_finding.id)
        .expect("new finding should be in frontier");
    assert!(
        new_f
            .links
            .iter()
            .any(|l| l.target == "vf_old" && l.link_type == "supersedes"),
        "new finding should have an auto-injected supersedes link to old finding"
    );
    // Event with kind finding.superseded targeting old, payload carries new_finding_id.
    let supersede_event = loaded
        .events
        .iter()
        .find(|e| e.kind == "finding.superseded")
        .expect("a finding.superseded event should be emitted");
    assert_eq!(supersede_event.target.id, "vf_old");
    assert_eq!(
        supersede_event.payload["new_finding_id"].as_str(),
        Some(new_finding.id.as_str())
    );
    // suppress unused warning
    let _ = &mut frontier;
}

#[test]
fn v0_14_supersede_refuses_already_superseded() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let mut old = finding("vf_already_done");
    old.flags.superseded = true;
    let frontier = project::assemble("test", vec![old], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    let (_, payload) = make_supersede_payload("vf_already_done", "Newer wording");
    let proposal = new_proposal(
        "finding.supersede",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_already_done".to_string(),
        },
        "reviewer:test",
        "human",
        "Attempt to double-supersede",
        payload,
        Vec::new(),
        Vec::new(),
    );
    let result = create_and_accept_via_decision_plan(&path, proposal);
    assert!(
        result.is_err(),
        "double-supersede should be refused; got {result:?}"
    );
}

#[test]
fn v0_14_supersede_refuses_same_content_address() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontier.json");
    let frontier = project::assemble("test", vec![finding("vf_same")], 0, 0, "test");
    repo::save_to_path(&path, &frontier).unwrap();
    // new_finding.id == target.id should be refused at validate-time.
    let mut new_finding = finding("vf_same");
    new_finding.assertion.text = "Different text but reused id".to_string();
    let proposal = new_proposal(
        "finding.supersede",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_same".to_string(),
        },
        "reviewer:test",
        "human",
        "Same id, should fail",
        json!({"new_finding": new_finding}),
        Vec::new(),
        Vec::new(),
    );
    let result = create_and_accept_via_decision_plan(&path, proposal);
    assert!(
        result.is_err(),
        "supersede with same content address should be refused; got {result:?}"
    );
}

/// v0.22 byte-stability: a proposal with `agent_run = None`
/// must serialize without an `agent_run` field, so existing
/// frontiers (none of which have agent_run today) round-trip
/// byte-identically. The whole substrate guarantee depends on
/// canonical-JSON not silently gaining new keys.
#[test]
fn agent_run_none_skips_serialization() {
    let p = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test0000000000".to_string(),
        },
        "reviewer:will-blair",
        "human",
        "test",
        json!({}),
        Vec::new(),
        Vec::new(),
    );
    let bytes = canonical::to_canonical_bytes(&p).unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(
        !s.contains("agent_run"),
        "proposal without agent_run leaked the field into canonical JSON: {s}"
    );
}

/// And when `agent_run` *is* set, the same proposal id is
/// produced regardless — `proposal_id`'s preimage explicitly
/// excludes agent_run, so attaching provenance never changes
/// the content address.
#[test]
fn agent_run_does_not_change_proposal_id() {
    let bare = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test0000000000".to_string(),
        },
        "agent:literature-scout",
        "agent",
        "scout extracted this from paper_014",
        json!({}),
        vec!["src_paper_014".to_string()],
        Vec::new(),
    );
    let id_bare = bare.id.clone();

    let mut with_run = bare.clone();
    with_run.agent_run = Some(AgentRun {
        agent: "literature-scout".to_string(),
        model: "claude-opus-4-7".to_string(),
        run_id: "vrun_abc1234567890def".to_string(),
        started_at: "2026-04-26T01:23:45Z".to_string(),
        finished_at: Some("2026-04-26T01:24:10Z".to_string()),
        context: BTreeMap::from([
            ("input_folder".to_string(), "./papers".to_string()),
            ("pdf_count".to_string(), "12".to_string()),
        ]),
        tool_calls: Vec::new(),
        permissions: None,
    });
    let id_with_run = proposal_id(&with_run);
    assert_eq!(
        id_bare, id_with_run,
        "agent_run leaked into proposal_id preimage"
    );
}

/// v0.49 byte-stability: tool_calls and permissions on AgentRun
/// must skip serialization when empty/None, so existing frontiers
/// (none of which carry these fields today) round-trip byte-
/// identically through canonical JSON. Same invariant as
/// agent_run itself in v0.22.
#[test]
fn agent_run_empty_tool_calls_and_permissions_skip_serialization() {
    let p = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test0000000000".to_string(),
        },
        "agent:scout",
        "agent",
        "test",
        json!({}),
        Vec::new(),
        Vec::new(),
    );
    let mut with_run = p.clone();
    with_run.agent_run = Some(AgentRun {
        agent: "scout".to_string(),
        model: "claude-opus-4-7".to_string(),
        run_id: "vrun_x".to_string(),
        started_at: "2026-04-26T01:00:00Z".to_string(),
        finished_at: None,
        context: BTreeMap::new(),
        tool_calls: Vec::new(),
        permissions: None,
    });
    let bytes = canonical::to_canonical_bytes(&with_run).unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(
        !s.contains("tool_calls"),
        "empty tool_calls leaked into canonical JSON: {s}"
    );
    assert!(
        !s.contains("permissions"),
        "empty permissions leaked into canonical JSON: {s}"
    );
}

/// v0.49: when populated, tool_calls and permissions DO serialize
/// — this is the round-trip we want for new agent runs that
/// actually carry tool traces.
#[test]
fn agent_run_populated_tool_calls_and_permissions_roundtrip() {
    let mut p = new_proposal(
        "finding.add",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test0000000000".to_string(),
        },
        "agent:scout",
        "agent",
        "test",
        json!({}),
        Vec::new(),
        Vec::new(),
    );
    p.agent_run = Some(AgentRun {
        agent: "scout".to_string(),
        model: "claude-opus-4-7".to_string(),
        run_id: "vrun_x".to_string(),
        started_at: "2026-04-26T01:00:00Z".to_string(),
        finished_at: None,
        context: BTreeMap::new(),
        tool_calls: vec![
            ToolCallTrace {
                tool: "pubmed_search".to_string(),
                input_sha256: "a".repeat(64),
                output_sha256: Some("b".repeat(64)),
                at: "2026-04-26T01:00:05Z".to_string(),
                duration_ms: Some(842),
                status: "ok".to_string(),
                error_message: String::new(),
            },
            // v0.49: a failed tool call with an explanatory
            // error_message — the field a reviewer needs to audit
            // what went wrong without re-running the agent.
            ToolCallTrace {
                tool: "arxiv_fetch".to_string(),
                input_sha256: "c".repeat(64),
                output_sha256: None,
                at: "2026-04-26T01:00:18Z".to_string(),
                duration_ms: Some(1200),
                status: "error".to_string(),
                error_message: "HTTP 503 from arxiv.org; retry budget exhausted".to_string(),
            },
        ],
        permissions: Some(PermissionState {
            data_access: vec!["pubmed:".to_string(), "frontier:vfr_bd91".to_string()],
            tool_access: vec!["pubmed_search".to_string(), "arxiv_fetch".to_string()],
            note: "read-only access to BBB Flagship".to_string(),
        }),
    });
    let bytes = canonical::to_canonical_bytes(&p).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).expect("canonical bytes round-trip");
    assert_eq!(
        json["agent_run"]["tool_calls"][0]["tool"], "pubmed_search",
        "tool_calls did not survive the round trip: {json}"
    );
    assert_eq!(
        json["agent_run"]["permissions"]["data_access"][0], "pubmed:",
        "permissions did not survive the round trip: {json}"
    );
    // v0.49: a failed tool call with error_message carries the
    // explanation through canonical JSON. A reviewer can audit
    // exactly what failed without rerunning the agent.
    assert_eq!(
        json["agent_run"]["tool_calls"][1]["status"], "error",
        "failed tool call status did not survive: {json}"
    );
    assert_eq!(
        json["agent_run"]["tool_calls"][1]["error_message"],
        "HTTP 503 from arxiv.org; retry budget exhausted",
        "error_message did not survive the round trip: {json}"
    );
    // ...and successful calls still don't leak an empty
    // error_message into canonical bytes.
    let raw = std::str::from_utf8(&bytes).unwrap();
    let okay_call_block_end = raw.find("pubmed_search").unwrap();
    let until_first_call = &raw[..okay_call_block_end + 200];
    assert!(
        !until_first_call.contains("\"error_message\":\"\""),
        "successful tool call leaked an empty error_message: {until_first_call}"
    );
}

use crate::sign::ActorRecord;
use ed25519_dalek::SigningKey;

fn accept_actor(id: &str, pubkey_hex: &str) -> ActorRecord {
    ActorRecord {
        id: id.to_string(),
        public_key: pubkey_hex.to_string(),
        algorithm: "ed25519".to_string(),
        created_at: "2026-05-01T00:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    }
}

/// A frontier carrying one pending proposal targeting a finding,
/// plus the actors passed in. Returns (project, proposal).
fn frontier_with_proposal(actors: Vec<ActorRecord>) -> (Project, StateProposal) {
    let mut project =
        project::assemble("accept-gate", vec![finding("vf_target0000000")], 0, 0, "t");
    project.frontier_id = Some(VFR.to_string());
    let proposal = new_proposal(
        "finding.review",
        StateTarget {
            r#type: "finding".to_string(),
            id: "vf_target0000000".to_string(),
        },
        "agent:literature-scout",
        "agent",
        "Mouse-only evidence; recommend contested",
        json!({"status": "contested"}),
        Vec::new(),
        Vec::new(),
    );
    project.proposals.push(proposal.clone());
    project.actors = actors;
    (project, proposal)
}

#[test]
fn pending_insertion_matches_split_repository_proposal_id_order() {
    let (mut project, existing) = frontier_with_proposal(vec![]);
    let earlier = (0..1024)
        .map(|nonce| {
            new_proposal(
                "finding.review",
                StateTarget {
                    r#type: "finding".to_string(),
                    id: "vf_target0000000".to_string(),
                },
                "agent:literature-scout",
                "agent",
                format!("Canonical ordering fixture {nonce}"),
                json!({"status": "contested"}),
                Vec::new(),
                Vec::new(),
            )
        })
        .find(|proposal| proposal.id < existing.id)
        .expect("a deterministic fixture should sort before the existing proposal");
    let earlier_id = earlier.id.clone();

    insert_pending_in_frontier(&mut project, earlier).unwrap();

    assert_eq!(
        project
            .proposals
            .iter()
            .map(|proposal| proposal.id.as_str())
            .collect::<Vec<_>>(),
        vec![earlier_id.as_str(), existing.id.as_str()]
    );
}

const VFR: &str = "vfr_accept_gate_fixture";
const NOW: &str = "2026-05-29T00:00:00Z";

// ── Signed review events + decision parity ────────────────────────

#[test]
fn parity_flags_status_with_no_backing_event() {
    // Hand-edit a status to "rejected" with no event behind it — the
    // exact tamper the mutable field used to allow silently.
    let (mut project, proposal) = frontier_with_proposal(vec![]);
    let idx = project
        .proposals
        .iter()
        .position(|p| p.id == proposal.id)
        .unwrap();
    project.proposals[idx].status = "rejected".to_string();
    project.proposals[idx].reviewed_by = Some("reviewer:ghost".to_string());
    let conflicts = verify_proposal_decision_parity(&project);
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.contains("NO decision event")),
        "{conflicts:?}"
    );
}

#[test]
fn review_event_targeting_missing_proposal_is_flagged() {
    let (mut project, _proposal) = frontier_with_proposal(vec![]);
    let orphan = events::new_review_decision_event(
        "vpr_does_not_exist",
        "finding.add",
        "rejected",
        None,
        "reviewer:x",
        "orphan",
        Some("2026-06-01T00:00:00Z"),
    )
    .unwrap();
    project.events.push(orphan);
    let conflicts = verify_proposal_decision_parity(&project);
    assert!(
        conflicts.iter().any(|c| c.contains("does not exist")),
        "an orphan review event must be flagged: {conflicts:?}"
    );
}

#[test]
fn parity_flags_hand_edited_terminal_decision_fields() {
    let (mut project, proposal) = frontier_with_proposal(vec![]);
    let decided_at = "2026-06-01T00:00:00Z";
    let reason = "exact reviewer reason";
    let event = events::new_review_decision_event(
        &proposal.id,
        &proposal.kind,
        "rejected",
        None,
        "reviewer:x",
        reason,
        Some(decided_at),
    )
    .unwrap();
    let stored = project
        .proposals
        .iter_mut()
        .find(|candidate| candidate.id == proposal.id)
        .unwrap();
    stored.status = "rejected".to_string();
    stored.reviewed_by = Some("reviewer:x".to_string());
    stored.reviewed_at = Some(decided_at.to_string());
    stored.decision_reason = Some(reason.to_string());
    project.events.push(event);
    assert!(verify_proposal_decision_parity(&project).is_empty());

    project.proposals[0].decision_reason = Some("hand-edited reason".to_string());
    let conflicts = verify_proposal_decision_parity(&project);
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.contains("stored decision fields")),
        "{conflicts:?}"
    );
}

// -- ADR 0003 Slice 4: exact decision binding -----------------------------

fn fixed_decision_fixture() -> (Project, StateProposal, SigningKey) {
    let key = SigningKey::from_bytes(&[11_u8; 32]);
    let pubkey = crate::sign::pubkey_hex(&key);
    let (project, proposal) = frontier_with_proposal(vec![accept_actor("reviewer:will", &pubkey)]);
    (project, proposal, key)
}

fn clone_project_for_decision(project: &Project) -> Project {
    serde_json::from_value(serde_json::to_value(project).unwrap()).unwrap()
}

#[test]
fn public_accept_preparation_rejects_legacy_retirement_while_preview_remains_pure() {
    const DECIDED_AT: &str = "2026-07-14T12:34:56Z";
    let key = SigningKey::from_bytes(&[73_u8; 32]);
    let mut project = project::assemble("legacy-retirement-seam", vec![], 0, 0, "test");
    project.actors = vec![accept_actor(
        "reviewer:will",
        &crate::sign::pubkey_hex(&key),
    )];
    let proposal = new_proposal_at(
        policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
        StateTarget {
            r#type: "governance".to_string(),
            id: project.frontier_id().to_string(),
        },
        "agent:test",
        "agent",
        "retire unsupported prelaunch policy bytes",
        serde_json::to_value(policy_accept::LegacyPolicyRetirementPayload {
            schema: policy_accept::LEGACY_POLICY_RETIREMENT_SCHEMA.to_string(),
            policy_id: "vap_e0abc750544408e637bd90e0661bac15".to_string(),
            policy_bytes_root: format!("sha256:{}", "a".repeat(64)),
            signature_bytes_root: format!("sha256:{}", "b".repeat(64)),
            retire_identical_snapshot_pair: true,
        })
        .unwrap(),
        vec![],
        vec![],
        "2026-07-14T11:00:00Z",
    );
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);
    let before = serde_json::to_value(&project).unwrap();

    let error = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal_id,
        "reviewer:will",
        "The exact legacy bytes are obsolete",
        None,
        DECIDED_AT,
    )
    .unwrap_err();
    assert!(error.contains("public protocol preparation API"), "{error}");
    assert_eq!(serde_json::to_value(&project).unwrap(), before);

    let error = prepare_proposal_accept_candidate_at(
        &project,
        &proposal_id,
        "reviewer:will",
        "The exact legacy bytes are obsolete",
        None,
        DECIDED_AT,
    )
    .unwrap_err();
    assert!(error.contains("public protocol preparation API"), "{error}");
    assert_eq!(serde_json::to_value(&project).unwrap(), before);

    let temp = TempDir::new().unwrap();
    preview_engine_verdict_in_frontier(&project, temp.path(), &proposal_id, false).unwrap();
    assert_eq!(serde_json::to_value(&project).unwrap(), before);
}

#[test]
fn prepared_accept_is_fixed_time_root_bound_and_exactly_signed() {
    const DECIDED_AT: &str = "2026-07-14T12:34:56Z";
    let root_a = format!("sha256:{}", "a".repeat(64));
    let root_b = format!("sha256:{}", "b".repeat(64));
    let (base, proposal, key) = fixed_decision_fixture();
    let pubkey = crate::sign::pubkey_hex(&key);

    let mut first = clone_project_for_decision(&base);
    let mut prepared = prepare_proposal_accept_in_memory_at(
        &mut first,
        &proposal.id,
        "reviewer:will",
        "Evidence and caveats checked",
        None,
        DECIDED_AT,
    )
    .unwrap();
    assert_eq!(prepared.appended_event_ids.len(), 2);
    assert_ne!(prepared.primary_event_id, prepared.decision_event_id);
    assert!(prepared.appended_event_ids.iter().all(|id| {
        first
            .events
            .iter()
            .find(|event| &event.id == id)
            .unwrap()
            .timestamp
            == DECIDED_AT
    }));
    let domain_event_id = prepared.primary_event_id.clone();
    assert_eq!(
        first
            .proposals
            .iter()
            .find(|candidate| candidate.id == proposal.id)
            .unwrap()
            .applied_event_id
            .as_deref(),
        Some(domain_event_id.as_str())
    );

    bind_decision_root_to_prepared(&mut first, &mut prepared, &root_a).unwrap();
    let bound_review_id = prepared.decision_event_id.clone();
    assert_eq!(prepared.primary_event_id, domain_event_id);
    assert_eq!(
        first
            .proposals
            .iter()
            .find(|candidate| candidate.id == proposal.id)
            .unwrap()
            .applied_event_id
            .as_deref(),
        Some(domain_event_id.as_str()),
        "ordinary accepts keep the domain event as applied_event_id"
    );
    bind_decision_root_to_prepared(&mut first, &mut prepared, &root_a).unwrap();
    assert_eq!(
        prepared.decision_event_id, bound_review_id,
        "rebinding the same root is idempotent"
    );

    let mut same = clone_project_for_decision(&base);
    let mut same_prepared = prepare_proposal_accept_in_memory_at(
        &mut same,
        &proposal.id,
        "reviewer:will",
        "Evidence and caveats checked",
        None,
        DECIDED_AT,
    )
    .unwrap();
    bind_decision_root_to_prepared(&mut same, &mut same_prepared, &root_a).unwrap();
    assert_eq!(prepared, same_prepared);
    let first_events = first
        .events
        .iter()
        .filter(|event| prepared.appended_event_ids.contains(&event.id))
        .map(|event| serde_json::to_value(event).unwrap())
        .collect::<Vec<_>>();
    let same_events = same
        .events
        .iter()
        .filter(|event| same_prepared.appended_event_ids.contains(&event.id))
        .map(|event| serde_json::to_value(event).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(first_events, same_events);

    let mut different = base;
    let mut different_prepared = prepare_proposal_accept_in_memory_at(
        &mut different,
        &proposal.id,
        "reviewer:will",
        "Evidence and caveats checked",
        None,
        DECIDED_AT,
    )
    .unwrap();
    bind_decision_root_to_prepared(&mut different, &mut different_prepared, &root_b).unwrap();
    assert_eq!(different_prepared.primary_event_id, domain_event_id);
    assert_ne!(
        different_prepared.decision_event_id,
        prepared.decision_event_id
    );

    sign_prepared_decision_events(&mut first, &prepared, "reviewer:will", &key).unwrap();
    for event_id in &prepared.appended_event_ids {
        let event = first
            .events
            .iter()
            .find(|event| &event.id == event_id)
            .unwrap();
        assert!(event.signature.is_some());
        assert!(crate::sign::verify_event_signature(event, &pubkey).unwrap());
    }
    assert!(verify_proposal_decision_parity(&first).is_empty());

    let decision = first
        .events
        .iter_mut()
        .find(|event| event.id == prepared.decision_event_id)
        .unwrap();
    decision.payload["provenance"]["input_refs"][0] =
        json!(format!("urn:vela:decision-root:{root_b}"));
    decision.id = events::compute_event_id(decision);
    assert!(
        !crate::sign::verify_event_signature(decision, &pubkey).unwrap(),
        "changing the signed decision-root reference must invalidate the signature"
    );
}

#[test]
fn prepared_reject_is_fixed_time_root_bound_and_exactly_signed() {
    const DECIDED_AT: &str = "2026-07-14T12:34:56Z";
    let root = format!("sha256:{}", "c".repeat(64));
    let (mut project, proposal, key) = fixed_decision_fixture();
    let pubkey = crate::sign::pubkey_hex(&key);
    let mut prepared = prepare_proposal_reject_in_memory_at(
        &mut project,
        &proposal.id,
        "reviewer:will",
        "Evidence is insufficient",
        None,
        DECIDED_AT,
    )
    .unwrap();
    assert_eq!(prepared.appended_event_ids.len(), 1);
    assert_eq!(prepared.primary_event_id, prepared.decision_event_id);
    bind_decision_root_to_prepared(&mut project, &mut prepared, &root).unwrap();
    assert_eq!(prepared.primary_event_id, prepared.decision_event_id);
    let event = project
        .events
        .iter()
        .find(|event| event.id == prepared.decision_event_id)
        .unwrap();
    assert_eq!(event.timestamp, DECIDED_AT);
    sign_prepared_decision_events(&mut project, &prepared, "reviewer:will", &key).unwrap();
    let event = project
        .events
        .iter()
        .find(|event| event.id == prepared.decision_event_id)
        .unwrap();
    assert!(crate::sign::verify_event_signature(event, &pubkey).unwrap());
    assert!(verify_proposal_decision_parity(&project).is_empty());
}

#[test]
fn prepared_accept_error_is_atomic_even_after_candidate_mutation() {
    let (mut project, proposal, _key) = fixed_decision_fixture();
    let before = serde_json::to_value(&project).unwrap();
    let invalid_provenance = crate::provenance::Provenance {
        machine_contributions: vec![crate::provenance::MachineContribution {
            id: "reviewer:smuggled-human".to_string(),
            authority: "none".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal.id,
        "reviewer:will",
        "would mutate the candidate before provenance attachment",
        Some(&invalid_provenance),
        NOW,
    )
    .unwrap_err();
    assert!(error.contains("may not name a human"));
    assert_eq!(
        serde_json::to_value(&project).unwrap(),
        before,
        "a failed preparation must leave the caller's canonical input untouched"
    );
}

#[test]
fn prepared_signing_rejects_unbound_and_structurally_forged_handles() {
    let (base, proposal, key) = fixed_decision_fixture();
    let mut unbound_project = clone_project_for_decision(&base);
    let unbound = prepare_proposal_accept_in_memory_at(
        &mut unbound_project,
        &proposal.id,
        "reviewer:will",
        "checked",
        None,
        NOW,
    )
    .unwrap();
    let error =
        sign_prepared_decision_events(&mut unbound_project, &unbound, "reviewer:will", &key)
            .unwrap_err();
    assert!(error.contains("must be decision-root bound"));
    assert!(unbound_project.events.iter().all(|event| {
        !unbound.appended_event_ids().contains(&event.id) || event.signature.is_none()
    }));

    let mut forged_project = clone_project_for_decision(&base);
    let mut forged = prepare_proposal_accept_in_memory_at(
        &mut forged_project,
        &proposal.id,
        "reviewer:will",
        "checked",
        None,
        NOW,
    )
    .unwrap();
    bind_decision_root_to_prepared(
        &mut forged_project,
        &mut forged,
        &format!("sha256:{}", "e".repeat(64)),
    )
    .unwrap();
    forged.appended_event_ids.reverse();
    let error = sign_prepared_decision_events(&mut forged_project, &forged, "reviewer:will", &key)
        .unwrap_err();
    assert!(error.contains("contiguous event range"));
    assert!(
        forged_project
            .events
            .iter()
            .all(|event| event.signature.is_none())
    );

    let mut duplicate_root_project = clone_project_for_decision(&base);
    let mut duplicate_root = prepare_proposal_accept_in_memory_at(
        &mut duplicate_root_project,
        &proposal.id,
        "reviewer:will",
        "checked",
        None,
        NOW,
    )
    .unwrap();
    bind_decision_root_to_prepared(
        &mut duplicate_root_project,
        &mut duplicate_root,
        &format!("sha256:{}", "f".repeat(64)),
    )
    .unwrap();
    let old_id = duplicate_root.decision_event_id.clone();
    let event = duplicate_root_project
        .events
        .iter_mut()
        .find(|event| event.id == old_id)
        .unwrap();
    event.payload["provenance"]["input_refs"]
        .as_array_mut()
        .unwrap()
        .push(json!(format!(
            "urn:vela:decision-root:sha256:{}",
            "1".repeat(64)
        )));
    event.id = events::compute_event_id(event);
    let new_id = event.id.clone();
    duplicate_root
        .appended_event_ids
        .iter_mut()
        .filter(|id| **id == old_id)
        .for_each(|id| *id = new_id.clone());
    duplicate_root.decision_event_id = new_id;
    let error = sign_prepared_decision_events(
        &mut duplicate_root_project,
        &duplicate_root,
        "reviewer:will",
        &key,
    )
    .unwrap_err();
    assert!(error.contains("exactly one canonical decision-root"));
    assert!(
        duplicate_root_project
            .events
            .iter()
            .all(|event| event.signature.is_none())
    );
}

#[test]
fn prepared_decisions_fix_nested_state_and_fanout_times() {
    const DECIDED_AT: &str = "2026-07-14T12:34:56Z";
    let key = SigningKey::from_bytes(&[13_u8; 32]);
    let actor = accept_actor("reviewer:will", &crate::sign::pubkey_hex(&key));

    let mut note_project = project::assemble("fixed-note", vec![finding("vf_note")], 0, 0, "t");
    note_project.actors = vec![actor.clone()];
    let note = new_proposal_at(
        "finding.note",
        StateTarget {
            r#type: "finding".into(),
            id: "vf_note".into(),
        },
        "agent:drafter",
        "agent",
        "record scope",
        json!({"text": "mouse-only"}),
        Vec::new(),
        Vec::new(),
        "2026-07-14T00:00:00Z",
    );
    note_project.proposals.push(note.clone());
    let note_prepared = prepare_proposal_accept_in_memory_at(
        &mut note_project,
        &note.id,
        "reviewer:will",
        "Scoped correctly",
        None,
        DECIDED_AT,
    )
    .unwrap();
    assert_eq!(
        note_project.findings[0].annotations[0].timestamp,
        DECIDED_AT
    );
    assert!(note_prepared.appended_event_ids.iter().all(|id| {
        note_project
            .events
            .iter()
            .find(|event| &event.id == id)
            .unwrap()
            .timestamp
            == DECIDED_AT
    }));

    let mut supersede_project =
        project::assemble("fixed-supersede", vec![finding("vf_old")], 0, 0, "t");
    supersede_project.actors = vec![actor.clone()];
    let (_, payload) = make_supersede_payload("vf_old", "Corrected claim");
    let supersede = new_proposal_at(
        "finding.supersede",
        StateTarget {
            r#type: "finding".into(),
            id: "vf_old".into(),
        },
        "agent:drafter",
        "agent",
        "correct wording",
        payload,
        Vec::new(),
        Vec::new(),
        "2026-07-14T00:00:00Z",
    );
    supersede_project.proposals.push(supersede.clone());
    let supersede_prepared = prepare_proposal_accept_in_memory_at(
        &mut supersede_project,
        &supersede.id,
        "reviewer:will",
        "Correction checked",
        None,
        DECIDED_AT,
    )
    .unwrap();
    let new_finding = supersede_project
        .findings
        .iter()
        .find(|candidate| candidate.id != "vf_old")
        .unwrap();
    assert_eq!(new_finding.links[0].created_at, DECIDED_AT);
    assert!(supersede_prepared.appended_event_ids.iter().all(|id| {
        supersede_project
            .events
            .iter()
            .find(|event| &event.id == id)
            .unwrap()
            .timestamp
            == DECIDED_AT
    }));

    let mut confidence_project =
        project::assemble("fixed-confidence", vec![finding("vf_conf")], 0, 0, "t");
    confidence_project.actors = vec![actor];
    let revise = new_proposal_at(
        "finding.confidence_revise",
        StateTarget {
            r#type: "finding".into(),
            id: "vf_conf".into(),
        },
        "agent:drafter",
        "agent",
        "new bound",
        json!({"confidence": 0.4}),
        Vec::new(),
        Vec::new(),
        "2026-07-14T00:00:00Z",
    );
    confidence_project.proposals.push(revise.clone());
    let confidence_prepared = prepare_proposal_accept_in_memory_at(
        &mut confidence_project,
        &revise.id,
        "reviewer:will",
        "Bound checked",
        None,
        DECIDED_AT,
    )
    .unwrap();
    assert_eq!(
        confidence_project.findings[0].updated.as_deref(),
        Some(DECIDED_AT)
    );
    assert!(confidence_prepared.appended_event_ids.iter().all(|id| {
        confidence_project
            .events
            .iter()
            .find(|event| &event.id == id)
            .unwrap()
            .timestamp
            == DECIDED_AT
    }));
}

#[test]
fn prepared_decision_authority_is_registered_active_and_role_bound() {
    let key = SigningKey::from_bytes(&[17_u8; 32]);
    let pubkey = crate::sign::pubkey_hex(&key);
    let (mut project, proposal) = frontier_with_proposal(Vec::new());
    let error = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal.id,
        "reviewer:missing",
        "check",
        None,
        NOW,
    )
    .unwrap_err();
    assert!(error.contains("not registered"));

    let mut wrong_role = accept_actor("operator:writer", &pubkey);
    wrong_role.tier = Some("historical-tier".to_string());
    project.actors = vec![wrong_role];
    let error = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal.id,
        "operator:writer",
        "check",
        None,
        NOW,
    )
    .unwrap_err();
    assert!(error.contains("does not carry"));

    project.actors = vec![
        accept_actor("reviewer:will", &pubkey),
        accept_actor("reviewer:will", &pubkey),
    ];
    let error = validate_human_reviewer_authority_at(&project, "reviewer:will", NOW).unwrap_err();
    assert!(error.contains("ambiguously"));

    let mut not_yet_registered = accept_actor("reviewer:will", &pubkey);
    not_yet_registered.created_at = "2026-05-30T00:00:00Z".to_string();
    project.actors = vec![not_yet_registered];
    let error = validate_human_reviewer_authority_at(&project, "reviewer:will", NOW).unwrap_err();
    assert!(error.contains("not yet registered"));

    let mut future_revocation = accept_actor("reviewer:will", &pubkey);
    future_revocation.revoked_at = Some("2026-05-29T00:00:01Z".to_string());
    project.actors = vec![future_revocation];
    validate_human_reviewer_authority_at(&project, "reviewer:will", NOW).unwrap();

    let mut equivalent_offset_revocation = accept_actor("reviewer:will", &pubkey);
    equivalent_offset_revocation.revoked_at = Some("2026-05-28T20:00:00-04:00".to_string());
    project.actors = vec![equivalent_offset_revocation];
    let error = validate_human_reviewer_authority_at(&project, "reviewer:will", NOW).unwrap_err();
    assert!(error.contains("revoked"));

    let mut revoked = accept_actor("reviewer:will", &pubkey);
    revoked.revoked_at = Some("2026-05-10T00:00:00Z".to_string());
    project.actors = vec![revoked];
    let error = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal.id,
        "reviewer:will",
        "check",
        None,
        NOW,
    )
    .unwrap_err();
    assert!(error.contains("revoked"));
}

#[test]
fn policy_head_preparation_allows_future_revocation_and_updates_primary_cache() {
    let key = SigningKey::from_bytes(&[19_u8; 32]);
    let pubkey = crate::sign::pubkey_hex(&key);
    let mut actor = accept_actor("reviewer:will", &pubkey);
    actor.revoked_at = Some("2026-06-01T00:00:00Z".to_string());
    let mut project = project::assemble("policy", vec![finding("vf_policy")], 0, 0, "t");
    project.frontier_id = Some("vfr_policy_fixture".to_string());
    project.actors = vec![actor];
    project.events[0].timestamp = "2026-05-10T00:00:00Z".to_string();
    project.events[0].id = events::compute_event_id(&project.events[0]);
    let parent_event_ids = project
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let payload = policy_accept::PolicyHeadPayload {
        schema: policy_accept::POLICY_HEAD_SCHEMA.to_string(),
        action: policy_accept::PolicyHeadAction::Activate,
        policy_id: Some("vap_policy_fixture".to_string()),
        prior_head_event_id: None,
        expected_parent_event_log_root: format!(
            "sha256:{}",
            events::event_log_hash(&project.events)
        ),
        parent_event_ids,
        epoch: 1,
    };
    let proposal = new_proposal_at(
        policy_accept::POLICY_HEAD_PROPOSAL_KIND,
        StateTarget {
            r#type: "governance".to_string(),
            id: "vfr_policy_fixture".to_string(),
        },
        "agent:policy-drafter",
        "agent",
        "activate the reviewed policy",
        serde_json::to_value(payload).unwrap(),
        Vec::new(),
        Vec::new(),
        "2026-05-20T00:00:00Z",
    );
    project.proposals.push(proposal.clone());
    let mut prepared = prepare_proposal_accept_in_memory_at(
        &mut project,
        &proposal.id,
        "reviewer:will",
        "activate policy",
        None,
        NOW,
    )
    .unwrap();
    let unbound_id = prepared.decision_event_id().to_string();
    bind_decision_root_to_prepared(
        &mut project,
        &mut prepared,
        &format!("sha256:{}", "d".repeat(64)),
    )
    .unwrap();
    assert_ne!(prepared.decision_event_id(), unbound_id);
    assert_eq!(prepared.primary_event_id(), prepared.decision_event_id());
    assert_eq!(
        project
            .proposals
            .iter()
            .find(|candidate| candidate.id == proposal.id)
            .unwrap()
            .applied_event_id
            .as_deref(),
        Some(prepared.decision_event_id())
    );
}
