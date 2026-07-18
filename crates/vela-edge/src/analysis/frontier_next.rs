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
//! - **attack** — open entries from a derived, hash-pinned `targets.json`
//!   catalogue and open campaign seeds (`campaign.yaml`, when present).
//!   Neither projection is authority; both only prepare a work target.
//! - **verify** — accepted findings the gate still holds at
//!   `needs_verification`: the honest accepted-but-unverified gap,
//!   closest-to-the-bar first.
//!
//! A ranking is advice, never authority: nothing here mutates state,
//! and claiming a target still goes through the lease tool.

use std::io::Read;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::project::Project;
use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};

use super::decision_brief::ReviewSnapshot;

#[derive(Debug, Clone, Serialize)]
pub struct NextTarget {
    /// "seed" | "review" | "attack" | "verify"
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
const TARGET_INDEX_JSON_MAX_BYTES: u64 = 4 * 1024 * 1024;
const TARGET_PACKET_MAX_BYTES: u64 = 1024 * 1024;
const TARGET_INDEX_MAX_TARGETS: usize = 16_384;
const TARGET_INDEX_MAX_LABELS: usize = 64;
pub const EXTERNAL_TARGET_ID_MAX_BYTES: usize = 256;

#[derive(Debug, Clone, Deserialize)]
struct TargetIndex {
    schema: String,
    frontier_id: String,
    as_of: TargetIndexAsOf,
    targets: Vec<TargetIndexEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TargetIndexAsOf {
    snapshot_hash: String,
    event_log_hash: String,
    proposal_state_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TargetIndexEntry {
    id: String,
    title: String,
    why: String,
    state: String,
    rank: u64,
    objective: String,
    #[serde(default)]
    labels: Vec<String>,
    packet: TargetPacketRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TargetPacketRef {
    path: String,
    sha256: String,
    schema: String,
}

#[derive(Debug, Clone)]
struct LoadedTargetIndex {
    sha256: String,
    stale_against_loaded_frontier: bool,
    loaded_event_log_root: String,
    index: TargetIndex,
}

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

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn bounded_text(value: &str, field: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(format!(
            "target index {field} must be non-empty, at most {max} bytes, and free of control characters"
        ));
    }
    Ok(())
}

fn read_regular_file(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{label} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "{label} {} exceeds the {max_bytes}-byte limit",
            path.display()
        ));
    }
    let initial_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("identify {label} {}: {error}", path.display()))?;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("open {label} {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("inspect open {label} {}: {error}", path.display()))?;
    if !opened.is_file() || opened.len() > max_bytes {
        return Err(format!(
            "{label} must remain a regular file within the {max_bytes}-byte limit: {}",
            path.display()
        ));
    }
    let opened_identity = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone open {label} {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("identify open {label} {}: {error}", path.display()))?;
    if initial_identity != opened_identity {
        return Err(format!(
            "{label} changed while it was being opened: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label} {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "{label} {} exceeds the {max_bytes}-byte limit",
            path.display()
        ));
    }
    let named = std::fs::symlink_metadata(path)
        .map_err(|error| format!("reinspect {label} {}: {error}", path.display()))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("reidentify {label} {}: {error}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || opened_identity != final_identity {
        return Err(format!(
            "{label} changed while it was being read: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn validate_target_packet_relative_path(relative: &str) -> Result<&Path, String> {
    bounded_text(relative, "packet.path", 1024)?;
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "target packet path must be a normalized frontier-relative path: {relative:?}"
        ));
    }
    Ok(relative_path)
}

fn target_packet_path(dir: &Path, relative: &str) -> Result<std::path::PathBuf, String> {
    let relative_path = validate_target_packet_relative_path(relative)?;
    let mut cursor = dir.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(component) = component else {
            unreachable!("relative path was validated above");
        };
        cursor.push(component);
        let metadata = std::fs::symlink_metadata(&cursor)
            .map_err(|error| format!("inspect target packet path {}: {error}", cursor.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "target packet path must not contain symlinks: {}",
                cursor.display()
            ));
        }
    }
    let root = std::fs::canonicalize(dir)
        .map_err(|error| format!("resolve frontier directory {}: {error}", dir.display()))?;
    let candidate = dir.join(relative_path);
    let resolved = std::fs::canonicalize(&candidate)
        .map_err(|error| format!("resolve target packet {}: {error}", candidate.display()))?;
    if !resolved.starts_with(&root) {
        return Err(format!(
            "target packet escapes the frontier: {}",
            candidate.display()
        ));
    }
    Ok(candidate)
}

fn load_target_index(project: &Project, dir: &Path) -> Result<Option<LoadedTargetIndex>, String> {
    let path = dir.join("targets.json");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_regular_file(&path, TARGET_INDEX_JSON_MAX_BYTES, "target index")?;
    let index: TargetIndex = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse target index {}: {error}", path.display()))?;
    if index.schema != "vela.target-index.v1" {
        return Err(format!(
            "target index has unsupported schema {:?}",
            index.schema
        ));
    }
    if index.frontier_id != project.frontier_id() {
        return Err(format!(
            "target index frontier {:?} differs from loaded frontier {:?}",
            index.frontier_id,
            project.frontier_id()
        ));
    }
    for (field, digest) in [
        ("as_of.snapshot_hash", index.as_of.snapshot_hash.as_str()),
        ("as_of.event_log_hash", index.as_of.event_log_hash.as_str()),
        (
            "as_of.proposal_state_hash",
            index.as_of.proposal_state_hash.as_str(),
        ),
    ] {
        if !valid_sha256(digest) {
            return Err(format!("target index {field} must be a sha256: digest"));
        }
    }
    if index.targets.len() > TARGET_INDEX_MAX_TARGETS {
        return Err(format!(
            "target index has {} targets; limit is {TARGET_INDEX_MAX_TARGETS}",
            index.targets.len()
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for target in &index.targets {
        validate_external_target_id(&target.id)
            .map_err(|error| format!("invalid target index id {:?}: {error}", target.id))?;
        bounded_text(&target.title, "target.title", 512)?;
        bounded_text(&target.why, "target.why", 2048)?;
        bounded_text(&target.objective, "target.objective", 4096)?;
        if !matches!(
            target.state.as_str(),
            "open" | "paused" | "blocked" | "done" | "retired"
        ) {
            return Err(format!(
                "target index state for {:?} is unsupported: {:?}",
                target.id, target.state
            ));
        }
        if target.labels.len() > TARGET_INDEX_MAX_LABELS {
            return Err(format!(
                "target index target {:?} has more than {TARGET_INDEX_MAX_LABELS} labels",
                target.id
            ));
        }
        for label in &target.labels {
            bounded_text(label, "target.labels[]", 128)?;
        }
        bounded_text(&target.packet.schema, "packet.schema", 256)?;
        if !valid_sha256(&target.packet.sha256) {
            return Err(format!(
                "target index packet digest for {:?} must be a sha256: digest",
                target.id
            ));
        }
        validate_target_packet_relative_path(&target.packet.path)?;
        if !ids.insert(target.id.clone()) {
            return Err(format!("duplicate target index id {:?}", target.id));
        }
    }
    let loaded_event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    let stale_against_loaded_frontier =
        target_index_is_stale(project, &index, &loaded_event_log_root);
    Ok(Some(LoadedTargetIndex {
        sha256: format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        stale_against_loaded_frontier,
        loaded_event_log_root,
        index,
    }))
}

fn target_index_is_stale(
    project: &Project,
    index: &TargetIndex,
    loaded_event_log_root: &str,
) -> bool {
    index.as_of.snapshot_hash != format!("sha256:{}", vela_protocol::events::snapshot_hash(project))
        || index.as_of.event_log_hash != loaded_event_log_root
}

fn pinned_target_index_task(
    project: &Project,
    loaded: &LoadedTargetIndex,
    target: &TargetIndexEntry,
) -> Value {
    json!({
        "kind": "target_packet",
        "objective": target.objective,
        "state": target.state,
        "rank": target.rank,
        "labels": target.labels,
        "packet_ref": target.packet,
        "index": {
            "path": "targets.json",
            "schema": loaded.index.schema,
            "sha256": loaded.sha256,
            "as_of": loaded.index.as_of,
            "stale_against_loaded_frontier": loaded.stale_against_loaded_frontier,
        },
        "fixed_base": {
            "frontier_id": project.frontier_id(),
            "event_log_root": loaded.loaded_event_log_root,
        },
        "authority_ceiling": PRODUCER_AUTHORITY_CEILING,
    })
}

/// Return the non-authorizing target-index task metadata for one external
/// target. The index is a deletable projection; accepted state remains the
/// event log and the selected packet is hash-checked separately.
pub fn target_index_task_for_target(
    project: &Project,
    dir: &Path,
    target: &str,
) -> Result<Option<Value>, String> {
    let Some(loaded) = load_target_index(project, dir)? else {
        return Ok(None);
    };
    Ok(loaded
        .index
        .targets
        .iter()
        .find(|entry| entry.id == target)
        .map(|entry| pinned_target_index_task(project, &loaded, entry)))
}

/// Load and hash-check the selected target packet. This is producer briefing
/// material only: the wrapper names both its derived index root and the live
/// frontier root, and never converts packet content into accepted state.
pub fn target_index_packet_for_target(
    project: &Project,
    dir: &Path,
    target: &str,
) -> Result<Option<Value>, String> {
    let Some(loaded) = load_target_index(project, dir)? else {
        return Ok(None);
    };
    let Some(entry) = loaded.index.targets.iter().find(|entry| entry.id == target) else {
        return Ok(None);
    };
    let path = target_packet_path(dir, &entry.packet.path)?;
    let bytes = read_regular_file(&path, TARGET_PACKET_MAX_BYTES, "target packet")?;
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if digest != entry.packet.sha256 {
        return Err(format!(
            "target packet digest mismatch for {:?}: index {} != bytes {}",
            target, entry.packet.sha256, digest
        ));
    }
    let packet: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse target packet {}: {error}", path.display()))?;
    if !packet.is_object() {
        return Err(format!(
            "target packet {} must be a JSON object",
            path.display()
        ));
    }
    if packet.get("schema").and_then(Value::as_str) != Some(entry.packet.schema.as_str()) {
        return Err(format!(
            "target packet schema mismatch for {:?}: index {:?} != packet {:?}",
            target,
            entry.packet.schema,
            packet.get("schema")
        ));
    }
    Ok(Some(json!({
        "kind": "target_packet",
        "target": target,
        "objective": entry.objective,
        "packet": packet,
        "packet_ref": entry.packet,
        "index": {
            "path": "targets.json",
            "schema": loaded.index.schema,
            "sha256": loaded.sha256,
            "as_of": loaded.index.as_of,
            "stale_against_loaded_frontier": loaded.stale_against_loaded_frontier,
        },
        "authority_ceiling": PRODUCER_AUTHORITY_CEILING,
        "caveat": "The target index and packet are derived briefing projections. Their bytes are pinned here, but only signed frontier events carry accepted truth.",
    })))
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

fn scientific_target_punctuation_is_balanced(target: &str) -> bool {
    let mut bracket_depth = 0_u8;
    for byte in target.bytes() {
        match byte {
            b'[' => {
                bracket_depth = match bracket_depth.checked_add(1) {
                    Some(depth) if depth <= 2 => depth,
                    _ => return false,
                };
            }
            b']' if bracket_depth > 0 => bracket_depth -= 1,
            b']' => return false,
            b',' if bracket_depth > 0 => {}
            b',' => return false,
            _ => {}
        }
    }
    bracket_depth == 0
}

fn shell_target_argument(target: &str) -> String {
    if target.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-')
    }) {
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
            .all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b':' | b'.' | b'_' | b'-' | b'[' | b']' | b',')
            })
        || !scientific_target_punctuation_is_balanced(target)
    {
        return Err(
            "external target id must have non-empty, non-option-like ':'-separated segments using ASCII letters, digits, '.', '_', '-', or balanced square-bracket notation"
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

    // ── attack: open target-index entries and campaign seeds ───────────
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
        let mut indexed_ids = std::collections::BTreeSet::new();
        if let Some(loaded) = load_target_index(project, dir)? {
            let mut indexed = loaded
                .index
                .targets
                .iter()
                .filter(|target| target.state == "open")
                .collect::<Vec<_>>();
            indexed.sort_by(|left, right| left.rank.cmp(&right.rank).then(left.id.cmp(&right.id)));
            for target in indexed {
                indexed_ids.insert(target.id.clone());
                if live_leases.contains(&target.id) {
                    continue;
                }
                actionable_targets.push(NextTarget {
                    lane: "attack".into(),
                    id: target.id.clone(),
                    title: target.title.clone(),
                    why: target.why.clone(),
                    next_command: format!("vela work {}", shell_target_argument(&target.id)),
                    task: Some(pinned_target_index_task(project, &loaded, target)),
                });
            }
        }
        for seed in campaign_seeds(dir, &ns)? {
            let obligation = campaign_target_id(&seed, &ns);
            if indexed_ids.contains(&obligation) {
                continue;
            }
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
                next_command: format!("vela work {}", shell_target_argument(&obligation)),
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

    // A brand-new frontier must still answer its first `next`. This offer is
    // coordination, not scientific content or authority: the producer must
    // state a bounded claim and land a Receipt before anything enters review.
    // Hide it while another producer holds the bootstrap lease.
    let bootstrap_lease_live = project.attempt_claims.iter().any(|lease| {
        lease.obligation_id == "seed:first"
            && lease_live_at(
                &lease.claimed_at,
                lease.lease_ttl_seconds,
                observed_at.as_ref(),
            )
    });
    if review_targets.is_empty()
        && actionable_targets.is_empty()
        && project.findings.is_empty()
        && project.proposals.is_empty()
        && !bootstrap_lease_live
    {
        actionable_targets.push(NextTarget {
            lane: "seed".into(),
            id: "seed:first".into(),
            title: "Define the first bounded research result".into(),
            why: "new frontier: land one scoped Receipt with an artifact and explicit caveat"
                .into(),
            next_command: "vela work seed:first".into(),
            task: None,
        });
    }

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

#[cfg(test)]
mod tests {
    use super::*;

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
                    "proposal_state_hash": format!("sha256:{}", "0".repeat(64)),
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
    fn target_index_offers_and_loads_one_hash_pinned_native_target() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443,"statement":"fixture"}"#,
            None,
        );

        let targets =
            try_frontier_next(&project, &[], Some(temp.path()), "2026-07-16T12:00:00Z", 10)
                .unwrap();
        assert_eq!(targets[0].id, "erdos:443");
        assert_eq!(targets[0].lane, "attack");
        assert_eq!(targets[0].task.as_ref().unwrap()["kind"], "target_packet");
        assert_eq!(
            targets[0].task.as_ref().unwrap()["authority_ceiling"],
            PRODUCER_AUTHORITY_CEILING
        );

        let loaded = target_index_packet_for_target(&project, temp.path(), "erdos:443")
            .unwrap()
            .unwrap();
        assert_eq!(loaded["packet"]["problem"], 443);
        assert_eq!(loaded["packet_ref"]["path"], "packets/443.json");
        assert_eq!(loaded["index"]["stale_against_loaded_frontier"], false);
    }

    #[test]
    fn target_index_refuses_packet_digest_drift_and_path_escape() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("target-index", Vec::new(), 0, 0, "fixture");
        write_target_index(
            temp.path(),
            &project,
            br#"{"schema":"fixture.problem.v1","problem":443}"#,
            Some(format!("sha256:{}", "1".repeat(64))),
        );
        assert!(
            target_index_packet_for_target(&project, temp.path(), "erdos:443")
                .unwrap_err()
                .contains("digest mismatch")
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
            load_target_index(&project, temp.path())
                .unwrap_err()
                .contains("normalized frontier-relative")
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
            try_frontier_next(&project, &[], Some(temp.path()), "2026-07-16T12:00:00Z", 10)
                .unwrap();
        assert!(targets.iter().all(|target| target.id != "erdos:443"));
        assert!(
            target_index_packet_for_target(&project, temp.path(), "erdos:443")
                .unwrap()
                .is_some()
        );
    }

    #[cfg(unix)]
    #[test]
    fn target_index_packet_rejects_a_symlinked_parent_directory() {
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
        assert!(
            target_index_packet_for_target(&project, temp.path(), "erdos:443")
                .unwrap_err()
                .contains("must not contain symlinks")
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
        let targets = try_frontier_next(&project, &[], None, "2026-07-15T12:00:00Z", 10).unwrap();
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
