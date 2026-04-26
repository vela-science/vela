//! `vela` — the command-line binary.
//!
//! Wires the agent handlers from `vela-scientist` into the
//! substrate's CLI dispatch table, then hands off to
//! `vela_protocol::cli::run_from_args`.
//!
//! Doctrine: the substrate library doesn't know about agents. This
//! binary does the marriage.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use colored::Colorize;

fn main() {
    vela_protocol::cli::register_scout_handler(scout_handler);
    vela_protocol::cli::register_notes_handler(notes_handler);
    vela_protocol::cli::run_from_args();
}

/// Adapter from the substrate's `ScoutHandler` signature to
/// `vela_scientist::scout::run`. Owns the user-facing rendering of
/// the report so the agent crate can stay UI-free.
fn scout_handler(
    folder: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::scout::{ScoutInput, run};
        // The substrate's CLI plumbs through a generic `backend`
        // string from the `vela scout --backend` flag. v0.22's only
        // backend is `claude-cli`, so we treat the legacy flag as a
        // model-alias override (e.g. `--backend sonnet`) and ignore
        // empty / "claude-cli" / "default" values.
        let model = backend.and_then(|b| {
            let trimmed = b.trim().to_string();
            if trimmed.is_empty() || trimmed == "claude-cli" || trimmed == "default" {
                None
            } else {
                Some(trimmed)
            }
        });
        let input = ScoutInput {
            folder: folder.clone(),
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI")
                .unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
        };
        match run(input).await {
            Ok(report) => {
                if json_out {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    return;
                }
                println!();
                println!("  {}", "VELA · SCOUT · LITERATURE".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:           {}", report.run.agent);
                println!("  run id:          {}", report.run.run_id);
                println!(
                    "  model:           {}",
                    if report.run.model.is_empty() {
                        "(env default)"
                    } else {
                        &report.run.model
                    }
                );
                println!("  folder:          {}", folder.display());
                println!("  frontier:        {}", frontier.display());
                println!("  pdfs seen:       {}", report.pdfs_seen);
                println!("  pdfs processed:  {}", report.pdfs_processed);
                println!("  candidates:      {}", report.candidates_emitted);
                println!(
                    "  proposals:       {} {}",
                    report.proposals_written,
                    if dry_run {
                        "(dry-run, not written)"
                    } else {
                        "(appended to frontier)"
                    }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:         {} files", report.skipped.len());
                    for s in report.skipped.iter().take(5) {
                        println!("    - {}: {}", s.path, s.reason);
                    }
                    if report.skipped.len() > 5 {
                        println!("    … {} more", report.skipped.len() - 5);
                    }
                }
                println!();
                if !dry_run && report.proposals_written > 0 {
                    println!(
                        "  next: review in the Workbench Inbox, then `vela queue sign --all`."
                    );
                }
            }
            Err(e) => {
                eprintln!("  scout failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Adapter for `vela compile-notes` (v0.23). Same shape as
/// scout_handler — render the report to terminal in a friendly form,
/// or as JSON when requested.
fn notes_handler(
    vault: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    max_files: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::notes::{NotesInput, run};
        let model = backend.and_then(|b| {
            let trimmed = b.trim().to_string();
            if trimmed.is_empty() || trimmed == "claude-cli" || trimmed == "default" {
                None
            } else {
                Some(trimmed)
            }
        });
        let input = NotesInput {
            vault: vault.clone(),
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI")
                .unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            max_files: max_files.or(Some(50)),
        };
        match run(input).await {
            Ok(report) => {
                if json_out {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    return;
                }
                println!();
                println!("  {}", "VELA · COMPILE-NOTES · NOTES-COMPILER".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:                 {}", report.run.agent);
                println!("  run id:                {}", report.run.run_id);
                println!(
                    "  model:                 {}",
                    if report.run.model.is_empty() {
                        "(env default)"
                    } else {
                        &report.run.model
                    }
                );
                println!("  vault:                 {}", vault.display());
                println!("  frontier:              {}", frontier.display());
                println!("  notes seen:            {}", report.notes_seen);
                println!("  notes processed:       {}", report.notes_processed);
                println!("  open questions:        {}", report.open_questions_emitted);
                println!("  hypotheses:            {}", report.hypotheses_emitted);
                println!(
                    "  candidate findings:    {}",
                    report.candidate_findings_emitted
                );
                println!("  tensions:              {}", report.tensions_emitted);
                println!(
                    "  proposals:             {} {}",
                    report.proposals_written,
                    if dry_run {
                        "(dry-run, not written)"
                    } else {
                        "(appended to frontier)"
                    }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:               {} files", report.skipped.len());
                    for s in report.skipped.iter().take(5) {
                        println!("    - {}: {}", s.path, s.reason);
                    }
                    if report.skipped.len() > 5 {
                        println!("    … {} more", report.skipped.len() - 5);
                    }
                }
                println!();
                if !dry_run && report.proposals_written > 0 {
                    println!(
                        "  next: review in the Workbench Inbox, then `vela queue sign --all`."
                    );
                }
            }
            Err(e) => {
                eprintln!("  notes compiler failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Tiny copy of `vela_protocol::cli_style::tick_row` to keep the
/// binary independent of crate-private chrome helpers. If the
/// instrument styling diverges, that's fine — this binary's output
/// is local-only.
fn tick_row(width: usize) -> String {
    let mut out = String::with_capacity(width);
    for i in 0..width {
        out.push(if i % 4 == 0 { '·' } else { ' ' });
    }
    out
}
