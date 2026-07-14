//! Pure pack-membership impact for Decision Brief consumers.
//!
//! A released diff pack is context around a proposal, not implicit authority
//! to decide every member. This adapter reads only replayed `Project` state;
//! it performs no filesystem I/O and never claims proposal-scoped decisions
//! apply to a set.

use serde::Serialize;
use vela_protocol::project::Project;

const PACK_BUDGET: usize = 32;
const SUMMARY_BUDGET: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackMembership {
    pub pack_id: String,
    pub summary: String,
    pub summary_full_root: String,
    pub summary_truncated: bool,
    pub aggregate_kind: String,
    pub member_count: usize,
    pub proposal_position: usize,
    pub verdict: Option<String>,
    pub decision_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackMembershipProjection {
    pub schema: String,
    pub full_root: String,
    pub total: usize,
    pub omitted: usize,
    pub items: Vec<PackMembership>,
}

/// Derive bounded pack context for one proposal from canonical replay state.
#[must_use]
pub fn proposal_pack_memberships(project: &Project, proposal_id: &str) -> PackMembershipProjection {
    let mut records = project
        .released_diff_packs
        .iter()
        .filter_map(|record| {
            record
                .member_proposals
                .iter()
                .position(|member| member == proposal_id)
                .map(|position| (record, position))
        })
        .collect::<Vec<_>>();
    records.sort_by(|(left, _), (right, _)| left.pack_id.cmp(&right.pack_id));
    let full_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(
            &records
                .iter()
                .map(|(record, position)| serde_json::json!({
                    "record": record,
                    "proposal_position": position,
                }))
                .collect::<Vec<_>>()
        )
        .expect("serde_json::Value canonicalization is infallible")
    );
    let total = records.len();
    let items = records
        .into_iter()
        .take(PACK_BUDGET)
        .map(|(record, position)| {
            let (summary, summary_truncated) = truncate_utf8(&record.summary, SUMMARY_BUDGET);
            PackMembership {
                pack_id: record.pack_id.clone(),
                summary,
                summary_full_root: text_root(&record.summary),
                summary_truncated,
                aggregate_kind: record.aggregate_kind.clone(),
                member_count: record.member_proposals.len(),
                proposal_position: position,
                verdict: record
                    .verdict
                    .map(|verdict| verdict.canonical().to_string()),
                decision_scope: "proposal_only".to_string(),
            }
        })
        .collect::<Vec<_>>();
    PackMembershipProjection {
        schema: "vela.pack-membership-impact.testing.v1".to_string(),
        full_root,
        total,
        omitted: total.saturating_sub(items.len()),
        items,
    }
}

fn truncate_utf8(value: &str, limit: usize) -> (String, bool) {
    if value.len() <= limit {
        return (value.to_string(), false);
    }
    let mut end = limit;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &value[..end]), true)
}

fn text_root(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(b"vela.pack-membership-text.v1");
    digest.update([0]);
    digest.update(value.as_bytes());
    format!("sha256:{}", hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_protocol::released_diff_pack::ReleasedDiffPackRecord;

    #[test]
    fn pack_membership_is_bounded_pure_and_proposal_scoped() {
        let mut project = vela_protocol::project::assemble("test", vec![], 0, 0, "test");
        project
            .released_diff_packs
            .push(ReleasedDiffPackRecord::from_released_event(
                "vsd_test".to_string(),
                "vfr_test".to_string(),
                "x".repeat(2_000),
                "mixed".to_string(),
                "2026-07-13T00:00:00Z".to_string(),
                "vev_test".to_string(),
                vec!["vpr_other".to_string(), "vpr_target".to_string()],
            ));

        let projection = proposal_pack_memberships(&project, "vpr_target");

        assert_eq!(projection.total, 1);
        assert_eq!(projection.items[0].proposal_position, 1);
        assert_eq!(projection.items[0].decision_scope, "proposal_only");
        assert!(projection.items[0].summary_truncated);
        assert!(
            !serde_json::to_string(&projection)
                .unwrap()
                .contains("decides the set")
        );
    }
}
