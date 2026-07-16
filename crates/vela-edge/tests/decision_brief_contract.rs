use std::path::{Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_edge::decision_brief::{
    DecisionBrief, DecisionBriefInput, PublicationProjection, ReceiptMaterial, ReviewPolicyFacts,
    ReviewRoute, build_decision_brief,
};
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, Constraints, Outcome, PolicyRule, PolicySignatureRecord, Quorum,
};
use vela_protocol::bundle::Link;
use vela_protocol::contradiction::Contradiction;
use vela_protocol::events::{self, StateTarget};
use vela_protocol::identity::{ActorClass, IdentityBinding};
use vela_protocol::project::{self, AttemptClaim, Project};
use vela_protocol::proposals;
use vela_protocol::proposals::policy_accept::{PermitReadiness, PolicyState};
use vela_protocol::receipt_v1::{
    ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput, ReceiptV1,
    ScientificChainAssertion,
};
use vela_protocol::sign::ActorRecord;
use vela_protocol::test_support::{make_finding, make_project};

const OBSERVED_AT: &str = "2026-07-13T13:00:00Z";
const CREATED_AT: &str = "2026-07-13T12:35:00Z";
const FIXTURE_ACTOR: &str = "agent:decision-brief-fixture";
const RECEIPT_ACTOR: &str = "agent:decision-brief-test";
const RECEIPT_OPERATION_ID: &str =
    "vop_cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn absent_policy_facts() -> ReviewPolicyFacts<'static> {
    ReviewPolicyFacts::new(PolicyState::Absent, PermitReadiness::HumanOnly, &[], None)
}

fn fixed_project(name: &str, findings: Vec<vela_protocol::bundle::FindingBundle>) -> Project {
    let mut project = make_project(name, findings);
    project.project.compiled_at = "2026-07-13T12:00:00Z".to_string();
    project.project.compiler = "vela/decision-brief-fixture.v1".to_string();

    let genesis = project
        .events
        .first_mut()
        .expect("test-support projects have a genesis event");
    genesis.timestamp = "2026-07-13T12:00:00Z".to_string();
    genesis.actor.id = "vela/decision-brief-fixture.v1".to_string();
    genesis.payload["compiled_at"] = json!("2026-07-13T12:00:00Z");
    genesis.payload["creator"] = json!("vela/decision-brief-fixture.v1");
    genesis.payload["schema_version"] = json!("decision-brief-fixture.v1");
    genesis.id = events::compute_event_id(genesis);
    project.frontier_id = project::frontier_id_from_genesis(&project.events);
    project
}

fn finding_value(id: &str, assertion_type: &str, claim: &str) -> Value {
    let mut value = serde_json::to_value(make_finding(id, 0.3, assertion_type)).unwrap();
    value["assertion"]["text"] = json!(claim);
    value
}

fn install_proposal(
    project: &mut Project,
    kind: &str,
    target_id: &str,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
    reason: &str,
) -> String {
    let proposal = proposals::new_proposal_at(
        kind,
        StateTarget {
            r#type: "finding".to_string(),
            id: target_id.to_string(),
        },
        FIXTURE_ACTOR,
        "agent",
        reason,
        payload,
        source_refs,
        caveats,
        CREATED_AT,
    );
    let id = proposal.id.clone();
    project.proposals.push(proposal);
    id
}

fn frozen_receipt_identity() -> IdentityBinding {
    // Public test vector copied from the protocol's Receipt builder tests.
    // The Decision Brief builder receives only the validated Receipt; no key
    // or signing capability is present on its review path.
    IdentityBinding {
        schema: "vela.identity_binding.v0.1".to_string(),
        binding_id: "vib_7067542ae284b71a".to_string(),
        actor_id: RECEIPT_ACTOR.to_string(),
        actor_class: ActorClass::Agent,
        public_key_hex: "fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618"
            .to_string(),
        created_at: "2026-07-13T12:00:00Z".to_string(),
        signature: "cb5dda1a80e38de6b023f1ddc9346d77dc112d1fa38c61512b10057822432908a076bd08509e965b927dd6a0d04f83e9f952a78cf5a5b762bacc574b06bf2b05"
            .to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn receipt_bound_brief(
    mut project: Project,
    proposal_kind: &str,
    target: &str,
    claim: &str,
    claim_type: &str,
    replayability: &str,
    artifacts: Vec<ArtifactInput>,
    caveats: Vec<String>,
    producer_runs: Vec<ProducerReportedRun>,
    route_code: &str,
    route_detail: &str,
) -> DecisionBrief {
    let event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let receipt = ReceiptBuilder::build(
        ReceiptInput::new(
            claim.to_string(),
            claim_type.to_string(),
            replayability.to_string(),
            artifacts,
            caveats.clone(),
            producer_runs,
            RECEIPT_ACTOR.to_string(),
            "2026-07-13T12:34:56Z".to_string(),
            event_log_root,
            ".".to_string(),
            RECEIPT_OPERATION_ID.to_string(),
            "urn:vela:policy:none".to_string(),
        )
        .unwrap(),
        &frozen_receipt_identity(),
    )
    .unwrap();
    receipt_bound_brief_from_receipt(
        &mut project,
        proposal_kind,
        target,
        claim,
        claim_type,
        caveats,
        receipt,
        route_code,
        route_detail,
    )
}

#[allow(clippy::too_many_arguments)]
fn receipt_bound_brief_from_receipt(
    project: &mut Project,
    proposal_kind: &str,
    target: &str,
    claim: &str,
    claim_type: &str,
    caveats: Vec<String>,
    receipt: ReceiptV1,
    route_code: &str,
    route_detail: &str,
) -> DecisionBrief {
    let receipt_root = receipt.canonical_root().unwrap();
    let receipt_digest = receipt_root.strip_prefix("sha256:").unwrap();
    let proposal = proposals::new_proposal_at(
        proposal_kind,
        StateTarget {
            r#type: "finding".to_string(),
            id: target.to_string(),
        },
        RECEIPT_ACTOR,
        "agent",
        "route adversarial evidence to a human reviewer",
        json!({
            "finding": finding_value(target, claim_type, claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": receipt_root,
                "receipt_path": format!("records/receipts/sha256/{receipt_digest}.json"),
                "record_id": format!("vrc_{receipt_digest}"),
                "operation_id": RECEIPT_OPERATION_ID,
            }
        }),
        vec!["urn:source:adversarial-review".to_string()],
        caveats,
        CREATED_AT,
    );
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);

    build_decision_brief(
        project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::from_receipt(&receipt),
            route: ReviewRoute::human_only(absent_policy_facts(), route_code, route_detail),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn ordinary_brief() -> DecisionBrief {
    let target = "vf_decision_brief_ordinary";
    let claim = "A bounded computational note is ready for human review.";
    let mut project = fixed_project("Decision brief ordinary fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({"finding": finding_value(target, "computational", claim)}),
        vec!["urn:source:ordinary".to_string()],
        vec!["The note is scoped to the declared bounded case.".to_string()],
        "record a bounded note",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_applicable"),
            route: ReviewRoute::human_only(
                absent_policy_facts(),
                "proposal_kind_requires_human_review",
                "this proposal kind is intentionally reviewed by a human",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn critical_warning_brief() -> DecisionBrief {
    let target = "vf_decision_brief_contested";
    let claim = "The contested claim should remain visible during review.";
    let mut existing = make_finding(target, 0.3, "computational");
    existing.flags.contested = true;
    let mut project = fixed_project("Decision brief critical warning fixture", vec![existing]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({"finding": finding_value(target, "computational", claim)}),
        vec!["urn:source:contested-review".to_string()],
        vec!["An active challenge must be resolved by the reviewer.".to_string()],
        "record review context without resolving the challenge",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_applicable"),
            route: ReviewRoute::human_only(
                absent_policy_facts(),
                "active_challenge_requires_human_review",
                "the active challenge requires an explicit human decision",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn missing_brief() -> DecisionBrief {
    let target = "vf_decision_brief_missing";
    let claim = "A receipt-bound computational finding is proposed.";
    let declared_root = format!("sha256:{}", "4".repeat(64));
    let mut project = fixed_project("Decision brief missing fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.add",
        target,
        json!({
            "finding": finding_value(target, "computational", claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": declared_root,
                "receipt_path": format!("records/receipts/sha256/{}.json", "4".repeat(64)),
                "record_id": "vrc_decision_brief_missing",
                "operation_id": format!("vop_{}", "5".repeat(64))
            }
        }),
        vec!["urn:source:missing-receipt".to_string()],
        vec!["The receipt must be recovered before acceptance.".to_string()],
        "land a receipt-bound finding for review",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_found"),
            route: ReviewRoute::unavailable(
                absent_policy_facts(),
                "receipt_material_unavailable",
                "the coherent policy route could not be reconstructed",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn restricted_evidence_brief() -> DecisionBrief {
    let target = "vf_decision_brief_restricted";
    let claim = "A theoretical claim depends on evidence unavailable in this review context.";
    let declared_root = format!("sha256:{}", "6".repeat(64));
    // The locator is a safe descriptor, not restricted content. Its length
    // exercises the bounded rendering while raw_references_root binds the
    // complete source value.
    let restricted_locator = format!(
        "restricted://review-vault/safe-descriptor/{}",
        "bounded-segment-".repeat(40)
    );
    let mut project = fixed_project("Decision brief restricted evidence fixture", vec![]);
    let proposal_id = install_proposal(
        &mut project,
        "finding.add",
        target,
        json!({
            "finding": finding_value(target, "theoretical", claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": declared_root,
                "receipt_path": format!("records/receipts/sha256/{}.json", "6".repeat(64)),
                "record_id": "vrc_decision_brief_restricted",
                "operation_id": format!("vop_{}", "7".repeat(64))
            }
        }),
        vec![restricted_locator],
        vec!["Evidence access is restricted; no evidentiary authority is inferred.".to_string()],
        "route restricted evidence to an authorized human reviewer",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("restricted_evidence_not_available_to_reviewer"),
            route: ReviewRoute::human_only(
                absent_policy_facts(),
                "restricted_evidence_requires_authorized_human_review",
                "restricted evidence requires an authorized human review",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn build_statement_fidelity_brief() -> DecisionBrief {
    let target = "vf_decision_brief_build_statement_fidelity";
    let claim = "The Lean theorem faithfully formalizes the intended informal statement.";
    receipt_bound_brief(
        fixed_project("Decision brief build versus statement fixture", vec![]),
        "finding.note",
        target,
        claim,
        "theoretical",
        "exact",
        vec![
            ArtifactInput::new(
                "proofs/fidelity.olean".to_string(),
                "proof".to_string(),
                Some("8".repeat(64)),
                None,
            )
            .unwrap(),
        ],
        vec![
            "A passing kernel build establishes type checking, not fidelity to the intended informal statement."
                .to_string(),
        ],
        vec![
            ProducerReportedRun::producer_reported(
                "lean build --frozen".to_string(),
                "pass".to_string(),
            )
            .unwrap(),
        ],
        "statement_fidelity_requires_human_review",
        "theoretical statement fidelity has no independent attestation",
    )
}

fn vacuity_tautology_brief() -> DecisionBrief {
    let target = "vf_decision_brief_vacuity";
    let formalization_ref = "vlv_vacuity_probe";
    let claim = "The formal theorem establishes the intended conditional claim.";
    let mut existing = make_finding(target, 0.3, "theoretical");
    existing.assertion.text = claim.to_string();
    let mut project = fixed_project("Decision brief vacuity fixture", vec![existing]);
    project
        .contradictions
        .push(Contradiction::from_misformalization(
            project.frontier_id.as_deref().unwrap(),
            target,
            formalization_ref,
            "The adversarial probe proved only x = x and used none of the intended hypotheses.",
        ));

    receipt_bound_brief(
        project,
        "finding.note",
        target,
        claim,
        "theoretical",
        "exact",
        vec![
            ArtifactInput::new(
                "proofs/vacuity.olean".to_string(),
                "proof".to_string(),
                Some("9".repeat(64)),
                None,
            )
            .unwrap(),
        ],
        vec![
            "The current proof closes x = x without using the hypotheses of the intended theorem."
                .to_string(),
        ],
        vec![
            ProducerReportedRun::producer_reported(
                "lean build --frozen".to_string(),
                "pass".to_string(),
            )
            .unwrap(),
        ],
        "misformalization_candidate_requires_human_review",
        "a vacuity probe raised an unadjudicated formalism-fidelity contradiction",
    )
}

fn producer_trusting_partial_verifier_brief() -> DecisionBrief {
    let target = "vf_decision_brief_partial_verifier";
    let claim = "The submitted result has been independently recomputed.";
    receipt_bound_brief(
        fixed_project("Decision brief partial verifier fixture", vec![]),
        "finding.note",
        target,
        claim,
        "computational",
        "bounded",
        vec![
            ArtifactInput::new(
                "witnesses/partial-check.json".to_string(),
                "witness".to_string(),
                Some("a".repeat(64)),
                None,
            )
            .unwrap(),
        ],
        vec![
            "The producer check trusts producer-supplied inputs and does not independently recompute the result."
                .to_string(),
        ],
        vec![
            ProducerReportedRun::producer_reported(
                "producer.partial-check --trust-inputs".to_string(),
                "pass".to_string(),
            )
            .unwrap(),
        ],
        "independent_verification_requires_human_review",
        "producer-reported partial verification is not a durable independent verifier attachment",
    )
}

fn contradiction_blast_radius_brief() -> DecisionBrief {
    let target = "vf_decision_brief_blast_target";
    let counter = "vf_decision_brief_blast_counter";
    let claim = "The target claim should remain active while its contradiction is reviewed.";
    let mut target_finding = make_finding(target, 0.3, "computational");
    target_finding.assertion.text = claim.to_string();
    let mut counter_finding = make_finding(counter, 0.3, "computational");
    counter_finding.assertion.text =
        "A reproducible counterexample refutes the target claim.".to_string();
    let dependent = |id: &str| {
        let mut finding = make_finding(id, 0.3, "computational");
        finding.links.push(Link {
            target: target.to_string(),
            link_type: "depends".to_string(),
            note: "This result inherits the target claim.".to_string(),
            inferred_by: "vela/decision-brief-fixture.v1".to_string(),
            created_at: CREATED_AT.to_string(),
            mechanism: None,
        });
        finding
    };
    let mut project = fixed_project(
        "Decision brief contradiction blast-radius fixture",
        vec![
            target_finding,
            counter_finding,
            dependent("vf_decision_brief_dependent_one"),
            dependent("vf_decision_brief_dependent_two"),
        ],
    );
    project.contradictions.push(Contradiction::candidate(
        project.frontier_id.as_deref().unwrap(),
        target,
        counter,
        "The counter-finding reports a reproducible result incompatible with the target.",
    ));
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({"finding": finding_value(target, "computational", claim)}),
        vec!["urn:source:counterexample".to_string()],
        vec!["The contradiction remains unadjudicated.".to_string()],
        "record the unresolved contradiction and its downstream exposure",
    );

    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::missing("receipt_not_applicable"),
            route: ReviewRoute::human_only(
                absent_policy_facts(),
                "open_contradiction_requires_human_review",
                "the contradiction and downstream blast radius require explicit review",
            ),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: None,
        },
    )
    .unwrap()
}

fn rich_distillation_contributors_brief() -> DecisionBrief {
    let target = "vf_decision_brief_rich_distillation";
    let claim = "A replayable derivation is accompanied by a reviewer-oriented distillation.";
    let input_path = fixture_dir()
        .join("inputs")
        .join("rich-distillation-contributors.receipt.json");
    let receipt_bytes = std::fs::read(&input_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", input_path.display()));
    let receipt = ReceiptV1::parse(&receipt_bytes)
        .unwrap_or_else(|error| panic!("parse {}: {error}", input_path.display()));
    let mut project = fixed_project("Decision brief rich distillation fixture", vec![]);
    receipt_bound_brief_from_receipt(
        &mut project,
        "finding.note",
        target,
        claim,
        "computational",
        vec![
            "The distillation remains a draft and does not confer scientific acceptance."
                .to_string(),
        ],
        receipt,
        "draft_distillation_requires_human_review",
        "the attributed draft helps review but carries no acceptance authority",
    )
}

fn install_active_permit_policy(path: &Path, project: &Project) -> String {
    let mut policy = AcceptancePolicy {
        schema: "vela.acceptance_policy.v0.1".to_string(),
        id: String::new(),
        frontier_id: project.frontier_id().to_string(),
        epoch: 1,
        issued_by: vec!["reviewer:all-facets-fixture".to_string()],
        quorum: Quorum {
            threshold: 1,
            eligible_roles: vec!["reviewer".to_string()],
        },
        rules: vec![PolicyRule {
            id: "permit:all-facets-fixture".to_string(),
            effect: Outcome::Permit,
            claim_classes: vec!["receipt_theoretical".to_string()],
            constraints: Constraints {
                max_changed_findings: 1,
                max_downstream_dependents: 1,
                required_assurance_min: 0,
                allow_semantic_text_change: true,
                allow_contested: true,
                allow_governance_mutation: false,
                require_independence: false,
                require_method_integrity: false,
            },
        }],
        default: Outcome::Defer,
        expires_at: "2099-12-31T23:59:59Z".to_string(),
        revocation_ref: None,
    };
    policy.id = policy.content_address();
    let key = SigningKey::from_bytes(&[0x63; 32]);
    let signed_at = "2026-07-13T12:10:00Z";
    let signature = key.sign(
        &vela_protocol::acceptance_policy::policy_signature_preimage(&policy, signed_at).unwrap(),
    );
    let policy_dir = path.join(".vela/policies");
    std::fs::create_dir_all(&policy_dir).unwrap();
    std::fs::write(
        policy_dir.join("active.json"),
        serde_json::to_vec_pretty(&policy).unwrap(),
    )
    .unwrap();
    std::fs::write(
        policy_dir.join("active.sig.json"),
        serde_json::to_vec_pretty(&PolicySignatureRecord {
            policy_id: policy.id.clone(),
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(signature.to_bytes()),
            signed_at: signed_at.to_string(),
        })
        .unwrap(),
    )
    .unwrap();
    policy.id
}

fn all_facets_brief() -> DecisionBrief {
    let target = "vf_decision_brief_all_facets";
    let claim = "A theoretical result has a bounded scientific chain and complete review context.";
    let mut existing = make_finding(target, 0.3, "theoretical");
    existing.assertion.text = "The prior theoretical claim remains contested.".to_string();
    existing.flags.contested = true;
    let mut project = fixed_project("Decision brief all-facets fixture", vec![existing]);
    let identity = frozen_receipt_identity();
    project.actors.push(ActorRecord {
        id: identity.actor_id.clone(),
        public_key: identity.public_key_hex.clone(),
        algorithm: "ed25519".to_string(),
        created_at: "2026-07-13T11:00:00Z".to_string(),
        tier: None,
        orcid: None,
        access_clearance: None,
        revoked_at: None,
        revoked_reason: None,
    });
    project.attempt_claims.push(AttemptClaim {
        obligation_id: target.to_string(),
        claimant_actor: RECEIPT_ACTOR.to_string(),
        claimant_pubkey: identity.public_key_hex,
        claimed_at: "2026-07-13T12:20:00Z".to_string(),
        lease_ttl_seconds: 3600,
        claim_event_id: Some(format!("vev_{}", "1".repeat(64))),
    });

    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join(".vela")).unwrap();
    let policy_id = install_active_permit_policy(temp.path(), &project);
    let event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let receipt = ReceiptBuilder::build(
        ReceiptInput::new(
            claim.to_string(),
            "theoretical".to_string(),
            "exact".to_string(),
            vec![
                ArtifactInput::new(
                    "proofs/all-facets.olean".to_string(),
                    "proof".to_string(),
                    Some("d".repeat(64)),
                    None,
                )
                .unwrap(),
            ],
            vec!["The producer assertion remains subject to human review.".to_string()],
            vec![
                ProducerReportedRun::producer_reported(
                    "lean build --frozen".to_string(),
                    "pass".to_string(),
                )
                .unwrap(),
            ],
            RECEIPT_ACTOR.to_string(),
            "2026-07-13T12:34:56Z".to_string(),
            event_log_root,
            ".".to_string(),
            RECEIPT_OPERATION_ID.to_string(),
            policy_id,
        )
        .unwrap()
        .with_scientific_chain(
            ScientificChainAssertion::new(
                Some("The frozen proof checks without new axioms.".to_string()),
                false,
                "Build the exact Lean artifact and inspect its axiom closure.".to_string(),
                "The build passed and the declared closure was observed.".to_string(),
                vec!["proofs/all-facets.olean".to_string()],
                vec!["records/attempts/failed-generalization.json".to_string()],
            )
            .unwrap(),
        )
        .unwrap(),
        &frozen_receipt_identity(),
    )
    .unwrap();
    let receipt_root = receipt.canonical_root().unwrap();
    let receipt_digest = receipt_root.strip_prefix("sha256:").unwrap();
    let proposal_id = install_proposal(
        &mut project,
        "finding.note",
        target,
        json!({
            "text": claim,
            "finding": finding_value(target, "theoretical", claim),
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": receipt_root,
                "receipt_path": format!("records/receipts/sha256/{receipt_digest}.json"),
                "record_id": format!("vrc_{receipt_digest}"),
                "operation_id": RECEIPT_OPERATION_ID,
                "same_claim_findings": ["vf_independent_replication"],
            }
        }),
        vec!["urn:source:all-facets".to_string()],
        vec!["The producer assertion remains subject to human review.".to_string()],
        "exercise every bounded Decision Brief facet",
    );
    vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();
    let snapshot =
        vela_protocol::acceptance_policy::load_active_policy_snapshot(temp.path()).unwrap();
    let staged = vela_protocol::proposals::policy_accept::stage_policy_route_in_frontier_at(
        temp.path(),
        &project,
        &proposal_id,
        &receipt,
        OBSERVED_AT,
        &snapshot,
    )
    .unwrap();
    assert_eq!(
        staged.decision().unwrap().outcome,
        Outcome::Permit,
        "decision={:?} context={:?}",
        staged.decision().unwrap(),
        staged.context()
    );
    let publication_root = format!("sha256:{}", "e".repeat(64));
    build_decision_brief(
        &project,
        DecisionBriefInput {
            proposal_id: &proposal_id,
            receipt: ReceiptMaterial::from_receipt(&receipt),
            route: ReviewRoute::from_staged(&staged),
            observed_at: OBSERVED_AT,
            replay_ok: true,
            publication: Some(PublicationProjection {
                root: &publication_root,
                state: "committed_local",
            }),
        },
    )
    .unwrap()
}

fn generated_cases() -> Vec<(&'static str, DecisionBrief)> {
    vec![
        ("ordinary", ordinary_brief()),
        ("critical-warning", critical_warning_brief()),
        ("missing", missing_brief()),
        ("restricted-evidence", restricted_evidence_brief()),
        ("build-statement-fidelity", build_statement_fidelity_brief()),
        ("vacuity-tautology", vacuity_tautology_brief()),
        (
            "producer-trusting-partial-verifier",
            producer_trusting_partial_verifier_brief(),
        ),
        (
            "contradiction-blast-radius",
            contradiction_blast_radius_brief(),
        ),
        (
            "rich-distillation-contributors",
            rich_distillation_contributors_brief(),
        ),
    ]
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/fixtures/decision-brief-testing-v1")
}

fn canonical_sha256(value: &Value) -> String {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value).unwrap();
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn frozen_roots(name: &str) -> (&'static str, &'static str) {
    match name {
        "ordinary" => (
            "sha256:c3539201c6647c53ea034f064e5e972fa80d609c61db6012cf5e5a55b4a4fbbc",
            "sha256:d2b14e23b93f914ac59e1b47dcd3e7dfda3c84e8e791a1edbdde35a422298393",
        ),
        "critical-warning" => (
            "sha256:dc9edc7c2c8e9af004f72c9be66f826d20f89f6eefb774433a096aa5cd90899c",
            "sha256:86159badb0d041fa302ba179e2e8e9698e84699acbf81e737807a89d46fb1957",
        ),
        "missing" => (
            "sha256:f85c4c9017ad7a342812aef08939f829a25455786787bea437bcd55005721135",
            "sha256:c3223498a0ad414ded41fa869f76df9de63e5ca953ebc2420f1e9bac93d9faaf",
        ),
        "restricted-evidence" => (
            "sha256:c61aa27cae344de6665ba25f99e1f2bc687c50b7f6646eaf477158d8500ff317",
            "sha256:5adad5b602c48436de8f2e1f0a4fceac06f69305999b743dbeb3d83c74a76c81",
        ),
        "build-statement-fidelity" => (
            "sha256:48042cc6c59aeff61ccfa05a67f123ba887d9419ad87ce9eac4042a7b3038d62",
            "sha256:3e28cd464771147a2e89092382396321df831fe838bdff891a015620b59c725f",
        ),
        "vacuity-tautology" => (
            "sha256:74eae573086c6d1bd4b050ae4cbcca9d66e6192acb4c1f7955801e3f0620f9be",
            "sha256:39567fc65f80a9a33d0f724193067016825a2d2ae82500f05d17b6653ec7515a",
        ),
        "producer-trusting-partial-verifier" => (
            "sha256:209804dc49c48f0122961228f0174bede762214ef8b8e4657df3e2c9daea70ce",
            "sha256:7f432dbde8e331a4aac4c976286e18733cfbd3c4d1b68bbf95ec5e573ca70b76",
        ),
        "contradiction-blast-radius" => (
            "sha256:26bf05ec170724f30fe3a4ab42763385bdb75beaffab5d38fa8ae87193fd58a9",
            "sha256:8d8f9f8a05bc6be3c9b741d221d37c12c2329fda39955b65ae5a13b9bd7843e0",
        ),
        "rich-distillation-contributors" => (
            "sha256:ef999f2ccd71ac32d57992186a91bc11a59e45d7807dfded6b0530ece0c5c10a",
            "sha256:a30bdd8e87ef4bf315dde031f03c1ebd5646952d59f086fed2d43478a8e16e53",
        ),
        other => panic!("no frozen roots for {other}"),
    }
}

fn assert_sha256_root(value: &Value, field: &str) {
    let root = value
        .as_str()
        .unwrap_or_else(|| panic!("{field} must be a string root"));
    assert_eq!(root.len(), 71, "{field} has the wrong root length");
    let digest = root
        .strip_prefix("sha256:")
        .unwrap_or_else(|| panic!("{field} must be sha256-prefixed"));
    assert!(
        digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{field} must contain lowercase hexadecimal"
    );
}

fn assert_no_generic_authority_keys(value: &Value, path: &str) {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                assert_no_generic_authority_keys(value, &format!("{path}[{index}]"));
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                assert!(
                    key != "signed" && key != "trusted",
                    "generic authority key {path}.{key} is forbidden"
                );
                assert_no_generic_authority_keys(value, &format!("{path}.{key}"));
            }
        }
        _ => {}
    }
}

fn assert_bounded_facet_value(value: &Value, depth: usize) {
    assert!(depth <= 9, "facet projection exceeded its depth bound");
    match value {
        Value::String(text) => {
            assert!(text.len() <= 1027, "facet string exceeded its byte bound");
        }
        Value::Array(values) => {
            assert!(values.len() <= 32, "facet array exceeded its item bound");
            for value in values {
                assert_bounded_facet_value(value, depth + 1);
            }
        }
        Value::Object(values) => {
            assert!(values.len() <= 64, "facet object exceeded its field bound");
            for value in values.values() {
                assert_bounded_facet_value(value, depth + 1);
            }
        }
        _ => {}
    }
}

fn assert_contract_shape(value: &Value) {
    let object = value.as_object().expect("brief must be an object");
    for section in ["change", "basis", "impact", "authority", "audit"] {
        assert!(
            object.get(section).is_some_and(Value::is_object),
            "required section {section} is absent"
        );
    }
    assert_eq!(value["schema"], json!("vela.decision-brief.testing.v1"));
    assert_eq!(value["stability"], json!("testing"));

    let facets = value["facets"].as_object().expect("facets must be a map");
    assert!(!facets.is_empty(), "fixtures must exercise a typed facet");
    let keys = facets.keys().collect::<Vec<_>>();
    assert!(keys.is_sorted(), "facet map must serialize in sorted order");
    for (name, facet) in facets {
        assert!(
            facet["schema"]
                .as_str()
                .is_some_and(|schema| schema.starts_with("vela.decision-brief.facet.")),
            "facet {name} must carry a typed schema"
        );
        assert!(facet["critical"].is_boolean());
        assert!(facet["truncated"].is_boolean());
        assert_sha256_root(&facet["full_root"], &format!("facets.{name}.full_root"));
        assert_bounded_facet_value(&facet["data"], 1);
    }

    let actions = value["authority"]["actions"]
        .as_array()
        .expect("actions must be an array");
    assert_eq!(actions.len(), 2);
    for action_name in ["accept", "reject"] {
        let action = actions
            .iter()
            .find(|candidate| candidate["action"] == json!(action_name))
            .unwrap_or_else(|| panic!("missing {action_name} action"));
        let eligibility = action["eligibility"]
            .as_str()
            .expect("action eligibility must be typed");
        assert!(eligibility == "available" || eligibility == "blocked");
        let reasons = action["reasons"].as_array().expect("reasons must be typed");
        assert_eq!(eligibility == "available", reasons.is_empty());
        if action_name == "reject" {
            assert_eq!(eligibility, "available");
        }
    }

    let audit = &value["audit"];
    for field in [
        "proposal_root",
        "decision_facts_root",
        "policy_input_root",
        "policy_result_root",
        "raw_references_root",
        "missing_root",
    ] {
        assert_sha256_root(&audit[field], &format!("audit.{field}"));
    }
    let references = audit["raw_references"]
        .as_array()
        .expect("raw references must be typed");
    assert!(references.len() <= 64);
    for reference in references {
        assert!(reference.as_str().unwrap().len() <= 515);
    }
    assert!(value["missing"].as_array().unwrap().len() <= 16);
    for truncation in audit["truncations"].as_array().unwrap() {
        assert_sha256_root(&truncation["full_root"], "audit.truncations.full_root");
        assert!(truncation["omitted_bytes"].as_u64().unwrap() > 0);
    }
    assert_no_generic_authority_keys(value, "$brief");
}

#[test]
fn golden_fixtures_match_public_builder() {
    for (name, brief) in generated_cases() {
        let path = fixture_dir().join(format!("{name}.json"));
        let frozen: Value = serde_json::from_slice(
            &std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
        let generated = serde_json::to_value(brief).unwrap();
        assert_eq!(generated, frozen, "fixture drift: {}", path.display());
        assert_eq!(
            vela_protocol::canonical::to_canonical_bytes(&generated).unwrap(),
            vela_protocol::canonical::to_canonical_bytes(&frozen).unwrap(),
            "canonical byte drift: {}",
            path.display()
        );
        let (canonical_root, decision_facts_root) = frozen_roots(name);
        assert_eq!(canonical_sha256(&frozen), canonical_root);
        assert_eq!(
            frozen["audit"]["decision_facts_root"],
            json!(decision_facts_root)
        );
        assert_contract_shape(&frozen);
    }
}

#[test]
fn golden_cases_cover_critical_missing_and_restricted_evidence() {
    let load = |name: &str| -> Value {
        let path = fixture_dir().join(format!("{name}.json"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };

    let ordinary = load("ordinary");
    assert!(ordinary["missing"].as_array().unwrap().is_empty());
    assert!(
        ordinary["impact"]["critical_warnings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ordinary["authority"]["actions"][0]["eligibility"],
        json!("available")
    );

    let critical = load("critical-warning");
    assert_eq!(critical["facets"]["challenge"]["critical"], json!(true));
    assert!(
        critical["impact"]["critical_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == json!("active_challenge"))
    );

    let missing = load("missing");
    assert!(!missing["missing"].as_array().unwrap().is_empty());
    assert_eq!(missing["authority"]["route"], json!("broken"));
    assert_eq!(
        missing["authority"]["actions"][0]["eligibility"],
        json!("blocked")
    );

    let restricted = load("restricted-evidence");
    assert_eq!(
        restricted["facets"]["formal_fidelity"]["critical"],
        json!(true)
    );
    assert!(
        restricted["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| {
                fact["field"] == json!("basis.receipt")
                    && fact["reason"] == json!("restricted_evidence_not_available_to_reviewer")
            })
    );
    assert!(
        restricted["audit"]["raw_references"].as_array().unwrap()[1]
            .as_str()
            .unwrap()
            .ends_with('…')
    );
    assert_eq!(
        restricted["audit"]["truncations"][0]["field"],
        json!("audit.raw_references[1]")
    );
    assert_eq!(restricted["authority"]["route"], json!("defer"));
}

#[test]
fn all_facets_golden_has_the_exact_known_inventory() {
    const EXPECTED_FACETS: &[&str] = &[
        "acceptance_authority",
        "challenge",
        "contributor_roles",
        "distillation",
        "evidence_lineage",
        "external_certificates",
        "formal_fidelity",
        "gate_matrix",
        "hypothesis_evolution",
        "publication",
        "replication_diversity",
        "scientific_chain",
        "work_lease",
    ];
    let generated = serde_json::to_value(all_facets_brief()).unwrap();
    let actual = generated["facets"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(actual, EXPECTED_FACETS);
    assert_eq!(
        generated["facets"]["acceptance_authority"]["critical"],
        json!(false),
        "a verified Permit route is the positive acceptance-authority case"
    );
    assert_eq!(
        generated["facets"]["publication"]["data"]["state"],
        json!("committed_local")
    );
    assert_eq!(
        generated["facets"]["scientific_chain"]["data"]["authority"],
        json!("producer")
    );
    assert_contract_shape(&generated);
    let path = fixture_dir().join("all-facets/decision-brief.json");
    let frozen: Value = serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    assert_eq!(generated, frozen, "fixture drift: {}", path.display());
    assert_eq!(
        canonical_sha256(&generated),
        "sha256:9f79721056489bd676d652bcc1e5dbf71d41b3e1fcc05ddfdde2de7c785b2a19"
    );
}

#[test]
fn adversarial_cases_expose_the_review_facts_without_inventing_authority() {
    let load = |name: &str| -> Value {
        let path = fixture_dir().join(format!("{name}.json"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };

    let fidelity = load("build-statement-fidelity");
    assert_eq!(
        fidelity["facets"]["formal_fidelity"]["critical"],
        json!(true)
    );
    assert_eq!(
        fidelity["facets"]["formal_fidelity"]["data"]["statement_attestations"],
        json!([])
    );
    assert_eq!(
        fidelity["basis"]["check_state"]["producer_reported"],
        json!([{
            "method": "lean build --frozen",
            "outcome": "pass",
            "authority": "producer",
        }])
    );

    let vacuity = load("vacuity-tautology");
    assert_eq!(vacuity["facets"]["challenge"]["critical"], json!(true));
    assert_eq!(
        vacuity["facets"]["challenge"]["data"]["open_contradictions"][0]["other_subject"],
        json!("vlv_vacuity_probe")
    );
    assert!(
        vacuity["impact"]["critical_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning["code"] == json!("active_challenge"))
    );

    let partial = load("producer-trusting-partial-verifier");
    assert_eq!(
        partial["basis"]["check_state"]["producer_reported"][0],
        json!({
            "method": "producer.partial-check --trust-inputs",
            "outcome": "pass",
            "authority": "producer",
        })
    );
    assert_eq!(
        partial["basis"]["check_state"]["durable_verifier_count"],
        json!(0)
    );
    assert_eq!(
        partial["basis"]["check_state"]["gate_status"],
        json!("needs_verification")
    );

    let contradiction = load("contradiction-blast-radius");
    assert_eq!(
        contradiction["facets"]["challenge"]["data"]["open_contradictions"][0]["other_subject"],
        json!("vf_decision_brief_blast_counter")
    );
    assert_eq!(
        contradiction["impact"]["downstream_effect"]["downstream_dependents"],
        json!(2)
    );
    assert_eq!(
        contradiction["facets"]["challenge"]["critical"],
        json!(true)
    );

    let rich = load("rich-distillation-contributors");
    assert_eq!(rich["facets"]["distillation"]["critical"], json!(false));
    assert_eq!(
        rich["facets"]["distillation"]["data"]["status"],
        json!("draft")
    );
    assert_eq!(
        rich["facets"]["distillation"]["data"]["known_gaps"],
        json!(["Independent replay has not yet been attached."])
    );
    assert_eq!(
        rich["facets"]["contributor_roles"]["data"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        rich["facets"]["contributor_roles"]["data"][1]["roles"],
        json!(["human_distiller", "writing_original_draft"])
    );
    assert_eq!(rich["authority"]["route"], json!("defer"));
}

#[test]
fn adversarial_fixtures_publish_one_concrete_reviewer_question_each() {
    let path = fixture_dir().join("reviewer-questions.json");
    let questions: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        questions,
        json!([
            {
                "fixture": "build-statement-fidelity",
                "question": "Does the passing Lean build establish the intended informal statement, or is statement-faithfulness evidence still missing?"
            },
            {
                "fixture": "vacuity-tautology",
                "question": "Does the formal result use the intended hypotheses and establish a non-vacuous claim, or only a tautology?"
            },
            {
                "fixture": "producer-trusting-partial-verifier",
                "question": "Did an independent durable verifier recompute the result, or is this only a producer-reported partial check that trusts its inputs?"
            },
            {
                "fixture": "contradiction-blast-radius",
                "question": "Which live contradiction is unresolved, and how many downstream findings could be affected if this target changes?"
            },
            {
                "fixture": "rich-distillation-contributors",
                "question": "Is the distillation complete enough for this audience, what gaps remain, and who contributed in which roles without implying acceptance authority?"
            }
        ])
    );
    for item in questions.as_array().unwrap() {
        let fixture = item["fixture"].as_str().unwrap();
        assert!(
            fixture_dir().join(format!("{fixture}.json")).is_file(),
            "reviewer question has no frozen Decision Brief fixture: {fixture}"
        );
    }
}

#[test]
fn published_schema_freezes_sections_facets_and_actions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/vela.decision-brief.testing.v1.schema.json");
    let schema: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        schema["$schema"],
        json!("https://json-schema.org/draft/2020-12/schema")
    );
    assert_eq!(
        schema["properties"]["schema"]["const"],
        json!("vela.decision-brief.testing.v1")
    );
    let required = schema["required"].as_array().unwrap();
    for field in [
        "change",
        "basis",
        "impact",
        "authority",
        "audit",
        "missing",
        "facets",
    ] {
        assert!(
            required.contains(&json!(field)),
            "schema must require {field}"
        );
    }
    assert_eq!(
        schema["properties"]["facets"]["additionalProperties"]["$ref"],
        json!("#/$defs/typed_facet")
    );
    assert_eq!(
        schema["$defs"]["action"]["properties"]["eligibility"]["enum"],
        json!(["available", "blocked"])
    );
}
