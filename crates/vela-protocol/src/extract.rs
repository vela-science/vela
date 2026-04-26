//! Stage 2: EXTRACT — DTOs, JSON parsers, and a deterministic
//! offline fallback for finding-bundle extraction. The LLM-driven
//! extraction path moved to `vela-scientist::legacy_extract` in
//! v0.27 (substrate cleanup); this module stays LLM-free.

use chrono::Utc;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::bundle::*;
use crate::fetch::Paper;
use crate::normalize;


#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtractedFindingDto {
    #[serde(default)]
    pub assertion: String,
    #[serde(default)]
    pub assertion_type: String,
    #[serde(default)]
    pub evidence_type: String,
    #[serde(default)]
    pub model_system: String,
    #[serde(default)]
    pub species: Option<String>,
    #[serde(default)]
    pub method: String,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub sample_size: Option<String>,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub effect_size: Option<String>,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub p_value: Option<String>,
    #[serde(default)]
    pub replicated: bool,
    #[serde(default, deserialize_with = "optional_u32_any")]
    pub replication_count: Option<u32>,
    #[serde(default, deserialize_with = "conditions_any")]
    pub conditions: ExtractedConditionsDto,
    #[serde(default)]
    pub in_vitro: bool,
    #[serde(default)]
    pub in_vivo: bool,
    #[serde(default)]
    pub human_data: bool,
    #[serde(default)]
    pub clinical_trial: bool,
    #[serde(default)]
    pub entities: Vec<ExtractedEntityDto>,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub gap: bool,
    #[serde(default)]
    pub negative_space: bool,
    #[serde(default)]
    pub contested: bool,
    #[serde(default)]
    pub evidence_spans: Vec<ExtractedEvidenceSpanDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtractedEntityDto {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type", alias = "entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub species_context: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtractedConditionsDto {
    #[serde(default)]
    pub text: String,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub concentration_range: Option<String>,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub duration: Option<String>,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub age_group: Option<String>,
    #[serde(default, deserialize_with = "optional_string_any")]
    pub cell_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ExtractedEvidenceSpanDto {
    Object {
        #[serde(default)]
        text: String,
        #[serde(default)]
        section: Option<String>,
    },
    Text(String),
}

impl ExtractedEvidenceSpanDto {
    pub fn text(&self) -> &str {
        match self {
            ExtractedEvidenceSpanDto::Object { text, .. } => text,
            ExtractedEvidenceSpanDto::Text(text) => text,
        }
    }

    pub fn into_value(self) -> Value {
        match self {
            ExtractedEvidenceSpanDto::Object { text, section } => serde_json::json!({
                "text": text,
                "section": section.unwrap_or_else(|| "unknown".to_string()),
            }),
            ExtractedEvidenceSpanDto::Text(text) => serde_json::json!({
                "text": text,
                "section": "unknown",
            }),
        }
    }
}

/// Parse LLM extraction JSON into typed DTOs with item-indexed error messages.
pub fn parse_extraction_items(parsed: Value) -> Result<Vec<ExtractedFindingDto>, String> {
    let items = extraction_array(parsed)?;
    items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            serde_json::from_value::<ExtractedFindingDto>(item)
                .map_err(|e| format!("extraction[{idx}]: {e}"))
        })
        .collect()
}

fn extraction_array(parsed: Value) -> Result<Vec<Value>, String> {
    match parsed {
        Value::Array(arr) => Ok(arr),
        Value::Object(map) => map
            .into_iter()
            .find_map(|(_, value)| match value {
                Value::Array(arr) => Some(arr),
                _ => None,
            })
            .ok_or_else(|| "Expected JSON array or object containing an array".to_string()),
        _ => Err("Expected JSON array".to_string()),
    }
}

fn optional_string_any<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => non_empty_string(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        Some(other) => non_empty_string(other.to_string()),
    })
}

fn optional_u32_any<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Some(Value::String(s)) => parse_first_u32(&s),
        Some(_) => None,
    })
}

fn conditions_any<'de, D>(deserializer: D) -> Result<ExtractedConditionsDto, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => ExtractedConditionsDto::default(),
        Some(Value::String(text)) => ExtractedConditionsDto {
            text,
            ..Default::default()
        },
        Some(value @ Value::Object(_)) => {
            serde_json::from_value::<ExtractedConditionsDto>(value).unwrap_or_default()
        }
        Some(other) => ExtractedConditionsDto {
            text: other.to_string(),
            ..Default::default()
        },
    })
}

fn non_empty_string(s: String) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_first_u32(s: &str) -> Option<u32> {
    s.split(|c: char| !c.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<u32>().ok())
}


/// Deterministic fallback used when no LLM backend is configured.
///
/// This deliberately produces conservative abstract-backed findings. It is not
/// intended to match LLM extraction quality; it keeps the compile/check/proof
/// quickstart credential-free and reviewable.
pub fn extract_paper_offline(paper: &Paper) -> Vec<FindingBundle> {
    let now = Utc::now().to_rfc3339();
    let mut bundles = Vec::new();
    let text = format!("{} {}", paper.title, paper.abstract_text);

    for sentence in candidate_sentences(&paper.abstract_text)
        .into_iter()
        .take(3)
    {
        let assertion = Assertion {
            text: sentence.clone(),
            assertion_type: infer_assertion_type(&sentence).to_string(),
            entities: infer_entities(&text),
            relation: None,
            direction: None,
        };

        let evidence = Evidence {
            evidence_type: "observational".to_string(),
            model_system: "abstract-only deterministic extraction".to_string(),
            species: None,
            method: "abstract sentence extraction".to_string(),
            sample_size: None,
            effect_size: None,
            p_value: None,
            replicated: false,
            replication_count: None,
            evidence_spans: vec![serde_json::json!({
                "text": sentence,
                "section": "abstract"
            })],
        };

        let lower = assertion.text.to_lowercase();
        let conditions = Conditions {
            text: "Extracted from paper abstract; requires human review before interpretation."
                .to_string(),
            species_verified: Vec::new(),
            species_unverified: Vec::new(),
            in_vitro: lower.contains("in vitro") || lower.contains("cell"),
            in_vivo: lower.contains("mouse") || lower.contains("mice") || lower.contains("rat"),
            human_data: lower.contains("patient")
                || lower.contains("human")
                || lower.contains("clinical"),
            clinical_trial: lower.contains("trial"),
            concentration_range: None,
            duration: None,
            age_group: None,
            cell_type: None,
        };

        let mut confidence = compute_confidence(&evidence, &conditions, false);
        confidence.extraction_confidence = 0.35;
        confidence.basis = format!("{}; deterministic abstract-only fallback", confidence.basis);

        let provenance = Provenance {
            source_type: "published_paper".to_string(),
            doi: paper.doi.clone(),
            pmid: None,
            pmc: None,
            openalex_id: paper.openalex_id.clone(),
            url: None,
            title: paper.title.clone(),
            authors: paper
                .authors
                .iter()
                .map(|a| Author {
                    name: a.name.clone(),
                    orcid: a.orcid.clone(),
                })
                .collect(),
            year: paper.year,
            journal: None,
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Extraction {
                method: "hybrid".to_string(),
                model: None,
                model_version: None,
                extracted_at: now.clone(),
                extractor_version: "vela/0.2.0-offline".to_string(),
            },
            review: None,
            citation_count: Some(paper.citations),
        };

        let flags = Flags {
            gap: lower.contains("future research")
                || lower.contains("further research")
                || lower.contains("unknown")
                || lower.contains("unclear"),
            negative_space: lower.contains("no significant")
                || lower.contains("did not")
                || lower.contains("failed to"),
            contested: lower.contains("controvers")
                || lower.contains("conflicting")
                || lower.contains("debate"),
            retracted: false,
            declining: false,
            gravity_well: false,
            review_state: None,
            superseded: false,
        };

        bundles.push(FindingBundle::new(
            assertion, evidence, conditions, confidence, provenance, flags,
        ));
    }

    bundles
}

fn candidate_sentences(abstract_text: &str) -> Vec<String> {
    abstract_text
        .split_terminator(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| s.split_whitespace().count() >= 8)
        .map(|s| format!("{s}."))
        .collect()
}

fn infer_assertion_type(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("diagnos") || lower.contains("biomarker") {
        "diagnostic"
    } else if lower.contains("treat") || lower.contains("therapy") || lower.contains("drug") {
        "therapeutic"
    } else if lower.contains("associate")
        || lower.contains("correlat")
        || lower.contains("risk")
        || lower.contains("cohort")
    {
        "epidemiological"
    } else if lower.contains("method") || lower.contains("assay") || lower.contains("model") {
        "methodological"
    } else {
        "observational"
    }
}

fn infer_entities(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();
    let lower = text.to_lowercase();
    let known = [
        ("blood-brain barrier", "anatomical_structure"),
        ("bbb", "anatomical_structure"),
        ("alzheimer", "disease"),
        ("amyloid", "protein"),
        ("amyloid-beta", "protein"),
        ("tau", "protein"),
        ("lrp1", "protein"),
        ("rage", "receptor"),
        ("transcytosis", "pathway"),
        ("microglia", "cell_type"),
        ("astrocyte", "cell_type"),
    ];

    for (name, entity_type) in known {
        if lower.contains(name) {
            entities.push(Entity {
                name: normalize::entity_name(name),
                entity_type: normalize::entity_type(entity_type),
                identifiers: Default::default(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: None,
                resolution_confidence: 1.0,
                resolution_method: None,
                species_context: None,
                needs_review: false,
            });
        }
    }

    if entities.is_empty() {
        entities.push(Entity {
            name: normalize::entity_name("paper topic"),
            entity_type: "other".to_string(),
            identifiers: Default::default(),
            canonical_id: None,
            candidates: Vec::new(),
            aliases: Vec::new(),
            resolution_provenance: None,
            resolution_confidence: 1.0,
            resolution_method: None,
            species_context: None,
            needs_review: false,
        });
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extraction_items_preserves_prompted_fields() {
        let parsed = serde_json::json!({
            "findings": [{
                "assertion": "LRP1 clears amyloid beta in mice.",
                "assertion_type": "mechanism",
                "evidence_type": "experimental",
                "model_system": "5xFAD mouse",
                "species": "Mus musculus",
                "method": "Western blot with vehicle control",
                "sample_size": 24,
                "p_value": "p<0.001",
                "replicated": true,
                "replication_count": "3 independent cohorts",
                "conditions": {
                    "text": "12 week old mice",
                    "duration": "12 weeks",
                    "cell_type": "endothelial cells"
                },
                "entities": [{"name": "LRP1", "type": "receptor", "species_context": "Mus musculus"}],
                "evidence_spans": [
                    {"text": "LRP1 increased amyloid clearance.", "section": "results"},
                    "A second cohort replicated this effect."
                ]
            }]
        });

        let items = parse_extraction_items(parsed).unwrap();
        let item = &items[0];

        assert_eq!(item.sample_size.as_deref(), Some("24"));
        assert_eq!(item.p_value.as_deref(), Some("p<0.001"));
        assert_eq!(item.replication_count, Some(3));
        assert_eq!(item.conditions.duration.as_deref(), Some("12 weeks"));
        assert_eq!(
            item.conditions.cell_type.as_deref(),
            Some("endothelial cells")
        );
        assert_eq!(item.model_system, "5xFAD mouse");
        assert_eq!(item.method, "Western blot with vehicle control");
        assert_eq!(item.species.as_deref(), Some("Mus musculus"));
        assert_eq!(item.evidence_spans.len(), 2);
    }

    #[test]
    fn parse_extraction_items_reports_item_path() {
        let parsed = serde_json::json!([{
            "assertion": "Malformed span.",
            "evidence_spans": [42]
        }]);

        let err = parse_extraction_items(parsed).unwrap_err();

        assert!(err.contains("extraction[0]"));
    }
}
