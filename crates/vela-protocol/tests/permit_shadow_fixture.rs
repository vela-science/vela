use serde_json::Value;
use sha2::{Digest, Sha256};
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, Outcome, PolicyContext, evaluate, policy_signature_preimage,
};
use vela_protocol::execution_binding::ExecutionBindingV1;

const FIXTURE: &str = include_str!("../../../conformance/fixtures/permit-shadow-v1.json");

fn root(preimage: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(preimage.as_bytes()))
}

#[test]
fn acceptance_policy_v0_2_permits_only_the_registered_sidon_binding() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("shadow fixture must parse");
    let cases = fixture["cases"].as_array().expect("cases must be an array");
    let intended: ExecutionBindingV1 =
        serde_json::from_value(cases[0]["binding"].clone()).expect("binding must parse");
    let mut policy: AcceptancePolicy =
        serde_json::from_value(fixture["policy"].clone()).expect("policy must parse");
    policy.schema = "vela.acceptance_policy.v0.2".to_string();
    let constraints = &mut policy.rules[0].constraints;
    constraints.allowed_packet_roots = Some(vec![intended.packet_root.clone()]);
    constraints.allowed_profile_roots = Some(vec![intended.profile_root.clone()]);
    constraints.allowed_verifier_capsule_roots = Some(vec![intended.verifier_capsule_root.clone()]);
    constraints.allowed_result_contract_roots = Some(vec![intended.result_contract_root.clone()]);
    constraints.required_replayability = Some("exact".to_string());
    policy.id = policy.content_address();
    let decision_time = fixture["decision_time"]
        .as_str()
        .expect("decision time must be a string");
    assert!(
        policy_signature_preimage(&policy, decision_time).is_ok(),
        "a closed exact v0.2 policy must be signable"
    );

    for case in cases {
        let mut context: PolicyContext = serde_json::from_value(case["policy_context"].clone())
            .expect("policy context must parse");
        context.execution_binding =
            Some(serde_json::from_value(case["binding"].clone()).expect("binding must parse"));
        let decision = evaluate(&policy, &context, decision_time);
        assert_eq!(
            decision.outcome.as_str(),
            case["expected_v0_2"].as_str().unwrap(),
            "{} produced the wrong v0.2 route: {:?}",
            case["id"],
            decision.reasons
        );
    }

    let mut missing: PolicyContext = serde_json::from_value(cases[0]["policy_context"].clone())
        .expect("policy context must parse");
    assert_eq!(
        evaluate(&policy, &missing, decision_time).outcome,
        Outcome::Defer
    );
    missing.execution_binding = Some(ExecutionBindingV1 {
        schema: "vela.execution-binding.v999".to_string(),
        ..intended.clone()
    });
    let invalid = evaluate(&policy, &missing, decision_time);
    assert_eq!(invalid.outcome, Outcome::Defer);
    assert!(
        invalid
            .reasons
            .iter()
            .any(|reason| reason.contains("execution_binding_invalid"))
    );

    let mut result_drift: PolicyContext =
        serde_json::from_value(cases[0]["policy_context"].clone())
            .expect("policy context must parse");
    let mut changed = intended.clone();
    changed.result_contract_root = root("sidon:a24 negative result contract v1");
    result_drift.execution_binding = Some(changed);
    assert_eq!(
        evaluate(&policy, &result_drift, decision_time).outcome,
        Outcome::Defer
    );

    let mut replayability_drift: PolicyContext =
        serde_json::from_value(cases[0]["policy_context"].clone())
            .expect("policy context must parse");
    replayability_drift.execution_binding = Some(intended.clone());
    replayability_drift.replayability = "bounded".to_string();
    assert_eq!(
        evaluate(&policy, &replayability_drift, decision_time).outcome,
        Outcome::Defer
    );

    let mut short_policy = policy.clone();
    short_policy.rules[0].constraints.allowed_packet_roots = Some(vec!["sha256:abcd".into()]);
    short_policy.id = short_policy.content_address();
    assert!(
        policy_signature_preimage(&short_policy, decision_time).is_err(),
        "malformed v0.2 roots must fail before signing"
    );
    let mut intended_context: PolicyContext =
        serde_json::from_value(cases[0]["policy_context"].clone())
            .expect("policy context must parse");
    intended_context.execution_binding = Some(intended);
    assert_eq!(
        evaluate(&short_policy, &intended_context, decision_time).outcome,
        Outcome::Deny
    );

    let mut mislabeled_v1 = policy;
    mislabeled_v1.schema = "vela.acceptance_policy.v0.1".to_string();
    mislabeled_v1.id = mislabeled_v1.content_address();
    assert_eq!(
        evaluate(&mislabeled_v1, &intended_context, decision_time).outcome,
        Outcome::Deny
    );
}

#[test]
fn acceptance_policy_v0_1_cannot_distinguish_sidon_binding_substitutions() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("shadow fixture must parse");
    assert_eq!(
        fixture["schema"], "vela.permit-shadow-experiment.v1",
        "fixture schema drifted"
    );
    let mut policy: AcceptancePolicy =
        serde_json::from_value(fixture["policy"].clone()).expect("policy must parse");
    policy.id = policy.content_address();
    let decision_time = fixture["decision_time"]
        .as_str()
        .expect("decision time must be a string");
    let cases = fixture["cases"].as_array().expect("cases must be an array");
    assert_eq!(cases.len(), 3);

    let intended_binding = &cases[0]["binding"];
    assert_eq!(cases[1]["target_id"], cases[0]["target_id"]);
    assert_eq!(
        cases[1]["binding"]["packet_root"],
        intended_binding["packet_root"]
    );
    assert_ne!(
        cases[1]["binding"]["profile_root"],
        intended_binding["profile_root"]
    );
    assert_ne!(
        cases[1]["binding"]["verifier_capsule_root"],
        intended_binding["verifier_capsule_root"]
    );
    assert_ne!(cases[2]["target_id"], cases[0]["target_id"]);
    assert_ne!(
        cases[2]["binding"]["packet_root"],
        intended_binding["packet_root"]
    );

    let mut policy_language_digest = None;
    for case in cases {
        let binding = case["binding"]
            .as_object()
            .expect("binding must be an object");
        let preimages = case["root_preimages"]
            .as_object()
            .expect("preimages must be an object");
        for field in [
            "packet_root",
            "profile_root",
            "verifier_capsule_root",
            "result_contract_root",
        ] {
            let preimage = preimages[field]
                .as_str()
                .expect("root preimage must be a string");
            assert_eq!(
                binding[field],
                root(preimage),
                "{field} is not content addressed"
            );
        }

        let context: PolicyContext = serde_json::from_value(case["policy_context"].clone())
            .expect("policy context must parse");
        let digest = context
            .policy_language_digest()
            .expect("context must canonicalize");
        match &policy_language_digest {
            None => policy_language_digest = Some(digest),
            Some(expected) => assert_eq!(
                &digest, expected,
                "v0.1 unexpectedly observes a shadow binding field"
            ),
        }
        let decision = evaluate(&policy, &context, decision_time);
        assert_eq!(case["expected_v0_1"], "permit");
        assert_eq!(
            decision.outcome,
            Outcome::Permit,
            "{} did not reproduce the registered v0.1 result: {:?}",
            case["id"],
            decision.reasons
        );
    }
    assert_eq!(
        policy_language_digest.as_deref(),
        Some("sha256:05f4c43817a4301da40e476393639ba042756f593d48582718135e92b653c7ac")
    );
}
