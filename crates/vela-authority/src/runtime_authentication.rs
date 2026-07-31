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

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde_json::{Value, json};
use vela_protocol::authentication::{
    AUTHENTICATION_OBSERVATION_SCHEMA_V1, AuthenticationAssurance, AuthenticationMethod,
    AuthenticationObservationV1,
};
use vela_protocol::authority::{CedarDecision, CedarEvaluation};
use vela_protocol::principal::PrincipalClass;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::{CedarEvaluationInput, evaluate};

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
    ReservedContext,
    AuthorizationInvalid(Vec<String>),
    AuthorizationDenied,
}

impl fmt::Display for AuthorityPreflightFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "{error}"),
            Self::PrincipalMismatch => {
                formatter.write_str("Cedar principal differs from the authenticated principal")
            }
            Self::ReservedContext => formatter
                .write_str("caller-supplied Cedar context uses the reserved authentication field"),
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
    pub authorization: CedarEvaluation,
    pub authorization_context: Value,
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
/// Authentication context is derived from the verified observation and added
/// under the reserved `authentication` field. Callers cannot provide or
/// override that field.
pub fn preflight_authority_action<A: AuthenticationAdapter>(
    adapter: &mut A,
    request: &AuthenticationRequest,
    state: &RuntimeSessionState,
    authorization: &CedarEvaluationInput,
) -> Result<AuthorityPreflightResult, AuthorityPreflightFailure> {
    if authorization.principal_class != request.principal_class
        || authorization.principal != cedar_principal(request)
    {
        return Err(AuthorityPreflightFailure::PrincipalMismatch);
    }
    let mut evaluated_input = authorization.clone();
    let context = evaluated_input.context.as_object_mut().ok_or_else(|| {
        AuthorityPreflightFailure::AuthorizationInvalid(vec![
            "Cedar context must be an object".into(),
        ])
    })?;
    if context.contains_key("authentication") {
        return Err(AuthorityPreflightFailure::ReservedContext);
    }
    let observation = authenticate_for_transaction(adapter, request, state)
        .map_err(AuthorityPreflightFailure::Authentication)?;
    context.insert(
        "authentication".into(),
        json!({
            "method": enum_value(observation.method),
            "assurance": enum_value(observation.assurance),
            "authenticated_at": observation.authenticated_at,
            "observed_at": observation.observed_at,
            "expires_at": observation.expires_at,
            "user_presence": observation.user_presence,
            "user_verification": observation.user_verification,
            "recovery_recent": observation.recovery_recent,
        }),
    );
    let evaluation = evaluate(&evaluated_input);
    if !evaluation.valid || !evaluation.diagnostics.is_empty() {
        return Err(AuthorityPreflightFailure::AuthorizationInvalid(
            evaluation.diagnostics,
        ));
    }
    if evaluation.decision != CedarDecision::Allow {
        return Err(AuthorityPreflightFailure::AuthorizationDenied);
    }
    Ok(AuthorityPreflightResult {
        authentication: observation,
        authorization: evaluation,
        authorization_context: evaluated_input.context,
    })
}

fn cedar_principal(request: &AuthenticationRequest) -> String {
    let entity_type = match request.principal_class {
        PrincipalClass::Human => "Human",
        PrincipalClass::Agent => "Agent",
        PrincipalClass::Workload => "Workload",
        PrincipalClass::Service => "Service",
        PrincipalClass::Institution => "Institution",
    };
    let identifier = serde_json::to_string(&request.principal_id)
        .expect("serializing a string as JSON cannot fail");
    format!("{entity_type}::{identifier}")
}

fn enum_value<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("closed authentication enum serialization cannot fail")
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

/// One closed platform-owned user-presence ceremony over an exact authority
/// intent. The provider credential stays inside LocalAuthentication, Windows
/// Hello, or polkit; the adapter retains only a bearer-free observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformUserPresenceSession {
    pub principal_id: String,
    pub issuer: String,
    pub subject: String,
    pub session_root: String,
    pub authenticated_at: String,
    pub expires_at: String,
}

impl AuthenticationAdapter for PlatformUserPresenceSession {
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
            method: AuthenticationMethod::PlatformUserPresence,
            assurance: AuthenticationAssurance::MultiFactor,
            session_root: self.session_root.clone(),
            authenticated_at: self.authenticated_at.clone(),
            observed_at: request.transaction_at.clone(),
            expires_at: self.expires_at.clone(),
            user_presence: true,
            user_verification: true,
            recovery_recent: false,
            revocation_ref: None,
        })
    }
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

/// A short-lived local agent session proven by one exact Submission v1.
///
/// The Submission signature binds the producer, claim, artifacts, caveats,
/// requested change, and optional execution binding. Repository authority
/// retains only the full Submission root as a bearer-free observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAgentSubmissionSession {
    actor: String,
    submission_root: String,
}

impl SignedAgentSubmissionSession {
    pub fn from_submission(submission: &SubmissionV1) -> Result<Self, String> {
        submission.verify()?;
        let actor = &submission.provenance.producer;
        if !(actor.starts_with("agent:") || actor.starts_with("ci:")) {
            return Err(
                "signed Submission authentication requires an agent: or ci: producer".into(),
            );
        }
        Ok(Self {
            actor: actor.clone(),
            submission_root: submission.canonical_root()?,
        })
    }
}

impl AuthenticationAdapter for SignedAgentSubmissionSession {
    fn observe(
        &mut self,
        request: &AuthenticationRequest,
    ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
        let authenticated_at = DateTime::parse_from_rfc3339(&request.transaction_at)
            .map_err(|error| {
                AuthenticationFailure::InvalidObservation(format!(
                    "Submission authentication time is invalid: {error}"
                ))
            })?
            .with_timezone(&Utc);
        let canonical_authenticated_at =
            authenticated_at.to_rfc3339_opts(SecondsFormat::Secs, true);
        let expires_at =
            (authenticated_at + Duration::minutes(5)).to_rfc3339_opts(SecondsFormat::Secs, true);
        Ok(AuthenticationObservationV1 {
            schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
            principal_id: self.actor.clone(),
            principal_class: PrincipalClass::Agent,
            issuer: "vela.submission.v1".into(),
            subject: self.actor.clone(),
            method: AuthenticationMethod::AgentRecordSignature,
            assurance: AuthenticationAssurance::SingleFactor,
            session_root: self.submission_root.clone(),
            authenticated_at: canonical_authenticated_at.clone(),
            observed_at: canonical_authenticated_at,
            expires_at,
            user_presence: false,
            user_verification: false,
            recovery_recent: false,
            revocation_ref: None,
        })
    }
}

/// A short-lived local verifier session proven by one exact Verification
/// Record v1. The retained observation commits to the signed record without
/// retaining a key or granting scientific authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedVerificationRecordSession {
    actor: String,
    verification_root: String,
}

impl SignedVerificationRecordSession {
    pub fn from_record(record: &VerificationRecordV1) -> Result<Self, String> {
        record.verify()?;
        let actor = &record.verifier;
        if !(actor.starts_with("agent:")
            || actor.starts_with("ci:")
            || actor.starts_with("verifier:"))
        {
            return Err(
                "signed Verification Record authentication requires an agent:, ci:, or verifier: actor"
                    .into(),
            );
        }
        Ok(Self {
            actor: actor.clone(),
            verification_root: record.canonical_root()?,
        })
    }
}

impl AuthenticationAdapter for SignedVerificationRecordSession {
    fn observe(
        &mut self,
        request: &AuthenticationRequest,
    ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
        let authenticated_at = DateTime::parse_from_rfc3339(&request.transaction_at)
            .map_err(|error| {
                AuthenticationFailure::InvalidObservation(format!(
                    "Verification Record authentication time is invalid: {error}"
                ))
            })?
            .with_timezone(&Utc);
        let canonical_authenticated_at =
            authenticated_at.to_rfc3339_opts(SecondsFormat::Secs, true);
        let expires_at =
            (authenticated_at + Duration::minutes(5)).to_rfc3339_opts(SecondsFormat::Secs, true);
        Ok(AuthenticationObservationV1 {
            schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
            principal_id: self.actor.clone(),
            principal_class: PrincipalClass::Agent,
            issuer: "vela.verification-record.v1".into(),
            subject: self.actor.clone(),
            method: AuthenticationMethod::AgentRecordSignature,
            assurance: AuthenticationAssurance::SingleFactor,
            session_root: self.verification_root.clone(),
            authenticated_at: canonical_authenticated_at.clone(),
            observed_at: canonical_authenticated_at,
            expires_at,
            user_presence: false,
            user_verification: false,
            recovery_recent: false,
            revocation_ref: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use ed25519_dalek::SigningKey;
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

    #[test]
    fn signed_submission_authenticates_at_registration_time_without_becoming_authority() {
        use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
        use vela_protocol::submission_v1::{
            RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft,
            SubmissionProvenance, SubmissionV1,
        };

        let key = SigningKey::from_bytes(&[71_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:submission-auth".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-01-01T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "An old portable Submission remains authentic.".to_string(),
                    claim_type: "theoretical".to_string(),
                    conditions: Vec::new(),
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".to_string(),
                    path: "witness.json".to_string(),
                    digest: format!("sha256:{}", "a".repeat(64)),
                }],
                caveats: vec!["Authentication is not verification.".to_string()],
                replayability: "unknown".to_string(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["independent review".to_string()],
                requested_change: RequestedChange {
                    kind: "add_claim".to_string(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:submission-auth".to_string(),
                    source_system: "fixture".to_string(),
                    source_attempt: None,
                    source_run: None,
                    emitted_at: "2026-01-01T00:00:00Z".to_string(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap();
        let mut session = SignedAgentSubmissionSession::from_submission(&submission).unwrap();
        let observation = session
            .observe(&AuthenticationRequest {
                principal_id: "agent:submission-auth".to_string(),
                principal_class: PrincipalClass::Agent,
                transaction_at: "2026-07-26T00:00:00Z".to_string(),
            })
            .unwrap();
        assert_eq!(observation.issuer, "vela.submission.v1");
        assert_eq!(
            observation.session_root,
            submission.canonical_root().unwrap()
        );
        assert_eq!(observation.authenticated_at, "2026-07-26T00:00:00Z");
        assert!(!observation.user_presence);
        observation.validate().unwrap();
    }

    #[test]
    fn signed_verification_record_authenticates_without_becoming_authority() {
        use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
        use vela_protocol::verification_record::{
            IndependenceDisclosure, VerificationMethod, VerificationRecordDraft,
            VerificationRecordV1, VerificationScope, VerificationSubject,
        };

        let key = SigningKey::from_bytes(&[72_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "verifier:record-auth".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-26T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let record = VerificationRecordV1::build(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id: "vf_0123456789abcdef".to_string(),
                    artifact_ids: Vec::new(),
                    submission_id: "vsb_0123456789abcdef".to_string(),
                    submission_root: root('b'),
                    proposal_id: "vpr_0123456789abcdef".to_string(),
                },
                method: VerificationMethod {
                    profile: "frozen-test".to_string(),
                    implementation: "fixture-verifier".to_string(),
                    environment_root: root('c'),
                },
                scope: VerificationScope {
                    property: "the bounded witness replays".to_string(),
                    does_not_establish: vec!["scientific acceptance".to_string()],
                },
                outcome: "pass".to_string(),
                verifier: "verifier:record-auth".to_string(),
                independence: IndependenceDisclosure {
                    declared_independent_of: vec!["agent:producer".to_string()],
                    shared_dependencies: Vec::new(),
                },
                output_artifact_ids: Vec::new(),
                started_at: "2026-07-26T00:00:00Z".to_string(),
                completed_at: "2026-07-26T00:01:00Z".to_string(),
            },
            identity,
            &key,
        )
        .unwrap();
        let mut session = SignedVerificationRecordSession::from_record(&record).unwrap();
        let observation = session
            .observe(&AuthenticationRequest {
                principal_id: "verifier:record-auth".to_string(),
                principal_class: PrincipalClass::Agent,
                transaction_at: "2026-07-26T12:00:00Z".to_string(),
            })
            .unwrap();
        assert_eq!(observation.issuer, "vela.verification-record.v1");
        assert_eq!(observation.session_root, record.canonical_root().unwrap());
        assert!(!observation.user_presence);
        observation.validate().unwrap();
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

    fn authorization(policy_condition: &str) -> CedarEvaluationInput {
        CedarEvaluationInput {
            schema: r#"
                entity Human;
                entity Proposal;
                action "review_reject" appliesTo {
                    principal: Human,
                    resource: Proposal,
                    context: {
                        exact: Bool,
                        authentication: {
                            method: String,
                            assurance: String,
                            authenticated_at: String,
                            observed_at: String,
                            expires_at: String,
                            user_presence: Bool,
                            user_verification: Bool,
                            recovery_recent: Bool
                        }
                    }
                };
            "#
            .into(),
            policies: format!(
                r#"permit(principal, action, resource) when {{ context.exact && {policy_condition} }};"#
            ),
            entities: serde_json::json!([
                {
                    "uid": {"type": "Human", "id": request().principal_id},
                    "attrs": {},
                    "parents": []
                },
                {
                    "uid": {"type": "Proposal", "id": "vpr_fixture"},
                    "attrs": {},
                    "parents": []
                }
            ]),
            principal: cedar_principal(&request()),
            principal_class: PrincipalClass::Human,
            action: "review_reject".into(),
            resource: r#"Proposal::"vpr_fixture""#.into(),
            context: serde_json::json!({"exact": true}),
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
    fn platform_presence_yields_verified_bearer_free_human_observation() {
        let local = local_session();
        let mut protected = PlatformUserPresenceSession {
            principal_id: local.principal_id,
            issuer: local.issuer,
            subject: local.subject,
            session_root: local.session_root,
            authenticated_at: "2026-07-24T12:00:00Z".into(),
            expires_at: "2026-07-24T12:10:00Z".into(),
        };
        let observation = authenticate_for_transaction(
            &mut protected,
            &request(),
            &RuntimeSessionState::default(),
        )
        .unwrap();
        assert_eq!(
            observation.method,
            AuthenticationMethod::PlatformUserPresence
        );
        assert_eq!(observation.assurance, AuthenticationAssurance::MultiFactor);
        assert!(observation.user_presence);
        assert!(observation.user_verification);
        let encoded = serde_json::to_string(&observation).unwrap();
        assert!(!encoded.contains("bearer"));
        assert!(!encoded.contains("cookie"));
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

    #[test]
    fn authorization_context_is_derived_from_the_verified_observation() {
        let result = preflight_authority_action(
            &mut local_session(),
            &request(),
            &RuntimeSessionState::default(),
            &authorization("!context.authentication.recovery_recent"),
        )
        .unwrap();
        assert_eq!(
            result.authorization_context["authentication"]["method"],
            "local_os_session"
        );
        assert_eq!(
            result.authorization_context["authentication"]["assurance"],
            "local_session"
        );
        assert_eq!(
            result.authorization_context["authentication"]["recovery_recent"],
            false
        );
    }

    #[test]
    fn policy_denial_invalid_input_and_recovery_change_write_nothing() {
        let directory = sentinel();

        assert_eq!(
            preflight_authority_action(
                &mut local_session(),
                &request(),
                &RuntimeSessionState::default(),
                &authorization("context.authentication.recovery_recent"),
            ),
            Err(AuthorityPreflightFailure::AuthorizationDenied)
        );

        let mut invalid = authorization("true");
        invalid.schema = "not cedar".into();
        assert!(matches!(
            preflight_authority_action(
                &mut local_session(),
                &request(),
                &RuntimeSessionState::default(),
                &invalid,
            ),
            Err(AuthorityPreflightFailure::AuthorizationInvalid(_))
        ));

        let mut recovered = local_session();
        recovered.recovery_recent = true;
        assert_eq!(
            preflight_authority_action(
                &mut recovered,
                &request(),
                &RuntimeSessionState::default(),
                &authorization("!context.authentication.recovery_recent"),
            ),
            Err(AuthorityPreflightFailure::AuthorizationDenied)
        );
        assert_sentinel_unchanged(&directory);
    }

    #[test]
    fn caller_cannot_spoof_authentication_context_or_cedar_principal() {
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

        let mut spoofed_context = authorization("true");
        spoofed_context.context["authentication"] = serde_json::json!({"recovery_recent": false});
        let mut adapter = MustNotAuthenticate::default();
        assert_eq!(
            preflight_authority_action(
                &mut adapter,
                &request(),
                &RuntimeSessionState::default(),
                &spoofed_context,
            ),
            Err(AuthorityPreflightFailure::ReservedContext)
        );
        assert!(!adapter.called);

        let mut wrong_principal = authorization("true");
        wrong_principal.principal = r#"Human::"other""#.into();
        let mut adapter = MustNotAuthenticate::default();
        assert_eq!(
            preflight_authority_action(
                &mut adapter,
                &request(),
                &RuntimeSessionState::default(),
                &wrong_principal,
            ),
            Err(AuthorityPreflightFailure::PrincipalMismatch)
        );
        assert!(!adapter.called);
    }
}
