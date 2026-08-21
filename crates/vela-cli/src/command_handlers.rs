//! Handlers shared by the direct CLI dispatch.

use crate::cli::{fail_return, print_json};
use crate::command_spec::*;

pub(crate) fn cmd_verify_evidence(action: VerifyAction) {
    match action {
        VerifyAction::Check {
            method,
            profile,
            property,
            actor,
            does_not_establish,
            json,
        } => {
            crate::ui::set_mode("verification.check", json);
            let request = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
                "method": method.display().to_string(),
                "profile": profile,
                "property": property,
                "actor": actor,
                "does_not_establish": does_not_establish,
            }))
            .unwrap_or_default();
            let operation_id =
                vela_repository::OperationId::derive("review-method-check", &request);
            let checked = crate::verification::check_review_method(
                &method,
                &profile,
                &property,
                &actor,
                does_not_establish,
            )
            .unwrap_or_else(|error| {
                crate::ui::fail_unchanged_coded(
                    crate::ui::ErrorKind::Domain,
                    Some(error.code()),
                    error.message(),
                    operation_id.as_str(),
                    "correct the canonical Review Method or its intended --profile, --property, --as, and --does-not-establish bindings, then rerun `vela verification check`",
                )
            });
            let result = serde_json::json!({
                "schema": "vela.review-method-validation.v1",
                "ok": true,
                "command": "verification.check",
                "changed": false,
                "authority_effect": "none",
                "standing_effect": "none",
                "method_path": method.display().to_string(),
                "method_root": checked.root,
                "method_bytes": checked.bytes,
                "review_method": checked.method,
                "bindings": {
                    "matched": true,
                    "profile": profile,
                    "property": property,
                    "actor": actor,
                },
            });
            if json {
                print_json(&result);
            } else {
                println!("verification check: canonical Review Method is valid");
                println!(
                    "  root      {}",
                    result["method_root"].as_str().unwrap_or("")
                );
                println!("  bindings  matched");
                println!("  changed   false");
            }
        }
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
            output,
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
                    output_paths: output.clone(),
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
                crate::verification::import_with_outputs(&repository, &record, &actor, &output)
                    .unwrap_or_else(|error| {
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
