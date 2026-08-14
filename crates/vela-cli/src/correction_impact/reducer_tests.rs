use super::reducer::{
    CorrectionImpactInputV1, CorrectionImpactProjectionV1, correction_impact_projection_root,
    derive_correction_impact,
};
use sha2::{Digest, Sha256};

fn fixture() -> CorrectionImpactInputV1 {
    serde_json::from_str(include_str!(
        "../../../../conformance/fixtures/correction/diamond-input.json"
    ))
    .expect("parse correction fixture")
}

fn apply_mutation(
    mut input: CorrectionImpactInputV1,
    mutation: &serde_json::Value,
) -> CorrectionImpactInputV1 {
    let operation = mutation["op"].as_str().expect("mutation operation");
    match operation {
        "set_relation_kind" => {
            let relation = input
                .relations
                .iter_mut()
                .find(|relation| relation.relation_id == mutation["relation_id"])
                .expect("relation");
            relation.kind = mutation["value"].as_str().expect("kind").to_string();
        }
        "set_rule_effect" => {
            let rule = input
                .relation_rules
                .iter_mut()
                .find(|rule| rule.kind == mutation["kind"])
                .expect("rule");
            rule.effect = mutation["value"].as_str().expect("effect").to_string();
        }
        "set_relation_target" => {
            let relation = input
                .relations
                .iter_mut()
                .find(|relation| relation.relation_id == mutation["relation_id"])
                .expect("relation");
            relation.target_claim_id = mutation["value"].as_str().expect("target").to_string();
        }
        "set_predecessor_root" => {
            input.transition.predecessor.claim_root =
                mutation["value"].as_str().expect("root").to_string();
        }
        "set_complete_relation_set" => {
            input.bounds.complete_relation_set = mutation["value"].as_bool().expect("completeness");
        }
        "remove_relation" => {
            input
                .relations
                .retain(|relation| relation.relation_id != mutation["relation_id"]);
        }
        "add_connected_cycle" => {
            use super::reducer::CorrectionRelation;
            input.relations.extend([
                CorrectionRelation {
                    relation_id: "relation-b-depends-on-c".to_string(),
                    relation_root: format!("sha256:{}", "05".repeat(32)),
                    kind: "depends_on".to_string(),
                    source_claim_id: format!("vcl_{}", "b".repeat(64)),
                    target_claim_id: format!("vcl_{}", "c".repeat(64)),
                },
                CorrectionRelation {
                    relation_id: "relation-c-depends-on-b".to_string(),
                    relation_root: format!("sha256:{}", "06".repeat(32)),
                    kind: "depends_on".to_string(),
                    source_claim_id: format!("vcl_{}", "c".repeat(64)),
                    target_claim_id: format!("vcl_{}", "b".repeat(64)),
                },
            ]);
        }
        "set_max_relations" => {
            input.bounds.max_relations = mutation["value"]
                .as_u64()
                .expect("relation bound")
                .try_into()
                .expect("usize relation bound");
        }
        other => panic!("unknown mutation {other}"),
    }
    input
}

#[test]
fn correction_impact_preserves_the_independent_route_and_opens_one_repair() {
    let projection = derive_correction_impact(&fixture()).expect("derive correction impact");
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/fixtures/correction/diamond-expected.json"
    ))
    .expect("parse expected correction projection");
    let expected_projection: CorrectionImpactProjectionV1 =
        serde_json::from_value(expected["projection"].clone()).expect("expected projection");

    assert_eq!(projection.status, "complete");
    assert_eq!(projection, expected_projection);
    assert_eq!(projection.affected_claims.len(), 2);
    assert_eq!(
        projection
            .affected_claims
            .iter()
            .map(|claim| (
                claim.claim_id.chars().nth(4).unwrap(),
                claim.classification.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![('b', "repair_required"), ('c', "route_changed")]
    );
    assert_eq!(projection.repair_obligations.len(), 1);
    assert_eq!(projection.lost_support_routes.len(), 1);
    assert_eq!(projection.surviving_support_routes.len(), 1);
    assert_eq!(
        projection.surviving_support_routes[0].relation_id,
        "relation-c-supported-by-e"
    );
    assert_eq!(
        projection
            .unaffected_claims
            .iter()
            .map(|claim| claim.claim_id.chars().nth(4).unwrap())
            .collect::<Vec<_>>(),
        vec!['d', 'e']
    );
    assert_eq!(
        correction_impact_projection_root(&projection).expect("projection root"),
        expected["projection_root"].as_str().expect("expected root")
    );
}

#[test]
fn incomplete_relation_set_never_claims_a_complete_affected_set() {
    let mut fixture = fixture();
    fixture.bounds.complete_relation_set = false;
    let projection = derive_correction_impact(&fixture).expect("bounded incomplete projection");

    assert_eq!(projection.status, "incomplete");
    assert_eq!(projection.diagnostics, ["relation_set_incomplete"]);
    assert!(projection.affected_claims.is_empty());
    assert!(projection.unaffected_claims.is_empty());
}

#[test]
fn unknown_or_semantically_rebound_relations_fail_closed() {
    let mut unknown = fixture();
    unknown.relations[0].kind = "generic_link".to_string();
    assert_eq!(
        derive_correction_impact(&unknown).expect_err("unknown relation"),
        "correction_relation_unknown"
    );

    let mut rebound = fixture();
    rebound.relation_rules[1].effect = "hard_dependency".to_string();
    assert_eq!(
        derive_correction_impact(&rebound).expect_err("discovery rebound"),
        "correction_relation_rule_conflict"
    );
}

#[test]
fn missing_independent_route_opens_repair_instead_of_silently_dropping_it() {
    let mut fixture = fixture();
    fixture
        .relations
        .retain(|relation| relation.relation_id != "relation-c-supported-by-e");
    let projection = derive_correction_impact(&fixture).expect("derive without independent route");

    let c = projection
        .affected_claims
        .iter()
        .find(|claim| claim.claim_id.starts_with("vcl_c"))
        .expect("Claim C");
    assert_eq!(c.classification, "repair_required");
    assert_eq!(projection.repair_obligations.len(), 2);
    assert!(projection.surviving_support_routes.is_empty());
}

#[test]
fn adversarial_vectors_have_identical_bounded_outcomes() {
    let base = fixture();
    let base_root = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            vela_protocol::canonical::to_canonical_bytes(&base).expect("canonical input")
        ))
    );
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/fixtures/correction/diamond-adversarial.json"
    ))
    .expect("parse adversarial vectors");
    assert_eq!(vectors["schema"], "vela.correction-impact-adversarial.v1");
    assert_eq!(vectors["base_input_root"], base_root);

    for case in vectors["cases"].as_array().expect("cases") {
        let input = apply_mutation(base.clone(), &case["mutation"]);
        if let Some(expected_error) = case["expected_error"].as_str() {
            assert_eq!(
                derive_correction_impact(&input).expect_err("expected fail-closed vector"),
                expected_error,
                "{}",
                case["id"]
            );
            continue;
        }
        let projection = derive_correction_impact(&input).expect("bounded projection");
        let expected = &case["expected_projection"];
        assert_eq!(projection.status, expected["status"], "{}", case["id"]);
        assert_eq!(
            projection.diagnostics,
            serde_json::from_value::<Vec<String>>(expected["diagnostics"].clone())
                .expect("diagnostics"),
            "{}",
            case["id"]
        );
        let affected = projection
            .affected_claims
            .iter()
            .map(|claim| claim.claim_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            affected,
            serde_json::from_value::<Vec<String>>(expected["affected_claim_ids"].clone())
                .expect("affected ids"),
            "{}",
            case["id"]
        );
        if let Some(expected_repair) = expected["repair_required_claim_ids"].as_array() {
            let repair = projection
                .affected_claims
                .iter()
                .filter(|claim| claim.classification == "repair_required")
                .map(|claim| claim.claim_id.clone())
                .collect::<Vec<_>>();
            assert_eq!(
                repair,
                expected_repair
                    .iter()
                    .map(|value| value.as_str().expect("repair ID").to_string())
                    .collect::<Vec<_>>(),
                "{}",
                case["id"]
            );
        }
        if let Some(expected_routes) = expected["surviving_support_routes"].as_u64() {
            assert_eq!(
                projection.surviving_support_routes.len(),
                usize::try_from(expected_routes).expect("route count"),
                "{}",
                case["id"]
            );
        }
    }
}
