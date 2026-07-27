//! Witness-file collection for `vela reproduce` and current Submission
//! proposal derivation.

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

/// Derive a pending finding proposal from a current Submission v1.
///
/// The proposal points to the Submission but deliberately does not point to
/// the Registration Record: that record is created after the proposal and
/// records the outcome of the intake transaction, so reversing the edge would
/// create a content-addressing cycle.
pub(crate) fn proposal_for_submission(
    frontier: &vela_protocol::project::Project,
    submission: &vela_protocol::submission_v1::SubmissionV1,
    submission_root: &str,
    submission_path: &str,
    operation_id: &str,
    at: &str,
) -> Result<vela_protocol::proposals::StateProposal, String> {
    submission.verify()?;
    let requested_target = match submission.requested_change.target.as_ref() {
        Some(target) => {
            let finding = frontier
                .findings
                .iter()
                .find(|finding| finding.id == target.claim_id)
                .ok_or_else(|| {
                    format!(
                        "Submission requested_change targets missing Claim {}",
                        target.claim_id
                    )
                })?;
            let observed_root = vela_protocol::events::finding_hash(finding);
            if observed_root != target.claim_root {
                return Err(format!(
                    "Submission requested_change target root mismatch for {}: declared {}, observed {observed_root}",
                    target.claim_id, target.claim_root
                ));
            }
            Some(finding)
        }
        None => None,
    };
    let evidence_spans = submission
        .artifacts
        .iter()
        .map(|artifact| {
            serde_json::json!({
                "source": submission_path,
                "artifact_path": artifact.path,
                "artifact_kind": artifact.kind,
                "artifact_sha256": artifact.digest,
                "start": 0,
                "end": 0,
            })
        })
        .collect::<Vec<_>>();
    let conditions_text = if submission.claim.conditions.is_empty() {
        None
    } else {
        Some(submission.claim.conditions.join("\n"))
    };
    let mut source_refs = vec![submission_path.to_string()];
    source_refs.extend(
        submission
            .artifacts
            .iter()
            .map(|artifact| artifact.path.clone()),
    );
    source_refs.sort();
    source_refs.dedup();
    let mut proposal = vela_protocol::state::build_add_finding_proposal_at(
        vela_protocol::state::FindingDraftOptions {
            text: submission.claim.assertion.clone(),
            assertion_type: submission.claim.claim_type.clone(),
            source: format!("Submission {}", submission.submission_id),
            source_type: "vela_submission".to_string(),
            author: submission.provenance.producer.clone(),
            confidence: 0.5,
            evidence_type: submission.claim.claim_type.clone(),
            doi: None,
            year: None,
            url: None,
            source_authors: Vec::new(),
            source_refs,
            conditions_text,
            evidence_spans,
            gap: false,
            negative_space: submission.claim.claim_type == "negative",
            replication_attestation: None,
        },
        at,
    )?;
    proposal.reason = "Register an authenticated producer Submission for review".to_string();
    proposal.caveats = submission.caveats.clone();
    proposal.caveats.push(
        "Registration records producer input; it does not verify or accept the claim.".to_string(),
    );
    proposal.caveats.sort();
    proposal.caveats.dedup();
    let submission_link = serde_json::json!({
        "schema": "vela.submission-proposal-links.internal.v1",
        "submission_id": submission.submission_id,
        "submission_root": submission_root,
        "submission_path": submission_path,
        "operation_id": operation_id,
    });
    proposal.payload["submission"] = submission_link.clone();
    match submission.requested_change.kind.as_str() {
        "add_claim" => {}
        "correct_claim" | "supersede_claim" => {
            let target = requested_target.expect("validated corrective Submission target");
            let new_finding = proposal
                .payload
                .get("finding")
                .cloned()
                .ok_or("corrective Submission failed to derive its replacement Claim")?;
            proposal.kind = "finding.supersede".to_string();
            proposal.target = vela_protocol::events::StateTarget {
                r#type: "finding".to_string(),
                id: target.id.clone(),
            };
            proposal.reason = if submission.requested_change.kind == "correct_claim" {
                "Correct an exact historical Claim through a new authenticated Submission"
                    .to_string()
            } else {
                "Supersede an exact historical Claim through a new authenticated Submission"
                    .to_string()
            };
            proposal.payload = serde_json::json!({
                "new_finding": new_finding,
                "submission": submission_link,
            });
        }
        "retract_claim" => {
            let target = requested_target.expect("validated retraction Submission target");
            proposal.kind = "finding.retract".to_string();
            proposal.target = vela_protocol::events::StateTarget {
                r#type: "finding".to_string(),
                id: target.id.clone(),
            };
            proposal.reason =
                "Retract an exact historical Claim through a new authenticated Submission"
                    .to_string();
            proposal.payload = serde_json::json!({
                "submission": submission_link,
            });
        }
        other => {
            return Err(format!(
                "unsupported Submission requested_change kind {other}"
            ));
        }
    }
    proposal.id = vela_protocol::proposals::proposal_id(&proposal);
    Ok(proposal)
}

#[cfg(test)]
mod current_submission_tests {
    use ed25519_dalek::SigningKey;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::submission_v1::{
        RequestedChange, RequestedChangeTarget, SubmissionArtifact, SubmissionClaim,
        SubmissionDraft, SubmissionProvenance, SubmissionV1,
    };

    use super::*;

    #[test]
    fn current_submission_proposal_is_pending_and_receipt_free() {
        let key = SigningKey::from_bytes(&[91_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:submission-fixture".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-26T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "A bounded witness exists.".to_string(),
                    claim_type: "computational".to_string(),
                    conditions: vec!["fixture domain".to_string()],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".to_string(),
                    path: "witness.json".to_string(),
                    digest: format!("sha256:{}", "a".repeat(64)),
                }],
                caveats: vec!["Fixture only.".to_string()],
                replayability: "exact".to_string(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["independent replay".to_string()],
                requested_change: RequestedChange {
                    kind: "add_claim".to_string(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:submission-fixture".to_string(),
                    source_system: "fixture".to_string(),
                    source_attempt: Some(format!("vat_{}", "3".repeat(64))),
                    source_run: Some("vws_fixture".to_string()),
                    emitted_at: "2026-07-26T00:00:00Z".to_string(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap();
        let root = submission.canonical_root().unwrap();
        let path = format!(
            "records/submissions/sha256/{}.json",
            root.strip_prefix("sha256:").unwrap()
        );
        let project =
            vela_protocol::project::assemble("submission-fixture", Vec::new(), 0, 0, "test");
        let proposal = proposal_for_submission(
            &project,
            &submission,
            &root,
            &path,
            &format!("vop_{}", "b".repeat(64)),
            "2026-07-26T00:00:01Z",
        )
        .unwrap();
        assert_eq!(proposal.status, "pending_review");
        assert!(proposal.payload.get("submission").is_some());
        assert!(proposal.payload.get("vela_submission").is_none());
        let mut project = project;
        let event_count = project.events.len();
        vela_protocol::proposals::insert_pending_in_frontier(&mut project, proposal).unwrap();
        assert_eq!(project.events.len(), event_count);
        assert_eq!(project.proposals.len(), 1);
        assert_eq!(
            project.proposals[0].payload["submission"]["submission_root"],
            root
        );
    }

    #[test]
    fn corrective_submission_binds_the_exact_historical_claim() {
        let original_proposal = vela_protocol::state::build_add_finding_proposal_at(
            vela_protocol::state::FindingDraftOptions {
                text: "The original bounded assertion.".to_string(),
                assertion_type: "computational".to_string(),
                source: "Original fixture".to_string(),
                source_type: "fixture".to_string(),
                author: "agent:original-fixture".to_string(),
                confidence: 0.5,
                evidence_type: "computational".to_string(),
                doi: None,
                year: None,
                url: None,
                source_authors: Vec::new(),
                source_refs: vec!["original.json".to_string()],
                conditions_text: Some("Only the original fixture domain.".to_string()),
                evidence_spans: Vec::new(),
                gap: false,
                negative_space: false,
                replication_attestation: None,
            },
            "2026-07-26T00:00:00Z",
        )
        .unwrap();
        let original: vela_protocol::bundle::FindingBundle =
            serde_json::from_value(original_proposal.payload["finding"].clone()).unwrap();
        let original_root = vela_protocol::events::finding_hash(&original);
        let project = vela_protocol::project::assemble(
            "correction-fixture",
            vec![original.clone()],
            0,
            0,
            "test",
        );
        let key = SigningKey::from_bytes(&[92_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:correction-fixture".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-26T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let build = |claim_root: String| {
            SubmissionV1::build(
                SubmissionDraft {
                    claim: SubmissionClaim {
                        assertion: "The corrected bounded assertion.".to_string(),
                        claim_type: "computational".to_string(),
                        conditions: vec!["Only the corrected fixture domain.".to_string()],
                    },
                    artifacts: vec![SubmissionArtifact {
                        kind: "witness".to_string(),
                        path: "corrected.json".to_string(),
                        digest: format!("sha256:{}", "c".repeat(64)),
                    }],
                    caveats: vec!["Fixture only.".to_string()],
                    replayability: "exact".to_string(),
                    producer_checks: Vec::new(),
                    verification_requirements: vec!["independent replay".to_string()],
                    requested_change: RequestedChange {
                        kind: "correct_claim".to_string(),
                        target: Some(RequestedChangeTarget {
                            claim_id: original.id.clone(),
                            claim_root,
                        }),
                    },
                    provenance: SubmissionProvenance {
                        producer: "agent:correction-fixture".to_string(),
                        source_system: "fixture".to_string(),
                        source_attempt: None,
                        source_run: Some("correction-run".to_string()),
                        emitted_at: "2026-07-26T00:00:00Z".to_string(),
                    },
                    execution_binding: None,
                },
                identity.clone(),
                &key,
            )
            .unwrap()
        };
        let submission = build(original_root);
        let root = submission.canonical_root().unwrap();
        let path = format!(
            "records/submissions/sha256/{}.json",
            root.strip_prefix("sha256:").unwrap()
        );
        let proposal = proposal_for_submission(
            &project,
            &submission,
            &root,
            &path,
            &format!("vop_{}", "d".repeat(64)),
            "2026-07-26T00:00:01Z",
        )
        .unwrap();
        assert_eq!(proposal.kind, "finding.supersede");
        assert_eq!(proposal.target.id, original.id);
        assert_eq!(
            proposal.payload["submission"]["submission_root"],
            submission.canonical_root().unwrap()
        );
        assert_eq!(
            proposal.payload["new_finding"]["assertion"]["text"],
            "The corrected bounded assertion."
        );

        let wrong_root = build(format!("sha256:{}", "f".repeat(64)));
        let error = proposal_for_submission(
            &project,
            &wrong_root,
            &wrong_root.canonical_root().unwrap(),
            "records/submissions/sha256/wrong.json",
            &format!("vop_{}", "e".repeat(64)),
            "2026-07-26T00:00:01Z",
        )
        .unwrap_err();
        assert!(error.contains("target root mismatch"));
    }
}
