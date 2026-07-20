//! `cmd_frontier` and its handler logic, split out of cli.rs.

use crate::cli::{
    cmd_frontier_audit, cmd_frontier_release, cmd_frontier_releases, fail_return, print_json,
};
use crate::cli_commands::FrontierAction;
use colored::Colorize;
use serde_json::json;
use vela_protocol::cli_style as style;
use vela_protocol::frontier_repo;
use vela_protocol::project;
use vela_protocol::proposals;

pub(crate) fn cmd_frontier(action: FrontierAction) {
    use vela_protocol::project::ProjectDependency;
    use vela_protocol::repo;
    match action {
        FrontierAction::New {
            path,
            name,
            description,
            force,
            json,
        } => {
            if path.exists() && !force {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Exists,
                    &format!("{} already exists", path.display()),
                    Some("pass --force to overwrite"),
                );
            }
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let project = project::Project {
                vela_version: project::VELA_SCHEMA_VERSION.to_string(),
                schema: project::VELA_SCHEMA_URL.to_string(),
                frontier_id: None,
                project: project::ProjectMeta {
                    name: name.clone(),
                    description: description.clone(),
                    compiled_at: now,
                    compiler: project::VELA_COMPILER_VERSION.to_string(),
                    papers_processed: 0,
                    errors: 0,
                    dependencies: Vec::new(),
                },
                stats: project::ProjectStats::default(),
                findings: Vec::new(),
                sources: Vec::new(),
                evidence_atoms: Vec::new(),
                condition_records: Vec::new(),
                review_events: Vec::new(),
                confidence_updates: Vec::new(),
                events: Vec::new(),
                proposals: Vec::new(),
                proof_state: proposals::ProofState::default(),
                signatures: Vec::new(),
                actors: Vec::new(),
                artifacts: Vec::new(),
                released_diff_packs: Vec::new(),
                verdict_conflicts: Vec::new(),
                contradictions: Vec::new(),
                verifier_attachments: Vec::new(),
                attempts: Vec::new(),
                attempt_resolutions: Vec::new(),
                transfers: Vec::new(),
                endorsements: Vec::new(),
                statement_attestations: Vec::new(),
                anchor_links: Vec::new(),
                attempt_claims: Vec::new(),
                statement_registrations: Vec::new(),
            };
            repo::save_to_path(&path, &project).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "frontier.new",
                "path": path.display().to_string(),
                "name": name,
                "schema": project::VELA_SCHEMA_URL,
                "vela_version": env!("CARGO_PKG_VERSION"),
                "next_steps": [
                    "vela id create --handle <your-name>",
                    "vela actor add <path>",
                    "vela land <receipt.json> --frontier <path> --as agent:<you>",
                    "vela sign --frontier <path>",
                    "git push   # publication to a Hub-configured source repository",
                ],
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} scaffolded frontier '{name}' at {}",
                    style::ok("frontier"),
                    path.display()
                );
                println!("  next steps:");
                println!("    1. vela id create --handle <your-name>");
                println!("    2. vela actor add {}", path.display());
                println!(
                    "    3. vela land <receipt.json> --frontier {} --as agent:<you>",
                    path.display()
                );
                println!("    4. vela sign --frontier {}", path.display());
                println!("    5. git push   # publication to a Hub-configured source repository");
            }
        }
        FrontierAction::Materialize { frontier, json } => {
            let spin = (!json).then(|| {
                crate::cli::progress::Spinner::start("materializing derived views from the log")
            });
            let payload = frontier_repo::materialize(&frontier).unwrap_or_else(|e| fail_return(&e));
            if let Some(s) = spin {
                s.finish("materialized");
            }
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} materialized frontier repo at {}",
                    style::ok("frontier"),
                    frontier.display()
                );
            }
        }
        FrontierAction::ListDeps { frontier, json } => {
            let p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let deps: Vec<&ProjectDependency> = p.project.dependencies.iter().collect();
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "frontier.list-deps",
                    "frontier": frontier.display().to_string(),
                    "count": deps.len(),
                    "dependencies": deps,
                });
                print_json(&payload);
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · FRONTIER · LIST-DEPS · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if deps.is_empty() {
                    println!("  (no dependencies declared)");
                } else {
                    for d in &deps {
                        let kind = if d.is_cross_frontier() {
                            "cross-frontier"
                        } else {
                            "compile-time"
                        };
                        println!("  · {} [{kind}]", d.name);
                        if let Some(v) = &d.vfr_id {
                            println!("    vfr_id:   {v}");
                        }
                        if let Some(l) = &d.locator {
                            println!("    locator:  {l}");
                        }
                        if let Some(s) = &d.pinned_snapshot_hash {
                            println!("    snapshot: {s}");
                        }
                    }
                }
            }
        }
        FrontierAction::Diff {
            left,
            right,
            json,
            quiet,
        } => vela_protocol::diff::run(&left, &right, json, quiet),
        FrontierAction::RecoverPublication {
            operation,
            frontier,
            push,
            json,
        } => {
            crate::ui::set_mode("frontier recover-publication", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let opts = if push {
                crate::config::git_publish::PublishOptions::pushing()
            } else {
                crate::config::git_publish::PublishOptions::new(false)
            };
            let publication =
                match crate::decision_plan::recover_decision_operation(&dir, &operation) {
                    Ok(Some(outcome)) => crate::cli::sign_session::publish_exact_decision(
                        &dir,
                        &format!("recover decision: {operation}"),
                        &outcome,
                        &opts,
                    ),
                    Ok(None) => {
                        crate::config::git_publish::recover_publication(&dir, &operation, &opts)
                    }
                    Err(error) => crate::config::git_publish::PublicationOutcome {
                        state: crate::config::git_publish::PublicationState::Unknown {
                            reason: error.to_string(),
                        },
                        recovery_command: None,
                    },
                };
            let ok = matches!(
                &publication.state,
                crate::config::git_publish::PublicationState::Unchanged { .. }
                    | crate::config::git_publish::PublicationState::CommittedLocal { .. }
                    | crate::config::git_publish::PublicationState::Pushed { .. }
            );
            let payload = json!({
                "ok": ok,
                "command": "frontier.recover-publication",
                "operation_id": operation,
                "publication": publication,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "frontier publication recovery · {}",
                    if ok { "recovered" } else { "blocked" }
                );
                println!("  operation: {operation}");
                println!("  publication: {}", payload["publication"]);
            }
            if !ok {
                std::process::exit(1);
            }
        }
        FrontierAction::Release {
            frontier,
            name,
            notes,
            previous,
            json,
        } => cmd_frontier_release(frontier, name, notes, previous, json),
        FrontierAction::Releases { frontier, json } => cmd_frontier_releases(frontier, json),
        FrontierAction::Audit { frontier, json } => cmd_frontier_audit(frontier, json),
        FrontierAction::Rank {
            frontier,
            limit,
            json,
        } => cmd_frontier_rank(&frontier, limit, json),
    }
}

/// `vela frontier rank` — rank OPEN findings by accumulating structural support
/// (which is closest to a verifier-run from done). A read-only projection over
/// the typed frontier graph; carries the popularity baseline and the evidence
/// behind each score so the suggestion is inspectable.
fn cmd_frontier_rank(frontier: &std::path::Path, limit: usize, json: bool) {
    use vela_protocol::frontier_identification::{
        frontier_identification, heterogeneity_surfacing,
    };
    use vela_protocol::repo;
    let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
    let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
    let ranked = frontier_identification(&proj);
    // The companion query: relations in live disagreement (Garg's heterogeneity
    // surfacing). Never auto-adjudicated — a lead for the human review queue.
    let contested = heterogeneity_surfacing(&proj);
    let shown: Vec<_> = ranked.iter().take(limit).collect();
    if json {
        print_json(&json!({
            "command": "frontier rank",
            "schema": "vela.frontier_rank.v0.1",
            "ranking_kind": "structural_opportunity",
            "authority": "advice_only",
            "work_queue": false,
            "producer_work_command": "vela next . --json",
            "frontier_id": proj.frontier_id(),
            "open_total": ranked.len(),
            "candidates": shown,
            "contested": contested,
        }));
        return;
    }
    if ranked.is_empty() {
        println!(
            "no structural opportunities are currently surfaced; use `vela next . --json` for producer work."
        );
        return;
    }
    println!(
        "frontier rank: {} structural opportunity candidate(s) (advice only, not the producer work queue)",
        ranked.len()
    );
    for (i, c) in shown.iter().enumerate() {
        println!(
            "{:>3}. {}  {}",
            i + 1,
            c.id,
            style::dim(&c.label.chars().take(70).collect::<String>())
        );
        println!(
            "      score {:.2} (popularity baseline {:.0}) — {}",
            c.score, c.baseline, c.why
        );
        if !c.evidence.is_empty() {
            let ev: Vec<&str> = c.evidence.iter().take(4).map(String::as_str).collect();
            println!("      evidence: {}", ev.join(", "));
        }
    }
    if !contested.is_empty() {
        println!(
            "\n  contested ({} relation(s) in live disagreement — for human review, never auto-resolved):",
            contested.len()
        );
        for h in contested.iter().take(limit) {
            println!(
                "    {} <-> {}  {}",
                h.finding,
                h.partner,
                style::dim(&h.label.chars().take(56).collect::<String>())
            );
        }
    }
    println!(
        "\n  structural advice, not authority. Use `vela next . --json` for canonical producer offers; verification is not acceptance."
    );
}
