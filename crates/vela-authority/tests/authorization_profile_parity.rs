use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::{
    AUTHORIZATION_PROFILE_V1, AUTHORIZATION_REQUEST_SCHEMA_V1, AuthorityActionV1,
    AuthorityResourceTypeV1, AuthorityRoleV1, AuthorizationDecisionV1, AuthorizationModelV1,
    AuthorizationRequestV1, AuthorizationResourceV1, PrincipalClass, evaluate_authorization_v1,
};
use vela_protocol::canonical::to_canonical_bytes;

const FIXTURE: &str =
    include_str!("../../../conformance/fixtures/authorization-profile-parity-v1.json");
const LEGACY_REQUEST_DOMAIN: &[u8] = b"vela.authority-authorization-request.internal.v1\0";

#[derive(Debug, Deserialize)]
struct ParityFixture {
    schema: String,
    principal_id: String,
    models: Vec<ModelFixture>,
    cases: Vec<CaseFixture>,
    negative_cases: Vec<NegativeFixture>,
}

#[derive(Debug, Deserialize)]
struct ModelFixture {
    source_repository: String,
    model: AuthorizationModelV1,
    expected_model_root: String,
}

#[derive(Debug, Deserialize)]
struct CaseFixture {
    source_repository: String,
    record_id: String,
    sequence: u64,
    frontier_id: String,
    action: String,
    resource_type: String,
    resource_id: String,
    role: String,
    legacy_request_root: String,
    legacy_authentication: Value,
    authentication_root: String,
    transaction_read_set_root: String,
    intent_digest: String,
    candidate_request_root: String,
}

#[derive(Debug, Deserialize)]
struct NegativeFixture {
    mutation: String,
    expected_reason: String,
}

fn fixture() -> ParityFixture {
    serde_json::from_str(FIXTURE).expect("authorization parity fixture is strict JSON")
}

fn action(value: &str) -> AuthorityActionV1 {
    serde_json::from_value(Value::String(value.into())).expect("fixture action is closed")
}

fn resource_type(value: &str) -> AuthorityResourceTypeV1 {
    serde_json::from_value(Value::String(value.into())).expect("fixture resource type is closed")
}

fn role(value: &str) -> AuthorityRoleV1 {
    serde_json::from_value(Value::String(value.into())).expect("fixture role is closed")
}

fn candidate_request(
    principal_id: &str,
    model: &AuthorizationModelV1,
    case: &CaseFixture,
) -> AuthorizationRequestV1 {
    AuthorizationRequestV1 {
        schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
        profile: AUTHORIZATION_PROFILE_V1.into(),
        model_root: model.root().unwrap(),
        frontier_id: case.frontier_id.clone(),
        principal_id: principal_id.into(),
        principal_class: PrincipalClass::Human,
        action: action(&case.action),
        resource: AuthorizationResourceV1 {
            frontier_id: case.frontier_id.clone(),
            resource_type: resource_type(&case.resource_type),
            resource_id: case.resource_id.clone(),
        },
        authentication_root: case.authentication_root.clone(),
        transaction_read_set_root: case.transaction_read_set_root.clone(),
        intent_digest: case.intent_digest.clone(),
        recovery_recent: false,
    }
}

fn legacy_request_root(principal_id: &str, case: &CaseFixture) -> String {
    let entity_type = match case.resource_type.as_str() {
        "frontier" => "Frontier",
        "proposal" => "Proposal",
        other => panic!("unsupported fixture resource type {other}"),
    };
    let principal = format!(
        "Human::{}",
        serde_json::to_string(principal_id).expect("principal serializes")
    );
    let resource = format!(
        "{entity_type}::{}",
        serde_json::to_string(&case.resource_id).expect("resource ID serializes")
    );
    let commitment = json!({
        "schema": "vela.authority-authorization-request.internal.v1",
        "principal": principal,
        "principal_class": "human",
        "action": case.action,
        "resource": resource,
        "context": {
            "exact": true,
            "authentication": case.legacy_authentication
        }
    });
    let canonical = to_canonical_bytes(&commitment).unwrap();
    let mut preimage = Vec::with_capacity(LEGACY_REQUEST_DOMAIN.len() + canonical.len());
    preimage.extend_from_slice(LEGACY_REQUEST_DOMAIN);
    preimage.extend_from_slice(&canonical);
    format!("sha256:{}", hex::encode(Sha256::digest(preimage)))
}

#[test]
fn seven_current_authority_transactions_have_closed_profile_parity() {
    let fixture = fixture();
    assert_eq!(fixture.schema, "vela.authorization-profile-parity.v1");
    assert_eq!(fixture.models.len(), 4);
    assert_eq!(fixture.cases.len(), 7);

    for model in &fixture.models {
        assert_eq!(
            model.model.root().unwrap(),
            model.expected_model_root,
            "{} model root drifted",
            model.source_repository
        );
    }

    for case in &fixture.cases {
        assert!(case.sequence > 0, "{} has no sequence", case.record_id);
        assert!(
            case.record_id.starts_with("var_") && case.source_repository.ends_with("-frontier"),
            "fixture source identity is incomplete"
        );
        assert_eq!(
            legacy_request_root(&fixture.principal_id, case),
            case.legacy_request_root,
            "{} legacy request parity drifted",
            case.record_id
        );
        let model = fixture
            .models
            .iter()
            .find(|model| model.model.frontier_id == case.frontier_id)
            .expect("case has one candidate model");
        let request = candidate_request(&fixture.principal_id, &model.model, case);
        assert_eq!(
            request.root().unwrap(),
            case.candidate_request_root,
            "{} candidate request root drifted",
            case.record_id
        );
        let evaluation = evaluate_authorization_v1(&model.model, &request).unwrap();
        assert_eq!(evaluation.decision, AuthorizationDecisionV1::Allow);
        assert_eq!(evaluation.matched_role, Some(role(&case.role)));
    }
}

#[test]
fn closed_profile_negative_cases_fail_with_stable_reasons() {
    let fixture = fixture();
    let base_model = fixture
        .models
        .iter()
        .find(|model| model.model.frontier_id == "vfr_97d7d25957384f80")
        .unwrap()
        .model
        .clone();
    let base_case = fixture
        .cases
        .iter()
        .find(|case| case.record_id == "var_0fc27d0a149c7227")
        .unwrap();

    for negative in &fixture.negative_cases {
        let mut model = base_model.clone();
        let mut request = candidate_request(&fixture.principal_id, &model, base_case);
        match negative.mutation.as_str() {
            "unbound_principal" => request.principal_id = "local:unbound|uid:501".into(),
            "machine_principal" => request.principal_class = PrincipalClass::Agent,
            "wrong_role" => {
                model
                    .members
                    .retain(|member| member.role == AuthorityRoleV1::Administrator);
                request.model_root = model.root().unwrap();
            }
            "wrong_frontier" => request.frontier_id = "vfr_other".into(),
            "wrong_resource_frontier" => request.resource.frontier_id = "vfr_other".into(),
            "wrong_resource_type" => {
                request.resource.resource_type = AuthorityResourceTypeV1::Frontier;
                request.resource.resource_id = request.frontier_id.clone();
            }
            "recovery_recent" => request.recovery_recent = true,
            other => panic!("unsupported negative mutation {other}"),
        }
        let evaluation = evaluate_authorization_v1(&model, &request).unwrap();
        assert_eq!(evaluation.decision, AuthorizationDecisionV1::Deny);
        assert_eq!(
            serde_json::to_value(evaluation.reason).unwrap(),
            Value::String(negative.expected_reason.clone()),
            "{} did not retain its stable reason",
            negative.mutation
        );
    }
}
