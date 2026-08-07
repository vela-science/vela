//! Experimental, non-authoritative correction-impact projection.
//!
//! This reader deliberately consumes a closed, root-bound causal slice rather
//! than interpreting a whole repository. It cannot write Standing, infer a
//! Decision, or turn a foreign transition into local authority.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CORRECTION_IMPACT_INPUT_SCHEMA_V1: &str = "vela.correction-impact-input.v1";
pub const CORRECTION_IMPACT_PROJECTION_SCHEMA_V1: &str = "vela.correction-impact-projection.v1";

/// The domain separator the repair-obligation root is computed under.
///
/// This is a hashing preimage tag, not a wire document. Nothing ever sends
/// these bytes: the object that travels is the projection above, which carries
/// each `RepairObligation` inline. It is stated here beside its two siblings
/// because it was the only one of the three written as a literal inside a
/// function body, which is why nothing outside that function could name it.
///
/// It is deliberately absent from `schemas/`. That directory is generated from
/// `vela-protocol` and asserted to hold exactly what `wire_schema::published()`
/// produces, so publishing this would mean moving the type into the kernel —
/// promoting a non-authoritative analysis to a canonical object by way of a
/// directory. `docs/ECOSYSTEM.md` §6 states the argument in full.
pub const CORRECTION_REPAIR_OBLIGATION_SCHEMA_V1: &str = "vela.correction-repair-obligation.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRef {
    pub claim_id: String,
    pub claim_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionClaim {
    pub claim_id: String,
    pub claim_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_condition: Option<String>,
}

impl CorrectionClaim {
    fn as_ref(&self) -> ClaimRef {
        ClaimRef {
            claim_id: self.claim_id.clone(),
            claim_root: self.claim_root.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionTransition {
    pub kind: String,
    pub predecessor: ClaimRef,
    pub successor: ClaimRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionRelation {
    pub relation_id: String,
    pub relation_root: String,
    pub kind: String,
    pub source_claim_id: String,
    pub target_claim_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionRelationRule {
    pub kind: String,
    pub effect: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionBounds {
    pub max_claims: usize,
    pub max_relations: usize,
    pub complete_claim_set: bool,
    pub complete_relation_set: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionImpactInputV1 {
    pub schema: String,
    pub fixture_id: String,
    pub transition: CorrectionTransition,
    pub claims: Vec<CorrectionClaim>,
    pub relations: Vec<CorrectionRelation>,
    pub relation_rules: Vec<CorrectionRelationRule>,
    pub bounds: CorrectionBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactedClaim {
    pub claim_id: String,
    pub claim_root: String,
    pub classification: String,
    pub causal_relation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportRoute {
    pub relation_id: String,
    pub relation_root: String,
    pub source_claim_id: String,
    pub target_claim_id: String,
    pub target_claim_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairObligation {
    pub obligation_root: String,
    pub claim_id: String,
    pub claim_root: String,
    pub causal_relation_ids: Vec<String>,
    pub discharge_condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionImpactProjectionV1 {
    pub schema: String,
    pub fixture_id: String,
    pub status: String,
    pub transition: CorrectionTransition,
    pub retained_predecessor: ClaimRef,
    pub affected_claims: Vec<ImpactedClaim>,
    pub unaffected_claims: Vec<ClaimRef>,
    pub lost_support_routes: Vec<SupportRoute>,
    pub surviving_support_routes: Vec<SupportRoute>,
    pub repair_obligations: Vec<RepairObligation>,
    pub diagnostics: Vec<String>,
    pub bounds: CorrectionBounds,
}

pub fn correction_impact_projection_root(
    projection: &CorrectionImpactProjectionV1,
) -> Result<String, String> {
    canonical_root(projection)
}

pub fn derive_correction_impact(
    input: &CorrectionImpactInputV1,
) -> Result<CorrectionImpactProjectionV1, String> {
    validate_input(input)?;

    if !input.bounds.complete_claim_set || !input.bounds.complete_relation_set {
        let mut diagnostics = Vec::new();
        if !input.bounds.complete_claim_set {
            diagnostics.push("claim_set_incomplete".to_string());
        }
        if !input.bounds.complete_relation_set {
            diagnostics.push("relation_set_incomplete".to_string());
        }
        return Ok(CorrectionImpactProjectionV1 {
            schema: CORRECTION_IMPACT_PROJECTION_SCHEMA_V1.to_string(),
            fixture_id: input.fixture_id.clone(),
            status: "incomplete".to_string(),
            transition: input.transition.clone(),
            retained_predecessor: input.transition.predecessor.clone(),
            affected_claims: Vec::new(),
            unaffected_claims: Vec::new(),
            lost_support_routes: Vec::new(),
            surviving_support_routes: Vec::new(),
            repair_obligations: Vec::new(),
            diagnostics,
            bounds: input.bounds.clone(),
        });
    }

    let claims = input
        .claims
        .iter()
        .map(|claim| (claim.claim_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    let rules = input
        .relation_rules
        .iter()
        .map(|rule| (rule.kind.as_str(), rule.effect.as_str()))
        .collect::<BTreeMap<_, _>>();
    let unavailable = input.transition.predecessor.claim_id.as_str();
    let mut repair_required = BTreeSet::<String>::new();
    let mut route_changed = BTreeSet::<String>::new();
    let mut causes = BTreeMap::<String, BTreeSet<String>>::new();

    loop {
        let before = (
            repair_required.clone(),
            route_changed.clone(),
            causes.clone(),
        );

        for relation in &input.relations {
            if rules.get(relation.kind.as_str()) != Some(&"hard_dependency") {
                continue;
            }
            if relation.target_claim_id == unavailable
                || repair_required.contains(&relation.target_claim_id)
            {
                let target_causes = causes
                    .get(&relation.target_claim_id)
                    .cloned()
                    .unwrap_or_default();
                let entry = causes.entry(relation.source_claim_id.clone()).or_default();
                entry.insert(relation.relation_id.clone());
                entry.extend(target_causes);
                repair_required.insert(relation.source_claim_id.clone());
                route_changed.remove(&relation.source_claim_id);
            }
        }

        let mut support_by_source = BTreeMap::<&str, Vec<&CorrectionRelation>>::new();
        for relation in &input.relations {
            if rules.get(relation.kind.as_str()) == Some(&"support_route") {
                support_by_source
                    .entry(&relation.source_claim_id)
                    .or_default()
                    .push(relation);
            }
        }
        for (source, relations) in support_by_source {
            let (lost, surviving): (Vec<_>, Vec<_>) = relations.into_iter().partition(|relation| {
                relation.target_claim_id == unavailable
                    || repair_required.contains(&relation.target_claim_id)
            });
            if lost.is_empty() {
                continue;
            }
            let mut lost_causes = BTreeSet::new();
            for relation in &lost {
                lost_causes.insert(relation.relation_id.clone());
                if let Some(target_causes) = causes.get(&relation.target_claim_id).cloned() {
                    lost_causes.extend(target_causes);
                }
            }
            causes
                .entry(source.to_string())
                .or_default()
                .extend(lost_causes);
            if surviving.is_empty() {
                repair_required.insert(source.to_string());
                route_changed.remove(source);
            } else if !repair_required.contains(source) {
                route_changed.insert(source.to_string());
            }
        }

        if before
            == (
                repair_required.clone(),
                route_changed.clone(),
                causes.clone(),
            )
        {
            break;
        }
    }

    let mut affected_claims = repair_required
        .iter()
        .chain(route_changed.iter())
        .map(|claim_id| {
            let claim = claims
                .get(claim_id.as_str())
                .expect("validated relation endpoint");
            ImpactedClaim {
                claim_id: claim.claim_id.clone(),
                claim_root: claim.claim_root.clone(),
                classification: if repair_required.contains(claim_id) {
                    "repair_required"
                } else {
                    "route_changed"
                }
                .to_string(),
                causal_relation_ids: causes
                    .get(claim_id)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    affected_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

    let affected_ids = affected_claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut unaffected_claims = input
        .claims
        .iter()
        .filter(|claim| {
            claim.claim_id != input.transition.predecessor.claim_id
                && claim.claim_id != input.transition.successor.claim_id
                && !affected_ids.contains(claim.claim_id.as_str())
        })
        .map(CorrectionClaim::as_ref)
        .collect::<Vec<_>>();
    unaffected_claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));

    let mut lost_support_routes = Vec::new();
    let mut surviving_support_routes = Vec::new();
    let sources_with_lost_support = input
        .relations
        .iter()
        .filter(|relation| {
            rules.get(relation.kind.as_str()) == Some(&"support_route")
                && (relation.target_claim_id == unavailable
                    || repair_required.contains(&relation.target_claim_id))
        })
        .map(|relation| relation.source_claim_id.as_str())
        .collect::<BTreeSet<_>>();
    for relation in &input.relations {
        if rules.get(relation.kind.as_str()) != Some(&"support_route")
            || !sources_with_lost_support.contains(relation.source_claim_id.as_str())
        {
            continue;
        }
        let target = claims
            .get(relation.target_claim_id.as_str())
            .expect("validated relation target");
        let route = SupportRoute {
            relation_id: relation.relation_id.clone(),
            relation_root: relation.relation_root.clone(),
            source_claim_id: relation.source_claim_id.clone(),
            target_claim_id: relation.target_claim_id.clone(),
            target_claim_root: target.claim_root.clone(),
        };
        if relation.target_claim_id == unavailable
            || repair_required.contains(&relation.target_claim_id)
        {
            lost_support_routes.push(route);
        } else {
            surviving_support_routes.push(route);
        }
    }
    lost_support_routes.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    surviving_support_routes.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));

    let mut repair_obligations = Vec::new();
    for claim_id in &repair_required {
        let claim = claims
            .get(claim_id.as_str())
            .expect("validated affected Claim");
        let discharge_condition = claim
            .repair_condition
            .clone()
            .ok_or_else(|| format!("repair_condition_missing_for_affected_claim:{claim_id}"))?;
        let causal_relation_ids = causes
            .get(claim_id)
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        #[derive(Serialize)]
        struct ObligationPreimage<'a> {
            schema: &'static str,
            claim_id: &'a str,
            claim_root: &'a str,
            causal_relation_ids: &'a [String],
            discharge_condition: &'a str,
        }
        let obligation_root = canonical_root(&ObligationPreimage {
            schema: CORRECTION_REPAIR_OBLIGATION_SCHEMA_V1,
            claim_id: &claim.claim_id,
            claim_root: &claim.claim_root,
            causal_relation_ids: &causal_relation_ids,
            discharge_condition: &discharge_condition,
        })?;
        repair_obligations.push(RepairObligation {
            obligation_root,
            claim_id: claim.claim_id.clone(),
            claim_root: claim.claim_root.clone(),
            causal_relation_ids,
            discharge_condition,
        });
    }

    Ok(CorrectionImpactProjectionV1 {
        schema: CORRECTION_IMPACT_PROJECTION_SCHEMA_V1.to_string(),
        fixture_id: input.fixture_id.clone(),
        status: "complete".to_string(),
        transition: input.transition.clone(),
        retained_predecessor: input.transition.predecessor.clone(),
        affected_claims,
        unaffected_claims,
        lost_support_routes,
        surviving_support_routes,
        repair_obligations,
        diagnostics: Vec::new(),
        bounds: input.bounds.clone(),
    })
}

fn validate_input(input: &CorrectionImpactInputV1) -> Result<(), String> {
    if input.schema != CORRECTION_IMPACT_INPUT_SCHEMA_V1 {
        return Err("correction_impact_schema_invalid".to_string());
    }
    require_text("fixture_id", &input.fixture_id)?;
    if !matches!(
        input.transition.kind.as_str(),
        "correct_claim" | "supersede_claim" | "retract_claim"
    ) {
        return Err("correction_transition_kind_invalid".to_string());
    }
    validate_claim_ref(&input.transition.predecessor)?;
    validate_claim_ref(&input.transition.successor)?;
    if input.transition.predecessor.claim_id == input.transition.successor.claim_id
        || input.transition.predecessor.claim_root == input.transition.successor.claim_root
    {
        return Err("correction_transition_not_distinct".to_string());
    }
    if input.claims.len() > input.bounds.max_claims {
        return Err("correction_claim_bound_exceeded".to_string());
    }
    if input.relations.len() > input.bounds.max_relations {
        return Err("correction_relation_bound_exceeded".to_string());
    }

    let mut claim_ids = BTreeSet::new();
    let mut claim_roots = BTreeSet::new();
    for claim in &input.claims {
        validate_claim_ref(&claim.as_ref())?;
        if !claim_ids.insert(claim.claim_id.as_str()) {
            return Err("correction_claim_id_duplicate".to_string());
        }
        if !claim_roots.insert(claim.claim_root.as_str()) {
            return Err("correction_claim_root_duplicate".to_string());
        }
        if let Some(condition) = &claim.repair_condition {
            require_text("repair_condition", condition)?;
        }
    }
    for required in [&input.transition.predecessor, &input.transition.successor] {
        if !input.claims.iter().any(|claim| {
            claim.claim_id == required.claim_id && claim.claim_root == required.claim_root
        }) {
            return Err("correction_transition_claim_missing".to_string());
        }
    }

    let expected_rules = BTreeMap::from([
        ("depends_on", "hard_dependency"),
        ("discovery", "discovery_only"),
        ("supports", "support_route"),
    ]);
    let mut rules = BTreeMap::new();
    for rule in &input.relation_rules {
        if rules
            .insert(rule.kind.as_str(), rule.effect.as_str())
            .is_some()
        {
            return Err("correction_relation_rule_duplicate".to_string());
        }
        if expected_rules.get(rule.kind.as_str()) != Some(&rule.effect.as_str()) {
            return Err("correction_relation_rule_conflict".to_string());
        }
    }

    let mut relation_ids = BTreeSet::new();
    let mut relation_roots = BTreeSet::new();
    for relation in &input.relations {
        require_text("relation_id", &relation.relation_id)?;
        require_sha256("relation_root", &relation.relation_root)?;
        if !relation_ids.insert(relation.relation_id.as_str()) {
            return Err("correction_relation_id_duplicate".to_string());
        }
        if !relation_roots.insert(relation.relation_root.as_str()) {
            return Err("correction_relation_root_duplicate".to_string());
        }
        if !claim_ids.contains(relation.source_claim_id.as_str())
            || !claim_ids.contains(relation.target_claim_id.as_str())
        {
            return Err("correction_relation_endpoint_missing".to_string());
        }
        if relation.source_claim_id == relation.target_claim_id {
            return Err("correction_relation_self_loop".to_string());
        }
        if !rules.contains_key(relation.kind.as_str()) {
            return Err("correction_relation_unknown".to_string());
        }
    }
    Ok(())
}

fn validate_claim_ref(reference: &ClaimRef) -> Result<(), String> {
    if !is_prefixed_hex(&reference.claim_id, "vcl_", 64) {
        return Err("correction_claim_id_invalid".to_string());
    }
    require_sha256("claim_root", &reference.claim_root)
}

fn require_sha256(_field: &str, value: &str) -> Result<(), String> {
    if is_prefixed_hex(value, "sha256:", 64) {
        Ok(())
    } else {
        Err("correction_sha256_invalid".to_string())
    }
}

fn is_prefixed_hex(value: &str, prefix: &str, length: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == length
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn require_text(_field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        Err("correction_text_invalid".to_string())
    } else {
        Ok(())
    }
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}
