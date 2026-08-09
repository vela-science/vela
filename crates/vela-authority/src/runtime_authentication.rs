//! Replaceable runtime authentication boundary for repository authority.
//!
//! The boundary consumes an operating-system, passkey, OIDC, or workload
//! session established by an adapter. It never accepts or returns the runtime
//! credential. Successful preflight yields only the closed, bearer-free
//! observation retained by an authority record.
//!
//! This module is intentionally filesystem-free. A caller must complete this
//! preflight before creating an authority transaction journal or asking a
//! repository signer to sign.

use std::collections::BTreeSet;
use std::fmt;

use vela_protocol::authentication::{
    AUTHENTICATION_OBSERVATION_SCHEMA_V1, AuthenticationAssurance, AuthenticationMethod,
    AuthenticationObservationV1,
};
use vela_protocol::authorization::{
    AuthorizationDecisionV1, AuthorizationEvaluationV1, AuthorizationModelV1,
    AuthorizationRequestV1, evaluate_authorization_v1,
};
use vela_protocol::principal::PrincipalClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticationRequest {
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub transaction_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSessionState {
    pub revoked_session_roots: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationFailure {
    Cancelled,
    Provider(String),
    InvalidObservation(String),
    PrincipalMismatch,
}

impl fmt::Display for AuthenticationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("authentication was cancelled"),
            Self::Provider(message) => {
                write!(formatter, "authentication provider failed: {message}")
            }
            Self::InvalidObservation(message) => {
                write!(
                    formatter,
                    "authentication observation is invalid: {message}"
                )
            }
            Self::PrincipalMismatch => {
                formatter.write_str("authentication principal differs from the requested principal")
            }
        }
    }
}

impl std::error::Error for AuthenticationFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityPreflightFailure {
    Authentication(AuthenticationFailure),
    PrincipalMismatch,
    AuthorizationInvalid(Vec<String>),
    AuthorizationDenied,
}

impl fmt::Display for AuthorityPreflightFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "{error}"),
            Self::PrincipalMismatch => formatter
                .write_str("authorization principal differs from the authenticated principal"),
            Self::AuthorizationInvalid(diagnostics) => write!(
                formatter,
                "authorization input or evaluation is invalid: {}",
                diagnostics.join("; ")
            ),
            Self::AuthorizationDenied => formatter.write_str("authorization denied"),
        }
    }
}

impl std::error::Error for AuthorityPreflightFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityPreflightResult {
    pub authentication: AuthenticationObservationV1,
    pub authorization: AuthorizationEvaluationV1,
    /// The exact request the evaluation decided, retained by the record.
    pub request: AuthorizationRequestV1,
}

/// Replaceable adapter over an already-established standard session.
///
/// Implementations must not return cookies, bearer tokens, provider
/// assertions, or raw session identifiers. Those remain entirely inside the
/// operating system or identity provider.
pub trait AuthenticationAdapter {
    fn observe(
        &mut self,
        request: &AuthenticationRequest,
    ) -> Result<AuthenticationObservationV1, AuthenticationFailure>;
}

/// Run the complete authentication preflight for one exact transaction.
///
/// The returned value is safe to retain in an authority record. Failure
/// returns no partial observation and this function has no filesystem or
/// signing capability.
pub fn authenticate_for_transaction<A: AuthenticationAdapter>(
    adapter: &mut A,
    request: &AuthenticationRequest,
    state: &RuntimeSessionState,
) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
    let observation = adapter.observe(request)?;
    if observation.principal_id != request.principal_id
        || observation.principal_class != request.principal_class
    {
        return Err(AuthenticationFailure::PrincipalMismatch);
    }
    observation
        .validate_at(&request.transaction_at, &state.revoked_session_roots)
        .map_err(AuthenticationFailure::InvalidObservation)?;
    Ok(observation)
}

/// Authenticate and authorize one action without acquiring any write or signer
/// capability.
///
/// The authentication observation used to be folded into a free-form Cedar
/// context under a reserved `authentication` key, with a guard stopping a
/// caller from writing that key itself. The closed profile has no free-form
/// context: it reads exactly one fact from the session, `recovery_recent`, and
/// binds the observation by root. Both are set here from the verified
/// observation rather than supplied, so there is nothing left for a caller to
/// override and no reserved name to defend.
pub fn preflight_authority_action<A: AuthenticationAdapter>(
    adapter: &mut A,
    request: &AuthenticationRequest,
    state: &RuntimeSessionState,
    model: &AuthorizationModelV1,
    authorization: &AuthorizationRequestV1,
) -> Result<AuthorityPreflightResult, AuthorityPreflightFailure> {
    if authorization.principal_class != request.principal_class
        || authorization.principal_id != request.principal_id
    {
        return Err(AuthorityPreflightFailure::PrincipalMismatch);
    }
    let observation = authenticate_for_transaction(adapter, request, state)
        .map_err(AuthorityPreflightFailure::Authentication)?;

    let mut authorization = authorization.clone();
    authorization.recovery_recent = observation.recovery_recent;
    authorization.authentication_root = observation
        .root()
        .map_err(|error| AuthorityPreflightFailure::AuthorizationInvalid(vec![error]))?;

    let evaluation = evaluate_authorization_v1(model, &authorization)
        .map_err(|error| AuthorityPreflightFailure::AuthorizationInvalid(vec![error]))?;
    if evaluation.decision != AuthorizationDecisionV1::Allow {
        return Err(AuthorityPreflightFailure::AuthorizationDenied);
    }
    Ok(AuthorityPreflightResult {
        authentication: observation,
        authorization: evaluation,
        request: authorization,
    })
}

/// Snapshot of a standard local operating-system login session.
///
/// Platform code is responsible only for obtaining these public facts from
/// the current OS account. It does not prompt for or retrieve a Vela key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOsSession {
    pub principal_id: String,
    pub issuer: String,
    pub subject: String,
    pub session_root: String,
    pub authenticated_at: String,
    pub expires_at: String,
    pub recovery_recent: bool,
}

impl AuthenticationAdapter for LocalOsSession {
    fn observe(
        &mut self,
        request: &AuthenticationRequest,
    ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
        Ok(AuthenticationObservationV1 {
            schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
            principal_id: self.principal_id.clone(),
            principal_class: PrincipalClass::Human,
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            method: AuthenticationMethod::LocalOsSession,
            assurance: AuthenticationAssurance::LocalSession,
            session_root: self.session_root.clone(),
            authenticated_at: self.authenticated_at.clone(),
            observed_at: request.transaction_at.clone(),
            expires_at: self.expires_at.clone(),
            user_presence: false,
            user_verification: false,
            recovery_recent: self.recovery_recent,
            revocation_ref: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tempfile::TempDir;

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn request() -> AuthenticationRequest {
        AuthenticationRequest {
            principal_id: "local:device-1|uid:501".into(),
            principal_class: PrincipalClass::Human,
            transaction_at: "2026-07-24T12:05:00Z".into(),
        }
    }

    fn local_session() -> LocalOsSession {
        LocalOsSession {
            principal_id: request().principal_id,
            issuer: "device-1".into(),
            subject: "uid:501".into(),
            session_root: root('a'),
            authenticated_at: "2026-07-24T12:00:00Z".into(),
            expires_at: "2026-07-24T13:00:00Z".into(),
            recovery_recent: false,
        }
    }

    fn sentinel() -> TempDir {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("canonical-event.json"), b"unchanged").unwrap();
        directory
    }

    const FIXTURE_REPOSITORY: &str = "vrepo_00000000000000000000000000000000";
    const NULL_ROOT: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    fn model() -> AuthorizationModelV1 {
        AuthorizationModelV1 {
            schema: vela_protocol::authorization::AUTHORIZATION_MODEL_SCHEMA_V1.into(),
            profile: vela_protocol::authorization::AUTHORIZATION_PROFILE_V1.into(),
            repository_id: FIXTURE_REPOSITORY.into(),
            members: vec![vela_protocol::authorization::AuthorityMemberV1 {
                principal_id: request().principal_id,
                principal_class: PrincipalClass::Human,
                role: vela_protocol::authorization::AuthorityRoleV1::Reviewer,
            }],
            previous_model_root: None,
        }
    }

    fn authorization(model: &AuthorizationModelV1) -> AuthorizationRequestV1 {
        AuthorizationRequestV1 {
            schema: vela_protocol::authorization::AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: vela_protocol::authorization::AUTHORIZATION_PROFILE_V1.into(),
            model_root: model.root().unwrap(),
            repository_id: FIXTURE_REPOSITORY.into(),
            principal_id: request().principal_id,
            principal_class: PrincipalClass::Human,
            action: vela_protocol::authorization::AuthorityActionV1::ReviewReject,
            resource: vela_protocol::authorization::AuthorizationResourceV1 {
                repository_id: FIXTURE_REPOSITORY.into(),
                resource_type: vela_protocol::authorization::AuthorityResourceTypeV1::Proposal,
                resource_id: "vpr_0123456789abcdef".into(),
            },
            authentication_root: NULL_ROOT.into(),
            transaction_read_set_root: NULL_ROOT.into(),
            intent_digest: NULL_ROOT.into(),
            recovery_recent: false,
        }
    }

    fn assert_sentinel_unchanged(directory: &TempDir) {
        assert_eq!(
            fs::read(directory.path().join("canonical-event.json")).unwrap(),
            b"unchanged"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn local_session_yields_only_a_bearer_free_observation() {
        let observation = authenticate_for_transaction(
            &mut local_session(),
            &request(),
            &RuntimeSessionState::default(),
        )
        .unwrap();
        let encoded = serde_json::to_string(&observation).unwrap();
        assert_eq!(observation.method, AuthenticationMethod::LocalOsSession);
        assert!(!encoded.contains("bearer"));
        assert!(!encoded.contains("cookie"));
        assert!(!encoded.contains("session_id"));
    }

    #[test]
    fn cancelled_authentication_returns_no_observation_and_writes_nothing() {
        struct Cancelled;
        impl AuthenticationAdapter for Cancelled {
            fn observe(
                &mut self,
                _request: &AuthenticationRequest,
            ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
                Err(AuthenticationFailure::Cancelled)
            }
        }

        let directory = sentinel();
        assert_eq!(
            authenticate_for_transaction(
                &mut Cancelled,
                &request(),
                &RuntimeSessionState::default()
            ),
            Err(AuthenticationFailure::Cancelled)
        );
        assert_sentinel_unchanged(&directory);
    }

    #[test]
    fn identity_expiry_and_revocation_fail_before_any_write() {
        let directory = sentinel();

        let mut wrong_identity = local_session();
        wrong_identity.principal_id = "local:other-device|uid:501".into();
        assert_eq!(
            authenticate_for_transaction(
                &mut wrong_identity,
                &request(),
                &RuntimeSessionState::default()
            ),
            Err(AuthenticationFailure::PrincipalMismatch)
        );

        let mut expired = local_session();
        expired.expires_at = "2026-07-24T12:04:59Z".into();
        assert!(matches!(
            authenticate_for_transaction(&mut expired, &request(), &RuntimeSessionState::default()),
            Err(AuthenticationFailure::InvalidObservation(_))
        ));

        let state = RuntimeSessionState {
            revoked_session_roots: BTreeSet::from([root('a')]),
        };
        assert!(matches!(
            authenticate_for_transaction(&mut local_session(), &request(), &state),
            Err(AuthenticationFailure::InvalidObservation(_))
        ));
        assert_sentinel_unchanged(&directory);
    }

    #[test]
    fn recent_recovery_remains_visible_to_later_policy_evaluation() {
        let mut session = local_session();
        session.recovery_recent = true;
        let observation =
            authenticate_for_transaction(&mut session, &request(), &RuntimeSessionState::default())
                .unwrap();
        assert!(observation.recovery_recent);
    }

    /// The two session-derived fields come from the verified observation, not
    /// from the caller.
    ///
    /// The Cedar version of this read three values back out of a free-form
    /// `context.authentication` object the preflight had inserted. The closed
    /// request has no such object: `recovery_recent` and `authentication_root`
    /// are typed fields, and both are overwritten here.
    #[test]
    fn the_session_fields_are_derived_from_the_verified_observation() {
        let model = model();
        let mut supplied = authorization(&model);
        supplied.recovery_recent = true;
        supplied.authentication_root =
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".into();

        let result = preflight_authority_action(
            &mut local_session(),
            &request(),
            &RuntimeSessionState::default(),
            &model,
            &supplied,
        )
        .unwrap();

        assert!(!result.request.recovery_recent);
        assert_eq!(
            result.request.authentication_root,
            result.authentication.root().unwrap()
        );
        assert_eq!(
            result.authorization.decision,
            AuthorizationDecisionV1::Allow
        );
    }

    #[test]
    fn denial_invalid_input_and_recent_recovery_write_nothing() {
        let directory = sentinel();
        let model = model();

        // A principal the model has never heard of.
        let mut stranger = authorization(&model);
        stranger.principal_id = "local:stranger|uid:999".into();
        assert_eq!(
            preflight_authority_action(
                &mut local_session(),
                &AuthenticationRequest {
                    principal_id: "local:stranger|uid:999".into(),
                    principal_class: PrincipalClass::Human,
                    transaction_at: "2026-07-24T12:05:00Z".into(),
                },
                &RuntimeSessionState::default(),
                &model,
                &stranger,
            ),
            Err(AuthorityPreflightFailure::Authentication(
                AuthenticationFailure::PrincipalMismatch
            ))
        );

        let mut invalid = authorization(&model);
        invalid.model_root = "not a root".into();
        assert!(matches!(
            preflight_authority_action(
                &mut local_session(),
                &request(),
                &RuntimeSessionState::default(),
                &model,
                &invalid,
            ),
            Err(AuthorityPreflightFailure::AuthorizationInvalid(_))
        ));

        // A session that recently recovered is refused by the profile itself.
        let mut recovered = local_session();
        recovered.recovery_recent = true;
        assert_eq!(
            preflight_authority_action(
                &mut recovered,
                &request(),
                &RuntimeSessionState::default(),
                &model,
                &authorization(&model),
            ),
            Err(AuthorityPreflightFailure::AuthorizationDenied)
        );
        assert_sentinel_unchanged(&directory);
    }

    #[test]
    fn a_caller_cannot_authorize_a_principal_it_is_not_authenticating() {
        #[derive(Default)]
        struct MustNotAuthenticate {
            called: bool,
        }

        impl AuthenticationAdapter for MustNotAuthenticate {
            fn observe(
                &mut self,
                _request: &AuthenticationRequest,
            ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
                self.called = true;
                Err(AuthenticationFailure::Provider(
                    "adapter should not have been invoked".into(),
                ))
            }
        }

        let model = model();
        let mut wrong_principal = authorization(&model);
        wrong_principal.principal_id = "local:other|uid:1".into();
        let mut adapter = MustNotAuthenticate::default();
        assert_eq!(
            preflight_authority_action(
                &mut adapter,
                &request(),
                &RuntimeSessionState::default(),
                &model,
                &wrong_principal,
            ),
            Err(AuthorityPreflightFailure::PrincipalMismatch)
        );
        assert!(!adapter.called);
    }
}
