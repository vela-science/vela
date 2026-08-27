//! Read-only synthesis of source-owned research activity.
//!
//! This module deliberately lives in the CLI. Its input is not a protocol
//! object and its output has no authority or Standing effect.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const INPUT_SCHEMA: &str = "vela.cli.research-program-input.v1";
const OUTPUT_SCHEMA: &str = "vela.cli.research-program-view.v1";
const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
const SOURCE_KINDS: &[&str] = &[
    "episode_denominator",
    "h112_analysis",
    "experiment_ledger",
    "hypothesis_register",
    "research_episode_index",
    "longitudinal_synthesis",
    "retrospective_report",
];
const TASK_STATES: &[&str] = &[
    "active",
    "completed",
    "stopped",
    "failed",
    "awaiting_scientific_decision",
];
const CHANGE_TARGETS: &[&str] = &["experiment_ledger", "hypothesis_register"];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema: String,
    program: Program,
    as_of: String,
    sources: Vec<Source>,
    tasks: Vec<Task>,
    proposed_changes: Vec<ProposedChange>,
    next_actions: Vec<NextAction>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Program {
    id: String,
    name: String,
    thesis: String,
    thesis_evidence_refs: Vec<String>,
    does_not_establish: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    kind: String,
    path: PathBuf,
    sha256: String,
    expected_records: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Task {
    id: String,
    title: String,
    state: String,
    state_reason: String,
    source_pointer: String,
    observed_at: String,
    stale_after_seconds: Option<i64>,
    callback_due_at: Option<String>,
    callback_received: bool,
    requires_scientific_decision: bool,
    evidence_refs: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProposedChange {
    id: String,
    target: String,
    subject_id: String,
    current_state: String,
    proposed_state: String,
    reason: String,
    evidence_refs: Vec<String>,
    decision_ref: Option<DecisionRef>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionRef {
    repository: String,
    proposal_id: String,
    claim_id: String,
    claim_root: String,
    decision_event_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NextAction {
    id: String,
    priority: u32,
    action: String,
    why: String,
    evidence_refs: Vec<String>,
    requires_proposal_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Denominator {
    metadata: Value,
    campaign_episodes: Vec<Value>,
    material_attempts: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct SourceCheck {
    id: String,
    kind: String,
    path: String,
    expected_sha256: String,
    actual_sha256: Option<String>,
    status: &'static str,
    expected_records: Option<usize>,
    observed_records: Option<usize>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct EvidenceCheck {
    episode_id: String,
    path: Option<String>,
    expected_sha256: Option<String>,
    actual_sha256: Option<String>,
    status: &'static str,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct TaskView {
    id: String,
    title: String,
    declared_state: String,
    effective_state: String,
    state_reason: String,
    source_pointer: String,
    observed_at: String,
    callback_due_at: Option<String>,
    callback_received: bool,
    requires_scientific_decision: bool,
    evidence_refs: Vec<String>,
}

pub(crate) fn cmd_program(input_path: &Path, as_of_override: Option<&str>, json_out: bool) {
    let view =
        inspect(input_path, as_of_override).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&view);
        return;
    }

    println!(
        "{}",
        view["program"]["name"]
            .as_str()
            .unwrap_or("Research program")
    );
    println!("as of {}", view["as_of"].as_str().unwrap_or("unknown"));
    println!();
    println!("thesis");
    println!("  {}", view["program"]["thesis"].as_str().unwrap_or(""));
    println!();
    let counts = &view["denominator"]["status_counts"];
    println!(
        "denominator  {} campaigns · {} attempts · {} evidence gaps · {} source gaps",
        view["denominator"]["campaign_count"].as_u64().unwrap_or(0),
        view["denominator"]["material_attempt_count"]
            .as_u64()
            .unwrap_or(0),
        view["evidence"]["gap_count"].as_u64().unwrap_or(0),
        view["attention"]["source_gap_count"].as_u64().unwrap_or(0)
    );
    println!(
        "  positive {} · null {} · invalid {} · stopped {} · killed {} · no-go {}",
        counts["positive"].as_u64().unwrap_or(0),
        counts["null"].as_u64().unwrap_or(0),
        counts["invalid"].as_u64().unwrap_or(0),
        counts["stopped"].as_u64().unwrap_or(0),
        counts["killed"].as_u64().unwrap_or(0),
        counts["no_go"].as_u64().unwrap_or(0)
    );
    println!();
    println!("attention");
    for source in view["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|source| {
            source["status"]
                .as_str()
                .is_some_and(|status| status != "verified")
        })
    {
        println!(
            "  source_{}  {} — {}",
            source["status"].as_str().unwrap_or("unknown"),
            source["id"].as_str().unwrap_or("unknown"),
            source["message"].as_str().unwrap_or("source is not exact")
        );
    }
    for task in view["attention"]["tasks"].as_array().into_iter().flatten() {
        println!(
            "  {}  {} — {}",
            task["effective_state"].as_str().unwrap_or("unknown"),
            task["id"].as_str().unwrap_or("unknown"),
            task["state_reason"].as_str().unwrap_or("")
        );
    }
    for change in view["proposed_changes"].as_array().into_iter().flatten() {
        println!(
            "  propose  {}: {} -> {}",
            change["subject_id"].as_str().unwrap_or("unknown"),
            change["current_state"].as_str().unwrap_or("unknown"),
            change["proposed_state"].as_str().unwrap_or("unknown")
        );
    }
    println!();
    println!("next");
    println!(
        "  {}",
        view["next_action"]["action"]
            .as_str()
            .unwrap_or("No evidence-justified action available")
    );
    println!(
        "  why: {}",
        view["next_action"]["why"].as_str().unwrap_or("")
    );
    println!("authority effect: none; Standing unchanged");
}

fn inspect(input_path: &Path, as_of_override: Option<&str>) -> Result<Value, String> {
    let input_bytes = read_limited(input_path)?;
    let input: Input = serde_json::from_slice(&input_bytes).map_err(|error| {
        format!(
            "parse research-program input {}: {error}",
            input_path.display()
        )
    })?;
    validate_input(&input)?;
    let as_of_text = as_of_override.unwrap_or(&input.as_of);
    let as_of = parse_time(as_of_text, "program as_of")?;
    let input_root = vela_protocol::canonical::to_canonical_bytes(&input)
        .map_err(|error| format!("canonicalize research-program input: {error}"))?;
    let input_root = format!("sha256:{}", hex::encode(Sha256::digest(input_root)));

    let mut source_checks = Vec::new();
    let mut source_bytes = BTreeMap::new();
    for source in &input.sources {
        let check = match read_limited(&source.path) {
            Ok(bytes) => {
                let actual = hex::encode(Sha256::digest(&bytes));
                let status = if actual == source.sha256 {
                    "verified"
                } else {
                    "mismatch"
                };
                source_bytes.insert(source.id.clone(), bytes);
                SourceCheck {
                    id: source.id.clone(),
                    kind: source.kind.clone(),
                    path: source.path.display().to_string(),
                    expected_sha256: source.sha256.clone(),
                    actual_sha256: Some(actual),
                    status,
                    expected_records: source.expected_records,
                    observed_records: None,
                    message: (status == "mismatch").then_some(
                        "source bytes changed; rows remain visible but the source is not exact"
                            .into(),
                    ),
                }
            }
            Err(error) => SourceCheck {
                id: source.id.clone(),
                kind: source.kind.clone(),
                path: source.path.display().to_string(),
                expected_sha256: source.sha256.clone(),
                actual_sha256: None,
                status: "missing",
                expected_records: source.expected_records,
                observed_records: None,
                message: Some(error),
            },
        };
        source_checks.push(check);
    }

    let denominator_source = input
        .sources
        .iter()
        .find(|source| source.kind == "episode_denominator")
        .ok_or("research-program input has no episode_denominator source")?;
    let denominator = match source_bytes.get(&denominator_source.id) {
        Some(bytes) => serde_json::from_slice::<Denominator>(bytes).map_err(|error| {
            format!(
                "parse episode denominator {}: {error}",
                denominator_source.path.display()
            )
        })?,
        None => Denominator {
            metadata: json!({}),
            campaign_episodes: Vec::new(),
            material_attempts: Vec::new(),
        },
    };
    if let Some(check) = source_checks
        .iter_mut()
        .find(|check| check.id == denominator_source.id)
    {
        check.observed_records = Some(denominator.campaign_episodes.len());
        if let Some(expected) = check.expected_records
            && expected != denominator.campaign_episodes.len()
        {
            check.status = "record_count_mismatch";
            check.message = Some(format!(
                "expected {expected} campaign rows, observed {}; no row was silently dropped",
                denominator.campaign_episodes.len()
            ));
        }
    }

    let mut episode_rows = Vec::new();
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut category_counts = BTreeMap::<String, usize>::new();
    let mut ids = BTreeMap::<String, usize>::new();
    let mut evidence_checks = Vec::new();
    for (index, episode) in denominator.campaign_episodes.iter().enumerate() {
        let object = episode
            .as_object()
            .ok_or_else(|| format!("episode row {} is not an object", index + 1))?;
        let id = required_text(object.get("episode_id"), "episode_id", index)?;
        let status = required_text(object.get("result_status"), "result_status", index)?;
        let question = required_text(object.get("question"), "question", index)?;
        let summary = required_text(object.get("result_summary"), "result_summary", index)?;
        let disposition = required_text(
            object.get("ledger_disposition"),
            "ledger_disposition",
            index,
        )?;
        *status_counts.entry(status.to_string()).or_default() += 1;
        *ids.entry(id.to_string()).or_default() += 1;
        let scientific = object
            .get("scientific_yield_eligible")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let flags = object
            .get("result_flags")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let category = category(status, scientific, &flags);
        *category_counts.entry(category.into()).or_default() += 1;
        let evidence = check_episode_evidence(id, object.get("evidence_identity"));
        let evidence_status = evidence.status;
        evidence_checks.push(evidence);
        episode_rows.push(json!({
            "episode_id": id,
            "date": object.get("date").cloned().unwrap_or(Value::Null),
            "question": question,
            "result_summary": summary,
            "result_status": status,
            "category": category,
            "ledger_disposition": disposition,
            "scientific_yield_eligible": scientific,
            "conversion_ready_outcome": object.get("conversion_ready_outcome").cloned().unwrap_or(Value::Null),
            "evidence_pointer": object.get("evidence_pointer").cloned().unwrap_or(Value::Null),
            "evidence_status": evidence_status,
            "missing_required_fields": object.get("missing_required_fields").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    let duplicate_ids = ids
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, count)| json!({"episode_id": id, "occurrences": count}))
        .collect::<Vec<_>>();

    let tasks = input
        .tasks
        .iter()
        .map(|task| task_view(task, as_of))
        .collect::<Result<Vec<_>, _>>()?;
    let attention_tasks = tasks
        .iter()
        .filter(|task| {
            matches!(
                task.effective_state.as_str(),
                "stalled" | "missing_callback" | "awaiting_scientific_decision" | "stopped"
            )
        })
        .collect::<Vec<_>>();

    let (proposals, rejected_proposals) = validate_proposals(&input.proposed_changes);
    let proposal_ids = proposals
        .iter()
        .filter_map(|proposal| proposal["id"].as_str())
        .collect::<BTreeSet<_>>();
    let mut actions = input.next_actions.iter().collect::<Vec<_>>();
    actions.sort_by_key(|action| (action.priority, action.id.as_str()));
    let next_action = actions
        .into_iter()
        .find(|action| {
            action
                .requires_proposal_ids
                .iter()
                .all(|id| proposal_ids.contains(id.as_str()))
        })
        .map(|action| {
            json!({
                "id": action.id,
                "priority": action.priority,
                "action": action.action,
                "why": action.why,
                "evidence_refs": action.evidence_refs,
            })
        })
        .unwrap_or(Value::Null);

    let manual = compare_manual_sources(
        &input.sources,
        &source_bytes,
        &denominator,
        &input.proposed_changes,
    );
    let evidence_gap_count = evidence_checks
        .iter()
        .filter(|check| check.status != "verified")
        .count();
    let source_gap_count = source_checks
        .iter()
        .filter(|check| check.status != "verified")
        .count();

    Ok(json!({
        "schema": OUTPUT_SCHEMA,
        "ok": true,
        "command": "integration program",
        "authority_effect": "none",
        "standing_effect": "none",
        "accepted_standing_changes": 0,
        "as_of": as_of.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "input_path": input_path.display().to_string(),
        "input_root": input_root,
        "program": {
            "id": input.program.id,
            "name": input.program.name,
            "thesis": input.program.thesis,
            "thesis_evidence_refs": input.program.thesis_evidence_refs,
            "does_not_establish": input.program.does_not_establish,
        },
        "sources": source_checks,
        "denominator": {
            "campaign_count": denominator.campaign_episodes.len(),
            "unique_campaign_count": ids.len(),
            "material_attempt_count": denominator.material_attempts.len(),
            "status_counts": status_counts,
            "category_counts": category_counts,
            "duplicate_episode_ids": duplicate_ids,
            "metadata": denominator.metadata,
            "episodes": episode_rows,
            "material_attempts": denominator.material_attempts,
            "human_acceptance": {
                "decision_bound_count": 0,
                "inferred_from_episode_status": false,
                "source_of_truth": "verified Vela Decision and replayed Standing only",
            },
        },
        "hypotheses": manual["hypotheses"].clone(),
        "tasks": tasks,
        "attention": {
            "task_count": attention_tasks.len(),
            "tasks": attention_tasks,
            "source_gap_count": source_gap_count,
            "evidence_gap_count": evidence_gap_count,
        },
        "evidence": {
            "gap_count": evidence_gap_count,
            "checks": evidence_checks,
        },
        "proposed_changes": proposals,
        "rejected_proposals": rejected_proposals,
        "next_action": next_action,
        "manual_comparison": manual,
        "cold_successor": {
            "answers": {
                "what_was_tried": "denominator.episodes",
                "what_succeeded_failed_or_remains_open": "denominator.status_counts + denominator.episodes[].result_status",
                "exact_evidence": "denominator.episodes[].evidence_pointer + evidence.checks",
                "tasks_needing_choice_or_callback": "attention.tasks",
                "next_action_and_why": "next_action",
            },
            "single_command": "vela integration program <INPUT> --json",
        },
    }))
}

fn validate_input(input: &Input) -> Result<(), String> {
    if input.schema != INPUT_SCHEMA {
        return Err(format!(
            "research-program input schema must be {INPUT_SCHEMA}"
        ));
    }
    parse_time(&input.as_of, "input as_of")?;
    let mut ids = BTreeSet::new();
    for source in &input.sources {
        if !ids.insert(source.id.as_str()) {
            return Err(format!("duplicate source id {}", source.id));
        }
        if !SOURCE_KINDS.contains(&source.kind.as_str()) {
            return Err(format!("unsupported source kind {}", source.kind));
        }
        if source.sha256.len() != 64 || !source.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "source {} sha256 is not 64 hex characters",
                source.id
            ));
        }
    }
    if input
        .sources
        .iter()
        .filter(|source| source.kind == "episode_denominator")
        .count()
        != 1
    {
        return Err("research-program input requires exactly one episode_denominator".into());
    }
    let source_ids = input
        .sources
        .iter()
        .map(|source| source.id.as_str())
        .collect::<BTreeSet<_>>();
    for reference in input
        .program
        .thesis_evidence_refs
        .iter()
        .chain(
            input
                .proposed_changes
                .iter()
                .flat_map(|change| change.evidence_refs.iter()),
        )
        .chain(
            input
                .tasks
                .iter()
                .flat_map(|task| task.evidence_refs.iter()),
        )
        .chain(
            input
                .next_actions
                .iter()
                .flat_map(|action| action.evidence_refs.iter()),
        )
    {
        if !source_ids.contains(reference.as_str()) {
            return Err(format!("unknown source reference {reference}"));
        }
    }
    for task in &input.tasks {
        if !TASK_STATES.contains(&task.state.as_str()) {
            return Err(format!(
                "task {} has unsupported state {}",
                task.id, task.state
            ));
        }
        parse_time(&task.observed_at, "task observed_at")?;
        if let Some(due) = &task.callback_due_at {
            parse_time(due, "task callback_due_at")?;
        }
    }
    let mut proposal_ids = BTreeSet::new();
    for change in &input.proposed_changes {
        if !proposal_ids.insert(change.id.as_str()) {
            return Err(format!("duplicate proposal id {}", change.id));
        }
        if !CHANGE_TARGETS.contains(&change.target.as_str()) {
            return Err(format!(
                "proposal {} has unsupported target {}",
                change.id, change.target
            ));
        }
    }
    for action in &input.next_actions {
        for required in &action.requires_proposal_ids {
            if !proposal_ids.contains(required.as_str()) {
                return Err(format!(
                    "next action {} requires unknown proposal {required}",
                    action.id
                ));
            }
        }
    }
    Ok(())
}

fn task_view(task: &Task, as_of: DateTime<Utc>) -> Result<TaskView, String> {
    let observed_at = parse_time(&task.observed_at, "task observed_at")?;
    let callback_due = task
        .callback_due_at
        .as_deref()
        .map(|value| parse_time(value, "task callback_due_at"))
        .transpose()?;
    let effective = if task.state == "active"
        && callback_due.is_some_and(|due| due <= as_of)
        && !task.callback_received
    {
        "missing_callback"
    } else if task.state == "active"
        && task
            .stale_after_seconds
            .is_some_and(|seconds| (as_of - observed_at).num_seconds() > seconds)
    {
        "stalled"
    } else if task.requires_scientific_decision && task.state == "completed" {
        "awaiting_scientific_decision"
    } else {
        task.state.as_str()
    };
    Ok(TaskView {
        id: task.id.clone(),
        title: task.title.clone(),
        declared_state: task.state.clone(),
        effective_state: effective.into(),
        state_reason: task.state_reason.clone(),
        source_pointer: task.source_pointer.clone(),
        observed_at: task.observed_at.clone(),
        callback_due_at: task.callback_due_at.clone(),
        callback_received: task.callback_received,
        requires_scientific_decision: task.requires_scientific_decision,
        evidence_refs: task.evidence_refs.clone(),
    })
}

fn validate_proposals(changes: &[ProposedChange]) -> (Vec<Value>, Vec<Value>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for change in changes {
        let requests_acceptance = change
            .proposed_state
            .to_ascii_lowercase()
            .contains("accepted");
        let value = json!({
            "id": change.id,
            "target": change.target,
            "subject_id": change.subject_id,
            "current_state": change.current_state,
            "proposed_state": change.proposed_state,
            "reason": change.reason,
            "evidence_refs": change.evidence_refs,
            "decision_ref": change.decision_ref,
            "authority_effect": "none",
            "standing_effect": "none",
            "applied": false,
        });
        if requests_acceptance && change.decision_ref.is_none() {
            rejected.push(json!({
                "proposal": value,
                "reason": "an accepted state requires an exact existing Vela Decision reference; prose, a task result, Verification, or a signature is not acceptance",
            }));
        } else {
            accepted.push(value);
        }
    }
    (accepted, rejected)
}

fn check_episode_evidence(episode_id: &str, raw: Option<&Value>) -> EvidenceCheck {
    let Some(identity) = raw.and_then(Value::as_object) else {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path: None,
            expected_sha256: None,
            actual_sha256: None,
            status: "missing_identity",
            message: Some("episode has no structured evidence identity".into()),
        };
    };
    let access = identity
        .get("access_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let path = identity
        .get("primary_path")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let expected = identity
        .get("reconstruction_sha256")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if access != "available" {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path,
            expected_sha256: expected,
            actual_sha256: None,
            status: "not_resolved",
            message: Some("retained denominator marks the primary evidence unresolved".into()),
        };
    }
    let Some(path_text) = path.as_deref() else {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path,
            expected_sha256: expected,
            actual_sha256: None,
            status: "missing_path",
            message: Some("available evidence has no primary path".into()),
        };
    };
    let kind = identity
        .get("primary_path_kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if kind == "directory" {
        return check_directory_evidence(episode_id, Path::new(path_text), identity, expected);
    }
    if kind != "file" {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path,
            expected_sha256: expected,
            actual_sha256: None,
            status: "unsupported_pointer",
            message: Some(format!("unsupported primary_path_kind {kind}")),
        };
    }
    match read_limited(Path::new(path_text)) {
        Ok(bytes) => {
            let actual = hex::encode(Sha256::digest(bytes));
            let status = if expected.as_deref() == Some(actual.as_str()) {
                "verified"
            } else {
                "mismatch"
            };
            EvidenceCheck {
                episode_id: episode_id.into(),
                path,
                expected_sha256: expected,
                actual_sha256: Some(actual),
                status,
                message: (status == "mismatch").then_some(
                    "current file bytes differ from the retained reconstruction digest".into(),
                ),
            }
        }
        Err(error) => EvidenceCheck {
            episode_id: episode_id.into(),
            path,
            expected_sha256: expected,
            actual_sha256: None,
            status: "missing",
            message: Some(error.to_string()),
        },
    }
}

fn check_directory_evidence(
    episode_id: &str,
    path: &Path,
    identity: &serde_json::Map<String, Value>,
    expected: Option<String>,
) -> EvidenceCheck {
    if !path.is_dir() {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path: Some(path.display().to_string()),
            expected_sha256: expected,
            actual_sha256: None,
            status: "missing",
            message: Some("evidence directory does not exist".into()),
        };
    }
    let repo = identity
        .get("repository_identity")
        .and_then(Value::as_object);
    let expected_head = repo
        .and_then(|value| value.get("reconstruction_head"))
        .and_then(Value::as_str);
    let expected_tree = repo
        .and_then(|value| value.get("reconstruction_tree"))
        .and_then(Value::as_str);
    if expected_head.is_none() && expected_tree.is_none() {
        return EvidenceCheck {
            episode_id: episode_id.into(),
            path: Some(path.display().to_string()),
            expected_sha256: expected,
            actual_sha256: None,
            status: "available_unfixed_directory",
            message: Some(
                "directory exists but carries no exact repository head/tree identity".into(),
            ),
        };
    }
    let head = git_value(path, &["rev-parse", "HEAD"]);
    let tree = git_value(path, &["rev-parse", "HEAD^{tree}"]);
    let matched = head.as_deref() == expected_head && tree.as_deref() == expected_tree;
    EvidenceCheck {
        episode_id: episode_id.into(),
        path: Some(path.display().to_string()),
        expected_sha256: expected,
        actual_sha256: None,
        status: if matched {
            "verified"
        } else {
            "repository_drift"
        },
        message: (!matched).then_some(format!(
            "expected head/tree {:?}/{:?}, observed {:?}/{:?}",
            expected_head, expected_tree, head, tree
        )),
    }
}

fn git_value(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn compare_manual_sources(
    sources: &[Source],
    bytes: &BTreeMap<String, Vec<u8>>,
    denominator: &Denominator,
    proposals: &[ProposedChange],
) -> Value {
    let mut result = json!({
        "experiment_ledger": {"status": "unavailable"},
        "hypotheses": {"status": "unavailable", "current": [], "proposed_conflicts": []},
        "research_episode_index": {"status": "unavailable"},
    });
    if let Some(source) = sources
        .iter()
        .find(|source| source.kind == "experiment_ledger")
        && let Some(content) = bytes.get(&source.id)
    {
        let text = String::from_utf8_lossy(content);
        let rows = markdown_rows(&text, "## Summary ledger", "## Decision index");
        let manual = rows
            .into_iter()
            .filter(|row| row.first().is_some_and(|id| !id.is_empty() && id != "ID"))
            .map(|row| (row[0].clone(), row))
            .collect::<BTreeMap<_, _>>();
        let mut mismatches = Vec::new();
        let mut missing = Vec::new();
        let fields = [
            ("date", 1usize),
            ("question", 2),
            ("design_and_search_configuration", 3),
            ("result_summary", 4),
            ("ledger_disposition", 5),
            ("evidence_pointer", 6),
        ];
        for episode in &denominator.campaign_episodes {
            let Some(id) = episode.get("episode_id").and_then(Value::as_str) else {
                continue;
            };
            let Some(row) = manual.get(id) else {
                missing.push(id.to_string());
                continue;
            };
            for (field, column) in fields {
                let manual_value = row.get(column).map(String::as_str).unwrap_or("");
                let structured = episode.get(field).and_then(Value::as_str).unwrap_or("");
                if manual_value != structured {
                    mismatches.push(json!({
                        "episode_id": id,
                        "field": field,
                        "manual": manual_value,
                        "structured": structured,
                    }));
                }
            }
        }
        result["experiment_ledger"] = json!({
            "status": if missing.is_empty() && mismatches.is_empty() { "matched" } else { "discrepant" },
            "manual_row_count": manual.len(),
            "structured_row_count": denominator.campaign_episodes.len(),
            "missing_from_manual": missing,
            "field_mismatches": mismatches,
        });
    }
    if let Some(source) = sources
        .iter()
        .find(|source| source.kind == "hypothesis_register")
        && let Some(content) = bytes.get(&source.id)
    {
        let text = String::from_utf8_lossy(content);
        let current = text
            .lines()
            .filter(|line| line.starts_with("| H-"))
            .filter_map(split_markdown_row)
            .filter(|row| row.len() >= 3)
            .map(|row| json!({"id": row[0], "hypothesis": row[1], "state": row[2]}))
            .collect::<Vec<_>>();
        let states = current
            .iter()
            .filter_map(|value| Some((value["id"].as_str()?, value["state"].as_str()?)))
            .collect::<BTreeMap<_, _>>();
        let conflicts = proposals
            .iter()
            .filter(|proposal| proposal.target == "hypothesis_register")
            .filter_map(|proposal| {
                let manual = states.get(proposal.subject_id.as_str())?;
                (*manual != proposal.proposed_state).then(|| {
                    json!({
                        "subject_id": proposal.subject_id,
                        "manual_state": manual,
                        "proposed_state": proposal.proposed_state,
                        "proposal_id": proposal.id,
                        "evidence_refs": proposal.evidence_refs,
                    })
                })
            })
            .collect::<Vec<_>>();
        result["hypotheses"] = json!({
            "status": if conflicts.is_empty() { "matched" } else { "discrepant" },
            "current": current,
            "proposed_conflicts": conflicts,
        });
    }
    if let Some(source) = sources
        .iter()
        .find(|source| source.kind == "research_episode_index")
        && let Some(content) = bytes.get(&source.id)
    {
        let text = String::from_utf8_lossy(content);
        let rows = markdown_rows(&text, "## Episode table", "## Repeated dead ends");
        let count = rows
            .iter()
            .filter(|row| row.first().is_some_and(|value| value != "Date"))
            .count();
        result["research_episode_index"] = json!({
            "status": if count == denominator.campaign_episodes.len() { "matched" } else { "different_granularity" },
            "manual_row_count": count,
            "structured_campaign_count": denominator.campaign_episodes.len(),
            "discrepancy": "The manual episode index is a curated strategic view; the structured campaign ledger is the full denominator. Neither count is silently substituted for the other.",
        });
    }
    result
}

fn markdown_rows(text: &str, start: &str, end: &str) -> Vec<Vec<String>> {
    let Some(after) = text.split_once(start).map(|(_, after)| after) else {
        return Vec::new();
    };
    let body = after.split_once(end).map(|(body, _)| body).unwrap_or(after);
    body.lines()
        .filter_map(split_markdown_row)
        .filter(|row| {
            !row.iter()
                .all(|cell| cell.chars().all(|c| c == '-' || c == ':' || c == ' '))
        })
        .collect()
}

fn split_markdown_row(line: &str) -> Option<Vec<String>> {
    let body = line.strip_prefix('|')?.strip_suffix('|')?;
    Some(
        body.split('|')
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

fn category(status: &str, scientific: bool, flags: &[Value]) -> &'static str {
    match status {
        "invalid" => "invalid_experiment",
        "null" => "null_result",
        "stopped" => "stopped_work",
        "positive" if scientific => "scientific_result",
        "positive" => "software_qualification",
        "killed" => "killed_direction",
        "no_go" => "apparatus_no_go",
        "not_run" => "not_run",
        "inconclusive" => "inconclusive",
        "known_theory" => "known_theory",
        "open" => "open_work",
        "mixed" => {
            if flags
                .iter()
                .any(|flag| flag.as_str() == Some("infrastructure"))
            {
                "software_qualification"
            } else {
                "mixed_result"
            }
        }
        _ => "unclassified",
    }
}

fn required_text<'a>(
    value: Option<&'a Value>,
    field: &str,
    index: usize,
) -> Result<&'a str, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("episode row {} has no {field}", index + 1))
}

fn parse_time(value: &str, label: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| format!("{label} is not RFC3339: {error}"))
}

fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "{} exceeds the {} byte research-program source limit",
            path.display(),
            MAX_SOURCE_BYTES
        ));
    }
    fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_and_null_are_never_collapsed_by_summary_prose() {
        assert_eq!(category("invalid", true, &[]), "invalid_experiment");
        assert_eq!(category("null", true, &[]), "null_result");
    }

    #[test]
    fn unauthorized_acceptance_is_rejected_without_dropping_other_proposals() {
        let changes = vec![
            ProposedChange {
                id: "bad".into(),
                target: "hypothesis_register".into(),
                subject_id: "H-1".into(),
                current_state: "OPEN".into(),
                proposed_state: "ACCEPTED".into(),
                reason: "prose says so".into(),
                evidence_refs: vec![],
                decision_ref: None,
            },
            ProposedChange {
                id: "good".into(),
                target: "hypothesis_register".into(),
                subject_id: "H-2".into(),
                current_state: "OPEN".into(),
                proposed_state: "KILLED".into(),
                reason: "bounded result".into(),
                evidence_refs: vec![],
                decision_ref: None,
            },
        ];
        let (accepted, rejected) = validate_proposals(&changes);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0]["id"], "good");
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0]["proposal"]["id"], "bad");
    }

    #[test]
    fn stale_and_missing_callback_states_are_derived_from_exact_time() {
        let task = Task {
            id: "task".into(),
            title: "task".into(),
            state: "active".into(),
            state_reason: "running".into(),
            source_pointer: "thread:1".into(),
            observed_at: "2026-08-27T20:00:00Z".into(),
            stale_after_seconds: Some(60),
            callback_due_at: Some("2026-08-27T20:01:00Z".into()),
            callback_received: false,
            requires_scientific_decision: false,
            evidence_refs: vec![],
        };
        let view = task_view(&task, parse_time("2026-08-27T20:02:00Z", "test").unwrap()).unwrap();
        assert_eq!(view.effective_state, "missing_callback");
    }

    #[test]
    fn adversarial_projection_retains_duplicates_gaps_and_authority_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing-evidence.txt");
        let episode = |id: &str, status: &str, scientific: bool, summary: &str| {
            json!({
                "episode_id": id,
                "date": "2026-08-27",
                "question": "What happened?",
                "result_summary": summary,
                "ledger_disposition": format!("`{status}`"),
                "result_status": status,
                "result_flags": [],
                "scientific_yield_eligible": scientific,
                "conversion_ready_outcome": "no",
                "evidence_pointer": missing.display().to_string(),
                "missing_required_fields": [],
                "evidence_identity": {
                    "primary_path": missing.display().to_string(),
                    "primary_path_kind": "file",
                    "access_status": "available",
                    "reconstruction_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            })
        };
        let denominator = json!({
            "metadata": {"schema_version": "test"},
            "campaign_episodes": [
                episode("invalid", "invalid", true, "Prose misleadingly says accepted."),
                episode("null", "null", true, "No effect in a valid bounded assay."),
                episode("stopped", "stopped", false, "Stopped by strategy."),
                episode("science", "positive", true, "Bounded result."),
                episode("software", "positive", false, "Qualification pass."),
                episode("duplicate", "positive", true, "First occurrence."),
                episode("duplicate", "positive", true, "Second occurrence.")
            ],
            "material_attempts": []
        });
        let denominator_bytes = serde_json::to_vec(&denominator).unwrap();
        let denominator_path = temp.path().join("denominator.json");
        fs::write(&denominator_path, &denominator_bytes).unwrap();
        let input_path = temp.path().join("input.json");
        let input = json!({
            "schema": INPUT_SCHEMA,
            "program": {
                "id": "test",
                "name": "Test",
                "thesis": "No claim from prose alone.",
                "thesis_evidence_refs": ["denominator"],
                "does_not_establish": ["acceptance"]
            },
            "as_of": "2026-08-27T20:02:00Z",
            "sources": [{
                "id": "denominator",
                "kind": "episode_denominator",
                "path": denominator_path,
                "sha256": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                "expected_records": 7
            }],
            "tasks": [{
                "id": "stale-task",
                "title": "Stale task",
                "state": "active",
                "state_reason": "No callback",
                "source_pointer": "thread:test",
                "observed_at": "2026-08-27T20:00:00Z",
                "stale_after_seconds": 60,
                "callback_due_at": "2026-08-27T20:01:00Z",
                "callback_received": false,
                "requires_scientific_decision": false,
                "evidence_refs": ["denominator"]
            }],
            "proposed_changes": [{
                "id": "unauthorized",
                "target": "hypothesis_register",
                "subject_id": "H-X",
                "current_state": "OPEN",
                "proposed_state": "ACCEPTED",
                "reason": "misleading prose",
                "evidence_refs": ["denominator"],
                "decision_ref": null
            }],
            "next_actions": []
        });
        fs::write(&input_path, serde_json::to_vec(&input).unwrap()).unwrap();

        let view = inspect(&input_path, None).unwrap();
        assert_eq!(view["denominator"]["campaign_count"], 7);
        assert_eq!(view["sources"][0]["status"], "mismatch");
        assert_eq!(view["attention"]["source_gap_count"], 1);
        assert_eq!(view["denominator"]["unique_campaign_count"], 6);
        assert_eq!(
            view["denominator"]["duplicate_episode_ids"][0]["occurrences"],
            2
        );
        assert_eq!(view["denominator"]["status_counts"]["invalid"], 1);
        assert_eq!(view["denominator"]["status_counts"]["null"], 1);
        assert_eq!(view["denominator"]["status_counts"]["stopped"], 1);
        assert_eq!(
            view["denominator"]["category_counts"]["scientific_result"],
            3
        );
        assert_eq!(
            view["denominator"]["category_counts"]["software_qualification"],
            1
        );
        assert_eq!(view["evidence"]["gap_count"], 7);
        assert_eq!(view["tasks"][0]["effective_state"], "missing_callback");
        assert_eq!(
            view["rejected_proposals"][0]["proposal"]["id"],
            "unauthorized"
        );
        assert_eq!(view["accepted_standing_changes"], 0);
    }
}
