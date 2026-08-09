//! The runtime boundary for repository authority.
//!
//! What this crate was: a restricted, fail-closed Cedar runtime. It carried a
//! policy set, an entity snapshot, a schema validator, a forbidden-extension
//! screen, and the rule that no diagnostic may ever accompany an Allow —
//! roughly four hundred lines whose whole purpose was to make a general policy
//! language behave like the fixed rule underneath it: one authenticated human
//! holding one of two roles may request one of six actions on one exact
//! resource.
//!
//! That rule is now stated directly by `vela_protocol::authorization`, and
//! proven equal to every Cedar decision this ecosystem ever published by
//! `tests/authorization_profile_parity.rs`. What remains here is the part
//! Cedar never did: establishing that there is an authenticated human at all.

pub mod runtime_authentication;

pub use vela_protocol::authority::PrincipalClass;
pub use vela_protocol::authorization::{
    AUTHORIZATION_EVALUATION_SCHEMA_V1, AUTHORIZATION_MODEL_SCHEMA_V1, AUTHORIZATION_PROFILE_V1,
    AUTHORIZATION_REQUEST_SCHEMA_V1, AuthorityActionV1, AuthorityMemberV1, AuthorityResourceTypeV1,
    AuthorityRoleV1, AuthorizationDecisionV1, AuthorizationEvaluationV1, AuthorizationModelV1,
    AuthorizationReasonV1, AuthorizationRequestV1, AuthorizationResourceV1,
    evaluate_authorization_v1,
};
