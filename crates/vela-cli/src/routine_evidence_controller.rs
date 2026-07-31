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

pub(crate) struct RoutineEvidenceController {
    repository_signer: Box<dyn RepositoryAuthoritySigner>,
}

impl RoutineEvidenceController {
    pub(crate) fn new(repository_signer: Box<dyn RepositoryAuthoritySigner>) -> Self {
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
            self.repository_signer.as_mut(),
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
            self.repository_signer.as_mut(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use vela_protocol::authority::DsseSignatureV1;

    use super::*;

    struct CountingSigner {
        calls: Arc<AtomicUsize>,
    }

    impl RepositoryAuthoritySigner for CountingSigner {
        fn sign(
            &mut self,
            _payload_type: &str,
            _canonical_payload: &[u8],
        ) -> Result<Vec<DsseSignatureV1>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err("fixture signer must not be reached".into())
        }
    }

    #[test]
    fn controller_construction_does_not_touch_the_signer() {
        let calls = Arc::new(AtomicUsize::new(0));
        let signer = CountingSigner {
            calls: Arc::clone(&calls),
        };
        let _controller = RoutineEvidenceController::new(Box::new(signer));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}
