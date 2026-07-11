//! Independence as a DERIVED predicate — never a stored or asserted flag.
//!
//! Two evaluations are not independent because two names appear. They may
//! share a model, a prompt template, code, data, a parent run, or one
//! binary compiled twice. This module owns the two derivations:
//!
//! - [`independence_from_attachments`] — the gate's judgment over matched
//!   verifier attachments (the G1 clause plus the monoculture demotion,
//!   lifted verbatim from `derive_gate_status`, which now calls it);
//! - [`independence_from_receipt`] — the landing-time judgment over a
//!   receipt's verifier runs, its declared `independence_basis`, and its
//!   lineage layer, feeding `PolicyContext.independence_satisfied`.
//!
//! Both are fail-closed and monotonic: missing lineage never counts as
//! diversity, a declared coupling defeats independence, and the outcome
//! can only refuse — it never upgrades anything a stricter check refused.
//! The receipt-side predicate reads producer-DECLARED inputs, so it is a
//! policy lever (what the signed policy may auto-admit), never gate truth:
//! the gate's independence stays derived from attachments.

use serde::{Deserialize, Serialize};

use super::verifier_attachment::VerifierAttachment;
use crate::objects::receipt_v1::{IndependenceBasis, ReceiptLineage};

/// The derived judgment. Mirrors `GateOutcome`: satisfied iff `reasons`
/// is empty, so every refusal is inspectable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndependenceOutcome {
    pub satisfied: bool,
    pub reasons: Vec<String>,
}

impl IndependenceOutcome {
    fn from_reasons(reasons: Vec<String>) -> Self {
        IndependenceOutcome {
            satisfied: reasons.is_empty(),
            reasons,
        }
    }
}

/// Attachment-level independence: the G1 clause plus the monoculture
/// demotion, as one pure function of `(claim_digest, attachments)`.
/// Extracted from `derive_gate_status` (which calls this); the reason
/// strings are unchanged so existing vectors and callers see identical
/// judgments.
#[must_use]
pub fn independence_from_attachments(
    current_claim_digest: &str,
    attachments: &[VerifierAttachment],
) -> IndependenceOutcome {
    let matched: Vec<&VerifierAttachment> = attachments
        .iter()
        .filter(|a| a.is_passing_match(current_claim_digest))
        .collect();
    let mut reasons = Vec::new();

    // Monoculture observation: when the matched set agrees but every run
    // names the SAME implementation, say so — N runs of one binary are
    // replication, not implementation diversity.
    let impls: std::collections::HashSet<&str> = matched
        .iter()
        .map(|a| a.implementation_id.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    if matched.len() >= 2 && impls.len() == 1 {
        reasons.push(format!(
            "monoculture: {} matched run(s) all from implementation '{}' — independent implementations would strengthen this",
            matched.len(),
            impls.iter().next().unwrap()
        ));
    }

    // G1-L: matched attachments sharing a declared lineage coupling tag
    // (`model:` / `code:` / `data:` / `run:`) are one failure domain
    // regardless of method/solver diversity. Producer-declared, so a
    // missing declaration is invisible here — G1's positive requirements
    // below remain the floor.
    let mut tag_owners: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for a in &matched {
        let unique: std::collections::BTreeSet<&str> =
            a.lineage_couplings.iter().map(String::as_str).collect();
        for t in unique {
            *tag_owners.entry(t).or_insert(0) += 1;
        }
    }
    let shared: Vec<&str> = tag_owners
        .iter()
        .filter(|(_, n)| **n >= 2)
        .map(|(t, _)| *t)
        .collect();
    if !shared.is_empty() {
        reasons.push(format!(
            "G1-L: matched attachments share failure domain [{}]",
            shared.join(", ")
        ));
    }

    // G1 independence: >=2 matched attachments by different method/solver,
    // with at least one declaring independence (one-directional; mutual is a
    // hash circularity over the content-addressed id).
    if matched.len() < 2 {
        reasons.push(format!(
            "G1: need >=2 matched independent attachments, have {}",
            matched.len()
        ));
    } else {
        let distinct_methods: std::collections::BTreeSet<_> = matched
            .iter()
            .map(|a| (a.verifier_method, a.solver_id.as_str()))
            .collect();
        if distinct_methods.len() < 2 {
            reasons.push(
                "G1: >=2 attachments but all share one method/solver (not independent)".to_string(),
            );
        } else {
            let ids: std::collections::BTreeSet<&str> =
                matched.iter().map(|a| a.id.as_str()).collect();
            let declares_independence = matched.iter().any(|a| {
                a.independent_of
                    .iter()
                    .any(|other| other != &a.id && ids.contains(other.as_str()))
            });
            if !declares_independence {
                reasons.push(
                    "G1: attachments do not declare independence (independent_of)".to_string(),
                );
            }
        }
    }

    IndependenceOutcome::from_reasons(reasons)
}

/// A borrowed view of one receipt verifier run — the three fields the
/// predicate reads. `vela-cli`'s landing type maps into this so the
/// derivation lives here, below every caller.
#[derive(Debug, Clone, Copy)]
pub struct ReceiptRunView<'a> {
    pub method: &'a str,
    pub solver: &'a str,
    pub outcome: &'a str,
}

/// Receipt-level independence, derived from what the receipt actually
/// carries. Fail-closed rules, each with an inspectable reason:
///
/// 1. fewer than two passing verifier runs — nothing to be independent;
/// 2. all passing runs share one `(method, solver)` — one failure domain;
/// 3. no `independence_basis` declared — missing lineage is not diversity;
/// 4. `known_couplings` non-empty — a declared coupling defeats the claim;
/// 5. no `declared_independent_of` — the declaration requirement, the
///    receipt-side mirror of G1's `independent_of`;
/// 6. the receipt declares independence from its own lineage parent — a
///    self-contradiction lineage makes visible.
#[must_use]
pub fn independence_from_receipt(
    runs: &[ReceiptRunView<'_>],
    basis: Option<&IndependenceBasis>,
    lineage: Option<&ReceiptLineage>,
) -> IndependenceOutcome {
    let mut reasons = Vec::new();

    let passing: Vec<&ReceiptRunView> = runs
        .iter()
        .filter(|r| r.outcome.eq_ignore_ascii_case("pass"))
        .collect();
    if passing.len() < 2 {
        reasons.push(format!(
            "need >=2 passing verifier runs, have {}",
            passing.len()
        ));
    } else {
        let distinct: std::collections::BTreeSet<(&str, &str)> =
            passing.iter().map(|r| (r.method, r.solver)).collect();
        if distinct.len() < 2 {
            reasons
                .push("all passing runs share one method/solver (one failure domain)".to_string());
        }
    }

    match basis {
        None => {
            reasons.push(
                "no independence_basis declared (missing lineage never counts as diversity)"
                    .to_string(),
            );
        }
        Some(b) => {
            if !b.known_couplings.is_empty() {
                reasons.push(format!(
                    "declared couplings defeat independence: [{}]",
                    b.known_couplings.join(", ")
                ));
            }
            if b.declared_independent_of.is_empty() {
                reasons.push(
                    "no declared_independent_of (independence must be declared, not implied)"
                        .to_string(),
                );
            } else if let Some(lin) = lineage {
                let contradiction: Vec<&str> = b
                    .declared_independent_of
                    .iter()
                    .filter(|d| lin.parents.contains(d) || lin.derived_from.contains(d))
                    .map(String::as_str)
                    .collect();
                if !contradiction.is_empty() {
                    reasons.push(format!(
                        "declares independence from its own lineage parent(s): [{}]",
                        contradiction.join(", ")
                    ));
                }
            }
        }
    }

    IndependenceOutcome::from_reasons(reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<'a>(method: &'a str, solver: &'a str, outcome: &'a str) -> ReceiptRunView<'a> {
        ReceiptRunView {
            method,
            solver,
            outcome,
        }
    }

    fn diverse_basis() -> IndependenceBasis {
        IndependenceBasis {
            method_family: "exact".to_string(),
            declared_independent_of: vec!["vva_other".to_string()],
            ..IndependenceBasis::default()
        }
    }

    #[test]
    fn bare_receipt_is_not_independent() {
        let out = independence_from_receipt(&[], None, None);
        assert!(!out.satisfied);
        assert!(out.reasons.iter().any(|r| r.contains(">=2 passing")));
        assert!(
            out.reasons
                .iter()
                .any(|r| r.contains("no independence_basis"))
        );
    }

    #[test]
    fn one_failure_domain_is_not_independent() {
        let runs = [
            run("sidon_exact", "vela-verify", "pass"),
            run("sidon_exact", "vela-verify", "pass"),
        ];
        let out = independence_from_receipt(&runs, Some(&diverse_basis()), None);
        assert!(!out.satisfied);
        assert!(out.reasons.iter().any(|r| r.contains("one failure domain")));
    }

    #[test]
    fn declared_coupling_defeats_independence() {
        let runs = [
            run("sidon_exact", "vela-verify", "pass"),
            run("sat", "kissat", "pass"),
        ];
        let mut basis = diverse_basis();
        basis.known_couplings = vec!["model:claude-fable-5".to_string()];
        let out = independence_from_receipt(&runs, Some(&basis), None);
        assert!(!out.satisfied);
        assert!(out.reasons.iter().any(|r| r.contains("couplings")));
    }

    #[test]
    fn diverse_and_declared_is_independent() {
        let runs = [
            run("sidon_exact", "vela-verify", "pass"),
            run("sat", "kissat", "pass"),
        ];
        let out = independence_from_receipt(&runs, Some(&diverse_basis()), None);
        assert!(out.satisfied, "refused for: {:?}", out.reasons);
    }

    #[test]
    fn independence_from_own_parent_is_a_contradiction() {
        let runs = [
            run("sidon_exact", "vela-verify", "pass"),
            run("sat", "kissat", "pass"),
        ];
        let lineage = ReceiptLineage {
            parents: vec!["vva_other".to_string()],
            ..ReceiptLineage::default()
        };
        let out = independence_from_receipt(&runs, Some(&diverse_basis()), Some(&lineage));
        assert!(!out.satisfied);
        assert!(out.reasons.iter().any(|r| r.contains("own lineage parent")));
    }

    #[test]
    fn failing_runs_never_count() {
        let runs = [
            run("sidon_exact", "vela-verify", "pass"),
            run("sat", "kissat", "fail"),
        ];
        let out = independence_from_receipt(&runs, Some(&diverse_basis()), None);
        assert!(!out.satisfied);
    }
}
