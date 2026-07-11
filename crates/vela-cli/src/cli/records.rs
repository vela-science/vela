//! Portable activity records: the `vela land` mint/propose adapters and
//! witness-file collection for `vela reproduce`.

use super::*;

/// Parse a witness file: either a bare `vela_verify::Witness`, or an
/// object with a `witness` field wrapping one (a record that ships its
/// construction).
pub(crate) fn parse_witness(raw: &str) -> Result<vela_verify::Witness, String> {
    if let Ok(w) = serde_json::from_str::<vela_verify::Witness>(raw) {
        return Ok(w);
    }
    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if let Some(inner) = value.get("witness") {
        return serde_json::from_value(inner.clone()).map_err(|e| e.to_string());
    }
    Err("not a witness (missing recognized `kind`, and no `witness` field)".to_string())
}

/// Collect witness files for `vela reproduce`: a single file, or every
/// `*.witness.json` under a directory (preferring a `witnesses/` subdir).
pub(crate) fn collect_witness_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let root = {
        let sub = path.join("witnesses");
        if sub.is_dir() {
            sub
        } else {
            path.to_path_buf()
        }
    };
    let mut out = Vec::new();
    collect_witness_files_into(&root, &mut out);
    out.sort();
    out
}

fn collect_witness_files_into(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_witness_files_into(&p, out);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".witness.json"))
        {
            out.push(p);
        }
    }
}

// ── land adapters (the workflow engine's record path) ────────────────

fn value_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn value_str_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| value_str(Some(item)))
                .collect()
        })
        .unwrap_or_default()
}

/// Mint a signed activity record from a Receipt, for `vela land`.
/// Artifacts are hashed NOW (land time), the head is pinned NOW, and
/// the record signs under the executor's agent session key (agents) or
/// lands unsigned-honest for humans (their accept carries the key).
pub(crate) fn mint_record_for_land(
    frontier: &std::path::Path,
    receipt: &crate::workflow::Receipt,
    executor: &str,
) -> Result<serde_json::Value, String> {
    use sha2::{Digest, Sha256};
    use vela_protocol::record::{
        ActivityRecord, ActivityRecordDraft, RecordArtifact, RecordSource, RecordVerifierRun,
    };

    let project = repo::load_from_path(frontier)?;
    let mut artifacts = Vec::new();
    for a in &receipt.artifacts {
        let path = frontier.join(&a.path);
        let bytes =
            std::fs::read(&path).map_err(|e| format!("artifact {}: {e}", path.display()))?;
        artifacts.push(RecordArtifact {
            locator: a.path.clone(),
            kind: if a.kind.is_empty() {
                "witness".to_string()
            } else {
                a.kind.clone()
            },
            sha256: hex::encode(Sha256::digest(&bytes)),
            note: String::new(),
        });
    }
    let verifier_runs = receipt
        .verifier_runs
        .iter()
        .map(|r| RecordVerifierRun {
            method: r.method.clone(),
            outcome: r.outcome.clone(),
            output_hash: r.log.clone(),
            solver: r.solver.clone(),
        })
        .collect();
    let source_env = receipt.environment.get("source").and_then(Value::as_object);
    let source = source_env.map(|source| {
        let name = value_str(source.get("name"))
            .or_else(|| value_str(source.get("project")))
            .or_else(|| value_str(source.get("system")))
            .unwrap_or_default();
        let source_type =
            value_str(source.get("source_type")).unwrap_or_else(|| "database_record".to_string());
        let uri = value_str(source.get("source_uri"))
            .or_else(|| value_str(source.get("uri")))
            .unwrap_or_default();
        let authors = value_str_array(source.get("authors"));
        RecordSource {
            name,
            source_type,
            uri,
            authors,
        }
    });
    let source_refs = source_env
        .map(|source| value_str_array(source.get("source_refs")))
        .unwrap_or_default();
    let receipt_value = serde_json::to_value(receipt)
        .map_err(|e| format!("serialize receipt for review binding: {e}"))?;
    let receipt_bytes = vela_protocol::canonical::to_canonical_bytes(&receipt_value)
        .map_err(|e| format!("canonicalize receipt for review binding: {e}"))?;
    let receipt_digest = format!("sha256:{}", hex::encode(Sha256::digest(&receipt_bytes)));
    let lineage = vela_protocol::receipt_v1::lineage_from_layer(&receipt.lineage);
    let draft = ActivityRecordDraft {
        frontier_id: project.frontier_id().to_string(),
        against_head: vela_protocol::events::event_log_hash(&project.events),
        assertion: receipt.claim.clone(),
        assertion_type: receipt.r#type.clone(),
        artifacts,
        verifier_runs,
        caveats: receipt.caveats.clone(),
        source,
        source_refs,
        receipt_digest,
        lineage,
        emitted_by: executor.to_string(),
        emitted_at: chrono::Utc::now().to_rfc3339(),
    };
    let key = if executor.starts_with("agent:") || executor.starts_with("ci:") {
        Some(vela_edge::vela_agent_mcp::agent_signing_key(Some(
            executor,
        ))?)
    } else {
        None
    };
    let record = ActivityRecord::build(draft, key.as_ref())?;
    // Persist next to the frontier's other records so locators resolve.
    let records_dir = frontier.join("records");
    std::fs::create_dir_all(&records_dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_value(&record).map_err(|e| e.to_string())?;
    std::fs::write(
        records_dir.join(format!("{}.json", record.id)),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(body)
}

/// Land a minted record as a PENDING finding proposal (never applies —
/// deciding is the policy's or the human's job). Returns the vpr_ id.
pub(crate) fn propose_record_for_land(
    frontier: &std::path::Path,
    record_json: &serde_json::Value,
) -> Result<String, String> {
    use vela_protocol::record::ActivityRecord;
    let rc: ActivityRecord =
        serde_json::from_value(record_json.clone()).map_err(|e| format!("record parse: {e}"))?;
    let signed = rc.verify()?;
    let report = state::add_finding(
        frontier,
        rc.to_finding_draft("recorded against the current head", signed),
        false,
    )?;
    if report.proposal_id.is_empty() {
        return Err("record landed no proposal".to_string());
    }
    Ok(report.proposal_id)
}
