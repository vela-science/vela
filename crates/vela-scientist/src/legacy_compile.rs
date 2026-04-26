//! # Legacy `vela compile` and `vela jats` pipelines
//!
//! Moved from `vela-protocol::cli` in v0.27 (substrate cleanup).
//! Both commands' bodies live here so the substrate's CLI surface
//! keeps the flag parsing while the LLM-using pipeline runs in
//! the agent crate.
//!
//! Substrate's `cmd_compile` and `cmd_jats` are now thin
//! dispatchers that look up `CompileHandler` / `JatsHandler`
//! OnceLocks and forward the args here. The binary in `vela-cli`
//! registers these handlers at startup.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::sync::Semaphore;

use vela_protocol::extract::extract_paper_offline;
use vela_protocol::{fetch, jats, link, normalize, project, repo};

use crate::legacy_corpus;
use crate::legacy_extract;
use crate::legacy_link;
use crate::legacy_llm;

pub async fn cmd_compile(
    topic: &str,
    max_papers: usize,
    output: &Path,
    backend: Option<&str>,
    fulltext: bool,
) {
    let compile_start = Instant::now();

    // Local-corpus path: a directory of papers/notes/data → frontier.
    let local_source = Path::new(topic);
    if local_source.exists() {
        match legacy_corpus::compile_local_corpus(local_source, output, backend).await {
            Ok(report) => {
                println!();
                println!("  {}", "VELA · COMPILE · LEGACY · LOCAL".dimmed());
                println!("  {}", tick_row(60));
                println!("source: {}", local_source.display());
                println!("mode: local corpus");
                println!("findings: {}", report.summary.findings);
                println!("accepted sources: {}", report.summary.accepted);
                println!("skipped sources: {}", report.summary.skipped);
                println!("errors: {}", report.summary.errors);
                println!("output: {}", output.display());
                println!("report: {}", report.artifacts.compile_report);
                println!("quality table: {}", report.artifacts.quality_table);
                println!("frontier quality: {}", report.artifacts.frontier_quality);
                if !report.warnings.is_empty() {
                    println!();
                    println!("warnings:");
                    for w in &report.warnings {
                        println!("  · {w}");
                    }
                }
                println!();
                println!("time: {:.1}s", compile_start.elapsed().as_secs_f64());
            }
            Err(e) => {
                eprintln!("err · compile failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Topic-fetch path: PubMed search + LLM extraction.
    println!();
    println!("  {}", "VELA · COMPILE · LEGACY · TOPIC".dimmed());
    println!("  {}", tick_row(60));
    println!("topic: {topic}");
    println!("max papers: {max_papers}");

    let config = match legacy_llm::LlmConfig::from_env(backend) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("err · {e}");
            std::process::exit(1);
        }
    };
    let client = Client::new();

    let mut papers = match fetch::fetch_papers(&client, topic, max_papers).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("err · paper fetch failed: {e}");
            std::process::exit(1);
        }
    };
    println!("fetched: {} papers", papers.len());

    if fulltext {
        let enriched = fetch::fetch_fulltext(&client, &mut papers).await;
        println!("enriched with full text: {enriched}");
    }

    let pb = ProgressBar::new(papers.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:>12.bold} [{bar:40.cyan/blue}] {pos:>3}/{len:3} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    pb.set_prefix("extracting");
    let pb = Arc::new(pb);

    let semaphore = Arc::new(Semaphore::new(4));
    let findings_count = Arc::new(AtomicUsize::new(0));
    let errors_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for paper in papers {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let client = client.clone();
        let config = config.clone();
        let fc = findings_count.clone();
        let ec = errors_count.clone();
        let pb = pb.clone();
        handles.push(tokio::spawn(async move {
            let result = legacy_extract::extract_paper(&client, &config, &paper).await;
            drop(permit);
            match &result {
                Ok(bundles) => {
                    fc.fetch_add(bundles.len(), Ordering::Relaxed);
                }
                Err(_) => {
                    ec.fetch_add(1, Ordering::Relaxed);
                }
            }
            let f = fc.load(Ordering::Relaxed);
            let e = ec.load(Ordering::Relaxed);
            pb.set_message(format!("{f} findings, {e} errors"));
            pb.inc(1);
            (paper, result)
        }));
    }

    let mut all_bundles = Vec::new();
    let mut errors = 0usize;
    let mut accepted = 0usize;
    for handle in handles {
        let (_paper, result) = handle.await.expect("extract task panicked");
        match result {
            Ok(bundles) => {
                accepted += 1;
                all_bundles.extend(bundles);
            }
            Err(_) => errors += 1,
        }
    }
    pb.finish_with_message(format!(
        "{} findings, {errors} errors",
        all_bundles.len()
    ));

    if all_bundles.is_empty() {
        println!("no findings extracted.");
        return;
    }

    normalize::normalize_findings(&mut all_bundles);
    let det = link::deterministic_links(&mut all_bundles);
    let llm_added = legacy_link::infer_links(&client, &config, &mut all_bundles)
        .await
        .unwrap_or(0);
    println!("links: {det} deterministic + {llm_added} llm-inferred");

    let frontier = project::assemble(topic, all_bundles, accepted, errors, topic);
    if let Err(e) = repo::save_to_path(output, &frontier) {
        eprintln!("err · save failed: {e}");
        std::process::exit(1);
    }

    println!();
    println!("output: {}", output.display());
    println!("time:   {:.1}s", compile_start.elapsed().as_secs_f64());
    println!();
}

pub async fn cmd_jats(source: &str, output: &Path, backend: Option<&str>) {
    let start = Instant::now();
    let config = match legacy_llm::LlmConfig::from_env(backend) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("err · {e}");
            std::process::exit(1);
        }
    };
    let client = Client::new();

    println!("loading JATS XML…");
    let xml = if source.to_lowercase().starts_with("pmc") {
        match jats::fetch_pmc_jats(&client, source).await {
            Ok(x) => x,
            Err(e) => {
                eprintln!("err · {e}");
                std::process::exit(1);
            }
        }
    } else {
        match std::fs::read_to_string(source) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("err · failed to read {source}: {e}");
                std::process::exit(1);
            }
        }
    };
    let parsed = match jats::parse_jats(&xml) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("err · {e}");
            std::process::exit(1);
        }
    };
    let paper = jats::jats_to_paper(&parsed);
    let mut bundles = match legacy_extract::extract_paper(&client, &config, &paper).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("err · {e}");
            std::process::exit(1);
        }
    };
    if bundles.is_empty() {
        println!("no findings extracted.");
        return;
    }

    let base_prov = jats::jats_to_provenance(&parsed);
    for bundle in &mut bundles {
        bundle.provenance.doi = base_prov.doi.clone();
        bundle.provenance.pmid = base_prov.pmid.clone();
        bundle.provenance.pmc = base_prov.pmc.clone();
        bundle.provenance.title = base_prov.title.clone();
        bundle.provenance.journal = base_prov.journal.clone();
        bundle.provenance.year = base_prov.year;
        if !base_prov.authors.is_empty() {
            bundle.provenance.authors = base_prov.authors.clone();
        }
    }
    normalize::normalize_findings(&mut bundles);
    link::deterministic_links(&mut bundles);
    let description = format!("Compiled from JATS: {}", parsed.title);
    let frontier = project::assemble(&parsed.title, bundles, 1, 0, &description);
    if let Err(e) = repo::save_to_path(output, &frontier) {
        eprintln!("err · save failed: {e}");
        std::process::exit(1);
    }

    println!();
    println!("  {}", "VELA · JATS · LEGACY".dimmed());
    println!("  {}", tick_row(60));
    println!("output:   {}", output.display());
    println!("findings: {}", frontier.findings.len());
    println!("time:     {:.1}s", start.elapsed().as_secs_f64());
}

// Stub fn used by `extract_paper_offline` upstream — keep imported
// as a sanity check on the substrate API surface.
#[allow(dead_code)]
fn _ensure_offline_stays_in_substrate(p: &vela_protocol::fetch::Paper) {
    let _ = extract_paper_offline(p);
}

fn tick_row(width: usize) -> String {
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i % 4 == 0 { '·' } else { ' ' });
    }
    s
}
