//! Derived signals, research traces, audits, health, provenance.
//! Re-exported flat (`vela_edge::*`) at the crate root; file organization only.

pub mod actor_registration;
pub mod artifact_audit;
pub mod channel_map;
pub mod decision_brief;
pub mod frontier_health;
pub mod frontier_next;
pub mod frontier_repository;
pub mod git_read;
pub mod provenance_compute;
pub mod repository_write;
pub mod review_backpressure;
pub mod sign_preview;
pub mod sign_queue;
pub mod signals;
pub mod target_index;
pub mod verify;
