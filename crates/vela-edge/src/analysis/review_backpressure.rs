//! Bounded review-queue and backpressure projection.
//!
//! This module folds typed queue facts into compact operating metrics. It is a
//! pure derived view: it does not read a clock, load policy, mutate a reducer,
//! or make an authority decision. The caller supplies the reference time, and
//! every classification is exact arithmetic over the supplied facts.
//!
//! The aggregate input is capped by [`MAX_REVIEW_QUEUE_FACTS`], and every
//! identity/root field is length-bounded. The report deliberately contains no
//! receipt rows. A UI or API that selects receipts or builds page material must
//! enforce the separate [`MAX_SELECTED_RECEIPTS_PER_PAGE`] bound; the aggregate
//! input cap is not permission to materialize the whole queue for display.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const REVIEW_BACKPRESSURE_SCHEMA: &str = "vela.review-backpressure.testing.v1";

/// Hard ceiling for one deterministic aggregate pass.
pub const MAX_REVIEW_QUEUE_FACTS: usize = 16_384;

/// Maximum UTF-8 byte length of every fact identity/root field.
pub const MAX_FACT_KEY_BYTES: usize = 160;

/// Independent bound for any caller that selects receipts for a page.
pub const MAX_SELECTED_RECEIPTS_PER_PAGE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierClass {
    LeanKernel,
    FrozenRust,
    ExactPython,
    ContainerReplay,
    InstrumentReplay,
    OtherFrozen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDirection {
    SupportsClaim,
    ChallengesClaim,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkValue {
    Substantive,
    LowValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPriority {
    Routine,
    CriticalException,
}

/// One typed receipt-level fact consumed by the aggregate.
///
/// `independence_key` identifies the producer/config/environment boundary.
/// Distinct boundaries on one claim are independent replications. Repeated
/// work inside one boundary is duplication; when its `exact_work_root` is also
/// identical, it is the stricter exact-retry subset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewQueueFact {
    pub receipt_id: String,
    pub claim_id: String,
    pub actor_id: String,
    pub independence_key: String,
    pub verifier_class: VerifierClass,
    pub exact_work_root: String,
    pub evidence: EvidenceDirection,
    pub value: WorkValue,
    pub priority: ReviewPriority,
    pub submitted_at_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction_requested_at_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_at_seconds: Option<u64>,
    #[serde(default)]
    pub reviewer_minutes: u32,
    #[serde(default)]
    pub downstream_uses: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBackpressureThresholds {
    pub elevated_queue_depth: usize,
    pub critical_queue_depth: usize,
    pub elevated_oldest_age_seconds: u64,
    pub critical_oldest_age_seconds: u64,
    pub elevated_critical_latency_seconds: u64,
    pub critical_critical_latency_seconds: u64,
    pub elevated_correction_latency_seconds: u64,
    pub critical_correction_latency_seconds: u64,
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
            elevated_critical_latency_seconds: 6 * 60 * 60,
            critical_critical_latency_seconds: 24 * 60 * 60,
            elevated_correction_latency_seconds: 3 * 24 * 60 * 60,
            critical_correction_latency_seconds: 10 * 24 * 60 * 60,
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
    pub critical_queue_depth: usize,
    pub oldest_age_seconds: u64,
    /// Age of the oldest unresolved critical exception; zero when none exists.
    pub critical_latency_seconds: u64,
    pub reviewer_minutes: u64,
    pub correction_cases: usize,
    pub open_corrections: usize,
    pub correction_latency_seconds: u64,
    pub mean_correction_latency_seconds: u64,
    pub verifier_class_diversity: usize,
    pub verifier_classes: Vec<VerifierClass>,
    pub claims: usize,
    pub exact_retries: usize,
    pub independent_replications: usize,
    /// Extra same-claim work inside an already represented independence key.
    /// Exact retries are a subset of this count.
    pub duplications: usize,
    /// Independent replications / (replications + duplications), in basis points.
    pub replication_rate_bps: u64,
    pub conflicting_claims: usize,
    pub low_value_work: usize,
    pub repeated_low_value_work: usize,
    pub critical_exceptions: usize,
    pub downstream_uses: u64,
    pub receipts_with_downstream_use: usize,
    pub actor_pressure: ActorPressureMetrics,
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
    TimestampBeforeSubmission,
    CorrectionWithoutReview,
    CompletionWithoutRequest,
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
    DuplicateReceiptId {
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
            Self::DuplicateReceiptId { index } => {
                write!(formatter, "review queue fact {index} repeats a receipt id")
            }
        }
    }
}

impl std::error::Error for ReviewBackpressureError {}

#[derive(Default)]
struct ClaimFacts<'a> {
    total: usize,
    independence_keys: BTreeSet<&'a str>,
    evidence: BTreeSet<EvidenceDirection>,
}

/// Aggregate a bounded queue without reading external state or wall clock.
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

    let mut receipt_ids = BTreeSet::new();
    let mut claims: BTreeMap<&str, ClaimFacts<'_>> = BTreeMap::new();
    let mut exact_work = BTreeSet::new();
    let mut low_value_groups: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut verifier_classes = BTreeSet::new();
    let mut actor_queue: BTreeMap<&str, usize> = BTreeMap::new();

    let mut queue_depth = 0usize;
    let mut critical_queue_depth = 0usize;
    let mut oldest_age_seconds = 0u64;
    let mut critical_latency_seconds = 0u64;
    let mut reviewer_minutes = 0u64;
    let mut correction_cases = 0usize;
    let mut open_corrections = 0usize;
    let mut correction_latency_seconds = 0u64;
    let mut correction_latency_sum = 0u128;
    let mut exact_retries = 0usize;
    let mut low_value_work = 0usize;
    let mut critical_exceptions = 0usize;
    let mut downstream_uses = 0u64;
    let mut receipts_with_downstream_use = 0usize;

    for (index, fact) in facts.iter().enumerate() {
        validate_fact(fact, index, as_of_seconds)?;
        if !receipt_ids.insert(fact.receipt_id.as_str()) {
            return Err(ReviewBackpressureError::DuplicateReceiptId { index });
        }

        let pending = fact.reviewed_at_seconds.is_none()
            || (fact.correction_requested_at_seconds.is_some()
                && fact.corrected_at_seconds.is_none());
        if pending {
            let age = as_of_seconds - fact.submitted_at_seconds;
            queue_depth += 1;
            oldest_age_seconds = oldest_age_seconds.max(age);
            *actor_queue.entry(fact.actor_id.as_str()).or_default() += 1;
            if fact.priority == ReviewPriority::CriticalException {
                critical_queue_depth += 1;
                critical_latency_seconds = critical_latency_seconds.max(age);
            }
        }

        reviewer_minutes += u64::from(fact.reviewer_minutes);
        downstream_uses += u64::from(fact.downstream_uses);
        receipts_with_downstream_use += usize::from(fact.downstream_uses > 0);
        verifier_classes.insert(fact.verifier_class);

        if fact.priority == ReviewPriority::CriticalException {
            critical_exceptions += 1;
        }
        if fact.value == WorkValue::LowValue {
            low_value_work += 1;
            *low_value_groups
                .entry((fact.claim_id.as_str(), fact.independence_key.as_str()))
                .or_default() += 1;
        }

        if !exact_work.insert((
            fact.claim_id.as_str(),
            fact.independence_key.as_str(),
            fact.verifier_class,
            fact.exact_work_root.as_str(),
        )) {
            exact_retries += 1;
        }

        let claim = claims.entry(fact.claim_id.as_str()).or_default();
        claim.total += 1;
        claim
            .independence_keys
            .insert(fact.independence_key.as_str());
        claim.evidence.insert(fact.evidence);

        if let Some(requested_at) = fact.correction_requested_at_seconds {
            correction_cases += 1;
            let completed_at = fact.corrected_at_seconds.unwrap_or(as_of_seconds);
            let latency = completed_at - requested_at;
            correction_latency_seconds = correction_latency_seconds.max(latency);
            correction_latency_sum += u128::from(latency);
            open_corrections += usize::from(fact.corrected_at_seconds.is_none());
        }
    }

    let mut independent_replications = 0usize;
    let mut duplications = 0usize;
    let mut conflicting_claims = 0usize;
    for claim in claims.values() {
        let independent = claim.independence_keys.len();
        independent_replications += independent.saturating_sub(1);
        duplications += claim.total.saturating_sub(independent);
        conflicting_claims += usize::from(
            claim.evidence.contains(&EvidenceDirection::SupportsClaim)
                && claim.evidence.contains(&EvidenceDirection::ChallengesClaim),
        );
    }

    let repeated_low_value_work = low_value_groups
        .values()
        .map(|count| count.saturating_sub(1))
        .sum();
    let replication_denominator = independent_replications + duplications;
    let replication_rate_bps = basis_points(independent_replications, replication_denominator);

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

    let mean_correction_latency_seconds = if correction_cases == 0 {
        0
    } else {
        u64::try_from(correction_latency_sum / correction_cases as u128).unwrap_or(u64::MAX)
    };
    let metrics = ReviewBackpressureMetrics {
        queue_depth,
        critical_queue_depth,
        oldest_age_seconds,
        critical_latency_seconds,
        reviewer_minutes,
        correction_cases,
        open_corrections,
        correction_latency_seconds,
        mean_correction_latency_seconds,
        verifier_class_diversity: verifier_classes.len(),
        verifier_classes: verifier_classes.into_iter().collect(),
        claims: claims.len(),
        exact_retries,
        independent_replications,
        duplications,
        replication_rate_bps,
        conflicting_claims,
        low_value_work,
        repeated_low_value_work,
        critical_exceptions,
        downstream_uses,
        receipts_with_downstream_use,
        actor_pressure,
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
        || metrics.critical_latency_seconds >= thresholds.critical_critical_latency_seconds
        || metrics.correction_latency_seconds >= thresholds.critical_correction_latency_seconds
        || metrics.actor_pressure.largest_actor_queue_depth >= thresholds.critical_actor_queue_depth
    {
        BackpressureLevel::Critical
    } else if metrics.queue_depth >= thresholds.elevated_queue_depth
        || metrics.oldest_age_seconds >= thresholds.elevated_oldest_age_seconds
        || metrics.critical_latency_seconds >= thresholds.elevated_critical_latency_seconds
        || metrics.correction_latency_seconds >= thresholds.elevated_correction_latency_seconds
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
        && thresholds.elevated_critical_latency_seconds > 0
        && thresholds.elevated_critical_latency_seconds
            < thresholds.critical_critical_latency_seconds
        && thresholds.elevated_correction_latency_seconds > 0
        && thresholds.elevated_correction_latency_seconds
            < thresholds.critical_correction_latency_seconds
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
        ("receipt_id", fact.receipt_id.as_str()),
        ("claim_id", fact.claim_id.as_str()),
        ("actor_id", fact.actor_id.as_str()),
        ("independence_key", fact.independence_key.as_str()),
        ("exact_work_root", fact.exact_work_root.as_str()),
    ] {
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
        return invalid_time(
            index,
            "submitted_at_seconds",
            FactViolation::TimestampAfterReference,
        );
    }
    if let Some(reviewed_at) = fact.reviewed_at_seconds {
        validate_ordered_time(
            index,
            "reviewed_at_seconds",
            reviewed_at,
            fact.submitted_at_seconds,
            as_of_seconds,
        )?;
    }
    if let Some(requested_at) = fact.correction_requested_at_seconds {
        let Some(reviewed_at) = fact.reviewed_at_seconds else {
            return invalid_time(
                index,
                "correction_requested_at_seconds",
                FactViolation::CorrectionWithoutReview,
            );
        };
        validate_ordered_time(
            index,
            "correction_requested_at_seconds",
            requested_at,
            reviewed_at,
            as_of_seconds,
        )?;
    }
    if let Some(corrected_at) = fact.corrected_at_seconds {
        let Some(requested_at) = fact.correction_requested_at_seconds else {
            return invalid_time(
                index,
                "corrected_at_seconds",
                FactViolation::CompletionWithoutRequest,
            );
        };
        validate_ordered_time(
            index,
            "corrected_at_seconds",
            corrected_at,
            requested_at,
            as_of_seconds,
        )?;
    }
    Ok(())
}

fn validate_ordered_time(
    index: usize,
    field: &'static str,
    value: u64,
    lower_bound: u64,
    as_of_seconds: u64,
) -> Result<(), ReviewBackpressureError> {
    if value < lower_bound {
        invalid_time(index, field, FactViolation::TimestampBeforeSubmission)
    } else if value > as_of_seconds {
        invalid_time(index, field, FactViolation::TimestampAfterReference)
    } else {
        Ok(())
    }
}

fn invalid_time<T>(
    index: usize,
    field: &'static str,
    violation: FactViolation,
) -> Result<T, ReviewBackpressureError> {
    Err(ReviewBackpressureError::InvalidFact {
        index,
        field,
        violation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AS_OF: u64 = 2_000_000_000;

    fn mixed_fact(index: usize) -> ReviewQueueFact {
        let group = index / 4;
        let slot = index % 4;
        let work_boundary = match slot {
            0 | 1 => 0,
            2 => 1,
            _ => 2,
        };
        let submitted = AS_OF - 100 - index as u64;
        let (reviewed, correction_requested, corrected) = if index.is_multiple_of(97) {
            let reviewed = submitted + 10;
            let requested = submitted + 20;
            (
                Some(reviewed),
                Some(requested),
                (!index.is_multiple_of(194)).then_some(submitted + 40),
            )
        } else if index.is_multiple_of(3) {
            (None, None, None)
        } else {
            (Some(submitted + 10), None, None)
        };
        let verifier_class = match group % 6 {
            0 => VerifierClass::LeanKernel,
            1 => VerifierClass::FrozenRust,
            2 => VerifierClass::ExactPython,
            3 => VerifierClass::ContainerReplay,
            4 => VerifierClass::InstrumentReplay,
            _ => VerifierClass::OtherFrozen,
        };
        ReviewQueueFact {
            receipt_id: if index == 0 {
                "r".repeat(MAX_FACT_KEY_BYTES)
            } else {
                format!("vr_{index:05}")
            },
            claim_id: format!("claim_{group:05}"),
            actor_id: format!("actor_{:03}", index % 128),
            independence_key: format!("producer_{work_boundary}_{}", group % 64),
            verifier_class,
            exact_work_root: format!("sha256:{group:064x}:{work_boundary}"),
            evidence: if slot == 3 && group.is_multiple_of(11) {
                EvidenceDirection::ChallengesClaim
            } else {
                EvidenceDirection::SupportsClaim
            },
            value: if slot == 1 || (slot == 0 && group.is_multiple_of(5)) {
                WorkValue::LowValue
            } else {
                WorkValue::Substantive
            },
            priority: if index.is_multiple_of(257) {
                ReviewPriority::CriticalException
            } else {
                ReviewPriority::Routine
            },
            submitted_at_seconds: submitted,
            reviewed_at_seconds: reviewed,
            correction_requested_at_seconds: correction_requested,
            corrected_at_seconds: corrected,
            reviewer_minutes: 5 + (index % 45) as u32,
            downstream_uses: if index.is_multiple_of(7) {
                1 + (index % 3) as u32
            } else {
                0
            },
        }
    }

    #[test]
    fn mixed_near_limit_queue_is_stable_bounded_and_compact() {
        let facts: Vec<_> = (0..MAX_REVIEW_QUEUE_FACTS - 1).map(mixed_fact).collect();
        let report = review_backpressure(&facts, AS_OF, ReviewBackpressureThresholds::default())
            .expect("near-limit mixed queue should aggregate");

        assert_eq!(report.input_count, 16_383);
        assert_eq!(report.schema, REVIEW_BACKPRESSURE_SCHEMA);
        assert_eq!(report.stability, "testing");
        assert_eq!(report.level, BackpressureLevel::Critical);
        assert_eq!(report.metrics.claims, 4_096);
        assert_eq!(report.metrics.exact_retries, 4_096);
        assert_eq!(report.metrics.independent_replications, 8_191);
        assert_eq!(report.metrics.duplications, 4_096);
        assert_eq!(report.metrics.replication_rate_bps, 6_666);
        assert_eq!(report.metrics.conflicting_claims, 373);
        assert_eq!(report.metrics.low_value_work, 4_916);
        assert_eq!(report.metrics.repeated_low_value_work, 820);
        assert_eq!(report.metrics.critical_exceptions, 64);
        assert_eq!(report.metrics.verifier_class_diversity, 6);
        assert!(report.metrics.queue_depth > 5_000);
        assert!(report.metrics.open_corrections > 80);
        assert!(report.metrics.downstream_uses > 4_000);

        let expected_critical_latency = facts
            .iter()
            .filter(|fact| {
                fact.priority == ReviewPriority::CriticalException
                    && (fact.reviewed_at_seconds.is_none()
                        || (fact.correction_requested_at_seconds.is_some()
                            && fact.corrected_at_seconds.is_none()))
            })
            .map(|fact| AS_OF - fact.submitted_at_seconds)
            .max()
            .unwrap();
        assert_eq!(
            report.metrics.critical_latency_seconds,
            expected_critical_latency
        );

        let mut reversed = facts.clone();
        reversed.reverse();
        let reordered =
            review_backpressure(&reversed, AS_OF, ReviewBackpressureThresholds::default())
                .expect("input order must not affect the projection");
        assert_eq!(report, reordered);

        let encoded = serde_json::to_vec(&report).expect("report should serialize");
        assert!(
            encoded.len() < 1_600,
            "aggregate output unexpectedly grew to {} bytes",
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
    fn pressure_level_transitions_are_typed() {
        let thresholds = ReviewBackpressureThresholds::default();
        let empty = review_backpressure(&[], AS_OF, thresholds).unwrap();
        assert_eq!(empty.level, BackpressureLevel::Normal);

        let elevated_facts: Vec<_> = (0..thresholds.elevated_queue_depth)
            .map(|index| {
                let mut fact = mixed_fact(index * 3 + 3);
                fact.receipt_id = format!("elevated_{index}");
                fact.claim_id = format!("elevated_claim_{index}");
                fact.actor_id = format!("elevated_actor_{index}");
                fact.independence_key = format!("elevated_producer_{index}");
                fact.reviewed_at_seconds = None;
                fact.correction_requested_at_seconds = None;
                fact.corrected_at_seconds = None;
                fact
            })
            .collect();
        let elevated = review_backpressure(&elevated_facts, AS_OF, thresholds).unwrap();
        assert_eq!(elevated.level, BackpressureLevel::Elevated);

        let critical_facts: Vec<_> = (0..thresholds.critical_queue_depth)
            .map(|index| {
                let mut fact = mixed_fact(index * 3 + 3);
                fact.receipt_id = format!("critical_{index}");
                fact.claim_id = format!("critical_claim_{index}");
                fact.actor_id = format!("critical_actor_{index}");
                fact.independence_key = format!("critical_producer_{index}");
                fact.reviewed_at_seconds = None;
                fact.correction_requested_at_seconds = None;
                fact.corrected_at_seconds = None;
                fact
            })
            .collect();
        let critical = review_backpressure(&critical_facts, AS_OF, thresholds).unwrap();
        assert_eq!(critical.level, BackpressureLevel::Critical);
    }

    #[test]
    fn every_fact_key_accepts_the_boundary_and_rejects_one_byte_more() {
        let boundary = "x".repeat(MAX_FACT_KEY_BYTES);
        let mut fact = mixed_fact(0);
        fact.receipt_id = boundary.clone();
        fact.claim_id = boundary.clone();
        fact.actor_id = boundary.clone();
        fact.independence_key = boundary.clone();
        fact.exact_work_root = boundary;
        review_backpressure(
            std::slice::from_ref(&fact),
            AS_OF,
            ReviewBackpressureThresholds::default(),
        )
        .expect("every key at the byte boundary should remain valid");

        for field in [
            "receipt_id",
            "claim_id",
            "actor_id",
            "independence_key",
            "exact_work_root",
        ] {
            let mut oversized = fact.clone();
            let value = "x".repeat(MAX_FACT_KEY_BYTES + 1);
            match field {
                "receipt_id" => oversized.receipt_id = value,
                "claim_id" => oversized.claim_id = value,
                "actor_id" => oversized.actor_id = value,
                "independence_key" => oversized.independence_key = value,
                "exact_work_root" => oversized.exact_work_root = value,
                _ => unreachable!(),
            }
            assert_eq!(
                review_backpressure(&[oversized], AS_OF, ReviewBackpressureThresholds::default(),),
                Err(ReviewBackpressureError::InvalidFact {
                    index: 0,
                    field,
                    violation: FactViolation::TooLong,
                })
            );
        }
    }
}
