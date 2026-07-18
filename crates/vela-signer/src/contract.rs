use std::collections::BTreeSet;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::events::{EVENT_KIND_REVIEW_ACCEPTED, EVENT_KIND_REVIEW_REJECTED, StateEvent};

pub const REQUEST_SCHEMA: &str = "vela.signer-request.v1";
pub const RESPONSE_SCHEMA: &str = "vela.signer-response.v1";
pub const ENROLLMENT_REQUEST_SCHEMA: &str = "vela.signer-enrollment-request.v1";
pub const ENROLLMENT_RESPONSE_SCHEMA: &str = "vela.signer-enrollment-response.v1";
pub const REBIND_REQUEST_SCHEMA: &str = "vela.signer-rebind-request.v1";
pub const REBIND_RESPONSE_SCHEMA: &str = "vela.signer-rebind-response.v1";
const REQUEST_DOMAIN: &[u8] = b"vela.signer-request.v1\0";
const REBIND_REQUEST_DOMAIN: &[u8] = b"vela.signer-rebind-request.v1\0";
const REBIND_AUTHORIZATION_DOMAIN: &[u8] = b"vela.signer-rebind-authorization.v1\0";
const MAX_REQUEST_LIFETIME_SECONDS: i64 = 120;
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionMode {
    Session,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebindPurpose {
    Upgrade,
    EnrollmentRecovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerEvent {
    pub event: StateEvent,
}

/// Bounded, plain-language material for the human decision card.
///
/// These fields are presentation, not a second authority model. They are
/// derived from the same Decision Brief as the signed request and included in
/// the request root so the helper cannot display one decision and sign
/// another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerDisplay {
    pub frontier_name: String,
    pub claim: String,
    pub requester: String,
    pub decisive_facts: Vec<String>,
    pub consequence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub proposal_id: String,
    pub proposal_root: String,
    pub action: String,
    pub reason: String,
    pub reviewer_actor: String,
    pub reviewer_public_key: String,
    pub observed_at: String,
    pub decision_plan_root: String,
    pub gate_state: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub display: SignerDisplay,
    pub events: Vec<SignerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSignature {
    pub event_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerResponse {
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
    pub signatures: Vec<EventSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub actor: String,
    pub public_key: String,
    pub source_path: String,
    pub provider: String,
    pub protection_mode: ProtectionMode,
    pub remove_source_after_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentResponse {
    pub schema: String,
    pub nonce: String,
    pub helper_version: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub actor: String,
    pub public_key: String,
    pub key_id: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub installed_at: String,
    pub source_removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RebindRequest {
    pub schema: String,
    pub purpose: RebindPurpose,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub previous_vela_binary_sha256: String,
    pub helper_sha256: String,
    pub previous_helper_sha256: String,
    pub actor: String,
    pub public_key: String,
    pub provider: String,
    pub previous_protection_mode: ProtectionMode,
    pub protection_mode: ProtectionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RebindResponse {
    pub schema: String,
    pub request_root: String,
    pub actor: String,
    pub public_key: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub rebound_at: String,
    pub signature: String,
}

pub fn validate_enrollment_request(
    request: &EnrollmentRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != ENROLLMENT_REQUEST_SCHEMA {
        return Err(format!(
            "signer enrollment schema must be {ENROLLMENT_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_lower_hex("public_key", &request.public_key, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    for (name, value) in [
        ("actor", request.actor.as_str()),
        ("source_path", request.source_path.as_str()),
        ("provider", request.provider.as_str()),
        ("vela_binary_path", request.vela_binary_path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    if !request.remove_source_after_install {
        return Err("protected enrollment requires explicit source removal".to_string());
    }
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("signer enrollment request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("signer enrollment expiry exceeds two minutes".to_string());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match enrollment".to_string());
    }
    let source = Path::new(&request.source_path);
    if !source.is_file() {
        return Err(format!(
            "plaintext source key is missing at {}",
            source.display()
        ));
    }
    let metadata =
        std::fs::metadata(source).map_err(|error| format!("inspect source key: {error}"))?;
    if metadata.len() > 129 {
        return Err("plaintext source key is unexpectedly large".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            return Err("plaintext source key must not be group/world accessible".to_string());
        }
        let owner = rustix::process::geteuid().as_raw();
        if metadata.uid() != owner {
            return Err("plaintext source key must be owned by the current user".to_string());
        }
    }
    Ok(())
}

pub fn validate_enrollment_fresh_for_install(
    request: &EnrollmentRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expiry {
        return Err("signer enrollment expired before authentication completed".to_string());
    }
    Ok(())
}

pub fn rebind_request_root(request: &RebindRequest) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize signer rebind request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(REBIND_REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn rebind_response_signing_bytes(response: &RebindResponse) -> Result<Vec<u8>, String> {
    let mut unsigned = response.clone();
    unsigned.signature.clear();
    let canonical = vela_protocol::canonical::to_canonical_bytes(&unsigned)
        .map_err(|error| format!("canonicalize signer rebind response: {error}"))?;
    let mut bytes = REBIND_AUTHORIZATION_DOMAIN.to_vec();
    bytes.extend(canonical);
    Ok(bytes)
}

pub fn validate_rebind_request(request: &RebindRequest, now: DateTime<Utc>) -> Result<(), String> {
    if request.schema != REBIND_REQUEST_SCHEMA {
        return Err(format!(
            "signer rebind schema must be {REBIND_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_lower_hex("public_key", &request.public_key, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256(
        "previous_vela_binary_sha256",
        &request.previous_vela_binary_sha256,
    )?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    require_sha256("previous_helper_sha256", &request.previous_helper_sha256)?;
    let unchanged = request.previous_vela_binary_sha256 == request.vela_binary_sha256
        && request.previous_helper_sha256 == request.helper_sha256
        && request.previous_protection_mode == request.protection_mode;
    if request.purpose == RebindPurpose::Upgrade && unchanged {
        return Err(
            "signer rebind requires a changed Vela binary, helper digest, or protection mode"
                .to_string(),
        );
    }
    if request.purpose == RebindPurpose::EnrollmentRecovery && !unchanged {
        return Err(
            "enrollment recovery cannot also change the Vela binary, helper, or protection mode"
                .to_string(),
        );
    }
    for (name, value) in [
        ("actor", request.actor.as_str()),
        ("provider", request.provider.as_str()),
        ("vela_binary_path", request.vela_binary_path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("signer rebind request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("signer rebind expiry exceeds two minutes".to_string());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match rebind request".to_string());
    }
    Ok(())
}

pub fn validate_rebind_fresh(request: &RebindRequest, now: DateTime<Utc>) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expiry {
        return Err("signer rebind expired before authentication completed".to_string());
    }
    Ok(())
}

pub fn validate_rebind_response(
    request: &RebindRequest,
    response: &RebindResponse,
) -> Result<(), String> {
    if response.schema != REBIND_RESPONSE_SCHEMA
        || response.request_root != rebind_request_root(request)?
        || response.actor != request.actor
        || response.public_key != request.public_key
        || response.helper_sha256 != request.helper_sha256
        || response.provider != request.provider
        || response.protection_mode != request.protection_mode
        || response.protection_grade.trim().is_empty()
    {
        return Err("signer rebind response does not match the exact request".to_string());
    }
    let rebound_at = DateTime::parse_from_rfc3339(&response.rebound_at)
        .map_err(|error| format!("rebound_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if rebound_at > expires_at {
        return Err("signer rebind response was authorized after expiry".to_string());
    }
    let public = hex::decode(&response.public_key)
        .map_err(|error| format!("invalid rebind public key: {error}"))?;
    let verifying = VerifyingKey::from_bytes(
        &public
            .try_into()
            .map_err(|_| "rebind public key must be 32 bytes".to_string())?,
    )
    .map_err(|error| format!("invalid rebind public key: {error}"))?;
    let raw = response
        .signature
        .strip_prefix("v1:")
        .ok_or_else(|| "rebind response must use a v1 signature".to_string())?;
    let bytes = hex::decode(raw).map_err(|error| format!("invalid rebind signature: {error}"))?;
    let signature = Signature::from_bytes(
        &bytes
            .try_into()
            .map_err(|_| "rebind signature must be 64 bytes".to_string())?,
    );
    verifying
        .verify(&rebind_response_signing_bytes(response)?, &signature)
        .map_err(|_| "signer rebind signature does not verify".to_string())
}

pub fn request_root(request: &SignerRequest) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize signer request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read pinned binary {}: {error}", path.display()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

pub fn validate_request(request: &SignerRequest, now: DateTime<Utc>) -> Result<(), String> {
    if request.schema != REQUEST_SCHEMA {
        return Err(format!("signer request schema must be {REQUEST_SCHEMA}"));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    require_sha256("proposal_root", &request.proposal_root)?;
    require_sha256("decision_plan_root", &request.decision_plan_root)?;
    require_lower_hex("reviewer_public_key", &request.reviewer_public_key, 64)?;
    if !request.frontier_id.starts_with("vfr_") {
        return Err("frontier_id must start with vfr_".to_string());
    }
    if !request.proposal_id.starts_with("vpr_") {
        return Err("proposal_id must start with vpr_".to_string());
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_path", request.frontier_path.as_str()),
        ("reason", request.reason.as_str()),
        ("reviewer_actor", request.reviewer_actor.as_str()),
        ("gate_state", request.gate_state.as_str()),
        ("provider", request.provider.as_str()),
        ("protection_grade", request.protection_grade.as_str()),
        ("frontier_name", request.display.frontier_name.as_str()),
        ("claim", request.display.claim.as_str()),
        ("requester", request.display.requester.as_str()),
        ("consequence", request.display.consequence.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    validate_display(&request.display)?;
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("signer request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("signer request expiry exceeds two minutes".to_string());
    }
    DateTime::parse_from_rfc3339(&request.observed_at)
        .map_err(|error| format!("observed_at is not RFC3339: {error}"))?;

    let expected_review_kind = match request.action.as_str() {
        "accept" => EVENT_KIND_REVIEW_ACCEPTED,
        "reject" => EVENT_KIND_REVIEW_REJECTED,
        _ => return Err("action must be accept or reject".to_string()),
    };
    if request.events.is_empty() {
        return Err("signer request must contain at least one event".to_string());
    }
    let decision_reference =
        vela_protocol::provenance::decision_root_input_ref(&request.decision_plan_root)?;
    let mut event_ids = BTreeSet::new();
    let mut matching_review_events = 0_usize;
    for item in &request.events {
        let event = &item.event;
        if event.signature.is_some() {
            return Err(format!("event {} is already signed", event.id));
        }
        if vela_protocol::events::compute_event_id(event) != event.id {
            return Err(format!("event {} has a stale content id", event.id));
        }
        if !event_ids.insert(event.id.clone()) {
            return Err(format!("event {} appears more than once", event.id));
        }
        if event.actor.id != request.reviewer_actor {
            return Err(format!("event {} has a different actor", event.id));
        }
        if event.reason != request.reason {
            return Err(format!("event {} has a different reason", event.id));
        }
        if event.kind == expected_review_kind {
            matching_review_events += 1;
            if event.target.id != request.proposal_id {
                return Err("review event target does not match proposal_id".to_string());
            }
            let payload_proposal = event
                .payload
                .get("proposal_id")
                .and_then(serde_json::Value::as_str);
            if payload_proposal != Some(request.proposal_id.as_str()) {
                return Err("review payload proposal_id does not match request".to_string());
            }
            let verdict = event
                .payload
                .get("verdict")
                .and_then(serde_json::Value::as_str);
            let expected_verdict = if request.action == "accept" {
                "accepted"
            } else {
                "rejected"
            };
            if verdict != Some(expected_verdict) {
                return Err("review payload verdict does not match action".to_string());
            }
            let has_decision_root = event
                .payload
                .get("provenance")
                .and_then(|value| value.get("input_refs"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|refs| {
                    refs.iter()
                        .any(|value| value.as_str() == Some(&decision_reference))
                });
            if !has_decision_root {
                return Err("review event does not bind the Decision Plan root".to_string());
            }
        }
    }
    if matching_review_events != 1 {
        return Err(format!(
            "request must contain exactly one {expected_review_kind} event"
        ));
    }

    let vela_digest = file_sha256(Path::new(&request.vela_binary_path))?;
    if vela_digest != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match the request".to_string());
    }
    Ok(())
}

pub(crate) fn validate_display(display: &SignerDisplay) -> Result<(), String> {
    const MAX_FIELD_BYTES: usize = 512;
    const MAX_FACTS: usize = 4;
    if display.decisive_facts.is_empty() || display.decisive_facts.len() > MAX_FACTS {
        return Err(format!(
            "decision display must contain between one and {MAX_FACTS} decisive facts"
        ));
    }
    for (name, value) in [
        ("frontier_name", display.frontier_name.as_str()),
        ("claim", display.claim.as_str()),
        ("requester", display.requester.as_str()),
        ("consequence", display.consequence.as_str()),
    ] {
        validate_display_text(name, value, MAX_FIELD_BYTES)?;
    }
    for fact in &display.decisive_facts {
        validate_display_text("decisive_fact", fact, MAX_FIELD_BYTES)?;
    }
    Ok(())
}

pub(crate) fn validate_hex_signature(name: &str, value: &str) -> Result<Signature, String> {
    let raw = value.strip_prefix("v1:").unwrap_or(value);
    let bytes = hex::decode(raw).map_err(|error| format!("invalid {name} hex: {error}"))?;
    Ok(Signature::from_bytes(
        &bytes
            .try_into()
            .map_err(|_| format!("{name} must be 64 bytes"))?,
    ))
}

fn validate_display_text(name: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > maximum {
        return Err(format!("{name} exceeds {maximum} bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{name} must not contain control characters"));
    }
    Ok(())
}

pub fn validate_request_fresh_for_signing(
    request: &SignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expiry {
        return Err("signer request expired before approval completed".to_string());
    }
    Ok(())
}

pub fn validate_response(request: &SignerRequest, response: &SignerResponse) -> Result<(), String> {
    if response.schema != RESPONSE_SCHEMA {
        return Err(format!("signer response schema must be {RESPONSE_SCHEMA}"));
    }
    if response.request_root != request_root(request)? {
        return Err("signer response request_root mismatch".to_string());
    }
    if response.reviewer_public_key != request.reviewer_public_key {
        return Err("signer response public key mismatch".to_string());
    }
    if response.helper_sha256 != request.helper_sha256 {
        return Err("signer response helper digest mismatch".to_string());
    }
    if response.provider != request.provider
        || response.protection_grade != request.protection_grade
        || response.protection_mode != request.protection_mode
    {
        return Err("signer response provider or protection mode mismatch".to_string());
    }
    if response.provider_session.trim().is_empty() {
        return Err("signer response provider_session must not be empty".to_string());
    }
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if approved_at > expires_at {
        return Err("signer response was approved after request expiry".to_string());
    }
    if response.signatures.len() != request.events.len() {
        return Err("signer response signature count mismatch".to_string());
    }
    let pubkey_bytes = hex::decode(&response.reviewer_public_key)
        .map_err(|error| format!("invalid response public key: {error}"))?;
    let verifying_key = VerifyingKey::from_bytes(
        &pubkey_bytes
            .try_into()
            .map_err(|_| "response public key must be 32 bytes".to_string())?,
    )
    .map_err(|error| format!("invalid response public key: {error}"))?;
    for (event, signed) in request.events.iter().zip(&response.signatures) {
        if signed.event_id != event.event.id {
            return Err("signer response event order or id mismatch".to_string());
        }
        let raw = signed
            .signature
            .strip_prefix("v1:")
            .ok_or_else(|| "signer response must use v1 event signatures".to_string())?;
        let bytes = hex::decode(raw).map_err(|error| format!("invalid signature hex: {error}"))?;
        let signature = Signature::from_bytes(
            &bytes
                .try_into()
                .map_err(|_| "event signature must be 64 bytes".to_string())?,
        );
        let preimage = vela_protocol::sign::event_signing_bytes(
            &event.event,
            vela_protocol::signing_input::SigVersion::V1,
        )?;
        verifying_key
            .verify(&preimage, &signature)
            .map_err(|_| format!("signature for {} does not verify", event.event.id))?;
    }
    Ok(())
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must be sha256:<64 lowercase hex>"));
    };
    require_lower_hex(name, digest, 64)
}

fn require_lower_hex(name: &str, value: &str, len: usize) -> Result<(), String> {
    if value.len() != len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} must be {len} lowercase hex characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> (tempfile::NamedTempFile, SignerRequest) {
        let mut binary = tempfile::NamedTempFile::new().unwrap();
        binary.write_all(b"pinned vela fixture").unwrap();
        let decision_root = format!("sha256:{}", "4".repeat(64));
        let mut event = vela_protocol::events::new_review_decision_event(
            "vpr_fixture",
            "finding.add",
            "rejected",
            None,
            "reviewer:fixture",
            "insufficient independent evidence",
            Some("2026-07-17T12:00:00Z"),
        )
        .unwrap();
        let mut provenance = vela_protocol::provenance::Provenance::default();
        provenance.bind_decision_root(&decision_root).unwrap();
        vela_protocol::provenance::attach_to_payload(&mut event.payload, &provenance).unwrap();
        event.id = vela_protocol::events::compute_event_id(&event);
        let request = SignerRequest {
            schema: REQUEST_SCHEMA.to_string(),
            nonce: "1".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: format!("sha256:{}", "2".repeat(64)),
            frontier_id: "vfr_fixture".to_string(),
            frontier_path: "/tmp/frontier".to_string(),
            proposal_id: "vpr_fixture".to_string(),
            proposal_root: format!("sha256:{}", "3".repeat(64)),
            action: "reject".to_string(),
            reason: "insufficient independent evidence".to_string(),
            reviewer_actor: "reviewer:fixture".to_string(),
            reviewer_public_key: "5".repeat(64),
            observed_at: "2026-07-17T12:00:00Z".to_string(),
            decision_plan_root: decision_root,
            gate_state: "accept_blocked_reject_available".to_string(),
            provider: "test".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: ProtectionMode::Session,
            display: SignerDisplay {
                frontier_name: "Fixture frontier".to_string(),
                claim: "A bounded fixture result".to_string(),
                requester: "agent:fixture".to_string(),
                decisive_facts: vec!["No independent verifier evidence".to_string()],
                consequence: "Keep accepted state unchanged and close this proposal".to_string(),
            },
            events: vec![SignerEvent { event }],
        };
        (binary, request)
    }

    #[test]
    fn signer_contract_valid_exact_reject_request_passes() {
        let (_binary, request) = fixture();
        validate_request(&request, "2026-07-17T12:00:00Z".parse().unwrap()).unwrap();
    }

    #[test]
    fn signer_contract_unknown_fields_are_rejected() {
        let (_binary, request) = fixture();
        let mut value = serde_json::to_value(request).unwrap();
        value["wildcard"] = serde_json::json!(true);
        assert!(serde_json::from_value::<SignerRequest>(value).is_err());
    }

    #[test]
    fn signer_contract_rejects_unbounded_or_hostile_display_text() {
        let (_binary, request) = fixture();
        let now = "2026-07-17T12:00:00Z".parse().unwrap();
        let mut control = request.clone();
        control.display.claim = "safe\nforged button".to_string();
        assert!(validate_request(&control, now).is_err());
        let mut empty = request.clone();
        empty.display.decisive_facts.clear();
        assert!(validate_request(&empty, now).is_err());
        let mut many = request;
        many.display.decisive_facts = vec!["fact".to_string(); 5];
        assert!(validate_request(&many, now).is_err());
    }

    #[test]
    fn signer_contract_action_reason_and_decision_root_drift_fail() {
        let (binary, request) = fixture();
        let now = "2026-07-17T12:00:00Z".parse().unwrap();
        let mut action = request.clone();
        action.action = "accept".to_string();
        assert!(validate_request(&action, now).is_err());
        let mut reason = request.clone();
        reason.reason.push_str(" changed");
        assert!(validate_request(&reason, now).is_err());
        let mut root = request;
        root.decision_plan_root = format!("sha256:{}", "9".repeat(64));
        assert!(validate_request(&root, now).is_err());
        drop(binary);
    }

    #[test]
    fn signer_contract_expiry_and_binary_drift_fail() {
        let (mut binary, request) = fixture();
        assert!(validate_request(&request, "2026-07-17T12:04:00Z".parse().unwrap()).is_err());
        binary.write_all(b" drift").unwrap();
        assert!(validate_request(&request, "2026-07-17T12:00:00Z".parse().unwrap()).is_err());
    }

    #[test]
    fn signer_rebind_requires_a_real_pinned_change() {
        let mut binary = tempfile::NamedTempFile::new().unwrap();
        binary.write_all(b"pinned vela fixture").unwrap();
        let mut request = RebindRequest {
            schema: REBIND_REQUEST_SCHEMA.to_string(),
            purpose: RebindPurpose::Upgrade,
            nonce: "6".repeat(64),
            expires_at: "2026-07-17T12:01:00Z".to_string(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            previous_vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: format!("sha256:{}", "7".repeat(64)),
            previous_helper_sha256: format!("sha256:{}", "7".repeat(64)),
            actor: "reviewer:fixture".to_string(),
            public_key: "8".repeat(64),
            provider: "test".to_string(),
            previous_protection_mode: ProtectionMode::Session,
            protection_mode: ProtectionMode::Session,
        };
        let now = "2026-07-17T12:00:00Z".parse().unwrap();
        assert!(validate_rebind_request(&request, now).is_err());
        request.protection_mode = ProtectionMode::Always;
        validate_rebind_request(&request, now).unwrap();
        request.protection_mode = ProtectionMode::Session;
        request.helper_sha256 = format!("sha256:{}", "9".repeat(64));
        validate_rebind_request(&request, now).unwrap();
        request.helper_sha256 = request.previous_helper_sha256.clone();
        request.previous_vela_binary_sha256 = format!("sha256:{}", "0".repeat(64));
        validate_rebind_request(&request, now).unwrap();
        request.previous_vela_binary_sha256 = request.vela_binary_sha256.clone();
        request.purpose = RebindPurpose::EnrollmentRecovery;
        validate_rebind_request(&request, now).unwrap();
        request.helper_sha256 = format!("sha256:{}", "1".repeat(64));
        assert!(validate_rebind_request(&request, now).is_err());
    }

    #[test]
    fn documented_signer_request_schema_covers_every_root_bound_field() {
        let (_binary, request) = fixture();
        let value = serde_json::to_value(request).unwrap();
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/schemas/vela.signer-request.v1.schema.json"
        ))
        .unwrap();
        let required = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<BTreeSet<_>>();
        let actual = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(required, actual);

        let display_properties = schema["properties"]["display"]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let display_actual = value["display"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(display_properties, display_actual);
    }
}
