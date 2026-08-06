//! Exact execution identity attached to a current Submission.
//!
//! The binding is a producer-side description of the packet, profile,
//! verifier capsule, and result contract used for bounded work. It carries no
//! verification or decision authority.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const EXECUTION_BINDING_SCHEMA: &str = "vela.execution-binding.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBindingV1 {
    #[schemars(schema_with = "crate::wire_schema::execution_binding_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub packet_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub profile_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub verifier_capsule_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub result_contract_root: String,
}

impl ExecutionBindingV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EXECUTION_BINDING_SCHEMA {
            return Err(format!("schema must be {EXECUTION_BINDING_SCHEMA}"));
        }
        for (field, value) in [
            ("packet_root", &self.packet_root),
            ("profile_root", &self.profile_root),
            ("verifier_capsule_root", &self.verifier_capsule_root),
            ("result_contract_root", &self.result_contract_root),
        ] {
            if !is_full_sha256_root(value) {
                return Err(format!("{field} must be a full lowercase sha256 root"));
            }
        }
        Ok(())
    }
}

pub fn is_full_sha256_root(value: &str) -> bool {
    crate::shape::is_full_sha256_root(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_binding_requires_exact_roots() {
        let binding = ExecutionBindingV1 {
            schema: EXECUTION_BINDING_SCHEMA.into(),
            packet_root: format!("sha256:{}", "1".repeat(64)),
            profile_root: format!("sha256:{}", "2".repeat(64)),
            verifier_capsule_root: format!("sha256:{}", "3".repeat(64)),
            result_contract_root: format!("sha256:{}", "4".repeat(64)),
        };
        assert!(binding.validate().is_ok());

        let mut short = binding;
        short.packet_root = "sha256:1".into();
        assert!(short.validate().is_err());
    }
}
