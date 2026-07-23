//! Closed helper IPC for one protected actor-registry bootstrap.
//!
//! Actor bootstrap is local custody plumbing, not a scientific decision and
//! not a second actor-registration protocol. The trusted helper receives the
//! exact Profile v1 repository facts and the one candidate [`ActorRecord`],
//! obtains fresh platform user presence, proves possession of the matching
//! OS-custodied key, and exits. The signed response authorizes no other actor
//! or repository mutation.

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_protocol::frontier_profile::{FRONTIER_PROFILE_SCHEMA_V1, FrontierProfileV1};
use vela_protocol::sign::ActorRecord;

use crate::contract::{ProtectionMode, file_sha256, validate_hex_signature};

pub const ACTOR_BOOTSTRAP_REQUEST_SCHEMA: &str = "vela.actor-bootstrap-proof-request.v1";
pub const ACTOR_BOOTSTRAP_RESPONSE_SCHEMA: &str = "vela.actor-bootstrap-proof-response.v1";
const ACTOR_BOOTSTRAP_REQUEST_DOMAIN: &[u8] = b"vela.actor-bootstrap-proof-request.v1\0";
const ACTOR_BOOTSTRAP_RESPONSE_DOMAIN: &[u8] = b"vela.actor-bootstrap-proof-response.v1\0";
const ACTOR_RECORD_ROOT_DOMAIN: &[u8] = b"vela.actor-bootstrap-record.v1\0";
const MAX_REQUEST_LIFETIME_SECONDS: i64 = 120;
const MAX_CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBootstrapDisplay {
    pub frontier_name: String,
    pub actor: String,
    pub consequence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBootstrapProofRequest {
    pub schema: String,
    pub nonce: String,
    pub expires_at: String,
    pub vela_binary_path: String,
    pub vela_binary_sha256: String,
    pub helper_sha256: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub profile: FrontierProfileV1,
    pub profile_root: String,
    pub actor_id: String,
    pub actor_public_key: String,
    pub actor_record: ActorRecord,
    pub actor_record_root: String,
    pub actor_registry_root_before: String,
    pub actor_registry_root_after: String,
    pub event_log_root: String,
    pub event_count: u64,
    pub snapshot_root: String,
    pub reason: String,
    pub observed_at: String,
    pub provider: String,
    pub protection_grade: String,
    pub protection_mode: ProtectionMode,
    pub display: ActorBootstrapDisplay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBootstrapProofResponse {
    pub schema: String,
    pub request_root: String,
    pub frontier_id: String,
    pub profile_root: String,
    pub actor_id: String,
    pub actor_public_key: String,
    pub actor_record_root: String,
    pub actor_registry_root_before: String,
    pub actor_registry_root_after: String,
    pub helper_version: String,
    pub helper_sha256: String,
    pub provider: String,
    pub protection_grade: String,
    pub approved_at: String,
    pub protection_mode: ProtectionMode,
    pub signature: String,
}

pub fn actor_bootstrap_request_root(
    request: &ActorBootstrapProofRequest,
) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(request)
        .map_err(|error| format!("canonicalize actor-bootstrap proof request: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(ACTOR_BOOTSTRAP_REQUEST_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

pub fn actor_record_root(record: &ActorRecord) -> Result<String, String> {
    let canonical = vela_protocol::canonical::to_canonical_bytes(record)
        .map_err(|error| format!("canonicalize actor-bootstrap record: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(ACTOR_RECORD_ROOT_DOMAIN);
    digest.update(canonical);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

/// Exact byte root used by the canonical `.vela/actors.json` representation.
///
/// The bootstrap transaction owns installation. This helper exists so the
/// request can bind both the empty preimage and the single-actor postimage.
pub fn actor_registry_file_root(actors: &[ActorRecord]) -> Result<String, String> {
    let bytes = serde_json::to_vec_pretty(actors)
        .map_err(|error| format!("serialize actor registry bytes: {error}"))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

/// Human-readable semantic summary for the one protected bootstrap prompt.
/// Every field here is already inside the signed request; surfacing clearance,
/// ORCID, and the actor-record root prevents a user-presence approval from
/// authorizing security-relevant metadata that the operating-system card did
/// not name.
pub fn actor_bootstrap_prompt(request: &ActorBootstrapProofRequest) -> String {
    let clearance = request
        .actor_record
        .access_clearance
        .map_or("public", |value| value.canonical());
    let orcid = request.actor_record.orcid.as_deref().unwrap_or("none");
    format!(
        "Register {} for {} ({}); clearance {}; ORCID {}; actor record {}; key {}. Key possession only: no scientific acceptance or policy activation.",
        request.actor_id,
        request.display.frontier_name,
        request.frontier_id,
        clearance,
        orcid,
        short_root(&request.actor_record_root),
        request
            .actor_public_key
            .chars()
            .take(16)
            .collect::<String>(),
    )
}

fn short_root(value: &str) -> &str {
    value.get(..value.len().min(15)).unwrap_or(value)
}

pub fn actor_bootstrap_response_signing_bytes(
    response: &ActorBootstrapProofResponse,
) -> Result<Vec<u8>, String> {
    let mut unsigned = response.clone();
    unsigned.signature.clear();
    let canonical = vela_protocol::canonical::to_canonical_bytes(&unsigned)
        .map_err(|error| format!("canonicalize actor-bootstrap proof response: {error}"))?;
    let mut bytes = ACTOR_BOOTSTRAP_RESPONSE_DOMAIN.to_vec();
    bytes.extend(canonical);
    Ok(bytes)
}

pub fn validate_actor_bootstrap_request(
    request: &ActorBootstrapProofRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    if request.schema != ACTOR_BOOTSTRAP_REQUEST_SCHEMA {
        return Err(format!(
            "actor-bootstrap proof request schema must be {ACTOR_BOOTSTRAP_REQUEST_SCHEMA}"
        ));
    }
    require_lower_hex("nonce", &request.nonce, 64)?;
    for (name, value) in [
        ("vela_binary_sha256", request.vela_binary_sha256.as_str()),
        ("helper_sha256", request.helper_sha256.as_str()),
        ("profile_root", request.profile_root.as_str()),
        ("actor_record_root", request.actor_record_root.as_str()),
        (
            "actor_registry_root_before",
            request.actor_registry_root_before.as_str(),
        ),
        (
            "actor_registry_root_after",
            request.actor_registry_root_after.as_str(),
        ),
        ("event_log_root", request.event_log_root.as_str()),
        ("snapshot_root", request.snapshot_root.as_str()),
    ] {
        require_sha256(name, value)?;
    }
    require_lower_hex("actor_public_key", &request.actor_public_key, 64)?;
    let public_bytes = hex::decode(&request.actor_public_key)
        .map_err(|error| format!("invalid actor_public_key: {error}"))?;
    VerifyingKey::from_bytes(
        &public_bytes
            .try_into()
            .map_err(|_| "actor_public_key must be 32 bytes".to_string())?,
    )
    .map_err(|error| format!("actor_public_key is not a valid Ed25519 key: {error}"))?;

    if !request.frontier_id.starts_with("vfr_") {
        return Err("frontier_id must start with vfr_".to_string());
    }
    let actor_handle = request
        .actor_id
        .strip_prefix("reviewer:")
        .or_else(|| request.actor_id.strip_prefix("steward:"))
        .unwrap_or_default();
    if actor_handle.is_empty()
        || actor_handle != actor_handle.trim()
        || actor_handle.chars().any(char::is_control)
    {
        return Err(
            "actor bootstrap requires a namespaced reviewer: or steward: human actor".to_string(),
        );
    }
    let frontier_path = Path::new(&request.frontier_path);
    if !frontier_path.is_absolute() {
        return Err("frontier_path must be absolute".to_string());
    }
    for (name, value) in [
        ("vela_binary_path", request.vela_binary_path.as_str()),
        ("frontier_path", request.frontier_path.as_str()),
        ("reason", request.reason.as_str()),
        ("actor_id", request.actor_id.as_str()),
        ("provider", request.provider.as_str()),
        ("protection_grade", request.protection_grade.as_str()),
        (
            "display.frontier_name",
            request.display.frontier_name.as_str(),
        ),
        ("display.actor", request.display.actor.as_str()),
        ("display.consequence", request.display.consequence.as_str()),
    ] {
        require_text(name, value)?;
    }
    if request.reason != request.reason.trim() {
        return Err("reason must not contain outer whitespace".to_string());
    }

    validate_window(request, now)?;
    let observed_at = DateTime::parse_from_rfc3339(&request.observed_at)
        .map_err(|error| format!("observed_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if observed_at < now - Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS)
        || observed_at > now + Duration::seconds(MAX_CLOCK_SKEW_SECONDS)
    {
        return Err("actor-bootstrap observation time is outside the request window".to_string());
    }

    request.profile.validate()?;
    if request.profile.schema != FRONTIER_PROFILE_SCHEMA_V1
        || request.profile.frontier_id != request.frontier_id
    {
        return Err("actor-bootstrap profile does not match the exact frontier".to_string());
    }
    if request.profile.profile_root()? != request.profile_root {
        return Err("actor-bootstrap profile_root does not match the supplied profile".to_string());
    }
    if request.display.frontier_name != request.profile.name {
        return Err(
            "actor-bootstrap display frontier differs from the supplied profile".to_string(),
        );
    }

    let actor = &request.actor_record;
    if actor.id != request.actor_id
        || actor.public_key != request.actor_public_key
        || actor.algorithm != "ed25519"
        || actor.created_at != request.observed_at
        || actor.tier.is_some()
        || actor.revoked_at.is_some()
        || actor.revoked_reason.is_some()
    {
        return Err(
            "actor-bootstrap record does not match the exact active reviewer identity".to_string(),
        );
    }
    DateTime::parse_from_rfc3339(&actor.created_at)
        .map_err(|error| format!("actor created_at is not RFC3339: {error}"))?;
    if let Some(orcid) = actor.orcid.as_deref()
        && vela_protocol::sign::validate_orcid(orcid)? != orcid
    {
        return Err("actor-bootstrap ORCID must use canonical bare form".to_string());
    }
    if actor_record_root(actor)? != request.actor_record_root {
        return Err("actor_record_root does not match the candidate actor".to_string());
    }
    if request.actor_registry_root_before != actor_registry_file_root(&[])? {
        return Err("actor bootstrap requires the exact empty actor registry".to_string());
    }
    if request.actor_registry_root_after != actor_registry_file_root(std::slice::from_ref(actor))? {
        return Err("actor_registry_root_after does not match the one-actor registry".to_string());
    }
    if request.event_count == 0 {
        return Err("actor bootstrap requires an existing structural frontier event".to_string());
    }
    if request.display.actor != request.actor_id {
        return Err("actor-bootstrap display actor differs from the candidate actor".to_string());
    }
    let expected_consequence = concat!(
        "Register this one human key as the first repository actor. ",
        "This proves key possession only; it does not accept scientific state or activate policy."
    );
    if request.display.consequence != expected_consequence {
        return Err("actor-bootstrap display omits the exact authority consequence".to_string());
    }
    if file_sha256(Path::new(&request.vela_binary_path))? != request.vela_binary_sha256 {
        return Err("pinned Vela binary digest does not match actor-bootstrap request".to_string());
    }
    Ok(())
}

pub fn validate_actor_bootstrap_request_fresh(
    request: &ActorBootstrapProofRequest,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if now > expires_at {
        return Err("actor-bootstrap proof request expired before approval completed".to_string());
    }
    Ok(())
}

pub fn validate_actor_bootstrap_response(
    request: &ActorBootstrapProofRequest,
    response: &ActorBootstrapProofResponse,
) -> Result<(), String> {
    if response.schema != ACTOR_BOOTSTRAP_RESPONSE_SCHEMA {
        return Err(format!(
            "actor-bootstrap proof response schema must be {ACTOR_BOOTSTRAP_RESPONSE_SCHEMA}"
        ));
    }
    if response.request_root != actor_bootstrap_request_root(request)?
        || response.frontier_id != request.frontier_id
        || response.profile_root != request.profile_root
        || response.actor_id != request.actor_id
        || response.actor_public_key != request.actor_public_key
        || response.actor_record_root != request.actor_record_root
        || response.actor_registry_root_before != request.actor_registry_root_before
        || response.actor_registry_root_after != request.actor_registry_root_after
    {
        return Err("actor-bootstrap proof response does not match the exact request".to_string());
    }
    if response.helper_version.trim().is_empty()
        || response.helper_sha256 != request.helper_sha256
        || response.provider != request.provider
        || response.protection_grade != request.protection_grade
        || response.protection_mode != request.protection_mode
    {
        return Err("actor-bootstrap proof helper or custody binding mismatch".to_string());
    }
    let approved_at = DateTime::parse_from_rfc3339(&response.approved_at)
        .map_err(|error| format!("approved_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let observed_at = DateTime::parse_from_rfc3339(&request.observed_at)
        .map_err(|error| format!("observed_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    let expires_at = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if approved_at < observed_at - Duration::seconds(MAX_CLOCK_SKEW_SECONDS)
        || approved_at > expires_at
    {
        return Err("actor-bootstrap proof was authorized outside its request window".to_string());
    }
    let signature = validate_hex_signature("actor-bootstrap proof signature", &response.signature)?;
    let public = hex::decode(&response.actor_public_key)
        .map_err(|error| format!("invalid actor-bootstrap public key: {error}"))?;
    let verifying = VerifyingKey::from_bytes(
        &public
            .try_into()
            .map_err(|_| "actor-bootstrap public key must be 32 bytes".to_string())?,
    )
    .map_err(|error| format!("invalid actor-bootstrap public key: {error}"))?;
    verifying
        .verify(
            &actor_bootstrap_response_signing_bytes(response)?,
            &Signature::from_bytes(&signature.to_bytes()),
        )
        .map_err(|_| "actor-bootstrap proof signature does not verify".to_string())
}

fn validate_window(request: &ActorBootstrapProofRequest, now: DateTime<Utc>) -> Result<(), String> {
    let expiry = DateTime::parse_from_rfc3339(&request.expires_at)
        .map_err(|error| format!("expires_at is not RFC3339: {error}"))?
        .with_timezone(&Utc);
    if expiry < now - Duration::seconds(MAX_CLOCK_SKEW_SECONDS) {
        return Err("actor-bootstrap proof request expired".to_string());
    }
    if expiry > now + Duration::seconds(MAX_REQUEST_LIFETIME_SECONDS) {
        return Err("actor-bootstrap proof expiry exceeds two minutes".to_string());
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

fn require_text(name: &str, value: &str) -> Result<(), String> {
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
    use ed25519_dalek::{Signer, SigningKey};
    use tempfile::NamedTempFile;
    use vela_protocol::frontier_profile::{FrontierProfileLicenseV1, FrontierProfileScopeV1};

    const NOW: &str = "2026-07-23T12:00:00Z";
    const CONSEQUENCE: &str = concat!(
        "Register this one human key as the first repository actor. ",
        "This proves key possession only; it does not accept scientific state or activate policy."
    );

    fn request() -> (NamedTempFile, SigningKey, ActorBootstrapProofRequest) {
        let binary = NamedTempFile::new().unwrap();
        std::fs::write(binary.path(), b"fixed vela binary").unwrap();
        let key = SigningKey::from_bytes(&[41; 32]);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let profile = FrontierProfileV1 {
            schema: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            frontier_id: "vfr_0123456789abcdef".to_string(),
            name: "Bootstrap fixture".to_string(),
            summary: "Bind the first human actor to exact protected custody.".to_string(),
            scope: FrontierProfileScopeV1 {
                question: "Can the first actor prove possession of the configured key?".to_string(),
                includes: Vec::new(),
                excludes: Vec::new(),
            },
            maintainers: Vec::new(),
            license: FrontierProfileLicenseV1 {
                content: "CC-BY-4.0".to_string(),
                code: "Apache-2.0".to_string(),
                data: "varies".to_string(),
            },
        };
        let actor_record = ActorRecord {
            id: "reviewer:bootstrap".to_string(),
            public_key: public_key.clone(),
            algorithm: "ed25519".to_string(),
            created_at: NOW.to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let request = ActorBootstrapProofRequest {
            schema: ACTOR_BOOTSTRAP_REQUEST_SCHEMA.to_string(),
            nonce: "1".repeat(64),
            expires_at: "2026-07-23T12:01:00Z".to_string(),
            vela_binary_path: binary.path().display().to_string(),
            vela_binary_sha256: file_sha256(binary.path()).unwrap(),
            helper_sha256: format!("sha256:{}", "2".repeat(64)),
            frontier_id: profile.frontier_id.clone(),
            frontier_path: std::env::temp_dir()
                .join("bootstrap-frontier")
                .display()
                .to_string(),
            profile_root: profile.profile_root().unwrap(),
            profile,
            actor_id: actor_record.id.clone(),
            actor_public_key: public_key,
            actor_record_root: actor_record_root(&actor_record).unwrap(),
            actor_registry_root_before: actor_registry_file_root(&[]).unwrap(),
            actor_registry_root_after: actor_registry_file_root(std::slice::from_ref(
                &actor_record,
            ))
            .unwrap(),
            actor_record,
            event_log_root: format!("sha256:{}", "3".repeat(64)),
            event_count: 1,
            snapshot_root: format!("sha256:{}", "4".repeat(64)),
            reason: "Establish the first protected human repository actor".to_string(),
            observed_at: NOW.to_string(),
            provider: "test".to_string(),
            protection_grade: "user_session".to_string(),
            protection_mode: ProtectionMode::Session,
            display: ActorBootstrapDisplay {
                frontier_name: "Bootstrap fixture".to_string(),
                actor: "reviewer:bootstrap".to_string(),
                consequence: CONSEQUENCE.to_string(),
            },
        };
        (binary, key, request)
    }

    fn response(
        request: &ActorBootstrapProofRequest,
        key: &SigningKey,
    ) -> ActorBootstrapProofResponse {
        let mut response = ActorBootstrapProofResponse {
            schema: ACTOR_BOOTSTRAP_RESPONSE_SCHEMA.to_string(),
            request_root: actor_bootstrap_request_root(request).unwrap(),
            frontier_id: request.frontier_id.clone(),
            profile_root: request.profile_root.clone(),
            actor_id: request.actor_id.clone(),
            actor_public_key: request.actor_public_key.clone(),
            actor_record_root: request.actor_record_root.clone(),
            actor_registry_root_before: request.actor_registry_root_before.clone(),
            actor_registry_root_after: request.actor_registry_root_after.clone(),
            helper_version: "0.914.0".to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: "2026-07-23T12:00:30Z".to_string(),
            protection_mode: request.protection_mode,
            signature: String::new(),
        };
        let signature = key.sign(&actor_bootstrap_response_signing_bytes(&response).unwrap());
        response.signature = format!("v1:{}", hex::encode(signature.to_bytes()));
        response
    }

    #[test]
    fn exact_actor_bootstrap_request_and_proof_validate() {
        let (_binary, key, request) = request();
        validate_actor_bootstrap_request(&request, NOW.parse().unwrap()).unwrap();
        validate_actor_bootstrap_response(&request, &response(&request, &key)).unwrap();
    }

    #[test]
    fn exact_steward_bootstrap_request_and_proof_validate() {
        let (_binary, key, mut request) = request();
        request.actor_id = "steward:bootstrap".to_string();
        request.actor_record.id = request.actor_id.clone();
        request.display.actor = request.actor_id.clone();
        request.actor_record_root = actor_record_root(&request.actor_record).unwrap();
        request.actor_registry_root_after =
            actor_registry_file_root(std::slice::from_ref(&request.actor_record)).unwrap();

        validate_actor_bootstrap_request(&request, NOW.parse().unwrap()).unwrap();
        validate_actor_bootstrap_response(&request, &response(&request, &key)).unwrap();
    }

    #[test]
    fn protected_prompt_names_clearance_orcid_and_exact_actor_record() {
        let (_binary, _key, mut request) = request();
        request.actor_record.access_clearance =
            Some(vela_protocol::access_tier::AccessTier::Classified);
        request.actor_record.orcid = Some("0000-0002-1825-0097".to_string());
        request.actor_record_root = actor_record_root(&request.actor_record).unwrap();
        request.actor_registry_root_after =
            actor_registry_file_root(std::slice::from_ref(&request.actor_record)).unwrap();

        let prompt = actor_bootstrap_prompt(&request);
        assert!(prompt.contains("clearance classified"), "{prompt}");
        assert!(prompt.contains("ORCID 0000-0002-1825-0097"), "{prompt}");
        assert!(
            prompt.contains(&request.actor_record_root[..15]),
            "{prompt}"
        );
        assert!(prompt.contains("no scientific acceptance"), "{prompt}");

        let mut public = request.clone();
        public.actor_record.access_clearance = None;
        public.actor_record.orcid = None;
        public.actor_record_root = actor_record_root(&public.actor_record).unwrap();
        public.actor_registry_root_after =
            actor_registry_file_root(std::slice::from_ref(&public.actor_record)).unwrap();
        assert_ne!(actor_bootstrap_prompt(&public), prompt);
        assert!(actor_bootstrap_prompt(&public).contains("clearance public"));
    }

    #[test]
    fn forged_profile_or_actor_record_fails_closed() {
        let (_binary, _key, mut forged_profile) = request();
        forged_profile.profile.name = "Forged profile".to_string();
        assert!(validate_actor_bootstrap_request(&forged_profile, NOW.parse().unwrap()).is_err());

        let (_binary, _key, mut forged_actor) = request();
        forged_actor.actor_record.public_key = "5".repeat(64);
        assert!(validate_actor_bootstrap_request(&forged_actor, NOW.parse().unwrap()).is_err());
    }

    #[test]
    fn response_tampering_and_expiry_fail_closed() {
        let (_binary, key, request) = request();
        let mut tampered = response(&request, &key);
        tampered.helper_version = "tampered".to_string();
        assert!(validate_actor_bootstrap_response(&request, &tampered).is_err());

        let mut late = response(&request, &key);
        late.approved_at = "2026-07-23T12:01:01Z".to_string();
        let signature = key.sign(&actor_bootstrap_response_signing_bytes(&late).unwrap());
        late.signature = format!("v1:{}", hex::encode(signature.to_bytes()));
        assert!(validate_actor_bootstrap_response(&request, &late).is_err());
    }
}
