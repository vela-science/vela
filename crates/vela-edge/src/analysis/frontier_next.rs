//! `vela frontier next` — the "what should I work on" projection.
//!
//! The swarm runs proved the gap: agents picked targets by convention
//! (reading a generated markdown file) because the substrate had no
//! answer to the first question every worker asks. This module derives
//! one, read-only, from state the frontier already carries:
//!
//! - **review** — undecided packs and loose pending proposals: the
//!   human's decisions, listed first because a decision unblocks
//!   everything behind it.
//! - **attack** — open campaign seeds (`campaign.yaml`, when the
//!   frontier carries one): problems in non-terminal batches with no
//!   live lease and no landed statement finding. Batch order is kept —
//!   the file IS the curated ranking.
//! - **verify** — accepted findings the gate still holds at
//!   `needs_verification`: the honest accepted-but-unverified gap,
//!   closest-to-the-bar first.
//!
//! A ranking is advice, never authority: nothing here mutates state,
//! and claiming a target still goes through the lease tool.

use std::path::Path;

use serde::Serialize;
use vela_protocol::project::Project;
use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};

use super::decision_brief::ReviewSnapshot;

#[derive(Debug, Clone, Serialize)]
pub struct NextTarget {
    /// "review" | "attack" | "verify"
    pub lane: String,
    /// The target handle: `vsd_…` / `vpr_…` / a seed obligation id / `vf_…`.
    pub id: String,
    pub title: String,
    pub why: String,
    pub next_command: String,
}

/// A pack awaits a decision only while it has no verdict AND at least
/// one member proposal is still pending. A reviewer who accepts the
/// members individually (`--all-pending`) leaves the pack verdict-less
/// but decided in substance — listing it as blocked would be a lie.
pub fn pack_awaits_decision(
    rec: &vela_protocol::released_diff_pack::ReleasedDiffPackRecord,
    project: &Project,
) -> bool {
    rec.verdict.is_none()
        && !rec.member_proposals.is_empty()
        && rec.member_proposals.iter().any(|m| {
            project
                .proposals
                .iter()
                .any(|p| &p.id == m && p.status == "pending_review" && p.applied_event_id.is_none())
        })
}

/// Is this lease still live at `now` (RFC3339 comparison via chrono)?
fn lease_live_at(
    claimed_at: &str,
    ttl_seconds: u64,
    observed_at: Option<&chrono::DateTime<chrono::Utc>>,
) -> bool {
    let Some(observed_at) = observed_at else {
        return true;
    };
    chrono::DateTime::parse_from_rfc3339(claimed_at)
        .map(|claimed| claimed + chrono::Duration::seconds(ttl_seconds as i64) > *observed_at)
        .unwrap_or(true)
}

/// Does any assertion reference seed `n` as a `#n` token
/// (word-boundary on the right, so `#44` does not cover `#443`)?
fn seed_covered<'a>(mut assertions: impl Iterator<Item = &'a str>, n: &str) -> bool {
    let token = format!("#{n}");
    assertions.any(|text| {
        text.match_indices(&token).any(|(i, _)| {
            text[i + token.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_digit())
        })
    })
}

/// Campaign seeds from `<dir>/campaign.yaml`: `batches: [{name, state,
/// problems: […]}]`. Terminal batch states are skipped; anything else is
/// an open seed. Returns `(batch_name, problem)` in file order.
fn campaign_seeds(dir: &Path) -> Vec<(String, String)> {
    // Terminal AND in-flight states are both skipped: a batch sitting in
    // an open upstream PR is claimed work, not an open seed.
    const TERMINAL: &[&str] = &[
        "merged",
        "landed",
        "done",
        "closed",
        "accepted",
        "retired",
        "pr-open",
        "packeted",
        "submitted",
        "in-review",
    ];
    let Ok(body) = std::fs::read_to_string(dir.join("campaign.yaml")) else {
        return Vec::new();
    };
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&body) else {
        return Vec::new();
    };
    let mut seeds = Vec::new();
    let Some(batches) = doc.get("batches").and_then(|b| b.as_sequence()) else {
        return seeds;
    };
    for batch in batches {
        let state = batch
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("open");
        if TERMINAL.contains(&state) {
            continue;
        }
        let name = batch
            .get("name")
            .and_then(|s| s.as_str())
            .unwrap_or("batch")
            .to_string();
        if let Some(problems) = batch.get("problems").and_then(|p| p.as_sequence()) {
            for p in problems {
                let id = match (p.as_i64(), p.as_str()) {
                    (Some(n), _) => n.to_string(),
                    (_, Some(s)) => s.to_string(),
                    _ => continue,
                };
                seeds.push((name.clone(), id));
            }
        }
    }
    seeds
}

/// The obligation namespace in live use: the modal prefix of existing
/// lease ids (`erdos:443` → `erdos`), falling back to `seed`.
fn lease_namespace(project: &Project) -> String {
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for l in &project.attempt_claims {
        if let Some((ns, _)) = l.obligation_id.split_once(':') {
            *counts.entry(ns).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(ns, _)| ns.to_string())
        .unwrap_or_else(|| "seed".to_string())
}

pub fn frontier_next(
    project: &Project,
    reviews: &[ReviewSnapshot],
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
) -> Vec<NextTarget> {
    let observed_at = chrono::DateTime::parse_from_rfc3339(observed_at)
        .ok()
        .map(|time| time.to_utc());
    let mut review_targets = Vec::new();
    let mut actionable_targets = Vec::new();

    // ── review: the same selected Decision Briefs used everywhere else ──
    for review in reviews {
        review_targets.push(NextTarget {
            lane: "review".into(),
            id: review.brief.audit.proposal_id.clone(),
            title: review.brief.change.claim.chars().take(80).collect(),
            why: format!(
                "{} · accept {} · reject {} · facts {}",
                review.brief.authority.route,
                review
                    .brief
                    .action("accept")
                    .map(|action| action.eligibility.as_str())
                    .unwrap_or("unavailable"),
                review
                    .brief
                    .action("reject")
                    .map(|action| action.eligibility.as_str())
                    .unwrap_or("unavailable"),
                review.brief.audit.decision_facts_root,
            ),
            next_command: format!("vela diff {}", review.brief.audit.proposal_id),
        });
    }

    // ── attack: open campaign seeds, unleased and unlanded ─────────────
    if let Some(dir) = frontier_dir {
        let ns = lease_namespace(project);
        let live_leases: std::collections::BTreeSet<String> = project
            .attempt_claims
            .iter()
            .filter(|lease| {
                lease_live_at(
                    &lease.claimed_at,
                    lease.lease_ttl_seconds,
                    observed_at.as_ref(),
                )
            })
            .map(|l| l.obligation_id.clone())
            .collect();
        for (batch, seed) in campaign_seeds(dir) {
            let obligation = format!("{ns}:{seed}");
            if live_leases.contains(&obligation) || live_leases.contains(&seed) {
                continue;
            }
            if seed_covered(
                project.findings.iter().map(|b| b.assertion.text.as_str()),
                &seed,
            ) {
                continue;
            }
            actionable_targets.push(NextTarget {
                lane: "attack".into(),
                id: obligation.clone(),
                title: format!("{batch} seed {seed}"),
                why: "open campaign seed: no live lease, no landed statement".into(),
                next_command: format!("vela work {obligation}"),
            });
        }
    }

    // ── verify: accepted findings the gate refuses ─────────────────────
    let mut by_target: std::collections::HashMap<&str, Vec<_>> = std::collections::HashMap::new();
    for a in &project.verifier_attachments {
        by_target.entry(a.target.as_str()).or_default().push(a);
    }
    // Structural leverage: how many findings rest on X as a required premise
    // (`depends`/`synthesized_from`/`derived_from`/`discharges`). Verifying a
    // high-leverage finding unblocks more downstream work — the structural
    // signal from `frontier_identification`, applied as the verify-lane tiebreak.
    let mut unlock: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for f in &project.findings {
        for l in &f.links {
            if matches!(
                l.link_type.as_str(),
                "depends" | "synthesized_from" | "derived_from" | "discharges"
            ) {
                *unlock.entry(l.target.as_str()).or_default() += 1;
            }
        }
    }
    // (attachment_count, unlock_count, target)
    let mut verify: Vec<(usize, usize, NextTarget)> = Vec::new();
    for bundle in &project.findings {
        use vela_protocol::bundle::ReviewState;
        if !matches!(bundle.flags.review_state, Some(ReviewState::Accepted)) {
            continue;
        }
        let attachments: Vec<_> = by_target
            .get(bundle.id.as_str())
            .map(|v| v.iter().map(|a| (*a).clone()).collect())
            .unwrap_or_default();
        let outcome = derive_gate_status(&claim_digest(&bundle.assertion.text), &attachments);
        if outcome.status == GateStatus::NeedsVerification {
            let lev = unlock.get(bundle.id.as_str()).copied().unwrap_or(0);
            let why = match outcome.reasons.first() {
                Some(r) if lev > 0 => format!("{r} ({lev} finding(s) rest on this)"),
                Some(r) => r.clone(),
                None => "accepted but unverified".into(),
            };
            verify.push((
                attachments.len(),
                lev,
                NextTarget {
                    lane: "verify".into(),
                    id: bundle.id.clone(),
                    title: bundle.assertion.text.chars().take(80).collect(),
                    why,
                    next_command: format!("vela work {}", bundle.id),
                },
            ));
        }
    }
    // Closest to the bar first (more attachments = one run from verified), then
    // highest structural leverage (unblocks the most downstream work), then id.
    verify.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.id.cmp(&b.2.id)));
    actionable_targets.extend(verify.into_iter().map(|(_, _, target)| target));

    if limit == 0 {
        return Vec::new();
    }
    if actionable_targets.is_empty() {
        review_targets.truncate(limit);
        return review_targets;
    }
    if limit == 1 {
        return actionable_targets.into_iter().take(1).collect();
    }
    let mut targets = Vec::with_capacity(limit);
    if let Some(review) = review_targets.first().cloned() {
        targets.push(review);
        review_targets.remove(0);
    }
    if let Some(actionable) = actionable_targets.first().cloned() {
        targets.push(actionable);
        actionable_targets.remove(0);
    }
    targets.extend(review_targets);
    targets.extend(actionable_targets);
    targets.truncate(limit);
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_token_matching_respects_digit_boundary() {
        let texts = ["FC statement draft for Erdős #443: gate green"];
        assert!(seed_covered(texts.iter().copied(), "443"));
        assert!(!seed_covered(texts.iter().copied(), "44"));
        assert!(!seed_covered(texts.iter().copied(), "4"));
    }

    #[test]
    fn expired_lease_is_not_live() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-13T00:00:00Z")
            .unwrap()
            .to_utc();
        assert!(!lease_live_at("2020-01-01T00:00:00+00:00", 60, Some(&now)));
    }
}
