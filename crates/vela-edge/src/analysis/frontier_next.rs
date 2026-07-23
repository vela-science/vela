//! `vela frontier next` — the "what should I work on" projection.
//!
//! The swarm runs proved the gap: agents picked targets by convention
//! (reading a generated markdown file) because the substrate had no
//! answer to the first question every worker asks. This module derives
//! one, read-only, from state the frontier already carries:
//!
//! - **attack** — open entries from a derived, hash-pinned `targets.json`
//!   catalogue and open campaign seeds (`campaign.yaml`, when present).
//!   Neither projection is authority; both only prepare a work target.
//! Review has its own `vela review` surface and structural opportunities have
//! `vela frontier rank`. Mixing either into `vela.offer.v1` would make advice
//! or human work look like an executable producer target.
//!
//! A ranking is advice, never authority: nothing here mutates state,
//! and claiming a target still goes through the lease tool.

use std::io::Read;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};
use vela_protocol::project::Project;

#[derive(Debug, Clone, Serialize)]
pub struct NextTarget {
    /// "seed" | "attack"
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

/// One configured producer target withheld from the offer list by a live
/// coordination lease. This is a read-only projection: it carries no key
/// material and grants no authority to the claimant.
#[derive(Debug, Clone, Serialize)]
pub struct LeasedProducerTarget {
    pub target_id: String,
    pub title: String,
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_event_id: Option<String>,
    pub claimed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Counts for the configured producer queue before and after live leases are
/// applied. Review and structural verification suggestions are intentionally
/// excluded: they are not configured producer targets.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProducerWorkAvailability {
    pub configured_open: usize,
    pub available: usize,
    pub leased: usize,
    /// Configured open entries withheld because their index or packet is
    /// historical, stale, or otherwise fails closed.
    pub stale: usize,
    pub leased_targets: Vec<LeasedProducerTarget>,
}

/// The complete read-only `next` projection. `targets` contains producer work
/// only; `producer_work` explains why an otherwise open configured target may
/// be absent from that list.
#[derive(Debug, Clone, Serialize)]
pub struct FrontierNextProjection {
    pub targets: Vec<NextTarget>,
    pub producer_work: ProducerWorkAvailability,
}

const PRODUCER_AUTHORITY_CEILING: &str = "Producer evidence only. The session can create a receipt and proposal; it cannot create human acceptance.";
const CAMPAIGN_YAML_MAX_BYTES: u64 = 1024 * 1024;
const CAMPAIGN_TASK_MAX_BYTES: usize = 256 * 1024;
const CAMPAIGN_MAX_BATCHES: usize = 4096;
const CAMPAIGN_MAX_SEEDS: usize = 16_384;
pub use super::target_index::EXTERNAL_TARGET_ID_MAX_BYTES;

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

#[derive(Debug, Clone)]
pub struct TargetIndexSelection {
    pub packet: Value,
    pub task: Value,
    pub binding: super::target_index::TargetTaskBindingV1,
}

fn pinned_target_index_task(
    project: &Project,
    assessment: &super::target_index::TargetIndexAssessment,
    target: &super::target_index::TargetIndexEntryV2,
) -> Value {
    let index = assessment
        .v2()
        .expect("a v2 target can only come from a v2 assessment");
    json!({
        "kind": "target_packet",
        "objective": target.objective,
        "state": target.state,
        "rank": target.rank,
        "labels": target.labels,
        "packet_ref": target.packet,
        "index": {
            "path": "targets.json",
            "schema": index.schema,
            "root": index.index_root,
            "file_sha256": assessment.document_root,
            "source": index.source,
            "input_root": index.inputs.input_root,
            "roots": index.roots,
            "stale_against_loaded_frontier": false,
        },
        "fixed_base": {
            "frontier_id": project.frontier_id(),
            "event_log_root": index.roots.event_log_root,
            "nonlease_event_log_root": index.roots.nonlease_event_log_root,
        },
        "authority_ceiling": PRODUCER_AUTHORITY_CEILING,
    })
}

/// Resolve one actionable v2 target from one Git-backed assessment. Historical
/// v1 entries remain inspectable through `target_index::assess_target_index`,
/// but they cannot cross this work-selection edge.
pub fn target_index_selection_for_target(
    project: &Project,
    dir: &Path,
    target_id: &str,
) -> Result<Option<TargetIndexSelection>, String> {
    target_index_selection_for_target_with_trust_anchor(project, dir, target_id, None)
}

/// Resolve one actionable v2 target using an independently retained first
/// repository-boundary trust pin. The pin is never inferred from the target
/// index or any other bytes in the Frontier under assessment.
pub fn target_index_selection_for_target_with_trust_anchor(
    project: &Project,
    dir: &Path,
    target_id: &str,
    trust_anchor: Option<&super::frontier_repository::RepositoryTrustAnchor>,
) -> Result<Option<TargetIndexSelection>, String> {
    let Some(assessment) =
        super::target_index::assess_target_index_with_trust_anchor(project, dir, trust_anchor)?
    else {
        return Ok(None);
    };
    if assessment.is_historical_v1() {
        if assessment.indexed_ids().contains(target_id) {
            return Err(format!(
                "target index entry {target_id:?} is historical v1 inspection only; seal vela.target-index.v2 before work"
            ));
        }
        return Ok(None);
    }
    let Some(index) = assessment.v2() else {
        unreachable!("non-v1 assessment must be v2");
    };
    let Some(target) = index.targets.iter().find(|target| target.id == target_id) else {
        return Ok(None);
    };
    if target.state != "open" {
        return Err(format!(
            "target index entry {target_id:?} is {}, not open",
            target.state
        ));
    }
    if !assessment.global_issues.is_empty()
        || assessment
            .target_issues
            .get(target_id)
            .is_some_and(|issues| !issues.is_empty())
    {
        let codes = assessment
            .global_issues
            .iter()
            .chain(
                assessment
                    .target_issues
                    .get(target_id)
                    .into_iter()
                    .flatten(),
            )
            .map(|issue| issue.code)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "target index entry {target_id:?} is stale or invalid ({codes})"
        ));
    }
    let packet = assessment
        .packet_value(target_id)
        .ok_or_else(|| format!("target index entry {target_id:?} has no verified packet"))?;
    let binding =
        super::target_index::build_target_task_binding(project, dir, &assessment, target_id)?;
    Ok(Some(TargetIndexSelection {
        packet: json!({
            "kind": "target_packet",
            "target": target_id,
            "objective": target.objective,
            "packet": packet,
            "packet_ref": target.packet,
            "index": {
                "path": "targets.json",
                "schema": index.schema,
                "root": index.index_root,
                "file_sha256": assessment.document_root,
                "source": index.source,
                "input_root": index.inputs.input_root,
                "roots": index.roots,
                "stale_against_loaded_frontier": false,
            },
            "authority_ceiling": PRODUCER_AUTHORITY_CEILING,
            "caveat": "The target index and packet are derived briefing projections. Their exact bytes are pinned here, but only signed frontier events carry accepted truth.",
        }),
        task: pinned_target_index_task(project, &assessment, target),
        binding,
    }))
}

/// Compatibility accessor for existing internal callers. It only returns an
/// actionable v2 value; stale, terminal, or historical entries fail closed.
pub fn target_index_task_for_target(
    project: &Project,
    dir: &Path,
    target: &str,
) -> Result<Option<Value>, String> {
    Ok(target_index_selection_for_target(project, dir, target)?.map(|value| value.task))
}

/// Compatibility accessor for existing internal callers. Prefer
/// [`target_index_selection_for_target`] when both values are required so the
/// repository and index are assessed exactly once.
pub fn target_index_packet_for_target(
    project: &Project,
    dir: &Path,
    target: &str,
) -> Result<Option<Value>, String> {
    Ok(target_index_selection_for_target(project, dir, target)?.map(|value| value.packet))
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
    vela_protocol::events::attempt_lease_expiry(claimed_at, ttl_seconds)
        .is_ok_and(|expires_at| expires_at > *observed_at)
}

fn lease_expires_at(lease: &vela_protocol::project::AttemptClaim) -> Option<String> {
    vela_protocol::events::attempt_lease_expiry(&lease.claimed_at, lease.lease_ttl_seconds)
        .ok()
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn leased_producer_target(
    lease: &vela_protocol::project::AttemptClaim,
    target_id: &str,
    title: String,
) -> LeasedProducerTarget {
    LeasedProducerTarget {
        target_id: target_id.to_string(),
        title,
        actor: lease.claimant_actor.clone(),
        claim_event_id: lease.claim_event_id.clone(),
        claimed_at: lease.claimed_at.clone(),
        expires_at: lease_expires_at(lease),
    }
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

fn shell_target_argument(target: &str) -> String {
    if target
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-'))
    {
        target.to_string()
    } else {
        // The external-target grammar never admits quotes. Scientific notation
        // such as [[10,1,4]] is therefore safe and copyable as one quoted argv.
        format!("'{target}'")
    }
}

/// The single grammar shared by campaign offers and the lease write edge.
/// It admits a bounded square-bracket notation for conventional scientific
/// identifiers and quotes those identifiers in the human `next_command`.
pub fn validate_external_target_id(target: &str) -> Result<(), String> {
    super::target_index::validate_target_id(target)
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
    let initial_identity = same_file::Handle::from_path(&path)
        .map_err(|error| format!("identify campaign file {}: {error}", path.display()))?;
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
    let opened_identity = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone campaign file {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("identify open campaign file {}: {error}", path.display()))?;
    if initial_identity != opened_identity {
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
    let final_identity = same_file::Handle::from_path(&path)
        .map_err(|error| format!("reidentify campaign file {}: {error}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || opened_identity != final_identity {
        return Err(format!(
            "campaign file changed while it was being read: {}",
            path.display()
        ));
    }
    Ok(Some(bytes))
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

fn validate_producer_targets(targets: &[NextTarget], available: usize) -> Result<(), String> {
    let mut unique_ids = std::collections::BTreeSet::new();
    for target in targets {
        if !unique_ids.insert(target.id.as_str()) {
            return Err(format!(
                "producer work projection contains duplicate target id {}",
                target.id
            ));
        }
    }
    if targets.len() != available {
        return Err(format!(
            "producer work projection count mismatch: {available} available but {} offers",
            targets.len()
        ));
    }
    Ok(())
}

pub fn try_frontier_next_projection(
    project: &Project,
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
) -> Result<FrontierNextProjection, String> {
    try_frontier_next_projection_with_trust_anchor(project, frontier_dir, observed_at, limit, None)
}

pub fn try_frontier_next_projection_with_trust_anchor(
    project: &Project,
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
    trust_anchor: Option<&super::frontier_repository::RepositoryTrustAnchor>,
) -> Result<FrontierNextProjection, String> {
    let observed_at = chrono::DateTime::parse_from_rfc3339(observed_at)
        .ok()
        .map(|time| time.to_utc());
    let mut actionable_targets = Vec::new();
    let mut producer_work = ProducerWorkAvailability::default();

    // ── attack: open target-index entries and campaign seeds ───────────
    if let Some(dir) = frontier_dir {
        let ns = lease_namespace(project);
        let live_leases: std::collections::BTreeMap<&str, &vela_protocol::project::AttemptClaim> =
            project
                .attempt_claims
                .iter()
                .filter(|lease| {
                    lease_live_at(
                        &lease.claimed_at,
                        lease.lease_ttl_seconds,
                        observed_at.as_ref(),
                    )
                })
                .map(|lease| (lease.obligation_id.as_str(), lease))
                .collect();
        let mut indexed_ids = std::collections::BTreeSet::new();
        if let Some(assessment) =
            super::target_index::assess_target_index_with_trust_anchor(project, dir, trust_anchor)?
        {
            indexed_ids.extend(assessment.indexed_ids().into_iter().map(str::to_string));
            producer_work.configured_open += assessment.configured_open();
            producer_work.stale += assessment.stale_open();
            for target in assessment.fresh_open_v2_targets() {
                if let Some(lease) = live_leases.get(target.id.as_str()) {
                    producer_work.leased += 1;
                    producer_work.leased_targets.push(leased_producer_target(
                        lease,
                        &target.id,
                        target.title.clone(),
                    ));
                    continue;
                }
                producer_work.available += 1;
                actionable_targets.push(NextTarget {
                    lane: "attack".into(),
                    id: target.id.clone(),
                    title: target.title.clone(),
                    why: target.why.clone(),
                    next_command: format!("vela work {}", shell_target_argument(&target.id)),
                    task: Some(pinned_target_index_task(project, &assessment, target)),
                });
            }
        }
        for seed in campaign_seeds(dir, &ns)? {
            let obligation = campaign_target_id(&seed, &ns);
            if indexed_ids.contains(&obligation) {
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
            let title = seed
                .title
                .clone()
                .unwrap_or_else(|| format!("{} seed {}", seed.batch, seed.handle));
            producer_work.configured_open += 1;
            if let Some(lease) = live_leases
                .get(obligation.as_str())
                .or_else(|| live_leases.get(seed.handle.as_str()))
            {
                producer_work.leased += 1;
                producer_work.leased_targets.push(leased_producer_target(
                    lease,
                    &obligation,
                    title,
                ));
                continue;
            }
            producer_work.available += 1;
            let task = seed
                .task
                .as_ref()
                .and_then(|raw| pinned_campaign_task(project, raw));
            actionable_targets.push(NextTarget {
                lane: "attack".into(),
                id: obligation.clone(),
                title,
                why: seed.why.unwrap_or_else(|| {
                    "open campaign seed: no live lease, no landed statement".into()
                }),
                next_command: format!("vela work {}", shell_target_argument(&obligation)),
                task,
            });
        }
    }

    // A brand-new frontier must still answer its first `next`. This offer is
    // coordination, not scientific content or authority: the producer must
    // state a bounded claim and land a Receipt before anything enters review.
    // Hide it while another producer holds the bootstrap lease.
    let bootstrap_lease = project.attempt_claims.iter().find(|lease| {
        lease.obligation_id == "seed:first"
            && lease_live_at(
                &lease.claimed_at,
                lease.lease_ttl_seconds,
                observed_at.as_ref(),
            )
    });
    if actionable_targets.is_empty()
        && producer_work.configured_open == 0
        && project.findings.is_empty()
        && project.proposals.is_empty()
    {
        let title = "Define the first bounded research result".to_string();
        producer_work.configured_open += 1;
        if let Some(lease) = bootstrap_lease {
            producer_work.leased += 1;
            producer_work
                .leased_targets
                .push(leased_producer_target(lease, "seed:first", title));
        } else {
            producer_work.available += 1;
            actionable_targets.push(NextTarget {
                lane: "seed".into(),
                id: "seed:first".into(),
                title,
                why: "new frontier: land one scoped Receipt with an artifact and explicit caveat"
                    .into(),
                next_command: "vela work seed:first".into(),
                task: None,
            });
        }
    }

    if producer_work.configured_open
        != producer_work.available + producer_work.leased + producer_work.stale
    {
        return Err(format!(
            "producer work availability is inconsistent: configured={}, available={}, leased={}, stale={}",
            producer_work.configured_open,
            producer_work.available,
            producer_work.leased,
            producer_work.stale
        ));
    }
    validate_producer_targets(&actionable_targets, producer_work.available)?;

    let targets = actionable_targets.into_iter().take(limit).collect();
    Ok(FrontierNextProjection {
        targets,
        producer_work,
    })
}

/// Compatibility wrapper for callers that only need the ranked available
/// targets. New product surfaces should use `try_frontier_next_projection` so
/// they can explain lease-withheld work.
pub fn try_frontier_next(
    project: &Project,
    frontier_dir: Option<&Path>,
    observed_at: &str,
    limit: usize,
) -> Result<Vec<NextTarget>, String> {
    Ok(try_frontier_next_projection(project, frontier_dir, observed_at, limit)?.targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use vela_protocol::bundle::{
        Assertion, Conditions, Confidence, Evidence, Extraction, FindingBundle, Flags, Provenance,
        ReviewState,
    };

    fn accepted_unverified_finding(id: &str) -> FindingBundle {
        let mut finding = FindingBundle::new(
            Assertion {
                text: "Accepted result awaiting more verifier evidence".into(),
                assertion_type: "computational".into(),
                entities: Vec::new(),
                relation: None,
                direction: None,
                causal_claim: None,
                causal_evidence_grade: None,
            },
            Evidence {
                evidence_type: "computational".into(),
                model_system: String::new(),
                method: "fixture".into(),
                replicated: false,
                replication_count: None,
                evidence_spans: Vec::new(),
            },
            Conditions {
                text: "fixture".into(),
                duration: None,
            },
            Confidence::raw(0.5, "fixture", 0.5),
            Provenance {
                source_type: "agent_run".into(),
                doi: None,
                url: None,
                title: "Producer offer separation fixture".into(),
                authors: Vec::new(),
                year: None,
                license: None,
                publisher: None,
                funders: Vec::new(),
                extraction: Extraction::default(),
                review: None,
                contributions: Vec::new(),
            },
            Flags {
                review_state: Some(ReviewState::Accepted),
                ..Flags::default()
            },
        );
        finding.id = id.to_string();
        finding
    }

    fn write_target_index(
        dir: &Path,
        project: &Project,
        packet_bytes: &[u8],
        packet_digest: Option<String>,
    ) {
        std::fs::create_dir_all(dir.join("packets")).unwrap();
        std::fs::write(dir.join("packets/443.json"), packet_bytes).unwrap();
        let digest = packet_digest
            .unwrap_or_else(|| format!("sha256:{}", hex::encode(Sha256::digest(packet_bytes))));
        std::fs::write(
            dir.join("targets.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "vela.target-index.v1",
                "frontier_id": project.frontier_id(),
                "as_of": {
                    "snapshot_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::snapshot_hash(project)
                    ),
                    "event_log_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::event_log_hash(&project.events)
                    ),
                    "proposal_state_hash": format!(
                        "sha256:{}",
                        vela_protocol::proposals::proposal_state_hash(&project.proposals)
                    ),
                },
                "claim_boundary": {
                    "derived": true,
                    "authoritative": false,
                },
                "targets": [{
                    "id": "erdos:443",
                    "title": "Erdős 443",
                    "why": "Open problem with a pinned packet",
                    "state": "open",
                    "rank": 7,
                    "objective": "Advance Erdős problem 443 from its pinned state.",
                    "labels": ["erdos", "open"],
                    "packet": {
                        "path": "packets/443.json",
                        "sha256": digest,
                        "schema": "fixture.problem.v1",
                    },
                }],
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn target_index_v1_is_historical_inspection_only_and_never_offered() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443,"statement":"fixture"}"#,
            None,
        );

        let projection =
            try_frontier_next_projection(&project, Some(temp.path()), "2026-07-16T12:00:00Z", 10)
                .unwrap();
        assert!(projection.targets.is_empty());
        assert_eq!(projection.producer_work.configured_open, 1);
        assert_eq!(projection.producer_work.stale, 1);
        assert_eq!(projection.producer_work.available, 0);
        assert!(
            target_index_selection_for_target(&project, temp.path(), "erdos:443")
                .unwrap_err()
                .contains("historical v1 inspection only")
        );
    }

    #[test]
    fn reproduced_stale_v1_index_is_counted_but_never_offered() {
        let temp = tempfile::tempdir().unwrap();
        let mut project = vela_protocol::project::assemble("stale-v1", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443}"#,
            None,
        );
        project.project.description = "changed after target-index generation".to_string();

        let assessment = super::super::target_index::assess_target_index(&project, temp.path())
            .unwrap()
            .unwrap();
        assert!(
            assessment
                .global_issues
                .iter()
                .any(|issue| issue.code == super::super::target_index::CODE_STATE_ROOT_MISMATCH)
        );
        let projection =
            try_frontier_next_projection(&project, Some(temp.path()), "2026-07-16T12:00:00Z", 10)
                .unwrap();
        assert_eq!(projection.producer_work.configured_open, 1);
        assert_eq!(projection.producer_work.stale, 1);
        assert_eq!(projection.producer_work.available, 0);
        assert!(projection.targets.is_empty());
    }

    #[test]
    fn producer_offers_exclude_structural_verification_advice() {
        let temp = tempfile::tempdir().unwrap();
        let mut project =
            vela_protocol::project::assemble("producer-only", Vec::new(), 0, 0, "fixture");
        project
            .findings
            .push(accepted_unverified_finding("vf_structural_advice"));
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443}"#,
            None,
        );

        let projection =
            try_frontier_next_projection(&project, Some(temp.path()), "2026-07-16T12:00:00Z", 10)
                .unwrap();
        assert_eq!(projection.producer_work.configured_open, 1);
        assert_eq!(projection.producer_work.available, 0);
        assert_eq!(projection.producer_work.stale, 1);
        assert!(projection.targets.is_empty());
    }

    #[test]
    fn producer_offer_output_rejects_duplicate_ids_and_count_drift() {
        let target = NextTarget {
            lane: "attack".into(),
            id: "fixture:one".into(),
            title: "Fixture".into(),
            why: "Fixture".into(),
            next_command: "vela work fixture:one".into(),
            task: None,
        };
        assert!(
            validate_producer_targets(&[target.clone(), target.clone()], 2)
                .unwrap_err()
                .contains("duplicate target id fixture:one")
        );
        assert!(
            validate_producer_targets(&[target], 2)
                .unwrap_err()
                .contains("2 available but 1 offers")
        );
    }

    #[test]
    fn live_lease_is_explained_without_becoming_an_available_offer() {
        let temp = tempfile::tempdir().unwrap();
        let mut project =
            vela_protocol::project::assemble("leased-target", Vec::new(), 0, 0, "fixture");
        std::fs::write(
            temp.path().join("campaign.yaml"),
            "batches:\n  - name: open\n    state: open\n    problems:\n      - id: erdos:443\n        title: Erdős 443\n",
        )
        .unwrap();
        project
            .attempt_claims
            .push(vela_protocol::project::AttemptClaim {
                obligation_id: "erdos:443".to_string(),
                claimant_actor: "agent:bounded-worker".to_string(),
                claimant_pubkey: "00".repeat(32),
                claimed_at: "2026-07-16T12:00:00Z".to_string(),
                lease_ttl_seconds: 3600,
                claim_event_id: Some("vev_lease_fixture".to_string()),
            });

        let projection =
            try_frontier_next_projection(&project, Some(temp.path()), "2026-07-16T12:30:00Z", 10)
                .unwrap();
        assert!(
            projection
                .targets
                .iter()
                .all(|target| target.id != "erdos:443")
        );
        assert_eq!(projection.producer_work.configured_open, 1);
        assert_eq!(projection.producer_work.available, 0);
        assert_eq!(projection.producer_work.leased, 1);
        let lease = &projection.producer_work.leased_targets[0];
        assert_eq!(lease.target_id, "erdos:443");
        assert_eq!(lease.actor, "agent:bounded-worker");
        assert_eq!(lease.claim_event_id.as_deref(), Some("vev_lease_fixture"));
        assert_eq!(lease.expires_at.as_deref(), Some("2026-07-16T13:00:00Z"));
    }

    #[test]
    fn target_index_v1_refuses_path_escape_before_historical_inspection() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443}"#,
            Some(format!("sha256:{}", "1".repeat(64))),
        );
        let mut index: Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("targets.json")).unwrap())
                .unwrap();
        index["targets"][0]["packet"]["path"] = json!("../outside.json");
        std::fs::write(
            temp.path().join("targets.json"),
            serde_json::to_vec_pretty(&index).unwrap(),
        )
        .unwrap();
        assert!(
            super::super::target_index::assess_target_index(&project, temp.path())
                .unwrap_err()
                .contains("normalized frontier-relative path")
        );
    }

    #[test]
    fn target_index_terminal_entries_are_addressable_but_not_ranked() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443}"#,
            None,
        );
        let mut index: Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("targets.json")).unwrap())
                .unwrap();
        index["targets"][0]["state"] = json!("done");
        std::fs::write(
            temp.path().join("targets.json"),
            serde_json::to_vec_pretty(&index).unwrap(),
        )
        .unwrap();

        let targets =
            try_frontier_next(&project, Some(temp.path()), "2026-07-16T12:00:00Z", 10).unwrap();
        assert!(targets.iter().all(|target| target.id != "erdos:443"));
        assert!(
            target_index_selection_for_target(&project, temp.path(), "erdos:443")
                .unwrap_err()
                .contains("historical v1 inspection only")
        );
    }

    #[cfg(unix)]
    #[test]
    fn historical_v1_inspection_rejects_a_symlinked_packet_parent() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        let packet = br#"{"schema":"fixture.problem.v1","problem":443}"#;
        std::fs::write(outside.path().join("443.json"), packet).unwrap();
        symlink(outside.path(), temp.path().join("packets")).unwrap();
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(packet)));
        std::fs::write(
            temp.path().join("targets.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "vela.target-index.v1",
                "frontier_id": project.frontier_id(),
                "as_of": {
                    "snapshot_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::snapshot_hash(&project)
                    ),
                    "event_log_hash": format!(
                        "sha256:{}",
                        vela_protocol::events::event_log_hash(&project.events)
                    ),
                    "proposal_state_hash": format!("sha256:{}", "0".repeat(64)),
                },
                "targets": [{
                    "id": "erdos:443",
                    "title": "Erdős 443",
                    "why": "fixture",
                    "state": "open",
                    "rank": 0,
                    "objective": "fixture",
                    "packet": {
                        "path": "packets/443.json",
                        "sha256": digest,
                        "schema": "fixture.problem.v1",
                    },
                }],
            }))
            .unwrap(),
        )
        .unwrap();
        let inspection = super::super::target_index::inspect_target_index_target(
            &project,
            temp.path(),
            "erdos:443",
        )
        .unwrap()
        .unwrap();
        assert!(inspection.historical_only);
        assert!(!inspection.actionable);
        assert!(inspection.packet.is_none());
        assert!(
            inspection
                .codes
                .contains(&super::super::target_index::CODE_PACKET_MISMATCH)
        );
    }

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
    fn empty_frontier_offers_one_non_authorizing_bootstrap_target() {
        let project = vela_protocol::project::assemble("empty frontier", Vec::new(), 0, 0, "empty");
        let targets = try_frontier_next(&project, None, "2026-07-15T12:00:00Z", 10).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].lane, "seed");
        assert_eq!(targets[0].id, "seed:first");
        assert_eq!(targets[0].next_command, "vela work seed:first");
        assert!(targets[0].task.is_none());
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
            "quantum:[10,1,4",
            "quantum:10,1,4",
            "quantum:[[[10,1,4]]]",
            "quantum:[[10,1,4]]'",
        ] {
            assert!(
                validate_external_target_id(invalid).is_err(),
                "accepted unsafe external target {invalid:?}"
            );
        }
        assert!(validate_external_target_id("seed:prepared-target").is_ok());
        assert!(validate_external_target_id("quantum:[[10,1,4]]").is_ok());
        assert_eq!(
            shell_target_argument("quantum:[[10,1,4]]"),
            "'quantum:[[10,1,4]]'"
        );

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
