use serde_json::Value;
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, Outcome, PolicyContext, evaluate, policy_signature_preimage,
};
use vela_protocol::identity::IdentityBinding;

const FIXTURE: &str =
    include_str!("../../../conformance/fixtures/policy-scoped-producer-credential-v1.json");

#[test]
fn policy_v0_3_scopes_the_live_sidon_producer_by_full_credential_root() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("credential fixture must parse");
    assert_eq!(
        fixture["schema"],
        "vela.policy-scoped-producer-credential-fixture.v1"
    );
    let binding: IdentityBinding = serde_json::from_value(fixture["identity_binding"].clone())
        .expect("identity binding must parse");
    binding
        .verify()
        .expect("live binding must prove possession");
    let credential_root = binding
        .credential_root()
        .expect("credential root must derive");
    assert_eq!(
        credential_root,
        fixture["producer_credential_root"].as_str().unwrap()
    );
    assert_eq!(
        &credential_root["sha256:".len()..][..16],
        &binding.binding_id["vib_".len()..]
    );

    let decision_time = fixture["decision_time"].as_str().unwrap();
    let mut v2: AcceptancePolicy =
        serde_json::from_value(fixture["policy"].clone()).expect("v0.2 policy must parse");
    v2.id = v2.content_address();
    policy_signature_preimage(&v2, decision_time).expect("v0.2 policy must remain signable");

    let mut v3 = v2.clone();
    v3.schema = "vela.acceptance_policy.v0.3".to_string();
    v3.rules[0].constraints.allowed_producer_credential_roots = Some(vec![credential_root.clone()]);
    v3.id = v3.content_address();
    policy_signature_preimage(&v3, decision_time).expect("v0.3 policy must be signable");

    for case in fixture["cases"].as_array().unwrap() {
        let mut v2_context: PolicyContext =
            serde_json::from_value(fixture["context"].clone()).unwrap();
        v2_context.credential_valid = case["credential_valid"].as_bool().unwrap();
        assert_eq!(v2_context.producer_credential_root, None);
        let v2_decision = evaluate(&v2, &v2_context, decision_time);
        assert_eq!(
            v2_decision.outcome.as_str(),
            case["expected_v0_2"].as_str().unwrap(),
            "{} changed historical v0.2 behavior: {:?}",
            case["id"],
            v2_decision.reasons
        );

        let mut v3_context = v2_context;
        v3_context.producer_credential_root = case["producer_credential_root"]
            .as_str()
            .map(ToString::to_string);
        let v3_decision = evaluate(&v3, &v3_context, decision_time);
        assert_eq!(
            v3_decision.outcome.as_str(),
            case["expected_v0_3"].as_str().unwrap(),
            "{} produced the wrong scoped route: {:?}",
            case["id"],
            v3_decision.reasons
        );
    }

    let mut duplicate = v3.clone();
    duplicate.rules[0]
        .constraints
        .allowed_producer_credential_roots = Some(vec![credential_root.clone(), credential_root]);
    duplicate.id = duplicate.content_address();
    assert!(policy_signature_preimage(&duplicate, decision_time).is_err());
    let context: PolicyContext = serde_json::from_value(fixture["context"].clone()).unwrap();
    assert_eq!(
        evaluate(&duplicate, &context, decision_time).outcome,
        Outcome::Deny
    );

    for (label, tampered) in [
        ("backdated", {
            let mut value = binding.clone();
            value.created_at = "2020-01-01T00:00:00Z".to_string();
            value
        }),
        ("wrong actor", {
            let mut value = binding.clone();
            value.actor_id = "agent:substitute".to_string();
            value
        }),
        ("wrong class", {
            let mut value = binding.clone();
            value.actor_class = vela_protocol::identity::ActorClass::Human;
            value
        }),
        ("wrong key", {
            let mut value = binding.clone();
            value.public_key_hex = "00".repeat(32);
            value
        }),
        ("wrong signature", {
            let mut value = binding.clone();
            value.signature = "00".repeat(64);
            value
        }),
    ] {
        assert!(tampered.verify().is_err(), "{label} binding must fail");
        if label != "wrong signature" {
            assert_ne!(
                tampered.credential_root().unwrap(),
                fixture["producer_credential_root"],
                "{label} binding must not retain the authorized root"
            );
        }
    }
}
