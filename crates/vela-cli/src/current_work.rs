//! Private producer work for current repository origins.
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
use vela_protocol::submission_v1::SubmissionV1;

use crate::cli::safe_text;

const ATTEMPT_SCHEMA: &str = "vela.attempt.v4";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v3";
const ATTEMPT_MAX_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_MAX_SUBMISSIONS: u64 = 16;
const DEFAULT_MAX_ARTIFACTS: u64 = 64;
const DEFAULT_MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const CONSEQUENCE_EVIDENCE_ONLY: &str = "evidence_only";
const CONSEQUENCE_PENDING_REVIEW: &str = "pending_review";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentBuildIdentity {
    program: String,
    version: String,
    binary_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAttemptBudget {
    max_submissions: u64,
    max_artifacts: u64,
    max_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAttemptUsage {
    submissions: u64,
    artifacts: u64,
    artifact_bytes: u64,
    registered_submission_ids: Vec<String>,
}

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
pub(crate) struct CurrentAttempt {
    pub(crate) schema: String,
    pub(crate) attempt_id: String,
    authorization_root: String,
    pub(crate) frontier_id: String,
    pub(crate) target: String,
    pub(crate) actor: String,
    pub(crate) created_at: String,
    pub(crate) expires_at: String,
    controller_build: CurrentBuildIdentity,
    runner_build_root: String,
    allowed_operations: Vec<String>,
    allowed_artifact_classes: Vec<String>,
    budget: CurrentAttemptBudget,
    usage: CurrentAttemptUsage,
    consequence_ceiling: String,
    task_contract: CurrentTaskContract,
    pub(crate) task_contract_root: String,
    starting_target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    pub(crate) target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    briefing: Value,
}

pub(crate) struct CurrentSubmissionAttempt {
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
            "Produce bounded evidence for this exact target; each registered Submission must remain within the Attempt authorization."
                .to_string(),
        allowed_actions: vec![
            "inspect the pinned repository, packet, and task contract".to_string(),
            "run frozen verifiers and private search or experiment loops".to_string(),
            "create bounded evidence artifacts and authorized Submissions".to_string(),
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

fn require_sha256_root(label: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{label} must be a full sha256 root"));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} must be a full sha256 root"));
    }
    Ok(())
}

fn controller_build() -> Result<CurrentBuildIdentity, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    Ok(CurrentBuildIdentity {
        program: "vela-cli".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        binary_sha256: crate::authority_transaction::execution_binary_sha256(&executable)?,
    })
}

fn authorization_root(attempt: &CurrentAttempt) -> Result<String, String> {
    canonical_root(&json!({
        "schema": "vela.attempt-authorization.internal.v1",
        "frontier_id": attempt.frontier_id,
        "target": attempt.target,
        "actor": attempt.actor,
        "created_at": attempt.created_at,
        "expires_at": attempt.expires_at,
        "controller_build": attempt.controller_build,
        "runner_build_root": attempt.runner_build_root,
        "allowed_operations": attempt.allowed_operations,
        "allowed_artifact_classes": attempt.allowed_artifact_classes,
        "budget": attempt.budget,
        "consequence_ceiling": attempt.consequence_ceiling,
        "task_contract_root": attempt.task_contract_root,
        "starting_target_task_binding_root": attempt.starting_target_task_binding.binding_root,
    }))
}

fn attempt_id(authorization_root: &str) -> Result<String, String> {
    let preimage = json!({
        "schema": ATTEMPT_SCHEMA,
        "authorization_root": authorization_root,
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
    attempt.starting_target_task_binding.validate()?;
    attempt.target_task_binding.validate()?;
    if attempt.frontier_id != attempt.starting_target_task_binding.frontier_id
        || attempt.target != attempt.starting_target_task_binding.target_id
        || attempt.frontier_id != attempt.target_task_binding.frontier_id
        || attempt.target != attempt.target_task_binding.target_id
    {
        return Err("current Attempt does not match its target binding".to_string());
    }
    if attempt.starting_target_task_binding.source != attempt.target_task_binding.source
        || attempt.starting_target_task_binding.input_root != attempt.target_task_binding.input_root
        || attempt.starting_target_task_binding.packet != attempt.target_task_binding.packet
    {
        return Err(
            "current Attempt target source, inputs, or packet changed outside its authorization"
                .to_string(),
        );
    }
    require_sha256_root(
        "current Attempt controller binary",
        &attempt.controller_build.binary_sha256,
    )?;
    require_sha256_root("current Attempt runner build", &attempt.runner_build_root)?;
    if attempt.allowed_operations.is_empty()
        || !attempt
            .allowed_operations
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err("current Attempt operations must be a non-empty sorted set".to_string());
    }
    if attempt.allowed_artifact_classes.is_empty()
        || !attempt
            .allowed_artifact_classes
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err("current Attempt Artifact classes must be a non-empty sorted set".to_string());
    }
    if !matches!(
        attempt.consequence_ceiling.as_str(),
        CONSEQUENCE_EVIDENCE_ONLY | CONSEQUENCE_PENDING_REVIEW
    ) {
        return Err("current Attempt has an unsupported consequence ceiling".to_string());
    }
    if attempt.budget.max_submissions == 0
        || attempt.budget.max_artifacts == 0
        || attempt.budget.max_artifact_bytes == 0
        || attempt.usage.submissions > attempt.budget.max_submissions
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
            "current Attempt registered Submission IDs must be a sorted set matching usage"
                .to_string(),
        );
    }
    if authorization_root(attempt)? != attempt.authorization_root {
        return Err("current Attempt authorization does not match its root".to_string());
    }
    let expected = attempt_id(&attempt.authorization_root)?;
    if attempt.attempt_id != expected {
        return Err("current Attempt identity does not match its closed preimage".to_string());
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
    if refreshed.source != attempt.starting_target_task_binding.source
        || refreshed.input_root != attempt.starting_target_task_binding.input_root
        || refreshed.packet != attempt.starting_target_task_binding.packet
    {
        return Err(
            "current Attempt cannot continue after its Target source, inputs, or packet changed; revoke it and start the new scope"
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

pub(crate) fn resolve_submission_attempt(
    frontier: &Path,
    actor: &str,
    requested_attempt: Option<&str>,
) -> Result<Option<CurrentSubmissionAttempt>, String> {
    let Some(requested_attempt) = requested_attempt else {
        return Ok(None);
    };
    let work = frontier.join(".vela/work");
    let matches = fs::read_dir(&work)
        .map_err(|error| format!("read current Attempt root {}: {error}", work.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("attempt.json"))
        .filter(|path| path.is_file())
        .map(|path| read(&path).map(|attempt| (path, attempt)))
        .collect::<Result<Vec<_>, String>>()?
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
    if attempt.actor != actor {
        return Err(format!(
            "current Attempt {requested_attempt} belongs to {}, not {actor}",
            attempt.actor
        ));
    }
    let expires_at = chrono::DateTime::parse_from_rfc3339(&attempt.expires_at)
        .map_err(|error| format!("current Attempt expires_at: {error}"))?;
    if expires_at <= Utc::now() {
        return Err(format!("current Attempt {requested_attempt} has expired"));
    }
    refresh_target_binding(frontier, path, &mut attempt)?;
    Ok(Some(CurrentSubmissionAttempt {
        attempt,
        path: path.clone(),
        _lock: lock,
    }))
}

pub(crate) fn authorize_submission(
    resolved: Option<&CurrentSubmissionAttempt>,
    submission: &SubmissionV1,
    artifact_bytes: u64,
) -> Result<(), String> {
    let Some(resolved) = resolved else {
        return Ok(());
    };
    let attempt = &resolved.attempt;
    if attempt.consequence_ceiling != CONSEQUENCE_PENDING_REVIEW
        || !attempt
            .allowed_operations
            .iter()
            .any(|operation| operation == "submission_register")
    {
        return Err(format!(
            "current Attempt {} permits evidence only and cannot create a pending Proposal",
            attempt.attempt_id
        ));
    }
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
    let next_submissions = attempt
        .usage
        .submissions
        .checked_add(1)
        .ok_or_else(|| "current Attempt Submission usage overflowed".to_string())?;
    let next_artifacts = attempt
        .usage
        .artifacts
        .checked_add(artifact_count)
        .ok_or_else(|| "current Attempt Artifact usage overflowed".to_string())?;
    let next_artifact_bytes = attempt
        .usage
        .artifact_bytes
        .checked_add(artifact_bytes)
        .ok_or_else(|| "current Attempt Artifact-byte usage overflowed".to_string())?;
    if next_submissions > attempt.budget.max_submissions
        || next_artifacts > attempt.budget.max_artifacts
        || next_artifact_bytes > attempt.budget.max_artifact_bytes
    {
        return Err(format!(
            "current Attempt {} budget exhausted: requested submissions={next_submissions}/{}, artifacts={next_artifacts}/{}, artifact_bytes={next_artifact_bytes}/{}",
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
            .ok_or_else(|| format!("Submission artifact {index} digest is not sha256"))?;
        let path = frontier.join("records/artifacts/sha256").join(digest);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "inspect retained Submission artifact {index} {}: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "retained Submission artifact {index} must be a regular non-symlink file"
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
    resolved: Option<CurrentSubmissionAttempt>,
    submission: &SubmissionV1,
    artifact_bytes: u64,
) -> Result<(), String> {
    let Some(mut resolved) = resolved else {
        return Ok(());
    };
    authorize_submission(Some(&resolved), submission, artifact_bytes)?;
    match resolved
        .attempt
        .usage
        .registered_submission_ids
        .binary_search(&submission.submission_id)
    {
        Ok(_) => return Ok(()),
        Err(index) => {
            resolved
                .attempt
                .usage
                .registered_submission_ids
                .insert(index, submission.submission_id.clone());
        }
    }
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

impl Drop for AttemptLock {
    fn drop(&mut self) {
        let _ = self._file.unlock();
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
        .get("output_contract")
        .and_then(|contract| contract.get("kind"))
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
            || !class.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'_' | b'/' | b'+' | b':' | b'-')
            })
        {
            return Err(format!(
                "Attempt Artifact class {class:?} must be 1..128 portable identifier characters"
            ));
        }
    }
    classes.sort();
    classes.dedup();
    Ok(classes)
}

fn allowed_operations(consequence_ceiling: &str) -> Result<Vec<String>, String> {
    let mut operations = vec![
        "inspect".to_string(),
        "run_tool".to_string(),
        "write_private_artifact".to_string(),
    ];
    match consequence_ceiling {
        CONSEQUENCE_EVIDENCE_ONLY => {}
        CONSEQUENCE_PENDING_REVIEW => {
            operations.push("submission_author".to_string());
            operations.push("submission_register".to_string());
        }
        _ => {
            return Err(format!(
                "consequence ceiling must be {CONSEQUENCE_EVIDENCE_ONLY} or {CONSEQUENCE_PENDING_REVIEW}"
            ));
        }
    }
    operations.sort();
    Ok(operations)
}

fn result(attempt: &CurrentAttempt, path: &Path, idempotent: bool) -> Value {
    let packet = &attempt.target_task_binding.packet;
    let next_command = if attempt.consequence_ceiling == CONSEQUENCE_PENDING_REVIEW {
        Value::String(format!(
            "vela submit --attempt {} --claim <scoped-result> --type <type> --replayability <class> --artifact <path>:<kind> --caveat <limit> --as {} --json",
            attempt.attempt_id, attempt.actor
        ))
    } else {
        Value::Null
    };
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
            "controller_build": attempt.controller_build,
            "runner_build_root": attempt.runner_build_root,
            "allowed_operations": attempt.allowed_operations,
            "allowed_artifact_classes": attempt.allowed_artifact_classes,
            "budget": attempt.budget,
            "usage": attempt.usage,
            "consequence_ceiling": attempt.consequence_ceiling,
        },
        "starting_roots": {
            "origin": attempt.target_task_binding.repository.origin_id,
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
        "next_command": next_command,
    })
}

#[allow(clippy::too_many_arguments)]
fn open(
    frontier: &Path,
    target_id: &str,
    actor: &str,
    ttl_seconds: u64,
    runner_build_root: Option<&str>,
    requested_artifact_classes: &[String],
    max_submissions: u64,
    max_artifacts: u64,
    max_artifact_bytes: u64,
    consequence_ceiling: &str,
) -> Result<Value, String> {
    if ttl_seconds == 0 || ttl_seconds > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
        return Err(format!(
            "private Attempt TTL must be between 1 and {} seconds",
            vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
        ));
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
        .ok_or_else(|| format!("current target {target_id:?} has no verified packet"))?;
    let controller_build = controller_build()?;
    let runner_build_root = runner_build_root
        .unwrap_or(&controller_build.binary_sha256)
        .to_string();
    require_sha256_root("runner build", &runner_build_root)?;
    let operations = allowed_operations(consequence_ceiling)?;
    if max_submissions == 0 || max_artifacts == 0 || max_artifact_bytes == 0 {
        return Err("private Attempt budgets must be positive".to_string());
    }
    if max_submissions > DEFAULT_MAX_SUBMISSIONS
        || max_artifacts > DEFAULT_MAX_ARTIFACTS
        || max_artifact_bytes > DEFAULT_MAX_ARTIFACT_BYTES
    {
        return Err(format!(
            "private Attempt budgets may only narrow the defaults: submissions<={DEFAULT_MAX_SUBMISSIONS}, artifacts<={DEFAULT_MAX_ARTIFACTS}, artifact_bytes<={DEFAULT_MAX_ARTIFACT_BYTES}"
        ));
    }
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
        let expires_at = chrono::DateTime::parse_from_rfc3339(&existing.expires_at)
            .map_err(|error| format!("current Attempt expires_at: {error}"))?;
        if expires_at <= Utc::now() {
            return Err(format!(
                "target {target_id} has an expired private Attempt; remove it with `vela start {target_id} --drop --as {actor} --reason <reason>`"
            ));
        }
        let requested_classes = artifact_classes(&packet, requested_artifact_classes)?;
        let requested_budget = CurrentAttemptBudget {
            max_submissions,
            max_artifacts,
            max_artifact_bytes,
        };
        if existing.controller_build != controller_build
            || existing.runner_build_root != runner_build_root
            || existing.allowed_operations != operations
            || existing.allowed_artifact_classes != requested_classes
            || existing.budget != requested_budget
            || existing.consequence_ceiling != consequence_ceiling
        {
            return Err(format!(
                "target {target_id} already has a private Attempt with different authorization; revoke it with `vela start {target_id} --drop --as {actor} --reason <reason>`"
            ));
        }
        return Ok(result(&existing, &path, true));
    }
    let task_contract = contract(&packet, &target);
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
        controller_build,
        runner_build_root,
        allowed_operations: operations,
        allowed_artifact_classes: artifact_classes(&packet, requested_artifact_classes)?,
        budget: CurrentAttemptBudget {
            max_submissions,
            max_artifacts,
            max_artifact_bytes,
        },
        usage: CurrentAttemptUsage {
            submissions: 0,
            artifacts: 0,
            artifact_bytes: 0,
            registered_submission_ids: Vec::new(),
        },
        consequence_ceiling: consequence_ceiling.to_string(),
        task_contract,
        task_contract_root,
        starting_target_task_binding: binding.clone(),
        target_task_binding: binding,
        briefing: json!({
            "schema": "vela.work-briefing.v2",
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
    }))
}

fn attempt_list_entry(attempt: CurrentAttempt, path: &Path) -> Value {
    json!({
        "attempt_id": attempt.attempt_id,
        "target_id": attempt.target,
        "actor": attempt.actor,
        "authorization_root": attempt.authorization_root,
        "expires_at": attempt.expires_at,
        "allowed_operations": attempt.allowed_operations,
        "allowed_artifact_classes": attempt.allowed_artifact_classes,
        "consequence_ceiling": attempt.consequence_ceiling,
        "task_contract_root": attempt.task_contract_root,
        "usage": attempt.usage,
        "budget": attempt.budget,
        "path": path.display().to_string(),
    })
}

pub(crate) fn project_attempts(frontier: &Path) -> Result<Value, String> {
    crate::current_repository::load_current_repository_at(frontier, true)?;
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
            Ok(attempt_list_entry(attempt, &path))
        })
        .collect::<Result<Vec<_>, String>>()?;
    attempts.sort_by(|left, right| left["target_id"].as_str().cmp(&right["target_id"].as_str()));
    Ok(json!({
        "schema": "vela.attempt-list.v2",
        "ok": true,
        "command": "start",
        "attempts": attempts,
    }))
}

pub(crate) fn cmd_start(
    frontier: &Path,
    target: &str,
    ttl: Option<u64>,
    runner_build_root: Option<&str>,
    artifact_classes: &[String],
    max_submissions: Option<u64>,
    max_artifacts: Option<u64>,
    max_artifact_bytes: Option<u64>,
    consequence_ceiling: &str,
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
                runner_build_root,
                artifact_classes,
                max_submissions.unwrap_or(DEFAULT_MAX_SUBMISSIONS),
                max_artifacts.unwrap_or(DEFAULT_MAX_ARTIFACTS),
                max_artifact_bytes.unwrap_or(DEFAULT_MAX_ARTIFACT_BYTES),
                consequence_ceiling,
            )
        }
    }
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&result);
    } else {
        let command = result["command"].as_str().unwrap_or("start");
        println!(
            "{command} · {}",
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
    use ed25519_dalek::SigningKey;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::repository_inputs::GitObjectFormat;
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };

    #[test]
    fn current_attempt_identity_binds_actor_target_contract_and_offer() {
        let first = attempt_id(&format!("sha256:{}", "1".repeat(64))).unwrap();
        let second = attempt_id(&format!("sha256:{}", "2".repeat(64))).unwrap();
        assert_ne!(first, second);
        assert!(first.starts_with("vat_"));
    }

    #[test]
    fn current_attempt_round_trips_as_private_state_without_event_fields() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join(".vela")).unwrap();
        let mut binding = vela_edge::target_index::TargetTaskBindingV3 {
            schema: vela_edge::target_index::TARGET_TASK_BINDING_SCHEMA_V3.to_string(),
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
            repository: vela_edge::target_index::TargetIndexRepositoryV4 {
                origin_id: "vro_1234567890abcdef".to_string(),
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
        let mut attempt = CurrentAttempt {
            schema: ATTEMPT_SCHEMA.to_string(),
            attempt_id: String::new(),
            authorization_root: String::new(),
            frontier_id: binding.frontier_id.clone(),
            target: binding.target_id.clone(),
            actor: "agent:codex".to_string(),
            created_at: "2026-07-27T00:00:00Z".to_string(),
            expires_at: "2026-07-28T00:00:00Z".to_string(),
            controller_build: CurrentBuildIdentity {
                program: "vela-cli".to_string(),
                version: "0.950.1".to_string(),
                binary_sha256: format!("sha256:{}", "9".repeat(64)),
            },
            runner_build_root: format!("sha256:{}", "a".repeat(64)),
            allowed_operations: vec![
                "inspect".to_string(),
                "submission_author".to_string(),
                "submission_register".to_string(),
            ],
            allowed_artifact_classes: vec!["witness".to_string()],
            budget: CurrentAttemptBudget {
                max_submissions: 1,
                max_artifacts: 8,
                max_artifact_bytes: 1024,
            },
            usage: CurrentAttemptUsage {
                submissions: 0,
                artifacts: 0,
                artifact_bytes: 0,
                registered_submission_ids: Vec::new(),
            },
            consequence_ceiling: CONSEQUENCE_PENDING_REVIEW.to_string(),
            task_contract,
            task_contract_root,
            starting_target_task_binding: binding.clone(),
            target_task_binding: binding,
            briefing: json!({"schema": "vela.work-briefing.v2"}),
        };
        attempt.authorization_root = authorization_root(&attempt).unwrap();
        attempt.attempt_id = attempt_id(&attempt.authorization_root).unwrap();
        let path = write(directory.path(), &attempt).unwrap();
        let decoded = read(&path).unwrap();
        assert_eq!(decoded.attempt_id, attempt.attempt_id);
        let projected = attempt_list_entry(decoded.clone(), &path);
        assert_eq!(projected["authorization_root"], attempt.authorization_root);
        assert_eq!(
            projected["allowed_operations"],
            json!(["inspect", "submission_author", "submission_register"])
        );
        assert_eq!(projected["allowed_artifact_classes"], json!(["witness"]));
        assert_eq!(projected["consequence_ceiling"], CONSEQUENCE_PENDING_REVIEW);
        assert_eq!(projected["task_contract_root"], attempt.task_contract_root);
        assert_eq!(projected["budget"]["max_submissions"], 1);
        assert_eq!(projected["expires_at"], "2026-07-28T00:00:00Z");
        let encoded = String::from_utf8(fs::read(path).unwrap()).unwrap();
        assert!(!encoded.contains("event_log_root"));
        assert!(!encoded.contains("claim_event_id"));
        assert!(!encoded.contains("signature"));

        let submission_for = |assertion: &str| {
            let key = SigningKey::from_bytes(&[41_u8; 32]);
            let identity = IdentityBinding::build(
                IdentityBindingDraft {
                    actor_id: "agent:codex".to_string(),
                    actor_class: ActorClass::Agent,
                    created_at: "2026-07-27T00:00:00Z".to_string(),
                },
                &key,
            )
            .unwrap();
            SubmissionV1::build(
                SubmissionDraft {
                    claim: SubmissionClaim {
                        assertion: assertion.to_string(),
                        claim_type: "computational".to_string(),
                        conditions: vec!["bounded fixture".to_string()],
                    },
                    artifacts: vec![SubmissionArtifact {
                        kind: "witness".to_string(),
                        path: "artifacts/witness.json".to_string(),
                        digest: format!("sha256:{}", "b".repeat(64)),
                    }],
                    caveats: vec!["fixture only".to_string()],
                    replayability: "exact".to_string(),
                    producer_checks: Vec::new(),
                    verification_requirements: Vec::new(),
                    requested_change: RequestedChange {
                        kind: "add_claim".to_string(),
                        target: None,
                    },
                    provenance: SubmissionProvenance {
                        producer: "agent:codex".to_string(),
                        source_system: "vela-cli-test".to_string(),
                        source_attempt: Some(attempt.attempt_id.clone()),
                        source_run: None,
                        emitted_at: "2026-07-27T00:00:00Z".to_string(),
                    },
                    execution_binding: None,
                },
                identity,
                &key,
            )
            .unwrap()
        };
        let first_submission = submission_for("First bounded fixture.");
        record_submission_attempt(
            directory.path(),
            Some(CurrentSubmissionAttempt {
                attempt: read(&attempt_path(directory.path(), "erdos:1056")).unwrap(),
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            &first_submission,
            64,
        )
        .unwrap();
        let retained = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_eq!(retained.usage.submissions, 1);
        assert_eq!(retained.usage.artifacts, 1);
        assert_eq!(retained.usage.artifact_bytes, 64);
        assert!(attempt_path(directory.path(), "erdos:1056").is_file());

        record_submission_attempt(
            directory.path(),
            Some(CurrentSubmissionAttempt {
                attempt: retained,
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            &first_submission,
            64,
        )
        .unwrap();
        let retained = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_eq!(retained.usage.submissions, 1);

        let second_submission = submission_for("Second bounded fixture.");
        let error = authorize_submission(
            Some(&CurrentSubmissionAttempt {
                attempt: retained,
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            &second_submission,
            64,
        )
        .unwrap_err();
        assert!(error.contains("budget exhausted"));
    }
}
