use crate::cli::{fail_return, print_json};
use crate::cli_commands::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use vela_protocol::cli_style as style;
use vela_protocol::proposals;
use vela_protocol::repo;

pub(crate) fn cmd_proposals(action: ProposalAction) {
    match action {
        ProposalAction::List {
            frontier,
            status,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposals_list = proposals::list(&frontier_state, status.as_deref());
            let payload = json!({
                "ok": true,
                "command": "proposals.list",
                "frontier": frontier_state.project.name,
                "status_filter": status,
                "summary": proposals::summary(&frontier_state),
                "proposals": proposals_list,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals list");
                println!("  frontier: {}", frontier_state.project.name);
                println!(
                    "  proposals: {}",
                    payload["proposals"].as_array().map_or(0, Vec::len)
                );
            }
        }
        ProposalAction::Show {
            frontier,
            proposal_id,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposal =
                proposals::show(&frontier_state, &proposal_id).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.show",
                "frontier": frontier_state.project.name,
                "proposal": proposal,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals show");
                println!("  frontier: {}", frontier_state.project.name);
                println!("  proposal: {}", proposal_id);
                println!("  kind: {}", proposal.kind);
                println!("  status: {}", proposal.status);
            }
        }
        ProposalAction::Preview {
            frontier,
            proposal_id,
            reviewer: _,
            json,
        } => {
            let review = crate::review_material::ReviewProjection::one(&frontier, &proposal_id)
                .unwrap_or_else(|error| fail_return(&error.to_string()));
            let payload = json!({
                "ok": true,
                "command": "proposals.preview",
                "frontier": frontier.display().to_string(),
                "review": review,
            });
            if json {
                print_json(&payload);
            } else {
                println!("vela proposals preview");
                println!("  proposal: {}", proposal_id);
                for line in crate::cli::sign_session::render_decision_brief_lines(&review.brief) {
                    println!("    {line}");
                }
            }
        }
        ProposalAction::Validate { source, json } => {
            let report = proposals::validate_source(&source).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": report.ok,
                "command": "proposals.validate",
                "source": source.display().to_string(),
                "summary": {
                    "checked": report.checked,
                    "valid": report.valid,
                    "invalid": report.invalid,
                },
                "proposal_ids": report.proposal_ids,
                "errors": report.errors,
            });
            if json {
                print_json(&payload);
            } else if report.ok {
                println!("{} validated {} proposals", style::ok("ok"), report.valid);
            } else {
                println!(
                    "{} validated {} proposals, {} invalid",
                    style::lost("lost"),
                    report.valid,
                    report.invalid
                );
                for error in &report.errors {
                    println!("  · {error}");
                }
                std::process::exit(1);
            }
        }
        ProposalAction::Export {
            frontier,
            output,
            status,
            json,
        } => {
            let count = proposals::export_to_path(&frontier, &output, status.as_deref())
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.export",
                "frontier": frontier.display().to_string(),
                "output": output.display().to_string(),
                "status": status,
                "exported": count,
            });
            if json {
                print_json(&payload);
            } else {
                println!("sealed · {count} proposals · {}", output.display());
            }
        }
    }
}

/// The derived credit view for a finding (read-only projection). Renders the
/// accountable human author(s) of record, the disclosed contributors, and the
/// originating agents. A machine never appears as an author.
pub(crate) fn cmd_credit(frontier: &Path, finding_id: &str, json_out: bool) {
    let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
    let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
    let view = vela_protocol::credit::credit(&proj, finding_id)
        .unwrap_or_else(|| fail_return(&format!("no such finding: {finding_id}")));
    if json_out {
        print_json(&json!({
            "command": "credit",
            "schema": "vela.credit.v0.1",
            "credit": view,
        }));
        return;
    }
    println!("credit · {finding_id}");
    if view.author_of_record.is_empty() {
        println!("  author of record: (none — no accountable author yet)");
    } else {
        println!("  author of record: {}", view.author_of_record.join(", "));
    }
    if view.contributors.is_empty() {
        println!("  contributors:     (none recorded)");
    } else {
        println!("  contributors:");
        for c in &view.contributors {
            println!(
                "    {} [{}] {} — {}",
                c.agent_id, c.agent_kind, c.role, c.unit
            );
        }
    }
    if !view.originating_agents.is_empty() {
        println!("  originating agents (disclosed, not authors):");
        for c in &view.originating_agents {
            println!(
                "    {} [{}] originated {}",
                c.agent_id, c.agent_kind, c.unit
            );
        }
    }
    println!("  {}", view.statement);
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
