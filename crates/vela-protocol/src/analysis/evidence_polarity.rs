//! Evidence polarity: which way a proposed operation pushes the record.
//!
//! A reviewer deciding a pack needs to know at a glance whether the set
//! adds support, removes it, or displaces prior state — before reading
//! any single proposal. This is a pure classification over the proposal's
//! `kind` (and, for `verifier.attach`, its payload outcome); it judges
//! nothing and never feeds a gate. `Neutral` is the honest default for
//! operations that qualify or annotate without pushing either way.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolarity {
    /// Adds a claim or adds support for one (`finding.add`,
    /// `artifact.assert`, a passing `verifier.attach`).
    Supports,
    /// Removes a claim or support (`finding.retract`, `finding.reject`,
    /// a failing `verifier.attach`).
    Refutes,
    /// Displaces prior state rather than arguing with it
    /// (`finding.supersede`).
    Contradicts,
    /// Annotates, qualifies, or repairs without pushing either way
    /// (notes, caveats, reviews, span/locator repairs, traces).
    Neutral,
}

impl EvidencePolarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidencePolarity::Supports => "supports",
            EvidencePolarity::Refutes => "refutes",
            EvidencePolarity::Contradicts => "contradicts",
            EvidencePolarity::Neutral => "neutral",
        }
    }
}

/// Classify one proposal by kind and payload. Every kind the proposal
/// layer accepts maps somewhere; an unknown kind is `Neutral`, never a
/// guess.
#[must_use]
pub fn classify_proposal_polarity(kind: &str, payload: &Value) -> EvidencePolarity {
    match kind {
        "finding.add" | "artifact.assert" => EvidencePolarity::Supports,
        "finding.retract" | "finding.reject" => EvidencePolarity::Refutes,
        "finding.supersede" => EvidencePolarity::Contradicts,
        "verifier.attach" => {
            // The attachment's outcome decides the direction; an absent or
            // unrecognized outcome stays honest.
            match payload
                .get("attachment")
                .and_then(|a| a.get("outcome"))
                .or_else(|| payload.get("outcome"))
                .and_then(Value::as_str)
            {
                Some("passed") | Some("pass") => EvidencePolarity::Supports,
                Some("failed") | Some("fail") => EvidencePolarity::Refutes,
                _ => EvidencePolarity::Neutral,
            }
        }
        _ => EvidencePolarity::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kinds_classify_by_direction() {
        assert_eq!(
            classify_proposal_polarity("finding.add", &json!({})),
            EvidencePolarity::Supports
        );
        assert_eq!(
            classify_proposal_polarity("finding.retract", &json!({})),
            EvidencePolarity::Refutes
        );
        assert_eq!(
            classify_proposal_polarity("finding.supersede", &json!({})),
            EvidencePolarity::Contradicts
        );
        assert_eq!(
            classify_proposal_polarity("finding.note", &json!({})),
            EvidencePolarity::Neutral
        );
        assert_eq!(
            classify_proposal_polarity("something.future", &json!({})),
            EvidencePolarity::Neutral
        );
    }

    #[test]
    fn verifier_attach_follows_its_outcome() {
        assert_eq!(
            classify_proposal_polarity(
                "verifier.attach",
                &json!({"attachment": {"outcome": "passed"}})
            ),
            EvidencePolarity::Supports
        );
        assert_eq!(
            classify_proposal_polarity("verifier.attach", &json!({"outcome": "failed"})),
            EvidencePolarity::Refutes
        );
        assert_eq!(
            classify_proposal_polarity("verifier.attach", &json!({})),
            EvidencePolarity::Neutral
        );
    }
}
