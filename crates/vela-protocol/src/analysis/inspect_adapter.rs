//! Inspect-AI eval log → verifier attachment (`vva_`).
//!
//! The first external attachment *source* beyond the built-in frozen verifier
//! (vela-verify) and the Lean kernel. It closes the receipt boundary's demo
//! case: an [Inspect-AI](https://inspect.aisi.org.uk/) eval log — a task run
//! against a scorer — becomes a content-addressed [`VerifierAttachment`] bound
//! to the exact claim the target finding asserts.
//!
//! ## Evidence, not a verdict — by construction
//!
//! An Inspect eval is an *evaluation harness*, often with a model-graded
//! scorer. It is NOT a frozen exact verifier. This adapter therefore never
//! self-certifies:
//!
//! - The attachment's [`VerifierMethod`] is [`VerifierMethod::EvalHarness`], a
//!   method distinct from `ComputationalSearch`/`LeanKernel`, so the gate's G1
//!   independence sees it as its own kind of evidence.
//! - [`MethodIntegrity`] is left at the default `Unattested`. The exact-lane
//!   auto-admit predicate requires `Sound` on every matched attachment, so an
//!   Inspect attachment can NEVER auto-admit a finding — a human must clear it,
//!   exactly the "attested by CI, never reproduced" posture of the CI-proof
//!   source.
//! - Adversarial probes are populated ONLY from samples the log itself marks
//!   adversarial (`metadata.adversarial == true`). A plain accuracy eval with
//!   no adversarial sample yields an attachment with no probe, which fails the
//!   gate's G3 on its own — it records the check, it does not verify the claim.
//!
//! A lone Inspect attachment fails G1 (needs >=2 matched independent). The gate
//! (G1-G5) and the human key still decide. Not every Inspect score is
//! admissible verification; the frontier policy decides what a passing
//! `eval_harness` attachment is worth.
//!
//! ## The log shape it reads
//!
//! Inspect's `.eval` logs are archives, but Inspect also emits/serializes the
//! same `EvalLog` as JSON (`inspect log dump`, the older `.json` writer). This
//! parser is tolerant: it reads the fields it needs and ignores the rest, so
//! it survives Inspect's schema evolution.
//!
//! ```jsonc
//! {
//!   "status": "success",
//!   "eval": { "task": "erdos_sidon_bound", "model": "openai/gpt-4o",
//!             "dataset": {"name": "sidon-a17"}, "scorers": [{"name": "match"}] },
//!   "results": { "total_samples": 1, "completed_samples": 1,
//!                "scores": [{"name": "match", "scorer": "match",
//!                            "metrics": {"accuracy": {"value": 1.0}}}] },
//!   "samples": [{"id": "s1", "score": {"value": 1.0},
//!                "metadata": {"adversarial": true}}]
//! }
//! ```

use crate::verifier_attachment::{
    AdversarialProbe, AttachmentDraft, AttachmentOutcome, MatchToClaim, ProbeKind, ProbeResult,
    VerifierMethod,
};
use serde::Deserialize;

/// The threshold below which a headline eval score is treated as a fail. An
/// exact-match single-sample task passes at 1.0; a caller can loosen it.
pub const DEFAULT_PASS_THRESHOLD: f64 = 1.0;

/// The minimal Inspect `EvalLog` shape this adapter reads. Every field is
/// optional/tolerant so an evolving Inspect schema still parses.
#[derive(Debug, Clone, Deserialize)]
pub struct InspectLog {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub eval: InspectEvalSpec,
    #[serde(default)]
    pub results: InspectResults,
    #[serde(default)]
    pub samples: Vec<InspectSample>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectEvalSpec {
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub dataset: InspectDataset,
    #[serde(default)]
    pub scorers: Vec<InspectScorer>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectDataset {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectScorer {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectResults {
    #[serde(default)]
    pub total_samples: u64,
    #[serde(default)]
    pub completed_samples: u64,
    #[serde(default)]
    pub scores: Vec<InspectScore>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectScore {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub scorer: String,
    #[serde(default)]
    pub metrics: std::collections::BTreeMap<String, InspectMetric>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectMetric {
    #[serde(default)]
    pub value: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectSample {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub score: InspectSampleScore,
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InspectSampleScore {
    #[serde(default)]
    pub value: f64,
}

impl InspectLog {
    /// The scorer name — the tool that produced the judgment. Prefers the
    /// results score's `scorer`, then its `name`, then the eval's declared
    /// scorer. `"scorer"` is the neutral fallback.
    #[must_use]
    pub fn scorer_name(&self) -> String {
        self.results
            .scores
            .first()
            .map(|s| {
                if !s.scorer.is_empty() {
                    s.scorer.clone()
                } else {
                    s.name.clone()
                }
            })
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.eval
                    .scorers
                    .first()
                    .map(|s| s.name.clone())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "scorer".to_string())
    }

    /// The headline metric value: the first score's `accuracy`, else its first
    /// metric, else `0.0`. This is what `--threshold` compares against.
    #[must_use]
    pub fn headline_value(&self) -> f64 {
        let Some(score) = self.results.scores.first() else {
            return 0.0;
        };
        if let Some(acc) = score.metrics.get("accuracy") {
            return acc.value;
        }
        score
            .metrics
            .values()
            .next()
            .map(|m| m.value)
            .unwrap_or(0.0)
    }
}

/// Build an [`AttachmentDraft`] from a parsed Inspect log, bound to `target`
/// and `claim_digest`. `threshold` is the headline-metric floor for `Passed`
/// ([`DEFAULT_PASS_THRESHOLD`] is exact-match at 1.0).
///
/// The draft is intentionally `Unattested` (the caller must NOT promote it to
/// `Sound`): an eval harness is evidence, not a frozen verifier.
///
/// `source_ref` names where the log came from (a path or uri), carried in the
/// attachment note for provenance.
pub fn draft_from_log(
    log: &InspectLog,
    target: &str,
    claim_digest: String,
    threshold: f64,
    source_ref: &str,
) -> Result<AttachmentDraft, String> {
    let scorer = log.scorer_name();
    let value = log.headline_value();
    let status_ok = log.status.is_empty() || log.status == "success";
    let complete = log.results.total_samples == 0
        || log.results.completed_samples >= log.results.total_samples;
    let passed = status_ok && complete && value >= threshold;

    // Adversarial probes ONLY from samples the log marks adversarial. A passing
    // adversarial sample survived the probe; a failing one refuted the claim.
    // No adversarial sample => no probe (the attachment then fails G3 alone).
    let mut adversarial_probes = Vec::new();
    for s in &log.samples {
        let is_adv = s
            .metadata
            .get("adversarial")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !is_adv {
            continue;
        }
        let survived = s.score.value >= threshold;
        adversarial_probes.push(AdversarialProbe {
            kind: ProbeKind::CounterexampleSearch,
            result: if survived {
                ProbeResult::Survived
            } else {
                ProbeResult::Refuted
            },
            note: format!("inspect adversarial sample {}", sample_id(&s.id)),
            evidence_root: String::new(),
        });
    }

    let model = if log.eval.model.is_empty() {
        "unknown-model".to_string()
    } else {
        log.eval.model.clone()
    };
    let task = if log.eval.task.is_empty() {
        "inspect-task".to_string()
    } else {
        log.eval.task.clone()
    };

    Ok(AttachmentDraft {
        target: target.to_string(),
        claim_digest,
        verifier_method: VerifierMethod::EvalHarness,
        // The tool that produced the check: Inspect + the scorer. Distinct from
        // a frozen solver id, so G1 treats it as its own method/solver.
        solver_id: format!("inspect:{scorer}"),
        independent_of: Vec::new(),
        match_to_claim: MatchToClaim {
            matches: true,
            checker_actor: format!("inspect:{task}"),
        },
        adversarial_probes,
        outcome: if passed {
            AttachmentOutcome::Passed
        } else {
            AttachmentOutcome::Failed
        },
        verifier_actor: format!("inspect:{model}"),
        note: format!(
            "Inspect-AI eval `{task}` (model {model}, scorer {scorer}, score {value:.3} \
             vs threshold {threshold:.3}); source {source_ref}. Evidence only — eval harness, \
             method_integrity unattested; does not by itself verify the claim."
        ),
    })
}

/// Parse a raw Inspect log JSON string into [`InspectLog`].
pub fn parse_log(json: &str) -> Result<InspectLog, String> {
    serde_json::from_str(json).map_err(|e| format!("parse Inspect eval log JSON: {e}"))
}

fn sample_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "?".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifier_attachment::{
        AttachmentOutcome, GateStatus, MethodIntegrity, VerifierAttachment, claim_digest,
        derive_gate_status,
    };

    const SAMPLE: &str = r#"{
        "status": "success",
        "eval": {
            "task": "erdos_sidon_bound",
            "model": "openai/gpt-4o",
            "dataset": {"name": "sidon-a17"},
            "scorers": [{"name": "match"}]
        },
        "results": {
            "total_samples": 1,
            "completed_samples": 1,
            "scores": [{"name": "match", "scorer": "match",
                        "metrics": {"accuracy": {"value": 1.0}}}]
        },
        "samples": [{"id": "s1", "score": {"value": 1.0},
                     "metadata": {"adversarial": true}}]
    }"#;

    #[test]
    fn parses_and_builds_a_passing_unattested_attachment() {
        let log = parse_log(SAMPLE).unwrap();
        let digest = claim_digest("a Sidon set of size 33 in [0,256]");
        let draft =
            draft_from_log(&log, "vf_0123456789abcdef", digest.clone(), 1.0, "run.json").unwrap();
        let att = VerifierAttachment::build(draft).unwrap();
        // Evidence, not a verdict: method is EvalHarness, integrity Unattested.
        assert_eq!(att.verifier_method, VerifierMethod::EvalHarness);
        assert_eq!(att.method_integrity, MethodIntegrity::Unattested);
        assert_eq!(att.outcome, AttachmentOutcome::Passed);
        assert_eq!(att.solver_id, "inspect:match");
        assert!(att.claim_digest == digest);
        // One adversarial sample => one surviving probe.
        assert_eq!(att.adversarial_probes.len(), 1);
        assert_eq!(att.adversarial_probes[0].result, ProbeResult::Survived);
        // Content-addressed id holds.
        att.verify().unwrap();
    }

    #[test]
    fn deterministic_id_from_a_fixed_log() {
        let log = parse_log(SAMPLE).unwrap();
        let digest = claim_digest("claim X");
        let a = VerifierAttachment::build(
            draft_from_log(&log, "vf_0123456789abcdef", digest.clone(), 1.0, "run.json").unwrap(),
        )
        .unwrap();
        let b = VerifierAttachment::build(
            draft_from_log(&log, "vf_0123456789abcdef", digest, 1.0, "run.json").unwrap(),
        )
        .unwrap();
        assert_eq!(a.id, b.id, "same log + target + digest => same vva_ id");
    }

    #[test]
    fn a_lone_inspect_attachment_does_not_verify() {
        // The doctrine: a single eval-harness attachment is evidence, never a
        // verdict. It fails G1 (needs >=2 matched independent) on its own.
        let log = parse_log(SAMPLE).unwrap();
        let digest = claim_digest("claim X");
        let att = VerifierAttachment::build(
            draft_from_log(&log, "vf_0123456789abcdef", digest.clone(), 1.0, "run.json").unwrap(),
        )
        .unwrap();
        let outcome = derive_gate_status(&digest, &[att]);
        assert_eq!(outcome.status, GateStatus::NeedsVerification);
        assert!(outcome.reasons.iter().any(|r| r.starts_with("G1")));
    }

    #[test]
    fn failing_score_yields_a_failed_attachment() {
        let json = SAMPLE.replace("\"value\": 1.0", "\"value\": 0.5");
        let log = parse_log(&json).unwrap();
        let digest = claim_digest("claim X");
        let draft = draft_from_log(&log, "vf_0123456789abcdef", digest, 1.0, "run.json").unwrap();
        assert_eq!(draft.outcome, AttachmentOutcome::Failed);
    }

    #[test]
    fn adversarial_failure_becomes_a_refuting_probe() {
        // A failing adversarial sample refutes: the probe drives Refuted, so an
        // eval that finds a counterexample cannot be spun as support.
        let json = r#"{
            "status": "success",
            "eval": {"task": "t", "model": "m", "scorers": [{"name": "match"}]},
            "results": {"total_samples": 1, "completed_samples": 1,
                        "scores": [{"scorer": "match", "metrics": {"accuracy": {"value": 1.0}}}]},
            "samples": [{"id": 1, "score": {"value": 0.0}, "metadata": {"adversarial": true}}]
        }"#;
        let log = parse_log(json).unwrap();
        let digest = claim_digest("claim X");
        let draft = draft_from_log(&log, "vf_0123456789abcdef", digest, 1.0, "run.json").unwrap();
        assert_eq!(draft.adversarial_probes.len(), 1);
        assert_eq!(draft.adversarial_probes[0].result, ProbeResult::Refuted);
    }

    #[test]
    fn tolerates_a_minimal_log() {
        // Only the fields the adapter needs; everything else absent.
        let json = r#"{"eval": {"task": "t"}, "results": {"scores": []}}"#;
        let log = parse_log(json).unwrap();
        let digest = claim_digest("claim X");
        let draft = draft_from_log(&log, "vf_0123456789abcdef", digest, 1.0, "run.json").unwrap();
        // No score => headline 0.0 => Failed, and no probe.
        assert_eq!(draft.outcome, AttachmentOutcome::Failed);
        assert!(draft.adversarial_probes.is_empty());
        assert_eq!(draft.solver_id, "inspect:scorer");
    }
}
