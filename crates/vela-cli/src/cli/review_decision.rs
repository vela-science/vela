//! Current repository-authority review decisions.
//!
//! Human Vela keys, batch signing, detached signing, copied root ceremonies,
//! and the local signer helper are deliberately absent. The exact command is
//! the semantic action, the local operating-system session authenticates the
//! principal, and the repository authority signs the covering transaction.

use std::path::PathBuf;

use crate::current_repository_decision::DecisionAction;
use crate::ui::{self, ErrorKind};

use super::safe_text::inline as safe_inline;

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
    cmd_current_review_decide(frontier, proposal_id, action, reason, json);
}

fn cmd_current_review_decide(
    frontier: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    reason: String,
    json: bool,
) {
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let prepared = crate::current_repository_decision::prepare(
        &frontier,
        proposal_id,
        action,
        &reason,
        &observed_at,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    if !json {
        println!(
            "review decision · {} · {}",
            action.as_str(),
            safe_inline(&prepared.plan.proposal_id)
        );
        println!("  frontier: {}", safe_inline(&prepared.plan.frontier_name));
        println!("  claim: {}", safe_inline(&prepared.plan.claim_id));
        println!("  reason: {}", safe_inline(&prepared.plan.reason));
        println!("  authority: local OS session → repository authority");
        println!(
            "  scientific state change: {}",
            if action == DecisionAction::Accept {
                "exact current Claim standing transition"
            } else {
                "none"
            }
        );
        println!("  executing this exact proposal, action, and reason");
    }
    let result = crate::current_repository_decision::execute(&frontier, &prepared.plan, action)
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Custody, &error, None));
    let payload = serde_json::json!({
        "ok": true,
        "command": format!("review.{}", action.as_str()),
        "schema": "vela.review-decision.v3",
        "frontier": frontier.display().to_string(),
        "frontier_id": prepared.plan.frontier_id,
        "repository_before": prepared.plan.repository_root,
        "proposal_id": prepared.plan.proposal_id,
        "proposal_root": prepared.plan.proposal_root,
        "claim_id": prepared.plan.claim_id,
        "claim_root": prepared.plan.claim_root,
        "verification_set_root": prepared.plan.verification_set_root,
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
        "legacy_runtime_used": false,
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("current review result JSON")
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
