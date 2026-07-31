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
use sha2::{Digest, Sha256};
use vela_protocol::submission_v1::SubmissionV1;

use crate::cli::safe_text;

const ATTEMPT_SCHEMA: &str = "vela.attempt.v8";
const TASK_CONTRACT_SCHEMA: &str = "vela.task-contract.internal.v3";
const ATTEMPT_MAX_BYTES: usize = 2 * 1024 * 1024;
const EXECUTION_BUNDLE_MAX_BYTES: u64 = 8 * 1024 * 1024;
const AGENT_RUN_FILE_MAX_BYTES: u64 = 8 * 1024 * 1024;
const AGENT_HELPER_OUTPUT_MAX_BYTES: u64 = 64 * 1024;
const DEFAULT_MAX_RUNS: u64 = 16;
const MAX_MAX_RUNS: u64 = 64;
const DEFAULT_MAX_SUBMISSIONS: u64 = 16;
const DEFAULT_MAX_VERIFICATIONS: u64 = 16;
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
    max_runs: u64,
    max_submissions: u64,
    max_verifications: u64,
    max_artifacts: u64,
    max_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAttemptUsage {
    runs: u64,
    submissions: u64,
    verifications: u64,
    artifacts: u64,
    artifact_bytes: u64,
    registered_submission_ids: Vec<String>,
    registered_verification_record_ids: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentExecutionBundle {
    schema: String,
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAgentRunReceipt {
    schema: String,
    receipt_root: String,
    run_number: u64,
    previous_receipt_root: Option<String>,
    result: AgentHelperRunOutput,
    helper_output_size: u64,
    helper_output_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAgentRunReservation {
    run_number: u64,
    request_root: String,
    /// The exact Target task binding used to construct this Run request.
    ///
    /// A private v8 reservation created before this field is conservatively
    /// checked against the Attempt's starting binding. Any later-binding
    /// receipt therefore still fails closed rather than being guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_task_binding: Option<vela_edge::target_index::TargetTaskBindingV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CurrentAgentRunSubmissionLink {
    run_id: String,
    receipt_root: String,
    submission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperRunOutput {
    schema: String,
    ok: bool,
    command: String,
    effect: String,
    authority: String,
    attempt_id: String,
    request_root: String,
    target: AgentHelperTarget,
    execution_bundle_root: String,
    source_state: AgentHelperSourceState,
    run: AgentHelperRootedFile,
    evidence_manifest: AgentHelperEvidenceManifest,
    candidate: AgentHelperCandidate,
    verifier: AgentHelperVerifier,
    reproduction: AgentHelperReproduction,
    usage: AgentHelperUsage,
    submission: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperTarget {
    id: String,
    binding_root: String,
    target_index_root: String,
    input_root: String,
    packet_root: String,
    source: AgentHelperGitObject,
    claim_read_set: AgentHelperGitObject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperGitObject {
    git_object_format: String,
    git_commit: String,
    git_tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperSourceState {
    state: String,
    git_object_format: String,
    commit: String,
    tree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperRootedFile {
    id: String,
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperEvidenceManifest {
    path: String,
    size: u64,
    sha256: String,
    root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperCandidate {
    digest: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperVerifier {
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperReproduction {
    matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AgentHelperUsage {
    observed_tokens: u64,
}

pub(crate) struct AgentRunRequest {
    pub(crate) bytes: Vec<u8>,
    pub(crate) request_root: String,
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
    execution_bundle: Option<CurrentExecutionBundle>,
    starting_target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    pub(crate) target_task_binding: vela_edge::target_index::TargetTaskBindingV3,
    briefing: Value,
    agent_run_reservations: Vec<CurrentAgentRunReservation>,
    agent_run_receipts: Vec<CurrentAgentRunReceipt>,
    agent_run_submission_links: Vec<CurrentAgentRunSubmissionLink>,
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

fn sha256_root(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn require_exact_id(label: &str, value: &str, prefix: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix(prefix) else {
        return Err(format!("{label} must start with {prefix}"));
    };
    if suffix.is_empty()
        || suffix.len() > 128
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(format!("{label} is not a bounded portable identifier"));
    }
    Ok(())
}

fn require_bounded_text(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(format!("{label} must be bounded non-control text"));
    }
    Ok(())
}

fn require_agent_run_states(candidate_status: &str, verifier_status: &str) -> Result<(), String> {
    if !matches!(candidate_status, "success" | "null" | "failed") {
        return Err("current Agent Run candidate status is unsupported".to_string());
    }
    if !matches!(verifier_status, "passed" | "failed" | "error") {
        return Err("current Agent Run verifier status is unsupported".to_string());
    }
    Ok(())
}

fn agent_run_receipt_root(receipt: &CurrentAgentRunReceipt) -> Result<String, String> {
    canonical_root(&json!({
        "schema": receipt.schema,
        "run_number": receipt.run_number,
        "previous_receipt_root": receipt.previous_receipt_root,
        "result": receipt.result,
        "helper_output_size": receipt.helper_output_size,
        "helper_output_sha256": receipt.helper_output_sha256,
    }))
}

fn git_format_name(format: vela_protocol::repository_inputs::GitObjectFormat) -> &'static str {
    match format {
        vela_protocol::repository_inputs::GitObjectFormat::Sha1 => "sha1",
        vela_protocol::repository_inputs::GitObjectFormat::Sha256 => "sha256",
    }
}

fn validate_agent_run_result_shape(result: &AgentHelperRunOutput) -> Result<(), String> {
    if result.schema != "vela.agent-run-result.v1"
        || !result.ok
        || result.command != "run"
        || result.effect != "none"
        || result.authority != "none"
        || !result.submission.is_null()
    {
        return Err(
            "Vela Agent result must be successful, effect-free, authority-free, and pre-Submission"
                .to_string(),
        );
    }
    require_exact_id("Vela Agent result run.id", &result.run.id, "run_")?;
    for (label, root) in [
        ("request", result.request_root.as_str()),
        ("target binding", result.target.binding_root.as_str()),
        ("Target Index", result.target.target_index_root.as_str()),
        ("target input", result.target.input_root.as_str()),
        ("target packet", result.target.packet_root.as_str()),
        ("execution bundle", result.execution_bundle_root.as_str()),
        ("Run file", result.run.sha256.as_str()),
        (
            "evidence manifest file",
            result.evidence_manifest.sha256.as_str(),
        ),
        ("evidence", result.evidence_manifest.root.as_str()),
        ("candidate", result.candidate.digest.as_str()),
    ] {
        require_sha256_root(&format!("Vela Agent result {label}"), root)?;
    }
    if result.source_state.state != "unchanged" {
        return Err("Vela Agent result must retain one unchanged source state".to_string());
    }
    require_agent_run_states(&result.candidate.status, &result.verifier.status)?;
    if result.usage.observed_tokens > 1_000_000_000 {
        return Err("Vela Agent result observed token usage is unbounded".to_string());
    }
    for (label, path, size) in [
        ("Run", result.run.path.as_str(), result.run.size),
        (
            "evidence manifest",
            result.evidence_manifest.path.as_str(),
            result.evidence_manifest.size,
        ),
    ] {
        if !Path::new(path).is_absolute() || size == 0 || size > AGENT_RUN_FILE_MAX_BYTES {
            return Err(format!(
                "Vela Agent result {label} must be one bounded absolute file"
            ));
        }
        require_bounded_text(&format!("Vela Agent result {label} path"), path, 4096)?;
    }
    Ok(())
}

fn validate_agent_run_result_attempt(
    attempt: &CurrentAttempt,
    target_task_binding: &vela_edge::target_index::TargetTaskBindingV3,
    result: &AgentHelperRunOutput,
) -> Result<(), String> {
    let bundle = attempt
        .execution_bundle
        .as_ref()
        .ok_or_else(|| "current Attempt has no exact Agent execution bundle".to_string())?;
    if result.attempt_id != attempt.attempt_id
        || result.target.id != attempt.target
        || result.target.binding_root != target_task_binding.binding_root
        || result.target.target_index_root != target_task_binding.target_index_root
        || result.target.input_root != target_task_binding.input_root
        || result.target.packet_root != target_task_binding.packet.sha256
        || result.execution_bundle_root != bundle.sha256
        || result.target.source.git_object_format
            != git_format_name(target_task_binding.source.git_object_format)
        || result.target.source.git_commit != target_task_binding.source.git_commit
        || result.target.source.git_tree != target_task_binding.source.git_tree
        || result.target.claim_read_set.git_object_format
            != git_format_name(target_task_binding.claim_read_set.git_object_format)
        || result.target.claim_read_set.git_commit != target_task_binding.claim_read_set.git_commit
        || result.target.claim_read_set.git_tree != target_task_binding.claim_read_set.git_tree
        || result.source_state.git_object_format != result.target.claim_read_set.git_object_format
        || result.source_state.commit != result.target.claim_read_set.git_commit
        || result.source_state.tree != result.target.claim_read_set.git_tree
    {
        return Err("Vela Agent result does not match its exact Attempt".to_string());
    }
    Ok(())
}

fn validate_agent_run_receipt(
    attempt: &CurrentAttempt,
    receipt: &CurrentAgentRunReceipt,
    reservation: &CurrentAgentRunReservation,
) -> Result<(), String> {
    if receipt.schema != "vela.agent-run-receipt.internal.v2" {
        return Err("current Agent Run receipt has an unsupported schema".to_string());
    }
    if receipt.run_number != reservation.run_number
        || receipt.result.request_root != reservation.request_root
    {
        return Err(
            "current Agent Run receipt does not match its exact reserved request".to_string(),
        );
    }
    validate_agent_run_result_shape(&receipt.result)?;
    let target_task_binding = reservation
        .target_task_binding
        .as_ref()
        .unwrap_or(&attempt.starting_target_task_binding);
    validate_agent_run_result_attempt(attempt, target_task_binding, &receipt.result)?;
    if receipt.helper_output_size == 0 || receipt.helper_output_size > AGENT_HELPER_OUTPUT_MAX_BYTES
    {
        return Err("current Agent Run receipt helper output is outside its byte limit".into());
    }
    require_sha256_root(
        "current Agent Run receipt.helper_output_sha256",
        &receipt.helper_output_sha256,
    )?;
    if agent_run_receipt_root(receipt)? != receipt.receipt_root {
        return Err("current Agent Run receipt does not match its root".to_string());
    }
    Ok(())
}

fn canonical_json_file(bytes: &[u8], label: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("parse {label}: {error}"))?;
    let mut canonical = vela_protocol::canonical::to_canonical_bytes(&value)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(format!(
            "{label} must be exact canonical JSON with one newline"
        ));
    }
    Ok(value)
}

fn bounded_locator(value: &Value, label: &str) -> Result<CurrentExecutionBundle, String> {
    let locator: CurrentExecutionBundle =
        serde_json::from_value(value.clone()).map_err(|error| format!("parse {label}: {error}"))?;
    if locator.schema != "vela.agent-execution-bundle.v1" {
        return Err(format!(
            "{label}.schema must be vela.agent-execution-bundle.v1"
        ));
    }
    if locator.path.is_empty()
        || Path::new(&locator.path).is_absolute()
        || !Path::new(&locator.path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{label}.path must be normalized and Frontier-relative"
        ));
    }
    if locator.size == 0 || locator.size > EXECUTION_BUNDLE_MAX_BYTES {
        return Err(format!(
            "{label}.size must be between 1 and {EXECUTION_BUNDLE_MAX_BYTES}"
        ));
    }
    require_sha256_root(&format!("{label}.sha256"), &locator.sha256)?;
    Ok(locator)
}

fn nested_file_locator<'a>(
    value: &'a Value,
    pointer: &str,
    label: &str,
) -> Result<(&'a str, u64, &'a str), String> {
    let object = value
        .pointer(pointer)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{label} locator is missing"))?;
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label}.path is missing"))?;
    let size = object
        .get("size")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label}.size is missing"))?;
    let sha256 = object
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label}.sha256 is missing"))?;
    if object.len() != 3
        || path.is_empty()
        || Path::new(path).is_absolute()
        || !Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "{label} locator is not a closed relative-file reference"
        ));
    }
    if size == 0 || size > 268_435_456 {
        return Err(format!("{label}.size is outside the bounded file limit"));
    }
    require_sha256_root(&format!("{label}.sha256"), sha256)?;
    Ok((path, size, sha256))
}

fn pinned_bundle_file(
    frontier: &Path,
    commit: &str,
    path: &str,
    size: u64,
    root: &str,
    label: &str,
) -> Result<Vec<u8>, String> {
    let bytes = vela_edge::target_index::exact_git_blob_at(frontier, commit, path, size)?;
    if bytes.len() as u64 != size {
        return Err(format!(
            "{label} size mismatch: expected {size}, observed {}",
            bytes.len()
        ));
    }
    let observed = sha256_root(&bytes);
    if observed != root {
        return Err(format!(
            "{label} root mismatch: expected {root}, observed {observed}"
        ));
    }
    Ok(bytes)
}

fn exact_target_packet(
    frontier: &Path,
    attempt: &CurrentAttempt,
) -> Result<(String, Value), String> {
    let packet = &attempt.starting_target_task_binding.packet;
    let bytes = pinned_bundle_file(
        frontier,
        &attempt.starting_target_task_binding.source.git_commit,
        &packet.path,
        packet.size,
        &packet.sha256,
        "Target packet",
    )?;
    let (packet_json, value) = parse_target_packet_bytes(&bytes, packet)?;
    if attempt.briefing.get("packet") != Some(&value) {
        return Err("current Attempt briefing does not match the exact Target packet".to_string());
    }
    Ok((packet_json, value))
}

fn parse_target_packet_bytes(
    bytes: &[u8],
    packet: &vela_edge::target_index::TargetPacketRefV2,
) -> Result<(String, Value), String> {
    if bytes.len() as u64 != packet.size || sha256_root(bytes) != packet.sha256 {
        return Err("Target packet bytes do not match their exact reference".to_string());
    }
    let packet_json = String::from_utf8(bytes.to_vec())
        .map_err(|error| format!("Target packet is not UTF-8 JSON: {error}"))?;
    let value: Value = serde_json::from_str(&packet_json)
        .map_err(|error| format!("parse exact Target packet: {error}"))?;
    if !value.is_object()
        || value.get("schema").and_then(Value::as_str) != Some(packet.schema.as_str())
    {
        return Err(format!(
            "exact Target packet must be one object with schema {:?}",
            packet.schema
        ));
    }
    Ok((packet_json, value))
}

fn execution_bundle_for_attempt(
    frontier: &Path,
    attempt: &CurrentAttempt,
    packet: &Value,
) -> Result<(CurrentExecutionBundle, Value, Value), String> {
    let locator = bounded_locator(
        packet
            .get("execution_bundle")
            .ok_or_else(|| "current Target packet has no execution_bundle".to_string())?,
        "Target packet execution_bundle",
    )?;
    let commit = &attempt.starting_target_task_binding.source.git_commit;
    let bytes = pinned_bundle_file(
        frontier,
        commit,
        &locator.path,
        locator.size,
        &locator.sha256,
        "Agent execution bundle",
    )?;
    let bundle = canonical_json_file(&bytes, "Agent execution bundle")?;
    if bundle.get("schema").and_then(Value::as_str) != Some(locator.schema.as_str())
        || bundle.get("authority").and_then(Value::as_str) != Some("non_authoritative")
        || bundle.get("effect").and_then(Value::as_str) != Some("none")
        || bundle.pointer("/target/id").and_then(Value::as_str) != Some(attempt.target.as_str())
    {
        return Err(
            "Agent execution bundle must be non-authoritative, effect-free, and match the exact Target"
                .to_string(),
        );
    }
    let worker_inputs = bundle
        .pointer("/safeguards/worker_inputs")
        .and_then(Value::as_array)
        .ok_or_else(|| "Agent execution bundle has no worker input boundary".to_string())?;
    if worker_inputs.as_slice() != [json!("mission"), json!("target_packet")]
        || bundle
            .pointer("/safeguards/prior_answer_inputs")
            .and_then(Value::as_array)
            .is_none_or(|values| !values.is_empty())
        || bundle
            .pointer("/safeguards/duplicate_work")
            .and_then(Value::as_str)
            != Some("target_revalidation")
    {
        return Err(
            "Agent execution bundle must expose only mission and Target packet inputs and rely on Target revalidation for duplicate work"
                .to_string(),
        );
    }
    if bundle
        .pointer("/verifier/isolation/network")
        .and_then(Value::as_str)
        != Some("deny")
        || bundle
            .pointer("/verifier/isolation/writes")
            .and_then(Value::as_str)
            != Some("deny")
    {
        return Err("Agent execution bundle verifier must deny network and writes".to_string());
    }

    let (mission_path, mission_size, mission_root) =
        nested_file_locator(&bundle, "/mission", "mission")?;
    let mission_bytes = pinned_bundle_file(
        frontier,
        commit,
        mission_path,
        mission_size,
        mission_root,
        "Agent mission draft",
    )?;
    let mission = canonical_json_file(&mission_bytes, "Agent mission draft")?;
    if mission.get("target").and_then(Value::as_str) != Some(attempt.target.as_str())
        || mission.get("actor").and_then(Value::as_str) != Some(attempt.actor.as_str())
        || mission.get("frontier").and_then(Value::as_str) != Some(".")
        || mission.get("role").and_then(Value::as_str) != Some("producer")
    {
        return Err("Agent mission draft does not match the exact Attempt".to_string());
    }
    let artifact_path = bundle
        .pointer("/artifact_contract/path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Agent execution bundle has no Artifact path".to_string())?;
    let artifact_kind = bundle
        .pointer("/artifact_contract/kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "Agent execution bundle has no Artifact class".to_string())?;
    if !attempt
        .allowed_artifact_classes
        .iter()
        .any(|allowed| allowed == artifact_kind)
    {
        return Err(format!(
            "Agent execution bundle Artifact class {artifact_kind:?} is outside the Attempt"
        ));
    }
    let allowed_paths = mission
        .get("allowed_paths")
        .and_then(Value::as_array)
        .ok_or_else(|| "Agent mission draft has no allowed_paths".to_string())?;
    if allowed_paths.as_slice() != [json!(artifact_path)] {
        return Err("Agent mission and Artifact contract paths disagree".to_string());
    }

    for (pointer, label) in [
        ("/verifier/capsule", "verifier capsule"),
        ("/verifier/source", "verifier source"),
    ] {
        let (path, size, root) = nested_file_locator(&bundle, pointer, label)?;
        let _ = pinned_bundle_file(frontier, commit, path, size, root, label)?;
    }
    Ok((locator, bundle, mission))
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

fn require_current_controller(
    attempt: &CurrentAttempt,
    observed: &CurrentBuildIdentity,
) -> Result<(), String> {
    if &attempt.controller_build != observed {
        return Err(format!(
            "current Attempt {} authorizes another Vela controller build",
            attempt.attempt_id
        ));
    }
    Ok(())
}

fn reserve_agent_run(attempt: &mut CurrentAttempt) -> Result<u64, String> {
    if attempt.usage.runs >= attempt.budget.max_runs {
        return Err(format!(
            "current Attempt {} has exhausted its Agent run budget",
            attempt.attempt_id
        ));
    }
    attempt.usage.runs = attempt
        .usage
        .runs
        .checked_add(1)
        .ok_or_else(|| "current Attempt Agent run usage overflowed".to_string())?;
    Ok(attempt.usage.runs)
}

fn system_temporary_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/private/tmp"),
        PathBuf::from("/var/tmp"),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|candidate| candidate.exists())
}

fn reject_system_temporary_agent_output(output: Option<&Path>) -> Result<(), String> {
    let Some(output) = output else {
        return Ok(());
    };
    let lexical = output.to_path_buf();
    let canonical_ancestor =
        existing_ancestor(output).and_then(|ancestor| std::fs::canonicalize(ancestor).ok());
    for root in system_temporary_roots() {
        if path_is_within(&lexical, &root)
            || canonical_ancestor
                .as_deref()
                .is_some_and(|ancestor| path_is_within(ancestor, &root))
        {
            return Err(
                "Vela Agent output cannot use a system temporary directory because native worker custody is not isolated there; use the default user-home store or another local non-temporary directory"
                    .to_string(),
            );
        }
        if let Ok(canonical_root) = std::fs::canonicalize(&root)
            && (path_is_within(&lexical, &canonical_root)
                || canonical_ancestor
                    .as_deref()
                    .is_some_and(|ancestor| path_is_within(ancestor, &canonical_root)))
        {
            return Err(
                "Vela Agent output cannot use a system temporary directory because native worker custody is not isolated there; use the default user-home store or another local non-temporary directory"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn reserve_agent_run_for_output(
    attempt: &mut CurrentAttempt,
    output: Option<&Path>,
) -> Result<u64, String> {
    reject_system_temporary_agent_output(output)?;
    reserve_agent_run(attempt)
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
        "execution_bundle": attempt.execution_bundle,
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
    if let Some(bundle) = &attempt.execution_bundle {
        if bundle.schema != "vela.agent-execution-bundle.v1"
            || bundle.path.is_empty()
            || bundle.size == 0
            || bundle.size > EXECUTION_BUNDLE_MAX_BYTES
        {
            return Err("current Attempt has an invalid Agent execution bundle".to_string());
        }
        require_sha256_root("current Attempt execution bundle", &bundle.sha256)?;
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
    if attempt.budget.max_runs == 0
        || attempt.budget.max_runs > MAX_MAX_RUNS
        || attempt.budget.max_submissions == 0
        || attempt.budget.max_verifications == 0
        || attempt.budget.max_artifacts == 0
        || attempt.budget.max_artifact_bytes == 0
        || attempt.usage.runs > attempt.budget.max_runs
        || attempt.usage.submissions > attempt.budget.max_submissions
        || attempt.usage.verifications > attempt.budget.max_verifications
        || attempt.usage.artifacts > attempt.budget.max_artifacts
        || attempt.usage.artifact_bytes > attempt.budget.max_artifact_bytes
    {
        return Err("current Attempt budget or usage is invalid".to_string());
    }
    let reserved_run_count = u64::try_from(attempt.agent_run_reservations.len())
        .map_err(|_| "current Attempt Agent run reservation count overflowed".to_string())?;
    if attempt.usage.runs != reserved_run_count {
        return Err(
            "current Attempt Agent run reservations must exactly replay run usage".to_string(),
        );
    }
    for (index, reservation) in attempt.agent_run_reservations.iter().enumerate() {
        let expected = u64::try_from(index)
            .map_err(|_| "current Attempt Agent run reservation index overflowed".to_string())?
            .checked_add(1)
            .ok_or_else(|| "current Attempt Agent run reservation index overflowed".to_string())?;
        if reservation.run_number != expected {
            return Err(
                "current Attempt Agent run reservations must be an ordered complete sequence"
                    .to_string(),
            );
        }
        require_sha256_root(
            "current Attempt Agent run reservation request",
            &reservation.request_root,
        )?;
        if let Some(target_task_binding) = &reservation.target_task_binding {
            target_task_binding.validate()?;
            if target_task_binding.frontier_id != attempt.frontier_id
                || target_task_binding.target_id != attempt.target
                || target_task_binding.repository.origin_id
                    != attempt.starting_target_task_binding.repository.origin_id
                || target_task_binding.source != attempt.starting_target_task_binding.source
                || target_task_binding.input_root != attempt.starting_target_task_binding.input_root
                || target_task_binding.packet != attempt.starting_target_task_binding.packet
            {
                return Err(
                    "current Attempt Agent run reservation changed its authorized Target identity, source, inputs, or packet"
                        .to_string(),
                );
            }
        }
        if attempt.agent_run_reservations[..index]
            .iter()
            .any(|prior| prior.request_root == reservation.request_root)
        {
            return Err(
                "current Attempt Agent run reservations must have unique request roots".to_string(),
            );
        }
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
            "current Attempt registered Verification Record IDs must be a sorted set matching usage"
                .to_string(),
        );
    }
    if attempt.agent_run_receipts.len() > attempt.agent_run_reservations.len() {
        return Err(
            "current Attempt retains more Agent Run receipts than reservations".to_string(),
        );
    }
    let mut previous_receipt_root: Option<&str> = None;
    for (index, receipt) in attempt.agent_run_receipts.iter().enumerate() {
        if receipt.previous_receipt_root.as_deref() != previous_receipt_root {
            return Err("current Agent Run receipts do not form one exact root chain".to_string());
        }
        let reservation = attempt
            .agent_run_reservations
            .iter()
            .find(|reservation| reservation.run_number == receipt.run_number)
            .ok_or_else(|| {
                "current Agent Run receipt has no matching reserved request".to_string()
            })?;
        validate_agent_run_receipt(attempt, receipt, reservation)?;
        if attempt.agent_run_receipts[..index].iter().any(|prior| {
            prior.run_number == receipt.run_number
                || prior.result.run.id == receipt.result.run.id
                || prior.result.request_root == receipt.result.request_root
        }) {
            return Err(
                "current Agent Run receipts must have unique reservations, Runs, and requests"
                    .to_string(),
            );
        }
        previous_receipt_root = Some(&receipt.receipt_root);
    }
    if !attempt
        .agent_run_submission_links
        .windows(2)
        .all(|pair| pair[0].run_id < pair[1].run_id)
    {
        return Err("current Agent Run Submission links must be sorted by Run id".to_string());
    }
    for link in &attempt.agent_run_submission_links {
        require_exact_id(
            "current Agent Run Submission link Run",
            &link.run_id,
            "run_",
        )?;
        require_exact_id(
            "current Agent Run Submission link Submission",
            &link.submission_id,
            "vsb_",
        )?;
        require_sha256_root(
            "current Agent Run Submission link receipt",
            &link.receipt_root,
        )?;
        let receipt = attempt
            .agent_run_receipts
            .iter()
            .find(|receipt| receipt.result.run.id == link.run_id)
            .ok_or_else(|| {
                "current Attempt cannot link a Submission without an Agent Run receipt".to_string()
            })?;
        if receipt.receipt_root != link.receipt_root {
            return Err(
                "current Agent Run Submission link does not match its exact receipt".to_string(),
            );
        }
        if attempt
            .usage
            .registered_submission_ids
            .binary_search(&link.submission_id)
            .is_err()
        {
            return Err(
                "current Agent Run Submission link names a Submission outside Attempt usage"
                    .to_string(),
            );
        }
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
    apply_refreshed_target_binding(attempt, refreshed)?;
    write(frontier, attempt)?;
    if attempt_path(frontier, &attempt.target) != path {
        return Err("current Attempt path changed while refreshing its read set".to_string());
    }
    vela_edge::target_index::revalidate_current_target_task_binding(
        frontier,
        &attempt.target_task_binding,
    )
}

fn apply_refreshed_target_binding(
    attempt: &mut CurrentAttempt,
    refreshed: vela_edge::target_index::TargetTaskBindingV3,
) -> Result<(), String> {
    if refreshed.frontier_id != attempt.frontier_id
        || refreshed.target_id != attempt.target
        || refreshed.repository.origin_id
            != attempt.starting_target_task_binding.repository.origin_id
        || refreshed.source != attempt.starting_target_task_binding.source
        || refreshed.input_root != attempt.starting_target_task_binding.input_root
        || refreshed.packet != attempt.starting_target_task_binding.packet
    {
        return Err(
            "current Attempt cannot continue after its Target identity, source, inputs, or packet changed; revoke it and start the new scope"
                .to_string(),
        );
    }
    attempt.target_task_binding = refreshed;
    Ok(())
}

fn discover_attempts(frontier: &Path) -> Result<Vec<(PathBuf, CurrentAttempt)>, String> {
    let work = frontier.join(".vela/work");
    let entries = match fs::read_dir(&work) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read current Attempt root {}: {error}",
                work.display()
            ));
        }
    };
    let mut attempts = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("read current Attempt entry: {error}"))?;
        let path = entry.path().join("attempt.json");
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

fn require_live_at(attempt: &CurrentAttempt, now: chrono::DateTime<Utc>) -> Result<(), String> {
    if expires_at(attempt)? <= now {
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
    require_live_at(&attempt, Utc::now())?;
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

/// Build the private, authority-free request passed to the optional Agent
/// helper. All scientific scope comes from one live Attempt and its pinned
/// Target packet; the caller cannot substitute a profile, Target, or bundle.
pub(crate) fn agent_run_request(
    frontier: &Path,
    attempt_id: &str,
    helper_build: &crate::agent_delegate::AgentHelperBuild,
    output: Option<&Path>,
) -> Result<AgentRunRequest, String> {
    let helper_build_root = helper_build.root()?;
    require_sha256_root("Agent helper build", &helper_build_root)?;
    let frontier = frontier
        .canonicalize()
        .map_err(|error| format!("resolve current Frontier {}: {error}", frontier.display()))?;
    let mut resolved = resolve_attempt(&frontier, Some(attempt_id))?
        .ok_or_else(|| format!("current Attempt {attempt_id} is unavailable"))?;
    revalidate_routine_attempt(&frontier, Some(&resolved))?;
    vela_edge::target_index::revalidate_current_target_execution_binding(
        &frontier,
        &resolved.attempt.target_task_binding,
    )?;
    let attempt = &mut resolved.attempt;
    require_current_controller(attempt, &controller_build()?)?;
    if attempt.runner_build_root != helper_build_root {
        return Err(format!(
            "current Attempt {} authorizes runner {}, not Agent helper {}",
            attempt.attempt_id, attempt.runner_build_root, helper_build_root
        ));
    }
    for operation in ["run_tool", "write_private_artifact"] {
        if !attempt
            .allowed_operations
            .iter()
            .any(|allowed| allowed == operation)
        {
            return Err(format!(
                "current Attempt {} does not authorize {operation}",
                attempt.attempt_id
            ));
        }
    }
    if attempt.usage.artifacts >= attempt.budget.max_artifacts
        || attempt.usage.artifact_bytes >= attempt.budget.max_artifact_bytes
    {
        return Err(format!(
            "current Attempt {} has exhausted its private Artifact budget",
            attempt.attempt_id
        ));
    }
    let (packet_json, packet) = exact_target_packet(&frontier, attempt)?;
    let (bundle_ref, bundle, mission) = execution_bundle_for_attempt(&frontier, attempt, &packet)?;
    if attempt.execution_bundle.as_ref() != Some(&bundle_ref) {
        return Err("current Attempt execution bundle changed after authorization".to_string());
    }
    let output = match output {
        Some(path) if path.is_absolute() => Some(path.to_path_buf()),
        Some(path) => Some(
            std::env::current_dir()
                .map_err(|error| format!("resolve Agent output base: {error}"))?
                .join(path),
        ),
        None => None,
    };
    let run_number = reserve_agent_run_for_output(attempt, output.as_deref())?;
    let preimage = json!({
        "schema": "vela.agent-run-request.internal.v1",
        "authority": "none",
        "effect": "none",
        "frontier": {
            "path": frontier,
            "id": attempt.frontier_id,
            "origin_id": attempt.target_task_binding.repository.origin_id,
            "repository_root": attempt.target_task_binding.repository.repository_root,
        },
        "attempt": {
            "id": attempt.attempt_id,
            "authorization_root": attempt.authorization_root,
            "actor": attempt.actor,
            "created_at": attempt.created_at,
            "expires_at": attempt.expires_at,
            "controller_build": attempt.controller_build,
            "runner_build_root": attempt.runner_build_root,
            "allowed_operations": attempt.allowed_operations,
            "allowed_artifact_classes": attempt.allowed_artifact_classes,
            "budget": attempt.budget,
            "usage": attempt.usage,
            "consequence_ceiling": attempt.consequence_ceiling,
            "task_contract_root": attempt.task_contract_root,
        },
        "target": {
            "id": attempt.target,
            "binding_root": attempt.target_task_binding.binding_root,
            "target_index_root": attempt.target_task_binding.target_index_root,
            "input_root": attempt.target_task_binding.input_root,
            "source": attempt.target_task_binding.source,
            "claim_read_set": attempt.target_task_binding.claim_read_set,
            "packet": attempt.target_task_binding.packet,
            "packet_json": packet_json,
        },
        "execution_bundle": {
            "reference": bundle_ref,
            "value": bundle,
            "mission": mission,
        },
        "runner_build": helper_build,
        "output_root": output,
    });
    let request_root = canonical_root(&preimage)?;
    let mut request = preimage
        .as_object()
        .cloned()
        .ok_or_else(|| "Agent run request preimage is not an object".to_string())?;
    request.insert(
        "request_root".to_string(),
        Value::String(request_root.clone()),
    );
    let mut bytes = vela_protocol::canonical::to_canonical_bytes(&request)?;
    bytes.push(b'\n');
    if bytes.len() > ATTEMPT_MAX_BYTES {
        return Err(format!(
            "Agent run request is {} bytes; limit is {ATTEMPT_MAX_BYTES}",
            bytes.len()
        ));
    }
    attempt
        .agent_run_reservations
        .push(CurrentAgentRunReservation {
            run_number,
            request_root: request_root.clone(),
            target_task_binding: Some(attempt.target_task_binding.clone()),
        });
    write(&frontier, attempt)?;
    Ok(AgentRunRequest {
        bytes,
        request_root,
    })
}

fn read_exact_agent_file(
    frontier: &Path,
    raw_path: &str,
    expected_size: u64,
    expected_sha256: &str,
    label: &str,
) -> Result<(String, Vec<u8>), String> {
    let path = PathBuf::from(raw_path);
    if !path.is_absolute() || expected_size == 0 || expected_size > AGENT_RUN_FILE_MAX_BYTES {
        return Err(format!("{label} must be one bounded absolute regular file"));
    }
    require_bounded_text(&format!("{label} path"), raw_path, 4096)?;
    require_sha256_root(&format!("{label} sha256"), expected_sha256)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("resolve {label} {}: {error}", path.display()))?;
    if canonical.starts_with(frontier) {
        return Err(format!("{label} must remain outside the Frontier worktree"));
    }
    let before =
        fs::symlink_metadata(&path).map_err(|error| format!("inspect {label}: {error}"))?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() != expected_size {
        return Err(format!(
            "{label} must match its exact bounded regular non-symlink file identity"
        ));
    }
    let bytes = fs::read(&path).map_err(|error| format!("read {label}: {error}"))?;
    let after =
        fs::symlink_metadata(&path).map_err(|error| format!("reinspect {label}: {error}"))?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
        || bytes.len() as u64 != expected_size
        || sha256_root(&bytes) != expected_sha256
    {
        return Err(format!(
            "{label} changed or drifted while it was being bound"
        ));
    }
    Ok((canonical.display().to_string(), bytes))
}

/// Retain the exact successful helper handoff in ignored Attempt state.
///
/// This writes no canonical record and grants no authority. It binds the
/// helper's exact stdout bytes and retained Run file so a later `status` can
/// continue with the precise export and registration path. The helper owns its
/// internal Run schema; Vela validates only its own narrow result envelope and
/// the exact retained bytes named by that envelope.
pub(crate) fn record_agent_run_receipt(
    frontier: &Path,
    attempt_id: &str,
    expected_request_root: &str,
    helper_output_bytes: &[u8],
) -> Result<(), String> {
    if helper_output_bytes.is_empty()
        || helper_output_bytes.len() as u64 > AGENT_HELPER_OUTPUT_MAX_BYTES
    {
        return Err(format!(
            "Vela Agent helper output must be 1..={AGENT_HELPER_OUTPUT_MAX_BYTES} bytes"
        ));
    }
    require_sha256_root("Agent request root", expected_request_root)?;
    let mut output: AgentHelperRunOutput = serde_json::from_slice(helper_output_bytes)
        .map_err(|error| format!("parse Vela Agent helper run output: {error}"))?;
    validate_agent_run_result_shape(&output)?;
    if output.attempt_id != attempt_id || output.request_root != expected_request_root {
        return Err("Vela Agent helper output does not match its exact request".to_string());
    }
    let frontier = frontier
        .canonicalize()
        .map_err(|error| format!("resolve current Frontier {}: {error}", frontier.display()))?;

    let matches = discover_attempts(&frontier)?
        .into_iter()
        .filter(|(_, attempt)| attempt.attempt_id == attempt_id)
        .collect::<Vec<_>>();
    let [(path, discovered)] = matches.as_slice() else {
        return Err(format!(
            "current Attempt {attempt_id:?} must resolve to exactly one private record; found {}",
            matches.len()
        ));
    };
    let _lock = lock_attempt(&frontier, &discovered.target)?;
    let mut attempt = read(path)?;
    if attempt.attempt_id != attempt_id {
        return Err(format!(
            "current Attempt {attempt_id} changed while retaining its Agent Run"
        ));
    }
    let reservation = attempt
        .agent_run_reservations
        .iter()
        .find(|reservation| reservation.request_root == expected_request_root)
        .cloned()
        .ok_or_else(|| {
            "current Attempt has no exact reserved Agent run for this request".to_string()
        })?;
    let target_task_binding = reservation
        .target_task_binding
        .as_ref()
        .unwrap_or(&attempt.starting_target_task_binding);
    validate_agent_run_result_attempt(&attempt, target_task_binding, &output)?;
    let (run_file, _run_bytes) = read_exact_agent_file(
        &frontier,
        &output.run.path,
        output.run.size,
        &output.run.sha256,
        "retained Agent Run",
    )?;
    let (evidence_manifest_file, evidence_manifest_bytes) = read_exact_agent_file(
        &frontier,
        &output.evidence_manifest.path,
        output.evidence_manifest.size,
        &output.evidence_manifest.sha256,
        "retained Agent evidence manifest",
    )?;
    let evidence_manifest =
        canonical_json_file(&evidence_manifest_bytes, "retained Agent evidence manifest")?;
    if canonical_root(&evidence_manifest)? != output.evidence_manifest.root {
        return Err(
            "retained Agent evidence manifest does not match its canonical root".to_string(),
        );
    }
    output.run.path = run_file;
    output.evidence_manifest.path = evidence_manifest_file;
    if let Some(existing) = attempt
        .agent_run_receipts
        .iter()
        .find(|receipt| receipt.run_number == reservation.run_number)
    {
        if existing.result == output
            && existing.helper_output_size == helper_output_bytes.len() as u64
            && existing.helper_output_sha256 == sha256_root(helper_output_bytes)
        {
            return Ok(());
        }
        return Err(format!(
            "current Attempt {attempt_id} already retains another Agent Run receipt for reserved run {}",
            reservation.run_number
        ));
    }
    let mut receipt = CurrentAgentRunReceipt {
        schema: "vela.agent-run-receipt.internal.v2".to_string(),
        receipt_root: String::new(),
        run_number: reservation.run_number,
        previous_receipt_root: attempt
            .agent_run_receipts
            .last()
            .map(|receipt| receipt.receipt_root.clone()),
        result: output,
        helper_output_size: helper_output_bytes.len() as u64,
        helper_output_sha256: sha256_root(helper_output_bytes),
    };
    receipt.receipt_root = agent_run_receipt_root(&receipt)?;
    validate_agent_run_receipt(&attempt, &receipt, &reservation)?;
    attempt.agent_run_receipts.push(receipt);
    write(&frontier, &attempt).map_err(|error| {
        format!(
            "Agent Run succeeded but private Attempt receipt failed at {}: {error}",
            path.display()
        )
    })?;
    Ok(())
}

/// Recover private Verification budget attribution after a canonical import.
///
/// A missing or expired private Attempt cannot invalidate durable canonical
/// evidence. Present malformed, duplicated, or lock-raced state still fails
/// closed rather than silently charging a different Attempt.
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
            "current Attempt {source_attempt:?} must resolve to at most one private record for Verification reconciliation; found {}",
            matches.len()
        ));
    };
    let lock = lock_attempt(frontier, &discovered.target)?;
    let mut attempt = read(path)?;
    if attempt.attempt_id != source_attempt {
        return Err(format!(
            "current Attempt {source_attempt} changed while acquiring its private reconciliation lock"
        ));
    }
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

/// Recheck the time- and Target-bound execution authority while the canonical
/// repository write barrier is held.
fn revalidate_routine_attempt_at(
    frontier: &Path,
    resolved: Option<&CurrentRoutineAttempt>,
    now: chrono::DateTime<Utc>,
) -> Result<(), String> {
    let Some(resolved) = resolved else {
        return Ok(());
    };
    require_live_at(&resolved.attempt, now)?;
    vela_edge::target_index::revalidate_current_target_task_binding(
        frontier,
        &resolved.attempt.target_task_binding,
    )
}

pub(crate) fn revalidate_routine_attempt(
    frontier: &Path,
    resolved: Option<&CurrentRoutineAttempt>,
) -> Result<(), String> {
    revalidate_routine_attempt_at(frontier, resolved, Utc::now())
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
    if let Some(source_run) = submission.provenance.source_run.as_deref()
        && attempt
            .agent_run_receipts
            .iter()
            .any(|receipt| receipt.result.run.id == source_run)
        && let Some(link) = attempt
            .agent_run_submission_links
            .iter()
            .find(|link| link.run_id == source_run)
        && link.submission_id != submission.submission_id
    {
        return Err(format!(
            "Agent Run {source_run} is already bound to registered Submission {}",
            link.submission_id
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
    resolved: Option<CurrentRoutineAttempt>,
    submission: &SubmissionV1,
    artifact_bytes: u64,
) -> Result<(), String> {
    let Some(mut resolved) = resolved else {
        return Ok(());
    };
    authorize_submission(Some(&resolved), submission, artifact_bytes)?;
    let already_registered = match resolved
        .attempt
        .usage
        .registered_submission_ids
        .binary_search(&submission.submission_id)
    {
        Ok(_) => true,
        Err(index) => {
            resolved
                .attempt
                .usage
                .registered_submission_ids
                .insert(index, submission.submission_id.clone());
            false
        }
    };
    if !already_registered {
        resolved.attempt.usage.submissions += 1;
        resolved.attempt.usage.artifacts += u64::try_from(submission.artifacts.len())
            .map_err(|_| "Submission Artifact count overflowed".to_string())?;
        resolved.attempt.usage.artifact_bytes += artifact_bytes;
    }
    let mut receipt_link_changed = false;
    if let Some(source_run) = submission.provenance.source_run.as_deref()
        && let Some(receipt) = resolved
            .attempt
            .agent_run_receipts
            .iter()
            .find(|receipt| receipt.result.run.id == source_run)
    {
        let receipt_root = receipt.receipt_root.clone();
        match resolved
            .attempt
            .agent_run_submission_links
            .iter()
            .find(|link| link.run_id == source_run)
        {
            Some(link) if link.submission_id != submission.submission_id => {
                return Err(format!(
                    "Agent Run {source_run} is already bound to registered Submission {}",
                    link.submission_id
                ));
            }
            Some(_) => {}
            None => {
                let link = CurrentAgentRunSubmissionLink {
                    run_id: source_run.to_string(),
                    receipt_root,
                    submission_id: submission.submission_id.clone(),
                };
                let index = resolved
                    .attempt
                    .agent_run_submission_links
                    .partition_point(|existing| existing.run_id < link.run_id);
                resolved
                    .attempt
                    .agent_run_submission_links
                    .insert(index, link);
                receipt_link_changed = true;
            }
        }
    }
    if already_registered && !receipt_link_changed {
        return Ok(());
    }
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
    if attempt.consequence_ceiling != CONSEQUENCE_PENDING_REVIEW
        || !attempt
            .allowed_operations
            .iter()
            .any(|operation| operation == "verification_import")
    {
        return Err(format!(
            "current Attempt {} does not authorize Verification import",
            attempt.attempt_id
        ));
    }
    if attempt
        .usage
        .registered_verification_record_ids
        .binary_search_by(|candidate| candidate.as_str().cmp(verification_record_id))
        .is_ok()
    {
        return Ok(());
    }
    let next_verifications = attempt
        .usage
        .verifications
        .checked_add(1)
        .ok_or_else(|| "current Attempt Verification usage overflowed".to_string())?;
    if next_verifications > attempt.budget.max_verifications {
        return Err(format!(
            "current Attempt {} Verification budget exhausted: requested verifications={next_verifications}/{}",
            attempt.attempt_id, attempt.budget.max_verifications
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
    match resolved
        .attempt
        .usage
        .registered_verification_record_ids
        .binary_search_by(|candidate| candidate.as_str().cmp(verification_record_id))
    {
        Ok(_) => return Ok(()),
        Err(index) => {
            resolved
                .attempt
                .usage
                .registered_verification_record_ids
                .insert(index, verification_record_id.to_string());
        }
    }
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
            operations.push("verification_import".to_string());
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
        "execution_bundle": attempt.execution_bundle,
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
    max_runs: u64,
    max_submissions: u64,
    max_verifications: u64,
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
    if max_runs == 0
        || max_submissions == 0
        || max_verifications == 0
        || max_artifacts == 0
        || max_artifact_bytes == 0
    {
        return Err("private Attempt budgets must be positive".to_string());
    }
    if max_runs > MAX_MAX_RUNS
        || max_submissions > DEFAULT_MAX_SUBMISSIONS
        || max_verifications > DEFAULT_MAX_VERIFICATIONS
        || max_artifacts > DEFAULT_MAX_ARTIFACTS
        || max_artifact_bytes > DEFAULT_MAX_ARTIFACT_BYTES
    {
        return Err(format!(
            "private Attempt budgets exceed their ceilings: runs<={MAX_MAX_RUNS}, submissions<={DEFAULT_MAX_SUBMISSIONS}, verifications<={DEFAULT_MAX_VERIFICATIONS}, artifacts<={DEFAULT_MAX_ARTIFACTS}, artifact_bytes<={DEFAULT_MAX_ARTIFACT_BYTES}"
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
            max_runs,
            max_submissions,
            max_verifications,
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
            max_runs,
            max_submissions,
            max_verifications,
            max_artifacts,
            max_artifact_bytes,
        },
        usage: CurrentAttemptUsage {
            runs: 0,
            submissions: 0,
            verifications: 0,
            artifacts: 0,
            artifact_bytes: 0,
            registered_submission_ids: Vec::new(),
            registered_verification_record_ids: Vec::new(),
        },
        consequence_ceiling: consequence_ceiling.to_string(),
        task_contract,
        task_contract_root,
        execution_bundle: None,
        starting_target_task_binding: binding.clone(),
        target_task_binding: binding,
        briefing: json!({
            "schema": "vela.work-briefing.v2",
            "target": target,
            "packet": packet,
        }),
        agent_run_reservations: Vec::new(),
        agent_run_receipts: Vec::new(),
        agent_run_submission_links: Vec::new(),
    };
    let (_, exact_packet) = exact_target_packet(frontier, &attempt)?;
    attempt.execution_bundle = execution_bundle_for_attempt(frontier, &attempt, &exact_packet)
        .map(|(bundle, _, _)| Some(bundle))
        .or_else(|error| {
            if packet.get("execution_bundle").is_none() {
                Ok(None)
            } else {
                Err(error)
            }
        })?;
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

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':' | b'@')
        })
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn agent_receipt_file_states(frontier: &Path, receipt: &CurrentAgentRunReceipt) -> (bool, bool) {
    let result = &receipt.result;
    let run_matches = read_exact_agent_file(
        frontier,
        &result.run.path,
        result.run.size,
        &result.run.sha256,
        "retained Agent Run",
    )
    .is_ok();
    let evidence_matches = read_exact_agent_file(
        frontier,
        &result.evidence_manifest.path,
        result.evidence_manifest.size,
        &result.evidence_manifest.sha256,
        "retained Agent evidence manifest",
    )
    .and_then(|(_, bytes)| {
        let value = canonical_json_file(&bytes, "retained Agent evidence manifest")?;
        if canonical_root(&value)? != result.evidence_manifest.root {
            return Err(
                "retained Agent evidence manifest does not match its canonical root".to_string(),
            );
        }
        Ok(())
    })
    .is_ok();
    (run_matches, evidence_matches)
}

fn agent_run_projection(
    frontier: &Path,
    attempt: &CurrentAttempt,
    receipt: &CurrentAgentRunReceipt,
) -> Option<Value> {
    let result = &receipt.result;
    let (run_matches, evidence_matches) = agent_receipt_file_states(frontier, receipt);
    let export_root = Path::new(&result.run.path)
        .parent()
        .and_then(Path::parent)
        .map(|parent| parent.join(format!("submission-{}", result.run.id)))?;
    let export = format!(
        "vela agent export {} --output {} --as {} --attempt {}",
        shell_quote(&result.run.path),
        shell_quote(&export_root.display().to_string()),
        shell_quote(&attempt.actor),
        shell_quote(&attempt.attempt_id),
    );
    let submit = format!(
        "vela submit {} --frontier {} --attempt {} --as {} --json",
        shell_quote(&export_root.join("submission.json").display().to_string()),
        shell_quote(&frontier.display().to_string()),
        shell_quote(&attempt.attempt_id),
        shell_quote(&attempt.actor),
    );
    let show = format!("vela agent show {}", shell_quote(&result.run.path));
    let replay = format!("vela agent replay {}", shell_quote(&result.run.path));
    let exportable = run_matches
        && evidence_matches
        && result.candidate.status == "success"
        && result.verifier.status == "passed"
        && result.reproduction.matched;
    let submission_id = attempt
        .agent_run_submission_links
        .iter()
        .find(|link| link.run_id == result.run.id)
        .map(|link| link.submission_id.as_str());
    let submission_state = if submission_id.is_some() {
        "registered"
    } else if exportable {
        "ready_to_export"
    } else {
        "not_exportable"
    };
    Some(json!({
        "receipt_root": receipt.receipt_root,
        "previous_receipt_root": receipt.previous_receipt_root,
        "run_number": receipt.run_number,
        "run_id": result.run.id,
        "run_file": {
            "path": result.run.path,
            "size": result.run.size,
            "sha256": result.run.sha256,
            "state": if run_matches { "matched" } else { "drifted_or_missing" },
        },
        "evidence_manifest": {
            "path": result.evidence_manifest.path,
            "size": result.evidence_manifest.size,
            "sha256": result.evidence_manifest.sha256,
            "root": result.evidence_manifest.root,
            "state": if evidence_matches { "matched" } else { "drifted_or_missing" },
        },
        "evidence_root": result.evidence_manifest.root,
        "target_binding_root": result.target.binding_root,
        "execution_bundle_root": result.execution_bundle_root,
        "source": {
            "git_commit": result.source_state.commit,
            "git_tree": result.source_state.tree,
        },
        "candidate": {
            "digest": result.candidate.digest,
            "status": result.candidate.status,
        },
        "producer_verifier": {
            "status": result.verifier.status,
        },
        "clean_clone_reproduced": result.reproduction.matched,
        "usage": {
            "observed_tokens": result.usage.observed_tokens,
        },
        "request_root": result.request_root,
        "helper_output": {
            "size": receipt.helper_output_size,
            "sha256": receipt.helper_output_sha256,
        },
        "submission": {
            "state": submission_state,
            "id": submission_id,
        },
        "next_commands": {
            "export": exportable.then_some(export),
            "submit": exportable.then_some(submit),
            "show": run_matches.then_some(show),
            "replay": run_matches.then_some(replay),
        },
        "authority": "none",
        "canonical_write": false,
    }))
}

fn attempt_list_entry(frontier: &Path, attempt: CurrentAttempt, path: &Path) -> Value {
    let agent_runs = attempt
        .agent_run_receipts
        .iter()
        .filter_map(|receipt| agent_run_projection(frontier, &attempt, receipt))
        .collect::<Vec<_>>();
    let agent_run = agent_runs.last().cloned();
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
        "target_packet_sha256": attempt.target_task_binding.packet.sha256,
        "execution_bundle": attempt.execution_bundle,
        "usage": attempt.usage,
        "budget": attempt.budget,
        "agent_run": agent_run,
        "agent_runs": agent_runs,
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
            Ok(attempt_list_entry(frontier, attempt, &path))
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
    max_runs: Option<u64>,
    max_submissions: Option<u64>,
    max_verifications: Option<u64>,
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
                max_runs.unwrap_or(DEFAULT_MAX_RUNS),
                max_submissions.unwrap_or(DEFAULT_MAX_SUBMISSIONS),
                max_verifications.unwrap_or(DEFAULT_MAX_VERIFICATIONS),
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
    fn agent_output_refuses_system_temporary_placement() {
        let output = std::env::temp_dir().join("vela-agent-evidence");
        let error = reject_system_temporary_agent_output(Some(&output)).unwrap_err();
        assert!(error.contains("system temporary directory"), "{error}");
        reject_system_temporary_agent_output(None).unwrap();
        let local_home_output = if cfg!(windows) {
            PathBuf::from(r"C:\Users\vela\.vela\agent\runs\evidence")
        } else {
            PathBuf::from("/home/vela/.vela/agent/runs/evidence")
        };
        reject_system_temporary_agent_output(Some(&local_home_output)).unwrap();
    }

    #[test]
    fn exact_packet_and_bundle_bytes_fail_closed() {
        let packet_bytes = br#"{"schema":"erdos.problem-work.v1","statement":"bounded fixture"}"#;
        let packet = vela_edge::target_index::TargetPacketRefV2 {
            schema: "erdos.problem-work.v1".to_string(),
            path: "packets/1056.json".to_string(),
            size: packet_bytes.len() as u64,
            sha256: sha256_root(packet_bytes),
        };
        let (packet_json, parsed) = parse_target_packet_bytes(packet_bytes, &packet).unwrap();
        assert_eq!(packet_json.as_bytes(), packet_bytes);
        assert_eq!(parsed["statement"], "bounded fixture");

        let error = parse_target_packet_bytes(
            br#"{"schema":"erdos.problem-work.v1","statement":"substituted"}"#,
            &packet,
        )
        .unwrap_err();
        assert!(
            error.contains("do not match their exact reference"),
            "{error}"
        );

        let other_bytes = br#"{"schema":"another.packet.v1"}"#;
        let other = vela_edge::target_index::TargetPacketRefV2 {
            size: other_bytes.len() as u64,
            sha256: sha256_root(other_bytes),
            ..packet
        };
        let error = parse_target_packet_bytes(other_bytes, &other).unwrap_err();
        assert!(error.contains("must be one object with schema"), "{error}");

        let bundle = json!({
            "authority": "non_authoritative",
            "effect": "none",
            "schema": "vela.agent-execution-bundle.v1",
        });
        let mut canonical = vela_protocol::canonical::to_canonical_bytes(&bundle).unwrap();
        canonical.push(b'\n');
        assert_eq!(
            canonical_json_file(&canonical, "Agent execution bundle").unwrap(),
            bundle
        );
        let pretty = serde_json::to_vec_pretty(&bundle).unwrap();
        let error = canonical_json_file(&pretty, "Agent execution bundle").unwrap_err();
        assert!(error.contains("exact canonical JSON"), "{error}");

        let error = bounded_locator(
            &json!({
                "schema": "vela.agent-execution-bundle.v1",
                "path": "../bundle.json",
                "size": 1,
                "sha256": format!("sha256:{}", "1".repeat(64)),
            }),
            "Target packet execution_bundle",
        )
        .unwrap_err();
        assert!(error.contains("Frontier-relative"), "{error}");
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
                "verification_import".to_string(),
            ],
            allowed_artifact_classes: vec!["witness".to_string()],
            budget: CurrentAttemptBudget {
                max_runs: 3,
                max_submissions: 1,
                max_verifications: 1,
                max_artifacts: 8,
                max_artifact_bytes: 1024,
            },
            usage: CurrentAttemptUsage {
                runs: 0,
                submissions: 0,
                verifications: 0,
                artifacts: 0,
                artifact_bytes: 0,
                registered_submission_ids: Vec::new(),
                registered_verification_record_ids: Vec::new(),
            },
            consequence_ceiling: CONSEQUENCE_PENDING_REVIEW.to_string(),
            task_contract,
            task_contract_root,
            execution_bundle: Some(CurrentExecutionBundle {
                schema: "vela.agent-execution-bundle.v1".to_string(),
                path: "execution/bundle.json".to_string(),
                size: 42,
                sha256: format!("sha256:{}", "e".repeat(64)),
            }),
            starting_target_task_binding: binding.clone(),
            target_task_binding: binding,
            briefing: json!({"schema": "vela.work-briefing.v2"}),
            agent_run_reservations: Vec::new(),
            agent_run_receipts: Vec::new(),
            agent_run_submission_links: Vec::new(),
        };
        attempt.authorization_root = authorization_root(&attempt).unwrap();
        attempt.attempt_id = attempt_id(&attempt.authorization_root).unwrap();
        let path = write(directory.path(), &attempt).unwrap();
        let decoded = read(&path).unwrap();
        assert_eq!(decoded.attempt_id, attempt.attempt_id);
        assert_eq!(decoded.schema, "vela.attempt.v8");
        let observed_controller = decoded.controller_build.clone();
        require_current_controller(&decoded, &observed_controller).unwrap();
        let mut other_controller = observed_controller;
        other_controller.binary_sha256 = format!("sha256:{}", "f".repeat(64));
        let error = require_current_controller(&decoded, &other_controller).unwrap_err();
        assert!(error.contains("another Vela controller build"), "{error}");

        let temporary_output_root = tempfile::tempdir().unwrap();
        let temporary_output = temporary_output_root.path().join("agent-output");
        let mut rejected = decoded.clone();
        let error =
            reserve_agent_run_for_output(&mut rejected, Some(&temporary_output)).unwrap_err();
        assert!(error.contains("system temporary directory"), "{error}");
        assert_eq!(rejected.usage.runs, 0);

        let mut single_run = decoded.clone();
        single_run.budget.max_runs = 1;
        reserve_agent_run_for_output(&mut single_run, None).unwrap();
        assert_eq!(single_run.usage.runs, 1);
        let error = reserve_agent_run(&mut single_run).unwrap_err();
        assert!(error.contains("exhausted its Agent run budget"), "{error}");

        let request_root = format!("sha256:{}", "f".repeat(64));
        let mut reserved = decoded.clone();
        let run_number = reserve_agent_run_for_output(&mut reserved, None).unwrap();
        reserved
            .agent_run_reservations
            .push(CurrentAgentRunReservation {
                run_number,
                request_root: request_root.clone(),
                target_task_binding: Some(attempt.target_task_binding.clone()),
            });
        write(directory.path(), &reserved).unwrap();

        let retained_output = tempfile::tempdir().unwrap();
        let run_directory = retained_output.path().join("run");
        fs::create_dir(&run_directory).unwrap();
        let run_file = run_directory.join("run.json");
        let run_id = "run_4cb32738-305e-4a86-8384-b48787d72b28";
        let candidate_digest = format!("sha256:{}", "b".repeat(64));
        let run_bytes = b"private helper-owned Run bytes\n".to_vec();
        fs::write(&run_file, &run_bytes).unwrap();
        let evidence_manifest_file = run_directory.join("evidence-manifest.json");
        let evidence_manifest = json!({
            "schema": "canopus.run-evidence.v1",
            "run_id": run_id,
            "files": {"run": sha256_root(&run_bytes)},
        });
        let mut evidence_manifest_bytes =
            vela_protocol::canonical::to_canonical_bytes(&evidence_manifest).unwrap();
        evidence_manifest_bytes.push(b'\n');
        fs::write(&evidence_manifest_file, &evidence_manifest_bytes).unwrap();
        let evidence_root = canonical_root(&evidence_manifest).unwrap();
        let helper_output = json!({
            "schema": "vela.agent-run-result.v1",
            "ok": true,
            "command": "run",
            "effect": "none",
            "authority": "none",
            "attempt_id": attempt.attempt_id,
            "request_root": request_root,
            "target": {
                "id": attempt.target,
                "binding_root": attempt.target_task_binding.binding_root,
                "target_index_root": attempt.target_task_binding.target_index_root,
                "input_root": attempt.target_task_binding.input_root,
                "packet_root": attempt.target_task_binding.packet.sha256,
                "source": {
                    "git_object_format": "sha1",
                    "git_commit": attempt.target_task_binding.source.git_commit,
                    "git_tree": attempt.target_task_binding.source.git_tree,
                },
                "claim_read_set": {
                    "git_object_format": "sha1",
                    "git_commit": attempt.target_task_binding.claim_read_set.git_commit,
                    "git_tree": attempt.target_task_binding.claim_read_set.git_tree,
                },
            },
            "execution_bundle_root": attempt.execution_bundle.as_ref().unwrap().sha256,
            "source_state": {
                "state": "unchanged",
                "git_object_format": "sha1",
                "commit": attempt.target_task_binding.claim_read_set.git_commit,
                "tree": attempt.target_task_binding.claim_read_set.git_tree,
            },
            "run": {
                "id": run_id,
                "path": run_file.display().to_string(),
                "size": run_bytes.len(),
                "sha256": sha256_root(&run_bytes),
            },
            "evidence_manifest": {
                "path": evidence_manifest_file.display().to_string(),
                "size": evidence_manifest_bytes.len(),
                "sha256": sha256_root(&evidence_manifest_bytes),
                "root": evidence_root,
            },
            "candidate": {
                "digest": candidate_digest,
                "status": "success",
            },
            "verifier": {"status": "passed"},
            "reproduction": {"matched": true},
            "usage": {"observed_tokens": 70910},
            "submission": null,
        });
        let mut failed_output = helper_output.clone();
        failed_output["candidate"]["status"] = Value::String("failed".to_string());
        failed_output["verifier"]["status"] = Value::String("failed".to_string());
        failed_output["reproduction"]["matched"] = Value::Bool(false);
        let mut failed_output_bytes = serde_json::to_vec(&failed_output).unwrap();
        failed_output_bytes.push(b'\n');
        record_agent_run_receipt(
            directory.path(),
            &attempt.attempt_id,
            &request_root,
            &failed_output_bytes,
        )
        .unwrap();
        let mut failed_attempt = read(&path).unwrap();
        let projected = attempt_list_entry(directory.path(), failed_attempt.clone(), &path);
        assert_eq!(projected["agent_run"]["candidate"]["status"], "failed");
        assert_eq!(
            projected["agent_run"]["producer_verifier"]["status"],
            "failed"
        );
        assert_eq!(
            projected["agent_run"]["submission"]["state"],
            "not_exportable"
        );
        assert!(projected["agent_run"]["next_commands"]["export"].is_null());
        failed_attempt.agent_run_receipts.clear();
        write(directory.path(), &failed_attempt).unwrap();

        let mut wrong_bundle = helper_output.clone();
        wrong_bundle["execution_bundle_root"] = Value::String(format!("sha256:{}", "d".repeat(64)));
        let mut wrong_bundle_bytes = serde_json::to_vec(&wrong_bundle).unwrap();
        wrong_bundle_bytes.push(b'\n');
        let error = record_agent_run_receipt(
            directory.path(),
            &attempt.attempt_id,
            &request_root,
            &wrong_bundle_bytes,
        )
        .unwrap_err();
        assert!(error.contains("exact Attempt"), "{error}");

        let mut unknown_field = helper_output.clone();
        unknown_field["legacy"] = Value::Bool(true);
        let mut unknown_field_bytes = serde_json::to_vec(&unknown_field).unwrap();
        unknown_field_bytes.push(b'\n');
        let error = record_agent_run_receipt(
            directory.path(),
            &attempt.attempt_id,
            &request_root,
            &unknown_field_bytes,
        )
        .unwrap_err();
        assert!(error.contains("unknown field"), "{error}");

        let mut helper_output_bytes = serde_json::to_vec(&helper_output).unwrap();
        helper_output_bytes.push(b'\n');
        record_agent_run_receipt(
            directory.path(),
            &attempt.attempt_id,
            &request_root,
            &helper_output_bytes,
        )
        .unwrap();
        let decoded = read(&path).unwrap();
        let projected = attempt_list_entry(directory.path(), decoded.clone(), &path);
        assert_eq!(projected["authorization_root"], attempt.authorization_root);
        assert_eq!(
            projected["allowed_operations"],
            json!([
                "inspect",
                "submission_author",
                "submission_register",
                "verification_import"
            ])
        );
        assert_eq!(projected["allowed_artifact_classes"], json!(["witness"]));
        assert_eq!(projected["consequence_ceiling"], CONSEQUENCE_PENDING_REVIEW);
        assert_eq!(projected["task_contract_root"], attempt.task_contract_root);
        assert_eq!(projected["budget"]["max_runs"], 3);
        assert_eq!(projected["budget"]["max_submissions"], 1);
        assert_eq!(projected["budget"]["max_verifications"], 1);
        assert_eq!(projected["expires_at"], "2026-07-28T00:00:00Z");
        assert_eq!(projected["agent_run"]["run_id"], run_id);
        assert_eq!(
            projected["agent_run"]["submission"]["state"],
            "ready_to_export"
        );
        assert!(
            projected["agent_run"]["next_commands"]["export"]
                .as_str()
                .unwrap()
                .contains(&attempt.attempt_id)
        );
        assert!(
            projected["agent_run"]["next_commands"]["submit"]
                .as_str()
                .unwrap()
                .contains("submission.json")
        );
        let receipt_root = decoded
            .agent_run_receipts
            .first()
            .unwrap()
            .receipt_root
            .clone();
        fs::write(&run_file, b"drifted helper-owned Run bytes\n").unwrap();
        let projected = attempt_list_entry(directory.path(), decoded.clone(), &path);
        assert_eq!(
            projected["agent_run"]["run_file"]["state"],
            "drifted_or_missing"
        );
        assert_eq!(
            projected["agent_run"]["submission"]["state"],
            "not_exportable"
        );
        assert!(projected["agent_run"]["next_commands"]["export"].is_null());
        fs::write(&run_file, &run_bytes).unwrap();
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
                        source_run: Some(run_id.to_string()),
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
            Some(CurrentRoutineAttempt {
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
        assert_eq!(
            retained
                .agent_run_submission_links
                .first()
                .map(|link| link.submission_id.as_str()),
            Some(first_submission.submission_id.as_str())
        );
        assert_eq!(
            retained.agent_run_receipts.first().unwrap().receipt_root,
            receipt_root
        );
        let projected = attempt_list_entry(
            directory.path(),
            retained.clone(),
            &attempt_path(directory.path(), "erdos:1056"),
        );
        assert_eq!(projected["agent_run"]["submission"]["state"], "registered");
        let second_submission = submission_for("second fixture");
        let error = authorize_submission(
            Some(&CurrentRoutineAttempt {
                attempt: retained.clone(),
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            &second_submission,
            64,
        )
        .unwrap_err();
        assert!(error.contains("already bound"), "{error}");
        assert!(attempt_path(directory.path(), "erdos:1056").is_file());

        record_submission_attempt(
            directory.path(),
            Some(CurrentRoutineAttempt {
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

        // Registering a Submission advances the repository/Target Index read
        // set without changing the authorized source, inputs, or packet. Old
        // Run receipts remain bound to the exact task binding they reserved,
        // while later Runs reserve the refreshed binding.
        let historical_binding = retained.agent_run_reservations[0]
            .target_task_binding
            .clone()
            .unwrap();
        let mut refreshed = retained.clone();
        let mut refreshed_binding = refreshed.target_task_binding.clone();
        refreshed_binding.target_index_root = format!("sha256:{}", "a".repeat(64));
        refreshed_binding.repository.repository_root = format!("sha256:{}", "b".repeat(64));
        refreshed_binding.claim_read_set.git_commit = "c".repeat(40);
        refreshed_binding.claim_read_set.git_tree = "d".repeat(40);
        refreshed_binding.binding_root = refreshed_binding.computed_binding_root().unwrap();
        let mut source_drift = refreshed_binding.clone();
        source_drift.source.git_commit = "e".repeat(40);
        source_drift.binding_root = source_drift.computed_binding_root().unwrap();
        let error =
            apply_refreshed_target_binding(&mut refreshed.clone(), source_drift).unwrap_err();
        assert!(error.contains("source, inputs, or packet"), "{error}");
        apply_refreshed_target_binding(&mut refreshed, refreshed_binding).unwrap();
        write(directory.path(), &refreshed).unwrap();
        let retained = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_ne!(
            retained.target_task_binding.binding_root,
            historical_binding.binding_root
        );
        assert_eq!(
            retained.agent_run_reservations[0]
                .target_task_binding
                .as_ref(),
            Some(&historical_binding)
        );

        // The one private v8 Attempt created before reservation bindings were
        // recorded remains readable without rewriting its ignored file. Such
        // reservations necessarily used the starting binding.
        let mut pre_binding_field = retained.clone();
        pre_binding_field.agent_run_reservations[0].target_task_binding = None;
        validate(&pre_binding_field).unwrap();

        // Re-rooting a tampered historical receipt cannot make it match the
        // reservation-time task binding.
        let mut tampered_receipt = retained.clone();
        tampered_receipt.agent_run_receipts[0]
            .result
            .target
            .claim_read_set
            .git_commit = retained
            .target_task_binding
            .claim_read_set
            .git_commit
            .clone();
        tampered_receipt.agent_run_receipts[0].receipt_root =
            agent_run_receipt_root(&tampered_receipt.agent_run_receipts[0]).unwrap();
        let error = validate(&tampered_receipt).unwrap_err();
        assert!(error.contains("exact Attempt"), "{error}");

        let second_submission = submission_for("Second bounded fixture.");
        let error = authorize_submission(
            Some(&CurrentRoutineAttempt {
                attempt: retained,
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            &second_submission,
            64,
        )
        .unwrap_err();
        assert!(error.contains("already bound"), "{error}");

        let second_request_root = format!("sha256:{}", "c".repeat(64));
        let mut resumed = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        let second_run_number = reserve_agent_run_for_output(&mut resumed, None).unwrap();
        resumed
            .agent_run_reservations
            .push(CurrentAgentRunReservation {
                run_number: second_run_number,
                request_root: second_request_root.clone(),
                target_task_binding: Some(resumed.target_task_binding.clone()),
            });
        write(directory.path(), &resumed).unwrap();

        let second_run_directory = retained_output.path().join("run-2");
        fs::create_dir(&second_run_directory).unwrap();
        let second_run_file = second_run_directory.join("run.json");
        let second_run_id = "run_0560106d-c4b0-4584-ad99-f7b1bf867487";
        let second_run_bytes = b"second private helper-owned Run bytes\n".to_vec();
        fs::write(&second_run_file, &second_run_bytes).unwrap();
        let second_evidence_file = second_run_directory.join("evidence-manifest.json");
        let second_evidence = json!({
            "schema": "canopus.run-evidence.v1",
            "run_id": second_run_id,
            "files": {"run": sha256_root(&second_run_bytes)},
        });
        let mut second_evidence_bytes =
            vela_protocol::canonical::to_canonical_bytes(&second_evidence).unwrap();
        second_evidence_bytes.push(b'\n');
        fs::write(&second_evidence_file, &second_evidence_bytes).unwrap();
        let mut second_output = helper_output.clone();
        second_output["request_root"] = Value::String(second_request_root.clone());
        second_output["target"]["binding_root"] =
            Value::String(resumed.target_task_binding.binding_root.clone());
        second_output["target"]["target_index_root"] =
            Value::String(resumed.target_task_binding.target_index_root.clone());
        second_output["target"]["claim_read_set"]["git_commit"] = Value::String(
            resumed
                .target_task_binding
                .claim_read_set
                .git_commit
                .clone(),
        );
        second_output["target"]["claim_read_set"]["git_tree"] =
            Value::String(resumed.target_task_binding.claim_read_set.git_tree.clone());
        second_output["source_state"]["commit"] = Value::String(
            resumed
                .target_task_binding
                .claim_read_set
                .git_commit
                .clone(),
        );
        second_output["source_state"]["tree"] =
            Value::String(resumed.target_task_binding.claim_read_set.git_tree.clone());
        second_output["run"]["id"] = Value::String(second_run_id.to_string());
        second_output["run"]["path"] = Value::String(second_run_file.display().to_string());
        second_output["run"]["size"] = json!(second_run_bytes.len());
        second_output["run"]["sha256"] = Value::String(sha256_root(&second_run_bytes));
        second_output["evidence_manifest"]["path"] =
            Value::String(second_evidence_file.display().to_string());
        second_output["evidence_manifest"]["size"] = json!(second_evidence_bytes.len());
        second_output["evidence_manifest"]["sha256"] =
            Value::String(sha256_root(&second_evidence_bytes));
        second_output["evidence_manifest"]["root"] =
            Value::String(canonical_root(&second_evidence).unwrap());
        let mut second_output_bytes = serde_json::to_vec(&second_output).unwrap();
        second_output_bytes.push(b'\n');
        record_agent_run_receipt(
            directory.path(),
            &attempt.attempt_id,
            &second_request_root,
            &second_output_bytes,
        )
        .unwrap();

        let resumed = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_eq!(resumed.usage.runs, 2);
        assert_eq!(resumed.agent_run_reservations.len(), 2);
        assert_eq!(resumed.agent_run_receipts.len(), 2);
        assert_eq!(resumed.agent_run_receipts[0].receipt_root, receipt_root);
        assert_eq!(
            resumed.agent_run_receipts[1]
                .previous_receipt_root
                .as_deref(),
            Some(receipt_root.as_str())
        );
        assert_eq!(resumed.agent_run_receipts[1].run_number, 2);
        let mut usage_drift = resumed.clone();
        usage_drift.usage.runs = 1;
        let error = validate(&usage_drift).unwrap_err();
        assert!(error.contains("exactly replay run usage"), "{error}");
        let mut chain_drift = resumed.clone();
        chain_drift.agent_run_receipts[1].previous_receipt_root = None;
        let error = validate(&chain_drift).unwrap_err();
        assert!(error.contains("exact root chain"), "{error}");
        let projected = attempt_list_entry(
            directory.path(),
            resumed.clone(),
            &attempt_path(directory.path(), "erdos:1056"),
        );
        assert_eq!(projected["agent_runs"].as_array().unwrap().len(), 2);
        assert_eq!(
            projected["agent_runs"][0]["submission"]["state"],
            "registered"
        );
        assert_eq!(projected["agent_run"]["run_id"], second_run_id);

        let third_request_root = format!("sha256:{}", "d".repeat(64));
        let mut resumed = resumed;
        let third_run_number = reserve_agent_run_for_output(&mut resumed, None).unwrap();
        resumed
            .agent_run_reservations
            .push(CurrentAgentRunReservation {
                run_number: third_run_number,
                request_root: third_request_root,
                target_task_binding: Some(resumed.target_task_binding.clone()),
            });
        write(directory.path(), &resumed).unwrap();
        let mut restarted = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_eq!(restarted.usage.runs, 3);
        assert_eq!(restarted.agent_run_reservations.len(), 3);
        let error = reserve_agent_run(&mut restarted).unwrap_err();
        assert!(error.contains("exhausted its Agent run budget"), "{error}");

        record_verification_attempt(
            directory.path(),
            Some(CurrentRoutineAttempt {
                attempt: read(&attempt_path(directory.path(), "erdos:1056")).unwrap(),
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            "vvr_fixture",
        )
        .unwrap();
        let retained = read(&attempt_path(directory.path(), "erdos:1056")).unwrap();
        assert_eq!(retained.usage.verifications, 1);
        assert_eq!(
            retained.usage.registered_verification_record_ids,
            vec!["vvr_fixture"]
        );
        let error = authorize_verification(
            Some(&CurrentRoutineAttempt {
                attempt: retained,
                path: attempt_path(directory.path(), "erdos:1056"),
                _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
            }),
            "vvr_second",
        )
        .unwrap_err();
        assert!(error.contains("Verification budget exhausted"));

        let expired = CurrentRoutineAttempt {
            attempt: read(&attempt_path(directory.path(), "erdos:1056")).unwrap(),
            path: attempt_path(directory.path(), "erdos:1056"),
            _lock: lock_attempt(directory.path(), "erdos:1056").unwrap(),
        };
        let after_expiry = chrono::DateTime::parse_from_rfc3339("2026-07-29T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let error = revalidate_routine_attempt_at(directory.path(), Some(&expired), after_expiry)
            .unwrap_err();
        assert!(error.contains("has expired"));
        drop(expired);
        assert!(
            resolve_verification_reconciliation_attempt(
                directory.path(),
                Some(&attempt.attempt_id)
            )
            .unwrap()
            .is_none(),
            "expired private scratch must not strand durable Verification replay"
        );
    }

    #[test]
    fn verification_reconciliation_fails_closed_on_corrupt_private_state() {
        let directory = tempfile::tempdir().unwrap();
        let attempt_directory = directory.path().join(".vela/work/corrupt");
        fs::create_dir_all(&attempt_directory).unwrap();
        fs::write(attempt_directory.join("attempt.json"), b"{not-json}\n").unwrap();

        let error = resolve_verification_reconciliation_attempt(
            directory.path(),
            Some("vat_0000000000000000"),
        )
        .err()
        .expect("corrupt private Attempt state must fail closed");
        assert!(error.contains("parse current Attempt"), "{error}");
    }
}
