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
            | Self::AuthorityModelUpdate => AuthorityResourceTypeV1::Repository,
            Self::ReviewAccept | Self::ReviewReject => AuthorityResourceTypeV1::Proposal,
        }
    }
}

/// The authority boundary is the Repository (ADR 0039), and this variant now
/// says so on the wire.
///
/// `rename_all = "snake_case"` means the variant name *is* the token. While
/// this variant was spelled with the retired noun it emitted the retired token,
/// and the comment here asserted it already emitted `"repository"` — a statement
/// about the code that the code contradicted. It could not be renamed in place
/// while a live genesis held the old token inside
/// `AuthorizationRequestV1::root()`. The 0.970.0 re-genesis of
/// `vela-science/math` removed that constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityResourceTypeV1 {
    Repository,
    Proposal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityMemberV1 {
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub role: AuthorityRoleV1,
}

/// Exact, content-addressed membership model for one repository.
///
/// The model has no policy language, inheritance, network lookup, quorum
/// engine, or executable extension surface. One principal may hold both roles
/// through two sorted entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationModelV1 {
    pub schema: String,
    pub profile: String,
    pub repository_id: String,
    pub members: Vec<AuthorityMemberV1>,
    pub previous_model_root: Option<String>,
}

impl AuthorizationModelV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORIZATION_MODEL_SCHEMA_V1 || self.profile != AUTHORIZATION_PROFILE_V1
        {
            return Err("authorization model schema or profile is invalid".into());
        }
        require_identifier(
            "authorization model repository_id",
            &self.repository_id,
            "vrepo_",
        )?;
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
            crate::shape::require_sha256_root("previous_model_root", root)?;
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
    pub repository_id: String,
    pub resource_type: AuthorityResourceTypeV1,
    pub resource_id: String,
}

impl AuthorizationResourceV1 {
    pub fn validate(&self) -> Result<(), String> {
        require_identifier(
            "authorization resource repository_id",
            &self.repository_id,
            "vrepo_",
        )?;
        let prefix = match self.resource_type {
            AuthorityResourceTypeV1::Repository => "vrepo_",
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
    pub repository_id: String,
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
        crate::shape::require_sha256_root("authorization request model_root", &self.model_root)?;
        require_identifier(
            "authorization request repository_id",
            &self.repository_id,
            "vrepo_",
        )?;
        crate::shape::require_bounded_text(
            "authorization request principal_id",
            &self.principal_id,
            2048,
        )?;
        self.resource.validate()?;
        crate::shape::require_sha256_root(
            "authorization request authentication_root",
            &self.authentication_root,
        )?;
        crate::shape::require_sha256_root(
            "authorization request transaction_read_set_root",
            &self.transaction_read_set_root,
        )?;
        crate::shape::require_sha256_root(
            "authorization request intent_digest",
            &self.intent_digest,
        )?;
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

/// `RepositoryMismatch` and `ResourceRepositoryMismatch` serialize as
/// `"repository_mismatch"` and `"resource_repository_mismatch"` into
/// `AuthorityEvaluationV1`, which is hashed into the authority record. Same
/// migration as `AuthorityResourceTypeV1::Repository` above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationReasonV1 {
    MemberRoleAuthorized,
    ModelRootMismatch,
    RepositoryMismatch,
    ResourceRepositoryMismatch,
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
        crate::shape::require_sha256_root("authorization evaluation model_root", &self.model_root)?;
        crate::shape::require_sha256_root(
            "authorization evaluation request_root",
            &self.request_root,
        )?;
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

/// Check that one authorization model succeeds another exactly.
///
/// The retired policy bundle carried this rule under `previous_bundle_root`;
/// the model has always carried the same field for the same reason.
pub fn verify_authorization_model_transition(
    current: &AuthorizationModelV1,
    next: &AuthorizationModelV1,
) -> Result<(), String> {
    current.validate()?;
    next.validate()?;
    if current.repository_id != next.repository_id
        || next.previous_model_root.as_deref() != Some(current.root()?.as_str())
    {
        return Err("rotated authorization model does not extend the exact prior model".into());
    }
    Ok(())
}

/// Evaluate one exact request under Vela's closed repository-authorization
/// profile.
///
/// This pure evaluator is a shadow implementation until current repository
/// parity and an explicit repository-epoch cut are complete. It acquires no
/// authentication, signer, filesystem, clock, or network capability. Valid
/// but unauthorized requests return a content-addressed Deny; malformed model
/// or request bytes return an error and cannot produce an authority record.
pub fn evaluate_authorization_v1(
    model: &AuthorizationModelV1,
    request: &AuthorizationRequestV1,
) -> Result<AuthorizationEvaluationV1, String> {
    model.validate()?;
    request.validate()?;
    let model_root = model.root()?;
    let request_root = request.root()?;

    let outcome = if request.model_root != model_root {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ModelRootMismatch,
            None,
        )
    } else if request.repository_id != model.repository_id {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::RepositoryMismatch,
            None,
        )
    } else if request.resource.repository_id != request.repository_id {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ResourceRepositoryMismatch,
            None,
        )
    } else if request.principal_class != PrincipalClass::Human {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::PrincipalClassMismatch,
            None,
        )
    } else if request.recovery_recent {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::RecoverySessionForbidden,
            None,
        )
    } else if request.resource.resource_type != request.action.required_resource_type() {
        (
            AuthorizationDecisionV1::Deny,
            AuthorizationReasonV1::ResourceTypeMismatch,
            None,
        )
    } else {
        let members = model
            .members
            .iter()
            .filter(|member| member.principal_id == request.principal_id)
            .collect::<Vec<_>>();
        if members.is_empty() {
            (
                AuthorizationDecisionV1::Deny,
                AuthorizationReasonV1::UnknownMember,
                None,
            )
        } else if members
            .iter()
            .all(|member| member.principal_class != request.principal_class)
        {
            (
                AuthorizationDecisionV1::Deny,
                AuthorizationReasonV1::PrincipalClassMismatch,
                None,
            )
        } else {
            let required_role = request.action.required_role();
            if members.iter().any(|member| {
                member.principal_class == request.principal_class && member.role == required_role
            }) {
                (
                    AuthorizationDecisionV1::Allow,
                    AuthorizationReasonV1::MemberRoleAuthorized,
                    Some(required_role),
                )
            } else {
                (
                    AuthorizationDecisionV1::Deny,
                    AuthorizationReasonV1::RoleActionMismatch,
                    None,
                )
            }
        }
    };

    let evaluation = AuthorizationEvaluationV1 {
        schema: AUTHORIZATION_EVALUATION_SCHEMA_V1.into(),
        profile: AUTHORIZATION_PROFILE_V1.into(),
        model_root,
        request_root,
        decision: outcome.0,
        reason: outcome.1,
        matched_role: outcome.2,
    };
    evaluation.validate()?;
    Ok(evaluation)
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
            repository_id: "vrepo_fixture".into(),
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
            repository_id: "vrepo_fixture".into(),
            principal_id: "local:device-1|uid:501".into(),
            principal_class: PrincipalClass::Human,
            action: AuthorityActionV1::ReviewAccept,
            resource: AuthorizationResourceV1 {
                repository_id: "vrepo_fixture".into(),
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
                AuthorityResourceTypeV1::Repository
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

    fn closed_model() -> AuthorizationModelV1 {
        AuthorizationModelV1 {
            schema: AUTHORIZATION_MODEL_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            repository_id: "vrepo_fixture".into(),
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

    fn closed_request(model: &AuthorizationModelV1) -> AuthorizationRequestV1 {
        AuthorizationRequestV1 {
            schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root: model.root().unwrap(),
            repository_id: "vrepo_fixture".into(),
            principal_id: "local:device-1|uid:501".into(),
            principal_class: PrincipalClass::Human,
            action: AuthorityActionV1::ReviewAccept,
            resource: AuthorizationResourceV1 {
                repository_id: "vrepo_fixture".into(),
                resource_type: AuthorityResourceTypeV1::Proposal,
                resource_id: "vpr_0123456789abcdef".into(),
            },
            authentication_root: format!("sha256:{}", "b".repeat(64)),
            transaction_read_set_root: format!("sha256:{}", "c".repeat(64)),
            intent_digest: format!("sha256:{}", "d".repeat(64)),
            recovery_recent: false,
        }
    }

    #[test]
    fn closed_profile_allows_only_the_exact_member_role() {
        let model = closed_model();
        let reviewer = evaluate_authorization_v1(&model, &closed_request(&model)).unwrap();
        assert_eq!(reviewer.decision, AuthorizationDecisionV1::Allow);
        assert_eq!(reviewer.reason, AuthorizationReasonV1::MemberRoleAuthorized);
        assert_eq!(reviewer.matched_role, Some(AuthorityRoleV1::Reviewer));
        assert!(reviewer.root().unwrap().starts_with("sha256:"));

        let mut administrator_request = closed_request(&model);
        administrator_request.action = AuthorityActionV1::AuthorityInitialize;
        administrator_request.resource = AuthorizationResourceV1 {
            repository_id: "vrepo_fixture".into(),
            resource_type: AuthorityResourceTypeV1::Repository,
            resource_id: "vrepo_fixture".into(),
        };
        let administrator = evaluate_authorization_v1(&model, &administrator_request).unwrap();
        assert_eq!(administrator.decision, AuthorizationDecisionV1::Allow);
        assert_eq!(
            administrator.matched_role,
            Some(AuthorityRoleV1::Administrator)
        );
    }

    #[test]
    fn closed_profile_denies_every_boundary_mismatch_with_stable_reasons() {
        let model = closed_model();

        let mut wrong_model = closed_request(&model);
        wrong_model.model_root = format!("sha256:{}", "f".repeat(64));
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_model)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ModelRootMismatch
        );

        let mut wrong_frontier = closed_request(&model);
        wrong_frontier.repository_id = "vrepo_other".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_frontier)
                .unwrap()
                .reason,
            AuthorizationReasonV1::RepositoryMismatch
        );

        let mut wrong_resource_frontier = closed_request(&model);
        wrong_resource_frontier.resource.repository_id = "vrepo_other".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_resource_frontier)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ResourceRepositoryMismatch
        );

        let mut machine = closed_request(&model);
        machine.principal_class = PrincipalClass::Agent;
        assert_eq!(
            evaluate_authorization_v1(&model, &machine).unwrap().reason,
            AuthorizationReasonV1::PrincipalClassMismatch
        );

        let mut unknown = closed_request(&model);
        unknown.principal_id = "local:unknown-device|uid:501".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &unknown).unwrap().reason,
            AuthorizationReasonV1::UnknownMember
        );

        let mut administrator_only = closed_model();
        administrator_only.members.truncate(1);
        let reviewer = closed_request(&administrator_only);
        assert_eq!(
            evaluate_authorization_v1(&administrator_only, &reviewer)
                .unwrap()
                .reason,
            AuthorizationReasonV1::RoleActionMismatch
        );

        let mut wrong_resource_type = closed_request(&model);
        wrong_resource_type.resource.resource_type = AuthorityResourceTypeV1::Repository;
        wrong_resource_type.resource.resource_id = "vrepo_fixture".into();
        assert_eq!(
            evaluate_authorization_v1(&model, &wrong_resource_type)
                .unwrap()
                .reason,
            AuthorizationReasonV1::ResourceTypeMismatch
        );

        let mut recovered = closed_request(&model);
        recovered.recovery_recent = true;
        assert_eq!(
            evaluate_authorization_v1(&model, &recovered)
                .unwrap()
                .reason,
            AuthorizationReasonV1::RecoverySessionForbidden
        );
    }
}
