//! Read-only frontier state projections plus the Receipt v1 finding-proposal
//! builder and the explicitly chartered draft artifact-retirement exception.

use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::{
    Assertion, Author, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Evidence,
    Extraction, FindingBundle, Flags, Provenance, Review,
};
use crate::events;
use crate::project::{self, Project};
use crate::proposals::{self, StateProposal};
use crate::repo;

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactLifecycleReport {
    pub ok: bool,
    pub command: String,
    #[serde(skip)]
    pub frontier: String,
    pub artifact_id: String,
    pub proposal_id: String,
    pub status: String,
    pub route: String,
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
    /// v0.11: structured provenance — populates the existing `Provenance`
    /// fields instead of jamming everything into `title`. Each is optional
    /// so Receipt producers do not have to know all of them up front.
    pub doi: Option<String>,
    pub year: Option<i32>,
    pub url: Option<String>,
    /// Authors of the source artifact (the paper/preprint/etc).
    /// Distinct from `author` above, which is the Vela actor doing the curation.
    pub source_authors: Vec<String>,
    /// External source references that justify this proposal. These are
    /// proposal provenance only; they never confer acceptance authority.
    pub source_refs: Vec<String>,
    /// v0.11: structured conditions — replaces the placeholder
    /// "Manually added finding; requires evidence review…" that was on
    /// every manually-added finding in v0.10. Each field independently optional.
    pub conditions_text: Option<String>,
    pub evidence_spans: Vec<Value>,
    pub gap: bool,
    pub negative_space: bool,
    /// Optional replication evidence for a circuit-claim proposal. It remains
    /// review material only: recording an attestation never grants its author
    /// acceptance authority or bypasses the Decision Plan.
    pub replication_attestation: Option<Value>,
}

pub fn retract_artifact(
    path: &Path,
    artifact_id: &str,
    actor: &str,
    reason: &str,
) -> Result<ArtifactLifecycleReport, String> {
    let frontier = repo::load_from_path(path)?;
    if reason.trim().is_empty() {
        return Err("artifact retirement reason must be non-empty".to_string());
    }
    let artifact = frontier
        .artifacts
        .iter()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| format!("Artifact not found: {artifact_id}"))?;
    if artifact.retracted {
        return Err(format!("Artifact {artifact_id} is already retracted"));
    }
    let actor_type = events::actor_kind(actor);
    let proposal = proposals::new_proposal(
        "artifact.retract",
        events::StateTarget {
            r#type: "artifact".to_string(),
            id: artifact_id.to_string(),
        },
        actor,
        actor_type,
        reason,
        json!({}),
        Vec::new(),
        vec![
            "Retirement changes proof readiness, not the truth or quality of any linked finding."
                .to_string(),
        ],
    );
    let result = proposals::insert_pending_at_path(path, proposal)?;
    Ok(ArtifactLifecycleReport {
        ok: true,
        command: "artifact retract".to_string(),
        frontier: frontier.project.name,
        artifact_id: artifact_id.to_string(),
        proposal_id: result.proposal_id,
        status: result.status,
        route: "deferred".to_string(),
    })
}

pub fn history(path: &Path, finding_id: &str) -> Result<Value, String> {
    history_as_of(path, finding_id, None)
}

/// v0.55: time-travel replay. When `as_of` is `Some(ts)`, the returned
/// `events` / `review_events` / `confidence_updates` are filtered to
/// records whose timestamp is `<= ts` (RFC3339 lexicographic compare),
/// the `confidence` field reports the **score at that time** (last
/// confidence update at-or-before cutoff, or genesis if none), and a
/// `replayed_at_score` field surfaces it explicitly so a caller doesn't
/// need to walk the updates array.
pub fn history_as_of(path: &Path, finding_id: &str, as_of: Option<&str>) -> Result<Value, String> {
    let frontier = repo::load_from_path(path)?;
    let context = finding_context(&frontier, finding_id)?;
    let finding = context
        .get("finding")
        .ok_or_else(|| format!("Finding not found: {finding_id}"))?;

    let cutoff = as_of.map(|s| s.to_string());
    let filter_by_ts = |arr: Option<&Value>, ts_field: &str| -> Value {
        let Some(v) = arr else {
            return Value::Array(Vec::new());
        };
        let Some(items) = v.as_array() else {
            return Value::Array(Vec::new());
        };
        match &cutoff {
            None => Value::Array(items.clone()),
            Some(c) => Value::Array(
                items
                    .iter()
                    .filter(|item| {
                        item.get(ts_field)
                            .and_then(Value::as_str)
                            .map(|t| t <= c.as_str())
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect(),
            ),
        }
    };

    let events_filtered = filter_by_ts(context.get("events"), "timestamp");
    let review_events_filtered = filter_by_ts(context.get("review_events"), "reviewed_at");
    let confidence_updates_filtered = filter_by_ts(context.get("confidence_updates"), "updated_at");

    // Score at cutoff: last confidence update at-or-before cutoff. If the
    // finding is at its genesis confidence, fall back to the current score
    // from the bundle (it never changed).
    let score_at = if let Some(arr) = confidence_updates_filtered.as_array() {
        let mut sorted: Vec<&Value> = arr.iter().collect();
        sorted.sort_by(|a, b| {
            let ta = a.get("updated_at").and_then(Value::as_str).unwrap_or("");
            let tb = b.get("updated_at").and_then(Value::as_str).unwrap_or("");
            ta.cmp(tb)
        });
        sorted
            .last()
            .and_then(|u| u.get("new_score"))
            .cloned()
            .unwrap_or_else(|| {
                finding
                    .pointer("/confidence/score")
                    .cloned()
                    .unwrap_or(Value::Null)
            })
    } else {
        finding
            .pointer("/confidence/score")
            .cloned()
            .unwrap_or(Value::Null)
    };

    Ok(json!({
        "ok": true,
        "command": "history",
        "frontier": frontier.project.name,
        "as_of": cutoff,
        "finding": {
            "id": finding.get("id"),
            "assertion": finding.pointer("/assertion/text"),
            "confidence": finding.pointer("/confidence/score"),
            "flags": finding.get("flags"),
            "annotations": finding.get("annotations"),
        },
        "replayed_at_score": score_at,
        "confidence_score": context.get("confidence_score"),
        "confidence_basis": context.get("confidence_basis"),
        "reviewed": context.get("reviewed"),
        "reviewed_by_kind": context.get("reviewed_by_kind"),
        "review_events": review_events_filtered,
        "confidence_updates": confidence_updates_filtered,
        "sources": context.get("sources"),
        "evidence_atoms": context.get("evidence_atoms"),
        "condition_records": context.get("condition_records"),
        "proposals": context.get("proposals"),
        "events": events_filtered,
        "proof_state": frontier.proof_state,
    }))
}

pub fn finding_context(frontier: &Project, finding_id: &str) -> Result<Value, String> {
    let finding = frontier
        .findings
        .iter()
        .find(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))?;
    // Legacy `.vela/reviews/` records. The canonical reviewer verdicts
    // live in the `.vela/events/` log (exposed as `events` below); this
    // collection is deliberately the legacy side and stays separate.
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
    // v0.326: `Confidence` serializes as a bare score, so a consumer
    // of the finding payload cannot see the basis or reviewed-state. A
    // confidence number must never stand alone — surface the basis and
    // the (actor-classified) review state explicitly.
    let review = finding.provenance.review.as_ref();
    let reviewed = review.map(|r| r.reviewed).unwrap_or(false);
    let reviewed_by_kind = review
        .and_then(|r| r.reviewer.as_deref())
        .map(crate::events::actor_kind);
    Ok(json!({
        "finding": finding,
        "review_events": reviews,
        "confidence_updates": confidence_updates,
        "confidence_score": finding.confidence.score,
        "confidence_basis": finding.confidence.basis,
        "reviewed": reviewed,
        "reviewed_by_kind": reviewed_by_kind,
        "sources": source_records,
        "evidence_atoms": evidence_atoms,
        "condition_records": condition_records,
        "proposals": proposals::proposals_for_finding(frontier, finding_id),
        "events": events::events_for_finding(frontier, finding_id),
        "proof_state": frontier.proof_state,
        // Phase 1A: the verification trust tier (candidate / schema_checked /
        // machine_verified / accepted). A read-only projection; machine_verified
        // is the deterministic exact-lane admission and is DISTINCT from a human
        // `accepted` (canonical-state landing via key custody). Every surface
        // that renders this must keep the two visually + semantically separate.
        "trust_tier": proposals::derive_trust_tier(frontier, finding_id).as_str(),
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

pub fn build_add_finding_proposal_at(
    options: FindingDraftOptions,
    now: &str,
) -> Result<StateProposal, String> {
    validate_score(options.confidence)?;
    chrono::DateTime::parse_from_rfc3339(now)
        .map_err(|e| format!("finding proposal timestamp must be RFC3339: {e}"))?;
    let assertion = Assertion {
        text: options.text.clone(),
        assertion_type: options.assertion_type.clone(),
        entities: vec![],
        relation: None,
        direction: None,
        causal_claim: None,
        causal_evidence_grade: None,
    };
    let evidence = Evidence {
        evidence_type: options.evidence_type.clone(),
        model_system: String::new(),
        method: if options.evidence_type == "experimental" {
            "manual state transition; control details require source inspection".to_string()
        } else {
            "manual state transition".to_string()
        },
        replicated: false,
        replication_count: None,
        evidence_spans: options.evidence_spans.clone(),
    };
    // v0.11: conditions text falls back to the v0.10 placeholder only when
    // the caller didn't supply --conditions-text. The placeholder is a
    // signal to a reviewer that scope needs to be added; once a real
    // conditions string is provided, the placeholder isn't useful.
    let conditions = Conditions {
        text: options.conditions_text.clone().unwrap_or_else(|| {
            "Manually added finding; requires evidence review before scientific use.".to_string()
        }),
        duration: None,
    };
    let confidence = Confidence {
        kind: ConfidenceKind::FrontierEpistemic,
        score: options.confidence,
        basis: "operator-supplied frontier prior; review required".to_string(),
        method: ConfidenceMethod::ExpertJudgment,
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
        url: options.url.clone(),
        title: options.source.clone(),
        authors: source_authors,
        year: options.year,
        license: None,
        publisher: None,
        funders: Vec::new(),
        extraction: Extraction {
            method: "manual_curation".to_string(),
            model: None,
            model_version: None,
            extracted_at: now.to_string(),
            extractor_version: project::VELA_COMPILER_VERSION.to_string(),
        },
        review: Some(Review {
            reviewed: false,
            reviewer: None,
            reviewed_at: None,
            corrections: Vec::new(),
        }),
        contributions: Vec::new(),
    };
    let flags = Flags {
        gap: options.gap,
        negative_space: options.negative_space,
        ..Default::default()
    };
    let finding = FindingBundle::new(
        assertion, evidence, conditions, confidence, provenance, flags,
    );
    let finding_id = finding.id.clone();
    // An agent author remains visibly typed as an agent proposal originator;
    // proposal authorship never grants decision authority.
    let actor_type = if options.author.starts_with("agent:") {
        "agent"
    } else {
        "human"
    };
    // Replication evidence rides beside `finding` without changing its frozen
    // shape. Reviewers may inspect it, but it cannot authorize acceptance.
    let payload = match options.replication_attestation {
        Some(att) => json!({"finding": finding, "replication_attestation": att}),
        None => json!({"finding": finding}),
    };
    Ok(proposals::new_proposal_at(
        "finding.add",
        events::StateTarget {
            r#type: "finding".to_string(),
            id: finding_id,
        },
        options.author,
        actor_type,
        "Manual finding added to frontier state",
        payload,
        options.source_refs,
        vec!["Manual findings require evidence review before scientific use.".to_string()],
        now,
    ))
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
            doi: None,
            year: None,
            url: None,
            source_authors: Vec::new(),
            source_refs: Vec::new(),
            conditions_text: None,
            evidence_spans: Vec::new(),
            gap: false,
            negative_space: false,
            replication_attestation: None,
        }
    }

    #[test]
    fn provenance_flags_populate_structured_fields() {
        let mut opts = base_options();
        opts.doi = Some("10.1056/NEJMoa2212948".to_string());
        opts.year = Some(2023);
        opts.url = Some("https://nejm.org/...".to_string());
        opts.source_authors = vec!["van Dyck CH".to_string(), "Swanson CJ".to_string()];
        let proposal = build_add_finding_proposal_at(opts, "2026-07-15T00:00:00Z").unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
        assert_eq!(
            finding.provenance.doi.as_deref(),
            Some("10.1056/NEJMoa2212948")
        );
        assert_eq!(finding.provenance.year, Some(2023));
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
        let proposal = build_add_finding_proposal_at(opts, "2026-07-15T00:00:00Z").unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();
        assert_eq!(finding.conditions.text, "Phase 3 RCT, 18 mo");
    }

    #[test]
    fn reviewed_entities_spans_and_gap_flags_populate_structured_fields() {
        let mut opts = base_options();
        opts.evidence_spans = vec![json!({
            "section": "abstract",
            "text": "Lecanemab slowed decline under early symptomatic AD trial conditions."
        })];
        opts.gap = true;
        opts.negative_space = true;

        let proposal = build_add_finding_proposal_at(opts, "2026-07-15T00:00:00Z").unwrap();
        let finding: bundle::FindingBundle =
            serde_json::from_value(proposal.payload["finding"].clone()).unwrap();

        assert_eq!(finding.evidence.evidence_spans.len(), 1);
        assert_eq!(
            finding.evidence.evidence_spans[0]["section"].as_str(),
            Some("abstract")
        );
        assert!(finding.flags.gap);
        assert!(finding.flags.negative_space);
    }

    #[test]
    fn omitted_flags_fall_back_to_pre_v011_shape() {
        let proposal =
            build_add_finding_proposal_at(base_options(), "2026-07-15T00:00:00Z").unwrap();
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
