//! Producer package: `vela.submission.v2`, carried in a DSSE envelope.
//!
//! A Submission is authenticated producer input. It may request a scientific
//! change, but it cannot assert Standing, mint a Verification Record, or create
//! an Event.
//!
//! ## What v2 changed
//!
//! v1 was one JSON object that carried its own signature over a preimage built
//! by cloning itself and clearing `submission_id` and
//! `authentication.signature`. v2 is a payload inside a [`EnvelopeV1`]: the
//! signature is the envelope's, over exactly the payload bytes, and the reader
//! parses those same bytes rather than a preimage it reconstructs.
//!
//! Two fields left with the convention that required them. The signature is
//! now the envelope's, and `submission_id` is derived from the retained
//! envelope's root by [`crate::shape::derive_handle`] — an object cannot carry
//! its own content address, and the id never was anything but a prefix of one.

use ed25519_dalek::SigningKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dsse::EnvelopeV1;
use crate::signer_identity::{ActorClass, SignerIdentityV1};

pub const SUBMISSION_V2_SCHEMA: &str = "vela.submission.v2";
pub const SUBMISSION_V2_PAYLOAD_TYPE: &str = "application/vnd.vela.submission.v2+json";
pub const SUBMISSION_MAX_BYTES: usize = 8 * 1024 * 1024;
pub const SUBMISSION_HANDLE_PREFIX: &str = "vsb_";
pub(crate) const PRODUCER_REPORTED_AUTHORITY: &str = "producer_reported";

// The closed vocabularies below are read by `validate_semantics` and by the
// wire-schema builders in `crate::wire_schema`. Writing a member in only one of
// those places is the drift the generated schema exists to prevent, so neither
// side spells the members itself.

/// What kind of Claim a Submission asserts.
pub const CLAIM_TYPES: &[&str] = &[
    "computational",
    "theoretical",
    "empirical",
    "negative",
    "contradiction",
];

/// How exactly the producer expects the work to replay.
pub const REPLAYABILITY_LEVELS: &[&str] =
    &["exact", "bounded", "approximate", "unavailable", "unknown"];

/// What a producer-reported check may report. `pass` here carries no
/// verification authority.
pub const PRODUCER_CHECK_OUTCOMES: &[&str] = &["pass", "fail", "error", "skipped", "unknown"];

/// What change a Submission may request against Claim Standing.
pub const REQUESTED_CHANGE_KINDS: &[&str] = &[
    "add_claim",
    "correct_claim",
    "supersede_claim",
    "retract_claim",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionClaim {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub assertion: String,
    #[serde(rename = "type")]
    #[schemars(schema_with = "crate::wire_schema::claim_type")]
    pub claim_type: String,
    #[schemars(schema_with = "crate::wire_schema::text_array")]
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionArtifact {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub kind: String,
    #[schemars(schema_with = "crate::wire_schema::safe_relative_path")]
    pub path: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProducerCheck {
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub method: String,
    #[schemars(schema_with = "crate::wire_schema::producer_check_outcome")]
    pub outcome: String,
    #[schemars(schema_with = "crate::wire_schema::producer_reported_authority")]
    pub authority: String,
}

impl ProducerCheck {
    pub fn new(method: String, outcome: String) -> Result<Self, String> {
        let value = Self {
            method,
            outcome,
            authority: PRODUCER_REPORTED_AUTHORITY.to_string(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), String> {
        require_text("producer_checks.method", &self.method)?;
        require_member(
            "producer_checks.outcome",
            &self.outcome,
            PRODUCER_CHECK_OUTCOMES,
        )?;
        if self.authority != PRODUCER_REPORTED_AUTHORITY {
            return Err(
                "submission producer_checks.authority must be `producer_reported`; \
                 producer checks are not Verification Records"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequestedChangeTarget {
    #[schemars(schema_with = "crate::wire_schema::claim_id")]
    pub claim_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub claim_root: String,
}

/// `add_claim` opens a new Claim and names no target; every other kind edits an
/// exact historical Claim and must name one. The `oneOf` below is the wire form
/// of the match in [`RequestedChange::validate`] — JSON Schema cannot read one
/// field's requirement off another's value, so the dependency is stated rather
/// than derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(extend("oneOf" = [
    {
        "properties": {"kind": {"const": "add_claim"}},
        "required": ["kind"],
        "not": {"required": ["target"]},
    },
    {
        "properties": {"kind": {"enum": ["correct_claim", "supersede_claim", "retract_claim"]}},
        "required": ["kind", "target"],
    },
]))]
pub struct RequestedChange {
    #[schemars(schema_with = "crate::wire_schema::requested_change_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "RequestedChangeTarget")]
    pub target: Option<RequestedChangeTarget>,
}

impl RequestedChange {
    pub fn validate(&self) -> Result<(), String> {
        require_member("requested_change.kind", &self.kind, REQUESTED_CHANGE_KINDS)?;
        match (self.kind.as_str(), self.target.as_ref()) {
            ("add_claim", None) => Ok(()),
            ("add_claim", Some(_)) => {
                Err("Submission requested_change.target must be absent for add_claim".into())
            }
            (_, Some(target)) => {
                require_prefixed_hex(
                    "requested_change.target.claim_id",
                    &target.claim_id,
                    "vcl_",
                    64,
                )?;
                require_sha256("requested_change.target.claim_root", &target.claim_root)
            }
            (_, None) => Err(format!(
                "Submission requested_change.target is required for {}",
                self.kind
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionProvenance {
    #[schemars(schema_with = "crate::wire_schema::producer_actor")]
    pub producer: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_system: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_run: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub emitted_at: String,
}

/// Exact current producer input, as the signed payload of a DSSE envelope.
///
/// Every field here is authenticated: the envelope signature covers the whole
/// canonical payload, `identity` included, and must verify under the key
/// `identity` names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionV2 {
    #[schemars(schema_with = "crate::wire_schema::submission_schema_tag")]
    pub schema: String,
    pub identity: SignerIdentityV1,
    pub claim: SubmissionClaim,
    #[schemars(length(min = 1))]
    pub artifacts: Vec<SubmissionArtifact>,
    #[schemars(schema_with = "crate::wire_schema::nonempty_text_array")]
    pub caveats: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::replayability")]
    pub replayability: String,
    pub producer_checks: Vec<ProducerCheck>,
    #[schemars(schema_with = "crate::wire_schema::text_array")]
    pub verification_requirements: Vec<String>,
    pub requested_change: RequestedChange,
    pub provenance: SubmissionProvenance,
}

/// One retained Submission: the envelope as stored, and what it decodes to.
///
/// The repository stores the envelope, so the envelope's canonical bytes are
/// what a content-addressed path and every `producer_package.root` name. The
/// `id` beside it is the derived handle for that root, carried here so the
/// callers that used to read `submission.submission_id` read one field rather
/// than re-deriving at every use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionRecordV2 {
    pub envelope: EnvelopeV1,
    pub submission: SubmissionV2,
    /// Canonical bytes of the envelope: exactly what is written to disk.
    pub bytes: Vec<u8>,
    /// `sha256:` over [`Self::bytes`].
    pub root: String,
    /// `vsb_` plus the first sixteen hexadecimal characters of [`Self::root`].
    pub id: String,
}

impl SubmissionRecordV2 {
    /// Sign a draft and seal it into its envelope.
    pub fn seal(
        draft: SubmissionDraft,
        identity: SignerIdentityV1,
        key: &SigningKey,
    ) -> Result<Self, String> {
        identity.validate()?;
        if identity.actor_class != ActorClass::Agent {
            return Err("Submission producers must use an agent-class identity".into());
        }
        if identity.actor_id != draft.provenance.producer {
            return Err("Submission provenance.producer must match the signer identity".into());
        }
        if identity.public_key_hex != hex::encode(key.verifying_key().to_bytes()) {
            return Err("Submission signing key does not match its declared identity".into());
        }
        let submission = SubmissionV2 {
            schema: SUBMISSION_V2_SCHEMA.to_string(),
            identity,
            claim: draft.claim,
            artifacts: draft.artifacts,
            caveats: draft.caveats,
            replayability: draft.replayability,
            producer_checks: draft.producer_checks,
            verification_requirements: draft.verification_requirements,
            requested_change: draft.requested_change,
            provenance: draft.provenance,
        };
        submission.validate_semantics()?;
        let payload = crate::canonical::to_canonical_bytes(&submission)?;
        let envelope = EnvelopeV1::seal_single(key, SUBMISSION_V2_PAYLOAD_TYPE, &payload);
        Self::from_envelope(envelope)
    }

    /// Read a retained Submission from its exact stored bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let envelope = EnvelopeV1::parse("Submission", bytes, SUBMISSION_MAX_BYTES)?;
        Self::from_envelope(envelope)
    }

    /// Verify an envelope and decode the payload it authenticates.
    ///
    /// The key the signature is checked against is the one the payload
    /// declares, so this is deliberately circular in a way that is safe: the
    /// signature covers the declaration, so a payload naming a key its signer
    /// does not hold cannot verify. It establishes possession, not identity —
    /// who `agent:erdos-search` really is remains a question for whoever pinned
    /// that key.
    pub fn from_envelope(envelope: EnvelopeV1) -> Result<Self, String> {
        let declared = crate::signer_identity::declared_public_key(
            "Submission",
            &crate::dsse::decode_base64("Submission payload", &envelope.payload)?,
        )?;
        let payload = envelope.open_single("Submission", SUBMISSION_V2_PAYLOAD_TYPE, &declared)?;
        let submission: SubmissionV2 = crate::canonical::from_json_slice_strict(&payload)
            .map_err(|error| format!("parse Submission v2: {error}"))?;
        if crate::canonical::to_canonical_bytes(&submission)? != payload {
            return Err("Submission payload is not canonical JSON".into());
        }
        submission.validate_semantics()?;

        let bytes = envelope.canonical_bytes()?;
        let root = crate::canonical::sha256_root(&bytes);
        let id = crate::shape::derive_handle(SUBMISSION_HANDLE_PREFIX, &root)?;
        Ok(Self {
            envelope,
            submission,
            bytes,
            root,
            id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SubmissionDraft {
    pub claim: SubmissionClaim,
    pub artifacts: Vec<SubmissionArtifact>,
    pub caveats: Vec<String>,
    pub replayability: String,
    pub producer_checks: Vec<ProducerCheck>,
    pub verification_requirements: Vec<String>,
    pub requested_change: RequestedChange,
    pub provenance: SubmissionProvenance,
}

impl SubmissionV2 {
    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != SUBMISSION_V2_SCHEMA {
            return Err(format!(
                "Submission schema must be `{SUBMISSION_V2_SCHEMA}`"
            ));
        }
        self.identity.validate()?;
        if self.identity.actor_class != ActorClass::Agent
            || self.identity.actor_id != self.provenance.producer
        {
            return Err("Submission signer identity does not bind its producer".into());
        }
        require_text("claim.assertion", &self.claim.assertion)?;
        require_member("claim.type", &self.claim.claim_type, CLAIM_TYPES)?;
        for condition in &self.claim.conditions {
            require_text("claim.conditions", condition)?;
        }
        if self.artifacts.is_empty() {
            return Err("Submission must bind at least one Artifact".into());
        }
        for artifact in &self.artifacts {
            require_text("artifacts.kind", &artifact.kind)?;
            require_safe_relative_path("artifacts.path", &artifact.path)?;
            require_sha256("artifacts.digest", &artifact.digest)?;
        }
        if self.caveats.is_empty() {
            return Err("Submission must state at least one caveat".into());
        }
        for caveat in &self.caveats {
            require_text("caveats", caveat)?;
        }
        require_member("replayability", &self.replayability, REPLAYABILITY_LEVELS)?;
        for check in &self.producer_checks {
            check.validate()?;
        }
        for requirement in &self.verification_requirements {
            require_text("verification_requirements", requirement)?;
        }
        self.requested_change.validate()?;
        require_actor("provenance.producer", &self.provenance.producer)?;
        require_text("provenance.source_system", &self.provenance.source_system)?;
        if let Some(source_run) = &self.provenance.source_run {
            require_text("provenance.source_run", source_run)?;
        }
        chrono::DateTime::parse_from_rfc3339(&self.provenance.emitted_at)
            .map_err(|_| "Submission provenance.emitted_at must be RFC 3339".to_string())?;
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Submission {field} must be non-empty, trimmed text"
        ));
    }
    if value.len() > crate::wire_schema::TEXT_MAX_BYTES {
        return Err(format!("Submission {field} exceeds 16 KiB"));
    }
    Ok(())
}

fn require_member(field: &str, value: &str, allowed: &[&str]) -> Result<(), String> {
    if !allowed.contains(&value) {
        return Err(format!(
            "Submission {field} must be one of {}",
            allowed.join(", ")
        ));
    }
    Ok(())
}

fn require_actor(field: &str, value: &str) -> Result<(), String> {
    let suffix = value
        .strip_prefix("agent:")
        .or_else(|| value.strip_prefix("ci:"))
        .filter(|suffix| !suffix.is_empty())
        .ok_or_else(|| format!("Submission {field} must be an agent: or ci: identity"))?;
    require_text(field, suffix)
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!("Submission {field} must be a full sha256: digest"))
    }
}

fn require_prefixed_hex(
    field: &str,
    value: &str,
    prefix: &str,
    hex_len: usize,
) -> Result<(), String> {
    let Some(hex) = value.strip_prefix(prefix) else {
        return Err(format!("{field} must begin with `{prefix}`"));
    };
    if hex.len() != hex_len || !hex.bytes().all(crate::shape::is_lower_hex) {
        return Err(format!(
            "{field} must contain exactly {hex_len} hexadecimal characters after `{prefix}`"
        ));
    }
    Ok(())
}

fn require_safe_relative_path(field: &str, value: &str) -> Result<(), String> {
    require_text(field, value)?;
    let path = std::path::Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("Submission {field} must be a safe relative path"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    fn fixture() -> (SubmissionDraft, SignerIdentityV1, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let identity = SignerIdentityV1::new(
            "agent:fixture",
            ActorClass::Agent,
            &key,
            "2026-07-26T00:00:00Z",
        )
        .unwrap();
        let draft = SubmissionDraft {
            claim: SubmissionClaim {
                assertion: "The bounded search contains no counterexample.".into(),
                claim_type: "computational".into(),
                conditions: vec!["n is in 1..10".into()],
            },
            artifacts: vec![SubmissionArtifact {
                kind: "witness".into(),
                path: "artifacts/result.json".into(),
                digest: format!("sha256:{}", "a".repeat(64)),
            }],
            caveats: vec!["This does not establish the unrestricted statement.".into()],
            replayability: "exact".into(),
            producer_checks: vec![
                ProducerCheck::new("fixture-check".into(), "pass".into()).unwrap(),
            ],
            verification_requirements: vec!["Run the frozen fixture verifier.".into()],
            requested_change: RequestedChange {
                kind: "add_claim".into(),
                target: None,
            },
            provenance: SubmissionProvenance {
                producer: "agent:fixture".into(),
                source_system: "fixture".into(),
                source_run: Some("run_fixture".into()),
                emitted_at: "2026-07-26T00:00:00Z".into(),
            },
        };
        (draft, identity, key)
    }

    /// Re-seal a mutated payload under the same key, without re-signing
    /// through `seal`. This is the shape of a producer who edits their own
    /// Submission: the signature is honest, so only the semantic checks stand
    /// between the edit and the repository.
    fn reseal(submission: &SubmissionV2, key: &SigningKey) -> EnvelopeV1 {
        let payload = crate::canonical::to_canonical_bytes(submission).unwrap();
        EnvelopeV1::seal_single(key, SUBMISSION_V2_PAYLOAD_TYPE, &payload)
    }

    #[test]
    fn a_sealed_submission_round_trips_and_derives_its_handle_from_its_root() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        assert!(record.root.starts_with("sha256:"));
        assert_eq!(
            record.id,
            format!("vsb_{}", &record.root["sha256:".len()..][..16]),
            "the handle is a prefix of the root and nothing else"
        );
        let reread = SubmissionRecordV2::parse(&record.bytes).unwrap();
        assert_eq!(reread, record);
    }

    #[test]
    fn a_payload_edited_after_signing_does_not_verify() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        let mut tampered = record.envelope.clone();
        let mut payload: serde_json::Value =
            serde_json::from_slice(&crate::dsse::decode_base64("p", &tampered.payload).unwrap())
                .unwrap();
        payload["claim"]["assertion"] = serde_json::json!("Something the producer did not sign.");
        tampered.payload = crate::dsse::encode_base64(&serde_json::to_vec(&payload).unwrap());
        assert!(SubmissionRecordV2::from_envelope(tampered).is_err());
    }

    #[test]
    fn a_signature_by_a_key_the_payload_does_not_declare_is_refused() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        let impostor = SigningKey::generate(&mut OsRng);
        let payload = crate::dsse::decode_base64("p", &record.envelope.payload).unwrap();
        let forged = EnvelopeV1::seal_single(&impostor, SUBMISSION_V2_PAYLOAD_TYPE, &payload);
        assert!(SubmissionRecordV2::from_envelope(forged).is_err());
    }

    #[test]
    fn a_submission_payload_is_not_readable_under_another_payload_type() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        let mut relabelled = record.envelope.clone();
        relabelled.payload_type = "application/vnd.vela.verification-record.v2+json".into();
        assert!(SubmissionRecordV2::from_envelope(relabelled).is_err());
    }

    #[test]
    fn producer_check_cannot_claim_independent_authority() {
        let (mut draft, identity, key) = fixture();
        draft.producer_checks[0].authority = "independent_verification".into();
        let error = SubmissionRecordV2::seal(draft, identity, &key).unwrap_err();
        assert!(error.contains("not Verification Records"), "{error}");
    }

    #[test]
    fn standing_and_event_fields_are_rejected_as_unknown() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        for forged in ["accepted", "event"] {
            let mut payload = serde_json::to_value(&record.submission).unwrap();
            payload[forged] = serde_json::json!("a field this type does not carry");
            let mut submission = record.submission.clone();
            // Sign the forged bytes honestly: the point is that the closed
            // payload parse rejects them, not that the signature does.
            let bytes = serde_json::to_vec(&payload).unwrap();
            let envelope = EnvelopeV1::seal_single(&key, SUBMISSION_V2_PAYLOAD_TYPE, &bytes);
            assert!(
                SubmissionRecordV2::from_envelope(envelope).is_err(),
                "`{forged}` reached a Submission"
            );
            submission.schema = SUBMISSION_V2_SCHEMA.into();
        }
    }

    #[test]
    fn the_producer_must_be_the_declared_signer() {
        let (draft, identity, key) = fixture();
        let record = SubmissionRecordV2::seal(draft, identity, &key).unwrap();

        let mut submission = record.submission.clone();
        submission.identity.actor_id = "agent:someone-else".into();
        assert!(SubmissionRecordV2::from_envelope(reseal(&submission, &key)).is_err());

        let mut submission = record.submission.clone();
        submission.identity.actor_class = ActorClass::Human;
        assert!(SubmissionRecordV2::from_envelope(reseal(&submission, &key)).is_err());
    }

    #[test]
    fn replayability_is_a_closed_submission_vocabulary() {
        for replayability in ["exact", "bounded", "approximate", "unavailable", "unknown"] {
            let (mut draft, identity, key) = fixture();
            draft.replayability = replayability.into();
            SubmissionRecordV2::seal(draft, identity, &key).unwrap();
        }

        let (mut draft, identity, key) = fixture();
        draft.replayability = "totally-reproducible-trust-me".into();
        let error = SubmissionRecordV2::seal(draft, identity, &key).unwrap_err();
        assert!(error.contains("replayability"), "{error}");
    }

    #[test]
    fn correction_requires_an_exact_historical_claim_target() {
        let (mut draft, identity, key) = fixture();
        draft.requested_change = RequestedChange {
            kind: "correct_claim".into(),
            target: None,
        };
        let error = SubmissionRecordV2::seal(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("target is required for correct_claim"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: "vf_0123456789abcdef".into(),
            claim_root: format!("sha256:{}", "a".repeat(64)),
        });
        let error = SubmissionRecordV2::seal(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("requested_change.target.claim_id"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: "sha256:not-a-root".into(),
        });
        let error = SubmissionRecordV2::seal(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("requested_change.target.claim_root"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: format!("sha256:{}", "c".repeat(64)),
        });
        SubmissionRecordV2::seal(draft, identity, &key).unwrap();
    }

    #[test]
    fn add_claim_refuses_a_target() {
        let (mut draft, identity, key) = fixture();
        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: format!("sha256:{}", "c".repeat(64)),
        });
        let error = SubmissionRecordV2::seal(draft, identity, &key).unwrap_err();
        assert!(error.contains("target must be absent for add_claim"));
    }

    /// Every string over `.`, `a`, `/` and a space, up to seven characters.
    ///
    /// The path pattern reads only four kinds of character — dot, slash,
    /// whitespace, and everything else — so one representative of each is a
    /// complete alphabet for it, and enumerating that alphabet decides the two
    /// rules the pattern carries rather than sampling them. Seven characters
    /// is two more than the longest string either rule turns on (`/../`, and
    /// the `..` component alone).
    fn path_alphabet() -> Vec<String> {
        let mut values = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..7 {
            let mut next = Vec::with_capacity(frontier.len() * 4);
            for value in &frontier {
                for character in ['.', 'a', '/', ' '] {
                    let mut grown = value.clone();
                    grown.push(character);
                    next.push(grown);
                }
            }
            values.extend(next.iter().cloned());
            frontier = next;
        }
        values
    }

    /// The published path pattern accepts exactly what this reader accepts.
    ///
    /// This is the check that could not be written while the pattern used
    /// lookahead, because the crate that would run it here cannot compile
    /// lookahead — so `Regex::new` succeeding is half the assertion, and the
    /// portability claim in `schemas/README.md` now has something holding it.
    #[test]
    fn safe_path_pattern_agrees_with_the_reader() {
        let compiled = regex::Regex::new(crate::wire_schema::SAFE_RELATIVE_PATH_PATTERN)
            .expect("the published path pattern compiles without lookahead");

        let mut accepted = 0usize;
        let values = path_alphabet();
        for value in &values {
            let reader_accepts = require_safe_relative_path("artifacts.path", value).is_ok();
            accepted += usize::from(reader_accepts);
            assert_eq!(
                compiled.is_match(value),
                reader_accepts,
                "the published pattern and this reader disagree about {value:?}"
            );
        }
        assert_eq!(values.len(), 21845);
        assert!(accepted > 0, "the alphabet must exercise both answers");

        for value in [
            "artifacts/result.json",
            ".vela/work/run/artifacts/report.json",
            "records/artifacts/sha256/",
            "a/./b",
            "..a",
            "a..",
            "a/..b",
            "...",
        ] {
            assert!(compiled.is_match(value), "{value:?} should be accepted");
            assert!(require_safe_relative_path("artifacts.path", value).is_ok());
        }
        for value in [
            "",
            "/",
            "/absolute",
            "..",
            "../escape",
            "a/../b",
            "a/..",
            " a",
            "a ",
        ] {
            assert!(!compiled.is_match(value), "{value:?} should be rejected");
            assert!(require_safe_relative_path("artifacts.path", value).is_err());
        }
    }

    /// A line terminator no longer hides a `..` from the published pattern.
    ///
    /// The lookahead this pattern replaced was written with `.`, which stops at
    /// a line terminator in both ECMA-262 and Python, so `a\n/..` reached the
    /// end of the negative lookahead without the `..` ever being looked at.
    /// A consumer holding only the schema would have admitted a path that
    /// climbs out of the tree. The reader has always rejected it, for the
    /// unrelated reason that the newline is a control character; the point of
    /// the case is that now the schema rejects it too.
    #[test]
    fn safe_path_pattern_sees_past_a_line_terminator() {
        let compiled =
            regex::Regex::new(crate::wire_schema::SAFE_RELATIVE_PATH_PATTERN).expect("compiles");
        for value in ["a\n/..", "a\r/..", "..\n/..", "a\n/../b"] {
            assert!(!compiled.is_match(value), "{value:?} should be rejected");
            assert!(require_safe_relative_path("artifacts.path", value).is_err());
        }
    }
}
