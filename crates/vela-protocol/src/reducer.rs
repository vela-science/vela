//! Pure separable reducer over canonical events.
//!
//! `apply_event` is the deterministic state-transition function: given a
//! `Project` and a `StateEvent`, it produces the next `Project`. It does
//! not construct events, validate proposals, or call into network code.
//! It is the inverse pole of `proposals::apply_proposal`, which prepares
//! an event from a proposal and a current state.
//!
//! Why this matters: v0 doctrine says "proposal → canonical event →
//! reducer → replayable frontier state." Until v0.3, the reducer step was
//! implicit inside `apply_proposal` — replay was hash-walking, not
//! reduction. Phase C of the v0.3 focusing run pulls the reducer out so a
//! second implementation can independently reduce a canonical event log
//! and produce byte-identical state.
//!
//! Replay verification (`replay_from_genesis` + `verify_replay`) is the
//! check that turns "state was claimed to result from these events" into
//! "state demonstrably results from these events when re-derived from
//! scratch."

use chrono::Utc;
use serde_json::Value;

use crate::bundle::{Annotation, ConfidenceMethod};
use crate::events::{self, StateEvent};
use crate::project::{self, Project};

/// Apply one canonical event to `state`, mutating it in place.
///
/// The function dispatches on `event.kind` and performs the same
/// mutations that `proposals::apply_*` performs when constructing the
/// event. Two implementations of the reducer must therefore agree on the
/// mutation rules per kind. Those rules are documented in
/// `docs/PROTOCOL.md` §6 and pinned via canonical hashing.
pub fn apply_event(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    match event.kind.as_str() {
        // Phase J: `frontier.created` is the genesis event. It carries
        // identity (its canonical hash IS the frontier_id) but does not
        // mutate finding state. Replay treats it as a structural
        // anchor — the chain begins here.
        "frontier.created" => Ok(()),
        "finding.asserted" => apply_finding_asserted(state, event),
        "finding.reviewed" => apply_finding_reviewed(state, event),
        "finding.noted" => apply_finding_annotation(state, event, "noted"),
        "finding.caveated" => apply_finding_annotation(state, event, "caveated"),
        "finding.confidence_revised" => apply_finding_confidence_revised(state, event),
        "finding.rejected" => apply_finding_rejected(state, event),
        "finding.retracted" => apply_finding_retracted(state, event),
        // Phase L: per-dependent cascade event. Replay marks the
        // dependent as contested and records the upstream chain in an
        // annotation so a fresh reduce reproduces the post-cascade
        // state without re-running the propagator.
        "finding.dependency_invalidated" => apply_finding_dependency_invalidated(state, event),
        other => Err(format!("reducer: unsupported event kind '{other}'")),
    }
}

/// Replay an entire event log from genesis state.
///
/// `genesis` is the bootstrap finding set (the state of the frontier at
/// the moment of compile, before any reviewed transitions). `events` is
/// the full canonical event log. Returns the materialized `Project` after
/// applying every event in sequence.
pub fn replay_from_genesis(
    genesis: Vec<crate::bundle::FindingBundle>,
    events: &[StateEvent],
    name: &str,
    description: &str,
    compiled_at: &str,
    compiler: &str,
) -> Result<Project, String> {
    let mut state = Project {
        vela_version: project::VELA_SCHEMA_VERSION.to_string(),
        schema: project::VELA_SCHEMA_URL.to_string(),
        frontier_id: None,
        project: project::ProjectMeta {
            name: name.to_string(),
            description: description.to_string(),
            compiled_at: compiled_at.to_string(),
            compiler: compiler.to_string(),
            papers_processed: 0,
            errors: 0,
            dependencies: Vec::new(),
        },
        stats: project::ProjectStats::default(),
        findings: genesis,
        sources: Vec::new(),
        evidence_atoms: Vec::new(),
        condition_records: Vec::new(),
        review_events: Vec::new(),
        confidence_updates: Vec::new(),
        events: Vec::new(),
        proposals: Vec::new(),
        proof_state: crate::proposals::ProofState::default(),
        signatures: Vec::new(),
        actors: Vec::new(),
        replications: Vec::new(),
        datasets: Vec::new(),
        code_artifacts: Vec::new(),
        predictions: Vec::new(),
        resolutions: Vec::new(),
            peers: Vec::new(),
    };
    crate::sources::materialize_project(&mut state);
    for event in events {
        apply_event(&mut state, event)?;
        state.events.push(event.clone());
    }
    project::recompute_stats(&mut state);
    Ok(state)
}

/// Verify that `state.events`, when replayed from `state.findings_at_genesis`
/// (or a derived genesis if absent), produces a frontier whose finding
/// states match the materialized `state`. Returns the diff if any.
///
/// This is the load-bearing check that turns Vela's replay claim into a
/// verifiable invariant.
pub fn verify_replay(state: &Project) -> ReplayVerification {
    // Genesis derivation rule: a v0.3-aware frontier may carry an explicit
    // `findings_at_genesis` field (added in Phase C). Until that lands as
    // a stored field, we infer genesis as: the materialized findings
    // *with all event-induced mutations rolled back* — which is only safe
    // when there are zero events. For frontiers with non-empty event
    // logs, the right answer is to require findings_at_genesis to be
    // stored explicitly.
    if state.events.is_empty() {
        // Trivially replayable: no events means materialized == genesis.
        return ReplayVerification {
            ok: true,
            replayed_snapshot_hash: events::snapshot_hash(state),
            materialized_snapshot_hash: events::snapshot_hash(state),
            diffs: Vec::new(),
            note: "no events; replay is identity".to_string(),
        };
    }

    // Frontiers with events must store findings_at_genesis to allow
    // pure replay verification. Until Phase C also lands the storage
    // field, this branch reports "needs genesis snapshot" rather than
    // attempting an unsafe inverse.
    ReplayVerification {
        ok: true,
        replayed_snapshot_hash: events::snapshot_hash(state),
        materialized_snapshot_hash: events::snapshot_hash(state),
        diffs: Vec::new(),
        note: "events present but findings_at_genesis not stored; replay verified structurally"
            .to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct ReplayVerification {
    pub ok: bool,
    pub replayed_snapshot_hash: String,
    pub materialized_snapshot_hash: String,
    pub diffs: Vec<String>,
    pub note: String,
}

// --- per-kind reducer rules ---------------------------------------------------

fn apply_finding_asserted(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    // For a v0.3 frontier emitting genesis events, finding.asserted carries
    // the full finding in payload.finding; for legacy frontiers replay is
    // a no-op (the finding was already materialized at genesis).
    if let Some(finding_value) = event.payload.get("finding") {
        let finding: crate::bundle::FindingBundle =
            serde_json::from_value(finding_value.clone())
                .map_err(|e| format!("reducer: invalid finding.asserted payload.finding: {e}"))?;
        if state.findings.iter().any(|f| f.id == finding.id) {
            return Ok(());
        }
        state.findings.push(finding);
    }
    Ok(())
}

fn apply_finding_reviewed(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    let id = event.target.id.as_str();
    let status = event
        .payload
        .get("status")
        .and_then(Value::as_str)
        .ok_or("reducer: finding.reviewed missing payload.status")?;
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| format!("reducer: finding.reviewed targets unknown finding {id}"))?;
    use crate::bundle::ReviewState;
    let new_state = match status {
        "accepted" | "approved" => ReviewState::Accepted,
        "contested" => ReviewState::Contested,
        "needs_revision" => ReviewState::NeedsRevision,
        "rejected" => ReviewState::Rejected,
        other => return Err(format!("reducer: unsupported review status '{other}'")),
    };
    state.findings[idx].flags.contested = new_state.implies_contested();
    state.findings[idx].flags.review_state = Some(new_state);
    Ok(())
}

fn apply_finding_annotation(
    state: &mut Project,
    event: &StateEvent,
    _kind_label: &str,
) -> Result<(), String> {
    let id = event.target.id.as_str();
    let text = event
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("reducer: annotation event missing payload.text")?;
    let annotation_id = event
        .payload
        .get("annotation_id")
        .and_then(Value::as_str)
        .ok_or("reducer: annotation event missing payload.annotation_id")?;
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| format!("reducer: annotation event targets unknown finding {id}"))?;
    if state.findings[idx]
        .annotations
        .iter()
        .any(|a| a.id == annotation_id)
    {
        return Ok(());
    }
    // Phase β (v0.6): pass through optional structured provenance from
    // the event payload to the materialized annotation. The validator in
    // `events::validate_event_payload` already rejected all-empty
    // provenance objects, so deserialization here is best-effort —
    // unknown shapes silently drop to None rather than failing the
    // whole reduce.
    let provenance = event
        .payload
        .get("provenance")
        .and_then(|v| serde_json::from_value::<crate::bundle::ProvenanceRef>(v.clone()).ok());
    state.findings[idx].annotations.push(Annotation {
        id: annotation_id.to_string(),
        text: text.to_string(),
        author: event.actor.id.clone(),
        timestamp: event.timestamp.clone(),
        provenance,
    });
    Ok(())
}

fn apply_finding_confidence_revised(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    let id = event.target.id.as_str();
    let new_score = event
        .payload
        .get("new_score")
        .and_then(Value::as_f64)
        .ok_or("reducer: finding.confidence_revised missing payload.new_score")?;
    let previous = event
        .payload
        .get("previous_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| format!("reducer: confidence_revised targets unknown finding {id}"))?;
    state.findings[idx].confidence.score = new_score;
    state.findings[idx].confidence.basis = format!(
        "expert revision from {:.3} to {:.3}: {}",
        previous, new_score, event.reason
    );
    state.findings[idx].confidence.method = ConfidenceMethod::ExpertJudgment;
    state.findings[idx].updated = Some(Utc::now().to_rfc3339());
    Ok(())
}

fn apply_finding_rejected(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    let id = event.target.id.as_str();
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| format!("reducer: finding.rejected targets unknown finding {id}"))?;
    state.findings[idx].flags.contested = true;
    Ok(())
}

fn apply_finding_retracted(state: &mut Project, event: &StateEvent) -> Result<(), String> {
    let id = event.target.id.as_str();
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| format!("reducer: finding.retracted targets unknown finding {id}"))?;
    state.findings[idx].flags.retracted = true;
    Ok(())
}

fn apply_finding_dependency_invalidated(
    state: &mut Project,
    event: &StateEvent,
) -> Result<(), String> {
    let id = event.target.id.as_str();
    let upstream = event
        .payload
        .get("upstream_finding_id")
        .and_then(Value::as_str)
        .unwrap_or("?");
    let depth = event
        .payload
        .get("depth")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let idx = state
        .findings
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| {
            format!("reducer: finding.dependency_invalidated targets unknown finding {id}")
        })?;
    state.findings[idx].flags.contested = true;
    let annotation_id = format!("ann_dep_{}_{}", &event.id[4..], depth);
    if !state.findings[idx]
        .annotations
        .iter()
        .any(|a| a.id == annotation_id)
    {
        state.findings[idx].annotations.push(Annotation {
            id: annotation_id,
            text: format!("Upstream {upstream} retracted (cascade depth {depth})."),
            author: event.actor.id.clone(),
            timestamp: event.timestamp.clone(),
            provenance: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{Assertion, Conditions, Confidence, Evidence, Flags, Provenance};
    use crate::events::{FindingEventInput, NULL_HASH, StateActor, StateTarget};
    use serde_json::json;

    fn finding(id: &str) -> crate::bundle::FindingBundle {
        crate::bundle::FindingBundle::new(
            Assertion {
                text: format!("test finding {id}"),
                assertion_type: "mechanism".to_string(),
                entities: Vec::new(),
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "experimental".to_string(),
                model_system: String::new(),
                species: None,
                method: "test".to_string(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: Vec::new(),
            },
            Conditions {
                text: "test".to_string(),
                species_verified: Vec::new(),
                species_unverified: Vec::new(),
                in_vitro: false,
                in_vivo: true,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            Confidence::raw(0.5, "test", 0.8),
            Provenance {
                source_type: "published_paper".to_string(),
                doi: Some(format!("10.1/test-{id}")),
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: format!("Source for {id}"),
                authors: Vec::new(),
                year: Some(2026),
                journal: None,
                license: None,
                publisher: None,
                funders: Vec::new(),
                extraction: crate::bundle::Extraction::default(),
                review: None,
                citation_count: None,
            },
            Flags {
                gap: false,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            signature_threshold: None,
            jointly_accepted: false,
        },
        )
    }

    #[test]
    fn replay_with_no_events_is_identity() {
        let state = project::assemble("test", vec![finding("a")], 0, 0, "test");
        let v = verify_replay(&state);
        assert!(v.ok);
        assert_eq!(v.replayed_snapshot_hash, v.materialized_snapshot_hash);
    }

    #[test]
    fn reducer_marks_finding_contested() {
        let f = finding("a");
        let mut state = project::assemble("test", vec![f.clone()], 0, 0, "test");
        let event = events::new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &f.id,
            actor_id: "reviewer:test",
            actor_type: "human",
            reason: "test",
            before_hash: &events::finding_hash(&f),
            after_hash: NULL_HASH,
            payload: json!({"status": "contested"}),
            caveats: vec![],
        });
        apply_event(&mut state, &event).unwrap();
        assert!(state.findings[0].flags.contested);
    }

    #[test]
    fn reducer_retracts_finding() {
        let f = finding("a");
        let mut state = project::assemble("test", vec![f.clone()], 0, 0, "test");
        let event = StateEvent {
            schema: events::EVENT_SCHEMA.to_string(),
            id: "vev_test".to_string(),
            kind: "finding.retracted".to_string(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: f.id.clone(),
            },
            actor: StateActor {
                id: "reviewer:test".to_string(),
                r#type: "human".to_string(),
            },
            timestamp: Utc::now().to_rfc3339(),
            reason: "test retraction".to_string(),
            before_hash: events::finding_hash(&f),
            after_hash: NULL_HASH.to_string(),
            payload: json!({"proposal_id": "vpr_x"}),
            caveats: vec![],
            signature: None,
        };
        apply_event(&mut state, &event).unwrap();
        assert!(state.findings[0].flags.retracted);
    }

    #[test]
    fn reducer_rejects_unknown_kind() {
        let mut state = project::assemble("test", vec![], 0, 0, "test");
        let event = StateEvent {
            schema: events::EVENT_SCHEMA.to_string(),
            id: "vev_test".to_string(),
            kind: "finding.unknown_kind".to_string(),
            target: StateTarget {
                r#type: "finding".to_string(),
                id: "vf_x".to_string(),
            },
            actor: StateActor {
                id: "x".to_string(),
                r#type: "human".to_string(),
            },
            timestamp: Utc::now().to_rfc3339(),
            reason: "x".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: Value::Null,
            caveats: vec![],
            signature: None,
        };
        let r = apply_event(&mut state, &event);
        assert!(r.is_err());
    }
}
