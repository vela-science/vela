//! Era-1 repository-authority review decisions and shared review rendering.
//!
//! Human Vela keys, batch signing, detached signing, copied root ceremonies,
//! and the local signer helper are deliberately absent. The exact command is
//! the semantic action, the local operating-system session authenticates the
//! principal, and the repository authority signs the covering transaction.

use std::path::{Path, PathBuf};

use crate::decision_plan::{DecisionAction, DecisionExecutionOutcome};
use crate::ui::{self, ErrorKind};

use super::safe_text::inline as safe_inline;

pub(crate) fn render_decision_brief_lines(
    brief: &vela_edge::decision_brief::DecisionBrief,
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "change    {} {} · {}",
            brief.change.subject.subject_type,
            brief.change.subject.id,
            brief.change.requested_action
        ),
        format!("base      {}", brief.change.fixed_base.event_log_root),
    ];
    if let Some(before) = &brief.change.before {
        lines.push(format!("before    {}", before.text));
    }
    if let Some(after) = &brief.change.after {
        lines.push(format!("after     {}", after.text));
    }
    let evidence = brief
        .basis
        .primary_evidence_roots
        .iter()
        .map(|root| format!("{} {}", root.kind, root.root))
        .collect::<Vec<_>>()
        .join(" · ");
    lines.push(format!(
        "basis     {} · {}",
        brief.basis.check_state.gate_status, evidence
    ));
    if let Some(caveat) = &brief.basis.main_caveat {
        lines.push(format!("caveat    {caveat}"));
    }
    lines.push(format!(
        "impact    {} changed · {} downstream · tier {}",
        brief.impact.downstream_effect.changed_findings,
        brief.impact.downstream_effect.downstream_dependents,
        brief.impact.downstream_effect.impact_tier
    ));
    for warning in &brief.impact.critical_warnings {
        lines.push(format!(
            "warning   {}{}",
            warning.code,
            warning
                .reference
                .as_deref()
                .map(|reference| format!(" · {reference}"))
                .unwrap_or_default()
        ));
    }
    lines.push(format!(
        "authority {} · {}",
        brief.authority.route, brief.authority.scope
    ));
    for action in &brief.authority.actions {
        let reasons = if action.reasons.is_empty() {
            String::new()
        } else {
            format!(" · {}", action.reasons.join("; "))
        };
        lines.push(format!(
            "action {:<7} {}{}",
            action.action, action.eligibility, reasons
        ));
    }
    for (name, facet) in brief.facets.iter().filter(|(_, facet)| facet.critical) {
        lines.push(format!(
            "facet     {} · {} · {}",
            name,
            facet.full_root,
            serde_json::to_string(&facet.data).unwrap_or_else(|_| "unavailable".to_string())
        ));
    }
    if !brief.missing.is_empty() {
        lines.push(format!(
            "missing   {}",
            brief
                .missing
                .iter()
                .map(|fact| format!("{} ({})", fact.field, fact.reason))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    lines.push(format!(
        "audit     {} · facts {}",
        brief.audit.proposal_root, brief.audit.decision_facts_root
    ));
    lines.into_iter().map(|line| safe_inline(&line)).collect()
}

pub(crate) fn cmd_review_decide(
    frontier: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    reason: String,
    json: bool,
) {
    if reason.trim().is_empty() {
        ui::fail_with(ErrorKind::Usage, "--reason must not be empty", None);
    }
    match crate::repository_decision::is_repository_authority_frontier(&frontier) {
        Ok(true) => {}
        Ok(false) => ui::fail_with(
            ErrorKind::Domain,
            "this Frontier still uses the retired personal-signing authority model",
            Some(
                "use Vela v0.915.1 only for exact historical replay; the current candidate has no legacy authority writer",
            ),
        ),
        Err(error) => ui::fail_with(ErrorKind::Domain, &error, None),
    }

    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let prepared = match action {
        DecisionAction::Accept => crate::repository_decision::prepare_accept(
            &frontier,
            proposal_id,
            &reason,
            &observed_at,
        ),
        DecisionAction::Reject => crate::repository_decision::prepare_reject(
            &frontier,
            proposal_id,
            &reason,
            &observed_at,
        ),
    }
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));

    if !json {
        println!(
            "review decision · {} · {}",
            action.as_str(),
            safe_inline(&prepared.plan.proposal_id)
        );
        println!("  frontier: {}", safe_inline(&prepared.plan.frontier_name));
        println!(
            "  claim: {}",
            safe_inline(&prepared.review.brief.change.claim)
        );
        println!("  reason: {}", safe_inline(&prepared.plan.reason));
        println!("  authority: local OS session → repository authority");
        println!(
            "  scientific state change: {}",
            if action == DecisionAction::Accept {
                "exact accepted transition"
            } else {
                "none"
            }
        );
        println!("  executing this exact proposal, action, and reason");
    }

    let result = match action {
        DecisionAction::Accept => {
            crate::repository_decision::execute_accept(&frontier, &prepared.plan)
        }
        DecisionAction::Reject => {
            crate::repository_decision::execute_reject(&frontier, &prepared.plan)
        }
    }
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Custody, &error, None));
    let payload = serde_json::json!({
        "ok": true,
        "command": "review.decide",
        "schema": "vela.review-decision.v2",
        "frontier": frontier.display().to_string(),
        "proposal_id": prepared.plan.proposal_id,
        "proposal_root": prepared.plan.proposal_root,
        "principal_id": prepared.plan.principal_id,
        "action": action.as_str(),
        "reason": prepared.plan.reason,
        "decision_plan_root": prepared.plan.plan_root,
        "event_ids": result.event_ids,
        "authority_record_id": result.authority_record_id,
        "authority_record_root": result.authority_record_root,
        "before_event_log_root": result.before_event_log_root,
        "after_event_log_root": result.after_event_log_root,
        "scientific_state_changed": action == DecisionAction::Accept,
        "authentication": "local_os_session",
        "transaction_signer": "repository_authority",
        "human_key_read": false,
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("repository decision result JSON")
        );
    } else {
        println!(
            "  · {} {}",
            safe_inline(action.as_str()),
            safe_inline(proposal_id)
        );
        println!(
            "  · authority record {}",
            safe_inline(
                payload
                    .get("authority_record_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
            )
        );
        println!("  · inspect and commit the exact canonical delta");
    }
}

/// Resume publication for a completed historical journal without reopening
/// its retired human-signing path.
pub(crate) fn publish_exact_decision(
    frontier: &Path,
    summary: &str,
    outcome: &DecisionExecutionOutcome,
    opts: &crate::config::git_publish::PublishOptions,
) -> crate::config::git_publish::PublicationOutcome {
    use crate::config::git_publish::{PublicationOutcome, PublicationState};
    let Some(delta) = &outcome.publication_delta else {
        return PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "frontier transaction had no public Git delta".to_string(),
            },
            recovery_command: Some("git status --short".to_string()),
        };
    };
    match crate::config::git_publish::exact_publication_resume_preflight(frontier, delta, opts) {
        Ok(preflight) => match crate::config::git_publish::publish_exact_delta(
            frontier,
            summary,
            &outcome.event_ids,
            delta,
            preflight,
            opts,
        ) {
            Ok(publication) => publication,
            Err(error) => PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: error.to_string(),
                },
                recovery_command: Some("git status --short".to_string()),
            },
        },
        Err(publication) => publication,
    }
}
