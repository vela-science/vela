//! Repository-authority review decisions.
//!
//! Human Vela keys, batch signing, detached signing, copied root ceremonies,
//! and the local signer helper are deliberately absent. The exact command is
//! the semantic action, the local operating-system session authenticates the
//! principal, and the repository authority signs the covering transaction.

use std::path::PathBuf;

use crate::repository_decision::DecisionAction;
use crate::ui::{self, ErrorKind};

use super::safe_text::inline as safe_inline;

pub(crate) fn cmd_review_decide(
    repository_path: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    expected_entry_root: Option<&str>,
    reason: String,
    json: bool,
) {
    ui::set_mode(
        match action {
            DecisionAction::Accept => "review.accept",
            DecisionAction::Reject => "review.reject",
        },
        json,
    );
    ui::require_initialized_repo(&repository_path);
    if reason.trim().is_empty() {
        ui::fail_with(ErrorKind::Usage, "--reason must not be empty", None);
    }
    run_review_decision(
        repository_path,
        proposal_id,
        action,
        expected_entry_root,
        reason,
        json,
    );
}

fn run_review_decision(
    repository_path: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    expected_entry_root: Option<&str>,
    reason: String,
    json: bool,
) {
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let (prepared, recovery_barrier) = crate::repository_decision::prepare_locked(
        &repository_path,
        proposal_id,
        action,
        &reason,
        &observed_at,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    if let Some(entry_root) = expected_entry_root {
        crate::decision_inbox::require_prepared_entry_root(&prepared, entry_root)
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
    }
    let plan = prepared.plan.clone();
    if !json {
        println!(
            "review decision · {} · {}",
            action.as_str(),
            safe_inline(&plan.proposal_id)
        );
        println!("  repository: {}", safe_inline(&plan.repository_name));
        println!("  claim: {}", safe_inline(&plan.claim_id));
        println!("  reason: {}", safe_inline(&plan.reason));
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
    let result = crate::repository_decision::execute_prepared(
        &repository_path,
        prepared,
        recovery_barrier,
        action,
    )
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Custody, &error, None));
    let payload = serde_json::json!({
        "ok": true,
        "command": format!("review.{}", action.as_str()),
        "schema": "vela.review-decision.v4",
        "repository_path": repository_path.display().to_string(),
        "repository_id": plan.repository_id,
        "repository_before": plan.repository_root,
        "proposal_id": plan.proposal_id,
        "proposal_root": plan.proposal_root,
        "claim_id": plan.claim_id,
        "claim_root": plan.claim_root,
        "verification_set_root": plan.verification_set_root,
        "principal_id": plan.principal_id,
        "action": action.as_str(),
        "reason": plan.reason,
        "decision_plan_root": plan.plan_root,
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
        println!("  · exact Decision committed locally; inspect and push when ready");
    }
}
