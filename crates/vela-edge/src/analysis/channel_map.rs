//! Channel map: a pure projection of a frontier's attack-channel ledger.
//!
//! A *channel* is a named line of attack on a problem (curated in a
//! `channels.yaml` next to the frontier). Attempts opt into a channel by
//! naming a `channel:<id>` obstruction; accepted findings can close one by
//! carrying a `channel`-namespace anchor. The projection folds the attempt
//! ledger into one status per channel — Open, Cold (enough independent
//! failures to deprioritize), or Dead (an accepted finding closed it) — so
//! the next producer reads where the frontier already ground to a halt
//! instead of re-searching it.
//!
//! Everything here is deterministic from the event log: attempt timestamps
//! come from `attempt.deposited` events (never wall clock), decay is
//! measured against the newest event in the log, and dead channels come
//! from the reducer's `anchor.attached` projection (`Project.anchor_links`)
//! filtered to accepted findings. Two calls on the same project serialize
//! byte-identically.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use vela_protocol::attempt::{Attempt, AttemptResolution};
use vela_protocol::bundle::ReviewState;
use vela_protocol::project::Project;

pub const CHANNELS_SCHEMA: &str = "vela.channels.v0.1";

/// One curated channel under a problem: a stable id (`erdos647:prime`) and a
/// human title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub id: String,
    pub title: String,
}

/// The curated channel taxonomy, deserialized from a frontier's
/// `channels.yaml`. Curation is human judgment; the statuses derived over it
/// are mechanical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelTaxonomy {
    #[serde(default)]
    pub schema: String,
    /// A channel goes cold at this much weighted failure...
    pub cold_threshold: f64,
    /// ...but only when the failures came from at least this many distinct
    /// producers (one producer grinding alone is not evidence the channel is
    /// cold, only that one configuration is).
    pub cold_min_producers: usize,
    /// Attempts older than this many days (relative to the newest event in
    /// the log, never wall clock) weigh 0.5 instead of 1.0.
    pub decay_days: i64,
    /// problem number (as a string key) → its curated channels.
    #[serde(default)]
    pub problems: BTreeMap<String, Vec<ChannelDef>>,
}

impl ChannelTaxonomy {
    /// Parse a `channels.yaml` body.
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        serde_yaml::from_str(text).map_err(|e| format!("parse channels.yaml: {e}"))
    }

    /// Load the taxonomy curated next to a frontier: `<dir>/channels.yaml`
    /// (the parent directory when `path` points at a frontier.json file).
    /// `None` when no taxonomy is curated — an absent file is not an error.
    #[must_use]
    pub fn load_for_frontier(path: &Path) -> Option<Self> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(Path::new("."))
        };
        let body = std::fs::read_to_string(dir.join("channels.yaml")).ok()?;
        Self::from_yaml(&body).ok()
    }
}

/// The derived state of one channel. Never adjudicated by a model: Cold is
/// arithmetic over the ledger, Dead is an accepted finding's anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelState {
    /// Still worth a pass (or never tried).
    Open,
    /// Enough independent weighted failures to route around by default.
    Cold,
    /// An accepted finding carries a `channel`-namespace anchor closing it.
    Dead,
}

/// One channel's derived status: the ledger totals and the state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelStatus {
    pub channel_id: String,
    pub title: String,
    /// Attempts naming this channel (any `channel:<id>` obstruction).
    pub tried: usize,
    /// The failed subset (self-reported failure or an adjudicated refuted
    /// resolution).
    pub failed: usize,
    /// Distinct producers behind the attempts (keyed on config digest).
    pub distinct_producers: usize,
    /// Failed attempts weighted by age: 1.0 recent, 0.5 past `decay_days`.
    pub weighted_failed: f64,
    /// Newest deposit timestamp among the attempts, from the event log.
    pub last_tried: Option<String>,
    pub status: ChannelState,
}

/// The channel an attempt names: the id after its first `channel:`-prefixed
/// named obstruction (`channel:erdos647:prime` → `erdos647:prime`). This is
/// the fold key's channel component; the map itself matches any entry.
#[must_use]
pub fn attempt_channel(a: &Attempt) -> Option<&str> {
    a.named_obstructions
        .iter()
        .find_map(|o| o.strip_prefix("channel:"))
}

/// Whether an attempt counts as failed for the channel ledger: the
/// proposer's own failed/refuted self-report (`claimed_status` is
/// display-only for trust, but for the failure *denominator* the honest
/// self-report is exactly the signal), or an adjudicated `Refuted` head
/// resolution.
#[must_use]
pub fn attempt_is_failed(project: &Project, a: &Attempt) -> bool {
    let claimed = a.claimed_status.to_ascii_lowercase();
    if claimed.contains("fail") || claimed == "refuted" {
        return true;
    }
    matches!(
        project
            .head_resolution(&a.attempt_id)
            .map(|r| &r.resolution),
        Some(AttemptResolution::Refuted { .. })
    )
}

/// The producer identity behind an attempt, for the distinct-producer count.
/// Prefers the config digest (two runs of one system with different configs
/// are different searches), then system@version, then the signer.
fn producer_key(a: &Attempt) -> String {
    if !a.producer.config_digest.is_empty() {
        return a.producer.config_digest.clone();
    }
    if !a.producer.system.is_empty() {
        return format!("{}@{}", a.producer.system, a.producer.version);
    }
    a.signer_pubkey_hex.clone()
}

/// The attempt's deposit time, read from the log (`attempt.deposited`
/// events targeting it), falling back to the attempt's own provenance date
/// for grafted attempts with no deposit event. Never wall clock.
fn attempt_timestamp(project: &Project, a: &Attempt) -> Option<String> {
    let from_log = project
        .events
        .iter()
        .filter(|e| e.kind == "attempt.deposited" && e.target.id == a.attempt_id)
        .map(|e| e.timestamp.as_str())
        .max();
    match from_log {
        Some(ts) => Some(ts.to_string()),
        None if !a.provenance.date.is_empty() => Some(a.provenance.date.clone()),
        None => None,
    }
}

/// Age weight: 0.5 when the attempt is more than `decay_days` older than the
/// newest event in the log, 1.0 otherwise (including when either timestamp
/// does not parse — an undatable failure keeps full weight rather than
/// silently fading).
fn age_weight(attempt_ts: Option<&str>, newest_ts: Option<&str>, decay_days: i64) -> f64 {
    let (Some(at), Some(newest)) = (attempt_ts, newest_ts) else {
        return 1.0;
    };
    let (Ok(at), Ok(newest)) = (
        chrono::DateTime::parse_from_rfc3339(at),
        chrono::DateTime::parse_from_rfc3339(newest),
    ) else {
        return 1.0;
    };
    if newest.signed_duration_since(at).num_days() > decay_days {
        0.5
    } else {
        1.0
    }
}

/// Channel ids closed by an accepted finding: the reducer's
/// `anchor.attached` projection filtered to `channel`-namespace anchors
/// whose target finding is accepted (and not retracted). Deterministic from
/// the log — `Project.anchor_links` is itself a replay projection.
fn dead_channels(project: &Project) -> BTreeSet<String> {
    let mut dead = BTreeSet::new();
    for link in &project.anchor_links {
        if link.anchor.namespace != "channel" {
            continue;
        }
        let accepted = project.findings.iter().any(|f| {
            f.id == link.target
                && !f.flags.retracted
                && f.flags.review_state == Some(ReviewState::Accepted)
        });
        if accepted {
            dead.insert(link.anchor.id.clone());
        }
    }
    dead
}

/// The channel map: one [`ChannelStatus`] per curated (problem, channel), in
/// taxonomy order (problems sorted by key, channels in curated order). Pure
/// and deterministic — two calls on the same project serialize identically.
#[must_use]
pub fn channel_map(project: &Project, taxonomy: &ChannelTaxonomy) -> Vec<ChannelStatus> {
    let dead = dead_channels(project);
    let newest_ts = project
        .events
        .iter()
        .map(|e| e.timestamp.as_str())
        .max()
        .map(str::to_string);

    let mut out = Vec::new();
    for defs in taxonomy.problems.values() {
        for def in defs {
            let mut tried = 0usize;
            let mut failed = 0usize;
            let mut weighted_failed = 0.0f64;
            let mut producers: BTreeSet<String> = BTreeSet::new();
            let mut last_tried: Option<String> = None;
            for a in &project.attempts {
                let names_channel = a
                    .named_obstructions
                    .iter()
                    .any(|o| o.strip_prefix("channel:") == Some(def.id.as_str()));
                if !names_channel {
                    continue;
                }
                tried += 1;
                producers.insert(producer_key(a));
                let ts = attempt_timestamp(project, a);
                if ts > last_tried {
                    last_tried.clone_from(&ts);
                }
                if attempt_is_failed(project, a) {
                    failed += 1;
                    weighted_failed +=
                        age_weight(ts.as_deref(), newest_ts.as_deref(), taxonomy.decay_days);
                }
            }
            let status = if dead.contains(&def.id) {
                ChannelState::Dead
            } else if weighted_failed >= taxonomy.cold_threshold
                && producers.len() >= taxonomy.cold_min_producers
            {
                ChannelState::Cold
            } else {
                ChannelState::Open
            };
            out.push(ChannelStatus {
                channel_id: def.id.clone(),
                title: def.title.clone(),
                tried,
                failed,
                distinct_producers: producers.len(),
                weighted_failed,
                last_tried,
                status,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::anchor::{Anchor, AnchorKind, AnchorLink, AnchorLinkDraft, JoinPolicy};
    use vela_protocol::attempt::{AttemptDraft, ProducerRef};
    use vela_protocol::test_support::{make_finding, make_project};

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn taxonomy() -> ChannelTaxonomy {
        ChannelTaxonomy::from_yaml(
            r#"
schema: vela.channels.v0.1
cold_threshold: 3
cold_min_producers: 2
decay_days: 45
problems:
  "647":
    - id: "erdos647:prime"
      title: "prime-power / prime channel"
    - id: "erdos647:crt_cover"
      title: "CRT covering systems"
"#,
        )
        .unwrap()
    }

    fn failed_attempt(claim: &str, config: &str, seed: u8) -> Attempt {
        let draft = AttemptDraft {
            problem: 647,
            frontier: "erdos-frontier".into(),
            kind: "upper_bound".into(),
            claim: claim.into(),
            claimed_status: "failed".into(),
            named_obstructions: vec!["channel:erdos647:prime".into()],
            method_families: vec!["sieve".into()],
            producer: ProducerRef {
                system: "test-producer".into(),
                version: "1".into(),
                config_digest: config.into(),
            },
            ..Default::default()
        };
        Attempt::build(draft, &key(seed)).unwrap()
    }

    /// Push an `attempt.deposited` event at a controlled timestamp so decay
    /// is exercised against the log, never wall clock.
    fn deposit_at(project: &mut Project, a: &Attempt, ts: &str) {
        let mut ev = a.deposit_event("agent:test", "agent", "test deposit");
        ev.timestamp = ts.to_string();
        project.events.push(ev);
        project.attempts.push(a.clone());
    }

    fn pinned_project() -> Project {
        let mut project = make_project("channels", vec![]);
        // Pin the genesis timestamp so the newest-event anchor is controlled.
        project.events[0].timestamp = "2026-01-01T00:00:00Z".into();
        project
    }

    #[test]
    fn cold_needs_threshold_and_distinct_producers() {
        let mut project = pinned_project();
        // Three recent failures from two distinct producers -> Cold.
        deposit_at(
            &mut project,
            &failed_attempt("route A", "cfg-1", 1),
            "2026-01-10T00:00:00Z",
        );
        deposit_at(
            &mut project,
            &failed_attempt("route B", "cfg-1", 1),
            "2026-01-11T00:00:00Z",
        );
        deposit_at(
            &mut project,
            &failed_attempt("route C", "cfg-2", 2),
            "2026-01-12T00:00:00Z",
        );
        project.events[0].timestamp = "2026-01-12T00:00:00Z".into();

        let map = channel_map(&project, &taxonomy());
        let prime = map
            .iter()
            .find(|c| c.channel_id == "erdos647:prime")
            .unwrap();
        assert_eq!(prime.tried, 3);
        assert_eq!(prime.failed, 3);
        assert_eq!(prime.distinct_producers, 2);
        assert_eq!(prime.status, ChannelState::Cold);
        assert_eq!(prime.last_tried.as_deref(), Some("2026-01-12T00:00:00Z"));
        // The untouched channel stays open with an empty ledger.
        let crt = map
            .iter()
            .find(|c| c.channel_id == "erdos647:crt_cover")
            .unwrap();
        assert_eq!(crt.tried, 0);
        assert_eq!(crt.status, ChannelState::Open);
    }

    #[test]
    fn one_producer_grinding_alone_stays_open() {
        let mut project = pinned_project();
        for (i, claim) in ["a", "b", "c", "d"].iter().enumerate() {
            deposit_at(
                &mut project,
                &failed_attempt(claim, "cfg-1", 1),
                &format!("2026-01-1{i}T00:00:00Z"),
            );
        }
        let map = channel_map(&project, &taxonomy());
        let prime = map
            .iter()
            .find(|c| c.channel_id == "erdos647:prime")
            .unwrap();
        assert!(prime.weighted_failed >= 3.0);
        assert_eq!(prime.distinct_producers, 1);
        assert_eq!(prime.status, ChannelState::Open);
    }

    #[test]
    fn old_attempts_decay_to_half_weight() {
        let mut project = pinned_project();
        // Two failures 100 days before the newest event (decay_days = 45).
        deposit_at(
            &mut project,
            &failed_attempt("old A", "cfg-1", 1),
            "2026-01-01T00:00:00Z",
        );
        deposit_at(
            &mut project,
            &failed_attempt("old B", "cfg-2", 2),
            "2026-01-02T00:00:00Z",
        );
        // A later event moves the log's newest timestamp forward.
        project.events[0].timestamp = "2026-04-15T00:00:00Z".into();

        let map = channel_map(&project, &taxonomy());
        let prime = map
            .iter()
            .find(|c| c.channel_id == "erdos647:prime")
            .unwrap();
        assert_eq!(prime.failed, 2);
        assert!((prime.weighted_failed - 1.0).abs() < f64::EPSILON);
        assert_eq!(prime.status, ChannelState::Open, "decayed below threshold");
    }

    #[test]
    fn accepted_finding_with_channel_anchor_marks_dead() {
        let mut project = pinned_project();
        let mut f = make_finding("vf_chan_close", 0.9, "mechanism");
        f.flags.review_state = Some(ReviewState::Accepted);
        project.findings.push(f);
        let link = AnchorLink::build(
            AnchorLinkDraft {
                target: "vf_chan_close".into(),
                anchor: Anchor {
                    namespace: "channel".into(),
                    id: "erdos647:prime".into(),
                    role: "channel-closure".into(),
                    kind: AnchorKind::Statement,
                    join_policy: JoinPolicy::SearchOnly,
                    namespace_version: None,
                    source_revision: None,
                    statement_fingerprint: None,
                },
                attached_by: "reviewer:test".into(),
                attached_at: "2026-01-02T00:00:00Z".into(),
            },
            &key(7),
        )
        .unwrap();
        project.anchor_links.push(link);

        let map = channel_map(&project, &taxonomy());
        let prime = map
            .iter()
            .find(|c| c.channel_id == "erdos647:prime")
            .unwrap();
        assert_eq!(prime.status, ChannelState::Dead);
        // An anchor on an unaccepted finding must NOT kill a channel.
        let mut project2 = pinned_project();
        project2
            .findings
            .push(make_finding("vf_chan_close", 0.9, "mechanism"));
        project2.anchor_links = project.anchor_links.clone();
        let map2 = channel_map(&project2, &taxonomy());
        let prime2 = map2
            .iter()
            .find(|c| c.channel_id == "erdos647:prime")
            .unwrap();
        assert_eq!(prime2.status, ChannelState::Open);
    }

    #[test]
    fn projection_is_deterministic_byte_for_byte() {
        let mut project = pinned_project();
        deposit_at(
            &mut project,
            &failed_attempt("route A", "cfg-1", 1),
            "2026-01-10T00:00:00Z",
        );
        deposit_at(
            &mut project,
            &failed_attempt("route B", "cfg-2", 2),
            "2026-01-11T00:00:00Z",
        );
        let tax = taxonomy();
        let first = serde_json::to_vec(&channel_map(&project, &tax)).unwrap();
        let second = serde_json::to_vec(&channel_map(&project, &tax)).unwrap();
        assert_eq!(first, second, "same project, same bytes");
    }
}
