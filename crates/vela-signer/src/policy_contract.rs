//! Closed helper IPC for one protected policy-head decision.
//!
//! This is product plumbing, not a frontier protocol. The request carries the
//! exact policy, ephemeral policy-head proposal, and unsigned authority event
//! so the helper can derive every signing input it approves. It never accepts
//! caller-supplied opaque bytes.

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::acceptance_policy::AcceptancePolicy;
use vela_protocol::events::{EVENT_KIND_REVIEW_ACCEPTED, StateEvent};
use vela_protocol::proposals::StateProposal;
use vela_protocol::proposals::policy_accept::{
    POLICY_HEAD_PROPOSAL_KIND, PolicyHeadAction, PolicyHeadPayload,
};

use crate::contract::{
    ProtectionMode, SignerDisplay, file_sha256, validate_display, validate_hex_signature,
};

pub const POLICY_REQUEST_SCHEMA: &str = "vela.policy-signer-request.v1";
pub const POLICY_RESPONSE_SCHEMA: &str = "vela.policy-signer-response.v1";
const POLICY_REQUEST_DOMAIN: &[u8] = b"vela.policy-signer-request.v1\0";
const MAX_REQUEST_LIFETIME_SECONDS: i64 = 120;
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecisionAction {
    Activate,
    Rotate,
    Revoke,
}

impl PolicyDecisionAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activate => "activate",
            Self::Rotate => "rotate",
            Self::Revoke => "revoke",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySignerRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub action: PolicyDecisionAction,
    pub selected_policy_id: String,
    pub selected_policy_root: String,
    pub reason: String,
    pub reviewer_actor: String,
    pub reviewer_public_key: String,
    pub observed_at: String,
    pub decision_plan_root: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub display: SignerDisplay,
    /// Exact selected policy for every action. Revoke does not sign a new
    /// envelope, but still binds and displays the authority it closes.
    pub policy: AcceptancePolicy,
    pub proposal: StateProposal,
    pub event: StateEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySignerResponse {
    pub schema: String,
    pub request_root: String,
    pub reviewer_public_key: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub provider: String,
    pub protection_grade: String,
    pub provider_session: String,
    pub approved_at: String,
    pub protection_mode: ProtectionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_signature: Option<String>,
    pub event_id: String,
    pub event_signature: String,
}

pub fn policy_request_root(request: &PolicySignerRequest) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize policy signer request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(POLICY_REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn validate_policy_request(
    request: &PolicySignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != POLICY_REQUEST_SCHEMA {
        return Err(format!(
            "policy signer request schema must be {POLICY_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    require_sha256("selected_policy_root", &request.selected_policy_root)?;
    require_sha256("decision_plan_root", &request.decision_plan_root)?;
    require_lower_hex("reviewer_public_key", &request.reviewer_public_key, 64)?;
    if !request.frontier_id.starts_with("vfr_") {
        return Err("frontier_id must start with vfr_".to_string());
    }
    if !request.selected_policy_id.starts_with("vap_") {
        return Err("selected_policy_id must start with vap_".to_string());
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_path", request.frontier_path.as_str()),
        ("reason", request.reason.as_str()),
        ("reviewer_actor", request.reviewer_actor.as_str()),
        ("provider", request.provider.as_str()),
        ("protection_grade", request.protection_grade.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    validate_display(&request.display)?;
    validate_window(&request.expires_at, now)?;
    let observed_at = DateTime::parse_from_rfc3339(&request.observed_at)
        .map_err(|error| format!("observed_at is not RFC3339: {error}"))?;

    if request.proposal.id != vela_protocol::proposals::proposal_id(&request.proposal) {
        return Err("policy-head proposal id does not rederive".to_string());
    }
    if request.proposal.kind != POLICY_HEAD_PROPOSAL_KIND
        || request.proposal.status != "pending_review"
        || request.proposal.actor.id != request.reviewer_actor
    {
        return Err("policy-head proposal shape does not match the requester".to_string());
    }
    vela_protocol::canonical::sha256_canonical(&request.proposal)
        .map_err(|error| format!("canonicalize policy-head proposal: {error}"))?;
    let payload: PolicyHeadPayload = serde_json::from_value(request.proposal.payload.clone())
        .map_err(|error| format!("decode policy-head payload: {error}"))?;
    let expected_head_action = match request.action {
        PolicyDecisionAction::Activate => PolicyHeadAction::Activate,
        PolicyDecisionAction::Rotate => PolicyHeadAction::Rotate,
        PolicyDecisionAction::Revoke => PolicyHeadAction::Revoke,
    };
    if payload.action != expected_head_action {
        return Err("policy-head payload action does not match the request".to_string());
    }
    let policy = &request.policy;
    if policy.id != request.selected_policy_id
        || !policy.id_is_valid()
        || policy.frontier_id != request.frontier_id
    {
        return Err("selected policy identity does not match the request".to_string());
    }
    let policy_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(policy)
            .map_err(|error| format!("canonicalize selected policy: {error}"))?
    );
    if policy_root != request.selected_policy_root {
        return Err("selected policy root does not match the request".to_string());
    }
    match request.action {
        PolicyDecisionAction::Activate | PolicyDecisionAction::Rotate => {
            if payload.policy_id.as_deref() != Some(request.selected_policy_id.as_str()) {
                return Err("policy-head payload names a different policy".to_string());
            }
            vela_protocol::acceptance_policy::policy_signature_preimage(
                policy,
                &request.observed_at,
            )?;
        }
        PolicyDecisionAction::Revoke => {
            if payload.policy_id.is_some() {
                return Err("policy revocation must not carry a replacement policy".to_string());
            }
        }
    }

    let event = &request.event;
    if event.signature.is_some() || vela_protocol::events::compute_event_id(event) != event.id {
        return Err("policy-head authority event must be unsigned with a current id".to_string());
    }
    if event.kind != EVENT_KIND_REVIEW_ACCEPTED
        || event.actor.id != request.reviewer_actor
        || event.reason != request.reason
        || event.timestamp != request.observed_at
        || event.target.r#type != "proposal"
        || event.target.id != request.proposal.id
    {
        return Err("policy-head authority event does not match the request".to_string());
    }
    if event
        .payload
        .get("proposal_id")
        .and_then(serde_json::Value::as_str)
        != Some(request.proposal.id.as_str())
        || event
            .payload
            .get("proposal_kind")
            .and_then(serde_json::Value::as_str)
            != Some(POLICY_HEAD_PROPOSAL_KIND)
        || event
            .payload
            .get("verdict")
            .and_then(serde_json::Value::as_str)
            != Some("accepted")
    {
        return Err("policy-head authority event payload is inconsistent".to_string());
    }
    if request.proposal.created_at != request.observed_at
        || observed_at
            > request
                .expires_at
                .parse::<DateTime<Utc>>()
                .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
    {
        return Err("policy-head proposal time is outside the approval request".to_string());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match the request".to_string());
    }
    Ok(())
}

pub fn validate_policy_request_fresh(
    request: &PolicySignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expiry {
        return Err("policy signer request expired before approval completed".to_string());
    }
    Ok(())
}

pub fn validate_policy_response(
    request: &PolicySignerRequest,
    response: &PolicySignerResponse,
) -> Result<(), String> {
    if response.schema != POLICY_RESPONSE_SCHEMA
        || response.request_root != policy_request_root(request)?
        || response.reviewer_public_key != request.reviewer_public_key
        || response.helper_sha256 != request.helper_sha256
        || response.provider != request.provider
        || response.protection_grade != request.protection_grade
        || response.protection_mode != request.protection_mode
        || response.provider_session.trim().is_empty()
        || response.event_id != request.event.id
    {
        return Err("policy signer response does not match the exact request".to_string());
    }
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    validate_policy_request_fresh(request, approved_at)?;
    let public = hex::decode(&response.reviewer_public_key)
        .map_err(|error| format!("invalid response public key: {error}"))?;
    let verifying = VerifyingKey::from_bytes(
        &public
            .try_into()
            .map_err(|_| "response public key must be 32 bytes".to_string())?,
    )
    .map_err(|error| format!("invalid response public key: {error}"))?;
    let event_signature = validate_hex_signature("event_signature", &response.event_signature)?;
    let event_bytes = vela_protocol::sign::event_signing_bytes(
        &request.event,
        vela_protocol::signing_input::SigVersion::V1,
    )?;
    verifying
        .verify(&event_bytes, &event_signature)
        .map_err(|_| "policy-head event signature does not verify".to_string())?;
    match (request.action, &response.policy_signature) {
        (PolicyDecisionAction::Activate | PolicyDecisionAction::Rotate, Some(signature)) => {
            let signature = validate_hex_signature("policy_signature", signature)?;
            let bytes = vela_protocol::acceptance_policy::policy_signature_preimage(
                &request.policy,
                &request.observed_at,
            )?;
            verifying
                .verify(&bytes, &signature)
                .map_err(|_| "policy envelope signature does not verify".to_string())?;
        }
        (PolicyDecisionAction::Revoke, None) => {}
        _ => {
            return Err(
                "policy signer response has the wrong envelope signature shape".to_string(),
            );
        }
    }
    Ok(())
}

fn validate_window(expires_at: &str, now: DateTime<Utc>) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("policy signer request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("policy signer expiry exceeds two minutes".to_string());
    }
    Ok(())
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use sha256:<64 lowercase hex>"));
    };
    require_lower_hex(name, hex, 64)
}

fn require_lower_hex(name: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} must be {length} lowercase hex characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use vela_protocol::acceptance_policy::{Outcome, Quorum};
    use vela_protocol::events::StateTarget;

    fn fixture() -> (tempfile::NamedTempFile, SigningKey, PolicySignerRequest) {
        use std::io::Write;
        let mut binary = tempfile::NamedTempFile::new().unwrap();
        binary.write_all(b"vela-test-binary").unwrap();
        let key = SigningKey::from_bytes(&[9_u8; 32]);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_policy_fixture".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["reviewer".to_string()],
            },
            rules: Vec::new(),
            default: Outcome::Defer,
            expires_at: "2100-01-01T00:00:00Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();
        let at = "2026-07-18T12:00:00Z";
        let payload = PolicyHeadPayload {
            schema: "vela.policy-head.v1".to_string(),
            action: PolicyHeadAction::Activate,
            policy_id: Some(policy.id.clone()),
            prior_head_event_id: None,
            expected_parent_event_log_root: format!("sha256:{}", "0".repeat(64)),
            parent_event_ids: Vec::new(),
            epoch: 1,
        };
        let proposal = vela_protocol::proposals::new_proposal_at(
            POLICY_HEAD_PROPOSAL_KIND,
            StateTarget {
                r#type: "governance".to_string(),
                id: policy.frontier_id.clone(),
            },
            "reviewer:test",
            "human",
            "activate bounded policy",
            serde_json::to_value(payload).unwrap(),
            Vec::new(),
            Vec::new(),
            at,
        );
        let event = vela_protocol::events::new_review_decision_event(
            &proposal.id,
            POLICY_HEAD_PROPOSAL_KIND,
            "accepted",
            None,
            "reviewer:test",
            "activate bounded policy",
            Some(at),
        )
        .unwrap();
        let request = PolicySignerRequest {
            schema: POLICY_REQUEST_SCHEMA.to_string(),
            nonce: "1".repeat(64),
            expires_at: "2026-07-18T12:02:00Z".to_string(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: format!("sha256:{}", "2".repeat(64)),
            frontier_id: policy.frontier_id.clone(),
            frontier_path: "/tmp/frontier".to_string(),
            action: PolicyDecisionAction::Activate,
            selected_policy_id: policy.id.clone(),
            selected_policy_root: format!(
                "sha256:{}",
                vela_protocol::canonical::sha256_canonical(&policy).unwrap()
            ),
            reason: "activate bounded policy".to_string(),
            reviewer_actor: "reviewer:test".to_string(),
            reviewer_public_key: public_key,
            observed_at: at.to_string(),
            decision_plan_root: format!("sha256:{}", "3".repeat(64)),
            provider: "test_store".to_string(),
            protection_grade: "test".to_string(),
            protection_mode: ProtectionMode::Session,
            display: SignerDisplay {
                frontier_name: "fixture".to_string(),
                claim: "activate exact policy".to_string(),
                requester: "reviewer:test".to_string(),
                decisive_facts: vec!["default defer".to_string()],
                consequence: "bounded receipts may permit".to_string(),
            },
            policy,
            proposal,
            event,
        };
        (binary, key, request)
    }

    #[test]
    fn exact_policy_request_and_response_verify() {
        let (_binary, key, request) = fixture();
        validate_policy_request(&request, "2026-07-18T12:00:30Z".parse().unwrap()).unwrap();
        let policy_signature = hex::encode(
            key.sign(
                &vela_protocol::acceptance_policy::policy_signature_preimage(
                    &request.policy,
                    &request.observed_at,
                )
                .unwrap(),
            )
            .to_bytes(),
        );
        let response = PolicySignerResponse {
            schema: POLICY_RESPONSE_SCHEMA.to_string(),
            request_root: policy_request_root(&request).unwrap(),
            reviewer_public_key: request.reviewer_public_key.clone(),
            helper_version: "test".to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            provider_session: "session".to_string(),
            approved_at: "2026-07-18T12:00:45Z".to_string(),
            protection_mode: request.protection_mode,
            policy_signature: Some(policy_signature),
            event_id: request.event.id.clone(),
            event_signature: vela_protocol::sign::sign_event(&request.event, &key).unwrap(),
        };
        validate_policy_response(&request, &response).unwrap();
    }

    #[test]
    fn policy_action_target_and_event_drift_fail_closed() {
        let (_binary, _key, request) = fixture();
        let mut action = request.clone();
        action.action = PolicyDecisionAction::Rotate;
        assert!(validate_policy_request(&action, "2026-07-18T12:00:30Z".parse().unwrap()).is_err());

        let mut target = request.clone();
        target.selected_policy_id = "vap_different".to_string();
        assert!(validate_policy_request(&target, "2026-07-18T12:00:30Z".parse().unwrap()).is_err());

        let mut event = request;
        event.event.reason = "different reason".to_string();
        event.event.id = vela_protocol::events::compute_event_id(&event.event);
        assert!(validate_policy_request(&event, "2026-07-18T12:00:30Z".parse().unwrap()).is_err());
    }
}
