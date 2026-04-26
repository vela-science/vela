//! Stage 5: ASSEMBLE — build the project with stats and metadata.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::bundle::{ConfidenceUpdate, FindingBundle, ReviewEvent};
use crate::events::StateEvent;
use crate::proposals::{ProofState, StateProposal};
use crate::sign::{ActorRecord, SignedEnvelope};
use crate::sources::{ConditionRecord, EvidenceAtom, SourceRecord};

/// A dependency on another project (like a Cargo dependency for science).
///
/// v0.8 extends this with three optional fields that turn it into a
/// **cross-frontier dependency**: when `vfr_id` is set, the entry pins
/// a remote frontier by its content-addressed id and a snapshot hash.
/// `Link.target` values of the form `vf_<id>@vfr_<id>` resolve through
/// here. Without `vfr_id`, the entry behaves as a pre-v0.8 compile-time
/// dependency record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectDependency {
    pub name: String,
    pub source: String,
    pub version: Option<String>,
    pub pinned_hash: Option<String>,
    /// v0.8: content-addressed id of the dependent frontier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vfr_id: Option<String>,
    /// v0.8: where to fetch the dependent frontier file from
    /// (typically an `https://…` URL pointing at raw JSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    /// v0.8: SHA-256 of the canonical snapshot the dependent commits
    /// to. Strict pull verifies the fetched dependency's actual
    /// `snapshot_hash` matches this value before satisfying any link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_snapshot_hash: Option<String>,
}

impl ProjectDependency {
    /// True if this entry declares a cross-frontier dependency
    /// (`vfr_id` is set). Pre-v0.8 entries return `false`.
    pub fn is_cross_frontier(&self) -> bool {
        self.vfr_id.is_some()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub vela_version: String,
    pub schema: String,
    /// Stable Vela-addressable frontier ID, derived from a `frontier.created`
    /// genesis event hash. Optional for backward compatibility with v0.2
    /// frontiers; new v0.3 frontiers populate it on `assemble()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontier_id: Option<String>,
    #[serde(rename = "frontier")]
    pub project: ProjectMeta,
    pub stats: ProjectStats,
    pub findings: Vec<FindingBundle>,
    /// Source artifacts that produced evidence-bearing units.
    #[serde(default)]
    pub sources: Vec<SourceRecord>,
    /// Materialized source-grounded evidence units linked to findings.
    #[serde(default)]
    pub evidence_atoms: Vec<EvidenceAtom>,
    /// Materialized condition boundaries used to avoid claim overgeneralization.
    #[serde(default)]
    pub condition_records: Vec<ConditionRecord>,
    /// Append-only log of review events (content-addressed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_events: Vec<ReviewEvent>,
    /// Append-only log of confidence updates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub confidence_updates: Vec<ConfidenceUpdate>,
    /// Canonical append-only event log for replayable frontier state.
    #[serde(default)]
    pub events: Vec<StateEvent>,
    /// Portable pending/applied proposal records for proposal-first writes.
    #[serde(default)]
    pub proposals: Vec<StateProposal>,
    /// Frontier-local proof freshness projection.
    #[serde(default)]
    pub proof_state: ProofState,
    /// Cryptographic signatures for findings (Ed25519).
    #[serde(default)]
    pub signatures: Vec<SignedEnvelope>,
    /// Registered actor identities, mapping a stable actor.id to an
    /// Ed25519 public key. Phase M (v0.4): once an actor is registered,
    /// any canonical event referencing that actor.id under
    /// `--strict` must carry a verifiable Ed25519 signature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actors: Vec<ActorRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub description: String,
    pub compiled_at: String,
    pub compiler: String,
    pub papers_processed: usize,
    pub errors: usize,
    #[serde(default)]
    pub dependencies: Vec<ProjectDependency>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProjectStats {
    pub findings: usize,
    pub links: usize,
    pub replicated: usize,
    pub unreplicated: usize,
    pub avg_confidence: f64,
    pub gaps: usize,
    pub negative_space: usize,
    pub contested: usize,
    pub categories: HashMap<String, usize>,
    pub link_types: HashMap<String, usize>,
    pub human_reviewed: usize,
    /// Number of review events in this frontier.
    #[serde(default)]
    pub review_event_count: usize,
    /// Number of confidence updates in this frontier.
    #[serde(default)]
    pub confidence_update_count: usize,
    /// Number of canonical state events in this frontier.
    #[serde(default)]
    pub event_count: usize,
    /// Number of source records in the frontier source registry.
    #[serde(default)]
    pub source_count: usize,
    /// Number of materialized evidence atoms in the frontier.
    #[serde(default)]
    pub evidence_atom_count: usize,
    /// Number of materialized condition records in the frontier.
    #[serde(default)]
    pub condition_record_count: usize,
    /// Number of persisted proposals in the frontier.
    #[serde(default)]
    pub proposal_count: usize,
    pub confidence_distribution: ConfidenceDistribution,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ConfidenceDistribution {
    pub high_gt_80: usize,
    pub medium_60_80: usize,
    pub low_lt_60: usize,
}

/// Schema and compiler defaults for the current Vela protocol release.
pub const VELA_SCHEMA_URL: &str = "https://vela.science/schema/finding-bundle/v0.10.0";
pub const VELA_SCHEMA_VERSION: &str = "0.10.0";
pub const VELA_COMPILER_VERSION: &str = "vela/0.29.3";

/// Derive a `vfr_<hash>` frontier ID from frontier metadata. Used as a
/// fallback for legacy frontiers without a `frontier.created` genesis
/// event; v0.4+ frontiers derive from the genesis event itself via
/// `frontier_id_from_genesis`.
#[must_use]
pub fn derive_frontier_id_from_meta(meta: &ProjectMeta) -> String {
    let preimage = serde_json::json!({
        "name": meta.name,
        "compiled_at": meta.compiled_at,
        "compiler": meta.compiler,
    });
    let bytes = crate::canonical::to_canonical_bytes(&preimage).unwrap_or_default();
    use sha2::{Digest, Sha256};
    format!("vfr_{}", &hex::encode(Sha256::digest(bytes))[..16])
}

/// Derive a `vfr_<hash>` frontier ID from the canonical hash of the
/// `frontier.created` genesis event. Returns `None` if `events[0]` is
/// absent or not a `frontier.created` event (legacy frontiers fall back
/// to meta-derivation via `derive_frontier_id_from_meta`).
///
/// The preimage shape matches `event_id` exactly so the same canonical
/// rule produces both the event's `vev_…` and the frontier's `vfr_…`
/// from the same logical content. Doctrine line: a frontier IS what the
/// `frontier.created` event creates.
#[must_use]
pub fn frontier_id_from_genesis(events: &[crate::events::StateEvent]) -> Option<String> {
    let genesis = events.first()?;
    if genesis.kind != "frontier.created" {
        return None;
    }
    let preimage = serde_json::json!({
        "schema": genesis.schema,
        "kind": genesis.kind,
        "target": genesis.target,
        "actor": genesis.actor,
        "timestamp": genesis.timestamp,
        "reason": genesis.reason,
        "before_hash": genesis.before_hash,
        "after_hash": genesis.after_hash,
        "payload": genesis.payload,
        "caveats": genesis.caveats,
    });
    let bytes = crate::canonical::to_canonical_bytes(&preimage).ok()?;
    use sha2::{Digest, Sha256};
    Some(format!("vfr_{}", &hex::encode(Sha256::digest(bytes))[..16]))
}

/// Construct the `frontier.created` canonical event for a freshly
/// compiled frontier. The event becomes `events[0]` and the frontier_id
/// derives from its canonical hash.
///
/// Targets `frontier:<name>` (not `finding:…`) so replay's orphan-target
/// detection does not flag it; the genesis event carries identity, not a
/// finding mutation.
fn build_genesis_event(name: &str, compiled_at: &str, creator: &str) -> crate::events::StateEvent {
    use crate::events::{EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget};
    let mut event = StateEvent {
        schema: EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: "frontier.created".to_string(),
        target: StateTarget {
            r#type: "frontier".to_string(),
            id: name.to_string(),
        },
        actor: StateActor {
            id: creator.to_string(),
            r#type: "frontier".to_string(),
        },
        timestamp: compiled_at.to_string(),
        reason: "frontier compiled".to_string(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: serde_json::json!({
            "name": name,
            "creator": creator,
            "schema_version": VELA_SCHEMA_VERSION,
            "compiled_at": compiled_at,
        }),
        caveats: vec![],
        signature: None,
    };
    event.id = crate::events::compute_event_id(&event);
    event
}

pub fn assemble(
    name: &str,
    bundles: Vec<FindingBundle>,
    papers_processed: usize,
    errors: usize,
    description: &str,
) -> Project {
    let compiled_at = Utc::now().to_rfc3339();
    let meta = ProjectMeta {
        name: name.to_string(),
        description: description.to_string(),
        compiled_at: compiled_at.clone(),
        compiler: VELA_COMPILER_VERSION.to_string(),
        papers_processed,
        errors,
        dependencies: Vec::new(),
    };
    // Phase J (v0.4): emit a `frontier.created` canonical event as
    // events[0] and derive frontier_id from its canonical hash. The
    // address primitive becomes doctrine-grounded — a frontier IS what
    // the genesis event creates, not a convenience over its metadata.
    let genesis = build_genesis_event(name, &compiled_at, VELA_COMPILER_VERSION);
    let frontier_id = frontier_id_from_genesis(std::slice::from_ref(&genesis));
    let mut project = Project {
        vela_version: VELA_SCHEMA_VERSION.to_string(),
        schema: VELA_SCHEMA_URL.to_string(),
        frontier_id,
        project: meta,
        stats: ProjectStats::default(),
        findings: bundles,
        sources: Vec::new(),
        evidence_atoms: Vec::new(),
        condition_records: Vec::new(),
        review_events: Vec::new(),
        confidence_updates: Vec::new(),
        events: vec![genesis],
        proposals: Vec::new(),
        proof_state: ProofState::default(),
        signatures: Vec::new(),
        actors: Vec::new(),
    };
    crate::sources::materialize_project(&mut project);
    project
}

impl Project {
    /// Return the stable Vela-addressable frontier ID. Prefers the stored
    /// field; if absent, derives from the `frontier.created` genesis
    /// event in `events[0]`; if no genesis event is present, falls back
    /// to meta-derivation (legacy v0.3 frontiers).
    #[must_use]
    pub fn frontier_id(&self) -> String {
        if let Some(id) = self.frontier_id.clone() {
            return id;
        }
        if let Some(id) = frontier_id_from_genesis(&self.events) {
            return id;
        }
        derive_frontier_id_from_meta(&self.project)
    }

    /// Materialize the frontier_id field if absent. Idempotent.
    pub fn ensure_frontier_id(&mut self) -> String {
        if self.frontier_id.is_none() {
            self.frontier_id = Some(self.frontier_id());
        }
        self.frontier_id.clone().unwrap()
    }

    /// v0.8: iterate the cross-frontier dependencies (those with
    /// `vfr_id` set). Pre-v0.8 compile-time deps without `vfr_id`
    /// are filtered out.
    pub fn cross_frontier_deps(&self) -> impl Iterator<Item = &ProjectDependency> {
        self.project
            .dependencies
            .iter()
            .filter(|d| d.is_cross_frontier())
    }

    /// v0.8: look up the dependency record for a specific `vfr_id`.
    /// Returns `None` if no matching cross-frontier dep is declared.
    pub fn dep_for_vfr(&self, vfr_id: &str) -> Option<&ProjectDependency> {
        self.cross_frontier_deps()
            .find(|d| d.vfr_id.as_deref() == Some(vfr_id))
    }
}

#[cfg(test)]
mod cross_frontier_dep_tests {
    use super::*;

    fn dep_local(name: &str) -> ProjectDependency {
        ProjectDependency {
            name: name.into(),
            source: "local".into(),
            version: None,
            pinned_hash: None,
            vfr_id: None,
            locator: None,
            pinned_snapshot_hash: None,
        }
    }

    fn dep_cross(vfr: &str) -> ProjectDependency {
        ProjectDependency {
            name: "ext".into(),
            source: "vela.hub".into(),
            version: None,
            pinned_hash: None,
            vfr_id: Some(vfr.into()),
            locator: Some(format!("https://example.test/{vfr}.json")),
            pinned_snapshot_hash: Some("a".repeat(64)),
        }
    }

    #[test]
    fn is_cross_frontier_only_when_vfr_id_set() {
        assert!(!dep_local("x").is_cross_frontier());
        assert!(dep_cross("vfr_abc").is_cross_frontier());
    }

    #[test]
    fn dep_serializes_byte_identical_when_v0_8_fields_absent() {
        // Backward compat: a pre-v0.8 dep round-trips through serde
        // without emitting any of the new optional v0.8 fields.
        let d = dep_local("legacy");
        let s = serde_json::to_string(&d).unwrap();
        assert!(!s.contains("vfr_id"));
        assert!(!s.contains("locator"));
        assert!(!s.contains("pinned_snapshot_hash"));
    }
}

/// Recompute derived frontier statistics after mechanical edits.
pub fn recompute_stats(project: &mut Project) {
    let total_links: usize = project.findings.iter().map(|b| b.links.len()).sum();

    let mut link_types: HashMap<String, usize> = HashMap::new();
    for b in &project.findings {
        for l in &b.links {
            *link_types.entry(l.link_type.clone()).or_default() += 1;
        }
    }

    let mut categories: HashMap<String, usize> = HashMap::new();
    for b in &project.findings {
        *categories
            .entry(b.assertion.assertion_type.clone())
            .or_default() += 1;
    }

    let replicated = project
        .findings
        .iter()
        .filter(|b| b.evidence.replicated)
        .count();
    let avg_confidence = if project.findings.is_empty() {
        0.0
    } else {
        (project
            .findings
            .iter()
            .map(|b| b.confidence.score)
            .sum::<f64>()
            / project.findings.len() as f64
            * 1000.0)
            .round()
            / 1000.0
    };

    project.stats.findings = project.findings.len();
    project.stats.links = total_links;
    project.stats.replicated = replicated;
    project.stats.unreplicated = project.findings.len().saturating_sub(replicated);
    project.stats.avg_confidence = avg_confidence;
    project.stats.gaps = project.findings.iter().filter(|b| b.flags.gap).count();
    project.stats.negative_space = project
        .findings
        .iter()
        .filter(|b| b.flags.negative_space)
        .count();
    project.stats.contested = project
        .findings
        .iter()
        .filter(|b| b.flags.contested)
        .count();
    project.stats.categories = categories;
    project.stats.link_types = link_types;
    let reviewed_from_legacy = project
        .findings
        .iter()
        .filter_map(|b| {
            b.provenance
                .review
                .as_ref()
                .filter(|r| r.reviewed)
                .map(|_| b.id.clone())
        })
        .collect::<HashSet<_>>();
    let reviewed_from_events = project
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind.as_str(),
                "finding.reviewed"
                    | "finding.noted"
                    | "finding.caveated"
                    | "finding.confidence_revised"
                    | "finding.rejected"
                    | "finding.retracted"
            )
        })
        .filter(|event| {
            project
                .findings
                .iter()
                .any(|finding| finding.id == event.target.id)
        })
        .map(|event| event.target.id.clone())
        .collect::<HashSet<_>>();
    let reviewed_ids = reviewed_from_legacy.union(&reviewed_from_events).count();
    project.stats.human_reviewed = reviewed_ids;
    let canonical_review_events = project
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind.as_str(),
                "finding.reviewed"
                    | "finding.noted"
                    | "finding.caveated"
                    | "finding.rejected"
                    | "finding.retracted"
                    | "finding.asserted"
            )
        })
        .count();
    project.stats.review_event_count = canonical_review_events + project.review_events.len();
    project.stats.confidence_update_count = project
        .events
        .iter()
        .filter(|event| event.kind == "finding.confidence_revised")
        .count()
        + project.confidence_updates.len();
    project.stats.event_count = project.events.len();
    project.stats.source_count = project.sources.len();
    project.stats.evidence_atom_count = project.evidence_atoms.len();
    project.stats.condition_record_count = project.condition_records.len();
    project.stats.proposal_count = project.proposals.len();
    project.stats.confidence_distribution = ConfidenceDistribution {
        high_gt_80: project
            .findings
            .iter()
            .filter(|b| b.confidence.score > 0.8)
            .count(),
        medium_60_80: project
            .findings
            .iter()
            .filter(|b| (0.6..=0.8).contains(&b.confidence.score))
            .count(),
        low_lt_60: project
            .findings
            .iter()
            .filter(|b| b.confidence.score < 0.6)
            .count(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::*;

    fn make_finding(
        id: &str,
        score: f64,
        assertion_type: &str,
        replicated: bool,
        gap: bool,
    ) -> FindingBundle {
        FindingBundle {
            id: id.into(),
            version: 1,
            previous_version: None,
            assertion: Assertion {
                text: format!("Finding {id}"),
                assertion_type: assertion_type.into(),
                entities: vec![],
                relation: None,
                direction: None,
            },
            evidence: Evidence {
                evidence_type: "experimental".into(),
                model_system: String::new(),
                species: None,
                method: String::new(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated,
                replication_count: None,
                evidence_spans: vec![],
            },
            conditions: Conditions {
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
            confidence: Confidence::legacy(score, "seeded prior", 0.85),
            provenance: Provenance {
                source_type: "published_paper".into(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test".into(),
                authors: vec![],
                year: Some(2024),
                journal: None,
                license: None,
                publisher: None,
                funders: vec![],
                extraction: Extraction::default(),
                review: None,
                citation_count: None,
            },
            flags: Flags {
                gap,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            },
            links: vec![],
            annotations: vec![],
            attachments: vec![],
            created: String::new(),
            updated: None,
        }
    }

    #[test]
    fn empty_frontier() {
        let c = assemble("test", vec![], 0, 0, "empty");
        assert_eq!(c.stats.findings, 0);
        assert_eq!(c.stats.links, 0);
        assert_eq!(c.stats.avg_confidence, 0.0);
        assert_eq!(c.stats.replicated, 0);
        assert_eq!(c.stats.unreplicated, 0);
        assert_eq!(c.project.name, "test");
        assert_eq!(c.project.description, "empty");
    }

    #[test]
    fn findings_count() {
        let bundles = vec![
            make_finding("f1", 0.8, "mechanism", false, false),
            make_finding("f2", 0.6, "therapeutic", true, false),
            make_finding("f3", 0.9, "mechanism", false, true),
        ];
        let c = assemble("test", bundles, 5, 1, "desc");
        assert_eq!(c.stats.findings, 3);
        assert_eq!(c.project.papers_processed, 5);
        assert_eq!(c.project.errors, 1);
    }

    #[test]
    fn replicated_unreplicated_counts() {
        let bundles = vec![
            make_finding("f1", 0.8, "mechanism", true, false),
            make_finding("f2", 0.6, "mechanism", true, false),
            make_finding("f3", 0.9, "mechanism", false, false),
        ];
        let c = assemble("test", bundles, 3, 0, "desc");
        assert_eq!(c.stats.replicated, 2);
        assert_eq!(c.stats.unreplicated, 1);
    }

    #[test]
    fn category_counts() {
        let bundles = vec![
            make_finding("f1", 0.8, "mechanism", false, false),
            make_finding("f2", 0.6, "mechanism", false, false),
            make_finding("f3", 0.9, "therapeutic", false, false),
        ];
        let c = assemble("test", bundles, 3, 0, "desc");
        assert_eq!(*c.stats.categories.get("mechanism").unwrap(), 2);
        assert_eq!(*c.stats.categories.get("therapeutic").unwrap(), 1);
    }

    #[test]
    fn link_counting() {
        let mut f1 = make_finding("f1", 0.8, "mechanism", false, false);
        f1.add_link("f2", "extends", "shared entity");
        f1.add_link("f3", "contradicts", "opposite direction");
        let f2 = make_finding("f2", 0.7, "mechanism", false, false);
        let c = assemble("test", vec![f1, f2], 2, 0, "desc");
        assert_eq!(c.stats.links, 2);
        assert_eq!(*c.stats.link_types.get("extends").unwrap(), 1);
        assert_eq!(*c.stats.link_types.get("contradicts").unwrap(), 1);
    }

    #[test]
    fn avg_confidence() {
        let bundles = vec![
            make_finding("f1", 0.8, "mechanism", false, false),
            make_finding("f2", 0.6, "mechanism", false, false),
        ];
        let c = assemble("test", bundles, 2, 0, "desc");
        assert!((c.stats.avg_confidence - 0.7).abs() < 0.01);
    }

    #[test]
    fn confidence_distribution_buckets() {
        let bundles = vec![
            make_finding("f1", 0.9, "mechanism", false, false), // high
            make_finding("f2", 0.85, "mechanism", false, false), // high
            make_finding("f3", 0.7, "mechanism", false, false), // medium
            make_finding("f4", 0.6, "mechanism", false, false), // medium (0.6 is in 0.6..=0.8)
            make_finding("f5", 0.4, "mechanism", false, false), // low
        ];
        let c = assemble("test", bundles, 5, 0, "desc");
        assert_eq!(c.stats.confidence_distribution.high_gt_80, 2);
        assert_eq!(c.stats.confidence_distribution.medium_60_80, 2);
        assert_eq!(c.stats.confidence_distribution.low_lt_60, 1);
    }

    #[test]
    fn gaps_counted() {
        let bundles = vec![
            make_finding("f1", 0.8, "mechanism", false, true),
            make_finding("f2", 0.6, "mechanism", false, false),
            make_finding("f3", 0.9, "mechanism", false, true),
        ];
        let c = assemble("test", bundles, 3, 0, "desc");
        assert_eq!(c.stats.gaps, 2);
    }

    #[test]
    fn metadata_preserved() {
        let c = assemble("my frontier", vec![], 10, 2, "A description");
        assert_eq!(c.project.name, "my frontier");
        assert_eq!(c.project.description, "A description");
        assert_eq!(c.project.papers_processed, 10);
        assert_eq!(c.project.errors, 2);
        assert_eq!(c.vela_version, VELA_SCHEMA_VERSION);
        assert!(!c.project.compiled_at.is_empty());
    }
}
