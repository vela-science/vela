//! Canonical replayable frontier events.
//!
//! Events are the authoritative record for user-visible state transitions in
//! the finding-centered v0 kernel. Frontier snapshots remain the convenient
//! materialized state, but checks and proof packets can validate the event log.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::FindingBundle;
use crate::canonical;
use crate::project::Project;

pub const EVENT_SCHEMA: &str = "vela.event.v0.1";
pub const NULL_HASH: &str = "sha256:null";

/// v0.49: explicit event kind for actor-key revocation. Coalition
/// governance promises that key compromise is handled by a signed
/// `RevocationEvent` that names the key, the moment of compromise,
/// and the recommended replacement. This constant pairs with
/// `RevocationPayload` and `new_revocation_event` below.
///
/// Existing signed history stays valid as a record of what was
/// signed when; clients that re-verify against the post-revocation
/// actor list flag any signature whose `signed_at` is after the
/// `revoked_at` moment. The hub is transport, not authority — it
/// stores the revocation alongside the entries that referenced the
/// revoked key, lets readers decide.
pub const EVENT_KIND_KEY_REVOKE: &str = "key.revoke";

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

/// Payload of an `EVENT_KIND_KEY_REVOKE` event. Carries the
/// revoked Ed25519 pubkey (hex-encoded), the moment compromise was
/// detected (ISO-8601), an optional replacement pubkey the actor is
/// migrating to, and a free-form reason string. Stored on the event's
/// `payload` field; the event's `actor` is the actor whose key is
/// being revoked, and the event itself must be signed by a key that
/// was authoritative *before* the revocation (typically a co-signer
/// or the actor's prior key — never the revoked key itself).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevocationPayload {
    /// The Ed25519 pubkey being revoked, hex-encoded (64 chars).
    pub revoked_pubkey: String,
    /// ISO-8601 moment when compromise was detected. Signatures
    /// whose `signed_at` falls after this should be flagged on
    /// re-verification.
    pub revoked_at: String,
    /// Optional replacement pubkey the actor is now signing with,
    /// hex-encoded. Reviewers re-verifying signed history use this
    /// to walk forward to the new key.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub replacement_pubkey: String,
    /// Free-form reason — "key file leaked", "stolen device",
    /// "scheduled rotation", etc. Reviewer-facing only; the
    /// substrate doesn't enumerate.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// Construct a signed-shape `key.revoke` event for the given actor.
/// Mirrors `new_finding_event` in shape but targets an actor and
/// carries a `RevocationPayload` in `payload`. The returned event is
/// unsigned (caller signs it); `event.id` is the canonical content
/// address of the unsigned shape.
pub fn new_revocation_event(
    actor_id: &str,
    actor_type: &str,
    payload: RevocationPayload,
    reason: &str,
    before_hash: &str,
    after_hash: &str,
) -> StateEvent {
    let timestamp = Utc::now().to_rfc3339();
    let payload_value =
        serde_json::to_value(&payload).expect("RevocationPayload serializes to a JSON object");
    let mut event = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: EVENT_KIND_KEY_REVOKE.to_string(),
        target: StateTarget {
            r#type: "actor".to_string(),
            id: actor_id.to_string(),
        },
        actor: StateActor {
            id: actor_id.to_string(),
            r#type: actor_type.to_string(),
        },
        timestamp,
        reason: reason.to_string(),
        before_hash: before_hash.to_string(),
        after_hash: after_hash.to_string(),
        payload: payload_value,
        caveats: Vec::new(),
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
        // v0.40.1: prediction expired without resolution. Emitted by
        // `calibration::expire_overdue_predictions` when a prediction's
        // `resolves_by` is in the past and no Resolution targets it.
        // Closing the prediction this way does not generate a
        // synthesized Resolution — the predictor failed to commit
        // either way, and calibration tracks it as a separate count.
        "prediction.expired_unresolved" => {
            require_str("prediction_id")?;
            require_str("resolves_by")?;
            require_str("expired_at")?;
        }
        // v0.39: federation events. Both record interactions with a
        // peer hub registered in `Project.peers`. The actual sync
        // runtime (HTTP fetch + manifest verification) ships in
        // v0.39.1+; v0.39.0 only validates the event schema so a
        // hand-emitted sync record can already be replay-checked.
        "frontier.synced_with_peer" => {
            require_str("peer_id")?;
            require_str("peer_snapshot_hash")?;
            require_str("our_snapshot_hash")?;
            let _ = object
                .get("divergence_count")
                .and_then(Value::as_u64)
                .ok_or("missing required non-negative integer 'divergence_count'")?;
        }
        "frontier.conflict_detected" => {
            require_str("peer_id")?;
            require_str("finding_id")?;
            let kind = require_str("kind")?;
            // The conflict kind is open-ended for now; v0.39.1+ will
            // tighten this enum once the sync runtime lands. For
            // v0.39.0 we only require it to be non-empty so a replay
            // can group conflicts by category.
            if kind.trim().is_empty() {
                return Err("payload.kind must be a non-empty string".to_string());
            }
        }
        // v0.38: causal-typing reinterpretation. The substrate doesn't
        // erase the prior reading; it appends a new event recording who
        // re-graded the claim and why. `before` and `after` payloads
        // each carry `claim` (correlation|mediation|intervention) and
        // optionally `grade` (rct|quasi_experimental|observational|
        // theoretical). Pre-v0.38 findings carried neither, so a
        // reinterpretation may originate from a block with both fields
        // absent or null.
        "assertion.reinterpreted_causal" => {
            require_str("proposal_id")?;
            let check_block = |block_name: &str| -> Result<(), String> {
                let block = object
                    .get(block_name)
                    .and_then(Value::as_object)
                    .ok_or_else(|| format!("payload.{block_name} must be an object"))?;
                if let Some(claim) = block.get("claim").and_then(Value::as_str)
                    && !crate::bundle::VALID_CAUSAL_CLAIMS.contains(&claim)
                {
                    return Err(format!(
                        "{block_name}.claim '{claim}' not in {:?}",
                        crate::bundle::VALID_CAUSAL_CLAIMS
                    ));
                }
                if let Some(grade) = block.get("grade").and_then(Value::as_str)
                    && !crate::bundle::VALID_CAUSAL_EVIDENCE_GRADES.contains(&grade)
                {
                    return Err(format!(
                        "{block_name}.grade '{grade}' not in {:?}",
                        crate::bundle::VALID_CAUSAL_EVIDENCE_GRADES
                    ));
                }
                Ok(())
            };
            check_block("before")?;
            check_block("after")?;
        }
        // v0.37: multi-sig kernel events. `threshold_set` records the
        // policy attached to a finding (k unique valid signatures
        // required); `threshold_met` records the moment the k-th
        // signature lands. Both are content-addressed under the same
        // canonical-JSON discipline as every other event kind.
        "finding.threshold_set" => {
            let threshold = object
                .get("threshold")
                .and_then(Value::as_u64)
                .ok_or("missing required positive integer 'threshold'")?;
            if threshold == 0 {
                return Err("threshold must be >= 1".to_string());
            }
        }
        "finding.threshold_met" => {
            let count = object
                .get("signature_count")
                .and_then(Value::as_u64)
                .ok_or("missing required positive integer 'signature_count'")?;
            let threshold = object
                .get("threshold")
                .and_then(Value::as_u64)
                .ok_or("missing required positive integer 'threshold'")?;
            if count < threshold {
                return Err(format!(
                    "signature_count {count} below threshold {threshold}"
                ));
            }
        }
        // v0.49: key revocation event. Carries the revoked Ed25519
        // pubkey (hex-encoded 64 chars), the ISO-8601 moment compromise
        // was detected, and an optional replacement pubkey + reason.
        // Validating here keeps a hand-emitted or peer-fetched
        // revocation honest at the event-pipeline boundary so a
        // malformed revocation can't slip through replay and silently
        // re-trust the compromised key.
        EVENT_KIND_KEY_REVOKE => {
            let revoked = require_str("revoked_pubkey")?;
            if revoked.len() != 64 || !revoked.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!(
                    "revoked_pubkey must be 64 hex chars (Ed25519 pubkey), got {} chars",
                    revoked.len()
                ));
            }
            let revoked_at = require_str("revoked_at")?;
            if revoked_at.trim().is_empty() {
                return Err("revoked_at must be a non-empty ISO-8601 timestamp".to_string());
            }
            // v0.49.1: parse as RFC-3339 / ISO-8601 so a typo'd value
            // ("yesterday", "x", "2026-13-99T...") fails at the
            // validator boundary rather than poisoning re-verification
            // of post-revocation signatures further downstream.
            if DateTime::parse_from_rfc3339(revoked_at).is_err() {
                return Err(format!(
                    "revoked_at must parse as RFC-3339/ISO-8601, got {revoked_at:?}"
                ));
            }
            // replacement_pubkey is optional but if present must be a
            // valid hex pubkey of the same shape — a typo here would
            // strand the actor's identity at the wrong forward key.
            if let Some(replacement) = object.get("replacement_pubkey")
                && let Some(rep_str) = replacement.as_str()
                && !rep_str.is_empty()
                && (rep_str.len() != 64 || !rep_str.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Err(format!(
                    "replacement_pubkey must be 64 hex chars when present, got {} chars",
                    rep_str.len()
                ));
            }
            // The revoked key cannot also be the replacement; that
            // would be a self-rotation that revokes nothing.
            if let Some(replacement) = object.get("replacement_pubkey").and_then(Value::as_str)
                && !replacement.is_empty()
                && replacement.eq_ignore_ascii_case(revoked)
            {
                return Err("replacement_pubkey must differ from revoked_pubkey".to_string());
            }
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
                causal_claim: None,
                causal_evidence_grade: None,
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
            Confidence::raw(0.6, "test", 0.8),
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
                signature_threshold: None,
                jointly_accepted: false,
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

    // v0.39 — federation event validation
    #[test]
    fn validates_synced_with_peer_payload() {
        // OK: full payload.
        assert!(
            validate_event_payload(
                "frontier.synced_with_peer",
                &json!({
                    "peer_id": "hub:peer",
                    "peer_snapshot_hash": "abc",
                    "our_snapshot_hash": "def",
                    "divergence_count": 3,
                }),
            )
            .is_ok()
        );
        // FAIL: missing divergence_count.
        assert!(
            validate_event_payload(
                "frontier.synced_with_peer",
                &json!({
                    "peer_id": "hub:peer",
                    "peer_snapshot_hash": "abc",
                    "our_snapshot_hash": "def",
                }),
            )
            .is_err()
        );
        // FAIL: missing peer_id.
        assert!(
            validate_event_payload(
                "frontier.synced_with_peer",
                &json!({
                    "peer_snapshot_hash": "abc",
                    "our_snapshot_hash": "def",
                    "divergence_count": 0,
                }),
            )
            .is_err()
        );
    }

    #[test]
    fn validates_conflict_detected_payload() {
        // OK: full payload.
        assert!(
            validate_event_payload(
                "frontier.conflict_detected",
                &json!({
                    "peer_id": "hub:peer",
                    "finding_id": "vf_xyz",
                    "kind": "different_review_verdict",
                }),
            )
            .is_ok()
        );
        // FAIL: empty kind.
        assert!(
            validate_event_payload(
                "frontier.conflict_detected",
                &json!({
                    "peer_id": "hub:peer",
                    "finding_id": "vf_xyz",
                    "kind": "  ",
                }),
            )
            .is_err()
        );
        // FAIL: missing finding_id.
        assert!(
            validate_event_payload(
                "frontier.conflict_detected",
                &json!({
                    "peer_id": "hub:peer",
                    "kind": "missing_in_peer",
                }),
            )
            .is_err()
        );
    }

    // v0.38 — causal-typing event validation
    #[test]
    fn validates_reinterpreted_causal_payload() {
        // OK: missing claim/grade is fine (None means no prior reading).
        assert!(
            validate_event_payload(
                "assertion.reinterpreted_causal",
                &json!({
                    "proposal_id": "vpr_test",
                    "before": {},
                    "after": { "claim": "intervention", "grade": "rct" },
                }),
            )
            .is_ok()
        );
        // OK: pure claim revision, no grade.
        assert!(
            validate_event_payload(
                "assertion.reinterpreted_causal",
                &json!({
                    "proposal_id": "vpr_test",
                    "before": { "claim": "correlation" },
                    "after": { "claim": "mediation" },
                }),
            )
            .is_ok()
        );
        // FAIL: invalid claim.
        assert!(
            validate_event_payload(
                "assertion.reinterpreted_causal",
                &json!({
                    "proposal_id": "vpr_test",
                    "before": {},
                    "after": { "claim": "magic" },
                }),
            )
            .is_err()
        );
        // FAIL: invalid grade.
        assert!(
            validate_event_payload(
                "assertion.reinterpreted_causal",
                &json!({
                    "proposal_id": "vpr_test",
                    "before": {},
                    "after": { "claim": "intervention", "grade": "vibes" },
                }),
            )
            .is_err()
        );
        // FAIL: missing proposal_id.
        assert!(
            validate_event_payload(
                "assertion.reinterpreted_causal",
                &json!({
                    "before": {},
                    "after": { "claim": "intervention" },
                }),
            )
            .is_err()
        );
    }

    /// v0.49: a `key.revoke` event names the revoked pubkey, the
    /// moment of compromise, and (optionally) the replacement key.
    /// Empty optional fields skip canonical-JSON serialization so
    /// existing event logs round-trip byte-identically.
    #[test]
    fn revocation_event_canonical_shape() {
        use crate::canonical;
        let payload = RevocationPayload {
            revoked_pubkey: "4892f93877e637b5f59af31d9ec6704814842fb278cacb0eb94704baef99455e"
                .to_string(),
            revoked_at: "2026-05-01T17:00:00Z".to_string(),
            replacement_pubkey: "8891a2ab35ca2ed2182ed4e46b6567ce8dacc9985eb496d895578201272a1cd9"
                .to_string(),
            reason: "key file leaked from CI cache".to_string(),
        };
        let event = new_revocation_event(
            "reviewer:will-blair",
            "human",
            payload,
            "rotating compromised key",
            NULL_HASH,
            NULL_HASH,
        );
        assert_eq!(event.kind, EVENT_KIND_KEY_REVOKE);
        assert_eq!(event.target.r#type, "actor");
        assert!(event.id.starts_with("vev_"));
        let bytes = canonical::to_canonical_bytes(&event).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(
            s.contains("\"revoked_pubkey\""),
            "canonical bytes missing revoked_pubkey: {s}"
        );
        assert!(
            s.contains("\"revoked_at\""),
            "canonical bytes missing revoked_at: {s}"
        );
        assert!(
            s.contains("\"replacement_pubkey\""),
            "canonical bytes missing replacement_pubkey: {s}"
        );

        // Empty replacement_pubkey skips serialization.
        let payload_minimal = RevocationPayload {
            revoked_pubkey: "a".repeat(64),
            revoked_at: "2026-05-01T17:00:00Z".to_string(),
            replacement_pubkey: String::new(),
            reason: String::new(),
        };
        let minimal_event = new_revocation_event(
            "reviewer:will-blair",
            "human",
            payload_minimal,
            "scheduled rotation",
            NULL_HASH,
            NULL_HASH,
        );
        let minimal_bytes = canonical::to_canonical_bytes(&minimal_event).unwrap();
        let minimal_s = std::str::from_utf8(&minimal_bytes).unwrap();
        assert!(
            !minimal_s.contains("\"replacement_pubkey\""),
            "empty replacement_pubkey leaked into canonical JSON: {minimal_s}"
        );
        assert!(
            !minimal_s.contains("\"reason\":\"\""),
            "empty payload reason leaked into canonical JSON: {minimal_s}"
        );
    }

    /// v0.49: validate_event_payload now recognises `key.revoke`.
    /// Tests cover the four real failure modes plus the happy path.
    #[test]
    fn revocation_payload_validation() {
        let good_pubkey = "4892f93877e637b5f59af31d9ec6704814842fb278cacb0eb94704baef99455e";
        let other_pubkey = "8891a2ab35ca2ed2182ed4e46b6567ce8dacc9985eb496d895578201272a1cd9";

        // OK: minimal valid payload.
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": good_pubkey,
                    "revoked_at": "2026-05-01T17:00:00Z",
                }),
            )
            .is_ok()
        );

        // OK: full payload with replacement and reason.
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": good_pubkey,
                    "revoked_at": "2026-05-01T17:00:00Z",
                    "replacement_pubkey": other_pubkey,
                    "reason": "key file leaked",
                }),
            )
            .is_ok()
        );

        // FAIL: revoked_pubkey wrong length (32 bytes ASCII, not 64 hex).
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": "abc123",
                    "revoked_at": "2026-05-01T17:00:00Z",
                }),
            )
            .is_err()
        );

        // FAIL: revoked_pubkey contains non-hex chars.
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": "ZZ".repeat(32),
                    "revoked_at": "2026-05-01T17:00:00Z",
                }),
            )
            .is_err()
        );

        // FAIL: missing revoked_at.
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": good_pubkey,
                }),
            )
            .is_err()
        );

        // FAIL: replacement_pubkey wrong length.
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": good_pubkey,
                    "revoked_at": "2026-05-01T17:00:00Z",
                    "replacement_pubkey": "deadbeef",
                }),
            )
            .is_err()
        );

        // FAIL: replacement equals revoked (no-op rotation).
        assert!(
            validate_event_payload(
                EVENT_KIND_KEY_REVOKE,
                &json!({
                    "revoked_pubkey": good_pubkey,
                    "revoked_at": "2026-05-01T17:00:00Z",
                    "replacement_pubkey": good_pubkey,
                }),
            )
            .is_err()
        );

        // FAIL: revoked_at is non-empty but not a valid ISO-8601 stamp.
        // The v0.49.1 validator parses it as RFC-3339 so typos can't
        // reach replay verification.
        // chrono's parse_from_rfc3339 is intentionally lenient on the
        // `T` vs space separator (RFC-3339 §5.6), so we don't include
        // that case here — chronologically nonsensical strings still
        // fail, which is the bar we care about.
        for bad in [
            "yesterday",
            "2026-13-01T00:00:00Z", // month 13
            "2026-05-01",           // date only, no time
            "x",
        ] {
            assert!(
                validate_event_payload(
                    EVENT_KIND_KEY_REVOKE,
                    &json!({
                        "revoked_pubkey": good_pubkey,
                        "revoked_at": bad,
                    }),
                )
                .is_err(),
                "expected revoked_at {bad:?} to fail validation"
            );
        }
    }
}
