//! Vela edge layer: significance, curation, ingestion, search, operations.
//! Depends on vela-protocol (the waist); never the reverse.

mod validation;
pub use validation::{
    conformance, deliverable_grade, lint, normalize, permission, state_integrity, validate,
};
mod analysis;
pub use analysis::{
    actor_registration, artifact_audit, channel_map, decision_brief, frontier_health,
    frontier_next, frontier_repository, git_read, provenance_compute, repository_write,
    review_backpressure, sign_preview, sign_queue, signals, target_index, verify,
};
mod registry;
pub use registry::incremental_ingest;
mod review;
pub use review::{governance, reviewer_identity};
pub mod agent_identity;
