//! Canonical replayable frontier events.
//!
//! Events are the authoritative record for user-visible state transitions in
//! the finding-centered v0 kernel. Frontier snapshots remain the convenient
//! materialized state, but checks and proof packets can validate the event log.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::FindingBundle;
use crate::canonical;
use crate::project::Project;

pub const EVENT_SCHEMA: &str = "vela.event.v0.1";
pub const NULL_HASH: &str = "sha256:null";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateTarget {
    pub r#type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateActor {
    pub id: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    pub kind: String,
    pub target: StateTarget,
    pub actor: StateActor,
    pub timestamp: String,
    pub reason: String,
    pub before_hash: String,
    pub after_hash: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub caveats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

pub struct FindingEventInput<'a> {
    pub kind: &'a str,
    pub finding_id: &'a str,
    pub actor_id: &'a str,
    pub actor_type: &'a str,
    pub reason: &'a str,
    pub before_hash: &'a str,
    pub after_hash: &'a str,
    pub payload: Value,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogSummary {
    pub count: usize,
    pub kinds: BTreeMap<String, usize>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub duplicate_ids: Vec<String>,
    pub orphan_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayReport {
    pub ok: bool,
    pub status: String,
    pub event_log: EventLogSummary,
    pub source_hash: String,
    pub event_log_hash: String,
    pub replayed_hash: String,
    pub current_hash: String,
    pub conflicts: Vec<String>,
}

fn default_schema() -> String {
    EVENT_SCHEMA.to_string()
}

pub fn new_finding_event(input: FindingEventInput<'_>) -> StateEvent {
    let timestamp = Utc::now().to_rfc3339();
    let mut event = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: input.kind.to_string(),
        target: StateTarget {
            r#type: "finding".to_string(),
            id: input.finding_id.to_string(),
        },
        actor: StateActor {
            id: input.actor_id.to_string(),
            r#type: input.actor_type.to_string(),
        },
        timestamp,
        reason: input.reason.to_string(),
        before_hash: input.before_hash.to_string(),
        after_hash: input.after_hash.to_string(),
        payload: input.payload,
        caveats: input.caveats,
        signature: None,
    };
    event.id = event_id(&event);
    event
}

pub fn finding_hash(finding: &FindingBundle) -> String {
    // Per Protocol §5, links are "review surfaces" — typed relationships
    // between findings inferred at compile or review time, NOT part of the
    // finding's content commitment. They are mutable: `vela link add`
    // appends links without emitting a state-event (links don't change
    // what the finding asserts; they change which findings know about
    // each other). For event-replay validity the finding hash must therefore
    // exclude `links`, otherwise any CLI-added link breaks the asserted-event
    // chain. v0.12: hash a links-cleared copy. State-changing events
    // (caveat/note/review/revise/retract) still mutate annotations/flags/
    // confidence — those remain in the hash and chain through events properly.
    let mut hashable = finding.clone();
    hashable.links.clear();
    let bytes = canonical::to_canonical_bytes(&hashable).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

pub fn finding_hash_by_id(frontier: &Project, finding_id: &str) -> String {
    frontier
        .findings
        .iter()
        .find(|finding| finding.id == finding_id)
        .map(finding_hash)
        .unwrap_or_else(|| NULL_HASH.to_string())
}

pub fn event_log_hash(events: &[StateEvent]) -> String {
    let bytes = canonical::to_canonical_bytes(events).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

pub fn snapshot_hash(frontier: &Project) -> String {
    let value = serde_json::to_value(frontier).unwrap_or(Value::Null);
    let mut value = value;
    if let Value::Object(map) = &mut value {
        map.remove("events");
        map.remove("signatures");
        map.remove("proof_state");
    }
    let bytes = canonical::to_canonical_bytes(&value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

pub fn events_for_finding<'a>(frontier: &'a Project, finding_id: &str) -> Vec<&'a StateEvent> {
    frontier
        .events
        .iter()
        .filter(|event| event.target.r#type == "finding" && event.target.id == finding_id)
        .collect()
}

pub fn replay_report(frontier: &Project) -> ReplayReport {
    let event_log = summarize(frontier);
    let mut conflicts = Vec::new();

    if frontier.events.is_empty() {
        let current_hash = snapshot_hash(frontier);
        return ReplayReport {
            ok: true,
            status: "no_events".to_string(),
            event_log,
            source_hash: current_hash.clone(),
            event_log_hash: event_log_hash(&frontier.events),
            replayed_hash: current_hash.clone(),
            current_hash,
            conflicts,
        };
    }

    for duplicate in &event_log.duplicate_ids {
        conflicts.push(format!("duplicate event id: {duplicate}"));
    }
    for orphan in &event_log.orphan_targets {
        conflicts.push(format!("orphan event target: {orphan}"));
    }

    let mut chains = BTreeMap::<String, Vec<&StateEvent>>::new();
    for event in &frontier.events {
        if event.schema != EVENT_SCHEMA {
            conflicts.push(format!(
                "unsupported event schema for {}: {}",
                event.id, event.schema
            ));
        }
        if event.reason.trim().is_empty() {
            conflicts.push(format!("event {} has empty reason", event.id));
        }
        if event.before_hash.trim().is_empty() || event.after_hash.trim().is_empty() {
            conflicts.push(format!("event {} has empty hash boundary", event.id));
        }
        // Phase E: per-kind payload schema validation. Each event kind has
        // a normative payload shape documented in `docs/PROTOCOL.md` §6;
        // payloads that don't match are conformance failures, not just
        // "weird optional content."
        if let Err(err) = validate_event_payload(&event.kind, &event.payload) {
            conflicts.push(format!("event {} payload invalid: {err}", event.id));
        }
        chains
            .entry(format!("{}:{}", event.target.r#type, event.target.id))
            .or_default()
            .push(event);
    }

    for (target, events) in chains {
        let mut sorted = events;
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then(a.id.cmp(&b.id)));
        for pair in sorted.windows(2) {
            let previous = pair[0];
            let next = pair[1];
            if previous.after_hash != next.before_hash {
                conflicts.push(format!(
                    "event chain break for {target}: {} after_hash does not match {} before_hash",
                    previous.id, next.id
                ));
            }
        }
        if let Some(last) = sorted.last()
            && last.target.r#type == "finding"
        {
            let current = finding_hash_by_id(frontier, &last.target.id);
            if current != last.after_hash {
                conflicts.push(format!(
                    "materialized finding {} hash does not match last event {}",
                    last.target.id, last.id
                ));
            }
        }
    }

    let current_hash = snapshot_hash(frontier);
    let ok = conflicts.is_empty();
    ReplayReport {
        ok,
        status: if ok { "ok" } else { "conflict" }.to_string(),
        event_log,
        source_hash: current_hash.clone(),
        event_log_hash: event_log_hash(&frontier.events),
        replayed_hash: if ok {
            current_hash.clone()
        } else {
            "unavailable".to_string()
        },
        current_hash,
        conflicts,
    }
}

pub fn replay_report_json(frontier: &Project) -> Value {
    serde_json::to_value(replay_report(frontier)).unwrap_or_else(|_| json!({"ok": false}))
}

pub fn summarize(frontier: &Project) -> EventLogSummary {
    let mut kinds = BTreeMap::<String, usize>::new();
    let mut seen = BTreeSet::<String>::new();
    let mut duplicate_ids = BTreeSet::<String>::new();
    let finding_ids = frontier
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut orphan_targets = BTreeSet::<String>::new();
    let mut timestamps = Vec::<String>::new();

    for event in &frontier.events {
        *kinds.entry(event.kind.clone()).or_default() += 1;
        if !seen.insert(event.id.clone()) {
            duplicate_ids.insert(event.id.clone());
        }
        if event.target.r#type == "finding"
            && !finding_ids.contains(event.target.id.as_str())
            && event.kind != "finding.retracted"
        {
            orphan_targets.insert(event.target.id.clone());
        }
        timestamps.push(event.timestamp.clone());
    }
    timestamps.sort();

    EventLogSummary {
        count: frontier.events.len(),
        kinds,
        first_timestamp: timestamps.first().cloned(),
        last_timestamp: timestamps.last().cloned(),
        duplicate_ids: duplicate_ids.into_iter().collect(),
        orphan_targets: orphan_targets.into_iter().collect(),
    }
}

/// Validate a canonical event's payload against its per-kind schema.
///
/// Each event kind has a normative payload shape. Phase E pins those
/// shapes so a second implementation can reject malformed events
/// without per-kind ad-hoc parsing. The schemas are documented in
/// `docs/PROTOCOL.md` §6 and conformance-checked at the v0.3 level.
///
/// Unknown kinds are rejected so future-event-kind reads from older
/// implementations fail fast rather than silently accepting opaque
/// content.
pub fn validate_event_payload(kind: &str, payload: &Value) -> Result<(), String> {
    let object = payload.as_object().ok_or_else(|| {
        if matches!(payload, Value::Null) {
            "payload must be a JSON object (got null)".to_string()
        } else {
            "payload must be a JSON object".to_string()
        }
    })?;
    let require_str = |key: &str| -> Result<&str, String> {
        object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing required string field '{key}'"))
    };
    let require_f64 = |key: &str| -> Result<f64, String> {
        object
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("missing required number field '{key}'"))
    };
    match kind {
        "finding.asserted" => {
            // proposal_id required; optional `finding` for v0.3 genesis
            // events that carry the bootstrap finding inline.
            require_str("proposal_id")?;
        }
        "finding.reviewed" => {
            require_str("proposal_id")?;
            let status = require_str("status")?;
            if !matches!(
                status,
                "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
            ) {
                return Err(format!("invalid review status '{status}'"));
            }
        }
        "finding.noted" | "finding.caveated" => {
            require_str("proposal_id")?;
            require_str("annotation_id")?;
            let text = require_str("text")?;
            if text.trim().is_empty() {
                return Err("payload.text must be non-empty".to_string());
            }
            // Phase β (v0.6): optional structured `provenance` block.
            // When present, MUST be an object and MUST carry at least one
            // identifying field (doi/pmid/title). An all-empty
            // `provenance: {}` is a contract violation, not a tolerable
            // default — agents that pass the field are expected to mean it.
            if let Some(prov) = object.get("provenance") {
                let prov_obj = prov
                    .as_object()
                    .ok_or("payload.provenance must be a JSON object when present")?;
                let has_id = prov_obj
                    .get("doi")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty())
                    || prov_obj
                        .get("pmid")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty())
                    || prov_obj
                        .get("title")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty());
                if !has_id {
                    return Err(
                        "payload.provenance must include at least one of doi/pmid/title"
                            .to_string(),
                    );
                }
            }
        }
        "finding.confidence_revised" => {
            require_str("proposal_id")?;
            let new_score = require_f64("new_score")?;
            if !(0.0..=1.0).contains(&new_score) {
                return Err(format!("new_score {new_score} out of [0.0, 1.0]"));
            }
            let _ = require_f64("previous_score")?;
        }
        "finding.rejected" => {
            require_str("proposal_id")?;
        }
        "finding.superseded" => {
            require_str("proposal_id")?;
            require_str("new_finding_id")?;
        }
        "finding.retracted" => {
            require_str("proposal_id")?;
            // affected and cascade are summary fields; optional but if
            // present, affected must be a non-negative integer.
            if let Some(affected) = object.get("affected") {
                let _ = affected
                    .as_u64()
                    .ok_or("affected must be a non-negative integer")?;
            }
        }
        // Phase L: per-dependent cascade events. Each one names the
        // upstream retraction it descends from, the cascade depth, and
        // the canonical event ID of the source retraction so a replay
        // can reconstruct the cascade without trusting summary fields.
        "finding.dependency_invalidated" => {
            require_str("upstream_finding_id")?;
            require_str("upstream_event_id")?;
            let depth = object
                .get("depth")
                .and_then(Value::as_u64)
                .ok_or("missing required positive integer 'depth'")?;
            if depth == 0 {
                return Err("depth must be >= 1 (genesis is the source retraction)".to_string());
            }
            // proposal_id present for cascade-source traceability.
            require_str("proposal_id")?;
        }
        // Phase H will introduce frontier.created. For v0.3 it accepts
        // a name + creator pair; left here for forward compatibility.
        "frontier.created" => {
            require_str("name")?;
            require_str("creator")?;
        }
        other => return Err(format!("unknown event kind '{other}'")),
    }
    Ok(())
}

/// Public form of `event_id` so callers building non-finding events
/// (e.g. the `frontier.created` genesis event in `project::assemble`)
/// can compute the canonical event ID with the same canonical-JSON
/// preimage shape as `new_finding_event`.
pub fn compute_event_id(event: &StateEvent) -> String {
    event_id(event)
}

fn event_id(event: &StateEvent) -> String {
    let content = json!({
        "schema": event.schema,
        "kind": event.kind,
        "target": event.target,
        "actor": event.actor,
        "timestamp": event.timestamp,
        "reason": event.reason,
        "before_hash": event.before_hash,
        "after_hash": event.after_hash,
        "payload": event.payload,
        "caveats": event.caveats,
    });
    let bytes = canonical::to_canonical_bytes(&content).unwrap_or_default();
    format!("vev_{}", &hex::encode(Sha256::digest(bytes))[..16])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{
        Assertion, Conditions, Confidence, Evidence, Extraction, FindingBundle, Flags, Provenance,
    };
    use crate::project;

    fn finding() -> FindingBundle {
        FindingBundle::new(
            Assertion {
                text: "LRP1 clears amyloid beta at the BBB".to_string(),
                assertion_type: "mechanism".to_string(),
                entities: Vec::new(),
                relation: None,
                direction: None,
            },
            Evidence {
                evidence_type: "experimental".to_string(),
                model_system: "mouse".to_string(),
                species: Some("Mus musculus".to_string()),
                method: "assay".to_string(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: Vec::new(),
            },
            Conditions {
                text: "mouse model".to_string(),
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
            Confidence::legacy(0.6, "test", 0.8),
            Provenance {
                source_type: "published_paper".to_string(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test source".to_string(),
                authors: Vec::new(),
                year: Some(2026),
                journal: None,
                license: None,
                publisher: None,
                funders: Vec::new(),
                extraction: Extraction::default(),
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
            },
        )
    }

    #[test]
    fn event_id_is_deterministic_for_content() {
        let event = new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: "vf_test",
            actor_id: "reviewer",
            actor_type: "human",
            reason: "checked",
            before_hash: NULL_HASH,
            after_hash: "sha256:abc",
            payload: json!({"status": "accepted", "proposal_id": "vpr_test"}),
            caveats: vec![],
        });
        let mut same = event.clone();
        same.id = String::new();
        same.id = super::event_id(&same);
        assert_eq!(event.id, same.id);
    }

    #[test]
    fn genesis_only_event_log_replays_ok() {
        // Phase J: assemble() emits a `frontier.created` genesis event,
        // so a freshly compiled frontier never has an empty event log.
        // Replay over genesis-only must succeed with status "ok" and the
        // single event accounted for.
        let frontier = project::assemble("test", Vec::new(), 0, 0, "test");
        let report = replay_report(&frontier);
        assert!(report.ok, "{:?}", report.conflicts);
        assert_eq!(report.event_log.count, 1);
        assert_eq!(report.event_log.kinds.get("frontier.created"), Some(&1));
    }

    #[test]
    fn replay_detects_duplicate_event_ids() {
        let finding = finding();
        let after_hash = finding_hash(&finding);
        let event = new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &finding.id,
            actor_id: "reviewer",
            actor_type: "human",
            reason: "checked",
            before_hash: &after_hash,
            after_hash: &after_hash,
            payload: json!({"status": "accepted", "proposal_id": "vpr_test"}),
            caveats: vec![],
        });
        let mut frontier = project::assemble("test", vec![finding], 0, 0, "test");
        frontier.events = vec![event.clone(), event];

        let report = replay_report(&frontier);
        assert!(!report.ok);
        assert_eq!(report.status, "conflict");
        assert!(!report.event_log.duplicate_ids.is_empty());
    }

    #[test]
    fn replay_detects_orphan_targets() {
        let mut frontier = project::assemble("test", Vec::new(), 0, 0, "test");
        frontier.events.push(new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: "vf_missing",
            actor_id: "reviewer",
            actor_type: "human",
            reason: "checked",
            before_hash: NULL_HASH,
            after_hash: "sha256:abc",
            payload: json!({"status": "accepted", "proposal_id": "vpr_test"}),
            caveats: vec![],
        }));

        let report = replay_report(&frontier);
        assert!(!report.ok);
        assert_eq!(report.event_log.orphan_targets, vec!["vf_missing"]);
    }

    #[test]
    fn replay_accepts_current_hash_boundary() {
        let finding = finding();
        let hash = finding_hash(&finding);
        let event = new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &finding.id,
            actor_id: "reviewer",
            actor_type: "human",
            reason: "checked",
            before_hash: &hash,
            after_hash: &hash,
            payload: json!({"status": "accepted", "proposal_id": "vpr_test"}),
            caveats: vec![],
        });
        let mut frontier = project::assemble("test", vec![finding], 0, 0, "test");
        frontier.events.push(event);

        let report = replay_report(&frontier);
        assert!(report.ok, "{:?}", report.conflicts);
        assert_eq!(report.status, "ok");
    }
}
