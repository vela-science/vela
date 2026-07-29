use crate::cli::{collect_witness_files, fail, fail_return, parse_witness, print_json};
use crate::cli_commands::*;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::submission_v1::SubmissionV1;

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

fn verified_frontier_file(
    frontier: &Path,
    label: &str,
    locator: &str,
    expected_root: &str,
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
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&resolved).map_err(|error| format!("read {label}: {error}"))?;
    let observed = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if observed != expected_root {
        return Err(format!(
            "{label} content root does not match retained bytes"
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
    let repository = crate::current_repository::load_compacted_repository_at(path, true)?;
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    let proposal_file = verified_frontier_file(
        path,
        "current Proposal",
        &proposal_reference.path,
        &proposal_reference.root,
    )?;
    let proposal = ProposalV1::parse(
        &std::fs::read(&proposal_file)
            .map_err(|error| format!("read current Proposal: {error}"))?,
    )?;
    if proposal.proposal_id != proposal_reference.id {
        return Err(format!(
            "current Proposal {} does not match its repository reference",
            proposal_reference.id
        ));
    }
    let decisions = crate::current_repository::load_current_proposal_decisions(path, &repository)?;
    let standing = decisions
        .get(proposal_id)
        .map(|decision| decision.standing.as_str())
        .unwrap_or("pending_review");
    if standing != "pending_review" {
        return Err(format!(
            "proposal {proposal_id} is {standing}, not pending_review"
        ));
    }
    let submission_reference = repository
        .submissions
        .iter()
        .find(|reference| {
            reference.id == proposal.producer_package.id
                && reference.root == proposal.producer_package.root
                && reference.path == proposal.producer_package.path
        })
        .ok_or_else(|| {
            format!("proposal {proposal_id} does not bind one exact current Submission")
        })?;
    let submission_file = verified_frontier_file(
        path,
        "current Submission",
        &submission_reference.path,
        &submission_reference.root,
    )?;
    let submission = SubmissionV1::parse(
        &std::fs::read(&submission_file)
            .map_err(|error| format!("read current Submission: {error}"))?,
    )?;
    if submission.submission_id != submission_reference.id {
        return Err(format!(
            "current Submission {} does not match its repository reference",
            submission_reference.id
        ));
    }

    submission
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind.contains("witness") || artifact.path.ends_with(".witness.json")
        })
        .map(|artifact| {
            let digest = artifact.digest.strip_prefix("sha256:").ok_or_else(|| {
                "current Submission artifact digest is not a full sha256 identity".to_string()
            })?;
            let reference = repository
                .artifacts
                .iter()
                .find(|reference| {
                    reference.id == digest
                        && reference.root == artifact.digest
                        && reference.path == artifact.path
                        && reference.schema == "content-addressed-artifact"
                })
                .ok_or_else(|| {
                    format!(
                        "current Submission artifact {} is not retained at its exact repository reference",
                        artifact.digest
                    )
                })?;
            let file = verified_frontier_file(
                path,
                "current proposal witness",
                &reference.path,
                &reference.root,
            )?;
            let raw = std::fs::read_to_string(&file)
                .map_err(|error| format!("read current proposal witness: {error}"))?;
            parse_witness(&raw)
                .map_err(|error| format!("current proposal artifact is not a frozen witness: {error}"))?;
            Ok(file)
        })
        .collect::<Result<Vec<_>, String>>()
}

pub(crate) fn cmd_reproduce(path: &Path, proposal_id: Option<&str>, json_output: bool) {
    crate::ui::set_mode("reproduce", json_output);
    if path.is_dir()
        && path.join("frontier.yaml").is_file()
        && !path.join(".vela/origin.json").is_file()
    {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Domain,
            "this Vela release reproduces only current repository origins",
            Some(
                "inspect a predecessor with its pinned historical Vela release; current repositories contain `.vela/origin.json`",
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
    let files = if let Some(proposal_id) = proposal_id {
        scope = "pending_proposal";
        proposal_reproduction_files(path, proposal_id).unwrap_or_else(|error| fail_return(&error))
    } else {
        collect_witness_files(path)
    };
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
            "fixture witness",
            "records/witness.json",
            &root,
        )
        .unwrap();
        assert!(resolved.starts_with(std::fs::canonicalize(frontier.path()).unwrap()));
        assert_eq!(
            reproduction_result_path(frontier.path(), &resolved, true),
            "records/witness.json"
        );

        let traversal =
            verified_frontier_file(frontier.path(), "fixture witness", "../secret.json", &root)
                .unwrap_err();
        assert!(traversal.contains("frontier-relative"));

        let tampered = verified_frontier_file(
            frontier.path(),
            "fixture witness",
            "records/witness.json",
            &format!("sha256:{}", "0".repeat(64)),
        )
        .unwrap_err();
        assert!(tampered.contains("content root"));
    }
}
