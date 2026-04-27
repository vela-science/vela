//! v0.39: Hub federation — peer registry + conflict detection.
//!
//! Pre-v0.39, every Vela frontier had exactly one source of truth: the
//! single hub it was published to (`vela-hub.fly.dev`). The substrate
//! claimed the kernel was content-addressed and signed, but the
//! distribution layer was centralized — there was no way for a second
//! hub to mirror a frontier and detect when its view diverged from
//! the original.
//!
//! v0.39.0 lands the *schema layer* of federation. A frontier can now
//! register peer hubs (id + URL + public key) in `Project.peers`, and
//! the kernel knows two new event kinds:
//!
//! - `frontier.synced_with_peer` — append-only record of a sync pass:
//!   what we exchanged, what hash we ended up agreeing on, how many
//!   findings differed.
//! - `frontier.conflict_detected` — emitted per finding when our view
//!   and the peer's view disagree on a substantive field (review
//!   verdict, confidence, retraction, presence).
//!
//! The actual sync runtime (HTTP fetch, manifest verification,
//! conflict-resolution proposal emission) ships in v0.39.1+. Same
//! staging discipline as v0.32 (Replication object) → v0.36.1
//! (Project.replications becomes authoritative) and v0.38.0 (causal
//! schema) → v0.38.1 (causal math).
//!
//! Doctrine for v0.39.0:
//! - The peer registry is a frontier-local declaration. Adding a peer
//!   does not yet trust their state; it just establishes who we know
//!   about.
//! - Peer signatures still verify under the same Ed25519 discipline
//!   as `actors`. A peer's `frontier.merged` event signed by their
//!   key can be replayed locally only when their pubkey is in our
//!   `peers` registry.
//! - Conflicts are recorded, not auto-resolved. v0.39.1+ will surface
//!   them through proposals so a human reviewer chooses which side
//!   to accept.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::events::{self, FindingEventInput, NULL_HASH, StateEvent, snapshot_hash};
use crate::project::Project;

/// v0.39: A registered peer hub the local frontier knows about.
/// Content-addressed by `(id, public_key)` so two registry entries
/// for the same peer with different keys can be detected as a
/// material change rather than silent overwrite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerHub {
    /// Stable, namespaced identifier — the equivalent of an
    /// `actor.id` for hub-scale identities. Recommended form
    /// `hub:<short-name>` (e.g. `hub:vela-mirror-eu`).
    pub id: String,
    /// HTTPS URL where the peer publishes signed manifests. The
    /// expected shape is `<base>/manifest/<vfr_id>.json` with a
    /// detached signature at `<base>/manifest/<vfr_id>.sig`.
    pub url: String,
    /// Hex-encoded Ed25519 public key (64 hex chars) the peer signs
    /// their manifests with. Used to verify any
    /// `frontier.merged` event coming from them.
    pub public_key: String,
    /// ISO 8601 timestamp of when the peer was added to this
    /// frontier's registry.
    pub added_at: String,
    /// Optional human-readable note: "EU mirror, run by lab Z."
    /// Doesn't enter any content address; stored verbatim.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl PeerHub {
    /// Validate the structural shape of a `PeerHub` before insertion.
    /// Specifically: id must be non-empty, url must be HTTPS, and
    /// public_key must be 64 hex chars.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("peer id must be non-empty".into());
        }
        if !self.url.starts_with("https://") {
            return Err(format!(
                "peer url must start with `https://` (got `{}`)",
                self.url
            ));
        }
        let trimmed = self.public_key.trim();
        if trimmed.len() != 64 {
            return Err(format!(
                "peer public_key must be 64 hex chars (got {})",
                trimmed.len()
            ));
        }
        if hex::decode(trimmed).is_err() {
            return Err("peer public_key must be valid hex".into());
        }
        Ok(())
    }
}

/// v0.39.1: Conflict taxonomy. The kinds of disagreement two hubs can
/// have over the same `vfr_id`. v0.39.0 left `kind` as an open string;
/// v0.39.1 pins it to this closed set, derived from auditing every
/// substantive field-level disagreement we expect to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Finding present in our frontier, absent in the peer's.
    MissingInPeer,
    /// Finding present in the peer's frontier, absent in ours.
    MissingLocally,
    /// Same `vf_id`, score differs by more than 0.05. Below the
    /// threshold it's noise from confidence recompute drift.
    ConfidenceDiverged,
    /// Same `vf_id`, one side has it retracted, the other doesn't.
    RetractedDiverged,
    /// Same `vf_id`, different `flags.review_state`.
    ReviewStateDiverged,
    /// Same `vf_id`, one side has it superseded, the other doesn't.
    SupersededDiverged,
    /// Same `vf_id`, different assertion text. This is a serious
    /// signal — `vf_id` is content-addressed over the assertion, so
    /// matching id with diverging text means a content-address
    /// collision or signing-bytes mismatch between implementations.
    AssertionTextDiverged,
}

impl ConflictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ConflictKind::MissingInPeer => "missing_in_peer",
            ConflictKind::MissingLocally => "missing_locally",
            ConflictKind::ConfidenceDiverged => "confidence_diverged",
            ConflictKind::RetractedDiverged => "retracted_diverged",
            ConflictKind::ReviewStateDiverged => "review_state_diverged",
            ConflictKind::SupersededDiverged => "superseded_diverged",
            ConflictKind::AssertionTextDiverged => "assertion_text_diverged",
        }
    }
}

/// One per-finding disagreement detected during sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub finding_id: String,
    pub kind: ConflictKind,
    /// Free-form context for the rendering layer ("our: 0.82, peer:
    /// 0.65"). Not part of any content address.
    pub detail: String,
}

/// Result of one `sync_with_peer` pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub peer_id: String,
    pub our_snapshot_hash: String,
    pub peer_snapshot_hash: String,
    pub conflicts: Vec<Conflict>,
    /// Number of `StateEvent`s appended to our project (1 sync event +
    /// N conflict events).
    pub events_appended: usize,
}

/// v0.39.1: Diff two frontiers and produce a list of conflicts. Pure
/// function, no I/O. The peer's state is passed in directly so the
/// sync orchestrator can be unit-tested without HTTP.
///
/// Confidence diff threshold is 0.05 — below that it's recompute drift
/// (the v0.36.1 formula change moved scores by < 0.001 on real data).
#[must_use]
pub fn diff_frontiers(ours: &Project, theirs: &Project) -> Vec<Conflict> {
    use std::collections::HashMap;

    let our_by_id: HashMap<&str, &crate::bundle::FindingBundle> =
        ours.findings.iter().map(|f| (f.id.as_str(), f)).collect();
    let their_by_id: HashMap<&str, &crate::bundle::FindingBundle> =
        theirs.findings.iter().map(|f| (f.id.as_str(), f)).collect();

    let mut conflicts = Vec::new();

    // Findings only in ours.
    for (id, _) in &our_by_id {
        if !their_by_id.contains_key(id) {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::MissingInPeer,
                detail: "present locally, absent in peer".to_string(),
            });
        }
    }
    // Findings only in theirs.
    for (id, _) in &their_by_id {
        if !our_by_id.contains_key(id) {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::MissingLocally,
                detail: "present in peer, absent locally".to_string(),
            });
        }
    }
    // Findings in both — check field-level disagreements.
    for (id, ours_f) in &our_by_id {
        let Some(theirs_f) = their_by_id.get(id) else {
            continue;
        };
        if (ours_f.confidence.score - theirs_f.confidence.score).abs() > 0.05 {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::ConfidenceDiverged,
                detail: format!(
                    "ours: {:.3}, peer: {:.3}",
                    ours_f.confidence.score, theirs_f.confidence.score
                ),
            });
        }
        if ours_f.flags.retracted != theirs_f.flags.retracted {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::RetractedDiverged,
                detail: format!(
                    "ours: {}, peer: {}",
                    ours_f.flags.retracted, theirs_f.flags.retracted
                ),
            });
        }
        if ours_f.flags.review_state != theirs_f.flags.review_state {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::ReviewStateDiverged,
                detail: format!(
                    "ours: {:?}, peer: {:?}",
                    ours_f.flags.review_state, theirs_f.flags.review_state
                ),
            });
        }
        if ours_f.flags.superseded != theirs_f.flags.superseded {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::SupersededDiverged,
                detail: format!(
                    "ours: {}, peer: {}",
                    ours_f.flags.superseded, theirs_f.flags.superseded
                ),
            });
        }
        if ours_f.assertion.text.trim() != theirs_f.assertion.text.trim() {
            conflicts.push(Conflict {
                finding_id: (*id).to_string(),
                kind: ConflictKind::AssertionTextDiverged,
                detail: "matching id but diverging assertion text — possible content-address collision".to_string(),
            });
        }
    }

    conflicts.sort_by(|a, b| a.finding_id.cmp(&b.finding_id).then_with(|| a.kind.as_str().cmp(b.kind.as_str())));
    conflicts
}

/// v0.39.1: Run a full sync pass against a peer's already-fetched
/// frontier state. Diffs, emits one `frontier.synced_with_peer`
/// event recording the pass, and one `frontier.conflict_detected`
/// event per disagreement. Returns the report; caller persists the
/// project.
///
/// Splitting fetch from sync this way lets the sync logic be
/// fully unit-testable without HTTP — the CLI pipes a real fetch
/// into this function.
pub fn sync_with_peer(
    project: &mut Project,
    peer_id: &str,
    peer: &Project,
) -> SyncReport {
    let our_hash = snapshot_hash(project);
    let peer_hash = snapshot_hash(peer);
    let conflicts = diff_frontiers(project, peer);

    let now = Utc::now().to_rfc3339();
    let frontier_id = project.frontier_id().clone();

    let synced_reason = format!("synced with peer {peer_id}");
    let synced_event = events::new_finding_event(FindingEventInput {
        kind: "frontier.synced_with_peer",
        finding_id: &frontier_id,
        actor_id: "federation",
        actor_type: "system",
        reason: &synced_reason,
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({
            "peer_id": peer_id,
            "peer_snapshot_hash": peer_hash,
            "our_snapshot_hash": our_hash,
            "divergence_count": conflicts.len(),
        }),
        caveats: Vec::new(),
    });
    let _ = now; // event timestamp comes from new_finding_event

    let mut conflict_events: Vec<StateEvent> = Vec::with_capacity(conflicts.len());
    for c in &conflicts {
        let reason = format!(
            "peer={peer_id} kind={} {}",
            c.kind.as_str(),
            c.detail
        );
        let ev = events::new_finding_event(FindingEventInput {
            kind: "frontier.conflict_detected",
            finding_id: &c.finding_id,
            actor_id: "federation",
            actor_type: "system",
            reason: &reason,
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({
                "peer_id": peer_id,
                "finding_id": c.finding_id,
                "kind": c.kind.as_str(),
                "detail": c.detail,
            }),
            caveats: Vec::new(),
        });
        conflict_events.push(ev);
    }

    let events_appended = 1 + conflict_events.len();
    project.events.push(synced_event);
    project.events.extend(conflict_events);

    SyncReport {
        peer_id: peer_id.to_string(),
        our_snapshot_hash: our_hash,
        peer_snapshot_hash: peer_hash,
        conflicts,
        events_appended,
    }
}

/// v0.39.1: Fetch a peer's frontier JSON over HTTP. The URL is
/// expected to serve a JSON-serialized `Project`. Blocking call —
/// `vela federation sync` is a one-shot CLI verb, not a service.
///
/// Verification of peer signatures (and registry entries) is a
/// separate concern, addressed in v0.39.2+. v0.39.1 trusts the
/// transport so the sync diff/event-emission machinery can be
/// validated against real peer state first.
pub fn fetch_peer_frontier(url: &str) -> Result<Project, String> {
    let resp = reqwest::blocking::get(url)
        .map_err(|e| format!("HTTP GET {url} failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("peer returned HTTP {status}"));
    }
    let body = resp
        .text()
        .map_err(|e| format!("read body from {url}: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("parse peer frontier from {url}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> PeerHub {
        PeerHub {
            id: "hub:test".into(),
            url: "https://example.invalid/".into(),
            public_key: "00".repeat(32),
            added_at: "2026-04-27T00:00:00Z".into(),
            note: String::new(),
        }
    }

    #[test]
    fn validates_correct_shape() {
        assert!(good().validate().is_ok());
    }

    #[test]
    fn rejects_empty_id() {
        let mut p = good();
        p.id = "  ".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_http_url() {
        let mut p = good();
        p.url = "http://insecure.example/".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_short_pubkey() {
        let mut p = good();
        p.public_key = "abcd".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_non_hex_pubkey() {
        let mut p = good();
        p.public_key = "z".repeat(64);
        assert!(p.validate().is_err());
    }

    // ── v0.39.1 sync runtime tests ───────────────────────────────────

    use crate::bundle::{
        Assertion, Conditions, Confidence, Evidence, Extraction, FindingBundle, Flags, Provenance,
        ReviewState,
    };
    use crate::project::{self, Project};

    fn finding(id: &str, score: f64) -> FindingBundle {
        let mut b = FindingBundle::new(
            Assertion {
                text: format!("claim {id}"),
                assertion_type: "mechanism".into(),
                entities: vec![],
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "experimental".into(),
                model_system: String::new(),
                species: None,
                method: String::new(),
                sample_size: Some("n=30".into()),
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            Conditions {
                text: String::new(),
                species_verified: vec![],
                species_unverified: vec![],
                in_vitro: false,
                in_vivo: false,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            Confidence::raw(score, "test", 0.85),
            Provenance {
                source_type: "published_paper".into(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test".into(),
                authors: vec![],
                year: Some(2025),
                journal: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                citation_count: None,
            },
            Flags::default(),
        );
        b.id = id.to_string();
        b
    }

    fn assemble(name: &str, findings: Vec<FindingBundle>) -> Project {
        project::assemble(name, findings, 1, 0, "test")
    }

    #[test]
    fn diff_identical_frontiers_returns_no_conflicts() {
        let f = finding("vf_001", 0.7);
        let ours = assemble("ours", vec![f.clone()]);
        let theirs = assemble("theirs", vec![f]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn diff_detects_missing_in_peer_and_locally() {
        let f1 = finding("vf_001", 0.7);
        let f2 = finding("vf_002", 0.7);
        let ours = assemble("ours", vec![f1.clone()]);
        let theirs = assemble("theirs", vec![f2.clone()]);
        let conflicts = diff_frontiers(&ours, &theirs);
        let kinds: Vec<&str> = conflicts.iter().map(|c| c.kind.as_str()).collect();
        assert!(kinds.contains(&"missing_in_peer"));
        assert!(kinds.contains(&"missing_locally"));
    }

    #[test]
    fn diff_detects_confidence_divergence_above_threshold() {
        let mut f_ours = finding("vf_001", 0.85);
        let mut f_theirs = finding("vf_001", 0.55);
        // Force same id by aligning content; here they share id by construction.
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        let ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert!(
            conflicts.iter().any(|c| c.kind == ConflictKind::ConfidenceDiverged),
            "expected confidence_diverged in {conflicts:?}"
        );
    }

    #[test]
    fn diff_ignores_confidence_drift_below_threshold() {
        let mut f_ours = finding("vf_001", 0.700);
        let mut f_theirs = finding("vf_001", 0.730);
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        let ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert!(
            !conflicts.iter().any(|c| c.kind == ConflictKind::ConfidenceDiverged),
            "0.03 drift should not flag: {conflicts:?}"
        );
    }

    #[test]
    fn diff_detects_retracted_divergence() {
        let mut f_ours = finding("vf_001", 0.7);
        let mut f_theirs = finding("vf_001", 0.7);
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        f_theirs.flags.retracted = true;
        let ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert!(conflicts.iter().any(|c| c.kind == ConflictKind::RetractedDiverged));
    }

    #[test]
    fn diff_detects_review_state_divergence() {
        let mut f_ours = finding("vf_001", 0.7);
        let mut f_theirs = finding("vf_001", 0.7);
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        f_theirs.flags.review_state = Some(ReviewState::Contested);
        let ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert!(conflicts.iter().any(|c| c.kind == ConflictKind::ReviewStateDiverged));
    }

    #[test]
    fn diff_detects_assertion_text_divergence() {
        let mut f_ours = finding("vf_001", 0.7);
        let mut f_theirs = finding("vf_001", 0.7);
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        f_theirs.assertion.text = "different claim".into();
        let ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let conflicts = diff_frontiers(&ours, &theirs);
        assert!(conflicts.iter().any(|c| c.kind == ConflictKind::AssertionTextDiverged));
    }

    #[test]
    fn sync_appends_one_synced_event_plus_one_per_conflict() {
        let mut f_ours = finding("vf_001", 0.7);
        let mut f_theirs = finding("vf_001", 0.7);
        f_ours.id = "vf_001".into();
        f_theirs.id = "vf_001".into();
        f_theirs.flags.retracted = true;
        let mut ours = assemble("ours", vec![f_ours]);
        let theirs = assemble("theirs", vec![f_theirs]);
        let events_before = ours.events.len();
        let report = sync_with_peer(&mut ours, "hub:test-peer", &theirs);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.events_appended, 2); // 1 sync + 1 conflict
        assert_eq!(ours.events.len() - events_before, 2);
        // The first appended event is the sync record.
        let sync_ev = &ours.events[events_before];
        assert_eq!(sync_ev.kind, "frontier.synced_with_peer");
        assert_eq!(sync_ev.payload["divergence_count"].as_u64(), Some(1));
        // The second is the conflict.
        let conf_ev = &ours.events[events_before + 1];
        assert_eq!(conf_ev.kind, "frontier.conflict_detected");
        assert_eq!(conf_ev.payload["kind"], "retracted_diverged");
    }

    #[test]
    fn sync_with_clean_diff_emits_zero_divergence_event() {
        let f = finding("vf_001", 0.7);
        let mut ours = assemble("ours", vec![f.clone()]);
        let theirs = assemble("theirs", vec![f]);
        let report = sync_with_peer(&mut ours, "hub:test-peer", &theirs);
        assert_eq!(report.conflicts.len(), 0);
        assert_eq!(report.events_appended, 1);
        let last = ours.events.last().unwrap();
        assert_eq!(last.kind, "frontier.synced_with_peer");
        assert_eq!(last.payload["divergence_count"].as_u64(), Some(0));
    }
}
