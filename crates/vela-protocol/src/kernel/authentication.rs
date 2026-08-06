//! Bearer-free observations of standard authentication ceremonies.
//!
//! Runtime credentials, cookies, assertions, and tokens remain with their
//! providers. Canonical history retains only facts the repository authority
//! validated at transaction time.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::principal::PrincipalClass;

pub const AUTHENTICATION_OBSERVATION_SCHEMA_V1: &str = "vela.authentication-observation.v1";
pub const MAX_AUTHENTICATION_AGE_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationMethod {
    LocalOsSession,
    PlatformUserPresence,
    AgentEventSignature,
    AgentRecordSignature,
    Passkey,
    Oidc,
    WorkloadOidc,
    GithubApp,
    SciTokens,
    Spiffe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationAssurance {
    LocalSession,
    SingleFactor,
    MultiFactor,
    PhishingResistant,
    WorkloadAttested,
}

/// `session_root` commits to a provider-side session or assertion record. It
/// is never the session identifier or bearer credential itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationObservationV1 {
    pub schema: String,
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub issuer: String,
    pub subject: String,
    pub method: AuthenticationMethod,
    pub assurance: AuthenticationAssurance,
    pub session_root: String,
    pub authenticated_at: String,
    pub observed_at: String,
    pub expires_at: String,
    pub user_presence: bool,
    pub user_verification: bool,
    pub recovery_recent: bool,
    pub revocation_ref: Option<String>,
}

impl AuthenticationObservationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHENTICATION_OBSERVATION_SCHEMA_V1 {
            return Err(format!(
                "authentication schema must be {AUTHENTICATION_OBSERVATION_SCHEMA_V1}"
            ));
        }
        crate::shape::require_bounded_text("authentication principal", &self.principal_id, 2048)?;
        crate::shape::require_bounded_text("authentication issuer", &self.issuer, 1024)?;
        crate::shape::require_bounded_text("authentication subject", &self.subject, 1024)?;
        require_sha256("authentication session_root", &self.session_root)?;
        if let Some(root) = &self.revocation_ref {
            require_sha256("authentication revocation_ref", root)?;
        }

        let authenticated_at = crate::shape::parse_canonical_time(
            "authentication authenticated_at",
            &self.authenticated_at,
        )?;
        let observed_at =
            crate::shape::parse_canonical_time("authentication observed_at", &self.observed_at)?;
        let expires_at =
            crate::shape::parse_canonical_time("authentication expires_at", &self.expires_at)?;
        if observed_at < authenticated_at
            || observed_at >= expires_at
            || (expires_at - authenticated_at).num_seconds() > MAX_AUTHENTICATION_AGE_SECONDS
        {
            return Err("authentication observation is stale or exceeds 24 hours".into());
        }

        match self.principal_class {
            PrincipalClass::Human => {
                let identifiers = [
                    format!("local:{}|{}", self.issuer, self.subject),
                    format!("oidc:{}|{}", self.issuer, self.subject),
                    format!("orcid:{}|{}", self.issuer, self.subject),
                ];
                if !identifiers.contains(&self.principal_id) {
                    return Err(
                        "human authentication must bind the exact principal issuer-subject".into(),
                    );
                }
                if !matches!(
                    self.method,
                    AuthenticationMethod::LocalOsSession
                        | AuthenticationMethod::PlatformUserPresence
                        | AuthenticationMethod::Passkey
                        | AuthenticationMethod::Oidc
                ) {
                    return Err("human authentication method is not supported".into());
                }
                if self.method == AuthenticationMethod::Passkey
                    && (!self.user_presence
                        || !self.user_verification
                        || self.assurance != AuthenticationAssurance::PhishingResistant)
                {
                    return Err(
                        "passkey authentication requires presence, verification, and phishing resistance"
                            .into(),
                    );
                }
                if self.method == AuthenticationMethod::PlatformUserPresence
                    && (!self.user_presence
                        || !self.user_verification
                        || self.assurance < AuthenticationAssurance::MultiFactor)
                {
                    return Err(
                        "platform user-presence authentication requires presence and verification"
                            .into(),
                    );
                }
            }
            PrincipalClass::Agent => {
                let locally_signed = matches!(
                    self.method,
                    AuthenticationMethod::AgentEventSignature
                        | AuthenticationMethod::AgentRecordSignature
                ) && self.assurance == AuthenticationAssurance::SingleFactor;
                let externally_attested = matches!(
                    self.method,
                    AuthenticationMethod::WorkloadOidc
                        | AuthenticationMethod::GithubApp
                        | AuthenticationMethod::SciTokens
                        | AuthenticationMethod::Spiffe
                ) && self.assurance
                    == AuthenticationAssurance::WorkloadAttested;
                if (!locally_signed && !externally_attested)
                    || self.user_presence
                    || self.user_verification
                {
                    return Err("agent authentication facts are invalid".into());
                }
            }
            PrincipalClass::Workload => {
                if !matches!(
                    self.method,
                    AuthenticationMethod::WorkloadOidc
                        | AuthenticationMethod::GithubApp
                        | AuthenticationMethod::SciTokens
                        | AuthenticationMethod::Spiffe
                ) || self.assurance != AuthenticationAssurance::WorkloadAttested
                    || self.user_presence
                    || self.user_verification
                {
                    return Err("workload authentication facts are invalid".into());
                }
            }
            PrincipalClass::Service | PrincipalClass::Institution => {
                return Err(
                    "service and institution entities do not authenticate as transaction principals"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub fn validate_at(
        &self,
        transaction_at: &str,
        revoked_session_roots: &BTreeSet<String>,
    ) -> Result<(), String> {
        self.validate()?;
        if transaction_at != self.observed_at {
            return Err("authentication observation time differs from the authority record".into());
        }
        if revoked_session_roots.contains(&self.session_root) {
            return Err("authentication session was revoked before transaction use".into());
        }
        Ok(())
    }
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use a full sha256: digest"));
    };
    if !crate::shape::is_lower_hex_64(hex) {
        return Err(format!(
            "{name} must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn human() -> AuthenticationObservationV1 {
        AuthenticationObservationV1 {
            schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
            principal_id: "oidc:https://github.com|1234567".into(),
            principal_class: PrincipalClass::Human,
            issuer: "https://github.com".into(),
            subject: "1234567".into(),
            method: AuthenticationMethod::Passkey,
            assurance: AuthenticationAssurance::PhishingResistant,
            session_root: root('a'),
            authenticated_at: "2026-07-24T12:00:00Z".into(),
            observed_at: "2026-07-24T12:05:00Z".into(),
            expires_at: "2026-07-24T13:00:00Z".into(),
            user_presence: true,
            user_verification: true,
            recovery_recent: false,
            revocation_ref: Some(root('b')),
        }
    }

    #[test]
    fn exact_human_observation_is_bearer_free() {
        let observation = human();
        observation.validate().unwrap();
        let encoded = serde_json::to_string(&observation).unwrap();
        assert!(!encoded.contains("bearer"));
        assert!(!encoded.contains("session_id"));
        assert!(!encoded.contains("cookie"));
    }

    #[test]
    fn passkey_requires_exact_identity_presence_and_verification() {
        let mut wrong_identity = human();
        wrong_identity.principal_id = "someone@example.com".into();
        assert!(wrong_identity.validate().is_err());

        let mut no_verification = human();
        no_verification.user_verification = false;
        assert!(no_verification.validate().is_err());
    }

    #[test]
    fn signed_agent_record_is_a_single_factor_agent_observation() {
        let observation = AuthenticationObservationV1 {
            schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
            principal_id: "agent:fixture".into(),
            principal_class: PrincipalClass::Agent,
            issuer: "vela.activity-record.v1".into(),
            subject: "agent:fixture".into(),
            method: AuthenticationMethod::AgentRecordSignature,
            assurance: AuthenticationAssurance::SingleFactor,
            session_root: root('c'),
            authenticated_at: "2026-07-24T12:00:00Z".into(),
            observed_at: "2026-07-24T12:01:00Z".into(),
            expires_at: "2026-07-24T12:05:00Z".into(),
            user_presence: false,
            user_verification: false,
            recovery_recent: false,
            revocation_ref: None,
        };
        observation.validate().unwrap();

        let mut wrong_assurance = observation.clone();
        wrong_assurance.assurance = AuthenticationAssurance::MultiFactor;
        assert!(wrong_assurance.validate().is_err());

        let mut fabricated_presence = observation;
        fabricated_presence.user_presence = true;
        assert!(fabricated_presence.validate().is_err());
    }

    #[test]
    fn expiry_and_revocation_fail_closed() {
        let observation = human();
        assert!(
            observation
                .validate_at(&observation.observed_at, &BTreeSet::new())
                .is_ok()
        );
        assert!(
            observation
                .validate_at(
                    &observation.observed_at,
                    &BTreeSet::from([observation.session_root.clone()])
                )
                .is_err()
        );

        let mut stale = human();
        stale.expires_at = "2026-07-26T12:00:00Z".into();
        assert!(stale.validate().is_err());
    }

    #[test]
    fn recovery_is_retained_policy_context_not_hidden_state() {
        let mut observation = human();
        observation.recovery_recent = true;
        observation.validate().unwrap();
        assert_eq!(
            serde_json::to_value(observation).unwrap()["recovery_recent"],
            true
        );
    }
}
