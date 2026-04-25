//! Stage 1: FETCH — retrieve papers from OpenAlex, with optional PMC full-text enrichment.

use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENALEX_BASE: &str = "https://api.openalex.org";

const NCBI_IDCONV: &str = "https://www.ncbi.nlm.nih.gov/pmc/utils/idconv/v1.0/";
const NCBI_EFETCH: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";
const NCBI_TOOL: &str = "vela";

fn api_email() -> String {
    std::env::var("VELA_EMAIL").unwrap_or_else(|_| "vela-cli@localhost".into())
}
/// Delay between NCBI requests (3 req/sec limit without API key).
const NCBI_DELAY_MS: u64 = 350;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperFullText {
    pub abstract_text: String,
    pub results: String,
    pub discussion: String,
    pub methods: String,
    pub figure_captions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub title: String,
    pub abstract_text: String,
    pub doi: Option<String>,
    pub authors: Vec<PaperAuthor>,
    pub year: Option<i32>,
    pub citations: u64,
    pub openalex_id: Option<String>,
    #[serde(default)]
    pub full_text: Option<PaperFullText>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperAuthor {
    pub name: String,
    pub orcid: Option<String>,
}

pub async fn fetch_papers(
    client: &Client,
    topic: &str,
    max_papers: usize,
) -> Result<Vec<Paper>, String> {
    let mut papers = Vec::new();
    let mut page = 1u32;
    let per_page = max_papers.min(50);

    while papers.len() < max_papers {
        let label = format!("OpenAlex page {}", page);
        let json: serde_json::Value = crate::retry::retry_with_backoff(&label, 3, || {
            let client = client.clone();
            let topic = topic.to_string();
            let per_page_s = per_page.to_string();
            let page_s = page.to_string();
            let email = api_email();
            async move {
                let resp = client
                    .get(format!("{OPENALEX_BASE}/works"))
                    .query(&[
                        ("search", topic.as_str()),
                        ("per_page", per_page_s.as_str()),
                        ("page", page_s.as_str()),
                        ("sort", "cited_by_count:desc"),
                        ("filter", "type:article"),
                        ("select", "id,doi,title,authorships,publication_year,cited_by_count,abstract_inverted_index"),
                        ("mailto", email.as_str()),
                    ])
                    .send()
                    .await
                    .map_err(|e| format!("OpenAlex error: {e}"))?;

                if !resp.status().is_success() {
                    return Err(format!("OpenAlex {}", resp.status()));
                }

                resp.json::<serde_json::Value>().await.map_err(|e| format!("OpenAlex parse: {e}"))
            }
        }).await?;
        let results = json["results"].as_array().ok_or("No results")?;

        if results.is_empty() {
            break;
        }

        for work in results {
            // Reconstruct abstract from inverted index
            let abstract_text = if let Some(inv) = work["abstract_inverted_index"].as_object() {
                let mut positions: Vec<(usize, &str)> = Vec::new();
                for (word, idxs) in inv {
                    if let Some(arr) = idxs.as_array() {
                        for idx in arr {
                            if let Some(i) = idx.as_u64() {
                                positions.push((i as usize, word.as_str()));
                            }
                        }
                    }
                }
                positions.sort_by_key(|(i, _)| *i);
                positions
                    .iter()
                    .map(|(_, w)| *w)
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                continue;
            };

            if abstract_text.len() < 100 {
                continue;
            }

            let doi = work["doi"]
                .as_str()
                .map(|d| d.replace("https://doi.org/", ""));

            let authors: Vec<PaperAuthor> = work["authorships"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .take(10)
                .filter_map(|a| {
                    let name = a["author"]["display_name"].as_str()?.to_string();
                    let orcid = a["author"]["orcid"]
                        .as_str()
                        .map(|o| o.replace("https://orcid.org/", ""));
                    Some(PaperAuthor { name, orcid })
                })
                .collect();

            papers.push(Paper {
                title: work["title"].as_str().unwrap_or("").to_string(),
                abstract_text,
                doi,
                authors,
                year: work["publication_year"].as_i64().map(|y| y as i32),
                citations: work["cited_by_count"].as_u64().unwrap_or(0),
                openalex_id: work["id"].as_str().map(|s| s.to_string()),
                full_text: None,
            });

            if papers.len() >= max_papers {
                break;
            }
        }

        page += 1;
    }

    Ok(papers)
}

// ---------------------------------------------------------------------------
// PMC full-text enrichment
// ---------------------------------------------------------------------------

/// Try to fetch PMC full text for each paper that has a DOI.
/// Papers without PMC access keep `full_text = None` (abstract-only fallback).
/// Runs up to 2 concurrent fetches (NCBI allows ~3 req/sec without API key).
/// Returns the number of papers successfully enriched.
pub async fn fetch_fulltext(client: &Client, papers: &mut [Paper]) -> usize {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let semaphore = Arc::new(Semaphore::new(2));
    let mut handles = Vec::new();

    for (idx, paper) in papers.iter().enumerate() {
        let doi = match &paper.doi {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };

        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let client = client.clone();
        let abstract_text = paper.abstract_text.clone();

        handles.push(tokio::spawn(async move {
            // 1. Convert DOI -> PMCID
            tokio::time::sleep(std::time::Duration::from_millis(NCBI_DELAY_MS)).await;
            let pmcid = match doi_to_pmcid(&client, &doi).await {
                Some(id) => id,
                None => {
                    drop(permit);
                    return (idx, None);
                }
            };

            // 2. Fetch JATS XML
            tokio::time::sleep(std::time::Duration::from_millis(NCBI_DELAY_MS)).await;
            let xml = match fetch_pmc_xml(&client, &pmcid).await {
                Some(x) => x,
                None => {
                    drop(permit);
                    return (idx, None);
                }
            };

            drop(permit);

            // 3. Parse sections
            let ft = parse_jats(&xml, &abstract_text);
            (idx, ft)
        }));
    }

    let mut enriched = 0usize;
    for handle in handles {
        let (idx, ft) = handle.await.expect("fulltext fetch task panicked");
        if let Some(fulltext) = ft {
            papers[idx].full_text = Some(fulltext);
            enriched += 1;
        }
    }

    enriched
}

/// Use NCBI ID Converter to get PMCID from a DOI.
/// Retries up to 3 times with exponential backoff on transient failures.
async fn doi_to_pmcid(client: &Client, doi: &str) -> Option<String> {
    let result = crate::retry::retry_with_backoff("NCBI ID conversion", 3, || {
        let client = client.clone();
        let doi = doi.to_string();
        let email = api_email();
        async move {
            let resp = client
                .get(NCBI_IDCONV)
                .query(&[
                    ("ids", doi.as_str()),
                    ("format", "json"),
                    ("tool", NCBI_TOOL),
                    ("email", email.as_str()),
                ])
                .send()
                .await
                .map_err(|e| format!("NCBI ID conv error: {e}"))?;

            if !resp.status().is_success() {
                return Err(format!("NCBI ID conv {}", resp.status()));
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("NCBI ID conv parse: {e}"))?;
            Ok(json)
        }
    })
    .await
    .ok()?;

    let records = result["records"].as_array()?;
    let pmcid = records.first()?["pmcid"].as_str()?;
    if pmcid.is_empty() {
        return None;
    }
    Some(pmcid.to_string())
}

/// Fetch full-text JATS XML from PMC.
/// Retries up to 3 times with exponential backoff on transient failures.
async fn fetch_pmc_xml(client: &Client, pmcid: &str) -> Option<String> {
    crate::retry::retry_with_backoff("PMC XML fetch", 3, || {
        let client = client.clone();
        let pmcid = pmcid.to_string();
        let email = api_email();
        async move {
            let resp = client
                .get(NCBI_EFETCH)
                .query(&[
                    ("db", "pmc"),
                    ("id", pmcid.as_str()),
                    ("rettype", "xml"),
                    ("tool", NCBI_TOOL),
                    ("email", email.as_str()),
                ])
                .send()
                .await
                .map_err(|e| format!("PMC fetch error: {e}"))?;

            if !resp.status().is_success() {
                return Err(format!("PMC fetch {}", resp.status()));
            }

            resp.text()
                .await
                .map_err(|e| format!("PMC fetch body: {e}"))
        }
    })
    .await
    .ok()
}

/// Strip XML tags from a string, collapsing whitespace.
fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").expect("invalid regex for HTML tag stripping");
    let stripped = re.replace_all(s, " ");
    // Collapse whitespace
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract a named section from JATS XML.
/// Looks for `<sec sec-type="TYPE">` or `<sec>` containing `<title>TITLE</title>`.
fn extract_section(xml: &str, sec_type: &str, title_pattern: &str) -> String {
    // Strategy 1: sec-type attribute
    let attr_pattern = format!(r#"<sec[^>]*sec-type\s*=\s*"{sec_type}"[^>]*>([\s\S]*?)</sec>"#);
    if let Ok(re) = Regex::new(&attr_pattern)
        && let Some(cap) = re.captures(xml)
    {
        return strip_tags(&cap[1]);
    }

    // Strategy 2: title element inside sec
    let title_pat = format!(
        r#"<sec[^>]*>\s*<title[^>]*>\s*{}\s*</title>([\s\S]*?)</sec>"#,
        title_pattern
    );
    if let Ok(re) = Regex::new(&title_pat)
        && let Some(cap) = re.captures(xml)
    {
        return strip_tags(&cap[1]);
    }

    String::new()
}

/// Extract figure and table captions from JATS XML.
fn extract_captions(xml: &str) -> Vec<String> {
    let mut captions = Vec::new();
    let re =
        Regex::new(r"<caption>([\s\S]*?)</caption>").expect("invalid regex for caption extraction");
    for cap in re.captures_iter(xml) {
        let text = strip_tags(&cap[1]);
        if text.len() > 20 {
            captions.push(text);
        }
    }
    captions
}

/// Parse JATS XML into structured sections.
fn parse_jats(xml: &str, fallback_abstract: &str) -> Option<PaperFullText> {
    // At least one of results or discussion should be present for this to be useful.
    let results = extract_section(xml, "results", r"(?i)results?");
    let discussion = extract_section(xml, "discussion", r"(?i)discussions?");

    if results.is_empty() && discussion.is_empty() {
        return None;
    }

    let methods = extract_section(
        xml,
        "methods",
        r"(?i)(methods?|materials?\s+and\s+methods?)",
    );

    // Try to get abstract from XML; fall back to the OpenAlex-reconstructed one.
    let abstract_text = {
        let re = Regex::new(r"<abstract[^>]*>([\s\S]*?)</abstract>")
            .expect("invalid regex for abstract extraction");
        re.captures(xml)
            .map(|c| strip_tags(&c[1]))
            .filter(|t| t.len() > 50)
            .unwrap_or_else(|| fallback_abstract.to_string())
    };

    let figure_captions = extract_captions(xml);

    Some(PaperFullText {
        abstract_text,
        results,
        discussion,
        methods,
        figure_captions,
    })
}
