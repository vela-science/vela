//! Frontier health projection.
//!
//! Health is an operational view over local frontier state. It reports
//! review debt, stale proof, source queue issues, and missing scoped
//! attestations. It does not decide whether a scientific claim is true.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::reviewer_identity;
use vela_protocol::evidence_ci::{self, EvidenceCiSeverity};
use vela_protocol::frontier_policy;
use vela_protocol::project::Project;
use vela_protocol::released_diff_pack::ReleasedVerdict;
use vela_protocol::repo::{self, VelaSource};
use vela_protocol::scientific_diff::ScientificDiffPack;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrontierHealthReport {
    pub ok: bool,
    pub command: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub checked_at: String,
    pub policy_class: String,
    pub metrics: FrontierHealthMetrics,
    #[serde(default)]
    pub issues: Vec<FrontierHealthIssue>,
    #[serde(default)]
    pub links: Vec<FrontierHealthLink>,
    #[serde(default)]
    pub threshold_classes: Vec<FrontierHealthThreshold>,
    #[serde(default)]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FrontierHealthMetrics {
    pub active_tasks: usize,
    pub blocked_tasks: usize,
    pub awaiting_review_tasks: usize,
    pub pending_diff_packs: usize,
    pub accepted_diff_packs: usize,
    pub rejected_diff_packs: usize,
    pub revision_requested_diff_packs: usize,
    pub proof_status: String,
    pub stale_proof: bool,
    pub source_inbox_issues: usize,
    pub evidence_ci_failures: usize,
    pub evidence_ci_warnings: usize,
    pub stale_claims: usize,
    pub contradiction_debt: usize,
    pub retraction_impacts: usize,
    pub max_review_latency_days: i64,
    pub missing_attestations: usize,
    pub missing_attestation_targets: usize,
    /// The compounding-loop block: is accepted state doing work, and is
    /// failure landing where the next producer can read it?
    #[serde(default)]
    pub compounding: CompoundingMetrics,
}

/// The compounding-loop metrics (v0.736): ratios over the event log and the
/// attempt ledger that say whether the frontier is accumulating leverage —
/// policy-mediated acceptance, channel-attributed failure, reused context —
/// or just accumulating events.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompoundingMetrics {
    /// Accepted-kind events (kinds ending in `.accepted`, plus
    /// `policy.auto_admitted`) whose payload names a `policy_id`, over all
    /// accepted-kind events. Trailing all-time for now; a policy-named
    /// acceptance is one a machine could re-derive, so this is the fraction
    /// of acceptance running on rails rather than ad-hoc judgment.
    pub autonomy_ratio: f64,
    /// Failed attempts naming a `channel:` obstruction, over all failed
    /// attempts. Failure that names its channel is failure the channel map
    /// can compound; anonymous failure is heat loss.
    pub dead_channel_coverage: f64,
    /// `one_premise_away(promoted) − one_premise_away(actual)` for the most
    /// recent acceptance. Left 0.0 until the `Boundary::derive_with_promoted`
    /// wiring lands in the accept path.
    pub unlock_yield_last: f64,
    /// Attempts carrying a `base_frontier_root`, over all attempts — a proxy
    /// for context reuse (the producer pinned what state it searched against).
    /// Refinement: check the pinned root against actually-materialized
    /// frontier roots so a fabricated or stale pin does not count as reuse.
    pub context_reuse_ratio: f64,
    /// Deposits avoided by the fold-at-deposit path. 0 for now: folds are
    /// absence-of-events (nothing lands in the log), so this is counted at
    /// deposit time by the MCP layer later, not derivable from replay.
    pub attempts_avoided: usize,
}

/// Compute the compounding block from a project. Pure and deterministic —
/// reads only the event log and the attempt ledger, never wall clock.
#[must_use]
pub fn compounding_metrics(project: &Project) -> CompoundingMetrics {
    let mut accepted_events = 0usize;
    let mut policy_backed = 0usize;
    for event in &project.events {
        let kind = event.kind.as_str();
        if !(kind.ends_with(".accepted") || kind == "policy.auto_admitted") {
            continue;
        }
        accepted_events += 1;
        if event.payload.get("policy_id").is_some() {
            policy_backed += 1;
        }
    }

    let mut with_root = 0usize;
    let mut failed = 0usize;
    let mut failed_with_channel = 0usize;
    for attempt in &project.attempts {
        if !attempt.base_frontier_root.is_empty() {
            with_root += 1;
        }
        if crate::channel_map::attempt_is_failed(project, attempt) {
            failed += 1;
            if crate::channel_map::attempt_channel(attempt).is_some() {
                failed_with_channel += 1;
            }
        }
    }

    CompoundingMetrics {
        autonomy_ratio: ratio(policy_backed, accepted_events),
        dead_channel_coverage: ratio(failed_with_channel, failed),
        unlock_yield_last: 0.0,
        context_reuse_ratio: ratio(with_root, project.attempts.len()),
        attempts_avoided: 0,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierHealthIssue {
    pub id: String,
    pub severity: String,
    pub count: usize,
    pub label: String,
    pub message: String,
    pub href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierHealthLink {
    pub label: String,
    pub href: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontierHealthThreshold {
    pub review_class: String,
    pub required_reviewer_count: usize,
    #[serde(default)]
    pub reviewer_roles: Vec<String>,
}

pub fn analyze(frontier_path: &Path) -> Result<FrontierHealthReport, String> {
    let project = repo::load_from_path(frontier_path)?;
    let repo_root = local_repo_root(frontier_path);
    let local_path = repo_root.as_deref().unwrap_or(frontier_path);
    let evidence = evidence_ci::run_frontier(frontier_path)?;
    let policy = frontier_policy::load_policy_summary(frontier_path).ok();

    let (
        pending_diff_packs,
        accepted_diff_packs,
        rejected_diff_packs,
        revision_requested_diff_packs,
    ) = diff_pack_counts(&project);
    let (missing_attestations, missing_attestation_targets) =
        missing_attestations(&project, local_path, repo_root.as_ref().is_some());

    let evidence_ci_failures = evidence.summary.release_blocking_failed;
    let evidence_ci_warnings = evidence.summary.warnings;
    let stale_claims = stale_claim_count(&evidence);
    let contradiction_debt = contradiction_debt(&project);
    let proof_status = project.proof_state.latest_packet.status.clone();
    let stale_proof = !matches!(proof_status.as_str(), "fresh" | "current" | "ready");
    let max_review_latency_days = max_review_latency_days(&project);
    let retraction_impacts = project
        .events
        .iter()
        .filter(|event| event.kind.as_str().contains("retract"))
        .count();

    let metrics = FrontierHealthMetrics {
        active_tasks: 0,
        blocked_tasks: 0,
        awaiting_review_tasks: 0,
        pending_diff_packs,
        accepted_diff_packs,
        rejected_diff_packs,
        revision_requested_diff_packs,
        proof_status,
        stale_proof,
        source_inbox_issues: 0,
        evidence_ci_failures,
        evidence_ci_warnings,
        stale_claims,
        contradiction_debt,
        retraction_impacts,
        max_review_latency_days,
        missing_attestations,
        missing_attestation_targets,
        compounding: compounding_metrics(&project),
    };

    let mut report = FrontierHealthReport {
        ok: false,
        command: "frontier.health".to_string(),
        frontier_id: project.frontier_id(),
        frontier_path: frontier_path.display().to_string(),
        checked_at: Utc::now().to_rfc3339(),
        policy_class: if policy.as_ref().is_some_and(|p| p.ok) {
            "frontier_policy".to_string()
        } else {
            "built_in_defaults".to_string()
        },
        metrics,
        issues: Vec::new(),
        links: health_links(),
        threshold_classes: threshold_classes(policy.as_ref()),
        caveats: vec![
            "Health is an operating projection for local review. It is not a truth verdict."
                .to_string(),
            "Hosted surfaces must remain read-only; the local review server (`vela serve`) and CLI own review actions."
                .to_string(),
        ],
    };
    report.issues = build_issues(&report.metrics);
    report.ok = !report.issues.iter().any(|issue| issue.severity == "error");
    Ok(report)
}

fn diff_pack_counts(project: &Project) -> (usize, usize, usize, usize) {
    let mut pending = 0;
    let mut accepted = 0;
    let mut rejected = 0;
    let mut revise = 0;
    for record in &project.released_diff_packs {
        match record.verdict {
            Some(ReleasedVerdict::Accept) => accepted += 1,
            Some(ReleasedVerdict::Reject) => rejected += 1,
            Some(ReleasedVerdict::Revise) => revise += 1,
            None => pending += 1,
        }
    }
    (pending, accepted, rejected, revise)
}

fn missing_attestations(
    project: &Project,
    repo_path: &Path,
    is_local_repo: bool,
) -> (usize, usize) {
    if !is_local_repo {
        return (0, 0);
    }
    let mut missing = 0usize;
    let mut targets = 0usize;
    for record in &project.released_diff_packs {
        if record.verdict.is_some() {
            continue;
        }
        let Some(pack) = load_diff_pack(repo_path, &record.pack_id) else {
            continue;
        };
        let summary = pack.review_summary(repo_path);
        let required = summary.required_reviewers;
        if required.is_empty() {
            continue;
        }
        let missing_roles =
            reviewer_identity::missing_roles_for_target(repo_path, &pack.pack_id, &required)
                .unwrap_or(required);
        if !missing_roles.is_empty() {
            missing += missing_roles.len();
            targets += 1;
        }
    }
    (missing, targets)
}

fn load_diff_pack(repo_path: &Path, pack_id: &str) -> Option<ScientificDiffPack> {
    let path = repo_path
        .join(".vela")
        .join("diff_packs")
        .join(format!("{pack_id}.json"));
    let body = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&body).ok()
}

fn stale_claim_count(evidence: &evidence_ci::EvidenceCiReport) -> usize {
    evidence
        .checks
        .iter()
        .filter(|check| {
            check.target_type == "finding"
                && matches!(
                    check.severity,
                    EvidenceCiSeverity::Warn | EvidenceCiSeverity::Error
                )
                && matches!(
                    check.id.as_str(),
                    "source.id_presence"
                        | "source.canonical_locator"
                        | "evidence.span_presence"
                        | "trial.registry_reference"
                        | "condition.population"
                        | "condition.comparator_or_baseline"
                        | "condition.endpoint"
                )
        })
        .map(|check| check.target_id.clone())
        .collect::<BTreeSet<_>>()
        .len()
}

fn contradiction_debt(project: &Project) -> usize {
    project
        .findings
        .iter()
        .flat_map(|finding| finding.links.iter())
        .filter(|link| link.link_type == "contradicts")
        .count()
}

fn max_review_latency_days(project: &Project) -> i64 {
    let mut max_days = 0i64;
    for proposal in &project.proposals {
        if proposal.status == "pending_review" {
            max_days = max_days.max(age_days(
                proposal
                    .drafted_at
                    .as_deref()
                    .unwrap_or(proposal.created_at.as_str()),
            ));
        }
    }
    for pack in &project.released_diff_packs {
        if pack.verdict.is_none() {
            max_days = max_days.max(age_days(&pack.released_at));
        }
    }
    max_days
}

fn age_days(timestamp: &str) -> i64 {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|dt| {
            Utc::now()
                .signed_duration_since(dt.with_timezone(&Utc))
                .num_days()
                .max(0)
        })
        .unwrap_or(0)
}

fn threshold_classes(
    policy: Option<&frontier_policy::FrontierPolicySummary>,
) -> Vec<FrontierHealthThreshold> {
    [
        ("low_risk", "add_evidence_atom", false),
        ("source_repair", "repair_locator", false),
        ("entity_issue", "resolve_entity", false),
        ("confidence_change", "revise_confidence", false),
        ("contradiction_change", "mark_contradiction", false),
        ("decision_impact", "request_downstream_review", true),
        ("retraction_impact", "retraction_impact", true),
    ]
    .into_iter()
    .map(|(review_class, operation, downstream)| {
        let requirement = frontier_policy::review_requirement_for_operation(
            policy, operation, "health", downstream,
        );
        FrontierHealthThreshold {
            review_class: review_class.to_string(),
            required_reviewer_count: requirement.required_reviewer_count,
            reviewer_roles: requirement.reviewer_roles,
        }
    })
    .collect()
}

fn health_links() -> Vec<FrontierHealthLink> {
    vec![
        FrontierHealthLink {
            label: "tasks".to_string(),
            href: "/tasks".to_string(),
            count: 0,
        },
        FrontierHealthLink {
            label: "source inbox".to_string(),
            href: "/source-inbox".to_string(),
            count: 0,
        },
        FrontierHealthLink {
            label: "Diff Packs".to_string(),
            href: "/diff-packs".to_string(),
            count: 0,
        },
        FrontierHealthLink {
            label: "Evidence CI".to_string(),
            href: "/review/session".to_string(),
            count: 0,
        },
        FrontierHealthLink {
            label: "proof".to_string(),
            href: "/proof".to_string(),
            count: 0,
        },
    ]
}

fn build_issues(metrics: &FrontierHealthMetrics) -> Vec<FrontierHealthIssue> {
    let mut issues = Vec::new();
    push_issue(
        &mut issues,
        metrics.stale_proof.then_some(1),
        "proof_freshness",
        "error",
        "Stale proof",
        "Recorded proof is not fresh against current frontier state.",
        "/proof",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.evidence_ci_failures),
        "evidence_ci_failures",
        "error",
        "Evidence CI failures",
        "Release-blocking Evidence CI checks need review.",
        "/review/session",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.missing_attestations),
        "missing_attestations",
        "warn",
        "Missing attestations",
        "One or more pending Diff Packs are missing required scoped reviewer roles.",
        "/diff-packs",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.blocked_tasks),
        "blocked_tasks",
        "warn",
        "Blocked tasks",
        "Local frontier tasks have unresolved blockers.",
        "/tasks",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.source_inbox_issues),
        "source_inbox_issues",
        "warn",
        "Source inbox issues",
        "Source records are quarantined, retracted, or stale.",
        "/source-inbox",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.stale_claims),
        "stale_claims",
        "warn",
        "Claims needing source review",
        "Evidence CI found source, condition, trial, or locator debt on findings.",
        "/review/inbox",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.contradiction_debt),
        "contradiction_debt",
        "warn",
        "Contradiction debt",
        "Contradictory links are visible and should stay in the review loop.",
        "/conflicts",
        None,
    );
    push_issue(
        &mut issues,
        nonzero(metrics.retraction_impacts),
        "retraction_impacts",
        "warn",
        "Retraction impacts",
        "Retraction-linked source or event state needs downstream review.",
        "/source-inbox?state=retracted",
        None,
    );
    push_issue(
        &mut issues,
        (metrics.max_review_latency_days > 7).then_some(metrics.max_review_latency_days as usize),
        "review_latency",
        "warn",
        "Review latency",
        "At least one pending proposal or Diff Pack has waited more than seven days.",
        "/review/inbox",
        None,
    );
    issues
}

fn push_issue(
    issues: &mut Vec<FrontierHealthIssue>,
    count: Option<usize>,
    id: &str,
    severity: &str,
    label: &str,
    message: &str,
    href: &str,
    target_id: Option<String>,
) {
    if let Some(count) = count.filter(|count| *count > 0) {
        issues.push(FrontierHealthIssue {
            id: id.to_string(),
            severity: severity.to_string(),
            count,
            label: label.to_string(),
            message: message.to_string(),
            href: href.to_string(),
            target_id,
        });
    }
}

fn nonzero(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn local_repo_root(path: &Path) -> Option<PathBuf> {
    match repo::detect(path).ok()? {
        VelaSource::VelaRepo(root) => Some(root),
        VelaSource::ProjectFile(_) | VelaSource::PacketDir(_) => None,
    }
}

#[cfg(test)]
mod compounding_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::attempt::{Attempt, AttemptDraft};
    use vela_protocol::test_support::make_project;

    fn attempt(claim: &str, status: &str, obstructions: Vec<&str>, root: &str) -> Attempt {
        let draft = AttemptDraft {
            problem: 647,
            frontier: "t".into(),
            kind: "route".into(),
            claim: claim.into(),
            claimed_status: status.into(),
            named_obstructions: obstructions.into_iter().map(String::from).collect(),
            base_frontier_root: root.into(),
            ..Default::default()
        };
        Attempt::build(draft, &SigningKey::from_bytes(&[3u8; 32])).unwrap()
    }

    #[test]
    fn empty_project_yields_all_zero_ratios() {
        let project = make_project("empty", vec![]);
        let c = compounding_metrics(&project);
        assert_eq!(c, CompoundingMetrics::default());
    }

    #[test]
    fn ratios_read_the_log_and_the_ledger() {
        let mut project = make_project("comp", vec![]);
        // Two accepted-kind events, one policy-backed.
        let mut with_policy = project.events[0].clone();
        with_policy.kind = "review.accepted".into();
        with_policy.payload = serde_json::json!({"policy_id": "vap_d03dc"});
        let mut without_policy = project.events[0].clone();
        without_policy.kind = "review.accepted".into();
        without_policy.payload = serde_json::json!({});
        project.events.push(with_policy);
        project.events.push(without_policy);
        // Three attempts: two failed (one channel-named), one banked with a
        // pinned base root.
        project.attempts.push(attempt(
            "route a",
            "failed",
            vec!["channel:erdos647:prime"],
            "",
        ));
        project
            .attempts
            .push(attempt("route b", "failed", vec![], ""));
        project
            .attempts
            .push(attempt("route c", "banked", vec![], "sha256:deadbeef"));

        let c = compounding_metrics(&project);
        assert!((c.autonomy_ratio - 0.5).abs() < f64::EPSILON);
        assert!((c.dead_channel_coverage - 0.5).abs() < f64::EPSILON);
        assert!((c.context_reuse_ratio - 1.0 / 3.0).abs() < f64::EPSILON);
        assert_eq!(c.unlock_yield_last, 0.0);
        assert_eq!(c.attempts_avoided, 0);
    }
}
