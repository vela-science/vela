//! Closed, dependency-free repository authorization values.
//!
//! Authorization answers whether an authenticated human may request one exact
//! repository-authority action. It never verifies scientific evidence, makes a
//! human Decision, or changes Standing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::sha256_canonical;
use crate::principal::PrincipalClass;

pub const AUTHORIZATION_PROFILE_V1: &str = "vela.repository-authorization.v1";
pub const AUTHORIZATION_MODEL_SCHEMA_V1: &str = "vela.authorization-model.v1";
pub const AUTHORIZATION_REQUEST_SCHEMA_V1: &str = "vela.authorization-request.v1";
pub const AUTHORIZATION_EVALUATION_SCHEMA_V1: &str = "vela.authorization-evaluation.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityRoleV1 {
    Administrator,
    Reviewer,
}

/// The complete current repository-authority action vocabulary. Routine
/// evidence production, Submission registration, and Verification import are
/// intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityActionV1 {
    AuthorityInitialize,
    AuthorityRotate,
    AuthorityClose,
    AuthorityModelUpdate,
    ReviewAccept,
    ReviewReject,
}

impl AuthorityActionV1 {
    pub fn required_role(self) -> AuthorityRoleV1 {
        match self {
            Self::AuthorityInitialize
            | Self::AuthorityRotate
            | Self::AuthorityClose
            | Self::AuthorityModelUpdate => AuthorityRoleV1::Administrator,
            Self::ReviewAccept | Self::ReviewReject => AuthorityRoleV1::Reviewer,
        }
    }

    pub fn required_resource_type(self) -> AuthorityResourceTypeV1 {
        match self {
            Self::AuthorityInitialize
            | Self::AuthorityRotate
            | Self::AuthorityClose
            | Self::AuthorityModelUpdate => AuthorityResourceTypeV1::Frontier,
            Self::ReviewAccept | Self::ReviewReject => AuthorityResourceTypeV1::Proposal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityResourceTypeV1 {
    Frontier,
    Proposal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityMemberV1 {
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub role: AuthorityRoleV1,
}

/// Exact, content-addressed membership model for one Frontier.
///
/// The model has no policy language, inheritance, network lookup, quorum
/// engine, or executable extension surface. One principal may hold both roles
/// through two sorted entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationModelV1 {
    pub schema: String,
    pub profile: String,
    pub frontier_id: String,
    pub members: Vec<AuthorityMemberV1>,
    pub previous_model_root: Option<String>,
}

impl AuthorizationModelV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORIZATION_MODEL_SCHEMA_V1 || self.profile != AUTHORIZATION_PROFILE_V1
        {
            return Err("authorization model schema or profile is invalid".into());
        }
        require_identifier("authorization model frontier_id", &self.frontier_id, "vfr_")?;
        if self.members.is_empty() {
            return Err("authorization model must contain at least one member".into());
        }
        let mut principal_classes = BTreeMap::new();
        for member in &self.members {
            crate::shape::require_bounded_text(
                "authorization member principal_id",
                &member.principal_id,
                2048,
            )?;
            if member.principal_class != PrincipalClass::Human {
                return Err("repository-authority membership is human-only".into());
            }
            if let Some(principal_class) =
                principal_classes.insert(member.principal_id.as_str(), member.principal_class)
                && principal_class != member.principal_class
            {
                return Err("authorization member changes principal class across roles".into());
            }
        }
        if self.members.windows(2).any(|pair| {
            (&pair[0].principal_id, pair[0].role) >= (&pair[1].principal_id, pair[1].role)
        }) {
            return Err("authorization model members must be strictly sorted".into());
        }
        if let Some(root) = &self.previous_model_root {
            require_sha256("previous_model_root", root)?;
        }
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationResourceV1 {
    pub frontier_id: String,
    pub resource_type: AuthorityResourceTypeV1,
    pub resource_id: String,
}

impl AuthorizationResourceV1 {
    pub fn validate(&self) -> Result<(), String> {
        require_identifier(
            "authorization resource frontier_id",
            &self.frontier_id,
            "vfr_",
        )?;
        let prefix = match self.resource_type {
            AuthorityResourceTypeV1::Frontier => "vfr_",
            AuthorityResourceTypeV1::Proposal => "vpr_",
        };
        require_identifier("authorization resource_id", &self.resource_id, prefix)
    }
}

/// Fully bound request evaluated before a repository-authority transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationRequestV1 {
    pub schema: String,
    pub profile: String,
    pub model_root: String,
    pub frontier_id: String,
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub action: AuthorityActionV1,
    pub resource: AuthorizationResourceV1,
    pub authentication_root: String,
    pub transaction_read_set_root: String,
    pub intent_digest: String,
    pub recovery_recent: bool,
}

impl AuthorizationRequestV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORIZATION_REQUEST_SCHEMA_V1
            || self.profile != AUTHORIZATION_PROFILE_V1
        {
            return Err("authorization request schema or profile is invalid".into());
        }
        require_sha256("authorization request model_root", &self.model_root)?;
        require_identifier(
            "authorization request frontier_id",
            &self.frontier_id,
            "vfr_",
        )?;
        crate::shape::require_bounded_text(
            "authorization request principal_id",
            &self.principal_id,
            2048,
        )?;
        self.resource.validate()?;
        require_sha256(
            "authorization request authentication_root",
            &self.authentication_root,
        )?;
        require_sha256(
            "authorization request transaction_read_set_root",
            &self.transaction_read_set_root,
        )?;
        require_sha256("authorization request intent_digest", &self.intent_digest)?;
        Ok(())
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecisionV1 {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationReasonV1 {
    MemberRoleAuthorized,
    ModelRootMismatch,
    FrontierMismatch,
    ResourceFrontierMismatch,
    PrincipalClassMismatch,
    UnknownMember,
    RoleActionMismatch,
    ResourceTypeMismatch,
    RecoverySessionForbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationEvaluationV1 {
    pub schema: String,
    pub profile: String,
    pub model_root: String,
    pub request_root: String,
    pub decision: AuthorizationDecisionV1,
    pub reason: AuthorizationReasonV1,
    pub matched_role: Option<AuthorityRoleV1>,
}

impl AuthorizationEvaluationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORIZATION_EVALUATION_SCHEMA_V1
            || self.profile != AUTHORIZATION_PROFILE_V1
        {
            return Err("authorization evaluation schema or profile is invalid".into());
        }
        require_sha256("authorization evaluation model_root", &self.model_root)?;
        require_sha256("authorization evaluation request_root", &self.request_root)?;
        match (self.decision, self.reason, self.matched_role) {
            (
                AuthorizationDecisionV1::Allow,
                AuthorizationReasonV1::MemberRoleAuthorized,
                Some(_),
            ) => Ok(()),
            (AuthorizationDecisionV1::Deny, reason, None)
                if reason != AuthorizationReasonV1::MemberRoleAuthorized =>
            {
                Ok(())
            }
            _ => Err("authorization evaluation outcome fields are inconsistent".into()),
        }
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!("sha256:{}", sha256_canonical(self)?))
    }
}

fn require_identifier(name: &str, value: &str, prefix: &str) -> Result<(), String> {
    crate::shape::require_bounded_text(name, value, 2048)?;
    if value.len() <= prefix.len() || !value.starts_with(prefix) {
        return Err(format!("{name} must start with {prefix}"));
    }
    Ok(())
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

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn model() -> AuthorizationModelV1 {
        AuthorizationModelV1 {
            schema: AUTHORIZATION_MODEL_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            frontier_id: "vfr_fixture".into(),
            members: vec![
                AuthorityMemberV1 {
                    principal_id: "local:device-1|uid:501".into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Administrator,
                },
                AuthorityMemberV1 {
                    principal_id: "local:device-1|uid:501".into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Reviewer,
                },
            ],
            previous_model_root: None,
        }
    }

    fn request(model_root: String) -> AuthorizationRequestV1 {
        AuthorizationRequestV1 {
            schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root,
            frontier_id: "vfr_fixture".into(),
            principal_id: "local:device-1|uid:501".into(),
            principal_class: PrincipalClass::Human,
            action: AuthorityActionV1::ReviewAccept,
            resource: AuthorizationResourceV1 {
                frontier_id: "vfr_fixture".into(),
                resource_type: AuthorityResourceTypeV1::Proposal,
                resource_id: "vpr_0123456789abcdef".into(),
            },
            authentication_root: root('b'),
            transaction_read_set_root: root('c'),
            intent_digest: root('d'),
            recovery_recent: false,
        }
    }

    #[test]
    fn objects_are_canonical_closed_and_strict() {
        let model = model();
        let model_root = model.root().unwrap();
        let request = request(model_root.clone());
        let evaluation = AuthorizationEvaluationV1 {
            schema: AUTHORIZATION_EVALUATION_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root,
            request_root: request.root().unwrap(),
            decision: AuthorizationDecisionV1::Allow,
            reason: AuthorizationReasonV1::MemberRoleAuthorized,
            matched_role: Some(AuthorityRoleV1::Reviewer),
        };
        assert!(evaluation.root().unwrap().starts_with("sha256:"));

        let mut unknown = serde_json::to_value(&request).unwrap();
        unknown["policy"] = serde_json::json!("permit everything");
        assert!(serde_json::from_value::<AuthorizationRequestV1>(unknown).is_err());
        assert!(serde_json::from_str::<AuthorityActionV1>(r#""submission_register""#).is_err());
    }

    #[test]
    fn model_requires_sorted_human_members() {
        let mut unsorted = model();
        unsorted.members.swap(0, 1);
        assert!(unsorted.validate().unwrap_err().contains("strictly sorted"));

        let mut machine = model();
        machine.members[0].principal_class = PrincipalClass::Agent;
        assert!(machine.validate().unwrap_err().contains("human-only"));
    }

    #[test]
    fn actions_have_one_role_and_resource_kind() {
        for action in [
            AuthorityActionV1::AuthorityInitialize,
            AuthorityActionV1::AuthorityRotate,
            AuthorityActionV1::AuthorityClose,
            AuthorityActionV1::AuthorityModelUpdate,
        ] {
            assert_eq!(action.required_role(), AuthorityRoleV1::Administrator);
            assert_eq!(
                action.required_resource_type(),
                AuthorityResourceTypeV1::Frontier
            );
        }
        for action in [
            AuthorityActionV1::ReviewAccept,
            AuthorityActionV1::ReviewReject,
        ] {
            assert_eq!(action.required_role(), AuthorityRoleV1::Reviewer);
            assert_eq!(
                action.required_resource_type(),
                AuthorityResourceTypeV1::Proposal
            );
        }
    }
}
