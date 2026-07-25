//! Closed helper IPC for the one-time Era-0 to Era-1 authority migration.
//!
//! This contract is intentionally narrower than the historical decision
//! signer. It accepts exactly one unsigned `authority.model_migrated` event,
//! rederives every displayed migration fact from that event, obtains fresh
//! platform user presence, signs the event with the existing human continuity
//! key, and exits. It is migration scaffolding, not a reusable authority API.

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::authority_history::AuthorityModelMigrationV1;
use vela_protocol::events::{EVENT_KIND_AUTHORITY_MODEL_MIGRATED, StateEvent};

use crate::contract::{ProtectionMode, file_sha256, validate_hex_signature};

pub const AUTHORITY_MIGRATION_REQUEST_SCHEMA: &str = "vela.authority-migration-signer-request.v1";
pub const AUTHORITY_MIGRATION_RESPONSE_SCHEMA: &str = "vela.authority-migration-signer-response.v1";
pub const AUTHORITY_MIGRATION_REQUEST_LIFETIME_SECONDS: i64 = 10 * 60;
const REQUEST_DOMAIN: &[u8] = b"vela.authority-migration-signer-request.v1\0";
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityMigrationSignerRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub frontier_name: String,
    pub reason: String,
    pub legacy_actor: String,
    pub legacy_public_key: String,
    pub observed_at: String,
    pub migration_plan_root: String,
    pub new_principal_id: String,
    pub new_authority_keyset_root: String,
    pub new_policy_bundle_root: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub event: StateEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityMigrationSignerResponse {
    pub schema: String,
    pub request_root: String,
    pub legacy_public_key: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub provider: String,
    pub protection_grade: String,
    pub approved_at: String,
    pub protection_mode: ProtectionMode,
    pub event_id: String,
    pub event_signature: String,
}

pub fn authority_migration_request_root(
    request: &AuthorityMigrationSignerRequest,
) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize authority-migration signer request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn validate_authority_migration_request(
    request: &AuthorityMigrationSignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != AUTHORITY_MIGRATION_REQUEST_SCHEMA {
        return Err(format!(
            "authority-migration signer request schema must be {AUTHORITY_MIGRATION_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    require_sha256("vela_binary_sha256", &request.vela_binary_sha256)?;
    require_sha256("helper_sha256", &request.helper_sha256)?;
    require_sha256("migration_plan_root", &request.migration_plan_root)?;
    require_sha256(
        "new_authority_keyset_root",
        &request.new_authority_keyset_root,
    )?;
    require_sha256("new_policy_bundle_root", &request.new_policy_bundle_root)?;
    require_lower_hex("legacy_public_key", &request.legacy_public_key, 64)?;
    if !request.frontier_id.starts_with("vfr_") {
        return Err("frontier_id must start with vfr_".to_string());
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_path", request.frontier_path.as_str()),
        ("frontier_name", request.frontier_name.as_str()),
        ("reason", request.reason.as_str()),
        ("legacy_actor", request.legacy_actor.as_str()),
        ("new_principal_id", request.new_principal_id.as_str()),
        ("provider", request.provider.as_str()),
        ("protection_grade", request.protection_grade.as_str()),
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
    if event.kind.as_str() != EVENT_KIND_AUTHORITY_MODEL_MIGRATED {
        return Err("authority migration signer accepts only authority.model_migrated".to_string());
    }
    if event.signature.is_some() {
        return Err("authority migration event is already signed".to_string());
    }
    if vela_protocol::events::compute_event_id(event) != event.id {
        return Err("authority migration event has a stale content id".to_string());
    }
    if event.target.r#type != "frontier" || event.target.id != request.frontier_id {
        return Err("authority migration target does not match frontier_id".to_string());
    }
    if event.actor.r#type != "human" || event.actor.id != request.legacy_actor {
        return Err("authority migration actor does not match the continuity actor".to_string());
    }
    if event.reason != request.reason || event.timestamp != request.observed_at {
        return Err(
            "authority migration reason or observation time differs from request".to_string(),
        );
    }
    let payload: AuthorityModelMigrationV1 = serde_json::from_value(event.payload.clone())
        .map_err(|error| format!("invalid authority migration payload: {error}"))?;
    payload.validate()?;
    if payload.frontier_id != request.frontier_id
        || payload.reason != request.reason
        || payload.new_principal_id != request.new_principal_id
        || payload.new_authority_keyset_root != request.new_authority_keyset_root
        || payload.new_policy_bundle_root != request.new_policy_bundle_root
    {
        return Err("authority migration payload differs from the closed request".to_string());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match migration request".to_string());
    }
    Ok(())
}

pub fn validate_authority_migration_request_fresh(
    request: &AuthorityMigrationSignerRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expires_at {
        return Err("authority-migration signer request expired before approval completed".into());
    }
    Ok(())
}

pub fn validate_authority_migration_response(
    request: &AuthorityMigrationSignerRequest,
    response: &AuthorityMigrationSignerResponse,
) -> Result<(), String> {
    if response.schema != AUTHORITY_MIGRATION_RESPONSE_SCHEMA {
        return Err(format!(
            "authority-migration signer response schema must be {AUTHORITY_MIGRATION_RESPONSE_SCHEMA}"
        ));
    }
    if response.request_root != authority_migration_request_root(request)? {
        return Err("authority-migration response request_root mismatch".to_string());
    }
    if response.legacy_public_key != request.legacy_public_key {
        return Err("authority-migration response public key mismatch".to_string());
    }
    if response.helper_sha256 != request.helper_sha256
        || response.provider != request.provider
        || response.protection_grade != request.protection_grade
        || response.protection_mode != request.protection_mode
    {
        return Err("authority-migration response helper or custody binding mismatch".to_string());
    }
    if response.event_id != request.event.id {
        return Err("authority-migration response event id mismatch".to_string());
    }
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if approved_at > expires_at {
        return Err("authority-migration response was approved after expiry".to_string());
    }
    validate_hex_signature(
        "authority migration event signature",
        &response.event_signature,
    )?;
    let mut signed = request.event.clone();
    signed.signature = Some(response.event_signature.clone());
    vela_protocol::sign::verify_event_signature(&signed, &request.legacy_public_key)?;
    Ok(())
}

pub fn authority_migration_prompt(request: &AuthorityMigrationSignerRequest) -> String {
    format!(
        "Move {} to repository authority?\n\nContinuity actor\n{}\n\nNew administrator\n{}\n\nRepository keyset\n{}\n\nAuthorization policy\n{}\n\nRationale\n{}\n\nThis is the final use of the legacy personal key. Historical events remain byte-identical. Later authority is recorded by the repository authority.\n\nPlan {}",
        request.frontier_name,
        request.legacy_actor,
        request.new_principal_id,
        short_root(&request.new_authority_keyset_root),
        short_root(&request.new_policy_bundle_root),
        request.reason,
        short_root(&request.migration_plan_root),
    )
}

fn validate_window(value: &str, now: DateTime<Utc>) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(value)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("authority-migration signer request expired".to_string());
    }
    if expiry > now + Duration::seconds(AUTHORITY_MIGRATION_REQUEST_LIFETIME_SECONDS) {
        return Err("authority-migration signer request expiry exceeds ten minutes".to_string());
    }
    Ok(())
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{name} must use sha256:"))?;
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
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(format!("{name} must be non-empty bounded display text"));
    }
    Ok(())
}

fn short_root(root: &str) -> String {
    root.chars().take(23).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::NamedTempFile;
    use vela_protocol::authority_history::{
        AUTHORITY_MODEL_MIGRATION_SCHEMA_V1, AuthorityModelMigrationV1,
    };
    use vela_protocol::events::{
        EVENT_SCHEMA, EventKind, NULL_HASH, StateActor, StateTarget, compute_event_id,
    };
    use vela_protocol::sign::{pubkey_hex, sign_event};

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn fixture_request() -> (NamedTempFile, SigningKey, AuthorityMigrationSignerRequest) {
        let binary = NamedTempFile::new().unwrap();
        std::fs::write(binary.path(), b"vela fixture binary").unwrap();
        let legacy_key = SigningKey::from_bytes(&[7; 32]);
        let payload = AuthorityModelMigrationV1 {
            schema: AUTHORITY_MODEL_MIGRATION_SCHEMA_V1.into(),
            frontier_id: "vfr_fixture".into(),
            legacy_event_log_root: root('1'),
            legacy_actor_registry_root: root('2'),
            legacy_active_policy_head_root: root('3'),
            legacy_policy_store_manifest_root: root('4'),
            new_authority_keyset_root: root('5'),
            new_policy_bundle_root: root('6'),
            new_principal_id: "local:device|uid:501".into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Move this fixture to repository authority.".into(),
        };
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: EventKind::AuthorityModelMigrated,
            target: StateTarget {
                r#type: "frontier".into(),
                id: "vfr_fixture".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: "reviewer:fixture".into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: payload.reason.clone(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: serde_json::to_value(payload).unwrap(),
            caveats: vec!["Historical events remain byte-identical.".into()],
            signature: None,
        };
        event.id = compute_event_id(&event);
        let request = AuthorityMigrationSignerRequest {
            schema: AUTHORITY_MIGRATION_REQUEST_SCHEMA.into(),
            nonce: "a".repeat(64),
            expires_at: "2026-07-24T12:10:00Z".into(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: root('7'),
            frontier_id: "vfr_fixture".into(),
            frontier_path: "/tmp/frontier".into(),
            frontier_name: "Fixture Frontier".into(),
            reason: "Move this fixture to repository authority.".into(),
            legacy_actor: "reviewer:fixture".into(),
            legacy_public_key: pubkey_hex(&legacy_key),
            observed_at: "2026-07-24T12:00:00Z".into(),
            migration_plan_root: root('8'),
            new_principal_id: "local:device|uid:501".into(),
            new_authority_keyset_root: root('5'),
            new_policy_bundle_root: root('6'),
            provider: "os_store".into(),
            protection_grade: "hardware_or_os_protected".into(),
            protection_mode: ProtectionMode::Always,
            event,
        };
        (binary, legacy_key, request)
    }

    #[test]
    fn accepts_only_the_exact_unsigned_migration_event() {
        let (_binary, legacy_key, request) = fixture_request();
        validate_authority_migration_request(&request, "2026-07-24T12:00:00Z".parse().unwrap())
            .unwrap();

        let mut response = AuthorityMigrationSignerResponse {
            schema: AUTHORITY_MIGRATION_RESPONSE_SCHEMA.into(),
            request_root: authority_migration_request_root(&request).unwrap(),
            legacy_public_key: request.legacy_public_key.clone(),
            helper_version: "0.930.0-rc.1".into(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: "2026-07-24T12:01:00Z".into(),
            protection_mode: request.protection_mode,
            event_id: request.event.id.clone(),
            event_signature: sign_event(&request.event, &legacy_key).unwrap(),
        };
        validate_authority_migration_response(&request, &response).unwrap();

        response.event_id = "vev_substituted".into();
        assert!(
            validate_authority_migration_response(&request, &response)
                .unwrap_err()
                .contains("event id mismatch")
        );
    }

    #[test]
    fn rejects_kind_payload_and_binary_substitution() {
        let (_binary, _legacy_key, mut request) = fixture_request();
        request.event.kind = EventKind::ReviewAccepted;
        request.event.id = compute_event_id(&request.event);
        assert!(
            validate_authority_migration_request(&request, "2026-07-24T12:00:00Z".parse().unwrap())
                .unwrap_err()
                .contains("accepts only")
        );

        let (_binary, _legacy_key, mut request) = fixture_request();
        request.new_policy_bundle_root = root('9');
        assert!(
            validate_authority_migration_request(&request, "2026-07-24T12:00:00Z".parse().unwrap())
                .unwrap_err()
                .contains("payload differs")
        );

        let (_binary, _legacy_key, mut request) = fixture_request();
        request.vela_binary_sha256 = root('f');
        assert!(
            validate_authority_migration_request(&request, "2026-07-24T12:00:00Z".parse().unwrap())
                .unwrap_err()
                .contains("binary digest")
        );
    }
}
