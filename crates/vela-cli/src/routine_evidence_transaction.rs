//! Crash-safe writes for authenticated routine evidence.
//!
//! Submissions and Verification Records authenticate their own producers.
//! Retaining those objects must not mint a repository-authority record or
//! touch the repository-authority SSH key. This module deliberately reuses
//! the existing repository transaction journal and publication delta while
//! leaving scientific Events and authority history unchanged.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::authority_transaction::AuthorityObjectDraft;
use crate::config::git_publish::{
    ExactPublicationPreflight, PublicationDelta, PublishOptions, exact_publication_preflight,
};
use crate::repository_ops::{publication_delta, publication_error};
use vela_repository::{
    CanonicalWriteBarrier, ContentDigest, DeltaDraft, InputBinding, OperationId, OperationKind,
    PlannedWrite, RepoPath, RepositoryBinding, RepositoryTxn, RepositoryTxnError,
    RepositoryTxnPlan, RepositoryTxnPlanSpec, WriteClass,
};

const RESULT_SCHEMA: &str = "vela.routine-evidence-transaction-result.internal.v1";

#[derive(Serialize)]
struct DurableResult<'a> {
    schema: &'static str,
    operation_id: &'a str,
    accepted_event_delta: u8,
}

/// Prepared routine evidence write with the same durability/publication
/// surface as an authority transaction, but no authority signer or envelope.
pub(crate) struct PreparedRoutineEvidenceTransaction {
    transaction: RepositoryTxn,
}

impl PreparedRoutineEvidenceTransaction {
    pub(crate) fn resolved_public_writes(
        &self,
    ) -> Result<Vec<vela_repository::ResolvedWrite>, RepositoryTxnError> {
        self.transaction.resolved_public_writes()
    }

    pub(crate) fn canonical_delta_root(&self) -> &str {
        self.transaction.canonical_delta_root()
    }

    pub(crate) fn abort_prepared(&mut self) -> Result<(), RepositoryTxnError> {
        self.transaction.abort_prepared()
    }

    pub(crate) fn preflight_publication(
        &mut self,
        repository: &Path,
        options: impl FnOnce() -> Result<PublishOptions, String>,
        missing_delta: &str,
    ) -> Result<(PublicationDelta, ExactPublicationPreflight), String> {
        let precommit = (|| {
            let public = self
                .resolved_public_writes()
                .map_err(|error| error.to_string())?;
            let delta_root = self.canonical_delta_root().to_string();
            let publish_options = options()?;
            let delta = publication_delta(repository, &delta_root, public)?
                .ok_or_else(|| missing_delta.to_string())?;
            let preflight = exact_publication_preflight(repository, &delta, &publish_options)
                .map_err(publication_error)?;
            Ok::<_, String>((delta, preflight))
        })();
        match precommit {
            Ok(value) => Ok(value),
            Err(error) => {
                self.abort_prepared()
                    .map_err(|abort| format!("{error}; abort failed: {abort}"))?;
                Err(error)
            }
        }
    }

    pub(crate) fn mark_committed(&mut self) -> Result<(), RepositoryTxnError> {
        self.transaction.mark_committed()
    }

    pub(crate) fn install(&mut self) -> Result<(), RepositoryTxnError> {
        self.transaction.install()
    }

    pub(crate) fn complete(&mut self) -> Result<(), RepositoryTxnError> {
        self.transaction.complete()
    }

    pub(crate) fn retire_completed_recovery_blobs(&mut self) -> Result<usize, RepositoryTxnError> {
        self.transaction.retire_completed_recovery_blobs()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_routine_evidence_transaction(
    barrier: CanonicalWriteBarrier,
    repository: &Path,
    repository_id: &str,
    kind: OperationKind,
    operation_id: OperationId,
    request_root: &str,
    fixed_time: String,
    mut read_set: Vec<InputBinding>,
    object_drafts: Vec<AuthorityObjectDraft>,
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

    let writes = object_drafts
        .iter()
        .map(routine_object_write)
        .collect::<Result<Vec<_>, _>>()?;
    let draft = DeltaDraft::prepare(repository, writes).map_err(|error| error.to_string())?;
    if draft.delta.writes().is_empty() {
        return Err("routine evidence transaction changes no bytes".into());
    }
    let plan = RepositoryTxnPlan::new(
        RepositoryTxnPlanSpec {
            kind,
            operation_id: operation_id.clone(),
            request_root: ContentDigest::parse(request_root.to_string())
                .map_err(|error| error.to_string())?,
            repository: RepositoryBinding::new(repository, repository_id)
                .map_err(|error| error.to_string())?,
            fixed_time,
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
    let transaction = RepositoryTxn::prepare_with_barrier(barrier, plan, draft)
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
        ("proposal_withdrawal", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/proposal-withdrawals/sha256/", ".json")
        }
        ("submission", WriteClass::PublicReview) => {
            rooted_sha256_path(&draft.path, "records/submissions/sha256/", ".json")
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

fn rooted_sha256_path(path: &str, prefix: &str, suffix: &str) -> bool {
    let Some(stem) = path
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    vela_protocol::is_lower_hex_64(stem)
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
        let withdrawal = AuthorityObjectDraft {
            path: rooted("records/proposal-withdrawals/sha256/", ".json"),
            object_kind: "proposal_withdrawal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(b"withdrawal".to_vec()),
        };
        assert!(routine_object_write(&withdrawal).is_ok());

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
                ".vela/authority/models/forbidden.json",
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
}
