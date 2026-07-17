//! One-shot human decision signing for Vela.
//!
//! This crate is a local product boundary, not a frontier protocol. The helper
//! accepts one closed request, displays one exact decision card, signs only the
//! request's validated event cores, returns one response, and exits.

pub mod contract;
pub mod helper;
pub mod session;
pub mod system;

pub use contract::{
    EnrollmentRequest, EnrollmentResponse, EventSignature, ProtectionMode, SignerEvent,
    SignerRequest, SignerResponse, request_root, validate_request, validate_response,
};
pub use helper::{Approval, Custody, approve_and_sign, enroll};
pub use session::{SessionRecord, SessionState};
