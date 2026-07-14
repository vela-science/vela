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

use std::io::Read;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
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
    /// Optional, non-authorizing producer coordination data from a rich
    /// `campaign.yaml` target. Vela pins its fixed base and authority ceiling;
    /// neither value is delegated to campaign metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
}

const PRODUCER_AUTHORITY_CEILING: &str = "Producer evidence only. The session can create a receipt and proposal; it cannot create human acceptance.";
const CAMPAIGN_YAML_MAX_BYTES: u64 = 1024 * 1024;
const CAMPAIGN_TASK_MAX_BYTES: usize = 256 * 1024;
const CAMPAIGN_MAX_BATCHES: usize = 4096;
const CAMPAIGN_MAX_SEEDS: usize = 16_384;
pub const EXTERNAL_TARGET_ID_MAX_BYTES: usize = 256;

#[derive(Debug, Clone)]
struct CampaignSeed {
    batch: String,
    handle: String,
    explicit_id: bool,
    coverage_token: Option<String>,
    title: Option<String>,
    why: Option<String>,
    task: Option<Value>,
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

fn yaml_scalar_string(value: &serde_yaml::Value) -> Option<String> {
    match (value.as_i64(), value.as_str()) {
        (Some(number), _) => Some(number.to_string()),
        (_, Some(text)) if !text.trim().is_empty() => Some(text.to_string()),
        _ => None,
    }
}

/// The single grammar shared by campaign offers and the lease write edge.
/// Keeping this shell-safe means `next_command` can remain a plain positional
/// command without quoting or option ambiguity.
pub fn validate_external_target_id(target: &str) -> Result<(), String> {
    if target.is_empty() || target.len() > EXTERNAL_TARGET_ID_MAX_BYTES {
        return Err(format!(
            "external target id must be 1..={EXTERNAL_TARGET_ID_MAX_BYTES} bytes"
        ));
    }
    if target.starts_with('-') || target.starts_with("vf_") {
        return Err(
            "external target id must not start with '-' or use the reserved vf_ prefix".to_string(),
        );
    }
    if !target.contains(':')
        || target
            .split(':')
            .any(|segment| segment.is_empty() || segment.starts_with('-'))
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-'))
    {
        return Err(
            "external target id must have non-empty, non-option-like ':'-separated segments using only ASCII letters, digits, '.', '_', and '-'"
                .to_string(),
        );
    }
    Ok(())
}

fn read_campaign_yaml(dir: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = dir.join("campaign.yaml");
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "campaign file must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > CAMPAIGN_YAML_MAX_BYTES {
        return Err(format!(
            "campaign file {} exceeds the {CAMPAIGN_YAML_MAX_BYTES}-byte limit",
            path.display()
        ));
    }
    let file = std::fs::File::open(&path)
        .map_err(|error| format!("open campaign file {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("inspect open campaign file {}: {error}", path.display()))?;
    if !opened.is_file() || opened.len() > CAMPAIGN_YAML_MAX_BYTES {
        return Err(format!(
            "campaign file must remain a regular file within the {CAMPAIGN_YAML_MAX_BYTES}-byte limit: {}",
            path.display()
        ));
    }
    if !same_file_identity(&metadata, &opened) {
        return Err(format!(
            "campaign file changed while it was being opened: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(CAMPAIGN_YAML_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read campaign file {}: {error}", path.display()))?;
    if bytes.len() as u64 > CAMPAIGN_YAML_MAX_BYTES {
        return Err(format!(
            "campaign file {} exceeds the {CAMPAIGN_YAML_MAX_BYTES}-byte limit",
            path.display()
        ));
    }
    let named = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("reinspect campaign file {}: {error}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || !same_file_identity(&opened, &named) {
        return Err(format!(
            "campaign file changed while it was being read: {}",
            path.display()
        ));
    }
    Ok(Some(bytes))
}

#[cfg(unix)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(not(any(unix, windows)))]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

fn bounded_campaign_task(value: &serde_yaml::Value) -> Result<Value, String> {
    let task = serde_json::to_value(value)
        .map_err(|error| format!("campaign task is not JSON-compatible: {error}"))?;
    if !task.is_object() {
        return Err("campaign task must be an object".to_string());
    }
    let encoded = serde_json::to_vec(&task)
        .map_err(|error| format!("encode campaign task for its size boundary: {error}"))?;
    if encoded.len() > CAMPAIGN_TASK_MAX_BYTES {
        return Err(format!(
            "campaign task exceeds the {CAMPAIGN_TASK_MAX_BYTES}-byte limit"
        ));
    }
    Ok(task)
}

/// Campaign seeds from `<dir>/campaign.yaml`: `batches: [{name, state,
/// problems: […]}]`. A problem may remain the legacy integer/string scalar or
/// use the additive mapping `{id, title?, why?, task?}`. Explicit mapping IDs
/// are target handles, not values to be re-namespaced. Terminal batch states
/// are skipped; anything else is an open seed. File order remains the ranking.
fn campaign_seeds(dir: &Path, namespace: &str) -> Result<Vec<CampaignSeed>, String> {
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
    let Some(body) = read_campaign_yaml(dir)? else {
        return Ok(Vec::new());
    };
    let doc = serde_yaml::from_slice::<serde_yaml::Value>(&body)
        .map_err(|error| format!("parse campaign.yaml: {error}"))?;
    let mut seeds = Vec::new();
    let Some(batches) = doc.get("batches").and_then(|b| b.as_sequence()) else {
        return Ok(seeds);
    };
    if batches.len() > CAMPAIGN_MAX_BATCHES {
        return Err(format!(
            "campaign has {} batches; limit is {CAMPAIGN_MAX_BATCHES}",
            batches.len()
        ));
    }
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
                if let Some(handle) = yaml_scalar_string(p) {
                    seeds.push(CampaignSeed {
                        batch: name.clone(),
                        coverage_token: Some(handle.clone()),
                        handle,
                        explicit_id: false,
                        title: None,
                        why: None,
                        task: None,
                    });
                    if seeds.len() > CAMPAIGN_MAX_SEEDS {
                        return Err(format!("campaign has more than {CAMPAIGN_MAX_SEEDS} seeds"));
                    }
                    continue;
                }
                let Some(mapping) = p.as_mapping() else {
                    continue;
                };
                let id = mapping
                    .get(serde_yaml::Value::String("id".to_string()))
                    .and_then(yaml_scalar_string);
                let problem = mapping
                    .get(serde_yaml::Value::String("problem".to_string()))
                    .and_then(yaml_scalar_string);
                let (handle, explicit_id) = match (id, problem.as_ref()) {
                    (Some(id), _) => (id, true),
                    (None, Some(problem)) => (problem.clone(), false),
                    (None, None) => continue,
                };
                let numeric_suffix = handle
                    .rsplit_once(':')
                    .map(|(_, suffix)| suffix)
                    .unwrap_or(&handle);
                let coverage_token = problem.or_else(|| {
                    numeric_suffix
                        .chars()
                        .all(|character| character.is_ascii_digit())
                        .then(|| numeric_suffix.to_string())
                });
                let field = |name: &str| {
                    mapping
                        .get(serde_yaml::Value::String(name.to_string()))
                        .and_then(serde_yaml::Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .map(ToString::to_string)
                };
                let task = mapping
                    .get(serde_yaml::Value::String("task".to_string()))
                    .map(bounded_campaign_task)
                    .transpose()?;
                seeds.push(CampaignSeed {
                    batch: name.clone(),
                    handle,
                    explicit_id,
                    coverage_token,
                    title: field("title"),
                    why: field("why"),
                    task,
                });
                if seeds.len() > CAMPAIGN_MAX_SEEDS {
                    return Err(format!("campaign has more than {CAMPAIGN_MAX_SEEDS} seeds"));
                }
            }
        }
    }
    let mut resolved = std::collections::BTreeSet::new();
    for seed in &seeds {
        let target = campaign_target_id(seed, namespace);
        validate_external_target_id(&target)
            .map_err(|error| format!("invalid campaign target {target:?}: {error}"))?;
        if !resolved.insert(target.clone()) {
            return Err(format!("duplicate resolved campaign target id {target:?}"));
        }
    }
    Ok(seeds)
}

fn campaign_target_id(seed: &CampaignSeed, namespace: &str) -> String {
    if seed.explicit_id {
        seed.handle.clone()
    } else {
        format!("{namespace}:{}", seed.handle)
    }
}

/// Normalize descriptive campaign task metadata against trusted frontier
/// state. Authority-shaped fields are not accepted from the campaign; the
/// executable work-session contract is built independently by the CLI.
fn pinned_campaign_task(project: &Project, raw: &Value) -> Option<Value> {
    let mut task = raw.as_object()?.clone();
    for field in [
        "authority",
        "authority_ceiling",
        "allowed_actions",
        "forbidden_actions",
        "decision",
        "escalation_path",
        "human_key",
        "policy",
        "route",
        "sign",
        "verdict",
    ] {
        task.remove(field);
    }
    task.insert(
        "fixed_base".to_string(),
        json!({
            "frontier_id": project.frontier_id(),
            "event_log_root": format!(
                "sha256:{}",
                vela_protocol::events::event_log_hash(&project.events)
            ),
        }),
    );
    task.insert(
        "authority_ceiling".to_string(),
        Value::String(PRODUCER_AUTHORITY_CEILING.to_string()),
    );
    Some(Value::Object(task))
}

/// Return the same pinned optional campaign task used by `next`/`orient` for
/// a `work` target. This is coordination data only; it cannot alter policy,
/// leases, the work-session contract, or accepted state.
pub fn campaign_task_for_target(
    project: &Project,
    dir: &Path,
    target: &str,
) -> Result<Option<Value>, String> {
    let namespace = lease_namespace(project);
    Ok(campaign_seeds(dir, &namespace)?
        .into_iter()
        .find(|seed| campaign_target_id(seed, &namespace) == target)
        .and_then(|seed| seed.task)
        .and_then(|task| pinned_campaign_task(project, &task)))
}

/// The obligation namespace in live use: the modal prefix of existing
/// lease ids (`erdos:443` → `erdos`), falling back to `seed`.
fn lease_namespace(project: &Project) -> String {
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for l in &project.attempt_claims {
        if let Some((ns, _)) = l.obligation_id.split_once(':') {
            *counts.entry(ns).or_default() += 1;
        }
    }
    let mut counts = counts.into_iter().collect::<Vec<_>>();
    counts.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    counts
        .first()
        .map(|(namespace, _)| (*namespace).to_string())
        .unwrap_or_else(|| "seed".to_string())
}

pub fn try_frontier_next(
    project: &Project,
    reviews: &[ReviewSnapshot],
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
) -> Result<Vec<NextTarget>, String> {
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
            task: None,
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
        for seed in campaign_seeds(dir, &ns)? {
            let obligation = campaign_target_id(&seed, &ns);
            if live_leases.contains(&obligation) || live_leases.contains(&seed.handle) {
                continue;
            }
            if let Some(token) = seed.coverage_token.as_deref()
                && seed_covered(
                    project.findings.iter().map(|b| b.assertion.text.as_str()),
                    token,
                )
            {
                continue;
            }
            let task = seed
                .task
                .as_ref()
                .and_then(|raw| pinned_campaign_task(project, raw));
            actionable_targets.push(NextTarget {
                lane: "attack".into(),
                id: obligation.clone(),
                title: seed
                    .title
                    .unwrap_or_else(|| format!("{} seed {}", seed.batch, seed.handle)),
                why: seed.why.unwrap_or_else(|| {
                    "open campaign seed: no live lease, no landed statement".into()
                }),
                next_command: format!("vela work {obligation}"),
                task,
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
                    task: None,
                },
            ));
        }
    }
    // Closest to the bar first (more attachments = one run from verified), then
    // highest structural leverage (unblocks the most downstream work), then id.
    verify.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.id.cmp(&b.2.id)));
    actionable_targets.extend(verify.into_iter().map(|(_, _, target)| target));

    if limit == 0 {
        return Ok(Vec::new());
    }
    if actionable_targets.is_empty() {
        review_targets.truncate(limit);
        return Ok(review_targets);
    }
    if limit == 1 {
        return Ok(actionable_targets.into_iter().take(1).collect());
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
    Ok(targets)
}

/// Compatibility wrapper for callers that cannot report projection errors.
/// Invalid campaign bytes never reach an offer; checked CLI/MCP surfaces use
/// `try_frontier_next` and return the exact error instead.
pub fn frontier_next(
    project: &Project,
    reviews: &[ReviewSnapshot],
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
) -> Vec<NextTarget> {
    try_frontier_next(project, reviews, frontier_dir, observed_at, limit).unwrap_or_else(|_| {
        try_frontier_next(project, reviews, None, observed_at, limit).unwrap_or_default()
    })
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

    #[test]
    fn campaign_parser_keeps_scalar_compatibility_and_explicit_rich_ids() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("campaign.yaml"),
            r#"
batches:
  - name: prepared
    state: open
    problems:
      - 443
      - id: seed:prepared-target
        title: Reproduce the prepared declaration
        why: Public verifier exercise
        task:
          kind: external_lean_reproduction
          constraints:
            - Use the pinned source.
"#,
        )
        .unwrap();

        let seeds = campaign_seeds(temp.path(), "seed").unwrap();
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0].handle, "443");
        assert!(!seeds[0].explicit_id);
        assert!(seeds[0].task.is_none());
        assert_eq!(seeds[1].handle, "seed:prepared-target");
        assert!(seeds[1].explicit_id);
        assert_eq!(
            seeds[1].title.as_deref(),
            Some("Reproduce the prepared declaration")
        );
        assert_eq!(
            seeds[1]
                .task
                .as_ref()
                .and_then(|task| task["kind"].as_str()),
            Some("external_lean_reproduction")
        );
    }

    #[test]
    fn campaign_file_and_task_size_boundaries_are_exact() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("campaign.yaml");
        std::fs::write(&path, vec![b' '; CAMPAIGN_YAML_MAX_BYTES as usize]).unwrap();
        assert_eq!(
            read_campaign_yaml(temp.path()).unwrap().unwrap().len(),
            CAMPAIGN_YAML_MAX_BYTES as usize
        );
        std::fs::write(&path, vec![b' '; CAMPAIGN_YAML_MAX_BYTES as usize + 1]).unwrap();
        assert!(
            read_campaign_yaml(temp.path())
                .unwrap_err()
                .contains("exceeds")
        );

        let empty_overhead = serde_json::to_vec(&json!({"x": ""})).unwrap().len();
        let at_limit = serde_yaml::to_value(json!({
            "x": "x".repeat(CAMPAIGN_TASK_MAX_BYTES - empty_overhead)
        }))
        .unwrap();
        let task = bounded_campaign_task(&at_limit).unwrap();
        assert_eq!(
            serde_json::to_vec(&task).unwrap().len(),
            CAMPAIGN_TASK_MAX_BYTES
        );
        let over_limit = serde_yaml::to_value(json!({
            "x": "x".repeat(CAMPAIGN_TASK_MAX_BYTES - empty_overhead + 1)
        }))
        .unwrap();
        assert!(
            bounded_campaign_task(&over_limit)
                .unwrap_err()
                .contains("exceeds")
        );
    }

    #[test]
    fn resolved_campaign_ids_reject_unsafe_empty_and_duplicate_values() {
        for invalid in [
            "seed:foo;id",
            "seed:foo bar",
            "seed:foo\nbar",
            "seed:",
            ":foo",
            "-seed:foo",
            "seed:-foo",
        ] {
            assert!(
                validate_external_target_id(invalid).is_err(),
                "accepted unsafe external target {invalid:?}"
            );
        }
        assert!(validate_external_target_id("seed:prepared-target").is_ok());

        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("campaign.yaml"),
            "batches:\n  - problems:\n      - foo;id\n",
        )
        .unwrap();
        assert!(
            campaign_seeds(temp.path(), "seed")
                .unwrap_err()
                .contains("invalid campaign target")
        );

        std::fs::write(
            temp.path().join("campaign.yaml"),
            "batches:\n  - problems:\n      - 443\n      - id: seed:443\n",
        )
        .unwrap();
        assert_eq!(
            campaign_seeds(temp.path(), "seed").unwrap_err(),
            "duplicate resolved campaign target id \"seed:443\""
        );
    }

    #[test]
    fn scalar_seed_count_and_namespace_ties_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let campaign = |count: usize| {
            let mut body = "batches:\n  - problems:\n".to_string();
            for index in 0..count {
                body.push_str(&format!("      - {index}\n"));
            }
            body
        };
        std::fs::write(
            temp.path().join("campaign.yaml"),
            campaign(CAMPAIGN_MAX_SEEDS),
        )
        .unwrap();
        assert_eq!(
            campaign_seeds(temp.path(), "seed").unwrap().len(),
            CAMPAIGN_MAX_SEEDS
        );
        std::fs::write(
            temp.path().join("campaign.yaml"),
            campaign(CAMPAIGN_MAX_SEEDS + 1),
        )
        .unwrap();
        assert!(
            campaign_seeds(temp.path(), "seed")
                .unwrap_err()
                .contains("more than")
        );

        let mut project =
            vela_protocol::project::assemble("namespace-tie", Vec::new(), 0, 0, "test");
        for namespace in ["zeta", "alpha"] {
            project
                .attempt_claims
                .push(vela_protocol::project::AttemptClaim {
                    obligation_id: format!("{namespace}:1"),
                    claimant_actor: "agent:test".to_string(),
                    claimant_pubkey: "00".repeat(32),
                    claimed_at: "2026-07-14T00:00:00Z".to_string(),
                    lease_ttl_seconds: 1,
                    claim_event_id: None,
                });
        }
        assert_eq!(lease_namespace(&project), "alpha");
    }

    #[cfg(unix)]
    #[test]
    fn campaign_file_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.yaml");
        std::fs::write(&source, "batches: []\n").unwrap();
        symlink(&source, temp.path().join("campaign.yaml")).unwrap();
        assert!(
            read_campaign_yaml(temp.path())
                .unwrap_err()
                .contains("non-symlink")
        );
    }
}
