//! Crash-safe writes for authenticated routine evidence.
//!
//! Submissions and Verification Records authenticate their own producers.
//! Retaining those objects must not mint a repository-authority record or
//! touch the repository-authority SSH key. This module deliberately reuses
//! the existing Frontier transaction journal and publication delta while
//! leaving scientific Events and authority history unchanged.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::authority_transaction::{AuthorityDerivedDraft, AuthorityObjectDraft};
use crate::frontier_txn::{
    CanonicalWriteBarrier, ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn,
    FrontierTxnError, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding, OperationId,
    OperationKind, PlannedWrite, RepoPath, WriteClass,
};

const LAYOUT_SCHEMA: &str = "vela.routine-evidence-layout.internal.v1";
const RESULT_SCHEMA: &str = "vela.routine-evidence-transaction-result.internal.v1";

#[derive(Serialize)]
struct LayoutCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    object_paths: Vec<&'a str>,
    derived_paths: Vec<&'a str>,
}

#[derive(Serialize)]
struct DurableResult<'a> {
    schema: &'static str,
    operation_id: &'a str,
    accepted_event_delta: u8,
}

/// Prepared routine evidence write with the same durability/publication
/// surface as an authority transaction, but no authority signer or envelope.
pub(crate) struct PreparedRoutineEvidenceTransaction {
    transaction: FrontierTxn,
}

impl PreparedRoutineEvidenceTransaction {
    pub(crate) fn resolved_public_writes(
        &self,
    ) -> Result<Vec<crate::frontier_txn::ResolvedWrite>, FrontierTxnError> {
        self.transaction.resolved_public_writes()
    }

    pub(crate) fn canonical_delta_root(&self) -> &str {
        self.transaction.canonical_delta_root()
    }

    pub(crate) fn mark_committed(&mut self) -> Result<(), FrontierTxnError> {
        self.transaction.mark_committed()
    }

    pub(crate) fn install(&mut self) -> Result<(), FrontierTxnError> {
        self.transaction.install()
    }

    pub(crate) fn complete(&mut self) -> Result<(), FrontierTxnError> {
        self.transaction.complete()
    }

    pub(crate) fn retire_completed_recovery_blobs(&mut self) -> Result<usize, FrontierTxnError> {
        self.transaction.retire_completed_recovery_blobs()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_routine_evidence_transaction(
    barrier: CanonicalWriteBarrier,
    frontier: &Path,
    frontier_id: &str,
    kind: OperationKind,
    operation_id: OperationId,
    request_root: &str,
    fixed_time: String,
    mut read_set: Vec<InputBinding>,
    object_drafts: Vec<AuthorityObjectDraft>,
    derived_drafts: Vec<AuthorityDerivedDraft>,
) -> Result<PreparedRoutineEvidenceTransaction, String> {
    if object_drafts.is_empty() {
        return Err("routine evidence transaction changes no canonical object".into());
    }
    let mut inputs = BTreeMap::new();
    for input in read_set.drain(..) {
        if let Some(previous) = inputs.insert(input.name.clone(), input.digest.clone())
            && previous != input.digest
        {
            return Err(format!(
                "routine evidence input {} has conflicting roots",
                input.name
            ));
        }
    }
    let read_set = inputs
        .into_iter()
        .map(|(name, digest)| InputBinding { name, digest })
        .collect::<Vec<_>>();

    let mut writes = object_drafts
        .iter()
        .map(routine_object_write)
        .collect::<Result<Vec<_>, _>>()?;
    writes.extend(
        derived_drafts
            .iter()
            .map(routine_derived_write)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let draft = DeltaDraft::prepare(frontier, writes).map_err(|error| error.to_string())?;
    if draft.delta.writes().is_empty() {
        return Err("routine evidence transaction changes no bytes".into());
    }
    let layout = vela_protocol::canonical::to_canonical_bytes(&LayoutCommitment {
        schema: LAYOUT_SCHEMA,
        frontier_id,
        object_paths: object_drafts
            .iter()
            .map(|draft| draft.path.as_str())
            .collect(),
        derived_paths: derived_drafts
            .iter()
            .map(|draft| draft.path.as_str())
            .collect(),
    })?;
    // Current repositories retain scientific Decisions as authority events;
    // FrontierTxn's retired StateEvent log is intentionally empty.
    let empty_event_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&[])
    ))
    .map_err(|error| error.to_string())?;
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind,
            operation_id: operation_id.clone(),
            request_root: ContentDigest::parse(request_root.to_string())
                .map_err(|error| error.to_string())?,
            frontier: FrontierBinding::new(frontier, frontier_id, &layout)
                .map_err(|error| error.to_string())?,
            fixed_time,
            expected_event_log_root: empty_event_root.clone(),
            resulting_event_log_root: empty_event_root,
            resulting_event_ids: Vec::new(),
            read_set,
            result: serde_json::to_value(DurableResult {
                schema: RESULT_SCHEMA,
                operation_id: operation_id.as_str(),
                accepted_event_delta: 0,
            })
            .map_err(|error| error.to_string())?,
        },
        draft.delta.clone(),
    )
    .map_err(|error| error.to_string())?;
    let transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(|error| error.to_string())?;
    Ok(PreparedRoutineEvidenceTransaction { transaction })
}

fn routine_object_write(draft: &AuthorityObjectDraft) -> Result<PlannedWrite, String> {
    let path = RepoPath::parse(draft.path.clone()).map_err(|error| error.to_string())?;
    let Some(bytes) = &draft.postimage else {
        return Err(format!(
            "routine evidence transaction cannot delete {}",
            draft.path
        ));
    };
    let valid = match (draft.object_kind.as_str(), draft.class) {
        ("repository_manifest", WriteClass::CanonicalEvidence) => {
            draft.path == ".vela/repository.json"
        }
        ("claim_record", WriteClass::CanonicalEvidence) => {
            rooted_sha256_path(&draft.path, "records/claims/sha256/", ".json")
        }
        ("submission_artifact", WriteClass::CanonicalEvidence) => {
            rooted_sha256_path(&draft.path, "records/artifacts/sha256/", "")
        }
        ("proposal", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/proposals/sha256/", ".json")
        }
        ("submission", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/submissions/sha256/", ".json")
        }
        ("registration_record", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/registrations/sha256/", ".json")
        }
        ("verification_record", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/verifications/sha256/", ".json")
        }
        _ => false,
    };
    if !valid {
        return Err(format!(
            "routine evidence object {} at {} is outside the append-only evidence allowlist",
            draft.object_kind, draft.path
        ));
    }
    Ok(PlannedWrite::write(path, draft.class, bytes.clone()))
}

fn routine_derived_write(draft: &AuthorityDerivedDraft) -> Result<PlannedWrite, String> {
    if draft.path != "targets.json" {
        return Err(format!(
            "routine evidence derived path {} is not a Vela materialized view",
            draft.path
        ));
    }
    let Some(bytes) = &draft.postimage else {
        return Err("routine evidence transaction cannot delete targets.json".into());
    };
    let path = RepoPath::parse(draft.path.clone()).map_err(|error| error.to_string())?;
    Ok(PlannedWrite::write(
        path,
        WriteClass::Derived,
        bytes.clone(),
    ))
}

fn rooted_sha256_path(path: &str, prefix: &str, suffix: &str) -> bool {
    let Some(stem) = path
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    stem.len() == 64
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rooted(prefix: &str, suffix: &str) -> String {
        format!("{prefix}{}{suffix}", "a".repeat(64))
    }

    #[test]
    fn routine_evidence_allowlist_is_append_only_and_excludes_authority() {
        let submission = AuthorityObjectDraft {
            path: rooted("records/submissions/sha256/", ".json"),
            object_kind: "submission".into(),
            class: WriteClass::PublicReview,
            postimage: Some(b"submission".to_vec()),
        };
        assert!(routine_object_write(&submission).is_ok());

        let mut deletion = submission.clone();
        deletion.postimage = None;
        assert!(
            routine_object_write(&deletion)
                .unwrap_err()
                .contains("cannot delete")
        );

        for (object_kind, class, path) in [
            (
                "authority_record",
                WriteClass::Authority,
                ".vela/authority/records/forbidden.dsse.json",
            ),
            (
                "event",
                WriteClass::Authority,
                ".vela/authority/events/forbidden.json",
            ),
            (
                "decision",
                WriteClass::PublicReview,
                "records/decisions/forbidden.json",
            ),
            (
                "policy",
                WriteClass::CanonicalEvidence,
                ".vela/authority/policies/forbidden.json",
            ),
        ] {
            let draft = AuthorityObjectDraft {
                path: path.into(),
                object_kind: object_kind.into(),
                class,
                postimage: Some(Vec::new()),
            };
            assert!(routine_object_write(&draft).is_err(), "accepted {path}");
        }
    }

    #[test]
    fn routine_derived_writes_are_limited_to_a_present_target_index() {
        let target_index = AuthorityDerivedDraft {
            path: "targets.json".into(),
            postimage: Some(b"targets".to_vec()),
        };
        assert!(routine_derived_write(&target_index).is_ok());

        let mut deletion = target_index.clone();
        deletion.postimage = None;
        assert!(routine_derived_write(&deletion).is_err());

        let unrelated = AuthorityDerivedDraft {
            path: "frontier.json".into(),
            postimage: Some(Vec::new()),
        };
        assert!(routine_derived_write(&unrelated).is_err());
    }
}
