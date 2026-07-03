//! Derived signals, research traces, audits, health, provenance.
//! Re-exported flat (`vela_edge::*`) at the crate root; file organization only.

pub mod artifact_audit;
pub mod channel_map;
pub mod frontier_health;
pub mod frontier_next;
pub mod provenance_compute;
pub mod research_trace;
pub mod sign_queue;
pub mod signals;
pub mod verify;
