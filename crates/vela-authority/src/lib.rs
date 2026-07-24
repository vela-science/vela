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
use serde_json::Value;

pub mod legacy_translation;
pub mod runtime_authentication;

pub use vela_protocol::authority::{
    CEDAR_ENGINE, CEDAR_ENGINE_VERSION, CEDAR_PROFILE_V1, CedarDecision, CedarEvaluation,
    PrincipalClass,
};
use vela_protocol::principal_capability::principal_class_may_request;
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
            frontier_id: "vfr_fixture".into(),
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
}
