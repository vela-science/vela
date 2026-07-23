//! Frontier identification: rank OPEN findings by accumulating structural
//! support — the graded, evaluable generalization of [`crate::boundary`]'s
//! binary `one_premise_away`. It answers "where does accumulated structure
//! suggest an opportunity?", and is a deliberate adaptation of Prashant Garg's
//! *Frontier Graph* method ("What Should Economics Ask Next?", 2026) to a
//! verifier-gated substrate. It is exposed as `vela frontier rank` and as
//! structural advice inside the read-only MCP `orient` tool; it is not the
//! configured producer queue returned by `vela next`.
//!
//! ## What transfers, and what we deliberately change
//!
//! Garg ranks missing concept-links in a literature graph by a learned
//! combination of *underexplored-pair*, *path-support*, and *motif-support*
//! features, and shows it beats a preferential-attachment (popularity) baseline
//! at predicting which links get *published* next (a ~10x recall lift at fine,
//! ontology-normalized concept granularity — the regime where popularity breaks
//! down because new edges spread across many non-hub nodes).
//!
//! Two honest departures for Vela:
//!
//! 1. **Target = solvability, not publication.** Vela's nodes are already
//!    canonical (a finding is a finding), and the useful question is not "what
//!    will be published" but "which open finding is a *verifier-run* from done".
//!    The signal is the accumulated **structural support** around an open
//!    finding — its premise/support/path density — exactly the topology Garg's
//!    frontier-identification scores. That support is read RAW from the edge
//!    graph, present before anything is curated; premise-establishment is a
//!    BONUS multiplier on top (verified scaffolding ⇒ closer to a
//!    one-frozen-verifier-run close), never a gate. (Gating the whole score on
//!    `Established` made a frontier with real structure but no accepted reviews
//!    score a flat zero — the signal has to live in the topology, not a review
//!    flag.) The forward foundry+verifier loop is the ground truth, not a
//!    publication backtest.
//! 2. **Advice, never authority.** Like [`crate::boundary`], this is a pure
//!    projection over the typed [`FrontierGraph`]. It feeds `vela frontier
//!    rank` and MCP orientation only; it never reorders `vela next`, mutates
//!    state, or becomes a producer target. It carries the *inspectable
//!    evidence* (the established premises and mediating support) behind every
//!    score so a human can judge the suggestion.
//!
//! The preferential-attachment score is computed alongside every candidate so a
//! consumer can always show the lift over popularity, not just the rank.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::frontier_graph::{EdgeKind, FindingState, FrontierGraph};
use crate::project::Project;

/// Required-premise edges: A rests on B (A cannot be established until B is).
/// Mirrors [`FrontierGraph::REQUIRED_PREMISE_KINDS`].
const PREMISE_KINDS: [EdgeKind; 3] = [
    EdgeKind::DependsOn,
    EdgeKind::DerivedFrom,
    EdgeKind::Discharges,
];
/// Corroboration edges pointing AT a finding (it is supported by B).
const SUPPORT_KINDS: [EdgeKind; 2] = [EdgeKind::Supports, EdgeKind::Improves];

/// Weights for the structural-support score. Named so the combination is legible
/// and tunable; the premise term dominates because premise-establishment is the
/// direct solvability signal. This PATH-DOMINANT shape is empirically grounded:
/// reproducing Garg's Frontier Graph scoring from his public 2026-04-13 release
/// (`research/garg-repro/`) confirms his published `base_score` is r² ≈ 0.86 on
/// `0.5·path_support + 0.2·gap_bonus` — a path-dominant combination wins, which
/// `W_PREMISE = 1.0` mirrors (premise-establishment is Vela's solvability analog
/// of his path-support). His model also carries a hub PENALTY, down-weighting
/// support that flows through high-degree mediators (in a citation graph, that is
/// spurious popularity). Vela DELIBERATELY does not: a Vela "hub" is a
/// widely-depended-on lemma, and resting on it is good scaffolding toward a close,
/// not spurious support — so the degree signal is not transferred.
const W_PREMISE: f64 = 1.0;
const W_SUPPORT: f64 = 0.4;
const W_PATH: f64 = 0.3;
const W_UNLOCK: f64 = 0.2;

/// One ranked open finding, with the evidence behind its score so the
/// suggestion is inspectable (never a bare number).
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub id: String,
    pub label: String,
    /// The structural-support score (higher = more accumulated support / closer
    /// to a verifier-run from done).
    pub score: f64,
    /// Preferential-attachment (popularity) score for the same node, so a
    /// consumer can show the lift over the popularity baseline.
    pub baseline: f64,
    pub premises_total: usize,
    pub premises_established: usize,
    /// Established results that corroborate this finding (incoming support).
    pub support_established: usize,
    /// Distinct established findings within two support-hops (mediating depth).
    pub mediating_support: usize,
    /// Findings that would be unblocked if this one becomes established.
    pub unlock: usize,
    /// A one-line, human-readable justification.
    pub why: String,
    /// The established premises / supporters behind the score (finding ids).
    pub evidence: Vec<String>,
}

/// A relation carrying live disagreement — Garg's *heterogeneity surfacing*
/// mapped onto Vela's contradiction structure. Never auto-adjudicated.
#[derive(Debug, Clone, Serialize)]
pub struct HeterogeneityItem {
    pub finding: String,
    pub label: String,
    pub partner: String,
    pub reason: String,
}

fn is_established(g: &FrontierGraph, id: &str) -> bool {
    g.node(id)
        .is_some_and(|n| n.state == FindingState::Established)
}

/// Rank the frontier's OPEN findings by accumulating structural support. Pure
/// and deterministic; sorted by score descending, id ascending for stability.
#[must_use]
pub fn frontier_identification(project: &Project) -> Vec<Candidate> {
    let graph = FrontierGraph::from_project(project);

    // Adjacency in one pass: premises (outgoing required), supporters (incoming
    // support), and total degree for the popularity baseline.
    let mut premises: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut supporters: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut out_deg: BTreeMap<&str, usize> = BTreeMap::new();
    let mut in_deg: BTreeMap<&str, usize> = BTreeMap::new();
    // Who lists X as a premise (for the unlock / downstream count).
    let mut dependents: BTreeMap<&str, usize> = BTreeMap::new();
    for e in graph.all_edges() {
        *out_deg.entry(&e.source).or_default() += 1;
        *in_deg.entry(&e.target).or_default() += 1;
        if PREMISE_KINDS.contains(&e.kind) {
            premises.entry(&e.source).or_default().push(&e.target);
            *dependents.entry(&e.target).or_default() += 1;
        }
        if SUPPORT_KINDS.contains(&e.kind) {
            supporters.entry(&e.target).or_default().push(&e.source);
        }
    }

    let mut out = Vec::new();
    for node in graph.nodes() {
        if node.state != FindingState::Open {
            continue;
        }
        let id = node.id.as_str();
        let prem = premises.get(id).cloned().unwrap_or_default();
        let prem_total = prem.len();
        let prem_est = prem.iter().filter(|t| is_established(&graph, t)).count();
        let sup = supporters.get(id).cloned().unwrap_or_default();
        let sup_est = sup.iter().filter(|s| is_established(&graph, s)).count();

        // Mediating support: distinct findings within two support hops — the
        // "path support" family (Garg's signal). Counted RAW from the accumulated
        // edge structure: it is present in a dense graph regardless of any review
        // state, which is exactly why it discriminates before anything is curated.
        let mut mediating: BTreeSet<&str> = BTreeSet::new();
        for hop1 in prem.iter().chain(sup.iter()) {
            mediating.insert(hop1);
            if let Some(p2) = premises.get(*hop1) {
                for hop2 in p2 {
                    mediating.insert(hop2);
                }
            }
        }
        let unlock = dependents.get(id).copied().unwrap_or(0);

        // Raw structural support — the Garg signal. Score by the accumulated
        // premise / support / path density around this open finding, present in
        // the topology before anything is established. `Established` is folded in
        // below as a BONUS multiplier, never a gate: a graph with rich structure
        // but no curated review state still ranks, and gains a lift as its
        // scaffolding gets verified. (The old form multiplied every term by the
        // established fraction, so a frontier with zero accepted findings scored
        // flat zero even with thousands of real support edges.)
        let ln = |n: usize| ((n as f64) + 1.0).ln();
        let raw_support = W_PREMISE * ln(prem_total)
            + W_SUPPORT * ln(sup.len())
            + W_PATH * ln(mediating.len())
            + W_UNLOCK * ln(unlock);
        let scaffold = prem_total + sup.len();
        let est_fraction = if scaffold == 0 {
            0.0
        } else {
            (prem_est + sup_est) as f64 / scaffold as f64
        };
        // Multiplier in [1, 2]: the verified fraction of the neighborhood is a
        // bonus on top of the raw structural rank.
        let score = raw_support * (1.0 + est_fraction);
        // Popularity baseline: preferential attachment on total degree.
        let baseline =
            (out_deg.get(id).copied().unwrap_or(0) * in_deg.get(id).copied().unwrap_or(0)) as f64;

        let why = if scaffold == 0 {
            "open, no structural support yet".to_string()
        } else if prem_est + sup_est == scaffold {
            format!(
                "{prem_total} premise(s) + {} supporter(s), all established — a verifier run from done",
                sup.len()
            )
        } else {
            format!(
                "{prem_total} premise(s), {} supporter(s), {} mediating result(s); {} established",
                sup.len(),
                mediating.len(),
                prem_est + sup_est
            )
        };
        // Evidence: prefer the established scaffolding to inspect; fall back to
        // the raw premises/supporters so there is always something behind a score.
        let mut evidence: Vec<String> = prem
            .iter()
            .chain(sup.iter())
            .filter(|s| is_established(&graph, s))
            .map(|s| (*s).to_string())
            .collect();
        if evidence.is_empty() {
            evidence = prem
                .iter()
                .chain(sup.iter())
                .map(|s| (*s).to_string())
                .collect();
        }
        evidence.sort();
        evidence.dedup();

        out.push(Candidate {
            id: node.id.clone(),
            label: node.label.clone(),
            score,
            baseline,
            premises_total: prem_total,
            premises_established: prem_est,
            support_established: sup_est,
            mediating_support: mediating.len(),
            unlock,
            why,
            evidence,
        });
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    out
}

/// Surface relations in live disagreement (Garg's heterogeneity surfacing):
/// recorded contradictions, each endpoint once. A projection for the review
/// lane; contradictions are never auto-adjudicated.
#[must_use]
pub fn heterogeneity_surfacing(project: &Project) -> Vec<HeterogeneityItem> {
    let graph = FrontierGraph::from_project(project);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for (a, b) in graph.contradiction_pairs() {
        for (node, partner) in [(&a, &b), (&b, &a)] {
            if seen.insert(node.clone()) {
                out.push(HeterogeneityItem {
                    finding: node.clone(),
                    label: graph.label_of(node).unwrap_or("").to_string(),
                    partner: partner.clone(),
                    reason: "in a recorded contradiction — estimates disagree".to_string(),
                });
            }
        }
    }
    out.sort_by(|x, y| x.finding.cmp(&y.finding));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::ReviewState;
    use crate::project::assemble;
    use crate::project::reverse_dep_index_tests::{link_to, synth_finding};

    fn link_typed(target: &str, link_type: &str) -> crate::bundle::Link {
        let mut link = link_to(target);
        link.link_type = link_type.into();
        link
    }

    #[test]
    fn solvable_open_finding_outranks_isolated_one() {
        // The structural-advice invariant: an open finding whose premise is
        // already established ranks above an isolated open finding with no
        // scaffolding — and above the popularity baseline's blindness (both
        // open findings have low raw degree). This does not affect `vela next`.
        let mut a = synth_finding(0, vec![]); // established premise
        a.flags.review_state = Some(ReviewState::Accepted);
        a.confidence.score = 0.9;
        let b = synth_finding(1, vec![link_typed(&a.id, "depends")]); // open, rests on a
        let c = synth_finding(2, vec![]); // open, isolated
        let (b_id, c_id) = (b.id.clone(), c.id.clone());
        let mut project = assemble("fi", vec![], 0, 0, "test");
        project.findings = vec![a, b, c];

        let ranked = frontier_identification(&project);
        // `a` is Established (not Open) so it is not a candidate; b and c are.
        assert!(ranked.iter().all(|r| r.id != _established_id(&project)));
        let pos = |id: &str| ranked.iter().position(|r| r.id == id).unwrap();
        assert!(
            pos(&b_id) < pos(&c_id),
            "one-premise-away b must outrank isolated c: {ranked:?}"
        );
        let b_cand = &ranked[pos(&b_id)];
        assert!(b_cand.score > 0.0 && b_cand.premises_established == 1);
        assert!(b_cand.why.contains("verifier run") || b_cand.why.contains("premise"));
        // evidence names the established premise it rests on
        assert!(!b_cand.evidence.is_empty());
    }

    #[test]
    fn raw_structure_discriminates_with_zero_established() {
        // The Garg fix: on a graph where NOTHING is established, an open finding
        // resting on real premise structure still outranks an isolated one. The
        // raw topology is the signal; `Established` is only a bonus multiplier.
        // Before the fix, every candidate scored a flat zero without a curated
        // review state — which is exactly the Erdős-frontier failure mode.
        let a = synth_finding(0, vec![]); // open, unestablished premise
        let d = synth_finding(3, vec![]); // open, unestablished premise
        let b = synth_finding(
            1,
            vec![link_typed(&a.id, "depends"), link_typed(&d.id, "depends")],
        ); // open, rests on real (unestablished) structure
        let c = synth_finding(2, vec![]); // open, isolated
        let (b_id, c_id) = (b.id.clone(), c.id.clone());
        let mut project = assemble("fi", vec![], 0, 0, "test");
        project.findings = vec![a, b, c, d];
        // Precondition: not one finding is established.
        assert!(
            project
                .findings
                .iter()
                .all(|f| f.flags.review_state.is_none())
        );

        let ranked = frontier_identification(&project);
        let pos = |id: &str| ranked.iter().position(|r| r.id == id).unwrap();
        assert!(
            pos(&b_id) < pos(&c_id),
            "b rests on real structure and must outrank isolated c with zero established: {ranked:?}"
        );
        let b_cand = &ranked[pos(&b_id)];
        assert!(
            b_cand.score > 0.0 && b_cand.premises_established == 0,
            "b scores on raw structure alone (0 established), got {b_cand:?}"
        );
    }

    #[test]
    fn verified_but_unreviewed_findings_remain_structural_candidates() {
        let a = synth_finding(0, vec![]);
        let b = synth_finding(1, vec![link_typed(&a.id, "depends")]);
        let (a_id, b_id) = (a.id.clone(), b.id.clone());
        let attachments = crate::test_support::verified_attachment_pair(&a);
        let mut project = assemble("fi-verified-open", vec![], 0, 0, "test");
        project.findings = vec![a, b];
        project.verifier_attachments = attachments;

        let ranked = frontier_identification(&project);
        assert!(
            ranked.iter().any(|candidate| candidate.id == a_id),
            "verified-only a remains Open and eligible for structural advice"
        );
        let b_candidate = ranked
            .iter()
            .find(|candidate| candidate.id == b_id)
            .expect("b remains an open structural candidate");
        assert_eq!(
            b_candidate.premises_established, 0,
            "verification must not count as accepted premise establishment"
        );
    }

    #[test]
    fn multi_node_topology_populates_all_structural_signals() {
        // A realistic slice, nothing established: B depends on A; C and D both
        // support B; E depends on B (so B unlocks E). B's score must reflect its
        // premise, its two supporters, its mediating path count, and its unlock.
        let a = synth_finding(0, vec![]);
        let b = synth_finding(1, vec![link_typed(&a.id, "depends")]);
        let c = synth_finding(2, vec![link_typed(&b.id, "supports")]);
        let d = synth_finding(3, vec![link_typed(&b.id, "supports")]);
        let e = synth_finding(4, vec![link_typed(&b.id, "depends")]);
        let b_id = b.id.clone();
        let mut project = assemble("fi", vec![], 0, 0, "test");
        project.findings = vec![a, b, c, d, e];

        let ranked = frontier_identification(&project);
        let b_cand = ranked.iter().find(|r| r.id == b_id).expect("B is open");
        assert_eq!(b_cand.premises_total, 1, "B depends on A");
        assert_eq!(b_cand.premises_established, 0, "nothing is established");
        assert_eq!(b_cand.support_established, 0, "C, D are not established");
        assert_eq!(b_cand.mediating_support, 3, "A + C + D within two hops");
        assert_eq!(b_cand.unlock, 1, "E rests on B");
        assert!(b_cand.score > 0.0);
    }

    fn _established_id(project: &crate::project::Project) -> String {
        project
            .findings
            .iter()
            .find(|f| matches!(f.flags.review_state, Some(ReviewState::Accepted)))
            .map(|f| f.id.clone())
            .unwrap_or_default()
    }

    #[test]
    fn support_kinds_are_disjoint_from_premise_kinds() {
        for s in SUPPORT_KINDS {
            assert!(
                !PREMISE_KINDS.contains(&s),
                "support and premise kinds must not overlap"
            );
        }
    }

    #[test]
    fn heterogeneity_surfaces_contradictions_once_per_endpoint() {
        let a = synth_finding(0, vec![]);
        let b = synth_finding(1, vec![link_typed(&a.id, "contradicts")]);
        let mut project = assemble("het", vec![], 0, 0, "test");
        project.findings = vec![a, b];
        let items = heterogeneity_surfacing(&project);
        assert_eq!(items.len(), 2, "both endpoints surfaced once");
    }
}
