//! Closed helper IPC for one protected Frontier repository boundary.
//!
//! This is local custody plumbing, not a second repository protocol. The
//! helper accepts one fully formed, unsigned `frontier.repository_bound`
//! event, rederives every fact it displays, obtains fresh platform user
//! presence, signs that one event, and exits.

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::events::{EVENT_KIND_FRONTIER_REPOSITORY_BOUND, StateEvent};
use vela_protocol::frontier_repository::{
    FrontierRepositoryBoundaryMode, FrontierRepositoryBoundaryPayloadV1,
    FrontierRepositoryTrustMode,
};

use crate::contract::{ProtectionMode, file_sha256, validate_hex_signature};

pub const REPOSITORY_REQUEST_SCHEMA: &str = "vela.repository-boundary-signer-request.v1";
pub const REPOSITORY_RESPONSE_SCHEMA: &str = "vela.repository-boundary-signer-response.v1";
const REPOSITORY_REQUEST_DOMAIN: &[u8] = b"vela.repository-boundary-signer-request.v1\0";
const MAX_REQUEST_LIFETIME_SECONDS: i64 = 120;
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBoundaryDisplay {
    pub frontier_name: String,
    pub profile_version: String,
    pub dependency_summary: String,
    pub consequence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBoundarySignerRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub reason: String,
    pub administrator_actor: String,
    pub administrator_public_key: String,
    pub observed_at: String,
    pub boundary_plan_root: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub display: RepositoryBoundaryDisplay,
    pub event: StateEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryBoundarySignerResponse {
    pub schema: String,
    pub request_root: String,
    pub administrator_public_key: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub provider: String,
    pub protection_grade: String,
    pub approved_at: String,
    pub protection_mode: ProtectionMode,
    pub event_id: String,
    pub event_signature: String,
}

pub fn repository_boundary_request_root(
    request: &RepositoryBoundarySignerRequest,
) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize repository-boundary signer request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(REPOSITORY_REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn validate_repository_boundary_request(
    request: &RepositoryBoundarySignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != REPOSITORY_REQUEST_SCHEMA {
        return Err(format!(
            "repository-boundary signer request schema must be {REPOSITORY_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    require_sha256("boundary_plan_root", &request.boundary_plan_root)?;
    require_lower_hex(
        "administrator_public_key",
        &request.administrator_public_key,
        64,
    )?;
    if !request.frontier_id.starts_with("vfr_") {
        return Err("frontier_id must start with vfr_".to_string());
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_path", request.frontier_path.as_str()),
        ("reason", request.reason.as_str()),
        ("administrator_actor", request.administrator_actor.as_str()),
        ("provider", request.provider.as_str()),
        ("protection_grade", request.protection_grade.as_str()),
        ("frontier_name", request.display.frontier_name.as_str()),
        ("profile_version", request.display.profile_version.as_str()),
        (
            "dependency_summary",
            request.display.dependency_summary.as_str(),
        ),
        ("consequence", request.display.consequence.as_str()),
    ] {
        require_display_text(name, value)?;
    }
    if request.reason != request.reason.trim() {
        return Err("reason must not contain outer whitespace".to_string());
    }

    validate_window(&request.expires_at, now)?;
    DateTime::parse_from_rfc3339(&request.observed_at)
        .map_err(|error| format!("observed_at is not RFC3339: {error}"))?;

    let event = &request.event;
    if event.kind.as_str() != EVENT_KIND_FRONTIER_REPOSITORY_BOUND {
        return Err("repository signer accepts only frontier.repository_bound".to_string());
    }
    if event.signature.is_some() {
        return Err("repository boundary event is already signed".to_string());
    }
    if vela_protocol::events::compute_event_id(event) != event.id {
        return Err("repository boundary event has a stale content id".to_string());
    }
    if event.target.r#type != "frontier" || event.target.id != request.frontier_id {
        return Err("repository boundary target does not match frontier_id".to_string());
    }
    if event.actor.r#type != "human" || event.actor.id != request.administrator_actor {
        return Err("repository boundary actor does not match administrator".to_string());
    }
    if event.reason != request.reason || event.timestamp != request.observed_at {
        return Err(
            "repository boundary reason or observation time differs from request".to_string(),
        );
    }
    let payload: FrontierRepositoryBoundaryPayloadV1 =
        serde_json::from_value(event.payload.clone())
            .map_err(|error| format!("invalid repository-boundary payload: {error}"))?;
    payload.validate()?;
    if payload.frontier_id != request.frontier_id
        || payload.administrator_actor_id != request.administrator_actor
        || payload.administrator_public_key != request.administrator_public_key
    {
        return Err("repository boundary payload identity differs from request".to_string());
    }
    if request.display.profile_version != "vela.frontier-profile.v1" {
        return Err("repository boundary display must name vela.frontier-profile.v1".to_string());
    }
    let expected_mode = match payload.mode {
        FrontierRepositoryBoundaryMode::TemporalizeExisting => "temporalize existing repository",
        FrontierRepositoryBoundaryMode::UpdateDependencies => "update exact dependencies",
    };
    let expected_trust = match payload.trust_mode {
        FrontierRepositoryTrustMode::Tofu => "first boundary requires an out-of-band pin",
        FrontierRepositoryTrustMode::Genesis => {
            "first administrator boundary requires an out-of-band pin"
        }
        FrontierRepositoryTrustMode::PreviousBoundary => "continues the pinned boundary chain",
    };
    if !request.display.consequence.contains(expected_mode)
        || !request.display.consequence.contains(expected_trust)
    {
        return Err(
            "repository boundary display omits the exact mode or trust consequence".to_string(),
        );
    }

    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match repository request".to_string());
    }
    Ok(())
}

pub fn validate_repository_boundary_request_fresh(
    request: &RepositoryBoundarySignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expires_at {
        return Err(
            "repository-boundary signer request expired before approval completed".to_string(),
        );
    }
    Ok(())
}

pub fn validate_repository_boundary_response(
    request: &RepositoryBoundarySignerRequest,
    response: &RepositoryBoundarySignerResponse,
) -> Result<(), String> {
    if response.schema != REPOSITORY_RESPONSE_SCHEMA {
        return Err(format!(
            "repository-boundary signer response schema must be {REPOSITORY_RESPONSE_SCHEMA}"
        ));
    }
    if response.request_root != repository_boundary_request_root(request)? {
        return Err("repository-boundary response request_root mismatch".to_string());
    }
    if response.administrator_public_key != request.administrator_public_key {
        return Err("repository-boundary response public key mismatch".to_string());
    }
    if response.helper_sha256 != request.helper_sha256
        || response.provider != request.provider
        || response.protection_grade != request.protection_grade
        || response.protection_mode != request.protection_mode
    {
        return Err("repository-boundary response helper or custody binding mismatch".to_string());
    }
    if response.event_id != request.event.id {
        return Err("repository-boundary response event id mismatch".to_string());
    }
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if approved_at > expires_at {
        return Err("repository-boundary response was approved after expiry".to_string());
    }
    validate_hex_signature(
        "repository boundary event signature",
        &response.event_signature,
    )?;
    let mut signed = request.event.clone();
    signed.signature = Some(response.event_signature.clone());
    if vela_protocol::sign::verify_event_signature(&signed, &response.administrator_public_key)? {
        Ok(())
    } else {
        Err("repository boundary event signature does not verify".to_string())
    }
}

fn validate_window(expires_at: &str, now: DateTime<Utc>) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("repository-boundary signer request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("repository-boundary signer expiry exceeds two minutes".to_string());
    }
    Ok(())
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use sha256:<64 lowercase hex>"));
    };
    require_lower_hex(name, digest, 64)
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

fn require_display_text(name: &str, value: &str) -> Result<(), String> {
    const MAX_BYTES: usize = 512;
    if value.trim().is_empty() || value.len() > MAX_BYTES || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} must be non-empty, at most {MAX_BYTES} bytes, and contain no control characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::NamedTempFile;
    use vela_protocol::frontier_repository::{
        FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierRepositoryBoundaryPayloadV1, GitObjectFormat,
        LegacyFrontierOriginV1, exact_dependency_root, new_repository_boundary_event,
    };

    const NOW: &str = "2026-07-22T12:00:00Z";

    fn request() -> (NamedTempFile, SigningKey, RepositoryBoundarySignerRequest) {
        let binary = NamedTempFile::new().unwrap();
        std::fs::write(binary.path(), b"fixed vela binary").unwrap();
        let key = SigningKey::from_bytes(&[37; 32]);
        let public = hex::encode(key.verifying_key().to_bytes());
        let dependencies = Vec::new();
        let legacy_root = format!("sha256:{}", "1".repeat(64));
        let mut payload = FrontierRepositoryBoundaryPayloadV1 {
            schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
            mode: FrontierRepositoryBoundaryMode::TemporalizeExisting,
            frontier_id: "vfr_0123456789abcdef".to_string(),
            identity_root: String::new(),
            observed_profile_root: format!("sha256:{}", "2".repeat(64)),
            dependency_root: exact_dependency_root(&dependencies).unwrap(),
            dependencies,
            previous_identity_event_root: None,
            legacy_identity_preimage_root: Some(legacy_root.clone()),
            administrator_actor_id: "reviewer:migration".to_string(),
            administrator_public_key: public.clone(),
            administrator_algorithm: "ed25519".to_string(),
            trust_mode: FrontierRepositoryTrustMode::Tofu,
            git_object_format: GitObjectFormat::Sha1,
            anchor_git_commit: "3".repeat(40),
            anchor_git_tree: "4".repeat(40),
            anchor_event_log_root: format!("sha256:{}", "5".repeat(64)),
            anchor_event_count: 7,
            anchor_snapshot_root: format!("sha256:{}", "6".repeat(64)),
            anchor_snapshot_schema: "vela.frontier.v0.1".to_string(),
            anchor_proposal_root: format!("sha256:{}", "7".repeat(64)),
            anchor_actor_registry_root: format!("sha256:{}", "8".repeat(64)),
            anchor_artifact_registry_root: format!("sha256:{}", "9".repeat(64)),
            anchor_canonical_store_root: format!("sha256:{}", "a".repeat(64)),
        };
        payload.identity_root = LegacyFrontierOriginV1 {
            schema: vela_protocol::frontier_repository::LEGACY_FRONTIER_ORIGIN_SCHEMA.to_string(),
            frontier_id: payload.frontier_id.clone(),
            legacy_identity_preimage_root: legacy_root,
            git_object_format: payload.git_object_format,
            anchor_git_commit: payload.anchor_git_commit.clone(),
            anchor_git_tree: payload.anchor_git_tree.clone(),
            anchor_event_log_root: payload.anchor_event_log_root.clone(),
            anchor_event_count: payload.anchor_event_count,
        }
        .identity_root()
        .unwrap();
        let event = new_repository_boundary_event(payload, "Bind exact repository", NOW).unwrap();
        let request = RepositoryBoundarySignerRequest {
            schema: REPOSITORY_REQUEST_SCHEMA.to_string(),
            nonce: "b".repeat(64),
            expires_at: "2026-07-22T12:01:00Z".to_string(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: format!("sha256:{}", "c".repeat(64)),
            frontier_id: "vfr_0123456789abcdef".to_string(),
            frontier_path: "/tmp/frontier".to_string(),
            reason: "Bind exact repository".to_string(),
            administrator_actor: "reviewer:migration".to_string(),
            administrator_public_key: public,
            observed_at: NOW.to_string(),
            boundary_plan_root: format!("sha256:{}", "d".repeat(64)),
            provider: "os_store".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: ProtectionMode::Session,
            display: RepositoryBoundaryDisplay {
                frontier_name: "Migration fixture".to_string(),
                profile_version: "vela.frontier-profile.v1".to_string(),
                dependency_summary: "0 exact dependencies".to_string(),
                consequence:
                    "temporalize existing repository; first boundary requires an out-of-band pin"
                        .to_string(),
            },
            event,
        };
        (binary, key, request)
    }

    #[test]
    fn exact_repository_request_and_response_validate() {
        let (_binary, key, request) = request();
        validate_repository_boundary_request(&request, NOW.parse().unwrap()).unwrap();
        let response = RepositoryBoundarySignerResponse {
            schema: REPOSITORY_RESPONSE_SCHEMA.to_string(),
            request_root: repository_boundary_request_root(&request).unwrap(),
            administrator_public_key: request.administrator_public_key.clone(),
            helper_version: "0.914.0".to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: "2026-07-22T12:00:30Z".to_string(),
            protection_mode: request.protection_mode,
            event_id: request.event.id.clone(),
            event_signature: vela_protocol::sign::sign_event(&request.event, &key).unwrap(),
        };
        validate_repository_boundary_response(&request, &response).unwrap();
    }

    #[test]
    fn repository_request_drift_fails_closed() {
        let (_binary, _key, mut drifted_request) = request();
        drifted_request.event.reason = "different".to_string();
        drifted_request.event.id = vela_protocol::events::compute_event_id(&drifted_request.event);
        assert!(
            validate_repository_boundary_request(&drifted_request, NOW.parse().unwrap()).is_err()
        );

        let (_binary, _key, mut display_drifted_request) = request();
        display_drifted_request.display.consequence = "looks harmless".to_string();
        assert!(
            validate_repository_boundary_request(&display_drifted_request, NOW.parse().unwrap())
                .is_err()
        );
    }
}
