//! Packet inspection and validation utilities.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Canonical packet artifacts: replay-bearing, signed, load-bearing for
/// proof. These are what `proof-trace.checked_artifacts` requires; a
/// proof packet's verifiability stands or falls on these.
///
/// Doctrine: a canonical artifact carries protocol state. Two
/// implementations should produce byte-identical canonical artifacts
/// from the same logical content.
pub const CANONICAL_PACKET_FILES: &[&str] = &[
    "manifest.json",
    "packet.lock.json",
    "proof-trace.json",
    "ro-crate-metadata.jsonld",
    "findings/full.json",
    "artifacts/artifacts.json",
    "artifacts/artifact-audit.json",
    "artifacts/blob-map.json",
    "sources/source-registry.json",
    "research-traces/research-traces.json",
    "research-traces/verifier-attachments.json",
    "evidence/evidence-atoms.json",
    "evidence/source-evidence-map.json",
    "conditions/condition-records.json",
    "events/events.json",
    "events/replay-report.json",
    "proposals/proposals.json",
    "reviews/review-events.json",
    "reviews/confidence-updates.json",
    "check-summary.json",
];

/// Derived packet artifacts: regenerable projections over canonical
/// state. These ship in the packet for human inspection but their
/// values are reconstructible from the canonical files. A consumer that
/// wants to verify a derived artifact should re-run the projection
/// from canonical inputs and compare, not trust the packet's copy.
///
/// Doctrine: a derived artifact is a view, not a fact. It must be
/// idempotently regenerable from the canonical layer.
pub const DERIVED_PACKET_ARTIFACTS: &[&str] = &[
    "overview.json",
    "scope.json",
    "source-table.json",
    "evidence-matrix.json",
    "conditions/condition-matrix.json",
    "source-integrity/source-debt.json",
    "reviewer/source-debt.json",
    "reviewer/research-trace-provenance.json",
    "reviewer/score-ledger.json",
    "reviewer/correction-returns.json",
    "reviewer/frontier-graph.json",
    "reviewer/impact-index.json",
    "reviewer/guided-tours.json",
    "reviewer/frontier-freshness-plan.json",
    "reviewer/replay-manifest.json",
    "decisions/decision-view.json",
    "signals.json",
    "review-queue.json",
    "quality-table.json",
    "state-transitions.json",
    "candidate-tensions.json",
    "candidate-gaps.json",
    "candidate-bridges.json",
    "mcp-session.json",
];

/// Every artifact a complete packet ships — canonical + derived. Used
/// by `vela packet validate` to assert structural completeness.
pub const REQUIRED_PACKET_FILES: &[&str] = &[
    "manifest.json",
    "packet.lock.json",
    "proof-trace.json",
    "ro-crate-metadata.jsonld",
    "findings/full.json",
    "artifacts/artifacts.json",
    "artifacts/artifact-audit.json",
    "artifacts/blob-map.json",
    "sources/source-registry.json",
    "research-traces/research-traces.json",
    "research-traces/verifier-attachments.json",
    "evidence/evidence-atoms.json",
    "evidence/source-evidence-map.json",
    "conditions/condition-records.json",
    "conditions/condition-matrix.json",
    "source-integrity/source-debt.json",
    "reviewer/source-debt.json",
    "reviewer/research-trace-provenance.json",
    "reviewer/score-ledger.json",
    "reviewer/correction-returns.json",
    "reviewer/frontier-graph.json",
    "reviewer/impact-index.json",
    "reviewer/guided-tours.json",
    "reviewer/frontier-freshness-plan.json",
    "reviewer/replay-manifest.json",
    "decisions/decision-view.json",
    "events/events.json",
    "events/replay-report.json",
    "proposals/proposals.json",
    "reviews/review-events.json",
    "reviews/confidence-updates.json",
    "check-summary.json",
    "overview.json",
    "scope.json",
    "source-table.json",
    "evidence-matrix.json",
    "signals.json",
    "review-queue.json",
    "quality-table.json",
    "state-transitions.json",
    "candidate-tensions.json",
    "candidate-gaps.json",
    "candidate-bridges.json",
    "mcp-session.json",
];
/// Canonical-only packet artifacts. Use when checking proof-bearing
/// correctness, not packet completeness.
pub fn canonical_packet_files() -> &'static [&'static str] {
    CANONICAL_PACKET_FILES
}

// Minimal deserialize schema for `validate`: only the fields it reads. The
// manifest carries more keys (packet_version, generated_at, stats, full
// source metadata); serde ignores the unmodeled ones. The human-facing
// `inspect` view that read them was retired.
#[derive(Debug, Deserialize)]
struct PacketManifest {
    packet_format: String,
    source: PacketSource,
    included_files: Vec<PacketManifestFile>,
}

#[derive(Debug, Deserialize)]
struct PacketSource {
    project_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PacketManifestFile {
    path: String,
    sha256: String,
    bytes: usize,
}

#[derive(Debug, Deserialize)]
struct ProofTrace {
    trace_version: String,
    generated_at: Option<String>,
    #[serde(default)]
    command: Vec<String>,
    source: String,
    source_hash: String,
    #[serde(default)]
    snapshot_hash: Option<String>,
    #[serde(default)]
    event_log_hash: Option<String>,
    #[serde(default)]
    proposal_state_hash: Option<String>,
    #[serde(default)]
    replay_status: Option<String>,
    #[serde(default)]
    packet_manifest_hash: Option<String>,
    schema_version: String,
    checked_artifacts: Vec<String>,
    packet_manifest: Option<String>,
    packet_validation: Option<String>,
    caveats: Vec<String>,
    status: String,
    trace_path: Option<String>,
}

pub fn validate(path: &Path) -> Result<String, String> {
    let manifest = load_manifest(path)?;
    if manifest.packet_format != "vela.frontier-packet" {
        return Err(format!(
            "Unsupported packet format '{}' in {}",
            manifest.packet_format,
            path.display()
        ));
    }

    let mut checked = 0usize;
    for file in &manifest.included_files {
        let abs = path.join(&file.path);
        let bytes = std::fs::read(&abs)
            .map_err(|e| format!("Missing or unreadable packet file {}: {e}", abs.display()))?;
        if file.path == "proof-trace.json" {
            validate_proof_trace(path, &abs)?;
        }
        if bytes.len() != file.bytes {
            return Err(format!(
                "Packet file size mismatch for {}: manifest={}, actual={}",
                file.path,
                file.bytes,
                bytes.len()
            ));
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != file.sha256 {
            return Err(format!(
                "Packet checksum mismatch for {}: manifest={}, actual={}",
                file.path, file.sha256, actual_hash
            ));
        }
        checked += 1;
    }

    for required in REQUIRED_PACKET_FILES {
        if !path.join(required).exists() {
            return Err(format!("Packet missing required file: {}", required));
        }
    }

    validate_packet_lock(path)?;
    validate_replay_report(path)?;
    validate_decision_view(path)?;
    validate_source_evidence(path)?;
    crate::research_trace::validate_packet_traces(path)?;
    validate_conditions(path)?;
    validate_artifact_payloads(path)?;

    validate_proof_trace(path, &path.join("proof-trace.json"))?;

    Ok(format!(
        "vela packet validate\n  root: {}\n  status: ok\n  checked_files: {}\n  project: {}",
        path.display(),
        checked,
        manifest.source.project_name
    ))
}

fn load_manifest(path: &Path) -> Result<PacketManifest, String> {
    let manifest_path = path.join("manifest.json");
    let manifest_data = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "Failed to read packet manifest {}: {e}",
            manifest_path.display()
        )
    })?;
    serde_json::from_str(&manifest_data).map_err(|e| {
        format!(
            "Failed to parse packet manifest {}: {e}",
            manifest_path.display()
        )
    })
}

fn validate_proof_trace(packet_dir: &Path, trace_path: &Path) -> Result<(), String> {
    let trace_data = std::fs::read_to_string(trace_path)
        .map_err(|e| format!("Failed to read proof trace {}: {e}", trace_path.display()))?;
    let trace: ProofTrace = serde_json::from_str(&trace_data)
        .map_err(|e| format!("Failed to parse proof trace {}: {e}", trace_path.display()))?;

    if trace.trace_version.trim().is_empty() {
        return Err("Proof trace missing trace_version".to_string());
    }
    if !trace.command.is_empty()
        && trace
            .command
            .first()
            .is_none_or(|command| command != "vela")
    {
        return Err("Proof trace command must start with vela when present".to_string());
    }
    if let Some(generated_at) = &trace.generated_at
        && generated_at.trim().is_empty()
    {
        return Err("Proof trace generated_at must be non-empty when present".to_string());
    }
    if trace.source.trim().is_empty() {
        return Err("Proof trace source must be non-empty".to_string());
    }
    if !is_sha256_hex(&trace.source_hash) {
        return Err(format!(
            "Proof trace source_hash must be a 64-character sha256 hex digest, got '{}'",
            trace.source_hash
        ));
    }
    if trace.schema_version.trim().is_empty() {
        return Err("Proof trace schema_version must be non-empty".to_string());
    }
    if trace
        .snapshot_hash
        .as_deref()
        .is_some_and(|hash| !is_sha256_hex(hash))
    {
        return Err("Proof trace snapshot_hash must be a sha256 hex digest".to_string());
    }
    if trace
        .event_log_hash
        .as_deref()
        .is_some_and(|hash| !is_sha256_hex(hash))
    {
        return Err("Proof trace event_log_hash must be a sha256 hex digest".to_string());
    }
    if trace
        .proposal_state_hash
        .as_deref()
        .is_some_and(|hash| !is_sha256_hex(hash))
    {
        return Err("Proof trace proposal_state_hash must be a sha256 hex digest".to_string());
    }
    if trace
        .packet_manifest_hash
        .as_deref()
        .is_some_and(|hash| !is_sha256_hex(hash))
    {
        return Err("Proof trace packet_manifest_hash must be a sha256 hex digest".to_string());
    }
    if trace
        .replay_status
        .as_deref()
        .is_some_and(|status| status != "ok" && status != "no_events")
    {
        return Err("Proof trace replay_status must be ok or no_events".to_string());
    }
    if trace.status != "ok" {
        return Err(format!(
            "Proof trace status must be ok, got '{}'",
            trace.status
        ));
    }
    if trace.caveats.is_empty() {
        return Err("Proof trace must include caveats".to_string());
    }
    // Phase K: proof-bearing means canonical-only. Derived artifacts
    // ship in the packet for inspection but are regenerable; their
    // checksums are validated structurally (manifest line above) but
    // their absence from `checked_artifacts` is not a proof failure.
    for required in CANONICAL_PACKET_FILES {
        if !trace
            .checked_artifacts
            .iter()
            .any(|artifact| artifact == required)
        {
            return Err(format!(
                "Proof trace checked_artifacts missing canonical artifact: {}",
                required
            ));
        }
    }
    if let Some(packet_manifest) = &trace.packet_manifest
        && !Path::new(packet_manifest).ends_with("manifest.json")
    {
        return Err("Proof trace packet_manifest must point to manifest.json".to_string());
    }
    if let Some(packet_validation) = &trace.packet_validation
        && !packet_validation.contains("status: ok")
    {
        return Err("Proof trace packet_validation must include status: ok".to_string());
    }
    if let Some(trace_path_value) = &trace.trace_path
        && !Path::new(trace_path_value).ends_with("proof-trace.json")
    {
        return Err("Proof trace trace_path must point to proof-trace.json".to_string());
    }
    if !packet_dir.join("manifest.json").exists() {
        return Err("Proof trace validation requires packet manifest".to_string());
    }

    Ok(())
}

fn validate_replay_report(packet_dir: &Path) -> Result<(), String> {
    let events_path = packet_dir.join("events/events.json");
    if !events_path.is_file() {
        return Err("Packet missing canonical events file".to_string());
    }
    let replay_path = packet_dir.join("events/replay-report.json");
    let replay_data = std::fs::read_to_string(&replay_path).map_err(|e| {
        format!(
            "Failed to read replay report {}: {e}",
            replay_path.display()
        )
    })?;
    let replay: serde_json::Value = serde_json::from_str(&replay_data).map_err(|e| {
        format!(
            "Failed to parse replay report {}: {e}",
            replay_path.display()
        )
    })?;
    if replay["ok"].as_bool() != Some(true) {
        return Err("Replay report status is not ok".to_string());
    }
    let status = replay["status"].as_str().unwrap_or_default();
    if status != "ok" && status != "no_events" {
        return Err(format!("Replay report has unsupported status: {status}"));
    }
    Ok(())
}

fn validate_decision_view(packet_dir: &Path) -> Result<(), String> {
    let events_path = packet_dir.join("events/events.json");
    let events_data = std::fs::read(&events_path)
        .map_err(|error| format!("Failed to read canonical events: {error}"))?;
    let events: Vec<serde_json::Value> = serde_json::from_slice(&events_data)
        .map_err(|error| format!("Failed to parse canonical events: {error}"))?;
    let events_by_id = events
        .iter()
        .filter_map(|event| {
            event
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id.to_string(), event))
        })
        .collect::<BTreeMap<_, _>>();
    let expected_decisions = events
        .iter()
        .filter(|event| {
            matches!(
                event.get("kind").and_then(serde_json::Value::as_str),
                Some("review.accepted" | "review.rejected" | "review.revision_requested")
            )
        })
        .filter_map(|event| event.get("id").and_then(serde_json::Value::as_str))
        .collect::<BTreeSet<_>>();

    let view_path = packet_dir.join("decisions/decision-view.json");
    let view_data = std::fs::read(&view_path)
        .map_err(|error| format!("Failed to read packet decision view: {error}"))?;
    let view: serde_json::Value = serde_json::from_slice(&view_data)
        .map_err(|error| format!("Failed to parse packet decision view: {error}"))?;
    if view.get("schema").and_then(serde_json::Value::as_str)
        != Some("vela.packet-decision-view.v1")
        || view.get("derived").and_then(serde_json::Value::as_bool) != Some(true)
        || view
            .get("authoritative_source")
            .and_then(serde_json::Value::as_str)
            != Some("events/events.json")
    {
        return Err(
            "Packet decision view must be typed, derived, and point to canonical events"
                .to_string(),
        );
    }
    let authority_statement = view
        .get("authority_statement")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !authority_statement.contains("event") || !authority_statement.contains("authority") {
        return Err("Packet decision view obscures its authority boundary".to_string());
    }
    let decisions = view
        .get("decisions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Packet decision view decisions must be an array".to_string())?;
    let mut actual_decisions = BTreeSet::new();
    for decision in decisions {
        let authoritative_event = decision
            .get("authoritative_event")
            .ok_or_else(|| "Packet decision record omits authoritative_event".to_string())?;
        let event_id = authoritative_event
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Packet decision authoritative event omits id".to_string())?;
        let canonical_event = events_by_id
            .get(event_id)
            .ok_or_else(|| format!("Packet decision references unknown event {event_id}"))?;
        if *canonical_event != authoritative_event {
            return Err(format!(
                "Packet decision authoritative event differs from events/events.json: {event_id}"
            ));
        }
        if !expected_decisions.contains(event_id) {
            return Err(format!(
                "Packet decision record references non-decision event {event_id}"
            ));
        }
        if !actual_decisions.insert(event_id) {
            return Err(format!("Packet decision event is duplicated: {event_id}"));
        }
        let decision_root = decision.get("decision_root");
        let root_status = decision
            .get("decision_root_status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let event_refs = authoritative_event
            .pointer("/payload/provenance/input_refs")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter_map(|reference| reference.strip_prefix("urn:vela:decision-root:"))
            .collect::<Vec<_>>();
        match (
            root_status,
            decision_root.and_then(serde_json::Value::as_str),
        ) {
            ("bound", Some(root))
                if root.strip_prefix("sha256:").is_some_and(|digest| {
                    digest.len() == 64
                        && digest
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                }) && event_refs == [root] => {}
            ("unavailable_legacy_event", None) if event_refs.is_empty() => {}
            _ => {
                return Err(format!(
                    "Packet decision root binding is inconsistent for {event_id}"
                ));
            }
        }
        let policy_certificate = decision.get("policy_certificate");
        let semantic_event = decision
            .pointer("/semantic_effect/event_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| events_by_id.get(id).copied());
        let canonical_certificate = authoritative_event
            .pointer("/payload/policy_lane/certificate")
            .or_else(|| {
                semantic_event.and_then(|event| event.pointer("/payload/policy_lane/certificate"))
            });
        let certificate_present = policy_certificate.is_some_and(|value| !value.is_null());
        if certificate_present != canonical_certificate.is_some()
            || (certificate_present && policy_certificate != canonical_certificate)
        {
            return Err(format!(
                "Packet decision policy certificate is inconsistent for {event_id}"
            ));
        }
        let authority_role = decision
            .get("authority_role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if (certificate_present && authority_role != "signed_policy")
            || (!certificate_present && authority_role == "signed_policy")
        {
            return Err(format!(
                "Packet decision authority role is false for {event_id}"
            ));
        }
        if let Some(effect) = decision
            .get("semantic_effect")
            .filter(|value| !value.is_null())
        {
            let effect_id = effect
                .get("event_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "Packet semantic effect omits event_id".to_string())?;
            let effect_event = events_by_id
                .get(effect_id)
                .ok_or_else(|| format!("Packet semantic effect event is missing: {effect_id}"))?;
            let bytes = vela_protocol::canonical::to_canonical_bytes(*effect_event)
                .map_err(|error| format!("Canonicalize packet semantic event: {error}"))?;
            let expected_root = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
            if effect.get("event_root").and_then(serde_json::Value::as_str)
                != Some(expected_root.as_str())
            {
                return Err(format!(
                    "Packet semantic effect root is stale for {effect_id}"
                ));
            }
        }
    }
    if actual_decisions != expected_decisions {
        return Err("Packet decision view omits or invents canonical decisions".to_string());
    }
    Ok(())
}

fn validate_source_evidence(packet_dir: &Path) -> Result<(), String> {
    let sources_path = packet_dir.join("sources/source-registry.json");
    let atoms_path = packet_dir.join("evidence/evidence-atoms.json");
    let findings_path = packet_dir.join("findings/full.json");

    let sources_data = std::fs::read_to_string(&sources_path).map_err(|e| {
        format!(
            "Failed to read source registry {}: {e}",
            sources_path.display()
        )
    })?;
    let atoms_data = std::fs::read_to_string(&atoms_path).map_err(|e| {
        format!(
            "Failed to read evidence atoms {}: {e}",
            atoms_path.display()
        )
    })?;
    let findings_data = std::fs::read_to_string(&findings_path).map_err(|e| {
        format!(
            "Failed to read packet findings {}: {e}",
            findings_path.display()
        )
    })?;

    let sources: serde_json::Value = serde_json::from_str(&sources_data).map_err(|e| {
        format!(
            "Failed to parse source registry {}: {e}",
            sources_path.display()
        )
    })?;
    let atoms: serde_json::Value = serde_json::from_str(&atoms_data).map_err(|e| {
        format!(
            "Failed to parse evidence atoms {}: {e}",
            atoms_path.display()
        )
    })?;
    let findings: serde_json::Value = serde_json::from_str(&findings_data).map_err(|e| {
        format!(
            "Failed to parse packet findings {}: {e}",
            findings_path.display()
        )
    })?;

    let source_ids = sources
        .as_array()
        .ok_or("Source registry must be a JSON array")?
        .iter()
        .filter_map(|source| source["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let finding_ids = findings
        .as_array()
        .ok_or("Packet findings/full.json must be a JSON array")?
        .iter()
        .filter_map(|finding| finding["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut atoms_by_finding = std::collections::BTreeMap::<&str, usize>::new();

    for atom in atoms
        .as_array()
        .ok_or("Evidence atoms must be a JSON array")?
    {
        let source_id = atom["source_id"]
            .as_str()
            .ok_or("Evidence atom missing source_id")?;
        let finding_id = atom["finding_id"]
            .as_str()
            .ok_or("Evidence atom missing finding_id")?;
        if !source_ids.contains(source_id) {
            return Err(format!(
                "Evidence atom references missing source_id: {source_id}"
            ));
        }
        if !finding_ids.contains(finding_id) {
            return Err(format!(
                "Evidence atom references missing finding_id: {finding_id}"
            ));
        }
        *atoms_by_finding.entry(finding_id).or_default() += 1;
    }

    for finding in findings
        .as_array()
        .ok_or("Packet findings/full.json must be a JSON array")?
    {
        let id = finding["id"].as_str().unwrap_or_default();
        let retracted = finding["flags"]["retracted"].as_bool().unwrap_or(false);
        if !retracted && !atoms_by_finding.contains_key(id) {
            return Err(format!("Active finding has no evidence atom: {id}"));
        }
    }

    Ok(())
}

fn validate_conditions(packet_dir: &Path) -> Result<(), String> {
    let conditions_path = packet_dir.join("conditions/condition-records.json");
    let atoms_path = packet_dir.join("evidence/evidence-atoms.json");
    let findings_path = packet_dir.join("findings/full.json");

    let conditions_data = std::fs::read_to_string(&conditions_path).map_err(|e| {
        format!(
            "Failed to read condition records {}: {e}",
            conditions_path.display()
        )
    })?;
    let atoms_data = std::fs::read_to_string(&atoms_path).map_err(|e| {
        format!(
            "Failed to read evidence atoms {}: {e}",
            atoms_path.display()
        )
    })?;
    let findings_data = std::fs::read_to_string(&findings_path).map_err(|e| {
        format!(
            "Failed to read packet findings {}: {e}",
            findings_path.display()
        )
    })?;

    let conditions: serde_json::Value = serde_json::from_str(&conditions_data).map_err(|e| {
        format!(
            "Failed to parse condition records {}: {e}",
            conditions_path.display()
        )
    })?;
    let atoms: serde_json::Value = serde_json::from_str(&atoms_data).map_err(|e| {
        format!(
            "Failed to parse evidence atoms {}: {e}",
            atoms_path.display()
        )
    })?;
    let findings: serde_json::Value = serde_json::from_str(&findings_data).map_err(|e| {
        format!(
            "Failed to parse packet findings {}: {e}",
            findings_path.display()
        )
    })?;

    let condition_ids = conditions
        .as_array()
        .ok_or("Condition records must be a JSON array")?
        .iter()
        .filter_map(|condition| condition["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let finding_ids = findings
        .as_array()
        .ok_or("Packet findings/full.json must be a JSON array")?
        .iter()
        .filter_map(|finding| finding["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for condition in conditions
        .as_array()
        .ok_or("Condition records must be a JSON array")?
    {
        let finding_id = condition["finding_id"]
            .as_str()
            .ok_or("Condition record missing finding_id")?;
        if !finding_ids.contains(finding_id) {
            return Err(format!(
                "Condition record references missing finding_id: {finding_id}"
            ));
        }
    }
    for atom in atoms
        .as_array()
        .ok_or("Evidence atoms must be a JSON array")?
    {
        for condition_ref in atom["condition_refs"]
            .as_array()
            .ok_or("Evidence atom missing condition_refs")?
            .iter()
            .filter_map(|value| value.as_str())
        {
            if condition_ref.starts_with("finding:") {
                continue;
            }
            if !condition_ids.contains(condition_ref) {
                return Err(format!(
                    "Evidence atom references missing condition record: {condition_ref}"
                ));
            }
        }
    }

    Ok(())
}

fn validate_artifact_payloads(packet_dir: &Path) -> Result<(), String> {
    let artifacts_path = packet_dir.join("artifacts/artifacts.json");
    let audit_path = packet_dir.join("artifacts/artifact-audit.json");
    let blob_map_path = packet_dir.join("artifacts/blob-map.json");

    let artifacts_data = std::fs::read_to_string(&artifacts_path).map_err(|e| {
        format!(
            "Failed to read artifact records {}: {e}",
            artifacts_path.display()
        )
    })?;
    let audit_data = std::fs::read_to_string(&audit_path).map_err(|e| {
        format!(
            "Failed to read artifact audit {}: {e}",
            audit_path.display()
        )
    })?;
    let blob_map_data = std::fs::read_to_string(&blob_map_path).map_err(|e| {
        format!(
            "Failed to read artifact blob map {}: {e}",
            blob_map_path.display()
        )
    })?;

    let artifacts: serde_json::Value = serde_json::from_str(&artifacts_data).map_err(|e| {
        format!(
            "Failed to parse artifact records {}: {e}",
            artifacts_path.display()
        )
    })?;
    let audit: serde_json::Value = serde_json::from_str(&audit_data).map_err(|e| {
        format!(
            "Failed to parse artifact audit {}: {e}",
            audit_path.display()
        )
    })?;
    let blob_map: serde_json::Value = serde_json::from_str(&blob_map_data).map_err(|e| {
        format!(
            "Failed to parse artifact blob map {}: {e}",
            blob_map_path.display()
        )
    })?;

    let artifact_rows = artifacts
        .as_array()
        .ok_or("Artifact records must be a JSON array")?;
    let blob_rows = blob_map
        .as_array()
        .ok_or("Artifact blob map must be a JSON array")?;

    if audit["ok"].as_bool() != Some(true) {
        return Err("Artifact audit status is not ok".to_string());
    }
    if audit["artifact_count"].as_u64() != Some(artifact_rows.len() as u64) {
        return Err("Artifact audit count does not match artifacts/artifacts.json".to_string());
    }
    if audit["issue_count"].as_u64().unwrap_or(1) != 0 {
        return Err("Artifact audit reports non-zero issues".to_string());
    }

    let lifecycle_v2 = audit["schema"].as_str() == Some("vela.artifact_audit.v2");
    if audit.get("schema").is_some() && !lifecycle_v2 {
        return Err("Artifact audit has unsupported schema".to_string());
    }
    let active_ids = artifact_rows
        .iter()
        .filter(|artifact| !artifact["retracted"].as_bool().unwrap_or(false))
        .filter_map(|artifact| artifact["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let retracted_ids = artifact_rows
        .iter()
        .filter(|artifact| artifact["retracted"].as_bool().unwrap_or(false))
        .filter_map(|artifact| artifact["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let active_local_ids = artifact_rows
        .iter()
        .filter(|artifact| !artifact["retracted"].as_bool().unwrap_or(false))
        .filter(|artifact| {
            matches!(
                artifact["storage_mode"].as_str(),
                Some("local_blob" | "local_file")
            )
        })
        .filter_map(|artifact| artifact["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if lifecycle_v2 {
        if audit["active_artifact_count"].as_u64() != Some(active_ids.len() as u64) {
            return Err("Artifact audit active count does not match artifacts.json".to_string());
        }
        if audit["retracted_artifact_count"].as_u64() != Some(retracted_ids.len() as u64) {
            return Err("Artifact audit retracted count does not match artifacts.json".to_string());
        }
        let issues = audit["issues"]
            .as_array()
            .ok_or("Artifact audit issues must be an array")?;
        if audit["issue_count"].as_u64() != Some(issues.len() as u64) {
            return Err("Artifact audit issue_count does not match issues".to_string());
        }
        if issues.iter().any(|issue| {
            issue["id"]
                .as_str()
                .is_none_or(|id| !active_ids.contains(id))
        }) {
            return Err("Artifact audit active issue targets a non-active artifact".to_string());
        }
        let historical = audit["historical_issues"]
            .as_array()
            .ok_or("Artifact audit historical_issues must be an array")?;
        if audit["historical_issue_count"].as_u64() != Some(historical.len() as u64) {
            return Err(
                "Artifact audit historical_issue_count does not match historical_issues"
                    .to_string(),
            );
        }
        if historical.iter().any(|issue| {
            issue["id"]
                .as_str()
                .is_none_or(|id| !retracted_ids.contains(id))
        }) {
            return Err(
                "Artifact audit historical issue targets a non-retracted artifact".to_string(),
            );
        }
    }

    let mut blob_by_artifact = std::collections::BTreeMap::new();
    for row in blob_rows {
        let id = row["artifact_id"]
            .as_str()
            .ok_or("Artifact blob map row missing artifact_id")?;
        if blob_by_artifact.insert(id, row).is_some() {
            return Err(format!("Duplicate artifact blob map row for {id}"));
        }
        if lifecycle_v2 && !active_local_ids.contains(id) {
            return Err(format!(
                "Artifact blob map targets unknown, remote, or retracted artifact {id}"
            ));
        }
    }
    let mut local_artifact_count = 0u64;

    for artifact in artifact_rows {
        let id = artifact["id"].as_str().unwrap_or("<unknown>");
        if lifecycle_v2 && artifact["retracted"].as_bool().unwrap_or(false) {
            continue;
        }
        let storage_mode = artifact["storage_mode"].as_str().unwrap_or_default();
        if storage_mode != "local_blob" && storage_mode != "local_file" {
            continue;
        }
        local_artifact_count += 1;
        let content_hash = artifact["content_hash"]
            .as_str()
            .ok_or_else(|| format!("Artifact {id} missing content_hash"))?;
        let Some(hex) = content_hash.strip_prefix("sha256:") else {
            return Err(format!(
                "Artifact {id} content_hash must use sha256:<hex> format"
            ));
        };
        if !is_sha256_hex(hex) {
            return Err(format!("Artifact {id} content_hash is not sha256 hex"));
        }
        let blob = blob_by_artifact
            .get(id)
            .ok_or_else(|| format!("Local artifact {id} missing packet blob map entry"))?;
        if blob["content_hash"].as_str() != Some(content_hash) {
            return Err(format!("Artifact {id} blob map content_hash mismatch"));
        }
        let packet_path = blob["packet_path"]
            .as_str()
            .ok_or_else(|| format!("Artifact {id} blob map missing packet_path"))?;
        let blob_path = packet_dir.join(packet_path);
        let bytes = std::fs::read(&blob_path).map_err(|e| {
            format!(
                "Artifact {id} packet blob is unreadable at {}: {e}",
                blob_path.display()
            )
        })?;
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != hex {
            return Err(format!(
                "Artifact {id} packet blob hash mismatch: expected {hex}, found {actual_hash}"
            ));
        }
        if let Some(size) = blob["size_bytes"].as_u64()
            && size != bytes.len() as u64
        {
            return Err(format!(
                "Artifact {id} blob size mismatch: expected {size}, found {}",
                bytes.len()
            ));
        }
    }

    if audit["checked_local_blobs"].as_u64().unwrap_or(0) != local_artifact_count {
        return Err(
            "Artifact audit checked_local_blobs does not match local artifacts".to_string(),
        );
    }

    Ok(())
}

fn validate_packet_lock(packet_dir: &Path) -> Result<(), String> {
    let lock_path = packet_dir.join("packet.lock.json");
    let lock_data = std::fs::read_to_string(&lock_path)
        .map_err(|e| format!("Failed to read packet lock {}: {e}", lock_path.display()))?;
    let lock: serde_json::Value = serde_json::from_str(&lock_data)
        .map_err(|e| format!("Failed to parse packet lock {}: {e}", lock_path.display()))?;
    if lock["lock_format"].as_str() != Some("vela.packet-lock.v1") {
        return Err("Packet lock has unsupported lock_format".to_string());
    }
    let Some(files) = lock["files"].as_array() else {
        return Err("Packet lock missing files array".to_string());
    };
    for file in files {
        let Some(path_value) = file["path"].as_str() else {
            return Err("Packet lock file entry missing path".to_string());
        };
        let Some(expected_hash) = file["sha256"].as_str() else {
            return Err(format!("Packet lock entry missing sha256 for {path_value}"));
        };
        let bytes = std::fs::read(packet_dir.join(path_value))
            .map_err(|e| format!("Packet lock references unreadable file {path_value}: {e}"))?;
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != expected_hash {
            return Err(format!(
                "Packet lock checksum mismatch for {}: lock={}, actual={}",
                path_value, expected_hash, actual_hash
            ));
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(root: &Path, path: &str, body: &[u8]) -> PacketManifestFile {
        let abs = root.join(path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&abs, body).unwrap();
        PacketManifestFile {
            path: path.to_string(),
            sha256: sha256_hex(body),
            bytes: body.len(),
        }
    }

    fn refresh_packet_entry(root: &Path, path: &str, body: &[u8]) {
        let lock_path = root.join("packet.lock.json");
        let mut lock: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        let lock_files = lock["files"].as_array_mut().unwrap();
        let lock_entry = lock_files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!(path))
            .unwrap();
        lock_entry["sha256"] = serde_json::json!(sha256_hex(body));
        lock_entry["bytes"] = serde_json::json!(body.len());
        let lock_bytes = serde_json::to_vec_pretty(&lock).unwrap();
        fs::write(&lock_path, &lock_bytes).unwrap();

        let manifest_path = root.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let manifest_files = manifest["included_files"].as_array_mut().unwrap();
        let manifest_entry = manifest_files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!(path))
            .unwrap();
        manifest_entry["sha256"] = serde_json::json!(sha256_hex(body));
        manifest_entry["bytes"] = serde_json::json!(body.len());
        let lock_entry = manifest_files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!("packet.lock.json"))
            .unwrap();
        lock_entry["sha256"] = serde_json::json!(sha256_hex(&lock_bytes));
        lock_entry["bytes"] = serde_json::json!(lock_bytes.len());
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn write_valid_packet(root: &Path) {
        let mut files = vec![
            write_file(root, "README.md", b"packet"),
            write_file(root, "reviewer-guide.md", b"guide"),
            write_file(root, "overview.json", br#"{"findings":1}"#),
            write_file(root, "scope.json", br#"{"frontier_name":"test"}"#),
            write_file(root, "source-table.json", br#"[]"#),
            write_file(root, "sources/source-registry.json", br#"[]"#),
            write_file(root, "research-traces/research-traces.json", br#"[]"#),
            write_file(
                root,
                "research-traces/verifier-attachments.json",
                br#"[]"#,
            ),
            write_file(root, "evidence-matrix.json", br#"[]"#),
            write_file(root, "evidence/evidence-atoms.json", br#"[]"#),
            write_file(root, "evidence/source-evidence-map.json", br#"{"schema":"vela.source-evidence-map.v0","sources":{}}"#),
            write_file(root, "conditions/condition-records.json", br#"[]"#),
            write_file(root, "conditions/condition-matrix.json", br#"{"schema":"vela.condition-matrix.v0","conditions":[]}"#),
            write_file(root, "source-integrity/source-debt.json", br#"{"schema":"vela.packet_source_debt.v0.1","summary":{"total":0,"critical":0,"high":0,"medium":0,"review":0,"missing_locator_rows":0,"missing_content_hash_rows":0},"claim_boundary":{"claims_source_truth":false,"claims_clinical_validity":false,"claims_scientific_discovery":false,"claims_treatment_advice":false},"items":[]}"#),
            write_file(root, "reviewer/source-debt.json", br#"{"schema":"vela.packet_source_debt.v0.1","summary":{"total":0,"critical":0,"high":0,"medium":0,"review":0,"missing_locator_rows":0,"missing_content_hash_rows":0},"claim_boundary":{"claims_source_truth":false,"claims_clinical_validity":false,"claims_scientific_discovery":false,"claims_treatment_advice":false},"items":[]}"#),
            write_file(root, "reviewer/research-trace-provenance.json", br#"{"schema":"vela.packet_research_trace_provenance.v0.1","summary":{"traces":0,"verifier_attachments":0},"claim_boundary":{"traces_are_source_material":true,"claims_accepted_findings":false,"claims_external_validation":false,"tracked_frontier_mutated":false},"research_traces":[],"verifier_attachments":[]}"#),
            write_file(root, "reviewer/score-ledger.json", br#"{"schema":"vela.public_benchmark_score_ledger.v0.1","summary":{"score_returns":0,"valid_returns":0,"invalid_returns":0,"local_returns":0,"external_returns":0,"externally_scored_tasks":0,"claim_status":"not_available_in_packet_export"},"claim_boundary":{"score_ledger_is_review_material":true,"claims_external_validation":false,"claims_external_review":false}}"#),
            write_file(root, "reviewer/correction-returns.json", br#"{"schema":"vela.packet_correction_returns.v0.1","summary":{"templates":0,"proposals":0},"template":null,"proposals":[],"claim_boundary":{"corrections_are_review_material":true,"claims_external_validation":false,"claims_clinical_validity":false,"claims_target_validation":false,"claims_treatment_advice":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "reviewer/frontier-graph.json", br#"{"schema":"vela.frontier_graph.v0.1","frontier":"test","summary":{"nodes":0,"edges":0},"nodes":[],"edges":[],"claim_boundary":{"graph_is_derived":true,"claims_external_validation":false,"claims_target_validation":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "reviewer/impact-index.json", br#"{"schema":"vela.frontier_graph_impact_index.v0.1","frontier":"test","summary":{"finding_neighborhoods":0,"max_neighbors_per_finding":0},"finding_neighborhoods":[],"claim_boundary":{"impact_is_simulated":true,"claims_external_validation":false,"claims_target_validation":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "reviewer/guided-tours.json", br#"{"schema":"vela.frontier_guided_tours.v0.1","frontier":"test","summary":{"tours":0,"steps":0},"tours":[],"claim_boundary":{"tours_are_review_material":true,"claims_external_validation":false,"claims_target_validation":false,"claims_treatment_advice":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "reviewer/frontier-freshness-plan.json", br#"{"schema":"vela.frontier_freshness_plan.v0.1","frontier":"test","summary":{"channels":0,"review_entry_paths":0},"channels":[],"claim_boundary":{"fresh_inputs_are_source_material":true,"claims_external_validation":false,"claims_target_validation":false,"claims_treatment_advice":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "reviewer/replay-manifest.json", br#"{"schema":"vela.packet_reviewer_replay_manifest.v0.1","frontier":"test","reviewer_inputs":{"source_debt":"reviewer/source-debt.json","research_trace_provenance":"reviewer/research-trace-provenance.json","score_ledger":"reviewer/score-ledger.json","correction_returns":"reviewer/correction-returns.json","outsider_handoff":"review/outsider-handoff.v1.json"},"commands":[],"artifact_hashes":[],"claim_boundary":{"replay_manifest_is_review_material":true,"claims_external_validation":false,"claims_target_validation":false,"tracked_frontier_mutated":false}}"#),
            write_file(root, "decisions/decision-view.json", br#"{"schema":"vela.packet-decision-view.v1","derived":true,"authority_statement":"The canonical event log is authority.","authoritative_source":"events/events.json","frontier_id":"vfr_test","event_log_hash":"sha256:0000000000000000000000000000000000000000000000000000000000000000","reducer_version":"0.0.0","replay_command":"vela frontier materialize <frontier>","decisions":[]}"#),
            write_file(root, "candidate-tensions.json", br#"[]"#),
            write_file(root, "candidate-gaps.json", br#"[]"#),
            write_file(root, "candidate-bridges.json", br#"[]"#),
            write_file(root, "mcp-session.json", br#"{"recommended_loop":[]}"#),
            write_file(root, "check-summary.json", br#"{"status":"ok"}"#),
            write_file(root, "signals.json", br#"[]"#),
            write_file(root, "review-queue.json", br#"[]"#),
            write_file(root, "quality-table.json", br#"{"proof_readiness":{"status":"ready"}}"#),
            write_file(
                root,
                "state-transitions.json",
                br#"{"schema":"vela.state-transitions.v0","transitions":[]}"#,
            ),
            write_file(root, "events/events.json", br#"[]"#),
            write_file(
                root,
                "events/replay-report.json",
                br#"{"ok":true,"status":"no_events","baseline_hash":null,"replayed_hash":null,"current_hash":null,"conflicts":[],"applied_events":0}"#,
            ),
            write_file(root, "proposals/proposals.json", br#"[]"#),
            write_file(root, "ro-crate-metadata.jsonld", br#"{"@context":"https://w3id.org/ro/crate/1.2/context","@graph":[]}"#),
            write_file(
                root,
                "proof-trace.json",
                br#"{"trace_version":"0.2.0","generated_at":"2026-04-22T00:00:00Z","source":"test","source_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","schema_version":"0.2.0","checked_artifacts":["manifest.json","overview.json","scope.json","source-table.json","sources/source-registry.json","research-traces/research-traces.json","research-traces/verifier-attachments.json","evidence-matrix.json","evidence/evidence-atoms.json","evidence/source-evidence-map.json","conditions/condition-records.json","conditions/condition-matrix.json","candidate-tensions.json","candidate-gaps.json","candidate-bridges.json","mcp-session.json","check-summary.json","signals.json","review-queue.json","quality-table.json","state-transitions.json","events/events.json","events/replay-report.json","proposals/proposals.json","ro-crate-metadata.jsonld","proof-trace.json","packet.lock.json","findings/full.json","artifacts/artifacts.json","artifacts/artifact-audit.json","artifacts/blob-map.json","reviews/review-events.json","reviews/confidence-updates.json"],"event_log_hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","proposal_state_hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","replay_status":"no_events","caveats":["candidate outputs require review"],"status":"ok"}"#,
            ),
            write_file(root, "findings/full.json", br#"[]"#),
            write_file(root, "artifacts/artifacts.json", br#"[]"#),
            write_file(root, "artifacts/artifact-audit.json", br#"{"ok":true,"command":"artifact-audit","frontier":"test","artifact_count":0,"checked_local_blobs":0,"local_blob_bytes":0,"by_kind":{},"by_storage_mode":{},"issue_count":0,"issues":[]}"#),
            write_file(root, "artifacts/blob-map.json", br#"[]"#),
            write_file(root, "reviews/review-events.json", br#"[]"#),
            write_file(root, "reviews/confidence-updates.json", br#"[]"#),
        ];
        let lock = serde_json::json!({
            "lock_format": "vela.packet-lock.v1",
            "generated_at": "2026-04-22T00:00:00Z",
            "files": files.clone(),
        });
        let lock_bytes = serde_json::to_vec_pretty(&lock).unwrap();
        files.push(write_file(root, "packet.lock.json", &lock_bytes));
        let manifest = serde_json::json!({
            "packet_format": "vela.frontier-packet",
            "packet_version": "v1",
            "generated_at": "2026-04-22T00:00:00Z",
            "source": {
                "project_name": "test",
                "description": "test packet",
                "compiled_at": "2026-04-22T00:00:00Z",
                "compiler": format!("vela/{}", env!("CARGO_PKG_VERSION")),
                "vela_version": "0.10.0",
                "schema": "https://vela.science/schema/finding-bundle/v0.10.0"
            },
            "stats": {
                "findings": 1,
                "sources": 0,
                "evidence_atoms": 0,
                "condition_records": 0,
                "review_events": 0,
                "gaps": 0,
                "contested": 0,
                "bridge_entities": 0,
                "contradiction_edges": 0
            },
            "included_files": files,
        });
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn write_valid_trace(root: &Path) {
        let trace = serde_json::json!({
            "trace_version": "0.1.0",
            "command": ["vela", "proof"],
            "source": "frontiers/fixture.json",
            "source_hash": "a".repeat(64),
            "schema_version": "0.2.0",
            "checked_artifacts": [
                "manifest.json",
                "overview.json",
                "scope.json",
                "source-table.json",
                "sources/source-registry.json",
                "research-traces/research-traces.json",
                "research-traces/verifier-attachments.json",
                "evidence-matrix.json",
                "evidence/evidence-atoms.json",
                "evidence/source-evidence-map.json",
                "conditions/condition-records.json",
                "conditions/condition-matrix.json",
                "candidate-tensions.json",
                "candidate-gaps.json",
                "candidate-bridges.json",
                "mcp-session.json",
                "check-summary.json",
                "signals.json",
                "review-queue.json",
                "quality-table.json",
                "state-transitions.json",
                "events/events.json",
                "events/replay-report.json",
                "proposals/proposals.json",
                "ro-crate-metadata.jsonld",
                "proof-trace.json",
                "packet.lock.json",
                "findings/full.json",
                "artifacts/artifacts.json",
                "artifacts/artifact-audit.json",
                "artifacts/blob-map.json",
                "reviews/review-events.json",
                "reviews/confidence-updates.json"
            ],
            "proposal_state_hash": "a".repeat(64),
            "benchmark": null,
            "packet_manifest": root.join("manifest.json").display().to_string(),
            "packet_validation": "vela packet validate\n  status: ok",
            "caveats": ["candidate outputs require review"],
            "status": "ok",
            "trace_path": root.join("proof-trace.json").display().to_string()
        });
        fs::write(
            root.join("proof-trace.json"),
            serde_json::to_vec_pretty(&trace).unwrap(),
        )
        .unwrap();
        let trace_bytes = fs::read(root.join("proof-trace.json")).unwrap();
        refresh_packet_entry(root, "proof-trace.json", &trace_bytes);
    }

    #[test]
    fn validates_packet_with_proof_trace() {
        let tmp = TempDir::new().unwrap();
        write_valid_packet(tmp.path());
        write_valid_trace(tmp.path());

        let result = validate(tmp.path()).unwrap();
        assert!(result.contains("status: ok"));
    }

    fn install_decision_fixture(
        root: &Path,
        event: serde_json::Value,
        decision: serde_json::Value,
    ) {
        let events = serde_json::to_vec_pretty(&serde_json::json!([event])).unwrap();
        fs::write(root.join("events/events.json"), &events).unwrap();
        refresh_packet_entry(root, "events/events.json", &events);
        let view = serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "vela.packet-decision-view.v1",
            "derived": true,
            "authority_statement": "This derived view is not authority; the canonical event is authority.",
            "authoritative_source": "events/events.json",
            "frontier_id": "vfr_test",
            "event_log_hash": format!("sha256:{}", "0".repeat(64)),
            "reducer_version": env!("CARGO_PKG_VERSION"),
            "replay_command": "vela frontier materialize <frontier>",
            "decisions": [decision]
        }))
        .unwrap();
        fs::write(root.join("decisions/decision-view.json"), &view).unwrap();
        refresh_packet_entry(root, "decisions/decision-view.json", &view);
    }

    #[test]
    fn validates_bound_human_decision_and_rejects_omission() {
        let tmp = TempDir::new().unwrap();
        write_valid_packet(tmp.path());
        write_valid_trace(tmp.path());
        let root = format!("sha256:{}", "a".repeat(64));
        let event = serde_json::json!({
            "id": "vev_human",
            "kind": "review.accepted",
            "actor": {"id": "reviewer:alice", "type": "reviewer"},
            "target": {"type": "proposal", "id": "vpr_human"},
            "payload": {
                "proposal_id": "vpr_human",
                "proposal_kind": "finding.add",
                "verdict": "accepted",
                "provenance": {"input_refs": [format!("urn:vela:decision-root:{root}")]}
            },
            "signature": "ed25519:test"
        });
        let decision = serde_json::json!({
            "authoritative_event": event.clone(),
            "proposal_id": "vpr_human",
            "decision_root": root,
            "decision_root_status": "bound",
            "authority_role": "reviewer",
            "policy_certificate": null,
            "semantic_effect": null
        });
        install_decision_fixture(tmp.path(), event, decision);
        validate(tmp.path()).unwrap();

        let path = tmp.path().join("decisions/decision-view.json");
        let mut view: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        view["decisions"] = serde_json::json!([]);
        let bytes = serde_json::to_vec_pretty(&view).unwrap();
        fs::write(&path, &bytes).unwrap();
        refresh_packet_entry(tmp.path(), "decisions/decision-view.json", &bytes);
        let error = validate(tmp.path()).unwrap_err();
        assert!(error.contains("omits or invents"), "{error}");
    }

    #[test]
    fn validates_policy_certificate_and_rejects_false_authority() {
        let tmp = TempDir::new().unwrap();
        write_valid_packet(tmp.path());
        write_valid_trace(tmp.path());
        let certificate = serde_json::json!({
            "schema": "vela.decision_certificate.v1",
            "id": "vdc_test",
            "outcome": "permit"
        });
        let event = serde_json::json!({
            "id": "vev_policy",
            "kind": "review.accepted",
            "actor": {"id": "policy:test", "type": "policy"},
            "target": {"type": "proposal", "id": "vpr_policy"},
            "payload": {
                "proposal_id": "vpr_policy",
                "proposal_kind": "finding.add",
                "verdict": "accepted",
                "policy_lane": {"certificate": certificate.clone()}
            },
            "signature": null
        });
        let decision = serde_json::json!({
            "authoritative_event": event.clone(),
            "proposal_id": "vpr_policy",
            "decision_root": null,
            "decision_root_status": "unavailable_legacy_event",
            "authority_role": "signed_policy",
            "policy_certificate": certificate,
            "semantic_effect": null
        });
        install_decision_fixture(tmp.path(), event, decision);
        validate(tmp.path()).unwrap();

        let path = tmp.path().join("decisions/decision-view.json");
        let mut view: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        view["decisions"][0]["policy_certificate"] = serde_json::Value::Null;
        let bytes = serde_json::to_vec_pretty(&view).unwrap();
        fs::write(&path, &bytes).unwrap();
        refresh_packet_entry(tmp.path(), "decisions/decision-view.json", &bytes);
        let error = validate(tmp.path()).unwrap_err();
        assert!(error.contains("policy certificate"), "{error}");
    }

    #[test]
    fn rejects_bad_proof_trace_hash() {
        let tmp = TempDir::new().unwrap();
        write_valid_packet(tmp.path());
        write_valid_trace(tmp.path());
        let trace_path = tmp.path().join("proof-trace.json");
        let mut trace: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&trace_path).unwrap()).unwrap();
        trace["source_hash"] = serde_json::json!("not-a-hash");
        let trace_bytes = serde_json::to_vec_pretty(&trace).unwrap();
        fs::write(&trace_path, &trace_bytes).unwrap();

        let lock_path = tmp.path().join("packet.lock.json");
        let mut lock: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        let files = lock["files"].as_array_mut().unwrap();
        let entry = files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!("proof-trace.json"))
            .unwrap();
        entry["sha256"] = serde_json::json!(sha256_hex(&trace_bytes));
        entry["bytes"] = serde_json::json!(trace_bytes.len());
        let lock_bytes = serde_json::to_vec_pretty(&lock).unwrap();
        fs::write(&lock_path, &lock_bytes).unwrap();

        let manifest_path = tmp.path().join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let manifest_files = manifest["included_files"].as_array_mut().unwrap();
        let manifest_entry = manifest_files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!("proof-trace.json"))
            .unwrap();
        manifest_entry["sha256"] = serde_json::json!(sha256_hex(&trace_bytes));
        manifest_entry["bytes"] = serde_json::json!(trace_bytes.len());
        let lock_entry = manifest_files
            .iter_mut()
            .find(|entry| entry["path"] == serde_json::json!("packet.lock.json"))
            .unwrap();
        lock_entry["sha256"] = serde_json::json!(sha256_hex(&lock_bytes));
        lock_entry["bytes"] = serde_json::json!(lock_bytes.len());
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let err = validate(tmp.path()).unwrap_err();
        assert!(err.contains("source_hash"));
    }

    #[test]
    fn validates_packet_local_artifact_blobs() {
        let tmp = TempDir::new().unwrap();
        let blob_bytes = b"{\"nct\":\"NCT03887455\"}\n";
        let digest = sha256_hex(blob_bytes);
        let content_hash = format!("sha256:{digest}");
        let packet_path = format!("artifacts/blobs/sha256/{digest}");
        write_file(tmp.path(), &packet_path, blob_bytes);
        write_file(
            tmp.path(),
            "artifacts/artifacts.json",
            serde_json::to_string(&serde_json::json!([
                {
                    "id": "va_checked_blob",
                    "storage_mode": "local_blob",
                    "content_hash": content_hash,
                    "size_bytes": blob_bytes.len()
                }
            ]))
            .unwrap()
            .as_bytes(),
        );
        write_file(
            tmp.path(),
            "artifacts/artifact-audit.json",
            serde_json::to_string(&serde_json::json!({
                "ok": true,
                "artifact_count": 1,
                "checked_local_blobs": 1,
                "issue_count": 0
            }))
            .unwrap()
            .as_bytes(),
        );
        write_file(
            tmp.path(),
            "artifacts/blob-map.json",
            serde_json::to_string(&serde_json::json!([
                {
                    "artifact_id": "va_checked_blob",
                    "content_hash": format!("sha256:{digest}"),
                    "packet_path": packet_path,
                    "size_bytes": blob_bytes.len()
                }
            ]))
            .unwrap()
            .as_bytes(),
        );

        validate_artifact_payloads(tmp.path()).unwrap();

        fs::write(
            tmp.path().join(format!("artifacts/blobs/sha256/{digest}")),
            b"tampered",
        )
        .unwrap();
        let err = validate_artifact_payloads(tmp.path()).unwrap_err();
        assert!(err.contains("packet blob hash mismatch"));
    }

    #[test]
    fn lifecycle_v2_accepts_historical_issues_without_retracted_blob() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "artifacts/artifacts.json",
            serde_json::to_string(&serde_json::json!([
                {
                    "id": "va_retired_blob",
                    "storage_mode": "local_blob",
                    "content_hash": format!("sha256:{}", "a".repeat(64)),
                    "retracted": true
                }
            ]))
            .unwrap()
            .as_bytes(),
        );
        write_file(
            tmp.path(),
            "artifacts/artifact-audit.json",
            serde_json::to_string(&serde_json::json!({
                "schema": "vela.artifact_audit.v2",
                "ok": true,
                "artifact_count": 1,
                "active_artifact_count": 0,
                "retracted_artifact_count": 1,
                "checked_local_blobs": 0,
                "issue_count": 0,
                "issues": [],
                "historical_issue_count": 1,
                "historical_issues": [{
                    "id": "va_retired_blob",
                    "field": "locator",
                    "message": "historical blob is unavailable"
                }]
            }))
            .unwrap()
            .as_bytes(),
        );
        write_file(tmp.path(), "artifacts/blob-map.json", b"[]");

        validate_artifact_payloads(tmp.path()).unwrap();

        let bad_map = serde_json::to_vec(&serde_json::json!([{
            "artifact_id": "va_retired_blob",
            "content_hash": format!("sha256:{}", "a".repeat(64)),
            "packet_path": "artifacts/blobs/sha256/retired"
        }]))
        .unwrap();
        fs::write(tmp.path().join("artifacts/blob-map.json"), bad_map).unwrap();
        let err = validate_artifact_payloads(tmp.path()).unwrap_err();
        assert!(err.contains("unknown, remote, or retracted"));
    }
}
