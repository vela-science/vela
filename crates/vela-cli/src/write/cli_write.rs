use crate::cli::{fail_return, print_json};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::PathBuf;
use vela_protocol::proposals;
use vela_protocol::repo;

/// Compact 0.9 review surface. Lists never embed Decision Briefs; callers use
/// `review show` or `review preview` for one exact proposal.
pub(crate) fn cmd_review(action: ReviewAction) {
    match action {
        ReviewAction::List {
            frontier,
            status,
            limit,
            cursor,
            json,
        } => {
            let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let status = status.unwrap_or_else(|| "pending_review".to_string());
            let mut items = project
                .proposals
                .iter()
                .filter(|proposal| proposal.status == status)
                .map(|proposal| {
                    let value = serde_json::to_value(proposal).unwrap_or(Value::Null);
                    let claim = value
                        .pointer("/payload/claim")
                        .or_else(|| value.pointer("/change/claim"))
                        .or_else(|| value.get("claim"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(240)
                        .collect::<String>();
                    let target = value
                        .pointer("/target/id")
                        .or_else(|| value.pointer("/payload/target/id"))
                        .or_else(|| value.get("finding_id"))
                        .and_then(Value::as_str);
                    json!({
                        "proposal_id": proposal.id,
                        "kind": proposal.kind,
                        "status": proposal.status,
                        "target": target,
                        "claim": claim,
                        "content_root": format!(
                            "sha256:{}",
                            vela_protocol::canonical::sha256_canonical(proposal)
                                .unwrap_or_else(|error| fail_return(&format!("canonicalize proposal: {error}")))
                        ),
                    })
                })
                .collect::<Vec<_>>();
            items.sort_by(|left, right| {
                left["proposal_id"]
                    .as_str()
                    .cmp(&right["proposal_id"].as_str())
            });
            let total = items.len();
            let limit = limit.clamp(1, 100);
            let mut page = items
                .into_iter()
                .filter(|item| {
                    cursor.as_deref().is_none_or(|cursor| {
                        item["proposal_id"].as_str().is_some_and(|id| id > cursor)
                    })
                })
                .take(limit + 1)
                .collect::<Vec<_>>();
            let has_more = page.len() > limit;
            page.truncate(limit);
            let next_cursor = has_more
                .then(|| page.last().and_then(|item| item["proposal_id"].as_str()))
                .flatten();
            let payload = json!({
                "ok": true,
                "command": "review.list",
                "schema": "vela.review.v1",
                "frontier_id": project.frontier_id(),
                "event_log_root": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
                "proposal_state_root": format!("sha256:{}", proposals::proposal_state_hash(&project.proposals)),
                "status": status,
                "total": total,
                "returned": page.len(),
                "next_cursor": next_cursor,
                "items": page,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "review · {} {} proposal(s)",
                    payload["total"],
                    payload["status"].as_str().unwrap_or("")
                );
                for item in payload["items"].as_array().into_iter().flatten() {
                    println!(
                        "  {}  {}  {}",
                        item["proposal_id"].as_str().unwrap_or(""),
                        item["kind"].as_str().unwrap_or(""),
                        item["claim"].as_str().unwrap_or("")
                    );
                }
            }
        }
        ReviewAction::Show {
            frontier,
            proposal_id,
            json,
        } => {
            let review = crate::review_material::ReviewProjection::one(&frontier, &proposal_id)
                .unwrap_or_else(|error| fail_return(&error.to_string()));
            let payload = json!({
                "ok": true,
                "command": "review.show",
                "schema": "vela.review.v1",
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "review": review,
            });
            if json {
                print_json(&payload);
            } else {
                println!("review · {proposal_id}");
                for line in crate::cli::sign_session::render_decision_brief_lines(&review.brief) {
                    println!("  {line}");
                }
            }
        }
        ReviewAction::Preview {
            frontier,
            proposal_id,
            json,
        } => {
            let review = crate::review_material::ReviewProjection::one(&frontier, &proposal_id)
                .unwrap_or_else(|error| fail_return(&error.to_string()));
            let payload = json!({
                "ok": true,
                "command": "review.preview",
                "schema": "vela.review.v1",
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "review": review,
            });
            if json {
                print_json(&payload);
            } else {
                println!("review preview · {proposal_id}");
                for line in crate::cli::sign_session::render_decision_brief_lines(&review.brief) {
                    println!("  {line}");
                }
            }
        }
        ReviewAction::Decide {
            frontier,
            proposal_id,
            accept,
            reject,
            reason,
            confirm_root,
            confirm_at,
            json,
        } => crate::cli::sign_session::cmd_review_decide(
            frontier,
            &proposal_id,
            if accept {
                crate::decision_plan::DecisionAction::Accept
            } else if reject {
                crate::decision_plan::DecisionAction::Reject
            } else {
                unreachable!("clap requires one decision action")
            },
            reason,
            confirm_root.as_deref(),
            confirm_at.as_deref(),
            json,
        ),
        ReviewAction::Withdraw {
            frontier,
            proposal_id,
            actor,
            reason,
            json,
        } => crate::withdrawal::cmd_review_withdraw(frontier, &proposal_id, &actor, &reason, json),
        ReviewAction::Export {
            frontier,
            output,
            status,
            json,
        } => {
            let count = proposals::export_to_path(&frontier, &output, status.as_deref())
                .unwrap_or_else(|error| fail_return(&error));
            let payload = json!({
                "ok": true,
                "command": "review.export",
                "schema": "vela.review.v1",
                "frontier": frontier.display().to_string(),
                "output": output.display().to_string(),
                "status": status,
                "exported": count,
            });
            if json {
                print_json(&payload);
            } else {
                println!("exported · {count} proposal(s) · {}", output.display());
            }
        }
    }
}

pub(crate) fn cmd_artifact_retract(
    frontier: PathBuf,
    artifact_id: String,
    reason: String,
    actor: String,
    json: bool,
) {
    let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
    if !project
        .artifacts
        .iter()
        .any(|artifact| artifact.id == artifact_id)
    {
        crate::cli::fail_not_found::<()>(
            &format!("no artifact '{artifact_id}' in this frontier"),
            "inspect the frontier with `vela status <frontier> --json`",
        );
    }
    let report = vela_protocol::state::retract_artifact(&frontier, &artifact_id, &actor, &reason)
        .unwrap_or_else(|e| fail_return(&e));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .expect("failed to serialize artifact lifecycle report")
        );
    } else {
        println!("Artifact retirement proposal recorded");
        println!("  frontier: {}", report.frontier);
        println!("  artifact: {}", report.artifact_id);
        println!("  proposal: {}", report.proposal_id);
        println!("  status:   {}", report.status);
        println!("  route:    {}", report.route);
    }
}
