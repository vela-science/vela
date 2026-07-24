//! Policy-bound acceptance (the human-governance redesign, `vela_human_governance_memo`).
//!
//! The governing change: instead of a key-holding human accepting EVERY trusted
//! transition, a human (or quorum) signs a scoped, revocable [`AcceptancePolicy`]
//! ONCE, and a deterministic evaluator then routes each proposal to `permit`,
//! `defer`, or `deny`. Humans sign policies, delegations, exceptions, and
//! irreversible commitments; the engine signs routine executions that already
//! satisfy a human-signed policy. This is a separation of duties, NOT a relaxation
//! of the gate: policy decides *authority*, never *evidence* — a transition is only
//! eligible for an auto-`permit` lane if its assurance profile already passed
//! (`verifier_attachment::exact_lane_attachment_admit` / `derive_gate_status`).
//!
//! The evaluator is **pure and replayable**: a decision is reproducible from the
//! proposal digest, the state root, the policy digest, the assurance evidence, the
//! actor credential, a bounded context object, and the evaluator version. It is
//! **monotonic on unknown data**: an unrecognized field or missing evidence can
//! only move a `permit` to `defer`/`deny`, never the reverse. The safe default is
//! `defer` (never silent denial or forced acceptance). Today this runs in SHADOW —
//! it decides and certifies but does not change the canonical accept path (no new
//! event kind, no wire change), so the autonomy can be proven before it is granted.

use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fmt};

use crate::receipt_v1::{ExecutionBindingV1, is_full_sha256_root};

/// The three routing outcomes. `defer` is the safe default and carries the reason
/// the transition needs a named human; `deny` is a structural/authority/explicit
/// prohibition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Permit,
    Defer,
    Deny,
}

impl Outcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Permit => "permit",
            Outcome::Defer => "defer",
            Outcome::Deny => "deny",
        }
    }
}

/// The bounded, structured context a rule is evaluated against. Every field is
/// derived deterministically from the proposal + its assurance evidence + the
/// frontier state (see the host that builds it). The evaluator reads ONLY these
/// fields; it never makes a network call, reads wall-clock outside `now`, or
/// consults a mutable reputation score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContext {
    /// The structural claim class (e.g. "sidon_lower_bound", "formal_theorem",
    /// "literature_finding", "metadata_repair", "governance").
    pub claim_class: String,
    /// Assurance level A0..A4 (0..4): A2 = one exact/formal check passed, A3 =
    /// independent corroboration, A4 = adversarial+semantic fidelity. Derived from
    /// the gate, NOT self-asserted.
    pub assurance_level: u8,
    /// Transition impact tier I0..I4 (0..4).
    pub impact_tier: u8,
    pub changed_findings: u32,
    pub downstream_dependents: u32,
    /// Does the proposal mutate claim LANGUAGE (vs attach exact evidence)?
    pub assertion_text_mutated: bool,
    pub target_contested: bool,
    pub governance_mutation: bool,
    /// Independence derived from failure-domain diversity (not self-declared).
    pub independence_satisfied: bool,
    /// MethodIntegrity::Sound on the matched attachments.
    pub method_integrity_sound: bool,
    /// The producer/delegate credential resolved and is unexpired/unrevoked.
    pub credential_valid: bool,
    /// The evaluator saw a field it does not recognize → never permit (monotonic).
    pub has_unknown_fields: bool,
    /// The originating receipt's honest replay classification (`exact` | `bounded`
    /// | `approximate` | `unavailable` | `unknown`; docs/RECEIPTS.md). Hosts set
    /// `unknown` explicitly for non-receipt transitions. A policy MAY require
    /// `exact` to auto-admit a serious claim class.
    pub replayability: String,
    /// Optional closed Receipt v1 execution identity. It is absent from
    /// historical contexts and ignored by policy v0.1; policy v0.2/v0.3 may
    /// require exact full-root matches before Permit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_binding: Option<ExecutionBindingV1>,
    /// Full root of the verified Receipt v1 producer identity binding. It is
    /// absent from historical contexts and populated only for policy v0.3 so
    /// v0.1/v0.2 decision certificates retain their exact context bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_credential_root: Option<String>,
}

impl Default for PolicyContext {
    fn default() -> Self {
        // The maximally-cautious context: nothing proven, everything that would
        // force a defer/deny set. A rule must positively clear each gate.
        PolicyContext {
            claim_class: String::new(),
            assurance_level: 0,
            impact_tier: 4,
            changed_findings: u32::MAX,
            downstream_dependents: u32::MAX,
            assertion_text_mutated: true,
            target_contested: true,
            governance_mutation: true,
            independence_satisfied: false,
            method_integrity_sound: false,
            credential_valid: false,
            has_unknown_fields: true,
            replayability: "unknown".to_string(),
            execution_binding: None,
            producer_credential_root: None,
        }
    }
}

impl PolicyContext {
    /// Canonical digest of the complete input surface consumed by the live
    /// policy language.
    ///
    /// Keep this helper beside [`PolicyContext`]: callers must not maintain a
    /// second hand-picked projection that can drift when the evaluator gains a
    /// field. The digest is evidence about which facts were evaluated; it is
    /// not an authority token and cannot turn a producer assertion into a
    /// verified fact.
    pub fn policy_language_digest(&self) -> Result<String, String> {
        crate::canonical::sha256_canonical(self).map(|digest| format!("sha256:{digest}"))
    }
}

/// The constraints a `permit` rule places on the transition. A `permit` fires only
/// if ALL hold; otherwise the rule does not match and routing falls through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    pub max_changed_findings: u32,
    pub max_downstream_dependents: u32,
    /// Lowest assurance level (0..4) that satisfies the rule.
    pub required_assurance_min: u8,
    /// `false` (the default) forbids claim-language mutation in this lane.
    #[serde(default)]
    pub allow_semantic_text_change: bool,
    #[serde(default)]
    pub allow_contested: bool,
    #[serde(default)]
    pub allow_governance_mutation: bool,
    /// Require failure-domain-diverse independent verification.
    #[serde(default)]
    pub require_independence: bool,
    /// Require MethodIntegrity::Sound.
    #[serde(default)]
    pub require_method_integrity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_packet_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_profile_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_verifier_capsule_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_result_contract_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_producer_credential_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_replayability: Option<String>,
}

impl Default for Constraints {
    fn default() -> Self {
        Constraints {
            max_changed_findings: 0,
            max_downstream_dependents: 0,
            required_assurance_min: 4,
            allow_semantic_text_change: false,
            allow_contested: false,
            allow_governance_mutation: false,
            require_independence: true,
            require_method_integrity: true,
            allowed_packet_roots: None,
            allowed_profile_roots: None,
            allowed_verifier_capsule_roots: None,
            allowed_result_contract_roots: None,
            allowed_producer_credential_roots: None,
            required_replayability: None,
        }
    }
}

/// One rule: an effect plus the claim classes and constraints it applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRule {
    pub id: String,
    pub effect: Outcome,
    /// Claim classes this rule governs. Empty = applies to any class.
    #[serde(default)]
    pub claim_classes: Vec<String>,
    #[serde(default)]
    pub constraints: Constraints,
}

impl PolicyRule {
    fn applies_to_class(&self, class: &str) -> bool {
        self.claim_classes.is_empty() || self.claim_classes.iter().any(|c| c == class)
    }
}

/// The quorum that must have signed the policy for it to carry authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quorum {
    pub threshold: u32,
    #[serde(default)]
    pub eligible_roles: Vec<String>,
}

/// A scoped, revocable, content-addressed acceptance policy. Humans sign THIS
/// (once); the evaluator applies it (many times). Shares signature/quorum/expiry/
/// revocation shape with registry governance; this governs ordinary scientific
/// state transitions, not owner rotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptancePolicy {
    pub schema: String,
    pub id: String,
    pub frontier_id: String,
    pub epoch: u32,
    pub issued_by: Vec<String>,
    pub quorum: Quorum,
    pub rules: Vec<PolicyRule>,
    /// MUST be `Defer` or `Deny` (a permit default would be a footgun).
    pub default: Outcome,
    /// RFC3339; the evaluator denies after this instant.
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_ref: Option<String>,
}

pub const ACCEPTANCE_POLICY_V0_1_SCHEMA: &str = "vela.acceptance_policy.v0.1";
pub const ACCEPTANCE_POLICY_V0_2_SCHEMA: &str = "vela.acceptance_policy.v0.2";
pub const ACCEPTANCE_POLICY_V0_3_SCHEMA: &str = "vela.acceptance_policy.v0.3";

/// The evaluator version, bound into every decision for replay.
pub const EVALUATOR_VERSION: &str = "vela-policy@0.1.0";
pub const EVALUATOR_VERSION_V0_2: &str = "vela-policy@0.2.0";
pub const EVALUATOR_VERSION_V0_3: &str = "vela-policy@0.3.0";

impl AcceptancePolicy {
    /// Content address of the policy's normative body (everything but `id`), so
    /// the id is reproducible and a tampered policy fails to verify. `vap_` prefix.
    #[must_use]
    pub fn content_address(&self) -> String {
        let mut body = self.clone();
        body.id.clear();
        // AcceptancePolicy contains only the canonical JSON data model. Keep
        // its content address on the same canonical bytes used by signatures;
        // never collapse a serialization failure to the hash of empty bytes.
        let bytes = crate::canonical::to_canonical_bytes(&body)
            .expect("AcceptancePolicy must serialize as canonical JSON");
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("vap_{}", hex16(&h.finalize()))
    }

    /// True iff `id` matches the content address (tamper check).
    #[must_use]
    pub fn id_is_valid(&self) -> bool {
        self.id == self.content_address()
    }

    /// Is the policy expired at `now`? Both values are parsed as RFC3339
    /// instants before comparison, so equivalent offsets compare correctly.
    /// Malformed lifecycle data fails closed and is treated as expired.
    #[must_use]
    pub fn is_expired(&self, now_rfc3339: &str) -> bool {
        let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(&self.expires_at) else {
            return true;
        };
        let Ok(now) = chrono::DateTime::parse_from_rfc3339(now_rfc3339) else {
            return true;
        };
        now >= expires_at
    }
}

/// A pure, replayable routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub outcome: Outcome,
    pub matched_rule_ids: Vec<String>,
    /// Machine-readable reason codes (always non-empty).
    pub reasons: Vec<String>,
    pub evaluator: String,
    pub policy_id: String,
}

/// The deterministic routing engine (memo Appendix C). Order of precedence:
/// 1. structural/authority DENY (expired/revoked policy, governance mutation
///    without an explicit governance-permit rule, invalid credential);
/// 2. any matching `deny` rule → DENY;
/// 3. escalation triggers (contested, semantic mutation, missing independence/
///    integrity where required, downstream over bound, unknown fields) → DEFER;
/// 4. a matching `permit` rule whose constraints all hold → PERMIT;
/// 5. otherwise the policy `default` (DEFER or DENY).
///
/// Monotonic on unknown: `has_unknown_fields` or an unrecognized class can only
/// push toward `defer`/`deny`.
#[must_use]
pub fn evaluate(policy: &AcceptancePolicy, ctx: &PolicyContext, now_rfc3339: &str) -> Decision {
    let evaluator = match policy.schema.as_str() {
        ACCEPTANCE_POLICY_V0_2_SCHEMA => EVALUATOR_VERSION_V0_2,
        ACCEPTANCE_POLICY_V0_3_SCHEMA => EVALUATOR_VERSION_V0_3,
        _ => EVALUATOR_VERSION,
    };
    let mk = |outcome: Outcome, rules: Vec<String>, reasons: Vec<String>| Decision {
        outcome,
        matched_rule_ids: rules,
        reasons: if reasons.is_empty() {
            vec!["default".to_string()]
        } else {
            reasons
        },
        evaluator: evaluator.to_string(),
        policy_id: policy.id.clone(),
    };

    // (0) Policy integrity + lifecycle: structural DENY.
    if let Some(reason) = structural_denial_reason(policy, now_rfc3339) {
        return mk(Outcome::Deny, vec![], vec![reason]);
    }

    // (1) Explicit DENY rules win over everything below.
    for r in &policy.rules {
        if r.effect == Outcome::Deny && r.applies_to_class(&ctx.claim_class) {
            return mk(
                Outcome::Deny,
                vec![r.id.clone()],
                vec!["explicit_deny_rule".into()],
            );
        }
    }

    // (2) Find a permit rule for this class; if it matches but constraints fail,
    // that is an escalation (defer), not a silent denial.
    let mut escalations: Vec<String> = Vec::new();
    for r in &policy.rules {
        if r.effect != Outcome::Permit || !r.applies_to_class(&ctx.claim_class) {
            continue;
        }
        let c = &r.constraints;
        let mut blocked: Vec<String> = Vec::new();

        // Monotonic-on-unknown + the universal escalation triggers.
        if ctx.has_unknown_fields {
            blocked.push("unknown_fields".into());
        }
        if policy.schema == ACCEPTANCE_POLICY_V0_3_SCHEMA {
            match ctx.producer_credential_root.as_ref() {
                Some(root)
                    if c.allowed_producer_credential_roots
                        .as_ref()
                        .is_some_and(|roots| roots.iter().any(|allowed| allowed == root)) => {}
                Some(_) => blocked.push("producer_credential_root_not_allowed".into()),
                None => blocked.push("producer_credential_root_missing".into()),
            }
        } else if !ctx.credential_valid {
            blocked.push("credential_invalid".into());
        }
        if ctx.governance_mutation && !c.allow_governance_mutation {
            blocked.push("governance_mutation".into());
        }
        if ctx.target_contested && !c.allow_contested {
            blocked.push("target_contested".into());
        }
        if ctx.assertion_text_mutated && !c.allow_semantic_text_change {
            blocked.push("semantic_text_change".into());
        }
        if ctx.assurance_level < c.required_assurance_min {
            blocked.push(format!(
                "assurance_below_min({}<{})",
                ctx.assurance_level, c.required_assurance_min
            ));
        }
        if ctx.changed_findings > c.max_changed_findings {
            blocked.push("changed_findings_over_bound".into());
        }
        if ctx.downstream_dependents > c.max_downstream_dependents {
            blocked.push("downstream_over_bound".into());
        }
        if c.require_independence && !ctx.independence_satisfied {
            blocked.push("independence_unsatisfied".into());
        }
        if c.require_method_integrity && !ctx.method_integrity_sound {
            blocked.push("method_integrity_unattested".into());
        }
        if matches!(
            policy.schema.as_str(),
            ACCEPTANCE_POLICY_V0_2_SCHEMA | ACCEPTANCE_POLICY_V0_3_SCHEMA
        ) {
            let Some(binding) = ctx.execution_binding.as_ref() else {
                blocked.push("execution_binding_missing".into());
                escalations.extend(
                    blocked
                        .into_iter()
                        .map(|reason| format!("{}:{reason}", r.id)),
                );
                continue;
            };
            if binding.validate().is_err() {
                blocked.push("execution_binding_invalid".into());
                escalations.extend(
                    blocked
                        .into_iter()
                        .map(|reason| format!("{}:{reason}", r.id)),
                );
                continue;
            }
            for (field, allowlist, actual) in [
                ("packet_root", &c.allowed_packet_roots, &binding.packet_root),
                (
                    "profile_root",
                    &c.allowed_profile_roots,
                    &binding.profile_root,
                ),
                (
                    "verifier_capsule_root",
                    &c.allowed_verifier_capsule_roots,
                    &binding.verifier_capsule_root,
                ),
                (
                    "result_contract_root",
                    &c.allowed_result_contract_roots,
                    &binding.result_contract_root,
                ),
            ] {
                if !allowlist
                    .as_ref()
                    .is_some_and(|roots| roots.iter().any(|root| root == actual))
                {
                    blocked.push(format!("{field}_not_allowed"));
                }
            }
            if c.required_replayability.as_deref() != Some(ctx.replayability.as_str()) {
                blocked.push("replayability_not_allowed".into());
            }
        }

        if blocked.is_empty() {
            return mk(
                Outcome::Permit,
                vec![r.id.clone()],
                vec!["all_constraints_satisfied".into()],
            );
        }
        // This permit rule matched the class but is blocked → remember why; a
        // matched-but-blocked permit rule means the item is plausibly valid but
        // needs human judgment, so we DEFER (not deny).
        escalations.extend(blocked.into_iter().map(|b| format!("{}:{}", r.id, b)));
    }

    if !escalations.is_empty() {
        return mk(Outcome::Defer, vec![], escalations);
    }

    // (3) No deny, no matching permit → the policy default.
    mk(
        policy.default,
        vec![],
        vec![format!("default_{}", policy.default.as_str())],
    )
}

/// Return the exact structural reason that prevents this policy from
/// participating in routing.
///
/// This is public only for the read-only Era-0-to-Era-1 shadow translator. The
/// live evaluator remains the authority during the migration and calls this
/// same helper, so the shadow path cannot maintain a second approximation of
/// policy lifecycle or binding validation.
#[must_use]
pub fn structural_denial_reason(policy: &AcceptancePolicy, now_rfc3339: &str) -> Option<String> {
    if !matches!(
        policy.schema.as_str(),
        ACCEPTANCE_POLICY_V0_1_SCHEMA
            | ACCEPTANCE_POLICY_V0_2_SCHEMA
            | ACCEPTANCE_POLICY_V0_3_SCHEMA
    ) {
        return Some("policy_schema_unsupported".into());
    }
    if !policy.id_is_valid() {
        return Some("policy_id_mismatch".into());
    }
    if policy.is_expired(now_rfc3339) {
        return Some("policy_expired".into());
    }
    if policy.revocation_ref.is_some() {
        return Some("policy_revoked".into());
    }
    if !matches!(policy.default, Outcome::Defer | Outcome::Deny) {
        return Some("illegal_permit_default".into());
    }
    binding_policy_error(policy)
}

fn binding_policy_error(policy: &AcceptancePolicy) -> Option<String> {
    for rule in policy
        .rules
        .iter()
        .filter(|rule| rule.effect == Outcome::Permit)
    {
        let constraints = &rule.constraints;
        let v2_fields_present = constraints.allowed_packet_roots.is_some()
            || constraints.allowed_profile_roots.is_some()
            || constraints.allowed_verifier_capsule_roots.is_some()
            || constraints.allowed_result_contract_roots.is_some()
            || constraints.required_replayability.is_some();
        let v3_fields_present = constraints.allowed_producer_credential_roots.is_some();
        if policy.schema == ACCEPTANCE_POLICY_V0_1_SCHEMA {
            if v2_fields_present || v3_fields_present {
                return Some("policy_v0_2_constraints_under_v0_1".to_string());
            }
            continue;
        }
        if policy.schema == ACCEPTANCE_POLICY_V0_2_SCHEMA && v3_fields_present {
            return Some("policy_v0_3_constraints_under_v0_2".to_string());
        }
        if !matches!(
            policy.schema.as_str(),
            ACCEPTANCE_POLICY_V0_2_SCHEMA | ACCEPTANCE_POLICY_V0_3_SCHEMA
        ) {
            continue;
        }
        for (field, roots) in [
            ("packet", &constraints.allowed_packet_roots),
            ("profile", &constraints.allowed_profile_roots),
            (
                "verifier_capsule",
                &constraints.allowed_verifier_capsule_roots,
            ),
            (
                "result_contract",
                &constraints.allowed_result_contract_roots,
            ),
        ] {
            let Some(roots) = roots.as_ref() else {
                return Some(format!("policy_{field}_allowlist_missing"));
            };
            if roots.is_empty() || roots.iter().any(|root| !is_full_sha256_root(root)) {
                return Some(format!("policy_{field}_allowlist_invalid"));
            }
        }
        if constraints.required_replayability.as_deref() != Some("exact") {
            return Some("policy_exact_replayability_required".to_string());
        }
        if policy.schema == ACCEPTANCE_POLICY_V0_3_SCHEMA {
            let Some(roots) = constraints.allowed_producer_credential_roots.as_ref() else {
                return Some("policy_producer_credential_allowlist_missing".to_string());
            };
            if roots.is_empty() || roots.iter().any(|root| !is_full_sha256_root(root)) {
                return Some("policy_producer_credential_allowlist_invalid".to_string());
            }
            let unique = roots.iter().collect::<HashSet<_>>();
            if unique.len() != roots.len() {
                return Some("policy_producer_credential_allowlist_duplicate".to_string());
            }
            if roots.len() != 1 {
                return Some("policy_producer_credential_allowlist_invalid".to_string());
            }
            for (_, allowed) in [
                ("packet", &constraints.allowed_packet_roots),
                ("profile", &constraints.allowed_profile_roots),
                (
                    "verifier_capsule",
                    &constraints.allowed_verifier_capsule_roots,
                ),
                (
                    "result_contract",
                    &constraints.allowed_result_contract_roots,
                ),
            ] {
                let roots = allowed.as_ref().expect("v0.3 exact roots validated above");
                if roots.iter().collect::<HashSet<_>>().len() != roots.len() {
                    return Some("policy_exact_allowlist_duplicate".to_string());
                }
            }
        }
    }
    None
}

/// First 16 bytes of a digest as hex (mirrors the substrate's short-id style).
fn hex16(digest: &[u8]) -> String {
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// How the transition was authorized. `PolicyDelegation` = a human-signed policy
/// permitted it (the engine executed); `DirectHuman` = a person signed this item;
/// `Quorum` = a governance quorum signed. Never collapse these into one "signed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityMode {
    PolicyDelegation,
    DirectHuman,
    Quorum,
}

/// The portable, content-addressed receipt of one acceptance decision — produced
/// by the engine, not performed by a human. It binds the proposal, the exact state
/// roots, the policy + matched rules, the authority chain, the assurance profile,
/// and (once recorded) the event + log inclusion proof, so any relying party can
/// REPLAY the decision. Reading it requires no signature; only a durable
/// endorsement/commitment does (the memo's separation of receipt from endorsement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionCertificate {
    #[serde(default = "default_cert_schema")]
    pub schema: String,
    pub id: String,
    pub frontier_id: String,
    pub proposal_id: String,
    pub state_root_before: String,
    pub state_root_after: String,
    pub outcome: Outcome,
    pub policy_id: String,
    pub rule_ids: Vec<String>,
    pub evaluator: String,
    pub authority_mode: AuthorityMode,
    /// The human(s)/quorum whose signed policy authorized this (for PolicyDelegation).
    pub human_authorizers: Vec<String>,
    /// The service/agent that executed under the policy.
    pub executor: String,
    /// The named assurance profile the evidence cleared (e.g.
    /// "exact_construction_dual_check_v1"); the policy NEVER manufactures this.
    pub assurance_profile: String,
    pub assurance_level: u8,
    pub claim_digest: String,
    pub impact_tier: u8,
    pub reasons: Vec<String>,
    /// Selected for post-accept audit (the calibrated-sample rollout).
    pub audit_required: bool,
}

fn default_cert_schema() -> String {
    "vela.decision_certificate.v0.1".to_string()
}

impl DecisionCertificate {
    /// Build a certificate from a decision + its bindings. `vdc_` prefix; the id is
    /// the content address of everything but `id`, so it is reproducible and a
    /// tampered certificate fails `id_is_valid`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        decision: &Decision,
        frontier_id: &str,
        proposal_id: &str,
        state_root_before: &str,
        state_root_after: &str,
        authority_mode: AuthorityMode,
        human_authorizers: Vec<String>,
        executor: &str,
        assurance_profile: &str,
        assurance_level: u8,
        claim_digest: &str,
        impact_tier: u8,
        audit_required: bool,
    ) -> Self {
        let mut c = DecisionCertificate {
            schema: default_cert_schema(),
            id: String::new(),
            frontier_id: frontier_id.to_string(),
            proposal_id: proposal_id.to_string(),
            state_root_before: state_root_before.to_string(),
            state_root_after: state_root_after.to_string(),
            outcome: decision.outcome,
            policy_id: decision.policy_id.clone(),
            rule_ids: decision.matched_rule_ids.clone(),
            evaluator: decision.evaluator.clone(),
            authority_mode,
            human_authorizers,
            executor: executor.to_string(),
            assurance_profile: assurance_profile.to_string(),
            assurance_level,
            claim_digest: claim_digest.to_string(),
            impact_tier,
            reasons: decision.reasons.clone(),
            audit_required,
        };
        c.id = c.content_address();
        c
    }

    #[must_use]
    pub fn content_address(&self) -> String {
        let mut probe = self.clone();
        probe.id = String::new();
        let bytes = serde_json::to_vec(&probe).unwrap_or_default();
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("vdc_{}", hex16(&h.finalize()))
    }

    #[must_use]
    pub fn id_is_valid(&self) -> bool {
        self.id == self.content_address()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;
    use serde_json::json;
    use tempfile::TempDir;

    fn exact_sidon_policy() -> AcceptancePolicy {
        let mut p = AcceptancePolicy {
            schema: ACCEPTANCE_POLICY_V0_1_SCHEMA.to_string(),
            id: String::new(),
            frontier_id: "vfr_test".into(),
            epoch: 1,
            issued_by: vec!["reviewer:will".into()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".into()],
            },
            rules: vec![PolicyRule {
                id: "sidon-exact-auto-v1".into(),
                effect: Outcome::Permit,
                claim_classes: vec!["sidon_lower_bound".into()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 5,
                    required_assurance_min: 3,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: true,
                    require_method_integrity: true,
                    ..Constraints::default()
                },
            }],
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".into(),
            revocation_ref: None,
        };
        p.id = p.content_address();
        p
    }

    fn clean_exact_ctx() -> PolicyContext {
        PolicyContext {
            claim_class: "sidon_lower_bound".into(),
            assurance_level: 3,
            impact_tier: 2,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: false,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: true,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "unknown".to_string(),
            execution_binding: None,
            producer_credential_root: None,
        }
    }

    const NOW: &str = "2026-06-23T00:00:00Z";
    const SIGNED_AT: &str = "2026-06-22T00:00:00Z";

    fn signed_policy_bytes(policy: &AcceptancePolicy, signed_at: &str) -> (Vec<u8>, Vec<u8>) {
        let key = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
        let preimage = policy_signature_preimage(policy, signed_at).unwrap();
        let signature = key.sign(&preimage);
        let record = PolicySignatureRecord {
            policy_id: policy.id.clone(),
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(signature.to_bytes()),
            signed_at: signed_at.to_string(),
        };
        (
            serde_json::to_vec_pretty(policy).unwrap(),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
    }

    fn write_active_pair(policy_bytes: &[u8], signature_bytes: &[u8]) -> TempDir {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join(".vela/policies");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("active.json"), policy_bytes).unwrap();
        std::fs::write(directory.join("active.sig.json"), signature_bytes).unwrap();
        temp
    }

    #[test]
    fn exact_clean_witness_permits() {
        let d = evaluate(&exact_sidon_policy(), &clean_exact_ctx(), NOW);
        assert_eq!(d.outcome, Outcome::Permit);
        assert_eq!(d.matched_rule_ids, vec!["sidon-exact-auto-v1"]);
    }

    #[test]
    fn contested_target_escalates_to_defer() {
        let mut ctx = clean_exact_ctx();
        ctx.target_contested = true;
        let d = evaluate(&exact_sidon_policy(), &ctx, NOW);
        assert_eq!(d.outcome, Outcome::Defer);
        assert!(d.reasons.iter().any(|r| r.contains("target_contested")));
    }

    #[test]
    fn semantic_text_mutation_escalates_to_defer() {
        let mut ctx = clean_exact_ctx();
        ctx.assertion_text_mutated = true;
        assert_eq!(
            evaluate(&exact_sidon_policy(), &ctx, NOW).outcome,
            Outcome::Defer
        );
    }

    #[test]
    fn low_assurance_escalates_to_defer() {
        let mut ctx = clean_exact_ctx();
        ctx.assurance_level = 2; // below the rule's min of 3
        let d = evaluate(&exact_sidon_policy(), &ctx, NOW);
        assert_eq!(d.outcome, Outcome::Defer);
        assert!(d.reasons.iter().any(|r| r.contains("assurance_below_min")));
    }

    #[test]
    fn monotonic_on_unknown_fields() {
        let mut ctx = clean_exact_ctx();
        ctx.has_unknown_fields = true;
        assert_eq!(
            evaluate(&exact_sidon_policy(), &ctx, NOW).outcome,
            Outcome::Defer
        );
    }

    #[test]
    fn unknown_claim_class_falls_through_to_default_defer() {
        let mut ctx = clean_exact_ctx();
        ctx.claim_class = "literature_finding".into(); // no rule covers it
        let d = evaluate(&exact_sidon_policy(), &ctx, NOW);
        assert_eq!(d.outcome, Outcome::Defer);
        assert!(d.reasons.iter().any(|r| r.contains("default")));
    }

    #[test]
    fn expired_policy_denies() {
        let p = exact_sidon_policy();
        let d = evaluate(&p, &clean_exact_ctx(), "2100-01-01T00:00:00Z");
        assert_eq!(d.outcome, Outcome::Deny);
        assert!(d.reasons.iter().any(|r| r == "policy_expired"));
    }

    #[test]
    fn expiry_compares_rfc3339_instants_and_malformed_values_fail_closed() {
        let mut policy = exact_sidon_policy();
        policy.expires_at = "2026-06-23T00:00:00Z".to_string();

        assert!(!policy.is_expired("2026-06-23T01:59:59+02:00"));
        assert!(policy.is_expired("2026-06-22T20:00:00-04:00"));
        assert!(policy.is_expired("not-rfc3339"));

        policy.id = policy.content_address();
        let decision = evaluate(&policy, &clean_exact_ctx(), "not-rfc3339");
        assert_eq!(decision.outcome, Outcome::Deny);
        assert_eq!(decision.reasons, vec!["policy_expired"]);

        policy.expires_at = "also-not-rfc3339".to_string();
        assert!(policy.is_expired(NOW));
    }

    #[test]
    fn unsupported_policy_schema_denies_even_with_a_matching_id() {
        let mut policy = exact_sidon_policy();
        policy.schema = "vela.acceptance_policy.v999".to_string();
        policy.id = policy.content_address();
        let decision = evaluate(&policy, &clean_exact_ctx(), NOW);
        assert_eq!(decision.outcome, Outcome::Deny);
        assert_eq!(decision.reasons, vec!["policy_schema_unsupported"]);
    }

    #[test]
    fn policy_scoped_producer_credential_is_exact_and_narrower_than_registry_status() {
        let root = |digit: char| format!("sha256:{}", digit.to_string().repeat(64));
        let binding = ExecutionBindingV1 {
            schema: crate::receipt_v1::EXECUTION_BINDING_SCHEMA.to_string(),
            packet_root: root('1'),
            profile_root: root('2'),
            verifier_capsule_root: root('3'),
            result_contract_root: root('4'),
        };
        let credential = root('5');
        let mut policy = exact_sidon_policy();
        policy.schema = ACCEPTANCE_POLICY_V0_3_SCHEMA.to_string();
        let constraints = &mut policy.rules[0].constraints;
        constraints.allowed_packet_roots = Some(vec![binding.packet_root.clone()]);
        constraints.allowed_profile_roots = Some(vec![binding.profile_root.clone()]);
        constraints.allowed_verifier_capsule_roots =
            Some(vec![binding.verifier_capsule_root.clone()]);
        constraints.allowed_result_contract_roots =
            Some(vec![binding.result_contract_root.clone()]);
        constraints.allowed_producer_credential_roots = Some(vec![credential.clone()]);
        constraints.required_replayability = Some("exact".to_string());
        policy.id = policy.content_address();

        let mut context = clean_exact_ctx();
        context.credential_valid = false;
        context.replayability = "exact".to_string();
        context.execution_binding = Some(binding);
        context.producer_credential_root = Some(credential.clone());
        let decision = evaluate(&policy, &context, NOW);
        assert_eq!(decision.outcome, Outcome::Permit, "{decision:?}");
        assert_eq!(decision.evaluator, EVALUATOR_VERSION_V0_3);

        context.credential_valid = true;
        context.producer_credential_root = Some(root('6'));
        let wrong = evaluate(&policy, &context, NOW);
        assert_eq!(wrong.outcome, Outcome::Defer);
        assert!(
            wrong
                .reasons
                .iter()
                .any(|reason| reason.ends_with("producer_credential_root_not_allowed"))
        );

        context.producer_credential_root = None;
        let missing = evaluate(&policy, &context, NOW);
        assert_eq!(missing.outcome, Outcome::Defer);
        assert!(
            missing
                .reasons
                .iter()
                .any(|reason| reason.ends_with("producer_credential_root_missing"))
        );

        let mut duplicate = policy.clone();
        duplicate.rules[0]
            .constraints
            .allowed_producer_credential_roots = Some(vec![credential.clone(), credential]);
        duplicate.id = duplicate.content_address();
        let denied = evaluate(&duplicate, &context, NOW);
        assert_eq!(denied.outcome, Outcome::Deny);
        assert_eq!(
            denied.reasons,
            vec!["policy_producer_credential_allowlist_duplicate"]
        );
    }

    #[test]
    fn tampered_policy_id_denies() {
        let mut p = exact_sidon_policy();
        p.rules[0].constraints.required_assurance_min = 0; // change body, keep old id
        let d = evaluate(&p, &clean_exact_ctx(), NOW);
        assert_eq!(d.outcome, Outcome::Deny);
        assert!(d.reasons.iter().any(|r| r == "policy_id_mismatch"));
    }

    #[test]
    fn revoked_policy_denies() {
        let mut p = exact_sidon_policy();
        p.revocation_ref = Some("vrv_x".into());
        p.id = p.content_address();
        assert_eq!(evaluate(&p, &clean_exact_ctx(), NOW).outcome, Outcome::Deny);
    }

    #[test]
    fn governance_mutation_without_permission_escalates() {
        let mut ctx = clean_exact_ctx();
        ctx.governance_mutation = true;
        let d = evaluate(&exact_sidon_policy(), &ctx, NOW);
        assert_eq!(d.outcome, Outcome::Defer);
        assert!(d.reasons.iter().any(|r| r.contains("governance_mutation")));
    }

    #[test]
    fn evaluation_is_deterministic() {
        let p = exact_sidon_policy();
        let ctx = clean_exact_ctx();
        assert_eq!(evaluate(&p, &ctx, NOW), evaluate(&p, &ctx, NOW));
    }

    #[test]
    fn policy_context_digest_is_canonical_and_binds_every_live_field() {
        let baseline = clean_exact_ctx();
        let digest = baseline.policy_language_digest().unwrap();
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest, baseline.policy_language_digest().unwrap());

        let mut variants = Vec::new();
        let mut value = baseline.clone();
        value.claim_class.push_str("_changed");
        variants.push(value);
        let mut value = baseline.clone();
        value.assurance_level += 1;
        variants.push(value);
        let mut value = baseline.clone();
        value.impact_tier += 1;
        variants.push(value);
        let mut value = baseline.clone();
        value.changed_findings += 1;
        variants.push(value);
        let mut value = baseline.clone();
        value.downstream_dependents += 1;
        variants.push(value);
        let mut value = baseline.clone();
        value.assertion_text_mutated = !value.assertion_text_mutated;
        variants.push(value);
        let mut value = baseline.clone();
        value.target_contested = !value.target_contested;
        variants.push(value);
        let mut value = baseline.clone();
        value.governance_mutation = !value.governance_mutation;
        variants.push(value);
        let mut value = baseline.clone();
        value.independence_satisfied = !value.independence_satisfied;
        variants.push(value);
        let mut value = baseline.clone();
        value.method_integrity_sound = !value.method_integrity_sound;
        variants.push(value);
        let mut value = baseline.clone();
        value.credential_valid = !value.credential_valid;
        variants.push(value);
        let mut value = baseline.clone();
        value.has_unknown_fields = !value.has_unknown_fields;
        variants.push(value);
        let mut value = baseline.clone();
        value.replayability = "exact".to_string();
        variants.push(value);
        let mut value = baseline.clone();
        value.execution_binding = Some(ExecutionBindingV1 {
            schema: crate::receipt_v1::EXECUTION_BINDING_SCHEMA.to_string(),
            packet_root: format!("sha256:{}", "1".repeat(64)),
            profile_root: format!("sha256:{}", "2".repeat(64)),
            verifier_capsule_root: format!("sha256:{}", "3".repeat(64)),
            result_contract_root: format!("sha256:{}", "4".repeat(64)),
        });
        variants.push(value);
        let mut value = baseline.clone();
        value.producer_credential_root = Some(format!("sha256:{}", "5".repeat(64)));
        variants.push(value);

        for variant in variants {
            assert_ne!(digest, variant.policy_language_digest().unwrap());
        }
    }

    #[test]
    fn policy_context_wire_requires_replayability() {
        let mut value = serde_json::to_value(clean_exact_ctx()).unwrap();
        value.as_object_mut().unwrap().remove("replayability");
        assert!(serde_json::from_value::<PolicyContext>(value).is_err());
    }

    #[test]
    fn content_address_is_stable_and_prefixed() {
        let p = exact_sidon_policy();
        assert!(p.id.starts_with("vap_"));
        assert!(p.id_is_valid());

        let mut body = p.clone();
        body.id.clear();
        let canonical = crate::canonical::to_canonical_bytes(&body).unwrap();
        let mut digest = Sha256::new();
        digest.update(canonical);
        assert_eq!(p.id, format!("vap_{}", hex16(&digest.finalize())));
    }

    #[test]
    fn bounded_closed_parser_accepts_only_the_supported_policy_shape() {
        let policy = exact_sidon_policy();
        let (policy_bytes, signature_bytes) = signed_policy_bytes(&policy, SIGNED_AT);
        let verified = verify_policy_signature_bytes(
            &policy_bytes,
            &signature_bytes,
            Some(&policy.id),
            "test policy",
        )
        .unwrap();
        assert_eq!(verified.policy, policy);

        let mut cases = Vec::new();
        let mut value = serde_json::to_value(&policy).unwrap();
        value["unexpected"] = json!(true);
        cases.push(value);
        let mut value = serde_json::to_value(&policy).unwrap();
        value["quorum"]["unexpected"] = json!(true);
        cases.push(value);
        let mut value = serde_json::to_value(&policy).unwrap();
        value["rules"][0]["unexpected"] = json!(true);
        cases.push(value);
        let mut value = serde_json::to_value(&policy).unwrap();
        value["rules"][0]["constraints"]["unexpected"] = json!(true);
        cases.push(value);

        for value in cases {
            let error = verify_policy_signature_bytes(
                &serde_json::to_vec(&value).unwrap(),
                &signature_bytes,
                None,
                "test policy",
            )
            .unwrap_err();
            assert!(error.contains("unknown field `unexpected`"), "{error}");
        }

        let mut missing_schema = serde_json::to_value(&policy).unwrap();
        missing_schema.as_object_mut().unwrap().remove("schema");
        let error = verify_policy_signature_bytes(
            &serde_json::to_vec(&missing_schema).unwrap(),
            &signature_bytes,
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("missing field `schema`"), "{error}");

        let mut unsupported_schema = serde_json::to_value(&policy).unwrap();
        unsupported_schema["schema"] = json!("vela.acceptance_policy.v999");
        let error = verify_policy_signature_bytes(
            &serde_json::to_vec(&unsupported_schema).unwrap(),
            &signature_bytes,
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("schema must be"), "{error}");

        let mut bad_expiry = serde_json::to_value(&policy).unwrap();
        bad_expiry["expires_at"] = json!("not-rfc3339");
        let error = verify_policy_signature_bytes(
            &serde_json::to_vec(&bad_expiry).unwrap(),
            &signature_bytes,
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("expires_at must be RFC3339"), "{error}");

        let mut signature_value: Value = serde_json::from_slice(&signature_bytes).unwrap();
        signature_value["unexpected"] = json!(true);
        let error = verify_policy_signature_bytes(
            &policy_bytes,
            &serde_json::to_vec(&signature_value).unwrap(),
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("unknown field `unexpected`"), "{error}");
    }

    #[test]
    fn duplicate_names_are_rejected_in_policy_and_signature_envelopes() {
        let policy = exact_sidon_policy();
        let (policy_bytes, signature_bytes) = signed_policy_bytes(&policy, SIGNED_AT);
        let policy_json = String::from_utf8(policy_bytes).unwrap();
        for malicious in [
            policy_json.replacen("\"epoch\": 1", "\"epoch\": 1,\n  \"epoch\": 2", 1),
            policy_json.replacen("\"epoch\": 1", "\"epoch\": 1,\n  \"e\\u0070och\": 2", 1),
            policy_json.replacen(
                "\"threshold\": 1",
                "\"threshold\": 1,\n    \"threshold\": 2",
                1,
            ),
        ] {
            let error = verify_policy_signature_bytes(
                malicious.as_bytes(),
                &signature_bytes,
                None,
                "test policy",
            )
            .unwrap_err();
            assert!(error.contains("duplicate object name"), "{error}");
        }

        let signature_json = String::from_utf8(signature_bytes).unwrap();
        let malicious = signature_json.replacen(
            &format!("\"signed_at\": \"{SIGNED_AT}\""),
            &format!("\"signed_at\": \"{SIGNED_AT}\",\n  \"signed_at\": \"2099-01-01T00:00:00Z\""),
            1,
        );
        let policy_bytes = serde_json::to_vec(&policy).unwrap();
        let error =
            verify_policy_signature_bytes(&policy_bytes, malicious.as_bytes(), None, "test policy")
                .unwrap_err();
        assert!(error.contains("duplicate object name"), "{error}");
    }

    #[test]
    fn parser_budgets_reject_oversized_and_excessively_deep_governance_json() {
        let policy = exact_sidon_policy();
        let (_, signature_bytes) = signed_policy_bytes(&policy, SIGNED_AT);
        let oversized = vec![b' '; POLICY_JSON_MAX_BYTES + 1];
        let error =
            verify_policy_signature_bytes(&oversized, &signature_bytes, None, "test policy")
                .unwrap_err();
        assert!(error.contains("limit is 65536 bytes"), "{error}");

        let deep = format!(
            "{}0{}",
            "{\"nested\":".repeat(GOVERNANCE_JSON_MAX_DEPTH + 1),
            "}".repeat(GOVERNANCE_JSON_MAX_DEPTH + 1)
        );
        let error =
            verify_policy_signature_bytes(deep.as_bytes(), &signature_bytes, None, "test policy")
                .unwrap_err();
        assert!(error.contains("JSON depth"), "{error}");

        let policy_bytes = serde_json::to_vec(&policy).unwrap();
        let oversized_signature = vec![b' '; POLICY_SIGNATURE_JSON_MAX_BYTES + 1];
        let error =
            verify_policy_signature_bytes(&policy_bytes, &oversized_signature, None, "test policy")
                .unwrap_err();
        assert!(error.contains("limit is 8192 bytes"), "{error}");
    }

    #[test]
    fn signatures_require_supported_policy_and_bound_signed_at() {
        let policy = exact_sidon_policy();
        let (policy_bytes, signature_bytes) = signed_policy_bytes(&policy, SIGNED_AT);

        let mut rewritten: PolicySignatureRecord =
            serde_json::from_slice(&signature_bytes).unwrap();
        rewritten.signed_at = "2026-06-22T00:00:01Z".to_string();
        let error = verify_policy_signature_bytes(
            &policy_bytes,
            &serde_json::to_vec(&rewritten).unwrap(),
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("signature does not verify"), "{error}");

        rewritten.signed_at = "not-rfc3339".to_string();
        let error = verify_policy_signature_bytes(
            &policy_bytes,
            &serde_json::to_vec(&rewritten).unwrap(),
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("signed_at must be RFC3339"), "{error}");
        assert!(policy_signature_preimage(&policy, "not-rfc3339").is_err());

        let key = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
        let unbound_signature = key.sign(&crate::canonical::to_canonical_bytes(&policy).unwrap());
        let unbound_record = PolicySignatureRecord {
            policy_id: policy.id.clone(),
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(unbound_signature.to_bytes()),
            signed_at: SIGNED_AT.to_string(),
        };
        let error = verify_policy_signature_bytes(
            &policy_bytes,
            &serde_json::to_vec(&unbound_record).unwrap(),
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("signature does not verify"), "{error}");

        let mut changed = policy.clone();
        changed.expires_at = "2098-12-31T23:59:59Z".to_string();
        changed.id = changed.content_address();
        let mut stale_record: PolicySignatureRecord =
            serde_json::from_slice(&signature_bytes).unwrap();
        stale_record.policy_id = changed.id.clone();
        let error = verify_policy_signature_bytes(
            &serde_json::to_vec(&changed).unwrap(),
            &serde_json::to_vec(&stale_record).unwrap(),
            None,
            "test policy",
        )
        .unwrap_err();
        assert!(error.contains("signature does not verify"), "{error}");
    }

    #[test]
    fn active_pair_accepts_only_current_id_and_bound_signature() {
        let policy = exact_sidon_policy();
        let (policy_bytes, signature_bytes) = signed_policy_bytes(&policy, SIGNED_AT);

        let mut wrong_id = policy.clone();
        wrong_id.id = "vap_00000000000000000000000000000000".to_string();
        let temp = write_active_pair(&serde_json::to_vec(&wrong_id).unwrap(), &signature_bytes);
        let error = load_active_policy_snapshot(temp.path())
            .expect_err("a non-current content address must be a strict error");
        assert!(error.contains("id does not re-derive"), "{error}");

        let key = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
        let unbound = key.sign(&crate::canonical::to_canonical_bytes(&policy).unwrap());
        let record = PolicySignatureRecord {
            policy_id: policy.id,
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(unbound.to_bytes()),
            signed_at: SIGNED_AT.to_string(),
        };
        let temp = write_active_pair(&policy_bytes, &serde_json::to_vec(&record).unwrap());
        let error = load_active_policy_snapshot(temp.path())
            .expect_err("an unbound signature must be a strict error");
        assert!(error.contains("signature does not verify"), "{error}");
    }

    #[test]
    fn malformed_or_stale_unsigned_policy_is_broken_not_staged() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join(".vela/policies");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("active.json"), b"{not-json\n").unwrap();
        let error = load_active_policy_snapshot(temp.path())
            .expect_err("malformed unsigned bytes must not become a staged policy");
        assert!(error.contains("active policy parse"), "{error}");

        let mut stale = exact_sidon_policy();
        stale.id = "vap_00000000000000000000000000000000".to_string();
        std::fs::write(
            directory.join("active.json"),
            serde_json::to_vec_pretty(&stale).unwrap(),
        )
        .unwrap();
        let error = load_active_policy_snapshot(temp.path())
            .expect_err("a stale unsigned id must not become a staged policy");
        assert!(error.contains("id does not re-derive"), "{error}");
    }

    #[test]
    fn active_snapshot_retains_the_policy_parsed_from_its_exact_bytes() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join(".vela/policies");
        std::fs::create_dir_all(&directory).unwrap();

        let original = exact_sidon_policy();
        let original_bytes = serde_json::to_vec_pretty(&original).unwrap();
        std::fs::write(directory.join("active.json"), &original_bytes).unwrap();
        let snapshot = load_active_policy_snapshot(temp.path()).unwrap();

        let mut replacement = original.clone();
        replacement.epoch += 1;
        replacement.id = replacement.content_address();
        std::fs::write(
            directory.join("active.json"),
            serde_json::to_vec_pretty(&replacement).unwrap(),
        )
        .unwrap();

        assert_eq!(
            snapshot.policy_bytes.as_deref(),
            Some(original_bytes.as_slice())
        );
        assert_eq!(
            snapshot.policy().map(|policy| policy.id.as_str()),
            Some(original.id.as_str())
        );
        assert_ne!(snapshot.policy().unwrap().id, replacement.id);
    }

    #[test]
    fn decision_certificate_binds_and_verifies() {
        let p = exact_sidon_policy();
        let d = evaluate(&p, &clean_exact_ctx(), NOW);
        let c = DecisionCertificate::build(
            &d,
            "vfr_test",
            "vpr_abc",
            "sha256:before",
            "sha256:after",
            AuthorityMode::PolicyDelegation,
            vec!["reviewer:will".into()],
            "service:vela-policy-engine",
            "exact_construction_dual_check_v1",
            3,
            "sha256:claim",
            2,
            true,
        );
        assert!(c.id.starts_with("vdc_"));
        assert!(c.id_is_valid());
        assert_eq!(c.outcome, Outcome::Permit);
        assert_eq!(c.authority_mode, AuthorityMode::PolicyDelegation);
        // The authority is the policy a human signed, NOT a per-item human click.
        assert_eq!(c.human_authorizers, vec!["reviewer:will"]);
        assert_eq!(c.executor, "service:vela-policy-engine");
        // Tamper detection.
        let mut bad = c.clone();
        bad.outcome = Outcome::Deny;
        assert!(!bad.id_is_valid());
    }
}

/// The signature envelope over a SEALED policy: the one governance act.
///
/// Signatures are Ed25519 over [`policy_signature_preimage`], which binds both
/// the sealed policy and `signed_at`. Every other signature preimage is rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySignatureRecord {
    pub policy_id: String,
    pub signer_pubkey_hex: String,
    pub signature: String,
    pub signed_at: String,
}

/// A policy that passed loading checks: sealed id re-derives and the
/// signature verifies over the sealed bytes. AUTHORITY still requires the
/// caller to check the signer is a registered human reviewer on the
/// frontier — the file system is not an actor table.
#[derive(Debug, Clone)]
pub struct VerifiedPolicy {
    pub policy: AcceptancePolicy,
    pub signer_pubkey_hex: String,
    pub signed_at: String,
}

/// Classification of the mutable active-policy pointer. Only `Active` carries
/// a [`VerifiedPolicy`]. Any other signed encoding is a strict load error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePolicyMode {
    Absent,
    StagedUnsigned,
    Active,
}

impl ActivePolicyMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::StagedUnsigned => "staged_unsigned",
            Self::Active => "active",
        }
    }
}

/// One read of both mutable active-policy paths while the caller owns its
/// frontier recovery barrier. The exact bytes (or exact absence) are retained
/// so callers can bind the same snapshot into a marker-time read set and copy
/// the same bytes into a content-addressed policy snapshot.
#[derive(Debug, Clone)]
pub struct ActivePolicySnapshot {
    pub policy_bytes: Option<Vec<u8>>,
    pub signature_bytes: Option<Vec<u8>>,
    pub verified: Option<VerifiedPolicy>,
    pub mode: ActivePolicyMode,
    policy: Option<AcceptancePolicy>,
}

impl ActivePolicySnapshot {
    /// Return the policy parsed from this exact byte snapshot. This never
    /// re-reads the mutable active-policy pointer.
    #[must_use]
    pub fn policy(&self) -> Option<&AcceptancePolicy> {
        self.policy.as_ref()
    }
}

// Governance files are deliberately much smaller than general repository
// objects. The byte ceiling is the first bound; the structural budgets prevent
// a compact adversarial document from spending unbounded parser work.
pub const POLICY_JSON_MAX_BYTES: usize = 64 * 1024;
pub const POLICY_SIGNATURE_JSON_MAX_BYTES: usize = 8 * 1024;
const GOVERNANCE_JSON_MAX_DEPTH: usize = 16;
const GOVERNANCE_JSON_MAX_NODES: usize = 4_096;
const GOVERNANCE_JSON_MAX_OBJECT_FIELDS: usize = 2_048;
const GOVERNANCE_JSON_MAX_ARRAY_ELEMENTS: usize = 2_048;
const GOVERNANCE_JSON_MAX_STRING_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy)]
struct GovernanceJsonLimits {
    bytes: usize,
}

#[derive(Debug)]
struct GovernanceJsonState {
    nodes: usize,
    object_fields: usize,
    array_elements: usize,
}

impl GovernanceJsonState {
    fn bump_node<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > GOVERNANCE_JSON_MAX_NODES {
            return Err(E::custom(format!(
                "{path}: JSON node budget exceeds {GOVERNANCE_JSON_MAX_NODES}"
            )));
        }
        Ok(())
    }

    fn bump_object_field<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.object_fields = self.object_fields.saturating_add(1);
        if self.object_fields > GOVERNANCE_JSON_MAX_OBJECT_FIELDS {
            return Err(E::custom(format!(
                "{path}: object-field budget exceeds {GOVERNANCE_JSON_MAX_OBJECT_FIELDS}"
            )));
        }
        Ok(())
    }

    fn bump_array_element<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.array_elements = self.array_elements.saturating_add(1);
        if self.array_elements > GOVERNANCE_JSON_MAX_ARRAY_ELEMENTS {
            return Err(E::custom(format!(
                "{path}: array-element budget exceeds {GOVERNANCE_JSON_MAX_ARRAY_ELEMENTS}"
            )));
        }
        Ok(())
    }

    fn check_depth<E: de::Error>(&self, path: &str, depth: usize) -> Result<(), E> {
        if depth > GOVERNANCE_JSON_MAX_DEPTH {
            return Err(E::custom(format!(
                "{path}: JSON depth is {depth}; limit is {GOVERNANCE_JSON_MAX_DEPTH}"
            )));
        }
        Ok(())
    }

    fn check_string<E: de::Error>(&self, path: &str, value: &str) -> Result<(), E> {
        if value.len() > GOVERNANCE_JSON_MAX_STRING_BYTES {
            return Err(E::custom(format!(
                "{path}: string is {} bytes; limit is {GOVERNANCE_JSON_MAX_STRING_BYTES} bytes",
                value.len()
            )));
        }
        Ok(())
    }
}

struct GovernanceValueSeed<'a> {
    state: &'a mut GovernanceJsonState,
    parent_depth: usize,
    path: String,
}

impl<'de> DeserializeSeed<'de> for GovernanceValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        self.state.bump_node::<D::Error>(&self.path)?;
        deserializer.deserialize_any(GovernanceValueVisitor {
            state: self.state,
            parent_depth: self.parent_depth,
            path: self.path,
        })
    }
}

struct GovernanceValueVisitor<'a> {
    state: &'a mut GovernanceJsonState,
    parent_depth: usize,
    path: String,
}

impl<'de> Visitor<'de> for GovernanceValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON without duplicate object names")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom(format!("{}: non-finite number", self.path)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string::<E>(&self.path, value)?;
        Ok(Value::String(value.to_string()))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string::<E>(&self.path, &value)?;
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth::<A::Error>(&self.path, depth)?;
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(256));
        let mut index = 0usize;
        loop {
            let child_path = format!("{}[{index}]", self.path);
            let seed = GovernanceValueSeed {
                state: self.state,
                parent_depth: depth,
                path: child_path.clone(),
            };
            let Some(value) = sequence.next_element_seed(seed)? else {
                break;
            };
            self.state.bump_array_element::<A::Error>(&child_path)?;
            values.push(value);
            index = index.saturating_add(1);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut entries: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth::<A::Error>(&self.path, depth)?;
        let mut value = Map::new();
        let mut names = HashSet::new();
        while let Some(key) = entries.next_key::<String>()? {
            self.state.check_string::<A::Error>(&self.path, &key)?;
            let child_path = format!("{}.{}", self.path, key);
            self.state.bump_object_field::<A::Error>(&child_path)?;
            if !names.insert(key.clone()) {
                return Err(<A::Error as de::Error>::custom(format!(
                    "{child_path}: duplicate object name `{key}`"
                )));
            }
            let item = entries.next_value_seed(GovernanceValueSeed {
                state: self.state,
                parent_depth: depth,
                path: child_path,
            })?;
            value.insert(key, item);
        }
        Ok(Value::Object(value))
    }
}

fn parse_bounded_governance_json<T: DeserializeOwned>(
    bytes: &[u8],
    limits: GovernanceJsonLimits,
    label: &str,
) -> Result<T, String> {
    if bytes.len() > limits.bytes {
        return Err(format!(
            "{label} parse: encoded JSON is {} bytes; limit is {} bytes",
            bytes.len(),
            limits.bytes
        ));
    }
    let mut state = GovernanceJsonState {
        nodes: 0,
        object_fields: 0,
        array_elements: 0,
    };
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = GovernanceValueSeed {
        state: &mut state,
        parent_depth: 0,
        path: "$".to_string(),
    }
    .deserialize(&mut deserializer)
    .map_err(|error| format!("{label} parse: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("{label} parse: {error}"))?;
    serde_json::from_value(value).map_err(|error| format!("{label} parse: {error}"))
}

fn validate_rfc3339(label: &str, value: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|error| format!("{label} must be RFC3339: {error}"))
}

fn validate_supported_policy(policy: &AcceptancePolicy, label: &str) -> Result<(), String> {
    if !matches!(
        policy.schema.as_str(),
        ACCEPTANCE_POLICY_V0_1_SCHEMA
            | ACCEPTANCE_POLICY_V0_2_SCHEMA
            | ACCEPTANCE_POLICY_V0_3_SCHEMA
    ) {
        return Err(format!(
            "{label} schema must be {ACCEPTANCE_POLICY_V0_1_SCHEMA}, {ACCEPTANCE_POLICY_V0_2_SCHEMA}, or {ACCEPTANCE_POLICY_V0_3_SCHEMA}, got {}",
            policy.schema
        ));
    }
    if let Some(reason) = binding_policy_error(policy) {
        return Err(format!("{label} binding constraints are invalid: {reason}"));
    }
    validate_rfc3339(&format!("{label} expires_at"), &policy.expires_at)
}

const POLICY_SIGNATURE_INPUT_SCHEMA: &str = "vela.policy-signature-input.v1";

#[derive(Serialize)]
struct PolicySignatureInput<'a> {
    schema: &'static str,
    policy: &'a AcceptancePolicy,
    signed_at: &'a str,
}

/// Domain-separated bytes signed by every newly issued policy signature.
/// Binding `signed_at` prevents an attacker from rewriting policy chronology
/// while retaining a valid signature over the policy body.
pub fn policy_signature_preimage(
    policy: &AcceptancePolicy,
    signed_at: &str,
) -> Result<Vec<u8>, String> {
    validate_supported_policy(policy, "policy signature input")?;
    validate_rfc3339("policy signed_at", signed_at)?;
    crate::canonical::to_canonical_bytes(&PolicySignatureInput {
        schema: POLICY_SIGNATURE_INPUT_SCHEMA,
        policy,
        signed_at,
    })
    .map_err(|error| format!("canonical policy signature input: {error}"))
}

/// Frontier-scoped authority resolved from a verified singular policy
/// signature. Actor ids, rather than public keys, are the durable human
/// authorizers carried into decision certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAuthority {
    pub human_authorizers: Vec<String>,
}

/// Resolve the human authority behind a cryptographically valid policy.
///
/// Loading first proves a closed, supported policy shape and a signature that
/// binds its timestamp. This frontier-scoped predicate then proves lifecycle
/// and actor authority for both live Permit and strict replay. The current
/// signature envelope contains one signature, so it can satisfy only a
/// one-person quorum whose eligible roles admit a reviewer or steward.
pub fn resolve_policy_authority(
    project: &crate::project::Project,
    verified: &VerifiedPolicy,
    decision_time: &str,
) -> Result<PolicyAuthority, String> {
    resolve_policy_authority_inner(project, verified, decision_time)
}

fn resolve_policy_authority_inner(
    project: &crate::project::Project,
    verified: &VerifiedPolicy,
    decision_time: &str,
) -> Result<PolicyAuthority, String> {
    validate_supported_policy(&verified.policy, "verified policy")?;
    if !verified.policy.id_is_valid() {
        return Err(format!(
            "verified policy {} id does not re-derive",
            verified.policy.id
        ));
    }
    validate_rfc3339("policy decision time", decision_time)?;
    if verified.policy.is_expired(decision_time) {
        return Err(format!(
            "policy {} is expired at decision time",
            verified.policy.id
        ));
    }
    if verified.policy.revocation_ref.is_some() {
        return Err(format!("policy {} is revoked", verified.policy.id));
    }
    let frontier_id = project
        .frontier_id
        .as_deref()
        .ok_or_else(|| "policy authority requires a frontier id".to_string())?;
    if verified.policy.frontier_id != frontier_id {
        return Err(format!(
            "policy frontier mismatch: policy {} targets {}, current frontier is {frontier_id}",
            verified.policy.id, verified.policy.frontier_id
        ));
    }

    if verified.policy.quorum.threshold != 1 {
        return Err(format!(
            "singular policy signature cannot satisfy quorum threshold {} (must be exactly 1)",
            verified.policy.quorum.threshold
        ));
    }
    let eligible_roles = verified
        .policy
        .quorum
        .eligible_roles
        .iter()
        .map(|role| role.trim().to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    if !eligible_roles
        .iter()
        .any(|role| matches!(role.as_str(), "reviewer" | "steward"))
    {
        return Err("policy quorum eligible_roles must admit a reviewer or steward".to_string());
    }

    let matching = project
        .actors
        .iter()
        .filter(|actor| {
            actor
                .public_key
                .eq_ignore_ascii_case(&verified.signer_pubkey_hex)
        })
        .collect::<Vec<_>>();
    let actor = match matching.as_slice() {
        [] => {
            return Err(format!(
                "policy signer pubkey {} is not a registered actor on this frontier",
                verified.signer_pubkey_hex
            ));
        }
        [actor] => *actor,
        _ => {
            return Err(format!(
                "policy signer pubkey {} resolves ambiguously to {} actors on this frontier",
                verified.signer_pubkey_hex,
                matching.len()
            ));
        }
    };
    let actor_role = actor
        .id
        .split_once(':')
        .map(|(role, _)| role.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let registered_human_role = matches!(actor_role.as_str(), "reviewer" | "steward")
        && !crate::proposals::is_placeholder_reviewer(&actor.id);
    if !registered_human_role {
        return Err(format!(
            "policy signer actor '{}' is not a registered reviewer or steward human",
            actor.id
        ));
    }
    if !eligible_roles.contains(&actor_role) {
        return Err(format!(
            "policy signer actor '{}' has registered role '{}', which is not in eligible_roles",
            actor.id, actor_role
        ));
    }
    if !verified
        .policy
        .issued_by
        .iter()
        .any(|issuer| issuer == &actor.id)
    {
        return Err(format!(
            "policy signer actor '{}' is not named in issued_by",
            actor.id
        ));
    }

    let parse_time = |label: &str, value: &str| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map_err(|error| format!("{label} must be RFC3339: {error}"))
    };
    let actor_created_at = parse_time("actor created_at", &actor.created_at)?;
    let signed_at = parse_time("policy signed_at", &verified.signed_at)?;
    let decision_at = parse_time("policy decision time", decision_time)?;
    if actor_created_at >= signed_at {
        return Err(format!(
            "policy signer actor '{}' was not registered before policy signing",
            actor.id
        ));
    }
    if signed_at > decision_at {
        return Err(format!(
            "policy {} was signed after the decision time",
            verified.policy.id
        ));
    }
    if let Some(revoked_at) = actor.revoked_at.as_deref() {
        let revoked_at = parse_time("actor revoked_at", revoked_at)?;
        if revoked_at <= signed_at {
            return Err(format!(
                "policy signer actor '{}' was revoked at policy signing",
                actor.id
            ));
        }
        if revoked_at <= decision_at {
            return Err(format!(
                "policy signer actor '{}' is revoked at decision time",
                actor.id
            ));
        }
    }

    Ok(PolicyAuthority {
        human_authorizers: vec![actor.id.clone()],
    })
}

/// Load `.vela/policies/active.json` + `active.sig.json` from a frontier
/// dir. Returns None when absent; Err on a present-but-broken pair (corrupt
/// governance must never fail open into the absent state).
pub fn load_active_policy(
    frontier_dir: &std::path::Path,
) -> Result<Option<VerifiedPolicy>, String> {
    Ok(load_active_policy_snapshot(frontier_dir)?.verified)
}

/// Content observation for a prelaunch policy pair that no longer parses as a
/// supported [`AcceptancePolicy`]. This is provenance for a human retirement
/// proposal, never policy authority: it deliberately does not rederive the
/// policy id, validate the current schema, or verify the historical signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyPairObservation {
    pub stored_policy_id: String,
    pub policy_bytes_root: String,
    pub signature_bytes_root: String,
}

/// Inspect exact legacy policy/signature bytes under the same structural and
/// byte budgets as current governance inputs. Duplicate object names are
/// rejected before the two stored ids are compared. A successful observation
/// says only which immutable bytes are present; it must never be interpreted as
/// a valid signature or an authorization to admit scientific state.
pub fn observe_legacy_policy_pair_bytes(
    policy_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<LegacyPolicyPairObservation, String> {
    let policy: Value = parse_bounded_governance_json(
        policy_bytes,
        GovernanceJsonLimits {
            bytes: POLICY_JSON_MAX_BYTES,
        },
        "legacy active policy",
    )?;
    let signature: Value = parse_bounded_governance_json(
        signature_bytes,
        GovernanceJsonLimits {
            bytes: POLICY_SIGNATURE_JSON_MAX_BYTES,
        },
        "legacy active policy signature",
    )?;
    let stored_policy_id = policy
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "legacy active policy must contain a string id".to_string())?;
    let signature_policy_id = signature
        .as_object()
        .and_then(|object| object.get("policy_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "legacy active policy signature must contain a string policy_id".to_string()
        })?;
    if !is_policy_id_shape(stored_policy_id) {
        return Err(
            "legacy active policy id must be vap_ followed by 32 lowercase hex characters"
                .to_string(),
        );
    }
    if signature_policy_id != stored_policy_id {
        return Err(
            "legacy active policy and signature name different stored policy ids".to_string(),
        );
    }
    Ok(LegacyPolicyPairObservation {
        stored_policy_id: stored_policy_id.to_string(),
        policy_bytes_root: format!("sha256:{}", hex::encode(Sha256::digest(policy_bytes))),
        signature_bytes_root: format!("sha256:{}", hex::encode(Sha256::digest(signature_bytes))),
    })
}

fn is_policy_id_shape(value: &str) -> bool {
    value.strip_prefix("vap_").is_some_and(|digest| {
        digest.len() == 32
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod legacy_pair_observation_tests {
    use super::*;

    #[test]
    fn observes_legacy_bytes_without_granting_current_policy_authority() {
        let id = "vap_e0abc750544408e637bd90e0661bac15";
        let policy =
            format!(r#"{{"schema":"vela.acceptance_policy.prelaunch","id":"{id}","legacy":true}}"#);
        let signature = format!(
            r#"{{"policy_id":"{id}","signer_pubkey_hex":"not-authority","signature":"historical","signed_at":"before-current-format"}}"#
        );
        let observed =
            observe_legacy_policy_pair_bytes(policy.as_bytes(), signature.as_bytes()).unwrap();
        assert_eq!(observed.stored_policy_id, id);
        assert_eq!(
            observed.policy_bytes_root,
            format!("sha256:{}", hex::encode(Sha256::digest(policy.as_bytes())))
        );
        assert!(
            verify_policy_signature_bytes(
                policy.as_bytes(),
                signature.as_bytes(),
                None,
                "legacy fixture"
            )
            .is_err(),
            "observation must not make unsupported legacy bytes authoritative"
        );
    }

    #[test]
    fn rejects_duplicate_names_and_mismatched_envelope_ids() {
        let id = "vap_e0abc750544408e637bd90e0661bac15";
        let duplicate = format!(r#"{{"id":"{id}","id":"{id}"}}"#);
        let signature = format!(r#"{{"policy_id":"{id}"}}"#);
        let error = observe_legacy_policy_pair_bytes(duplicate.as_bytes(), signature.as_bytes())
            .unwrap_err();
        assert!(error.contains("duplicate object name"), "{error}");

        let other = "vap_dbbc9caf67767317e42e217c65bab979";
        let policy = format!(r#"{{"id":"{id}"}}"#);
        let signature = format!(r#"{{"policy_id":"{other}"}}"#);
        let error =
            observe_legacy_policy_pair_bytes(policy.as_bytes(), signature.as_bytes()).unwrap_err();
        assert!(error.contains("different stored policy ids"), "{error}");
    }
}

/// Load the active policy paths exactly once, retaining the bytes that were
/// verified. A present policy without a signature is staged and human-only;
/// an orphan signature is corruption and never fails open.
pub fn load_active_policy_snapshot(
    frontier_dir: &std::path::Path,
) -> Result<ActivePolicySnapshot, String> {
    let dir = frontier_dir.join(".vela").join("policies");
    let policy_path = dir.join("active.json");
    let sig_path = dir.join("active.sig.json");
    let policy_bytes =
        read_optional_regular_file(&policy_path, "active policy", POLICY_JSON_MAX_BYTES)?;
    let signature_bytes = read_optional_regular_file(
        &sig_path,
        "active policy signature",
        POLICY_SIGNATURE_JSON_MAX_BYTES,
    )?;
    let (mode, policy, verified) = match (&policy_bytes, &signature_bytes) {
        (None, None) => (ActivePolicyMode::Absent, None, None),
        (Some(policy), None) => {
            let policy = parse_supported_policy_bytes(policy, "active policy")?;
            if !policy.id_is_valid() {
                return Err(format!(
                    "active policy id does not re-derive: stored {}, sealed {}",
                    policy.id,
                    policy.content_address()
                ));
            }
            (ActivePolicyMode::StagedUnsigned, Some(policy), None)
        }
        (None, Some(_)) => {
            return Err(
                "active policy signature exists without .vela/policies/active.json".to_string(),
            );
        }
        (Some(policy), Some(signature)) => {
            let verified = verify_policy_signature_bytes(policy, signature, None, "active policy")?;
            (
                ActivePolicyMode::Active,
                Some(verified.policy.clone()),
                Some(verified),
            )
        }
    };
    Ok(ActivePolicySnapshot {
        policy_bytes,
        signature_bytes,
        verified,
        mode,
        policy,
    })
}

fn parse_supported_policy_bytes(
    policy_bytes: &[u8],
    label: &str,
) -> Result<AcceptancePolicy, String> {
    let policy: AcceptancePolicy = parse_bounded_governance_json(
        policy_bytes,
        GovernanceJsonLimits {
            bytes: POLICY_JSON_MAX_BYTES,
        },
        label,
    )?;
    validate_supported_policy(&policy, label)?;
    Ok(policy)
}

fn read_optional_regular_file(
    path: &std::path::Path,
    label: &str,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            if metadata.len() > max_bytes as u64 {
                return Err(format!(
                    "{label} exceeds the {max_bytes}-byte input limit: {}",
                    path.display()
                ));
            }
            std::fs::read(path)
                .map(Some)
                .map_err(|error| format!("read {label} {}: {error}", path.display()))
        }
        Ok(_) => Err(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("inspect {label} {}: {error}", path.display())),
    }
}

/// Verify an exact policy/signature byte pair. `expected_policy_id` binds
/// content-addressed historical snapshots to their path name.
pub fn verify_policy_signature_bytes(
    policy_bytes: &[u8],
    signature_bytes: &[u8],
    expected_policy_id: Option<&str>,
    label: &str,
) -> Result<VerifiedPolicy, String> {
    use ed25519_dalek::Verifier;

    let policy: AcceptancePolicy = parse_bounded_governance_json(
        policy_bytes,
        GovernanceJsonLimits {
            bytes: POLICY_JSON_MAX_BYTES,
        },
        label,
    )?;
    validate_supported_policy(&policy, label)?;
    if expected_policy_id.is_some_and(|expected| expected != policy.id) || !policy.id_is_valid() {
        return Err(format!(
            "{label} id does not re-derive: stored {}, sealed {}",
            policy.id,
            policy.content_address()
        ));
    }
    let signature_label = format!("{label} signature");
    let sig: PolicySignatureRecord = parse_bounded_governance_json(
        signature_bytes,
        GovernanceJsonLimits {
            bytes: POLICY_SIGNATURE_JSON_MAX_BYTES,
        },
        &signature_label,
    )?;
    validate_rfc3339("policy signed_at", &sig.signed_at)?;
    if sig.policy_id != policy.id {
        return Err(format!("{label} signature is for a different policy id"));
    }
    let pk_bytes: [u8; 32] = hex::decode(&sig.signer_pubkey_hex)
        .map_err(|e| format!("pubkey hex: {e}"))?
        .try_into()
        .map_err(|_| "pubkey must be 32 bytes".to_string())?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes).map_err(|e| e.to_string())?;
    let sig_bytes: [u8; 64] = hex::decode(&sig.signature)
        .map_err(|e| format!("signature hex: {e}"))?
        .try_into()
        .map_err(|_| "signature must be 64 bytes".to_string())?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);
    let bound_body = policy_signature_preimage(&policy, &sig.signed_at)?;
    vk.verify(&bound_body, &signature)
        .map_err(|_| format!("{label} signature does not verify"))?;
    Ok(VerifiedPolicy {
        policy,
        signer_pubkey_hex: sig.signer_pubkey_hex,
        signed_at: sig.signed_at,
    })
}
