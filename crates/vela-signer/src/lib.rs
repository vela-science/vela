//! One-shot human decision signing for Vela.
//!
//! This crate is a local product boundary, not a frontier protocol. The helper
//! accepts one closed request, displays one exact decision card, signs only the
//! request's validated event cores, returns one response, and exits.

pub mod actor_contract;
pub mod contract;
pub mod helper;
pub mod policy_contract;
pub mod repository_contract;
pub mod session;
pub mod system;

pub use actor_contract::{
    ACTOR_BOOTSTRAP_REQUEST_SCHEMA, ACTOR_BOOTSTRAP_RESPONSE_SCHEMA, ActorBootstrapDisplay,
    ActorBootstrapProofRequest, ActorBootstrapProofResponse, actor_bootstrap_prompt,
    actor_bootstrap_request_root, actor_bootstrap_response_signing_bytes, actor_record_root,
    actor_registry_file_root, validate_actor_bootstrap_request,
    validate_actor_bootstrap_request_fresh, validate_actor_bootstrap_response,
};
pub use contract::{
    EnrollmentRequest, EnrollmentResponse, EventSignature, ProtectionMode, RebindPurpose,
    RebindRequest, RebindResponse, SignerDisplay, SignerEvent, SignerRequest, SignerResponse,
    rebind_request_root, request_root, validate_rebind_request, validate_rebind_response,
    validate_request, validate_response,
};
pub use helper::{
    Approval, Custody, approve_and_sign, approve_and_sign_policy,
    approve_and_sign_repository_boundary, enroll, prove_actor_bootstrap, rebind,
};
pub use policy_contract::{
    POLICY_REQUEST_SCHEMA, POLICY_RESPONSE_SCHEMA, PolicyAuthorityDiff, PolicyDecisionAction,
    PolicySignerRequest, PolicySignerResponse, policy_authority_diff, policy_request_root,
    policy_signer_display, validate_policy_request, validate_policy_response,
};
pub use repository_contract::{
    REPOSITORY_REQUEST_SCHEMA, REPOSITORY_RESPONSE_SCHEMA, RepositoryBoundaryDisplay,
    RepositoryBoundarySignerRequest, RepositoryBoundarySignerResponse,
    repository_boundary_request_root, validate_repository_boundary_request,
    validate_repository_boundary_request_fresh, validate_repository_boundary_response,
};
pub use session::{SessionRecord, SessionState};
