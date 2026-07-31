//! Narrow controller boundary for long-running routine evidence work.
//!
//! The controller owns one already-selected repository signer and may use it
//! only through the existing closed Submission and Verification writers.
//! Worker and verifier processes receive neither this value nor signer
//! material. Deliberately, this type has no review or Decision method.

use std::path::Path;

use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::authority_transaction::RepositoryAuthoritySigner;
use crate::workflow::{SubmitOutcome, VerificationImportOutcome};

#[allow(dead_code)] // Compiled boundary; a durable controller host is deliberately not added yet.
pub(crate) struct RoutineEvidenceController<'a> {
    repository_signer: &'a mut dyn RepositoryAuthoritySigner,
}

#[allow(dead_code)] // Methods are the closed seam a future measured host may invoke.
impl<'a> RoutineEvidenceController<'a> {
    pub(crate) fn new(repository_signer: &'a mut dyn RepositoryAuthoritySigner) -> Self {
        Self { repository_signer }
    }

    pub(crate) fn register_submission(
        &mut self,
        frontier: &Path,
        submission: &SubmissionV1,
        executor: &str,
        attempt_id: &str,
        bundle_root: Option<&Path>,
        push: bool,
    ) -> Result<SubmitOutcome, String> {
        crate::current_submission::submit_with_repository_signer(
            frontier,
            submission,
            executor,
            Some(attempt_id),
            bundle_root,
            push,
            self.repository_signer,
        )
    }

    pub(crate) fn import_verification(
        &mut self,
        frontier: &Path,
        record: &VerificationRecordV1,
        executor: &str,
        attempt_id: &str,
        push: bool,
    ) -> Result<VerificationImportOutcome, String> {
        crate::current_verification::import_with_repository_signer(
            frontier,
            record,
            executor,
            attempt_id,
            push,
            self.repository_signer,
        )
    }
}

#[cfg(test)]
mod tests {
    use vela_protocol::authority::DsseSignatureV1;

    use super::*;

    #[derive(Default)]
    struct CountingSigner {
        calls: usize,
    }

    impl RepositoryAuthoritySigner for CountingSigner {
        fn sign(
            &mut self,
            _payload_type: &str,
            _canonical_payload: &[u8],
        ) -> Result<Vec<DsseSignatureV1>, String> {
            self.calls += 1;
            Err("fixture signer must not be reached".into())
        }
    }

    #[test]
    fn controller_construction_does_not_touch_the_signer() {
        let mut signer = CountingSigner::default();
        let _controller = RoutineEvidenceController::new(&mut signer);
        assert_eq!(signer.calls, 0);
    }
}
