//! The pack-level semantic preview a signer sees before the one confirm.
//!
//! When a sign-queue item is a member of a released diff pack, deciding it
//! decides the set — so the ceremony should show what the SET does to the
//! record, not only the one proposal's claim: the state operations, the
//! evidence polarity, each affected finding's derived gate status, and the
//! graded blast radius. Everything here is a projection over data that
//! already exists (`ScientificDiffPack::review_summary`, the polarity
//! classifier, `derive_gate_status`, `blast_radius_graded`); nothing is
//! computed by a model, nothing is stored, and rendering it adds no
//! prompts — the ceremony keeps one confirm and one key read.
//!
//! Honest fallback: a pack that cannot be loaded yields `None` and the
//! ceremony keeps its old one-line pack chip; anything uncomputable inside
//! a loaded pack lands in `missing`, rendered as one line, never an error.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use vela_protocol::evidence_polarity::classify_proposal_polarity;
use vela_protocol::frontier_graph::{BlastDirection, FrontierGraph};
use vela_protocol::project::Project;
use vela_protocol::scientific_diff::ScientificDiffPack;
use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};

/// Findings shown in the gate matrix / blast aggregate. A pack touching
/// more still shows the first N plus an honest remainder count.
const FINDING_BUDGET: usize = 5;
/// State operations shown before "…and N more".
const OP_BUDGET: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct GateRow {
    pub finding_id: String,
    pub status: String,
    pub reason_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlastLine {
    pub weakened: usize,
    pub support_killed: usize,
    pub downstream_candidates: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackSignPreview {
    pub pack_id: String,
    /// One line: members, findings touched, count deltas.
    pub scope: String,
    /// Up to [`OP_BUDGET`] semantic operations plus a remainder marker.
    pub state_ops: Vec<String>,
    /// Polarity name → count over the pack's member proposals.
    pub polarity: BTreeMap<String, usize>,
    /// Derived gate status per affected finding (first [`FINDING_BUDGET`]).
    pub gate_matrix: Vec<GateRow>,
    /// Graded downstream impact aggregated over the shown findings.
    pub blast: Option<BlastLine>,
    /// What could not be computed, stated plainly.
    pub missing: Vec<String>,
}

/// Build the preview for one released pack. `None` when the pack file is
/// absent or unreadable — the caller falls back to the plain pack chip.
#[must_use]
pub fn pack_sign_preview(
    project: &Project,
    frontier_dir: &Path,
    pack_id: &str,
) -> Option<PackSignPreview> {
    let path = frontier_dir
        .join(".vela")
        .join("diff_packs")
        .join(format!("{pack_id}.json"));
    let body = std::fs::read_to_string(path).ok()?;
    let pack: ScientificDiffPack = serde_json::from_str(&body).ok()?;
    let summary = pack.review_summary(frontier_dir);

    let mut missing = Vec::new();

    let counts = &summary.preview_counts;
    let scope = format!(
        "{} proposal(s) · {} finding(s) touched · findings {:+} · events {:+}",
        summary.members,
        summary.affected_findings.len(),
        counts.findings_delta,
        counts.events_delta,
    );

    let mut state_ops: Vec<String> = summary
        .proposed_operations
        .iter()
        .take(OP_BUDGET)
        .map(|op| match &op.target_id {
            Some(target) => format!("{} {}", op.operation_class, target),
            None => op.operation_class.clone(),
        })
        .collect();
    if summary.proposed_operations.len() > OP_BUDGET {
        state_ops.push(format!(
            "…and {} more",
            summary.proposed_operations.len() - OP_BUDGET
        ));
    }

    let mut polarity: BTreeMap<String, usize> = BTreeMap::new();
    for member in &pack.proposals {
        match project.proposals.iter().find(|p| &p.id == member) {
            Some(p) => {
                let pol = classify_proposal_polarity(&p.kind, &p.payload);
                *polarity.entry(pol.as_str().to_string()).or_insert(0) += 1;
            }
            None => missing.push(format!("member {member} not in projection")),
        }
    }

    let shown_findings: Vec<&String> = summary
        .affected_findings
        .iter()
        .take(FINDING_BUDGET)
        .collect();
    if summary.affected_findings.len() > FINDING_BUDGET {
        missing.push(format!(
            "{} more affected finding(s) not shown",
            summary.affected_findings.len() - FINDING_BUDGET
        ));
    }

    let mut gate_matrix = Vec::new();
    let graph = FrontierGraph::from_project(project);
    let mut weakened = 0usize;
    let mut support_killed = 0usize;
    let mut downstream_candidates = 0usize;
    let mut any_blast = false;
    for vf in &shown_findings {
        let Some(finding) = project.findings.iter().find(|f| &&f.id == vf) else {
            missing.push(format!("finding {vf} not in projection"));
            continue;
        };
        let digest = claim_digest(&finding.assertion.text);
        let attached: Vec<_> = project
            .verifier_attachments
            .iter()
            .filter(|a| &&a.target == vf)
            .cloned()
            .collect();
        let outcome = derive_gate_status(&digest, &attached);
        gate_matrix.push(GateRow {
            finding_id: (*vf).clone(),
            status: match outcome.status {
                GateStatus::Verified => "verified".to_string(),
                GateStatus::NeedsVerification => "needs_verification".to_string(),
                GateStatus::Refuted => "refuted".to_string(),
            },
            reason_count: outcome.reasons.len(),
        });

        let blast = graph.blast_radius_graded(project, vf, &[], BlastDirection::Downstream);
        weakened += blast.summary.weakened;
        support_killed += blast.summary.killed;
        downstream_candidates += blast.summary.downstream_candidates;
        any_blast = true;
    }

    Some(PackSignPreview {
        pack_id: pack_id.to_string(),
        scope,
        state_ops,
        polarity,
        gate_matrix,
        blast: any_blast.then_some(BlastLine {
            weakened,
            support_killed,
            downstream_candidates,
        }),
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_protocol::project;

    #[test]
    fn missing_pack_file_falls_back_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let proj = project::assemble("t", vec![], 0, 0, "test");
        assert!(pack_sign_preview(&proj, dir.path(), "vsd_absent").is_none());
    }

    #[test]
    fn minimal_pack_produces_a_bounded_preview() {
        let dir = tempfile::tempdir().unwrap();
        let packs = dir.path().join(".vela").join("diff_packs");
        std::fs::create_dir_all(&packs).unwrap();
        let pack = serde_json::json!({
            "schema": "vela.diff_pack.v1",
            "pack_id": "vsd_test0000000001",
            "frontier_id": "vfr_test",
            "created_at": "2026-07-10T00:00:00Z",
            "summary": "a test pack",
            "proposals": [],
            "aggregate_kind": "mixed"
        });
        std::fs::write(
            packs.join("vsd_test0000000001.json"),
            serde_json::to_string(&pack).unwrap(),
        )
        .unwrap();
        let proj = project::assemble("t", vec![], 0, 0, "test");
        let preview = pack_sign_preview(&proj, dir.path(), "vsd_test0000000001")
            .expect("readable pack previews");
        assert!(preview.scope.contains("proposal(s)"));
        assert!(preview.state_ops.len() <= OP_BUDGET + 1);
        assert!(preview.gate_matrix.len() <= FINDING_BUDGET);
        assert!(preview.blast.is_none(), "no findings, no blast aggregate");
    }
}
