//! Producer-owned lifecycle record: `vela.proposal-withdrawal.v1`.
//!
//! A withdrawal closes one still-pending Proposal using the exact identity that
//! signed its retained Submission. It is not a Decision, Event, Verification,
//! or change to accepted Standing.

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::proposal_v1::ProposalV1;
use crate::submission_v1::SubmissionV1;

pub const PROPOSAL_WITHDRAWAL_V1_SCHEMA: &str = "vela.proposal-withdrawal.v1";
pub const PROPOSAL_WITHDRAWAL_V1_AUTH_ALGORITHM: &str = "ed25519";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalWithdrawalAuthenticationV1 {
    pub algorithm: String,
    pub signature: String,
}

/// Exact, append-only producer request to stop reviewing one Proposal.
///
/// `withdrawal_id` and `authentication.signature` are cleared for the signed
/// preimage. The retained Submission supplies the public identity binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalWithdrawalV1 {
    pub schema: String,
    pub withdrawal_id: String,
    pub proposal_id: String,
    pub proposal_root: String,
    pub submission_id: String,
    pub submission_root: String,
    pub actor: String,
    pub reason: String,
    pub created_at: String,
    pub authentication: ProposalWithdrawalAuthenticationV1,
}

impl ProposalWithdrawalV1 {
    pub fn build(
        proposal: &ProposalV1,
        proposal_root: String,
        submission: &SubmissionV1,
        actor: String,
        reason: String,
        created_at: String,
        key: &SigningKey,
    ) -> Result<Self, String> {
        proposal.verify()?;
        submission.verify()?;
        if proposal.canonical_root()? != proposal_root {
            return Err("Proposal Withdrawal proposal_root does not match its Proposal".into());
        }
        if proposal.producer_package.id != submission.submission_id
            || proposal.producer_package.root != submission.canonical_root()?
        {
            return Err("Proposal Withdrawal does not bind the Proposal's exact Submission".into());
        }
        if proposal.actor != actor
            || submission.provenance.producer != actor
            || submission.authentication.identity_binding.actor_id != actor
        {
            return Err("Proposal Withdrawal actor does not own the exact Submission".into());
        }
        if submission.authentication.identity_binding.public_key_hex
            != hex::encode(key.verifying_key().to_bytes())
        {
            return Err(
                "Proposal Withdrawal signing key does not match the Submission identity".into(),
            );
        }
        let mut value = Self {
            schema: PROPOSAL_WITHDRAWAL_V1_SCHEMA.into(),
            withdrawal_id: String::new(),
            proposal_id: proposal.proposal_id.clone(),
            proposal_root,
            submission_id: submission.submission_id.clone(),
            submission_root: submission.canonical_root()?,
            actor,
            reason,
            created_at,
            authentication: ProposalWithdrawalAuthenticationV1 {
                algorithm: PROPOSAL_WITHDRAWAL_V1_AUTH_ALGORITHM.into(),
                signature: String::new(),
            },
        };
        value.validate_semantics()?;
        value.authentication.signature =
            hex::encode(crate::sign::sign_bytes(key, &value.signed_preimage()?));
        value.withdrawal_id = value.derive_id()?;
        value.verify_with(proposal, submission)?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Proposal Withdrawal exceeds the 2 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse Proposal Withdrawal v1: {error}"))?;
        value.validate_semantics()?;
        require_prefixed("withdrawal_id", &value.withdrawal_id, "vpw_", false)?;
        if value.authentication.signature.is_empty() {
            return Err("Proposal Withdrawal signature cannot be empty".into());
        }
        if value.canonical_bytes()?.as_slice() != bytes {
            return Err("Proposal Withdrawal bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify_with(
        &self,
        proposal: &ProposalV1,
        submission: &SubmissionV1,
    ) -> Result<(), String> {
        self.validate_semantics()?;
        proposal.verify()?;
        submission.verify()?;
        if self.proposal_id != proposal.proposal_id
            || self.proposal_root != proposal.canonical_root()?
            || self.submission_id != proposal.producer_package.id
            || self.submission_root != proposal.producer_package.root
            || self.submission_id != submission.submission_id
            || self.submission_root != submission.canonical_root()?
        {
            return Err(
                "Proposal Withdrawal does not bind its exact Proposal and Submission".into(),
            );
        }
        if self.actor != proposal.actor
            || self.actor != submission.provenance.producer
            || self.actor != submission.authentication.identity_binding.actor_id
        {
            return Err("Proposal Withdrawal actor does not own the exact Submission".into());
        }
        if !crate::sign::verify_action_signature(
            &self.signed_preimage()?,
            &self.authentication.signature,
            &submission.authentication.identity_binding.public_key_hex,
        )? {
            return Err("Proposal Withdrawal signature does not verify".into());
        }
        let expected = self.derive_id()?;
        if expected != self.withdrawal_id {
            return Err(format!(
                "Proposal Withdrawal id mismatch: declared {}, rebuilt {expected}",
                self.withdrawal_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    fn signed_preimage(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.withdrawal_id.clear();
        unsigned.authentication.signature.clear();
        crate::canonical::to_canonical_bytes(&unsigned)
    }

    fn derive_id(&self) -> Result<String, String> {
        Ok(format!(
            "vpw_{}",
            &hex::encode(Sha256::digest(self.signed_preimage()?))[..16]
        ))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != PROPOSAL_WITHDRAWAL_V1_SCHEMA {
            return Err(format!(
                "Proposal Withdrawal schema must be `{PROPOSAL_WITHDRAWAL_V1_SCHEMA}`"
            ));
        }
        require_prefixed("withdrawal_id", &self.withdrawal_id, "vpw_", true)?;
        require_prefixed("proposal_id", &self.proposal_id, "vpr_", false)?;
        require_sha256("proposal_root", &self.proposal_root)?;
        require_prefixed("submission_id", &self.submission_id, "vsb_", false)?;
        require_sha256("submission_root", &self.submission_root)?;
        require_text("actor", &self.actor)?;
        require_text("reason", &self.reason)?;
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| "Proposal Withdrawal created_at must be RFC 3339".to_string())?;
        if self.authentication.algorithm != PROPOSAL_WITHDRAWAL_V1_AUTH_ALGORITHM {
            return Err("Proposal Withdrawal authentication.algorithm must be `ed25519`".into());
        }
        if !self.authentication.signature.is_empty() {
            let bytes = hex::decode(&self.authentication.signature)
                .map_err(|_| "Proposal Withdrawal signature must be hex".to_string())?;
            if bytes.len() != 64 {
                return Err("Proposal Withdrawal signature must be 64 bytes".into());
            }
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Proposal Withdrawal {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(
    field: &str,
    value: &str,
    prefix: &str,
    allow_empty: bool,
) -> Result<(), String> {
    if allow_empty && value.is_empty() {
        return Ok(());
    }
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "Proposal Withdrawal {field} must start with {prefix}"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("Proposal Withdrawal {field} must be a full sha256: digest"))?;
    if !crate::shape::is_lower_hex_64(digest) {
        return Err(format!(
            "Proposal Withdrawal {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use crate::proposal_v1::{ProposalProducerPackage, ProposalSubject};
    use crate::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };

    fn fixture() -> (ProposalV1, SubmissionV1, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let actor = "agent:producer".to_string();
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: actor.clone(),
                actor_class: ActorClass::Agent,
                created_at: "2026-08-01T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "A bounded result.".into(),
                    claim_type: "computational".into(),
                    conditions: vec![],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "artifacts/result.json".into(),
                    digest: format!("sha256:{}", "c".repeat(64)),
                }],
                caveats: vec!["Bounded only.".into()],
                replayability: "exact".into(),
                producer_checks: vec![],
                verification_requirements: vec!["Replay.".into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: actor.clone(),
                    source_system: "fixture".into(),
                    source_attempt: Some(format!("vat_{}", "d".repeat(64))),
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-08-01T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap();
        let submission_root = submission.canonical_root().unwrap();
        let proposal = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: format!("vcl_{}", "a".repeat(64)),
                root: format!("sha256:{}", "b".repeat(64)),
            },
            actor,
            "2026-08-01T00:00:00Z".into(),
            "Request review.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission.submission_id.clone(),
                root: submission_root,
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission
                        .canonical_root()
                        .unwrap()
                        .trim_start_matches("sha256:")
                ),
            },
            vec![],
        )
        .unwrap();
        (proposal, submission, key)
    }

    #[test]
    fn exact_submission_identity_can_withdraw_its_proposal() {
        let (proposal, submission, key) = fixture();
        let withdrawal = ProposalWithdrawalV1::build(
            &proposal,
            proposal.canonical_root().unwrap(),
            &submission,
            proposal.actor.clone(),
            "Superseded by a corrected Submission.".into(),
            "2026-08-01T01:00:00Z".into(),
            &key,
        )
        .unwrap();
        withdrawal.verify_with(&proposal, &submission).unwrap();
        ProposalWithdrawalV1::parse(&withdrawal.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn another_key_cannot_withdraw_the_proposal() {
        let (proposal, submission, _) = fixture();
        let error = ProposalWithdrawalV1::build(
            &proposal,
            proposal.canonical_root().unwrap(),
            &submission,
            proposal.actor.clone(),
            "Not mine.".into(),
            "2026-08-01T01:00:00Z".into(),
            &SigningKey::from_bytes(&[9; 32]),
        )
        .unwrap_err();
        assert!(error.contains("does not match the Submission identity"));
    }
}
