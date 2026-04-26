//! # Legacy file-ingest paths (moved from `vela-protocol::ingest` in v0.27)
//!
//! Powers `vela ingest --pdf / --text / --doi` — the pre-v0.22
//! way to add findings to a frontier. v0.22+ Literature Scout
//! (`vela scout`) is the recommended path; this module exists so
//! the legacy CLI surface keeps working without forcing the
//! substrate to import LLM code.
//!
//! CSV ingest (no LLM) lives here too because `run_file_ingest`
//! is one dispatcher for every file type. The CSV-specific
//! helpers (`ingest_csv`, `parse_csv_line`) live in
//! `vela-protocol::ingest` and are re-used here.
//!
//! Doctrine: substrate stays dumb; this is the agent layer.

use std::path::Path;

use chrono::Datelike;
use colored::Colorize;
use vela_protocol::bundle::FindingBundle;
use vela_protocol::fetch::{Paper, PaperAuthor};
use vela_protocol::ingest::{
    extract_pdf_text, ingest_csv, link_new_finding, recompute_stats,
};
use vela_protocol::project::Project;
use vela_protocol::repo;

use crate::legacy_extract;
use crate::legacy_llm::LlmConfig;

/// Dispatcher for `vela ingest --pdf / --csv / --text / --doi`.
/// Loads the frontier, runs the appropriate file-type handler,
/// links + applies + saves in one shot.
#[allow(clippy::too_many_arguments)]
pub async fn run_file_ingest(
    frontier_path: &Path,
    pdf: Option<&Path>,
    csv: Option<&Path>,
    text: Option<&Path>,
    doi: Option<&str>,
    backend: Option<&str>,
    assertion_type_override: Option<&str>,
    assertion_col: Option<&str>,
    confidence_col: Option<&str>,
) {
    let mut frontier: Project =
        repo::load_from_path(frontier_path).expect("Failed to load frontier");
    let existing_count = frontier.findings.len();

    let new_findings = if let Some(csv_path) = csv {
        ingest_csv(
            csv_path,
            assertion_type_override.unwrap_or("mechanism"),
            assertion_col,
            confidence_col,
        )
        .expect("CSV ingest failed")
    } else if let Some(pdf_path) = pdf {
        let text_content = extract_pdf_text(pdf_path).expect("PDF text extraction failed");
        ingest_text_via_llm(&text_content, backend, pdf_path.to_string_lossy().as_ref())
            .await
            .expect("PDF extraction failed")
    } else if let Some(text_path) = text {
        let text_content = std::fs::read_to_string(text_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", text_path.display()));
        ingest_text_via_llm(&text_content, backend, text_path.to_string_lossy().as_ref())
            .await
            .expect("Text extraction failed")
    } else if let Some(doi_str) = doi {
        ingest_doi(doi_str, backend)
            .await
            .expect("DOI ingest failed")
    } else {
        eprintln!("err · no file source specified");
        std::process::exit(1);
    };

    let new_count = new_findings.len();
    for finding in new_findings {
        frontier.findings.push(finding);
        let new_idx = frontier.findings.len() - 1;
        link_new_finding(&mut frontier.findings, new_idx);
    }

    recompute_stats(&mut frontier);
    repo::save_to_path(frontier_path, &frontier).expect("Failed to save frontier");

    println!();
    println!("  {}", "VELA · INGEST · LEGACY · FILE".dimmed());
    println!("  {}", tick_row(60));
    println!("  ingested into: {}", frontier.project.name);
    println!("  existing findings: {existing_count}");
    println!("  new findings added: {new_count}");
    println!("  total findings: {}", frontier.findings.len());
    println!("  project saved: {}", frontier_path.display());
    println!();
}

fn tick_row(width: usize) -> String {
    let mut s = String::with_capacity(width);
    for i in 0..width {
        s.push(if i % 4 == 0 { '·' } else { ' ' });
    }
    s
}

pub async fn ingest_text_via_llm(
    text: &str,
    backend: Option<&str>,
    source_label: &str,
) -> Result<Vec<FindingBundle>, String> {
    let config = LlmConfig::from_env(backend)?;
    let client = reqwest::Client::new();

    let truncated: String = text.chars().take(8000).collect();

    let paper = Paper {
        title: source_label.to_string(),
        abstract_text: truncated,
        doi: None,
        authors: vec![],
        year: Some(chrono::Utc::now().naive_utc().year()),
        citations: 0,
        openalex_id: None,
        full_text: None,
    };

    legacy_extract::extract_paper(&client, &config, &paper).await
}

/// Ingest a DOI by fetching from OpenAlex and extracting via LLM.
pub async fn ingest_doi(
    doi: &str,
    backend: Option<&str>,
) -> Result<Vec<FindingBundle>, String> {
    let config = LlmConfig::from_env(backend)?;
    let client = reqwest::Client::new();

    let clean_doi = doi.trim_start_matches("https://doi.org/");
    let url = format!(
        "https://api.openalex.org/works/https://doi.org/{}?mailto=vela@example.com",
        urlencoding::encode(clean_doi)
    );

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("OpenAlex request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("OpenAlex parse failed: {e}"))?;

    let title = resp["title"].as_str().unwrap_or("Unknown").to_string();

    let abstract_text = if let Some(inv) = resp["abstract_inverted_index"].as_object() {
        let mut words: Vec<(usize, String)> = Vec::new();
        for (word, positions) in inv {
            if let Some(arr) = positions.as_array() {
                for pos in arr {
                    if let Some(p) = pos.as_u64() {
                        words.push((p as usize, word.clone()));
                    }
                }
            }
        }
        words.sort_by_key(|(p, _)| *p);
        words
            .iter()
            .map(|(_, w)| w.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };

    if abstract_text.is_empty() {
        return Err("No abstract available for this DOI".into());
    }

    let year = resp["publication_year"].as_i64().map(|y| y as i32);

    let authors: Vec<PaperAuthor> = resp["authorships"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a["author"]["display_name"]
                        .as_str()
                        .map(|name| PaperAuthor {
                            name: name.to_string(),
                            orcid: a["author"]["orcid"].as_str().map(String::from),
                        })
                })
                .collect()
        })
        .unwrap_or_default();

    let paper = Paper {
        title,
        abstract_text,
        doi: Some(clean_doi.to_string()),
        authors,
        year,
        citations: resp["cited_by_count"].as_u64().unwrap_or(0),
        openalex_id: resp["id"].as_str().map(String::from),
        full_text: None,
    };

    legacy_extract::extract_paper(&client, &config, &paper).await
}
