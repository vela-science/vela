//! Private, ignored producer leases for current repository origins.
//!
//! An Attempt binds one actor to one exact Target packet for a bounded time
//! and budget. It is local coordination only: it creates no Event, authority
//! record, Verification, Decision, or scientific Standing.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vela_protocol::submission_v1::SubmissionV1;

use crate::cli::safe_text;

const ATTEMPT_SCHEMA: &str = "vela.attempt.v9";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v4";
const ATTEMPT_MAX_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_SUBMISSIONS: u64 = 16;
const DEFAULT_MAX_VERIFICATIONS: u64 = 16;
const DEFAULT_MAX_ARTIFACTS: u64 = 64;
const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AttemptBudget {
    max_submissions: u64,
    max_verifications: u64,
    max_artifacts: u64,
    max_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AttemptUsage {
    submissions: u64,
    verifications: u64,
    artifacts: u64,
    artifact_bytes: u64,
    registered_submission_ids: Vec<String>,
    registered_verification_record_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskContract {
    schema: String,
    objective: String,
    completion_condition: String,
    allowed_artifact_classes: Vec<String>,
    required_checks: Vec<String>,
    authority_ceiling: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CurrentAttempt {
    pub(crate) schema: String,
    pub(crate) attempt_id: String,
    authorization_root: String,
    pub(crate) frontier_id: String,
    pub(crate) target: String,
    pub(crate) actor: String,
    pub(crate) created_at: String,
    pub(crate) expires_at: String,
    allowed_artifact_classes: Vec<String>,
    budget: AttemptBudget,
    usage: AttemptUsage,
    task_contract: TaskContract,
    pub(crate) task_contract_root: String,
    starting_target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    pub(crate) target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    briefing: Value,
}

pub(crate) struct CurrentRoutineAttempt {
    pub(crate) attempt: CurrentAttempt,
    pub(crate) path: PathBuf,
    _lock: AttemptLock,
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(value)?
    ))
}

fn task_contract(
    packet: &Value,
    target: &vela_edge::target_index::TargetIndexEntryV2,
    allowed_artifact_classes: Vec<String>,
) -> TaskContract {
    let objective = packet
        .get("statement")
        .and_then(Value::as_str)
        .or_else(|| packet.get("objective").and_then(Value::as_str))
        .map(ToString::to_string)
        .unwrap_or_else(|| target.objective.clone());
    TaskContract {
        schema: TASK_CONTRACT_SCHEMA.to_string(),
        objective,
        completion_condition:
            "Register one bounded Submission or scoped Verification against this exact Target."
                .to_string(),
        allowed_artifact_classes,
        required_checks: vec![
            "run every producer-side check claimed by the Submission".to_string(),
            "state the result's exact scope and limits".to_string(),
            "keep retained artifacts bounded and content-addressed".to_string(),
        ],
        authority_ceiling:
            "This local lease may register evidence for review; it cannot accept or reject scientific Standing."
                .to_string(),
    }
}

fn authorization_root(attempt: &CurrentAttempt) -> Result<String, String> {
    canonical_root(&json!({
        "schema": "vela.attempt-authorization.internal.v2",
        "frontier_id": attempt.frontier_id,
        "target": attempt.target,
        "actor": attempt.actor,
        "created_at": attempt.created_at,
        "expires_at": attempt.expires_at,
        "allowed_artifact_classes": attempt.allowed_artifact_classes,
        "budget": attempt.budget,
        "task_contract_root": attempt.task_contract_root,
        "starting_target_task_binding_root": attempt.starting_target_task_binding.binding_root,
    }))
}

fn attempt_id(authorization_root: &str) -> Result<String, String> {
    Ok(format!(
        "vat_{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": ATTEMPT_SCHEMA,
            "authorization_root": authorization_root,
        }))?
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
            "unsupported private Attempt schema {}; delete the ignored Attempt and run vela start again",
            attempt.schema
        ));
    }
    if attempt.task_contract.schema != TASK_CONTRACT_SCHEMA
        || canonical_root(&attempt.task_contract)? != attempt.task_contract_root
    {
        return Err("current Attempt task contract does not match its root".to_string());
    }
    attempt.starting_target_task_binding.validate()?;
    attempt.target_task_binding.validate()?;
    if attempt.frontier_id != attempt.starting_target_task_binding.frontier_id
        || attempt.target != attempt.starting_target_task_binding.target_id
        || attempt.frontier_id != attempt.target_task_binding.frontier_id
        || attempt.target != attempt.target_task_binding.target_id
    {
        return Err("current Attempt does not match its Target binding".to_string());
    }
    if attempt.starting_target_task_binding.source != attempt.target_task_binding.source
        || attempt.starting_target_task_binding.input_root != attempt.target_task_binding.input_root
        || attempt.starting_target_task_binding.packet != attempt.target_task_binding.packet
    {
        return Err("current Attempt Target source, inputs, or packet changed".to_string());
    }
    if attempt.allowed_artifact_classes.is_empty()
        || attempt.allowed_artifact_classes != attempt.task_contract.allowed_artifact_classes
        || !attempt
            .allowed_artifact_classes
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err("current Attempt Artifact classes must be one sorted set".to_string());
    }
    if attempt.budget.max_submissions == 0
        || attempt.budget.max_submissions > DEFAULT_MAX_SUBMISSIONS
        || attempt.budget.max_verifications == 0
        || attempt.budget.max_verifications > DEFAULT_MAX_VERIFICATIONS
        || attempt.budget.max_artifacts == 0
        || attempt.budget.max_artifacts > DEFAULT_MAX_ARTIFACTS
        || attempt.budget.max_artifact_bytes == 0
        || attempt.budget.max_artifact_bytes > DEFAULT_MAX_ARTIFACT_BYTES
        || attempt.usage.submissions > attempt.budget.max_submissions
        || attempt.usage.verifications > attempt.budget.max_verifications
        || attempt.usage.artifacts > attempt.budget.max_artifacts
        || attempt.usage.artifact_bytes > attempt.budget.max_artifact_bytes
    {
        return Err("current Attempt budget or usage is invalid".to_string());
    }
    if attempt.usage.submissions
        != u64::try_from(attempt.usage.registered_submission_ids.len())
            .map_err(|_| "current Attempt Submission count overflowed".to_string())?
        || !attempt
            .usage
            .registered_submission_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err(
            "current Attempt Submission IDs must be a sorted set matching usage".to_string(),
        );
    }
    if attempt.usage.verifications
        != u64::try_from(attempt.usage.registered_verification_record_ids.len())
            .map_err(|_| "current Attempt Verification count overflowed".to_string())?
        || !attempt
            .usage
            .registered_verification_record_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err(
            "current Attempt Verification IDs must be a sorted set matching usage".to_string(),
        );
    }
    if authorization_root(attempt)? != attempt.authorization_root
        || attempt_id(&attempt.authorization_root)? != attempt.attempt_id
    {
        return Err("current Attempt identity does not match its authorization".to_string());
    }
    let created_at = chrono::DateTime::parse_from_rfc3339(&attempt.created_at)
        .map_err(|error| format!("current Attempt created_at: {error}"))?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(&attempt.expires_at)
        .map_err(|error| format!("current Attempt expires_at: {error}"))?;
    if expires_at <= created_at {
        return Err("current Attempt expiry must follow creation".to_string());
    }
    Ok(())
}

fn encode(attempt: &CurrentAttempt) -> Result<Vec<u8>, String> {
    validate(attempt)?;
    let mut bytes = serde_json::to_vec_pretty(attempt)
        .map_err(|error| format!("encode current Attempt: {error}"))?;
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
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse current Attempt {}: {error}", path.display()))?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or("missing");
    if schema != ATTEMPT_SCHEMA {
        return Err(format!(
            "unsupported private Attempt schema {schema}; delete {} and run vela start again",
            path.display()
        ));
    }
    let attempt: CurrentAttempt = serde_json::from_value(value)
        .map_err(|error| format!("parse current Attempt {}: {error}", path.display()))?;
    validate(&attempt)?;
    Ok(attempt)
}

fn refresh_target_binding(
    frontier: &Path,
    path: &Path,
    attempt: &mut CurrentAttempt,
) -> Result<(), String> {
    if vela_edge::target_index::revalidate_current_target_task_binding(
        frontier,
        &attempt.target_task_binding,
    )
    .is_ok()
    {
        return Ok(());
    }
    let repository = crate::current_repository::load_current_repository_at(frontier, true)?;
    if repository.frontier_id != attempt.frontier_id {
        return Err("current Attempt Frontier identity changed".to_string());
    }
    let repository_root = repository.canonical_root()?;
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
    )?
    .ok_or_else(|| "current Attempt Target Index is unavailable".to_string())?;
    let refreshed = vela_edge::target_index::build_current_target_task_binding(
        frontier,
        &assessment,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
        &attempt.target,
    )?;
    if refreshed.frontier_id != attempt.frontier_id
        || refreshed.target_id != attempt.target
        || refreshed.repository.origin_id
            != attempt.starting_target_task_binding.repository.origin_id
        || refreshed.source != attempt.starting_target_task_binding.source
        || refreshed.input_root != attempt.starting_target_task_binding.input_root
        || refreshed.packet != attempt.starting_target_task_binding.packet
    {
        return Err(
            "current Attempt cannot continue after its Target identity, source, inputs, or packet changed; drop it and start the new scope"
                .to_string(),
        );
    }
    attempt.target_task_binding = refreshed;
    write(frontier, attempt)?;
    if attempt_path(frontier, &attempt.target) != path {
        return Err("current Attempt path changed while refreshing its read set".to_string());
    }
    vela_edge::target_index::revalidate_current_target_task_binding(
        frontier,
        &attempt.target_task_binding,
    )
}

fn discover_attempts(frontier: &Path) -> Result<Vec<(PathBuf, CurrentAttempt)>, String> {
    let entries = match fs::read_dir(frontier.join(".vela/work")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("read current Attempt root: {error}")),
    };
    let mut attempts = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("read current Attempt entry: {error}"))?
            .path()
            .join("attempt.json");
        match fs::symlink_metadata(&path) {
            Ok(_) => attempts.push((path.clone(), read(&path)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "inspect current Attempt {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(attempts)
}

fn expires_at(attempt: &CurrentAttempt) -> Result<chrono::DateTime<chrono::FixedOffset>, String> {
    chrono::DateTime::parse_from_rfc3339(&attempt.expires_at)
        .map_err(|error| format!("current Attempt expires_at: {error}"))
}

fn require_live(attempt: &CurrentAttempt) -> Result<(), String> {
    if expires_at(attempt)? <= Utc::now() {
        return Err(format!(
            "current Attempt {} has expired",
            attempt.attempt_id
        ));
    }
    Ok(())
}

fn resolve_attempt(
    frontier: &Path,
    requested_attempt: Option<&str>,
) -> Result<Option<CurrentRoutineAttempt>, String> {
    let Some(requested_attempt) = requested_attempt else {
        return Ok(None);
    };
    let matches = discover_attempts(frontier)?
        .into_iter()
        .filter(|(_, attempt)| attempt.attempt_id == requested_attempt)
        .collect::<Vec<_>>();
    let [(path, discovered)] = matches.as_slice() else {
        return Err(format!(
            "current Attempt {requested_attempt:?} must resolve to exactly one private record; found {}",
            matches.len()
        ));
    };
    let lock = lock_attempt(frontier, &discovered.target)?;
    let mut attempt = read(path)?;
    if attempt.attempt_id != requested_attempt {
        return Err(format!(
            "current Attempt {requested_attempt} changed while acquiring its private lock"
        ));
    }
    require_live(&attempt)?;
    refresh_target_binding(frontier, path, &mut attempt)?;
    Ok(Some(CurrentRoutineAttempt {
        attempt,
        path: path.clone(),
        _lock: lock,
    }))
}

pub(crate) fn resolve_submission_attempt(
    frontier: &Path,
    actor: &str,
    requested_attempt: Option<&str>,
) -> Result<Option<CurrentRoutineAttempt>, String> {
    let resolved = resolve_attempt(frontier, requested_attempt)?;
    if let Some(resolved) = &resolved
        && resolved.attempt.actor != actor
    {
        return Err(format!(
            "current Attempt {} belongs to {}, not {actor}",
            resolved.attempt.attempt_id, resolved.attempt.actor
        ));
    }
    Ok(resolved)
}

pub(crate) fn resolve_verification_attempt(
    frontier: &Path,
    requested_attempt: Option<&str>,
) -> Result<Option<CurrentRoutineAttempt>, String> {
    resolve_attempt(frontier, requested_attempt)
}

pub(crate) fn resolve_verification_reconciliation_attempt(
    frontier: &Path,
    source_attempt: Option<&str>,
) -> Result<Option<CurrentRoutineAttempt>, String> {
    let Some(source_attempt) = source_attempt else {
        return Ok(None);
    };
    let matches = discover_attempts(frontier)?
        .into_iter()
        .filter(|(_, attempt)| attempt.attempt_id == source_attempt)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(None);
    }
    let [(path, discovered)] = matches.as_slice() else {
        return Err(format!(
            "current Attempt {source_attempt:?} must resolve to at most one private record; found {}",
            matches.len()
        ));
    };
    let lock = lock_attempt(frontier, &discovered.target)?;
    let mut attempt = read(path)?;
    if expires_at(&attempt)? <= Utc::now() {
        return Ok(None);
    }
    refresh_target_binding(frontier, path, &mut attempt)?;
    Ok(Some(CurrentRoutineAttempt {
        attempt,
        path: path.clone(),
        _lock: lock,
    }))
}

pub(crate) fn revalidate_routine_attempt(
    frontier: &Path,
    resolved: Option<&CurrentRoutineAttempt>,
) -> Result<(), String> {
    let Some(resolved) = resolved else {
        return Ok(());
    };
    require_live(&resolved.attempt)?;
    vela_edge::target_index::revalidate_current_target_task_binding(
        frontier,
        &resolved.attempt.target_task_binding,
    )
}

pub(crate) fn authorize_submission(
    resolved: Option<&CurrentRoutineAttempt>,
    submission: &SubmissionV1,
    artifact_bytes: u64,
) -> Result<(), String> {
    let Some(resolved) = resolved else {
        return Ok(());
    };
    let attempt = &resolved.attempt;
    let mut denied = submission
        .artifacts
        .iter()
        .filter(|artifact| !attempt.allowed_artifact_classes.contains(&artifact.kind))
        .map(|artifact| artifact.kind.clone())
        .collect::<Vec<_>>();
    denied.sort();
    denied.dedup();
    if !denied.is_empty() {
        return Err(format!(
            "current Attempt {} does not authorize Artifact classes: {}",
            attempt.attempt_id,
            denied.join(", ")
        ));
    }
    if attempt
        .usage
        .registered_submission_ids
        .binary_search(&submission.submission_id)
        .is_ok()
    {
        return Ok(());
    }
    let artifact_count = u64::try_from(submission.artifacts.len())
        .map_err(|_| "Submission Artifact count overflowed".to_string())?;
    let submissions = attempt.usage.submissions.saturating_add(1);
    let artifacts = attempt.usage.artifacts.saturating_add(artifact_count);
    let bytes = attempt.usage.artifact_bytes.saturating_add(artifact_bytes);
    if submissions > attempt.budget.max_submissions
        || artifacts > attempt.budget.max_artifacts
        || bytes > attempt.budget.max_artifact_bytes
    {
        return Err(format!(
            "current Attempt {} budget exhausted: submissions={submissions}/{}, artifacts={artifacts}/{}, artifact_bytes={bytes}/{}",
            attempt.attempt_id,
            attempt.budget.max_submissions,
            attempt.budget.max_artifacts,
            attempt.budget.max_artifact_bytes,
        ));
    }
    Ok(())
}

pub(crate) fn retained_submission_artifact_bytes(
    frontier: &Path,
    submission: &SubmissionV1,
) -> Result<u64, String> {
    let mut total = 0_u64;
    for (index, artifact) in submission.artifacts.iter().enumerate() {
        let digest = artifact
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("Submission Artifact {index} digest is not sha256"))?;
        let path = frontier.join("records/artifacts/sha256").join(digest);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "inspect retained Submission Artifact {index} {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "retained Submission Artifact {index} must be a regular non-symlink file"
            ));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| "retained Submission Artifact-byte usage overflowed".to_string())?;
    }
    Ok(total)
}

pub(crate) fn record_submission_attempt(
    frontier: &Path,
    resolved: Option<CurrentRoutineAttempt>,
    submission: &SubmissionV1,
    artifact_bytes: u64,
) -> Result<(), String> {
    let Some(mut resolved) = resolved else {
        return Ok(());
    };
    authorize_submission(Some(&resolved), submission, artifact_bytes)?;
    let index = match resolved
        .attempt
        .usage
        .registered_submission_ids
        .binary_search(&submission.submission_id)
    {
        Ok(_) => return Ok(()),
        Err(index) => index,
    };
    resolved
        .attempt
        .usage
        .registered_submission_ids
        .insert(index, submission.submission_id.clone());
    resolved.attempt.usage.submissions += 1;
    resolved.attempt.usage.artifacts += u64::try_from(submission.artifacts.len())
        .map_err(|_| "Submission Artifact count overflowed".to_string())?;
    resolved.attempt.usage.artifact_bytes += artifact_bytes;
    write(frontier, &resolved.attempt).map_err(|error| {
        format!(
            "Submission registered but private Attempt progress failed at {}: {error}",
            resolved.path.display()
        )
    })?;
    Ok(())
}

pub(crate) fn authorize_verification(
    resolved: Option<&CurrentRoutineAttempt>,
    verification_record_id: &str,
) -> Result<(), String> {
    let Some(resolved) = resolved else {
        return Ok(());
    };
    let attempt = &resolved.attempt;
    if attempt
        .usage
        .registered_verification_record_ids
        .binary_search_by(|candidate| candidate.as_str().cmp(verification_record_id))
        .is_ok()
    {
        return Ok(());
    }
    if attempt.usage.verifications.saturating_add(1) > attempt.budget.max_verifications {
        return Err(format!(
            "current Attempt {} Verification budget exhausted",
            attempt.attempt_id
        ));
    }
    Ok(())
}

pub(crate) fn record_verification_attempt(
    frontier: &Path,
    resolved: Option<CurrentRoutineAttempt>,
    verification_record_id: &str,
) -> Result<(), String> {
    let Some(mut resolved) = resolved else {
        return Ok(());
    };
    authorize_verification(Some(&resolved), verification_record_id)?;
    let index = match resolved
        .attempt
        .usage
        .registered_verification_record_ids
        .binary_search_by(|candidate| candidate.as_str().cmp(verification_record_id))
    {
        Ok(_) => return Ok(()),
        Err(index) => index,
    };
    resolved
        .attempt
        .usage
        .registered_verification_record_ids
        .insert(index, verification_record_id.to_string());
    resolved.attempt.usage.verifications += 1;
    write(frontier, &resolved.attempt).map_err(|error| {
        format!(
            "Verification imported but private Attempt progress failed at {}: {error}",
            resolved.path.display()
        )
    })?;
    Ok(())
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
    file: fs::File,
}

impl Drop for AttemptLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
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
    match file.try_lock() {
        Ok(()) => Ok(AttemptLock { file }),
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
    let directory = safe_session_dir(frontier, &attempt.target);
    ensure_private_directory(&frontier.join(".vela"), "Frontier private directory")?;
    ensure_private_directory(&frontier.join(".vela/work"), "Attempt root")?;
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
    let temporary = directory.join(format!(".attempt-{}.tmp", std::process::id()));
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

fn artifact_classes(packet: &Value, requested: &[String]) -> Result<Vec<String>, String> {
    let mut classes = requested.to_vec();
    classes.extend(
        packet
            .get("allowed_outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|output| {
                output
                    .get("kind")
                    .or_else(|| output.get("type"))
                    .and_then(Value::as_str)
            })
            .map(ToString::to_string),
    );
    if let Some(kind) = packet
        .pointer("/output_contract/kind")
        .and_then(Value::as_str)
    {
        classes.push(kind.to_string());
    }
    if classes.is_empty() {
        classes.push("other".to_string());
    }
    for class in &classes {
        if class.is_empty()
            || class.len() > 128
            || class.contains("..")
            || class.starts_with('/')
            || !class.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'_' | b'/' | b'+' | b':' | b'-')
            })
        {
            return Err(format!(
                "Attempt Artifact class {class:?} must be a portable identifier"
            ));
        }
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}

fn result(attempt: &CurrentAttempt, path: &Path, idempotent: bool) -> Value {
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
        "authorization": {
            "root": attempt.authorization_root,
            "allowed_operations": ["submission_register", "verification_import"],
            "allowed_artifact_classes": attempt.allowed_artifact_classes,
            "budget": attempt.budget,
            "usage": attempt.usage,
            "authority_ceiling": "pending_review",
        },
        "starting_roots": {
            "origin": attempt.starting_target_task_binding.repository.origin_id,
            "repository": attempt.starting_target_task_binding.repository.repository_root,
            "target_index": attempt.starting_target_task_binding.target_index_root,
            "task_contract": attempt.task_contract_root,
            "git_commit": attempt.starting_target_task_binding.claim_read_set.git_commit,
            "git_tree": attempt.starting_target_task_binding.claim_read_set.git_tree,
        },
        "current_read_set": {
            "repository": attempt.target_task_binding.repository.repository_root,
            "target_index": attempt.target_task_binding.target_index_root,
            "git_commit": attempt.target_task_binding.claim_read_set.git_commit,
            "git_tree": attempt.target_task_binding.claim_read_set.git_tree,
        },
        "task": attempt.task_contract,
        "packet": attempt.target_task_binding.packet,
        "briefing": attempt.briefing,
        "canonical_write": false,
        "authority_key_read": false,
        "next_command": format!(
            "vela submit --attempt {} --claim <scoped-result> --type <type> --replayability <class> --artifact <path>:<kind> --caveat <limit> --as {} --json",
            attempt.attempt_id, attempt.actor
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn open(
    frontier: &Path,
    target_id: &str,
    actor: &str,
    ttl_seconds: u64,
    requested_artifact_classes: &[String],
    max_submissions: u64,
    max_verifications: u64,
    max_artifacts: u64,
    max_artifact_bytes: u64,
) -> Result<Value, String> {
    if ttl_seconds == 0 || ttl_seconds > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
        return Err(format!(
            "private Attempt TTL must be between 1 and {} seconds",
            vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
        ));
    }
    let budget = AttemptBudget {
        max_submissions,
        max_verifications,
        max_artifacts,
        max_artifact_bytes,
    };
    if max_submissions == 0
        || max_submissions > DEFAULT_MAX_SUBMISSIONS
        || max_verifications == 0
        || max_verifications > DEFAULT_MAX_VERIFICATIONS
        || max_artifacts == 0
        || max_artifacts > DEFAULT_MAX_ARTIFACTS
        || max_artifact_bytes == 0
        || max_artifact_bytes > DEFAULT_MAX_ARTIFACT_BYTES
    {
        return Err(
            "private Attempt budgets must be positive and within built-in ceilings".to_string(),
        );
    }
    let repository = crate::current_repository::load_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.origin_id,
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
        &repository.origin_id,
        &repository_root,
        target_id,
    )?;
    let packet = assessment
        .packet_value(target_id)
        .cloned()
        .ok_or_else(|| format!("current Target {target_id:?} has no verified packet"))?;
    let classes = artifact_classes(&packet, requested_artifact_classes)?;
    let _lock = lock_attempt(frontier, target_id)?;
    let path = attempt_path(frontier, target_id);
    if path.is_file() {
        let mut existing = read(&path)?;
        if existing.actor != actor {
            return Err(format!(
                "target {target_id} already has a private Attempt owned by {}",
                existing.actor
            ));
        }
        refresh_target_binding(frontier, &path, &mut existing)?;
        require_live(&existing)?;
        if existing.allowed_artifact_classes != classes || existing.budget != budget {
            return Err(format!(
                "target {target_id} already has a private Attempt with different scope or budget; drop it before starting a replacement"
            ));
        }
        return Ok(result(&existing, &path, true));
    }
    let task_contract = task_contract(&packet, &target, classes.clone());
    let task_contract_root = canonical_root(&task_contract)?;
    let created_at = Utc::now();
    let ttl = i64::try_from(ttl_seconds)
        .map_err(|_| "private Attempt TTL exceeds the supported range".to_string())?;
    let expires_at = created_at
        .checked_add_signed(Duration::seconds(ttl))
        .ok_or_else(|| "private Attempt expiry overflowed".to_string())?;
    let mut attempt = CurrentAttempt {
        schema: ATTEMPT_SCHEMA.to_string(),
        attempt_id: String::new(),
        authorization_root: String::new(),
        frontier_id: repository.frontier_id,
        target: target_id.to_string(),
        actor: actor.to_string(),
        created_at: created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        expires_at: expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        allowed_artifact_classes: classes,
        budget,
        usage: AttemptUsage {
            submissions: 0,
            verifications: 0,
            artifacts: 0,
            artifact_bytes: 0,
            registered_submission_ids: Vec::new(),
            registered_verification_record_ids: Vec::new(),
        },
        task_contract,
        task_contract_root,
        starting_target_task_binding: binding.clone(),
        target_task_binding: binding,
        briefing: json!({
            "schema": "vela.work-briefing.v3",
            "target": target,
            "packet": packet,
        }),
    };
    attempt.authorization_root = authorization_root(&attempt)?;
    attempt.attempt_id = attempt_id(&attempt.authorization_root)?;
    let path = write(frontier, &attempt)?;
    Ok(result(&attempt, &path, false))
}

fn drop_attempt(frontier: &Path, target: &str, actor: &str, reason: &str) -> Result<Value, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("start --drop requires a non-empty reason".to_string());
    }
    crate::current_repository::load_current_repository_at(frontier, true)?;
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
    let _ = fs::remove_dir(safe_session_dir(frontier, target));
    Ok(json!({
        "schema": "vela.attempt-drop.v3",
        "ok": true,
        "command": "start.drop",
        "attempt_id": attempt.attempt_id,
        "target_id": target,
        "actor": actor,
        "reason": reason,
        "canonical_write": false,
        "authority_key_read": false,
    }))
}

fn attempt_list_entry(attempt: CurrentAttempt, path: &Path) -> Value {
    json!({
        "attempt_id": attempt.attempt_id,
        "target_id": attempt.target,
        "actor": attempt.actor,
        "authorization_root": attempt.authorization_root,
        "expires_at": attempt.expires_at,
        "allowed_operations": ["submission_register", "verification_import"],
        "allowed_artifact_classes": attempt.allowed_artifact_classes,
        "authority_ceiling": "pending_review",
        "task_contract_root": attempt.task_contract_root,
        "target_packet_sha256": attempt.target_task_binding.packet.sha256,
        "usage": attempt.usage,
        "budget": attempt.budget,
        "path": path.display().to_string(),
    })
}

pub(crate) fn project_attempts(frontier: &Path) -> Result<Value, String> {
    crate::current_repository::load_current_repository_at(frontier, true)?;
    let mut attempts = discover_attempts(frontier)?
        .into_iter()
        .map(|(path, attempt)| attempt_list_entry(attempt, &path))
        .collect::<Vec<_>>();
    attempts.sort_by(|left, right| left["target_id"].as_str().cmp(&right["target_id"].as_str()));
    Ok(json!({
        "schema": "vela.attempt-list.v3",
        "ok": true,
        "command": "start",
        "attempts": attempts,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_start(
    frontier: &Path,
    target: &str,
    ttl: Option<u64>,
    artifact_classes: &[String],
    max_submissions: Option<u64>,
    max_verifications: Option<u64>,
    max_artifacts: Option<u64>,
    max_artifact_bytes: Option<u64>,
    drop_it: bool,
    reason: Option<&str>,
    actor: &str,
    json_out: bool,
) {
    crate::ui::set_mode("start", json_out);
    let result = match drop_it {
        true => drop_attempt(
            frontier,
            target,
            actor,
            reason.unwrap_or("producer abandoned the private Attempt"),
        ),
        false if reason.is_some() => Err("--reason is valid only with start --drop".to_string()),
        false => {
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
            open(
                frontier,
                target,
                actor,
                ttl,
                artifact_classes,
                max_submissions.unwrap_or(DEFAULT_MAX_SUBMISSIONS),
                max_verifications.unwrap_or(DEFAULT_MAX_VERIFICATIONS),
                max_artifacts.unwrap_or(DEFAULT_MAX_ARTIFACTS),
                max_artifact_bytes.unwrap_or(DEFAULT_MAX_ARTIFACT_BYTES),
            )
        }
    }
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&result);
    } else {
        println!(
            "{} · {}",
            result["command"].as_str().unwrap_or("start"),
            result["target_id"].as_str().unwrap_or("unavailable")
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

    #[test]
    fn attempt_identity_changes_with_authorization() {
        let first = attempt_id(&format!("sha256:{}", "1".repeat(64))).unwrap();
        let second = attempt_id(&format!("sha256:{}", "2".repeat(64))).unwrap();
        assert_ne!(first, second);
        assert!(first.starts_with("vat_"));
    }

    #[test]
    fn artifact_classes_are_sorted_and_bounded() {
        let classes = artifact_classes(
            &json!({"allowed_outputs": [{"kind": "witness"}]}),
            &["source-diff".to_string(), "witness".to_string()],
        )
        .unwrap();
        assert_eq!(classes, ["source-diff", "witness"]);
        assert!(artifact_classes(&json!({}), &["../bad".to_string()]).is_err());
    }
}
