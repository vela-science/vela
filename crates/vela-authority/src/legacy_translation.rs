//! Mechanical AcceptancePolicy v0.1-v0.3 translation for migration shadowing.
//!
//! This module is deliberately read-only. Era 0 remains the live writer until
//! every retained decision compares equivalent or stricter under the
//! translated Cedar bundle. Cedar answers only whether automatic admission is
//! authorized; Vela retains the domain distinction between structural Deny and
//! a valid proposal that must Defer.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::{
    acceptance_policy::{
        ACCEPTANCE_POLICY_V0_2_SCHEMA, ACCEPTANCE_POLICY_V0_3_SCHEMA, AcceptancePolicy, Outcome,
        PolicyContext, PolicyRule, evaluate as evaluate_legacy, structural_denial_reason,
    },
    authority::{
        CEDAR_ENGINE, CEDAR_ENGINE_VERSION, CEDAR_PROFILE_V1, CedarEvaluation,
        POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1, PrincipalClass,
    },
    canonical,
};

use crate::{CedarEvaluationInput, evaluate as evaluate_cedar};

pub const LEGACY_POLICY_TRANSLATION_SCHEMA_V1: &str = "vela.legacy-policy-translation.v1";
pub const LEGACY_POLICY_TRANSLATION_PROFILE_V1: &str = "vela.acceptance-policy-to-cedar.v1";
pub const LEGACY_POLICY_SHADOW_CORPUS_SCHEMA_V1: &str = "vela.legacy-policy-shadow-corpus.v1";
pub const LEGACY_POLICY_SHADOW_REPORT_SCHEMA_V1: &str = "vela.legacy-policy-shadow-report.v1";

const CEDAR_SCHEMA: &str = r#"entity Service;
entity Frontier;
action "automatic_permit" appliesTo {
    principal: Service,
    resource: Frontier,
    context: {
        structuralValid: Bool,
        claimClass: String,
        assuranceLevel: Long,
        impactTier: Long,
        changedFindings: Long,
        downstreamDependents: Long,
        assertionTextMutated: Bool,
        targetContested: Bool,
        governanceMutation: Bool,
        independenceSatisfied: Bool,
        methodIntegritySound: Bool,
        credentialValid: Bool,
        hasUnknownFields: Bool,
        replayability: String,
        executionBindingPresent: Bool,
        executionBindingValid: Bool,
        packetRoot: String,
        profileRoot: String,
        verifierCapsuleRoot: String,
        resultContractRoot: String,
        producerCredentialRootPresent: Bool,
        producerCredentialRoot: String
    }
};"#;

/// Complete, content-addressed translation output. The legacy policy remains a
/// replay input; these bytes are a candidate Era-1 bundle, never a mutation of
/// the policy or its signatures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyTranslationV1 {
    pub schema: String,
    pub profile: String,
    pub legacy_policy_id: String,
    pub legacy_policy_root: String,
    pub manifest: PolicyBundleV1,
    pub cedar_schema: String,
    pub cedar_policies: String,
    pub cedar_entities: Value,
}

impl LegacyPolicyTranslationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != LEGACY_POLICY_TRANSLATION_SCHEMA_V1
            || self.profile != LEGACY_POLICY_TRANSLATION_PROFILE_V1
            || self.legacy_policy_id.trim().is_empty()
        {
            return Err("legacy policy translation identity is invalid".into());
        }
        self.manifest.validate()?;
        if self.manifest.cedar_schema_root != sha256_bytes(self.cedar_schema.as_bytes())
            || self.manifest.policies_root != sha256_bytes(self.cedar_policies.as_bytes())
            || self.manifest.entities_root != sha256_canonical(&self.cedar_entities)?
        {
            return Err("legacy policy translation roots do not match bundle bytes".into());
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        sha256_canonical(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRelation {
    CedarNarrower,
    Equivalent,
    CedarBroader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyShadowDecision {
    pub legacy_policy_id: String,
    pub legacy_outcome: Outcome,
    pub cedar_outcome: Outcome,
    pub relation: PolicyRelation,
    pub new_cedar_permit: bool,
    pub legacy_matched_rule_ids: Vec<String>,
    pub legacy_reasons: Vec<String>,
    pub cedar: CedarEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyShadowCaseV1 {
    pub id: String,
    pub source: String,
    pub observed_at: String,
    pub policy: AcceptancePolicy,
    pub context: PolicyContext,
    pub expected_legacy_outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyShadowCorpusV1 {
    pub schema: String,
    pub tests_root: String,
    pub cases: Vec<LegacyPolicyShadowCaseV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyShadowResultV1 {
    pub case_id: String,
    pub source: String,
    pub decision: LegacyPolicyShadowDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyShadowReportV1 {
    pub schema: String,
    pub tests_root: String,
    pub case_count: u64,
    pub equivalent_count: u64,
    pub narrower_count: u64,
    pub broader_count: u64,
    pub new_cedar_permit_case_ids: Vec<String>,
    pub results: Vec<LegacyPolicyShadowResultV1>,
}

impl LegacyPolicyShadowReportV1 {
    pub fn root(&self) -> Result<String, String> {
        sha256_canonical(self)
    }

    #[must_use]
    pub fn passes_no_broader_authority_gate(&self) -> bool {
        self.broader_count == 0 && self.new_cedar_permit_case_ids.is_empty()
    }
}

/// Translate one legacy policy into deterministic Cedar source and a closed
/// bundle manifest. `tests_root` binds the independently committed shadow
/// corpus; it is not synthesized from policy prose.
pub fn translate_legacy_policy(
    policy: &AcceptancePolicy,
    tests_root: &str,
    previous_bundle_root: Option<String>,
) -> Result<LegacyPolicyTranslationV1, String> {
    let cedar_policies = cedar_policies(policy);
    let cedar_entities = cedar_entities(policy);
    let summary = authority_summary(policy);
    let legacy_policy_root = sha256_canonical(policy)?;
    let manifest = PolicyBundleV1 {
        schema: POLICY_BUNDLE_SCHEMA_V1.into(),
        frontier_id: policy.frontier_id.clone(),
        cedar_schema_root: sha256_bytes(CEDAR_SCHEMA.as_bytes()),
        policies_root: sha256_bytes(cedar_policies.as_bytes()),
        entities_root: sha256_canonical(&cedar_entities)?,
        tests_root: tests_root.into(),
        engine: CEDAR_ENGINE.into(),
        engine_version: CEDAR_ENGINE_VERSION.into(),
        restricted_profile: CEDAR_PROFILE_V1.into(),
        previous_bundle_root,
        authority_summary: summary,
    };
    let translated = LegacyPolicyTranslationV1 {
        schema: LEGACY_POLICY_TRANSLATION_SCHEMA_V1.into(),
        profile: LEGACY_POLICY_TRANSLATION_PROFILE_V1.into(),
        legacy_policy_id: policy.id.clone(),
        legacy_policy_root,
        manifest,
        cedar_schema: CEDAR_SCHEMA.into(),
        cedar_policies,
        cedar_entities,
    };
    translated.validate()?;
    Ok(translated)
}

/// Evaluate the same exact policy/context pair through both engines.
///
/// Any Cedar parser, validator, request, or runtime diagnostic prevents
/// automatic Permit. This comparison does not grant authority and cannot write
/// a policy or event.
pub fn shadow_evaluate(
    policy: &AcceptancePolicy,
    context: &PolicyContext,
    now_rfc3339: &str,
    tests_root: &str,
) -> Result<LegacyPolicyShadowDecision, String> {
    let legacy = evaluate_legacy(policy, context, now_rfc3339);
    let translated = translate_legacy_policy(policy, tests_root, None)?;
    let structural_reason = structural_denial_reason(policy, now_rfc3339);
    let cedar = evaluate_cedar(&CedarEvaluationInput {
        schema: translated.cedar_schema.clone(),
        policies: translated.cedar_policies.clone(),
        entities: translated.cedar_entities.clone(),
        principal: r#"Service::"repository-authority-shadow""#.into(),
        principal_class: PrincipalClass::Service,
        action: "automatic_permit".into(),
        resource: format!(r#"Frontier::{}"#, cedar_string(&policy.frontier_id)),
        context: cedar_context(context, structural_reason.is_none()),
    });
    let cedar_outcome = derive_domain_outcome(policy, context, structural_reason, &cedar);
    let relation = relation(legacy.outcome, cedar_outcome);
    Ok(LegacyPolicyShadowDecision {
        legacy_policy_id: policy.id.clone(),
        legacy_outcome: legacy.outcome,
        cedar_outcome,
        relation,
        new_cedar_permit: cedar_outcome == Outcome::Permit && legacy.outcome != Outcome::Permit,
        legacy_matched_rule_ids: legacy.matched_rule_ids,
        legacy_reasons: legacy.reasons,
        cedar,
    })
}

/// Compare a frozen, content-addressed corpus in stable case-ID order.
pub fn shadow_corpus(
    corpus: &LegacyPolicyShadowCorpusV1,
) -> Result<LegacyPolicyShadowReportV1, String> {
    if corpus.schema != LEGACY_POLICY_SHADOW_CORPUS_SCHEMA_V1 {
        return Err(format!(
            "shadow corpus schema must be {LEGACY_POLICY_SHADOW_CORPUS_SCHEMA_V1}"
        ));
    }
    if !is_sha256_root(&corpus.tests_root) || corpus.cases.is_empty() {
        return Err("shadow corpus requires a full tests root and at least one case".into());
    }
    let mut cases = corpus.cases.clone();
    cases.sort_by(|left, right| left.id.cmp(&right.id));
    if cases
        .windows(2)
        .any(|pair| pair[0].id.is_empty() || pair[0].id == pair[1].id)
        || cases.last().is_some_and(|case| case.id.is_empty())
    {
        return Err("shadow corpus case IDs must be unique and non-empty".into());
    }

    let mut results = Vec::with_capacity(cases.len());
    let mut equivalent_count = 0;
    let mut narrower_count = 0;
    let mut broader_count = 0;
    let mut new_cedar_permit_case_ids = Vec::new();
    for case in cases {
        if case.source.trim().is_empty() || case.observed_at.trim().is_empty() {
            return Err(format!("shadow case {} has incomplete provenance", case.id));
        }
        let decision = shadow_evaluate(
            &case.policy,
            &case.context,
            &case.observed_at,
            &corpus.tests_root,
        )?;
        if decision.legacy_outcome != case.expected_legacy_outcome {
            return Err(format!(
                "shadow case {} policy {} (derived {}) expected legacy {} but observed {} ({})",
                case.id,
                case.policy.id,
                case.policy.content_address(),
                case.expected_legacy_outcome.as_str(),
                decision.legacy_outcome.as_str(),
                decision.legacy_reasons.join(", ")
            ));
        }
        match decision.relation {
            PolicyRelation::Equivalent => equivalent_count += 1,
            PolicyRelation::CedarNarrower => narrower_count += 1,
            PolicyRelation::CedarBroader => broader_count += 1,
        }
        if decision.new_cedar_permit {
            new_cedar_permit_case_ids.push(case.id.clone());
        }
        results.push(LegacyPolicyShadowResultV1 {
            case_id: case.id,
            source: case.source,
            decision,
        });
    }
    Ok(LegacyPolicyShadowReportV1 {
        schema: LEGACY_POLICY_SHADOW_REPORT_SCHEMA_V1.into(),
        tests_root: corpus.tests_root.clone(),
        case_count: u64::try_from(results.len())
            .map_err(|_| "shadow corpus exceeds u64 case capacity")?,
        equivalent_count,
        narrower_count,
        broader_count,
        new_cedar_permit_case_ids,
        results,
    })
}

/// Deterministic reader-facing summary derived only from typed policy fields.
pub fn authority_summary(policy: &AcceptancePolicy) -> String {
    let mut clauses = Vec::new();
    clauses.push(format!(
        "Frontier {} policy epoch {} defaults to {}.",
        policy.frontier_id,
        policy.epoch,
        policy.default.as_str()
    ));
    for rule in &policy.rules {
        let classes = if rule.claim_classes.is_empty() {
            "all claim classes".to_string()
        } else {
            rule.claim_classes.join(", ")
        };
        if rule.effect == Outcome::Deny {
            clauses.push(format!("Deny {} under rule {}.", classes, rule.id));
            continue;
        }
        let constraints = &rule.constraints;
        let mut requirements = vec![
            format!("assurance at least {}", constraints.required_assurance_min),
            format!(
                "at most {} changed findings",
                constraints.max_changed_findings
            ),
            format!(
                "at most {} downstream dependents",
                constraints.max_downstream_dependents
            ),
        ];
        if constraints.require_independence {
            requirements.push("independent verification".into());
        }
        if constraints.require_method_integrity {
            requirements.push("sound method integrity".into());
        }
        if matches!(
            policy.schema.as_str(),
            ACCEPTANCE_POLICY_V0_2_SCHEMA | ACCEPTANCE_POLICY_V0_3_SCHEMA
        ) {
            requirements.push("exact packet, profile, verifier, and result bindings".into());
            requirements.push("exact replayability".into());
        }
        if policy.schema == ACCEPTANCE_POLICY_V0_3_SCHEMA {
            requirements.push("the one allowed producer credential".into());
        }
        clauses.push(format!(
            "Permit {} under rule {} only with {}.",
            classes,
            rule.id,
            requirements.join(", ")
        ));
    }
    clauses.join(" ")
}

fn cedar_policies(policy: &AcceptancePolicy) -> String {
    let mut policies = vec![
        r#"forbid (
    principal,
    action == Action::"automatic_permit",
    resource
) unless {
    context.structuralValid
};"#
        .to_string(),
    ];

    for rule in policy
        .rules
        .iter()
        .filter(|rule| rule.effect == Outcome::Deny)
    {
        policies.push(format!(
            "forbid (\n    principal,\n    action == Action::\"automatic_permit\",\n    resource\n) when {{\n    {}\n}};",
            class_expression(rule)
        ));
    }
    for rule in policy
        .rules
        .iter()
        .filter(|rule| rule.effect == Outcome::Permit)
    {
        let expressions = permit_expressions(policy, rule);
        policies.push(format!(
            "permit (\n    principal,\n    action == Action::\"automatic_permit\",\n    resource\n) when {{\n    {}\n}};",
            expressions.join(" &&\n    ")
        ));
    }
    format!("{}\n", policies.join("\n\n"))
}

fn permit_expressions(policy: &AcceptancePolicy, rule: &PolicyRule) -> Vec<String> {
    let constraints = &rule.constraints;
    let mut expressions = vec![
        class_expression(rule),
        "!context.hasUnknownFields".into(),
        format!(
            "context.assuranceLevel >= {}",
            constraints.required_assurance_min
        ),
        format!(
            "context.changedFindings <= {}",
            constraints.max_changed_findings
        ),
        format!(
            "context.downstreamDependents <= {}",
            constraints.max_downstream_dependents
        ),
    ];
    if policy.schema != ACCEPTANCE_POLICY_V0_3_SCHEMA {
        expressions.push("context.credentialValid".into());
    }
    if !constraints.allow_governance_mutation {
        expressions.push("!context.governanceMutation".into());
    }
    if !constraints.allow_contested {
        expressions.push("!context.targetContested".into());
    }
    if !constraints.allow_semantic_text_change {
        expressions.push("!context.assertionTextMutated".into());
    }
    if constraints.require_independence {
        expressions.push("context.independenceSatisfied".into());
    }
    if constraints.require_method_integrity {
        expressions.push("context.methodIntegritySound".into());
    }
    if matches!(
        policy.schema.as_str(),
        ACCEPTANCE_POLICY_V0_2_SCHEMA | ACCEPTANCE_POLICY_V0_3_SCHEMA
    ) {
        expressions.push("context.executionBindingPresent".into());
        expressions.push("context.executionBindingValid".into());
        for (field, roots) in [
            ("packetRoot", &constraints.allowed_packet_roots),
            ("profileRoot", &constraints.allowed_profile_roots),
            (
                "verifierCapsuleRoot",
                &constraints.allowed_verifier_capsule_roots,
            ),
            (
                "resultContractRoot",
                &constraints.allowed_result_contract_roots,
            ),
        ] {
            expressions.push(format!(
                "{}.contains(context.{field})",
                cedar_string_set(roots.as_deref().unwrap_or_default())
            ));
        }
        expressions.push(format!(
            "context.replayability == {}",
            cedar_string(constraints.required_replayability.as_deref().unwrap_or(""))
        ));
    }
    if policy.schema == ACCEPTANCE_POLICY_V0_3_SCHEMA {
        expressions.push("context.producerCredentialRootPresent".into());
        expressions.push(format!(
            "{}.contains(context.producerCredentialRoot)",
            cedar_string_set(
                constraints
                    .allowed_producer_credential_roots
                    .as_deref()
                    .unwrap_or_default()
            )
        ));
    }
    expressions
}

fn class_expression(rule: &PolicyRule) -> String {
    if rule.claim_classes.is_empty() {
        "true".into()
    } else {
        format!(
            "{}.contains(context.claimClass)",
            cedar_string_set(&rule.claim_classes)
        )
    }
}

fn cedar_entities(policy: &AcceptancePolicy) -> Value {
    json!([
        {
            "uid": {"type": "Service", "id": "repository-authority-shadow"},
            "attrs": {},
            "parents": []
        },
        {
            "uid": {"type": "Frontier", "id": policy.frontier_id},
            "attrs": {},
            "parents": []
        }
    ])
}

fn cedar_context(context: &PolicyContext, structural_valid: bool) -> Value {
    let execution = context.execution_binding.as_ref();
    json!({
        "structuralValid": structural_valid,
        "claimClass": context.claim_class,
        "assuranceLevel": i64::from(context.assurance_level),
        "impactTier": i64::from(context.impact_tier),
        "changedFindings": i64::from(context.changed_findings),
        "downstreamDependents": i64::from(context.downstream_dependents),
        "assertionTextMutated": context.assertion_text_mutated,
        "targetContested": context.target_contested,
        "governanceMutation": context.governance_mutation,
        "independenceSatisfied": context.independence_satisfied,
        "methodIntegritySound": context.method_integrity_sound,
        "credentialValid": context.credential_valid,
        "hasUnknownFields": context.has_unknown_fields,
        "replayability": context.replayability,
        "executionBindingPresent": execution.is_some(),
        "executionBindingValid": execution.is_some_and(|binding| binding.validate().is_ok()),
        "packetRoot": execution.map_or("", |binding| binding.packet_root.as_str()),
        "profileRoot": execution.map_or("", |binding| binding.profile_root.as_str()),
        "verifierCapsuleRoot": execution
            .map_or("", |binding| binding.verifier_capsule_root.as_str()),
        "resultContractRoot": execution
            .map_or("", |binding| binding.result_contract_root.as_str()),
        "producerCredentialRootPresent": context.producer_credential_root.is_some(),
        "producerCredentialRoot": context.producer_credential_root.as_deref().unwrap_or("")
    })
}

fn derive_domain_outcome(
    policy: &AcceptancePolicy,
    context: &PolicyContext,
    structural_reason: Option<String>,
    cedar: &CedarEvaluation,
) -> Outcome {
    if structural_reason.is_some()
        || policy.rules.iter().any(|rule| {
            rule.effect == Outcome::Deny && applies_to_class(rule, &context.claim_class)
        })
    {
        return Outcome::Deny;
    }
    if cedar.automatic_permit {
        return Outcome::Permit;
    }
    if policy
        .rules
        .iter()
        .any(|rule| rule.effect == Outcome::Permit && applies_to_class(rule, &context.claim_class))
    {
        return Outcome::Defer;
    }
    policy.default
}

fn applies_to_class(rule: &PolicyRule, class: &str) -> bool {
    rule.claim_classes.is_empty()
        || rule
            .claim_classes
            .iter()
            .any(|candidate| candidate == class)
}

fn relation(legacy: Outcome, cedar: Outcome) -> PolicyRelation {
    match outcome_rank(cedar).cmp(&outcome_rank(legacy)) {
        std::cmp::Ordering::Less => PolicyRelation::CedarNarrower,
        std::cmp::Ordering::Equal => PolicyRelation::Equivalent,
        std::cmp::Ordering::Greater => PolicyRelation::CedarBroader,
    }
}

const fn outcome_rank(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Deny => 0,
        Outcome::Defer => 1,
        Outcome::Permit => 2,
    }
}

fn cedar_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}

fn cedar_string_set(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| cedar_string(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn sha256_canonical<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    Ok(format!("sha256:{}", canonical::sha256_canonical(value)?))
}

fn is_sha256_root(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use vela_protocol::{
        acceptance_policy::{Constraints, PolicyRule, Quorum},
        receipt_v1::ExecutionBindingV1,
    };

    const NOW: &str = "2026-07-19T15:00:00Z";
    const TESTS_ROOT: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn policy(schema: &str) -> AcceptancePolicy {
        let mut policy = AcceptancePolicy {
            schema: schema.into(),
            id: String::new(),
            frontier_id: "vfr_fixture".into(),
            epoch: 1,
            issued_by: vec!["reviewer:fixture".into()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["reviewer".into()],
            },
            rules: vec![PolicyRule {
                id: "exact-work".into(),
                effect: Outcome::Permit,
                claim_classes: vec!["receipt_computational".into()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 2,
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: false,
                    require_method_integrity: true,
                    ..Constraints::default()
                },
            }],
            default: Outcome::Defer,
            expires_at: "9999-12-31T23:59:59Z".into(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();
        policy
    }

    fn binding(seed: char) -> ExecutionBindingV1 {
        ExecutionBindingV1 {
            schema: "vela.execution-binding.v1".into(),
            packet_root: full_root(seed),
            profile_root: full_root(seed),
            verifier_capsule_root: full_root(seed),
            result_contract_root: full_root(seed),
        }
    }

    fn context() -> PolicyContext {
        PolicyContext {
            claim_class: "receipt_computational".into(),
            assurance_level: 3,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: true,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: true,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "exact".into(),
            execution_binding: None,
            producer_credential_root: None,
        }
    }

    #[test]
    fn v0_1_translation_matches_permit_defer_and_deny() {
        let mut policy = policy("vela.acceptance_policy.v0.1");
        let exact = shadow_evaluate(&policy, &context(), NOW, TESTS_ROOT).unwrap();
        assert_eq!(exact.legacy_outcome, Outcome::Permit);
        assert_eq!(exact.cedar_outcome, Outcome::Permit);
        assert_eq!(exact.relation, PolicyRelation::Equivalent);

        let mut contested = context();
        contested.target_contested = true;
        let contested = shadow_evaluate(&policy, &contested, NOW, TESTS_ROOT).unwrap();
        assert_eq!(contested.cedar_outcome, Outcome::Defer);
        assert_eq!(contested.relation, PolicyRelation::Equivalent);

        policy.revocation_ref = Some("vev_revoked".into());
        policy.id = policy.content_address();
        let revoked = shadow_evaluate(&policy, &context(), NOW, TESTS_ROOT).unwrap();
        assert_eq!(revoked.cedar_outcome, Outcome::Deny);
        assert_eq!(revoked.relation, PolicyRelation::Equivalent);

        policy.revocation_ref = None;
        policy.expires_at = "2026-07-18T00:00:00Z".into();
        policy.id = policy.content_address();
        assert_equivalent(&policy, &context(), Outcome::Deny);

        policy.expires_at = "9999-12-31T23:59:59Z".into();
        policy.id = "vap_tampered".into();
        assert_equivalent(&policy, &context(), Outcome::Deny);
    }

    #[test]
    fn v0_2_translation_rejects_every_binding_substitution() {
        let mut policy = policy(ACCEPTANCE_POLICY_V0_2_SCHEMA);
        let allowed = binding('a');
        let constraints = &mut policy.rules[0].constraints;
        constraints.allowed_packet_roots = Some(vec![allowed.packet_root.clone()]);
        constraints.allowed_profile_roots = Some(vec![allowed.profile_root.clone()]);
        constraints.allowed_verifier_capsule_roots =
            Some(vec![allowed.verifier_capsule_root.clone()]);
        constraints.allowed_result_contract_roots =
            Some(vec![allowed.result_contract_root.clone()]);
        constraints.required_replayability = Some("exact".into());
        policy.id = policy.content_address();

        let mut exact = context();
        exact.execution_binding = Some(allowed);
        assert_equivalent(&policy, &exact, Outcome::Permit);

        for replacement in ['b', 'c', 'd', 'e'] {
            let mut hostile = exact.clone();
            let replacement = full_root(replacement);
            let binding = hostile.execution_binding.as_mut().unwrap();
            match replacement.as_bytes()[7] % 4 {
                0 => binding.packet_root = replacement,
                1 => binding.profile_root = replacement,
                2 => binding.verifier_capsule_root = replacement,
                _ => binding.result_contract_root = replacement,
            }
            assert_equivalent(&policy, &hostile, Outcome::Defer);
        }

        let mut missing = exact.clone();
        missing.execution_binding = None;
        assert_equivalent(&policy, &missing, Outcome::Defer);

        let mut invalid = exact.clone();
        invalid.execution_binding.as_mut().unwrap().schema = "vela.execution-binding.v999".into();
        assert_equivalent(&policy, &invalid, Outcome::Defer);

        let mut replayability_drift = exact.clone();
        replayability_drift.replayability = "bounded".into();
        assert_equivalent(&policy, &replayability_drift, Outcome::Defer);

        let mut malformed = policy.clone();
        malformed.rules[0].constraints.allowed_packet_roots = Some(vec!["sha256:abcd".into()]);
        malformed.id = malformed.content_address();
        assert_equivalent(&malformed, &exact, Outcome::Deny);
    }

    #[test]
    fn v0_3_translation_scopes_the_producer_credential() {
        let mut policy = policy(ACCEPTANCE_POLICY_V0_3_SCHEMA);
        let allowed = binding('a');
        let credential = full_root('f');
        let constraints = &mut policy.rules[0].constraints;
        constraints.allowed_packet_roots = Some(vec![allowed.packet_root.clone()]);
        constraints.allowed_profile_roots = Some(vec![allowed.profile_root.clone()]);
        constraints.allowed_verifier_capsule_roots =
            Some(vec![allowed.verifier_capsule_root.clone()]);
        constraints.allowed_result_contract_roots =
            Some(vec![allowed.result_contract_root.clone()]);
        constraints.allowed_producer_credential_roots = Some(vec![credential.clone()]);
        constraints.required_replayability = Some("exact".into());
        policy.id = policy.content_address();

        let mut exact = context();
        exact.credential_valid = false;
        exact.execution_binding = Some(allowed);
        exact.producer_credential_root = Some(credential);
        assert_equivalent(&policy, &exact, Outcome::Permit);

        exact.producer_credential_root = Some(full_root('e'));
        assert_equivalent(&policy, &exact, Outcome::Defer);
        exact.producer_credential_root = None;
        assert_equivalent(&policy, &exact, Outcome::Defer);
    }

    #[test]
    fn explicit_deny_and_default_deny_preserve_domain_routing() {
        let mut policy = policy("vela.acceptance_policy.v0.1");
        policy.rules.insert(
            0,
            PolicyRule {
                id: "forbid-computational".into(),
                effect: Outcome::Deny,
                claim_classes: vec!["receipt_computational".into()],
                constraints: Constraints::default(),
            },
        );
        policy.id = policy.content_address();
        assert_equivalent(&policy, &context(), Outcome::Deny);

        policy.rules.clear();
        policy.default = Outcome::Deny;
        policy.id = policy.content_address();
        assert_equivalent(&policy, &context(), Outcome::Deny);
    }

    #[test]
    fn bundle_roots_and_plain_language_summary_are_deterministic() {
        let policy = policy("vela.acceptance_policy.v0.1");
        let first = translate_legacy_policy(&policy, TESTS_ROOT, None).unwrap();
        let second = translate_legacy_policy(&policy, TESTS_ROOT, None).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.root().unwrap(), second.root().unwrap());
        assert!(
            first
                .manifest
                .authority_summary
                .contains("defaults to defer")
        );
        assert!(
            first
                .manifest
                .authority_summary
                .contains("Permit receipt_computational")
        );
    }

    #[test]
    fn no_new_permit_is_reported_as_a_typed_relation() {
        assert_eq!(
            relation(Outcome::Defer, Outcome::Permit),
            PolicyRelation::CedarBroader
        );
        assert_eq!(
            relation(Outcome::Permit, Outcome::Defer),
            PolicyRelation::CedarNarrower
        );
        assert_eq!(
            relation(Outcome::Permit, Outcome::Permit),
            PolicyRelation::Equivalent
        );
    }

    #[test]
    fn frozen_corpus_report_is_sorted_rooted_and_fail_closed() {
        let policy = policy("vela.acceptance_policy.v0.1");
        let mut blocked = context();
        blocked.has_unknown_fields = true;
        let corpus = LegacyPolicyShadowCorpusV1 {
            schema: LEGACY_POLICY_SHADOW_CORPUS_SCHEMA_V1.into(),
            tests_root: TESTS_ROOT.into(),
            cases: vec![
                LegacyPolicyShadowCaseV1 {
                    id: "z-blocked".into(),
                    source: "fixture:blocked".into(),
                    observed_at: NOW.into(),
                    policy: policy.clone(),
                    context: blocked,
                    expected_legacy_outcome: Outcome::Defer,
                },
                LegacyPolicyShadowCaseV1 {
                    id: "a-permit".into(),
                    source: "fixture:permit".into(),
                    observed_at: NOW.into(),
                    policy,
                    context: context(),
                    expected_legacy_outcome: Outcome::Permit,
                },
            ],
        };
        let report = shadow_corpus(&corpus).unwrap();
        assert!(report.passes_no_broader_authority_gate());
        assert_eq!(report.case_count, 2);
        assert_eq!(report.equivalent_count, 2);
        assert_eq!(report.results[0].case_id, "a-permit");
        assert!(report.root().unwrap().starts_with("sha256:"));

        let mut duplicate = corpus;
        duplicate.cases[1].id = duplicate.cases[0].id.clone();
        assert!(shadow_corpus(&duplicate).is_err());
    }

    #[test]
    fn retained_frontier_policy_corpus_has_no_broader_cedar_route() {
        let corpus: LegacyPolicyShadowCorpusV1 = serde_json::from_str(include_str!(
            "../../../conformance/fixtures/legacy-policy-shadow-corpus-v1.json"
        ))
        .unwrap();
        let report = shadow_corpus(&corpus).unwrap();
        assert!(report.passes_no_broader_authority_gate());
        assert_eq!(report.case_count, 4);
        assert_eq!(report.equivalent_count, 4);
        assert_eq!(report.narrower_count, 0);
        assert_eq!(report.broader_count, 0);
        assert!(report.new_cedar_permit_case_ids.is_empty());
        assert_eq!(
            report.root().unwrap(),
            "sha256:92f4c7568d74a87844d9b306a2dd64c95456dc867d3f8b3e9a0c6ad30c810504"
        );
    }

    #[test]
    fn adr_0013_hostile_fixture_is_equivalent_under_v0_1_and_v0_2() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../conformance/fixtures/permit-shadow-v1.json"
        ))
        .unwrap();
        let decision_time = fixture["decision_time"].as_str().unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let mut v1: AcceptancePolicy = serde_json::from_value(fixture["policy"].clone()).unwrap();
        v1.id = v1.content_address();

        let intended: ExecutionBindingV1 =
            serde_json::from_value(cases[0]["binding"].clone()).unwrap();
        let mut v2 = v1.clone();
        v2.schema = ACCEPTANCE_POLICY_V0_2_SCHEMA.into();
        let constraints = &mut v2.rules[0].constraints;
        constraints.allowed_packet_roots = Some(vec![intended.packet_root.clone()]);
        constraints.allowed_profile_roots = Some(vec![intended.profile_root.clone()]);
        constraints.allowed_verifier_capsule_roots =
            Some(vec![intended.verifier_capsule_root.clone()]);
        constraints.allowed_result_contract_roots =
            Some(vec![intended.result_contract_root.clone()]);
        constraints.required_replayability = Some("exact".into());
        v2.id = v2.content_address();

        for case in cases {
            let v1_context: PolicyContext =
                serde_json::from_value(case["policy_context"].clone()).unwrap();
            let v1_decision = shadow_evaluate(&v1, &v1_context, decision_time, TESTS_ROOT).unwrap();
            assert_eq!(
                v1_decision.legacy_outcome.as_str(),
                case["expected_v0_1"].as_str().unwrap()
            );
            assert_eq!(v1_decision.relation, PolicyRelation::Equivalent);
            assert!(!v1_decision.new_cedar_permit);

            let mut v2_context = v1_context;
            v2_context.execution_binding =
                Some(serde_json::from_value(case["binding"].clone()).unwrap());
            let v2_decision = shadow_evaluate(&v2, &v2_context, decision_time, TESTS_ROOT).unwrap();
            assert_eq!(
                v2_decision.legacy_outcome.as_str(),
                case["expected_v0_2"].as_str().unwrap()
            );
            assert_eq!(v2_decision.relation, PolicyRelation::Equivalent);
            assert!(!v2_decision.new_cedar_permit);
        }
    }

    #[test]
    fn adr_0014_credential_fixture_is_equivalent_under_v0_2_and_v0_3() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../conformance/fixtures/policy-scoped-producer-credential-v1.json"
        ))
        .unwrap();
        let decision_time = fixture["decision_time"].as_str().unwrap();
        let mut v2: AcceptancePolicy = serde_json::from_value(fixture["policy"].clone()).unwrap();
        v2.id = v2.content_address();
        let mut v3 = v2.clone();
        v3.schema = ACCEPTANCE_POLICY_V0_3_SCHEMA.into();
        v3.rules[0].constraints.allowed_producer_credential_roots = Some(vec![
            fixture["producer_credential_root"].as_str().unwrap().into(),
        ]);
        v3.id = v3.content_address();
        let base: PolicyContext = serde_json::from_value(fixture["context"].clone()).unwrap();

        for case in fixture["cases"].as_array().unwrap() {
            let mut context = base.clone();
            context.credential_valid = case["credential_valid"].as_bool().unwrap();
            context.producer_credential_root = case["producer_credential_root"]
                .as_str()
                .map(ToOwned::to_owned);

            let v2_decision = shadow_evaluate(&v2, &context, decision_time, TESTS_ROOT).unwrap();
            assert_eq!(
                v2_decision.legacy_outcome.as_str(),
                case["expected_v0_2"].as_str().unwrap()
            );
            assert_eq!(v2_decision.relation, PolicyRelation::Equivalent);
            assert!(!v2_decision.new_cedar_permit);

            let v3_decision = shadow_evaluate(&v3, &context, decision_time, TESTS_ROOT).unwrap();
            assert_eq!(
                v3_decision.legacy_outcome.as_str(),
                case["expected_v0_3"].as_str().unwrap()
            );
            assert_eq!(v3_decision.relation, PolicyRelation::Equivalent);
            assert!(!v3_decision.new_cedar_permit);
        }
    }

    fn assert_equivalent(policy: &AcceptancePolicy, context: &PolicyContext, expected: Outcome) {
        let decision = shadow_evaluate(policy, context, NOW, TESTS_ROOT).unwrap();
        assert_eq!(decision.legacy_outcome, expected);
        assert_eq!(decision.cedar_outcome, expected);
        assert_eq!(decision.relation, PolicyRelation::Equivalent);
        assert!(!decision.new_cedar_permit);
        assert!(decision.cedar.valid);
    }

    fn full_root(seed: char) -> String {
        format!("sha256:{}", seed.to_string().repeat(64))
    }
}
