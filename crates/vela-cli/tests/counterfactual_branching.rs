//! Same-lineage Git branching, sealed evaluation, branch isolation, metering,
//! and deterministic comparison over the shipped Vela CLI.
//!
//! This is campaign qualification, not a protocol object or a branch command.

#![cfg(all(unix, feature = "test-support"))]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use vela_protocol::review_method::{ReviewMethodV1, ReviewPerformerV1};

mod support;
use support::{
    EphemeralAgent, RemoveAnchorOnDrop, configure_git_identity, run_with_isolated_home as run,
    success_json,
};

const DEVICE: &str = "33333333333333333333333333333333";
const PRODUCER: &str = "agent:independent-js";
const VERIFIER: &str = "verifier:counterfactual-branching";
const EVALUATOR_SOURCE_PATH: &str = "crates/vela-cli/tests/counterfactual_branching.rs";
const METER_START: &str = "The branch-local Decision Inbox read begins.";
const METER_END: &str = "The branch-local authorized accept or reject Decision returns.";
const METER_EXCLUDED: [&str; 3] = [
    "Common pre-branch Submission and Verification setup.",
    "Post-Decision replay, comparison, and receipt persistence.",
    "Campaign authoring and supervisor review.",
];
const SEALED_PATHS: [&str; 3] = [
    "campaign/t3/sealed/task.json",
    "campaign/t3/sealed/evaluation.json",
    "campaign/t3/sealed/metering-plan.json",
];
const REQUIRED_METRICS: [&str; 15] = [
    "model_calls",
    "model_input_tokens",
    "model_output_tokens",
    "tool_invocations",
    "verifier_invocations",
    "solver_or_simulation_invocations",
    "wall_time_ms",
    "cpu_time_ms",
    "gpu_time_ms",
    "external_service_calls",
    "artifact_count",
    "artifact_bytes",
    "persistent_state_files",
    "persistent_state_bytes",
    "human_interventions",
];
const EVALUATION_CHECKS: [&str; 7] = [
    "Both branches name the same exact branch-point Git commit, tree, Repository root, origin, authority roots, and accepted-Standing commitment.",
    "The task, evaluation, and metering-plan blobs are unchanged from the branch point in both terminal histories.",
    "The accept branch has one accepted Claim and an accepted Proposal; the reject branch has no accepted Claim and a rejected Proposal.",
    "Each branch replays from a clean clone to its own terminal Repository root.",
    "A Decision in either branch does not change the other branch before its own Decision.",
    "Every required resource category is measured, not used, unavailable, or explicitly incomparable under the frozen metering plan.",
    "Comparison bytes and their SHA-256 root are identical across checkout paths.",
];

#[derive(Debug)]
struct TaskContract {
    submission_root: String,
    verification_requirement: String,
    accept_branch: String,
    reject_branch: String,
}

#[derive(Debug)]
struct MeteringContract {
    root: String,
    required_metrics: Vec<String>,
    start: String,
    end: String,
    excluded: Vec<String>,
}

#[derive(Debug)]
struct DecisionMeasurement {
    duration: Duration,
    tool_invocations: u64,
}

struct DecisionMeter<'a> {
    contract: &'a MeteringContract,
    started: Instant,
    tools: Vec<&'static str>,
}

impl<'a> DecisionMeter<'a> {
    fn start(contract: &'a MeteringContract) -> Self {
        assert_eq!(contract.start, METER_START);
        assert_eq!(contract.excluded, METER_EXCLUDED.map(str::to_owned));
        Self {
            contract,
            started: Instant::now(),
            tools: Vec::new(),
        }
    }

    fn record_tool(&mut self, tool: &'static str) {
        self.tools.push(tool);
    }

    fn finish(self) -> DecisionMeasurement {
        assert_eq!(self.contract.end, METER_END);
        assert_eq!(self.tools, ["vela review inbox", "vela review decision"]);
        DecisionMeasurement {
            duration: self.started.elapsed(),
            tool_invocations: self.tools.len().try_into().expect("tool invocation count"),
        }
    }
}

fn git(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repository)
        .args(args)
        .output()
        .expect("run Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git output is UTF-8")
        .trim()
        .to_string()
}

fn clone_repository(source: &Path, destination: &Path) {
    let output = Command::new("git")
        .args(["clone", "-q"])
        .arg(source)
        .arg(destination)
        .output()
        .expect("clone repository");
    assert!(
        output.status.success(),
        "git clone: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_on_device(cwd: &Path, socket: Option<&Path>, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(cwd)
        .args(args)
        .env("HOME", home)
        .env("NO_COLOR", "1")
        .env("VELA_ADVICE", "0")
        .env("VELA_TEST_DEVICE_IDENTIFIER", DEVICE)
        .env_remove("VELA_AGENT_KEY_HEX");
    match socket {
        Some(socket) => command.env("SSH_AUTH_SOCK", socket),
        None => command.env("SSH_AUTH_SOCK", cwd.join("missing-ssh-agent.sock")),
    };
    command.output().expect("run vela on synthetic device")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    value[field]
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{field} entries must be strings"))
        })
        .collect()
}

fn validate_task(value: &Value) -> Result<TaskContract, String> {
    if value["schema"] != "vela-compose-1.t3-task.v1" || value["authority_effect"] != "none" {
        return Err("unexpected T3 task contract".into());
    }
    let submission_root = value["subject"]["submission_root"]
        .as_str()
        .ok_or_else(|| "task submission_root must be a string".to_string())?
        .to_owned();
    let verification_requirement = value["subject"]["verification_requirement"]
        .as_str()
        .filter(|requirement| !requirement.is_empty())
        .ok_or_else(|| "task verification_requirement must be non-empty".to_string())?
        .to_owned();
    let variants = value["variants"]
        .as_array()
        .ok_or_else(|| "task variants must be an array".to_string())?;
    let mut by_decision = BTreeMap::new();
    for variant in variants {
        let decision = variant["decision"]
            .as_str()
            .ok_or_else(|| "task variant decision must be a string".to_string())?;
        let branch = variant["branch"]
            .as_str()
            .filter(|branch| !branch.is_empty())
            .ok_or_else(|| "task variant branch must be non-empty".to_string())?;
        if !matches!(decision, "accept" | "reject") {
            return Err(format!("unsupported task Decision variant {decision}"));
        }
        if by_decision.insert(decision, branch.to_owned()).is_some() {
            return Err(format!("duplicate task Decision variant {decision}"));
        }
    }
    if by_decision.len() != 2 || by_decision["accept"] == by_decision["reject"] {
        return Err("task must name distinct accept and reject branches".into());
    }
    Ok(TaskContract {
        submission_root,
        verification_requirement,
        accept_branch: by_decision
            .remove("accept")
            .expect("validated accept variant"),
        reject_branch: by_decision
            .remove("reject")
            .expect("validated reject variant"),
    })
}

fn validate_metering_plan(value: &Value, bytes: &[u8]) -> Result<MeteringContract, String> {
    if value["schema"] != "vela-compose-1.t3-metering-plan.v1"
        || value["authority_effect"] != "none"
    {
        return Err("unexpected T3 metering plan".into());
    }
    let required_metrics = string_array(value, "required_metrics")?;
    let expected_metrics = REQUIRED_METRICS
        .iter()
        .map(|metric| (*metric).to_owned())
        .collect::<Vec<_>>();
    if required_metrics != expected_metrics {
        return Err("metering plan inventory does not match the evaluator".into());
    }
    let boundary = &value["execution_boundary"];
    let start = boundary["start"]
        .as_str()
        .ok_or_else(|| "metering start boundary must be a string".to_string())?
        .to_owned();
    let end = boundary["end"]
        .as_str()
        .ok_or_else(|| "metering end boundary must be a string".to_string())?
        .to_owned();
    let excluded = string_array(boundary, "excluded")?;
    if start != METER_START || end != METER_END || excluded != METER_EXCLUDED.map(str::to_owned) {
        return Err("metering execution boundary does not match the evaluator".into());
    }
    for status in ["comparable", "incomparable", "unavailable", "not_used"] {
        if !value["comparison_rules"][status].is_string() {
            return Err(format!("metering comparison rule {status} is missing"));
        }
    }
    Ok(MeteringContract {
        root: vela_protocol::canonical::sha256_root(bytes),
        required_metrics,
        start,
        end,
        excluded,
    })
}

fn validate_evaluation(value: &Value, workspace: &Path) -> Result<(), String> {
    if value["schema"] != "vela-compose-1.t3-evaluation.v1" || value["authority_effect"] != "none" {
        return Err("unexpected T3 evaluation contract".into());
    }
    let checks = string_array(value, "checks")?;
    if checks != EVALUATION_CHECKS.map(str::to_owned) {
        return Err("evaluation check inventory does not match the evaluator".into());
    }
    let source_path = value["implementation"]["path"]
        .as_str()
        .ok_or_else(|| "evaluation implementation path must be a string".to_string())?;
    if source_path != EVALUATOR_SOURCE_PATH {
        return Err("evaluation implementation path does not match the evaluator".into());
    }
    let expected_root = value["implementation"]["sha256"]
        .as_str()
        .ok_or_else(|| "evaluation implementation SHA-256 must be a string".to_string())?;
    let source_bytes = std::fs::read(workspace.join(source_path))
        .map_err(|error| format!("read evaluation implementation: {error}"))?;
    let actual_root = vela_protocol::canonical::sha256_root(&source_bytes);
    if expected_root != actual_root {
        return Err(format!(
            "evaluation implementation drift: expected {expected_root}, got {actual_root}"
        ));
    }
    Ok(())
}

fn read_json(path: &Path) -> (Value, Vec<u8>) {
    let bytes = std::fs::read(path).expect("read JSON contract");
    let value = serde_json::from_slice(&bytes).expect("parse JSON contract");
    (value, bytes)
}

fn bind_task_to_submission(
    task: &TaskContract,
    submitted: &Value,
    repository: &Path,
) -> Result<(), String> {
    if submitted["submission_root"].as_str() != Some(task.submission_root.as_str()) {
        return Err("imported Submission root does not match the sealed task".into());
    }
    let manifest = repository_manifest(repository);
    let retained_path = manifest["submissions"][0]["path"]
        .as_str()
        .ok_or_else(|| "retained Submission path is missing".to_string())?;
    let retained_bytes = std::fs::read(repository.join(retained_path))
        .map_err(|error| format!("read retained Submission: {error}"))?;
    if vela_protocol::canonical::sha256_root(&retained_bytes) != task.submission_root {
        return Err("retained Submission bytes do not match the sealed task root".into());
    }
    let envelope: Value = serde_json::from_slice(&retained_bytes)
        .map_err(|error| format!("parse retained Submission envelope: {error}"))?;
    let payload = vela_protocol::dsse::decode_base64(
        "retained Submission payload",
        envelope["payload"]
            .as_str()
            .ok_or_else(|| "retained Submission payload is missing".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let submission: Value = serde_json::from_slice(&payload)
        .map_err(|error| format!("parse retained Submission payload: {error}"))?;
    if string_array(&submission, "verification_requirements")?
        != [task.verification_requirement.clone()]
    {
        return Err("imported verification requirement does not match the sealed task".into());
    }
    Ok(())
}

fn write_canonical(path: &Path, value: &Value) -> Vec<u8> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value).expect("canonical JSON");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create canonical output directory");
    }
    std::fs::write(path, &bytes).expect("write canonical JSON");
    bytes
}

fn repository_manifest(repository: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(repository.join(".vela/repository.json")).expect("read repository manifest"),
    )
    .expect("parse repository manifest")
}

fn accepted_standing_commitment(repository: &Path) -> String {
    let manifest = repository_manifest(repository);
    vela_protocol::canonical::sha256_canonical(&manifest["accepted_claims"])
        .expect("accepted Standing commitment")
}

fn changed_paths(repository: &Path, base_commit: &str) -> Vec<String> {
    let output = git(
        repository,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--name-only",
            "--no-renames",
            &format!("{base_commit}..HEAD"),
            "--",
        ],
    );
    let mut paths = output
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn changed_file_bytes(repository: &Path, base_commit: &str) -> u64 {
    changed_paths(repository, base_commit)
        .iter()
        .filter_map(|path| std::fs::metadata(repository.join(path)).ok())
        .map(|metadata| metadata.len())
        .sum()
}

fn metric(
    name: &str,
    status: &str,
    value: Option<u64>,
    unit: &str,
    comparison: &str,
    reason: &str,
) -> Value {
    json!({
        "comparison": comparison,
        "name": name,
        "reason": reason,
        "status": status,
        "unit": unit,
        "value": value,
    })
}

fn metering_receipt(
    branch: &str,
    base_commit: &str,
    base_repository_root: &str,
    decision_git_commit: &str,
    terminal_repository_root: &str,
    decision: &Value,
    decision_bytes: &[u8],
    measurement: &DecisionMeasurement,
    metering_plan_root: &str,
    persistent_files: u64,
    persistent_bytes: u64,
) -> Value {
    let zero = |name, unit, reason| metric(name, "not_used", Some(0), unit, "comparable", reason);
    let metrics = vec![
        zero(
            "model_calls",
            "calls",
            "The scoped branch action invoked only Git and the local Vela CLI.",
        ),
        zero(
            "model_input_tokens",
            "tokens",
            "No model call occurred inside the frozen execution boundary.",
        ),
        zero(
            "model_output_tokens",
            "tokens",
            "No model call occurred inside the frozen execution boundary.",
        ),
        metric(
            "tool_invocations",
            "measured",
            Some(measurement.tool_invocations),
            "top_level_calls",
            "comparable",
            "The harness counted one Decision Inbox read and one Vela Decision call; subprocesses inside Vela are excluded.",
        ),
        zero(
            "verifier_invocations",
            "calls",
            "The one scoped Verification Record was retained before the branch point.",
        ),
        zero(
            "solver_or_simulation_invocations",
            "calls",
            "No solver or simulation ran inside the branch action.",
        ),
        metric(
            "wall_time_ms",
            "measured",
            Some(
                measurement
                    .duration
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            "milliseconds",
            "incomparable",
            "The actions ran sequentially on one shared host, so elapsed time is retained but not used for a branch effect estimate.",
        ),
        metric(
            "cpu_time_ms",
            "unavailable",
            None,
            "milliseconds",
            "unavailable",
            "The test runner did not expose reliable per-branch child-process CPU accounting.",
        ),
        zero(
            "gpu_time_ms",
            "milliseconds",
            "No GPU process or device was requested by the scoped action.",
        ),
        zero(
            "external_service_calls",
            "calls",
            "The fixture used local Git, a disposable local SSH agent, and the local Vela binary without network access.",
        ),
        metric(
            "artifact_count",
            "measured",
            Some(1),
            "artifacts",
            "comparable",
            "The branch retained the canonical JSON returned by its Decision call.",
        ),
        metric(
            "artifact_bytes",
            "measured",
            Some(
                decision_bytes
                    .len()
                    .try_into()
                    .expect("artifact byte count"),
            ),
            "bytes",
            "comparable",
            "Exact canonical Decision-command output bytes.",
        ),
        metric(
            "persistent_state_files",
            "measured",
            Some(persistent_files),
            "files",
            "comparable",
            "Tracked files changed by the branch Decision relative to the exact branch point, before evaluator receipt persistence.",
        ),
        metric(
            "persistent_state_bytes",
            "measured",
            Some(persistent_bytes),
            "bytes",
            "comparable",
            "Current byte sizes of tracked files changed by the branch Decision, before evaluator receipt persistence.",
        ),
        zero(
            "human_interventions",
            "interventions",
            "The fixture used an attributed synthetic agent performer; campaign authoring and supervisor review are outside the frozen branch boundary.",
        ),
    ];
    json!({
        "authority_effect": "none",
        "base": {
            "git_commit": base_commit,
            "repository_root": base_repository_root,
        },
        "branch": branch,
        "decision": {
            "action": decision["action"],
            "authority_record_root": decision["authority_record_root"],
            "proposal_id": decision["proposal_id"],
        },
        "decision_git_commit": decision_git_commit,
        "metering_plan_root": metering_plan_root,
        "metrics": metrics,
        "output_artifact": {
            "bytes": decision_bytes.len(),
            "root": vela_protocol::canonical::sha256_root(decision_bytes),
        },
        "schema": "vela-compose-1.t3-metering-receipt.v1",
        "terminal_repository_root": terminal_repository_root,
    })
}

fn validate_metering(
    receipt: &Value,
    plan: &MeteringContract,
) -> Result<BTreeMap<String, Value>, String> {
    if receipt["schema"] != "vela-compose-1.t3-metering-receipt.v1" {
        return Err("unexpected metering receipt schema".into());
    }
    if receipt["metering_plan_root"] != plan.root {
        return Err("metering receipt is not bound to the frozen plan".into());
    }
    let entries = receipt["metrics"]
        .as_array()
        .ok_or_else(|| "metrics must be an array".to_string())?;
    let mut metrics = BTreeMap::new();
    for entry in entries {
        let name = entry["name"]
            .as_str()
            .ok_or_else(|| "metric name must be a string".to_string())?;
        if metrics.insert(name.to_string(), entry.clone()).is_some() {
            return Err(format!("duplicate metric {name}"));
        }
        let status = entry["status"]
            .as_str()
            .ok_or_else(|| format!("metric {name} has no status"))?;
        let comparison = entry["comparison"]
            .as_str()
            .ok_or_else(|| format!("metric {name} has no comparison status"))?;
        match status {
            "measured" => {
                if !entry["value"].is_u64() {
                    return Err(format!("measured metric {name} has no value"));
                }
            }
            "not_used" => {
                if entry["value"].as_u64() != Some(0) {
                    return Err(format!("not-used metric {name} must be zero"));
                }
            }
            "unavailable" => {
                if !entry["value"].is_null() || comparison != "unavailable" {
                    return Err(format!("unavailable metric {name} is ambiguous"));
                }
            }
            other => return Err(format!("metric {name} has unsupported status {other}")),
        }
        if !matches!(comparison, "comparable" | "incomparable" | "unavailable") {
            return Err(format!(
                "metric {name} has unsupported comparison status {comparison}"
            ));
        }
    }
    let actual = metrics.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = plan
        .required_metrics
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("metering coverage mismatch: {actual:?}"));
    }
    Ok(metrics)
}

fn persist_branch_evidence(
    repository: &Path,
    branch_label: &str,
    base_commit: &str,
    base_repository_root: &str,
    decision: &Value,
    measurement: &DecisionMeasurement,
    metering_plan: &MeteringContract,
    home: &Path,
) -> Value {
    let decision_git_commit = git(repository, &["rev-parse", "HEAD^{commit}"]);
    let replay = success_json(&run(repository, None, home, &["replay", ".", "--json"]));
    let decision_bytes =
        vela_protocol::canonical::to_canonical_bytes(decision).expect("canonical Decision output");
    let paths = changed_paths(repository, base_commit);
    let receipt = metering_receipt(
        &git(repository, &["branch", "--show-current"]),
        base_commit,
        base_repository_root,
        &decision_git_commit,
        replay["repository_root"]
            .as_str()
            .expect("terminal repository root"),
        decision,
        &decision_bytes,
        measurement,
        &metering_plan.root,
        paths.len().try_into().expect("changed path count"),
        changed_file_bytes(repository, base_commit),
    );
    validate_metering(&receipt, metering_plan).expect("complete branch metering");

    let result_root = repository.join(format!("campaign/t3/results/{branch_label}"));
    std::fs::create_dir_all(&result_root).expect("create branch result directory");
    std::fs::write(result_root.join("decision.json"), &decision_bytes)
        .expect("write Decision artifact");
    write_canonical(&result_root.join("metering.json"), &receipt);
    git(repository, &["add", "--", "campaign/t3/results"]);
    git(
        repository,
        &[
            "commit",
            "-qm",
            &format!("Retain {branch_label} branch evidence"),
        ],
    );
    let replay_after_receipt =
        success_json(&run(repository, None, home, &["replay", ".", "--json"]));
    assert_eq!(
        replay_after_receipt["repository_root"], replay["repository_root"],
        "source-owned receipts must not alter governed state"
    );
    receipt
}

fn sealed_input_receipts(repository: &Path, base_commit: &str) -> Result<Vec<Value>, String> {
    let mut receipts = Vec::new();
    for path in SEALED_PATHS {
        let base_spec = format!("{base_commit}:{path}");
        let base_blob = git(repository, &["rev-parse", &base_spec]);
        let head_blob = git(repository, &["rev-parse", &format!("HEAD:{path}")]);
        let worktree_blob = git(repository, &["hash-object", "--", path]);
        if base_blob != head_blob || base_blob != worktree_blob {
            return Err(format!("sealed input drifted: {path}"));
        }
        let bytes = std::fs::read(repository.join(path))
            .map_err(|error| format!("read sealed input {path}: {error}"))?;
        receipts.push(json!({
            "git_blob": base_blob,
            "path": path,
            "sha256": vela_protocol::canonical::sha256_root(&bytes),
        }));
    }
    Ok(receipts)
}

fn branch_summary(repository: &Path, branch_label: &str, base_commit: &str, home: &Path) -> Value {
    let replay = success_json(&run(repository, None, home, &["replay", ".", "--json"]));
    let decision: Value = serde_json::from_slice(
        &std::fs::read(
            repository.join(format!("campaign/t3/results/{branch_label}/decision.json")),
        )
        .expect("read Decision artifact"),
    )
    .expect("parse Decision artifact");
    let receipt_bytes =
        std::fs::read(repository.join(format!("campaign/t3/results/{branch_label}/metering.json")))
            .expect("read metering receipt");
    let receipt: Value = serde_json::from_slice(&receipt_bytes).expect("parse metering receipt");
    let (plan_value, plan_bytes) =
        read_json(&repository.join("campaign/t3/sealed/metering-plan.json"));
    let plan = validate_metering_plan(&plan_value, &plan_bytes).expect("bound metering plan");
    validate_metering(&receipt, &plan).expect("valid metering receipt");
    let proposal_id = decision["proposal_id"]
        .as_str()
        .expect("Decision Proposal id");
    let review = success_json(&run(
        repository,
        None,
        home,
        &["review", "show", ".", proposal_id, "--json"],
    ));
    json!({
        "accepted_standing_commitment": accepted_standing_commitment(repository),
        "branch": git(repository, &["branch", "--show-current"]),
        "changed_paths": changed_paths(repository, base_commit),
        "decision": decision["action"],
        "decision_authority_record_root": decision["authority_record_root"],
        "git_commit": replay["git_commit"],
        "git_tree": replay["git_tree"],
        "metering": receipt,
        "metering_root": vela_protocol::canonical::sha256_root(&receipt_bytes),
        "proposal_status": review["status"],
        "repository_root": replay["repository_root"],
        "standing": replay["counts"],
    })
}

fn metric_comparison(accept: &Value, reject: &Value, plan: &MeteringContract) -> Vec<Value> {
    let accept_metrics = validate_metering(accept, plan).expect("accept metering");
    let reject_metrics = validate_metering(reject, plan).expect("reject metering");
    plan.required_metrics
        .iter()
        .map(|name| {
            let left = &accept_metrics[name];
            let right = &reject_metrics[name];
            let comparison = if left["status"] == "unavailable" || right["status"] == "unavailable"
            {
                "unavailable"
            } else if left["comparison"] != "comparable"
                || right["comparison"] != "comparable"
                || left["unit"] != right["unit"]
            {
                "incomparable"
            } else {
                "comparable"
            };
            let delta = if comparison == "comparable" {
                Some(
                    right["value"].as_i64().expect("reject metric value")
                        - left["value"].as_i64().expect("accept metric value"),
                )
            } else {
                None
            };
            json!({
                "accept": {"status": left["status"], "value": left["value"]},
                "comparison": comparison,
                "delta_reject_minus_accept": delta,
                "name": name,
                "reject": {"status": right["status"], "value": right["value"]},
                "unit": left["unit"],
            })
        })
        .collect()
}

fn comparison_report(
    accept_repository: &Path,
    reject_repository: &Path,
    base: &Value,
    home: &Path,
) -> Value {
    let base_commit = base["git_commit"].as_str().expect("base Git commit");
    let (plan_value, plan_bytes) =
        read_json(&accept_repository.join("campaign/t3/sealed/metering-plan.json"));
    let plan = validate_metering_plan(&plan_value, &plan_bytes).expect("accept metering plan");
    let (_, reject_plan_bytes) =
        read_json(&reject_repository.join("campaign/t3/sealed/metering-plan.json"));
    assert_eq!(plan_bytes, reject_plan_bytes);
    let accept = branch_summary(accept_repository, "accept", base_commit, home);
    let reject = branch_summary(reject_repository, "reject", base_commit, home);
    assert_eq!(
        sealed_input_receipts(accept_repository, base_commit).expect("sealed accept inputs"),
        sealed_input_receipts(reject_repository, base_commit).expect("sealed reject inputs")
    );
    assert_ne!(accept["git_commit"], reject["git_commit"]);
    assert_ne!(
        accept["decision_authority_record_root"],
        reject["decision_authority_record_root"]
    );
    assert_ne!(accept["repository_root"], reject["repository_root"]);
    json!({
        "authority_effect": "none",
        "base": base,
        "branches": [accept.clone(), reject.clone()],
        "diff": {
            "accepted_claims": {
                "accept": accept["standing"]["accepted_claims"],
                "reject": reject["standing"]["accepted_claims"],
            },
            "decision": {
                "accept": accept["decision"],
                "reject": reject["decision"],
            },
            "metrics": metric_comparison(&accept["metering"], &reject["metering"], &plan),
            "proposal_status": {
                "accept": accept["proposal_status"],
                "reject": reject["proposal_status"],
            },
        },
        "schema": "vela-compose-1.t3-branch-comparison.v1",
        "sealed_inputs": sealed_input_receipts(accept_repository, base_commit)
            .expect("sealed comparison inputs"),
    })
}

#[test]
fn same_governed_root_branches_are_isolated_metered_and_deterministically_compared() {
    let temporary = tempfile::tempdir().expect("temporary fixture directory");
    let workspace = workspace_root();
    let fixture_source = workspace.join("docs/campaigns/vela-compose-1/fixtures/t3");
    let (task_value, _) = read_json(&fixture_source.join("task.json"));
    let task = validate_task(&task_value).expect("bound T3 task");
    let (metering_value, metering_bytes) = read_json(&fixture_source.join("metering-plan.json"));
    let metering =
        validate_metering_plan(&metering_value, &metering_bytes).expect("bound T3 metering plan");
    let (evaluation_value, _) = read_json(&fixture_source.join("evaluation.json"));
    validate_evaluation(&evaluation_value, &workspace).expect("bound T3 evaluation");

    let mut invalid_task = task_value.clone();
    invalid_task["variants"][1]["decision"] = json!("accept");
    assert!(validate_task(&invalid_task).is_err());
    let mut invalid_metering = metering_value.clone();
    invalid_metering["required_metrics"]
        .as_array_mut()
        .expect("mutable required metrics")
        .pop();
    assert!(validate_metering_plan(&invalid_metering, &metering_bytes).is_err());
    let mut invalid_boundary = metering_value.clone();
    invalid_boundary["execution_boundary"]["start"] = json!("After the Decision returns.");
    assert!(validate_metering_plan(&invalid_boundary, &metering_bytes).is_err());
    let mut invalid_evaluation = evaluation_value.clone();
    invalid_evaluation["checks"]
        .as_array_mut()
        .expect("mutable evaluation checks")
        .pop();
    assert!(validate_evaluation(&invalid_evaluation, &workspace).is_err());
    let mut invalid_implementation = evaluation_value.clone();
    invalid_implementation["implementation"]["sha256"] =
        json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    assert!(validate_evaluation(&invalid_implementation, &workspace).is_err());
    let mut invalid_implementation_path = evaluation_value.clone();
    invalid_implementation_path["implementation"]["path"] = json!("tests/other-evaluator.rs");
    assert!(validate_evaluation(&invalid_implementation_path, &workspace).is_err());

    let agent_root = temporary.path().join("authority-agent");
    let operator_home = temporary.path().join("operator-home");
    let verifier_home = temporary.path().join("verifier-home");
    for directory in [&agent_root, &operator_home, &verifier_home] {
        std::fs::create_dir_all(directory).expect("create fixture directory");
    }
    let agent = EphemeralAgent::start(&agent_root, "counterfactual branch authority");
    let source_repository = temporary.path().join("source-repository");
    let source_repository_text = source_repository.to_string_lossy().into_owned();
    let initialized_output = run_on_device(
        temporary.path(),
        Some(agent.socket()),
        &operator_home,
        &[
            "init",
            &source_repository_text,
            "--name",
            "Counterfactual branching fixture",
            "--scope",
            "Govern one synthetic Decision across controlled Git branches.",
            "--json",
        ],
    );
    let _anchor =
        RemoveAnchorOnDrop::from_init_json(&String::from_utf8_lossy(&initialized_output.stdout))
            .expect("init trust anchor");
    let initialized = success_json(&initialized_output);
    configure_git_identity(&source_repository);

    for (source_name, retained_name) in [
        ("task.json", "task.json"),
        ("evaluation.json", "evaluation.json"),
        ("metering-plan.json", "metering-plan.json"),
    ] {
        let destination = source_repository
            .join("campaign/t3/sealed")
            .join(retained_name);
        std::fs::create_dir_all(destination.parent().expect("sealed input parent"))
            .expect("create sealed input directory");
        std::fs::copy(fixture_source.join(source_name), destination)
            .expect("copy campaign-owned sealed input");
    }
    let method_path = "campaign/t3/sealed/review-method.json";
    let method = ReviewMethodV1 {
        schema: vela_protocol::review_method::REVIEW_METHOD_V1_SCHEMA.into(),
        profile: "counterfactual-branching-recompute-v1".into(),
        property: task.verification_requirement.clone(),
        question: "Do the exact retained fixture bytes equal the bounded result 42?".into(),
        reviewer: ReviewPerformerV1 {
            kind: "deterministic_tool".into(),
            display_name: "Counterfactual branching fixture checker".into(),
            identifier: "exact-bytes-42".into(),
            provider: None,
            version: None,
        },
        attested_by_actor_id: VERIFIER.into(),
        procedure: vec!["Compare the retained Artifact byte-for-byte with `42\\n`.".into()],
        required_output: vec!["Report only the bounded exact-byte outcome.".into()],
        does_not_establish: vec![
            "Scientific truth, cumulative value, or Standing without a Decision.".into(),
        ],
    };
    std::fs::write(
        source_repository.join(method_path),
        vela_protocol::canonical::to_canonical_bytes(&method).expect("canonical Review Method"),
    )
    .expect("write Review Method");
    git(&source_repository, &["add", "--", "campaign/t3/sealed"]);
    git(
        &source_repository,
        &["commit", "-qm", "Seal T3 task and evaluation inputs"],
    );

    let submission_path = workspace.join("conformance/current-objects/submission.json");
    let submitted = success_json(&run(
        &source_repository,
        None,
        &operator_home,
        &[
            "submit",
            submission_path.to_string_lossy().as_ref(),
            "--repo",
            ".",
            "--json",
        ],
    ));
    assert_eq!(submitted["accepted_state_changed"], false);
    bind_task_to_submission(&task, &submitted, &source_repository)
        .expect("sealed task matches imported Submission");
    let mismatched_task = TaskContract {
        submission_root: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
        verification_requirement: task.verification_requirement.clone(),
        accept_branch: task.accept_branch.clone(),
        reject_branch: task.reject_branch.clone(),
    };
    assert!(bind_task_to_submission(&mismatched_task, &submitted, &source_repository).is_err());
    let mismatched_requirement = TaskContract {
        submission_root: task.submission_root.clone(),
        verification_requirement: "A different unsealed requirement.".into(),
        accept_branch: task.accept_branch.clone(),
        reject_branch: task.reject_branch.clone(),
    };
    assert!(
        bind_task_to_submission(&mismatched_requirement, &submitted, &source_repository).is_err()
    );
    let proposal_id = submitted["proposal_id"]
        .as_str()
        .expect("pending Proposal id");
    let verified = success_json(&run(
        &source_repository,
        None,
        &verifier_home,
        &[
            "verification",
            "record",
            ".",
            proposal_id,
            "--profile",
            "counterfactual-branching-recompute-v1",
            "--method",
            method_path,
            "--property",
            &task.verification_requirement,
            "--outcome",
            "pass",
            "--does-not-establish",
            "Scientific truth, cumulative value, or Standing without a Decision.",
            "--independent-of",
            PRODUCER,
            "--as",
            VERIFIER,
            "--json",
        ],
    ));
    assert_eq!(verified["outcome"], "pass");
    assert_eq!(verified["accepted_event_delta"], 0);

    let base_replay = success_json(&run(
        &source_repository,
        None,
        &operator_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(base_replay["counts"]["accepted_claims"], 0);
    assert_eq!(base_replay["counts"]["pending_claims"], 1);
    let base_commit = git(&source_repository, &["rev-parse", "HEAD^{commit}"]);
    let base = json!({
        "accepted_standing_commitment": accepted_standing_commitment(&source_repository),
        "authority_keyset_root": base_replay["authority_keyset_root"],
        "authority_model_root": base_replay["authority_model_root"],
        "authority_sequence_one_root": initialized["authority"]["record_root"],
        "git_commit": base_commit,
        "git_tree": base_replay["git_tree"],
        "origin_id": base_replay["origin_id"],
        "origin_root": base_replay["origin_root"],
        "repository_id": base_replay["repository_id"],
        "repository_root": base_replay["repository_root"],
    });

    let accept_repository = temporary.path().join("accept-repository");
    let reject_repository = temporary.path().join("reject-repository");
    clone_repository(&source_repository, &accept_repository);
    clone_repository(&source_repository, &reject_repository);
    for (repository, branch) in [
        (&accept_repository, task.accept_branch.as_str()),
        (&reject_repository, task.reject_branch.as_str()),
    ] {
        configure_git_identity(repository);
        git(repository, &["switch", "-q", "-c", branch]);
        let start = success_json(&run(
            repository,
            None,
            &operator_home,
            &["replay", ".", "--json"],
        ));
        for key in [
            "repository_id",
            "origin_id",
            "origin_root",
            "repository_root",
            "authority_keyset_root",
            "authority_model_root",
            "git_commit",
            "git_tree",
        ] {
            assert_eq!(start[key], base[key], "branch-point mismatch for {key}");
        }
        assert_eq!(
            accepted_standing_commitment(repository),
            base["accepted_standing_commitment"]
        );
        assert_eq!(
            git(repository, &["merge-base", "HEAD", &base_commit]),
            base_commit
        );
        sealed_input_receipts(repository, &base_commit).expect("sealed branch inputs");
    }

    let mut accept_meter = DecisionMeter::start(&metering);
    let accept_inbox = success_json(&run(
        &accept_repository,
        None,
        &operator_home,
        &["review", "inbox", ".", "--json"],
    ));
    accept_meter.record_tool("vela review inbox");
    let accept_entry_root = accept_inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("accept entry root");
    let accepted = success_json(&run_on_device(
        &accept_repository,
        Some(agent.socket()),
        &operator_home,
        &[
            "review",
            "accept",
            ".",
            proposal_id,
            "--if-entry-root",
            accept_entry_root,
            "--reason",
            "Accept the exact synthetic Claim in this controlled branch.",
            "--as",
            "agent:counterfactual-acceptor",
            "--session-ref",
            "fixture:counterfactual:accept",
            "--json",
        ],
    ));
    accept_meter.record_tool("vela review decision");
    let accept_measurement = accept_meter.finish();
    assert_eq!(accepted["action"], "accept");
    assert_eq!(accepted["scientific_state_changed"], true);

    let reject_after_accept = success_json(&run(
        &reject_repository,
        None,
        &operator_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(
        reject_after_accept["repository_root"], base["repository_root"],
        "the accept Decision must not contaminate the untouched reject branch"
    );
    assert_eq!(reject_after_accept["counts"]["accepted_claims"], 0);
    let accept_receipt = persist_branch_evidence(
        &accept_repository,
        "accept",
        &base_commit,
        base["repository_root"].as_str().expect("base root"),
        &accepted,
        &accept_measurement,
        &metering,
        &operator_home,
    );

    let mut reject_meter = DecisionMeter::start(&metering);
    let reject_inbox = success_json(&run(
        &reject_repository,
        None,
        &operator_home,
        &["review", "inbox", ".", "--json"],
    ));
    reject_meter.record_tool("vela review inbox");
    let reject_entry_root = reject_inbox["entries"][0]["entry_root"]
        .as_str()
        .expect("reject entry root");
    assert_eq!(accept_entry_root, reject_entry_root);
    let rejected = success_json(&run_on_device(
        &reject_repository,
        Some(agent.socket()),
        &operator_home,
        &[
            "review",
            "reject",
            ".",
            proposal_id,
            "--if-entry-root",
            reject_entry_root,
            "--reason",
            "Reject the exact synthetic Claim in this controlled branch.",
            "--as",
            "agent:counterfactual-rejector",
            "--session-ref",
            "fixture:counterfactual:reject",
            "--json",
        ],
    ));
    reject_meter.record_tool("vela review decision");
    let reject_measurement = reject_meter.finish();
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);
    let reject_receipt = persist_branch_evidence(
        &reject_repository,
        "reject",
        &base_commit,
        base["repository_root"].as_str().expect("base root"),
        &rejected,
        &reject_measurement,
        &metering,
        &operator_home,
    );

    let accept_after_reject = success_json(&run(
        &accept_repository,
        None,
        &operator_home,
        &["replay", ".", "--json"],
    ));
    assert_eq!(
        accept_after_reject["repository_root"], accept_receipt["terminal_repository_root"],
        "the reject Decision must not contaminate the completed accept branch"
    );
    assert_ne!(
        accept_receipt["terminal_repository_root"],
        reject_receipt["terminal_repository_root"]
    );
    assert!(
        !accept_repository
            .join("campaign/t3/results/reject")
            .exists()
    );
    assert!(
        !reject_repository
            .join("campaign/t3/results/accept")
            .exists()
    );

    let comparison = comparison_report(
        &accept_repository,
        &reject_repository,
        &base,
        &operator_home,
    );
    assert_eq!(comparison["diff"]["accepted_claims"]["accept"], 1);
    assert_eq!(comparison["diff"]["accepted_claims"]["reject"], 0);
    assert_eq!(comparison["diff"]["proposal_status"]["accept"], "accepted");
    assert_eq!(comparison["diff"]["proposal_status"]["reject"], "rejected");
    let metric_diffs = comparison["diff"]["metrics"]
        .as_array()
        .expect("metric comparison array");
    assert_eq!(
        metric_diffs
            .iter()
            .find(|entry| entry["name"] == "cpu_time_ms")
            .expect("CPU comparison")["comparison"],
        "unavailable"
    );
    assert_eq!(
        metric_diffs
            .iter()
            .find(|entry| entry["name"] == "wall_time_ms")
            .expect("wall comparison")["comparison"],
        "incomparable"
    );
    let comparison_bytes = vela_protocol::canonical::to_canonical_bytes(&comparison)
        .expect("canonical comparison bytes");
    let comparison_root = vela_protocol::canonical::sha256_root(&comparison_bytes);
    assert_eq!(
        comparison_bytes,
        vela_protocol::canonical::to_canonical_bytes(&comparison_report(
            &accept_repository,
            &reject_repository,
            &base,
            &operator_home,
        ))
        .expect("repeated canonical comparison")
    );

    let accept_clone = temporary.path().join("accept-terminal-clone");
    let reject_clone = temporary.path().join("reject-terminal-clone");
    clone_repository(&accept_repository, &accept_clone);
    clone_repository(&reject_repository, &reject_clone);
    let cloned_comparison = comparison_report(&accept_clone, &reject_clone, &base, &operator_home);
    let cloned_bytes = vela_protocol::canonical::to_canonical_bytes(&cloned_comparison)
        .expect("canonical cloned comparison");
    assert_eq!(cloned_bytes, comparison_bytes);
    assert_eq!(
        vela_protocol::canonical::sha256_root(&cloned_bytes),
        comparison_root
    );

    let mut incomplete_receipt = accept_receipt.clone();
    incomplete_receipt["metrics"]
        .as_array_mut()
        .expect("mutable metric array")
        .retain(|entry| entry["name"] != "cpu_time_ms");
    assert!(validate_metering(&incomplete_receipt, &metering).is_err());
    std::fs::write(
        accept_clone.join("campaign/t3/sealed/evaluation.json"),
        b"{}\n",
    )
    .expect("mutate disposable sealed evaluation");
    assert!(sealed_input_receipts(&accept_clone, &base_commit).is_err());
}
