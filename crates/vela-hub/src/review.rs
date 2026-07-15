//! Review-queue projections for `GET /entries/{vfr_id}/review` (JSON).
//!
//! These are pure read-side projections over replayed frontier state —
//! what awaits a human key, what landed under a signed policy, what a
//! human key decided directly. They used to live in the presentation
//! tier (html.rs, deleted when the hub went protocol-only); the JSON
//! surface still needs them, so they live here on their own.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use vela_protocol::project::Project;

/// Match the frontier-local `ReviewProjection` page contract without
/// importing its filesystem-aware trust derivation into the Hub. The Hub is a
/// replay-only index, so it pages only the rows it already projects.
pub(crate) const REVIEW_PAGE_DEFAULT: usize = 25;
pub(crate) const REVIEW_PAGE_MAX: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewPageRequest {
    pub limit: usize,
    pub offset: usize,
}

impl Default for ReviewPageRequest {
    fn default() -> Self {
        Self {
            limit: REVIEW_PAGE_DEFAULT,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ReviewPageMetadata {
    pub limit: usize,
    pub offset: usize,
    pub returned: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}

impl ReviewPageMetadata {
    fn new(request: ReviewPageRequest, returned: usize, total: usize) -> Self {
        let consumed = request.offset.saturating_add(returned);
        Self {
            limit: request.limit,
            offset: request.offset,
            returned,
            total,
            next_offset: (consumed < total).then_some(consumed),
        }
    }
}

pub(crate) struct ReviewPage<T> {
    pub rows: Vec<T>,
    pub page: ReviewPageMetadata,
}

fn collect_page<T, U>(
    values: impl IntoIterator<Item = T>,
    request: ReviewPageRequest,
    mut project: impl FnMut(T) -> U,
) -> ReviewPage<U> {
    let mut total = 0usize;
    let mut rows = Vec::with_capacity(request.limit);
    for value in values {
        if total >= request.offset && rows.len() < request.limit {
            rows.push(project(value));
        }
        total = total.saturating_add(1);
    }
    ReviewPage {
        page: ReviewPageMetadata::new(request, rows.len(), total),
        rows,
    }
}

/// One row awaiting a human key. The hub mirrors replay state but does not own
/// the CLI's bounded receipt/policy read transaction, so it must not claim an
/// action is signable. It reports that eligibility was not evaluated and
/// leaves the authoritative Decision Brief to a frontier-local read.
pub(crate) struct ReviewQueueRow {
    pub lane: &'static str,
    pub id: String,
    pub title: String,
    pub why_here: String,
    pub accept_eligibility: &'static str,
    pub reject_eligibility: &'static str,
    /// Proposal-scoped pack context. Membership never expands the decision
    /// from this proposal to the rest of a pack.
    pub pack_memberships: vela_edge::sign_preview::PackMembershipProjection,
}

pub(crate) struct ReviewQueueView {
    pub rows: Vec<ReviewQueueRow>,
    pub page: ReviewPageMetadata,
    pub policy_active: bool,
    pub policy_id: Option<String>,
    /// False when this machine had no frontier checkout to evaluate the
    /// signed policy against: the page is then drawn from all pending
    /// proposals without policy filtering, and the response says so.
    pub policy_filtered: bool,
}

/// A bounded page of pending proposals, explicitly unfiltered. The hub does
/// not duplicate ReviewProjection filesystem reads or policy derivation, so
/// the surface degrades honestly instead of manufacturing a verdict from
/// partial state.
pub(crate) fn pending_review_fallback(
    project: &Project,
    request: ReviewPageRequest,
) -> ReviewQueueView {
    let projected = collect_page(
        project
            .proposals
            .iter()
            .filter(|p| p.status == "pending_review"),
        request,
        |p| ReviewQueueRow {
            lane: "decision",
            id: p.id.clone(),
            title: format!("{} · {}", p.kind, p.reason),
            why_here: "awaits a key-custody decision (policy routing not evaluated on this hub)"
                .to_string(),
            accept_eligibility: "not_evaluated",
            reject_eligibility: "not_evaluated",
            pack_memberships: vela_edge::sign_preview::proposal_pack_memberships(project, &p.id),
        },
    );
    ReviewQueueView {
        rows: projected.rows,
        page: projected.page,
        policy_active: false,
        policy_id: None,
        policy_filtered: false,
    }
}

/// One event admitted under a signed policy: the `policy_lane` payload
/// block is the marker (stamped into the event's content address at
/// landing; `vela check --strict` re-derives the Permit on replay).
pub(crate) struct PolicyAdmission {
    pub event_id: String,
    pub policy_id: String,
    pub rule_ids: Vec<String>,
    pub proposal_id: String,
    pub timestamp: String,
    pub target_type: String,
    pub target_id: String,
}

pub(crate) struct PolicyAdmissionPage {
    pub rows: Vec<PolicyAdmission>,
    pub page: ReviewPageMetadata,
    /// Counts over this page only. A full aggregate map could itself grow one
    /// key per event, so consumers aggregate bounded pages when they need it.
    pub by_policy: BTreeMap<String, usize>,
}

/// A bounded page of policy-admitted events, in canonical log order.
pub(crate) fn policy_admissions(
    project: &Project,
    request: ReviewPageRequest,
) -> PolicyAdmissionPage {
    use vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY;
    let projected = collect_page(
        project.events.iter().filter_map(|ev| {
            ev.payload
                .get(POLICY_LANE_PAYLOAD_KEY)
                .map(|lane| (ev, lane))
        }),
        request,
        |(ev, lane)| {
            let policy_id = lane
                .get("policy_id")
                .and_then(Value::as_str)
                .unwrap_or("(policy id missing)")
                .to_string();
            PolicyAdmission {
                event_id: ev.id.clone(),
                policy_id,
                rule_ids: lane
                    .get("rule_ids")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|rule| rule.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                proposal_id: lane
                    .get("certificate")
                    .and_then(|certificate| certificate.get("proposal_id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                timestamp: ev.timestamp.clone(),
                target_type: ev.target.r#type.clone(),
                target_id: ev.target.id.clone(),
            }
        },
    );
    let mut by_policy = BTreeMap::new();
    for admission in &projected.rows {
        *by_policy.entry(admission.policy_id.clone()).or_default() += 1;
    }
    PolicyAdmissionPage {
        rows: projected.rows,
        page: projected.page,
        by_policy,
    }
}

/// One accept/review event carried by a human key.
pub(crate) struct HumanDecision {
    pub event_id: String,
    pub kind: String,
    pub reviewer: String,
    pub timestamp: String,
    pub target_type: String,
    pub target_id: String,
}

/// A bounded page of accept/review events whose actor classifies as human (the
/// canonical `actor_kind`) AND that carry a signature. Newest first, preserving
/// the prior Hub presentation order. Kind gate: the `review.*` decision events
/// plus the `*.reviewed` verdict events.
pub(crate) fn human_decisions(
    project: &Project,
    request: ReviewPageRequest,
) -> ReviewPage<HumanDecision> {
    collect_page(
        project
            .events
            .iter()
            .rev()
            .filter(|ev| ev.signature.is_some())
            .filter(|ev| vela_protocol::events::actor_kind(&ev.actor.id) == "human")
            .filter(|ev| {
                let k = ev.kind.as_str();
                k.starts_with("review.") || k.ends_with(".reviewed")
            }),
        request,
        |ev| HumanDecision {
            event_id: ev.id.clone(),
            kind: ev.kind.as_str().to_string(),
            reviewer: ev.actor.id.clone(),
            timestamp: ev.timestamp.clone(),
            target_type: ev.target.r#type.clone(),
            target_id: ev.target.id.clone(),
        },
    )
}

#[cfg(test)]
mod review_projection_tests {
    use super::*;

    #[test]
    fn ten_thousand_row_page_counts_all_but_materializes_only_the_bounded_window() {
        let mut projected = 0usize;
        let page = collect_page(0..10_000, ReviewPageRequest::default(), |value| {
            projected += 1;
            value
        });
        assert_eq!(page.rows, (0..REVIEW_PAGE_DEFAULT).collect::<Vec<_>>());
        assert_eq!(page.page.total, 10_000);
        assert_eq!(page.page.returned, REVIEW_PAGE_DEFAULT);
        assert_eq!(page.page.next_offset, Some(REVIEW_PAGE_DEFAULT));
        assert_eq!(
            projected, REVIEW_PAGE_DEFAULT,
            "counting the catalog must not construct discarded row bodies"
        );

        let maximum = collect_page(
            0..150,
            ReviewPageRequest {
                limit: REVIEW_PAGE_MAX,
                offset: 25,
            },
            std::convert::identity,
        );
        assert_eq!(maximum.rows.len(), REVIEW_PAGE_MAX);
        assert_eq!(maximum.rows[0], 25);
        assert_eq!(maximum.rows[REVIEW_PAGE_MAX - 1], 124);
        assert_eq!(maximum.page.next_offset, Some(125));
    }

    /// The review projections over a minimal frontier: one pending
    /// proposal (the queue), one policy-lane accept (the autonomy
    /// ledger), one key-signed review (decided by key). All three
    /// projections must surface their row, and the fallback must declare
    /// itself unfiltered. (Formerly asserted through the HTML renderer;
    /// the projections are the JSON contract now.)
    #[test]
    fn queue_ledger_and_key_decisions_project() {
        let mut project =
            vela_protocol::project::assemble("review-fixture", vec![], 10, 0, "Test project");
        project.proposals.push(
            serde_json::from_value(serde_json::json!({
                "id": "vpr_pending1",
                "kind": "finding.assert",
                "target": {"type": "finding", "id": "vf_new"},
                "actor": {"id": "agent:scout", "type": "agent"},
                "created_at": "2026-07-01T00:00:00Z",
                "reason": "a new bound to weigh",
                "status": "pending_review",
            }))
            .expect("minimal pending proposal"),
        );

        let mut policy_ev = project.events[0].clone();
        policy_ev.id = "vev_policy1".to_string();
        policy_ev.kind = "review.accepted".into();
        policy_ev.actor.id = "policy:vap_test01".to_string();
        policy_ev.actor.r#type = "agent".to_string();
        policy_ev.target.r#type = "proposal".to_string();
        policy_ev.target.id = "vpr_landed1".to_string();
        policy_ev.payload = serde_json::json!({
            "policy_lane": {
                "policy_id": "vap_test01",
                "rule_ids": ["exact-lane-a2"],
                "certificate": {"proposal_id": "vpr_landed1"},
                "context": {},
            }
        });
        project.events.push(policy_ev);

        let mut human_ev = project.events[0].clone();
        human_ev.id = "vev_human1".to_string();
        human_ev.kind = "review.accepted".into();
        human_ev.actor.id = "reviewer:will".to_string();
        human_ev.actor.r#type = "human".to_string();
        human_ev.target.r#type = "proposal".to_string();
        human_ev.target.id = "vpr_decided1".to_string();
        human_ev.signature = Some("ed25519:test-signature".to_string());
        project.events.push(human_ev);

        let request = ReviewPageRequest::default();
        let queue = pending_review_fallback(&project, request);
        assert_eq!(queue.rows.len(), 1, "one pending proposal in the queue");
        assert_eq!(queue.page.total, 1);
        assert_eq!(queue.rows[0].id, "vpr_pending1");
        assert_eq!(queue.rows[0].lane, "decision");
        assert_eq!(queue.rows[0].accept_eligibility, "not_evaluated");
        assert_eq!(queue.rows[0].reject_eligibility, "not_evaluated");
        assert_eq!(queue.rows[0].pack_memberships.total, 0);
        assert!(
            !queue.policy_filtered,
            "fallback declares itself unfiltered"
        );
        assert!(!queue.policy_active);

        let admissions = policy_admissions(&project, request);
        assert_eq!(admissions.rows.len(), 1, "one policy-lane admission");
        assert_eq!(admissions.page.total, 1);
        assert_eq!(admissions.rows[0].policy_id, "vap_test01");
        assert_eq!(
            admissions.rows[0].rule_ids,
            vec!["exact-lane-a2".to_string()]
        );
        assert_eq!(admissions.rows[0].proposal_id, "vpr_landed1");
        assert_eq!(admissions.by_policy["vap_test01"], 1);

        let decisions = human_decisions(&project, request);
        assert_eq!(decisions.rows.len(), 1, "one key-signed human decision");
        assert_eq!(decisions.page.total, 1);
        assert_eq!(decisions.rows[0].reviewer, "reviewer:will");
        assert_eq!(decisions.rows[0].kind, "review.accepted");
    }

    #[test]
    fn every_review_ledger_is_bounded_and_independently_pageable() {
        let mut project =
            vela_protocol::project::assemble("review-pages", vec![], 10, 0, "Review pages");
        let template = project.events[0].clone();
        for index in 0..4 {
            project.proposals.push(
                serde_json::from_value(serde_json::json!({
                    "id": format!("vpr_pending{index}"),
                    "kind": "finding.assert",
                    "target": {"type": "finding", "id": format!("vf_{index}")},
                    "actor": {"id": "agent:scout", "type": "agent"},
                    "created_at": format!("2026-07-01T00:00:0{index}Z"),
                    "reason": format!("pending result {index}"),
                    "status": "pending_review",
                }))
                .expect("pending proposal"),
            );

            let mut policy = template.clone();
            policy.id = format!("vev_policy{index}");
            policy.payload = serde_json::json!({
                "policy_lane": {
                    "policy_id": "vap_page",
                    "rule_ids": ["exact"],
                    "certificate": {"proposal_id": format!("vpr_policy{index}")},
                }
            });
            project.events.push(policy);

            let mut human = template.clone();
            human.id = format!("vev_human{index}");
            human.kind = "review.accepted".into();
            human.actor.id = "reviewer:will".into();
            human.actor.r#type = "human".into();
            human.signature = Some(format!("ed25519:test-{index}"));
            project.events.push(human);
        }

        let request = ReviewPageRequest {
            limit: 2,
            offset: 1,
        };
        let queue = pending_review_fallback(&project, request);
        assert_eq!(
            queue
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["vpr_pending1", "vpr_pending2"]
        );
        assert_eq!(
            queue.page,
            ReviewPageMetadata {
                limit: 2,
                offset: 1,
                returned: 2,
                total: 4,
                next_offset: Some(3),
            }
        );

        let admissions = policy_admissions(&project, request);
        assert_eq!(
            admissions
                .rows
                .iter()
                .map(|row| row.event_id.as_str())
                .collect::<Vec<_>>(),
            ["vev_policy1", "vev_policy2"]
        );
        assert_eq!(admissions.page.total, 4);
        assert_eq!(admissions.page.next_offset, Some(3));
        assert_eq!(admissions.by_policy["vap_page"], 2);

        let decisions = human_decisions(&project, request);
        assert_eq!(
            decisions
                .rows
                .iter()
                .map(|row| row.event_id.as_str())
                .collect::<Vec<_>>(),
            ["vev_human2", "vev_human1"]
        );
        assert_eq!(decisions.page.total, 4);
        assert_eq!(decisions.page.next_offset, Some(3));
    }
}
