//! # Reviewer Agent (v0.28)
//!
//! Reads `frontier.proposals` filtered to `pending_review`, asks
//! `claude -p` to score each one on plausibility / scope tightness
//! / evidence quality / duplication risk, and emits a
//! `finding.note` `StateProposal` per scored proposal so the
//! Workbench Inbox can surface the reviewer's read-out alongside
//! the agent's original claim.
//!
//! Doctrine: agents propose, humans review, CLI signs. The
//! Reviewer Agent's notes are also proposals — a human still
//! decides whether to accept the underlying claim. The reviewer
//! just makes the human's decision faster by pre-grading.
//!
//! Scope discipline (v0.28):
//! * **One LLM call per pending proposal.** No batching for v0.28
//!   — keeps the per-proposal score auditable in the model
//!   transcript. v0.29 can add batched mode if cost matters.
//! * **Notes only.** The Reviewer doesn't accept or reject; it
//!   produces an annotation a human reads first.
//! * **Skip already-reviewed.** Proposals whose target id already
//!   has a `reviewer-agent`-authored caveat in the frontier are
//!   skipped (idempotent re-runs).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use vela_protocol::events::StateTarget;
use vela_protocol::project::Project;
use vela_protocol::proposals::{StateProposal, new_proposal};
use vela_protocol::repo;

use crate::agent::{AgentContext, agent_run_meta};
use crate::llm_cli::{ClaudeCall, run_structured};

pub const AGENT_REVIEWER: &str = "reviewer-agent";

#[derive(Debug, Clone)]
pub struct ReviewerInput {
    pub frontier_path: PathBuf,
    pub model: Option<String>,
    pub cli_command: String,
    pub apply: bool,
    /// Per-run cap on proposals scored. Default 30.
    pub max_proposals: Option<usize>,
}

impl Default for ReviewerInput {
    fn default() -> Self {
        Self {
            frontier_path: PathBuf::new(),
            model: None,
            cli_command: "claude".to_string(),
            apply: true,
            max_proposals: Some(30),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedProposal {
    pub proposal_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewerReport {
    pub run: vela_protocol::proposals::AgentRun,
    pub frontier_path: String,
    pub apply: bool,
    pub pending_seen: usize,
    pub scored: usize,
    pub notes_written: usize,
    pub skipped: Vec<SkippedProposal>,
}

pub async fn run(input: ReviewerInput) -> Result<ReviewerReport, String> {
    let mut frontier: Project = repo::load_from_path(&input.frontier_path)
        .map_err(|e| format!("load frontier {}: {e}", input.frontier_path.display()))?;

    let pending: Vec<StateProposal> = frontier
        .proposals
        .iter()
        .filter(|p| p.status == "pending_review" && p.kind == "finding.add")
        .cloned()
        .collect();
    let pending_count = pending.len();

    let already_reviewed_targets: HashSet<String> = frontier
        .proposals
        .iter()
        .filter(|p| {
            p.kind == "finding.note"
                && p.actor.id == format!("agent:{AGENT_REVIEWER}")
        })
        .map(|p| p.target.id.clone())
        .collect();

    let to_review: Vec<StateProposal> = pending
        .into_iter()
        .filter(|p| !already_reviewed_targets.contains(&p.target.id))
        .take(input.max_proposals.unwrap_or(usize::MAX))
        .collect();

    let ctx = AgentContext::new(
        AGENT_REVIEWER,
        input.frontier_path.clone(),
        input.frontier_path.clone(),
        input.model.clone(),
        input.cli_command.clone(),
    );
    let extra = BTreeMap::from([
        ("pending_seen".to_string(), pending_count.to_string()),
        ("to_review".to_string(), to_review.len().to_string()),
    ]);
    let mut report = ReviewerReport {
        run: agent_run_meta(&ctx, extra),
        frontier_path: input.frontier_path.display().to_string(),
        apply: input.apply,
        pending_seen: pending_count,
        scored: 0,
        notes_written: 0,
        skipped: Vec::new(),
    };

    let existing_proposal_ids: HashSet<String> = frontier
        .proposals
        .iter()
        .map(|p| p.id.clone())
        .collect();

    let mut new_notes: Vec<StateProposal> = Vec::new();

    for proposal in &to_review {
        let assessment = match call_reviewer(proposal, &input) {
            Ok(a) => a,
            Err(e) => {
                report.skipped.push(SkippedProposal {
                    proposal_id: proposal.id.clone(),
                    reason: format!("model call failed: {e}"),
                });
                continue;
            }
        };
        report.scored += 1;

        let note_text = format_note(&assessment);
        let payload = json!({ "text": note_text });
        let mut note = new_proposal(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: proposal.target.id.clone(),
            },
            &ctx.actor_id,
            "agent",
            format!("Reviewer Agent score for {}", proposal.id),
            payload,
            vec![proposal.id.clone()],
            assessment.flags(),
        );
        note.agent_run = Some(report.run.clone());

        if existing_proposal_ids.contains(&note.id) {
            report.skipped.push(SkippedProposal {
                proposal_id: proposal.id.clone(),
                reason: "reviewer note id already in frontier".to_string(),
            });
            continue;
        }
        new_notes.push(note);
    }

    if input.apply && !new_notes.is_empty() {
        for n in new_notes.drain(..) {
            report.notes_written += 1;
            frontier.proposals.push(n);
        }
        repo::save_to_path(&input.frontier_path, &frontier)
            .map_err(|e| format!("save frontier: {e}"))?;
    } else {
        report.notes_written = new_notes.len();
    }

    report.run.finished_at = Some(Utc::now().to_rfc3339());
    Ok(report)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Assessment {
    plausibility: f64,
    evidence_quality: f64,
    scope_tightness: f64,
    duplicate_risk: f64,
    summary: String,
    #[serde(default)]
    concerns: Vec<String>,
}

impl Assessment {
    fn flags(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.plausibility < 0.4 {
            out.push("low_plausibility".to_string());
        }
        if self.evidence_quality < 0.4 {
            out.push("weak_evidence".to_string());
        }
        if self.scope_tightness < 0.4 {
            out.push("loose_scope".to_string());
        }
        if self.duplicate_risk > 0.6 {
            out.push("possible_duplicate".to_string());
        }
        out
    }
}

fn format_note(a: &Assessment) -> String {
    let mut out = format!(
        "Reviewer Agent score: plausibility {:.2} · evidence {:.2} · scope {:.2} · duplicate-risk {:.2}.",
        a.plausibility, a.evidence_quality, a.scope_tightness, a.duplicate_risk
    );
    if !a.summary.is_empty() {
        out.push_str(&format!(" {}", a.summary));
    }
    if !a.concerns.is_empty() {
        out.push_str(&format!(" Concerns: {}.", a.concerns.join("; ")));
    }
    out
}

fn call_reviewer(
    proposal: &StateProposal,
    input: &ReviewerInput,
) -> Result<Assessment, String> {
    let user_prompt = build_user_prompt(proposal);
    let system_prompt = build_system_prompt();
    let schema = output_schema_json();

    let mut call = ClaudeCall::new(system_prompt, &user_prompt, &schema);
    call.cli_command = &input.cli_command;
    call.model = input.model.as_deref();
    let value = run_structured(call)?;
    serde_json::from_value(value.clone())
        .map_err(|e| format!("parse reviewer assessment: {e}\nvalue: {value}"))
}

fn build_system_prompt() -> &'static str {
    r#"You are Reviewer Agent, an annotator inside the Vela
scientific protocol. You score one pending `finding.add` proposal
on four axes and return strict JSON matching the provided schema.

Axes (each 0.0–1.0, higher = better):
  plausibility      — does the claim hold up against general
                      scientific plausibility?
  evidence_quality  — does the proposal's evidence_spans actually
                      support the claim, with verbatim quotes?
  scope_tightness   — is the claim narrow + testable + scoped to
                      a specific organism / intervention / context?
  duplicate_risk    — likelihood the same claim already exists in
                      the frontier (1.0 = very likely a duplicate;
                      use only the metadata you're given).

Plus:
  summary  — one sentence the human reviewer reads first.
  concerns — short list of specific issues (≤5). Empty if clean.

Rules:
1. Be calibrated. Scores near 0.5 are neutral; reserve 0.9+ for
   really clean proposals and 0.2- for serious problems.
2. Never invent context. Score only what's in the proposal text.
3. Output the JSON object directly — no markdown fences."#
}

fn build_user_prompt(p: &StateProposal) -> String {
    let claim = p
        .payload
        .get("finding")
        .and_then(|f| f.get("assertion"))
        .and_then(|a| a.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("(no claim text)");
    let assertion_type = p
        .payload
        .get("finding")
        .and_then(|f| f.get("assertion"))
        .and_then(|a| a.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let evidence_spans = p
        .payload
        .get("finding")
        .and_then(|f| f.get("evidence"))
        .and_then(|e| e.get("evidence_spans"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "[]".to_string());
    let agent = p
        .agent_run
        .as_ref()
        .map(|r| r.agent.as_str())
        .unwrap_or("(human)");
    let model = p
        .agent_run
        .as_ref()
        .map(|r| r.model.as_str())
        .unwrap_or("");
    let source_refs = p.source_refs.join(", ");

    format!(
        "Proposal id: {}\nProposed by: {agent} (model: {model})\nKind: {}\nAssertion type: {assertion_type}\nClaim: {claim}\nEvidence spans: {evidence_spans}\nSource refs: {source_refs}\nReason given: {}\n\nReturn the JSON object.",
        p.id, p.kind, p.reason
    )
}

fn output_schema_json() -> String {
    serde_json::json!({
        "type": "object",
        "properties": {
            "plausibility":     { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "evidence_quality": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "scope_tightness":  { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "duplicate_risk":   { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "summary":          { "type": "string" },
            "concerns":         { "type": "array", "items": { "type": "string" } }
        },
        "required": ["plausibility", "evidence_quality", "scope_tightness", "duplicate_risk", "summary"]
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_fire_at_thresholds() {
        let a = Assessment {
            plausibility: 0.3,
            evidence_quality: 0.3,
            scope_tightness: 0.3,
            duplicate_risk: 0.7,
            summary: "x".to_string(),
            concerns: vec![],
        };
        let f = a.flags();
        assert!(f.contains(&"low_plausibility".to_string()));
        assert!(f.contains(&"weak_evidence".to_string()));
        assert!(f.contains(&"loose_scope".to_string()));
        assert!(f.contains(&"possible_duplicate".to_string()));
    }

    #[test]
    fn flags_empty_for_strong_assessment() {
        let a = Assessment {
            plausibility: 0.9,
            evidence_quality: 0.85,
            scope_tightness: 0.8,
            duplicate_risk: 0.1,
            summary: "Strong claim".to_string(),
            concerns: vec![],
        };
        assert!(a.flags().is_empty());
    }

    #[test]
    fn format_note_includes_summary_and_concerns() {
        let a = Assessment {
            plausibility: 0.7,
            evidence_quality: 0.5,
            scope_tightness: 0.6,
            duplicate_risk: 0.2,
            summary: "Plausible but evidence is thin.".to_string(),
            concerns: vec!["only one cohort".to_string(), "n=5".to_string()],
        };
        let n = format_note(&a);
        assert!(n.contains("0.70"));
        assert!(n.contains("Plausible but evidence is thin"));
        assert!(n.contains("only one cohort"));
    }
}
