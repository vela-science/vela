use std::collections::{BTreeMap, BTreeSet};

use super::reducer::{
    ClaimRef, CorrectionBounds, CorrectionClaim, CorrectionImpactInputV1, CorrectionRelation,
    CorrectionRelationRule, CorrectionTransition, derive_correction_impact,
};
use serde_json::Value;
use vela_protocol::{canonical, is_full_sha256_root, is_repository_id};

const PROFILE: &[u8] =
    include_bytes!("../../../../conformance/experiments/claim-dependency-profile-v0/profile.json");
const STATE: &[u8] =
    include_bytes!("../../../../conformance/experiments/claim-dependency-profile-v0/state.json");
const EXPECTED: &[u8] =
    include_bytes!("../../../../conformance/experiments/claim-dependency-profile-v0/expected.json");
const REPAIR: &str = "Re-establish every exact requires edge against current accepted targets, narrow the Claim, or retract it.";

fn parse(bytes: &[u8]) -> Value {
    canonical::from_json_slice_strict(bytes).expect("strict experiment JSON")
}

fn root(value: &Value) -> String {
    canonical::sha256_root(&canonical::to_canonical_bytes(value).expect("RFC 8785 bytes"))
}

fn adapt(profile: &Value, state: &Value) -> Result<CorrectionImpactInputV1, String> {
    if profile["schema"] != "vela.claim-dependency-profile.v0" || profile["profile_version"] != 0 {
        return Err("profile_schema_unsupported".to_string());
    }
    let repository_id = profile["scope"]["repository_id"]
        .as_str()
        .filter(|id| is_repository_id(id))
        .ok_or("profile_scope_invalid")?;
    let origin = profile["scope"]["repository_origin_root"]
        .as_str()
        .filter(|root| is_full_sha256_root(root))
        .ok_or("profile_scope_invalid")?;
    if state["schema"] != "vela.claim-dependency-state.v0"
        || state["scenario"] != "synthetic_counterfactual_over_retained_math_anchors"
        || state["experiment_id"] != profile["experiment_id"]
        || state["repository_id"] != repository_id
        || state["repository_origin_root"] != origin
    {
        return Err("state_repository_context_mismatch".to_string());
    }

    let mut nodes = BTreeMap::new();
    for node in profile["nodes"].as_array().ok_or("profile_scope_invalid")? {
        let id = node["claim_id"]
            .as_str()
            .ok_or("profile_claim_ref_malformed")?;
        let claim_root = node["claim_root"]
            .as_str()
            .ok_or("profile_claim_ref_malformed")?;
        if node["repository_id"] != repository_id || node["repository_origin_root"] != origin {
            return Err("profile_repository_context_mismatch".to_string());
        }
        if nodes.insert(id, claim_root).is_some() {
            return Err("profile_node_duplicate".to_string());
        }
    }

    let mut edge_set = BTreeSet::new();
    let mut relations = Vec::new();
    for edge in profile["dependencies"]
        .as_array()
        .ok_or("profile_scope_invalid")?
    {
        if edge["kind"] != "requires" {
            return Err("profile_dependency_kind_unsupported".to_string());
        }
        let mut endpoints = Vec::new();
        for side in ["source", "target"] {
            let reference = &edge[side];
            let id = reference["claim_id"]
                .as_str()
                .ok_or("profile_claim_ref_malformed")?;
            let claim_root = reference["claim_root"]
                .as_str()
                .ok_or("profile_claim_ref_malformed")?;
            if reference["repository_id"] != repository_id
                || reference["repository_origin_root"] != origin
            {
                return Err("profile_repository_context_mismatch".to_string());
            }
            if nodes.get(id).ok_or("profile_dependency_endpoint_missing")? != &claim_root {
                return Err("profile_dependency_endpoint_root_mismatch".to_string());
            }
            endpoints.push(id);
        }
        if !edge_set.insert((endpoints[0], endpoints[1])) {
            return Err("profile_dependency_duplicate".to_string());
        }
        relations.push(CorrectionRelation {
            relation_id: format!("requires:{}:{}", endpoints[0], endpoints[1]),
            relation_root: root(edge),
            kind: "depends_on".to_string(),
            source_claim_id: endpoints[0].to_string(),
            target_claim_id: endpoints[1].to_string(),
        });
    }

    let mut seen_claims = BTreeSet::new();
    let mut claims = Vec::new();
    for claim in state["claims"]
        .as_array()
        .ok_or("state_schema_unsupported")?
    {
        let claim_id = claim["claim_id"]
            .as_str()
            .ok_or("state_schema_unsupported")?;
        if !seen_claims.insert(claim_id) {
            return Err("state_claim_duplicate".to_string());
        }
        if !claim["verification"].is_null()
            && (claim["verification"]["property"] != "claim_dependency_fidelity.v0"
                || claim["verification"]["input_claim_root"] != claim["claim_root"])
        {
            return Err("state_schema_unsupported".to_string());
        }
        claims.push(CorrectionClaim {
            claim_id: claim_id.to_string(),
            claim_root: claim["claim_root"]
                .as_str()
                .ok_or("state_schema_unsupported")?
                .to_string(),
            repair_condition: Some(REPAIR.to_string()),
        });
    }
    let transition_ref = |side: &str| ClaimRef {
        claim_id: state["transition"][side]["claim_id"]
            .as_str()
            .unwrap()
            .to_string(),
        claim_root: state["transition"][side]["claim_root"]
            .as_str()
            .unwrap()
            .to_string(),
    };
    let complete_claim_set = profile["scope"]["complete_claim_set"]
        .as_bool()
        .ok_or("profile_scope_invalid")?;
    let complete_dependency_set = profile["scope"]["complete_dependency_set"]
        .as_bool()
        .ok_or("profile_scope_invalid")?;
    Ok(CorrectionImpactInputV1 {
        schema: "vela.correction-impact-input.v1".to_string(),
        fixture_id: profile["experiment_id"].as_str().unwrap().to_string(),
        transition: CorrectionTransition {
            kind: "supersede_claim".to_string(),
            predecessor: transition_ref("predecessor"),
            successor: transition_ref("successor"),
        },
        claims,
        relations,
        relation_rules: vec![CorrectionRelationRule {
            kind: "depends_on".to_string(),
            effect: "hard_dependency".to_string(),
        }],
        bounds: CorrectionBounds {
            max_claims: profile["scope"]["max_claims"].as_u64().unwrap() as usize,
            max_relations: profile["scope"]["max_dependencies"].as_u64().unwrap() as usize,
            complete_claim_set,
            complete_relation_set: complete_dependency_set,
        },
    })
}

#[test]
fn rust_adapter_matches_frozen_roots_and_requires_impact() {
    let (profile, state, expected) = (parse(PROFILE), parse(STATE), parse(EXPECTED));
    assert_eq!(root(&profile), expected["profile_canonical_root"]);
    assert_eq!(root(&state), expected["state_canonical_root"]);
    assert_eq!(
        root(&expected["projection"]),
        expected["projection_canonical_root"]
    );
    let input = adapt(&profile, &state).unwrap();
    assert_eq!(
        input.bounds.complete_claim_set,
        profile["scope"]["complete_claim_set"]
    );
    assert_eq!(
        input.bounds.complete_relation_set,
        profile["scope"]["complete_dependency_set"]
    );
    let projection = derive_correction_impact(&input).unwrap();
    let labels = state["claims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|claim| {
            (
                claim["claim_id"].as_str().unwrap(),
                claim["label"].as_str().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let affected = projection
        .affected_claims
        .iter()
        .map(|claim| {
            (
                labels[claim.claim_id.as_str()],
                claim.classification.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let unaffected = projection
        .unaffected_claims
        .iter()
        .map(|claim| labels[claim.claim_id.as_str()])
        .collect::<Vec<_>>();
    assert_eq!(
        affected,
        [("B", "repair_required"), ("E", "repair_required")]
    );
    assert_eq!(unaffected, ["D"]);
    let affected_ids = projection
        .affected_claims
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    let stale = state["claims"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|claim| affected_ids.contains(claim["claim_id"].as_str().unwrap()))
        .map(|claim| claim["verification"]["verification_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        expected["projection"]["sets"]["stale_verifications"],
        serde_json::json!(stale)
    );
    assert_eq!(
        expected["projection"]["repair_batches"][0]["labels"],
        serde_json::json!(["B"])
    );
    assert_eq!(
        expected["projection"]["repair_batches"][1]["labels"],
        serde_json::json!(["E"])
    );
    assert_eq!(
        expected["projection"]["repair_batches"][0]["repair_layer"],
        0
    );
    assert_eq!(
        expected["projection"]["repair_batches"][1]["repair_layer"],
        1
    );
    assert_eq!(expected["projection"]["authority_effect"], "none");
}
