//! Evidence CI.
//!
//! Evidence CI is a review-readiness projection. It checks grounding,
//! locator, policy, and confidence-update inputs before review. It does
//! not decide whether a scientific claim is true.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::bundle::FindingBundle;
use crate::frontier_policy;
use crate::project::Project;
use crate::repo;
use crate::sources::{self, ConditionRecord, EvidenceAtom, SourceRecord};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCiStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCiSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCiClassification {
    ReleaseBlocking,
    ReviewWarning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceCiCheck {
    pub id: String,
    pub group: String,
    pub classification: EvidenceCiClassification,
    pub status: EvidenceCiStatus,
    pub severity: EvidenceCiSeverity,
    pub target_type: String,
    pub target_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub release_blocking: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceCiSummary {
    pub total: usize,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub release_blocking: usize,
    pub review_warning: usize,
    pub info: usize,
    pub release_blocking_failed: usize,
    #[serde(default)]
    pub groups: Vec<EvidenceCiGroupSummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceCiGroupSummary {
    pub group: String,
    pub total: usize,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub release_blocking: usize,
    pub review_warning: usize,
    pub info: usize,
    pub release_blocking_failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceCiReport {
    pub ok: bool,
    pub command: String,
    pub frontier_id: String,
    pub frontier_path: String,
    pub checked_at: String,
    pub scope: String,
    pub summary: EvidenceCiSummary,
    #[serde(default)]
    pub checks: Vec<EvidenceCiCheck>,
    #[serde(default)]
    pub caveats: Vec<String>,
}

impl EvidenceCiReport {
    fn finish(mut self) -> Self {
        self.summary = summarize(&self.checks);
        self.ok = self.summary.release_blocking_failed == 0;
        self
    }
}

pub fn run_frontier(frontier_path: &Path) -> Result<EvidenceCiReport, String> {
    let project = repo::load_from_path(frontier_path)?;
    Ok(run_project(&project, frontier_path))
}

/// In-memory Evidence CI over an already-loaded project. `frontier_path`
/// is read only for static policy documents and to label the report; all
/// frontier state (findings, proof) comes from the in-memory `project`.
///
/// This lets a caller run a *prospective* check on an unsaved, mutated
/// project — the basis of the accept-time Engine gate, which runs CI on
/// the post-accept state before deciding whether to persist it.
pub fn run_project(project: &Project, frontier_path: &Path) -> EvidenceCiReport {
    let frontier_id = project.frontier_id();
    let projection = sources::derive_projection(project);
    let source_by_finding = source_records_by_finding(&projection.sources);
    let atom_by_finding = evidence_atoms_by_finding(&projection.evidence_atoms);
    let condition_by_finding = condition_records_by_finding(&projection.condition_records);
    let mut checks = Vec::new();

    match frontier_policy::load_policy_summary(frontier_path) {
        Ok(summary) if summary.ok => checks.push(pass(
            "policy.review_requirement",
            "frontier",
            &frontier_id,
            "Frontier policy is available for review requirements.",
            None,
            true,
        )),
        Ok(summary) => checks.push(fail(
            "policy.review_requirement",
            "frontier",
            &frontier_id,
            "Frontier policy is missing required policy documents.",
            Some(format!("missing: {}", summary.missing_required.join(", "))),
            true,
        )),
        Err(e) => checks.push(fail(
            "policy.review_requirement",
            "frontier",
            &frontier_id,
            "Frontier policy could not be loaded.",
            Some(e),
            true,
        )),
    }

    checks.push(pass(
        "contradiction.scan_status",
        "frontier",
        &frontier_id,
        "Contradiction scan is available through local tension queries.",
        Some(format!(
            "{} finding bundle(s) are in scope.",
            project.findings.len()
        )),
        false,
    ));

    let proof_status = project.proof_state.latest_packet.status.as_str();
    if matches!(proof_status, "fresh" | "current" | "ready") {
        checks.push(pass(
            "proof.freshness",
            "frontier",
            &frontier_id,
            "Proof state is current for this frontier.",
            Some(proof_status.to_string()),
            false,
        ));
    } else {
        checks.push(warn(
            "proof.freshness",
            "frontier",
            &frontier_id,
            "Proof state is stale, missing, or needs regeneration before release.",
            Some(proof_status.to_string()),
        ));
    }

    for finding in &project.findings {
        let sources = source_by_finding
            .get(finding.id.as_str())
            .cloned()
            .unwrap_or_default();
        let atoms = atom_by_finding
            .get(finding.id.as_str())
            .cloned()
            .unwrap_or_default();
        let conditions = condition_by_finding
            .get(finding.id.as_str())
            .cloned()
            .unwrap_or_default();
        add_finding_checks(&mut checks, finding, &sources, &atoms, &conditions);
    }

    EvidenceCiReport {
        ok: false,
        command: "evidence-ci".to_string(),
        frontier_id,
        frontier_path: frontier_path.display().to_string(),
        checked_at: Utc::now().to_rfc3339(),
        scope: "frontier".to_string(),
        summary: EvidenceCiSummary::default(),
        checks,
        caveats: vec![
            "Evidence CI checks review readiness. It does not establish final truth.".to_string(),
            "Draft debt is reported as warning unless a release-critical policy or diff-pack check fails.".to_string(),
        ],
    }
    .finish()
}
fn add_finding_checks(
    checks: &mut Vec<EvidenceCiCheck>,
    finding: &FindingBundle,
    sources: &[&SourceRecord],
    atoms: &[&EvidenceAtom],
    conditions: &[&ConditionRecord],
) {
    if sources.is_empty() {
        checks.push(warn(
            "source.id_presence",
            "finding",
            &finding.id,
            "Finding has no source record linked to it.",
            None,
        ));
    } else {
        checks.push(pass(
            "source.id_presence",
            "finding",
            &finding.id,
            "Finding has a source record.",
            Some(
                sources
                    .iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            false,
        ));
    }

    if sources
        .iter()
        .any(|source| sources::source_has_canonical_locator(source))
    {
        checks.push(pass(
            "source.canonical_locator",
            "finding",
            &finding.id,
            "Finding has a canonical source locator.",
            Some(
                sources
                    .iter()
                    .map(|s| s.locator.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            false,
        ));
    } else {
        checks.push(warn(
            "source.canonical_locator",
            "finding",
            &finding.id,
            "Finding source locator needs canonical repair.",
            None,
        ));
    }

    if atoms.iter().any(|atom| atom.locator.is_some()) {
        checks.push(pass(
            "evidence.span_presence",
            "finding",
            &finding.id,
            "Finding has an evidence atom locator.",
            None,
            false,
        ));
    } else {
        checks.push(warn(
            "evidence.span_presence",
            "finding",
            &finding.id,
            "Finding is missing a source evidence span or locator.",
            None,
        ));
    }

    if finding.evidence.evidence_type.trim().is_empty() {
        checks.push(warn(
            "evidence.type",
            "finding",
            &finding.id,
            "Finding evidence type is missing.",
            None,
        ));
    } else {
        checks.push(pass(
            "evidence.type",
            "finding",
            &finding.id,
            "Finding declares an evidence type.",
            Some(finding.evidence.evidence_type.clone()),
            false,
        ));
    }

    let combined = finding_text(finding);

    // Math-domain profile. The clinical / experimental study-design checks
    // below (trial registry, population, comparator/baseline, endpoint) only
    // carry meaning for an EMPIRICAL claim. A theoretical or formal claim —
    // an Erdős conjecture, a proved theorem — has no comparator arm, primary
    // endpoint, or study population, so firing those warnings on it is a
    // category error, not a real review gap (the original biomedical profile
    // raised ~2 such warnings per math finding). For a non-empirical claim
    // the four checks are recorded as not-applicable passes, keeping the
    // warning meaningful only where a study-design dimension actually exists.
    if !is_study_design_applicable(finding) {
        for (id, dimension) in [
            ("trial.registry_reference", "trial registry reference"),
            ("condition.population", "population or model context"),
            ("condition.comparator_or_baseline", "comparator or baseline"),
            ("condition.endpoint", "endpoint or measured outcome"),
        ] {
            checks.push(pass(
                id,
                "finding",
                &finding.id,
                "Study-design dimension is not applicable to a theoretical, formal, or benchmark-result claim.",
                Some(format!(
                    "{dimension} is not a dimension of a theoretical/formal/benchmark claim"
                )),
                false,
            ));
        }
        return;
    }

    if mentions_trial(&combined) && !has_trial_registry_ref(&combined, sources) {
        checks.push(warn(
            "trial.registry_reference",
            "finding",
            &finding.id,
            "Finding appears trial-related but lacks an NCT registry reference.",
            None,
        ));
    } else {
        checks.push(pass(
            "trial.registry_reference",
            "finding",
            &finding.id,
            "Trial registry reference check completed.",
            None,
            false,
        ));
    }

    if has_population(finding, &combined, conditions) {
        checks.push(pass(
            "condition.population",
            "finding",
            &finding.id,
            "Population or model context is declared.",
            None,
            false,
        ));
    } else {
        checks.push(warn(
            "condition.population",
            "finding",
            &finding.id,
            "Population or model context is missing or unclear.",
            None,
        ));
    }

    if conditions
        .iter()
        .any(|record| record.comparator_status == "declared")
    {
        checks.push(pass(
            "condition.comparator_or_baseline",
            "finding",
            &finding.id,
            "Comparator or baseline is declared.",
            None,
            false,
        ));
    } else {
        checks.push(warn(
            "condition.comparator_or_baseline",
            "finding",
            &finding.id,
            "Comparator or baseline is missing or unclear.",
            None,
        ));
    }

    if has_endpoint(&combined) {
        checks.push(pass(
            "condition.endpoint",
            "finding",
            &finding.id,
            "Endpoint or measured outcome is declared.",
            None,
            false,
        ));
    } else {
        checks.push(warn(
            "condition.endpoint",
            "finding",
            &finding.id,
            "Endpoint or measured outcome is missing or unclear.",
            None,
        ));
    }
}

fn source_records_by_finding(sources: &[SourceRecord]) -> BTreeMap<&str, Vec<&SourceRecord>> {
    let mut map = BTreeMap::<&str, Vec<&SourceRecord>>::new();
    for source in sources {
        for finding_id in &source.finding_ids {
            map.entry(finding_id.as_str()).or_default().push(source);
        }
    }
    map
}

fn evidence_atoms_by_finding(atoms: &[EvidenceAtom]) -> BTreeMap<&str, Vec<&EvidenceAtom>> {
    let mut map = BTreeMap::<&str, Vec<&EvidenceAtom>>::new();
    for atom in atoms {
        map.entry(atom.finding_id.as_str()).or_default().push(atom);
    }
    map
}

fn condition_records_by_finding(
    records: &[ConditionRecord],
) -> BTreeMap<&str, Vec<&ConditionRecord>> {
    let mut map = BTreeMap::<&str, Vec<&ConditionRecord>>::new();
    for record in records {
        map.entry(record.finding_id.as_str())
            .or_default()
            .push(record);
    }
    map
}

fn summarize(checks: &[EvidenceCiCheck]) -> EvidenceCiSummary {
    let mut groups = standard_group_map();
    let mut summary = EvidenceCiSummary {
        total: checks.len(),
        ..EvidenceCiSummary::default()
    };
    for check in checks {
        match check.status {
            EvidenceCiStatus::Passed => summary.passed += 1,
            EvidenceCiStatus::Warning => summary.warnings += 1,
            EvidenceCiStatus::Failed => summary.failed += 1,
        }
        match check.classification {
            EvidenceCiClassification::ReleaseBlocking => summary.release_blocking += 1,
            EvidenceCiClassification::ReviewWarning => summary.review_warning += 1,
            EvidenceCiClassification::Info => summary.info += 1,
        }
        if check.release_blocking && check.status == EvidenceCiStatus::Failed {
            summary.release_blocking_failed += 1;
        }
        let group = groups
            .entry(check.group.clone())
            .or_insert_with(|| EvidenceCiGroupSummary {
                group: check.group.clone(),
                ..EvidenceCiGroupSummary::default()
            });
        group.total += 1;
        match check.status {
            EvidenceCiStatus::Passed => group.passed += 1,
            EvidenceCiStatus::Warning => group.warnings += 1,
            EvidenceCiStatus::Failed => group.failed += 1,
        }
        match check.classification {
            EvidenceCiClassification::ReleaseBlocking => group.release_blocking += 1,
            EvidenceCiClassification::ReviewWarning => group.review_warning += 1,
            EvidenceCiClassification::Info => group.info += 1,
        }
        if check.release_blocking && check.status == EvidenceCiStatus::Failed {
            group.release_blocking_failed += 1;
        }
    }
    summary.groups = groups.into_values().collect();
    summary
}

fn standard_group_map() -> BTreeMap<String, EvidenceCiGroupSummary> {
    [
        "source_locator_coverage",
        "evidence_atom_quality",
        "confidence_change_support",
        "policy_requirements",
        "unresolved_warnings",
        "stale_proof",
    ]
    .into_iter()
    .map(|group| {
        (
            group.to_string(),
            EvidenceCiGroupSummary {
                group: group.to_string(),
                ..EvidenceCiGroupSummary::default()
            },
        )
    })
    .collect()
}

fn group_for_check(id: &str, status: &EvidenceCiStatus) -> String {
    if id.starts_with("source.") || id == "trial.registry_reference" {
        "source_locator_coverage"
    } else if id.starts_with("evidence.") || id.starts_with("condition.") {
        "evidence_atom_quality"
    } else if id.starts_with("confidence_delta.") {
        "confidence_change_support"
    } else if id.starts_with("policy.") {
        "policy_requirements"
    } else if id.starts_with("proof.") {
        "stale_proof"
    } else if *status == EvidenceCiStatus::Warning || *status == EvidenceCiStatus::Failed {
        "unresolved_warnings"
    } else {
        "info"
    }
    .to_string()
}

fn classification_for_check(
    status: &EvidenceCiStatus,
    release_blocking: bool,
) -> EvidenceCiClassification {
    if release_blocking {
        EvidenceCiClassification::ReleaseBlocking
    } else if *status == EvidenceCiStatus::Warning || *status == EvidenceCiStatus::Failed {
        EvidenceCiClassification::ReviewWarning
    } else {
        EvidenceCiClassification::Info
    }
}

fn pass(
    id: &str,
    target_type: &str,
    target_id: &str,
    message: &str,
    detail: Option<String>,
    release_blocking: bool,
) -> EvidenceCiCheck {
    let status = EvidenceCiStatus::Passed;
    let group = group_for_check(id, &status);
    let classification = classification_for_check(&status, release_blocking);
    EvidenceCiCheck {
        id: id.to_string(),
        group,
        classification,
        status,
        severity: EvidenceCiSeverity::Info,
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        message: message.to_string(),
        detail,
        release_blocking,
    }
}

fn warn(
    id: &str,
    target_type: &str,
    target_id: &str,
    message: &str,
    detail: Option<String>,
) -> EvidenceCiCheck {
    let status = EvidenceCiStatus::Warning;
    let release_blocking = false;
    let group = group_for_check(id, &status);
    let classification = classification_for_check(&status, release_blocking);
    EvidenceCiCheck {
        id: id.to_string(),
        group,
        classification,
        status,
        severity: EvidenceCiSeverity::Warn,
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        message: message.to_string(),
        detail,
        release_blocking,
    }
}

fn fail(
    id: &str,
    target_type: &str,
    target_id: &str,
    message: &str,
    detail: Option<String>,
    release_blocking: bool,
) -> EvidenceCiCheck {
    let status = EvidenceCiStatus::Failed;
    let group = group_for_check(id, &status);
    let classification = classification_for_check(&status, release_blocking);
    EvidenceCiCheck {
        id: id.to_string(),
        group,
        classification,
        status,
        severity: EvidenceCiSeverity::Error,
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        message: message.to_string(),
        detail,
        release_blocking,
    }
}

fn finding_text(finding: &FindingBundle) -> String {
    let parts = [
        finding.assertion.text.as_str(),
        finding.conditions.text.as_str(),
        finding.evidence.model_system.as_str(),
        finding.evidence.method.as_str(),
        finding.evidence.evidence_type.as_str(),
    ];
    parts.join(" ").to_ascii_lowercase()
}

fn mentions_trial(text: &str) -> bool {
    text.contains("trial") || text.contains("phase ") || text.contains("randomized")
}

/// Whether the clinical / experimental study-design checks (trial registry,
/// population, comparator/baseline, endpoint) apply to this finding.
///
/// They apply to an EMPIRICAL claim and are a category error on a theoretical
/// or formal one (an Erdős conjecture has no comparator arm). A finding is
/// treated as empirical — checks apply, the original biomedical behaviour —
/// unless its assertion or evidence type marks it clearly theoretical/formal
/// AND it carries no empirical signal. The guard means a theoretical-typed
/// finding that still describes lab conditions or a trial keeps the checks,
/// so a *computational study of a clinical trial* is not misclassified.
fn is_study_design_applicable(finding: &FindingBundle) -> bool {
    const THEORETICAL_ASSERTION: &[&str] = &[
        "open_question",
        "theoretical",
        "conjecture",
        "theorem",
        "lemma",
        "proposition",
        "definition",
        "formal",
        // A benchmark result (model X scores Y on dataset Z) is empirical but
        // not a clinical study: its "comparator" is other models on the same
        // leaderboard, not a control arm, and it has no trial registry, primary
        // endpoint, or study population. The clinical study-design checks are a
        // category error on it, the same as on a theorem.
        "benchmark_result",
    ];
    const THEORETICAL_EVIDENCE: &[&str] = &["theoretical", "mathematical", "formal", "proof"];

    let assertion_type = finding.assertion.assertion_type.trim().to_ascii_lowercase();
    let evidence_type = finding.evidence.evidence_type.trim().to_ascii_lowercase();
    let theoretical = THEORETICAL_ASSERTION.contains(&assertion_type.as_str())
        || THEORETICAL_EVIDENCE.contains(&evidence_type.as_str());
    if !theoretical {
        return true;
    }

    // A theoretical-typed finding that still carries empirical signal (a trial
    // mention in its text) keeps the study-design checks.
    mentions_trial(&finding_text(finding))
}

fn has_trial_registry_ref(text: &str, sources: &[&SourceRecord]) -> bool {
    text.contains("nct")
        || sources.iter().any(|source| {
            let joined = format!(
                "{} {} {}",
                source.locator,
                source.title,
                source.content_hash.as_deref().unwrap_or("")
            )
            .to_ascii_lowercase();
            joined.contains("nct")
        })
}

fn has_population(_finding: &FindingBundle, text: &str, conditions: &[&ConditionRecord]) -> bool {
    conditions.iter().any(|record| {
        record.species.is_some()
            || matches!(
                record.translation_scope.as_str(),
                "human" | "animal_model" | "in_vitro" | "computational"
            )
    }) || [
        "patient",
        "patients",
        "human",
        "mouse",
        "mice",
        "rat",
        "cell",
        "cohort",
        "adult",
        "pediatric",
        "in vitro",
        "in vivo",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn has_endpoint(text: &str) -> bool {
    [
        "endpoint",
        "outcome",
        "survival",
        "cognition",
        "clearance",
        "uptake",
        "transport",
        "expression",
        "effect size",
        "p=",
        "p value",
        "hazard ratio",
        "odds ratio",
        "risk ratio",
        "auc",
        "measurement",
        "assay",
        "level",
        "concentration",
        "response",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

pub fn required_check_ids(report: &EvidenceCiReport) -> BTreeSet<String> {
    report.checks.iter().map(|check| check.id.clone()).collect()
}

/// Stable key for one check instance: `id@target_id`. The same check id
/// recurs once per finding, so the target disambiguates instances. Used
/// to diff a before/after report and isolate the checks a single state
/// change introduced.
fn check_key(check: &EvidenceCiCheck) -> String {
    format!("{}@{}", check.id, check.target_id)
}

/// Keys of the release-blocking checks that are currently *failing*.
/// A change that adds a key here introduces a release-blocking
/// regression — the Engine gate blocks truth-bearing acceptances on
/// exactly this set.
pub fn release_blocking_failures(report: &EvidenceCiReport) -> BTreeSet<String> {
    report
        .checks
        .iter()
        .filter(|c| c.release_blocking && c.status == EvidenceCiStatus::Failed)
        .map(check_key)
        .collect()
}

/// Keys of the review-warning checks — review-readiness gaps (missing
/// source id, locator, evidence span, …) that do not block release but
/// a reviewer should see. The Engine surfaces the ones a change
/// introduces, and `--strict` blocks on them.
pub fn review_warnings(report: &EvidenceCiReport) -> BTreeSet<String> {
    report
        .checks
        .iter()
        .filter(|c| c.status == EvidenceCiStatus::Warning)
        .map(check_key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::make_finding;

    /// Assemble a report from a raw check list, running the same
    /// `finish()` roll-up the real pipeline uses.
    fn report_with(checks: Vec<EvidenceCiCheck>) -> EvidenceCiReport {
        EvidenceCiReport {
            ok: false,
            command: "evidence-ci".to_string(),
            frontier_id: "vfr_test".to_string(),
            frontier_path: "/tmp/frontier".to_string(),
            checked_at: "2026-06-23T00:00:00Z".to_string(),
            scope: "frontier".to_string(),
            summary: EvidenceCiSummary::default(),
            checks,
            caveats: vec![],
        }
        .finish()
    }

    #[test]
    fn classification_ranks_release_blocking_over_warning() {
        // A failed release-blocking check classifies as ReleaseBlocking
        // regardless of status; a bare warning is a ReviewWarning; and a
        // non-blocking pass is Info.
        assert_eq!(
            classification_for_check(&EvidenceCiStatus::Failed, true),
            EvidenceCiClassification::ReleaseBlocking
        );
        // release_blocking flag dominates even for a passing check.
        assert_eq!(
            classification_for_check(&EvidenceCiStatus::Passed, true),
            EvidenceCiClassification::ReleaseBlocking
        );
        assert_eq!(
            classification_for_check(&EvidenceCiStatus::Warning, false),
            EvidenceCiClassification::ReviewWarning
        );
        assert_eq!(
            classification_for_check(&EvidenceCiStatus::Passed, false),
            EvidenceCiClassification::Info
        );
    }

    #[test]
    fn group_assignment_follows_check_id_prefix() {
        // Group is derived from the check id namespace, not its status.
        assert_eq!(
            group_for_check("source.id_presence", &EvidenceCiStatus::Passed),
            "source_locator_coverage"
        );
        // trial.registry_reference is folded into source coverage by name.
        assert_eq!(
            group_for_check("trial.registry_reference", &EvidenceCiStatus::Warning),
            "source_locator_coverage"
        );
        assert_eq!(
            group_for_check("condition.endpoint", &EvidenceCiStatus::Warning),
            "evidence_atom_quality"
        );
        assert_eq!(
            group_for_check("policy.review_requirement", &EvidenceCiStatus::Failed),
            "policy_requirements"
        );
        assert_eq!(
            group_for_check("proof.freshness", &EvidenceCiStatus::Warning),
            "stale_proof"
        );
    }

    #[test]
    fn release_ok_only_when_no_blocking_check_fails() {
        // A failing release-blocking check drops the report to not-ok and
        // records exactly one release_blocking_failed; the report `ok` mirrors it.
        let blocked = report_with(vec![
            fail(
                "policy.review_requirement",
                "frontier",
                "vfr_test",
                "policy missing",
                None,
                true,
            ),
            warn("source.id_presence", "finding", "vf_a", "no source", None),
        ]);
        assert!(!blocked.ok);
        assert_eq!(blocked.summary.release_blocking_failed, 1);
        assert_eq!(blocked.summary.failed, 1);
        assert_eq!(blocked.summary.warnings, 1);

        // A warning-only report has no blocking failure, so the release is ok.
        let clean = report_with(vec![warn(
            "source.id_presence",
            "finding",
            "vf_a",
            "no source",
            None,
        )]);
        assert!(clean.ok);
        assert_eq!(clean.summary.release_blocking_failed, 0);
    }

    #[test]
    fn failing_blocking_pass_does_not_block() {
        // A release-blocking check that PASSES is not a failure — only a
        // failing one blocks. This guards the difference between "this check
        // can block" and "this check is blocking right now".
        let report = report_with(vec![pass(
            "policy.review_requirement",
            "frontier",
            "vfr_test",
            "policy present",
            None,
            true,
        )]);
        assert!(report.ok);
        assert_eq!(report.summary.release_blocking, 1);
        assert_eq!(report.summary.release_blocking_failed, 0);
    }

    #[test]
    fn blocking_failures_and_warnings_keyed_by_id_and_target() {
        // The Engine diffs these keyed sets to find regressions. Keys are
        // `id@target_id`, so the same check id on two findings is two keys.
        let report = report_with(vec![
            fail(
                "policy.review_requirement",
                "frontier",
                "vfr_test",
                "policy missing",
                None,
                true,
            ),
            warn("source.id_presence", "finding", "vf_a", "no source", None),
            warn("source.id_presence", "finding", "vf_b", "no source", None),
        ]);

        let blocking = release_blocking_failures(&report);
        assert_eq!(blocking.len(), 1);
        assert!(blocking.contains("policy.review_requirement@vfr_test"));

        let warnings = review_warnings(&report);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.contains("source.id_presence@vf_a"));
        assert!(warnings.contains("source.id_presence@vf_b"));
        // A warning is never counted as a release-blocking failure.
        assert!(!blocking.contains("source.id_presence@vf_a"));
    }

    #[test]
    fn study_design_checks_skip_theoretical_findings() {
        // A theorem/conjecture-typed finding has no comparator arm or trial,
        // so the clinical study-design checks are not applicable...
        let theorem = make_finding("vf_thm", 0.9, "theorem");
        assert!(!is_study_design_applicable(&theorem));
        let conjecture = make_finding("vf_conj", 0.5, "conjecture");
        assert!(!is_study_design_applicable(&conjecture));

        // ...but an empirical (experimental) finding keeps them.
        let empirical = make_finding("vf_exp", 0.7, "experimental");
        assert!(is_study_design_applicable(&empirical));
    }

    #[test]
    fn theoretical_finding_with_trial_signal_keeps_study_checks() {
        // A theoretical-typed finding that still mentions a trial in its text
        // carries empirical signal, so the study-design checks re-apply — a
        // computational study of a clinical trial must not be waved through.
        let mut finding = make_finding("vf_mix", 0.6, "theorem");
        finding.assertion.text = "A computational model of a phase 3 trial".to_string();
        assert!(mentions_trial(&finding_text(&finding)));
        assert!(is_study_design_applicable(&finding));
    }

    #[test]
    fn endpoint_and_trial_text_detectors_match_expected_signals() {
        // Endpoint detection keys off measured-outcome vocabulary.
        assert!(has_endpoint("primary endpoint was overall survival"));
        assert!(has_endpoint("hazard ratio 0.7"));
        assert!(!has_endpoint("a purely combinatorial statement"));

        // Trial detection keys off trial / phase / randomized.
        assert!(mentions_trial("a randomized study"));
        assert!(mentions_trial("phase 2 results"));
        assert!(!mentions_trial("an upper bound on b_2 sets"));
    }

    #[test]
    fn theoretical_finding_records_study_dimensions_as_passes() {
        // Running the finding-level checks on a theorem records the four
        // study-design dimensions as not-applicable PASSES (not warnings),
        // so a math finding raises no spurious clinical review gaps.
        let theorem = make_finding("vf_thm", 0.9, "theorem");
        let mut checks = Vec::new();
        add_finding_checks(&mut checks, &theorem, &[], &[], &[]);

        for id in [
            "trial.registry_reference",
            "condition.population",
            "condition.comparator_or_baseline",
            "condition.endpoint",
        ] {
            let check = checks
                .iter()
                .find(|c| c.id == id)
                .unwrap_or_else(|| panic!("missing check {id}"));
            assert_eq!(
                check.status,
                EvidenceCiStatus::Passed,
                "{id} should be a not-applicable pass on a theorem"
            );
        }
        // None of the finding-level checks are release-blocking.
        assert!(checks.iter().all(|c| !c.release_blocking));
    }
}
