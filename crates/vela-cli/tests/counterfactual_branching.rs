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
const REQUIREMENT: &str = "Recompute the result from the exact fixture bytes.";
const ACCEPT_BRANCH: &str = "counterfactual/accept";
const REJECT_BRANCH: &str = "counterfactual/reject";
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
    duration: Duration,
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
            Some(2),
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
            Some(duration.as_millis().try_into().unwrap_or(u64::MAX)),
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
        "metrics": metrics,
        "output_artifact": {
            "bytes": decision_bytes.len(),
            "root": vela_protocol::canonical::sha256_root(decision_bytes),
        },
        "schema": "vela-compose-1.t3-metering-receipt.v1",
        "terminal_repository_root": terminal_repository_root,
    })
}

fn validate_metering(receipt: &Value) -> Result<BTreeMap<String, Value>, String> {
    if receipt["schema"] != "vela-compose-1.t3-metering-receipt.v1" {
        return Err("unexpected metering receipt schema".into());
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
    let expected = REQUIRED_METRICS.into_iter().collect::<BTreeSet<_>>();
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
    duration: Duration,
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
        duration,
        paths.len().try_into().expect("changed path count"),
        changed_file_bytes(repository, base_commit),
    );
    validate_metering(&receipt).expect("complete branch metering");

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
    validate_metering(&receipt).expect("valid metering receipt");
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

fn metric_comparison(accept: &Value, reject: &Value) -> Vec<Value> {
    let accept_metrics = validate_metering(accept).expect("accept metering");
    let reject_metrics = validate_metering(reject).expect("reject metering");
    REQUIRED_METRICS
        .iter()
        .map(|name| {
            let left = &accept_metrics[*name];
            let right = &reject_metrics[*name];
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
            "metrics": metric_comparison(&accept["metering"], &reject["metering"]),
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

    let fixture_source = workspace_root().join("docs/campaigns/vela-compose-1/fixtures/t3");
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
        property: REQUIREMENT.into(),
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

    let submission_path = workspace_root().join("conformance/current-objects/submission.json");
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
            REQUIREMENT,
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
        (&accept_repository, ACCEPT_BRANCH),
        (&reject_repository, REJECT_BRANCH),
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

    let accept_started = Instant::now();
    let accept_inbox = success_json(&run(
        &accept_repository,
        None,
        &operator_home,
        &["review", "inbox", ".", "--json"],
    ));
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
    let accept_duration = accept_started.elapsed();
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
        accept_duration,
        &operator_home,
    );

    let reject_started = Instant::now();
    let reject_inbox = success_json(&run(
        &reject_repository,
        None,
        &operator_home,
        &["review", "inbox", ".", "--json"],
    ));
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
    let reject_duration = reject_started.elapsed();
    assert_eq!(rejected["action"], "reject");
    assert_eq!(rejected["scientific_state_changed"], false);
    let reject_receipt = persist_branch_evidence(
        &reject_repository,
        "reject",
        &base_commit,
        base["repository_root"].as_str().expect("base root"),
        &rejected,
        reject_duration,
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
    assert!(validate_metering(&incomplete_receipt).is_err());
    std::fs::write(
        accept_clone.join("campaign/t3/sealed/evaluation.json"),
        b"{}\n",
    )
    .expect("mutate disposable sealed evaluation");
    assert!(sealed_input_receipts(&accept_clone, &base_commit).is_err());
}
