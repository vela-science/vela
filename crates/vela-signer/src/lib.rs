//! One-shot human decision signing for Vela.
//!
//! This crate is a local product boundary, not a frontier protocol. The helper
//! accepts one closed request, displays one exact decision card, signs only the
//! request's validated event cores, returns one response, and exits.

pub mod contract;
pub mod helper;
pub mod policy_contract;
pub mod session;
pub mod system;

pub use contract::{
    EnrollmentRequest, EnrollmentResponse, EventSignature, ProtectionMode, RebindPurpose,
    RebindRequest, RebindResponse, SignerDisplay, SignerEvent, SignerRequest, SignerResponse,
    rebind_request_root, request_root, validate_rebind_request, validate_rebind_response,
    validate_request, validate_response,
};
pub use helper::{Approval, Custody, approve_and_sign, approve_and_sign_policy, enroll, rebind};
pub use policy_contract::{
    POLICY_REQUEST_SCHEMA, POLICY_RESPONSE_SCHEMA, PolicyAuthorityDiff, PolicyDecisionAction,
    PolicySignerRequest, PolicySignerResponse, policy_authority_diff, policy_request_root,
    policy_signer_display, validate_policy_request, validate_policy_response,
};
pub use session::{SessionRecord, SessionState};
