use crate::cli::{collect_witness_files, fail, fail_return, parse_witness, print_json};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use vela_protocol::repo;

pub(crate) fn cmd_verify_evidence(action: VerifyAction) {
    match action {
        VerifyAction::Import {
            frontier,
            record,
            actor,
            push,
            json,
        } => {
            crate::ui::set_mode("verification.import", json);
            let bytes = std::fs::read(&record).unwrap_or_else(|error| {
                fail_return(&format!("read {}: {error}", record.display()))
            });
            let record = vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)
                .unwrap_or_else(|error| {
                    fail_return(&format!(
                        "parse {} as Verification Record v1: {error}",
                        record.display()
                    ))
                });
            let result = crate::workflow::import_verification(&frontier, &record, &actor, push)
                .unwrap_or_else(|error| fail_return(&error));
            if json {
                print_json(&result);
            } else {
                println!(
                    "verification import: retained {} for proposal {}",
                    result.verification_record_id, result.proposal_id
                );
                println!("  acceptance: unchanged (delta 0)");
                println!("  outcome: {}", result.outcome);
            }
        }
    }
}

fn verified_local_artifact(
    frontier: &Path,
    artifact: &vela_protocol::bundle::Artifact,
) -> Result<PathBuf, String> {
    verified_frontier_file(
        frontier,
        &artifact.id,
        &artifact.storage_mode,
        artifact.locator.as_deref(),
        &artifact.content_hash,
    )
}

fn verified_frontier_file(
    frontier: &Path,
    artifact_id: &str,
    storage_mode: &str,
    locator: Option<&str>,
    expected_root: &str,
) -> Result<PathBuf, String> {
    if !matches!(storage_mode, "local_blob" | "local_file") {
        return Err(format!(
            "proposal verifier artifact {} is not locally reproducible",
            artifact_id
        ));
    }
    let locator = locator
        .ok_or_else(|| format!("proposal verifier artifact {artifact_id} has no locator"))?;
    let relative = Path::new(locator);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "proposal verifier artifact {} locator must remain frontier-relative",
            artifact_id
        ));
    }
    let frontier_root = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve frontier root: {error}"))?;
    let file = frontier.join(relative);
    let metadata = std::fs::symlink_metadata(&file).map_err(|error| {
        format!(
            "inspect proposal verifier artifact {}: {error}",
            artifact_id
        )
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "proposal verifier artifact {} must be a regular non-symlink file",
            artifact_id
        ));
    }
    let resolved = std::fs::canonicalize(&file).map_err(|error| {
        format!(
            "resolve proposal verifier artifact {}: {error}",
            artifact_id
        )
    })?;
    if !resolved.starts_with(&frontier_root) {
        return Err(format!(
            "proposal verifier artifact {} resolves outside the frontier",
            artifact_id
        ));
    }
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&resolved)
        .map_err(|error| format!("read proposal verifier artifact {}: {error}", artifact_id))?;
    let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if observed != expected_root {
        return Err(format!(
            "proposal verifier artifact {} content root does not match retained bytes",
            artifact_id
        ));
    }
    Ok(resolved)
}

fn reproduction_result_path(frontier: &Path, file: &Path, proposal_scoped: bool) -> String {
    if !proposal_scoped {
        return file.display().to_string();
    }
    let root = std::fs::canonicalize(frontier).unwrap_or_else(|_| frontier.to_path_buf());
    let resolved = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    resolved.strip_prefix(&root).map_or_else(
        |_| {
            file.file_name().map_or_else(
                || "artifact".to_string(),
                |name| name.to_string_lossy().into(),
            )
        },
        |relative| relative.display().to_string(),
    )
}

pub(crate) fn proposal_reproduction_files(
    path: &Path,
    proposal_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let project =
        repo::load_from_path(path).map_err(|error| format!("load proposal frontier: {error}"))?;
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    if proposal.status != "pending_review" {
        return Err(format!(
            "proposal {proposal_id} is {}, not pending_review",
            proposal.status
        ));
    }
    if proposal.kind != "finding.add" {
        return Err(format!(
            "proposal-scoped reproduction requires finding.add, got {}",
            proposal.kind
        ));
    }
    let finding_id = proposal.target.id.as_str();
    let retained = project
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.media_type.as_deref() == Some("application/json")
                && artifact.metadata.contains_key("verifier")
                && artifact
                    .target_findings
                    .iter()
                    .any(|target| target == finding_id)
        })
        .collect::<Vec<_>>();
    if !retained.is_empty() {
        return retained
            .into_iter()
            .map(|artifact| {
                let file = verified_local_artifact(path, artifact)?;
                let raw = std::fs::read_to_string(&file).map_err(|error| {
                    format!("read proposal verifier artifact {}: {error}", artifact.id)
                })?;
                parse_witness(&raw).map_err(|error| {
                    format!(
                        "proposal verifier artifact {} is not a frozen witness: {error}",
                        artifact.id
                    )
                })?;
                Ok(file)
            })
            .collect::<Result<Vec<_>, _>>();
    }

    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(Value::as_object)
        .ok_or_else(|| "pending proposal has no Receipt binding".to_string())?;
    let receipt_path = submission
        .get("receipt_path")
        .and_then(Value::as_str)
        .ok_or_else(|| "proposal Receipt path is unavailable".to_string())?;
    let expected_root = submission
        .get("receipt_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "proposal Receipt root is unavailable".to_string())?;
    let receipt_file = verified_frontier_relative_file(path, "proposal Receipt", receipt_path)?;
    let receipt_bytes = std::fs::read(&receipt_file)
        .map_err(|error| format!("read proposal Receipt {receipt_path}: {error}"))?;
    let receipt = vela_protocol::receipt_v1::ReceiptV1::parse(&receipt_bytes)
        .map_err(|error| format!("parse proposal Receipt: {error}"))?;
    let observed_root = receipt
        .canonical_root()
        .map_err(|error| format!("root proposal Receipt: {error}"))?;
    if observed_root != expected_root {
        return Err("proposal Receipt root does not match its retained bytes".to_string());
    }

    receipt
        .as_value()
        .get("artifacts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|artifact| {
            artifact
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.contains("witness"))
                || artifact
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| path.ends_with(".witness.json"))
        })
        .filter_map(|artifact| artifact.get("sha256").and_then(Value::as_str))
        .map(|digest| {
            use sha2::{Digest, Sha256};
            let relative = format!("records/artifacts/sha256/{digest}");
            let file = verified_frontier_relative_file(path, "proposal artifact", &relative)?;
            let bytes = std::fs::read(&file)
                .map_err(|error| format!("read proposal artifact {}: {error}", file.display()))?;
            let observed = hex::encode(Sha256::digest(&bytes));
            if observed != digest {
                return Err("proposal artifact digest does not match retained bytes".to_string());
            }
            Ok(file)
        })
        .collect::<Result<Vec<_>, String>>()
}

fn verified_frontier_relative_file(
    frontier: &Path,
    label: &str,
    locator: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(locator);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("{label} path must remain frontier-relative"));
    }
    let frontier_root = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve frontier root: {error}"))?;
    let file = frontier.join(relative);
    let metadata =
        std::fs::symlink_metadata(&file).map_err(|error| format!("inspect {label}: {error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    let resolved =
        std::fs::canonicalize(&file).map_err(|error| format!("resolve {label}: {error}"))?;
    if !resolved.starts_with(&frontier_root) {
        return Err(format!("{label} resolves outside the frontier"));
    }
    Ok(resolved)
}

pub(crate) fn cmd_reproduce(path: &Path, proposal_id: Option<&str>, json_output: bool) {
    crate::ui::set_mode("reproduce", json_output);
    if path.is_dir()
        && path.join("frontier.yaml").is_file()
        && !path.join(".vela/epoch.json").is_file()
    {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Domain,
            "this Vela release reproduces only current repository epochs",
            Some(
                "inspect a predecessor with its pinned historical Vela release; current repositories contain `.vela/epoch.json`",
            ),
        );
    }
    let mut scope = if path.is_file() {
        "standalone_artifact"
    } else {
        "accepted_frontier"
    };
    if !json_output {
        crate::ui::header("REPRODUCE", &path.display().to_string(), None);
    }
    let mut files = if let Some(proposal_id) = proposal_id {
        scope = "pending_proposal";
        proposal_reproduction_files(path, proposal_id).unwrap_or_else(|error| fail_return(&error))
    } else {
        collect_witness_files(path)
    };
    // Also re-verify content-addressed witness blobs landed as verifier-tagged
    // artifacts under `.vela/artifact-blobs/`, bound to their findings rather
    // than stored as loose files. Cover any blob not already present as a loose
    // witness (deduped by content hash).
    if proposal_id.is_none()
        && !path.join(".vela/epoch.json").is_file()
        && let Ok(source) = repo::detect(path)
        && let Ok(proj) = repo::load(&source)
    {
        use sha2::{Digest, Sha256};
        let loose: std::collections::HashSet<String> = files
            .iter()
            .filter_map(|f| std::fs::read(f).ok())
            .map(|b| format!("sha256:{}", hex::encode(Sha256::digest(&b))))
            .collect();
        for art in &proj.artifacts {
            if art.media_type.as_deref() != Some("application/json")
                || !art.metadata.contains_key("verifier")
                || loose.contains(&art.content_hash)
            {
                continue;
            }
            if let ("local_blob" | "local_file", Some(loc)) =
                (art.storage_mode.as_str(), &art.locator)
            {
                let blob = path.join(loc);
                if blob.exists() {
                    files.push(blob);
                }
            }
        }
    }
    if files.is_empty() {
        if let Some(proposal_id) = proposal_id {
            fail(&format!(
                "proposal {proposal_id} has no frontier-local frozen witness to reproduce; inspect its retained artifacts and verifier evidence, or use the producer's exact replay bundle"
            ));
        }
        fail(&format!(
            "no witnesses found at {} (expected a `*.witness.json` file, or a directory containing them / a `witnesses/` subdir)",
            path.display()
        ));
    }
    let spinner = (!json_output).then(|| {
        crate::cli::progress::Spinner::start(&format!(
            "re-verifying {} witness(es) with the frozen verifiers",
            files.len()
        ))
    });
    let mut results: Vec<Value> = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for file in &files {
        let result_path = reproduction_result_path(path, file, proposal_id.is_some());
        let raw = match std::fs::read_to_string(file) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {result_path}  ·  read error: {e}");
                }
                results.push(json!({"path": result_path, "ok": false, "message": format!("read error: {e}")}));
                continue;
            }
        };
        let witness = match parse_witness(&raw) {
            Ok(w) => w,
            Err(e) => {
                failed += 1;
                if !json_output {
                    println!("  FAIL  {result_path}  ·  parse error: {e}");
                }
                results.push(json!({"path": result_path, "ok": false, "message": format!("parse error: {e}")}));
                continue;
            }
        };
        let mut outcome = vela_verify::verify_witness(&witness);
        // Machine-checked novelty: a witness may declare `improves_on`
        // (a sibling witness path relative to its own directory). The
        // claim then verifies ONLY if it also strictly dominates the
        // referenced witness — dominance is arithmetic, not opinion.
        if outcome.ok
            && let Ok(value) = serde_json::from_str::<Value>(&raw)
            && let Some(prior_rel) = value.get("improves_on").and_then(Value::as_str)
        {
            let prior_path = file
                .parent()
                .map(|d| d.join(prior_rel))
                .unwrap_or_else(|| std::path::PathBuf::from(prior_rel));
            match std::fs::read_to_string(&prior_path)
                .map_err(|e| format!("improves_on read {}: {e}", prior_path.display()))
                .and_then(|p| parse_witness(&p))
                .and_then(|prior| vela_verify::dominates(&witness, &prior))
            {
                Ok(true) => {
                    outcome.message =
                        format!("{} · strictly improves on {prior_rel}", outcome.message);
                }
                Ok(false) => {
                    outcome = vela_verify::VerifyResult::fail(format!(
                        "claims improves_on {prior_rel} but does NOT strictly dominate it"
                    ));
                }
                Err(e) => {
                    outcome =
                        vela_verify::VerifyResult::fail(format!("improves_on check failed: {e}"));
                }
            }
        }
        if outcome.ok {
            passed += 1;
        } else {
            failed += 1;
        }
        if !json_output {
            let status = if outcome.ok { "ok  " } else { "FAIL" };
            println!(
                "  {status}  {} [{}]  ·  {}",
                result_path,
                witness.kind(),
                outcome.message
            );
        }
        results.push(json!({
            "path": result_path,
            "kind": witness.kind(),
            "ok": outcome.ok,
            "message": outcome.message,
        }));
    }
    if let Some(s) = spinner {
        s.finish(&format!("{passed} verified, {failed} failed"));
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "command": "reproduce",
                "scope": scope,
                "proposal_id": proposal_id,
                "authority_effect": "none",
                "witnesses": files.len(),
                "passed": passed,
                "failed": failed,
                "results": results,
            }))
            .expect("serialize reproduce response")
        );
    } else {
        println!();
        println!("  scope: {scope}");
        if let Some(proposal_id) = proposal_id {
            println!("  proposal: {proposal_id} (pending; acceptance unchanged)");
        }
        if failed == 0 {
            println!(
                "  reproduce: ok ({passed}/{}) — every witness re-verified from scratch by the frozen verifiers.",
                files.len()
            );
        } else {
            println!(
                "  reproduce: FAIL ({failed}/{} did not re-verify). Investigate before trusting.",
                files.len()
            );
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    // The exact-lane vouch gate. An adversarial review showed the prior vouch
    // (a "registered non-agent reviewer" signing a verifier_attachment.added
    // event, or accepting a verifier.attach proposal) is forgeable: actor
    // registration is open self-enrollment, so an agent mints a key, registers
    // `reviewer:x`, and honestly signs. The fix scopes the vouch to where
    // attachments are load-bearing (the non-floor lane), and admits the exact
    // lane on the un-forgeable FLOOR alone.

    #[test]
    fn proposal_reproduction_reads_only_rooted_frontier_files() {
        let frontier = tempfile::tempdir().unwrap();
        std::fs::create_dir(frontier.path().join("records")).unwrap();
        let bytes = br#"{"schema":"fixture"}"#;
        std::fs::write(frontier.path().join("records/witness.json"), bytes).unwrap();
        let root = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        let resolved = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("records/witness.json"),
            &root,
        )
        .unwrap();
        assert!(resolved.starts_with(std::fs::canonicalize(frontier.path()).unwrap()));
        assert_eq!(
            reproduction_result_path(frontier.path(), &resolved, true),
            "records/witness.json"
        );

        let traversal = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("../secret.json"),
            &root,
        )
        .unwrap_err();
        assert!(traversal.contains("frontier-relative"));

        let tampered = verified_frontier_file(
            frontier.path(),
            "va_fixture",
            "local_file",
            Some("records/witness.json"),
            &format!("sha256:{}", "0".repeat(64)),
        )
        .unwrap_err();
        assert!(tampered.contains("content root"));
    }
}
