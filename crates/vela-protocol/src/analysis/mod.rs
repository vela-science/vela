//! Derived, non-authoritative projections over the log: transfer, graph, diffs, contradictions, boundaries.
//! Re-exported flat at the crate root; this grouping is file organization only.

pub mod contradiction;
pub mod credit;
pub mod diff;
pub mod diff_pack_review;
pub mod evidence_ci;
pub mod evidence_diff;
pub mod evidence_polarity;
pub mod frontier_graph;
pub mod independence;
pub mod inspect_adapter;
pub mod propagate;
pub mod released_diff_pack;
pub mod scientific_diff;
pub mod status_provenance;
pub mod transfer;
pub mod verdict_conflict;
pub mod verifier_attachment;
