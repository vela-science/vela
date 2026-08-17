//! Portable provenance for a human, model, organization, or tool review.
//!
//! A Review Method is retained source evidence, not an authority object. A
//! Verification Record binds its exact bytes through `method.implementation`
//! and `method.environment_root`; this profile makes the performer behind that
//! method explicit without changing Verification or Decision semantics.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const REVIEW_METHOD_V1_SCHEMA: &str = "vela.review-method.v1";
pub const REVIEWER_KINDS: &[&str] = &["human", "ai_model", "organization", "deterministic_tool"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewPerformerV1 {
    #[schemars(schema_with = "crate::wire_schema::reviewer_kind")]
    pub kind: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub display_name: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub identifier: String,
    #[schemars(schema_with = "crate::wire_schema::nullable_text")]
    pub provider: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::nullable_text")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewMethodV1 {
    #[schemars(schema_with = "crate::wire_schema::review_method_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub profile: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub property: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub question: String,
    pub reviewer: ReviewPerformerV1,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub attested_by_actor_id: String,
    #[schemars(schema_with = "crate::wire_schema::nonempty_text_array")]
    pub procedure: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::nonempty_text_array")]
    pub required_output: Vec<String>,
    #[schemars(schema_with = "crate::wire_schema::nonempty_text_array")]
    pub does_not_establish: Vec<String>,
}

impl ReviewMethodV1 {
    pub fn parse_canonical(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse Review Method v1: {error}"))?;
        let canonical = crate::canonical::to_canonical_bytes(&value)?;
        if bytes != canonical && bytes != [canonical.as_slice(), b"\n"].concat() {
            return Err(
                "Review Method v1 must be canonical JSON with at most one trailing LF".into(),
            );
        }
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != REVIEW_METHOD_V1_SCHEMA {
            return Err(format!(
                "Review Method schema must be `{REVIEW_METHOD_V1_SCHEMA}`"
            ));
        }
        for (field, value) in [
            ("profile", &self.profile),
            ("property", &self.property),
            ("question", &self.question),
            ("reviewer.display_name", &self.reviewer.display_name),
            ("reviewer.identifier", &self.reviewer.identifier),
            ("attested_by_actor_id", &self.attested_by_actor_id),
        ] {
            require_text(field, value)?;
        }
        if !REVIEWER_KINDS.contains(&self.reviewer.kind.as_str()) {
            return Err(
                "Review Method reviewer.kind must be human, ai_model, organization, or deterministic_tool"
                    .into(),
            );
        }
        if self.reviewer.kind == "ai_model" && self.reviewer.provider.is_none() {
            return Err("Review Method AI reviewer must name its provider".into());
        }
        for (field, value) in [
            ("reviewer.provider", self.reviewer.provider.as_deref()),
            ("reviewer.version", self.reviewer.version.as_deref()),
        ] {
            if let Some(value) = value {
                require_text(field, value)?;
            }
        }
        for (field, values) in [
            ("procedure", &self.procedure),
            ("required_output", &self.required_output),
            ("does_not_establish", &self.does_not_establish),
        ] {
            if values.is_empty() {
                return Err(format!("Review Method {field} must not be empty"));
            }
            for value in values {
                require_text(field, value)?;
            }
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Review Method {field} must be non-empty, trimmed text"
        ));
    }
    crate::shape::require_bounded_text(
        &format!("Review Method {field}"),
        value,
        crate::wire_schema::TEXT_MAX_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method(kind: &str) -> ReviewMethodV1 {
        ReviewMethodV1 {
            schema: REVIEW_METHOD_V1_SCHEMA.into(),
            profile: "statement-fidelity-gpt-5.6-sol-v1".into(),
            property: "statement_fidelity".into(),
            question: "Does the formal statement preserve the source question?".into(),
            reviewer: ReviewPerformerV1 {
                kind: kind.into(),
                display_name: "GPT-5.6 Sol".into(),
                identifier: "gpt-5.6-sol".into(),
                provider: Some("OpenAI".into()),
                version: None,
            },
            attested_by_actor_id: "agent:codex-review".into(),
            procedure: vec!["Compare the exact source and formal statement.".into()],
            required_output: vec!["Retain a witness for the first material mismatch.".into()],
            does_not_establish: vec!["Scientific acceptance or Standing.".into()],
        }
    }

    #[test]
    fn model_review_names_the_model_and_attesting_actor() {
        method("ai_model").validate().unwrap();
    }

    #[test]
    fn model_review_without_provider_fails_closed() {
        let mut value = method("ai_model");
        value.reviewer.provider = None;
        assert!(value.validate().is_err());
    }

    #[test]
    fn unknown_reviewer_kind_fails_closed() {
        assert!(method("committee_consensus").validate().is_err());
    }

    #[test]
    fn canonical_parser_refuses_pretty_printed_bytes() {
        let bytes = serde_json::to_vec_pretty(&method("ai_model")).unwrap();
        assert!(ReviewMethodV1::parse_canonical(&bytes).is_err());
    }

    #[test]
    fn canonical_parser_accepts_repository_file_framing() {
        let mut bytes = crate::canonical::to_canonical_bytes(&method("ai_model")).unwrap();
        bytes.push(b'\n');
        assert_eq!(
            ReviewMethodV1::parse_canonical(&bytes).unwrap(),
            method("ai_model")
        );
    }

    #[test]
    fn optional_review_checklists_are_canonical_current_methods() {
        for bytes in [
            include_bytes!("../../../../examples/review-methods/semantic-source-adequacy.json")
                .as_slice(),
            include_bytes!("../../../../examples/review-methods/mathematical-reasoning.json")
                .as_slice(),
            include_bytes!("../../../../examples/review-methods/computational-formal.json")
                .as_slice(),
            include_bytes!("../../../../examples/review-methods/meta-authority-independence.json")
                .as_slice(),
        ] {
            ReviewMethodV1::parse_canonical(bytes).unwrap();
        }
    }
}
