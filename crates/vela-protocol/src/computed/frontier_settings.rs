//! Closed, non-authoritative Frontier settings for repository profile v1.
//!
//! The value is deliberately small. It can affect local presentation and
//! mechanical workflow defaults, but it cannot carry credentials, network
//! routing, authority, policy, verifier, dependency, or accepted-state data.
//! This module only parses and validates the value; loader precedence and the
//! v0.1-to-v1 migration are separate edge concerns.

use serde::{Deserialize, Serialize};

use crate::events::MAX_ATTEMPT_LEASE_TTL_SECONDS;

pub const FRONTIER_SETTINGS_SCHEMA: &str = "vela.frontier-settings.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierSettingsV1 {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<PublishSettingsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<WorkSettingsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpSettingsV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishSettingsV1 {
    /// A checked-in Frontier may only narrow publication behavior to `off`.
    pub git_push: FrontierGitPush,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FrontierGitPush {
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkSettingsV1 {
    pub lease_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpSettingsV1 {
    pub profile: McpProfileV1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpProfileV1 {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "draft")]
    Draft,
}

impl FrontierSettingsV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FRONTIER_SETTINGS_SCHEMA {
            return Err(format!(
                "settings.schema must be {FRONTIER_SETTINGS_SCHEMA}"
            ));
        }
        if let Some(work) = self.work.as_ref() {
            if work.lease_ttl_seconds == 0 {
                return Err("settings.work.lease_ttl_seconds must be greater than zero".to_string());
            }
            if work.lease_ttl_seconds > MAX_ATTEMPT_LEASE_TTL_SECONDS {
                return Err(format!(
                    "settings.work.lease_ttl_seconds must be at most {MAX_ATTEMPT_LEASE_TTL_SECONDS}"
                ));
            }
        }
        Ok(())
    }

    pub fn from_toml(input: &str) -> Result<Self, String> {
        let settings: Self =
            toml::from_str(input).map_err(|error| format!("invalid frontier settings: {error}"))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string_pretty(self)
            .map_err(|error| format!("serialize frontier settings: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
schema = "vela.frontier-settings.v1"

[publish]
git_push = "off"

[work]
lease_ttl_seconds = 86400

[mcp]
profile = "read-only"
"#;

    #[test]
    fn frontier_settings_v1_parses_the_closed_allowlist() {
        let settings = FrontierSettingsV1::from_toml(VALID).unwrap();
        assert_eq!(settings.publish.unwrap().git_push, FrontierGitPush::Off);
        assert_eq!(settings.work.unwrap().lease_ttl_seconds, 86_400);
        assert_eq!(settings.mcp.unwrap().profile, McpProfileV1::ReadOnly);
    }

    #[test]
    fn frontier_settings_v1_round_trips_without_semantic_drift() {
        let settings = FrontierSettingsV1::from_toml(VALID).unwrap();
        let rendered = settings.to_toml().unwrap();
        assert_eq!(FrontierSettingsV1::from_toml(&rendered).unwrap(), settings);
    }

    #[test]
    fn frontier_settings_v1_rejects_unknown_and_forbidden_fields() {
        for forbidden in [
            "token = \"secret\"",
            "key_path = \"/tmp/key\"",
            "command = \"curl example.test\"",
            "hook = \"post-save\"",
            "network_endpoint = \"https://example.test\"",
            "policy = \"permit\"",
            "actor = \"reviewer:test\"",
            "verifier = \"local\"",
            "dependency = \"mutable-main\"",
            "accepted_state = true",
        ] {
            let input = format!("schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n{forbidden}\n");
            assert!(
                FrontierSettingsV1::from_toml(&input).is_err(),
                "accepted forbidden field {forbidden:?}"
            );
        }

        let nested_unknown = format!(
            "schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n[publish]\ngit_push = \"off\"\ntoken = \"secret\"\n"
        );
        assert!(FrontierSettingsV1::from_toml(&nested_unknown).is_err());
    }

    #[test]
    fn frontier_settings_v1_rejects_unbounded_lease_ttl() {
        let input = format!(
            "schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n[work]\nlease_ttl_seconds = {}\n",
            MAX_ATTEMPT_LEASE_TTL_SECONDS + 1
        );
        let error = FrontierSettingsV1::from_toml(&input).unwrap_err();
        assert!(error.contains("must be at most"), "{error}");
    }

    #[test]
    fn frontier_settings_v1_rejects_widening_and_invalid_values() {
        let widening =
            format!("schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n[publish]\ngit_push = \"auto\"\n");
        assert!(FrontierSettingsV1::from_toml(&widening).is_err());

        let zero_ttl =
            format!("schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n[work]\nlease_ttl_seconds = 0\n");
        assert!(FrontierSettingsV1::from_toml(&zero_ttl).is_err());

        let unknown_profile =
            format!("schema = \"{FRONTIER_SETTINGS_SCHEMA}\"\n[mcp]\nprofile = \"signing\"\n");
        assert!(FrontierSettingsV1::from_toml(&unknown_profile).is_err());
    }
}
