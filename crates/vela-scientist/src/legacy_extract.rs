//! # Legacy LLM extractor (moved from `vela-protocol::extract` in v0.27)
//!
//! This is the LLM-using path that powers the pre-v0.22 commands —
//! `vela compile`, `vela ingest --pdf`, `vela ingest --doi`,
//! `vela ingest --text`. It calls the legacy raw-API LLM client
//! (`legacy_llm`) which reads `ANTHROPIC_API_KEY` /
//! `OPENROUTER_API_KEY` / `GROQ_API_KEY` / `GOOGLE_API_KEY` from
//! the environment.
//!
//! v0.22+ Literature Scout uses the newer `extract::extract_via_claude_cli`
//! which routes through the user's Claude Code OAuth session and
//! does not need a raw-API key. Both extractors coexist.
//!
//! Doctrine: this module lives in `vela-scientist` so the substrate
//! (`vela-protocol`) has no LLM dependency. The DTOs + parsers +
//! offline heuristic still live in `vela-protocol::extract` —
//! only the LLM-calling path moved.

use chrono::Utc;
use reqwest::Client;
use vela_protocol::bundle::{
    Assertion, Author, Conditions, Evidence, Extraction, FindingBundle, Flags, Provenance,
    compute_confidence,
};
use vela_protocol::extract::{ExtractedEvidenceSpanDto, parse_extraction_items};
use vela_protocol::fetch::Paper;
use vela_protocol::normalize;

use crate::legacy_llm::{self, LlmConfig};

const EXTRACT_PROMPT_BASE: &str = r#"You are the Vela Compiler. Extract discrete scientific findings from this paper.

A FINDING is a single testable claim about reality with specific evidence. It is NOT:
- A definition ("The BBB is a barrier that...")
- A review summary ("Many studies have shown...")
- A general statement of field consensus without specific evidence
- Background or introductory context

For EACH finding, return a JSON object:
{
  "assertion": "Precise, specific, testable claim. Include the subject, predicate, and object.",
  "assertion_type": "mechanism|therapeutic|diagnostic|epidemiological|observational|methodological|computational|theoretical|negative|review",
  "evidence_type": "experimental|observational|computational|theoretical|meta_analysis|systematic_review|case_report",
  "model_system": "e.g., 'mouse, 5xFAD', 'human BMEC in vitro', 'computational/molecular dynamics'",
  "species": "e.g., 'Mus musculus', 'Homo sapiens', or null",
  "method": "Key method or assay (e.g., 'Western blot', 'RNA-seq', 'behavioral test')",
  "sample_size": "Number or description (e.g., 'n=24', '3 cohorts of 50'), or null if not reported",
  "replicated": false,
  "replication_count": null,
  "effect_size": "With units if reported (e.g., '2.3-fold increase', 'HR=0.67, 95% CI 0.45-0.99'), else null",
  "p_value": "If reported (e.g., 'p<0.001', 'p=0.03'), else null",
  "clinical_trial_phase": null,
  "blinding": null,
  "conditions": "Specific conditions: dose, duration, cell type, age, disease stage. Be precise.",
  "in_vitro": false,
  "in_vivo": false,
  "human_data": false,
  "entities": [{"name": "MFSD2A", "type": "protein", "species_context": "Homo sapiens"}],
  "relation": "e.g., 'inhibits', 'activates', 'crosses', 'causes', 'correlates_with'",
  "direction": "positive|negative|bidirectional|null",
  "gap": false,
  "negative_space": false,
  "contested": false,
  "evidence_spans": [{"text": "exact verbatim quote from the paper", "section": "results|discussion|methods|abstract"}]
}

Entity types: gene, protein, compound, disease, cell_type, organism, pathway, assay, anatomical_structure, metabolite, receptor, biomarker, other

CRITICAL RULES:
1. Extract ALL discrete findings. Do NOT cap at a fixed number. A data-rich paper may have 10+ findings. A case report may have 1. Extract what the paper actually contains.
2. REJECT definitions, field summaries, and textbook knowledge. Only extract claims backed by specific evidence in THIS paper.
3. PRESERVE UNCERTAINTY. If the paper says "preliminary" or "suggests", do NOT upgrade to "demonstrates" or "establishes". Use the paper's own language of certainty.
4. replicated=true ONLY if the paper explicitly cites independent replication or this is a replication study. Default to false for novel single-study findings.
5. NEGATIVE RESULTS are findings. If an experiment failed or showed no effect, extract it with negative_space=true. These are scientifically valuable.
6. gap=true when authors explicitly identify underexplored territory or call for further research on a specific question.
7. contested=true when the paper discusses conflicting evidence or ongoing scientific debate about this specific claim.
8. evidence_spans: For EACH finding, provide 1-3 EXACT VERBATIM quotes from the paper. Include the section (results, discussion, methods, abstract). If no exact quote supports the finding, reconsider whether it's a real finding.
9. Be SPECIFIC about conditions: include concentration, duration, cell type, animal strain, disease model, age group when mentioned.
10. For clinical trials: set clinical_trial_phase and blinding if applicable."#;

const EXTRACT_FULLTEXT_ADDENDUM: &str = r#"

FULL TEXT AVAILABLE — extraction priorities:
- Extract findings primarily from the RESULTS section (these contain the actual experimental data).
- Use the DISCUSSION for context, identified gaps, and contested claims.
- Reference METHODS for model system and assay details.
- Mark each evidence_span with the section it came from.
- Prefer quoting from RESULTS over ABSTRACT when both describe the same finding."#;

const EXTRACT_SUFFIX: &str = "\n\nReturn ONLY a JSON array.";

pub async fn extract_paper(
    client: &Client,
    config: &LlmConfig,
    paper: &Paper,
) -> Result<Vec<FindingBundle>, String> {
    let has_fulltext = paper.full_text.is_some();

    // Build paper text with section content when available
    let mut paper_text = format!(
        "TITLE: {}\n\nABSTRACT: {}",
        paper.title, paper.abstract_text
    );

    if let Some(ft) = &paper.full_text {
        // Give the model as much text as possible — modern LLMs handle 100K+ tokens.
        // Results get priority (actual data), then discussion (interpretation),
        // then methods (experimental details).
        if !ft.results.is_empty() {
            let results_trunc: String = ft.results.chars().take(12000).collect();
            paper_text.push_str(&format!("\n\nRESULTS: {results_trunc}"));
        }
        if !ft.discussion.is_empty() {
            let disc_trunc: String = ft.discussion.chars().take(8000).collect();
            paper_text.push_str(&format!("\n\nDISCUSSION: {disc_trunc}"));
        }
        if !ft.methods.is_empty() {
            let meth_trunc: String = ft.methods.chars().take(4000).collect();
            paper_text.push_str(&format!("\n\nMETHODS: {meth_trunc}"));
        }
    }

    if !paper.authors.is_empty() {
        let names: Vec<_> = paper
            .authors
            .iter()
            .take(5)
            .map(|a| a.name.as_str())
            .collect();
        paper_text.push_str(&format!("\n\nAUTHORS: {}", names.join(", ")));
    }
    if let Some(year) = paper.year {
        paper_text.push_str(&format!("\n\nYEAR: {year}"));
    }

    // Build system prompt: base + optional fulltext addendum + suffix
    let system_prompt = if has_fulltext {
        format!("{EXTRACT_PROMPT_BASE}{EXTRACT_FULLTEXT_ADDENDUM}{EXTRACT_SUFFIX}")
    } else {
        format!("{EXTRACT_PROMPT_BASE}{EXTRACT_SUFFIX}")
    };

    // Try up to 2 times on parse failure
    let mut parsed = None;
    for attempt in 0..2 {
        let raw = legacy_llm::call(client, config, &system_prompt, &paper_text).await?;
        match legacy_llm::parse_json(&raw) {
            Ok(v) => {
                parsed = Some(v);
                break;
            }
            Err(_) if attempt == 0 => continue,
            Err(e) => return Err(e),
        }
    }
    let parsed = parsed.ok_or_else(|| "Failed to parse after retries".to_string())?;

    let items = parse_extraction_items(parsed)?;

    let now = Utc::now().to_rfc3339();
    let mut bundles = Vec::new();

    for item in items {
        let entities: Vec<vela_protocol::bundle::Entity> = item
            .entities
            .iter()
            .map(|e| vela_protocol::bundle::Entity {
                name: normalize::entity_name(&e.name),
                entity_type: normalize::entity_type(if e.entity_type.is_empty() {
                    "other"
                } else {
                    &e.entity_type
                }),
                identifiers: Default::default(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: None,
                resolution_confidence: 1.0,
                resolution_method: None,
                species_context: e.species_context.clone(),
                needs_review: false,
            })
            .collect();

        let assertion = Assertion {
            text: item.assertion.clone(),
            assertion_type: if item.assertion_type.is_empty() {
                "mechanism".to_string()
            } else {
                item.assertion_type.clone()
            },
            entities,
            relation: item.relation.clone(),
            direction: item.direction.clone(),
        };

        let evidence_spans: Vec<serde_json::Value> = item
            .evidence_spans
            .clone()
            .into_iter()
            .filter(|span| !span.text().trim().is_empty())
            .map(ExtractedEvidenceSpanDto::into_value)
            .collect();

        let evidence = Evidence {
            evidence_type: if item.evidence_type.is_empty() {
                "experimental".to_string()
            } else {
                item.evidence_type.clone()
            },
            model_system: item.model_system.clone(),
            species: item.species.clone(),
            method: item.method.clone(),
            sample_size: item.sample_size.clone(),
            effect_size: item.effect_size.clone(),
            p_value: item.p_value.clone(),
            replicated: item.replicated,
            replication_count: item.replication_count,
            evidence_spans,
        };

        let conditions = Conditions {
            text: item.conditions.text.clone(),
            species_verified: Vec::new(),
            species_unverified: Vec::new(),
            in_vitro: item.in_vitro,
            in_vivo: item.in_vivo,
            human_data: item.human_data,
            clinical_trial: item.clinical_trial,
            concentration_range: item.conditions.concentration_range.clone(),
            duration: item.conditions.duration.clone(),
            age_group: item.conditions.age_group.clone(),
            cell_type: item.conditions.cell_type.clone(),
        };

        // Lower extraction confidence when no evidence spans are available.
        let extraction_confidence = if evidence.evidence_spans.is_empty() {
            0.6
        } else {
            0.85
        };

        let contested = item.contested;

        // Compute frontier support from structured evidence fields (v0.2.0).
        let mut confidence = compute_confidence(&evidence, &conditions, contested);
        confidence.extraction_confidence = extraction_confidence;

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
                method: "llm_extraction".to_string(),
                model: Some(config.model.clone()),
                model_version: None,
                extracted_at: now.clone(),
                extractor_version: "vela/0.2.0".to_string(),
            },
            review: None,
            citation_count: Some(paper.citations),
        };

        let flags = Flags {
            gap: item.gap,
            negative_space: item.negative_space,
            contested: item.contested,
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

    Ok(bundles)
}
