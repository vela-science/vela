//! Private producer work for current repository epochs.
//!
//! A current Attempt is ignored authoring state. It binds one exact Target
//! Offer and repository read set, but creates no canonical lease, Event,
//! authority record, or scientific standing.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cli::safe_text;

const ATTEMPT_SCHEMA: &str = "vela.attempt.v2";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v2";
const ATTEMPT_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentTaskContract {
    schema: String,
    objective: String,
    completion_condition: String,
    allowed_actions: Vec<String>,
    forbidden_actions: Vec<String>,
    required_outputs: Vec<String>,
    required_checks: Vec<String>,
    escalation_path: String,
    authority_ceiling: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentAttempt {
    schema: String,
    attempt_id: String,
    frontier_id: String,
    target: String,
    actor: String,
    created_at: String,
    expires_at: String,
    task_contract: CurrentTaskContract,
    task_contract_root: String,
    target_task_binding: vela_edge::target_index::TargetTaskBindingV2,
    briefing: Value,
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)?
    ))
}

fn contract(
    packet: &Value,
    target: &vela_edge::target_index::TargetIndexEntryV2,
) -> CurrentTaskContract {
    let objective = packet
        .get("statement")
        .and_then(Value::as_str)
        .map(|statement| format!("Produce decision-relevant evidence for: {statement}"))
        .or_else(|| {
            packet
                .get("objective")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| target.objective.clone());
    let mut required_outputs = packet
        .get("allowed_outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|output| output.get("type").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if required_outputs.is_empty() {
        required_outputs.push(
            "one evidence artifact or one deliberately informative negative result".to_string(),
        );
    }
    required_outputs.sort();
    required_outputs.dedup();
    CurrentTaskContract {
        schema: TASK_CONTRACT_SCHEMA.to_string(),
        objective,
        completion_condition:
            "Create one valid Submission whose evidence and caveats address this exact target."
                .to_string(),
        allowed_actions: vec![
            "inspect the pinned repository, packet, and task contract".to_string(),
            "run frozen verifiers and private search or experiment loops".to_string(),
            "create bounded evidence artifacts and one Submission".to_string(),
        ],
        forbidden_actions: vec![
            "accept, reject, apply, finalize, or sign a truth-bearing proposal".to_string(),
            "read or use a human or repository-authority signing key".to_string(),
            "hand-edit current repository records or derived views".to_string(),
            "treat producer output as verification or scientific acceptance".to_string(),
        ],
        required_outputs,
        required_checks: vec![
            "run every producer-side check claimed by the Submission".to_string(),
            "state at least one scope limit or explicit no-known-limit caveat".to_string(),
            "keep artifacts repository-relative, bounded, and content-addressed".to_string(),
        ],
        escalation_path:
            "Submit for independent Verification and an authorized terminal decision.".to_string(),
        authority_ceiling:
            "Private producer evidence only; this Attempt grants no repository or scientific authority."
                .to_string(),
    }
}

fn attempt_id(
    frontier_id: &str,
    target: &str,
    actor: &str,
    task_contract_root: &str,
    binding_root: &str,
) -> Result<String, String> {
    let preimage = json!({
        "schema": ATTEMPT_SCHEMA,
        "frontier_id": frontier_id,
        "target": target,
        "actor": actor,
        "task_contract_root": task_contract_root,
        "target_task_binding_root": binding_root,
    });
    Ok(format!(
        "vat_{}",
        vela_protocol::canonical::sha256_canonical(&preimage)?
    ))
}

fn safe_session_dir(frontier: &Path, target: &str) -> PathBuf {
    crate::workflow::session_dir(frontier, target)
}

fn attempt_path(frontier: &Path, target: &str) -> PathBuf {
    safe_session_dir(frontier, target).join("attempt.json")
}

fn validate(attempt: &CurrentAttempt) -> Result<(), String> {
    if attempt.schema != ATTEMPT_SCHEMA {
        return Err(format!(
            "unsupported current Attempt schema {}",
            attempt.schema
        ));
    }
    if attempt.task_contract.schema != TASK_CONTRACT_SCHEMA
        || canonical_root(&attempt.task_contract)? != attempt.task_contract_root
    {
        return Err("current Attempt task contract does not match its root".to_string());
    }
    attempt.target_task_binding.validate()?;
    if attempt.frontier_id != attempt.target_task_binding.frontier_id
        || attempt.target != attempt.target_task_binding.target_id
    {
        return Err("current Attempt does not match its target binding".to_string());
    }
    let expected = attempt_id(
        &attempt.frontier_id,
        &attempt.target,
        &attempt.actor,
        &attempt.task_contract_root,
        &attempt.target_task_binding.binding_root,
    )?;
    if attempt.attempt_id != expected {
        return Err("current Attempt identity does not match its closed preimage".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(&attempt.created_at)
        .map_err(|error| format!("current Attempt created_at: {error}"))?;
    chrono::DateTime::parse_from_rfc3339(&attempt.expires_at)
        .map_err(|error| format!("current Attempt expires_at: {error}"))?;
    Ok(())
}

fn encode(attempt: &CurrentAttempt) -> Result<Vec<u8>, String> {
    validate(attempt)?;
    let mut bytes =
        serde_json::to_vec_pretty(attempt).map_err(|error| format!("encode Attempt: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() > ATTEMPT_MAX_BYTES {
        return Err(format!(
            "current Attempt is {} bytes; limit is {ATTEMPT_MAX_BYTES}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn read(path: &Path) -> Result<CurrentAttempt, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect current Attempt {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > ATTEMPT_MAX_BYTES as u64
    {
        return Err(format!(
            "current Attempt must be a bounded regular non-symlink file: {}",
            path.display()
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("read current Attempt {}: {error}", path.display()))?;
    let attempt: CurrentAttempt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse current Attempt {}: {error}", path.display()))?;
    validate(&attempt)?;
    Ok(attempt)
}

fn ensure_private_directory(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "{label} must be a real directory: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)
            .map_err(|error| format!("create {label} {}: {error}", path.display())),
        Err(error) => Err(format!("inspect {label} {}: {error}", path.display())),
    }
}

struct AttemptLock {
    _file: fs::File,
}

fn lock_attempt(frontier: &Path, target: &str) -> Result<AttemptLock, String> {
    let vela = frontier.join(".vela");
    ensure_private_directory(&vela, "Frontier private directory")?;
    let work = vela.join("work");
    ensure_private_directory(&work, "Attempt root")?;
    let directory = safe_session_dir(frontier, target);
    ensure_private_directory(&directory, "Attempt directory")?;
    let path = directory.join(".lock");
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!(
            "Attempt lock must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("open Attempt lock {}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| format!("inspect Attempt lock {}: {error}", path.display()))?
        .is_file()
    {
        return Err(format!("Attempt lock is not a file: {}", path.display()));
    }
    match file.try_lock() {
        Ok(()) => Ok(AttemptLock { _file: file }),
        Err(std::fs::TryLockError::WouldBlock) => {
            Err(format!("another command is changing target {target}"))
        }
        Err(std::fs::TryLockError::Error(error)) => {
            Err(format!("lock Attempt {}: {error}", path.display()))
        }
    }
}

fn write(frontier: &Path, attempt: &CurrentAttempt) -> Result<PathBuf, String> {
    let bytes = encode(attempt)?;
    let vela = frontier.join(".vela");
    ensure_private_directory(&vela, "Frontier private directory")?;
    let work = vela.join("work");
    ensure_private_directory(&work, "Attempt root")?;
    let directory = safe_session_dir(frontier, &attempt.target);
    ensure_private_directory(&directory, "Attempt directory")?;
    let path = directory.join("attempt.json");
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!(
            "current Attempt must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    let temporary = directory.join(format!(
        ".attempt-{}-{}.tmp",
        std::process::id(),
        &attempt.attempt_id[4..20]
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("create current Attempt: {error}"))?;
        file.write_all(&bytes)
            .map_err(|error| format!("write current Attempt: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("persist current Attempt: {error}"))?;
        fs::rename(&temporary, &path)
            .map_err(|error| format!("install current Attempt: {error}"))?;
        Ok(path.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn result(attempt: &CurrentAttempt, path: &Path, idempotent: bool) -> Value {
    let packet = &attempt.target_task_binding.packet;
    json!({
        "schema": ATTEMPT_SCHEMA,
        "ok": true,
        "command": "start",
        "idempotent": idempotent,
        "frontier_id": attempt.frontier_id,
        "target_id": attempt.target,
        "attempt": {
            "id": attempt.attempt_id,
            "path": path.display().to_string(),
            "actor": attempt.actor,
            "expires_at": attempt.expires_at,
        },
        "starting_roots": {
            "epoch": attempt.target_task_binding.repository.epoch_id,
            "repository": attempt.target_task_binding.repository.repository_root,
            "target_index": attempt.target_task_binding.target_index_root,
            "task_contract": attempt.task_contract_root,
            "git_commit": attempt.target_task_binding.claim_read_set.git_commit,
            "git_tree": attempt.target_task_binding.claim_read_set.git_tree,
        },
        "task": {
            "objective": attempt.task_contract.objective,
            "completion_condition": attempt.task_contract.completion_condition,
            "required_outputs": attempt.task_contract.required_outputs,
            "required_checks": attempt.task_contract.required_checks,
            "authority_ceiling": attempt.task_contract.authority_ceiling,
        },
        "packet": packet,
        "briefing": attempt.briefing,
        "canonical_write": false,
        "authority_key_read": false,
        "legacy_runtime_used": false,
        "next_command": format!(
            "vela submit --attempt {} --claim <scoped-result> --type <type> --replayability <class> --artifact <path>:<kind> --caveat <limit> --as {} --json",
            attempt.attempt_id, attempt.actor
        ),
    })
}

fn open(frontier: &Path, target_id: &str, actor: &str, ttl_seconds: u64) -> Result<Value, String> {
    if ttl_seconds == 0 || ttl_seconds > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
        return Err(format!(
            "private Attempt TTL must be between 1 and {} seconds",
            vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
        ));
    }
    let repository = crate::repository_upgrade::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.epoch_id,
        &repository_root,
    )?
    .ok_or_else(|| "current repository has no Target Index".to_string())?;
    let target = assessment
        .index
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .cloned()
        .ok_or_else(|| format!("current Target Index has no target {target_id:?}"))?;
    let binding = vela_edge::target_index::build_current_target_task_binding(
        frontier,
        &assessment,
        &repository.frontier_id,
        &repository.epoch_id,
        &repository_root,
        target_id,
    )?;
    let _lock = lock_attempt(frontier, target_id)?;
    let path = attempt_path(frontier, target_id);
    if path.is_file() {
        let existing = read(&path)?;
        if existing.actor != actor {
            return Err(format!(
                "target {target_id} already has a private Attempt owned by {}",
                existing.actor
            ));
        }
        vela_edge::target_index::revalidate_current_target_task_binding(
            frontier,
            &existing.target_task_binding,
        )?;
        let expires_at = chrono::DateTime::parse_from_rfc3339(&existing.expires_at)
            .map_err(|error| format!("current Attempt expires_at: {error}"))?;
        if expires_at <= Utc::now() {
            return Err(format!(
                "target {target_id} has an expired private Attempt; remove it with `vela start {target_id} --drop --as {actor} --reason <reason>`"
            ));
        }
        return Ok(result(&existing, &path, true));
    }
    let packet = assessment
        .packet_value(target_id)
        .cloned()
        .ok_or_else(|| format!("current target {target_id:?} has no verified packet"))?;
    let task_contract = contract(&packet, &target);
    let task_contract_root = canonical_root(&task_contract)?;
    let attempt_id = attempt_id(
        &repository.frontier_id,
        target_id,
        actor,
        &task_contract_root,
        &binding.binding_root,
    )?;
    let created_at = Utc::now();
    let ttl = i64::try_from(ttl_seconds)
        .map_err(|_| "private Attempt TTL exceeds the supported range".to_string())?;
    let expires_at = created_at
        .checked_add_signed(Duration::seconds(ttl))
        .ok_or_else(|| "private Attempt expiry overflowed".to_string())?;
    let attempt = CurrentAttempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        attempt_id,
        frontier_id: repository.frontier_id,
        target: target_id.to_string(),
        actor: actor.to_string(),
        created_at: created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        task_contract,
        task_contract_root,
        target_task_binding: binding,
        briefing: json!({
            "schema": "vela.work-briefing.v2",
            "target": target,
            "packet": packet,
        }),
    };
    let path = write(frontier, &attempt)?;
    Ok(result(&attempt, &path, false))
}

fn drop_attempt(frontier: &Path, target: &str, actor: &str, reason: &str) -> Result<Value, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("start --drop requires a non-empty reason".to_string());
    }
    crate::repository_upgrade::verify_current_repository_at(frontier, true)?;
    let _lock = lock_attempt(frontier, target)?;
    let path = attempt_path(frontier, target);
    let attempt = read(&path)?;
    if attempt.actor != actor {
        return Err(format!(
            "target {target} private Attempt belongs to {}, not {actor}",
            attempt.actor
        ));
    }
    fs::remove_file(&path)
        .map_err(|error| format!("remove private Attempt {}: {error}", path.display()))?;
    let directory = safe_session_dir(frontier, target);
    let _ = fs::remove_dir(&directory);
    Ok(json!({
        "schema": "vela.attempt-drop.v2",
        "ok": true,
        "command": "start.drop",
        "attempt_id": attempt.attempt_id,
        "target_id": target,
        "actor": actor,
        "reason": reason,
        "canonical_write": false,
        "authority_key_read": false,
        "legacy_runtime_used": false,
    }))
}

fn list(frontier: &Path) -> Result<Value, String> {
    crate::repository_upgrade::verify_current_repository_at(frontier, true)?;
    let root = frontier.join(".vela/work");
    let mut attempts = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path().join("attempt.json");
            path.is_file().then_some(path)
        })
        .map(|path| {
            let attempt = read(&path)?;
            Ok(json!({
                "attempt_id": attempt.attempt_id,
                "target_id": attempt.target,
                "actor": attempt.actor,
                "expires_at": attempt.expires_at,
                "path": path.display().to_string(),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    attempts.sort_by(|left, right| left["target_id"].as_str().cmp(&right["target_id"].as_str()));
    Ok(json!({
        "schema": "vela.attempt-list.v2",
        "ok": true,
        "command": "start",
        "attempts": attempts,
        "legacy_runtime_used": false,
    }))
}

pub(crate) fn cmd_start(
    frontier: &Path,
    target: Option<&str>,
    ttl: Option<u64>,
    drop_it: bool,
    reason: Option<&str>,
    actor: &str,
    json_out: bool,
) {
    crate::ui::set_mode("start", json_out);
    let result = match (target, drop_it) {
        (None, false) => list(frontier),
        (None, true) => Err("start --drop requires an exact target".to_string()),
        (Some(target), true) => drop_attempt(
            frontier,
            target,
            actor,
            reason.unwrap_or("producer abandoned the private Attempt"),
        ),
        (Some(_), false) if reason.is_some() => {
            Err("--reason is valid only with start --drop".to_string())
        }
        (Some(target), false) => {
            let ttl = ttl.unwrap_or_else(|| {
                crate::config::settings::try_resolve("work.lease_ttl_seconds", Some(frontier))
                    .unwrap_or_else(|error| crate::cli::fail_return(&error))
                    .0
                    .parse()
                    .unwrap_or_else(|_| {
                        crate::cli::fail_return(
                            "resolved work.lease_ttl_seconds is not a positive integer",
                        )
                    })
            });
            open(frontier, target, actor, ttl)
        }
    }
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&result);
    } else {
        let command = result["command"].as_str().unwrap_or("start");
        println!(
            "{command} · {}",
            result["target_id"].as_str().unwrap_or("Attempts")
        );
        if let Some(path) = result.pointer("/attempt/path").and_then(Value::as_str) {
            println!("  private {}", safe_text::inline(path));
        }
        if let Some(next) = result["next_command"].as_str() {
            println!("  next    {}", safe_text::inline(next));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_protocol::frontier_repository::GitObjectFormat;

    #[test]
    fn current_attempt_identity_binds_actor_target_contract_and_offer() {
        let first = attempt_id(
            "vfr_1234567890abcdef",
            "erdos:1056",
            "agent:codex",
            &format!("sha256:{}", "1".repeat(64)),
            &format!("sha256:{}", "2".repeat(64)),
        )
        .unwrap();
        let second = attempt_id(
            "vfr_1234567890abcdef",
            "erdos:1056",
            "agent:other",
            &format!("sha256:{}", "1".repeat(64)),
            &format!("sha256:{}", "2".repeat(64)),
        )
        .unwrap();
        assert_ne!(first, second);
        assert!(first.starts_with("vat_"));
    }

    #[test]
    fn current_attempt_round_trips_as_private_state_without_event_fields() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(".vela")).unwrap();
        let mut binding = vela_edge::target_index::TargetTaskBindingV2 {
            schema: vela_edge::target_index::TARGET_TASK_BINDING_SCHEMA_V2.to_string(),
            frontier_id: "vfr_1234567890abcdef".to_string(),
            target_id: "erdos:1056".to_string(),
            target_index_root: format!("sha256:{}", "1".repeat(64)),
            source: vela_edge::target_index::TargetIndexSourceV2 {
                git_object_format: GitObjectFormat::Sha1,
                git_commit: "2".repeat(40),
                git_tree: "3".repeat(40),
            },
            input_root: format!("sha256:{}", "4".repeat(64)),
            packet: vela_edge::target_index::TargetPacketRefV2 {
                schema: "erdos.problem-work.v1".to_string(),
                path: "packets/1056.json".to_string(),
                size: 42,
                sha256: format!("sha256:{}", "5".repeat(64)),
            },
            repository: vela_edge::target_index::TargetIndexRepositoryV3 {
                epoch_id: "vre_1234567890abcdef".to_string(),
                repository_root: format!("sha256:{}", "6".repeat(64)),
            },
            claim_read_set: vela_edge::target_index::TargetTaskClaimReadSetV2 {
                git_object_format: GitObjectFormat::Sha1,
                git_commit: "7".repeat(40),
                git_tree: "8".repeat(40),
            },
            binding_root: format!("sha256:{}", "0".repeat(64)),
        };
        binding.binding_root = binding.computed_binding_root().unwrap();
        let task_contract = CurrentTaskContract {
            schema: TASK_CONTRACT_SCHEMA.to_string(),
            objective: "Produce one bounded result.".to_string(),
            completion_condition: "Create one Submission.".to_string(),
            allowed_actions: vec!["inspect".to_string()],
            forbidden_actions: vec!["sign".to_string()],
            required_outputs: vec!["artifact".to_string()],
            required_checks: vec!["replay".to_string()],
            escalation_path: "verify".to_string(),
            authority_ceiling: "none".to_string(),
        };
        let task_contract_root = canonical_root(&task_contract).unwrap();
        let attempt = CurrentAttempt {
            schema: ATTEMPT_SCHEMA.to_string(),
            attempt_id: attempt_id(
                &binding.frontier_id,
                &binding.target_id,
                "agent:codex",
                &task_contract_root,
                &binding.binding_root,
            )
            .unwrap(),
            frontier_id: binding.frontier_id.clone(),
            target: binding.target_id.clone(),
            actor: "agent:codex".to_string(),
            created_at: "2026-07-27T00:00:00Z".to_string(),
            expires_at: "2026-07-28T00:00:00Z".to_string(),
            task_contract,
            task_contract_root,
            target_task_binding: binding,
            briefing: json!({"schema": "vela.work-briefing.v2"}),
        };
        let path = write(directory.path(), &attempt).unwrap();
        let decoded = read(&path).unwrap();
        assert_eq!(decoded.attempt_id, attempt.attempt_id);
        let encoded = String::from_utf8(fs::read(path).unwrap()).unwrap();
        assert!(!encoded.contains("event_log_root"));
        assert!(!encoded.contains("claim_event_id"));
        assert!(!encoded.contains("signature"));
    }
}
