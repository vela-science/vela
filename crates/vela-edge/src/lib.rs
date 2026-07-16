//! Vela edge layer: significance, curation, ingestion, search, operations.
//! Depends on vela-protocol (the waist); never the reverse.

mod validation;
pub use validation::{
    conformance, deliverable_grade, lint, normalize, permission, state_integrity, validate,
};
mod analysis;
pub use analysis::{
    actor_registration, artifact_audit, channel_map, decision_brief, frontier_health,
    frontier_next, provenance_compute, research_trace, review_backpressure, sign_preview,
    sign_queue, signals, verify,
};
mod packaging;
pub use packaging::{export, packet, proof_packet};
mod registry;
pub use registry::{frontier_release, incremental_ingest, index_db_schema, tool_registry};
mod review;
pub use review::{agent_attestation, governance, lean_anchors, reviewer_identity};
mod mcp;
pub use mcp::{doctor, vela_agent_mcp};
