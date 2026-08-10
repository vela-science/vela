//! Handlers shared by the direct CLI dispatch.

use crate::cli::{
    collect_witness_files, fail, fail_kind_return, fail_return, parse_witness, print_json,
};
use crate::command_spec::*;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use vela_protocol::proposal::ProposalV1;
use vela_protocol::submission::SubmissionRecordV2;

const REPLAY_CAPSULE_MAX_BYTES: u64 = 1024 * 1024;
const WITNESS_MAX_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn cmd_verify_evidence(action: VerifyAction) {
    match action {
        VerifyAction::Record {
            first,
            second,
            repo_flag,
            profile,
            method,
            property,
            complementary,
            outcome,
            does_not_establish,
            independent_of,
            shared_dependency,
            actor,
            json,
        } => {
            crate::ui::set_mode("verification.record", json);
            let (repository, proposal) = crate::cli::repo_arg::bind_repo_and_object(
                "verification record",
                "a Proposal id (vpr_...)",
                "PROPOSAL",
                first,
                second,
                repo_flag,
            );
            crate::ui::require_initialized_repo(&repository);
            let record = crate::verification::author_record(
                &repository,
                crate::verification::VerificationRecordRequest {
                    proposal_id: proposal,
                    profile,
                    method_path: method.clone(),
                    property,
                    complementary,
                    outcome,
                    does_not_establish,
                    independent_of,
                    shared_dependencies: shared_dependency,
                    actor: actor.clone(),
                },
            )
            .unwrap_or_else(|error| {
                if matches!(
                    error.as_str(),
                    "Verification method manifest must be retained in the current Git commit"
                        | "Verification method manifest differs from the retained current Git bytes"
                ) {
                    let hint = format!(
                        "Commit the exact method manifest {} at the current repository HEAD, then rerun the same vela verification record command",
                        method.display()
                    );
                    crate::ui::fail_with(crate::ui::ErrorKind::Domain, &error, Some(&hint));
                }
                fail_return(&error)
            });
            let result =
                crate::verification::import(&repository, &record, &actor).unwrap_or_else(|error| {
                    crate::ui::fail_if_recovery_required(&repository);
                    fail_return(&error)
                });
            print_verification_result(&result, "verification record", json);
        }
        VerifyAction::Import {
            first,
            second,
            repo_flag,
            actor,
            json,
        } => {
            crate::ui::set_mode("verification.import", json);
            let (repository, record) = crate::cli::repo_arg::bind_repo_and_object(
                "verification import",
                "a signed Verification Record file",
                "RECORD",
                first,
                second,
                repo_flag,
            );
            let record = std::path::PathBuf::from(record);
            crate::ui::require_initialized_repo(&repository);
            let bytes = crate::bounded_file::read_bounded_file(
                &record,
                vela_protocol::verification_record::VERIFICATION_RECORD_MAX_BYTES as u64,
                "Verification Record v2 envelope",
            )
            .unwrap_or_else(|error| fail_return(&error.to_string()));
            let record =
                vela_protocol::verification_record::VerificationRecordEnvelopeV2::parse(&bytes)
                    .unwrap_or_else(|error| {
                        fail_return(&format!(
                            "parse {} as Verification Record v2 envelope: {error}",
                            record.display()
                        ))
                    });
            let result =
                crate::verification::import(&repository, &record, &actor).unwrap_or_else(|error| {
                    crate::ui::fail_if_recovery_required(&repository);
                    fail_return(&error)
                });
            print_verification_result(&result, "verification import", json);
        }
    }
}

fn print_verification_result(
    result: &crate::repository_ops::VerificationImportOutcome,
    command: &str,
    json_output: bool,
) {
    if json_output {
        print_json(result);
    } else {
        println!(
            "{command}: retained {} for proposal {}",
            result.verification_record_id, result.proposal_id
        );
        println!("  acceptance: unchanged (delta 0)");
        println!("  outcome: {}", result.outcome);
    }
}

fn verified_repository_file(
    repository: &Path,
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
        return Err(format!("{label} path must remain repository-relative"));
    }
    let repository_root = std::fs::canonicalize(repository)
        .map_err(|error| format!("resolve repository root: {error}"))?;
    let file = repository.join(relative);
    let metadata =
        std::fs::symlink_metadata(&file).map_err(|error| format!("inspect {label}: {error}"))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    let resolved =
        std::fs::canonicalize(&file).map_err(|error| format!("resolve {label}: {error}"))?;
    if !resolved.starts_with(&repository_root) {
        return Err(format!("{label} resolves outside the repository"));
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

fn reproduction_result_path(repository: &Path, file: &Path, proposal_scoped: bool) -> String {
    if !proposal_scoped {
        return file.display().to_string();
    }
    let root = std::fs::canonicalize(repository).unwrap_or_else(|_| repository.to_path_buf());
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeReplayHint {
    command: String,
    source: Option<String>,
}

fn proposal_native_replay_hint(
    repository: &Path,
    proposal_id: &str,
    proposal_path: &str,
    proposal_root: &str,
) -> Result<Option<NativeReplayHint>, String> {
    let reproductions = repository.join("reproductions");
    if !reproductions.is_dir() {
        return Ok(None);
    }

    let mut capsule_paths = std::fs::read_dir(&reproductions)
        .map_err(|error| format!("inspect source-local reproductions: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("capsule.json"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    capsule_paths.sort();

    let mut matches = Vec::new();
    for capsule_path in capsule_paths {
        let metadata = std::fs::symlink_metadata(&capsule_path)
            .map_err(|error| format!("inspect source-local replay capsule: {error}"))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let bytes = crate::bounded_file::read_bounded_file(
            &capsule_path,
            REPLAY_CAPSULE_MAX_BYTES,
            "source-local replay capsule",
        )
        .map_err(|error| error.to_string())?;
        let capsule: Value = vela_protocol::canonical::parse_json_value_strict(&bytes)
            .map_err(|error| format!("parse source-local replay capsule: {error}"))?;
        if capsule
            .pointer("/identity/proposal_id")
            .and_then(Value::as_str)
            != Some(proposal_id)
        {
            continue;
        }
        if capsule.get("authority").and_then(Value::as_str) != Some("evidence_only")
            || capsule.get("standing_effect").and_then(Value::as_str) != Some("none")
        {
            return Err(format!(
                "source-local replay capsule for proposal {proposal_id} must be evidence-only with no Standing effect"
            ));
        }

        let retained_proposal_path = capsule
            .pointer("/inputs/proposal/path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "source-local replay capsule for proposal {proposal_id} has no Proposal path"
                )
            })?;
        let retained_proposal_root = capsule
            .pointer("/inputs/proposal/sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "source-local replay capsule for proposal {proposal_id} has no Proposal root"
                )
            })?;
        if retained_proposal_path != proposal_path || retained_proposal_root != proposal_root {
            return Err(format!(
                "source-local replay capsule for proposal {proposal_id} does not bind the exact current Proposal"
            ));
        }
        verified_repository_file(
            repository,
            "source-local replay Proposal",
            retained_proposal_path,
            retained_proposal_root,
        )?;

        let implementation_path = capsule
            .pointer("/inputs/implementation/path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("source-local replay capsule for proposal {proposal_id} has no implementation path")
            })?;
        let implementation_root = capsule
            .pointer("/inputs/implementation/sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("source-local replay capsule for proposal {proposal_id} has no implementation root")
            })?;
        let implementation = verified_repository_file(
            repository,
            "source-local replay implementation",
            implementation_path,
            implementation_root,
        )?;
        if implementation
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("py")
        {
            return Err(format!(
                "source-local replay implementation for proposal {proposal_id} is not a directly inspectable Python program"
            ));
        }
        let implementation_relative = reproduction_result_path(repository, &implementation, true);
        let source = capsule
            .pointer("/source/repository")
            .and_then(Value::as_str)
            .zip(
                capsule
                    .pointer("/source/git_commit")
                    .and_then(Value::as_str),
            )
            .map(|(repository, commit)| format!("{repository}@{commit}"));
        matches.push(NativeReplayHint {
            command: format!("python3 {implementation_relative} --validate-only"),
            source,
        });
    }

    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(format!(
            "proposal {proposal_id} has more than one source-local replay capsule"
        )),
    }
}

pub(crate) fn proposal_reproduction_files(
    path: &Path,
    proposal_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let repository = crate::repository::load_repository_at(path, true)?;
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        /* This lookup, not the later one in `cmd_reproduce`, is what an unknown
        `--proposal` actually reaches; every other failure below is a broken or
        unverifiable Proposal, which stays a domain failure. */
        .unwrap_or_else(|| {
            fail_kind_return(
                crate::ui::ErrorKind::NotFound,
                &format!("proposal {proposal_id} does not exist"),
            )
        });
    let proposal_file = verified_repository_file(
        path,
        "current Proposal",
        &proposal_reference.path,
        &proposal_reference.root,
    )?;
    let proposal = ProposalV1::parse(
        &std::fs::read(&proposal_file)
            .map_err(|error| format!("read current Proposal: {error}"))?,
    )?;
    if proposal.id() != proposal_reference.id {
        return Err(format!(
            "current Proposal {} does not match its repository reference",
            proposal_reference.id
        ));
    }
    let standings = crate::repository::load_current_proposal_standings(path, &repository)?;
    let standing = standings
        .get(proposal_id)
        .map(String::as_str)
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
    let submission_file = verified_repository_file(
        path,
        "current Submission",
        &submission_reference.path,
        &submission_reference.root,
    )?;
    let submission = SubmissionRecordV2::parse(
        &std::fs::read(&submission_file)
            .map_err(|error| format!("read current Submission: {error}"))?,
    )?;
    if submission.id != submission_reference.id {
        return Err(format!(
            "current Submission {} does not match its repository reference",
            submission_reference.id
        ));
    }

    submission
        .submission
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind.contains("witness") || artifact.path.ends_with(".witness.json")
        })
        .map(|artifact| {
            let digest = artifact.digest.strip_prefix("sha256:").ok_or_else(|| {
                "current Submission artifact digest is not a full sha256 identity".to_string()
            })?;
            let retained_path = format!("records/artifacts/sha256/{digest}");
            let reference = repository
                .artifacts
                .iter()
                .find(|reference| {
                    reference.id == digest
                        && reference.root == artifact.digest
                        && reference.path == retained_path
                        && reference.schema == "content-addressed-artifact"
                })
                .ok_or_else(|| {
                    format!(
                        "current Submission artifact {} is not retained at its exact repository reference",
                        artifact.digest
                    )
                })?;
            let file = verified_repository_file(
                path,
                "current proposal witness",
                &reference.path,
                &reference.root,
            )?;
            let raw = crate::bounded_file::read_bounded_file(
                &file,
                WITNESS_MAX_BYTES,
                "current proposal witness",
            )
            .map_err(|error| error.to_string())?;
            parse_witness(&raw)
                .map_err(|error| format!("current proposal artifact is not a frozen witness: {error}"))?;
            Ok(file)
        })
        .collect::<Result<Vec<_>, String>>()
}

pub(crate) fn cmd_reproduce(path: &Path, proposal_id: Option<&str>, json_output: bool) {
    crate::ui::set_mode("reproduce", json_output);
    if path.is_dir() && path.join("vela.toml").is_file() {
        crate::ui::require_initialized_repo(path);
    }
    /* The scope names what the witnesses being re-run belong to. v1 spelled
    the repository case `accepted_frontier`, which named the one thing here
    that holds nothing: a Frontier is a derived query, and what carries
    accepted Standing is the Repository. The token is printed verbatim on the
    human surface as well as emitted, so the two moved together, under v2. */
    let mut scope = if path.is_file() {
        "standalone_artifact"
    } else {
        "accepted_repository"
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
            let repository = crate::repository::load_repository_at(path, true)
                .unwrap_or_else(|error| fail_return(&error));
            let proposal = repository
                .proposals
                .iter()
                .find(|reference| reference.id == proposal_id)
                .unwrap_or_else(|| {
                    fail_kind_return(
                        crate::ui::ErrorKind::NotFound,
                        &format!("proposal {proposal_id} does not exist"),
                    )
                });
            match proposal_native_replay_hint(path, proposal_id, &proposal.path, &proposal.root)
                .unwrap_or_else(|error| fail_return(&error))
            {
                Some(hint) => {
                    let message = match hint.source {
                        Some(source) => format!(
                            "proposal {proposal_id} uses a source-local native replay rather than a Vela witness; the capsule binds the exact current Proposal and implementation bytes (full replay source {source})"
                        ),
                        None => format!(
                            "proposal {proposal_id} uses a source-local native replay rather than a Vela witness; the capsule binds the exact current Proposal and implementation bytes"
                        ),
                    };
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Domain,
                        &message,
                        Some(&hint.command),
                    );
                }
                None => fail(&format!(
                    "proposal {proposal_id} has no repository-local frozen witness or rooted source-local replay to reproduce; inspect its retained artifacts and verifier evidence"
                )),
            }
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
        let raw = match crate::bounded_file::read_bounded_file(file, WITNESS_MAX_BYTES, "witness") {
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
        let outcome = witness.verify();
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
        s.finish(&format!("{passed} re-ran and matched, {failed} did not"));
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                /* ui.rs states the contract: under --json every outcome is one
                   object carrying {ok, command, ...}. This payload named the
                   command and nothing else, so an agent could not test one
                   field to know whether the run succeeded. */
                "ok": failed == 0,
                "command": "reproduce",
                "schema": "vela.reproduction-summary.v2",
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
                "  reproduce: ok ({passed}/{}) — every witness re-ran from scratch under the frozen verifiers and matched.",
                files.len()
            );
        } else {
            println!(
                "  reproduce: FAIL ({failed}/{} did not match on re-run). Investigate before trusting.",
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
    fn proposal_reproduction_reads_only_rooted_repository_files() {
        let repository = tempfile::tempdir().unwrap();
        std::fs::create_dir(repository.path().join("records")).unwrap();
        let bytes = br#"{"schema":"fixture"}"#;
        std::fs::write(repository.path().join("records/witness.json"), bytes).unwrap();
        let root = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        let resolved = verified_repository_file(
            repository.path(),
            "fixture witness",
            "records/witness.json",
            &root,
        )
        .unwrap();
        assert!(resolved.starts_with(std::fs::canonicalize(repository.path()).unwrap()));
        assert_eq!(
            reproduction_result_path(repository.path(), &resolved, true),
            "records/witness.json"
        );

        let traversal = verified_repository_file(
            repository.path(),
            "fixture witness",
            "../secret.json",
            &root,
        )
        .unwrap_err();
        assert!(traversal.contains("repository-relative"));

        let tampered = verified_repository_file(
            repository.path(),
            "fixture witness",
            "records/witness.json",
            &format!("sha256:{}", "0".repeat(64)),
        )
        .unwrap_err();
        assert!(tampered.contains("content root"));
    }

    #[test]
    fn proposal_native_replay_hint_binds_current_proposal_and_implementation() {
        let repository = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repository.path().join("records/proposals/sha256")).unwrap();
        std::fs::create_dir_all(repository.path().join("reproductions/example")).unwrap();

        let proposal_bytes = br#"{"schema":"fixture-proposal"}"#;
        let proposal_path = "records/proposals/sha256/proposal.json";
        std::fs::write(repository.path().join(proposal_path), proposal_bytes).unwrap();
        let proposal_root = format!("sha256:{}", hex::encode(Sha256::digest(proposal_bytes)));

        let implementation_bytes = b"#!/usr/bin/env python3\n";
        let implementation_path = "reproductions/example/replay.py";
        std::fs::write(
            repository.path().join(implementation_path),
            implementation_bytes,
        )
        .unwrap();
        let implementation_root = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(implementation_bytes))
        );

        std::fs::write(
            repository.path().join("reproductions/example/capsule.json"),
            serde_json::to_vec_pretty(&json!({
                "authority": "evidence_only",
                "standing_effect": "none",
                "identity": {"proposal_id": "vpr_fixture"},
                "inputs": {
                    "proposal": {
                        "path": proposal_path,
                        "sha256": proposal_root,
                    },
                    "implementation": {
                        "path": implementation_path,
                        "sha256": implementation_root,
                    }
                },
                "source": {
                    "repository": "https://example.invalid/source.git",
                    "git_commit": "0123456789abcdef",
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let hint = proposal_native_replay_hint(
            repository.path(),
            "vpr_fixture",
            proposal_path,
            &proposal_root,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            hint.command,
            "python3 reproductions/example/replay.py --validate-only"
        );
        assert_eq!(
            hint.source.as_deref(),
            Some("https://example.invalid/source.git@0123456789abcdef")
        );

        let mismatch = proposal_native_replay_hint(
            repository.path(),
            "vpr_fixture",
            proposal_path,
            &format!("sha256:{}", "0".repeat(64)),
        )
        .unwrap_err();
        assert!(mismatch.contains("exact current Proposal"));
    }
}
