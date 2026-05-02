//! Cross-implementation reducer fixtures.
//!
//! Doctrine: "two implementations of the reducer must agree on the
//! mutation rules per kind." The cascade test in
//! `cascade_replay_at_scale.rs` already proves Rust agrees with Rust at
//! scale. This test exports the same cascade fixtures to JSON files
//! that a second-implementation reducer (e.g. `clients/python/vela_reducer.py`)
//! can consume and verify byte-equivalently.
//!
//! What gets exported per fixture:
//!   - `genesis_findings`: the initial finding bundles (FindingBundle JSON)
//!   - `event_log`: the canonical event log (StateEvent JSON)
//!   - `expected_states`: the post-replay reducer-effects array, sorted
//!     by finding id, capturing only the fields the reducer mutates
//!     (retracted, contested, review_state, confidence_score, annotation_ids)
//!
//! A second-implementation reducer reads `genesis_findings` + `event_log`,
//! applies its own per-kind mutation rules, builds the same shape from
//! its result, and asserts deep equality with `expected_states`. If two
//! implementations agree on this mutation surface across N fixtures with
//! cascade chains, the doctrine is no longer a single-implementation
//! claim.

use serde_json::{Map, Value, json};
use std::path::PathBuf;

use vela_protocol::bundle::{
    Assertion, Author, Confidence, ConfidenceKind, ConfidenceMethod, Conditions, Entity, Evidence,
    Extraction, FindingBundle, Flags, Link, Provenance,
};
use vela_protocol::events::{self, FindingEventInput, NULL_HASH};
use vela_protocol::reducer::replay_from_genesis;

const FIXTURE_FRONTIER_COUNT: usize = 3;
const FINDINGS_PER_FRONTIER: usize = 8;
const CASCADE_DEPTH: usize = 5;

fn make_finding(frontier_idx: usize, finding_idx: usize) -> FindingBundle {
    let assertion = Assertion {
        text: format!(
            "Cross-impl finding {finding_idx} in frontier {frontier_idx}: protein-X activates pathway-Y."
        ),
        assertion_type: "mechanism".into(),
        entities: vec![Entity {
            name: format!("ProteinX{finding_idx}"),
            entity_type: "protein".into(),
            identifiers: Map::new(),
            canonical_id: None,
            candidates: vec![],
            aliases: vec![],
            resolution_provenance: None,
            resolution_confidence: 1.0,
            resolution_method: None,
            species_context: None,
            needs_review: false,
        }],
        relation: Some("activates".into()),
        direction: Some("positive".into()),
        causal_claim: None,
        causal_evidence_grade: None,
    };
    let evidence = Evidence {
        evidence_type: "experimental".into(),
        model_system: "mouse".into(),
        species: Some("Mus musculus".into()),
        method: "Western blot".into(),
        sample_size: Some("n=30".into()),
        effect_size: None,
        p_value: Some("p<0.05".into()),
        replicated: true,
        replication_count: Some(3),
        evidence_spans: vec![],
    };
    let conditions = Conditions {
        text: "In vitro, mouse microglia".into(),
        species_verified: vec!["Mus musculus".into()],
        species_unverified: vec![],
        in_vitro: true,
        in_vivo: false,
        human_data: false,
        clinical_trial: false,
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: Some("microglia".into()),
    };
    let confidence = Confidence {
        kind: ConfidenceKind::FrontierEpistemic,
        score: 0.7,
        basis: "Cross-impl test fixture".into(),
        method: ConfidenceMethod::LlmInitial,
        components: None,
        extraction_confidence: 0.9,
    };
    let provenance = Provenance {
        source_type: "published_paper".into(),
        doi: Some(format!(
            "10.0000/crossimpl.frontier{frontier_idx:04}.finding{finding_idx:04}"
        )),
        pmid: None,
        pmc: None,
        openalex_id: None,
        url: None,
        title: format!("Cross-impl paper {frontier_idx}-{finding_idx}"),
        authors: vec![Author {
            name: "Cross-Impl A".into(),
            orcid: None,
        }],
        year: Some(2026),
        journal: Some("Cross Journal".into()),
        license: None,
        publisher: None,
        funders: vec![],
        extraction: Extraction::default(),
        review: None,
        citation_count: Some(0),
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
        signature_threshold: None,
        jointly_accepted: false,
    };
    let mut bundle = FindingBundle::new(
        assertion, evidence, conditions, confidence, provenance, flags,
    );
    if finding_idx + 1 < FINDINGS_PER_FRONTIER {
        let next_id = synthetic_id(frontier_idx, finding_idx + 1);
        bundle.links = vec![Link {
            target: next_id,
            link_type: "supports".into(),
            note: "synthetic dependency".into(),
            inferred_by: "vela-cross-impl-fixture/0".into(),
            created_at: "2026-05-02T00:00:00Z".into(),
            mechanism: None,
        }];
    }
    bundle
}

fn synthetic_id(frontier_idx: usize, finding_idx: usize) -> String {
    let assertion = Assertion {
        text: format!(
            "Cross-impl finding {finding_idx} in frontier {frontier_idx}: protein-X activates pathway-Y."
        ),
        assertion_type: "mechanism".into(),
        entities: vec![],
        relation: None,
        direction: None,
        causal_claim: None,
        causal_evidence_grade: None,
    };
    let provenance = Provenance {
        source_type: "published_paper".into(),
        doi: Some(format!(
            "10.0000/crossimpl.frontier{frontier_idx:04}.finding{finding_idx:04}"
        )),
        pmid: None,
        pmc: None,
        openalex_id: None,
        url: None,
        title: format!("Cross-impl paper {frontier_idx}-{finding_idx}"),
        authors: vec![],
        year: None,
        journal: None,
        license: None,
        publisher: None,
        funders: vec![],
        extraction: Extraction::default(),
        review: None,
        citation_count: None,
    };
    FindingBundle::content_address(&assertion, &provenance)
}

fn build_event_log(
    frontier_idx: usize,
    findings: &[FindingBundle],
) -> Vec<events::StateEvent> {
    let mut log = Vec::new();
    let actor_id = format!("reviewer:cross-impl-{frontier_idx}");
    for f in findings {
        let proposal_id = format!("vpr_{}_{}", frontier_idx, &f.id[3..]);
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.asserted",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "cross-impl genesis assertion",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id}),
            caveats: vec![],
        }));
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "cross-impl review",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id, "status": "accepted"}),
            caveats: vec![],
        }));
    }
    let root = &findings[0];
    let root_proposal = format!("vpr_{}_{}", frontier_idx, &root.id[3..]);
    let retract = events::new_finding_event(FindingEventInput {
        kind: "finding.retracted",
        finding_id: &root.id,
        actor_id: &actor_id,
        actor_type: "human",
        reason: "cross-impl retraction triggers cascade",
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({
            "proposal_id": root_proposal,
            "affected": CASCADE_DEPTH,
        }),
        caveats: vec![],
    });
    let retract_event_id = retract.id.clone();
    let root_id = root.id.clone();
    log.push(retract);

    for depth in 1..=CASCADE_DEPTH {
        if depth >= findings.len() {
            break;
        }
        let dep = &findings[depth];
        let dep_proposal = format!("vpr_{}_{}", frontier_idx, &dep.id[3..]);
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.dependency_invalidated",
            finding_id: &dep.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "cross-impl cascade",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({
                "proposal_id": dep_proposal,
                "upstream_finding_id": root_id,
                "upstream_event_id": retract_event_id,
                "depth": depth as u64,
            }),
            caveats: vec![],
        }));
    }
    log
}

/// v0.49.3 — Coverage fixture: exercises every dispatch arm in the
/// reducer that the cascade fixtures don't already touch. Each
/// finding gets:
///   - finding.asserted (genesis)
///   - finding.reviewed (rotated through accepted/contested/needs_revision/rejected)
///   - finding.confidence_revised (alternating int and float new_score
///     values to lock the basis-string formatting and the 6-decimal
///     score boundary across implementations)
///
/// This is the fixture the engineer + integrator both flagged as
/// missing. After this, every per-kind branch in
/// reducer.rs::apply_event has at least one cross-impl reproducer.
fn build_review_branches_log(
    frontier_idx: usize,
    findings: &[FindingBundle],
) -> Vec<events::StateEvent> {
    let mut log = Vec::new();
    let actor_id = format!("reviewer:review-branches-{frontier_idx}");
    let statuses = ["accepted", "contested", "needs_revision", "rejected"];
    for (i, f) in findings.iter().enumerate() {
        let proposal_id = format!("vpr_{}_{}", frontier_idx, &f.id[3..]);
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.asserted",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "review-branch genesis",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id}),
            caveats: vec![],
        }));
        // Rotate through every status so all four arms in
        // apply_finding_reviewed land at least once.
        let status = statuses[i % statuses.len()];
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.reviewed",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "review-branch coverage",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id, "status": status}),
            caveats: vec![],
        }));
        // Alternate integer vs fractional new_score to stress the
        // basis-string formatting (Rust {:.3}, Python :.3f, JS
        // .toFixed(3)) and the digest 6-decimal boundary.
        let (prev, new) = if i % 2 == 0 { (0.7, 1.0) } else { (0.7, 0.42_f64) };
        let revise_reason = format!("revise to {new:.3}");
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.confidence_revised",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: &revise_reason,
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({
                "proposal_id": proposal_id,
                "previous_score": prev,
                "new_score": new,
            }),
            caveats: vec![],
        }));
    }
    log
}

/// v0.49.3 — Annotations fixture: exercises both annotation kinds
/// (finding.noted and finding.caveated) plus finding.rejected, the
/// last reducer arms not covered by cascade or review-branches.
fn build_annotations_log(
    frontier_idx: usize,
    findings: &[FindingBundle],
) -> Vec<events::StateEvent> {
    let mut log = Vec::new();
    let actor_id = format!("reviewer:annotations-{frontier_idx}");
    for (i, f) in findings.iter().enumerate() {
        let proposal_id = format!("vpr_{}_{}", frontier_idx, &f.id[3..]);
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.asserted",
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "annotations-fixture genesis",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id}),
            caveats: vec![],
        }));
        // First half get a noted; second half a caveated. Both go
        // through apply_finding_annotation but are dispatched on
        // distinct kinds, so a future reducer that forgets the
        // caveated → annotation route will fail one half.
        let kind = if i < findings.len() / 2 {
            "finding.noted"
        } else {
            "finding.caveated"
        };
        let annotation_id = format!("ann_{}_{}", frontier_idx, i);
        log.push(events::new_finding_event(FindingEventInput {
            kind,
            finding_id: &f.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "annotation coverage",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({
                "proposal_id": proposal_id,
                "annotation_id": annotation_id,
                "text": format!("note {i} on finding {}", &f.id[..8]),
                // Provenance with a doi satisfies the validator's
                // "at least one of doi/pmid/title" rule.
                "provenance": {
                    "doi": format!("10.0000/annot.{frontier_idx}.{i}"),
                },
            }),
            caveats: vec![],
        }));
    }
    // Reject the last finding — the only event kind that's not
    // exercised by cascade, review-branches, or annotations.
    if let Some(last) = findings.last() {
        let proposal_id = format!("vpr_{}_{}", frontier_idx, &last.id[3..]);
        log.push(events::new_finding_event(FindingEventInput {
            kind: "finding.rejected",
            finding_id: &last.id,
            actor_id: &actor_id,
            actor_type: "human",
            reason: "rejection coverage",
            before_hash: NULL_HASH,
            after_hash: NULL_HASH,
            payload: json!({"proposal_id": proposal_id}),
            caveats: vec![],
        }));
    }
    log
}

/// Reducer-effects digest per finding. Captures only the fields the
/// reducer actually mutates, in a shape that's deterministic across
/// implementations. Annotations are reduced to sorted ids since
/// timestamps and authors come from the event itself and a second
/// implementation will reproduce them identically by mirroring the
/// same per-kind mutation rule.
fn finding_state(f: &FindingBundle) -> Value {
    let review_state = f
        .flags
        .review_state
        .as_ref()
        .map(|s| match s {
            vela_protocol::bundle::ReviewState::Accepted => "accepted",
            vela_protocol::bundle::ReviewState::Contested => "contested",
            vela_protocol::bundle::ReviewState::NeedsRevision => "needs_revision",
            vela_protocol::bundle::ReviewState::Rejected => "rejected",
        })
        .unwrap_or("none");
    let mut annotation_ids: Vec<String> = f.annotations.iter().map(|a| a.id.clone()).collect();
    annotation_ids.sort();
    json!({
        "id": f.id,
        "retracted": f.flags.retracted,
        "contested": f.flags.contested,
        "review_state": review_state,
        // Format to 6 decimal places so f64 precision noise can't
        // cross the cross-implementation boundary.
        "confidence_score": format!("{:.6}", f.confidence.score),
        "annotation_ids": annotation_ids,
    })
}

/// Helper: replay an event log from a fresh genesis, sort by id,
/// extract the reducer-effects digest, and write the fixture.
fn export_one(
    out_dir: &PathBuf,
    fixture_idx: usize,
    scenario: &str,
    findings: Vec<FindingBundle>,
    event_log: Vec<events::StateEvent>,
) {
    let post = replay_from_genesis(
        findings.clone(),
        &event_log,
        &format!("Cross-Impl Frontier {fixture_idx} ({scenario})"),
        "Cross-implementation reducer fixture",
        "2026-05-02T00:00:00Z",
        "vela-cross-impl/0",
    )
    .expect("replay must succeed");

    let mut sorted = post.findings.clone();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let expected_states: Vec<Value> = sorted.iter().map(finding_state).collect();

    // Inventory which event kinds appear in this fixture. Lets a
    // reviewer spot-check that the coverage promise is real per
    // fixture, not just "we ship some events."
    let mut kinds_seen: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for ev in &event_log {
        *kinds_seen.entry(ev.kind.clone()).or_insert(0) += 1;
    }
    let kinds_value: Value = serde_json::to_value(&kinds_seen).unwrap();

    let fixture = json!({
        "fixture_version": "1",
        "schema_url": "https://vela.science/schema/cross-impl-reducer-fixture/v1",
        "doctrine": "two implementations of the reducer must agree on the mutation rules per kind",
        "scenario": scenario,
        "frontier_idx": fixture_idx,
        "stats": {
            "findings": findings.len(),
            "events": event_log.len(),
            "cascade_depth": if scenario == "cascade" {
                CASCADE_DEPTH.min(findings.len() - 1)
            } else {
                0
            },
            "kinds_seen": kinds_value,
        },
        "genesis_findings": findings,
        "event_log": event_log,
        "expected_states": expected_states,
    });

    let path = out_dir.join(format!("cascade-fixture-{fixture_idx:02}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&fixture).unwrap())
        .expect("write fixture");
    eprintln!("wrote {}", path.display());
}

#[test]
fn export_cross_impl_reducer_fixtures() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    std::fs::create_dir_all(&out_dir).expect("create fixtures dir");

    // Fixtures 00..02 — cascade scenario (the original 3).
    for frontier_idx in 0..FIXTURE_FRONTIER_COUNT {
        let findings: Vec<FindingBundle> = (0..FINDINGS_PER_FRONTIER)
            .map(|i| make_finding(frontier_idx, i))
            .collect();
        let event_log = build_event_log(frontier_idx, &findings);
        export_one(&out_dir, frontier_idx, "cascade", findings, event_log);
    }

    // Fixture 03 — review-branches + confidence-revised scenario.
    // Exercises every status arm of finding.reviewed plus the
    // confidence-revised path with both integer and fractional
    // new_score values.
    {
        let frontier_idx = FIXTURE_FRONTIER_COUNT;
        let findings: Vec<FindingBundle> = (0..FINDINGS_PER_FRONTIER)
            .map(|i| make_finding(frontier_idx, i))
            .collect();
        let event_log = build_review_branches_log(frontier_idx, &findings);
        export_one(&out_dir, frontier_idx, "review_branches", findings, event_log);
    }

    // Fixture 04 — annotations + rejected scenario. Exercises both
    // finding.noted and finding.caveated (which share a reducer arm
    // but dispatch on distinct kinds) plus finding.rejected.
    {
        let frontier_idx = FIXTURE_FRONTIER_COUNT + 1;
        let findings: Vec<FindingBundle> = (0..FINDINGS_PER_FRONTIER)
            .map(|i| make_finding(frontier_idx, i))
            .collect();
        let event_log = build_annotations_log(frontier_idx, &findings);
        export_one(&out_dir, frontier_idx, "annotations", findings, event_log);
    }
}

/// Coverage-completeness assertion: the union of event kinds across
/// all exported fixtures must include every dispatch arm in
/// `apply_event`. v0.49.3 derives the required-kinds list from
/// `vela_protocol::reducer::REDUCER_MUTATION_KINDS` instead of a
/// hand-maintained mirror, so adding a new arm to the reducer
/// automatically extends the fixture coverage requirement (and the
/// `dispatch_handles_every_declared_kind` test in reducer.rs catches
/// the inverse drift).
#[test]
fn fixture_coverage_includes_every_reducer_arm() {
    use vela_protocol::reducer::REDUCER_MUTATION_KINDS;

    let frontier_idx = 0;
    let findings: Vec<FindingBundle> = (0..FINDINGS_PER_FRONTIER)
        .map(|i| make_finding(frontier_idx, i))
        .collect();

    let mut all_kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for ev in build_event_log(frontier_idx, &findings) {
        all_kinds.insert(ev.kind);
    }
    for ev in build_review_branches_log(frontier_idx, &findings) {
        all_kinds.insert(ev.kind);
    }
    for ev in build_annotations_log(frontier_idx, &findings) {
        all_kinds.insert(ev.kind);
    }

    for kind in REDUCER_MUTATION_KINDS {
        assert!(
            all_kinds.contains(*kind),
            "cross-impl fixture coverage missing reducer arm: {kind} \
             (declared in REDUCER_MUTATION_KINDS but not exercised by \
             any fixture builder)"
        );
    }
}
