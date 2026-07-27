//! Stable principals and retained short-lived capability claims.
//!
//! Runtime OAuth, OIDC, SciTokens, GitHub App, or local credentials are
//! adapters. Their bearer bytes never enter canonical Vela history. This
//! module defines only the closed, independently replayable claims retained by
//! an authority record.

use std::collections::BTreeSet;

use chrono::{DateTime, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::canonical::sha256_canonical;

pub const PRINCIPAL_SCHEMA_V1: &str = "vela.principal.v1";
pub const CAPABILITY_SCHEMA_V1: &str = "vela.capability-grant.v1";
pub const VERIFIED_CAPABILITY_CLAIM_SCHEMA_V1: &str = "vela.verified-capability-claim.v1";
pub const CAPABILITY_AUDIENCE_V1: &str = "vela.repository-authority.v1";
pub const MAX_CAPABILITY_LIFETIME_SECONDS: i64 = 24 * 60 * 60;
pub const MAX_CAPABILITY_DELEGATION_DEPTH: u8 = 1;
pub const HUMAN_ONLY_AUTHORITY_ACTIONS_V1: &[&str] = &[
    "authority_initialize",
    "authority_migrate",
    "authority_model_migrate",
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
        require_bounded_text("account issuer", &self.issuer, 1024)?;
        require_bounded_text("account subject", &self.subject, 1024)?;
        let linked_at = parse_canonical_time("account linked_at", &self.linked_at)?;
        if let Some(revoked_at) = &self.revoked_at
            && parse_canonical_time("account revoked_at", revoked_at)? < linked_at
        {
            return Err("account revocation precedes account linkage".into());
        }
        Ok(())
    }
}

/// A stable principal record. Display name and affiliation are readable
/// snapshots only; authorization uses `principal_id`, class, current account
/// links, roles, and capabilities.
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
        require_bounded_text("principal_id", &self.principal_id, 2048)?;
        if let Some(display_name) = &self.display_name {
            require_bounded_text("display_name", display_name, 512)?;
        }
        if let Some(affiliation) = &self.affiliation {
            require_bounded_text("affiliation", affiliation, 512)?;
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
            PrincipalClass::Workload => {
                require_one_prefix(
                    "workload principal",
                    &self.principal_id,
                    &["workload:", "oidc:", "spiffe:", "github-app:"],
                )?;
            }
            PrincipalClass::Service => {
                require_prefix("service principal", &self.principal_id, "service:")?;
            }
            PrincipalClass::Institution => {
                require_prefix("institution principal", &self.principal_id, "institution:")?;
            }
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAction {
    ArtifactRegister,
    ArtifactRetractOwn,
    ProposalCreate,
    ProposalWithdrawOwn,
    ReceiptLand,
    VerifierAttach,
    WorkClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsequenceCeiling {
    PendingReview,
    PolicyRouted,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityResourceBindingV1 {
    pub resource_type: String,
    pub resource_id: String,
    pub resource_root: String,
}

impl CapabilityResourceBindingV1 {
    fn validate(&self) -> Result<(), String> {
        require_bounded_text("capability resource_type", &self.resource_type, 128)?;
        require_bounded_text("capability resource_id", &self.resource_id, 1024)?;
        require_sha256("capability resource_root", &self.resource_root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityExecutionBindingV1 {
    pub binding_type: String,
    pub binding_root: String,
}

impl CapabilityExecutionBindingV1 {
    fn validate(&self) -> Result<(), String> {
        require_bounded_text("execution binding_type", &self.binding_type, 128)?;
        require_sha256("execution binding_root", &self.binding_root)
    }
}

/// Canonical grant identity. A JWT/CWT or provider token may carry equivalent
/// runtime claims, but the bearer token is never serialized here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantV1 {
    pub schema: String,
    pub capability_id: String,
    pub issuer_principal_id: String,
    pub subject_principal_id: String,
    pub subject_class: PrincipalClass,
    pub current_actor_principal_id: String,
    pub actor_chain: Vec<String>,
    pub parent_capability_root: Option<String>,
    pub delegation_depth: u8,
    pub maximum_delegation_depth: u8,
    pub audience: String,
    pub frontier_id: String,
    pub actions: Vec<CapabilityAction>,
    pub resources: Vec<CapabilityResourceBindingV1>,
    pub execution_bindings: Vec<CapabilityExecutionBindingV1>,
    pub consequence_ceiling: ConsequenceCeiling,
    pub issued_at: String,
    pub not_before: String,
    pub expires_at: String,
    pub token_id: String,
    pub revocation_ref: Option<String>,
}

impl CapabilityGrantV1 {
    pub fn new(
        issuer_principal_id: String,
        subject_principal_id: String,
        subject_class: PrincipalClass,
        current_actor_principal_id: String,
        actor_chain: Vec<String>,
        parent_capability_root: Option<String>,
        delegation_depth: u8,
        maximum_delegation_depth: u8,
        audience: String,
        frontier_id: String,
        actions: Vec<CapabilityAction>,
        resources: Vec<CapabilityResourceBindingV1>,
        execution_bindings: Vec<CapabilityExecutionBindingV1>,
        consequence_ceiling: ConsequenceCeiling,
        issued_at: String,
        not_before: String,
        expires_at: String,
        token_id: String,
        revocation_ref: Option<String>,
    ) -> Result<Self, String> {
        let mut grant = Self {
            schema: CAPABILITY_SCHEMA_V1.into(),
            capability_id: String::new(),
            issuer_principal_id,
            subject_principal_id,
            subject_class,
            current_actor_principal_id,
            actor_chain,
            parent_capability_root,
            delegation_depth,
            maximum_delegation_depth,
            audience,
            frontier_id,
            actions,
            resources,
            execution_bindings,
            consequence_ceiling,
            issued_at,
            not_before,
            expires_at,
            token_id,
            revocation_ref,
        };
        grant.capability_id = grant.derive_id()?;
        grant.validate()?;
        Ok(grant)
    }

    pub fn derive_id(&self) -> Result<String, String> {
        let mut content = self.clone();
        content.capability_id.clear();
        let digest = sha256_canonical(&content)?;
        Ok(format!("vcap_{}", &digest[..32]))
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CAPABILITY_SCHEMA_V1 || self.capability_id != self.derive_id()? {
            return Err("capability schema or content address is invalid".into());
        }
        if !matches!(
            self.subject_class,
            PrincipalClass::Agent | PrincipalClass::Workload
        ) {
            return Err("runtime capabilities are only for agent or workload principals".into());
        }
        match self.subject_class {
            PrincipalClass::Agent => {
                require_prefix(
                    "capability agent subject",
                    &self.subject_principal_id,
                    "agent:",
                )?;
            }
            PrincipalClass::Workload => {
                require_one_prefix(
                    "capability workload subject",
                    &self.subject_principal_id,
                    &["workload:", "oidc:", "spiffe:", "github-app:"],
                )?;
            }
            _ => unreachable!("subject class was restricted above"),
        }
        for (name, value) in [
            ("capability issuer", self.issuer_principal_id.as_str()),
            ("capability subject", self.subject_principal_id.as_str()),
            (
                "capability current actor",
                self.current_actor_principal_id.as_str(),
            ),
            ("capability token_id", self.token_id.as_str()),
        ] {
            require_bounded_text(name, value, 2048)?;
        }
        if self.issuer_principal_id == self.subject_principal_id
            || self.current_actor_principal_id != self.subject_principal_id
        {
            return Err("capability issuer, subject, or current actor is invalid".into());
        }
        if self.actor_chain.is_empty()
            || self.actor_chain.len() < 2
            || self.actor_chain.get(self.actor_chain.len() - 2) != Some(&self.issuer_principal_id)
            || self.actor_chain.last() != Some(&self.current_actor_principal_id)
            || self.actor_chain.iter().any(String::is_empty)
        {
            return Err("capability actor chain does not connect issuer to current actor".into());
        }
        let unique_actors = self.actor_chain.iter().collect::<BTreeSet<_>>();
        if unique_actors.len() != self.actor_chain.len() {
            return Err("capability actor chain contains a cycle".into());
        }
        if self.audience != CAPABILITY_AUDIENCE_V1 || !self.frontier_id.starts_with("vfr_") {
            return Err("capability audience or Frontier is invalid".into());
        }
        if self.maximum_delegation_depth > MAX_CAPABILITY_DELEGATION_DEPTH
            || self.delegation_depth > self.maximum_delegation_depth
            || (self.delegation_depth == 0) != self.parent_capability_root.is_none()
        {
            return Err("capability delegation depth or parent is invalid".into());
        }
        if let Some(parent) = &self.parent_capability_root {
            require_sha256("parent_capability_root", parent)?;
        }
        if let Some(revocation_ref) = &self.revocation_ref {
            require_sha256("capability revocation_ref", revocation_ref)?;
        }
        require_strictly_sorted("capability actions", &self.actions)?;
        require_strictly_sorted("capability resources", &self.resources)?;
        require_strictly_sorted("capability execution bindings", &self.execution_bindings)?;
        if self.actions.is_empty()
            || self.resources.is_empty()
            || self.execution_bindings.is_empty()
        {
            return Err(
                "capability must bind at least one action, resource, and execution input".into(),
            );
        }
        for resource in &self.resources {
            resource.validate()?;
        }
        let resource_identities = self
            .resources
            .iter()
            .map(|resource| (&resource.resource_type, &resource.resource_id))
            .collect::<BTreeSet<_>>();
        if resource_identities.len() != self.resources.len() {
            return Err("capability contains an ambiguous resource identity".into());
        }
        for binding in &self.execution_bindings {
            binding.validate()?;
        }

        let issued_at = parse_canonical_time("capability issued_at", &self.issued_at)?;
        let not_before = parse_canonical_time("capability not_before", &self.not_before)?;
        let expires_at = parse_canonical_time("capability expires_at", &self.expires_at)?;
        if not_before < issued_at
            || expires_at <= not_before
            || (expires_at - issued_at).num_seconds() > MAX_CAPABILITY_LIFETIME_SECONDS
        {
            return Err("capability validity window is invalid or exceeds 24 hours".into());
        }
        Ok(())
    }

    pub fn validate_at(
        &self,
        observed_at: &str,
        revoked_capability_roots: &BTreeSet<String>,
    ) -> Result<(), String> {
        self.validate()?;
        let observed_at = parse_canonical_time("capability observed_at", observed_at)?;
        let not_before = parse_canonical_time("capability not_before", &self.not_before)?;
        let expires_at = parse_canonical_time("capability expires_at", &self.expires_at)?;
        if observed_at < not_before || observed_at >= expires_at {
            return Err("capability is not active at the recorded observation time".into());
        }
        if revoked_capability_roots.contains(&self.root()?) {
            return Err("capability was revoked before use".into());
        }
        Ok(())
    }

    pub fn validate_delegation_from(&self, parent: &Self) -> Result<(), String> {
        self.validate()?;
        parent.validate()?;
        if self.parent_capability_root.as_deref() != Some(parent.root()?.as_str())
            || self.issuer_principal_id != parent.subject_principal_id
            || self.frontier_id != parent.frontier_id
            || self.audience != parent.audience
            || self.delegation_depth != parent.delegation_depth + 1
            || self.maximum_delegation_depth > parent.maximum_delegation_depth
        {
            return Err("child capability does not form an attenuating delegation".into());
        }
        if !is_subset(&self.actions, &parent.actions)
            || !is_subset(&self.resources, &parent.resources)
            || !is_subset(&self.execution_bindings, &parent.execution_bindings)
            || self.consequence_ceiling > parent.consequence_ceiling
        {
            return Err("child capability broadens its parent".into());
        }
        let child_not_before = parse_canonical_time("child not_before", &self.not_before)?;
        let child_expires = parse_canonical_time("child expires_at", &self.expires_at)?;
        let child_issued = parse_canonical_time("child issued_at", &self.issued_at)?;
        let parent_not_before = parse_canonical_time("parent not_before", &parent.not_before)?;
        let parent_expires = parse_canonical_time("parent expires_at", &parent.expires_at)?;
        let parent_issued = parse_canonical_time("parent issued_at", &parent.issued_at)?;
        if child_issued < parent_issued
            || child_not_before < parent_not_before
            || child_expires > parent_expires
        {
            return Err("child capability widens its parent's validity window".into());
        }
        let expected_chain = parent
            .actor_chain
            .iter()
            .cloned()
            .chain(std::iter::once(self.subject_principal_id.clone()))
            .collect::<Vec<_>>();
        if self.actor_chain != expected_chain {
            return Err("child capability actor chain is not the parent's act chain".into());
        }
        Ok(())
    }
}

/// The claim retained in an authority record after the runtime credential has
/// been validated. It deliberately contains no bearer token or token bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedCapabilityClaimV1 {
    pub schema: String,
    pub capability_id: String,
    pub capability_root: String,
    pub issuer_principal_id: String,
    pub subject_principal_id: String,
    pub subject_class: PrincipalClass,
    pub current_actor_principal_id: String,
    pub actor_chain: Vec<String>,
    pub audience: String,
    pub frontier_id: String,
    pub actions: Vec<CapabilityAction>,
    pub resource_roots: Vec<String>,
    pub execution_binding_roots: Vec<String>,
    pub consequence_ceiling: ConsequenceCeiling,
    pub issued_at: String,
    pub expires_at: String,
    pub token_id: String,
    pub revocation_ref: Option<String>,
    pub verified_at: String,
}

impl VerifiedCapabilityClaimV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != VERIFIED_CAPABILITY_CLAIM_SCHEMA_V1
            || !self.capability_id.starts_with("vcap_")
        {
            return Err("verified capability schema or ID is invalid".into());
        }
        require_sha256("verified capability_root", &self.capability_root)?;
        if !matches!(
            self.subject_class,
            PrincipalClass::Agent | PrincipalClass::Workload
        ) || self.issuer_principal_id.is_empty()
            || self.subject_principal_id.is_empty()
            || self.current_actor_principal_id != self.subject_principal_id
            || self.actor_chain.len() < 2
            || self.actor_chain.get(self.actor_chain.len() - 2) != Some(&self.issuer_principal_id)
            || self.actor_chain.last() != Some(&self.current_actor_principal_id)
            || self.audience != CAPABILITY_AUDIENCE_V1
            || !self.frontier_id.starts_with("vfr_")
            || self.token_id.is_empty()
        {
            return Err("verified capability attribution or scope is invalid".into());
        }
        if self.actor_chain.iter().collect::<BTreeSet<_>>().len() != self.actor_chain.len() {
            return Err("verified capability actor chain contains a cycle".into());
        }
        require_strictly_sorted("verified capability actions", &self.actions)?;
        require_strictly_sorted("verified capability resources", &self.resource_roots)?;
        require_strictly_sorted(
            "verified capability execution bindings",
            &self.execution_binding_roots,
        )?;
        if self.actions.is_empty()
            || self.resource_roots.is_empty()
            || self.execution_binding_roots.is_empty()
        {
            return Err("verified capability scope is empty".into());
        }
        for root in self
            .resource_roots
            .iter()
            .chain(self.execution_binding_roots.iter())
        {
            require_sha256("verified capability binding", root)?;
        }
        if let Some(revocation_ref) = &self.revocation_ref {
            require_sha256("verified capability revocation_ref", revocation_ref)?;
        }
        let issued_at = parse_canonical_time("verified capability issued_at", &self.issued_at)?;
        let expires_at = parse_canonical_time("verified capability expires_at", &self.expires_at)?;
        let verified_at =
            parse_canonical_time("verified capability verified_at", &self.verified_at)?;
        if expires_at <= issued_at
            || (expires_at - issued_at).num_seconds() > MAX_CAPABILITY_LIFETIME_SECONDS
            || verified_at < issued_at
            || verified_at >= expires_at
        {
            return Err("verified capability time claim is invalid".into());
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

fn require_sha256(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use a full sha256: digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{name} must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}

fn parse_canonical_time(name: &str, value: &str) -> Result<DateTime<chrono::FixedOffset>, String> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|error| format!("{name} is not RFC3339: {error}"))?;
    if parsed.to_rfc3339_opts(SecondsFormat::Secs, true) != value {
        return Err(format!(
            "{name} must use canonical whole-second UTC RFC3339"
        ));
    }
    Ok(parsed)
}

fn require_bounded_text(name: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(|character| character.is_control())
    {
        return Err(format!(
            "{name} is empty, oversized, or contains control text"
        ));
    }
    Ok(())
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

fn require_strictly_sorted<T: Ord>(name: &str, values: &[T]) -> Result<(), String> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(format!("{name} must be strictly sorted and unique"))
    } else {
        Ok(())
    }
}

fn is_subset<T: Ord>(candidate: &[T], parent: &[T]) -> bool {
    let parent = parent.iter().collect::<BTreeSet<_>>();
    candidate.iter().all(|value| parent.contains(value))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn resource(character: char) -> CapabilityResourceBindingV1 {
        CapabilityResourceBindingV1 {
            resource_type: "target".into(),
            resource_id: format!("erdos:{character}"),
            resource_root: root(character),
        }
    }

    fn execution(character: char) -> CapabilityExecutionBindingV1 {
        CapabilityExecutionBindingV1 {
            binding_type: "packet".into(),
            binding_root: root(character),
        }
    }

    fn grant() -> CapabilityGrantV1 {
        CapabilityGrantV1::new(
            "service:local-authority".into(),
            "agent:codex:fixture:run-1".into(),
            PrincipalClass::Agent,
            "agent:codex:fixture:run-1".into(),
            vec![
                "service:local-authority".into(),
                "agent:codex:fixture:run-1".into(),
            ],
            None,
            0,
            1,
            CAPABILITY_AUDIENCE_V1.into(),
            "vfr_0123456789abcdef".into(),
            vec![CapabilityAction::ReceiptLand, CapabilityAction::WorkClaim],
            vec![resource('a'), resource('b')],
            vec![execution('c')],
            ConsequenceCeiling::PendingReview,
            "2026-07-24T12:00:00Z".into(),
            "2026-07-24T12:00:00Z".into(),
            "2026-07-24T13:00:00Z".into(),
            "jti-fixture-1".into(),
            Some(root('d')),
        )
        .unwrap()
    }

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

    fn child_grant(parent: &CapabilityGrantV1) -> CapabilityGrantV1 {
        CapabilityGrantV1::new(
            parent.subject_principal_id.clone(),
            "workload:local|worker-2".into(),
            PrincipalClass::Workload,
            "workload:local|worker-2".into(),
            vec![
                "service:local-authority".into(),
                "agent:codex:fixture:run-1".into(),
                "workload:local|worker-2".into(),
            ],
            Some(parent.root().unwrap()),
            1,
            1,
            parent.audience.clone(),
            parent.frontier_id.clone(),
            vec![CapabilityAction::ReceiptLand],
            vec![resource('a')],
            vec![execution('c')],
            ConsequenceCeiling::PendingReview,
            "2026-07-24T12:10:00Z".into(),
            "2026-07-24T12:10:00Z".into(),
            "2026-07-24T12:50:00Z".into(),
            "jti-fixture-2".into(),
            Some(root('d')),
        )
        .unwrap()
    }

    fn verified_claim(grant: &CapabilityGrantV1) -> VerifiedCapabilityClaimV1 {
        VerifiedCapabilityClaimV1 {
            schema: VERIFIED_CAPABILITY_CLAIM_SCHEMA_V1.into(),
            capability_id: grant.capability_id.clone(),
            capability_root: grant.root().unwrap(),
            issuer_principal_id: grant.issuer_principal_id.clone(),
            subject_principal_id: grant.subject_principal_id.clone(),
            subject_class: grant.subject_class,
            current_actor_principal_id: grant.current_actor_principal_id.clone(),
            actor_chain: grant.actor_chain.clone(),
            audience: grant.audience.clone(),
            frontier_id: grant.frontier_id.clone(),
            actions: grant.actions.clone(),
            resource_roots: grant
                .resources
                .iter()
                .map(|resource| resource.resource_root.clone())
                .collect(),
            execution_binding_roots: grant
                .execution_bindings
                .iter()
                .map(|binding| binding.binding_root.clone())
                .collect(),
            consequence_ceiling: grant.consequence_ceiling,
            issued_at: grant.issued_at.clone(),
            expires_at: grant.expires_at.clone(),
            token_id: grant.token_id.clone(),
            revocation_ref: grant.revocation_ref.clone(),
            verified_at: "2026-07-24T12:30:00Z".into(),
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
            "submission_register"
        ));
    }

    #[test]
    fn capability_is_content_addressed_scoped_and_time_bounded() {
        let grant = grant();
        assert!(grant.capability_id.starts_with("vcap_"));
        assert!(
            grant
                .validate_at("2026-07-24T12:30:00Z", &BTreeSet::new())
                .is_ok()
        );
        assert!(
            grant
                .validate_at("2026-07-24T13:00:00Z", &BTreeSet::new())
                .is_err()
        );

        let mut long_lived = grant.clone();
        long_lived.expires_at = "2026-07-26T12:00:00Z".into();
        long_lived.capability_id = long_lived.derive_id().unwrap();
        assert!(long_lived.validate().is_err());
    }

    #[test]
    fn capability_rejects_human_subjects_unsorted_scope_and_revocation() {
        let mut human = grant();
        human.subject_class = PrincipalClass::Human;
        human.capability_id = human.derive_id().unwrap();
        assert!(human.validate().is_err());

        let mut unsorted = grant();
        unsorted.actions.reverse();
        unsorted.capability_id = unsorted.derive_id().unwrap();
        assert!(unsorted.validate().is_err());

        let grant = grant();
        assert!(
            grant
                .validate_at(
                    "2026-07-24T12:30:00Z",
                    &BTreeSet::from([grant.root().unwrap()])
                )
                .is_err()
        );
    }

    #[test]
    fn child_capability_can_only_attenuate_its_parent() {
        let parent = grant();
        let child = child_grant(&parent);
        child.validate_delegation_from(&parent).unwrap();

        let mut broadened = child.clone();
        broadened.actions.push(CapabilityAction::VerifierAttach);
        broadened.actions.sort();
        broadened.capability_id = broadened.derive_id().unwrap();
        assert!(broadened.validate_delegation_from(&parent).is_err());
    }

    #[test]
    fn verified_claim_retains_no_bearer_token_and_fails_on_time_drift() {
        let grant = grant();
        let claim = verified_claim(&grant);
        claim.validate().unwrap();
        let encoded = serde_json::to_string(&claim).unwrap();
        assert!(!encoded.contains("bearer"));
        assert!(!encoded.contains("token_bytes"));

        let mut expired = claim;
        expired.verified_at = expired.expires_at.clone();
        assert!(expired.validate().is_err());
    }

    #[test]
    fn principal_capability_cross_implementation_fixture_is_exact() {
        let principal = human_principal();
        let parent = grant();
        let child = child_grant(&parent);
        let claim = verified_claim(&parent);
        let mut fixture = json!({
            "schema": "vela.principal-capability-conformance.v1",
            "principal": principal,
            "parent_capability": parent,
            "child_capability": child,
            "verified_claim": claim,
            "expected": {
                "principal_root": principal.root().unwrap(),
                "parent_capability_id": parent.capability_id,
                "parent_capability_root": parent.root().unwrap(),
                "child_capability_id": child.capability_id,
                "child_capability_root": child.root().unwrap(),
                "verified_claim_root": claim.root().unwrap(),
            },
        });
        let fixture_root = format!("sha256:{}", sha256_canonical(&fixture).unwrap());
        fixture
            .as_object_mut()
            .unwrap()
            .insert("fixture_root".into(), json!(fixture_root));
        let bytes = format!("{}\n", serde_json::to_string_pretty(&fixture).unwrap());
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/principal-capability-v1.json");
        if std::env::var_os("VELA_UPDATE_PRINCIPAL_CAPABILITY_FIXTURE").is_some() {
            std::fs::write(&path, &bytes).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
    }
}
