//! Era-1 attributed repository-authority records and DSSE verification.
//!
//! These types are read/verify-only in the first migration slice. The released
//! Era-0 event and signer path remains the sole writer until shadow evaluation
//! and migration gates pass.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::authentication::{
    AuthenticationAssurance, AuthenticationMethod, AuthenticationObservationV1,
};
use crate::canonical::{from_json_slice_strict, sha256_canonical, to_canonical_bytes};
use crate::dsse::CandidateKey;
use crate::events::{
    EVENT_SCHEMA, EventKind, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
};
pub use crate::principal::PrincipalClass;

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

fn is_false(value: &bool) -> bool {
    !*value
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
    pub repository_id: String,
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
            crate::shape::require_sha256_root(name, value)?;
        }
        if let Some(root) = &self.previous_bundle_root {
            crate::shape::require_sha256_root("previous_bundle_root", root)?;
        }
        if self.repository_id.trim().is_empty() || self.authority_summary.trim().is_empty() {
            return Err("policy bundle repository and authority summary must be non-empty".into());
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

    /// Recover the transaction-independent semantic event consumed by the
    /// scientific reducer.
    ///
    /// Era-1 authority event IDs cover repository attribution, including the
    /// transaction ID. Scientific event links such as
    /// `review.accepted.payload.applied_event_id` must not depend on that
    /// transaction ID or they create a hash cycle. The reducer identity is
    /// therefore the ordinary unsigned `StateEvent` identity derived from the
    /// exact shared semantic fields. The covering authority record remains the
    /// sole authority for the resulting event bytes.
    pub fn semantic_state_event(&self) -> Result<StateEvent, String> {
        self.validate()?;
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: self.content.kind.clone(),
            target: self.content.target.clone(),
            actor: self.content.actor.clone(),
            timestamp: self.content.timestamp.clone(),
            reason: self.content.reason.clone(),
            before_hash: self.content.before_hash.clone(),
            after_hash: self.content.after_hash.clone(),
            payload: self.content.payload.clone(),
            caveats: self.content.caveats.clone(),
            signature: None,
        };
        event.id = compute_event_id(&event);
        Ok(event)
    }

    pub fn semantic_event_id(&self) -> Result<String, String> {
        Ok(self.semantic_state_event()?.id)
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
        require_state_root("before_hash", &self.content.before_hash)?;
        require_state_root("after_hash", &self.content.after_hash)?;
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
    pub repository_id: String,
    pub generation: u64,
    pub threshold: u32,
    pub keys: Vec<AuthorityKeyV1>,
    pub previous_keyset_root: Option<String>,
    pub activation_record_root: Option<String>,
    /// A terminal successor keyset closes future repository authority. The
    /// field is omitted for every historical/open v1 keyset, preserving its
    /// canonical bytes and root.
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
}

impl AuthorityKeysetV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_KEYSET_SCHEMA_V1 {
            return Err(format!(
                "authority keyset schema must be {AUTHORITY_KEYSET_SCHEMA_V1}"
            ));
        }
        if self.repository_id.trim().is_empty() || self.generation == 0 {
            return Err("authority keyset repository and generation must be set".into());
        }
        if self.closed {
            if !self.keys.is_empty()
                || self.threshold != 0
                || self.previous_keyset_root.is_none()
                || self.activation_record_root.is_none()
            {
                return Err(
                    "closed authority keyset must be an empty terminal successor generation".into(),
                );
            }
        } else if self.keys.is_empty()
            || self.threshold == 0
            || usize::try_from(self.threshold).unwrap_or(usize::MAX) > self.keys.len()
        {
            return Err("authority keyset threshold must be within the key count".into());
        }
        let mut key_ids = BTreeSet::new();
        let mut public_keys = BTreeSet::new();
        for key in &self.keys {
            if !key_ids.insert(key.key_id.as_str()) {
                return Err(format!("duplicate authority key ID {}", key.key_id));
            }
            let public_key = decode_fixed_hex::<32>("authority public key", &key.public_key)?;
            if !public_keys.insert(public_key) {
                return Err(format!(
                    "authority key {} duplicates public-key material",
                    key.key_id
                ));
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
        }
        if let Some(root) = &self.previous_keyset_root {
            crate::shape::require_sha256_root("previous_keyset_root", root)?;
        }
        if let Some(root) = &self.activation_record_root {
            crate::shape::require_sha256_root("activation_record_root", root)?;
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

/// Verify a non-cyclic repository-authority keyset rotation.
///
/// The new keyset links the prior keyset root and the authority-record root
/// that existed immediately before the rotation transaction. The rotation
/// record can then cover the new keyset root without requiring either object
/// to contain the other's root. The new keyset becomes usable only on the
/// following authority-record sequence.
pub fn verify_authority_keyset_transition(
    current: &AuthorityKeysetV1,
    next: &AuthorityKeysetV1,
    activation_sequence: u64,
    previous_authority_record_root: &str,
) -> Result<(), String> {
    current.validate()?;
    next.validate()?;
    crate::shape::require_sha256_root(
        "previous_authority_record_root",
        previous_authority_record_root,
    )?;
    if current.closed {
        return Err("closed repository authority cannot transition again".into());
    }
    if activation_sequence <= 1 {
        return Err("rotated authority keyset cannot activate at sequence 1".into());
    }
    if current.repository_id != next.repository_id
        || next.generation != current.generation.saturating_add(1)
        || next.previous_keyset_root.as_deref() != Some(current.root()?.as_str())
        || next.activation_record_root.as_deref() != Some(previous_authority_record_root)
    {
        return Err(
            "rotated authority keyset does not extend the exact prior generation and chain head"
                .into(),
        );
    }
    if !next.closed {
        let active_keys = next
            .keys
            .iter()
            .filter(|key| {
                key.valid_from_sequence <= activation_sequence
                    && key
                        .valid_through_sequence
                        .is_none_or(|through| activation_sequence <= through)
            })
            .count();
        if active_keys < usize::try_from(next.threshold).unwrap_or(usize::MAX) {
            return Err("rotated authority keyset cannot meet threshold at activation".into());
        }
    }
    Ok(())
}

/// Verify a content-addressed policy-bundle rotation.
///
/// Activation is recorded by the covering authority transaction, so the
/// bundle needs only the exact prior bundle root and introduces no record-root
/// hash cycle.
pub fn verify_policy_bundle_transition(
    current: &PolicyBundleV1,
    next: &PolicyBundleV1,
) -> Result<(), String> {
    current.validate()?;
    next.validate()?;
    if current.repository_id != next.repository_id
        || next.previous_bundle_root.as_deref() != Some(current.root()?.as_str())
    {
        return Err("rotated policy bundle does not extend the exact prior bundle".into());
    }
    Ok(())
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

pub type AuthenticationClaimV1 = AuthenticationObservationV1;

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
    pub repository_id: String,
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
    /// Reserved serialized slot retained so existing `delegation: null`
    /// authority records preserve their exact canonical bytes and roots.
    pub delegation: Option<Value>,
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
        if content.repository_id.trim().is_empty()
            || content.operation_id.trim().is_empty()
            || content.transaction_id.trim().is_empty()
            || (content.event_ids.is_empty() && content.object_delta.is_empty())
        {
            return Err(
                "authority record repository, operation, transaction, and a changed event or object are required"
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
            crate::shape::require_sha256_root(name, root)?;
        }
        if let Some(root) = &content.previous_authority_record_root {
            crate::shape::require_sha256_root("previous_authority_record_root", root)?;
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
                crate::shape::require_sha256_root("object_delta.before_root", root)?;
            }
            if let Some(root) = &delta.after_root {
                crate::shape::require_sha256_root("object_delta.after_root", root)?;
            }
            if delta.before_root == delta.after_root {
                return Err(format!("object delta {} changes no bytes", delta.path));
            }
        }
        content.authentication.validate()?;
        if content.principal.principal_id.trim().is_empty()
            || content.principal.principal_id != content.authentication.principal_id
            || content.principal.principal_class != content.authentication.principal_class
            || !content
                .principal
                .account_links
                .contains(&content.principal.principal_id)
        {
            return Err("authority attribution and authentication must be explicit".into());
        }
        if content.delegation.is_some() {
            return Err("authority-record delegation is unsupported".into());
        }
        Ok(())
    }
}

/// One DSSE signature entry, shared with every other signed Vela object.
pub use crate::dsse::SignatureV1 as DsseSignatureV1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("additionalProperties" = true))]
pub struct AuthorityEnvelopeV1 {
    #[serde(rename = "payloadType")]
    #[schemars(schema_with = "crate::wire_schema::authority_payload_type_tag")]
    pub payload_type: String,
    #[schemars(schema_with = "crate::wire_schema::base64_body")]
    pub payload: String,
    // `verify_authority_envelope` refuses an empty list before it reads a key.
    #[schemars(length(min = 1))]
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
            payload: crate::dsse::encode_base64(&to_canonical_bytes(record)?),
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
    expected_repository_id: &str,
    expected_sequence: u64,
    expected_previous_root: Option<&str>,
) -> Result<VerifiedAuthorityRecord, String> {
    keyset.validate()?;
    if envelope.payload_type != AUTHORITY_PAYLOAD_TYPE_V1 {
        return Err("authority envelope payload type is invalid".into());
    }

    // Key selection is authority policy, so it happens here: only keys whose
    // validity window covers this record's sequence are offered to DSSE.
    let candidates: Vec<CandidateKey> = keyset
        .keys
        .iter()
        .filter(|key| {
            expected_sequence >= key.valid_from_sequence
                && !key
                    .valid_through_sequence
                    .is_some_and(|through| expected_sequence > through)
        })
        .filter_map(|key| CandidateKey::from_hex(&key.key_id, &key.public_key))
        .collect();
    let verified = crate::dsse::verify(
        "authority envelope",
        &envelope.payload_type,
        &envelope.payload,
        &envelope.signatures,
        &candidates,
        usize::try_from(keyset.threshold).unwrap_or(usize::MAX),
    )?;
    let payload = verified.payload;

    let record: AuthorityRecordV1 = from_json_slice_strict(&payload)
        .map_err(|error| format!("authority record JSON is invalid: {error}"))?;
    if to_canonical_bytes(&record)? != payload {
        return Err("authority record payload is not canonical JSON".into());
    }
    record.validate()?;
    if record.content.repository_id != expected_repository_id
        || record.content.repository_id != keyset.repository_id
        || record.content.sequence != expected_sequence
        || record.content.previous_authority_record_root.as_deref() != expected_previous_root
    {
        return Err("authority record repository or chain position is invalid".into());
    }
    if record.content.authority_keyset_root != keyset.root()? {
        return Err("authority record does not bind the supplied keyset".into());
    }

    Ok(VerifiedAuthorityRecord {
        record_root: record.root()?,
        record,
        verified_key_ids: verified.verified_key_ids,
    })
}

fn require_state_root(name: &str, value: &str) -> Result<(), String> {
    if value == NULL_HASH {
        return Ok(());
    }
    crate::shape::require_sha256_root(name, value)
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
    use base64::Engine as _;
    use base64::engine::general_purpose::{
        STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
    };
    use ed25519_dalek::{Signer, SigningKey};

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn era_one_event_is_transaction_bound_and_content_addressed() {
        let event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_fixture".into(),
            principal_id: "local:device-1|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: "vpr_0123456789abcdef".into(),
            },
            actor: StateActor {
                id: "local:device-1|uid:501".into(),
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

    #[test]
    fn era_one_event_recovers_transaction_independent_reducer_identity() {
        let content = AuthorityEventContentV1 {
            transaction_id: "vtx_first".into(),
            principal_id: "local:device-1|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::ClaimNoted,
            target: StateTarget {
                r#type: "finding".into(),
                id: "vf_0123456789abcdef".into(),
            },
            actor: StateActor {
                id: "local:device-1|uid:501".into(),
                r#type: "human".into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: "Retain the exact scientific annotation.".into(),
            before_hash: root('a'),
            after_hash: root('b'),
            payload: serde_json::json!({"annotation": "bounded"}),
            caveats: vec!["scope remains exact".into()],
        };
        let first = AuthorityEventV1::new(content.clone()).unwrap();
        let second = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "vtx_second".into(),
            ..content
        })
        .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(
            first.semantic_event_id().unwrap(),
            second.semantic_event_id().unwrap()
        );
        let semantic = first.semantic_state_event().unwrap();
        assert_eq!(semantic.id, first.semantic_event_id().unwrap());
        assert_eq!(semantic.kind, first.content.kind);
        assert!(semantic.signature.is_none());
    }

    #[test]
    fn era_one_non_scientific_event_accepts_the_protocol_null_root() {
        let event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_lease_fixture".into(),
            principal_id: "agent:fixture".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::Other("work.claimed".into()),
            target: StateTarget {
                r#type: "target".into(),
                id: "erdos:124".into(),
            },
            actor: StateActor {
                id: "agent:fixture".into(),
                r#type: "agent".into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: "Claim one exact bounded work target.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: serde_json::json!({"target_id": "erdos:124"}),
            caveats: Vec::new(),
        })
        .unwrap();
        assert_eq!(event.content.before_hash, NULL_HASH);
        assert_eq!(event.content.after_hash, NULL_HASH);
    }

    fn fixture() -> (AuthorityRecordV1, AuthorityKeysetV1, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let mut keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            repository_id: "vrepo_fixture".into(),
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
            closed: false,
        };
        let content = AuthorityRecordContentV1 {
            repository_id: "vrepo_fixture".into(),
            sequence: 1,
            previous_authority_record_root: None,
            operation_id: "vop_fixture".into(),
            transaction_id: "txn_fixture".into(),
            intent_digest: root('a'),
            before_event_log_root: root('b'),
            after_event_log_root: root('c'),
            event_ids: vec!["vev_0123456789abcdef".into()],
            object_delta: vec![ObjectDeltaV1 {
                path: ".vela/authority/events/vev_0123456789abcdef.json".into(),
                before_root: None,
                after_root: Some(root('d')),
                object_kind: "event".into(),
            }],
            principal: PrincipalSnapshotV1 {
                principal_id: "local:device-1|uid:501".into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Alice".into()),
                affiliation: None,
                account_links: vec!["local:device-1|uid:501".into()],
            },
            authentication: AuthenticationClaimV1 {
                schema: crate::authentication::AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
                principal_id: "local:device-1|uid:501".into(),
                principal_class: PrincipalClass::Human,
                issuer: "device-1".into(),
                subject: "uid:501".into(),
                method: AuthenticationMethod::LocalOsSession,
                assurance: AuthenticationAssurance::LocalSession,
                session_root: root('9'),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                observed_at: "2026-07-24T12:00:00Z".into(),
                expires_at: "2026-07-24T13:00:00Z".into(),
                user_presence: false,
                user_verification: false,
                recovery_recent: false,
                revocation_ref: None,
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
                principal_id: "local:device-1|uid:501".into(),
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
        let signature = key.sign(&crate::dsse::pae(&envelope.payload_type, &payload));
        envelope.signatures.push(DsseSignatureV1 {
            keyid: "repo-key-1".into(),
            sig: BASE64_STANDARD.encode(signature.to_bytes()),
        });
        envelope
    }

    #[test]
    fn authority_record_roundtrips_through_dsse() {
        let (record, keyset, key) = fixture();
        let envelope = signed_envelope(&record, &key);
        assert_eq!(
            envelope.payload,
            BASE64_STANDARD.encode(to_canonical_bytes(&record).unwrap())
        );
        let verified =
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).unwrap();
        assert_eq!(verified.record, record);
        assert_eq!(verified.verified_key_ids, vec!["repo-key-1"]);
    }

    #[test]
    fn authority_record_payload_rejects_nested_duplicate_properties() {
        let (record, keyset, key) = fixture();
        let canonical = String::from_utf8(to_canonical_bytes(&record).unwrap()).unwrap();
        let payload = canonical
            .replace(
                r#""delegation":null"#,
                r#""delegation":{"scope":"one","scope":"two"}"#,
            )
            .into_bytes();
        assert_ne!(payload, canonical.as_bytes());
        let signature = key.sign(&crate::dsse::pae(AUTHORITY_PAYLOAD_TYPE_V1, &payload));
        let envelope = AuthorityEnvelopeV1 {
            payload_type: AUTHORITY_PAYLOAD_TYPE_V1.into(),
            payload: BASE64_STANDARD.encode(&payload),
            signatures: vec![DsseSignatureV1 {
                keyid: "repo-key-1".into(),
                sig: BASE64_STANDARD.encode(signature.to_bytes()),
            }],
        };

        let error =
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).unwrap_err();
        assert!(error.contains("duplicate JSON property `scope`"), "{error}");
    }

    #[test]
    fn authority_record_accepts_dsse_base64_variants() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        envelope.payload =
            BASE64_URL_SAFE_NO_PAD.encode(BASE64_STANDARD.decode(&envelope.payload).unwrap());
        envelope.signatures[0].sig = BASE64_URL_SAFE_NO_PAD
            .encode(BASE64_STANDARD.decode(&envelope.signatures[0].sig).unwrap());

        let verified =
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).unwrap();
        assert_eq!(verified.record, record);
        assert_eq!(verified.verified_key_ids, vec!["repo-key-1"]);
    }

    #[test]
    fn authority_record_accepts_dsse_extensions_and_missing_keyid() {
        let (record, keyset, key) = fixture();
        let mut value = serde_json::to_value(signed_envelope(&record, &key)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("extension".into(), serde_json::json!({"retained": false}));
        let signature = value["signatures"][0].as_object_mut().unwrap();
        signature.remove("keyid");
        signature.insert("extension".into(), serde_json::json!("ignored"));
        let envelope: AuthorityEnvelopeV1 = serde_json::from_value(value).unwrap();

        let verified =
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).unwrap();
        assert_eq!(verified.record, record);
        assert_eq!(verified.verified_key_ids, vec!["repo-key-1"]);
    }

    #[test]
    fn authority_record_skips_invalid_and_unknown_dsse_signatures() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        envelope.signatures.insert(
            0,
            DsseSignatureV1 {
                keyid: "unknown-key".into(),
                sig: "not base64".into(),
            },
        );

        let verified =
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).unwrap();
        assert_eq!(verified.record, record);
        assert_eq!(verified.verified_key_ids, vec!["repo-key-1"]);
    }

    #[test]
    fn authority_record_rejects_object_delta_with_identical_roots() {
        let (record, _, _) = fixture();
        let mut content = record.content;
        content.object_delta[0].before_root = content.object_delta[0].after_root.clone();
        let error = AuthorityRecordV1::new(content).unwrap_err();
        assert!(error.contains("changes no bytes"), "{error}");
    }

    #[test]
    fn authority_record_preserves_null_delegation_and_rejects_non_null() {
        let (record, _, _) = fixture();
        let canonical = to_canonical_bytes(&record).unwrap();
        assert!(
            canonical
                .windows(b"\"delegation\":null".len())
                .any(|window| window == b"\"delegation\":null")
        );
        let roundtrip: AuthorityRecordV1 = serde_json::from_slice(&canonical).unwrap();
        assert_eq!(roundtrip.root().unwrap(), record.root().unwrap());

        let mut content = record.content;
        content.delegation = Some(serde_json::json!({"unsupported": true}));
        let error = AuthorityRecordV1::new(content).unwrap_err();
        assert!(error.contains("delegation is unsupported"), "{error}");
    }

    #[test]
    fn authority_record_tampering_fails_closed() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        let mut value: serde_json::Value =
            serde_json::from_slice(&BASE64_STANDARD.decode(&envelope.payload).unwrap()).unwrap();
        value["content"]["after_event_log_root"] = serde_json::json!(root('9'));
        envelope.payload = BASE64_STANDARD.encode(to_canonical_bytes(&value).unwrap());
        assert!(verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).is_err());
    }

    #[test]
    fn authority_record_rejects_authentication_identity_and_bearer_material() {
        let (record, _, _) = fixture();
        let mut mismatched = record.content.clone();
        mismatched.authentication.principal_id = "local:other-device|uid:501".into();
        assert!(AuthorityRecordV1::new(mismatched).is_err());

        let mut value = serde_json::to_value(&record).unwrap();
        value["content"]["authentication"]["bearer_token"] =
            serde_json::json!("must-not-enter-history");
        assert!(serde_json::from_value::<AuthorityRecordV1>(value).is_err());
    }

    #[test]
    fn authority_record_rejects_wrong_type_key_and_chain_position() {
        let (record, keyset, key) = fixture();
        let envelope = signed_envelope(&record, &key);

        let mut wrong_type = envelope.clone();
        wrong_type.payload_type = "application/json".into();
        assert!(verify_authority_envelope(&wrong_type, &keyset, "vrepo_fixture", 1, None).is_err());

        let wrong_key = SigningKey::from_bytes(&[8; 32]);
        assert!(
            verify_authority_envelope(
                &signed_envelope(&record, &wrong_key),
                &keyset,
                "vrepo_fixture",
                1,
                None
            )
            .is_err()
        );
        assert!(
            verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 2, Some(&root('9')))
                .is_err()
        );
    }

    #[test]
    fn authority_record_rejects_noncanonical_payload_and_does_not_double_count_signer() {
        let (record, keyset, key) = fixture();
        let mut envelope = signed_envelope(&record, &key);
        let decoded = BASE64_STANDARD.decode(&envelope.payload).unwrap();
        let pretty = serde_json::to_vec_pretty(
            &serde_json::from_slice::<serde_json::Value>(&decoded).unwrap(),
        )
        .unwrap();
        envelope.payload = BASE64_STANDARD.encode(pretty);
        assert!(verify_authority_envelope(&envelope, &keyset, "vrepo_fixture", 1, None).is_err());

        let second_key = SigningKey::from_bytes(&[8; 32]);
        let mut threshold_keyset = keyset;
        threshold_keyset.keys.push(AuthorityKeyV1 {
            key_id: "repo-key-2".into(),
            algorithm: AUTHORITY_KEY_ALGORITHM.into(),
            public_key: hex::encode(second_key.verifying_key().as_bytes()),
            valid_from_sequence: 1,
            valid_through_sequence: None,
            purpose: AUTHORITY_KEY_PURPOSE.into(),
        });
        threshold_keyset.threshold = 2;
        let mut content = record.content;
        content.authority_keyset_root = threshold_keyset.root().unwrap();
        let threshold_record = AuthorityRecordV1::new(content).unwrap();
        let mut duplicate = signed_envelope(&threshold_record, &key);
        duplicate.signatures.push(duplicate.signatures[0].clone());
        let error =
            verify_authority_envelope(&duplicate, &threshold_keyset, "vrepo_fixture", 1, None)
                .unwrap_err();
        assert!(error.contains("threshold"), "{error}");
    }

    #[test]
    fn authority_key_window_and_threshold_fail_closed() {
        let (record, mut keyset, key) = fixture();
        keyset.keys[0].valid_from_sequence = 2;
        let error = verify_authority_envelope(
            &signed_envelope(&record, &key),
            &keyset,
            "vrepo_fixture",
            1,
            None,
        )
        .unwrap_err();
        assert!(error.contains("threshold"), "{error}");

        let (_, mut keyset, _) = fixture();
        keyset.threshold = 2;
        assert!(keyset.validate().is_err());
    }

    #[test]
    fn authority_keyset_rejects_duplicate_public_key_material() {
        let (_, mut keyset, _) = fixture();
        let duplicate = AuthorityKeyV1 {
            key_id: "repo-key-alias".into(),
            ..keyset.keys[0].clone()
        };
        keyset.keys.push(duplicate);
        keyset.threshold = 2;
        assert!(
            keyset
                .validate()
                .unwrap_err()
                .contains("duplicates public-key material")
        );

        keyset.keys[1].public_key = keyset.keys[1].public_key.to_uppercase();
        assert!(
            keyset
                .validate()
                .unwrap_err()
                .contains("duplicates public-key material")
        );
    }

    #[test]
    fn keyset_rotation_is_non_cyclic_and_exact() {
        let (_, current, _) = fixture();
        let next_key = SigningKey::from_bytes(&[9; 32]);
        let previous_record_root = root('9');
        let mut next = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            repository_id: current.repository_id.clone(),
            generation: 2,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repo-key-2".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(next_key.verifying_key().to_bytes()),
                valid_from_sequence: 3,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(current.root().unwrap()),
            activation_record_root: Some(previous_record_root.clone()),
            closed: false,
        };
        verify_authority_keyset_transition(&current, &next, 3, &previous_record_root).unwrap();

        next.generation = 3;
        assert!(
            verify_authority_keyset_transition(&current, &next, 3, &previous_record_root)
                .unwrap_err()
                .contains("exact prior generation")
        );
        next.generation = 2;
        next.keys[0].valid_from_sequence = 4;
        assert!(
            verify_authority_keyset_transition(&current, &next, 3, &previous_record_root)
                .unwrap_err()
                .contains("cannot meet threshold")
        );
        next.keys[0].valid_from_sequence = 3;
        next.activation_record_root = Some(root('8'));
        assert!(
            verify_authority_keyset_transition(&current, &next, 3, &previous_record_root)
                .unwrap_err()
                .contains("chain head")
        );

        let closed = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            repository_id: current.repository_id.clone(),
            generation: 2,
            threshold: 0,
            keys: Vec::new(),
            previous_keyset_root: Some(current.root().unwrap()),
            activation_record_root: Some(previous_record_root.clone()),
            closed: true,
        };
        verify_authority_keyset_transition(&current, &closed, 3, &previous_record_root).unwrap();

        let mut invalid_closed = closed.clone();
        invalid_closed.threshold = 1;
        assert!(
            invalid_closed
                .validate()
                .unwrap_err()
                .contains("empty terminal successor")
        );
        assert!(
            verify_authority_keyset_transition(&closed, &next, 4, &previous_record_root)
                .unwrap_err()
                .contains("cannot transition again")
        );
    }

    #[test]
    fn policy_bundle_rotation_extends_one_exact_root() {
        let current = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            repository_id: "vrepo_fixture".into(),
            cedar_schema_root: root('1'),
            policies_root: root('2'),
            entities_root: root('3'),
            tests_root: root('4'),
            engine: CEDAR_ENGINE.into(),
            engine_version: CEDAR_ENGINE_VERSION.into(),
            restricted_profile: CEDAR_PROFILE_V1.into(),
            previous_bundle_root: None,
            authority_summary: "Initial repository authority.".into(),
        };
        let mut next = PolicyBundleV1 {
            policies_root: root('5'),
            tests_root: root('6'),
            previous_bundle_root: Some(current.root().unwrap()),
            authority_summary: "Rotated repository authority.".into(),
            ..current.clone()
        };
        verify_policy_bundle_transition(&current, &next).unwrap();

        next.previous_bundle_root = Some(root('7'));
        assert!(
            verify_policy_bundle_transition(&current, &next)
                .unwrap_err()
                .contains("exact prior bundle")
        );
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
            crate::dsse::pae("text/plain", b"hello"),
            b"DSSEv1 10 text/plain 5 hello"
        );
    }
}
