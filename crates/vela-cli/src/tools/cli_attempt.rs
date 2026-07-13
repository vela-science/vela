//! Historical Attempt-ledger import.
//!
//! The importer is deliberately a reconciliation pass first and a writer
//! second. It validates the complete source ledger and mapping before it
//! appends any event, defaults to a dry-run, and uses content-addressed
//! `vat_` ids to make a repeated apply an event/root no-op.

use crate::cli::{fail_return, print_json};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use vela_protocol::attempt::{Attempt, AttemptDraft};

const IMPORT_MAPPING_SCHEMA: &str = "vela.attempt-import-map.v1";
const IMPORT_EVENT_SCHEMA: &str = "vela.attempt-import.v1";
const AGENT_KEY_ENV: &str = "VELA_AGENT_KEY_HEX";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptImportMap {
    schema: String,
    #[serde(default)]
    exhaustive: bool,
    mappings: Vec<AttemptImportMapping>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptImportMapping {
    attempt_id: String,
    action: String,
    #[serde(default)]
    problem: Option<u32>,
    #[serde(default)]
    frontier: Option<String>,
    #[serde(default)]
    reason: String,
    /// Accounting destination for excluded records, such as
    /// `sidon-frontier` or `vela-platform-activity`.
    #[serde(default)]
    target: Option<String>,
    /// Optional guard for mappings that intentionally change a content id.
    #[serde(default)]
    expected_attempt_id: Option<String>,
    /// Narrow compatibility rule for a source id minted before problem=0 was
    /// skip-serialized. The target is always rebuilt under the current rule.
    #[serde(default)]
    source_id_rule: Option<String>,
}

#[derive(Debug)]
pub(crate) struct AttemptImportRequest<'a> {
    pub ledger: &'a Path,
    pub frontier: &'a Path,
    pub actor: &'a str,
    pub mapping: &'a Path,
    pub source_ref: &'a str,
    pub apply: bool,
    /// Production imports require one explicit disposition per source row.
    /// Unit fixtures may opt out to exercise the default unchanged import.
    pub require_exhaustive: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AttemptImportReport {
    ok: bool,
    command: &'static str,
    mode: &'static str,
    actor: String,
    source: AttemptImportSourceReport,
    frontier: AttemptImportFrontierReport,
    summary: AttemptImportSummary,
    reconciliation: Vec<AttemptImportRow>,
}

#[derive(Debug, Serialize)]
struct AttemptImportSourceReport {
    ledger: String,
    source_ref: String,
    sha256: String,
    mapping: String,
    mapping_schema: &'static str,
    exhaustive: bool,
}

#[derive(Debug, Serialize)]
struct AttemptImportFrontierReport {
    path: String,
    frontier_id: String,
    event_log_hash_before: String,
    event_log_hash_after: String,
    snapshot_hash_before: String,
    snapshot_hash_after: String,
}

#[derive(Debug, Default, Serialize)]
struct AttemptImportSummary {
    records_total: usize,
    import_records: usize,
    excluded: usize,
    ids_preserved: usize,
    ids_changed: usize,
    deposited: usize,
    already_imported: usize,
    already_present: usize,
    events_appended: usize,
}

#[derive(Debug, Serialize)]
struct AttemptImportRow {
    source_index: usize,
    source_attempt_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_legacy_id: Option<String>,
    disposition: String,
    status: String,
    mapping_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_attempt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id_preserved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

struct PlannedDeposit {
    row_index: usize,
    source_attempt_id: String,
    source_legacy_id: Option<String>,
    mapping_reason: Option<String>,
    attempt: Attempt,
}

/// CLI adapter. `--apply` deliberately accepts only the explicit agent-key
/// environment variable: historical bulk import must never mint a key as a
/// side effect.
pub(crate) fn cmd_attempt_import(
    ledger: &Path,
    frontier: &Path,
    actor: &str,
    mapping: &Path,
    source_ref: &str,
    apply: bool,
    json: bool,
) {
    validate_agent_actor(actor).unwrap_or_else(|e| fail_return(&e));
    let key = if apply {
        Some(load_existing_agent_key().unwrap_or_else(|e| fail_return(&e)))
    } else {
        None
    };
    let request = AttemptImportRequest {
        ledger,
        frontier,
        actor,
        mapping,
        source_ref,
        apply,
        require_exhaustive: true,
    };
    let report = import_attempts(&request, key.as_ref()).unwrap_or_else(|e| fail_return(&e));
    if json {
        print_json(&serde_json::to_value(&report).expect("serialize attempt import report"));
        return;
    }

    println!(
        "attempt.import {} — {} source record(s), {} deposited, {} excluded, {} already present",
        report.mode,
        report.summary.records_total,
        report.summary.deposited,
        report.summary.excluded,
        report.summary.already_imported + report.summary.already_present,
    );
    println!("  source    {}", report.source.source_ref);
    println!("  frontier  {}", report.frontier.frontier_id);
    println!("  event log {}", report.frontier.event_log_hash_after);
    println!("  snapshot  {}", report.frontier.snapshot_hash_after);
    if !apply && report.summary.import_records > 0 {
        println!("  dry-run only; pass --apply with {AGENT_KEY_ENV} set to append events");
    }
}

pub(crate) fn import_attempts(
    request: &AttemptImportRequest<'_>,
    signing_key: Option<&SigningKey>,
) -> Result<AttemptImportReport, String> {
    validate_agent_actor(request.actor)?;
    validate_source_ref(request.source_ref)?;
    if request.apply && signing_key.is_none() {
        return Err(format!(
            "attempt import --apply requires an existing agent key in {AGENT_KEY_ENV}"
        ));
    }

    let ledger_bytes = std::fs::read(request.ledger)
        .map_err(|e| format!("read {}: {e}", request.ledger.display()))?;
    let ledger_value: serde_json::Value = serde_json::from_slice(&ledger_bytes)
        .map_err(|e| format!("parse {}: {e}", request.ledger.display()))?;
    let record_values = ledger_records(&ledger_value)?;
    let source_digest = format!("sha256:{}", hex::encode(Sha256::digest(&ledger_bytes)));

    let mapping = load_mapping(request.mapping)?;
    if request.require_exhaustive && !mapping.exhaustive {
        return Err(
            "production attempt import requires mapping.exhaustive=true and one explicit disposition per ledger record"
                .to_string(),
        );
    }
    let mapping_exhaustive = mapping.exhaustive;
    let mut mapping_by_id: BTreeMap<String, AttemptImportMapping> = BTreeMap::new();
    for entry in mapping.mappings {
        validate_mapping_entry(&entry)?;
        let id = entry.attempt_id.clone();
        if mapping_by_id.insert(id.clone(), entry).is_some() {
            return Err(format!("duplicate mapping for source attempt {id}"));
        }
    }

    let mut source_ids = BTreeSet::new();
    let mut sources = Vec::with_capacity(record_values.len());
    for (index, value) in record_values.iter().enumerate() {
        let attempt: Attempt = serde_json::from_value(value.clone())
            .map_err(|e| format!("record {index}: parse Attempt: {e}"))?;
        if !source_ids.insert(attempt.attempt_id.clone()) {
            return Err(format!(
                "record {index}: duplicate source attempt id {}",
                attempt.attempt_id
            ));
        }
        let mapping_entry = mapping_by_id.get(&attempt.attempt_id);
        if mapping_exhaustive && mapping_entry.is_none() {
            return Err(format!(
                "exhaustive mapping is missing ledger record {index} ({})",
                attempt.attempt_id
            ));
        }
        let validation = if mapping_entry.is_some_and(|entry| entry.action == "exclude") {
            validate_source_envelope(&attempt)
        } else {
            validate_source_attempt(
                &attempt,
                mapping_entry.and_then(|entry| entry.source_id_rule.as_deref()),
            )
        };
        validation.map_err(|e| format!("record {index} ({}): {e}", attempt.attempt_id))?;
        let legacy_id = value
            .get("legacy_id")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);
        sources.push((attempt, legacy_id));
    }
    if sources.is_empty() {
        return Err("attempt import ledger contains no records".to_string());
    }
    for id in mapping_by_id.keys() {
        if !source_ids.contains(id) {
            return Err(format!(
                "mapping references source attempt {id}, but the ledger does not contain it"
            ));
        }
    }
    if mapping_exhaustive {
        let unmapped: Vec<&String> = source_ids
            .iter()
            .filter(|id| !mapping_by_id.contains_key(*id))
            .collect();
        if !unmapped.is_empty() {
            return Err(format!(
                "exhaustive mapping is missing {} ledger record(s): {}",
                unmapped.len(),
                unmapped
                    .into_iter()
                    .take(12)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    let source = vela_protocol::repo::detect(request.frontier)?;
    let mut project = vela_protocol::repo::load(&source)?;
    let frontier_id = project.frontier_id();
    let event_log_hash_before = event_log_hash(&project.events);
    let snapshot_hash_before = snapshot_hash(&project);
    let preview_key = SigningKey::from_bytes(&[0x5au8; 32]);
    let build_key = signing_key.unwrap_or(&preview_key);

    let mut rows = Vec::with_capacity(sources.len());
    let mut planned = Vec::new();
    let mut target_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut summary = AttemptImportSummary {
        records_total: sources.len(),
        ..AttemptImportSummary::default()
    };

    for (index, (source_attempt, legacy_id)) in sources.into_iter().enumerate() {
        let source_id = source_attempt.attempt_id.clone();
        let entry = mapping_by_id.get(&source_id);
        if entry.is_some_and(|m| m.action == "exclude") {
            let entry = entry.expect("checked above");
            summary.excluded += 1;
            rows.push(AttemptImportRow {
                source_index: index,
                source_attempt_id: source_id,
                source_legacy_id: legacy_id,
                disposition: "exclude".to_string(),
                status: "excluded".to_string(),
                mapping_applied: true,
                target_attempt_id: None,
                id_preserved: None,
                target: entry.target.clone(),
                reason: nonempty(&entry.reason),
            });
            continue;
        }

        summary.import_records += 1;
        let target = rebuild_for_import(&source_attempt, entry, build_key)?;
        if let Some(expected) = entry.and_then(|m| m.expected_attempt_id.as_deref())
            && target.attempt_id != expected
        {
            return Err(format!(
                "mapping for {source_id} expected target id {expected}, rebuilt {}",
                target.attempt_id
            ));
        }
        if let Some(other_source) = target_ids.insert(target.attempt_id.clone(), source_id.clone())
        {
            return Err(format!(
                "mapping collision: source attempts {other_source} and {source_id} both map to {}",
                target.attempt_id
            ));
        }

        let id_preserved = target.attempt_id == source_id;
        if id_preserved {
            summary.ids_preserved += 1;
        } else {
            summary.ids_changed += 1;
        }
        let previous_import = prior_import_target(&project, request.source_ref, &source_id)?;
        let status = if let Some(previous_target) = previous_import {
            if previous_target != target.attempt_id {
                return Err(format!(
                    "source {source_id} from {} was already imported as {previous_target}; current mapping produces {}",
                    request.source_ref, target.attempt_id
                ));
            }
            summary.already_imported += 1;
            "already_imported"
        } else if project
            .attempts
            .iter()
            .any(|existing| existing.attempt_id == target.attempt_id)
        {
            summary.already_present += 1;
            "already_present"
        } else {
            "would_import"
        };

        let row_index = rows.len();
        rows.push(AttemptImportRow {
            source_index: index,
            source_attempt_id: source_id.clone(),
            source_legacy_id: legacy_id.clone(),
            disposition: "import".to_string(),
            status: status.to_string(),
            mapping_applied: entry.is_some(),
            target_attempt_id: Some(target.attempt_id.clone()),
            id_preserved: Some(id_preserved),
            target: None,
            reason: entry.and_then(|m| nonempty(&m.reason)),
        });
        if status == "would_import" {
            planned.push(PlannedDeposit {
                row_index,
                source_attempt_id: source_id,
                source_legacy_id: legacy_id,
                mapping_reason: entry.and_then(|m| nonempty(&m.reason)),
                attempt: target,
            });
        }
    }

    if request.apply {
        let key = signing_key.expect("checked above");
        for deposit in planned {
            let mut event = deposit.attempt.deposit_event(
                request.actor,
                vela_protocol::events::actor_kind(request.actor),
                &format!(
                    "historical attempt import from {} (provenance, not a verdict)",
                    request.source_ref
                ),
            );
            let mut import = serde_json::json!({
                "schema": IMPORT_EVENT_SCHEMA,
                "source_ref": request.source_ref,
                "source_attempt_id": deposit.source_attempt_id,
            });
            if let Some(legacy_id) = deposit.source_legacy_id {
                import["source_legacy_id"] = serde_json::Value::String(legacy_id);
            }
            if let Some(reason) = deposit.mapping_reason {
                import["mapping_reason"] = serde_json::Value::String(reason);
            }
            event.payload["import"] = import;
            event.id = vela_protocol::events::compute_event_id(&event);
            event.signature = Some(vela_protocol::sign::sign_event(&event, key)?);
            vela_protocol::reducer::apply_event(&mut project, &event)?;
            project.events.push(event);
            rows[deposit.row_index].status = "imported".to_string();
            summary.deposited += 1;
            summary.events_appended += 1;
        }
        if summary.events_appended > 0 {
            // `apply_event` updates the event-derived side tables but is a
            // deliberately minimal one-step reducer and does not refresh the
            // aggregate stats. Visible state and `vela.lock` hash the full
            // Project, so normalize those derived counters before materializing.
            // Otherwise the first save reports a transient root and a reload
            // immediately detects a lock/frontier mismatch.
            vela_protocol::project::recompute_stats(&mut project);
            vela_protocol::repo::save(&source, &project)?;
        }
    }

    let event_log_hash_after = event_log_hash(&project.events);
    let snapshot_hash_after = snapshot_hash(&project);
    Ok(AttemptImportReport {
        ok: true,
        command: "attempt.import",
        mode: if request.apply { "apply" } else { "dry_run" },
        actor: request.actor.to_string(),
        source: AttemptImportSourceReport {
            ledger: request.ledger.display().to_string(),
            source_ref: request.source_ref.to_string(),
            sha256: source_digest,
            mapping: request.mapping.display().to_string(),
            mapping_schema: IMPORT_MAPPING_SCHEMA,
            exhaustive: mapping_exhaustive,
        },
        frontier: AttemptImportFrontierReport {
            path: request.frontier.display().to_string(),
            frontier_id,
            event_log_hash_before,
            event_log_hash_after,
            snapshot_hash_before,
            snapshot_hash_after,
        },
        summary,
        reconciliation: rows,
    })
}

fn load_mapping(path: &Path) -> Result<AttemptImportMap, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mapping: AttemptImportMap = match serde_json::from_slice(&bytes) {
        Ok(mapping) => mapping,
        Err(json_error) => serde_yaml::from_slice(&bytes).map_err(|yaml_error| {
            format!(
                "parse {} as JSON ({json_error}) or YAML ({yaml_error})",
                path.display()
            )
        })?,
    };
    if mapping.schema != IMPORT_MAPPING_SCHEMA {
        return Err(format!(
            "mapping.schema must be `{IMPORT_MAPPING_SCHEMA}`, got `{}`",
            mapping.schema
        ));
    }
    Ok(mapping)
}

fn ledger_records(value: &serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    if let Some(records) = value.get("records") {
        return records
            .as_array()
            .cloned()
            .ok_or_else(|| "ledger.records must be an array".to_string());
    }
    if value.is_object() {
        return Ok(vec![value.clone()]);
    }
    Err("attempt import input must be an Attempt object or a ledger object".to_string())
}

fn validate_mapping_entry(entry: &AttemptImportMapping) -> Result<(), String> {
    if !entry.attempt_id.starts_with("vat_") {
        return Err(format!(
            "mapping attempt_id must start with `vat_`, got `{}`",
            entry.attempt_id
        ));
    }
    if entry.attempt_id.trim() != entry.attempt_id {
        return Err(format!(
            "mapping attempt_id has surrounding whitespace: `{}`",
            entry.attempt_id
        ));
    }
    if entry
        .frontier
        .as_deref()
        .is_some_and(|s| s.trim().is_empty())
    {
        return Err(format!(
            "mapping {} has an empty frontier override",
            entry.attempt_id
        ));
    }
    if entry.target.as_deref().is_some_and(|s| s.trim().is_empty()) {
        return Err(format!(
            "mapping {} has an empty exclusion target",
            entry.attempt_id
        ));
    }
    if entry
        .expected_attempt_id
        .as_deref()
        .is_some_and(|s| !s.starts_with("vat_"))
    {
        return Err(format!(
            "mapping {} expected_attempt_id must start with `vat_`",
            entry.attempt_id
        ));
    }
    match entry.source_id_rule.as_deref() {
        None | Some("canonical") | Some("legacy_explicit_problem_zero") => {}
        Some(other) => {
            return Err(format!(
                "mapping {} has unknown source_id_rule `{other}`",
                entry.attempt_id
            ));
        }
    }
    match entry.action.as_str() {
        "import" => {
            if entry.target.is_some() {
                return Err(format!(
                    "mapping {}: target metadata is only valid for action=exclude",
                    entry.attempt_id
                ));
            }
            if (entry.problem.is_some() || entry.frontier.is_some())
                && entry.reason.trim().is_empty()
            {
                return Err(format!(
                    "mapping {} changes Attempt content and requires a reason",
                    entry.attempt_id
                ));
            }
            if entry.source_id_rule.as_deref() == Some("legacy_explicit_problem_zero")
                && (entry.problem != Some(0)
                    || entry.frontier.is_none()
                    || entry.expected_attempt_id.is_none())
            {
                return Err(format!(
                    "mapping {}: legacy_explicit_problem_zero requires explicit problem=0, frontier, and expected_attempt_id",
                    entry.attempt_id
                ));
            }
        }
        "exclude" => {
            if entry.reason.trim().is_empty() {
                return Err(format!(
                    "mapping {} exclusion requires a reason",
                    entry.attempt_id
                ));
            }
            if entry.problem.is_some()
                || entry.frontier.is_some()
                || entry.expected_attempt_id.is_some()
                || entry.source_id_rule.is_some()
            {
                return Err(format!(
                    "mapping {} exclusion cannot carry import overrides",
                    entry.attempt_id
                ));
            }
        }
        other => {
            return Err(format!(
                "mapping {} has unknown action `{other}` (expected `import` or `exclude`)",
                entry.attempt_id
            ));
        }
    }
    Ok(())
}

fn validate_source_envelope(source: &Attempt) -> Result<(), String> {
    if source.schema != vela_protocol::attempt::ATTEMPT_SCHEMA {
        return Err(format!(
            "attempt.schema must be `{}`, got `{}`",
            vela_protocol::attempt::ATTEMPT_SCHEMA,
            source.schema
        ));
    }
    if !source.attempt_id.starts_with("vat_") {
        return Err(format!(
            "attempt id must start with `vat_`, got `{}`",
            source.attempt_id
        ));
    }
    if source.signature.is_empty() != source.signer_pubkey_hex.is_empty() {
        return Err("signature and signer_pubkey_hex must both be present or both be empty".into());
    }
    if source.kind.trim().is_empty() {
        return Err("attempt.kind cannot be empty".to_string());
    }
    if source.claim.trim().is_empty() {
        return Err("attempt.claim cannot be empty".to_string());
    }
    if source.claim_digest != vela_protocol::attempt::claim_digest(&source.claim) {
        return Err("attempt.claim_digest does not match claim".to_string());
    }
    if source.cost.failed_attempts > source.cost.total_attempts {
        return Err("attempt.cost.failed_attempts cannot exceed total_attempts".to_string());
    }
    if source.reproduction.successes > source.reproduction.total {
        return Err("attempt.reproduction.successes cannot exceed total".to_string());
    }
    Ok(())
}

fn validate_source_attempt(source: &Attempt, source_id_rule: Option<&str>) -> Result<(), String> {
    validate_source_envelope(source)?;
    let rebuilt_id = if source_id_rule == Some("legacy_explicit_problem_zero") {
        legacy_explicit_problem_zero_id(source)?
    } else {
        source.derive_id()?
    };
    if rebuilt_id != source.attempt_id {
        return Err(format!(
            "attempt_id mismatch: declared {}, rebuilt {rebuilt_id}",
            source.attempt_id
        ));
    }
    if source_id_rule == Some("legacy_explicit_problem_zero") {
        return Ok(());
    }
    let validation_key = SigningKey::from_bytes(&[0x33u8; 32]);
    let rebuilt = Attempt::build(draft_from(source), &validation_key)?;
    if rebuilt.claim_digest != source.claim_digest {
        return Err("attempt.claim_digest does not match claim".to_string());
    }
    if !source.signature.is_empty() {
        source.verify()?;
    }
    Ok(())
}

fn legacy_explicit_problem_zero_id(source: &Attempt) -> Result<String, String> {
    if source.problem != 0 || !source.frontier.is_empty() || !source.signature.is_empty() {
        return Err(
            "legacy_explicit_problem_zero applies only to an unsigned problem=0 source with no frontier"
                .to_string(),
        );
    }
    let mut value = serde_json::to_value(source)
        .map_err(|e| format!("serialize legacy problem-zero source: {e}"))?;
    let object = value
        .as_object_mut()
        .ok_or("legacy problem-zero Attempt must serialize as an object")?;
    object.insert("problem".to_string(), serde_json::json!(0));
    object.insert("attempt_id".to_string(), serde_json::json!(""));
    object.insert("signature".to_string(), serde_json::json!(""));
    object.insert("signer_pubkey_hex".to_string(), serde_json::json!(""));
    let bytes = vela_protocol::canonical::to_canonical_bytes(&value)
        .map_err(|e| format!("canonicalize legacy problem-zero source: {e}"))?;
    Ok(format!("vat_{}", &hex::encode(Sha256::digest(bytes))[..16]))
}

fn rebuild_for_import(
    source: &Attempt,
    mapping: Option<&AttemptImportMapping>,
    key: &SigningKey,
) -> Result<Attempt, String> {
    let mapped_problem = mapping.and_then(|m| m.problem).unwrap_or(source.problem);
    let mapped_frontier = mapping
        .and_then(|m| m.frontier.clone())
        .unwrap_or_else(|| source.frontier.clone());
    let changed = mapped_problem != source.problem || mapped_frontier != source.frontier;
    if !changed && !source.signature.is_empty() {
        return Ok(source.clone());
    }
    let mut draft = draft_from(source);
    draft.problem = mapped_problem;
    draft.frontier = mapped_frontier;
    Attempt::build(draft, key)
}

/// Exhaustive typed copy: every Attempt field other than the four derived
/// authentication/content-address fields survives import byte-for-byte.
fn draft_from(source: &Attempt) -> AttemptDraft {
    AttemptDraft {
        problem: source.problem,
        frontier: source.frontier.clone(),
        kind: source.kind.clone(),
        claim: source.claim.clone(),
        detail: source.detail.clone(),
        claimed_status: source.claimed_status.clone(),
        reproduction: source.reproduction.clone(),
        cost: source.cost.clone(),
        insight: source.insight.clone(),
        depends_on: source.depends_on.clone(),
        related_problems: source.related_problems.clone(),
        reusable_for: source.reusable_for.clone(),
        verifier_attachments: source.verifier_attachments.clone(),
        deliverable_grade: source.deliverable_grade.clone(),
        provenance: source.provenance.clone(),
        base_frontier_root: source.base_frontier_root.clone(),
        target_obligation_id: source.target_obligation_id.clone(),
        statement_variant_id: source.statement_variant_id.clone(),
        method_families: source.method_families.clone(),
        remaining_obligations: source.remaining_obligations.clone(),
        named_obstructions: source.named_obstructions.clone(),
        producer: source.producer.clone(),
    }
}

fn prior_import_target(
    project: &vela_protocol::project::Project,
    source_ref: &str,
    source_attempt_id: &str,
) -> Result<Option<String>, String> {
    let mut targets = BTreeSet::new();
    for event in &project.events {
        if event.kind != vela_protocol::events::EVENT_KIND_ATTEMPT_DEPOSITED {
            continue;
        }
        let Some(import) = event.payload.get("import") else {
            continue;
        };
        if import.get("source_ref").and_then(|v| v.as_str()) != Some(source_ref)
            || import.get("source_attempt_id").and_then(|v| v.as_str()) != Some(source_attempt_id)
        {
            continue;
        }
        let target = event
            .payload
            .get("attempt")
            .and_then(|a| a.get("attempt_id"))
            .and_then(|v| v.as_str())
            .unwrap_or(&event.target.id);
        targets.insert(target.to_string());
    }
    if targets.len() > 1 {
        return Err(format!(
            "source {source_attempt_id} from {source_ref} has conflicting prior import targets: {}",
            targets.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(targets.into_iter().next())
}

fn validate_agent_actor(actor: &str) -> Result<(), String> {
    if let Some(suffix) = actor.strip_prefix("agent:")
        && !suffix.trim().is_empty()
    {
        return Ok(());
    }
    if let Some(suffix) = actor.strip_prefix("ci:")
        && !suffix.trim().is_empty()
    {
        return Ok(());
    }
    Err(format!(
        "attempt import is for agent:/ci: actors, got `{actor}`"
    ))
}

fn validate_source_ref(source_ref: &str) -> Result<(), String> {
    if source_ref.trim() != source_ref || source_ref.chars().any(char::is_whitespace) {
        return Err("source-ref must not contain whitespace".to_string());
    }
    let (repo, commit_and_path) = source_ref
        .rsplit_once('@')
        .ok_or("source-ref must use pinned `repo@commit:path` form")?;
    let (commit, path) = commit_and_path
        .split_once(':')
        .ok_or("source-ref must use pinned `repo@commit:path` form")?;
    if repo.is_empty() || path.is_empty() || path.starts_with('/') {
        return Err("source-ref needs a repository and a repository-relative path".to_string());
    }
    if !(7..=64).contains(&commit.len()) || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("source-ref commit must be a 7-64 character hexadecimal commit id".to_string());
    }
    Ok(())
}

fn load_existing_agent_key() -> Result<SigningKey, String> {
    let encoded = std::env::var(AGENT_KEY_ENV).map_err(|_| {
        format!("attempt import --apply requires an existing agent key in {AGENT_KEY_ENV}")
    })?;
    let bytes = hex::decode(encoded.trim()).map_err(|e| format!("decode {AGENT_KEY_ENV}: {e}"))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{AGENT_KEY_ENV} must be 32 hex bytes"))?;
    Ok(SigningKey::from_bytes(&seed))
}

fn event_log_hash(events: &[vela_protocol::events::StateEvent]) -> String {
    format!("sha256:{}", vela_protocol::events::event_log_hash(events))
}

fn snapshot_hash(project: &vela_protocol::project::Project) -> String {
    format!("sha256:{}", vela_protocol::events::snapshot_hash(project))
}

fn nonempty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use vela_protocol::attempt::{AttemptCost, ProducerRef, Provenance, Reproduction};

    fn key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn unsigned_attempt(problem: u32, claim: &str) -> Attempt {
        let draft = AttemptDraft {
            problem,
            frontier: "erdos".into(),
            kind: "partial_proof".into(),
            claim: claim.into(),
            detail: "exact historical detail".into(),
            claimed_status: "draft".into(),
            reproduction: Reproduction {
                successes: 2,
                total: 3,
            },
            cost: AttemptCost {
                total_attempts: 7,
                failed_attempts: 5,
                compute_note: "seven exact passes".into(),
            },
            insight: "the obstruction survives".into(),
            depends_on: vec!["lemma:one".into()],
            related_problems: vec![23, 686],
            reusable_for: "two-defect kernels".into(),
            verifier_attachments: vec!["vva_test".into()],
            deliverable_grade: Some("partial".into()),
            provenance: Provenance {
                proposer: "agent:test".into(),
                run: "run-1".into(),
                date: "2026-07-13".into(),
            },
            base_frontier_root: "sha256:base".into(),
            target_obligation_id: "obl:1".into(),
            statement_variant_id: "variant:main".into(),
            method_families: vec!["kernel".into()],
            remaining_obligations: vec!["obl:2".into()],
            named_obstructions: vec!["shape:overlap".into()],
            producer: ProducerRef {
                system: "codex".into(),
                version: "test".into(),
                config_digest: "cfg".into(),
            },
        };
        let mut attempt = Attempt::build(draft, &key(1)).unwrap();
        attempt.signature.clear();
        attempt.signer_pubkey_hex.clear();
        attempt
    }

    fn fixture(
        attempts: &[Attempt],
        mappings: serde_json::Value,
    ) -> (TempDir, PathBuf, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let ledger = tmp.path().join("ledger.json");
        let mapping = tmp.path().join("mapping.json");
        let frontier = tmp.path().join("frontier");
        std::fs::write(
            &ledger,
            serde_json::to_vec_pretty(&json!({ "records": attempts })).unwrap(),
        )
        .unwrap();
        std::fs::write(&mapping, serde_json::to_vec_pretty(&mappings).unwrap()).unwrap();
        let project = vela_protocol::project::assemble("import-test", vec![], 0, 0, "test");
        vela_protocol::repo::init_repo(&frontier, &project).unwrap();
        (tmp, ledger, mapping, frontier)
    }

    fn request<'a>(
        ledger: &'a Path,
        mapping: &'a Path,
        frontier: &'a Path,
        apply: bool,
    ) -> AttemptImportRequest<'a> {
        AttemptImportRequest {
            ledger,
            frontier,
            actor: "agent:history-import",
            mapping,
            source_ref: "vela-science/erdos-frontier@1234567890abcdef:attack/attempt-ledger.v2.json",
            apply,
            require_exhaustive: false,
        }
    }

    #[test]
    fn dry_run_then_apply_is_field_preserving_and_root_idempotent() {
        let original = unsigned_attempt(23, "two-defect branch remains open");
        let (_tmp, ledger, mapping, frontier) = fixture(
            std::slice::from_ref(&original),
            json!({ "schema": IMPORT_MAPPING_SCHEMA, "mappings": [] }),
        );

        let dry = import_attempts(&request(&ledger, &mapping, &frontier, false), None).unwrap();
        assert_eq!(dry.summary.records_total, 1);
        assert_eq!(dry.summary.ids_preserved, 1);
        assert_eq!(dry.summary.deposited, 0);
        assert_eq!(
            dry.frontier.event_log_hash_before,
            dry.frontier.event_log_hash_after
        );
        assert_eq!(
            dry.frontier.snapshot_hash_before,
            dry.frontier.snapshot_hash_after
        );
        assert_eq!(dry.reconciliation[0].status, "would_import");
        assert_eq!(
            dry.reconciliation[0].target_attempt_id.as_deref(),
            Some(original.attempt_id.as_str())
        );
        assert!(
            vela_protocol::repo::load_from_path(&frontier)
                .unwrap()
                .attempts
                .is_empty()
        );

        let applied =
            import_attempts(&request(&ledger, &mapping, &frontier, true), Some(&key(9))).unwrap();
        assert_eq!(applied.summary.deposited, 1);
        assert_eq!(applied.summary.events_appended, 1);
        assert_ne!(
            applied.frontier.event_log_hash_before,
            applied.frontier.event_log_hash_after
        );
        assert_ne!(
            applied.frontier.snapshot_hash_before,
            applied.frontier.snapshot_hash_after
        );
        let after_first = vela_protocol::repo::load_from_path(&frontier).unwrap();
        assert_eq!(
            applied.frontier.snapshot_hash_after,
            snapshot_hash(&after_first),
            "reported post-import root must match a fresh replay"
        );
        assert_eq!(after_first.attempts.len(), 1);
        let imported = &after_first.attempts[0];
        imported.verify().unwrap();
        assert_eq!(imported.attempt_id, original.attempt_id);
        let mut imported_body = serde_json::to_value(imported).unwrap();
        let mut original_body = serde_json::to_value(&original).unwrap();
        for field in ["signature", "signer_pubkey_hex"] {
            imported_body.as_object_mut().unwrap().remove(field);
            original_body.as_object_mut().unwrap().remove(field);
        }
        assert_eq!(imported_body, original_body, "all Attempt fields survive");
        let event_count = after_first.events.len();

        let repeated =
            import_attempts(&request(&ledger, &mapping, &frontier, true), Some(&key(9))).unwrap();
        assert_eq!(repeated.summary.deposited, 0);
        assert_eq!(repeated.summary.events_appended, 0);
        assert_eq!(repeated.summary.already_imported, 1);
        assert_eq!(
            repeated.frontier.event_log_hash_before,
            repeated.frontier.event_log_hash_after
        );
        assert_eq!(
            repeated.frontier.snapshot_hash_before,
            repeated.frontier.snapshot_hash_after
        );
        assert_eq!(
            vela_protocol::repo::load_from_path(&frontier)
                .unwrap()
                .events
                .len(),
            event_count
        );
    }

    #[test]
    fn mapping_changes_one_id_and_accounts_for_an_exclusion() {
        let normalize = unsigned_attempt(64, "normalize this campaign audit");
        let exclude = unsigned_attempt(396704, "OEIS B3 construction");
        let mappings = json!({
            "schema": IMPORT_MAPPING_SCHEMA,
            "mappings": [
                {
                    "attempt_id": normalize.attempt_id,
                    "action": "import",
                    "problem": 0,
                    "frontier": "vfr_erdos",
                    "reason": "corpus-level audit"
                },
                {
                    "attempt_id": exclude.attempt_id,
                    "action": "exclude",
                    "reason": "belongs to the Sidon/OEIS frontier",
                    "target": "sidon-frontier"
                }
            ]
        });
        let (_tmp, ledger, mapping, frontier) = fixture(&[normalize, exclude], mappings);
        let report = import_attempts(&request(&ledger, &mapping, &frontier, false), None).unwrap();
        assert_eq!(report.summary.records_total, 2);
        assert_eq!(report.summary.import_records, 1);
        assert_eq!(report.summary.ids_changed, 1);
        assert_eq!(report.summary.excluded, 1);
        assert!(!report.reconciliation[0].id_preserved.unwrap());
        assert_eq!(
            report.reconciliation[1].target.as_deref(),
            Some("sidon-frontier")
        );
        assert_eq!(report.reconciliation[1].status, "excluded");
    }

    #[test]
    fn validation_rejects_unaccountable_or_unpinned_inputs() {
        let attempt = unsigned_attempt(23, "claim");
        let unknown_id = "vat_0000000000000000";
        let mappings = json!({
            "schema": IMPORT_MAPPING_SCHEMA,
            "mappings": [{
                "attempt_id": unknown_id,
                "action": "exclude",
                "reason": "not present",
                "target": "elsewhere"
            }]
        });
        let (_tmp, ledger, mapping, frontier) = fixture(&[attempt], mappings);
        let error =
            import_attempts(&request(&ledger, &mapping, &frontier, false), None).unwrap_err();
        assert!(error.contains("ledger does not contain it"));

        let mut unpinned = request(&ledger, &mapping, &frontier, false);
        unpinned.source_ref = "local/ledger.json";
        assert!(
            import_attempts(&unpinned, None)
                .unwrap_err()
                .contains("repo@commit:path")
        );

        let mut human = request(&ledger, &mapping, &frontier, false);
        human.actor = "reviewer:human";
        assert!(
            import_attempts(&human, None)
                .unwrap_err()
                .contains("agent:/ci:")
        );
        let mut bare_agent = request(&ledger, &mapping, &frontier, false);
        bare_agent.actor = "agent:";
        assert!(
            import_attempts(&bare_agent, None)
                .unwrap_err()
                .contains("agent:/ci:")
        );

        let apply_without_key = request(&ledger, &mapping, &frontier, true);
        assert!(
            import_attempts(&apply_without_key, None)
                .unwrap_err()
                .contains(AGENT_KEY_ENV)
        );
    }

    #[test]
    fn exhaustive_yaml_mapping_supports_the_narrow_legacy_zero_rule() {
        let mut legacy = unsigned_attempt(64, "full-corpus audit");
        legacy.problem = 0;
        legacy.frontier.clear();
        legacy.attempt_id = legacy_explicit_problem_zero_id(&legacy).unwrap();
        let mut target_draft = draft_from(&legacy);
        target_draft.frontier = "vfr_0a25edabc16db143".into();
        let expected = Attempt::build(target_draft, &key(4)).unwrap().attempt_id;
        let (_tmp, ledger, mapping, frontier) = fixture(
            std::slice::from_ref(&legacy),
            json!({ "schema": IMPORT_MAPPING_SCHEMA, "mappings": [] }),
        );
        std::fs::write(
            &mapping,
            format!(
                "schema: {IMPORT_MAPPING_SCHEMA}\nexhaustive: true\nmappings:\n  - attempt_id: {}\n    action: import\n    problem: 0\n    frontier: vfr_0a25edabc16db143\n    reason: corpus-level audit\n    expected_attempt_id: {expected}\n    source_id_rule: legacy_explicit_problem_zero\n",
                legacy.attempt_id
            ),
        )
        .unwrap();
        let mut req = request(&ledger, &mapping, &frontier, false);
        req.require_exhaustive = true;
        let report = import_attempts(&req, None).unwrap();
        assert!(report.source.exhaustive);
        assert_eq!(report.summary.ids_changed, 1);
        assert_eq!(
            report.reconciliation[0].target_attempt_id.as_deref(),
            Some(expected.as_str())
        );

        std::fs::write(
            &mapping,
            format!("schema: {IMPORT_MAPPING_SCHEMA}\nexhaustive: true\nmappings: []\n"),
        )
        .unwrap();
        assert!(
            import_attempts(&req, None)
                .unwrap_err()
                .contains("exhaustive mapping is missing")
        );
    }
}
