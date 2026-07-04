//! Review-queue projections for `GET /entries/{vfr_id}/review` (JSON).
//!
//! These are pure read-side projections over replayed frontier state —
//! what awaits a human key, what landed under a signed policy, what a
//! human key decided directly. They used to live in the presentation
//! tier (html.rs, deleted when the hub went protocol-only); the JSON
//! surface still needs them, so they live here on their own.

use serde_json::Value;
use vela_protocol::project::Project;

pub(crate) const HUMAN_DECISIONS_SHOWN: usize = 20;

/// One row awaiting a human key. Mirrors `vela_edge::sign_queue::SignItem`
/// narrowed to the two lanes the review surface reports (judgment,
/// decision), so the fallback path — no frontier checkout on this hub
/// machine — can build the same rows straight from `Project.proposals`.
pub(crate) struct ReviewQueueRow {
    pub lane: &'static str,
    pub id: String,
    pub title: String,
    pub why_here: String,
    /// False for policy-Denied items: shown, never signable.
    pub signable: bool,
    /// Pack id when the item decides a whole changeset.
    pub pack: Option<String>,
}

pub(crate) struct ReviewQueueView {
    pub rows: Vec<ReviewQueueRow>,
    pub policy_active: bool,
    pub policy_id: Option<String>,
    /// False when this machine had no frontier checkout to evaluate the
    /// signed policy against: the rows are then every pending proposal,
    /// unfiltered, and the response says so.
    pub policy_filtered: bool,
}

/// Narrow the CLI's sign-queue projection to the review surface's lanes.
/// Hygiene and Detached are operator ceremonies on the frontier's own
/// machine — not decisions a reader of this surface can weigh.
pub(crate) fn review_queue_from_sign_queue(q: vela_edge::sign_queue::SignQueue) -> ReviewQueueView {
    use vela_edge::sign_queue::SignLane;
    let rows = q
        .items
        .into_iter()
        .filter_map(|item| {
            let lane = match item.lane {
                SignLane::Judgment => "judgment",
                SignLane::Decision => "decision",
                SignLane::Hygiene | SignLane::Detached => return None,
            };
            Some(ReviewQueueRow {
                lane,
                id: item.id,
                title: item.title,
                why_here: item.why_here,
                signable: item.signable,
                pack: item.pack,
            })
        })
        .collect();
    ReviewQueueView {
        rows,
        policy_active: q.policy_active,
        policy_id: q.policy_id,
        policy_filtered: true,
    }
}

/// The checkout-less fallback: every pending proposal, unfiltered. Same
/// row shape and the same pack resolution the sign queue uses, so the
/// surface degrades honestly instead of vanishing.
pub(crate) fn pending_review_fallback(project: &Project) -> ReviewQueueView {
    let pack_of = |proposal_id: &str| -> Option<String> {
        project
            .released_diff_packs
            .iter()
            .find(|p| p.verdict.is_none() && p.member_proposals.iter().any(|m| m == proposal_id))
            .map(|p| p.pack_id.clone())
    };
    let rows = project
        .proposals
        .iter()
        .filter(|p| p.status == "pending_review")
        .map(|p| ReviewQueueRow {
            lane: "decision",
            id: p.id.clone(),
            title: format!("{} · {}", p.kind, p.reason),
            why_here: "awaits a key-custody decision (policy routing not evaluated on this hub)"
                .to_string(),
            signable: true,
            pack: pack_of(&p.id),
        })
        .collect();
    ReviewQueueView {
        rows,
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

/// Every policy-admitted event on the frontier, in log order.
pub(crate) fn policy_admissions(project: &Project) -> Vec<PolicyAdmission> {
    use vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY;
    project
        .events
        .iter()
        .filter_map(|ev| {
            let lane = ev.payload.get(POLICY_LANE_PAYLOAD_KEY)?;
            let policy_id = lane
                .get("policy_id")
                .and_then(Value::as_str)
                .unwrap_or("(policy id missing)")
                .to_string();
            let rule_ids = lane
                .get("rule_ids")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|r| r.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let proposal_id = lane
                .get("certificate")
                .and_then(|c| c.get("proposal_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(PolicyAdmission {
                event_id: ev.id.clone(),
                policy_id,
                rule_ids,
                proposal_id,
                timestamp: ev.timestamp.clone(),
                target_type: ev.target.r#type.clone(),
                target_id: ev.target.id.clone(),
            })
        })
        .collect()
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

/// Accept/review events whose actor classifies as human (the canonical
/// `actor_kind`) AND that carry a signature. Log order (oldest first);
/// callers slice the tail for "recent". Kind gate: the `review.*`
/// decision events plus the `*.reviewed` verdict events.
pub(crate) fn human_decisions(project: &Project) -> Vec<HumanDecision> {
    project
        .events
        .iter()
        .filter(|ev| ev.signature.is_some())
        .filter(|ev| vela_protocol::events::actor_kind(&ev.actor.id) == "human")
        .filter(|ev| {
            let k = ev.kind.as_str();
            k.starts_with("review.") || k.ends_with(".reviewed")
        })
        .map(|ev| HumanDecision {
            event_id: ev.id.clone(),
            kind: ev.kind.as_str().to_string(),
            reviewer: ev.actor.id.clone(),
            timestamp: ev.timestamp.clone(),
            target_type: ev.target.r#type.clone(),
            target_id: ev.target.id.clone(),
        })
        .collect()
}

#[cfg(test)]
mod review_projection_tests {
    use super::*;

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

        let queue = pending_review_fallback(&project);
        assert_eq!(queue.rows.len(), 1, "one pending proposal in the queue");
        assert_eq!(queue.rows[0].id, "vpr_pending1");
        assert_eq!(queue.rows[0].lane, "decision");
        assert!(queue.rows[0].signable);
        assert!(
            !queue.policy_filtered,
            "fallback declares itself unfiltered"
        );
        assert!(!queue.policy_active);

        let admissions = policy_admissions(&project);
        assert_eq!(admissions.len(), 1, "one policy-lane admission");
        assert_eq!(admissions[0].policy_id, "vap_test01");
        assert_eq!(admissions[0].rule_ids, vec!["exact-lane-a2".to_string()]);
        assert_eq!(admissions[0].proposal_id, "vpr_landed1");

        let decisions = human_decisions(&project);
        assert_eq!(decisions.len(), 1, "one key-signed human decision");
        assert_eq!(decisions[0].reviewer, "reviewer:will");
        assert_eq!(decisions[0].kind, "review.accepted");
    }
}
