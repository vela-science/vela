//! Local corpus compiler for the paper-folder adoption path.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::Datelike;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::bundle::{
    Assertion, Conditions, Entity, Evidence, Extraction, FindingBundle, Flags, Provenance,
    compute_confidence,
};
use crate::fetch::{Paper, PaperAuthor};
use crate::{extract, jats, link, llm, normalize, project, repo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileReport {
    pub schema: String,
    pub command: String,
    pub source: ReportSource,
    pub output: ReportOutput,
    pub summary: ReportSummary,
    pub source_coverage: BTreeMap<String, usize>,
    pub extraction_modes: BTreeMap<String, usize>,
    pub sources: Vec<SourceReport>,
    pub warnings: Vec<String>,
    pub artifacts: ReportArtifacts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSource {
    pub path: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOutput {
    pub frontier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub files_seen: usize,
    pub accepted: usize,
    pub skipped: usize,
    pub errors: usize,
    pub findings: usize,
    pub links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReport {
    pub path: String,
    pub source_type: String,
    pub status: String,
    pub extraction_mode: String,
    pub findings: usize,
    pub diagnostics: SourceDiagnostics,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_chars: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportArtifacts {
    pub compile_report: String,
    pub quality_table: String,
    pub frontier_quality: String,
}

pub async fn compile_local_corpus(
    source: &Path,
    output: &Path,
    backend: Option<&str>,
) -> Result<CompileReport, String> {
    let config = match llm::LlmConfig::from_env(backend) {
        Ok(c) => Some(c),
        Err(e) if backend.is_some() => return Err(e),
        Err(_) => None,
    };
    let client = Client::new();

    let mut files = collect_sources(source)?;
    files.sort();

    let report_path = sidecar_path(output, "compile-report.json");
    let quality_path = sidecar_path(output, "quality-table.json");
    let quality_md_path = sidecar_path(output, "frontier-quality.md");

    let mut findings = Vec::new();
    let mut finding_source_hashes = BTreeMap::<String, String>::new();
    let mut finding_source_types = BTreeMap::<String, String>::new();
    let mut reports = Vec::new();
    let mut coverage: BTreeMap<String, usize> = BTreeMap::new();
    let mut modes: BTreeMap<String, usize> = BTreeMap::new();
    let mut warnings = Vec::new();

    if config.is_none() {
        warnings.push(
            "No LLM backend configured; text, JATS, PDF, and DOI sources used deterministic fallback extraction with explicit caveats.".to_string(),
        );
    }

    for path in &files {
        let source_type = classify(path);
        let local_hash = hash_file(path).ok();
        if source_type == "unsupported" {
            reports.push(SourceReport {
                path: path.display().to_string(),
                source_type: source_type.to_string(),
                status: "skipped".to_string(),
                extraction_mode: "none".to_string(),
                findings: 0,
                diagnostics: SourceDiagnostics {
                    content_hash: local_hash,
                    caveats: vec!["Unsupported file extension; source skipped.".to_string()],
                    ..SourceDiagnostics::default()
                },
                warnings: vec!["Unsupported file extension.".to_string()],
                error: None,
            });
            continue;
        }

        *coverage.entry(source_type.to_string()).or_default() += 1;
        let result = compile_source(path, source_type, config.as_ref(), &client).await;
        match result {
            Ok(SourceOutput {
                findings: mut source_findings,
                extraction_mode,
                mut diagnostics,
                warnings: source_warnings,
            }) => {
                diagnostics.content_hash = diagnostics.content_hash.or(local_hash.clone());
                *modes.entry(extraction_mode.clone()).or_default() += 1;
                let count = source_findings.len();
                if let Some(hash) = diagnostics.content_hash.clone() {
                    for finding in &source_findings {
                        finding_source_hashes.insert(finding.id.clone(), hash.clone());
                    }
                }
                for finding in &source_findings {
                    finding_source_types.insert(finding.id.clone(), source_type.to_string());
                }
                findings.append(&mut source_findings);
                reports.push(SourceReport {
                    path: path.display().to_string(),
                    source_type: source_type.to_string(),
                    status: "accepted".to_string(),
                    extraction_mode,
                    findings: count,
                    diagnostics,
                    warnings: source_warnings,
                    error: None,
                });
            }
            Err(error) => {
                reports.push(SourceReport {
                    path: path.display().to_string(),
                    source_type: source_type.to_string(),
                    status: "error".to_string(),
                    extraction_mode: "none".to_string(),
                    findings: 0,
                    diagnostics: SourceDiagnostics {
                        content_hash: local_hash,
                        caveats: vec![
                            "Source failed before findings could be compiled.".to_string(),
                        ],
                        ..SourceDiagnostics::default()
                    },
                    warnings: Vec::new(),
                    error: Some(error),
                });
            }
        }
    }

    if findings.is_empty() {
        let report = build_report(BuildReportInput {
            source,
            output,
            report_path: &report_path,
            quality_path: &quality_path,
            quality_md_path: &quality_md_path,
            reports: &reports,
            coverage,
            modes,
            warnings,
            links: 0,
        });
        write_json(&report_path, &report)?;
        return Err(format!(
            "No findings were compiled from '{}'. See {} for source diagnostics.",
            source.display(),
            report_path.display()
        ));
    }

    dedupe_findings(&mut findings);
    normalize::normalize_findings(&mut findings);
    link::deterministic_links(&mut findings);
    prune_fragile_links(&mut findings);

    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("local-corpus");
    let description = format!(
        "Compiled from local corpus '{}'. Candidate outputs require review before scientific use.",
        source.display()
    );
    let mut frontier = project::assemble(
        name,
        findings,
        reports.len(),
        error_count(&reports),
        &description,
    );
    crate::sources::attach_local_source_details(
        &mut frontier,
        &finding_source_hashes,
        &finding_source_types,
    );
    repo::save_to_path(output, &frontier)?;

    let quality = quality_table(&frontier, source, &reports);
    write_json(&quality_path, &quality)?;
    write_markdown(
        &quality_md_path,
        &quality_markdown(&frontier, source, &reports),
    )?;

    let report = build_report(BuildReportInput {
        source,
        output,
        report_path: &report_path,
        quality_path: &quality_path,
        quality_md_path: &quality_md_path,
        reports: &reports,
        coverage,
        modes,
        warnings,
        links: frontier.stats.links,
    });
    write_json(&report_path, &report)?;

    Ok(report)
}

struct SourceOutput {
    findings: Vec<FindingBundle>,
    extraction_mode: String,
    diagnostics: SourceDiagnostics,
    warnings: Vec<String>,
}

async fn compile_source(
    path: &Path,
    source_type: &str,
    config: Option<&llm::LlmConfig>,
    client: &Client,
) -> Result<SourceOutput, String> {
    match source_type {
        "csv" => {
            let findings = parse_curated_csv(path)?;
            Ok(SourceOutput {
                findings,
                extraction_mode: "curated_csv".to_string(),
                diagnostics: SourceDiagnostics {
                    text_quality: Some("curated".to_string()),
                    caveats: vec![
                        "Curated CSV rows are accepted as user-provided review state.".to_string(),
                    ],
                    ..SourceDiagnostics::default()
                },
                warnings: Vec::new(),
            })
        }
        "doi_list" => {
            let dois = parse_doi_list(path)?;
            let mut findings = Vec::new();
            let mut warnings = Vec::new();
            for doi in dois {
                match fetch_doi_paper(client, &doi).await {
                    Ok(paper) => {
                        let mut extracted =
                            extract_from_paper(client, config, &paper, &mut warnings).await?;
                        findings.append(&mut extracted);
                    }
                    Err(e) => warnings.push(format!("{doi}: {e}")),
                }
            }
            let extraction_mode = mode_for(config, "doi", &warnings);
            Ok(SourceOutput {
                findings,
                extraction_mode,
                diagnostics: SourceDiagnostics {
                    text_quality: Some("metadata".to_string()),
                    caveats: vec![
                        "DOI list sources rely on fetched metadata/abstracts where available."
                            .to_string(),
                    ],
                    ..SourceDiagnostics::default()
                },
                warnings,
            })
        }
        "jats" => {
            let xml = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read JATS XML '{}': {e}", path.display()))?;
            let parsed = jats::parse_jats(&xml)?;
            let paper = jats::jats_to_paper(&parsed);
            let mut diagnostics = text_diagnostics(&xml, "jats");
            diagnostics.detected_title = Some(paper.title.clone());
            let mut warnings = Vec::new();
            let findings = extract_from_paper(client, config, &paper, &mut warnings).await?;
            let extraction_mode = mode_for(config, "jats", &warnings);
            Ok(SourceOutput {
                findings,
                extraction_mode,
                diagnostics,
                warnings,
            })
        }
        "pdf" => {
            let (text, mut diagnostics) = extract_pdf_text(path)?;
            let paper = paper_from_text(path, &text);
            let mut warnings = Vec::new();
            warnings.extend(diagnostics.caveats.clone());
            let findings = extract_from_paper(client, config, &paper, &mut warnings).await?;
            let extraction_mode = mode_for(config, "pdf", &warnings);
            diagnostics.detected_title = Some(paper.title.clone());
            diagnostics.detected_doi = detect_doi(&text);
            Ok(SourceOutput {
                findings,
                extraction_mode,
                diagnostics,
                warnings,
            })
        }
        "text" => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read text source '{}': {e}", path.display()))?;
            let paper = paper_from_text(path, &text);
            let mut diagnostics = text_diagnostics(&text, "text");
            diagnostics.detected_title = Some(paper.title.clone());
            diagnostics.detected_doi = detect_doi(&text);
            let mut warnings = Vec::new();
            warnings.extend(diagnostics.caveats.clone());
            let findings = extract_from_paper(client, config, &paper, &mut warnings).await?;
            let extraction_mode = mode_for(config, "text", &warnings);
            Ok(SourceOutput {
                findings,
                extraction_mode,
                diagnostics,
                warnings,
            })
        }
        _ => Err(format!("Unsupported source type: {source_type}")),
    }
}

fn mode_for(config: Option<&llm::LlmConfig>, kind: &str, warnings: &[String]) -> String {
    let fallback = config.is_none()
        || warnings
            .iter()
            .any(|warning| warning.contains("deterministic fallback"));
    if fallback {
        format!("offline_{kind}")
    } else {
        format!("llm_{kind}")
    }
}

async fn extract_from_paper(
    client: &Client,
    config: Option<&llm::LlmConfig>,
    paper: &Paper,
    warnings: &mut Vec<String>,
) -> Result<Vec<FindingBundle>, String> {
    if let Some(config) = config {
        match extract::extract_paper(client, config, paper).await {
            Ok(findings) => Ok(findings),
            Err(e) => {
                warnings.push(format!(
                    "LLM extraction failed ({e}); used deterministic fallback extraction instead."
                ));
                Ok(extract::extract_paper_offline(paper))
            }
        }
    } else {
        warnings.push(
            "Used deterministic fallback extraction; review findings before scientific use."
                .to_string(),
        );
        Ok(extract::extract_paper_offline(paper))
    }
}

fn collect_sources(source: &Path) -> Result<Vec<PathBuf>, String> {
    if source.is_file() {
        return Ok(vec![source.to_path_buf()]);
    }
    if !source.is_dir() {
        return Err(format!(
            "Local corpus path does not exist: {}",
            source.display()
        ));
    }

    let mut out = Vec::new();
    collect_dir(source, &mut out)?;
    Ok(out)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory '{}': {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.starts_with('.') || name == "expected" {
            continue;
        }
        if path.is_dir() {
            collect_dir(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn classify(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "csv" | "tsv" => "csv",
        "xml" | "nxml" => "jats",
        "pdf" => "pdf",
        "md" | "markdown" | "txt" => "text",
        "doi" | "dois" => "doi_list",
        _ => "unsupported",
    }
}

fn sidecar_path(output: &Path, name: &str) -> PathBuf {
    output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Failed to read source hash input: {e}"))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}

fn parse_curated_csv(path: &Path) -> Result<Vec<FindingBundle>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CSV '{}': {e}", path.display()))?;
    let delimiter = if path.extension().and_then(|e| e.to_str()) == Some("tsv") {
        '\t'
    } else {
        ','
    };
    let mut lines = content.lines();
    let headers = split_row(lines.next().ok_or("CSV file is empty")?, delimiter);
    let index = header_index(&headers);

    let assertion_idx = required_col(&index, "assertion")?;
    let type_idx = optional_col(&index, &["type", "assertion_type"]);
    let confidence_idx = optional_col(&index, &["confidence", "score"]);
    let evidence_idx = optional_col(&index, &["evidence", "evidence_type"]);
    let entities_idx = optional_col(&index, &["entities"]);
    let source_idx = optional_col(&index, &["source", "title"]);
    let span_idx = optional_col(&index, &["span", "evidence_span"]);
    let direction_idx = optional_col(&index, &["direction"]);

    let mut findings = Vec::new();
    for (line_no, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cols = split_row(line, delimiter);
        let assertion_text = cols
            .get(assertion_idx)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Missing assertion on CSV line {}", line_no + 2))?;
        let raw_assertion_type = type_idx
            .and_then(|idx| cols.get(idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("mechanism");
        let gap_flag = raw_assertion_type == "gap";
        let assertion_type = if gap_flag {
            "theoretical"
        } else {
            raw_assertion_type
        };
        let evidence_type = evidence_idx
            .and_then(|idx| cols.get(idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("observational");
        let source_title = source_idx
            .and_then(|idx| cols.get(idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("curated CSV")
            });
        let score = confidence_idx
            .and_then(|idx| cols.get(idx))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.55)
            .clamp(0.0, 1.0);
        let span = span_idx
            .and_then(|idx| cols.get(idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(assertion_text);
        let direction = direction_idx
            .and_then(|idx| cols.get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let entities = entities_idx
            .and_then(|idx| cols.get(idx))
            .map(|raw| parse_entities(raw))
            .unwrap_or_default();

        let evidence = Evidence {
            evidence_type: evidence_type.to_string(),
            model_system: "curated local corpus row".to_string(),
            species: None,
            method: "manual CSV curation".to_string(),
            sample_size: None,
            effect_size: None,
            p_value: None,
            replicated: false,
            replication_count: None,
            evidence_spans: vec![json!({
                "text": span,
                "section": "curated_csv"
            })],
        };
        let conditions = Conditions {
            text: "Curated from a local corpus CSV row; verify against source before reuse."
                .to_string(),
            species_verified: Vec::new(),
            species_unverified: Vec::new(),
            in_vitro: false,
            in_vivo: false,
            human_data: false,
            clinical_trial: false,
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };
        let mut confidence = compute_confidence(&evidence, &conditions, false);
        confidence.score = score;
        confidence.basis = format!(
            "{}; operator-supplied local CSV prior retained for review fixture",
            confidence.basis
        );

        findings.push(FindingBundle::new(
            Assertion {
                text: assertion_text.to_string(),
                assertion_type: assertion_type.to_string(),
                entities,
                relation: None,
                direction,
            },
            evidence,
            conditions,
            confidence,
            Provenance {
                source_type: "database_record".to_string(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: source_title.to_string(),
                authors: Vec::new(),
                year: Some(chrono::Utc::now().naive_utc().year()),
                journal: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction {
                    method: "manual_curation".to_string(),
                    model: None,
                    model_version: None,
                    extracted_at: chrono::Utc::now().to_rfc3339(),
                    extractor_version: "vela/0.2.0-local-corpus".to_string(),
                },
                review: None,
                citation_count: None,
            },
            Flags {
                gap: gap_flag,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            },
        ));
    }

    if findings.is_empty() {
        Err("No findings extracted from CSV".to_string())
    } else {
        Ok(findings)
    }
}

fn parse_entities(raw: &str) -> Vec<Entity> {
    raw.split([';', '|'])
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut pieces = trimmed.splitn(2, ':');
            let name = pieces.next()?.trim();
            let entity_type = pieces.next().unwrap_or("other").trim();
            Some(Entity {
                name: normalize::entity_name(name),
                entity_type: normalize::entity_type(entity_type),
                identifiers: Default::default(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: Some("local corpus CSV".to_string()),
                resolution_confidence: 1.0,
                resolution_method: Some(crate::bundle::ResolutionMethod::Manual),
                species_context: None,
                needs_review: false,
            })
        })
        .collect()
}

fn split_row(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == delimiter && !in_quotes {
            fields.push(current.trim().trim_matches('"').to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().trim_matches('"').to_string());
    fields
}

fn header_index(headers: &[String]) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(idx, header)| (header.trim().to_ascii_lowercase(), idx))
        .collect()
}

fn required_col(index: &HashMap<String, usize>, name: &str) -> Result<usize, String> {
    index
        .get(name)
        .copied()
        .ok_or_else(|| format!("CSV column '{name}' is required"))
}

fn optional_col(index: &HashMap<String, usize>, names: &[&str]) -> Option<usize> {
    names.iter().find_map(|name| index.get(*name).copied())
}

fn parse_doi_list(path: &Path) -> Result<Vec<String>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read DOI list: {e}"))?;
    let dois: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches("https://doi.org/").to_string())
        .collect();
    if dois.is_empty() {
        Err("DOI list contains no DOI lines".to_string())
    } else {
        Ok(dois)
    }
}

async fn fetch_doi_paper(client: &Client, doi: &str) -> Result<Paper, String> {
    let url = format!(
        "https://api.openalex.org/works/https://doi.org/{}?mailto={}",
        urlencoding::encode(doi),
        urlencoding::encode(
            &std::env::var("VELA_EMAIL").unwrap_or_else(|_| "vela-cli@localhost".to_string())
        )
    );
    let resp: serde_json::Value = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("OpenAlex request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("OpenAlex parse failed: {e}"))?;
    let abstract_text = abstract_from_openalex(&resp)
        .filter(|text| text.split_whitespace().count() >= 20)
        .ok_or_else(|| "OpenAlex record has no usable abstract".to_string())?;
    let authors = resp["authorships"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item["author"]["display_name"]
                        .as_str()
                        .map(|name| PaperAuthor {
                            name: name.to_string(),
                            orcid: item["author"]["orcid"].as_str().map(String::from),
                        })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Paper {
        title: resp["title"]
            .as_str()
            .unwrap_or("Untitled DOI source")
            .to_string(),
        abstract_text,
        doi: Some(doi.to_string()),
        authors,
        year: resp["publication_year"].as_i64().map(|y| y as i32),
        citations: resp["cited_by_count"].as_u64().unwrap_or(0),
        openalex_id: resp["id"].as_str().map(String::from),
        full_text: None,
    })
}

fn abstract_from_openalex(value: &serde_json::Value) -> Option<String> {
    let inv = value["abstract_inverted_index"].as_object()?;
    let mut words: Vec<(usize, &str)> = Vec::new();
    for (word, positions) in inv {
        for pos in positions.as_array()? {
            words.push((pos.as_u64()? as usize, word.as_str()));
        }
    }
    words.sort_by_key(|(idx, _)| *idx);
    Some(
        words
            .into_iter()
            .map(|(_, word)| word)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn paper_from_text(path: &Path, text: &str) -> Paper {
    let title = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim().to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("local text source")
                .replace(['_', '-'], " ")
        });
    Paper {
        title,
        abstract_text: text.to_string(),
        doi: None,
        authors: Vec::new(),
        year: Some(chrono::Utc::now().naive_utc().year()),
        citations: 0,
        openalex_id: None,
        full_text: None,
    }
}

fn extract_pdf_text(path: &Path) -> Result<(String, SourceDiagnostics), String> {
    if let Ok(output) = std::process::Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            let mut diagnostics = text_diagnostics(&text, "pdf");
            diagnostics.page_count = count_pdf_pages(path);
            return Ok((text, diagnostics));
        }
    }

    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF file: {e}"))?;
    let page_count = count_pdf_pages_from_bytes(&bytes);
    let mut text = String::new();
    let mut current = String::new();
    for b in &bytes {
        if b.is_ascii_graphic() || *b == b' ' || *b == b'\n' || *b == b'\t' {
            current.push(*b as char);
        } else {
            if current.len() >= 20 {
                text.push_str(&current);
                text.push('\n');
            }
            current.clear();
        }
    }
    if current.len() >= 20 {
        text.push_str(&current);
    }
    if text.trim().is_empty() {
        Err(
            "Could not extract text from PDF. Install pdftotext for stronger PDF support."
                .to_string(),
        )
    } else {
        let mut diagnostics = text_diagnostics(&text, "pdf");
        diagnostics.page_count = page_count;
        diagnostics.caveats.push(
            "PDF text was recovered with a byte-level fallback; verify spans manually.".to_string(),
        );
        diagnostics.caveats.push(
            "PDF appears scanned or low-text; treat extracted findings as weak review leads."
                .to_string(),
        );
        Ok((text, diagnostics))
    }
}

fn text_diagnostics(text: &str, source_type: &str) -> SourceDiagnostics {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    let text_quality = if words < 20 {
        "low_text"
    } else if words < 80 {
        "thin_text"
    } else {
        "text_available"
    };
    let mut caveats = Vec::new();
    if words < 20 {
        caveats.push(format!(
            "{source_type} source has very little extractable text; treat findings as weak review leads."
        ));
    } else if words < 80 {
        caveats.push(format!(
            "{source_type} source has limited extractable text; verify evidence spans before use."
        ));
    }
    SourceDiagnostics {
        text_chars: Some(chars),
        word_count: Some(words),
        text_quality: Some(text_quality.to_string()),
        detected_title: None,
        detected_doi: detect_doi(text),
        caveats,
        ..SourceDiagnostics::default()
    }
}

fn count_pdf_pages(path: &Path) -> Option<usize> {
    let bytes = std::fs::read(path).ok()?;
    count_pdf_pages_from_bytes(&bytes)
}

fn count_pdf_pages_from_bytes(bytes: &[u8]) -> Option<usize> {
    let text = String::from_utf8_lossy(bytes);
    let count = text.matches("/Type /Page").count();
    if count == 0 { None } else { Some(count) }
}

fn detect_doi(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|token| {
            token
                .trim_matches(|ch: char| {
                    matches!(
                        ch,
                        '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
                    )
                })
                .trim_end_matches('.')
        })
        .find(|token| token.to_ascii_lowercase().starts_with("10."))
        .map(|token| token.trim_start_matches("doi:").to_string())
}

fn dedupe_findings(findings: &mut Vec<FindingBundle>) {
    let mut best_by_id: HashMap<String, usize> = HashMap::new();
    for (idx, finding) in findings.iter().enumerate() {
        best_by_id
            .entry(finding.id.clone())
            .and_modify(|existing| {
                if findings[idx].confidence.score > findings[*existing].confidence.score {
                    *existing = idx;
                }
            })
            .or_insert(idx);
    }
    if best_by_id.len() == findings.len() {
        return;
    }
    let mut keep: Vec<usize> = best_by_id.into_values().collect();
    keep.sort_unstable();
    *findings = keep.into_iter().map(|idx| findings[idx].clone()).collect();
}

fn prune_fragile_links(findings: &mut [FindingBundle]) {
    for finding in findings {
        if finding.confidence.score < 0.2 || finding.confidence.extraction_confidence < 0.4 {
            finding.links.clear();
        }
    }
}

fn quality_table(
    project: &project::Project,
    source: &Path,
    reports: &[SourceReport],
) -> serde_json::Value {
    let rows: Vec<_> = project
        .findings
        .iter()
        .map(|finding| {
            let unresolved_entities = finding
                .assertion
                .entities
                .iter()
                .filter(|entity| entity.needs_review || entity.resolution_confidence < 0.8)
                .count();
            let spans = finding.evidence.evidence_spans.len();
            let provenance_complete = finding.provenance.doi.is_some()
                || finding.provenance.pmid.is_some()
                || !finding.provenance.title.trim().is_empty();
            let source_report = infer_source_report(finding, reports);
            let mut caveats = Vec::new();
            if spans == 0 {
                caveats.push("missing evidence span".to_string());
            }
            if !provenance_complete {
                caveats.push("weak provenance".to_string());
            }
            if unresolved_entities > 0 {
                caveats.push("unresolved entities require review".to_string());
            }
            if finding.confidence.score < 0.35 {
                caveats.push("low confidence".to_string());
            }
            if finding.provenance.extraction.method.contains("offline")
                || finding.provenance.extraction.method.contains("deterministic")
            {
                caveats.push("deterministic fallback extraction".to_string());
            }
            if let Some(report) = source_report {
                caveats.extend(report.diagnostics.caveats.clone());
            }
            caveats.sort();
            caveats.dedup();
            let recommended_action = if spans == 0 {
                "add_or_repair_evidence_span"
            } else if !provenance_complete {
                "add_provenance"
            } else if unresolved_entities > 0 {
                "fix_entity_resolution"
            } else if finding.confidence.score < 0.35 {
                "review_low_confidence_claim"
            } else {
                "spot_check"
            };
            json!({
                "id": finding.id,
                "assertion": finding.assertion.text,
                "assertion_type": finding.assertion.assertion_type,
                "source_file": source_report.map(|report| report.path.clone()),
                "source_span_status": if spans > 0 { "present" } else { "missing" },
                "provenance_complete": provenance_complete,
                "source": {
                    "title": finding.provenance.title,
                    "doi": finding.provenance.doi,
                    "year": finding.provenance.year,
                    "extraction_method": finding.provenance.extraction.method,
                    "extractor_version": finding.provenance.extraction.extractor_version,
                },
                "confidence": {
                    "kind": finding.confidence.kind,
                    "score": finding.confidence.score,
                    "method": finding.confidence.method,
                    "components": finding.confidence.components,
                    "extraction_confidence": finding.confidence.extraction_confidence,
                    "basis": finding.confidence.basis,
                },
                "evidence": {
                    "type": finding.evidence.evidence_type,
                    "spans": finding.evidence.evidence_spans.len(),
                    "method": finding.evidence.method,
                    "model_system": finding.evidence.model_system,
                },
                "entities": finding.assertion.entities.len(),
                "entity_resolution_status": if unresolved_entities == 0 { "resolved_or_not_required" } else { "needs_review" },
                "unresolved_entities": unresolved_entities,
                "conditions": finding.conditions.text,
                "flags": {
                    "gap": finding.flags.gap,
                    "contested": finding.flags.contested,
                    "negative_space": finding.flags.negative_space,
                },
                "caveats": caveats,
                "recommended_review_action": recommended_action,
            })
        })
        .collect();
    json!({
        "schema": "vela.quality-table.v0",
        "source": source.display().to_string(),
        "frontier": {
            "name": project.project.name,
            "schema_version": project::VELA_SCHEMA_VERSION,
            "findings": project.stats.findings,
            "links": project.stats.links,
            "avg_confidence": project.stats.avg_confidence,
        },
        "source_status": reports,
        "findings": rows,
    })
}

fn infer_source_report<'a>(
    finding: &FindingBundle,
    reports: &'a [SourceReport],
) -> Option<&'a SourceReport> {
    if finding.provenance.extraction.method == "manual_curation" {
        return reports.iter().find(|report| report.source_type == "csv");
    }
    reports.iter().find(|report| {
        report
            .diagnostics
            .detected_title
            .as_ref()
            .is_some_and(|title| title == &finding.provenance.title)
    })
}

fn quality_markdown(project: &project::Project, source: &Path, reports: &[SourceReport]) -> String {
    let mut out = String::new();
    out.push_str("# Frontier quality\n\n");
    out.push_str(&format!("Source: `{}`\n\n", source.display()));
    out.push_str(&format!(
        "- Findings: {}\n- Links: {}\n- Average confidence: {:.3}\n\n",
        project.stats.findings, project.stats.links, project.stats.avg_confidence
    ));
    out.push_str("## Source accounting\n\n");
    out.push_str("| Source | Status | Type | Mode | Findings | Diagnostics |\n");
    out.push_str("|---|---:|---|---|---:|---|\n");
    for report in reports {
        let mut diagnostics = Vec::new();
        if let Some(quality) = &report.diagnostics.text_quality {
            diagnostics.push(format!("quality={quality}"));
        }
        if let Some(words) = report.diagnostics.word_count {
            diagnostics.push(format!("words={words}"));
        }
        if let Some(pages) = report.diagnostics.page_count {
            diagnostics.push(format!("pages={pages}"));
        }
        diagnostics.extend(report.warnings.clone());
        if let Some(error) = &report.error {
            diagnostics.push(format!("error={error}"));
        }
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            report.path,
            report.status,
            report.source_type,
            report.extraction_mode,
            report.findings,
            escape_markdown_cell(&diagnostics.join("; "))
        ));
    }
    out.push_str("\n## Review queue\n\n");
    out.push_str("| Finding | Confidence | Span | Provenance | Action |\n");
    out.push_str("|---|---:|---|---|---|\n");
    for finding in project.findings.iter().take(50) {
        let spans = finding.evidence.evidence_spans.len();
        let provenance = if finding.provenance.doi.is_some()
            || finding.provenance.pmid.is_some()
            || !finding.provenance.title.trim().is_empty()
        {
            "present"
        } else {
            "weak"
        };
        let action = if spans == 0 {
            "add span"
        } else if provenance == "weak" {
            "add provenance"
        } else if finding.confidence.score < 0.35 {
            "review confidence"
        } else {
            "spot check"
        };
        out.push_str(&format!(
            "| `{}` | {:.3} | {} | {} | {} |\n",
            finding.id,
            finding.confidence.score,
            if spans > 0 { "present" } else { "missing" },
            provenance,
            action
        ));
    }
    out.push_str(
        "\nCandidate gaps, bridges, tensions, prior-art checks, and observer rerankings are review surfaces, not conclusions.\n",
    );
    out
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

struct BuildReportInput<'a> {
    source: &'a Path,
    output: &'a Path,
    report_path: &'a Path,
    quality_path: &'a Path,
    quality_md_path: &'a Path,
    reports: &'a [SourceReport],
    coverage: BTreeMap<String, usize>,
    modes: BTreeMap<String, usize>,
    warnings: Vec<String>,
    links: usize,
}

fn build_report(input: BuildReportInput<'_>) -> CompileReport {
    let accepted = input
        .reports
        .iter()
        .filter(|r| r.status == "accepted")
        .count();
    let skipped = input
        .reports
        .iter()
        .filter(|r| r.status == "skipped")
        .count();
    let errors = error_count(input.reports);
    let findings = input.reports.iter().map(|r| r.findings).sum();
    CompileReport {
        schema: "vela.compile-report.v0".to_string(),
        command: "compile".to_string(),
        source: ReportSource {
            path: input.source.display().to_string(),
            mode: "local_corpus".to_string(),
        },
        output: ReportOutput {
            frontier: input.output.display().to_string(),
        },
        summary: ReportSummary {
            files_seen: input.reports.len(),
            accepted,
            skipped,
            errors,
            findings,
            links: input.links,
        },
        source_coverage: input.coverage,
        extraction_modes: input.modes,
        sources: input.reports.to_vec(),
        warnings: input.warnings,
        artifacts: ReportArtifacts {
            compile_report: input.report_path.display().to_string(),
            quality_table: input.quality_path.display().to_string(),
            frontier_quality: input.quality_md_path.display().to_string(),
        },
    }
}

fn error_count(reports: &[SourceReport]) -> usize {
    reports.iter().filter(|r| r.status == "error").count()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {e}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize JSON '{}': {e}", path.display()))?;
    std::fs::write(path, format!("{data}\n"))
        .map_err(|e| format!("Failed to write '{}': {e}", path.display()))
}

fn write_markdown(path: &Path, value: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {e}", parent.display()))?;
    }
    std::fs::write(path, value).map_err(|e| format!("Failed to write '{}': {e}", path.display()))
}
