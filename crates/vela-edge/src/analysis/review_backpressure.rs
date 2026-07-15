//! Bounded review-queue pressure derived from the pending catalog.
//!
//! This projection consumes only facts retained on every pending proposal. It
//! never asks a caller to invent independence, significance, evidence
//! direction, verifier diversity, reviewer effort, or downstream use. Metrics
//! that need those absent facts are returned as typed missing values instead
//! of plausible-looking zeroes.
//!
//! A true retry of one Vela operation is idempotent and never creates another
//! proposal. [`repeated_exact_work`](ReviewBackpressureMetrics::repeated_exact_work)
//! therefore measures additional queue entries that name the same retained
//! work root for the same claim; it is deliberately not called a retry.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const REVIEW_BACKPRESSURE_SCHEMA: &str = "vela.review-backpressure.testing.v1";

/// Hard ceiling for one deterministic aggregate pass.
pub const MAX_REVIEW_QUEUE_FACTS: usize = 16_384;

/// Maximum UTF-8 byte length of every catalog identity/root field.
pub const MAX_FACT_KEY_BYTES: usize = 160;

/// Independent bound for any caller that selects receipts for a page.
pub const MAX_SELECTED_RECEIPTS_PER_PAGE: usize = 100;

pub const MISSING_VERIFIER_FACTS: &str = "verifier_facts_not_in_pending_catalog";
pub const MISSING_INDEPENDENCE_FACTS: &str = "independence_not_derivable_from_actor_identity";
pub const MISSING_EVIDENCE_DIRECTION: &str = "evidence_direction_not_in_pending_catalog";
pub const MISSING_POLICY_PRIORITY: &str = "policy_priority_not_materialized_for_full_queue";
pub const MISSING_REVIEW_EFFORT: &str = "review_effort_not_in_pending_catalog";
pub const MISSING_CORRECTION_FACTS: &str = "correction_history_not_in_pending_catalog";
pub const MISSING_DOWNSTREAM_USE: &str = "downstream_use_not_in_pending_catalog";

/// One exact, durable pending-catalog row.
///
/// `exact_work_root` is optional because older and non-Receipt proposals do
/// not retain one. The CLI production adapter uses the proposal's declared
/// full Receipt root without opening that Receipt. The aggregate reports
/// coverage rather than substituting a proposal id or another synthetic value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueFact {
    pub proposal_id: String,
    pub claim_key: String,
    pub actor_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_work_root: Option<String>,
    pub submitted_at_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MetricAvailability<T> {
    Measured {
        value: T,
        observed: usize,
        total: usize,
    },
    Partial {
        value: T,
        observed: usize,
        total: usize,
        reason_code: String,
    },
    Missing {
        reason_code: String,
    },
}

impl<T> MetricAvailability<T> {
    #[must_use]
    pub fn missing(reason_code: &str) -> Self {
        Self::Missing {
            reason_code: reason_code.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBackpressureThresholds {
    pub elevated_queue_depth: usize,
    pub critical_queue_depth: usize,
    pub elevated_oldest_age_seconds: u64,
    pub critical_oldest_age_seconds: u64,
    pub elevated_actor_queue_depth: usize,
    pub critical_actor_queue_depth: usize,
}

impl Default for ReviewBackpressureThresholds {
    fn default() -> Self {
        Self {
            elevated_queue_depth: 64,
            critical_queue_depth: 512,
            elevated_oldest_age_seconds: 2 * 24 * 60 * 60,
            critical_oldest_age_seconds: 7 * 24 * 60 * 60,
            elevated_actor_queue_depth: 16,
            critical_actor_queue_depth: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackpressureLevel {
    Normal,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorPressureMetrics {
    pub pending_actors: usize,
    pub largest_actor_queue_depth: usize,
    pub largest_actor_share_bps: u64,
    pub pressured_actors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBackpressureMetrics {
    pub queue_depth: usize,
    pub claims: usize,
    pub oldest_age_seconds: u64,
    pub actor_pressure: ActorPressureMetrics,
    /// Additional rows naming the same claim and declared full work root.
    /// This measures repeated catalog references, not Receipt availability,
    /// evidence quality, or independent replication.
    pub repeated_exact_work: MetricAvailability<usize>,
    pub verifier_class_diversity: MetricAvailability<usize>,
    pub independent_replications: MetricAvailability<usize>,
    pub conflicting_claims: MetricAvailability<usize>,
    pub low_value_work: MetricAvailability<usize>,
    pub critical_exceptions: MetricAvailability<usize>,
    pub reviewer_minutes: MetricAvailability<u64>,
    pub correction_latency_seconds: MetricAvailability<u64>,
    pub downstream_uses: MetricAvailability<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBackpressureReport {
    pub schema: String,
    pub stability: String,
    pub as_of_seconds: u64,
    pub input_count: usize,
    pub level: BackpressureLevel,
    pub metrics: ReviewBackpressureMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactViolation {
    Empty,
    TooLong,
    TimestampAfterReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewBackpressureError {
    InputLimitExceeded {
        actual: usize,
        maximum: usize,
    },
    InvalidThresholds,
    InvalidFact {
        index: usize,
        field: &'static str,
        violation: FactViolation,
    },
    DuplicateProposalId {
        index: usize,
    },
}

impl fmt::Display for ReviewBackpressureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputLimitExceeded { actual, maximum } => {
                write!(
                    formatter,
                    "review queue has {actual} facts; maximum is {maximum}"
                )
            }
            Self::InvalidThresholds => write!(formatter, "backpressure thresholds are invalid"),
            Self::InvalidFact {
                index,
                field,
                violation,
            } => write!(
                formatter,
                "review queue fact {index} has invalid {field}: {violation:?}"
            ),
            Self::DuplicateProposalId { index } => {
                write!(formatter, "review queue fact {index} repeats a proposal id")
            }
        }
    }
}

impl std::error::Error for ReviewBackpressureError {}

/// Aggregate a bounded pending catalog without reading external state or a
/// wall clock. Input ordering does not affect the report.
pub fn review_backpressure(
    facts: &[ReviewQueueFact],
    as_of_seconds: u64,
    thresholds: ReviewBackpressureThresholds,
) -> Result<ReviewBackpressureReport, ReviewBackpressureError> {
    if facts.len() > MAX_REVIEW_QUEUE_FACTS {
        return Err(ReviewBackpressureError::InputLimitExceeded {
            actual: facts.len(),
            maximum: MAX_REVIEW_QUEUE_FACTS,
        });
    }
    validate_thresholds(thresholds)?;

    let mut proposal_ids = BTreeSet::new();
    let mut claims = BTreeSet::new();
    let mut actor_queue: BTreeMap<&str, usize> = BTreeMap::new();
    let mut exact_work = BTreeSet::new();
    let mut exact_work_observed = 0usize;
    let mut repeated_exact_work = 0usize;
    let mut oldest_age_seconds = 0u64;

    for (index, fact) in facts.iter().enumerate() {
        validate_fact(fact, index, as_of_seconds)?;
        if !proposal_ids.insert(fact.proposal_id.as_str()) {
            return Err(ReviewBackpressureError::DuplicateProposalId { index });
        }
        claims.insert(fact.claim_key.as_str());
        *actor_queue.entry(fact.actor_id.as_str()).or_default() += 1;
        oldest_age_seconds =
            oldest_age_seconds.max(as_of_seconds.saturating_sub(fact.submitted_at_seconds));
        if let Some(root) = fact.exact_work_root.as_deref() {
            exact_work_observed += 1;
            if !exact_work.insert((fact.claim_key.as_str(), root)) {
                repeated_exact_work += 1;
            }
        }
    }

    let queue_depth = facts.len();
    let largest_actor_queue_depth = actor_queue.values().copied().max().unwrap_or(0);
    let actor_pressure = ActorPressureMetrics {
        pending_actors: actor_queue.len(),
        largest_actor_queue_depth,
        largest_actor_share_bps: basis_points(largest_actor_queue_depth, queue_depth),
        pressured_actors: actor_queue
            .values()
            .filter(|depth| **depth >= thresholds.elevated_actor_queue_depth)
            .count(),
    };
    let repeated_exact_work = if exact_work_observed == 0 {
        MetricAvailability::missing("exact_work_root_absent")
    } else if exact_work_observed < queue_depth {
        MetricAvailability::Partial {
            value: repeated_exact_work,
            observed: exact_work_observed,
            total: queue_depth,
            reason_code: "some_pending_rows_have_no_exact_work_root".to_string(),
        }
    } else {
        MetricAvailability::Measured {
            value: repeated_exact_work,
            observed: exact_work_observed,
            total: queue_depth,
        }
    };

    let metrics = ReviewBackpressureMetrics {
        queue_depth,
        claims: claims.len(),
        oldest_age_seconds,
        actor_pressure,
        repeated_exact_work,
        verifier_class_diversity: MetricAvailability::missing(MISSING_VERIFIER_FACTS),
        independent_replications: MetricAvailability::missing(MISSING_INDEPENDENCE_FACTS),
        conflicting_claims: MetricAvailability::missing(MISSING_EVIDENCE_DIRECTION),
        low_value_work: MetricAvailability::missing(MISSING_POLICY_PRIORITY),
        critical_exceptions: MetricAvailability::missing(MISSING_POLICY_PRIORITY),
        reviewer_minutes: MetricAvailability::missing(MISSING_REVIEW_EFFORT),
        correction_latency_seconds: MetricAvailability::missing(MISSING_CORRECTION_FACTS),
        downstream_uses: MetricAvailability::missing(MISSING_DOWNSTREAM_USE),
    };
    let level = classify_level(&metrics, thresholds);

    Ok(ReviewBackpressureReport {
        schema: REVIEW_BACKPRESSURE_SCHEMA.to_string(),
        stability: "testing".to_string(),
        as_of_seconds,
        input_count: facts.len(),
        level,
        metrics,
    })
}

fn basis_points(numerator: usize, denominator: usize) -> u64 {
    if denominator == 0 {
        0
    } else {
        (numerator as u64 * 10_000) / denominator as u64
    }
}

fn classify_level(
    metrics: &ReviewBackpressureMetrics,
    thresholds: ReviewBackpressureThresholds,
) -> BackpressureLevel {
    if metrics.queue_depth >= thresholds.critical_queue_depth
        || metrics.oldest_age_seconds >= thresholds.critical_oldest_age_seconds
        || metrics.actor_pressure.largest_actor_queue_depth >= thresholds.critical_actor_queue_depth
    {
        BackpressureLevel::Critical
    } else if metrics.queue_depth >= thresholds.elevated_queue_depth
        || metrics.oldest_age_seconds >= thresholds.elevated_oldest_age_seconds
        || metrics.actor_pressure.largest_actor_queue_depth >= thresholds.elevated_actor_queue_depth
    {
        BackpressureLevel::Elevated
    } else {
        BackpressureLevel::Normal
    }
}

fn validate_thresholds(
    thresholds: ReviewBackpressureThresholds,
) -> Result<(), ReviewBackpressureError> {
    let valid = thresholds.elevated_queue_depth > 0
        && thresholds.elevated_queue_depth < thresholds.critical_queue_depth
        && thresholds.elevated_oldest_age_seconds > 0
        && thresholds.elevated_oldest_age_seconds < thresholds.critical_oldest_age_seconds
        && thresholds.elevated_actor_queue_depth > 0
        && thresholds.elevated_actor_queue_depth < thresholds.critical_actor_queue_depth;
    if valid {
        Ok(())
    } else {
        Err(ReviewBackpressureError::InvalidThresholds)
    }
}

fn validate_fact(
    fact: &ReviewQueueFact,
    index: usize,
    as_of_seconds: u64,
) -> Result<(), ReviewBackpressureError> {
    for (field, value) in [
        ("proposal_id", Some(fact.proposal_id.as_str())),
        ("claim_key", Some(fact.claim_key.as_str())),
        ("actor_id", Some(fact.actor_id.as_str())),
        ("exact_work_root", fact.exact_work_root.as_deref()),
    ] {
        let Some(value) = value else {
            continue;
        };
        let violation = if value.is_empty() {
            Some(FactViolation::Empty)
        } else if value.len() > MAX_FACT_KEY_BYTES {
            Some(FactViolation::TooLong)
        } else {
            None
        };
        if let Some(violation) = violation {
            return Err(ReviewBackpressureError::InvalidFact {
                index,
                field,
                violation,
            });
        }
    }
    if fact.submitted_at_seconds > as_of_seconds {
        return Err(ReviewBackpressureError::InvalidFact {
            index,
            field: "submitted_at_seconds",
            violation: FactViolation::TimestampAfterReference,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AS_OF: u64 = 2_000_000_000;

    fn mixed_fact(index: usize) -> ReviewQueueFact {
        let group = index / 4;
        let slot = index % 4;
        ReviewQueueFact {
            proposal_id: if index == 0 {
                "p".repeat(MAX_FACT_KEY_BYTES)
            } else {
                format!("vpr_{index:05}")
            },
            claim_key: format!("finding:claim_{group:05}"),
            actor_id: format!("agent:{:03}", index % 128),
            exact_work_root: (!index.is_multiple_of(13)).then(|| {
                let root_slot = if slot < 2 { 0 } else { slot };
                format!("sha256:{group:064x}:{root_slot}")
            }),
            submitted_at_seconds: AS_OF - 100 - index as u64,
        }
    }

    #[test]
    fn mixed_near_limit_catalog_is_stable_bounded_and_honest() {
        let facts: Vec<_> = (0..MAX_REVIEW_QUEUE_FACTS - 1).map(mixed_fact).collect();
        let report = review_backpressure(&facts, AS_OF, ReviewBackpressureThresholds::default())
            .expect("near-limit catalog should aggregate");

        assert_eq!(report.input_count, 16_383);
        assert_eq!(report.schema, REVIEW_BACKPRESSURE_SCHEMA);
        assert_eq!(report.stability, "testing");
        assert_eq!(report.level, BackpressureLevel::Critical);
        assert_eq!(report.metrics.queue_depth, 16_383);
        assert_eq!(report.metrics.claims, 4_096);
        assert_eq!(report.metrics.actor_pressure.pending_actors, 128);
        assert!(matches!(
            report.metrics.repeated_exact_work,
            MetricAvailability::Partial {
                value: 3_465,
                observed: 15_122,
                total: 16_383,
                ..
            }
        ));
        assert_eq!(
            report.metrics.independent_replications,
            MetricAvailability::missing(MISSING_INDEPENDENCE_FACTS)
        );
        assert_eq!(
            report.metrics.critical_exceptions,
            MetricAvailability::missing(MISSING_POLICY_PRIORITY)
        );

        let mut reversed = facts.clone();
        reversed.reverse();
        assert_eq!(
            review_backpressure(&reversed, AS_OF, ReviewBackpressureThresholds::default()).unwrap(),
            report
        );

        let encoded = serde_json::to_vec(&report).unwrap();
        assert!(
            encoded.len() < 2_500,
            "report grew to {} bytes",
            encoded.len()
        );

        let over_limit = vec![mixed_fact(1); MAX_REVIEW_QUEUE_FACTS + 1];
        assert_eq!(
            review_backpressure(&over_limit, AS_OF, ReviewBackpressureThresholds::default()),
            Err(ReviewBackpressureError::InputLimitExceeded {
                actual: MAX_REVIEW_QUEUE_FACTS + 1,
                maximum: MAX_REVIEW_QUEUE_FACTS,
            })
        );
    }

    #[test]
    fn fully_observed_exact_work_is_measured_and_true_retry_ids_are_rejected() {
        let facts = vec![
            ReviewQueueFact {
                proposal_id: "vpr_one".to_string(),
                claim_key: "finding:one".to_string(),
                actor_id: "agent:one".to_string(),
                exact_work_root: Some("sha256:one".to_string()),
                submitted_at_seconds: AS_OF - 10,
            },
            ReviewQueueFact {
                proposal_id: "vpr_two".to_string(),
                claim_key: "finding:one".to_string(),
                actor_id: "agent:two".to_string(),
                exact_work_root: Some("sha256:one".to_string()),
                submitted_at_seconds: AS_OF - 5,
            },
        ];
        let report =
            review_backpressure(&facts, AS_OF, ReviewBackpressureThresholds::default()).unwrap();
        assert_eq!(
            report.metrics.repeated_exact_work,
            MetricAvailability::Measured {
                value: 1,
                observed: 2,
                total: 2,
            }
        );

        let mut retry = facts;
        retry[1].proposal_id = retry[0].proposal_id.clone();
        assert_eq!(
            review_backpressure(&retry, AS_OF, ReviewBackpressureThresholds::default()),
            Err(ReviewBackpressureError::DuplicateProposalId { index: 1 })
        );
    }

    #[test]
    fn pressure_transitions_and_input_boundaries_are_typed() {
        let thresholds = ReviewBackpressureThresholds::default();
        assert_eq!(
            review_backpressure(&[], AS_OF, thresholds).unwrap().level,
            BackpressureLevel::Normal
        );

        let elevated: Vec<_> = (0..thresholds.elevated_queue_depth)
            .map(|index| {
                let mut fact = mixed_fact(index);
                fact.proposal_id = format!("elevated_{index}");
                fact.claim_key = format!("claim_{index}");
                fact.actor_id = format!("actor_{index}");
                fact
            })
            .collect();
        assert_eq!(
            review_backpressure(&elevated, AS_OF, thresholds)
                .unwrap()
                .level,
            BackpressureLevel::Elevated
        );

        let boundary = "x".repeat(MAX_FACT_KEY_BYTES);
        let fact = ReviewQueueFact {
            proposal_id: boundary.clone(),
            claim_key: boundary.clone(),
            actor_id: boundary.clone(),
            exact_work_root: Some(boundary),
            submitted_at_seconds: AS_OF,
        };
        review_backpressure(std::slice::from_ref(&fact), AS_OF, thresholds).unwrap();

        for field in ["proposal_id", "claim_key", "actor_id", "exact_work_root"] {
            let mut oversized = fact.clone();
            let value = "x".repeat(MAX_FACT_KEY_BYTES + 1);
            match field {
                "proposal_id" => oversized.proposal_id = value,
                "claim_key" => oversized.claim_key = value,
                "actor_id" => oversized.actor_id = value,
                "exact_work_root" => oversized.exact_work_root = Some(value),
                _ => unreachable!(),
            }
            assert_eq!(
                review_backpressure(&[oversized], AS_OF, thresholds),
                Err(ReviewBackpressureError::InvalidFact {
                    index: 0,
                    field,
                    violation: FactViolation::TooLong,
                })
            );
        }

        let mut future = fact;
        future.submitted_at_seconds = AS_OF + 1;
        assert_eq!(
            review_backpressure(&[future], AS_OF, thresholds),
            Err(ReviewBackpressureError::InvalidFact {
                index: 0,
                field: "submitted_at_seconds",
                violation: FactViolation::TimestampAfterReference,
            })
        );
    }
}
