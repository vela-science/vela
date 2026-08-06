//! Stable principals and closed authority-action boundaries.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::canonical::sha256_canonical;

pub const PRINCIPAL_SCHEMA_V1: &str = "vela.principal.v1";
pub const HUMAN_ONLY_AUTHORITY_ACTIONS_V1: &[&str] = &[
    "authority_initialize",
    "authority_close",
    "authority_revoke",
    "authority_rotate",
    "bulk_correct",
    "destroy",
    "membership_manage",
    "policy_activate",
    "policy_revoke",
    "policy_rotate",
    "quorum_manage",
    "recovery_approve",
    "review_accept",
    "review_reject",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Human,
    Agent,
    Workload,
    Service,
    Institution,
}

pub fn principal_class_may_request(principal_class: PrincipalClass, action: &str) -> bool {
    !matches!(
        principal_class,
        PrincipalClass::Agent | PrincipalClass::Workload
    ) || !HUMAN_ONLY_AUTHORITY_ACTIONS_V1.contains(&action)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalAccountKind {
    Local,
    Oidc,
    Orcid,
    Spiffe,
    GithubApp,
}

impl ExternalAccountKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Oidc => "oidc",
            Self::Orcid => "orcid",
            Self::Spiffe => "spiffe",
            Self::GithubApp => "github-app",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalAccountLinkV1 {
    pub kind: ExternalAccountKind,
    pub issuer: String,
    pub subject: String,
    pub linked_at: String,
    pub revoked_at: Option<String>,
}

impl ExternalAccountLinkV1 {
    pub fn principal_identifier(&self) -> String {
        format!("{}:{}|{}", self.kind.prefix(), self.issuer, self.subject)
    }

    fn validate(&self) -> Result<(), String> {
        crate::shape::require_bounded_text("account issuer", &self.issuer, 1024)?;
        crate::shape::require_bounded_text("account subject", &self.subject, 1024)?;
        let linked_at = crate::shape::parse_canonical_time("account linked_at", &self.linked_at)?;
        if let Some(revoked_at) = &self.revoked_at
            && crate::shape::parse_canonical_time("account revoked_at", revoked_at)? < linked_at
        {
            return Err("account revocation precedes account linkage".into());
        }
        Ok(())
    }
}

/// A stable principal record. Display name and affiliation are readable
/// snapshots only; authorization uses `principal_id`, class, and current
/// account links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalV1 {
    pub schema: String,
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub display_name: Option<String>,
    pub affiliation: Option<String>,
    pub account_links: Vec<ExternalAccountLinkV1>,
}

impl PrincipalV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PRINCIPAL_SCHEMA_V1 {
            return Err(format!("principal schema must be {PRINCIPAL_SCHEMA_V1}"));
        }
        crate::shape::require_bounded_text("principal_id", &self.principal_id, 2048)?;
        if let Some(display_name) = &self.display_name {
            crate::shape::require_bounded_text("display_name", display_name, 512)?;
        }
        if let Some(affiliation) = &self.affiliation {
            crate::shape::require_bounded_text("affiliation", affiliation, 512)?;
        }

        let mut links = BTreeSet::new();
        let mut link_identifiers = BTreeSet::new();
        for link in &self.account_links {
            link.validate()?;
            if !links.insert(link) || !link_identifiers.insert(link.principal_identifier()) {
                return Err("principal contains a duplicate account link".into());
            }
        }
        if self.account_links.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("principal account links must be strictly sorted".into());
        }

        match self.principal_class {
            PrincipalClass::Human => {
                if self.account_links.is_empty()
                    || !self
                        .account_links
                        .iter()
                        .any(|link| link.principal_identifier() == self.principal_id)
                {
                    return Err(
                        "human principal_id must equal one retained issuer-subject link".into(),
                    );
                }
                if self.account_links.iter().any(|link| {
                    !matches!(
                        link.kind,
                        ExternalAccountKind::Local
                            | ExternalAccountKind::Oidc
                            | ExternalAccountKind::Orcid
                    )
                }) {
                    return Err("human principals may use only local, OIDC, or ORCID links".into());
                }
            }
            PrincipalClass::Agent => {
                require_prefix("agent principal", &self.principal_id, "agent:")?
            }
            PrincipalClass::Workload => require_one_prefix(
                "workload principal",
                &self.principal_id,
                &["workload:", "oidc:", "spiffe:", "github-app:"],
            )?,
            PrincipalClass::Service => {
                require_prefix("service principal", &self.principal_id, "service:")?
            }
            PrincipalClass::Institution => {
                require_prefix("institution principal", &self.principal_id, "institution:")?
            }
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

fn require_prefix(name: &str, value: &str, prefix: &str) -> Result<(), String> {
    if value.len() <= prefix.len() || !value.starts_with(prefix) {
        return Err(format!("{name} must start with {prefix}"));
    }
    Ok(())
}

fn require_one_prefix(name: &str, value: &str, prefixes: &[&str]) -> Result<(), String> {
    if prefixes
        .iter()
        .any(|prefix| value.len() > prefix.len() && value.starts_with(prefix))
    {
        Ok(())
    } else {
        Err(format!("{name} has no supported identity namespace"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human_principal() -> PrincipalV1 {
        PrincipalV1 {
            schema: PRINCIPAL_SCHEMA_V1.into(),
            principal_id: "oidc:https://github.com|1234567".into(),
            principal_class: PrincipalClass::Human,
            display_name: Some("Fixture reviewer".into()),
            affiliation: None,
            account_links: vec![ExternalAccountLinkV1 {
                kind: ExternalAccountKind::Oidc,
                issuer: "https://github.com".into(),
                subject: "1234567".into(),
                linked_at: "2026-07-24T12:00:00Z".into(),
                revoked_at: None,
            }],
        }
    }

    #[test]
    fn human_principal_uses_an_exact_namespaced_account_link() {
        let principal = human_principal();
        assert!(principal.root().unwrap().starts_with("sha256:"));

        let mut inferred = principal.clone();
        inferred.principal_id = "fixture@example.com".into();
        assert!(inferred.validate().is_err());
    }

    #[test]
    fn account_link_times_must_be_canonical_utc() {
        human_principal().validate().unwrap();

        // The instant of the fixture's linked_at, written at -04:00.
        let mut shifted = human_principal();
        shifted.account_links[0].linked_at = "2026-07-24T08:00:00-04:00".into();
        let error = shifted
            .validate()
            .expect_err("a non-zero offset is not canonical");
        assert!(error.contains("account linked_at"), "{error}");
        assert!(error.contains("-04:00"), "{error}");

        let mut plus_zero = human_principal();
        plus_zero.account_links[0].linked_at = "2026-07-24T12:00:00+00:00".into();
        assert!(plus_zero.validate().is_err());

        let mut subsecond = human_principal();
        subsecond.account_links[0].linked_at = "2026-07-24T12:00:00.500Z".into();
        assert!(subsecond.validate().is_err());

        // revoked_at is guarded on the same terms.
        let mut revoked = human_principal();
        revoked.account_links[0].revoked_at = Some("2026-07-24T14:00:00-04:00".into());
        let revoked_error = revoked
            .validate()
            .expect_err("a non-zero offset is not canonical");
        assert!(
            revoked_error.contains("account revoked_at"),
            "{revoked_error}"
        );
    }

    #[test]
    fn every_human_governance_action_is_structurally_forbidden_to_machines() {
        for action in HUMAN_ONLY_AUTHORITY_ACTIONS_V1 {
            assert!(!principal_class_may_request(PrincipalClass::Agent, action));
            assert!(!principal_class_may_request(
                PrincipalClass::Workload,
                action
            ));
            assert!(principal_class_may_request(PrincipalClass::Human, action));
        }
        assert!(principal_class_may_request(
            PrincipalClass::Agent,
            "evidence_submit"
        ));
    }
}
