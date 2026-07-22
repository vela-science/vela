use crate::cli::{fail_return, print_json};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::PathBuf;
use vela_protocol::proposals;
use vela_protocol::repo;

fn proposal_next_actions(frontier: &std::path::Path, proposal_id: &str) -> Vec<Value> {
    let Ok(project) = repo::load_from_path(frontier) else {
        return Vec::new();
    };
    let Some(proposal) = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
    else {
        return Vec::new();
    };
    if proposal.status != "pending_review" || proposal.kind != "finding.add" {
        return Vec::new();
    }
    let finding = proposal.payload.get("finding").cloned().and_then(|value| {
        serde_json::from_value::<vela_protocol::bundle::FindingBundle>(value).ok()
    });
    let Some(finding) = finding else {
        return Vec::new();
    };
    let attachments = project
        .verifier_attachments
        .iter()
        .filter(|attachment| attachment.target == finding.id)
        .cloned()
        .collect::<Vec<_>>();
    let gate = vela_protocol::verifier_attachment::derive_gate_status(
        &vela_protocol::verifier_attachment::claim_digest(&finding.assertion.text),
        &attachments,
    );
    let mut actions = vec![json!({
        "kind": "reproduce_pending_artifact",
        "authority": "read_only",
        "command": format!("vela reproduce {} --proposal {proposal_id}", frontier.display()),
        "reason": "Re-run only the immutable artifacts bound to this pending proposal."
    })];
    if gate.status != vela_protocol::verifier_attachment::GateStatus::Verified {
        actions.push(json!({
            "kind": "add_verifier_evidence",
            "authority": "evidence_only",
            "command": format!("vela verify attach {} <attachment.json> --proposal {proposal_id} --as verifier:<actor>", frontier.display()),
            "reason": gate.reasons.first().cloned().unwrap_or_else(|| "Additional verification evidence is required.".to_string())
        }));
    }
    actions.push(json!({
        "kind": "human_decision",
        "authority": "human_key_required",
        "command": format!("vela review decide {} {proposal_id} --accept|--reject --reason <why> --json", frontier.display()),
        "reason": "Verification evidence never accepts the proposal; a protected human decision remains separate."
    }));
    actions
}

/// Compact 0.9 review surface. Lists never embed Decision Briefs; callers use
/// `review show` or `review preview` for one exact proposal.
fn compact_proposal_claim(kind: &str, value: &Value) -> String {
    let finding_assertion = || value.pointer("/payload/finding/assertion/text");
    let generic_claim = || {
        value
            .pointer("/payload/claim")
            .or_else(|| value.pointer("/change/claim"))
            .or_else(|| value.get("claim"))
    };
    (if kind == "finding.add" {
        finding_assertion().or_else(generic_claim)
    } else {
        generic_claim().or_else(finding_assertion)
    })
    .and_then(Value::as_str)
    .unwrap_or("")
    .chars()
    .take(240)
    .collect()
}

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
                    let created_at = chrono::DateTime::parse_from_rfc3339(&proposal.created_at)
                        .unwrap_or_else(|error| {
                            fail_return(&format!(
                                "proposal {} has invalid created_at: {error}",
                                proposal.id
                            ))
                        })
                        .to_utc();
                    let value = serde_json::to_value(proposal).unwrap_or(Value::Null);
                    let claim = compact_proposal_claim(&proposal.kind, &value);
                    let target = value
                        .pointer("/target/id")
                        .or_else(|| value.pointer("/payload/target/id"))
                        .or_else(|| value.get("finding_id"))
                        .and_then(Value::as_str);
                    (created_at, json!({
                        "proposal_id": proposal.id,
                        "created_at": proposal.created_at,
                        "kind": proposal.kind,
                        "status": proposal.status,
                        "target": target,
                        "claim": claim,
                        "content_root": format!(
                            "sha256:{}",
                            vela_protocol::canonical::sha256_canonical(proposal)
                                .unwrap_or_else(|error| fail_return(&format!("canonicalize proposal: {error}")))
                        ),
                    }))
                })
                .collect::<Vec<_>>();
            items.sort_by(|left, right| {
                right.0.cmp(&left.0).then_with(|| {
                    left.1["proposal_id"]
                        .as_str()
                        .cmp(&right.1["proposal_id"].as_str())
                })
            });
            let items = items.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
            let total = items.len();
            let limit = limit.clamp(1, 100);
            let start = match cursor.as_deref() {
                None => 0,
                Some(cursor) => items
                    .iter()
                    .position(|item| item["proposal_id"].as_str() == Some(cursor))
                    .map(|index| index + 1)
                    .unwrap_or_else(|| {
                        fail_return("review cursor does not name an exact proposal")
                    }),
            };
            let mut page = items
                .into_iter()
                .skip(start)
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
                "order": "created_at_desc_then_proposal_id",
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
                        "  {}  {}  {}  {}",
                        item["proposal_id"].as_str().unwrap_or(""),
                        item["created_at"].as_str().unwrap_or(""),
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
            let next_actions = proposal_next_actions(&frontier, &proposal_id);
            let payload = json!({
                "ok": true,
                "command": "review.show",
                "schema": "vela.review.v1",
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "review": review,
                "next_actions": next_actions,
            });
            if json {
                print_json(&payload);
            } else {
                println!("review · {proposal_id}");
                for line in crate::cli::sign_session::render_decision_brief_lines(&review.brief) {
                    println!("  {line}");
                }
                println!("  next actions:");
                for action in &next_actions {
                    println!("    {}", action["command"].as_str().unwrap_or(""));
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
