//! Claim standing, and the Proposal status it is not.
//!
//! Two questions get asked of one Claim — does it stand, and what happened to
//! the Proposal about it — and through `0.966.2` the read surface answered the
//! first in the second's words. A Claim whose Proposal was undecided reported
//! `pending_review`, one whose producer withdrew it reported `withdrawn`, one
//! a Decision rejected reported `rejected`. None of the three is declared for
//! the standing axis in `docs/PROTOCOL.md`, and the first consumer to
//! implement the declared vocabulary mapped all three onto `unassessed`, which
//! is where the difference between a rejected Claim and one nobody has looked
//! at was actually lost.
//!
//! Both axes are derived, and each is emitted under a name that says which
//! axis it is. Nothing here rewrites retained bytes: the repository manifest
//! keeps its own tokens, and this is the read boundary where they stop being
//! repeated as a standing.

/// A Claim's standing beside the Proposal status it was derived from.
///
/// `proposal_status` is `None` where no Proposal survives to have one. A Claim
/// carried through compaction keeps its standing in the repository manifest,
/// and the Proposal that once admitted it is not retained.
pub(crate) struct ClaimStanding {
    pub(crate) standing: &'static str,
    pub(crate) proposal_status: Option<String>,
}

pub(crate) const UNASSESSED: &str = "unassessed";
pub(crate) const ACCEPTED: &str = "accepted";
pub(crate) const SUPERSEDED: &str = "superseded";
pub(crate) const RETRACTED: &str = "retracted";

/// Read a Proposal-axis word onto the standing axis.
///
/// The standing axis reads a ruling, not a queue. Only an accepted Decision
/// puts a Claim in `accepted`; undecided, withdrawn by its producer, and
/// rejected all leave the same fact behind — no ruling stands over this Claim
/// — and `unassessed` is the word `docs/PROTOCOL.md` declares for exactly
/// that. Which of the three happened is a fact about the Proposal, carried
/// beside the standing rather than folded into it.
///
/// This also reads the repository manifest's claim-list tokens, `accepted` and
/// `pending_review`, because those are Proposal-axis words too: the manifest
/// records what a Decision did.
///
/// This reads a verdict alone, so it is only correct where the act behind the
/// verdict is already known to be an admission — the manifest's own lists, and
/// `claim.add` or `claim.revise`. Where the act could be a withdrawal, read
/// [`from_proposal_outcome`] instead.
///
/// `accepted_with_conditions` and `corrected` stay underived, and deriving them
/// here would be inventing semantics rather than reading them. A Decision
/// records no conditions, so nothing distinguishes `accepted_with_conditions`
/// from `accepted`. And `corrects` is a Claim relation no Decision reads, so
/// `corrected` has a relation behind it but no authority behind that.
pub(crate) fn from_proposal_status(status: &str) -> &'static str {
    match status {
        "accepted" => ACCEPTED,
        _ => UNASSESSED,
    }
}

/// Read a decided Proposal onto the standing axis, given what it asked for.
///
/// A verdict is only half of a ruling: the other half is the act it ruled on.
/// An accepted `claim.withdraw` is an accepted Decision that takes the Claim
/// out of repository state, and reading the verdict alone reported the
/// retracted Claim as `accepted` — the strongest word on the axis, for the one
/// act that removes standing. `retracted` is what `docs/PROTOCOL.md`
/// declares for it, and the `claim.retracted` Event the same transaction
/// commits is the authority behind the word.
pub(crate) fn from_proposal_outcome(action: &str, status: &str) -> &'static str {
    match (action, status) {
        ("claim.withdraw", "accepted") => RETRACTED,
        _ => from_proposal_status(status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three Proposal outcomes that are not an acceptance are one fact on
    /// the standing axis. Emitting any of them as a standing is the collapse
    /// this module exists to stop.
    #[test]
    fn every_unaccepted_proposal_outcome_reads_as_unassessed() {
        for status in ["pending_review", "rejected", "withdrawn"] {
            assert_eq!(from_proposal_status(status), UNASSESSED);
        }
        assert_eq!(from_proposal_status("accepted"), ACCEPTED);
    }

    /// The manifest's pending list carries `pending_review`, a Proposal-axis
    /// word, and the same map covers it.
    #[test]
    fn the_manifest_pending_token_reads_as_unassessed() {
        let reference = vela_protocol::repository::ClaimStandingRefV1 {
            claim_id: format!("vcl_{}", "a".repeat(64)),
            claim_root: format!("sha256:{}", "b".repeat(64)),
            standing: "pending_review".into(),
            path: "records/claims/sha256/b.json".into(),
        };
        assert_eq!(from_proposal_status(&reference.standing), UNASSESSED);
    }

    /// The one act whose acceptance removes standing rather than granting it.
    /// Reading the verdict alone made a retraction the strongest word on the
    /// axis.
    #[test]
    fn an_accepted_withdrawal_retracts_rather_than_accepts() {
        assert_eq!(
            from_proposal_outcome("claim.withdraw", "accepted"),
            RETRACTED
        );
        for action in ["claim.add", "claim.revise"] {
            assert_eq!(from_proposal_outcome(action, "accepted"), ACCEPTED);
        }
        /* An undecided or refused withdrawal leaves the Claim exactly where it
        was, which is the accepted list — so the manifest answers for it and
        this map never has to guess a standing from a withdrawal it did not
        grant. */
        for status in ["pending_review", "rejected", "withdrawn"] {
            assert_eq!(from_proposal_outcome("claim.withdraw", status), UNASSESSED);
        }
    }
}
