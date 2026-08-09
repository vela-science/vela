//! Stable structured failure returned by `vela --json`.
//!
//! This is a CLI read surface, not a signed protocol object. It reports why an
//! invocation failed and may state that a preflight proved zero durable delta;
//! it never carries scientific authority.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ERROR_V1_SCHEMA: &str = "vela.error.v1";
pub const ERROR_KINDS: &[&str] = &[
    "usage",
    "not_found",
    "custody_refused",
    "already_exists",
    "domain",
    "internal",
];
pub const ERROR_CODES: &[&str] = &[
    "file_missing",
    "file_not_regular",
    "file_symlink",
    "file_oversized",
    "file_path_escape",
    "file_path_invalid",
    "file_path_changed",
    "file_unreadable",
    "repository_missing",
    "repository_authority_uninitialized",
    "repository_incomplete",
    "repository_predecessor_layout",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorDetailV1 {
    #[schemars(schema_with = "crate::wire_schema::error_kind")]
    pub kind: String,
    #[schemars(required)]
    #[schemars(schema_with = "crate::wire_schema::nullable_error_code")]
    pub code: Option<String>,
    #[schemars(schema_with = "crate::wire_schema::error_text")]
    pub message: String,
    #[schemars(required)]
    #[schemars(schema_with = "crate::wire_schema::nullable_error_text")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorRetainedV1 {
    #[schemars(schema_with = "crate::wire_schema::error_text")]
    pub request_id: String,
    #[schemars(schema_with = "crate::wire_schema::false_value")]
    pub transaction_marker: bool,
}

/// One ordinary failure, or the enriched zero-delta preflight failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(extend("oneOf" = [
    {
        "not": {
            "anyOf": [
                {"required": ["request_id"]},
                {"required": ["operation_id"]},
                {"required": ["changed"]},
                {"required": ["retained"]},
                {"required": ["next"]}
            ]
        }
    },
    {
        "properties": {
            "request_id": {"type": "string"},
            "operation_id": {"type": "string"},
            "changed": {"const": false},
            "retained": {"type": "object"},
            "next": {"type": "string"}
        },
        "required": ["request_id", "operation_id", "changed", "retained", "next"]
    }
]))]
pub struct ErrorEnvelopeV1 {
    #[schemars(schema_with = "crate::wire_schema::error_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::false_value")]
    pub ok: bool,
    #[schemars(required)]
    #[schemars(schema_with = "crate::wire_schema::nullable_error_text")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained: Option<ErrorRetainedV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    pub error: ErrorDetailV1,
}

impl ErrorEnvelopeV1 {
    pub fn regular(
        command: Option<String>,
        kind: &str,
        code: Option<&str>,
        message: &str,
        hint: Option<&str>,
    ) -> Result<Self, String> {
        let value = Self {
            schema: ERROR_V1_SCHEMA.into(),
            ok: false,
            command,
            request_id: None,
            operation_id: None,
            changed: None,
            retained: None,
            next: None,
            error: ErrorDetailV1 {
                kind: kind.into(),
                code: code.map(str::to_owned),
                message: message.into(),
                hint: hint.map(str::to_owned),
            },
        };
        value.validate()?;
        Ok(value)
    }

    pub fn unchanged(
        command: Option<String>,
        kind: &str,
        code: Option<&str>,
        message: &str,
        operation_id: &str,
        next_command: &str,
    ) -> Result<Self, String> {
        let value = Self {
            schema: ERROR_V1_SCHEMA.into(),
            ok: false,
            command,
            request_id: Some(operation_id.into()),
            operation_id: Some(operation_id.into()),
            changed: Some(false),
            retained: Some(ErrorRetainedV1 {
                request_id: operation_id.into(),
                transaction_marker: false,
            }),
            next: Some(next_command.into()),
            error: ErrorDetailV1 {
                kind: kind.into(),
                code: code.map(str::to_owned),
                message: message.into(),
                hint: Some(next_command.into()),
            },
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != ERROR_V1_SCHEMA || self.ok {
            return Err("error envelope schema or success flag is invalid".into());
        }
        if !ERROR_KINDS.contains(&self.error.kind.as_str()) {
            return Err("error envelope kind is invalid".into());
        }
        if self
            .error
            .code
            .as_deref()
            .is_some_and(|code| !ERROR_CODES.contains(&code))
        {
            return Err("error envelope code is invalid".into());
        }
        match (
            self.request_id.as_deref(),
            self.operation_id.as_deref(),
            self.changed,
            self.retained.as_ref(),
            self.next.as_deref(),
        ) {
            (None, None, None, None, None) => Ok(()),
            (Some(request), Some(operation), Some(false), Some(retained), Some(next))
                if request == operation
                    && retained.request_id == request
                    && !retained.transaction_marker
                    && self.error.hint.as_deref() == Some(next) =>
            {
                Ok(())
            }
            _ => Err("error envelope zero-delta fields are inconsistent".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_and_zero_delta_errors_are_closed() {
        ErrorEnvelopeV1::regular(
            Some("status".into()),
            "domain",
            Some("repository_incomplete"),
            "repository replay failed",
            None,
        )
        .unwrap();
        ErrorEnvelopeV1::unchanged(
            Some("submit".into()),
            "usage",
            None,
            "artifact is absent",
            "op_fixture",
            "vela submit --help",
        )
        .unwrap();
    }

    #[test]
    fn invalid_codes_and_partial_zero_delta_state_fail() {
        assert!(
            ErrorEnvelopeV1::regular(None, "domain", Some("unknown"), "failure", None).is_err()
        );
        let mut value = ErrorEnvelopeV1::regular(None, "domain", None, "failure", None).unwrap();
        value.changed = Some(false);
        assert!(value.validate().is_err());
    }
}
