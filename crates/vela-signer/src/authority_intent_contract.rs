//! Closed, approval-only IPC for an Era-1 repository-authority exception.
//!
//! The helper obtains fresh platform user presence for one exact intent and
//! returns only a bearer-free observation. It never reads a Vela key and does
//! not sign the repository transaction; the repository authority remains the
//! sole transaction signer.

use std::path::Path;

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::contract::file_sha256;

pub const AUTHORITY_INTENT_REQUEST_SCHEMA: &str = "vela.authority-intent-request.v1";
pub const AUTHORITY_INTENT_RESPONSE_SCHEMA: &str = "vela.authority-intent-response.v1";
pub const AUTHORITY_INTENT_REQUEST_LIFETIME_SECONDS: i64 = 10 * 60;
const REQUEST_DOMAIN: &[u8] = b"vela.authority-intent-request.v1\0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityIntentRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_name: String,
    pub principal_id: String,
    pub action: String,
    pub reason: String,
    pub intent_digest: String,
    pub current_policy_bundle_root: String,
    pub next_policy_bundle_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityIntentResponse {
    pub schema: String,
    pub request_root: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub principal_id: String,
    pub action: String,
    pub approved_at: String,
    pub session_root: String,
}

pub fn authority_intent_request_root(request: &AuthorityIntentRequest) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize authority intent: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn validate_authority_intent_request(
    request: &AuthorityIntentRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != AUTHORITY_INTENT_REQUEST_SCHEMA {
        return Err(format!(
            "authority intent schema must be {AUTHORITY_INTENT_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    for (name, root) in [
        ("vela_binary_sha256", request.vela_binary_sha256.as_str()),
        ("helper_sha256", request.helper_sha256.as_str()),
        ("intent_digest", request.intent_digest.as_str()),
        (
            "current_policy_bundle_root",
            request.current_policy_bundle_root.as_str(),
        ),
        (
            "next_policy_bundle_root",
            request.next_policy_bundle_root.as_str(),
        ),
    ] {
        require_sha256(name, root)?;
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_id", request.frontier_id.as_str()),
        ("frontier_name", request.frontier_name.as_str()),
        ("principal_id", request.principal_id.as_str()),
        ("action", request.action.as_str()),
        ("reason", request.reason.as_str()),
    ] {
        require_text(name, value)?;
    }
    if request.action != vela_protocol::authority_history::POLICY_ROTATE_ACTION {
        return Err("authority intent helper accepts only policy_rotate".into());
    }
    if request.current_policy_bundle_root == request.next_policy_bundle_root {
        return Err("authority intent must change the policy bundle".into());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match authority intent".into());
    }
    validate_window(&request.expires_at, now)
}

pub fn validate_authority_intent_response(
    request: &AuthorityIntentRequest,
    response: &AuthorityIntentResponse,
) -> Result<(), String> {
    if response.schema != AUTHORITY_INTENT_RESPONSE_SCHEMA {
        return Err(format!(
            "authority intent response schema must be {AUTHORITY_INTENT_RESPONSE_SCHEMA}"
        ));
    }
    if response.request_root != authority_intent_request_root(request)?
        || response.helper_sha256 != request.helper_sha256
        || response.principal_id != request.principal_id
        || response.action != request.action
    {
        return Err("authority intent response differs from the closed request".into());
    }
    require_sha256("session_root", &response.session_root)?;
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    validate_window(&request.expires_at, approved_at)
}

pub fn authority_intent_prompt(request: &AuthorityIntentRequest) -> String {
    format!(
        "Authorize Vela policy change for {}?\n\nAction\nEnable signed-agent work leases\n\nWhat this permits\nAgents may claim, refresh, and release exact work leases after signing their own event.\n\nWhat this does not permit\nScientific acceptance, review decisions, policy administration, key rotation, or historical rewrites.\n\nReason\n{}\n\nCurrent policy {}\nNext policy {}\nIntent {}",
        request.frontier_name,
        request.reason,
        short_root(&request.current_policy_bundle_root),
        short_root(&request.next_policy_bundle_root),
        short_root(&request.intent_digest),
    )
}

pub fn session_root(request: &AuthorityIntentRequest, approved_at: &str) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "schema": "vela.platform-user-presence-session.v1",
        "request_root": authority_intent_request_root(request)?,
        "approved_at": approved_at,
    }))
    .map_err(|error| format!("canonicalize platform session: {error}"))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(canonical))))
}

fn validate_window(expires_at: &str, now: DateTime<Utc>) -> Result<(), String> {
    let expires_at = DateTime::parse_from_rfc3339(expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expires_at < now
        || expires_at - now > Duration::seconds(AUTHORITY_INTENT_REQUEST_LIFETIME_SECONDS)
    {
        return Err("authority intent is expired or exceeds ten minutes".into());
    }
    Ok(())
}

fn require_text(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > 4096
        || value.chars().any(char::is_control)
    {
        return Err(format!("{name} is invalid"));
    }
    Ok(())
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use sha256:"));
    };
    require_lower_hex(name, hex, 64)
}

fn require_lower_hex(name: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{name} must be {length} lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn short_root(value: &str) -> &str {
    value.get(..18).unwrap_or(value)
}

pub fn canonical_approved_at(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(now: DateTime<Utc>) -> AuthorityIntentRequest {
        let executable = std::env::current_exe().unwrap();
        let digest = file_sha256(&executable).unwrap();
        AuthorityIntentRequest {
            schema: AUTHORITY_INTENT_REQUEST_SCHEMA.into(),
            nonce: "a".repeat(64),
            expires_at: (now + Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Secs, true),
            vela_binary_path: executable.display().to_string(),
            vela_binary_sha256: digest.clone(),
            helper_sha256: digest,
            frontier_id: "vfr_fixture".into(),
            frontier_name: "Fixture frontier".into(),
            principal_id: "local:device-fixture|uid:501".into(),
            action: vela_protocol::authority_history::POLICY_ROTATE_ACTION.into(),
            reason: "Enable exact signed-agent work leases.".into(),
            intent_digest: format!("sha256:{}", "b".repeat(64)),
            current_policy_bundle_root: format!("sha256:{}", "c".repeat(64)),
            next_policy_bundle_root: format!("sha256:{}", "d".repeat(64)),
        }
    }

    #[test]
    fn exact_policy_intent_binds_action_scope_and_response() {
        let now = Utc::now();
        let request = request(now);
        validate_authority_intent_request(&request, now).unwrap();
        let approved_at = canonical_approved_at(now);
        let response = AuthorityIntentResponse {
            schema: AUTHORITY_INTENT_RESPONSE_SCHEMA.into(),
            request_root: authority_intent_request_root(&request).unwrap(),
            helper_version: "fixture".into(),
            helper_sha256: request.helper_sha256.clone(),
            principal_id: request.principal_id.clone(),
            action: request.action.clone(),
            approved_at: approved_at.clone(),
            session_root: session_root(&request, &approved_at).unwrap(),
        };
        validate_authority_intent_response(&request, &response).unwrap();

        let mut widened = request.clone();
        widened.action = "review_accept".into();
        assert!(validate_authority_intent_request(&widened, now).is_err());
        let mut drifted = response;
        drifted.principal_id = "local:other|uid:501".into();
        assert!(validate_authority_intent_response(&request, &drifted).is_err());
    }
}
