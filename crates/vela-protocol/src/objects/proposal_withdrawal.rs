//! Producer-owned lifecycle record: `vela.proposal-withdrawal.v2`.
//!
//! A withdrawal closes one still-pending Proposal using the exact identity that
//! signed its retained Submission. It is not a Decision, Event, Verification,
//! or change to accepted Standing.
//!
//! It is the one signed object that declares no identity of its own. The key
//! entitled to withdraw a Proposal is the key that signed the Submission
//! behind it, so the verifying key comes from that Submission rather than from
//! this payload — a withdrawal that named its own key could be written by
//! anyone.

use ed25519_dalek::SigningKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dsse::EnvelopeV1;
use crate::proposal::ProposalV1;
use crate::submission::SubmissionRecordV3;

pub const PROPOSAL_WITHDRAWAL_V2_SCHEMA: &str = "vela.proposal-withdrawal.v2";
pub const PROPOSAL_WITHDRAWAL_V2_PAYLOAD_TYPE: &str =
    "application/vnd.vela.proposal-withdrawal.v2+json";
pub const PROPOSAL_WITHDRAWAL_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const PROPOSAL_WITHDRAWAL_HANDLE_PREFIX: &str = "vpw_";

/// Exact, append-only producer request to stop reviewing one Proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalWithdrawalV2 {
    #[schemars(schema_with = "crate::wire_schema::proposal_withdrawal_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::proposal_id_reference")]
    pub proposal_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub proposal_root: String,
    #[schemars(schema_with = "crate::wire_schema::submission_id_reference")]
    pub submission_id: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub submission_root: String,
    #[schemars(schema_with = "crate::wire_schema::unbounded_text")]
    pub actor: String,
    #[schemars(schema_with = "crate::wire_schema::unbounded_text")]
    pub reason: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub created_at: String,
}

/// One retained withdrawal: the envelope as stored, and its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalWithdrawalEnvelopeV2 {
    pub envelope: EnvelopeV1,
    pub withdrawal: ProposalWithdrawalV2,
    /// Canonical bytes of the envelope: exactly what is written to disk.
    pub bytes: Vec<u8>,
    /// `sha256:` over [`Self::bytes`].
    pub root: String,
    /// `vpw_` plus the first sixteen hexadecimal characters of [`Self::root`].
    pub id: String,
}

impl ProposalWithdrawalEnvelopeV2 {
    pub fn seal(
        proposal: &ProposalV1,
        submission: &SubmissionRecordV3,
        actor: String,
        reason: String,
        created_at: String,
        key: &SigningKey,
    ) -> Result<Self, String> {
        proposal.verify()?;
        let proposal_root = proposal.canonical_root()?;
        if proposal.producer_package.root != submission.root {
            return Err("Proposal Withdrawal does not bind the Proposal's exact Submission".into());
        }
        if proposal.actor != actor
            || submission.submission.provenance.producer != actor
            || submission.submission.identity.actor_id != actor
        {
            return Err("Proposal Withdrawal actor does not own the exact Submission".into());
        }
        if submission.submission.identity.public_key_hex
            != hex::encode(key.verifying_key().to_bytes())
        {
            return Err(
                "Proposal Withdrawal signing key does not match the Submission identity".into(),
            );
        }
        let withdrawal = ProposalWithdrawalV2 {
            schema: PROPOSAL_WITHDRAWAL_V2_SCHEMA.into(),
            proposal_id: proposal.id(),
            proposal_root,
            submission_id: submission.id.clone(),
            submission_root: submission.root.clone(),
            actor,
            reason,
            created_at,
        };
        withdrawal.validate_semantics()?;
        let payload = crate::canonical::to_canonical_bytes(&withdrawal)?;
        let envelope = EnvelopeV1::seal_single(key, PROPOSAL_WITHDRAWAL_V2_PAYLOAD_TYPE, &payload);
        Self::from_envelope(envelope, submission)
    }

    /// Which Submission a retained withdrawal names, read before verification.
    ///
    /// A withdrawal is verified under the key of the Submission it names, so a
    /// reader holding only bytes has to learn that name before it can check
    /// anything. This returns the reference and nothing else — the withdrawal
    /// itself exists only once [`Self::parse`] has verified the signature
    /// under that Submission's declared key.
    pub fn declared_submission(bytes: &[u8]) -> Result<(String, String), String> {
        #[derive(Deserialize)]
        struct Declared {
            submission_id: String,
            submission_root: String,
        }

        let envelope =
            EnvelopeV1::parse("Proposal Withdrawal", bytes, PROPOSAL_WITHDRAWAL_MAX_BYTES)?;
        let payload = crate::dsse::decode_base64("Proposal Withdrawal payload", &envelope.payload)?;
        let declared: Declared = serde_json::from_slice(&payload).map_err(|error| {
            format!("Proposal Withdrawal names no retained Submission: {error}")
        })?;
        Ok((declared.submission_id, declared.submission_root))
    }

    /// Read a retained withdrawal, verifying it under the Submission's key.
    pub fn parse(bytes: &[u8], submission: &SubmissionRecordV3) -> Result<Self, String> {
        let envelope =
            EnvelopeV1::parse("Proposal Withdrawal", bytes, PROPOSAL_WITHDRAWAL_MAX_BYTES)?;
        Self::from_envelope(envelope, submission)
    }

    pub fn from_envelope(
        envelope: EnvelopeV1,
        submission: &SubmissionRecordV3,
    ) -> Result<Self, String> {
        let payload = envelope.open_single(
            "Proposal Withdrawal",
            PROPOSAL_WITHDRAWAL_V2_PAYLOAD_TYPE,
            &submission.submission.identity.public_key_hex,
        )?;
        let withdrawal: ProposalWithdrawalV2 =
            crate::canonical::from_json_slice_strict(&payload)
                .map_err(|error| format!("parse Proposal Withdrawal v2: {error}"))?;
        if crate::canonical::to_canonical_bytes(&withdrawal)? != payload {
            return Err("Proposal Withdrawal payload is not canonical JSON".into());
        }
        withdrawal.validate_semantics()?;
        if withdrawal.submission_root != submission.root {
            return Err("Proposal Withdrawal does not bind the supplied Submission".into());
        }

        let bytes = envelope.canonical_bytes()?;
        let root = crate::canonical::sha256_root(&bytes);
        let id = crate::shape::derive_handle(PROPOSAL_WITHDRAWAL_HANDLE_PREFIX, &root)?;
        Ok(Self {
            envelope,
            withdrawal,
            bytes,
            root,
            id,
        })
    }

    /// Check the withdrawal against the exact Proposal and Submission it names.
    pub fn verify_with(
        &self,
        proposal: &ProposalV1,
        submission: &SubmissionRecordV3,
    ) -> Result<(), String> {
        proposal.verify()?;
        if self.withdrawal.proposal_id != proposal.id()
            || self.withdrawal.proposal_root != proposal.canonical_root()?
            || self.withdrawal.submission_id != proposal.producer_package.id
            || self.withdrawal.submission_root != proposal.producer_package.root
            || self.withdrawal.submission_id != submission.id
            || self.withdrawal.submission_root != submission.root
        {
            return Err(
                "Proposal Withdrawal does not bind its exact Proposal and Submission".into(),
            );
        }
        if self.withdrawal.actor != proposal.actor
            || self.withdrawal.actor != submission.submission.provenance.producer
            || self.withdrawal.actor != submission.submission.identity.actor_id
        {
            return Err("Proposal Withdrawal actor does not own the exact Submission".into());
        }
        Ok(())
    }
}

impl ProposalWithdrawalV2 {
    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != PROPOSAL_WITHDRAWAL_V2_SCHEMA {
            return Err(format!(
                "Proposal Withdrawal schema must be `{PROPOSAL_WITHDRAWAL_V2_SCHEMA}`"
            ));
        }
        require_sha256("proposal_root", &self.proposal_root)?;
        require_sha256("submission_root", &self.submission_root)?;
        crate::shape::require_derived_handle(
            "Proposal Withdrawal proposal_id",
            &self.proposal_id,
            "vpr_",
            &self.proposal_root,
        )?;
        crate::shape::require_derived_handle(
            "Proposal Withdrawal submission_id",
            &self.submission_id,
            "vsb_",
            &self.submission_root,
        )?;
        require_text("actor", &self.actor)?;
        require_text("reason", &self.reason)?;
        crate::shape::parse_canonical_time("Proposal Withdrawal created_at", &self.created_at)?;
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

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "Proposal Withdrawal {field} must be a full sha256: digest"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal::{ProposalProducerPackage, ProposalSubject};
    use crate::signer_identity::{ActorClass, SignerIdentityV1};
    use crate::submission::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };

    fn fixture() -> (ProposalV1, SubmissionRecordV3, SigningKey) {
        let key = SigningKey::from_bytes(&[7; 32]);
        let actor = "agent:producer".to_string();
        let identity = SignerIdentityV1::new(
            actor.clone(),
            ActorClass::Agent,
            &key,
            "2026-08-01T00:00:00Z",
        )
        .unwrap();
        let submission = SubmissionRecordV3::seal(
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
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-08-01T00:00:00Z".into(),
                },
            },
            identity,
            &key,
        )
        .unwrap();
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
                kind: "submission".into(),
                id: submission.id.clone(),
                root: submission.root.clone(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission.root.trim_start_matches("sha256:")
                ),
            },
            vec![],
        )
        .unwrap();
        (proposal, submission, key)
    }

    fn withdraw(
        proposal: &ProposalV1,
        submission: &SubmissionRecordV3,
        key: &SigningKey,
    ) -> Result<ProposalWithdrawalEnvelopeV2, String> {
        ProposalWithdrawalEnvelopeV2::seal(
            proposal,
            submission,
            proposal.actor.clone(),
            "Superseded by a corrected Submission.".into(),
            "2026-08-01T01:00:00Z".into(),
            key,
        )
    }

    #[test]
    fn exact_submission_identity_can_withdraw_its_proposal() {
        let (proposal, submission, key) = fixture();
        let sealed = withdraw(&proposal, &submission, &key).unwrap();
        sealed.verify_with(&proposal, &submission).unwrap();
        assert_eq!(
            ProposalWithdrawalEnvelopeV2::parse(&sealed.bytes, &submission).unwrap(),
            sealed
        );
        assert_eq!(
            sealed.id,
            crate::shape::derive_handle("vpw_", &sealed.root).unwrap()
        );
    }

    #[test]
    fn another_key_cannot_withdraw_the_proposal() {
        let (proposal, submission, _) = fixture();
        let error =
            withdraw(&proposal, &submission, &SigningKey::from_bytes(&[9; 32])).unwrap_err();
        assert!(error.contains("does not match the Submission identity"));
    }

    /// The verifying key comes from the Submission, so a withdrawal signed by
    /// anyone else fails on the way in rather than on a field comparison.
    #[test]
    fn a_withdrawal_signed_by_a_stranger_does_not_open() {
        let (proposal, submission, key) = fixture();
        let sealed = withdraw(&proposal, &submission, &key).unwrap();

        let payload = crate::dsse::decode_base64("p", &sealed.envelope.payload).unwrap();
        let forged = EnvelopeV1::seal_single(
            &SigningKey::from_bytes(&[9; 32]),
            PROPOSAL_WITHDRAWAL_V2_PAYLOAD_TYPE,
            &payload,
        );
        assert!(ProposalWithdrawalEnvelopeV2::from_envelope(forged, &submission).is_err());
    }

    /// Every reference in a withdrawal derives from a root beside it, so a
    /// handle naming some other object cannot be written at all.
    #[test]
    fn a_reference_handle_must_derive_from_the_root_beside_it() {
        let (proposal, submission, key) = fixture();
        let sealed = withdraw(&proposal, &submission, &key).unwrap();

        type Field = fn(&mut ProposalWithdrawalV2, String);
        let cases: [(&str, &str, Field); 2] = [
            ("proposal_id", "vpr_", |value, handle| {
                value.proposal_id = handle
            }),
            ("submission_id", "vsb_", |value, handle| {
                value.submission_id = handle
            }),
        ];
        for (field, prefix, set) in cases {
            let mut mutated = sealed.withdrawal.clone();
            let other =
                crate::shape::derive_handle(prefix, &format!("sha256:{}", "e".repeat(64))).unwrap();
            set(&mut mutated, other);
            let error = mutated.validate_semantics().unwrap_err();
            assert!(error.contains(field), "{error}");
            assert!(error.contains("the handle its root derives"), "{error}");
        }
    }
}
