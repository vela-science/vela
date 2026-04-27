//! v0.34: Calibration scoring over resolved predictions.
//!
//! A `Prediction` carries the predictor's confidence in the expected
//! outcome (a number on `[0, 1]`). When a `Resolution` records what
//! actually happened, the resolver also records `matched_expected`
//! (a bool). Together those two facts let us compute, per actor:
//!
//! - **Hit rate**: fraction of resolved predictions that matched.
//! - **Brier score**: mean of `(confidence - matched)^2` across the
//!   resolved subset, where `matched ∈ {0, 1}`. Lower is better.
//!   Brier = 0 means perfect calibration; 0.25 is a chance-level
//!   binary predictor; 1.0 is maximally wrong.
//! - **Log score**: mean of `log(p_assigned_to_actual_outcome)`. We
//!   clip to `[1e-9, 1 - 1e-9]` to avoid `-∞`. Higher (closer to 0)
//!   is better.
//!
//! These are derived signals — never written to disk, always
//! recomputed from the canonical `predictions` and `resolutions`
//! collections. That keeps the kernel ledger source-of-truth and
//! avoids stale calibration cache concerns.
//!
//! Calibration is the move that makes Vela an epistemic ledger
//! rather than a knowledge graph: every actor accumulates a public,
//! reproducible track record of how well their stated beliefs match
//! reality.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::bundle::{Prediction, Resolution};

/// Per-actor calibration summary computed over the resolved subset of
/// the actor's predictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRecord {
    /// Stable actor id (e.g. `reviewer:will-blair`).
    pub actor: String,
    /// Total predictions made by this actor in the frontier.
    pub n_predictions: usize,
    /// Predictions that have been resolved (have an associated
    /// `Resolution`). Open predictions don't contribute to scoring.
    pub n_resolved: usize,
    /// Resolved predictions whose `matched_expected = true`.
    pub n_hit: usize,
    /// Hit rate over resolved (or `None` if `n_resolved == 0`).
    pub hit_rate: Option<f64>,
    /// Brier score, lower is better. `None` if no resolutions.
    pub brier_score: Option<f64>,
    /// Log score, higher (closer to 0) is better. `None` if no resolutions.
    pub log_score: Option<f64>,
    /// Bucketed reliability diagram: for each predicted-confidence
    /// band, the observed match rate. Empty bands are omitted.
    /// Format: `(confidence_lower_bound, observed_hit_rate, n_in_band)`.
    pub reliability_buckets: Vec<(f64, f64, usize)>,
}

/// Compute calibration records for every actor that has at least one
/// prediction in the frontier.
pub fn calibration_records(
    predictions: &[Prediction],
    resolutions: &[Resolution],
) -> Vec<CalibrationRecord> {
    // Index resolutions by prediction_id for cheap lookup.
    let mut resolution_by_pred: HashMap<&str, &Resolution> = HashMap::new();
    for r in resolutions {
        resolution_by_pred.insert(r.prediction_id.as_str(), r);
    }

    // Group predictions by actor.
    let mut by_actor: HashMap<String, Vec<&Prediction>> = HashMap::new();
    for p in predictions {
        by_actor.entry(p.made_by.clone()).or_default().push(p);
    }

    let mut out = Vec::with_capacity(by_actor.len());
    for (actor, preds) in by_actor {
        let n_predictions = preds.len();
        let mut resolved: Vec<(&Prediction, &Resolution)> = Vec::new();
        for p in &preds {
            if let Some(r) = resolution_by_pred.get(p.id.as_str()) {
                resolved.push((p, r));
            }
        }
        let n_resolved = resolved.len();
        let n_hit = resolved.iter().filter(|(_, r)| r.matched_expected).count();
        let hit_rate = if n_resolved > 0 {
            Some(n_hit as f64 / n_resolved as f64)
        } else {
            None
        };

        // Brier: mean of (confidence - matched_int)^2.
        let brier_score = if n_resolved > 0 {
            let sum: f64 = resolved
                .iter()
                .map(|(p, r)| {
                    let m = if r.matched_expected { 1.0 } else { 0.0 };
                    (p.confidence - m).powi(2)
                })
                .sum();
            Some(sum / n_resolved as f64)
        } else {
            None
        };

        // Log score: mean log(p_actual). For matched, p_actual = confidence;
        // for not matched, p_actual = (1 - confidence). Clipped.
        let log_score = if n_resolved > 0 {
            let sum: f64 = resolved
                .iter()
                .map(|(p, r)| {
                    let p_actual = if r.matched_expected {
                        p.confidence
                    } else {
                        1.0 - p.confidence
                    };
                    p_actual.clamp(1e-9, 1.0 - 1e-9).ln()
                })
                .sum();
            Some(sum / n_resolved as f64)
        } else {
            None
        };

        // Reliability buckets: 5 bands of width 0.2, omit empty bands.
        let bands: [(f64, f64); 5] = [
            (0.0, 0.2),
            (0.2, 0.4),
            (0.4, 0.6),
            (0.6, 0.8),
            (0.8, 1.001),
        ];
        let mut reliability_buckets: Vec<(f64, f64, usize)> = Vec::new();
        for (lo, hi) in bands {
            let in_band: Vec<&(&Prediction, &Resolution)> = resolved
                .iter()
                .filter(|(p, _)| p.confidence >= lo && p.confidence < hi)
                .collect();
            if in_band.is_empty() {
                continue;
            }
            let hits = in_band.iter().filter(|(_, r)| r.matched_expected).count();
            let observed = hits as f64 / in_band.len() as f64;
            reliability_buckets.push((lo, observed, in_band.len()));
        }

        out.push(CalibrationRecord {
            actor,
            n_predictions,
            n_resolved,
            n_hit,
            hit_rate,
            brier_score,
            log_score,
            reliability_buckets,
        });
    }

    // Stable order: by actor id alphabetically.
    out.sort_by(|a, b| a.actor.cmp(&b.actor));
    out
}

/// Convenience: calibration for a single actor.
pub fn calibration_for_actor(
    actor: &str,
    predictions: &[Prediction],
    resolutions: &[Resolution],
) -> Option<CalibrationRecord> {
    calibration_records(predictions, resolutions)
        .into_iter()
        .find(|r| r.actor == actor)
}
