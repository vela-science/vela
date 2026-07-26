//! MCP adapter for the vela_agent SDK surface.
//!
//! The underlying impls behind the `work`, `verify`, and `objects` MCP
//! tools: obligation leases, Receipt v1 landing, the strict/witness
//! verification runs, and the `.vela/` object reads.
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

use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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
    if !actor.starts_with("agent:") && !actor.starts_with("ci:") && !actor.starts_with("verifier:")
    {
        return Err(format!(
            "agent key auto-mint is for agent:/ci:/verifier: actors, not '{actor}' — humans run `vela id create`"
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

/// Validate, sign, and reduce one coordination lease event against an already
/// loaded Project. This function performs no filesystem write; the caller must
/// install the resulting Project through Vela's recoverable frontier
/// transaction rather than a load/save read-modify-write sequence.
pub fn apply_claim_task_to_project(
    args: &Value,
    project: &mut vela_protocol::project::Project,
) -> Result<Value, String> {
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
    if ttl > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
        return Err(format!(
            "ttl_seconds must be at most {}",
            vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
        ));
    }
    let pubkey = hex::encode(key.verifying_key().to_bytes());
    let state_root_before = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
    // An obligation is usually a finding (vf_…) but may be an EXTERNAL
    // work target (e.g. `erdos:443` — a problem with no finding yet).
    // External ids must be namespaced so a typo'd vf_ id can't slip.
    let is_finding = project.findings.iter().any(|f| f.id == obligation);
    let is_external = crate::frontier_next::validate_external_target_id(obligation).is_ok();
    let is_exact_legacy_release = ttl == 0
        && project
            .attempt_claims
            .iter()
            .any(|claim| claim.obligation_id == obligation);
    if !is_finding && !is_external && !is_exact_legacy_release {
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
            && vela_protocol::events::attempt_lease_expiry(
                &claim.claimed_at,
                claim.lease_ttl_seconds,
            )
            .is_ok_and(|expires_at| expires_at > now)
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
            "state_root_before": state_root_before,
        }));
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
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
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
            timestamp: Some(&timestamp),
        });
    // Sign before reducer application so every persisted event has already
    // crossed the exact signed-byte boundary. The reducer's lease checks are
    // pure and deterministic over those bytes.
    event.signature = Some(vela_protocol::sign::sign_event(&event, &key)?);
    vela_protocol::reducer::apply_event(project, &event)?;
    project.events.push(event.clone());
    // `snapshot_hash` includes the derived ProjectStats table. Keep the
    // in-memory candidate byte-equivalent to a fresh replay before the caller
    // renders frontier.json and vela.lock; otherwise the lease event is
    // present but those derived views retain the pre-claim snapshot hash.
    vela_protocol::project::recompute_stats(project);
    let state_root_after = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&project.events)
    );
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
        "state_root_before": state_root_before,
        "state_root_after": state_root_after,
    }))
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
