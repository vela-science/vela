//! Era-1 attributed repository-authority records and DSSE verification.
//!
//! These types are read/verify-only in the first migration slice. The released
//! Era-0 event and signer path remains the sole writer until shadow evaluation
//! and migration gates pass.

use std::collections::BTreeSet;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{sha256_canonical, to_canonical_bytes};
use crate::events::{EventKind, StateActor, StateTarget};

pub const AUTHORITY_KEYSET_SCHEMA_V1: &str = "vela.authority-keyset.v1";
pub const AUTHORITY_RECORD_SCHEMA_V1: &str = "vela.authority-record.v1";
pub const AUTHORITY_EVENT_SCHEMA_V1: &str = "vela.event.v1";
pub const AUTHORITY_PAYLOAD_TYPE_V1: &str = "application/vnd.vela.authority-record.v1+json";
pub const AUTHORITY_KEY_ALGORITHM: &str = "ed25519";
pub const AUTHORITY_KEY_PURPOSE: &str = "repository_authority";
pub const AUTHORITY_MODE: &str = "repository_authority";
pub const CEDAR_ENGINE: &str = "cedar-policy";
pub const CEDAR_ENGINE_VERSION: &str = "4.11.2";
pub const CEDAR_PROFILE_V1: &str = "vela.cedar-restricted.v1";
pub const POLICY_BUNDLE_SCHEMA_V1: &str = "vela.policy-bundle.v1";

/// Principal class is an application invariant as well as a Cedar attribute.
/// Agent and workload callers cannot gain human-only authority through a
/// malformed or overly broad policy bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Human,
    Agent,
    Workload,
    Service,
    Institution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CedarDecision {
    Allow,
    Deny,
}

/// Canonicalizable authorization result retained by an authority record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CedarEvaluation {
    pub engine: String,
    pub engine_version: String,
    pub profile: String,
    pub valid: bool,
    pub decision: CedarDecision,
    pub automatic_permit: bool,
    pub determining_policies: Vec<String>,
    pub diagnostics: Vec<String>,
}

/// Closed policy-bundle manifest. Bundle files remain separate canonical
/// bytes; this manifest binds their full roots and the exact evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBundleV1 {
    pub schema: String,
    pub frontier_id: String,
    pub cedar_schema_root: String,
    pub policies_root: String,
    pub entities_root: String,
    pub tests_root: String,
    pub engine: String,
    pub engine_version: String,
    pub restricted_profile: String,
    pub previous_bundle_root: Option<String>,
    pub authority_summary: String,
}

impl PolicyBundleV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != POLICY_BUNDLE_SCHEMA_V1 {
            return Err(format!(
                "policy bundle schema must be {POLICY_BUNDLE_SCHEMA_V1}"
            ));
        }
        if self.engine != CEDAR_ENGINE
            || self.engine_version != CEDAR_ENGINE_VERSION
            || self.restricted_profile != CEDAR_PROFILE_V1
        {
            return Err("policy bundle evaluator identity is not the pinned Vela profile".into());
        }
        for (name, value) in [
            ("cedar_schema_root", self.cedar_schema_root.as_str()),
            ("policies_root", self.policies_root.as_str()),
            ("entities_root", self.entities_root.as_str()),
            ("tests_root", self.tests_root.as_str()),
        ] {
            require_sha256(name, value)?;
        }
        if let Some(root) = &self.previous_bundle_root {
            require_sha256("previous_bundle_root", root)?;
        }
        if self.frontier_id.trim().is_empty() || self.authority_summary.trim().is_empty() {
            return Err("policy bundle frontier and authority summary must be non-empty".into());
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

/// Era-1 event content. Authority is carried by the covering transaction
/// record rather than a per-event human signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityEventContentV1 {
    pub transaction_id: String,
    pub principal_id: String,
    pub authority_mode: String,
    pub kind: EventKind,
    pub target: StateTarget,
    pub actor: StateActor,
    pub timestamp: String,
    pub reason: String,
    pub before_hash: String,
    pub after_hash: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityEventV1 {
    pub schema: String,
    pub id: String,
    pub content: AuthorityEventContentV1,
}

impl AuthorityEventV1 {
    pub fn new(content: AuthorityEventContentV1) -> Result<Self, String> {
        let digest = sha256_canonical(&content)?;
        let event = Self {
            schema: AUTHORITY_EVENT_SCHEMA_V1.into(),
            id: format!("vev_{}", &digest[..16]),
            content,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn derive_id(&self) -> Result<String, String> {
        let digest = sha256_canonical(&self.content)?;
        Ok(format!("vev_{}", &digest[..16]))
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_EVENT_SCHEMA_V1 || self.id != self.derive_id()? {
            return Err("Era-1 event schema or content address is invalid".into());
        }
        if self.content.transaction_id.trim().is_empty()
            || self.content.principal_id.trim().is_empty()
            || self.content.authority_mode != AUTHORITY_MODE
            || self.content.actor.id.trim().is_empty()
            || self.content.timestamp.trim().is_empty()
            || self.content.reason.trim().is_empty()
        {
            return Err("Era-1 event attribution and transaction fields must be explicit".into());
        }
        require_sha256("before_hash", &self.content.before_hash)?;
        require_sha256("after_hash", &self.content.after_hash)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityKeyV1 {
    pub key_id: String,
    pub algorithm: String,
    pub public_key: String,
    pub valid_from_sequence: u64,
    pub valid_through_sequence: Option<u64>,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityKeysetV1 {
    pub schema: String,
    pub frontier_id: String,
    pub generation: u64,
    pub threshold: u32,
    pub keys: Vec<AuthorityKeyV1>,
    pub previous_keyset_root: Option<String>,
    pub activation_record_root: Option<String>,
}

impl AuthorityKeysetV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_KEYSET_SCHEMA_V1 {
            return Err(format!(
                "authority keyset schema must be {AUTHORITY_KEYSET_SCHEMA_V1}"
            ));
        }
        if self.frontier_id.trim().is_empty() || self.generation == 0 {
            return Err("authority keyset frontier and generation must be set".into());
        }
        if self.keys.is_empty()
            || self.threshold == 0
            || usize::try_from(self.threshold).unwrap_or(usize::MAX) > self.keys.len()
        {
            return Err("authority keyset threshold must be within the key count".into());
        }
        let mut key_ids = BTreeSet::new();
        for key in &self.keys {
            if !key_ids.insert(key.key_id.as_str()) {
                return Err(format!("duplicate authority key ID {}", key.key_id));
            }
            if key.algorithm != AUTHORITY_KEY_ALGORITHM
                || key.purpose != AUTHORITY_KEY_PURPOSE
                || key.valid_from_sequence == 0
            {
                return Err(format!(
                    "authority key {} has an invalid algorithm, purpose, or sequence",
                    key.key_id
                ));
            }
            if let Some(through) = key.valid_through_sequence
                && through < key.valid_from_sequence
            {
                return Err(format!(
                    "authority key {} validity window is inverted",
                    key.key_id
                ));
            }
            decode_fixed_hex::<32>("authority public key", &key.public_key)?;
        }
        if let Some(root) = &self.previous_keyset_root {
            require_sha256("previous_keyset_root", root)?;
        }
        if let Some(root) = &self.activation_record_root {
            require_sha256("activation_record_root", root)?;
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalSnapshotV1 {
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub display_name: Option<String>,
    pub affiliation: Option<String>,
    pub account_links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationClaimV1 {
    pub method: String,
    pub session_id: String,
    pub authenticated_at: String,
    pub assurance: String,
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationClaimV1 {
    pub capability_id: String,
    pub issuer_principal_id: String,
    pub subject_principal_id: String,
    pub actions: Vec<String>,
    pub resource_roots: Vec<String>,
    pub issued_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationClaimV1 {
    pub policy_bundle_root: String,
    pub request_root: String,
    pub entity_snapshot_root: String,
    pub evaluation: CedarEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticApprovalV1 {
    pub principal_id: String,
    pub role: String,
    pub action: String,
    pub reason: String,
    pub approved_at: String,
    pub intent_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectDeltaV1 {
    pub path: String,
    pub before_root: Option<String>,
    pub after_root: Option<String>,
    pub object_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionClaimV1 {
    pub vela_version: String,
    pub binary_sha256: String,
    pub transaction_read_set_root: String,
    pub transaction_write_set_root: String,
    pub completed_at: String,
}

/// Content-addressed body. The short `record_id` is derived from these bytes;
/// the full content root is the security identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRecordContentV1 {
    pub frontier_id: String,
    pub sequence: u64,
    pub previous_authority_record_root: Option<String>,
    pub operation_id: String,
    pub transaction_id: String,
    pub intent_digest: String,
    pub before_event_log_root: String,
    pub after_event_log_root: String,
    pub event_ids: Vec<String>,
    pub object_delta: Vec<ObjectDeltaV1>,
    pub principal: PrincipalSnapshotV1,
    pub authentication: AuthenticationClaimV1,
    pub delegation: Option<DelegationClaimV1>,
    pub authorization: AuthorizationClaimV1,
    pub semantic_approvals: Vec<SemanticApprovalV1>,
    pub execution: ExecutionClaimV1,
    pub authority_keyset_root: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityRecordV1 {
    pub schema: String,
    pub record_id: String,
    pub content: AuthorityRecordContentV1,
}

impl AuthorityRecordV1 {
    pub fn new(content: AuthorityRecordContentV1) -> Result<Self, String> {
        let digest = sha256_canonical(&content)?;
        let record = Self {
            schema: AUTHORITY_RECORD_SCHEMA_V1.into(),
            record_id: format!("var_{}", &digest[..16]),
            content,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn derive_id(&self) -> Result<String, String> {
        let digest = sha256_canonical(&self.content)?;
        Ok(format!("var_{}", &digest[..16]))
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_RECORD_SCHEMA_V1 {
            return Err(format!(
                "authority record schema must be {AUTHORITY_RECORD_SCHEMA_V1}"
            ));
        }
        if self.record_id != self.derive_id()? {
            return Err("authority record ID does not match its content".into());
        }
        let content = &self.content;
        if content.frontier_id.trim().is_empty()
            || content.operation_id.trim().is_empty()
            || content.transaction_id.trim().is_empty()
            || content.event_ids.is_empty()
        {
            return Err(
                "authority record frontier, operation, transaction, and event set are required"
                    .into(),
            );
        }
        if content.sequence == 0
            || (content.sequence == 1 && content.previous_authority_record_root.is_some())
            || (content.sequence > 1 && content.previous_authority_record_root.is_none())
        {
            return Err("authority record sequence and previous root are inconsistent".into());
        }
        for (name, root) in [
            ("intent_digest", content.intent_digest.as_str()),
            (
                "before_event_log_root",
                content.before_event_log_root.as_str(),
            ),
            (
                "after_event_log_root",
                content.after_event_log_root.as_str(),
            ),
            (
                "authority_keyset_root",
                content.authority_keyset_root.as_str(),
            ),
            (
                "policy_bundle_root",
                content.authorization.policy_bundle_root.as_str(),
            ),
            ("request_root", content.authorization.request_root.as_str()),
            (
                "entity_snapshot_root",
                content.authorization.entity_snapshot_root.as_str(),
            ),
            ("binary_sha256", content.execution.binary_sha256.as_str()),
            (
                "transaction_read_set_root",
                content.execution.transaction_read_set_root.as_str(),
            ),
            (
                "transaction_write_set_root",
                content.execution.transaction_write_set_root.as_str(),
            ),
        ] {
            require_sha256(name, root)?;
        }
        if let Some(root) = &content.previous_authority_record_root {
            require_sha256("previous_authority_record_root", root)?;
        }
        let mut event_ids = BTreeSet::new();
        for event_id in &content.event_ids {
            if !event_id.starts_with("vev_") || !event_ids.insert(event_id) {
                return Err(format!("invalid or duplicate event ID {event_id}"));
            }
        }
        let mut paths = BTreeSet::new();
        for delta in &content.object_delta {
            if delta.path.is_empty() || !paths.insert(delta.path.as_str()) {
                return Err(format!(
                    "empty or duplicate object-delta path {}",
                    delta.path
                ));
            }
            if let Some(root) = &delta.before_root {
                require_sha256("object_delta.before_root", root)?;
            }
            if let Some(root) = &delta.after_root {
                require_sha256("object_delta.after_root", root)?;
            }
            if delta.before_root.is_none() && delta.after_root.is_none() {
                return Err(format!("object delta {} changes no bytes", delta.path));
            }
        }
        if content.principal.principal_id.trim().is_empty()
            || content.authentication.method.trim().is_empty()
            || content.authentication.session_id.trim().is_empty()
        {
            return Err("authority attribution and authentication must be explicit".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DsseSignatureV1 {
    pub keyid: String,
    pub sig: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityEnvelopeV1 {
    #[serde(rename = "payloadType")]
    pub payload_type: String,
    pub payload: String,
    pub signatures: Vec<DsseSignatureV1>,
}

impl AuthorityEnvelopeV1 {
    pub fn from_record(
        record: &AuthorityRecordV1,
        signatures: Vec<DsseSignatureV1>,
    ) -> Result<Self, String> {
        record.validate()?;
        Ok(Self {
            payload_type: AUTHORITY_PAYLOAD_TYPE_V1.into(),
            payload: BASE64_STANDARD.encode(to_canonical_bytes(record)?),
            signatures,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAuthorityRecord {
    pub record: AuthorityRecordV1,
    pub record_root: String,
    pub verified_key_ids: Vec<String>,
}

pub fn verify_authority_envelope(
    envelope: &AuthorityEnvelopeV1,
    keyset: &AuthorityKeysetV1,
    expected_frontier_id: &str,
    expected_sequence: u64,
    expected_previous_root: Option<&str>,
) -> Result<VerifiedAuthorityRecord, String> {
    keyset.validate()?;
    if envelope.payload_type != AUTHORITY_PAYLOAD_TYPE_V1 {
        return Err("authority envelope payload type is invalid".into());
    }
    if envelope.signatures.is_empty() {
        return Err("authority envelope has no signatures".into());
    }
    let payload = BASE64_STANDARD
        .decode(&envelope.payload)
        .map_err(|error| format!("authority envelope payload is not base64: {error}"))?;
    let record: AuthorityRecordV1 = serde_json::from_slice(&payload)
        .map_err(|error| format!("authority record JSON is invalid: {error}"))?;
    if to_canonical_bytes(&record)? != payload {
        return Err("authority record payload is not canonical JSON".into());
    }
    record.validate()?;
    if record.content.frontier_id != expected_frontier_id
        || record.content.frontier_id != keyset.frontier_id
        || record.content.sequence != expected_sequence
        || record.content.previous_authority_record_root.as_deref() != expected_previous_root
    {
        return Err("authority record frontier or chain position is invalid".into());
    }
    if record.content.authority_keyset_root != keyset.root()? {
        return Err("authority record does not bind the supplied keyset".into());
    }

    let pae = dsse_pae(&envelope.payload_type, &payload);
    let mut verified = BTreeSet::new();
    for signed in &envelope.signatures {
        if !verified.insert(signed.keyid.clone()) {
            return Err(format!("duplicate DSSE signature from {}", signed.keyid));
        }
        let key = keyset
            .keys
            .iter()
            .find(|candidate| candidate.key_id == signed.keyid)
            .ok_or_else(|| format!("unknown authority key {}", signed.keyid))?;
        if record.content.sequence < key.valid_from_sequence
            || key
                .valid_through_sequence
                .is_some_and(|through| record.content.sequence > through)
        {
            return Err(format!(
                "authority key {} is outside its sequence window",
                key.key_id
            ));
        }
        let public_key = VerifyingKey::from_bytes(&decode_fixed_hex::<32>(
            "authority public key",
            &key.public_key,
        )?)
        .map_err(|error| format!("invalid authority public key: {error}"))?;
        let signature_bytes = BASE64_STANDARD
            .decode(&signed.sig)
            .map_err(|error| format!("authority signature is not base64: {error}"))?;
        let signature = Signature::from_bytes(
            &signature_bytes
                .try_into()
                .map_err(|_| "authority signature must be exactly 64 bytes".to_string())?,
        );
        public_key
            .verify(&pae, &signature)
            .map_err(|error| format!("authority signature verification failed: {error}"))?;
    }
    if verified.len() < usize::try_from(keyset.threshold).unwrap_or(usize::MAX) {
        return Err("authority signature threshold was not met".into());
    }

    Ok(VerifiedAuthorityRecord {
        record_root: record.root()?,
        record,
        verified_key_ids: verified.into_iter().collect(),
    })
}

/// DSSE Pre-Authentication Encoding:
/// `DSSEv1 SP LEN(payloadType) SP payloadType SP LEN(payload) SP payload`.
pub fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(payload_type.len() + payload.len() + 32);
    output.extend_from_slice(b"DSSEv1 ");
    output.extend_from_slice(payload_type.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload_type.as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload);
    output
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use a full sha256: digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{name} must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn decode_fixed_hex<const N: usize>(name: &str, value: &str) -> Result<[u8; N], String> {
    hex::decode(value)
        .map_err(|error| format!("{name} is not hexadecimal: {error}"))?
        .try_into()
        .map_err(|_| format!("{name} must be exactly {N} bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn era_one_event_is_transaction_bound_and_content_addressed() {
        let event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_fixture".into(),
            principal_id: "principal:alice".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: "vpr_0123456789abcdef".into(),
            },
            actor: StateActor {
                id: "principal:alice".into(),
                r#type: "human".into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: "The evidence does not satisfy the claim.".into(),
            before_hash: root('a'),
            after_hash: root('a'),
            payload: serde_json::json!({"proposal_id": "vpr_0123456789abcdef"}),
            caveats: Vec::new(),
        })
        .unwrap();
        assert!(event.id.starts_with("vev_"));
        assert!(event.root().unwrap().starts_with("sha256:"));

        let mut tampered = event.clone();
        tampered.content.transaction_id = "txn_other".into();
        assert!(tampered.validate().is_err());
    }

    fn fixture() -> (AuthorityRecordV1, AuthorityKeysetV1, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let mut keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: "vfr_fixture".into(),
            generation: 1,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repo-key-1".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(key.verifying_key().to_bytes()),
                valid_from_sequence: 1,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: None,
            activation_record_root: None,
        };
        let content = AuthorityRecordContentV1 {
            frontier_id: "vfr_fixture".into(),
            sequence: 1,
            previous_authority_record_root: None,
            operation_id: "vop_fixture".into(),
            transaction_id: "txn_fixture".into(),
            intent_digest: root('a'),
            before_event_log_root: root('b'),
            after_event_log_root: root('c'),
            event_ids: vec!["vev_0123456789abcdef".into()],
            object_delta: vec![ObjectDeltaV1 {
                path: ".vela/events/vev_0123456789abcdef.json".into(),
                before_root: None,
                after_root: Some(root('d')),
                object_kind: "event".into(),
            }],
            principal: PrincipalSnapshotV1 {
                principal_id: "principal:alice".into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Alice".into()),
                affiliation: None,
                account_links: vec!["local:device|uid:501".into()],
            },
            authentication: AuthenticationClaimV1 {
                method: "local_os_session".into(),
                session_id: "session-1".into(),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                assurance: "local_user_session".into(),
                provider: "macos".into(),
            },
            delegation: None,
            authorization: AuthorizationClaimV1 {
                policy_bundle_root: root('e'),
                request_root: root('f'),
                entity_snapshot_root: root('1'),
                evaluation: CedarEvaluation {
                    engine: CEDAR_ENGINE.into(),
                    engine_version: CEDAR_ENGINE_VERSION.into(),
                    profile: CEDAR_PROFILE_V1.into(),
                    valid: true,
                    decision: CedarDecision::Allow,
                    automatic_permit: false,
                    determining_policies: vec!["policy0".into()],
                    diagnostics: Vec::new(),
                },
            },
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: "principal:alice".into(),
                role: "reviewer".into(),
                action: "review_reject".into(),
                reason: "Evidence does not satisfy the claim.".into(),
                approved_at: "2026-07-24T12:00:00Z".into(),
                intent_digest: root('a'),
            }],
            execution: ExecutionClaimV1 {
                vela_version: "0.930.0-rc.1".into(),
                binary_sha256: root('2'),
                transaction_read_set_root: root('3'),
                transaction_write_set_root: root('4'),
                completed_at: "2026-07-24T12:00:01Z".into(),
            },
            authority_keyset_root: String::new(),
            recorded_at: "2026-07-24T12:00:01Z".into(),
        };
        let keyset_root = keyset.root().unwrap();
        let mut content = content;
        content.authority_keyset_root = keyset_root;
        let record = AuthorityRecordV1::new(content).unwrap();
        // Keep this mutation explicit so future keyset fields cannot
        // accidentally depend on the record they activate.
        keyset.activation_record_root = None;
        (record, keyset, key)
    }

    fn signed_envelope(record: &AuthorityRecordV1, key: &SigningKey) -> AuthorityEnvelopeV1 {
        let mut envelope = AuthorityEnvelopeV1::from_record(record, Vec::new()).unwrap();
        let payload = BASE64_STANDARD.decode(&envelope.payload).unwrap();
        let signature = key.sign(&dsse_pae(&envelope.payload_type, &payload));
        envelope.signatures.push(DsseSignatureV1 {
            keyid: "repo-key-1".into(),
            sig: BASE64_STANDARD.encode(signature.to_bytes()),
        });
        envelope
    }

    #[test]
    fn authority_record_roundtrips_through_dsse() {
        let (record, keyset, key) = fixture();
        let verified = verify_authority_envelope(
            &signed_envelope(&record, &key),
            &keyset,
            "vfr_fixture",
            1,
            None,
        )
        .unwrap();
        assert_eq!(verified.record, record);
        assert_eq!(verified.verified_key_ids, vec!["repo-key-1"]);
    }

    #[test]
    fn authority_record_tampering_fails_closed() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        let mut value: serde_json::Value =
            serde_json::from_slice(&BASE64_STANDARD.decode(&envelope.payload).unwrap()).unwrap();
        value["content"]["after_event_log_root"] = serde_json::json!(root('9'));
        envelope.payload = BASE64_STANDARD.encode(to_canonical_bytes(&value).unwrap());
        assert!(verify_authority_envelope(&envelope, &keyset, "vfr_fixture", 1, None).is_err());
    }

    #[test]
    fn authority_record_rejects_wrong_type_key_and_chain_position() {
        let (record, keyset, key) = fixture();
        let envelope = signed_envelope(&record, &key);

        let mut wrong_type = envelope.clone();
        wrong_type.payload_type = "application/json".into();
        assert!(verify_authority_envelope(&wrong_type, &keyset, "vfr_fixture", 1, None).is_err());

        let wrong_key = SigningKey::from_bytes(&[8; 32]);
        assert!(
            verify_authority_envelope(
                &signed_envelope(&record, &wrong_key),
                &keyset,
                "vfr_fixture",
                1,
                None
            )
            .is_err()
        );
        assert!(
            verify_authority_envelope(&envelope, &keyset, "vfr_fixture", 2, Some(&root('9')))
                .is_err()
        );
    }

    #[test]
    fn authority_record_rejects_noncanonical_payload_and_duplicate_signer() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        let decoded = BASE64_STANDARD.decode(&envelope.payload).unwrap();
        let pretty = serde_json::to_vec_pretty(
            &serde_json::from_slice::<serde_json::Value>(&decoded).unwrap(),
        )
        .unwrap();
        envelope.payload = BASE64_STANDARD.encode(pretty);
        assert!(verify_authority_envelope(&envelope, &keyset, "vfr_fixture", 1, None).is_err());

        let mut duplicate = signed_envelope(&record, &key);
        duplicate.signatures.push(duplicate.signatures[0].clone());
        assert!(verify_authority_envelope(&duplicate, &keyset, "vfr_fixture", 1, None).is_err());
    }

    #[test]
    fn authority_key_window_and_threshold_fail_closed() {
        let (record, mut keyset, key) = fixture();
        keyset.keys[0].valid_from_sequence = 2;
        let error = verify_authority_envelope(
            &signed_envelope(&record, &key),
            &keyset,
            "vfr_fixture",
            1,
            None,
        )
        .unwrap_err();
        assert!(
            error.contains("keyset") || error.contains("sequence"),
            "{error}"
        );

        let (_, mut keyset, _) = fixture();
        keyset.threshold = 2;
        assert!(keyset.validate().is_err());
    }

    #[test]
    fn authority_record_unknown_fields_fail_to_decode() {
        let value = serde_json::json!({
            "schema": AUTHORITY_RECORD_SCHEMA_V1,
            "record_id": "var_bad",
            "content": {},
            "extra": true
        });
        assert!(serde_json::from_value::<AuthorityRecordV1>(value).is_err());
    }

    #[test]
    fn dsse_pae_matches_the_spec_shape() {
        assert_eq!(
            dsse_pae("text/plain", b"hello"),
            b"DSSEv1 10 text/plain 5 hello"
        );
    }
}
