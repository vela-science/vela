//! MCP adapter for the vela_agent SDK surface.
//!
//! The underlying impls behind the `work`, `verify`, and `objects` MCP
//! tools: signed one-shot submissions (attestation + diff pack), obligation
//! leases, record proposals, the strict/witness verification runs, and the
//! `.vela/` object reads.
//!
//! Stateless: each call is one-shot. The agent does its own
//! bookkeeping client-side (mirroring the Python SDK's run lifecycle)
//! and only invokes Vela when ready to submit a complete unit.
//!
//! Signing key: the agent's own session identity, minted automatically
//! at `~/.vela/agents/<actor>/private.key` on first use (the actor comes
//! from the tool argument or `VELA_ACTOR_ID`); `VELA_AGENT_KEY_HEX`
//! overrides when an explicit key is wanted. Minting is refused for
//! non-agent actors — a human identity is a deliberate `vela id create`.
//! **No silent unsigned submissions, and no key ceremony either.**

use crate::agent_attestation::{AgentAttestation, AttestationDraft, ToolCall};
use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use vela_protocol::scientific_diff::{PackDraft, ScientificDiffPack};

const AGENT_KEY_ENV: &str = "VELA_AGENT_KEY_HEX";

// ----------------------------------------------------------------------------
// v0.214: read-side helpers. None of these require a signing key.
// ----------------------------------------------------------------------------

fn read_one_artifact(frontier_path: &Path, subdir: &str, id: &str) -> Result<String, String> {
    let path = if frontier_path.is_dir() {
        frontier_path
            .join(".vela")
            .join(subdir)
            .join(format!("{id}.json"))
    } else {
        frontier_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(".vela")
            .join(subdir)
            .join(format!("{id}.json"))
    };
    std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}

fn list_artifacts(frontier_path: &Path, subdir: &str, prefix: &str) -> Vec<Value> {
    let dir = if frontier_path.is_dir() {
        frontier_path.join(".vela").join(subdir)
    } else {
        frontier_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(".vela")
            .join(subdir)
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .filter(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.starts_with(prefix))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    let mut out: Vec<Value> = Vec::new();
    for p in paths {
        if let Ok(body) = std::fs::read_to_string(&p)
            && let Ok(v) = serde_json::from_str::<Value>(&body)
        {
            out.push(v);
        }
    }
    out
}

fn frontier_path_arg(args: &Value) -> Result<PathBuf, String> {
    args.get("frontier_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "frontier_path required".to_string())
}

/// `objects` type=pack with id — fetch a single Diff Pack by id.
pub fn get_pack(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let pack_id = args
        .get("pack_id")
        .and_then(Value::as_str)
        .ok_or("pack_id required")?;
    if !pack_id.starts_with("vsd_") {
        return Err(format!("pack_id must start with `vsd_`, got `{pack_id}`"));
    }
    let body = read_one_artifact(&frontier, "diff_packs", pack_id)?;
    Ok(body)
}

/// `objects` type=pack listing — every Diff Pack on a frontier.
pub fn list_packs(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let packs = list_artifacts(&frontier, "diff_packs", "vsd_");
    let only_pending = args
        .get("only_pending")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let filtered: Vec<Value> = if only_pending {
        packs
            .into_iter()
            .filter(|p| p.get("signature").is_some() && p.get("applied_event_id").is_none())
            .collect()
    } else {
        packs
    };
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "count": filtered.len(),
        "packs": filtered,
    }))
    .unwrap_or_default())
}

/// List every Agent Attestation on a frontier (the `objects` tool's
/// attestation listing; there was no narrow-tool equivalent).
pub fn list_attestations(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let attestations = list_artifacts(&frontier, "agent_attestations", "vaa_");
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "count": attestations.len(),
        "attestations": attestations,
    }))
    .unwrap_or_default())
}

/// List every Tool Descriptor on a frontier (the `objects` tool's
/// descriptor listing; there was no narrow-tool equivalent).
pub fn list_tool_descriptors(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let descriptors = list_artifacts(&frontier, "tool_descriptors", "vtd_");
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "count": descriptors.len(),
        "descriptors": descriptors,
    }))
    .unwrap_or_default())
}

/// `objects` type=attestation with id — fetch a single Agent Attestation.
pub fn get_attestation(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let vaa_id = args
        .get("attestation_id")
        .and_then(Value::as_str)
        .ok_or("attestation_id required")?;
    if !vaa_id.starts_with("vaa_") {
        return Err(format!(
            "attestation_id must start with `vaa_`, got `{vaa_id}`"
        ));
    }
    let body = read_one_artifact(&frontier, "agent_attestations", vaa_id)?;
    Ok(body)
}

/// Quick counts of which agent-object primitives exist on a frontier;
/// folded into `orient` as the agent_objects lane.
pub fn frontier_summary(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let diff_packs = list_artifacts(&frontier, "diff_packs", "vsd_");
    let pending_packs: usize = diff_packs
        .iter()
        .filter(|p| p.get("signature").is_some() && p.get("applied_event_id").is_none())
        .count();
    let attestations = list_artifacts(&frontier, "agent_attestations", "vaa_");
    let tool_descriptors = list_artifacts(&frontier, "tool_descriptors", "vtd_");
    let evaluations = list_artifacts(&frontier, "evaluations", "ver_");
    let verdict_conflicts = list_artifacts(&frontier, "verdict_conflicts", "vdc_");
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "summary": {
            "diff_packs": diff_packs.len(),
            "pending_packs": pending_packs,
            "attestations": attestations.len(),
            "tool_descriptors": tool_descriptors.len(),
            "evaluations": evaluations.len(),
            "verdict_conflicts": verdict_conflicts.len(),
        }
    }))
    .unwrap_or_default())
}

// ----------------------------------------------------------------------------
// v0.220: read-tool parity for tool descriptors, evaluations, conflicts.
// ----------------------------------------------------------------------------

/// `objects` type=tool_descriptor with id — fetch a single Tool Descriptor.
pub fn get_tool_descriptor(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let vtd_id = args
        .get("descriptor_id")
        .and_then(Value::as_str)
        .ok_or("descriptor_id required")?;
    if !vtd_id.starts_with("vtd_") {
        return Err(format!(
            "descriptor_id must start with `vtd_`, got `{vtd_id}`"
        ));
    }
    let body = read_one_artifact(&frontier, "tool_descriptors", vtd_id)?;
    Ok(body)
}

/// `objects` type=evaluation with id — fetch a single Evaluation Record.
pub fn get_evaluation(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let ver_id = args
        .get("evaluation_id")
        .and_then(Value::as_str)
        .ok_or("evaluation_id required")?;
    if !ver_id.starts_with("ver_") {
        return Err(format!(
            "evaluation_id must start with `ver_`, got `{ver_id}`"
        ));
    }
    let body = read_one_artifact(&frontier, "evaluations", ver_id)?;
    Ok(body)
}

/// `objects` type=evaluation listing — every Evaluation Record on a
/// frontier, optionally filtered by target descriptor.
pub fn list_evaluations(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let evals = list_artifacts(&frontier, "evaluations", "ver_");
    let target_descriptor = args
        .get("target_descriptor_id")
        .and_then(Value::as_str)
        .map(String::from);
    let filtered: Vec<Value> = match target_descriptor {
        Some(td) => evals
            .into_iter()
            .filter(|e| {
                e.get("target_kind").and_then(Value::as_str) == Some("tool_descriptor")
                    && e.get("target_id").and_then(Value::as_str) == Some(td.as_str())
            })
            .collect(),
        None => evals,
    };
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "count": filtered.len(),
        "evaluations": filtered,
    }))
    .unwrap_or_default())
}

/// `objects` type=conflict with id — fetch a single resolved Verdict Conflict.
pub fn get_conflict(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let vdc_id = args
        .get("conflict_id")
        .and_then(Value::as_str)
        .ok_or("conflict_id required")?;
    if !vdc_id.starts_with("vdc_") {
        return Err(format!(
            "conflict_id must start with `vdc_`, got `{vdc_id}`"
        ));
    }
    let body = read_one_artifact(&frontier, "verdict_conflicts", vdc_id)?;
    Ok(body)
}

/// `objects` type=conflict listing — every resolved Verdict Conflict
/// on a frontier, optionally filtered by resolution_mode.
pub fn list_conflicts(args: &Value) -> Result<String, String> {
    let frontier = frontier_path_arg(args)?;
    let conflicts = list_artifacts(&frontier, "verdict_conflicts", "vdc_");
    let mode = args
        .get("resolution_mode")
        .and_then(Value::as_str)
        .map(String::from);
    let filtered: Vec<Value> = match mode {
        Some(m) => conflicts
            .into_iter()
            .filter(|c| c.get("resolution_mode").and_then(Value::as_str) == Some(m.as_str()))
            .collect(),
        None => conflicts,
    };
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "count": filtered.len(),
        "conflicts": filtered,
    }))
    .unwrap_or_default())
}

/// Resolve the agent's signing key with zero ceremony. Order:
///
/// 1. `VELA_AGENT_KEY_HEX` — an explicit key always wins.
/// 2. The per-actor session key at `~/.vela/agents/<actor>/private.key`,
///    MINTED on first use. An agent session that exported
///    `VELA_ACTOR_ID=agent:<name>` (the charter's first rule) needs no
///    key step at all — identity is a consequence of showing up.
///
/// Custody: minting is refused for anything but `agent:`/`ci:` actors —
/// a human identity is a deliberate `vela id create`, never a side
/// effect. The minted key signs only agent-grade objects (leases,
/// records); every decision verb still refuses agent actors outright.
pub fn agent_signing_key(explicit_actor: Option<&str>) -> Result<SigningKey, String> {
    if let Ok(hex_str) = std::env::var(AGENT_KEY_ENV) {
        let bytes =
            hex::decode(hex_str.trim()).map_err(|e| format!("decode {AGENT_KEY_ENV}: {e}"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| format!("{AGENT_KEY_ENV} must be 32 hex bytes"))?;
        return Ok(SigningKey::from_bytes(&arr));
    }
    let actor = explicit_actor
        .map(str::to_string)
        .or_else(|| std::env::var("VELA_ACTOR_ID").ok())
        .ok_or_else(|| {
            format!(
                "no agent identity: set VELA_ACTOR_ID=agent:<name> (or {AGENT_KEY_ENV} for an explicit key)"
            )
        })?;
    if !actor.starts_with("agent:") && !actor.starts_with("ci:") {
        return Err(format!(
            "agent key auto-mint is for agent:/ci: actors, not '{actor}' — humans run `vela id create`"
        ));
    }
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let base = std::path::PathBuf::from(home).join(".vela/agents");
    mint_or_load_agent_key(&base, &actor)
}

/// The mint itself, factored for tests: `<base>/<sanitized-actor>/private.key`
/// (hex seed, 0600), created once and reused for the actor's lifetime on
/// this machine.
fn mint_or_load_agent_key(base: &Path, actor: &str) -> Result<SigningKey, String> {
    let safe: String = actor
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let dir = base.join(safe);
    let key_path = dir.join("private.key");
    if key_path.exists() {
        let hex_str = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("read {}: {e}", key_path.display()))?;
        let bytes = hex::decode(hex_str.trim())
            .map_err(|e| format!("decode {}: {e}", key_path.display()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| format!("{} must be 32 hex bytes", key_path.display()))?;
        return Ok(SigningKey::from_bytes(&arr));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let mut seed = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let key = SigningKey::from_bytes(&seed);
    std::fs::write(&key_path, hex::encode(seed))
        .map_err(|e| format!("write {}: {e}", key_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn canonical_json_hash(v: &Value) -> String {
    match vela_protocol::canonical::to_canonical_bytes(v) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(_) => sha256_hex(b""),
    }
}

fn frontier_id_from_path(path: &Path) -> Result<String, String> {
    let frontier_json = if path.is_dir() {
        path.join("frontier.json")
    } else {
        path.to_path_buf()
    };
    let body = std::fs::read_to_string(&frontier_json)
        .map_err(|e| format!("read {}: {e}", frontier_json.display()))?;
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("parse frontier.json: {e}"))?;
    let fid = v
        .get("frontier_id")
        .and_then(Value::as_str)
        .or_else(|| {
            v.get("frontier")
                .and_then(|p| p.get("id"))
                .and_then(Value::as_str)
        })
        .ok_or("no frontier_id in frontier.json")?;
    Ok(fid.to_string())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn derive_proposal_id(kind: &str, payload: &Value, at: &str, actor: &str) -> String {
    let canonical = serde_json::to_string(payload).unwrap_or_default();
    let preimage = format!("{kind}|{canonical}|{at}|{actor}");
    format!("vpr_{}", &sha256_hex(preimage.as_bytes())[..16])
}

/// `work` action=pack. One-shot: builds a
/// signed AgentAttestation envelope and a signed ScientificDiffPack
/// bundling N proposals, writes both to the frontier's `.vela/`
/// tree, and returns the resulting ids.
///
/// Arguments (JSON):
///   {
///     "frontier_path": String,
///     "agent_actor": String (must start with "agent:"),
///     "model_name": String, "model_version": String,
///     "prompt": String? (hashed server-side),
///     "started_at": String, "finished_at": String,
///     "total_tokens": Number,
///     "tool_calls": [{
///       "tool_name": String, "input": Value, "output": Value,
///       "duration_ms": Number
///     }],
///     "proposals": [{"kind": String, "payload": Value}],
///     "summary": String, "aggregate_kind": String,
///     "parent_attestation": String?, "parent_pack": String?,
///   }
pub fn submit_diff_pack(args: &Value) -> Result<String, String> {
    let key = agent_signing_key(None)?;

    let frontier_path: PathBuf = args
        .get("frontier_path")
        .and_then(Value::as_str)
        .ok_or("frontier_path required")?
        .into();
    let frontier_id = frontier_id_from_path(&frontier_path)?;

    let agent_actor = args
        .get("agent_actor")
        .and_then(Value::as_str)
        .ok_or("agent_actor required")?;
    let model_name = args
        .get("model_name")
        .and_then(Value::as_str)
        .ok_or("model_name required")?;
    let model_version = args
        .get("model_version")
        .and_then(Value::as_str)
        .ok_or("model_version required")?;
    let started_at = args
        .get("started_at")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(now_rfc3339);
    let finished_at = args
        .get("finished_at")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(now_rfc3339);
    let total_tokens = args
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prompt_hash = args
        .get("prompt")
        .and_then(Value::as_str)
        .map(|p| sha256_hex(p.as_bytes()));
    let parent_attestation = args
        .get("parent_attestation")
        .and_then(Value::as_str)
        .map(String::from);
    let parent_pack = args
        .get("parent_pack")
        .and_then(Value::as_str)
        .map(String::from);
    let summary = args
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("summary required")?
        .to_string();
    let aggregate_kind = args
        .get("aggregate_kind")
        .and_then(Value::as_str)
        .ok_or("aggregate_kind required")?
        .to_string();

    // Tool calls.
    let tool_calls_json = args.get("tool_calls").and_then(Value::as_array);
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    if let Some(calls) = tool_calls_json {
        for c in calls {
            let tool_name = c
                .get("tool_name")
                .and_then(Value::as_str)
                .ok_or("tool_call.tool_name required")?
                .to_string();
            let input = c.get("input").cloned().unwrap_or(Value::Null);
            let output = c.get("output").cloned().unwrap_or(Value::Null);
            let duration_ms = c.get("duration_ms").and_then(Value::as_u64).unwrap_or(0);
            tool_calls.push(ToolCall {
                tool_name,
                input_hash: canonical_json_hash(&input),
                output_hash: canonical_json_hash(&output),
                duration_ms,
            });
        }
    }

    // Proposals.
    let proposals_json = args
        .get("proposals")
        .and_then(Value::as_array)
        .ok_or("proposals required")?;
    if proposals_json.is_empty() {
        return Err("proposals must contain at least one entry".to_string());
    }

    let mut output_hashes: Vec<String> = Vec::new();
    let mut proposal_ids: Vec<String> = Vec::new();
    let mut stub_writes: Vec<(String, Value)> = Vec::new();
    for p in proposals_json {
        let kind = p
            .get("kind")
            .and_then(Value::as_str)
            .ok_or("proposal.kind required")?
            .to_string();
        let payload = p.get("payload").cloned().unwrap_or(Value::Null);
        let proposed_at = now_rfc3339();
        let pid = derive_proposal_id(&kind, &payload, &proposed_at, agent_actor);
        let stub = json!({
            "schema": "vela.agent_sdk.proposal_stub.v0.1",
            "proposal_id": pid,
            "kind": kind,
            "payload": payload,
            "proposed_at": proposed_at,
            "actor": agent_actor,
            "meta": {},
        });
        output_hashes.push(canonical_json_hash(&payload));
        proposal_ids.push(pid.clone());
        stub_writes.push((pid, stub));
    }

    // Build the attestation.
    let attestation = AgentAttestation::build(
        AttestationDraft {
            agent_actor: agent_actor.to_string(),
            model_name: model_name.to_string(),
            model_version: model_version.to_string(),
            started_at,
            finished_at: finished_at.clone(),
            total_tokens,
            tool_calls,
            output_hashes,
            prompt_hash,
            parent_attestation,
        },
        &key,
    )?;

    // Build + sign the pack.
    let pack_draft = PackDraft {
        frontier_id: frontier_id.clone(),
        created_at: finished_at,
        summary,
        proposals: proposal_ids.clone(),
        aggregate_kind,
        agent_run: Some(attestation.attestation_id.clone()),
        parent_pack,
    };
    let mut pack = ScientificDiffPack::build(pack_draft)?;
    pack.sign(&key);

    // Write artifacts to disk.
    let vela_dir = if frontier_path.is_dir() {
        frontier_path.join(".vela")
    } else {
        frontier_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(".vela")
    };
    let att_dir = vela_dir.join("agent_attestations");
    let pack_dir = vela_dir.join("diff_packs");
    let prop_dir = vela_dir.join("agent_proposals");
    for d in [&att_dir, &pack_dir, &prop_dir] {
        std::fs::create_dir_all(d).map_err(|e| format!("create {}: {e}", d.display()))?;
    }
    let att_path = att_dir.join(format!("{}.json", attestation.attestation_id));
    let pack_path = pack_dir.join(format!("{}.json", pack.pack_id));
    let att_body =
        serde_json::to_string_pretty(&attestation).map_err(|e| format!("serialize vaa: {e}"))?;
    let pack_body =
        serde_json::to_string_pretty(&pack).map_err(|e| format!("serialize vsd: {e}"))?;
    std::fs::write(&att_path, format!("{att_body}\n"))
        .map_err(|e| format!("write {}: {e}", att_path.display()))?;
    std::fs::write(&pack_path, format!("{pack_body}\n"))
        .map_err(|e| format!("write {}: {e}", pack_path.display()))?;
    for (pid, stub) in stub_writes {
        let path = prop_dir.join(format!("{pid}.json"));
        let body =
            serde_json::to_string_pretty(&stub).map_err(|e| format!("serialize stub: {e}"))?;
        std::fs::write(&path, format!("{body}\n"))
            .map_err(|e| format!("write {}: {e}", path.display()))?;
    }

    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "frontier_id": frontier_id,
        "attestation_id": attestation.attestation_id,
        "pack_id": pack.pack_id,
        "proposal_ids": proposal_ids,
        "wrote": {
            "attestation": att_path.display().to_string(),
            "pack": pack_path.display().to_string(),
        }
    }))
    .unwrap_or_default())
}

/// `work` action=claim — lease an open obligation so other swarm agents route
/// around it. Emits a signed `attempt.claimed` event (the agent's OWN key
/// via VELA_AGENT_KEY_HEX; never a human's). One live lease per obligation;
/// expiry = claimed_at + ttl, computed at read time. Coordination, not
/// authority: a lease decides nothing.
pub fn claim_task(args: &Value) -> Result<String, String> {
    let frontier_path: PathBuf = args
        .get("frontier_path")
        .and_then(Value::as_str)
        .ok_or("frontier_path required")?
        .into();
    let obligation = args
        .get("obligation_id")
        .and_then(Value::as_str)
        .ok_or("obligation_id required (a vf_… finding id)")?;
    let agent_actor = args
        .get("agent_actor")
        .and_then(Value::as_str)
        .ok_or("agent_actor required")?;
    if !agent_actor.starts_with("agent:") && !agent_actor.starts_with("ci:") {
        return Err("claim_task is for agent:/ci: actors".to_string());
    }
    let key = agent_signing_key(Some(agent_actor))?;
    let ttl = args
        .get("ttl_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(86_400);
    let pubkey = hex::encode(key.verifying_key().to_bytes());
    let mut project = vela_protocol::repo::load_from_path(&frontier_path)
        .map_err(|e| format!("load frontier: {e}"))?;
    // An obligation is usually a finding (vf_…) but may be an EXTERNAL
    // work target (e.g. `erdos:443` — a problem with no finding yet).
    // External ids must be namespaced so a typo'd vf_ id can't slip.
    let is_finding = project.findings.iter().any(|f| f.id == obligation);
    let is_external = obligation.contains(':')
        && !obligation.starts_with("vf_")
        && obligation
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-'));
    if !is_finding && !is_external {
        return Err(format!(
            "obligation {obligation} is neither a finding on this frontier nor a              namespaced external target (e.g. erdos:443)"
        ));
    }
    // A competing owner is routed around. The exact same actor/key is allowed
    // to refresh its lease, and a zero-TTL same-owner update is allowed to
    // reach the reducer as the signed release compare-and-swap.
    let now = chrono::Utc::now();
    let live = project.attempt_claims.iter().find(|claim| {
        claim.obligation_id == obligation
            && chrono::DateTime::parse_from_rfc3339(&claim.claimed_at)
                .map(|claimed| {
                    claimed + chrono::Duration::seconds(claim.lease_ttl_seconds as i64) > now
                })
                .unwrap_or(false)
    });
    let requested_prior = args.get("prior_claim_event_id").and_then(Value::as_str);
    let release_reason = args.get("release_reason").and_then(Value::as_str);
    if ttl == 0 {
        let live = live
            .ok_or_else(|| format!("cannot release {obligation}: no current live lease exists"))?;
        if live.claimant_actor != agent_actor {
            return Err(format!(
                "cannot release {obligation}: leased by {}, not {agent_actor}",
                live.claimant_actor
            ));
        }
        if live.claimant_pubkey != pubkey {
            return Err(format!(
                "cannot release {obligation}: the active agent key does not match the lease key"
            ));
        }
        if requested_prior != live.claim_event_id.as_deref() {
            return Err(format!(
                "cannot release {obligation}: prior lease identity does not match"
            ));
        }
        if release_reason.is_none_or(|reason| reason.trim().is_empty()) {
            return Err(format!(
                "cannot release {obligation}: a non-empty release_reason is required"
            ));
        }
    } else if let Some(live) = live
        && (live.claimant_actor != agent_actor || live.claimant_pubkey != pubkey)
    {
        return Ok(serde_json::json!({
            "ok": false,
            "already_claimed_by": live.claimant_actor,
            "claimed_at": live.claimed_at,
            "ttl_seconds": live.lease_ttl_seconds,
        })
        .to_string());
    }
    let prior_claim_event_id = if ttl == 0 {
        requested_prior.map(ToString::to_string)
    } else {
        live.and_then(|claim| claim.claim_event_id.clone())
    };
    let event_reason = release_reason
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("obligation lease (swarm coordination)");
    let mut payload = serde_json::json!({
        "obligation_id": obligation,
        "lease_ttl_seconds": ttl,
        "claimant_actor": agent_actor,
        "claimant_pubkey": pubkey,
    });
    if let Some(prior) = prior_claim_event_id.as_deref() {
        payload["prior_claim_event_id"] = serde_json::json!(prior);
    }
    if ttl == 0 {
        payload["release_reason"] = serde_json::json!(event_reason);
    }
    let mut event =
        vela_protocol::events::new_finding_event(vela_protocol::events::FindingEventInput {
            kind: "attempt.claimed",
            finding_id: obligation,
            actor_id: agent_actor,
            actor_type: vela_protocol::events::actor_kind(agent_actor),
            reason: event_reason,
            before_hash: "sha256:null",
            after_hash: "sha256:null",
            payload,
            caveats: Vec::new(),
            timestamp: None,
        });
    // Sign before reducer application so every persisted event has already
    // crossed the exact signed-byte boundary. The reducer's lease checks are
    // pure and deterministic over those bytes.
    event.signature = Some(vela_protocol::sign::sign_event(&event, &key)?);
    vela_protocol::reducer::apply_event(&mut project, &event)?;
    project.events.push(event.clone());
    vela_protocol::repo::save_to_path(&frontier_path, &project)
        .map_err(|e| format!("save: {e}"))?;
    Ok(serde_json::json!({
        "ok": true,
        "obligation": obligation,
        "claimed_by": agent_actor,
        "ttl_seconds": ttl,
        "claim_event_id": event.id,
        "claimed_at": event.timestamp,
        "claimant_pubkey": pubkey,
        "prior_claim_event_id": prior_claim_event_id,
        "release_reason": if ttl == 0 { Some(event_reason) } else { None },
    })
    .to_string())
}

/// `verify` mode=strict — hold the LOCAL frontier to the one strict bar
/// (validation + strict reducer replay + signature signals), over MCP. The
/// agent's "does this frontier pass the gate right now?" question, answered
/// by the same bundle the hub's ingestor enforces.
pub fn check_run(args: &Value) -> Result<String, String> {
    let frontier_path: PathBuf = args
        .get("frontier_path")
        .and_then(Value::as_str)
        .ok_or("frontier_path required")?
        .into();
    match crate::verify::verify_frontier_strict(&frontier_path) {
        Ok((project, fid)) => Ok(serde_json::json!({
            "ok": true,
            "frontier_id": fid,
            "findings": project.findings.len(),
            "events": project.events.len(),
            "note": "strict bar held: validation + reducer replay + signature signals",
        })
        .to_string()),
        Err(e) => Ok(serde_json::json!({
            "ok": false,
            "error": e,
        })
        .to_string()),
    }
}

/// `verify` mode=witness — re-verify a frontier's stored witnesses from
/// scratch with the frozen exact verifiers, over MCP. Walks
/// `witnesses/*.witness.json` (or any `*.witness.json` under the path).
pub fn reproduce_run(args: &Value) -> Result<String, String> {
    let path: PathBuf = args
        .get("frontier_path")
        .and_then(Value::as_str)
        .ok_or("frontier_path required")?
        .into();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut roots = vec![path.clone()];
    if path.join("witnesses").is_dir() {
        roots.push(path.join("witnesses"));
    }
    for root in roots {
        if root.is_file() {
            files.push(root);
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let p = e.path();
                if p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".witness.json"))
                {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Ok(serde_json::json!({
            "ok": false,
            "error": format!("no *.witness.json under {}", path.display()),
        })
        .to_string());
    }
    let mut passed = 0usize;
    let mut failures: Vec<Value> = Vec::new();
    for f in &files {
        let outcome = std::fs::read_to_string(f)
            .map_err(|e| format!("read: {e}"))
            .and_then(|raw| {
                serde_json::from_str::<vela_verify::Witness>(&raw)
                    .map_err(|e| format!("parse: {e}"))
            })
            .map(|w| vela_verify::verify_witness(&w));
        match outcome {
            Ok(r) if r.ok => passed += 1,
            Ok(r) => failures.push(serde_json::json!({
                "witness": f.display().to_string(),
                "message": r.message,
            })),
            Err(e) => failures.push(serde_json::json!({
                "witness": f.display().to_string(),
                "message": e,
            })),
        }
    }
    Ok(serde_json::json!({
        "ok": failures.is_empty(),
        "passed": passed,
        "failed": failures.len(),
        "failures": failures,
    })
    .to_string())
}

/// `work` action=record — land an activity record (vrc_) on the LOCAL
/// frontier as a pending proposal. The git-native agent write path: the
/// agent works in the repo, the record becomes a reviewable proposal,
/// `git push` publishes, the hub re-indexes. Never decides — the proposal
/// waits for a human key.
pub fn record_propose(args: &Value) -> Result<String, String> {
    let frontier_path: PathBuf = args
        .get("frontier_path")
        .and_then(Value::as_str)
        .ok_or("frontier_path required")?
        .into();
    let record_path: PathBuf = args
        .get("record_path")
        .and_then(Value::as_str)
        .ok_or("record_path required")?
        .into();
    let raw = std::fs::read_to_string(&record_path)
        .map_err(|e| format!("read {}: {e}", record_path.display()))?;
    let rc: vela_protocol::record::ActivityRecord =
        serde_json::from_str(&raw).map_err(|e| format!("record parse: {e}"))?;
    let signed = rc.verify()?;
    let project = vela_protocol::repo::load_from_path(&frontier_path)
        .map_err(|e| format!("load frontier: {e}"))?;
    if project.frontier_id() != rc.frontier_id {
        return Err(format!(
            "record is for {}, this frontier is {}",
            rc.frontier_id,
            project.frontier_id()
        ));
    }
    let head_now = vela_protocol::events::event_log_hash(&project.events);
    let staleness = if head_now == rc.against_head {
        "recorded against the current head".to_string()
    } else {
        format!(
            "recorded against head {}…, current head {}… — review the delta",
            &rc.against_head[..rc.against_head.len().min(16)],
            &head_now[..head_now.len().min(16)]
        )
    };
    let report = vela_protocol::state::add_finding(
        &frontier_path,
        rc.to_finding_draft(&staleness, signed),
        false, // pending only: an MCP client never applies state
    )?;
    Ok(serde_json::json!({
        "ok": true,
        "record": rc.id,
        "signed": signed,
        "proposal_id": report.proposal_id,
        "status": report.proposal_status,
        "note": "pending; a human key accepts. git push publishes.",
    })
    .to_string())
}

// ----------------------------------------------------------------------------
// v0.736: fold-at-deposit — the attempt ledger compounds instead of bloating.
// ----------------------------------------------------------------------------

/// The search signature an attempt's identity-of-search folds on:
/// `sha256(target_obligation_id ‖ ":" ‖ channel ‖ ":" ‖
/// sorted(method_families).join(","))[:16]`. Two failed passes at the same
/// obligation, down the same channel, with the same method families are the
/// same search — a second deposit that learned nothing new is ledger noise.
#[must_use]
pub fn search_signature(
    target_obligation_id: &str,
    channel: &str,
    method_families: &[String],
) -> String {
    let mut fams: Vec<&str> = method_families.iter().map(String::as_str).collect();
    fams.sort_unstable();
    let preimage = format!("{target_obligation_id}:{channel}:{}", fams.join(","));
    sha256_hex(preimage.as_bytes())[..16].to_string()
}

/// An attempt's derived search signature: channel = its first `channel:`
/// named obstruction (or "" when it names none).
#[must_use]
pub fn attempt_search_signature(a: &vela_protocol::attempt::Attempt) -> String {
    let channel = crate::channel_map::attempt_channel(a).unwrap_or("");
    search_signature(&a.target_obligation_id, channel, &a.method_families)
}

/// The numeric bound a bound-shaped claim carries ("a(8) >= 33" → 33.0).
/// Attempts have no first-class bound field yet; the claim text is where
/// bounds live on this substrate (`kind` gives the direction). `None` for
/// non-bound claims.
fn claim_bound(a: &vela_protocol::attempt::Attempt) -> Option<f64> {
    for op in ["<=", ">=", "<", ">", "="] {
        if let Some(idx) = a.claim.rfind(op) {
            let tail = a.claim[idx + op.len()..].trim();
            let num: String = tail
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            if let Ok(v) = num.parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

/// Whether `new_bound` improves on `old_bound` for this attempt kind:
/// upper bounds improve downward, everything else (lower bounds, exact
/// values) improves upward.
fn bound_improves(kind: &str, new_bound: f64, old_bound: f64) -> bool {
    if kind.contains("upper") {
        new_bound < old_bound
    } else {
        new_bound > old_bound
    }
}

/// The fold predicate: true when the candidate deposit adds NO new
/// information over an already-banked attempt with the same search
/// signature — no new named obstruction, no artifact (verifier attachment)
/// hash not already present, no better bound. A fold returns the existing
/// `vat_` id instead of depositing a duplicate; anything new deposits
/// normally.
#[must_use]
pub fn folds_into(
    candidate: &vela_protocol::attempt::Attempt,
    existing: &vela_protocol::attempt::Attempt,
) -> bool {
    use std::collections::BTreeSet;
    if attempt_search_signature(candidate) != attempt_search_signature(existing) {
        return false;
    }
    let known_obstructions: BTreeSet<&str> = existing
        .named_obstructions
        .iter()
        .map(String::as_str)
        .collect();
    if candidate
        .named_obstructions
        .iter()
        .any(|o| !known_obstructions.contains(o.as_str()))
    {
        return false;
    }
    let known_artifacts: BTreeSet<&str> = existing
        .verifier_attachments
        .iter()
        .map(String::as_str)
        .collect();
    if candidate
        .verifier_attachments
        .iter()
        .any(|v| !known_artifacts.contains(v.as_str()))
    {
        return false;
    }
    match (claim_bound(candidate), claim_bound(existing)) {
        (Some(nb), Some(eb)) => !bound_improves(&candidate.kind, nb, eb),
        // A bound where none was banked is information.
        (Some(_), None) => false,
        _ => true,
    }
}

/// Whether the proposer self-reports this deposit as a failed pass (the
/// only deposits that fold; a success is never silently dropped).
fn is_failed_deposit(claimed_status: &str) -> bool {
    let s = claimed_status.to_ascii_lowercase();
    s.contains("fail") || s == "refuted"
}

/// `work` action=deposit — deposit a signed `vat_` attempt on the LOCAL
/// frontier, folding duplicate failed searches. Before a FAILED attempt
/// lands, the existing ledger is scanned for an attempt with the same
/// derived search signature; when one exists and the new attempt adds no
/// new information ([`folds_into`]), nothing is deposited and the existing
/// `vat_` id returns with `"folded": true`. Success deposits, and failures
/// that learned something new, always land. Signed under the agent's own
/// auto-minted session key; `claimed_status` stays display-only.
pub fn deposit_attempt(args: &Value) -> Result<String, String> {
    use vela_protocol::attempt::{Attempt, AttemptDraft, ProducerRef};

    let frontier_path = frontier_path_arg(args)?;
    let agent_actor = args
        .get("agent_actor")
        .and_then(Value::as_str)
        .ok_or("agent_actor required")?;
    if !agent_actor.starts_with("agent:") && !agent_actor.starts_with("ci:") {
        return Err("deposit_attempt is for agent:/ci: actors".to_string());
    }
    let key = agent_signing_key(Some(agent_actor))?;

    let str_arg = |k: &str| -> String {
        args.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let vec_arg = |k: &str| -> Vec<String> {
        args.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut project = vela_protocol::repo::load_from_path(&frontier_path)
        .map_err(|e| format!("load frontier: {e}"))?;

    let mut frontier_label = str_arg("frontier");
    if frontier_label.is_empty() {
        frontier_label = project.frontier_id();
    }
    let producer = args.get("producer").cloned().unwrap_or(Value::Null);
    let producer_str = |k: &str| -> String {
        producer
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let claimed_status = str_arg("claimed_status");
    let draft = AttemptDraft {
        problem: args.get("problem").and_then(Value::as_u64).unwrap_or(0) as u32,
        frontier: frontier_label,
        kind: str_arg("kind"),
        claim: str_arg("claim"),
        detail: str_arg("detail"),
        claimed_status: claimed_status.clone(),
        insight: str_arg("insight"),
        base_frontier_root: str_arg("base_frontier_root"),
        target_obligation_id: str_arg("target_obligation_id"),
        statement_variant_id: str_arg("statement_variant_id"),
        method_families: vec_arg("method_families"),
        remaining_obligations: vec_arg("remaining_obligations"),
        named_obstructions: vec_arg("named_obstructions"),
        verifier_attachments: vec_arg("verifier_attachments"),
        producer: ProducerRef {
            system: producer_str("system"),
            version: producer_str("version"),
            config_digest: producer_str("config_digest"),
        },
        ..Default::default()
    };
    let attempt = Attempt::build(draft, &key)?;

    // Fold: a failed pass that re-ran a banked search and learned nothing
    // new returns the banked id instead of depositing ledger noise.
    if is_failed_deposit(&claimed_status)
        && let Some(existing) = project.attempts.iter().find(|e| folds_into(&attempt, e))
    {
        return Ok(serde_json::json!({
            "ok": true,
            "attempt_id": existing.attempt_id,
            "folded": true,
            "search_signature": attempt_search_signature(&attempt),
            "note": "same search signature, no new information — banked attempt returned instead of a duplicate deposit",
        })
        .to_string());
    }

    let mut event = attempt.deposit_event(
        agent_actor,
        vela_protocol::events::actor_kind(agent_actor),
        "attempt deposit via MCP (provenance, not a verdict)",
    );
    vela_protocol::reducer::apply_event(&mut project, &event)?;
    event.signature = Some(vela_protocol::sign::sign_event(&event, &key)?);
    project.events.push(event);
    vela_protocol::repo::save_to_path(&frontier_path, &project)
        .map_err(|e| format!("save: {e}"))?;
    Ok(serde_json::json!({
        "ok": true,
        "attempt_id": attempt.attempt_id,
        "folded": false,
        "search_signature": attempt_search_signature(&attempt),
    })
    .to_string())
}

// Note: the write-side tools here mutate VELA_AGENT_KEY_HEX (the
// env-driven signing key for submit_diff_pack), which cannot run
// safely under cargo's parallel test runner because env mutation is
// `unsafe` in modern Rust editions. The submit_diff_pack signed
// roundtrip is exercised end-to-end by the bash gate
// `scripts/test-mcp-server.sh` instead, which spawns the server with
// a controlled env.

#[cfg(test)]
mod fold_tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use vela_protocol::attempt::{Attempt, AttemptDraft};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[9u8; 32])
    }

    fn failed(claim: &str, obstructions: Vec<&str>, attachments: Vec<&str>) -> Attempt {
        let draft = AttemptDraft {
            problem: 647,
            frontier: "erdos-frontier".into(),
            kind: "upper_bound".into(),
            claim: claim.into(),
            claimed_status: "failed".into(),
            target_obligation_id: "erdos:647".into(),
            method_families: vec!["sieve".into(), "cp-sat".into()],
            named_obstructions: obstructions.into_iter().map(String::from).collect(),
            verifier_attachments: attachments.into_iter().map(String::from).collect(),
            ..Default::default()
        };
        Attempt::build(draft, &key()).unwrap()
    }

    #[test]
    fn signature_is_order_independent_over_method_families() {
        let a = search_signature("erdos:647", "erdos647:prime", &["b".into(), "a".into()]);
        let b = search_signature("erdos:647", "erdos647:prime", &["a".into(), "b".into()]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        // Any component change moves the signature.
        assert_ne!(
            a,
            search_signature("erdos:647", "erdos647:crt_cover", &["a".into(), "b".into()])
        );
        assert_ne!(
            a,
            search_signature("erdos:648", "erdos647:prime", &["a".into(), "b".into()])
        );
    }

    #[test]
    fn duplicate_failed_search_folds() {
        let banked = failed("no route", vec!["channel:erdos647:prime"], vec![]);
        let rerun = failed("still no route", vec!["channel:erdos647:prime"], vec![]);
        assert!(folds_into(&rerun, &banked), "same search, nothing new");
    }

    #[test]
    fn new_obstruction_blocks_the_fold() {
        let banked = failed("no route", vec!["channel:erdos647:prime"], vec![]);
        let learned = failed(
            "no route",
            vec!["channel:erdos647:prime", "parity-wall:mod-4"],
            vec![],
        );
        assert!(
            !folds_into(&learned, &banked),
            "a named obstruction is information"
        );
    }

    #[test]
    fn new_artifact_hash_blocks_the_fold() {
        let banked = failed(
            "no route",
            vec!["channel:erdos647:prime"],
            vec!["vva_0000000000000001"],
        );
        let with_artifact = failed(
            "no route",
            vec!["channel:erdos647:prime"],
            vec!["vva_0000000000000001", "vva_0000000000000002"],
        );
        assert!(!folds_into(&with_artifact, &banked));
        // The reverse (a subset of already-known artifacts) still folds.
        let subset = failed("no route", vec!["channel:erdos647:prime"], vec![]);
        assert!(folds_into(&subset, &banked));
    }

    #[test]
    fn better_bound_blocks_the_fold() {
        // kind = upper_bound: smaller is better.
        let banked = failed("f(n) <= 40", vec!["channel:erdos647:prime"], vec![]);
        let better = failed("f(n) <= 35", vec!["channel:erdos647:prime"], vec![]);
        let worse = failed("f(n) <= 50", vec!["channel:erdos647:prime"], vec![]);
        assert!(
            !folds_into(&better, &banked),
            "an improved bound is information"
        );
        assert!(folds_into(&worse, &banked), "a worse bound adds nothing");
    }

    #[test]
    fn different_channel_never_folds() {
        let banked = failed("no route", vec!["channel:erdos647:prime"], vec![]);
        let other = failed("no route", vec!["channel:erdos647:crt_cover"], vec![]);
        assert!(!folds_into(&other, &banked), "different search signature");
    }

    #[test]
    fn only_failed_statuses_are_fold_candidates() {
        assert!(is_failed_deposit("failed"));
        assert!(is_failed_deposit("failed_search"));
        assert!(is_failed_deposit("refuted"));
        assert!(!is_failed_deposit("candidate"));
        assert!(!is_failed_deposit("machine_verified"));
    }
}

#[cfg(test)]
mod agent_key_tests {
    use super::*;

    #[test]
    fn mints_once_reuses_after_and_stays_agent_only() {
        let base = std::env::temp_dir().join(format!("vela-agent-keys-{}", std::process::id()));
        let a = mint_or_load_agent_key(&base, "agent:swarm-7").unwrap();
        let b = mint_or_load_agent_key(&base, "agent:swarm-7").unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes(), "same actor, same key");
        let c = mint_or_load_agent_key(&base, "agent:swarm-8").unwrap();
        assert_ne!(a.to_bytes(), c.to_bytes(), "different actor, different key");
        assert!(base.join("agent-swarm-7/private.key").exists());
        std::fs::remove_dir_all(&base).ok();
    }
}
