//! Non-interactive frontier state transitions.
//!
//! Write commands are proposal-first. Pending proposals are review artifacts;
//! accepted proposals become canonical state events through one reducer.

use std::path::Path;

use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::{
    Assertion, Author, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Entity, Evidence,
    Extraction, FindingBundle, Flags, Provenance, Review,
};
use crate::events;
use crate::project::{self, Project};
use crate::proposals::{self, StateProposal};
use crate::repo;

#[derive(Debug, Clone, Serialize)]
pub struct StateCommandReport {
    pub ok: bool,
    pub command: String,
    pub frontier: String,
    pub finding_id: String,
    pub proposal_id: String,
    pub proposal_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_event_id: Option<String>,
    pub wrote_to: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct FindingDraftOptions {
    pub text: String,
    pub assertion_type: String,
    pub source: String,
    pub source_type: String,
    pub author: String,
    pub confidence: f64,
    pub evidence_type: String,
    pub entities: Vec<(String, String)>,
    /// v0.11: structured provenance — populates the existing `Provenance`
    /// fields instead of jamming everything into `title`. Each is optional
    /// so `vela finding add` callers don't have to know all of them up front;
    /// the substrate has the fields, the CLI just exposes them.
    #[allow(dead_code)] // populated by CLI; consumed by build_add_finding_proposal
    pub doi: Option<String>,
    #[allow(dead_code)]
    pub pmid: Option<String>,
    #[allow(dead_code)]
    pub year: Option<i32>,
    #[allow(dead_code)]
    pub journal: Option<String>,
    #[allow(dead_code)]
    pub url: Option<String>,
    /// Authors of the source artifact (the paper/preprint/etc).
    /// Distinct from `author` above, which is the Vela actor doing the curation.
    #[allow(dead_code)]
    pub source_authors: Vec<String>,
    /// v0.11: structured conditions — replaces the placeholder
    /// "Manually added finding; requires evidence review…" that was on
    /// every manually-added finding in v0.10. Each field independently optional.
    #[allow(dead_code)]
    pub conditions_text: Option<String>,
    #[allow(dead_code)]
    pub species: Vec<String>,
    #[allow(dead_code)]
    pub in_vivo: bool,
    #[allow(dead_code)]
    pub in_vitro: bool,
    #[allow(dead_code)]
    pub human_data: bool,
    #[allow(dead_code)]
    pub clinical_trial: bool,
}

#[derive(Debug, Clone)]
pub struct ReviewOptions {
    pub status: String,
    pub reason: String,
    pub reviewer: String,
}

#[derive(Debug, Clone)]
pub struct ReviseOptions {
    pub confidence: f64,
    pub reason: String,
    pub reviewer: String,
}

pub fn add_finding(
    path: &Path,
    options: FindingDraftOptions,
    apply: bool,
) -> Result<StateCommandReport, String> {
    validate_score(options.confidence)?;
    let proposal = build_add_finding_proposal(options)?;
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "finding.add".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status.clone(),
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if result.status == "applied" {
            "Finding proposal applied".to_string()
        } else {
            "Finding proposal recorded".to_string()
        },
    })
}

pub fn review_finding(
    path: &Path,
    finding_id: &str,
    options: ReviewOptions,
    apply: bool,
) -> Result<StateCommandReport, String> {
    let proposal = proposals::new_proposal(
        "finding.review",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        options.reviewer.clone(),
        "human",
        options.reason.clone(),
        json!({"status": options.status}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "review".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Review proposal applied".to_string()
        } else {
            "Review proposal recorded".to_string()
        },
    })
}

pub fn add_note(
    path: &Path,
    finding_id: &str,
    text: &str,
    author: &str,
    apply: bool,
) -> Result<StateCommandReport, String> {
    let proposal = proposals::new_proposal(
        "finding.note",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        author.to_string(),
        "human",
        text.to_string(),
        json!({"text": text}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "note".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Note proposal applied".to_string()
        } else {
            "Note proposal recorded".to_string()
        },
    })
}

pub fn caveat_finding(
    path: &Path,
    finding_id: &str,
    text: &str,
    author: &str,
    apply: bool,
) -> Result<StateCommandReport, String> {
    let proposal = proposals::new_proposal(
        "finding.caveat",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        author.to_string(),
        "human",
        text.to_string(),
        json!({"text": text}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "caveat".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Caveat proposal applied".to_string()
        } else {
            "Caveat proposal recorded".to_string()
        },
    })
}

pub fn revise_confidence(
    path: &Path,
    finding_id: &str,
    options: ReviseOptions,
    apply: bool,
) -> Result<StateCommandReport, String> {
    validate_score(options.confidence)?;
    let proposal = proposals::new_proposal(
        "finding.confidence_revise",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        options.reviewer.clone(),
        "human",
        options.reason.clone(),
        json!({"confidence": options.confidence}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "revise".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Confidence revision applied".to_string()
        } else {
            "Confidence revision proposal recorded".to_string()
        },
    })
}

pub fn reject_finding(
    path: &Path,
    finding_id: &str,
    reviewer: &str,
    reason: &str,
    apply: bool,
) -> Result<StateCommandReport, String> {
    let proposal = proposals::new_proposal(
        "finding.reject",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        reviewer.to_string(),
        "human",
        reason.to_string(),
        json!({"status": "rejected"}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "reject".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Rejection proposal applied".to_string()
        } else {
            "Rejection proposal recorded".to_string()
        },
    })
}

pub fn retract_finding(
    path: &Path,
    finding_id: &str,
    reviewer: &str,
    reason: &str,
    apply: bool,
) -> Result<StateCommandReport, String> {
    let frontier = repo::load_from_path(path)?;
    find_finding_index(&frontier, finding_id)?;
    let proposal = proposals::new_proposal(
        "finding.retract",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id.to_string(),
        },
        reviewer,
        "human",
        reason,
        json!({}),
        Vec::new(),
        vec!["Retraction impact is simulated over declared dependency links.".to_string()],
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    Ok(StateCommandReport {
        ok: true,
        command: "retract".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status,
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if apply {
            "Retraction proposal applied".to_string()
        } else {
            "Retraction proposal recorded".to_string()
        },
    })
}

pub fn history(path: &Path, finding_id: &str) -> Result<Value, String> {
    let frontier = repo::load_from_path(path)?;
    let context = finding_context(&frontier, finding_id)?;
    let finding = context
        .get("finding")
        .ok_or_else(|| format!("Finding not found: {finding_id}"))?;
    Ok(json!({
        "ok": true,
        "command": "history",
        "frontier": frontier.project.name,
        "finding": {
            "id": finding.get("id"),
            "assertion": finding.pointer("/assertion/text"),
            "confidence": finding.pointer("/confidence/score"),
            "flags": finding.get("flags"),
            "annotations": finding.get("annotations"),
        },
        "review_events": context.get("review_events"),
        "confidence_updates": context.get("confidence_updates"),
        "sources": context.get("sources"),
        "evidence_atoms": context.get("evidence_atoms"),
        "condition_records": context.get("condition_records"),
        "proposals": context.get("proposals"),
        "events": context.get("events"),
        "proof_state": frontier.proof_state,
    }))
}

pub fn finding_context(frontier: &Project, finding_id: &str) -> Result<Value, String> {
    let finding = frontier
        .findings
        .iter()
        .find(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))?;
    let reviews = frontier
        .review_events
        .iter()
        .filter(|event| event.finding_id == finding_id)
        .collect::<Vec<_>>();
    let confidence_updates = frontier
        .confidence_updates
        .iter()
        .filter(|update| update.finding_id == finding_id)
        .collect::<Vec<_>>();
    let source_records = frontier
        .sources
        .iter()
        .filter(|source| source.finding_ids.iter().any(|id| id == finding_id))
        .collect::<Vec<_>>();
    let evidence_atoms = frontier
        .evidence_atoms
        .iter()
        .filter(|atom| atom.finding_id == finding_id)
        .collect::<Vec<_>>();
    let condition_records = frontier
        .condition_records
        .iter()
        .filter(|record| record.finding_id == finding_id)
        .collect::<Vec<_>>();
    Ok(json!({
        "finding": finding,
        "review_events": reviews,
        "confidence_updates": confidence_updates,
        "sources": source_records,
        "evidence_atoms": evidence_atoms,
        "condition_records": condition_records,
        "proposals": proposals::proposals_for_finding(frontier, finding_id),
        "events": events::events_for_finding(frontier, finding_id),
        "proof_state": frontier.proof_state,
    }))
}

pub fn state_transitions(frontier: &Project) -> Value {
    let mut transitions = Vec::new();
    if !frontier.events.is_empty() {
        for event in &frontier.events {
            transitions.push(json!({
                "kind": event.kind,
                "id": event.id,
                "target": event.target,
                "actor": event.actor,
                "timestamp": event.timestamp,
                "reason": event.reason,
                "before_hash": event.before_hash,
                "after_hash": event.after_hash,
                "payload": event.payload,
                "caveats": event.caveats,
            }));
        }
        transitions.sort_by(|a, b| {
            a.get("timestamp")
                .and_then(Value::as_str)
                .cmp(&b.get("timestamp").and_then(Value::as_str))
        });
        return json!({
            "schema": "vela.state-transitions.v1",
            "frontier": frontier.project.name,
            "source": "canonical_events",
            "transitions": transitions,
        });
    }
    for event in &frontier.review_events {
        transitions.push(json!({
            "kind": "review_event",
            "id": event.id,
            "target": {"type": "finding", "id": event.finding_id},
            "actor": event.reviewer,
            "timestamp": event.reviewed_at,
            "action": event.action,
            "reason": event.reason,
            "state_change": event.state_change,
        }));
    }
    for update in &frontier.confidence_updates {
        transitions.push(json!({
            "kind": "confidence_update",
            "id": confidence_update_id(update),
            "target": {"type": "finding", "id": update.finding_id},
            "actor": update.updated_by,
            "timestamp": update.updated_at,
            "action": "confidence_revised",
            "reason": update.basis,
            "state_change": {
                "previous_score": update.previous_score,
                "new_score": update.new_score,
            },
        }));
    }
    transitions.sort_by(|a, b| {
        a.get("timestamp")
            .and_then(Value::as_str)
            .cmp(&b.get("timestamp").and_then(Value::as_str))
    });
    json!({
        "schema": "vela.state-transitions.v0",
        "frontier": frontier.project.name,
        "transitions": transitions,
    })
}

/// Build a content-addressed FindingBundle from CLI-supplied options.
/// Shared by `finding.add` and v0.14 `finding.supersede`.
fn build_finding_bundle(options: &FindingDraftOptions) -> FindingBundle {
    let now = Utc::now().to_rfc3339();
    let assertion = Assertion {
        text: options.text.clone(),
        assertion_type: options.assertion_type.clone(),
        entities: options
            .entities
            .iter()
            .map(|(name, entity_type)| Entity {
                name: name.clone(),
                entity_type: entity_type.clone(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: Some("manual_state_transition".to_string()),
                resolution_confidence: 0.6,
                resolution_method: None,
                species_context: None,
                needs_review: true,
            })
            .collect(),
        relation: None,
        direction: None,
    };
    let evidence = Evidence {
        evidence_type: options.evidence_type.clone(),
        model_system: String::new(),
        species: None,
        method: "manual state transition".to_string(),
        sample_size: None,
        effect_size: None,
        p_value: None,
        replicated: false,
        replication_count: None,
        evidence_spans: Vec::new(),
    };
    let conditions = Conditions {
        text: options.conditions_text.clone().unwrap_or_else(|| {
            "Manually added finding; requires evidence review before scientific use.".to_string()
        }),
        species_verified: options.species.clone(),
        species_unverified: Vec::new(),
        in_vitro: options.in_vitro,
        in_vivo: options.in_vivo,
        human_data: options.human_data,
        clinical_trial: options.clinical_trial,
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: None,
    };
    let confidence = Confidence {
        kind: ConfidenceKind::FrontierEpistemic,
        score: options.confidence,
        basis: "operator-supplied frontier prior; review required".to_string(),
        method: ConfidenceMethod::ExpertJudgment,
        components: None,
        extraction_confidence: 1.0,
    };
    let source_authors = if options.source_authors.is_empty() {
        vec![Author {
            name: options.author.clone(),
            orcid: None,
        }]
    } else {
        options
            .source_authors
            .iter()
            .map(|name| Author {
                name: name.clone(),
                orcid: None,
            })
            .collect()
    };
    let provenance = Provenance {
        source_type: options.source_type.clone(),
        doi: options.doi.clone(),
        pmid: options.pmid.clone(),
        pmc: None,
        openalex_id: None,
        url: options.url.clone(),
        title: options.source.clone(),
        authors: source_authors,
        year: options.year,
        journal: options.journal.clone(),
        license: None,
        publisher: None,
        funders: Vec::new(),
        extraction: Extraction {
            method: "manual_curation".to_string(),
            model: None,
            model_version: None,
            extracted_at: now,
            extractor_version: project::VELA_COMPILER_VERSION.to_string(),
        },
        review: Some(Review {
            reviewed: false,
            reviewer: None,
            reviewed_at: None,
            corrections: Vec::new(),
        }),
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
        superseded: false,
    };
    FindingBundle::new(
        assertion, evidence, conditions, confidence, provenance, flags,
    )
}

/// v0.14: build the proposal that supersedes `old_id` with a new finding bundle.
pub fn supersede_finding(
    path: &Path,
    old_id: &str,
    reason: &str,
    options: FindingDraftOptions,
    apply: bool,
) -> Result<StateCommandReport, String> {
    validate_score(options.confidence)?;
    if reason.trim().is_empty() {
        return Err("--reason is required for finding supersede".to_string());
    }
    let new_finding = build_finding_bundle(&options);
    if new_finding.id == old_id {
        return Err(
            "supersede new assertion must produce a different content address than the old finding (change assertion text, type, or provenance to derive a distinct vf_…)"
                .to_string(),
        );
    }
    let proposal = proposals::new_proposal(
        "finding.supersede",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: old_id.to_string(),
        },
        options.author.clone(),
        "human",
        reason.to_string(),
        json!({"new_finding": new_finding}),
        Vec::new(),
        Vec::new(),
    );
    let result = proposals::create_or_apply(path, proposal, apply)?;
    let frontier = repo::load_from_path(path)?;
    Ok(StateCommandReport {
        ok: true,
        command: "finding.supersede".to_string(),
        frontier: frontier.project.name,
        finding_id: result.finding_id,
        proposal_id: result.proposal_id,
        proposal_status: result.status.clone(),
        applied_event_id: result.applied_event_id,
        wrote_to: path.display().to_string(),
        message: if result.status == "applied" {
            "Supersede proposal applied".to_string()
        } else {
            "Supersede proposal recorded".to_string()
        },
    })
}

fn build_add_finding_proposal(options: FindingDraftOptions) -> Result<StateProposal, String> {
    let now = Utc::now().to_rfc3339();
    let assertion = Assertion {
        text: options.text.clone(),
        assertion_type: options.assertion_type.clone(),
        entities: options
            .entities
            .iter()
            .map(|(name, entity_type)| Entity {
                name: name.clone(),
                entity_type: entity_type.clone(),
                identifiers: serde_json::Map::new(),
                canonical_id: None,
                candidates: Vec::new(),
                aliases: Vec::new(),
                resolution_provenance: Some("manual_state_transition".to_string()),
                resolution_confidence: 0.6,
                resolution_method: None,
                species_context: None,
                needs_review: true,
            })
            .collect(),
        relation: None,
        direction: None,
    };
    let evidence = Evidence {
        evidence_type: options.evidence_type.clone(),
        model_system: String::new(),
        species: None,
        method: "manual state transition".to_string(),
        sample_size: None,
        effect_size: None,
        p_value: None,
        replicated: false,
        replication_count: None,
        evidence_spans: Vec::new(),
    };
    // v0.11: conditions text falls back to the v0.10 placeholder only when
    // the caller didn't supply --conditions-text. The placeholder is a
    // signal to a reviewer that scope needs to be added; once a real
    // conditions string is provided, the placeholder isn't useful.
    let conditions = Conditions {
        text: options.conditions_text.clone().unwrap_or_else(|| {
            "Manually added finding; requires evidence review before scientific use.".to_string()
        }),
        species_verified: options.species.clone(),
        species_unverified: Vec::new(),
        in_vitro: options.in_vitro,
        in_vivo: options.in_vivo,
        human_data: options.human_data,
        clinical_trial: options.clinical_trial,
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: None,
    };
    let confidence = Confidence {
        kind: ConfidenceKind::FrontierEpistemic,
        score: options.confidence,
        basis: "operator-supplied frontier prior; review required".to_string(),
        method: ConfidenceMethod::ExpertJudgment,
        components: None,
        extraction_confidence: 1.0,
    };
    // v0.11: structured provenance. Source authors (the paper's authors)
    // are distinct from the Vela actor that curated the finding. When
    // --authors is omitted, fall back to the curator-as-author shape used
    // pre-v0.11 so existing scripts keep working.
    let source_authors = if options.source_authors.is_empty() {
        vec![Author {
            name: options.author.clone(),
            orcid: None,
        }]
    } else {
        options
            .source_authors
            .iter()
            .map(|name| Author {
                name: name.clone(),
                orcid: None,
            })
            .collect()
    };
    let provenance = Provenance {
        source_type: options.source_type.clone(),
        doi: options.doi.clone(),
        pmid: options.pmid.clone(),
        pmc: None,
        openalex_id: None,
        url: options.url.clone(),
        title: options.source.clone(),
        authors: source_authors,
        year: options.year,
        journal: options.journal.clone(),
        license: None,
        publisher: None,
        funders: Vec::new(),
        extraction: Extraction {
            method: "manual_curation".to_string(),
            model: None,
            model_version: None,
            extracted_at: now.clone(),
            extractor_version: project::VELA_COMPILER_VERSION.to_string(),
        },
        review: Some(Review {
            reviewed: false,
            reviewer: None,
            reviewed_at: None,
            corrections: Vec::new(),
        }),
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
        superseded: false,
    };
    let finding = FindingBundle::new(
        assertion, evidence, conditions, confidence, provenance, flags,
    );
    let finding_id = finding.id.clone();
    Ok(proposals::new_proposal(
        "finding.add",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id,
        },
        options.author,
        "human",
        "Manual finding added to frontier state",
        json!({"finding": finding}),
        Vec::new(),
        vec!["Manual findings require evidence review before scientific use.".to_string()],
    ))
}

fn find_finding_index(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

fn confidence_update_id(update: &crate::bundle::ConfidenceUpdate) -> String {
    let hash = Sha256::digest(
        format!(
            "{}|{}|{}|{}|{}",
            update.finding_id,
            update.previous_score,
            update.new_score,
            update.updated_by,
            update.updated_at
        )
        .as_bytes(),
    );
    format!("cu_{}", &hex::encode(hash)[..16])
}

fn validate_score(score: f64) -> Result<(), String> {
    if (0.0..=1.0).contains(&score) {
        Ok(())
    } else {
        Err("--confidence must be between 0.0 and 1.0".to_string())
    }
}

#[cfg(test)]
mod v0_11_finding_tests {
    use super::*;
    use crate::bundle;

    fn base_options() -> FindingDraftOptions {
        FindingDraftOptions {
            text: "Test claim".to_string(),
            assertion_type: "mechanism".to_string(),
            source: "Test 2024".to_string(),
            source_type: "published_paper".to_string(),
            author: "reviewer:test".to_string(),
            confidence: 0.5,
            evidence_type: "experimental".to_string(),
            entities: Vec::new(),
            doi: None,
            pmid: None,
            year: None,
            journal: None,
            url: None,
            source_authors: Vec::new(),
            conditions_text: None,
            species: Vec::new(),
            in_vivo: false,
            in_vitro: false,
            human_data: false,
            clinical_trial: false,
        }
    }

    #[test]
    fn provenance_flags_populate_structured_fields() {
        let mut opts = base_options();
        opts.doi = Some("10.1056/NEJMoa2212948".to_string());
        opts.pmid = Some("36449413".to_string());
        opts.year = Some(2023);
        opts.journal = Some("NEJM".to_string());
        opts.url = Some("https://nejm.org/...".to_string());
        opts.source_authors = vec!["van Dyck CH".to_string(), "Swanson CJ".to_string()];
        let proposal = build_add_finding_proposal(opts).unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
        assert_eq!(
            finding.provenance.doi.as_deref(),
            Some("10.1056/NEJMoa2212948")
        );
        assert_eq!(finding.provenance.pmid.as_deref(), Some("36449413"));
        assert_eq!(finding.provenance.year, Some(2023));
        assert_eq!(finding.provenance.journal.as_deref(), Some("NEJM"));
        assert_eq!(
            finding.provenance.url.as_deref(),
            Some("https://nejm.org/...")
        );
        assert_eq!(
            finding
                .provenance
                .authors
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["van Dyck CH", "Swanson CJ"],
        );
    }

    #[test]
    fn conditions_flags_populate_structured_fields() {
        let mut opts = base_options();
        opts.conditions_text = Some("Phase 3 RCT, 18 mo".to_string());
        opts.species = vec!["Homo sapiens".to_string()];
        opts.in_vivo = true;
        opts.human_data = true;
        opts.clinical_trial = true;
        let proposal = build_add_finding_proposal(opts).unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
        assert_eq!(finding.conditions.text, "Phase 3 RCT, 18 mo");
        assert_eq!(
            finding.conditions.species_verified,
            vec!["Homo sapiens".to_string()]
        );
        assert!(finding.conditions.in_vivo);
        assert!(finding.conditions.human_data);
        assert!(finding.conditions.clinical_trial);
    }

    #[test]
    fn omitted_flags_fall_back_to_pre_v011_shape() {
        let proposal = build_add_finding_proposal(base_options()).unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
        // Pre-v0.11 placeholder remains when --conditions-text is omitted.
        assert!(
            finding
                .conditions
                .text
                .starts_with("Manually added finding")
        );
        // No --source-authors → curator fills the authors slot, as in v0.10.
        assert_eq!(finding.provenance.authors.len(), 1);
        assert_eq!(finding.provenance.authors[0].name, "reviewer:test");
        // None of the new optional provenance fields populated.
        assert!(finding.provenance.doi.is_none());
        assert!(finding.provenance.year.is_none());
        assert!(finding.provenance.url.is_none());
    }
}
