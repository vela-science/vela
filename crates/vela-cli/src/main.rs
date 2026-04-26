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
    // v0.22+ agent handlers
    vela_protocol::cli::register_scout_handler(scout_handler);
    vela_protocol::cli::register_notes_handler(notes_handler);
    vela_protocol::cli::register_code_handler(code_handler);
    vela_protocol::cli::register_datasets_handler(datasets_handler);
    // v0.27 substrate-cleanup handlers (legacy CLI surfaces that
    // moved their LLM bodies into vela-scientist)
    vela_protocol::cli::register_ingest_handler(ingest_handler);
    vela_protocol::cli::register_compile_handler(compile_handler);
    vela_protocol::cli::register_jats_handler(jats_handler);
    // v0.28 agent handlers
    vela_protocol::cli::register_reviewer_handler(reviewer_handler);
    vela_protocol::cli::register_tensions_handler(tensions_handler);
    vela_protocol::cli::register_experiments_handler(experiments_handler);
    vela_protocol::cli::run_from_args();
}

/// Adapter for the legacy file-ingest path
/// (`vela ingest --pdf / --csv / --text / --doi`). Body lives in
/// `vela_scientist::legacy_ingest::run_file_ingest`.
#[allow(clippy::too_many_arguments)]
fn ingest_handler(
    frontier: PathBuf,
    pdf: Option<PathBuf>,
    csv: Option<PathBuf>,
    text: Option<PathBuf>,
    doi: Option<String>,
    backend: Option<String>,
    assertion_type_override: Option<String>,
    assertion_col: Option<String>,
    confidence_col: Option<String>,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        vela_scientist::legacy_ingest::run_file_ingest(
            &frontier,
            pdf.as_deref(),
            csv.as_deref(),
            text.as_deref(),
            doi.as_deref(),
            backend.as_deref(),
            assertion_type_override.as_deref(),
            assertion_col.as_deref(),
            confidence_col.as_deref(),
        )
        .await;
    })
}

/// Adapter for `vela compile`. Body lives in
/// `vela_scientist::legacy_compile::cmd_compile`.
fn compile_handler(
    topic: String,
    max_papers: usize,
    output: PathBuf,
    backend: Option<String>,
    fulltext: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        vela_scientist::legacy_compile::cmd_compile(
            &topic,
            max_papers,
            &output,
            backend.as_deref(),
            fulltext,
        )
        .await;
    })
}

/// Adapter for `vela jats`. Body lives in
/// `vela_scientist::legacy_compile::cmd_jats`.
fn jats_handler(
    source: String,
    output: PathBuf,
    backend: Option<String>,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        vela_scientist::legacy_compile::cmd_jats(&source, &output, backend.as_deref()).await;
    })
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
    max_items_per_category: Option<usize>,
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
            max_items_per_category: max_items_per_category.or(Some(4)),
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

/// Adapter for `vela compile-code` (v0.24).
fn code_handler(
    root: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    max_files: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::code_analyst::{CodeAnalystInput, run};
        let model = backend.and_then(|b| {
            let trimmed = b.trim().to_string();
            if trimmed.is_empty() || trimmed == "claude-cli" || trimmed == "default" {
                None
            } else {
                Some(trimmed)
            }
        });
        let input = CodeAnalystInput {
            root: root.clone(),
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI")
                .unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            max_files: max_files.or(Some(30)),
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
                println!("  {}", "VELA · COMPILE-CODE · CODE-ANALYST".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:                {}", report.run.agent);
                println!("  run id:               {}", report.run.run_id);
                println!(
                    "  model:                {}",
                    if report.run.model.is_empty() {
                        "(env default)"
                    } else {
                        &report.run.model
                    }
                );
                println!("  root:                 {}", root.display());
                println!("  frontier:             {}", frontier.display());
                println!("  files seen:           {}", report.files_seen);
                println!("  notebooks processed:  {}", report.notebooks_processed);
                println!("  scripts processed:    {}", report.scripts_processed);
                println!("  analyses:             {}", report.analyses_emitted);
                println!("  code findings:        {}", report.code_findings_emitted);
                println!("  experiment intents:   {}", report.experiment_intents_emitted);
                println!(
                    "  proposals:            {} {}",
                    report.proposals_written,
                    if dry_run {
                        "(dry-run, not written)"
                    } else {
                        "(appended to frontier)"
                    }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:              {} files", report.skipped.len());
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
                eprintln!("  code analyst failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Adapter for `vela compile-data` (v0.25).
fn datasets_handler(
    root: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    sample_rows: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::datasets::{DatasetInput, run};
        let model = backend.and_then(|b| {
            let trimmed = b.trim().to_string();
            if trimmed.is_empty() || trimmed == "claude-cli" || trimmed == "default" {
                None
            } else {
                Some(trimmed)
            }
        });
        let input = DatasetInput {
            root: root.clone(),
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI")
                .unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            sample_rows: sample_rows.unwrap_or(50),
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
                println!("  {}", "VELA · COMPILE-DATA · DATASETS".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:                {}", report.run.agent);
                println!("  run id:               {}", report.run.run_id);
                println!(
                    "  model:                {}",
                    if report.run.model.is_empty() {
                        "(env default)"
                    } else {
                        &report.run.model
                    }
                );
                println!("  root:                 {}", root.display());
                println!("  frontier:             {}", frontier.display());
                println!("  datasets seen:        {}", report.datasets_seen);
                println!("  csv processed:        {}", report.csv_processed);
                println!("  parquet processed:    {}", report.parquet_processed);
                println!("  dataset summaries:    {}", report.dataset_summaries_emitted);
                println!("  supported claims:     {}", report.supported_claims_emitted);
                println!(
                    "  proposals:            {} {}",
                    report.proposals_written,
                    if dry_run {
                        "(dry-run, not written)"
                    } else {
                        "(appended to frontier)"
                    }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:              {} files", report.skipped.len());
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
                eprintln!("  datasets agent failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Adapter for `vela review-pending` (v0.28).
fn reviewer_handler(
    frontier: PathBuf,
    backend: Option<String>,
    max_proposals: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::reviewer::{ReviewerInput, run};
        let model = backend.and_then(|b| {
            let t = b.trim().to_string();
            if t.is_empty() || t == "claude-cli" || t == "default" {
                None
            } else {
                Some(t)
            }
        });
        let input = ReviewerInput {
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI").unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            max_proposals: max_proposals.or(Some(30)),
        };
        match run(input).await {
            Ok(report) => {
                if json_out {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
                    return;
                }
                println!();
                println!("  {}", "VELA · REVIEW-PENDING · REVIEWER-AGENT".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:           {}", report.run.agent);
                println!("  run id:          {}", report.run.run_id);
                println!("  frontier:        {}", frontier.display());
                println!("  pending seen:    {}", report.pending_seen);
                println!("  scored:          {}", report.scored);
                println!(
                    "  notes:           {} {}",
                    report.notes_written,
                    if dry_run { "(dry-run, not written)" } else { "(appended to frontier)" }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:         {}", report.skipped.len());
                    for s in report.skipped.iter().take(5) {
                        println!("    - {}: {}", s.proposal_id, s.reason);
                    }
                }
                println!();
            }
            Err(e) => {
                eprintln!("  reviewer agent failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Adapter for `vela find-tensions` (v0.28).
fn tensions_handler(
    frontier: PathBuf,
    backend: Option<String>,
    max_findings: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::tensions::{TensionsInput, run};
        let model = backend.and_then(|b| {
            let t = b.trim().to_string();
            if t.is_empty() || t == "claude-cli" || t == "default" {
                None
            } else {
                Some(t)
            }
        });
        let input = TensionsInput {
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI").unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            max_findings: max_findings.or(Some(60)),
        };
        match run(input).await {
            Ok(report) => {
                if json_out {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
                    return;
                }
                println!();
                println!("  {}", "VELA · FIND-TENSIONS · CONTRADICTION-FINDER".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:               {}", report.run.agent);
                println!("  run id:              {}", report.run.run_id);
                println!("  frontier:            {}", frontier.display());
                println!("  findings seen:       {}", report.findings_seen);
                println!("  batches processed:   {}", report.batches_processed);
                println!("  tensions emitted:    {}", report.tensions_emitted);
                println!(
                    "  proposals:           {} {}",
                    report.proposals_written,
                    if dry_run { "(dry-run, not written)" } else { "(appended to frontier)" }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped batches:     {}", report.skipped.len());
                    for s in report.skipped.iter().take(5) {
                        println!("    - batch {}: {}", s.batch, s.reason);
                    }
                }
                println!();
            }
            Err(e) => {
                eprintln!("  contradiction finder failed: {e}");
                std::process::exit(1);
            }
        }
    })
}

/// Adapter for `vela plan-experiments` (v0.28).
fn experiments_handler(
    frontier: PathBuf,
    backend: Option<String>,
    max_findings: Option<usize>,
    dry_run: bool,
    json_out: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        use vela_scientist::experiments::{ExperimentsInput, run};
        let model = backend.and_then(|b| {
            let t = b.trim().to_string();
            if t.is_empty() || t == "claude-cli" || t == "default" {
                None
            } else {
                Some(t)
            }
        });
        let input = ExperimentsInput {
            frontier_path: frontier.clone(),
            model,
            cli_command: std::env::var("VELA_SCIENTIST_CLI").unwrap_or_else(|_| "claude".to_string()),
            apply: !dry_run,
            max_findings: max_findings.or(Some(20)),
        };
        match run(input).await {
            Ok(report) => {
                if json_out {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
                    return;
                }
                println!();
                println!("  {}", "VELA · PLAN-EXPERIMENTS · EXPERIMENT-PLANNER".dimmed());
                println!("  {}", tick_row(60));
                println!("  agent:               {}", report.run.agent);
                println!("  run id:              {}", report.run.run_id);
                println!("  frontier:            {}", frontier.display());
                println!("  questions seen:      {}", report.questions_seen);
                println!("  hypotheses seen:     {}", report.hypotheses_seen);
                println!("  experiments emitted: {}", report.experiments_emitted);
                println!(
                    "  proposals:           {} {}",
                    report.proposals_written,
                    if dry_run { "(dry-run, not written)" } else { "(appended to frontier)" }
                );
                if !report.skipped.is_empty() {
                    println!("  skipped:             {}", report.skipped.len());
                    for s in report.skipped.iter().take(5) {
                        println!("    - {}: {}", s.finding_id, s.reason);
                    }
                }
                println!();
            }
            Err(e) => {
                eprintln!("  experiment planner failed: {e}");
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
