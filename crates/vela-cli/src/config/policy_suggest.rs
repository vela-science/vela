//! `vela policy suggest` — the self-shrinking ceremony.
//!
//! Every ask that reaches a human key is a data point: either the active
//! policy DEFERRED it, or no policy exists and the lane is closed, or a
//! human already decided one just like it. This module folds those asks
//! into a histogram of (claim_class, reason) and, when a class keeps
//! recurring, SHOWS the one rule whose signature would cover the whole
//! class — so every sign session shrinks the next one.
//!
//! Custody doctrine: suggest never seals, never signs, never writes.
//! It is a projection with an opinion. Sealing goes through
//! `vela policy draft --from-suggest` (still unsigned); authority still
//! arrives only with the human signature in `vela policy sign`.
//!
//! Deliberately NOT suggested: classes the policy DENIES (a prohibition
//! is a decision already made, not friction) and the `unknown` class
//! (a rule over "unknown" would be delegation without a shape).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use vela_protocol::acceptance_policy::{
    Constraints, Outcome, PolicyRule, evaluate, load_active_policy,
};
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;

/// One histogram row: how often a class reached the human, and why.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AskRow {
    pub claim_class: String,
    /// Machine-readable reason class: a policy defer code
    /// ("default_defer", "rule:constraint"), "no_signed_policy" when the
    /// lane is closed, or "decided_by_you" for past human verdicts.
    pub reason: String,
    pub count: usize,
    pub sample_ids: Vec<String>,
}

/// A covering rule the human could sign, with its provenance.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Suggestion {
    pub claim_class: String,
    /// How many observed asks this rule would have absorbed.
    pub covers: usize,
    /// A named template when one already covers the class — the next
    /// command is then `vela policy draft <template>`.
    pub template: Option<String>,
    pub rule: PolicyRule,
}

/// The class a FUTURE landing of this proposal's shape would carry.
///
/// Record-landed drafts keep the receipt type at
/// `payload.finding.assertion.type`, and `vela land` stamps
/// `receipt_<type>` into the policy context — so a rule suggested for
/// this class actually fires on the next landing. Everything else falls
/// back to the text classifier the dry-run uses.
fn future_claim_class(p: &StateProposal) -> String {
    crate::review_material::proposal_claim_class(p)
}

/// Fold a frontier's asks into (claim_class, reason) rows, most frequent
/// first. Pure read.
pub(crate) fn ask_histogram(project: &Project, frontier: &Path) -> Result<Vec<AskRow>, String> {
    let policy = load_active_policy(frontier)?;
    let now = chrono::Utc::now().to_rfc3339();

    // Events the policy lane admitted never reached the human — their
    // proposals are autonomy, not asks.
    let admitted: std::collections::BTreeSet<String> =
        super::cli_policy::lane_admission_proposal_ids(project);

    let mut counts: BTreeMap<(String, String), (usize, Vec<String>)> = BTreeMap::new();
    let mut bump = |class: String, reason: String, id: &str| {
        let entry = counts.entry((class, reason)).or_default();
        entry.0 += 1;
        if entry.1.len() < 3 {
            entry.1.push(id.to_string());
        }
    };

    for p in &project.proposals {
        match p.status.as_str() {
            // Asks waiting right now.
            "pending_review" => {
                match &policy {
                    None => bump(future_claim_class(p), "no_signed_policy".to_string(), &p.id),
                    Some(vp) => {
                        let receipt =
                            crate::review_material::frontier_receipt_for_proposal(frontier, p);
                        let ctx = crate::review_material::derive_existing_proposal_policy_context(
                            project,
                            &p.id,
                            receipt.as_ref(),
                            &now,
                        );
                        let class = ctx.claim_class.clone();
                        let d = evaluate(&vp.policy, &ctx, &now);
                        match d.outcome {
                            Outcome::Permit => {}
                            // A Deny is a standing decision, not friction.
                            Outcome::Deny => {}
                            Outcome::Defer => {
                                bump(class, d.reasons.first().cloned().unwrap_or_default(), &p.id)
                            }
                        }
                    }
                }
            }
            // Asks the human already absorbed (not via the policy lane).
            "accepted" | "applied" | "rejected" => {
                if !admitted.contains(&p.id) {
                    bump(future_claim_class(p), "decided_by_you".to_string(), &p.id);
                }
            }
            _ => {}
        }
    }

    let mut rows: Vec<AskRow> = counts
        .into_iter()
        .map(|((claim_class, reason), (count, sample_ids))| AskRow {
            claim_class,
            reason,
            count,
            sample_ids,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.claim_class.cmp(&b.claim_class))
    });
    Ok(rows)
}

/// The recurrence bar: below this, an ask is judgment, not friction.
pub(crate) const SUGGEST_THRESHOLD: usize = 3;

/// Fold histogram rows into covering rules. One suggestion per class
/// (rows for the same class under different reasons pool their counts).
pub(crate) fn suggestions(rows: &[AskRow]) -> Vec<Suggestion> {
    let mut per_class: BTreeMap<&str, usize> = BTreeMap::new();
    for r in rows {
        if r.claim_class == "unknown" || r.claim_class.is_empty() {
            continue;
        }
        *per_class.entry(r.claim_class.as_str()).or_default() += r.count;
    }

    let mut out = Vec::new();
    for (class, covers) in per_class {
        if covers < SUGGEST_THRESHOLD {
            continue;
        }
        // A named template that already covers the class wins: its shape
        // was reviewed once and its name is the ceremony's vocabulary.
        let template = [
            "witness-rederivation",
            "lean-rederivation",
            "statement-drafts",
            "notes-threshold",
        ]
        .iter()
        .find(|t| super::cli_policy::template_covers_class(t, class))
        .map(|t| t.to_string());
        let rule = match &template {
            Some(t) => {
                super::cli_policy::template_rule(t).expect("template name came from the fixed list")
            }
            None => custom_rule(class),
        };
        out.push(Suggestion {
            claim_class: class.to_string(),
            covers,
            template,
            rule,
        });
    }
    out.sort_by(|a, b| {
        b.covers
            .cmp(&a.covers)
            .then(a.claim_class.cmp(&b.claim_class))
    });
    out
}

/// The conservative covering rule for a class no template names.
///
/// Receipt classes take the statement-drafts shape: drafts ARE text, so
/// the semantic-text guard opens, independence is a verdict-time
/// property, and the assurance floor stays at 2 (a passing verifier run
/// at landing). Everything else gets the tight shape: A3, no text
/// change, independence and method integrity required.
fn custom_rule(class: &str) -> PolicyRule {
    let receipt = class.starts_with("receipt_");
    PolicyRule {
        id: format!("suggested-{}-v1", class.replace('_', "-")),
        effect: Outcome::Permit,
        claim_classes: vec![class.to_string()],
        constraints: Constraints {
            max_changed_findings: 1,
            max_downstream_dependents: 0,
            required_assurance_min: if receipt { 2 } else { 3 },
            allow_semantic_text_change: receipt,
            allow_contested: false,
            allow_governance_mutation: false,
            require_independence: !receipt,
            require_method_integrity: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use vela_protocol::events::StateTarget;
    use vela_protocol::proposals::new_proposal;

    fn receipt_proposal(n: usize, status: &str) -> StateProposal {
        let mut p = new_proposal(
            "finding.add",
            StateTarget {
                r#type: "finding".to_string(),
                id: format!("vf_{n}"),
            },
            "agent:scout",
            "agent",
            "landed receipt",
            json!({"finding": {"assertion": {
                "text": format!("statement draft {n}"),
                "type": "theoretical",
            }}}),
            Vec::new(),
            Vec::new(),
        );
        p.status = status.to_string();
        p
    }

    #[test]
    fn histogram_counts_pending_and_decided_asks() {
        let tmp = TempDir::new().unwrap();
        let mut project = vela_protocol::project::assemble("t", vec![], 0, 0, "test");
        for n in 0..3 {
            project
                .proposals
                .push(receipt_proposal(n, "pending_review"));
        }
        project.proposals.push(receipt_proposal(3, "accepted"));

        // No policy dir at all: the lane is closed.
        let rows = ask_histogram(&project, tmp.path()).unwrap();
        let pending = rows
            .iter()
            .find(|r| r.reason == "no_signed_policy")
            .expect("pending asks counted");
        assert_eq!(pending.claim_class, "receipt_theoretical");
        assert_eq!(pending.count, 3);
        assert_eq!(pending.sample_ids.len(), 3);
        let decided = rows
            .iter()
            .find(|r| r.reason == "decided_by_you")
            .expect("past human decisions counted");
        assert_eq!(decided.count, 1);
    }

    #[test]
    fn recurring_class_maps_to_the_covering_template() {
        let rows = vec![AskRow {
            claim_class: "receipt_theoretical".to_string(),
            reason: "no_signed_policy".to_string(),
            count: 4,
            sample_ids: vec![],
        }];
        let s = suggestions(&rows);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].template.as_deref(), Some("statement-drafts"));
        assert_eq!(s[0].covers, 4);
    }

    #[test]
    fn unknown_and_rare_classes_are_never_suggested() {
        let rows = vec![
            AskRow {
                claim_class: "unknown".to_string(),
                reason: "no_signed_policy".to_string(),
                count: 50,
                sample_ids: vec![],
            },
            AskRow {
                claim_class: "receipt_computational".to_string(),
                reason: "default_defer".to_string(),
                count: SUGGEST_THRESHOLD - 1,
                sample_ids: vec![],
            },
        ];
        assert!(suggestions(&rows).is_empty());
    }

    #[test]
    fn uncovered_receipt_class_gets_the_draft_shaped_rule() {
        let rows = vec![AskRow {
            claim_class: "receipt_computational".to_string(),
            reason: "default_defer".to_string(),
            count: 5,
            sample_ids: vec![],
        }];
        let s = suggestions(&rows);
        assert_eq!(s.len(), 1);
        assert!(s[0].template.is_none());
        let c = &s[0].rule.constraints;
        assert!(c.allow_semantic_text_change);
        assert!(!c.require_independence);
        assert_eq!(c.required_assurance_min, 2);
        assert_eq!(s[0].rule.claim_classes, vec!["receipt_computational"]);
    }
}
