//! Current producer package: `vela.submission.v1`.
//!
//! A Submission is authenticated producer input. It may request a scientific
//! change, but it cannot assert Standing, mint a Verification Record, or create
//! an Event. Historical `vela.receipt.v1` remains a separate read-only era.

use ed25519_dalek::SigningKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::execution_binding::ExecutionBindingV1;
use crate::identity::{ActorClass, IdentityBinding};

pub const SUBMISSION_V1_SCHEMA: &str = "vela.submission.v1";
pub const SUBMISSION_V1_AUTH_ALGORITHM: &str = "ed25519";
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
    #[schemars(schema_with = "crate::wire_schema::source_attempt_id")]
    pub source_attempt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub source_run: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub emitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionAuthentication {
    #[schemars(schema_with = "crate::wire_schema::submission_auth_algorithm")]
    pub algorithm: String,
    pub identity_binding: IdentityBinding,
    #[schemars(schema_with = "crate::wire_schema::ed25519_signature")]
    pub signature: String,
}

/// Exact current producer input.
///
/// `submission_id` and `authentication.signature` are cleared for the signed
/// preimage. Every other field is authenticated. The readable `vsb_` handle is
/// routing only; [`Self::canonical_root`] is the full object identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmissionV1 {
    #[schemars(schema_with = "crate::wire_schema::submission_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::submission_id")]
    pub submission_id: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "ExecutionBindingV1")]
    pub execution_binding: Option<ExecutionBindingV1>,
    pub authentication: SubmissionAuthentication,
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
    pub execution_binding: Option<ExecutionBindingV1>,
}

impl SubmissionV1 {
    pub fn build(
        draft: SubmissionDraft,
        identity_binding: IdentityBinding,
        key: &SigningKey,
    ) -> Result<Self, String> {
        identity_binding.verify()?;
        if identity_binding.actor_class != ActorClass::Agent {
            return Err("Submission producers must use an agent-class identity binding".into());
        }
        if identity_binding.actor_id != draft.provenance.producer {
            return Err(
                "Submission provenance.producer must match the identity binding actor".into(),
            );
        }
        if identity_binding.public_key_hex != hex::encode(key.verifying_key().to_bytes()) {
            return Err("Submission signing key does not match its identity binding".into());
        }
        let mut value = Self {
            schema: SUBMISSION_V1_SCHEMA.to_string(),
            submission_id: String::new(),
            claim: draft.claim,
            artifacts: draft.artifacts,
            caveats: draft.caveats,
            replayability: draft.replayability,
            producer_checks: draft.producer_checks,
            verification_requirements: draft.verification_requirements,
            requested_change: draft.requested_change,
            provenance: draft.provenance,
            execution_binding: draft.execution_binding,
            authentication: SubmissionAuthentication {
                algorithm: SUBMISSION_V1_AUTH_ALGORITHM.to_string(),
                identity_binding,
                signature: String::new(),
            },
        };
        value.validate_semantics()?;
        let preimage = value.signed_preimage()?;
        value.authentication.signature = hex::encode(crate::sign::sign_bytes(key, &preimage));
        value.submission_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Submission exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse Submission v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        self.authentication.identity_binding.verify()?;
        if self.authentication.identity_binding.actor_class != ActorClass::Agent
            || self.authentication.identity_binding.actor_id != self.provenance.producer
        {
            return Err("Submission authentication does not bind its producer".into());
        }
        let preimage = self.signed_preimage()?;
        if !crate::sign::verify_action_signature(
            &preimage,
            &self.authentication.signature,
            &self.authentication.identity_binding.public_key_hex,
        )? {
            return Err("Submission whole-body signature does not verify".into());
        }
        let expected = self.derive_id()?;
        if expected != self.submission_id {
            return Err(format!(
                "Submission id mismatch: declared {}, rebuilt {expected}",
                self.submission_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    fn signed_preimage(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.submission_id.clear();
        unsigned.authentication.signature.clear();
        crate::canonical::to_canonical_bytes(&unsigned)
    }

    fn derive_id(&self) -> Result<String, String> {
        Ok(format!(
            "vsb_{}",
            &hex::encode(Sha256::digest(self.signed_preimage()?))[..16]
        ))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != SUBMISSION_V1_SCHEMA {
            return Err(format!(
                "Submission schema must be `{SUBMISSION_V1_SCHEMA}`"
            ));
        }
        if self.authentication.algorithm != SUBMISSION_V1_AUTH_ALGORITHM {
            return Err("Submission authentication.algorithm must be `ed25519`".into());
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
        if let Some(source_attempt) = &self.provenance.source_attempt {
            require_text("provenance.source_attempt", source_attempt)?;
            require_prefixed_hex("provenance.source_attempt", source_attempt, "vat_", 64)?;
        }
        if let Some(source_run) = &self.provenance.source_run {
            require_text("provenance.source_run", source_run)?;
        }
        chrono::DateTime::parse_from_rfc3339(&self.provenance.emitted_at)
            .map_err(|_| "Submission provenance.emitted_at must be RFC 3339".to_string())?;
        if let Some(binding) = &self.execution_binding {
            binding.validate()?;
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Submission {field} must be non-empty, trimmed text"
        ));
    }
    if value.len() > 16 * 1024 {
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
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("Submission {field} must be a full sha256: digest"))?;
    if !crate::shape::is_lower_hex_64(digest) {
        return Err(format!("Submission {field} must be a full sha256: digest"));
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
    use crate::identity::{IdentityBinding, IdentityBindingDraft};
    use rand_core::OsRng;

    fn fixture() -> (SubmissionDraft, IdentityBinding, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:fixture".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-26T00:00:00Z".into(),
            },
            &key,
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
                source_attempt: Some(format!("vat_{}", "4".repeat(64))),
                source_run: Some("run_fixture".into()),
                emitted_at: "2026-07-26T00:00:00Z".into(),
            },
            execution_binding: None,
        };
        (draft, identity, key)
    }

    #[test]
    fn submission_is_closed_content_addressed_and_whole_body_signed() {
        let (draft, identity, key) = fixture();
        let submission = SubmissionV1::build(draft, identity, &key).unwrap();
        assert!(submission.submission_id.starts_with("vsb_"));
        assert!(submission.canonical_root().unwrap().starts_with("sha256:"));
        SubmissionV1::parse(&submission.canonical_bytes().unwrap()).unwrap();

        let mut tampered = submission.clone();
        tampered.claim.assertion.push_str(" changed");
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn producer_check_cannot_claim_independent_authority() {
        let (mut draft, identity, key) = fixture();
        draft.producer_checks[0].authority = "independent_verification".into();
        let error = SubmissionV1::build(draft, identity, &key).unwrap_err();
        assert!(error.contains("not Verification Records"), "{error}");
    }

    #[test]
    fn standing_and_event_fields_are_rejected_as_unknown() {
        let (draft, identity, key) = fixture();
        let submission = SubmissionV1::build(draft, identity, &key).unwrap();
        let mut raw = serde_json::to_value(submission).unwrap();
        raw["accepted"] = serde_json::json!(true);
        assert!(SubmissionV1::parse(&serde_json::to_vec(&raw).unwrap()).is_err());
        raw.as_object_mut().unwrap().remove("accepted");
        raw["event"] = serde_json::json!({"id": "vev_forged"});
        assert!(SubmissionV1::parse(&serde_json::to_vec(&raw).unwrap()).is_err());
    }

    #[test]
    fn replayability_is_a_closed_submission_vocabulary() {
        for replayability in ["exact", "bounded", "approximate", "unavailable", "unknown"] {
            let (mut draft, identity, key) = fixture();
            draft.replayability = replayability.into();
            SubmissionV1::build(draft, identity, &key).unwrap();
        }

        let (mut draft, identity, key) = fixture();
        draft.replayability = "totally-reproducible-trust-me".into();
        let error = SubmissionV1::build(draft, identity, &key).unwrap_err();
        assert!(error.contains("replayability"), "{error}");
    }

    #[test]
    fn correction_requires_an_exact_historical_claim_target() {
        let (mut draft, identity, key) = fixture();
        draft.requested_change = RequestedChange {
            kind: "correct_claim".into(),
            target: None,
        };
        let error = SubmissionV1::build(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("target is required for correct_claim"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: "vf_0123456789abcdef".into(),
            claim_root: format!("sha256:{}", "a".repeat(64)),
        });
        let error = SubmissionV1::build(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("requested_change.target.claim_id"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: "sha256:not-a-root".into(),
        });
        let error = SubmissionV1::build(draft.clone(), identity.clone(), &key).unwrap_err();
        assert!(error.contains("requested_change.target.claim_root"));

        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: format!("sha256:{}", "c".repeat(64)),
        });
        SubmissionV1::build(draft, identity, &key).unwrap();
    }

    #[test]
    fn add_claim_refuses_a_target() {
        let (mut draft, identity, key) = fixture();
        draft.requested_change.target = Some(RequestedChangeTarget {
            claim_id: format!("vcl_{}", "b".repeat(64)),
            claim_root: format!("sha256:{}", "c".repeat(64)),
        });
        let error = SubmissionV1::build(draft, identity, &key).unwrap_err();
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
