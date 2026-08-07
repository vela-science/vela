//! Restricted, fail-closed Cedar evaluation for Era-1 repository authority.
//!
//! Cedar intentionally treats authorization errors as diagnostics on an
//! Allow/Deny response. Vela is stricter: an invalid bundle, request, entity
//! snapshot, schema warning, or evaluation diagnostic can never authorize an
//! automatic scientific-state transition.

use std::str::FromStr;

use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request, Schema, ValidationMode,
    Validator,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use vela_protocol::authority::PolicyBundleV1;
use vela_protocol::canonical::to_canonical_bytes;

pub mod runtime_authentication;

pub use vela_protocol::authority::{
    CEDAR_ENGINE, CEDAR_ENGINE_VERSION, CEDAR_PROFILE_V1, CedarDecision, CedarEvaluation,
    PrincipalClass,
};
pub use vela_protocol::authorization::{
    AUTHORIZATION_EVALUATION_SCHEMA_V1, AUTHORIZATION_MODEL_SCHEMA_V1, AUTHORIZATION_PROFILE_V1,
    AUTHORIZATION_REQUEST_SCHEMA_V1, AuthorityActionV1, AuthorityMemberV1, AuthorityResourceTypeV1,
    AuthorityRoleV1, AuthorizationDecisionV1, AuthorizationEvaluationV1, AuthorizationModelV1,
    AuthorizationReasonV1, AuthorizationRequestV1, AuthorizationResourceV1,
};
use vela_protocol::principal::principal_class_may_request;
const FORBIDDEN_EXTENSION_CONSTRUCTORS: &[&str] = &["datetime(", "decimal(", "duration(", "ip("];

#[derive(Debug, Clone)]
pub struct CedarEvaluationInput {
    pub schema: String,
    pub policies: String,
    pub entities: Value,
    pub principal: String,
    pub principal_class: PrincipalClass,
    pub action: String,
    pub resource: String,
    pub context: Value,
}

/// Exact Cedar source bytes retained beside a [`PolicyBundleV1`].
///
/// The protocol manifest already binds these members by full digest. This
/// carrier does not introduce another policy object or authority primitive; it
/// makes the bound schema, policy, and entity bytes replayable by later
/// repository-authority transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CedarPolicyMaterial {
    pub schema: String,
    pub policies: String,
    pub entities: Value,
}

impl CedarPolicyMaterial {
    pub fn from_evaluation(input: &CedarEvaluationInput) -> Self {
        Self {
            schema: input.schema.clone(),
            policies: input.policies.clone(),
            entities: input.entities.clone(),
        }
    }

    pub fn validate_against(&self, bundle: &PolicyBundleV1) -> Result<(), String> {
        bundle.validate()?;
        let entities = to_canonical_bytes(&self.entities)?;
        let actual = [
            (
                "cedar schema",
                sha256_root(self.schema.as_bytes()),
                &bundle.cedar_schema_root,
            ),
            (
                "policies",
                sha256_root(self.policies.as_bytes()),
                &bundle.policies_root,
            ),
            ("entities", sha256_root(&entities), &bundle.entities_root),
        ];
        if let Some((name, _, _)) = actual
            .iter()
            .find(|(_, observed, expected)| observed != *expected)
        {
            return Err(format!(
                "retained Cedar {name} bytes differ from policy bundle {}",
                bundle.root()?
            ));
        }
        Ok(())
    }

    pub fn canonical_entities(&self) -> Result<Vec<u8>, String> {
        to_canonical_bytes(&self.entities)
    }
}

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn denied(diagnostics: Vec<String>) -> CedarEvaluation {
    CedarEvaluation {
        engine: CEDAR_ENGINE.to_string(),
        engine_version: CEDAR_ENGINE_VERSION.to_string(),
        profile: CEDAR_PROFILE_V1.to_string(),
        valid: false,
        decision: CedarDecision::Deny,
        automatic_permit: false,
        determining_policies: Vec::new(),
        diagnostics: sorted(diagnostics),
    }
}

fn application_denied(diagnostic: String) -> CedarEvaluation {
    CedarEvaluation {
        engine: CEDAR_ENGINE.to_string(),
        engine_version: CEDAR_ENGINE_VERSION.to_string(),
        profile: CEDAR_PROFILE_V1.to_string(),
        valid: true,
        decision: CedarDecision::Deny,
        automatic_permit: false,
        determining_policies: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

/// Evaluate one exact Cedar request under Vela's restricted profile.
///
/// The function returns a recorded Deny instead of propagating parse errors.
/// This makes the fail-closed outcome explicit and deterministic for audit
/// records while callers may still distinguish malformed input via `valid`.
pub fn evaluate(input: &CedarEvaluationInput) -> CedarEvaluation {
    if !principal_class_may_request(input.principal_class, &input.action) {
        return application_denied(format!(
            "application_forbid: {:?} principals cannot perform {}",
            input.principal_class, input.action
        ));
    }

    if contains_extension_escape(&input.entities) || contains_extension_escape(&input.context) {
        return denied(vec![
            "restricted_profile: Cedar extension values are not permitted".into(),
        ]);
    }
    let compact_policy = input
        .policies
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if let Some(extension) = FORBIDDEN_EXTENSION_CONSTRUCTORS
        .iter()
        .find(|extension| compact_policy.contains(**extension))
    {
        return denied(vec![format!(
            "restricted_profile: Cedar extension constructor {extension} is not permitted"
        )]);
    }

    let (schema, schema_warnings) = match Schema::from_cedarschema_str(&input.schema) {
        Ok((schema, warnings)) => (
            schema,
            warnings
                .map(|warning| warning.to_string())
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            return denied(vec![format!("schema_parse: {error}")]);
        }
    };
    if !schema_warnings.is_empty() {
        return denied(
            schema_warnings
                .into_iter()
                .map(|warning| format!("schema_warning: {warning}"))
                .collect(),
        );
    }

    let policies = match PolicySet::from_str(&input.policies) {
        Ok(policies) => policies,
        Err(error) => {
            return denied(vec![format!("policy_parse: {error}")]);
        }
    };
    let validation = Validator::new(schema.clone()).validate(&policies, ValidationMode::Strict);
    let mut validation_diagnostics = validation
        .validation_errors()
        .map(|error| format!("policy_validation: {error}"))
        .collect::<Vec<_>>();
    validation_diagnostics.extend(
        validation
            .validation_warnings()
            .map(|warning| format!("policy_warning: {warning}")),
    );
    if !validation_diagnostics.is_empty() {
        return denied(validation_diagnostics);
    }

    let principal = match EntityUid::from_str(&input.principal) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("principal_parse: {error}")]);
        }
    };
    let action = match EntityUid::from_str(&format!(r#"Action::"{}""#, input.action)) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("action_parse: {error}")]);
        }
    };
    let resource = match EntityUid::from_str(&input.resource) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("resource_parse: {error}")]);
        }
    };
    let entities = match Entities::from_json_value(input.entities.clone(), Some(&schema)) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("entities: {error}")]);
        }
    };
    let context = match Context::from_json_value(input.context.clone(), Some((&schema, &action))) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("context: {error}")]);
        }
    };
    let request = match Request::new(principal, action, resource, context, Some(&schema)) {
        Ok(value) => value,
        Err(error) => {
            return denied(vec![format!("request_validation: {error}")]);
        }
    };

    let response = Authorizer::new().is_authorized(&request, &policies, &entities);
    let determining_policies = sorted(
        response
            .diagnostics()
            .reason()
            .map(ToString::to_string)
            .collect(),
    );
    let diagnostics = sorted(
        response
            .diagnostics()
            .errors()
            .map(|error| format!("evaluation: {error}"))
            .collect(),
    );
    let decision = match response.decision() {
        Decision::Allow => CedarDecision::Allow,
        Decision::Deny => CedarDecision::Deny,
    };

    CedarEvaluation {
        engine: CEDAR_ENGINE.to_string(),
        engine_version: CEDAR_ENGINE_VERSION.to_string(),
        profile: CEDAR_PROFILE_V1.to_string(),
        valid: diagnostics.is_empty(),
        decision,
        automatic_permit: decision == CedarDecision::Allow && diagnostics.is_empty(),
        determining_policies,
        diagnostics,
    }
}

/// Evaluate one exact request under Vela's closed repository-authorization
/// profile.
///
/// This pure evaluator is a shadow implementation until current Frontier
/// parity and an explicit repository-epoch cut are complete. It acquires no
/// authentication, signer, filesystem, clock, or network capability. Valid
/// but unauthorized requests return a content-addressed Deny; malformed model
/// or request bytes return an error and cannot produce an authority record.
pub fn evaluate_authorization_v1(
    model: &AuthorizationModelV1,
    request: &AuthorizationRequestV1,
) -> Result<AuthorizationEvaluationV1, String> {
    model.validate()?;
    request.validate()?;
    let model_root = model.root()?;
    let request_root = request.root()?;

    let outcome = if request.model_root != model_root {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ModelRootMismatch,
            None,
        )
    } else if request.repository_id != model.repository_id {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::FrontierMismatch,
            None,
        )
    } else if request.resource.repository_id != request.repository_id {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ResourceFrontierMismatch,
            None,
        )
    } else if request.principal_class != PrincipalClass::Human {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::PrincipalClassMismatch,
            None,
        )
    } else if request.recovery_recent {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::RecoverySessionForbidden,
            None,
        )
    } else if request.resource.resource_type != request.action.required_resource_type() {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ResourceTypeMismatch,
            None,
        )
    } else {
        let members = model
            .members
            .iter()
            .filter(|member| member.principal_id == request.principal_id)
            .collect::<Vec<_>>();
        if members.is_empty() {
            (
                AuthorizationDecisionV1::Deny,
                AuthorizationReasonV1::UnknownMember,
                None,
            )
        } else if members
            .iter()
            .all(|member| member.principal_class != request.principal_class)
        {
            (
                AuthorizationDecisionV1::Deny,
                AuthorizationReasonV1::PrincipalClassMismatch,
                None,
            )
        } else {
            let required_role = request.action.required_role();
            if members.iter().any(|member| {
                member.principal_class == request.principal_class && member.role == required_role
            }) {
                (
                    AuthorizationDecisionV1::Allow,
                    AuthorizationReasonV1::MemberRoleAuthorized,
                    Some(required_role),
                )
            } else {
                (
                    AuthorizationDecisionV1::Deny,
                    AuthorizationReasonV1::RoleActionMismatch,
                    None,
                )
            }
        }
    };

    let evaluation = AuthorizationEvaluationV1 {
        schema: AUTHORIZATION_EVALUATION_SCHEMA_V1.into(),
        profile: AUTHORIZATION_PROFILE_V1.into(),
        model_root,
        request_root,
        decision: outcome.0,
        reason: outcome.1,
        matched_role: outcome.2,
    };
    evaluation.validate()?;
    Ok(evaluation)
}

fn contains_extension_escape(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.contains_key("__extn") || object.values().any(contains_extension_escape)
        }
        Value::Array(values) => values.iter().any(contains_extension_escape),
        _ => false,
    }
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: &str = r#"
        entity Human;
        entity Agent;
        entity Proposal;
        action "review_accept" appliesTo {
            principal: [Human, Agent],
            resource: Proposal,
            context: { exact: Bool }
        };
    "#;

    fn input(policies: &str) -> CedarEvaluationInput {
        CedarEvaluationInput {
            schema: SCHEMA.into(),
            policies: policies.into(),
            entities: json!([
                {
                    "uid": {"type": "Human", "id": "alice"},
                    "attrs": {},
                    "parents": []
                },
                {
                    "uid": {"type": "Proposal", "id": "p1"},
                    "attrs": {},
                    "parents": []
                }
            ]),
            principal: r#"Human::"alice""#.into(),
            principal_class: PrincipalClass::Human,
            action: "review_accept".into(),
            resource: r#"Proposal::"p1""#.into(),
            context: json!({"exact": true}),
        }
    }

    fn closed_model() -> AuthorizationModelV1 {
        AuthorizationModelV1 {
            schema: AUTHORIZATION_MODEL_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            repository_id: "vrepo_fixture".into(),
            members: vec![
                AuthorityMemberV1 {
                    principal_id: "local:device-1|uid:501".into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Administrator,
                },
                AuthorityMemberV1 {
                    principal_id: "local:device-1|uid:501".into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Reviewer,
                },
            ],
            previous_model_root: None,
        }
    }

    fn closed_request(model: &AuthorizationModelV1) -> AuthorizationRequestV1 {
        AuthorizationRequestV1 {
            schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root: model.root().unwrap(),
            repository_id: "vrepo_fixture".into(),
            principal_id: "local:device-1|uid:501".into(),
            principal_class: PrincipalClass::Human,
            action: AuthorityActionV1::ReviewAccept,
            resource: AuthorizationResourceV1 {
                repository_id: "vrepo_fixture".into(),
                resource_type: AuthorityResourceTypeV1::Proposal,
                resource_id: "vpr_0123456789abcdef".into(),
            },
            authentication_root: format!("sha256:{}", "b".repeat(64)),
            transaction_read_set_root: format!("sha256:{}", "c".repeat(64)),
            intent_digest: format!("sha256:{}", "d".repeat(64)),
            recovery_recent: false,
        }
    }

    #[test]
    fn closed_profile_allows_only_the_exact_member_role() {
        let model = closed_model();
        let reviewer = evaluate_authorization_v1(&model, &closed_request(&model)).unwrap();
        assert_eq!(reviewer.decision, AuthorizationDecisionV1::Allow);
        assert_eq!(reviewer.reason, AuthorizationReasonV1::MemberRoleAuthorized);
        assert_eq!(reviewer.matched_role, Some(AuthorityRoleV1::Reviewer));
        assert!(reviewer.root().unwrap().starts_with("sha256:"));

        let mut administrator_request = closed_request(&model);
        administrator_request.action = AuthorityActionV1::AuthorityInitialize;
        administrator_request.resource = AuthorizationResourceV1 {
            repository_id: "vrepo_fixture".into(),
            resource_type: AuthorityResourceTypeV1::Frontier,
            resource_id: "vrepo_fixture".into(),
        };
        let administrator = evaluate_authorization_v1(&model, &administrator_request).unwrap();
        assert_eq!(administrator.decision, AuthorizationDecisionV1::Allow);
        assert_eq!(
            administrator.matched_role,
            Some(AuthorityRoleV1::Administrator)
        );
    }

    #[test]
    fn closed_profile_denies_every_boundary_mismatch_with_stable_reasons() {
        let model = closed_model();

        let mut wrong_model = closed_request(&model);
        wrong_model.model_root = format!("sha256:{}", "f".repeat(64));
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_model)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ModelRootMismatch
        );

        let mut wrong_frontier = closed_request(&model);
        wrong_frontier.repository_id = "vrepo_other".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_frontier)
                .unwrap()
                .reason,
            AuthorizationReasonV1::FrontierMismatch
        );

        let mut wrong_resource_frontier = closed_request(&model);
        wrong_resource_frontier.resource.repository_id = "vrepo_other".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_resource_frontier)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ResourceFrontierMismatch
        );

        let mut machine = closed_request(&model);
        machine.principal_class = PrincipalClass::Agent;
        assert_eq!(
            evaluate_authorization_v1(&model, &machine).unwrap().reason,
            AuthorizationReasonV1::PrincipalClassMismatch
        );

        let mut unknown = closed_request(&model);
        unknown.principal_id = "local:unknown-device|uid:501".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &unknown).unwrap().reason,
            AuthorizationReasonV1::UnknownMember
        );

        let mut administrator_only = closed_model();
        administrator_only.members.truncate(1);
        let reviewer = closed_request(&administrator_only);
        assert_eq!(
            evaluate_authorization_v1(&administrator_only, &reviewer)
                .unwrap()
                .reason,
            AuthorizationReasonV1::RoleActionMismatch
        );

        let mut wrong_resource_type = closed_request(&model);
        wrong_resource_type.resource.resource_type = AuthorityResourceTypeV1::Frontier;
        wrong_resource_type.resource.resource_id = "vrepo_fixture".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_resource_type)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ResourceTypeMismatch
        );

        let mut recovered = closed_request(&model);
        recovered.recovery_recent = true;
        assert_eq!(
            evaluate_authorization_v1(&model, &recovered)
                .unwrap()
                .reason,
            AuthorizationReasonV1::RecoverySessionForbidden
        );
    }

    #[test]
    fn cedar_profile_defaults_to_deny() {
        let result = evaluate(&input(""));
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.automatic_permit);
        assert!(result.valid);
    }

    #[test]
    fn cedar_profile_forbid_overrides_permit() {
        let result = evaluate(&input(
            r#"
            permit(principal, action, resource);
            forbid(principal, action, resource) when { context.exact };
            "#,
        ));
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.automatic_permit);
        assert!(result.valid);
    }

    #[test]
    fn cedar_profile_allows_only_clean_valid_evaluation() {
        let result = evaluate(&input(
            r#"permit(principal, action, resource) when { context.exact };"#,
        ));
        assert_eq!(result.decision, CedarDecision::Allow);
        assert!(result.automatic_permit);
        assert!(result.valid);
        assert!(!result.determining_policies.is_empty());
    }

    #[test]
    fn cedar_profile_blocks_schema_and_request_errors() {
        let mut bad_schema = input("permit(principal, action, resource);");
        bad_schema.schema = "not cedar".into();
        assert!(!evaluate(&bad_schema).valid);

        let mut bad_context = input("permit(principal, action, resource);");
        bad_context.context = json!({"exact": "yes"});
        let result = evaluate(&bad_context);
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.valid);
        assert!(!result.automatic_permit);
    }

    #[test]
    fn cedar_profile_blocks_evaluation_diagnostics() {
        let mut value =
            input(r#"permit(principal, action, resource) when { principal.missing_attribute };"#);
        // No schema means Cedar cannot reject the absent attribute statically;
        // the runtime diagnostic must still prevent an automatic Permit.
        value.schema = r#"
            entity Human;
            entity Proposal;
            action "review_accept" appliesTo {
                principal: Human,
                resource: Proposal,
                context: { exact: Bool }
            };
        "#
        .into();
        let result = evaluate(&value);
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.automatic_permit);
        assert!(!result.valid);
    }

    #[test]
    fn cedar_profile_rejects_compiled_extension_surface() {
        let result = evaluate(&input(
            r#"permit(principal, action, resource) when { ip("10.0.0.1").isIpv4() };"#,
        ));
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.valid);
        assert!(!result.automatic_permit);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("extension constructor"))
        );
    }

    #[test]
    fn agent_human_only_action_is_forbidden_before_cedar() {
        let mut value = input("permit(principal, action, resource);");
        value.principal = r#"Agent::"worker""#.into();
        value.principal_class = PrincipalClass::Agent;
        let result = evaluate(&value);
        assert_eq!(result.decision, CedarDecision::Deny);
        assert!(!result.automatic_permit);
        assert!(result.valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("application_forbid"))
        );
    }

    #[test]
    fn policy_bundle_is_closed_and_content_addressed() {
        use vela_protocol::authority::{POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1};

        let root = format!("sha256:{}", "a".repeat(64));
        let bundle = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            repository_id: "vrepo_fixture".into(),
            cedar_schema_root: root.clone(),
            policies_root: root.clone(),
            entities_root: root.clone(),
            tests_root: root,
            engine: CEDAR_ENGINE.into(),
            engine_version: CEDAR_ENGINE_VERSION.into(),
            restricted_profile: CEDAR_PROFILE_V1.into(),
            previous_bundle_root: None,
            authority_summary: "Humans may decide exact eligible proposals.".into(),
        };
        assert!(bundle.root().unwrap().starts_with("sha256:"));
        assert!(
            serde_json::from_str::<PolicyBundleV1>(
                r#"{"schema":"vela.policy-bundle.v1","extra":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn retained_policy_material_matches_only_its_exact_bundle_members() {
        use vela_protocol::authority::{POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1};

        let input = input("permit(principal, action, resource);");
        let material = CedarPolicyMaterial::from_evaluation(&input);
        let bundle = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            repository_id: "vrepo_fixture".into(),
            cedar_schema_root: sha256_root(material.schema.as_bytes()),
            policies_root: sha256_root(material.policies.as_bytes()),
            entities_root: sha256_root(&material.canonical_entities().unwrap()),
            tests_root: format!("sha256:{}", "a".repeat(64)),
            engine: CEDAR_ENGINE.into(),
            engine_version: CEDAR_ENGINE_VERSION.into(),
            restricted_profile: CEDAR_PROFILE_V1.into(),
            previous_bundle_root: None,
            authority_summary: "Retain exact Cedar source for offline replay.".into(),
        };
        material.validate_against(&bundle).unwrap();

        let mut altered = material.clone();
        altered
            .policies
            .push_str("\nforbid(principal, action, resource);");
        assert!(
            altered
                .validate_against(&bundle)
                .unwrap_err()
                .contains("policies")
        );
    }
}
