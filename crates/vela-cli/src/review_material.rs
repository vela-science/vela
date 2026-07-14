//! Pure derivation of decision-critical review facts.
//!
//! Producer-reported Receipt v1 verifier runs are provenance. They never enter
//! this builder. Assurance, independence, and method integrity derive only from
//! durable verifier attachments through the protocol gate; missing inputs make
//! the resulting [`PolicyContext`] more conservative.

use std::path::{Component, Path};

use vela_protocol::acceptance_policy::PolicyContext;
use vela_protocol::bundle::FindingBundle;
use vela_protocol::independence::independence_from_attachments;
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;
use vela_protocol::receipt_v1::{AttestationBinding, ReceiptV1};
use vela_protocol::verifier_attachment::{
    GateStatus, MethodIntegrity, VerifierAttachment, claim_digest, derive_gate_status,
};

const REPLAYABILITY: &[&str] = &["exact", "bounded", "approximate", "unavailable", "unknown"];

/// Inputs that are not themselves verifier judgments.
///
/// Every boolean defaults in the conservative direction. The call site may
/// supply a resolved credential result or graph facts, but cannot supply an
/// assurance level, independence verdict, or method-integrity verdict.
#[derive(Debug, Clone)]
pub(crate) struct PolicyContextInputs<'a> {
    pub(crate) proposal: &'a StateProposal,
    pub(crate) finding: &'a FindingBundle,
    pub(crate) attachments: &'a [VerifierAttachment],
    pub(crate) replayability: Option<&'a str>,
    pub(crate) receipt_is_body_bound: bool,
    pub(crate) credential_valid: bool,
    pub(crate) target_contested: bool,
    pub(crate) downstream_dependents: u32,
}

/// The one policy-context derivation used by submission, review, and status.
///
/// `GateStatus::Verified` is A3 because the protocol gate already requires
/// independent matched attachments. A producer-reported pass that has not
/// become a durable attachment therefore remains A0. A refutation also remains
/// A0; its effect is surfaced separately by the gate and cannot be mistaken for
/// positive assurance.
pub(crate) fn derive_policy_context(input: PolicyContextInputs<'_>) -> PolicyContext {
    let digest = claim_digest(&input.finding.assertion.text);
    let relevant = input
        .attachments
        .iter()
        .filter(|attachment| attachment.target == input.finding.id)
        .cloned()
        .collect::<Vec<_>>();
    let gate = derive_gate_status(&digest, &relevant);
    let independence = independence_from_attachments(&digest, &relevant);
    let method_integrity_sound = gate.status == GateStatus::Verified
        && relevant
            .iter()
            .filter(|attachment| {
                attachment.claim_digest == digest
                    && attachment.match_to_claim.matches
                    && attachment.outcome
                        == vela_protocol::verifier_attachment::AttachmentOutcome::Passed
            })
            .all(|attachment| attachment.method_integrity == MethodIntegrity::Sound);

    let replayability = input.replayability.unwrap_or("unknown");
    let replayability_known = REPLAYABILITY.contains(&replayability);
    let claim_class = format!("receipt_{}", input.finding.assertion.assertion_type);
    let governance_mutation = input.proposal.kind.starts_with("governance.")
        || input.proposal.target.r#type == "governance";

    PolicyContext {
        claim_class,
        assurance_level: if gate.status == GateStatus::Verified {
            3
        } else {
            0
        },
        impact_tier: if governance_mutation { 4 } else { 1 },
        changed_findings: 1,
        downstream_dependents: input.downstream_dependents,
        assertion_text_mutated: input.proposal.kind == "finding.add",
        target_contested: input.target_contested || gate.status == GateStatus::Refuted,
        governance_mutation,
        independence_satisfied: gate.status == GateStatus::Verified && independence.satisfied,
        method_integrity_sound,
        credential_valid: input.credential_valid,
        has_unknown_fields: !input.receipt_is_body_bound || !replayability_known,
        replayability: if replayability_known {
            replayability.to_string()
        } else {
            "unknown".to_string()
        },
    }
}

/// Derive the policy facts for a proposal already present in a frontier.
///
/// The optional receipt is trusted only when its canonical root and claim body
/// match the proposal's typed `vela_submission` links. Missing, unreadable, or
/// mismatched receipt material cannot raise assurance, credential validity, or
/// body-binding status. This makes queue, policy-preview, and policy-suggestion
/// projections agree with the landing derivation without letting a projection
/// manufacture facts that were never retained.
pub(crate) fn derive_existing_proposal_policy_context(
    project: &Project,
    proposal_id: &str,
    receipt: Option<&ReceiptV1>,
) -> PolicyContext {
    let decision_time = chrono::Utc::now().to_rfc3339();
    let Some(proposal) = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
    else {
        return PolicyContext::default();
    };
    let claim_class = proposal_claim_class(proposal);
    let Some(finding) = proposal_finding(project, proposal) else {
        return PolicyContext {
            claim_class,
            ..PolicyContext::default()
        };
    };
    let receipt = receipt.filter(|receipt| receipt_matches_proposal(receipt, proposal, &finding));
    let replayability = receipt
        .and_then(|receipt| receipt.as_value().get("replayability"))
        .and_then(serde_json::Value::as_str);
    let downstream_dependents = project
        .findings
        .iter()
        .filter(|candidate| {
            candidate.links.iter().any(|link| {
                vela_protocol::bundle::bare_finding_id(&link.target) == finding.id.as_str()
            })
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    let target_contested = finding.flags.contested
        || proposal
            .payload
            .get("vela_submission")
            .and_then(|submission| submission.get("same_claim_findings"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|related| {
                project
                    .findings
                    .iter()
                    .find(|candidate| candidate.id == related)
                    .is_some_and(|candidate| candidate.flags.contested)
            });
    let mut context = derive_policy_context(PolicyContextInputs {
        proposal,
        finding: &finding,
        attachments: &project.verifier_attachments,
        replayability,
        receipt_is_body_bound: receipt
            .is_some_and(|receipt| receipt.attestation_binding() == AttestationBinding::Bound),
        credential_valid: receipt.is_some_and(|receipt| {
            receipt_producer_credential_valid(project, receipt, &decision_time)
        }),
        target_contested,
        downstream_dependents,
    });
    context.claim_class = claim_class;
    context
}

/// Load the exact Receipt v1 named by a proposal's typed submission links.
/// Any path, symlink, parse, or root mismatch returns `None`; callers then use
/// the conservative branch of [`derive_existing_proposal_policy_context`].
pub(crate) fn frontier_receipt_for_proposal(
    frontier: &Path,
    proposal: &StateProposal,
) -> Option<ReceiptV1> {
    let submission = proposal.payload.get("vela_submission")?;
    let receipt_path = submission.get("receipt_path")?.as_str()?;
    let declared_root = submission.get("receipt_root")?.as_str()?;
    let relative = Path::new(receipt_path);
    if !receipt_path.starts_with("records/receipts/sha256/")
        || relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let canonical_frontier = frontier.canonicalize().ok()?;
    let lexical = canonical_frontier.join(relative);
    let metadata = std::fs::symlink_metadata(&lexical).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    if lexical.canonicalize().ok()? != lexical {
        return None;
    }
    let receipt = ReceiptV1::parse(&std::fs::read(lexical).ok()?).ok()?;
    (receipt.canonical_root().ok()?.as_str() == declared_root).then_some(receipt)
}

/// Structural class used by every existing-proposal projection. Receipt-backed
/// finding additions retain their receipt type even when the receipt bytes are
/// temporarily unavailable; the remaining facts still fail closed.
pub(crate) fn proposal_claim_class(proposal: &StateProposal) -> String {
    if proposal.kind == "finding.note" {
        return "finding_note".to_string();
    }
    if proposal.kind.starts_with("governance.") || proposal.target.r#type == "governance" {
        return "governance".to_string();
    }
    if proposal.kind == "finding.add"
        && let Some(
            claim_type @ ("computational" | "theoretical" | "empirical" | "negative"
            | "contradiction"),
        ) = proposal
            .payload
            .get("finding")
            .and_then(|finding| finding.get("assertion"))
            .and_then(|assertion| assertion.get("type"))
            .and_then(serde_json::Value::as_str)
    {
        return format!("receipt_{claim_type}");
    }
    let text = proposal
        .payload
        .get("assertion")
        .and_then(|assertion| assertion.get("text"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            proposal
                .payload
                .get("finding")
                .and_then(|finding| finding.get("assertion"))
                .and_then(|assertion| assertion.get("text"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            proposal
                .payload
                .get("text")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or_default();
    classify_claim(text).to_string()
}

fn classify_claim(text: &str) -> &'static str {
    let text = text.to_lowercase();
    if text.contains("a309370") || text.contains("sidon") {
        "sidon_lower_bound"
    } else if text.contains("lean") || text.contains("formaliz") || text.contains("theorem") {
        "formal_theorem"
    } else if text.contains("oeis ") || text.contains("oeis:") {
        "oeis_sequence"
    } else if text.contains("erdős problem") || text.contains("erdos problem") {
        "erdos_problem"
    } else {
        "unknown"
    }
}

fn proposal_finding(project: &Project, proposal: &StateProposal) -> Option<FindingBundle> {
    proposal
        .payload
        .get("finding")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .or_else(|| {
            project
                .findings
                .iter()
                .find(|finding| finding.id == proposal.target.id)
                .cloned()
        })
}

fn receipt_matches_proposal(
    receipt: &ReceiptV1,
    proposal: &StateProposal,
    finding: &FindingBundle,
) -> bool {
    let Some(submission) = proposal.payload.get("vela_submission") else {
        return false;
    };
    let Some(declared_root) = submission
        .get("receipt_root")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    receipt
        .canonical_root()
        .ok()
        .is_some_and(|root| root == declared_root)
        && receipt
            .as_value()
            .get("claim")
            .and_then(serde_json::Value::as_str)
            == Some(finding.assertion.text.as_str())
        && receipt
            .as_value()
            .get("type")
            .and_then(serde_json::Value::as_str)
            == Some(finding.assertion.assertion_type.as_str())
}

/// Resolve a producer proof-of-possession against frontier authority.
///
/// An embedded [`IdentityBinding`](vela_protocol::identity::IdentityBinding)
/// proves only that its key signed the binding. It becomes a valid producer
/// credential here only when the same actor/key pair resolves uniquely in the
/// frontier registry, the actor registration predates the signed binding, the
/// binding predates the fixed decision time, and the registered key is not
/// revoked at that time. Malformed authority timestamps and ambiguous registry
/// entries fail closed.
pub(crate) fn receipt_producer_credential_valid(
    project: &Project,
    receipt: &ReceiptV1,
    decision_time: &str,
) -> bool {
    let Some(binding) = receipt
        .as_value()
        .get("environment")
        .and_then(|value| value.get("vela:producer_context"))
        .and_then(|value| value.get("identity_binding"))
        .cloned()
    else {
        return false;
    };
    let Ok(binding) = serde_json::from_value::<vela_protocol::identity::IdentityBinding>(binding)
    else {
        return false;
    };
    if binding.verify().is_err() {
        return false;
    }
    let (Ok(decision_at), Ok(binding_at)) = (
        chrono::DateTime::parse_from_rfc3339(decision_time),
        chrono::DateTime::parse_from_rfc3339(&binding.created_at),
    ) else {
        return false;
    };
    if binding_at > decision_at {
        return false;
    }

    let mut matches = project.actors.iter().filter(|actor| {
        actor.id == binding.actor_id
            && actor
                .public_key
                .eq_ignore_ascii_case(&binding.public_key_hex)
    });
    let Some(actor) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let Ok(actor_created_at) = chrono::DateTime::parse_from_rfc3339(&actor.created_at) else {
        return false;
    };
    // A later registry entry cannot retroactively confer authority on an
    // earlier self-signed producer binding. Registration must already exist
    // when the producer signs/creates the binding, not merely by review time.
    if actor_created_at > binding_at {
        return false;
    }
    match actor.revoked_at.as_deref() {
        None => true,
        Some(revoked_at) => chrono::DateTime::parse_from_rfc3339(revoked_at)
            .is_ok_and(|revoked_at| revoked_at > decision_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence, Extraction,
        FindingBundle, Flags, Provenance,
    };
    use vela_protocol::events::{StateActor, StateTarget};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};
    use vela_protocol::sign::ActorRecord;

    fn finding() -> FindingBundle {
        FindingBundle::new(
            Assertion {
                text: "A bounded result".to_string(),
                assertion_type: "computational".to_string(),
                entities: vec![],
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "computational".to_string(),
                model_system: String::new(),
                method: "producer reported".to_string(),
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            Conditions {
                text: "pending review".to_string(),
                duration: None,
            },
            Confidence {
                kind: ConfidenceKind::FrontierEpistemic,
                score: 0.3,
                basis: "producer report".to_string(),
                method: ConfidenceMethod::ExpertJudgment,
                extraction_confidence: 1.0,
            },
            Provenance {
                source_type: "model_output".to_string(),
                doi: None,
                url: None,
                title: "receipt".to_string(),
                authors: vec![],
                year: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction {
                    method: "receipt_import".to_string(),
                    model: None,
                    model_version: None,
                    extracted_at: "2026-07-13T00:00:00Z".to_string(),
                    extractor_version: "test".to_string(),
                },
                review: None,
                contributions: vec![],
            },
            Flags::default(),
        )
    }

    fn proposal(finding: &FindingBundle) -> StateProposal {
        StateProposal {
            schema: vela_protocol::proposals::PROPOSAL_SCHEMA.to_string(),
            id: "vpr_test".to_string(),
            kind: "finding.add".to_string(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: finding.id.clone(),
            },
            actor: StateActor {
                id: "agent:test".to_string(),
                r#type: "agent".to_string(),
            },
            created_at: "2026-07-13T00:00:00Z".to_string(),
            drafted_at: None,
            reason: "test".to_string(),
            payload: json!({"finding": finding}),
            source_refs: vec![],
            status: "pending_review".to_string(),
            reviewed_by: None,
            reviewed_at: None,
            decision_reason: None,
            applied_event_id: None,
            caveats: vec![],
            agent_run: None,
        }
    }

    fn retained_receipt(finding: &FindingBundle) -> ReceiptV1 {
        let at = "2026-07-13T00:00:00Z";
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:test".to_string(),
                actor_class: ActorClass::Agent,
                created_at: at.to_string(),
            },
            &SigningKey::from_bytes(&[0x37; 32]),
        )
        .unwrap();
        let input = ReceiptInput::new(
            finding.assertion.text.clone(),
            finding.assertion.assertion_type.clone(),
            "exact".to_string(),
            vec![
                ArtifactInput::new(
                    "witnesses/result.json".to_string(),
                    "witness".to_string(),
                    Some("a".repeat(64)),
                    Some("https://example.test/result.json".to_string()),
                )
                .unwrap(),
            ],
            vec!["bounded test fixture".to_string()],
            Vec::new(),
            "agent:test".to_string(),
            at.to_string(),
            format!("sha256:{}", "b".repeat(64)),
            ".".to_string(),
            format!("vop_{}", "c".repeat(64)),
            "urn:vela:policy:none".to_string(),
        )
        .unwrap();
        ReceiptBuilder::build(input, &identity).unwrap()
    }

    #[test]
    fn producer_report_without_durable_attachments_cannot_raise_assurance() {
        let finding = finding();
        let proposal = proposal(&finding);
        let context = derive_policy_context(PolicyContextInputs {
            proposal: &proposal,
            finding: &finding,
            attachments: &[],
            replayability: Some("exact"),
            receipt_is_body_bound: true,
            credential_valid: true,
            target_contested: false,
            downstream_dependents: 0,
        });
        assert_eq!(context.assurance_level, 0);
        assert!(!context.independence_satisfied);
        assert!(!context.method_integrity_sound);
        assert!(!context.has_unknown_fields);
    }

    #[test]
    fn missing_receipt_binding_and_unknown_replayability_fail_closed() {
        let finding = finding();
        let proposal = proposal(&finding);
        let context = derive_policy_context(PolicyContextInputs {
            proposal: &proposal,
            finding: &finding,
            attachments: &[],
            replayability: Some("producer-invented"),
            receipt_is_body_bound: false,
            credential_valid: false,
            target_contested: false,
            downstream_dependents: 0,
        });
        assert!(context.has_unknown_fields);
        assert_eq!(context.replayability, "unknown");
        assert!(!context.credential_valid);
    }

    #[test]
    fn existing_proposal_context_matches_landing_derivation_for_retained_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let finding = finding();
        let mut proposal = proposal(&finding);
        let receipt = retained_receipt(&finding);
        let receipt_root = receipt.canonical_root().unwrap();
        let receipt_path = format!(
            "records/receipts/sha256/{}.json",
            receipt_root.strip_prefix("sha256:").unwrap()
        );
        proposal.payload["vela_submission"] = json!({
            "schema": "vela.submission-links.internal.v1",
            "receipt_root": receipt_root,
            "receipt_path": receipt_path.clone(),
            "record_id": "vrc_0123456789abcdef",
            "operation_id": format!("vop_{}", "c".repeat(64)),
        });
        let absolute = temp.path().join(&receipt_path);
        std::fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        std::fs::write(&absolute, receipt.canonical_bytes().unwrap()).unwrap();
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project.actors.push(ActorRecord {
            id: "agent:test".to_string(),
            public_key: hex::encode(
                SigningKey::from_bytes(&[0x37; 32])
                    .verifying_key()
                    .to_bytes(),
            ),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        project.proposals.push(proposal.clone());

        let loaded = frontier_receipt_for_proposal(temp.path(), &proposal)
            .expect("typed retained receipt must load");
        let actual = derive_existing_proposal_policy_context(&project, &proposal.id, Some(&loaded));
        let expected = derive_policy_context(PolicyContextInputs {
            proposal: &proposal,
            finding: &finding,
            attachments: &[],
            replayability: Some("exact"),
            receipt_is_body_bound: true,
            credential_valid: true,
            target_contested: false,
            downstream_dependents: 0,
        });

        assert_eq!(actual, expected);
        assert_eq!(actual.claim_class, "receipt_computational");
        assert!(actual.credential_valid);
        assert!(!actual.has_unknown_fields);
    }

    #[test]
    fn self_signed_producer_binding_needs_live_frontier_registration() {
        let finding = finding();
        let receipt = retained_receipt(&finding);
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        let decision_time = "2026-07-13T01:00:00Z";

        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "proof of possession alone is not frontier credential authority"
        );

        project.actors.push(ActorRecord {
            id: "agent:test".to_string(),
            public_key: hex::encode(
                SigningKey::from_bytes(&[0x37; 32])
                    .verifying_key()
                    .to_bytes(),
            ),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-12T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        assert!(receipt_producer_credential_valid(
            &project,
            &receipt,
            decision_time
        ));

        project.actors[0].created_at = "2026-07-13T00:30:00Z".to_string();
        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "registration after the producer binding cannot confer retroactive authority"
        );
        project.actors[0].created_at = "2026-07-12T00:00:00Z".to_string();

        project.actors[0].revoked_at = Some("2026-07-13T00:30:00Z".to_string());
        assert!(
            !receipt_producer_credential_valid(&project, &receipt, decision_time),
            "a key revoked before the decision must fail closed"
        );
    }

    #[test]
    fn existing_proposal_context_fails_closed_when_review_material_is_missing() {
        let finding = finding();
        let mut proposal = proposal(&finding);
        proposal.payload["finding"] = json!({
            "assertion": {
                "text": finding.assertion.text,
                "type": "theoretical",
            }
        });
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project.proposals.push(proposal.clone());

        let context = derive_existing_proposal_policy_context(&project, &proposal.id, None);

        assert_eq!(context.claim_class, "receipt_theoretical");
        assert_eq!(context.assurance_level, 0);
        assert_eq!(context.impact_tier, 4);
        assert_eq!(context.changed_findings, u32::MAX);
        assert_eq!(context.downstream_dependents, u32::MAX);
        assert!(context.has_unknown_fields);
        assert!(context.target_contested);
        assert!(!context.credential_valid);
        assert!(!context.independence_satisfied);
        assert!(!context.method_integrity_sound);
    }
}
