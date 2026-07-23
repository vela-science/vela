//! Shared test fixtures for the protocol record types.
//!
//! Available to this crate's own `#[cfg(test)]` code and, via the
//! `test-support` feature, to downstream crates' tests (e.g. vela-edge tests
//! that need a `Project`/`FindingBundle` to exercise edge behavior over real
//! records). It is never compiled into a normal build, so it adds nothing to
//! the protocol's public surface or the narrow waist.

use crate::bundle::*;
use crate::project::{self, Project};
use crate::verifier_attachment::{
    AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, ProbeKind, ProbeResult,
    VerifierAttachment, VerifierMethod, claim_digest,
};

/// A synthetic, fully-populated finding with one entity and a raw-confidence
/// prior. `score` sets the confidence value.
pub fn make_finding(id: &str, score: f64, assertion_type: &str) -> FindingBundle {
    FindingBundle {
        id: id.into(),
        version: 1,
        previous_version: None,
        assertion: Assertion {
            text: format!("Finding {id}"),
            assertion_type: assertion_type.into(),
            entities: vec![Entity {
                name: "A309370".into(),
                entity_type: "sequence".into(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: vec![],
                aliases: vec![],
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
            evidence_type: "experimental".into(),
            model_system: String::new(),
            method: String::new(),
            replicated: false,
            replication_count: None,
            evidence_spans: vec![],
        },
        conditions: Conditions {
            text: String::new(),
            duration: None,
        },
        confidence: Confidence::raw(score, "seeded prior", 0.85),
        provenance: Provenance {
            source_type: "published_paper".into(),
            doi: None,
            url: None,
            title: "Test".into(),
            authors: vec![],
            year: Some(2024),
            license: None,
            publisher: None,
            funders: vec![],
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
        links: vec![],
        annotations: vec![],
        attachments: vec![],
        created: String::new(),
        updated: None,
        access_tier: crate::access_tier::AccessTier::Public,
    }
}

/// Assemble a `Project` from findings, with placeholder counts and description.
pub fn make_project(name: &str, findings: Vec<FindingBundle>) -> Project {
    project::assemble(name, findings, 10, 0, "Test project")
}

/// Two claim-matched, independently declared attachments that derive a
/// [`crate::verifier_attachment::GateStatus::Verified`] gate for `finding`.
/// Review-state tests use this to prove that verification remains orthogonal to
/// authority-bearing review verdicts.
pub fn verified_attachment_pair(finding: &FindingBundle) -> Vec<VerifierAttachment> {
    let digest = claim_digest(&finding.assertion.text);
    let probe = AdversarialProbe {
        kind: ProbeKind::CounterexampleSearch,
        result: ProbeResult::Survived,
        note: String::new(),
        evidence_root: String::new(),
    };
    let first = VerifierAttachment::build(AttachmentDraft {
        target: finding.id.clone(),
        claim_digest: digest.clone(),
        verifier_method: VerifierMethod::ComputationalSearch,
        solver_id: "solver-a".to_string(),
        independent_of: vec![],
        match_to_claim: MatchToClaim {
            matches: true,
            checker_actor: "verifier:a".to_string(),
        },
        adversarial_probes: vec![probe.clone()],
        outcome: AttachmentOutcome::Passed,
        verifier_actor: "verifier:a".to_string(),
        note: String::new(),
    })
    .expect("build first verified attachment fixture");
    let second = VerifierAttachment::build(AttachmentDraft {
        target: finding.id.clone(),
        claim_digest: digest,
        verifier_method: VerifierMethod::ExactArithmeticRecompute,
        solver_id: "solver-b".to_string(),
        independent_of: vec![first.id.clone()],
        match_to_claim: MatchToClaim {
            matches: true,
            checker_actor: "verifier:b".to_string(),
        },
        adversarial_probes: vec![probe],
        outcome: AttachmentOutcome::Passed,
        verifier_actor: "verifier:b".to_string(),
        note: String::new(),
    })
    .expect("build second verified attachment fixture");
    vec![first, second]
}
