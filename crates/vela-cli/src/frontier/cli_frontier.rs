//! `cmd_frontier` and its handler logic, split out of cli.rs.

use crate::cli::{
    cmd_frontier_audit, cmd_frontier_diff, cmd_frontier_release, cmd_frontier_releases, fail,
    fail_return, print_json,
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
                    "vela finding add <path> --assertion '...' --author 'reviewer:you' --apply",
                    "vela sign generate-keypair --out keys",
                    "vela actor add <path> reviewer:you --pubkey \"$(cat keys/public.key)\"",
                    "git push   # publication; bind once with: vela hub register-git <vfr> --remote <url>",
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
                println!(
                    "    1. vela finding add {} --assertion '...' --author 'reviewer:you' --apply",
                    path.display()
                );
                println!("    2. vela id keygen --out keys");
                println!(
                    "    3. vela actor add {} reviewer:you --pubkey \"$(cat keys/public.key)\"",
                    path.display()
                );
                println!(
                    "    4. git push   # publication; bind once with: vela hub register-git <vfr> --remote <url>"
                );
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
        FrontierAction::AddDep {
            frontier,
            vfr_id,
            locator,
            snapshot,
            name,
            json,
        } => {
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if p.project
                .dependencies
                .iter()
                .any(|d| d.vfr_id.as_deref() == Some(&vfr_id))
            {
                fail(&format!(
                    "cross-frontier dependency '{vfr_id}' already declared; remove it first via `vela frontier remove-dep`"
                ));
            }
            let dep = ProjectDependency {
                name: name.unwrap_or_else(|| vfr_id.clone()),
                source: "vela.hub".into(),
                version: None,
                pinned_hash: None,
                vfr_id: Some(vfr_id.clone()),
                locator: Some(locator.clone()),
                pinned_snapshot_hash: Some(snapshot.clone()),
            };
            p.project.dependencies.push(dep);
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "frontier.add-dep",
                "frontier": frontier.display().to_string(),
                "vfr_id": vfr_id,
                "locator": locator,
                "pinned_snapshot_hash": snapshot,
                "declared_count": p.project.dependencies.len(),
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} declared cross-frontier dep {vfr_id}",
                    style::ok("frontier")
                );
                println!("  locator:  {locator}");
                println!("  snapshot: {snapshot}");
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
            frontier,
            since,
            week,
            json,
        } => cmd_frontier_diff(&frontier, since.as_deref(), week.as_deref(), json),
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
            "frontier_id": proj.frontier_id(),
            "open_total": ranked.len(),
            "candidates": shown,
            "contested": contested,
        }));
        return;
    }
    if ranked.is_empty() {
        println!("no open findings to rank — the frontier has no open work surfaced.");
        return;
    }
    println!(
        "frontier rank: {} open finding(s), most-solvable first (structural support)",
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
    println!("\n  advice, not authority: claim one with `vela work <id>`; the verifier decides.");
}
