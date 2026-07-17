//! Derived frontier signals.
//!
//! Signals are read-only projections over frontier state. They are not a second
//! source of truth and are intentionally safe to recompute from the frontier,
//! diagnostics, proof traces, or benchmark output.

#![allow(clippy::module_name_repetitions)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use vela_protocol::project::{self, Project};
use vela_protocol::proposals;
use vela_protocol::sources;

use super::actor_registration::{self, BoundaryOutcome};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignalTarget {
    pub r#type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignalItem {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub target: SignalTarget,
    pub reason: String,
    pub recommended_action: String,
    pub blocks: Vec<String>,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewQueueItem {
    pub id: String,
    pub priority: String,
    pub priority_score: u32,
    pub target: SignalTarget,
    pub signal_ids: Vec<String>,
    pub reasons: Vec<String>,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofReadiness {
    pub status: String,
    pub blockers: usize,
    pub warnings: usize,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalReport {
    pub schema: String,
    pub frontier: String,
    pub signals: Vec<SignalItem>,
    pub review_queue: Vec<ReviewQueueItem>,
    pub proof_readiness: ProofReadiness,
}

pub fn analyze(frontier: &Project, diagnostics: &[Value]) -> SignalReport {
    analyze_at(frontier, diagnostics, None)
}

pub fn analyze_at(
    frontier: &Project,
    diagnostics: &[Value],
    repo_dir: Option<&Path>,
) -> SignalReport {
    let mut signals = Vec::new();
    let actor_registration = actor_registration::assess(frontier, repo_dir);

    if let Some(repo_dir) = repo_dir {
        for (index, error) in proposals::verify_proposal_withdrawals(repo_dir, frontier)
            .into_iter()
            .enumerate()
        {
            signals.push(SignalItem {
                id: signal_id("invalid_proposal_withdrawal", &index.to_string()),
                kind: "invalid_proposal_withdrawal".to_string(),
                severity: "error".to_string(),
                target: SignalTarget {
                    r#type: "frontier".to_string(),
                    id: frontier.frontier_id().to_string(),
                },
                reason: error,
                recommended_action: "Treat the proposal as pending and restore the exact Receipt-bound signed withdrawal bytes.".to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "An invalid withdrawal grants the producer no terminal standing in either strict or non-strict reads.".to_string(),
                ],
            });
        }
    }

    for diagnostic in diagnostics {
        let severity = diagnostic
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or("info");
        let rule_id = diagnostic
            .get("rule_id")
            .and_then(Value::as_str)
            .unwrap_or("check_error");
        if severity == "error"
            || matches!(
                rule_id,
                "missing_source_record" | "missing_evidence_atom" | "condition_record_missing"
            )
        {
            let id = format!("sig_diagnostic_{}", signals.len() + 1);
            signals.push(SignalItem {
                id,
                kind: match rule_id {
                    "event_replay" => "event_replay_conflict",
                    "missing_source_record" => "missing_source_record",
                    "missing_evidence_atom" => "missing_evidence_atom",
                    "condition_record_missing" => "condition_record_missing",
                    "reviewer_identity_missing" => "reviewer_identity_missing",
                    _ => "check_error",
                }
                .to_string(),
                severity: severity.to_string(),
                target: SignalTarget {
                    r#type: diagnostic
                        .get("finding_id")
                        .and_then(Value::as_str)
                        .map_or("frontier", |_| "finding")
                        .to_string(),
                    id: diagnostic
                        .get("finding_id")
                        .and_then(Value::as_str)
                        .unwrap_or(&frontier.project.name)
                        .to_string(),
                },
                reason: diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Frontier validation error.")
                    .to_string(),
                recommended_action: diagnostic
                    .get("suggestion")
                    .and_then(Value::as_str)
                    .unwrap_or("Inspect and correct the referenced frontier field.")
                    .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![],
            });
        }
    }

    let projection = sources::derive_projection(frontier);
    let source_by_id = projection
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let finding_by_id = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect::<BTreeMap<_, _>>();
    let reviewed_finding_ids = frontier
        .events
        .iter()
        .filter(|event| {
            event.target.r#type == "finding"
                && event.actor.id.starts_with("reviewer:")
                && matches!(
                    event.kind.as_str(),
                    "finding.asserted" | "finding.reviewed" | "finding.caveated"
                )
        })
        .map(|event| event.target.id.as_str())
        .collect::<BTreeSet<_>>();

    for source in &projection.sources {
        if source.content_hash.is_none()
            && matches!(
                source.source_type.as_str(),
                "pdf"
                    | "jats"
                    | "csv"
                    | "text"
                    | "note"
                    | "agent_trace"
                    | "benchmark_output"
                    | "notebook_entry"
                    | "experiment_log"
                    | "synthetic_report"
            )
        {
            signals.push(SignalItem {
                id: signal_id("source_hash_missing", &source.id),
                kind: "source_hash_missing".into(),
                severity: "info".to_string(),
                target: SignalTarget {
                    r#type: "source".to_string(),
                    id: source.id.clone(),
                },
                reason: "Source record has no content hash for a local or generated artifact."
                    .to_string(),
                recommended_action:
                    "Recompile from the local corpus or add a source content hash before relying on this source."
                        .to_string(),
                blocks: vec![],
                caveats: vec!["Source identity and scientific confidence are separate.".to_string()],
            });
        }

        if source.source_type == "agent_trace" {
            signals.push(SignalItem {
                id: signal_id("agent_trace_unverified", &source.id),
                kind: "agent_trace_unverified".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "source".to_string(),
                    id: source.id.clone(),
                },
                reason: "Agent trace source requires review before it can support active frontier state."
                    .to_string(),
                recommended_action:
                    "Verify the trace against primary evidence and add review before proof use."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec!["Agent traces are source artifacts, not scientific truth.".to_string()],
            });
        }

        if source.source_type == "synthetic_report"
            && !source
                .finding_ids
                .iter()
                .any(|finding_id| reviewed_finding_ids.contains(finding_id.as_str()))
        {
            signals.push(SignalItem {
                id: signal_id("synthetic_source_requires_review", &source.id),
                kind: "synthetic_source_requires_review".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "source".to_string(),
                    id: source.id.clone(),
                },
                reason: "Synthetic report source requires human review and primary-source grounding."
                    .to_string(),
                recommended_action:
                    "Use synthetic reports as review leads unless evidence atoms trace back to primary sources."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec!["Synthetic sources should not silently become evidence.".to_string()],
            });
        }
    }

    for atom in &projection.evidence_atoms {
        if !atom.human_verified
            && source_by_id
                .get(atom.source_id.as_str())
                .is_some_and(|source| sources::is_synthetic_source(source))
            && !reviewed_finding_ids.contains(atom.finding_id.as_str())
        {
            signals.push(SignalItem {
                id: signal_id("synthetic_source_requires_review", &atom.id),
                kind: "synthetic_source_requires_review".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: atom.finding_id.clone(),
                },
                reason: "Evidence atom is linked to an unverified synthetic or agent source."
                    .to_string(),
                recommended_action:
                    "Attach primary evidence or review the atom before proof export.".to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "Generated traces can guide review but are not trusted evidence.".to_string(),
                ],
            });
        }
    }

    for record in &projection.condition_records {
        let theoretical_catalogue_record = finding_by_id
            .get(record.finding_id.as_str())
            .is_some_and(|finding| is_theoretical_catalogue_record(finding));
        if record.text.trim().is_empty() && !theoretical_catalogue_record {
            signals.push(SignalItem {
                id: signal_id("missing_conditions", &record.id),
                kind: "missing_conditions".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: record.finding_id.clone(),
                },
                reason: "Finding has no declared condition boundary.".to_string(),
                recommended_action:
                    "Add the species, model system, assay, comparator, endpoint, or scope that bounds the finding."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec!["A finding without conditions is incomplete frontier state.".to_string()],
            });
        }

        if record.comparator_status == "missing_or_unclear"
            && (record.exposure_or_efficacy == "efficacy" || record.exposure_or_efficacy == "both")
        {
            signals.push(SignalItem {
                id: signal_id("missing_comparator", &record.id),
                kind: "missing_comparator".into(),
                severity: "info".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: record.finding_id.clone(),
                },
                reason: "Condition record does not declare a comparator or baseline.".to_string(),
                recommended_action:
                    "Review whether the evidence supports the asserted direction without a declared comparator."
                        .to_string(),
                blocks: vec![],
                caveats: vec![
                    "Comparator absence is a review signal, not automatic disproof.".to_string(),
                ],
            });
        }

        if record.exposure_or_efficacy == "both" {
            signals.push(SignalItem {
                id: signal_id("exposure_efficacy_overgeneralization", &record.id),
                kind: "condition_loss_risk".into(),
                severity: "info".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: record.finding_id.clone(),
                },
                reason: "Exposure and efficacy language appear in the same condition boundary."
                    .to_string(),
                recommended_action:
                    "Keep exposure, functional delivery, and therapeutic efficacy separate unless the source directly supports the broader claim."
                        .to_string(),
                blocks: vec![],
                caveats: vec![
                    "Vela flags possible overgeneralization; reviewers decide the final scope."
                        .to_string(),
                ],
            });
        }

        if record.translation_scope == "animal_model"
            && record
                .caveats
                .iter()
                .any(|caveat| caveat.contains("human translation"))
        {
            signals.push(SignalItem {
                id: signal_id("mouse_human_translation_risk", &record.id),
                kind: "condition_loss_risk".into(),
                severity: "info".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: record.finding_id.clone(),
                },
                reason: "Animal-model evidence is adjacent to human translation language."
                    .to_string(),
                recommended_action:
                    "Preserve the animal-model scope unless human data are explicitly attached."
                        .to_string(),
                blocks: vec![],
                caveats: vec![
                    "Mouse or animal evidence should not silently become a human claim."
                        .to_string(),
                ],
            });
        }
    }

    // Build a set of finding IDs that have at least one evidence atom
    // attached. Used by the source-grounding doctrine invariant below.
    let evidence_grounded: BTreeSet<&str> = projection
        .evidence_atoms
        .iter()
        .map(|atom| atom.finding_id.as_str())
        .collect();

    for finding in &frontier.findings {
        if finding.provenance.doi.is_none() && finding.provenance.title.trim().is_empty() {
            signals.push(SignalItem {
                id: signal_id("weak_provenance", &finding.id),
                kind: "weak_provenance".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding lacks DOI, PMID, and source title fallback.".to_string(),
                recommended_action:
                    "Add source metadata or mark the finding as unresolved before proof export."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec!["Provenance is separate from confidence.".to_string()],
            });
        }

        // Doctrine line 3: a finding without conditions is incomplete.
        // Strict check blocker when conditions.text is empty AND the finding
        // is not theoretical (theoretical findings can be scope-free by nature).
        if finding.conditions.text.trim().is_empty()
            && finding.assertion.assertion_type != "theoretical"
            && !finding.flags.retracted
        {
            signals.push(SignalItem {
                id: signal_id("conditions_undeclared", &finding.id),
                kind: "conditions_undeclared".into(),
                severity: "error".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason:
                    "Finding has no condition text and no scope flag (in_vivo/in_vitro/human_data/clinical_trial)."
                        .to_string(),
                recommended_action:
                    "Declare at least one scope flag and condition text, or mark the finding theoretical."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "A finding without conditions is doctrinally incomplete state."
                        .to_string(),
                ],
            });
        }

        // Doctrine line 4: a result without provenance is not evidence.
        // Strict-check blocker when an active finding has no evidence atom.
        if !finding.flags.retracted && !evidence_grounded.contains(finding.id.as_str()) {
            signals.push(SignalItem {
                id: signal_id("evidence_atom_missing", &finding.id),
                kind: "evidence_atom_missing".into(),
                severity: "error".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason:
                    "Active finding has no materialized evidence atom in the source-evidence map."
                        .to_string(),
                recommended_action:
                    "Land explicit evidence as Receipt v1, or regenerate derived views with `vela frontier materialize`."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "A citation alone is not evidence in the v0.3 substrate.".to_string(),
                ],
            });
        }

        // Doctrine line 5: an agent trace is not truth.
        // Strict-check blocker when source_type implies the claim came from
        // a non-peer-reviewed source (model_output, expert_assertion,
        // agent_trace) AND the finding has not been reviewed.
        let agent_typed = matches!(
            finding.provenance.source_type.as_str(),
            "model_output" | "expert_assertion" | "agent_trace"
        );
        let has_review = finding
            .provenance
            .review
            .as_ref()
            .is_some_and(|r| r.reviewed)
            || finding.flags.review_state.is_some()
            || reviewed_finding_ids.contains(finding.id.as_str());
        if agent_typed && !has_review && !finding.flags.gap && !finding.flags.retracted {
            signals.push(SignalItem {
                id: signal_id("agent_typed_unreviewed", &finding.id),
                kind: "agent_typed_unreviewed".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: format!(
                    "Source type '{}' requires explicit review before strict acceptance.",
                    finding.provenance.source_type
                ),
                recommended_action:
                    "Decide a review verdict for this finding in `vela sign`, or flag it as gap before strict use."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "Agent traces, expert assertions, and model outputs are sources, not truth."
                        .to_string(),
                ],
            });
        }

        let theoretical_catalogue_record = is_theoretical_catalogue_record(finding);
        if finding.conditions.text.trim().is_empty() && !theoretical_catalogue_record {
            signals.push(SignalItem {
                id: signal_id("missing_conditions", &finding.id),
                kind: "missing_conditions".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding has no explicit condition boundary.".to_string(),
                recommended_action:
                    "Add species, model system, assay, regimen, population, or scope conditions."
                        .to_string(),
                blocks: vec!["proof_ready".to_string()],
                caveats: vec![
                    "Condition loss is a common source of overgeneralized scientific claims."
                        .to_string(),
                ],
            });
        }

        if finding.conditions.text.trim().is_empty()
            && !theoretical_catalogue_record
            && contains_condition_sensitive_claim(finding)
        {
            signals.push(SignalItem {
                id: signal_id("condition_loss_risk", &finding.id),
                kind: "condition_loss_risk".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding uses condition-sensitive language without explicit condition boundaries."
                    .to_string(),
                recommended_action:
                    "Separate exposure, efficacy, species, assay, payload, endpoint, and translation scope."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "Vela should flag overgeneralization rather than smooth it into a summary."
                        .to_string(),
                ],
            });
        }

        if finding
            .assertion
            .entities
            .iter()
            .any(|entity| entity.needs_review)
        {
            signals.push(SignalItem {
                id: signal_id("needs_human_review", &finding.id),
                kind: "needs_human_review".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding contains unresolved or low-confidence entity resolution."
                    .to_string(),
                recommended_action:
                    "Review entity names, types, identifiers, and source grounding before proof use."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec!["Entity review status is separate from assertion confidence.".to_string()],
            });
        }

        if finding.provenance.extraction.method.contains("fallback")
            || finding.provenance.extraction.method.contains("rough")
            || finding.provenance.extraction.method.contains("abstract")
        {
            signals.push(SignalItem {
                id: signal_id("rough_source_extraction", &finding.id),
                kind: "rough_source_extraction".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: format!(
                    "Finding was produced by extraction mode '{}'.",
                    finding.provenance.extraction.method
                ),
                recommended_action:
                    "Inspect the source text and mark caveats or review status before treating this as durable state."
                        .to_string(),
                blocks: vec!["proof_ready".to_string()],
                caveats: vec![
                    "Rough extraction can be useful as a review lead, not as a scientific conclusion."
                        .to_string(),
                ],
            });
        }

        if matches!(
            finding.provenance.source_type.as_str(),
            "model_output" | "summary" | "synthesis"
        ) && !reviewed_finding_ids.contains(finding.id.as_str())
        {
            signals.push(SignalItem {
                id: signal_id("synthesis_used_as_source", &finding.id),
                kind: "synthesis_used_as_source".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding provenance indicates synthesized text or model output as source."
                    .to_string(),
                recommended_action:
                    "Trace this finding back to primary source evidence or mark it as a review lead."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "Derived synthesis should not silently become primary evidence.".to_string(),
                ],
            });
        }

        if finding.flags.contested && finding.confidence.score >= 0.8 {
            signals.push(SignalItem {
                id: signal_id("contested_high_confidence", &finding.id),
                kind: "contested_high_confidence".into(),
                severity: "warning".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: "Finding is contested while carrying high confidence.".to_string(),
                recommended_action:
                    "Review contradiction links, provenance, and confidence components."
                        .to_string(),
                blocks: vec!["proof_ready".to_string()],
                caveats: vec![
                    "Candidate tensions are review surfaces, not definitive contradictions."
                        .to_string(),
                ],
            });
        }
    }

    // Phase N (v0.4): provenance authority. `Project.sources` is
    // canonical; `FindingBundle.provenance` is a denormalized cache.
    // Drift between the two is a strict-mode failure — the source
    // record wins, and the finding must be rewritten via
    // a provenance resync (retired `normalize` verb; the check remains).
    let mut by_doi: BTreeMap<String, &vela_protocol::sources::SourceRecord> = BTreeMap::new();
    let mut by_pmid: BTreeMap<String, &vela_protocol::sources::SourceRecord> = BTreeMap::new();
    let mut duplicate_dois: BTreeSet<String> = BTreeSet::new();
    let mut duplicate_pmids: BTreeSet<String> = BTreeSet::new();
    for source in &frontier.sources {
        if let Some(doi) = source.doi.as_deref() {
            let key = doi.to_lowercase();
            if by_doi.insert(key.clone(), source).is_some() {
                duplicate_dois.insert(key);
            }
        }
        if let Some(pmid) = source.pmid.as_deref() {
            let key = pmid.to_string();
            if by_pmid.insert(key.clone(), source).is_some() {
                duplicate_pmids.insert(key);
            }
        }
    }
    for key in &duplicate_dois {
        by_doi.remove(key);
    }
    for key in &duplicate_pmids {
        by_pmid.remove(key);
    }
    for finding in &frontier.findings {
        if finding.flags.retracted {
            continue;
        }
        let source = finding
            .provenance
            .doi
            .as_deref()
            .map(str::to_lowercase)
            .and_then(|k| by_doi.get(&k).copied());
        let Some(source) = source else { continue };

        let mut diffs: Vec<String> = Vec::new();
        if !source.title.is_empty() && source.title != finding.provenance.title {
            diffs.push(format!(
                "title differs (source='{}', cached='{}')",
                truncate(&source.title, 60),
                truncate(&finding.provenance.title, 60)
            ));
        }
        if source.year.is_some() && source.year != finding.provenance.year {
            diffs.push(format!(
                "year differs (source={:?}, cached={:?})",
                source.year, finding.provenance.year
            ));
        }
        if !diffs.is_empty() {
            signals.push(SignalItem {
                id: signal_id("provenance_drift", &finding.id),
                kind: "provenance_drift".into(),
                severity: "error".to_string(),
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: finding.id.clone(),
                },
                reason: format!(
                    "Cached finding.provenance disagrees with canonical source: {}",
                    diffs.join("; ")
                ),
                recommended_action:
                    "Regenerate finding.provenance from the canonical SourceRecord by landing corrected provenance through Receipt v1, then re-materialize."
                        .to_string(),
                blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                caveats: vec![
                    "sources is the authority; provenance is the cache."
                        .to_string(),
                ],
            });
        }
    }

    for removed in &actor_registration.removed_activation_event_ids {
        signals.push(SignalItem {
            id: signal_id("actor_registration_anchor_invalid", removed),
            kind: "actor_registration_anchor_invalid".into(),
            severity: "error".to_string(),
            target: SignalTarget {
                r#type: "event".to_string(),
                id: removed.clone(),
            },
            reason: format!(
                "A signed actor-registration activation present in ancestor Git history was removed from the checked descendant: {removed}."
            ),
            recommended_action:
                "Restore the activation event and matching actor record from the descendant history. Removing an activated boundary never grants an exemption."
                    .to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec![
                "A fresh clone of force-pushed history cannot infer withheld ancestors; pin trusted roots when consuming a frontier."
                    .to_string(),
            ],
        });
    }
    for boundary in actor_registration.boundaries.values() {
        let (kind, recommended_action) = match boundary.outcome {
            BoundaryOutcome::Valid => continue,
            BoundaryOutcome::Invalid => (
                "actor_registration_anchor_invalid",
                "Restore the exact signed activation, actor registry, anchored Git objects, and immutable event cores. Invalid activation grants no legacy exemption.",
            ),
            BoundaryOutcome::Unavailable => (
                "actor_registration_anchor_unavailable",
                "Fetch a complete clone or Git bundle containing the signed anchor. Missing history grants no legacy exemption.",
            ),
        };
        signals.push(SignalItem {
            id: signal_id(kind, &boundary.activation_event_id),
            kind: kind.to_string(),
            severity: "error".to_string(),
            target: SignalTarget {
                r#type: "event".to_string(),
                id: boundary.activation_event_id.clone(),
            },
            reason: format!(
                "Actor-registration boundary for '{}' cannot be used: {}.",
                boundary.actor_id,
                boundary
                    .reason
                    .as_deref()
                    .unwrap_or("the boundary did not validate")
            ),
            recommended_action: recommended_action.to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec![
                "An invalid or unavailable activation never suppresses timeless actor-signature checks."
                    .to_string(),
            ],
        });
    }

    // Registered actors sign their canonical events. A valid temporal
    // activation changes only the treatment of exact anchor members:
    // anchored unsigned events remain legacy and unauthenticated, while every
    // event absent from the anchor keeps the ordinary key requirement.
    if !frontier.actors.is_empty() {
        let registry: BTreeMap<&str, &vela_protocol::sign::ActorRecord> = frontier
            .actors
            .iter()
            .map(|actor| (actor.id.as_str(), actor))
            .collect();
        for event in &frontier.events {
            if event.actor.r#type != "human" {
                continue;
            }
            let Some(actor_record) = registry.get(event.actor.id.as_str()) else {
                continue;
            };
            // v0.127: A7 mitigation. If the actor's key is revoked at
            // or before this event's timestamp, the signature is
            // rejected regardless of whether it would otherwise
            // verify. Historical signatures (events with timestamp
            // strictly before revoked_at) remain valid: the
            // substrate does not retroactively invalidate canonical
            // history.
            if actor_record.is_revoked_at(event.timestamp.as_str()) {
                signals.push(SignalItem {
                    id: signal_id("post_revocation_signature", &event.id),
                    kind: "post_revocation_signature".into(),
                    severity: "error".to_string(),
                    target: SignalTarget {
                        r#type: "event".to_string(),
                        id: event.id.clone(),
                    },
                    reason: format!(
                        "Event {} carries a signature from actor '{}' whose key was revoked at {} (event timestamp {}).",
                        event.id,
                        event.actor.id,
                        actor_record.revoked_at.as_deref().unwrap_or("?"),
                        event.timestamp
                    ),
                    recommended_action:
                        "Reject this event. The signing key was revoked at-or-before the event timestamp; verify the rotation chain and re-sign under the current actor key."
                            .to_string(),
                    blocks: vec!["strict_check".to_string()],
                    caveats: Vec::new(),
                });
                continue;
            }
            let pubkey = actor_record.public_key.as_str();
            let anchored = actor_registration
                .boundary(event.actor.id.as_str())
                .filter(|boundary| boundary.outcome == BoundaryOutcome::Valid)
                .and_then(|boundary| boundary.anchored_events.get(&event.id));
            if let Some(anchored) = anchored {
                if anchored.signature_was_present {
                    let valid = event.signature.is_some()
                        && vela_protocol::sign::verify_event_signature(event, pubkey)
                            .unwrap_or(false);
                    if !valid {
                        signals.push(SignalItem {
                            id: signal_id("pre_registration_signature_lost", &event.id),
                            kind: "pre_registration_signature_lost".into(),
                            severity: "error".to_string(),
                            target: SignalTarget {
                                r#type: "event".to_string(),
                                id: event.id.clone(),
                            },
                            reason: format!(
                                "Anchored event {} from '{}' carried a signature at activation but no longer has a valid signature.",
                                event.id, event.actor.id
                            ),
                            recommended_action:
                                "Restore a valid signature under the activated actor key. Temporal registration never permits signature stripping."
                                    .to_string(),
                            blocks: vec![
                                "strict_check".to_string(),
                                "proof_ready".to_string(),
                            ],
                            caveats: vec![
                                "The event content address excludes signatures, but authenticated history must remain authenticated."
                                    .to_string(),
                            ],
                        });
                    }
                    continue;
                }
                match event.signature.as_deref() {
                    None => {
                        signals.push(SignalItem {
                            id: signal_id(
                                "pre_registration_unsigned_actor_event",
                                &event.id,
                            ),
                            kind: "pre_registration_unsigned_actor_event".into(),
                            severity: "info".to_string(),
                            target: SignalTarget {
                                r#type: "event".to_string(),
                                id: event.id.clone(),
                            },
                            reason: format!(
                                "Event {} from '{}' is an exact unsigned member of the signed pre-registration anchor. It remains legacy and unauthenticated.",
                                event.id, event.actor.id
                            ),
                            recommended_action:
                                "No rewrite is required. Preserve the historical bytes; future matching events require a valid actor signature."
                                    .to_string(),
                            blocks: vec![],
                            caveats: vec![
                                "Anchor membership does not attribute this event to the activated key holder."
                                    .to_string(),
                            ],
                        });
                        continue;
                    }
                    Some(_) => {
                        if vela_protocol::sign::verify_event_signature(event, pubkey)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                    }
                }
            }
            let invalid = match event.signature.as_deref() {
                None => Some("missing".to_string()),
                Some(_) => match vela_protocol::sign::verify_event_signature(event, pubkey) {
                    Ok(true) => None,
                    Ok(false) => Some("does not verify".to_string()),
                    Err(err) => Some(err),
                },
            };
            if let Some(reason) = invalid {
                signals.push(SignalItem {
                    id: signal_id("unsigned_registered_actor", &event.id),
                    kind: "unsigned_registered_actor".into(),
                    severity: "error".to_string(),
                    target: SignalTarget {
                        r#type: "event".to_string(),
                        id: event.id.clone(),
                    },
                    reason: format!(
                        "Event {} from registered actor '{}' has invalid signature: {reason}.",
                        event.id, event.actor.id
                    ),
                    recommended_action:
                        "Sign the event with the registered Ed25519 key before strict acceptance."
                            .to_string(),
                    blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
                    caveats: vec![
                        "Registered actors are bound to their public key; unsigned writes break that binding."
                            .to_string(),
                    ],
                });
            }
        }
    }

    let proposal_summary = proposals::summary(frontier);
    for duplicate in &proposal_summary.duplicate_ids {
        signals.push(SignalItem {
            id: signal_id("proposal_conflict", duplicate),
            kind: "proposal_conflict".into(),
            severity: "error".to_string(),
            target: SignalTarget {
                r#type: "frontier".to_string(),
                id: frontier.project.name.clone(),
            },
            reason: format!("Duplicate proposal id detected: {duplicate}."),
            recommended_action: "Remove or rename the duplicate proposal before applying writes."
                .to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec![],
        });
    }
    for target in &proposal_summary.invalid_targets {
        signals.push(SignalItem {
            id: signal_id("proposal_conflict", target),
            kind: "proposal_conflict".into(),
            severity: "error".to_string(),
            target: SignalTarget {
                r#type: "finding".to_string(),
                id: target.clone(),
            },
            reason: format!("Proposal target does not exist in frontier state: {target}."),
            recommended_action:
                "Fix the proposal target or remove the orphan proposal before applying writes."
                    .to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec![],
        });
    }
    for proposal in frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
    {
        signals.push(SignalItem {
            id: signal_id("pending_proposal_review", &proposal.id),
            kind: "pending_proposal_review".into(),
            // Info, deliberately: pending review is the sign queue
            // (`vela sign`), the normal state of a living frontier — not
            // integrity debt. Strict measures whether the tree IS the
            // signed state, not whether work is waiting.
            severity: "info".to_string(),
            target: SignalTarget {
                r#type: proposal.target.r#type.clone(),
                id: proposal.target.id.clone(),
            },
            reason: format!(
                "Pending {} proposal awaits a human key (`vela sign`).",
                proposal.kind
            ),
            recommended_action: "Decide it in `vela sign`.".to_string(),
            // Blocks NOTHING: a pending proposal is not active frontier
            // state (the caveat below) and therefore cannot be integrity
            // debt on the state that IS active. Pending review is the
            // inbox; strict and proof_ready measure the signed state.
            blocks: vec![],
            caveats: vec!["Pending proposals are not active frontier state.".to_string()],
        });
    }
    for proposal in frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "applied")
    {
        signals.push(SignalItem {
            id: signal_id("proposal_applied", &proposal.id),
            kind: "proposal_applied".into(),
            severity: "info".to_string(),
            target: SignalTarget {
                r#type: proposal.target.r#type.clone(),
                id: proposal.target.id.clone(),
            },
            reason: format!("Applied proposal {} changed frontier state.", proposal.id),
            recommended_action:
                "Re-export proof artifacts if this proposal materially changes what reviewers should inspect."
                    .to_string(),
            blocks: vec![],
            caveats: vec![],
        });
    }
    for proposal in frontier.proposals.iter().filter(|proposal| {
        matches!(proposal.status.as_str(), "accepted" | "applied")
            && proposal
                .reviewed_by
                .as_deref()
                .is_none_or(proposals::is_placeholder_reviewer)
    }) {
        signals.push(SignalItem {
            id: signal_id("reviewer_identity_missing", &proposal.id),
            kind: "reviewer_identity_missing".into(),
            severity: "error".to_string(),
            target: SignalTarget {
                r#type: proposal.target.r#type.clone(),
                id: proposal.target.id.clone(),
            },
            reason: format!(
                "Accepted or applied proposal {} lacks a stable named reviewer identity.",
                proposal.id
            ),
            recommended_action:
                "Re-accept the proposal with a stable named reviewer id before strict proof use."
                    .to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec![
                "Placeholder reviewer ids do not satisfy the v0 trust boundary.".to_string(),
            ],
        });
    }
    if frontier.proof_state.latest_packet.status == "stale" {
        signals.push(SignalItem {
            id: signal_id("stale_proof_packet", &frontier.project.name),
            kind: "stale_proof_packet".into(),
            severity: "warning".to_string(),
            target: SignalTarget {
                r#type: "frontier".to_string(),
                id: frontier.project.name.clone(),
            },
            reason: frontier
                .proof_state
                .stale_reason
                .clone()
                .unwrap_or_else(|| "Proof packet is stale relative to current frontier state.".to_string()),
            recommended_action:
                "Run `vela proof` again to export a packet that matches the current frontier snapshot."
                    .to_string(),
            blocks: vec!["strict_check".to_string(), "proof_ready".to_string()],
            caveats: vec!["Packet validation can still pass for stale but internally consistent packets.".to_string()],
        });
    }

    let review_queue = build_review_queue(frontier, &signals);
    let proof_readiness = build_proof_readiness(&signals);

    SignalReport {
        schema: "vela.signals.v0".to_string(),
        frontier: frontier.project.name.clone(),
        signals,
        review_queue,
        proof_readiness,
    }
}

pub fn quality_table(frontier: &Project, report: &SignalReport) -> Value {
    let mut by_kind = BTreeMap::<String, usize>::new();
    let mut by_severity = BTreeMap::<String, usize>::new();
    let proposal_summary = proposals::summary(frontier);
    for signal in &report.signals {
        *by_kind.entry(signal.kind.clone()).or_default() += 1;
        *by_severity.entry(signal.severity.clone()).or_default() += 1;
    }

    json!({
        "schema": "vela.quality-table.v0",
        "frontier": frontier.project.name,
        "stats": frontier.stats,
        "event_log": {
            "events": frontier.events.len(),
            "review_events_projection": frontier.review_events.len(),
            "confidence_updates_projection": frontier.confidence_updates.len(),
        },
        "signals": {
            "total": report.signals.len(),
            "by_kind": by_kind,
            "by_severity": by_severity,
        },
        "review_queue": {
            "items": report.review_queue.len(),
            "high_priority": report.review_queue.iter().filter(|item| item.priority == "high").count(),
        },
        "proposals": proposal_summary,
        "proof_state": frontier.proof_state,
        "proof_readiness": report.proof_readiness,
        "caveats": [
            "Signals are derived from frontier state and should be recomputed after edits.",
            "Candidate gaps, bridges, and tensions require human review.",
            "A clean quality table is not proof of scientific truth."
        ],
    })
}

pub fn ro_crate_metadata(frontier: &Project, files: &[String]) -> Value {
    let graph_files: Vec<Value> = files
        .iter()
        .map(|path| {
            json!({
                "@id": path,
                "@type": "File",
                "name": path,
            })
        })
        .collect();

    let mut graph = vec![
        json!({
            "@id": "ro-crate-metadata.jsonld",
            "@type": "CreativeWork",
            "conformsTo": {"@id": "https://w3id.org/ro/crate/1.2"},
            "about": {"@id": "./"}
        }),
        json!({
            "@id": "./",
            "@type": "Dataset",
            "name": format!("{} proof packet", frontier.project.name),
            "description": frontier.project.description,
            "dateCreated": frontier.project.compiled_at,
            "conformsTo": {"@id": project::VELA_SCHEMA_URL},
            "hasPart": files.iter().map(|path| json!({"@id": path})).collect::<Vec<_>>()
        }),
    ];
    graph.extend(graph_files);
    graph.extend(frontier.artifacts.iter().map(|artifact| {
        json!({
            "@id": artifact.id,
            "@type": "CreativeWork",
            "name": artifact.name,
            "encodingFormat": artifact.media_type,
            "sha256": artifact.content_hash,
            "url": artifact.source_url.as_ref().or(artifact.locator.as_ref()),
            "license": artifact.license,
            "retracted": artifact.retracted,
        })
    }));

    json!({
        "@context": "https://w3id.org/ro/crate/1.2/context",
        "@graph": graph,
    })
}

fn build_review_queue(frontier: &Project, signals: &[SignalItem]) -> Vec<ReviewQueueItem> {
    let link_counts = frontier
        .findings
        .iter()
        .map(|finding| {
            let outgoing = finding.links.len() as u32;
            let incoming = frontier
                .findings
                .iter()
                .flat_map(|other| &other.links)
                .filter(|link| link.target == finding.id)
                .count() as u32;
            (finding.id.clone(), outgoing + incoming)
        })
        .collect::<BTreeMap<_, _>>();

    let mut by_target = BTreeMap::<String, Vec<&SignalItem>>::new();
    for signal in signals {
        // Info-severity signals are visibility, not review debt: they never
        // enqueue. Without this, `proposal_applied` alone kept the queue
        // non-empty forever, so `check --strict` could not pass on ANY
        // frontier that had ever applied a proposal — strict was measuring
        // activity, not outstanding review.
        if signal.severity == "info" {
            continue;
        }
        if signal.target.r#type == "finding" {
            by_target
                .entry(signal.target.id.clone())
                .or_default()
                .push(signal);
        }
    }

    let mut queue = by_target
        .into_iter()
        .map(|(target_id, grouped)| {
            let signal_score = grouped
                .iter()
                .map(|signal| signal_weight(signal))
                .sum::<u32>();
            let centrality_score = link_counts.get(&target_id).copied().unwrap_or(0).min(25);
            let priority_score = signal_score + centrality_score;
            let priority = if grouped
                .iter()
                .any(|signal| signal.blocks.iter().any(|block| block == "strict_check"))
            {
                "high"
            } else if grouped
                .iter()
                .any(|signal| signal.blocks.iter().any(|block| block == "proof_ready"))
            {
                "medium"
            } else {
                "low"
            };
            ReviewQueueItem {
                id: format!("rq_{}", target_id.trim_start_matches("vf_")),
                priority: priority.to_string(),
                priority_score,
                target: SignalTarget {
                    r#type: "finding".to_string(),
                    id: target_id,
                },
                signal_ids: grouped.iter().map(|signal| signal.id.clone()).collect(),
                reasons: grouped.iter().map(|signal| signal.reason.clone()).collect(),
                recommended_action: grouped
                    .first()
                    .map(|signal| signal.recommended_action.clone())
                    .unwrap_or_else(|| "Review finding state.".to_string()),
            }
        })
        .collect::<Vec<_>>();
    queue.sort_by(|a, b| {
        b.priority_score
            .cmp(&a.priority_score)
            .then_with(|| a.target.id.cmp(&b.target.id))
    });
    queue
}

fn signal_weight(signal: &SignalItem) -> u32 {
    let severity = match signal.severity.as_str() {
        "error" => 100,
        "warning" => 30,
        _ => 10,
    };
    let kind = match signal.kind.as_str() {
        "check_error" => 100,
        "contested_high_confidence" => 70,
        "proposal_conflict" => 80,
        "pending_proposal_review" => 50,
        "weak_provenance" => 45,
        _ => 10,
    };
    let blocker = if signal.blocks.iter().any(|block| block == "strict_check") {
        30
    } else if signal.blocks.iter().any(|block| block == "proof_ready") {
        15
    } else {
        0
    };
    severity + kind + blocker
}

fn build_proof_readiness(signals: &[SignalItem]) -> ProofReadiness {
    let blockers = signals
        .iter()
        .filter(|signal| signal.blocks.iter().any(|block| block == "proof_ready"))
        .count();
    let warnings = signals
        .iter()
        .filter(|signal| signal.severity == "warning")
        .count();
    ProofReadiness {
        status: if blockers == 0 {
            "ready".to_string()
        } else {
            "needs_review".to_string()
        },
        blockers,
        warnings,
        caveats: vec![
            "Proof readiness means packet state is reviewable, not scientifically settled."
                .to_string(),
        ],
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}…")
    }
}

fn signal_id(kind: &str, finding_id: &str) -> String {
    format!("sig_{kind}_{}", finding_id.trim_start_matches("vf_"))
}

fn is_theoretical_catalogue_record(finding: &vela_protocol::bundle::FindingBundle) -> bool {
    finding.provenance.source_type == "database_record"
        && finding.provenance.extraction.method == "database_import"
        && finding.assertion.assertion_type == "theoretical"
        && finding.evidence.evidence_type == "theoretical"
        && finding.evidence.method == "erdos_deep"
        && finding.evidence.model_system.eq_ignore_ascii_case("n/a")
        && finding
            .provenance
            .title
            .strip_prefix("erdos_deep:")
            .is_some_and(|problem| {
                !problem.is_empty() && problem.bytes().all(|byte| byte.is_ascii_digit())
            })
        && !has_biomedical_context(finding)
}

fn contains_condition_sensitive_claim(finding: &vela_protocol::bundle::FindingBundle) -> bool {
    let lower = finding.assertion.text.to_ascii_lowercase();
    [
        "delivery",
        "efficacy",
        "therapeutic",
        "clinical",
        "human",
        "mouse",
        "mice",
        "assay",
        "endpoint",
        "payload",
        "exposure",
    ]
    .iter()
    .any(|term| lower.contains(term))
        || (lower.contains("translation") && has_biomedical_translation_context(finding))
}

fn has_biomedical_translation_context(finding: &vela_protocol::bundle::FindingBundle) -> bool {
    has_biomedical_context(finding)
}

fn has_biomedical_context(finding: &vela_protocol::bundle::FindingBundle) -> bool {
    if finding.assertion.entities.iter().any(|entity| {
        matches!(
            entity.entity_type.to_ascii_lowercase().as_str(),
            "protein"
                | "gene"
                | "disease"
                | "drug"
                | "compound"
                | "cell"
                | "organism"
                | "tissue"
                | "patient"
        )
    }) {
        return true;
    }
    let context = format!(
        "{} {} {}",
        finding.assertion.text, finding.evidence.model_system, finding.evidence.method
    )
    .to_ascii_lowercase();
    [
        "clinical",
        "patient",
        "human",
        "mouse",
        "mice",
        "animal",
        "in vitro",
        "in vivo",
        "cell",
        "therapeutic",
        "disease",
        "protein",
        "gene",
        "genomic",
        "dna",
        "rna",
        "ribosome",
        "drug",
        "toxicity",
        "adverse event",
        "efficacy",
    ]
    .iter()
    .any(|term| context.contains(term))
}

#[cfg(test)]
mod tests {
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, Evidence, FindingBundle, Flags, Provenance,
    };

    use super::*;

    fn minimal_finding(id: &str) -> FindingBundle {
        let assertion = Assertion {
            text: "LRP1 transport is altered in Alzheimer models.".to_string(),
            assertion_type: "mechanism".to_string(),
            entities: vec![],
            relation: None,
            direction: None,
            causal_claim: None,
            causal_evidence_grade: None,
        };
        let provenance = Provenance {
            source_type: "published_paper".to_string(),
            doi: None,
            url: None,
            title: String::new(),
            authors: vec![],
            year: Some(2020),
            license: None,
            publisher: None,
            funders: vec![],
            extraction: Default::default(),
            review: None,
            contributions: Vec::new(),
        };
        FindingBundle {
            id: id.to_string(),
            version: 1,
            previous_version: None,
            assertion,
            evidence: Evidence {
                evidence_type: "experimental".to_string(),
                model_system: "mouse".to_string(),
                method: "test".to_string(),
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            conditions: Conditions {
                text: String::new(),
                duration: None,
            },
            confidence: Confidence::raw(0.9, "test".to_string(), 0.9),
            provenance,
            flags: Flags {
                gap: false,
                negative_space: false,
                contested: true,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
                signature_threshold: None,
                jointly_accepted: false,
            },
            links: vec![],
            annotations: vec![],
            attachments: vec![],
            created: "2026-01-01T00:00:00Z".to_string(),
            updated: None,

            access_tier: vela_protocol::access_tier::AccessTier::Public,
        }
    }

    #[test]
    fn weak_and_contested_findings_emit_review_signals() {
        let frontier = project::assemble("test", vec![minimal_finding("vf_abc")], 1, 0, "test");
        let report = analyze(&frontier, &[]);
        assert!(report.signals.iter().any(|s| s.kind == "weak_provenance"));
        assert!(
            report
                .signals
                .iter()
                .any(|s| s.kind == "contested_high_confidence")
        );
        assert_eq!(report.review_queue.len(), 1);
    }

    #[test]
    fn erdos_database_imports_do_not_invent_empirical_condition_blockers() {
        for (finding_id, condition_id, problem) in [
            ("vf_00008dacb7640287", "vcnd_f3a9d1319a449e81", "1057"),
            ("vf_001ca4e62aaa125e", "vcnd_69697c99c4658b1c", "1018"),
            ("vf_0026e977242a051a", "vcnd_686c31cbec80687a", "703"),
            ("vf_0043888dee5bea76", "vcnd_3431d18d10bd7375", "647"),
            ("vf_00541e85275a706a", "vcnd_0e7f19e825b078dc", "699"),
        ] {
            let mut finding = minimal_finding(finding_id);
            finding.provenance.source_type = "database_record".to_string();
            finding.provenance.extraction.method = "database_import".to_string();
            finding.provenance.title = format!("erdos_deep:{problem}");
            finding.assertion.assertion_type = "theoretical".to_string();
            finding.evidence.evidence_type = "theoretical".to_string();
            finding.evidence.model_system = "n/a".to_string();
            finding.evidence.method = "erdos_deep".to_string();
            finding.assertion.text = "A catalogue status for an Erdős problem.".to_string();
            let frontier = project::assemble("erdos", vec![finding], 1, 0, "migrate_erdos");
            let report = analyze(&frontier, &[]);
            assert!(
                report
                    .signals
                    .iter()
                    .all(|signal| signal.id != format!("sig_missing_conditions_{condition_id}")),
                "{finding_id} should not manufacture an empirical condition blocker: {:?}",
                report.signals
            );
            assert!(
                report.signals.iter().all(|signal| {
                    signal.target.id != finding_id
                        || !matches!(
                            signal.kind.as_str(),
                            "missing_conditions" | "condition_loss_risk"
                        )
                }),
                "{finding_id} should carry catalogue provenance instead of condition debt"
            );
        }
    }

    #[test]
    fn mathematical_translation_property_is_not_biomedical_translation_risk() {
        let mut finding = minimal_finding("vf_af1c3ee8e0a0262c");
        finding.provenance.source_type = "published_paper".to_string();
        finding.provenance.extraction.method = "manual".to_string();
        finding.evidence.evidence_type = "theoretical".to_string();
        finding.evidence.model_system.clear();
        finding.evidence.method = "proof".to_string();
        finding.assertion.text =
            "The constructed set satisfies the translation property.".to_string();
        let frontier = project::assemble("erdos", vec![finding], 1, 0, "migrate_erdos");
        let report = analyze(&frontier, &[]);
        assert!(
            report
                .signals
                .iter()
                .all(|signal| signal.id != "sig_condition_loss_risk_af1c3ee8e0a0262c")
        );
        assert!(
            report
                .signals
                .iter()
                .any(|signal| signal.id == "sig_missing_conditions_af1c3ee8e0a0262c"),
            "non-catalogue theoretical claims still retain ordinary condition review"
        );
    }

    #[test]
    fn empirical_and_biomedical_condition_claims_remain_strict() {
        let mut finding = minimal_finding("vf_empirical");
        finding.assertion.text =
            "Mouse efficacy supports clinical translation to humans.".to_string();
        let frontier = project::assemble("biomed", vec![finding], 1, 0, "test");
        let report = analyze(&frontier, &[]);
        assert!(report.signals.iter().any(|signal| {
            signal.kind == "missing_conditions"
                && signal.blocks.iter().any(|block| block == "strict_check")
        }));
        assert!(report.signals.iter().any(|signal| {
            signal.kind == "condition_loss_risk"
                && signal.blocks.iter().any(|block| block == "strict_check")
        }));
    }

    #[test]
    fn producer_labels_cannot_disguise_biomedical_or_untyped_records_as_erdos_catalogue() {
        let mut biomedical = minimal_finding("vf_disguised_biomedical");
        biomedical.provenance.source_type = "database_record".to_string();
        biomedical.provenance.extraction.method = "database_import".to_string();
        biomedical.provenance.title = "erdos_deep:647".to_string();
        biomedical.assertion.assertion_type = "theoretical".to_string();
        biomedical.evidence.evidence_type = "theoretical".to_string();
        biomedical.evidence.model_system = "n/a".to_string();
        biomedical.evidence.method = "erdos_deep".to_string();
        biomedical.assertion.text =
            "Protein translation in patients changes therapeutic efficacy.".to_string();
        let report = analyze(
            &project::assemble("biomed", vec![biomedical], 1, 0, "test"),
            &[],
        );
        assert!(report.signals.iter().any(|signal| {
            signal.target.id == "vf_disguised_biomedical" && signal.kind == "missing_conditions"
        }));
        assert!(report.signals.iter().any(|signal| {
            signal.target.id == "vf_disguised_biomedical" && signal.kind == "condition_loss_risk"
        }));

        let mut untyped = minimal_finding("vf_disguised_untyped");
        untyped.provenance.source_type = "database_record".to_string();
        untyped.provenance.extraction.method = "database_import".to_string();
        untyped.provenance.title = "erdos_deep:647".to_string();
        untyped.evidence.evidence_type = "theoretical".to_string();
        untyped.evidence.model_system = "n/a".to_string();
        untyped.evidence.method = "erdos_deep".to_string();
        untyped.assertion.text = "An untyped catalogue-looking record.".to_string();
        let report = analyze(
            &project::assemble("catalogue", vec![untyped], 1, 0, "test"),
            &[],
        );
        assert!(report.signals.iter().any(|signal| {
            signal.target.id == "vf_disguised_untyped" && signal.kind == "missing_conditions"
        }));
    }
}
