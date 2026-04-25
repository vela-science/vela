//! Crossref metadata enrichment — journal, publisher, license, funder, citation data.

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossrefMeta {
    pub journal: Option<String>,
    pub publisher: Option<String>,
    pub license: Option<String>,
    pub funders: Vec<String>,
    pub reference_count: u32,
    pub is_referenced_by_count: u32,
}

pub async fn enrich(client: &Client, doi: &str) -> Result<CrossrefMeta, String> {
    let url = format!("https://api.crossref.org/works/{}", doi);
    let resp = client
        .get(&url)
        .header("User-Agent", "Vela/0.1.0 (mailto:will@vela.science)")
        .send()
        .await
        .map_err(|e| format!("Crossref request failed: {e}"))?;

    if !resp.status().is_success() {
        return Ok(CrossrefMeta::default()); // graceful fallback
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Crossref parse failed: {e}"))?;

    let message = &body["message"];

    Ok(CrossrefMeta {
        journal: message["container-title"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(String::from),
        publisher: message["publisher"].as_str().map(String::from),
        license: message["license"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|l| l["URL"].as_str())
            .map(String::from),
        funders: message["funder"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        reference_count: message["reference-count"].as_u64().unwrap_or(0) as u32,
        is_referenced_by_count: message["is-referenced-by-count"].as_u64().unwrap_or(0) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossref_meta_default() {
        let meta = CrossrefMeta::default();
        assert!(meta.journal.is_none());
        assert!(meta.publisher.is_none());
        assert!(meta.license.is_none());
        assert_eq!(meta.funders.len(), 0);
        assert_eq!(meta.reference_count, 0);
        assert_eq!(meta.is_referenced_by_count, 0);
    }

    #[test]
    fn crossref_meta_serialization_roundtrip() {
        let meta = CrossrefMeta {
            journal: Some("Nature".into()),
            publisher: Some("Springer Nature".into()),
            license: Some("https://creativecommons.org/licenses/by/4.0/".into()),
            funders: vec!["NIH".into(), "NSF".into()],
            reference_count: 42,
            is_referenced_by_count: 150,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: CrossrefMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.journal.as_deref(), Some("Nature"));
        assert_eq!(deserialized.funders.len(), 2);
        assert_eq!(deserialized.reference_count, 42);
    }
}
