//! Scoped verifier output: `vela.verification-record.v2`, in a DSSE envelope.
//!
//! Verification is an authenticated observation over exact inputs. Even a
//! passing record changes no Claim Standing without a separate authorized
//! Decision and canonical Event.
//!
//! v2 carries the same observation as v1 under the shared envelope: the
//! signature is the envelope's, `verification_record_id` is derived from the
//! retained envelope root, and the verifier's key is declared by the payload
//! the signature covers.

use ed25519_dalek::SigningKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::artifact_reference::require_artifact_reference_id;
use crate::dsse::EnvelopeV1;
use crate::signer_identity::SignerIdentityV1;

pub const VERIFICATION_RECORD_V2_SCHEMA: &str = "vela.verification-record.v2";
pub const VERIFICATION_RECORD_V2_PAYLOAD_TYPE: &str =
    "application/vnd.vela.verification-record.v2+json";
pub const VERIFICATION_RECORD_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const VERIFICATION_RECORD_HANDLE_PREFIX: &str = "vvr_";

/// What a Verification Record may report about the property it checked.
///
/// Read by `validate_semantics` and by `crate::wire_schema`, so the vocabulary
/// is written once. There is deliberately no member meaning "accepted": a
/// passing Verification is an observation, and Standing moves only by an
/// authorized Decision and its canonical Event.
pub const VERIFICATION_OUTCOMES: &[&str] = &["pass", "fail", "error", "inconclusive"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationSubject {
    #[schemars(schema_with = "crate::wire_schema::claim_id")]
    pub claim_id: String,
    #[schemars(schema_with = "crate::wire_schema::artifact_reference_id_array")]
    pub artifact_ids: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::submission_id_reference")]
    pub submission_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub submission_root: String,
    #[schemars(schema_with = "crate::wire_schema::proposal_id_reference")]
    pub proposal_id: String,
    /// The full root `proposal_id` is a prefix of.
    ///
    /// v1 named the Proposal by handle alone, which left nothing for a reader
    /// to check the handle against — the one reference in the object that
    /// could not be re-derived.
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub proposal_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationMethod {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub profile: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub implementation: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub environment_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationScope {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub property: String,
    #[schemars(schema_with = "crate::wire_schema::nonempty_text_array")]
    pub does_not_establish: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndependenceDisclosure {
    #[schemars(schema_with = "crate::wire_schema::text_array")]
    pub declared_independent_of: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::text_array")]
    pub shared_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationRecordV2 {
    #[schemars(schema_with = "crate::wire_schema::verification_record_schema_tag")]
    pub schema: String,
    pub identity: SignerIdentityV1,
    pub subject: VerificationSubject,
    pub method: VerificationMethod,
    pub scope: VerificationScope,
    #[schemars(schema_with = "crate::wire_schema::verification_outcome")]
    pub outcome: String,
    pub independence: IndependenceDisclosure,
    #[schemars(schema_with = "crate::wire_schema::artifact_reference_id_array")]
    pub output_artifact_ids: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub started_at: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub completed_at: String,
}

impl VerificationRecordV2 {
    /// Who performed the check.
    ///
    /// v1 carried this twice — a `verifier` field and the identity binding's
    /// `actor_id`, required to be equal — so one of them was always about to
    /// be the one a reader trusted. The signed identity is the one that means
    /// something, and this is the name for reading it.
    pub fn verifier(&self) -> &str {
        &self.identity.actor_id
    }
}

#[derive(Debug, Clone)]
pub struct VerificationRecordDraft {
    pub subject: VerificationSubject,
    pub method: VerificationMethod,
    pub scope: VerificationScope,
    pub outcome: String,
    pub independence: IndependenceDisclosure,
    pub output_artifact_ids: Vec<String>,
    pub started_at: String,
    pub completed_at: String,
}

/// One retained Verification Record: the envelope as stored, and its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRecordEnvelopeV2 {
    pub envelope: EnvelopeV1,
    pub record: VerificationRecordV2,
    /// Canonical bytes of the envelope: exactly what is written to disk.
    pub bytes: Vec<u8>,
    /// `sha256:` over [`Self::bytes`].
    pub root: String,
    /// `vvr_` plus the first sixteen hexadecimal characters of [`Self::root`].
    pub id: String,
}

impl VerificationRecordEnvelopeV2 {
    pub fn seal(
        draft: VerificationRecordDraft,
        identity: SignerIdentityV1,
        key: &SigningKey,
    ) -> Result<Self, String> {
        identity.validate()?;
        if identity.public_key_hex != hex::encode(key.verifying_key().to_bytes()) {
            return Err(
                "Verification Record signing key does not match its declared identity".into(),
            );
        }
        let record = VerificationRecordV2 {
            schema: VERIFICATION_RECORD_V2_SCHEMA.to_string(),
            identity,
            subject: draft.subject,
            method: draft.method,
            scope: draft.scope,
            outcome: draft.outcome,
            independence: draft.independence,
            output_artifact_ids: draft.output_artifact_ids,
            started_at: draft.started_at,
            completed_at: draft.completed_at,
        };
        record.validate_semantics()?;
        let payload = crate::canonical::to_canonical_bytes(&record)?;
        let envelope = EnvelopeV1::seal_single(key, VERIFICATION_RECORD_V2_PAYLOAD_TYPE, &payload);
        Self::from_envelope(envelope)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let envelope =
            EnvelopeV1::parse("Verification Record", bytes, VERIFICATION_RECORD_MAX_BYTES)?;
        Self::from_envelope(envelope)
    }

    pub fn from_envelope(envelope: EnvelopeV1) -> Result<Self, String> {
        let declared = crate::signer_identity::declared_public_key(
            "Verification Record",
            &crate::dsse::decode_base64("Verification Record payload", &envelope.payload)?,
        )?;
        let payload = envelope.open_single(
            "Verification Record",
            VERIFICATION_RECORD_V2_PAYLOAD_TYPE,
            &declared,
        )?;
        let record: VerificationRecordV2 = crate::canonical::from_json_slice_strict(&payload)
            .map_err(|error| format!("parse Verification Record v2: {error}"))?;
        if crate::canonical::to_canonical_bytes(&record)? != payload {
            return Err("Verification Record payload is not canonical JSON".into());
        }
        record.validate_semantics()?;

        let bytes = envelope.canonical_bytes()?;
        let root = crate::canonical::sha256_root(&bytes);
        let id = crate::shape::derive_handle(VERIFICATION_RECORD_HANDLE_PREFIX, &root)?;
        Ok(Self {
            envelope,
            record,
            bytes,
            root,
            id,
        })
    }
}

impl VerificationRecordV2 {
    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != VERIFICATION_RECORD_V2_SCHEMA {
            return Err(format!(
                "Verification Record schema must be `{VERIFICATION_RECORD_V2_SCHEMA}`"
            ));
        }
        self.identity.validate()?;
        require_prefixed_hex("subject.claim_id", &self.subject.claim_id, "vcl_", 64)?;
        require_sha256("subject.submission_root", &self.subject.submission_root)?;
        require_sha256("subject.proposal_root", &self.subject.proposal_root)?;
        crate::shape::require_derived_handle(
            "Verification Record subject.submission_id",
            &self.subject.submission_id,
            "vsb_",
            &self.subject.submission_root,
        )?;
        crate::shape::require_derived_handle(
            "Verification Record subject.proposal_id",
            &self.subject.proposal_id,
            "vpr_",
            &self.subject.proposal_root,
        )?;
        for artifact_id in self
            .subject
            .artifact_ids
            .iter()
            .chain(self.output_artifact_ids.iter())
        {
            require_artifact_reference_id("Verification Record", "artifact id", artifact_id)?;
        }
        require_text("method.profile", &self.method.profile)?;
        require_text("method.implementation", &self.method.implementation)?;
        require_sha256("method.environment_root", &self.method.environment_root)?;
        require_text("scope.property", &self.scope.property)?;
        if self.scope.does_not_establish.is_empty() {
            return Err(
                "Verification Record scope must state at least one limitation or explicit nonclaim"
                    .into(),
            );
        }
        for limitation in &self.scope.does_not_establish {
            require_text("scope.does_not_establish", limitation)?;
        }
        if !VERIFICATION_OUTCOMES.contains(&self.outcome.as_str()) {
            return Err(
                "Verification Record outcome must be pass, fail, error, or inconclusive".into(),
            );
        }
        for actor in &self.independence.declared_independent_of {
            require_text("independence.declared_independent_of", actor)?;
            if actor == self.verifier() {
                return Err("Verification Record cannot claim independence from itself".into());
            }
        }
        for dependency in &self.independence.shared_dependencies {
            require_text("independence.shared_dependencies", dependency)?;
        }
        let started = chrono::DateTime::parse_from_rfc3339(&self.started_at)
            .map_err(|_| "Verification Record started_at must be RFC 3339".to_string())?;
        let completed = chrono::DateTime::parse_from_rfc3339(&self.completed_at)
            .map_err(|_| "Verification Record completed_at must be RFC 3339".to_string())?;
        if completed < started {
            return Err("Verification Record completed_at precedes started_at".into());
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Verification Record {field} must be non-empty, trimmed text"
        ));
    }
    if value.len() > crate::wire_schema::TEXT_MAX_BYTES {
        return Err(format!("Verification Record {field} exceeds 16 KiB"));
    }
    Ok(())
}

fn require_prefixed_hex(
    field: &str,
    value: &str,
    prefix: &str,
    hex_len: usize,
) -> Result<(), String> {
    let Some(hex) = value.strip_prefix(prefix) else {
        return Err(format!(
            "Verification Record {field} must begin with `{prefix}`"
        ));
    };
    if hex.len() != hex_len || !hex.bytes().all(crate::shape::is_lower_hex) {
        return Err(format!(
            "Verification Record {field} must contain exactly {hex_len} hexadecimal characters after `{prefix}`"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "Verification Record {field} must be a full sha256: digest"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer_identity::ActorClass;
    use rand_core::OsRng;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn handle(prefix: &str, byte: char) -> String {
        crate::shape::derive_handle(prefix, &root(byte)).unwrap()
    }

    fn fixture() -> (VerificationRecordDraft, SignerIdentityV1, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let identity = SignerIdentityV1::new(
            "service:fixture-verifier",
            ActorClass::Org,
            &key,
            "2026-07-26T00:00:00Z",
        )
        .unwrap();
        let draft = VerificationRecordDraft {
            subject: VerificationSubject {
                claim_id: format!("vcl_{}", "c".repeat(64)),
                artifact_ids: vec!["a".repeat(64)],
                submission_id: handle("vsb_", 'a'),
                submission_root: root('a'),
                proposal_id: handle("vpr_", 'd'),
                proposal_root: root('d'),
            },
            method: VerificationMethod {
                profile: "fixture-v1".into(),
                implementation: "oci://fixture@sha256:abc".into(),
                environment_root: root('b'),
            },
            scope: VerificationScope {
                property: "The witness satisfies the bounded condition.".into(),
                does_not_establish: vec!["Scientific acceptance.".into()],
            },
            outcome: "pass".into(),
            independence: IndependenceDisclosure {
                declared_independent_of: vec!["agent:fixture".into()],
                shared_dependencies: vec!["problem specification v1".into()],
            },
            output_artifact_ids: vec!["b".repeat(64)],
            started_at: "2026-07-26T00:00:00Z".into(),
            completed_at: "2026-07-26T00:00:01Z".into(),
        };
        (draft, identity, key)
    }

    #[test]
    fn a_sealed_record_round_trips_and_changes_no_standing() {
        let (draft, identity, key) = fixture();
        let sealed = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap();

        assert_eq!(
            sealed.id,
            crate::shape::derive_handle("vvr_", &sealed.root).unwrap()
        );
        assert_eq!(
            VerificationRecordEnvelopeV2::parse(&sealed.bytes).unwrap(),
            sealed
        );

        let value = serde_json::to_value(&sealed.record).unwrap();
        assert!(value.get("standing").is_none());
        assert!(value.get("accepted").is_none());
    }

    #[test]
    fn subject_drift_breaks_the_envelope_signature() {
        let (draft, identity, key) = fixture();
        let sealed = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap();

        let mut drifted = sealed.record.clone();
        drifted.subject.submission_root = root('c');
        let mut envelope = sealed.envelope.clone();
        envelope.payload =
            crate::dsse::encode_base64(&crate::canonical::to_canonical_bytes(&drifted).unwrap());
        assert!(VerificationRecordEnvelopeV2::from_envelope(envelope).is_err());
    }

    #[test]
    fn historical_finding_ids_are_not_current_claim_references() {
        let (mut draft, identity, key) = fixture();
        draft.subject.claim_id = "vf_0123456789abcdef".into();
        let error = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap_err();
        assert!(
            error.contains("subject.claim_id must begin with `vcl_`"),
            "{error}"
        );
    }

    #[test]
    fn current_content_hash_artifact_ids_are_valid() {
        let (mut draft, identity, key) = fixture();
        draft.subject.artifact_ids = vec!["a".repeat(64)];
        draft.output_artifact_ids = vec!["f".repeat(64)];
        VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap();
    }

    #[test]
    fn malformed_artifact_ids_fail_closed() {
        for artifact_id in [
            "sha256:aaaaaaaa".to_string(),
            "A".repeat(64),
            "artifact".to_string(),
        ] {
            let (mut draft, identity, key) = fixture();
            draft.subject.artifact_ids = vec![artifact_id];
            let error = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap_err();
            assert!(error.contains("full lowercase content hash"), "{error}");
        }
    }

    /// A reference handle must be the one its root derives.
    ///
    /// v1 checked that these fields carried a `vsb_`/`vpr_` namespace and a
    /// non-empty body, which admitted any body at all — including one naming a
    /// different object than the root beside it. There is now one right answer
    /// per reference and everything else fails.
    #[test]
    fn a_reference_handle_must_derive_from_the_root_beside_it() {
        for field in ["submission", "proposal"] {
            let (mut draft, identity, key) = fixture();
            if field == "submission" {
                draft.subject.submission_id = handle("vsb_", 'e');
            } else {
                draft.subject.proposal_id = handle("vpr_", 'e');
            }
            let error = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap_err();
            assert!(error.contains(&format!("subject.{field}_id")), "{error}");
            assert!(error.contains("the handle its root derives"), "{error}");
        }

        // A bare namespace names nothing, and now fails for the same reason
        // rather than a special one.
        let (mut draft, identity, key) = fixture();
        draft.subject.submission_id = "vsb_".into();
        assert!(VerificationRecordEnvelopeV2::seal(draft, identity, &key).is_err());
    }

    /// The published patterns admit exactly the handles the reader derives.
    #[test]
    fn the_published_reference_patterns_match_derived_handles() {
        use crate::wire_schema::{PROPOSAL_ID_REFERENCE_PATTERN, SUBMISSION_ID_REFERENCE_PATTERN};

        for (pattern, prefix) in [
            (SUBMISSION_ID_REFERENCE_PATTERN, "vsb_"),
            (PROPOSAL_ID_REFERENCE_PATTERN, "vpr_"),
        ] {
            let compiled = regex::Regex::new(pattern).expect("reference pattern compiles");
            assert!(compiled.is_match(&handle(prefix, 'a')));
            assert!(!compiled.is_match(prefix), "a bare namespace names nothing");
            assert!(!compiled.is_match(&format!("{prefix}fixture")));
            assert!(
                !compiled.is_match(&format!("{prefix}{}", "a".repeat(64))),
                "a handle is a prefix of a root, not a whole one"
            );
        }
    }

    #[test]
    fn the_verifier_is_read_from_the_signed_identity() {
        let (draft, identity, key) = fixture();
        let sealed = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap();
        assert_eq!(sealed.record.verifier(), "service:fixture-verifier");
    }

    #[test]
    fn a_record_cannot_declare_independence_from_its_own_verifier() {
        let (mut draft, identity, key) = fixture();
        draft.independence.declared_independent_of = vec!["service:fixture-verifier".into()];
        let error = VerificationRecordEnvelopeV2::seal(draft, identity, &key).unwrap_err();
        assert!(error.contains("independence from itself"), "{error}");
    }
}
