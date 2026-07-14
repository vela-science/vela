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

// ── land adapters (the workflow engine's pure compatibility index) ──

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

/// Derive the compatibility activity-record index from one validated Receipt
/// v1 without writing anything. The canonical receipt remains the evidence
/// source of truth; every copied field here is a deterministic read-only index
/// that is bound back to `receipt_digest` and `receipt_path`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_record_for_land(
    receipt: &vela_protocol::receipt_v1::ReceiptV1,
    frontier_id: &str,
    against_head: &str,
    receipt_digest: &str,
    receipt_path: &str,
    operation_id: &str,
    executor: &str,
    emitted_at: &str,
    artifacts: Vec<vela_protocol::record::RecordArtifact>,
    key: Option<&ed25519_dalek::SigningKey>,
) -> Result<vela_protocol::record::ActivityRecord, String> {
    use vela_protocol::record::{
        ActivityRecord, ActivityRecordDraft, RecordSource, RecordVerifierRun,
    };

    let value = receipt.as_value();
    let claim = value
        .get("claim")
        .and_then(Value::as_str)
        .ok_or_else(|| "validated receipt is missing claim".to_string())?;
    let claim_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "validated receipt is missing type".to_string())?;
    let caveats = value
        .get("caveats")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated receipt is missing caveats".to_string())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(ToString::to_string)
                .ok_or_else(|| "validated receipt caveat is not text".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let verifier_runs = value
        .get("verifier_runs")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated receipt is missing verifier_runs".to_string())?
        .iter()
        .map(|run| RecordVerifierRun {
            method: run
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            outcome: run
                .get("outcome")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            output_hash: run
                .get("output_hash")
                .or_else(|| run.get("log_digest"))
                .or_else(|| run.get("log"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            solver: run
                .get("solver")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
        .collect();
    let environment = value.get("environment").unwrap_or(&Value::Null);
    let source_env = environment.get("source").and_then(Value::as_object);
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
    let lineage = vela_protocol::receipt_v1::lineage_from_receipt(value);
    let draft = ActivityRecordDraft {
        frontier_id: frontier_id.to_string(),
        against_head: against_head.to_string(),
        assertion: claim.to_string(),
        assertion_type: claim_type.to_string(),
        artifacts,
        verifier_runs,
        caveats,
        source,
        source_refs,
        receipt_digest: receipt_digest.to_string(),
        receipt_path: receipt_path.to_string(),
        operation_id: operation_id.to_string(),
        lineage,
        emitted_by: executor.to_string(),
        emitted_at: emitted_at.to_string(),
    };
    ActivityRecord::build(draft, key)
}

/// Derive the pending finding proposal without applying or persisting it.
pub(crate) fn proposal_for_record_land(
    record: &vela_protocol::record::ActivityRecord,
    at: &str,
) -> Result<vela_protocol::proposals::StateProposal, String> {
    let signed = record.verify()?;
    record.to_finding_proposal_at("recorded against the current head", signed, at)
}
