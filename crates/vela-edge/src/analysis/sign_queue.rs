//! Pure adapter for the human sign-session queue.
//!
//! Decision items arrive as already-built [`ReviewSnapshot`] values. This
//! module does not read a frontier, inspect policy files, evaluate policy,
//! preview packs, or derive another view of the evidence. Judgment, hygiene,
//! and detached-artifact work are separate caller-supplied lanes because their
//! discovery belongs at the filesystem or domain boundary.
//!
//! The queue preserves one fixed presentation order:
//!
//! 1. judgment products only a human can produce;
//! 2. proposal-scoped decisions;
//! 3. unsigned-event hygiene;
//! 4. detached governance artifacts.
//!
//! Accept and Reject eligibility remain action-specific facts in the embedded
//! Decision Brief. Skip is intentionally absent from [`SignDecisionAction`]:
//! it is session navigation, not a scientific decision. A review snapshot is
//! proposal-scoped; this adapter never infers that acting on one pack member
//! acts on any other member.

use serde::Serialize;

use super::decision_brief::{DecisionAction, ReviewSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignLane {
    Judgment,
    Decision,
    Hygiene,
    Detached,
}

/// The only decision actions exposed by the queue adapter.
///
/// Whether either action is currently available is read independently from
/// the item's Decision Brief. In particular, a blocked Accept does not imply a
/// blocked Reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignDecisionAction {
    Accept,
    Reject,
}

impl SignDecisionAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

/// Caller-supplied work for a non-decision lane.
///
/// Discovery of these items may require domain or filesystem knowledge. The
/// edge adapter receives only the completed display material and never tries
/// to rediscover it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupplementalSignItem {
    pub id: String,
    pub title: String,
    pub why_here: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preview: Vec<String>,
}

impl SupplementalSignItem {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        why_here: impl Into<String>,
        preview: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            why_here: why_here.into(),
            preview,
        }
    }
}

/// One queue row.
///
/// Decision rows retain the complete, already-derived review snapshot. The
/// summary fields are presentation indexes over that snapshot, not a second
/// policy or evidence evaluation. Supplemental rows have no `review` and are
/// copied unchanged from their explicitly supplied lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignItem {
    pub lane: SignLane,
    pub id: String,
    pub title: String,
    pub why_here: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preview: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewSnapshot>,
}

impl SignItem {
    fn supplemental(lane: SignLane, item: SupplementalSignItem) -> Self {
        debug_assert!(lane != SignLane::Decision);
        Self {
            lane,
            id: item.id,
            title: item.title,
            why_here: item.why_here,
            preview: item.preview,
            review: None,
        }
    }

    fn decision(review: ReviewSnapshot) -> Self {
        let id = review.brief.audit.proposal_id.clone();
        let title = review.brief.change.claim.clone();
        let why_here = if review.brief.authority.why_human.is_empty() {
            review.brief.authority.route.clone()
        } else {
            review.brief.authority.why_human.join("; ")
        };
        Self {
            lane: SignLane::Decision,
            id,
            title,
            why_here,
            preview: Vec::new(),
            review: Some(review),
        }
    }

    /// Return the existing Decision Brief entry for one explicit action.
    ///
    /// There is deliberately no aggregate `signable` answer: Accept and
    /// Reject can have different eligibility. Supplemental lanes return
    /// `None` because their ceremony is not a proposal decision.
    #[must_use]
    pub fn decision_action(&self, action: SignDecisionAction) -> Option<&DecisionAction> {
        self.review
            .as_ref()?
            .brief
            .authority
            .actions
            .iter()
            .find(|candidate| candidate.action == action.as_str())
    }

    #[must_use]
    pub fn accept_action(&self) -> Option<&DecisionAction> {
        self.decision_action(SignDecisionAction::Accept)
    }

    #[must_use]
    pub fn reject_action(&self) -> Option<&DecisionAction> {
        self.decision_action(SignDecisionAction::Reject)
    }
}

/// Completed inputs from the read-side boundary.
///
/// The four vectors are intentionally separate. This makes lane ownership
/// explicit and prevents the pure adapter from reaching back into a frontier
/// to discover hygiene or detached-signature work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignQueueInput {
    pub judgments: Vec<SupplementalSignItem>,
    pub decisions: Vec<ReviewSnapshot>,
    pub hygiene: Vec<SupplementalSignItem>,
    pub detached: Vec<SupplementalSignItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SignQueue {
    pub items: Vec<SignItem>,
}

/// Assemble a queue without I/O, evaluation, filtering, or mutation.
///
/// Decision order is the caller's already-selected ReviewSnapshot order. The
/// adapter changes only lane placement and transfers ownership of each item.
#[must_use]
pub fn sign_queue(input: SignQueueInput) -> SignQueue {
    let capacity =
        input.judgments.len() + input.decisions.len() + input.hygiene.len() + input.detached.len();
    let mut items = Vec::with_capacity(capacity);
    items.extend(
        input
            .judgments
            .into_iter()
            .map(|item| SignItem::supplemental(SignLane::Judgment, item)),
    );
    items.extend(input.decisions.into_iter().map(SignItem::decision));
    items.extend(
        input
            .hygiene
            .into_iter()
            .map(|item| SignItem::supplemental(SignLane::Hygiene, item)),
    );
    items.extend(
        input
            .detached
            .into_iter()
            .map(|item| SignItem::supplemental(SignLane::Detached, item)),
    );
    SignQueue { items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision_brief::{
        ClaimState, CorrectionPath, DecisionAudit, DecisionBasis, DecisionBindingFacts,
        DecisionBrief, DecisionChange, DecisionCheckState, DecisionFacets, DecisionImpact,
        DecisionSubject, DownstreamEffect, FixedBase, FrontierReference, ReviewSortKey,
    };

    fn snapshot(
        proposal_id: &str,
        claim: &str,
        accept_eligibility: &str,
        accept_reasons: Vec<&str>,
    ) -> ReviewSnapshot {
        let root = format!("sha256:{proposal_id}");
        ReviewSnapshot {
            observed_at: "2026-07-14T12:00:00Z".to_string(),
            event_log_root: "sha256:event-log".to_string(),
            sort_key: ReviewSortKey {
                created_at: "2026-07-14T11:00:00Z".to_string(),
                proposal_id: proposal_id.to_string(),
            },
            proposal_actor: "agent:queue-test".to_string(),
            brief: DecisionBrief {
                schema: "vela.decision-brief.testing.v1".to_string(),
                stability: "testing".to_string(),
                change: DecisionChange {
                    subject: DecisionSubject {
                        subject_type: "finding".to_string(),
                        id: "vf_test".to_string(),
                    },
                    fixed_base: FixedBase {
                        event_log_root: "sha256:event-log".to_string(),
                        receipt_event_log_root: Some("sha256:event-log".to_string()),
                    },
                    claim: claim.to_string(),
                    before: None,
                    after: Some(ClaimState {
                        id: "vf_test".to_string(),
                        claim_type: "computational".to_string(),
                        text: claim.to_string(),
                    }),
                    requested_action: "finding.add".to_string(),
                },
                basis: DecisionBasis {
                    primary_evidence_roots: Vec::new(),
                    check_state: DecisionCheckState {
                        gate_status: "needs_verification".to_string(),
                        gate_reasons: vec!["no_durable_verifier".to_string()],
                        durable_verifier_count: 0,
                        durable_verifier_snapshot_root: "sha256:verifiers".to_string(),
                        engine_status: Some("pass".to_string()),
                        engine_new_blocking: Vec::new(),
                        engine_new_warnings: Vec::new(),
                        producer_reported: Vec::new(),
                    },
                    main_caveat: Some("bounded case only".to_string()),
                    attributed_interpretation: None,
                },
                impact: DecisionImpact {
                    downstream_effect: DownstreamEffect {
                        changed_findings: 1,
                        downstream_dependents: 0,
                        impact_tier: 1,
                    },
                    correction_path: CorrectionPath {
                        while_pending: vec!["withdraw proposal".to_string()],
                        after_acceptance: vec!["retract finding".to_string()],
                    },
                    critical_warnings: Vec::new(),
                },
                authority: crate::decision_brief::DecisionAuthority {
                    frontier: FrontierReference {
                        id: Some("vfr_test".to_string()),
                        name: "Test frontier".to_string(),
                    },
                    route: "defer".to_string(),
                    scope: "hypothesis_only".to_string(),
                    why_human: vec!["human_scientific_judgment".to_string()],
                    actions: vec![
                        DecisionAction {
                            action: "accept".to_string(),
                            eligibility: accept_eligibility.to_string(),
                            reasons: accept_reasons.into_iter().map(str::to_string).collect(),
                        },
                        DecisionAction {
                            action: "reject".to_string(),
                            eligibility: "available".to_string(),
                            reasons: Vec::new(),
                        },
                    ],
                },
                audit: DecisionAudit {
                    observed_at: "2026-07-14T12:00:00Z".to_string(),
                    proposal_id: proposal_id.to_string(),
                    proposal_root: root,
                    decision_facts_root: "sha256:decision-facts".to_string(),
                    receipt_root: None,
                    declared_receipt_root: None,
                    artifact_root: None,
                    policy_input_root: "sha256:policy-input".to_string(),
                    policy_result_root: "sha256:policy-result".to_string(),
                    publication_root: None,
                    raw_references_root: "sha256:references".to_string(),
                    raw_references: Vec::new(),
                    raw_references_truncated: 0,
                    missing_root: "sha256:missing".to_string(),
                    missing_truncated: 0,
                    truncations: Vec::new(),
                },
                missing: Vec::new(),
                facets: DecisionFacets::default(),
            },
            decision_bindings: DecisionBindingFacts {
                proposal_root: format!("sha256:{proposal_id}"),
                receipt_observation_root: "sha256:receipt-observation".to_string(),
                receipt_root: None,
                evidence_or_reference_root: "sha256:evidence".to_string(),
                evidence_availability: "missing".to_string(),
                verifier_snapshot_root: "sha256:verifiers".to_string(),
                policy_input_root: "sha256:policy-input".to_string(),
                policy_result_root: "sha256:policy-result".to_string(),
                engine_gate_root: "sha256:engine-gate".to_string(),
                semantic_effect_root: "sha256:semantic-effect".to_string(),
                downstream_impact_root: "sha256:downstream-impact".to_string(),
            },
        }
    }

    fn supplemental(id: &str) -> SupplementalSignItem {
        SupplementalSignItem::new(
            id,
            format!("title for {id}"),
            format!("reason for {id}"),
            vec![format!("preview for {id}")],
        )
    }

    #[test]
    fn assembles_fixed_lanes_without_reordering_decisions() {
        let queue = sign_queue(SignQueueInput {
            judgments: vec![supplemental("judgment")],
            decisions: vec![
                snapshot("vpr_second", "Second supplied review", "available", vec![]),
                snapshot("vpr_first", "First supplied review", "available", vec![]),
            ],
            hygiene: vec![supplemental("hygiene")],
            detached: vec![supplemental("detached")],
        });

        assert_eq!(
            queue.items.iter().map(|item| item.lane).collect::<Vec<_>>(),
            [
                SignLane::Judgment,
                SignLane::Decision,
                SignLane::Decision,
                SignLane::Hygiene,
                SignLane::Detached,
            ]
        );
        assert_eq!(queue.items[1].id, "vpr_second");
        assert_eq!(queue.items[2].id, "vpr_first");
        assert!(queue.items[0].review.is_none());
        assert!(queue.items[1].review.is_some());
        assert_eq!(queue.items[4].preview, ["preview for detached"]);
    }

    #[test]
    fn accept_and_reject_eligibility_are_independent() {
        let queue = sign_queue(SignQueueInput {
            decisions: vec![snapshot(
                "vpr_blocked_accept",
                "A claim that can still be rejected",
                "blocked",
                vec!["policy_denied"],
            )],
            ..SignQueueInput::default()
        });
        let item = &queue.items[0];

        let accept = item.accept_action().expect("accept action is explicit");
        assert_eq!(accept.eligibility, "blocked");
        assert_eq!(accept.reasons, ["policy_denied"]);
        let reject = item.reject_action().expect("reject action is explicit");
        assert_eq!(reject.eligibility, "available");
        assert!(reject.reasons.is_empty());

        let json = serde_json::to_value(item).unwrap();
        assert_eq!(
            json["review"]["brief"]["authority"]["actions"][0]["action"],
            "accept"
        );
        assert_eq!(
            json["review"]["brief"]["authority"]["actions"][1]["action"],
            "reject"
        );
        assert!(!json.to_string().contains("\"skip\""));
        assert!(json.get("signable").is_none());
    }

    #[test]
    fn decision_rows_remain_proposal_scoped() {
        let queue = sign_queue(SignQueueInput {
            decisions: vec![snapshot(
                "vpr_pack_member",
                "One proposal in a larger transport grouping",
                "available",
                vec![],
            )],
            ..SignQueueInput::default()
        });
        let json = serde_json::to_value(&queue.items[0]).unwrap();

        assert_eq!(json["id"], "vpr_pack_member");
        assert!(json.get("pack").is_none());
        assert!(json.get("pack_preview").is_none());
    }
}
