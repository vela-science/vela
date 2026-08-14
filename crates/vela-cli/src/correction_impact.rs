//! `vela correction impact` — what one correction costs the Claims that rest on it.
//!
//! `vela-edge`'s `correction_impact` module has implemented
//! `vela.correction-impact-projection.v1` — dependency traversal, lost and
//! surviving support routes, repair obligations — since it was written, and
//! until now nothing reached it. Its only caller was a test holding synthetic
//! input. `docs/ECOSYSTEM.md` said so plainly: it *"has never run against a real
//! repository, and no CLI verb reaches it."* This module is that verb, and it
//! adds no object: it reads the repository a reader already has and hands the
//! existing derivation the existing input schema.
//!
//! ## What the argument names
//!
//! One Claim that carries a correction relation — `corrects` or `supersedes`,
//! the two kinds `CORRECTION_RELATION_KINDS` closes over. That Claim is the
//! successor; the Claim its relation targets is the predecessor. Naming the
//! successor rather than the predecessor is what makes the verb useful before a
//! Decision as well as after: a correction sitting in `vela review inbox` can be
//! asked what accepting it would cost, which is the question a repository
//! authority actually has.
//!
//! ## Which Claims the answer is computed over
//!
//! The accepted index in the verified repository manifest, plus the named
//! successor when it is still unassessed. Standing is what a correction moves,
//! and an unassessed Claim holds none: letting one carry a support route would
//! report a route surviving on the strength of something no Decision stands
//! over.
//!
//! ## Which relations reach the derivation, and why the rest cannot change the answer
//!
//! The derivation reads three rule kinds. Two of them are relation kinds the
//! protocol retains:
//!
//! | Retained kind | Rule kind | Effect |
//! |---|---|---|
//! | `depends` | `depends_on` | `hard_dependency` |
//! | `supports` | `supports` | `support_route` |
//!
//! The spelling changes across that boundary and the change is ADR 0004's, not
//! this module's: `depends` is the stored wire value and `depends_on` is the
//! derived-graph rendering, and a correction impact projection is a derived
//! graph. The left column is what a Claim Record retains, so that is what is
//! matched on.
//!
//! Three exclusions apply, and each is reported by count and by id rather than
//! applied quietly:
//!
//! - **Unmapped kind.** `replicates`, `contradicts`, `synthesized_from`, and the
//!   correction kinds themselves carry no rule. They are `ClaimRelationClass::
//!   Descriptive` — retained description that moves no Standing — so a
//!   derivation that reads them would be inventing consequence the protocol
//!   does not assign.
//! - **Endpoint not held.** A relation whose target is not in the computed claim
//!   set points at something this repository does not hold. It cannot be the
//!   corrected Claim, and it cannot vouch for a route: a support route to a
//!   Claim the repository has bound no Standing is not support the repository
//!   can offer. Dropping it can only move a source from `route_changed` toward
//!   `repair_required` — the conservative direction — never the reverse.
//! - **Self-loop.** Rejected by `validate_input`, and meaningless here.
//!
//! Because every exclusion is either inert or conservative, the claim set and
//! relation set handed to the derivation are complete over repository state, and
//! `bounds.complete_*` say so. What is excluded is printed alongside the answer,
//! so a reader can check that judgement rather than take it.
//!
//! ## Relation addresses
//!
//! `CorrectionRelation` wants an id and a `sha256:` root, and a relation has
//! neither of its own: it is a field inside a Claim Record, not a retained
//! object. Both are derived here. The id is the readable triple
//! `<source>:<kind>:<target>`, unique by construction once duplicates within one
//! record are folded. The root is the canonical root of a preimage that names
//! the source Claim's own root, so the address is bound to the exact retained
//! bytes the relation was read from — change the Claim and every relation
//! address in it moves.
//!
//! ## Repair conditions
//!
//! `derive_correction_impact` refuses to mint an obligation for a Claim that
//! declares no repair condition, which is right: an obligation nobody can
//! discharge is worse than none. A Claim Record can declare one under the
//! `vela.correction` extension, and where it does that text is used verbatim.
//! Where it does not, the protocol's own default obligation applies —
//! re-establish the Claim against the corrected predecessor, or record why it
//! never rested on the corrected content — and the CLI reports, per obligation,
//! which of the two it was. The distinction stays outside the projection: the
//! projection is exactly `vela.correction-impact-projection.v1` and gains no
//! field here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_edge::correction_impact::{
    ClaimRef, CorrectionBounds, CorrectionClaim, CorrectionImpactInputV1, CorrectionRelation,
    CorrectionRelationRule, CorrectionTransition, correction_impact_projection_root,
    derive_correction_impact,
};
use vela_protocol::claim_record::{CORRECTION_RELATION_KINDS, ClaimRecordV1};
use vela_protocol::repository::ClaimStandingRefV1;

#[derive(Debug)]
pub(crate) struct CorrectionImpactError {
    message: String,
    usage: bool,
}

impl CorrectionImpactError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            usage: true,
        }
    }
}

impl From<String> for CorrectionImpactError {
    fn from(message: String) -> Self {
        Self {
            message,
            usage: false,
        }
    }
}

impl std::fmt::Display for CorrectionImpactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// The extension namespace a Claim Record declares a repair condition under.
///
/// `extensions` is where the protocol puts namespaced domain detail that cannot
/// carry Standing or authority, which is what a repair condition is: the
/// producer's statement of what would discharge an obligation, not a ruling that
/// one exists.
const REPAIR_EXTENSION: &str = "vela.correction";
const REPAIR_FIELD: &str = "repair_condition";

/// What is owed when a Claim declares nothing.
///
/// Constant, because it enters the obligation root as a hash preimage: an
/// obligation whose discharge condition varied with the wording of a default
/// would have an address that moved without any record changing. It states the
/// protocol's own minimum rather than anything about the Claim.
const DEFAULT_REPAIR_CONDITION: &str = "Re-establish this Claim against the corrected predecessor, or record why it does not rest on the corrected content.";

/// The relation-kind mapping, stated once. Left is the spelling a Claim Record
/// retains; right is the rule kind `validate_input` expects.
const RULE_FOR_RETAINED_KIND: &[(&str, &str, &str)] = &[
    ("depends", "depends_on", "hard_dependency"),
    ("supports", "supports", "support_route"),
];

struct HeldClaim {
    reference: ClaimStandingRefV1,
    record: ClaimRecordV1,
}

pub(crate) fn cmd_correction_impact(repository_path: &Path, claim_arg: &str, json_out: bool) {
    crate::ui::set_mode("correction impact", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let (payload, projection) = match correction_impact_payload(repository_path, claim_arg) {
        Ok(result) => result,
        Err(error) if error.usage => {
            crate::cli::fail_kind_return(crate::ui::ErrorKind::Usage, &error.to_string())
        }
        Err(error) => crate::cli::fail_return(&error.to_string()),
    };
    if json_out {
        crate::cli::print_json(&payload);
        return;
    }
    render(&payload, &projection);
}

pub(crate) fn correction_impact_payload(
    repository_path: &Path,
    claim_arg: &str,
) -> Result<
    (
        Value,
        vela_edge::correction_impact::CorrectionImpactProjectionV1,
    ),
    CorrectionImpactError,
> {
    let repository_path = crate::ui::canonicalize_repo(repository_path);

    let repository = crate::repository::load_repository_at(&repository_path, true)?;
    let repository_root = repository.canonical_root()?;

    /* The successor is resolved over both lists so a correction can be asked
    its cost while it is still in the review queue. Everything else is drawn
    from the accepted index alone. */
    let successor_reference = resolve_claim(
        repository
            .accepted_claims
            .iter()
            .chain(&repository.pending_claims),
        claim_arg,
    )?;

    let mut held: BTreeMap<String, HeldClaim> = BTreeMap::new();
    for reference in &repository.accepted_claims {
        let record = crate::repository::read_claim(&repository_path, reference)?;
        held.insert(
            reference.claim_id.clone(),
            HeldClaim {
                reference: reference.clone(),
                record,
            },
        );
    }
    if !held.contains_key(&successor_reference.claim_id) {
        let record = crate::repository::read_claim(&repository_path, &successor_reference)?;
        held.insert(
            successor_reference.claim_id.clone(),
            HeldClaim {
                reference: successor_reference.clone(),
                record,
            },
        );
    }

    let (transition_kind, predecessor_id) = {
        let successor = held
            .get(&successor_reference.claim_id)
            .expect("successor inserted above");
        let correction = successor
            .record
            .relations
            .iter()
            .find(|relation| CORRECTION_RELATION_KINDS.contains(&relation.kind.as_str()))
            .ok_or_else(|| {
                CorrectionImpactError::usage(format!(
                    "Claim {} carries no `corrects` or `supersedes` relation, so it corrects nothing. Name the Claim that carries the correction, not the one it corrects.",
                    successor.reference.claim_id
                ))
            })?;
        let kind = match correction.kind.as_str() {
            "corrects" => "correct_claim",
            _ => "supersede_claim",
        };
        (kind, correction.target_claim_id.clone())
    };
    /* Once the correction is accepted the predecessor is retired and leaves
    the index, so a manifest lookup alone would make the verb answerable only
    before the Decision. The bytes are still retained and still addressed by
    their own root, which is what the content-addressed store is for. */
    let predecessor_retired = !held.contains_key(&predecessor_id);
    if predecessor_retired {
        let retained = read_retained_claim(&repository_path, &predecessor_id)
            .map_err(CorrectionImpactError::usage)?;
        held.insert(predecessor_id.clone(), retained);
    }
    let predecessor_reference = held
        .get(&predecessor_id)
        .expect("predecessor resolved above")
        .reference
        .clone();
    let successor_reference = held
        .get(&successor_reference.claim_id)
        .expect("successor inserted above")
        .reference
        .clone();

    let mut relations = Vec::new();
    let mut excluded_unmapped: BTreeMap<String, usize> = BTreeMap::new();
    let mut excluded_endpoint = Vec::new();
    let mut excluded_self_loop = Vec::new();
    let mut seen_relation_ids = BTreeSet::new();
    for claim in held.values() {
        for relation in &claim.record.relations {
            let Some((_, rule_kind, _)) = RULE_FOR_RETAINED_KIND
                .iter()
                .find(|(retained, _, _)| *retained == relation.kind)
            else {
                *excluded_unmapped.entry(relation.kind.clone()).or_default() += 1;
                continue;
            };
            let relation_id = format!(
                "{}:{}:{}",
                claim.reference.claim_id, rule_kind, relation.target_claim_id
            );
            if claim.reference.claim_id == relation.target_claim_id {
                excluded_self_loop.push(relation_id);
                continue;
            }
            if !held.contains_key(&relation.target_claim_id) {
                excluded_endpoint.push(relation_id);
                continue;
            }
            /* One Claim Record may retain the same edge twice, and the two
            spellings of `depends` fold to one rule kind. Both collapse here
            rather than reaching `validate_input` as a duplicate id. */
            if !seen_relation_ids.insert(relation_id.clone()) {
                continue;
            }
            relations.push(CorrectionRelation {
                relation_root: relation_root(
                    &claim.reference.claim_id,
                    &claim.reference.claim_root,
                    rule_kind,
                    &relation.target_claim_id,
                )?,
                relation_id,
                kind: rule_kind.to_string(),
                source_claim_id: claim.reference.claim_id.clone(),
                target_claim_id: relation.target_claim_id.clone(),
            });
        }
    }

    let mut declared_repair = BTreeSet::new();
    let claims = held
        .values()
        .map(|claim| {
            let condition = declared_repair_condition(&claim.record);
            if condition.is_some() {
                declared_repair.insert(claim.reference.claim_id.clone());
            }
            CorrectionClaim {
                claim_id: claim.reference.claim_id.clone(),
                claim_root: claim.reference.claim_root.clone(),
                repair_condition: Some(
                    condition.unwrap_or_else(|| DEFAULT_REPAIR_CONDITION.to_string()),
                ),
            }
        })
        .collect::<Vec<_>>();

    let used_rule_kinds = relations
        .iter()
        .map(|relation| relation.kind.as_str())
        .collect::<BTreeSet<_>>();
    let relation_rules = RULE_FOR_RETAINED_KIND
        .iter()
        .filter(|(_, rule_kind, _)| used_rule_kinds.contains(rule_kind))
        .map(|(_, rule_kind, effect)| CorrectionRelationRule {
            kind: (*rule_kind).to_string(),
            effect: (*effect).to_string(),
        })
        .collect::<Vec<_>>();

    let input = CorrectionImpactInputV1 {
        schema: vela_edge::correction_impact::CORRECTION_IMPACT_INPUT_SCHEMA_V1.to_string(),
        /* The schema calls this `fixture_id` because its only producer until
        now was a fixture. The field is an opaque identity for the input, so a
        repository names itself and the correction it is being asked about. */
        fixture_id: format!(
            "{}:{}",
            repository.repository_id, successor_reference.claim_id
        ),
        transition: CorrectionTransition {
            kind: transition_kind.to_string(),
            predecessor: ClaimRef {
                claim_id: predecessor_reference.claim_id.clone(),
                claim_root: predecessor_reference.claim_root.clone(),
            },
            successor: ClaimRef {
                claim_id: successor_reference.claim_id.clone(),
                claim_root: successor_reference.claim_root.clone(),
            },
        },
        claims,
        relations,
        relation_rules,
        bounds: CorrectionBounds {
            max_claims: held.len(),
            max_relations: seen_relation_ids.len(),
            complete_claim_set: true,
            complete_relation_set: true,
        },
    };

    let projection = derive_correction_impact(&input)
        .map_err(|error| format!("derive correction impact: {error}"))?;
    let projection_root = correction_impact_projection_root(&projection)?;
    let projection_value =
        serde_json::to_value(&projection).map_err(|error| format!("render projection: {error}"))?;

    let obligations = projection
        .repair_obligations
        .iter()
        .map(|obligation| {
            json!({
                "claim_id": obligation.claim_id,
                "obligation_root": obligation.obligation_root,
                "condition_source": if declared_repair.contains(&obligation.claim_id) {
                    "declared"
                } else {
                    "protocol_default"
                },
            })
        })
        .collect::<Vec<_>>();

    let payload = json!({
        "schema": "vela.correction-impact.v1",
        "ok": true,
        "command": "correction impact",
        "repository_id": repository.repository_id,
        "repository_root": repository_root,
        "successor_standing": crate::claim_standing::from_proposal_status(
            &successor_reference.standing,
        ),
        /* Whether the correction has already been ruled on. Before the
        Decision the predecessor still stands and this reads false; after it,
        the predecessor is retired and its bytes were resolved from the
        content-addressed store rather than the index. */
        "predecessor_retired": predecessor_retired,
        "projection_root": projection_root,
        "projection": projection_value,
        /* Everything the derivation was not given, and why. A reader who
        disagrees with one of these judgements can see exactly which edges it
        cost. */
        "relations_excluded": {
            "unmapped_kind": excluded_unmapped
                .iter()
                .map(|(kind, count)| json!({"kind": kind, "count": count}))
                .collect::<Vec<_>>(),
            "endpoint_not_held": excluded_endpoint,
            "self_loop": excluded_self_loop,
        },
        "repair_conditions": obligations,
    });

    Ok((payload, projection))
}

fn declared_repair_condition(record: &ClaimRecordV1) -> Option<String> {
    let text = record
        .extensions
        .get(REPAIR_EXTENSION)?
        .get(REPAIR_FIELD)?
        .as_str()?
        .trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

/// A relation's derived address. The source Claim's root is in the preimage, so
/// the address is bound to the retained bytes the relation was read from.
fn relation_root(
    source_claim_id: &str,
    source_claim_root: &str,
    rule_kind: &str,
    target_claim_id: &str,
) -> Result<String, String> {
    #[derive(Serialize)]
    struct RelationPreimage<'a> {
        schema: &'static str,
        kind: &'a str,
        source_claim_id: &'a str,
        source_claim_root: &'a str,
        target_claim_id: &'a str,
    }
    let bytes = vela_protocol::canonical::to_canonical_bytes(&RelationPreimage {
        schema: "vela.claim-relation-address.v1",
        kind: rule_kind,
        source_claim_id,
        source_claim_root,
        target_claim_id,
    })?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn resolve_claim<'a>(
    mut references: impl Iterator<Item = &'a ClaimStandingRefV1>,
    argument: &str,
) -> Result<ClaimStandingRefV1, CorrectionImpactError> {
    match references.find(|reference| reference.claim_id == argument) {
        Some(reference) => Ok(reference.clone()),
        None => Err(CorrectionImpactError::usage(format!(
            "this repository holds no Claim {argument}. `vela claims --status all` lists the full ids."
        ))),
    }
}

/// Find one retired Claim in the content-addressed store.
///
/// A Claim the manifest no longer binds still has retained bytes under
/// `records/claims/sha256/<root>.json`, where the file name *is* the root. The
/// bytes are hashed and held to that name before being parsed, so a renamed or
/// altered file is a failure rather than a silent substitution — the same
/// guarantee `read_rooted_object` gives for a Claim the manifest still binds.
///
/// ## A Claim id does not name one file
///
/// `derive_id` hashes a strict subset of the record — schema, revision,
/// assertion, conditions, evidence, provenance — and leaves out `relations`,
/// `created_at` and `extensions`. Two retained records can therefore carry the
/// same `vcl_` id and different roots, which is not exotic: submit a Claim, have
/// it rejected, resubmit the identical assertion and artifact, and the two
/// differ only in `created_at`. Both are retained, because a rejected Claim's
/// bytes stay on disk.
///
/// Returning whichever one `read_dir` yielded first would put an arbitrary root
/// into `transition.predecessor`, which is inside the projection preimage — so
/// the same repository would answer differently on two machines, and the
/// projection root would silently stop matching the one this verb returned
/// before the Decision. Every candidate is collected and an ambiguous id is
/// refused instead.
///
/// The complete fix resolves by root rather than by id: the accepted Decision's
/// applied `ClaimSuperseded` event binds the predecessor's exact root in
/// `before_hash` (`repository.rs` checks it), and the Submission
/// carried the same value as `--target-root`. Reaching either means loading the
/// authority event log here, which this read verb does not otherwise need.
/// Refusing an ambiguous id is the floor: it cannot return a wrong answer, only
/// no answer.
fn read_retained_claim(repository_path: &Path, claim_id: &str) -> Result<HeldClaim, String> {
    let directory = repository_path.join("records/claims/sha256");
    let entries = std::fs::read_dir(&directory)
        .map_err(|error| format!("read retained Claims at {}: {error}", directory.display()))?;
    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("read retained Claim {}: {error}", path.display()))?;
        if hex::encode(Sha256::digest(&bytes)) != stem {
            continue;
        }
        let Ok(record) = ClaimRecordV1::parse(&bytes) else {
            continue;
        };
        if record.claim_id != claim_id {
            continue;
        }
        matches.push(HeldClaim {
            reference: ClaimStandingRefV1 {
                claim_id: record.claim_id.clone(),
                claim_root: format!("sha256:{stem}"),
                standing: "retired".to_string(),
                path: format!("records/claims/sha256/{stem}.json"),
            },
            record,
        });
    }
    matches.sort_by(|left, right| left.reference.claim_root.cmp(&right.reference.claim_root));
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "this repository holds no accepted Claim {claim_id}, and no retained Claim Record carries that id. There is no Standing to move."
        )),
        _ => Err(format!(
            "{claim_id} is carried by {} retained Claim Records at different roots ({}), so which one the correction retired cannot be determined from the id alone. `vela why {claim_id}` reads the Decision that names the exact root.",
            matches.len(),
            matches
                .iter()
                .map(|held| held.reference.claim_root.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn render(
    payload: &Value,
    projection: &vela_edge::correction_impact::CorrectionImpactProjectionV1,
) {
    println!(
        "correction impact · {} · {}",
        projection.transition.kind,
        payload["repository_id"].as_str().unwrap_or("")
    );
    println!(
        "  correcting  {}",
        projection.transition.predecessor.claim_id
    );
    println!(
        "  with        {}  ({})",
        projection.transition.successor.claim_id,
        payload["successor_standing"].as_str().unwrap_or("")
    );
    println!("  status      {}", projection.status);
    for diagnostic in &projection.diagnostics {
        println!("    ! {diagnostic}");
    }
    /* `unaffected_claims` excludes the predecessor and successor by
    construction; `affected_claims` does not. Adding two to the two lists
    therefore counted either of them twice whenever the correction reached
    one. The bound the derivation was handed is the count. */
    println!(
        "  affected    {} of {} held Claim(s)",
        projection.affected_claims.len(),
        projection.bounds.max_claims
    );
    for claim in &projection.affected_claims {
        println!("    {} · {}", claim.claim_id, claim.classification);
        for relation_id in &claim.causal_relation_ids {
            println!("      via {relation_id}");
        }
    }
    println!(
        "  routes      {} lost, {} surviving",
        projection.lost_support_routes.len(),
        projection.surviving_support_routes.len()
    );
    println!("  obligations {}", projection.repair_obligations.len());
    for obligation in payload["repair_conditions"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "    {} · {}",
            obligation["claim_id"].as_str().unwrap_or(""),
            obligation["condition_source"].as_str().unwrap_or("")
        );
    }
    let excluded = &payload["relations_excluded"];
    let unmapped = excluded["unmapped_kind"]
        .as_array()
        .map(|kinds| {
            kinds
                .iter()
                .map(|entry| {
                    format!(
                        "{} {}",
                        entry["count"].as_u64().unwrap_or_default(),
                        entry["kind"].as_str().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if !unmapped.is_empty() {
        println!("  excluded    {unmapped} · retained description, moves no Standing");
    }
    let dangling = excluded["endpoint_not_held"]
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    if dangling > 0 {
        println!("  excluded    {dangling} relation(s) whose target this repository does not hold");
    }
    println!(
        "  projection  {}",
        payload["projection_root"].as_str().unwrap_or("")
    );
}
