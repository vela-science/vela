//! Ingest a new finding into an existing frontier.
//!
//! Supports two paths:
//! 1. Manual ingest: --assertion, --type, --evidence, etc.
//! 2. File ingest: --pdf, --csv, --text, --doi (extracts findings via LLM or CSV parsing)

use std::path::Path;

use chrono::Datelike;

use colored::Colorize;

use crate::bundle::{
    Assertion, Conditions, Confidence, Entity, Evidence, Extraction, FindingBundle, Flags,
    Provenance,
};
use crate::cli_style as style;
use crate::extract;
use crate::fetch::{Paper, PaperAuthor};
use crate::llm::LlmConfig;
use crate::project::Project;
use crate::repo;

/// Parsed arguments for the ingest command.
pub struct IngestArgs {
    pub assertion_text: String,
    pub assertion_type: String,
    pub evidence_type: String,
    pub species: Option<String>,
    pub method: String,
    pub confidence_score: f64,
    pub entities: Vec<(String, String)>, // (name, type)
    pub direction: Option<String>,
    pub source: String,
}

/// Run the ingest pipeline: build finding, link, update stats, save.
pub fn run(frontier_path: &Path, args: IngestArgs) {
    let mut frontier: Project =
        repo::load_from_path(frontier_path).expect("Failed to load frontier");

    let existing_count = frontier.findings.len();

    // Build the new finding.
    let entities: Vec<Entity> = args
        .entities
        .iter()
        .map(|(name, etype)| Entity {
            name: name.clone(),
            entity_type: etype.clone(),
            identifiers: serde_json::Map::new(),
            canonical_id: None,
            candidates: Vec::new(),
            aliases: Vec::new(),
            resolution_provenance: None,
            resolution_confidence: 1.0,
            resolution_method: None,
            species_context: None,
            needs_review: false,
        })
        .collect();

    let assertion = Assertion {
        text: args.assertion_text,
        assertion_type: args.assertion_type,
        entities,
        relation: None,
        direction: args.direction,
    };

    let evidence = Evidence {
        evidence_type: args.evidence_type,
        model_system: String::new(),
        species: args.species.clone(),
        method: args.method,
        sample_size: None,
        effect_size: None,
        p_value: None,
        replicated: false,
        replication_count: None,
        evidence_spans: Vec::new(),
    };

    let conditions = Conditions {
        text: args
            .species
            .as_deref()
            .map(|s| format!("In {s}"))
            .unwrap_or_default(),
        species_verified: args.species.iter().cloned().collect(),
        species_unverified: Vec::new(),
        in_vitro: false,
        in_vivo: args.species.is_some(),
        human_data: false,
        clinical_trial: false,
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: None,
    };

    let confidence = Confidence::legacy(
        args.confidence_score,
        "operator-supplied manual prior (manual_curation)",
        1.0,
    );

    let provenance = Provenance {
        source_type: "expert_assertion".into(),
        doi: None,
        pmid: None,
        pmc: None,
        openalex_id: None,
        title: args.source.clone(),
        authors: Vec::new(),
        year: Some(chrono::Utc::now().naive_utc().year()),
        journal: None,
        license: None,
        publisher: None,
        funders: vec![],
        extraction: Extraction {
            method: "manual_curation".into(),
            model: None,
            model_version: None,
            extracted_at: chrono::Utc::now().to_rfc3339(),
            extractor_version: "vela/0.2.0".into(),
        },
        review: None,
        citation_count: None,
    };

    let flags = Flags {
        gap: false,
        negative_space: false,
        contested: false,
        retracted: false,
        declining: false,
        gravity_well: false,
        review_state: None,
    };

    let finding = FindingBundle::new(
        assertion, evidence, conditions, confidence, provenance, flags,
    );
    let finding_id = finding.id.clone();

    frontier.findings.push(finding);

    // Run deterministic linking across all findings (including the new one).
    // Clear existing links on the new finding first (it has none), but we need
    // to re-run only for the new finding against existing ones. The full O(n^2)
    // pass is fine for demonstration — deterministic_links is idempotent per pair
    // and only adds links where none exist yet, but it will also try all old pairs.
    // Instead, we run a targeted pass: just the new finding against all others.
    let new_idx = frontier.findings.len() - 1;
    let _new_links = link_new_finding(&mut frontier.findings, new_idx);

    // Tally link types.
    let mut supports = 0usize;
    let mut extends = 0usize;
    let mut contradicts = 0usize;
    let mut depends = 0usize;
    let mut supersedes = 0usize;
    let mut other = 0usize;

    for l in &frontier.findings[new_idx].links {
        match l.link_type.as_str() {
            "supports" => supports += 1,
            "extends" => extends += 1,
            "contradicts" => contradicts += 1,
            "depends" => depends += 1,
            "supersedes" => supersedes += 1,
            _ => other += 1,
        }
    }

    let total_new_links = supports + extends + contradicts + depends + supersedes + other;

    // Update frontier stats.
    recompute_stats(&mut frontier);

    // Save.
    repo::save_to_path(frontier_path, &frontier).expect("Failed to save frontier");

    // Report.
    println!();
    println!("  {}", "VELA · INGEST · V0.7.0".dimmed());
    println!("  {}", style::tick_row(60));
    println!("  ingested into: {}", frontier.project.name);
    println!("  existing findings: {existing_count}");

    if total_new_links > 0 {
        let mut parts: Vec<String> = Vec::new();
        if supports > 0 {
            parts.push(format!("{supports} supports"));
        }
        if extends > 0 {
            parts.push(format!("{extends} extends"));
        }
        if contradicts > 0 {
            parts.push(format!("{contradicts} contradicts"));
        }
        if depends > 0 {
            parts.push(format!("{depends} depends"));
        }
        if supersedes > 0 {
            parts.push(format!("{supersedes} supersedes"));
        }
        if other > 0 {
            parts.push(format!("{other} other"));
        }
        println!(
            "  new finding links to {} existing findings ({})",
            total_new_links,
            parts.join(", ")
        );
    } else {
        println!("  no entity overlap with existing findings");
    }

    println!("  new finding id: {finding_id}");
    println!("  project saved: {}", frontier_path.display());
    println!();
}

/// Link a single new finding (at `new_idx`) against all earlier findings.
/// Returns the number of links created. This avoids re-running the full O(n^2)
/// pass and duplicating existing links between old findings.
fn link_new_finding(findings: &mut [FindingBundle], new_idx: usize) -> usize {
    use std::collections::HashSet;

    let n = findings.len();
    if n < 2 || new_idx >= n {
        return 0;
    }

    // Entity set for the new finding.
    let new_entities: HashSet<String> = {
        let f = &findings[new_idx];
        let mut names = HashSet::new();
        for e in &f.assertion.entities {
            names.insert(e.name.to_lowercase());
            for alias in &e.aliases {
                names.insert(alias.to_lowercase());
            }
        }
        names
    };

    let new_doi = findings[new_idx]
        .provenance
        .doi
        .as_ref()
        .map(|d| d.to_lowercase());

    struct PendingLink {
        from_idx: usize,
        to_id: String,
        link_type: String,
        note: String,
    }

    let mut pending: Vec<PendingLink> = Vec::new();

    for j in 0..new_idx {
        let other_entities: HashSet<String> = {
            let f = &findings[j];
            let mut names = HashSet::new();
            for e in &f.assertion.entities {
                names.insert(e.name.to_lowercase());
                for alias in &e.aliases {
                    names.insert(alias.to_lowercase());
                }
            }
            names
        };

        let shared: HashSet<&String> = new_entities.intersection(&other_entities).collect();
        if shared.is_empty() {
            continue;
        }

        // Skip intra-paper.
        let other_doi = findings[j]
            .provenance
            .doi
            .as_ref()
            .map(|d| d.to_lowercase());
        if let (Some(a), Some(b)) = (&new_doi, &other_doi)
            && a == b
        {
            continue;
        }

        let shared_names: Vec<String> = shared.iter().map(|s| s.to_string()).collect();
        let overlap_count = shared_names.len();
        let overlap_label = shared_names.join(", ");
        let strong = overlap_count >= 2;

        let dir_new = findings[new_idx].assertion.direction.as_deref();
        let dir_j = findings[j].assertion.direction.as_deref();

        let (link_type, note) = if is_opposite(dir_new, dir_j) {
            (
                "contradicts",
                format!(
                    "Opposite directions on shared entit{}: {}{}",
                    if overlap_count == 1 { "y" } else { "ies" },
                    overlap_label,
                    if strong { " (strong overlap)" } else { "" }
                ),
            )
        } else {
            (
                "extends",
                format!(
                    "Cross-paper shared entit{}: {}{}",
                    if overlap_count == 1 { "y" } else { "ies" },
                    overlap_label,
                    if strong { " (strong overlap)" } else { "" }
                ),
            )
        };

        pending.push(PendingLink {
            from_idx: new_idx,
            to_id: findings[j].id.clone(),
            link_type: link_type.to_string(),
            note,
        });
    }

    let count = pending.len();
    for pl in pending {
        findings[pl.from_idx].add_link_with_source(
            &pl.to_id,
            &pl.link_type,
            &pl.note,
            "entity_overlap",
        );
    }

    count
}

fn is_opposite(a: Option<&str>, b: Option<&str>) -> bool {
    matches!(
        (a, b),
        (Some("positive"), Some("negative")) | (Some("negative"), Some("positive"))
    )
}

// ── File ingest ─────────────────────────────────────────────────────

/// Run file-based ingest: PDF, CSV, text, or DOI.
#[allow(dead_code, clippy::too_many_arguments)]
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
        eprintln!("{} no file source specified", style::err_prefix());
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
    println!("  {}", "VELA · INGEST · V0.2.0 · FILE".dimmed());
    println!("  {}", style::tick_row(60));
    println!("  ingested into: {}", frontier.project.name);
    println!("  existing findings: {existing_count}");
    println!("  new findings added: {new_count}");
    println!("  total findings: {}", frontier.findings.len());
    println!("  project saved: {}", frontier_path.display());
    println!();
}

/// Extract text from a PDF. Tries `pdftotext` first, falls back to reading raw text.
#[allow(dead_code)]
fn extract_pdf_text(path: &Path) -> Result<String, String> {
    // Try pdftotext (poppler-utils).
    if let Ok(output) = std::process::Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    // Fallback: read raw bytes and extract printable text runs.
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF file: {e}"))?;

    // Extract ASCII text runs of length >= 20 (crude but works for most PDFs).
    let mut text = String::new();
    let mut current_run = String::new();
    for &b in &bytes {
        if b.is_ascii_graphic() || b == b' ' || b == b'\n' || b == b'\t' {
            current_run.push(b as char);
        } else {
            if current_run.len() >= 20 {
                text.push_str(&current_run);
                text.push('\n');
            }
            current_run.clear();
        }
    }
    if current_run.len() >= 20 {
        text.push_str(&current_run);
    }

    if text.trim().is_empty() {
        return Err(
            "Could not extract text from PDF. Install pdftotext for better results.".into(),
        );
    }
    Ok(text)
}

/// Feed text through LLM extraction pipeline to produce findings.
#[allow(dead_code)]
async fn ingest_text_via_llm(
    text: &str,
    backend: Option<&str>,
    source_label: &str,
) -> Result<Vec<FindingBundle>, String> {
    let config = LlmConfig::from_env(backend)?;
    let client = reqwest::Client::new();

    // Truncate to reasonable size for LLM.
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

    extract::extract_paper(&client, &config, &paper).await
}

/// Ingest a DOI by fetching from OpenAlex and extracting via LLM.
#[allow(dead_code)]
async fn ingest_doi(doi: &str, backend: Option<&str>) -> Result<Vec<FindingBundle>, String> {
    let config = LlmConfig::from_env(backend)?;
    let client = reqwest::Client::new();

    // Fetch paper metadata from OpenAlex.
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

    // Reconstruct abstract from inverted index.
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

    extract::extract_paper(&client, &config, &paper).await
}

/// Ingest from a CSV file. Each row becomes one finding.
#[allow(dead_code)]
fn ingest_csv(
    path: &Path,
    default_type: &str,
    assertion_col: Option<&str>,
    confidence_col: Option<&str>,
) -> Result<Vec<FindingBundle>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read CSV: {e}"))?;

    let mut lines = content.lines();
    let header_line = lines.next().ok_or("CSV file is empty")?;
    let headers: Vec<&str> = header_line
        .split(',')
        .map(|h| h.trim().trim_matches('"'))
        .collect();

    let assertion_key = assertion_col.unwrap_or("assertion");
    let confidence_key = confidence_col.unwrap_or("confidence");

    let assertion_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(assertion_key))
        .ok_or_else(|| {
            format!(
                "Column '{}' not found in CSV headers: {:?}",
                assertion_key, headers
            )
        })?;

    let confidence_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(confidence_key));

    let type_idx = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case("type") || h.eq_ignore_ascii_case("assertion_type"));

    let mut findings = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = parse_csv_line(line);
        if cols.len() <= assertion_idx {
            continue;
        }

        let assertion_text = cols[assertion_idx].to_string();
        if assertion_text.is_empty() {
            continue;
        }

        let score = confidence_idx
            .and_then(|i| cols.get(i))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.5);

        let atype = type_idx
            .and_then(|i| cols.get(i))
            .map(|v| v.to_string())
            .unwrap_or_else(|| default_type.to_string());

        let assertion = Assertion {
            text: assertion_text,
            assertion_type: atype,
            entities: vec![],
            relation: None,
            direction: None,
        };

        let evidence = Evidence {
            evidence_type: "observational".into(),
            model_system: String::new(),
            species: None,
            method: String::new(),
            sample_size: None,
            effect_size: None,
            p_value: None,
            replicated: false,
            replication_count: None,
            evidence_spans: vec![],
        };

        let conditions = Conditions {
            text: String::new(),
            species_verified: vec![],
            species_unverified: vec![],
            in_vitro: false,
            in_vivo: false,
            human_data: false,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };

        let confidence = Confidence::legacy(
            score.clamp(0.0, 1.0),
            "operator-supplied import prior (database_import)",
            1.0,
        );

        let provenance = Provenance {
            source_type: "database_record".into(),
            doi: None,
            pmid: None,
            pmc: None,
            openalex_id: None,
            title: path.display().to_string(),
            authors: vec![],
            year: Some(chrono::Utc::now().naive_utc().year()),
            journal: None,
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Extraction {
                method: "database_import".into(),
                model: None,
                model_version: None,
                extracted_at: chrono::Utc::now().to_rfc3339(),
                extractor_version: "vela/0.2.0".into(),
            },
            review: None,
            citation_count: None,
        };

        let flags = Flags {
            gap: false,
            negative_space: false,
            contested: false,
            retracted: false,
            declining: false,
            gravity_well: false,
            review_state: None,
        };

        findings.push(FindingBundle::new(
            assertion, evidence, conditions, confidence, provenance, flags,
        ));
    }

    if findings.is_empty() {
        return Err("No findings extracted from CSV".into());
    }

    Ok(findings)
}

/// Simple CSV line parser that handles quoted fields.
#[allow(dead_code)]
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let bytes = line.as_bytes();

    for i in 0..bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        } else if bytes[i] == b',' && !in_quotes {
            let field = line[start..i].trim().trim_matches('"');
            fields.push(field);
            start = i + 1;
        }
    }
    // Last field.
    let field = line[start..].trim().trim_matches('"');
    fields.push(field);
    fields
}

/// Recompute frontier stats from findings.
fn recompute_stats(frontier: &mut Project) {
    use std::collections::HashMap;

    let findings = &frontier.findings;

    let total_links: usize = findings.iter().map(|b| b.links.len()).sum();

    let mut link_types: HashMap<String, usize> = HashMap::new();
    for b in findings {
        for l in &b.links {
            *link_types.entry(l.link_type.clone()).or_default() += 1;
        }
    }

    let mut categories: HashMap<String, usize> = HashMap::new();
    for b in findings {
        *categories
            .entry(b.assertion.assertion_type.clone())
            .or_default() += 1;
    }

    let replicated = findings.iter().filter(|b| b.evidence.replicated).count();
    let avg_conf = if findings.is_empty() {
        0.0
    } else {
        (findings.iter().map(|b| b.confidence.score).sum::<f64>() / findings.len() as f64 * 1000.0)
            .round()
            / 1000.0
    };

    let s = &mut frontier.stats;
    s.findings = findings.len();
    s.links = total_links;
    s.replicated = replicated;
    s.unreplicated = findings.len() - replicated;
    s.avg_confidence = avg_conf;
    s.gaps = findings.iter().filter(|b| b.flags.gap).count();
    s.negative_space = findings.iter().filter(|b| b.flags.negative_space).count();
    s.contested = findings.iter().filter(|b| b.flags.contested).count();
    s.categories = categories;
    s.link_types = link_types;
    s.human_reviewed = findings
        .iter()
        .filter(|b| b.provenance.review.as_ref().is_some_and(|r| r.reviewed))
        .count();
    s.confidence_distribution.high_gt_80 =
        findings.iter().filter(|b| b.confidence.score > 0.8).count();
    s.confidence_distribution.medium_60_80 = findings
        .iter()
        .filter(|b| (0.6..=0.8).contains(&b.confidence.score))
        .count();
    s.confidence_distribution.low_lt_60 =
        findings.iter().filter(|b| b.confidence.score < 0.6).count();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn csv_ingest_basic() {
        let tmp = TempDir::new().unwrap();
        let csv_path = tmp.path().join("data.csv");
        std::fs::write(
            &csv_path,
            "assertion,confidence,type\nNLRP3 activates IL-1B,0.85,mechanism\nTau aggregates in AD,0.7,biomarker\n",
        ).unwrap();

        let findings = ingest_csv(&csv_path, "mechanism", None, None).unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].assertion.text, "NLRP3 activates IL-1B");
        assert!((findings[0].confidence.score - 0.85).abs() < 0.001);
        assert_eq!(findings[0].assertion.assertion_type, "mechanism");
        assert_eq!(findings[1].assertion.assertion_type, "biomarker");
    }

    #[test]
    fn csv_ingest_custom_columns() {
        let tmp = TempDir::new().unwrap();
        let csv_path = tmp.path().join("data.csv");
        std::fs::write(
            &csv_path,
            "claim,score,notes\nDrug X reduces inflammation,0.9,promising\nCompound Y is toxic,0.3,needs review\n",
        ).unwrap();

        let findings = ingest_csv(&csv_path, "therapeutic", Some("claim"), Some("score")).unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].assertion.text, "Drug X reduces inflammation");
        assert!((findings[0].confidence.score - 0.9).abs() < 0.001);
        assert_eq!(findings[0].assertion.assertion_type, "therapeutic");
    }

    #[test]
    fn csv_ingest_missing_column_errors() {
        let tmp = TempDir::new().unwrap();
        let csv_path = tmp.path().join("data.csv");
        std::fs::write(&csv_path, "name,value\nfoo,bar\n").unwrap();

        let result = ingest_csv(&csv_path, "mechanism", None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn csv_ingest_empty_rows_skipped() {
        let tmp = TempDir::new().unwrap();
        let csv_path = tmp.path().join("data.csv");
        std::fs::write(
            &csv_path,
            "assertion,confidence\nReal finding,0.8\n\n  \n,0.5\n",
        )
        .unwrap();

        let findings = ingest_csv(&csv_path, "mechanism", None, None).unwrap();
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn parse_csv_line_basic() {
        let fields = parse_csv_line("a,b,c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_csv_line_quoted() {
        let fields = parse_csv_line("\"hello, world\",b,c");
        assert_eq!(fields, vec!["hello, world", "b", "c"]);
    }

    #[test]
    fn pdf_text_extraction_missing_file() {
        let result = extract_pdf_text(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
    }
}
