//! Repository-authority review decisions.
//!
//! Performer-held repository keys, batch signing, detached signing, copied root
//! ceremonies, and the local signer helper are deliberately absent. The exact
//! command is the semantic action, the local operating-system session
//! authenticates the authority principal, an attributed human or agent is
//! recorded as the performer, and repository authority signs the transaction.

use std::path::PathBuf;

use crate::repository_decision::DecisionAction;
use crate::ui::{self, ErrorKind};

use super::safe_text::inline as safe_inline;

pub(crate) fn cmd_review_decide(
    repository_path: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    expected_entry_root: Option<&str>,
    actor: Option<String>,
    session_ref: Option<String>,
    reason: String,
    json: bool,
) {
    ui::require_initialized_repo(&repository_path);
    if reason.trim().is_empty() {
        ui::fail_with(ErrorKind::Usage, "--reason must not be empty", None);
    }
    let identity_request = format!(
        "{}|{}|{}|{}",
        repository_path.display(),
        proposal_id,
        action.as_str(),
        reason
    );
    let operation_id = vela_repository::OperationId::derive(
        "review-runtime-identity",
        identity_request.as_bytes(),
    );
    let device_identifier = super::runtime_device_identifier().unwrap_or_else(|error| {
        let recovery =
            super::runtime_identity_recovery(&error, &format!("vela review {}", action.as_str()));
        ui::fail_unchanged_coded(
            ErrorKind::Domain,
            Some(error.code),
            &error.message,
            operation_id.as_str(),
            &recovery,
        )
    });
    run_review_decision(
        repository_path,
        proposal_id,
        action,
        expected_entry_root,
        actor,
        session_ref,
        reason,
        json,
        device_identifier,
    );
}

fn run_review_decision(
    repository_path: PathBuf,
    proposal_id: &str,
    action: DecisionAction,
    expected_entry_root: Option<&str>,
    actor: Option<String>,
    session_ref: Option<String>,
    reason: String,
    json: bool,
    device_identifier: String,
) {
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let (prepared, recovery_barrier) = crate::repository_decision::prepare_locked(
        &repository_path,
        proposal_id,
        action,
        &reason,
        &observed_at,
        actor.as_deref(),
        session_ref.as_deref(),
        &device_identifier,
    )
    .unwrap_or_else(|error| {
        ui::fail_if_recovery_required(&repository_path);
        if error.starts_with(
            "current acceptance lacks an independent passing Verification Record",
        ) {
            ui::fail_coded(
                ErrorKind::Domain,
                Some("missing_independent_verification"),
                &error,
                Some("run `vela review show <proposal> --json`, retain the exact review method, then record an independent passing check with `vela verification record`"),
            )
        }
        ui::fail_with(ErrorKind::Domain, &error, None)
    });
    if let Some(entry_root) = expected_entry_root {
        crate::decision_inbox::require_prepared_entry_root(&prepared, entry_root)
            .unwrap_or_else(|error| {
                if is_stale_entry_error(&error) {
                    ui::fail_coded(
                        ErrorKind::Domain,
                        Some("decision_entry_stale"),
                        &error,
                        Some("rerun `vela review inbox --json`, inspect the Proposal again, and use the new exact entry_root"),
                    )
                }
                ui::fail_with(ErrorKind::Domain, &error, None)
            });
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
        println!(
            "  performer: {} · {}",
            safe_inline(&plan.actor_class),
            safe_inline(&plan.actor_id)
        );
        if let Some(reference) = &plan.session_ref {
            println!("  session: {}", safe_inline(reference));
        }
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
        &device_identifier,
    )
    .unwrap_or_else(|error| {
        ui::fail_if_recovery_required(&repository_path);
        if is_authority_refusal(&error) {
            ui::fail_coded(
                ErrorKind::Custody,
                Some("authority_refused"),
                &error,
                Some("run `ssh-add -l`, confirm the Repository policy principal and full authority-key fingerprint, then retry; --as identifies the performer and never selects or grants authority"),
            )
        }
        // This includes post-commit and post-publication failures whose own
        // message says not to retry. Never replace that exact recovery advice
        // with a generic authority hint.
        ui::fail_with(ErrorKind::Custody, &error, None)
    });
    let payload = serde_json::json!({
        "ok": true,
        "command": format!("review.{}", action.as_str()),
        "schema": "vela.review-decision.v5",
        "repository_path": repository_path.display().to_string(),
        "repository_id": plan.repository_id,
        "repository_before": plan.repository_root,
        "proposal_id": plan.proposal_id,
        "proposal_root": plan.proposal_root,
        "claim_id": plan.claim_id,
        "claim_root": plan.claim_root,
        "verification_set_root": plan.verification_set_root,
        "actor_id": plan.actor_id.clone(),
        "actor_class": plan.actor_class.clone(),
        "session_ref": plan.session_ref.clone(),
        "principal_id": plan.principal_id.clone(),
        "authority_principal_id": plan.principal_id.clone(),
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
        "performer_key_read": false,
        "performer": {
            "actor_id": plan.actor_id,
            "actor_class": plan.actor_class,
            "session_ref": plan.session_ref,
            "key_read": false,
        },
        "authority": {
            "principal_id": plan.principal_id,
            "authentication": "local_os_session",
            "transaction_signer": "repository_authority",
        },
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

fn is_stale_entry_error(error: &str) -> bool {
    error.starts_with("Decision Inbox entry changed:")
}

fn is_authority_refusal(error: &str) -> bool {
    error == "authorization denied"
        || error.starts_with("authorization principal differs")
        || error.starts_with("authorization input or evaluation is invalid:")
        || error.starts_with("authentication was cancelled")
        || error.starts_with("authentication provider failed:")
        || error.starts_with("authentication observation is invalid:")
        || error.starts_with("authentication principal differs")
        || error.starts_with("local operating-system principal changed")
        || error.starts_with("repository authority signing failed:")
}

#[cfg(test)]
mod tests {
    use super::{is_authority_refusal, is_stale_entry_error};

    #[test]
    fn stable_decision_codes_cover_only_their_exact_failure_class() {
        assert!(is_stale_entry_error(
            "Decision Inbox entry changed: requested sha256:old, current sha256:new"
        ));
        assert!(!is_stale_entry_error(
            "--if-entry-root must be a full sha256 root"
        ));
        assert!(is_authority_refusal("authorization denied"));
        assert!(is_authority_refusal(
            "repository authority signing failed: no matching key"
        ));
        assert!(!is_authority_refusal(
            "review Decision committed as record record_1 but exact Git publication failed; do not retry the Decision"
        ));
        assert!(!is_authority_refusal(
            "review Decision Git preflight failed before installation"
        ));
    }
}
