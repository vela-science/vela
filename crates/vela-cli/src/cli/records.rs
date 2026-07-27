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
    submission: &vela_protocol::submission_v1::SubmissionV1,
    submission_root: &str,
    submission_path: &str,
    operation_id: &str,
    at: &str,
) -> Result<vela_protocol::proposals::StateProposal, String> {
    submission.verify()?;
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
    proposal.payload["submission"] = serde_json::json!({
        "schema": "vela.submission-proposal-links.internal.v1",
        "submission_id": submission.submission_id,
        "submission_root": submission_root,
        "submission_path": submission_path,
        "operation_id": operation_id,
    });
    proposal.id = vela_protocol::proposals::proposal_id(&proposal);
    Ok(proposal)
}

#[cfg(test)]
mod current_submission_tests {
    use ed25519_dalek::SigningKey;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft,
        SubmissionProvenance, SubmissionV1,
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
        let proposal = proposal_for_submission(
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
        let mut project =
            vela_protocol::project::assemble("submission-fixture", Vec::new(), 0, 0, "test");
        let event_count = project.events.len();
        vela_protocol::proposals::insert_pending_in_frontier(&mut project, proposal).unwrap();
        assert_eq!(project.events.len(), event_count);
        assert_eq!(project.proposals.len(), 1);
        assert_eq!(
            project.proposals[0].payload["submission"]["submission_root"],
            root
        );
    }
}
