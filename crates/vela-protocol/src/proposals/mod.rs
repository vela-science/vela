//! Proposal-first frontier writes and proof freshness tracking.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::{Annotation, Artifact, ConfidenceMethod, FindingBundle};
use crate::canonical;
use crate::events::{self, NULL_HASH, StateActor, StateEvent, StateTarget};
use crate::project::{self, Project};
use crate::propagate::{self, PropagationAction};
use crate::repo;

pub mod policy_accept;
mod types;
pub use types::*;

pub fn new_proposal(
    kind: impl Into<String>,
    target: StateTarget,
    actor_id: impl Into<String>,
    actor_type: impl Into<String>,
    reason: impl Into<String>,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
) -> StateProposal {
    new_proposal_at(
        kind,
        target,
        actor_id,
        actor_type,
        reason,
        payload,
        source_refs,
        caveats,
        Utc::now().to_rfc3339(),
    )
}

/// Build a content-addressed proposal with an injected timestamp.
///
/// Transactional writers use one fixed instant for every staged object so a
/// retry or post-marker recovery never consults the wall clock again. The
/// timestamp remains non-canonical proposal metadata; the logical proposal ID
/// is derived by [`proposal_id`] exactly as before.
pub fn new_proposal_at(
    kind: impl Into<String>,
    target: StateTarget,
    actor_id: impl Into<String>,
    actor_type: impl Into<String>,
    reason: impl Into<String>,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
    created_at: impl Into<String>,
) -> StateProposal {
    let mut proposal = StateProposal {
        schema: PROPOSAL_SCHEMA.to_string(),
        id: String::new(),
        kind: kind.into(),
        target,
        actor: StateActor {
            id: actor_id.into(),
            r#type: actor_type.into(),
        },
        created_at: created_at.into(),
        drafted_at: None,
        reason: reason.into(),
        payload,
        source_refs,
        status: "pending_review".to_string(),
        reviewed_by: None,
        reviewed_at: None,
        decision_reason: None,
        applied_event_id: None,
        caveats,
        agent_run: None,
    };
    proposal.id = proposal_id(&proposal);
    proposal
}

/// Phase P (v0.5): `vpr_…` is content-addressed over the *logical* proposal
/// content only — `created_at` is excluded from the preimage. Identical
/// logical proposals (same actor, target, kind, reason, payload) deterministically
/// produce the same proposal_id regardless of when they were constructed.
///
/// This is the substrate property that makes agent retries idempotent.
/// `created_at` stays on the proposal as non-canonical metadata; replay-attack
/// detection layers on the signed envelope, not the content hash.
pub fn proposal_id(proposal: &StateProposal) -> String {
    let preimage = json!({
        "schema": proposal.schema,
        "kind": proposal.kind,
        "target": proposal.target,
        "actor": proposal.actor,
        "reason": proposal.reason,
        "payload": proposal.payload,
        "source_refs": proposal.source_refs,
        "caveats": proposal.caveats,
    });
    let bytes = canonical::to_canonical_bytes(&preimage).unwrap_or_default();
    format!("vpr_{}", &hex::encode(Sha256::digest(bytes))[..16])
}

pub fn is_placeholder_reviewer(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "local-reviewer"
        || normalized == "local-user"
        || normalized == "reviewer"
        || normalized == "user"
        || normalized == "unknown"
        || normalized.starts_with("local-")
}

pub fn validate_reviewer_identity(value: &str) -> Result<(), String> {
    if is_placeholder_reviewer(value) {
        return Err(format!(
            "Reviewer identity '{}' is missing or placeholder. Use a stable named reviewer id.",
            value
        ));
    }
    Ok(())
}

/// v0.128: the canonical action verb signed over the accept preimage.
/// A reviewer's accept signature commits to this verb so a captured
/// signature can never be repurposed as a different action.
pub const PROPOSAL_ACCEPT_ACTION: &str = "proposal.accept";

/// v0.128: structured outcome of [`authorize_proposal_accept`]. Carries
/// the resolved, non-revoked reviewer `ActorRecord` so the caller hands
/// the *registry-canonical* `actor.id` (not the wire pubkey) to
/// `accept_proposal_in_frontier`, keeping the persisted `reviewed_by`
/// honest.
#[derive(Debug, Clone)]
pub struct AcceptAuthorization {
    actor: crate::sign::ActorRecord,
    frontier_id: String,
    proposal_id: String,
    reason: String,
    parent_event_log_hash: String,
    decision_root: Option<String>,
}

impl AcceptAuthorization {
    /// The registry-canonical reviewer whose key signed this exact decision.
    #[must_use]
    pub fn actor(&self) -> &crate::sign::ActorRecord {
        &self.actor
    }

    fn validate_binding(
        &self,
        project: &Project,
        proposal_id: &str,
        reviewer: &str,
        reason: &str,
    ) -> Result<(), String> {
        if project.frontier_id() != self.frontier_id
            || proposal_id != self.proposal_id
            || reviewer != self.actor.id
            || reason != self.reason
            || crate::events::event_log_hash(&project.events) != self.parent_event_log_hash
        {
            return Err(
                "verified accept authority is stale or bound to different decision bytes"
                    .to_string(),
            );
        }
        let current = validate_human_reviewer_authority_at(
            project,
            reviewer,
            &chrono::Utc::now().to_rfc3339(),
        )?;
        if !current
            .public_key
            .eq_ignore_ascii_case(&self.actor.public_key)
        {
            return Err("verified accept authority actor binding changed".to_string());
        }
        Ok(())
    }

    /// Decision root committed by a v1 detached authority, when present.
    #[must_use]
    pub fn decision_root(&self) -> Option<&str> {
        self.decision_root.as_deref()
    }
}

/// v0.128: true iff an actor carries *reviewer* authority for the public
/// accept boundary. Doctrine: accept authority is the `reviewer:`
/// namespace and only that namespace — a non-placeholder
/// `reviewer:<name>` id. The v0.6 `auto-notes` write tier (and any other
/// tier) does NOT grant accept authority; tier gates one-call note
/// auto-apply, never reviewer decisions. This is deliberately stricter
/// than `validate_reviewer_identity` (which only rejects placeholders):
/// here an `agent:` or bare id is refused outright.
#[must_use]
pub fn actor_has_reviewer_authority(actor: &crate::sign::ActorRecord) -> bool {
    let id = actor.id.trim();
    id.to_ascii_lowercase().starts_with("reviewer:") && !is_placeholder_reviewer(id)
}

/// v0.128: canonical signing bytes for a detached proposal-accept
/// decision. Binds the action verb, the target frontier, the exact
/// proposal, the reviewer's resolved identity, and the decision reason
/// into one preimage. Because `proposal_id` and `reviewer_id` are both
/// inside the preimage, a signature captured for one proposal cannot be
/// replayed against another, nor by another reviewer, nor under a
/// different reason. Mirrors `proposal_signing_bytes`' canonical-JSON
/// discipline (`canonical::to_canonical_bytes`, sorted keys).
pub fn accept_preimage_bytes(
    vfr_id: &str,
    proposal_id: &str,
    reviewer_id: &str,
    reason: &str,
    parent_event_log_hash: &str,
) -> Result<Vec<u8>, String> {
    let preimage = json!({
        "action": PROPOSAL_ACCEPT_ACTION,
        "vfr_id": vfr_id,
        "proposal_id": proposal_id,
        "reviewer_id": reviewer_id,
        "reason": reason,
        // ADR 0001 Phase 0d: bind the chain head the reviewer accepted
        // against. The signature therefore attests "I accepted this
        // proposal on top of THIS history", so a captured accept cannot be
        // replayed onto a re-ordered or forked log where the head differs:
        // the verifier rebuilds the preimage with its own current head and
        // the signature fails. event_log_hash is the load-path-independent
        // (id-canonical) commitment, so the bound head is stable across the
        // packet / directory / git-per-file layouts.
        "parent_event_log_hash": parent_event_log_hash,
    });
    let body = canonical::to_canonical_bytes(&preimage)?;
    Ok(crate::signing_input::signing_input(
        crate::signing_input::SigVersion::V0,
        crate::signing_input::payload_type::ACCEPT,
        &body,
    ))
}

/// Canonical signing bytes for a new decision-bound detached acceptance.
///
/// This is additive to [`accept_preimage_bytes`], whose historical raw-v0
/// bytes remain unchanged. The v1 body binds the same exact accept facts plus
/// the private Decision Plan's `decision_root`, declares its body version, and
/// uses the shared DSSE/PAE v1 signing input. A root is a protocol-standard
/// `sha256:<64 lowercase hex>` digest; validation is shared with the signed
/// provenance input-ref lane used by local review events.
pub fn accept_preimage_bytes_v1(
    vfr_id: &str,
    proposal_id: &str,
    reviewer_id: &str,
    reason: &str,
    parent_event_log_hash: &str,
    decision_root: &str,
) -> Result<Vec<u8>, String> {
    crate::provenance::decision_root_input_ref(decision_root)?;
    let preimage = json!({
        "decision_preimage_version": crate::signing_input::ACCEPTANCE_DECISION_PREIMAGE_V1,
        "action": PROPOSAL_ACCEPT_ACTION,
        "vfr_id": vfr_id,
        "proposal_id": proposal_id,
        "reviewer_id": reviewer_id,
        "reason": reason,
        "parent_event_log_hash": parent_event_log_hash,
        "decision_root": decision_root,
    });
    let body = canonical::to_canonical_bytes(&preimage)?;
    Ok(crate::signing_input::decision_acceptance_signing_input(
        &body,
    ))
}

/// v0.128: protocol-side authority gate for the public accept boundary.
///
/// This closes the gap `publish_entry` leaves open:
/// open submission is fine because a self-signature *is* the bind, but a
/// reviewer **accept** must additionally prove the signer is a
/// registered, non-revoked actor on this frontier carrying reviewer
/// authority. Given an in-memory `Project` (the materialized frontier),
/// the signer's hex pubkey, a detached hex signature, the target
/// `proposal`, and the decision `reason`, this returns
/// `Ok(AcceptAuthorization)` **only if every** condition holds — in the
/// same fail-fast order the hub boundary enforces:
///
///   1. `reason` is non-empty (an accept must carry a decision reason).
///   2. AUTHORITY: `signer_pubkey` resolves to a registered
///      `ActorRecord` on `project.actors` (case-insensitive hex match).
///      No match → rejected (this is the check `publish_entry` lacks).
///   3. REVOCATION: the actor is not revoked as of `now`
///      (`ActorRecord::is_revoked_at`).
///   4. REVIEWER AUTHORITY: the actor carries reviewer authority
///      (`reviewer:` namespace, non-placeholder; the write `tier` does
///      NOT qualify).
///   5. SIGNATURE: the detached signature verifies (via the generic
///      `verify_action_signature`) over the canonical accept preimage
///      rebuilt with `reviewer_id = actor.id` — the *resolved* identity,
///      never client-supplied — binding (action, vfr_id, proposal_id,
///      reviewer_id, reason).
///
/// `vfr_id` is the frontier id from the boundary (the route path
/// parameter), bound into the signed preimage alongside the proposal so
/// a signature is non-transferable across frontiers. It is passed
/// explicitly rather than read from `proposal.target.id` because a
/// proposal targets a finding/artifact, not the frontier, and the
/// materialized `Project.frontier_id` may be `None` in a projection.
///
/// `now` is the caller's current RFC-3339 timestamp (e.g.
/// `Utc::now().to_rfc3339()`), injected so the revocation check is
/// testable and the function stays pure.
///
/// This function performs **no mutation** and does **not** weaken
/// `accept_proposal_in_frontier` or the Engine gate — it is purely the
/// per-reviewer-key authority predicate that must pass *before* the
/// caller runs the strict-mode canonical accept. The caller should use
/// the returned `actor.id` as the reviewer for that accept.
pub fn authorize_proposal_accept(
    project: &Project,
    vfr_id: &str,
    signer_pubkey_hex: &str,
    signature_hex: &str,
    proposal: &StateProposal,
    reason: &str,
    now: &str,
) -> Result<AcceptAuthorization, String> {
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }

    if project.frontier_id() != vfr_id {
        return Err(format!(
            "accept frontier binding mismatch: signed for {vfr_id}, loaded {}",
            project.frontier_id()
        ));
    }

    // 2. AUTHORITY: resolve the signer pubkey to a registered actor.
    let actor = project
        .actors
        .iter()
        .find(|a| a.public_key.eq_ignore_ascii_case(signer_pubkey_hex))
        .cloned()
        .ok_or_else(|| {
            format!("signer pubkey {signer_pubkey_hex} is not a registered actor on this frontier")
        })?;

    // 3. REVOCATION: reject a key revoked/retired as of now.
    if actor.is_revoked_at(now) {
        return Err(format!(
            "reviewer key for actor '{}' is revoked as of {now}",
            actor.id
        ));
    }

    // 4. REVIEWER AUTHORITY: the actor must carry reviewer authority. A
    //    write tier (auto-notes) never grants accept authority.
    if !actor_has_reviewer_authority(&actor) {
        return Err(format!(
            "actor '{}' does not carry reviewer authority (accept requires a non-placeholder \
             reviewer: identity; the write tier does not qualify)",
            actor.id
        ));
    }

    // 5. SIGNATURE: verify the detached decision signature over the
    //    canonical accept preimage, rebuilt with the *resolved*
    //    reviewer_id so a client cannot substitute a different identity,
    //    and with the head the accept is being applied against
    //    (ADR 0001 Phase 0d). `project` here is the PRE-accept state (the
    //    boundary loads the current frontier before applying), so its
    //    event_log_hash is exactly the head the reviewer must have signed
    //    over. A signature captured against a different head fails.
    let parent_event_log_hash = crate::events::event_log_hash(&project.events);
    let preimage = accept_preimage_bytes(
        vfr_id,
        &proposal.id,
        &actor.id,
        reason,
        &parent_event_log_hash,
    )?;
    let verified =
        crate::sign::verify_action_signature(&preimage, signature_hex, &actor.public_key)?;
    if !verified {
        return Err(format!(
            "accept signature does not verify for actor '{}' over the canonical accept preimage",
            actor.id
        ));
    }

    Ok(AcceptAuthorization {
        actor,
        frontier_id: vfr_id.to_string(),
        proposal_id: proposal.id.clone(),
        reason: reason.to_string(),
        parent_event_log_hash,
        decision_root: None,
    })
}

/// Decision-bound variant of [`authorize_proposal_accept`].
///
/// It runs the same authority, revocation, reviewer-role, and exact-head gates,
/// but verifies the additive v1 acceptance preimage that also commits to
/// `decision_root`. Callers creating new Decision Plan accepts use this seam;
/// historical callers keep the legacy function and remain byte-valid.
#[allow(clippy::too_many_arguments)]
pub fn authorize_proposal_accept_v1(
    project: &Project,
    vfr_id: &str,
    signer_pubkey_hex: &str,
    signature_hex: &str,
    proposal: &StateProposal,
    reason: &str,
    decision_root: &str,
    now: &str,
) -> Result<AcceptAuthorization, String> {
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }

    if project.frontier_id() != vfr_id {
        return Err(format!(
            "accept frontier binding mismatch: signed for {vfr_id}, loaded {}",
            project.frontier_id()
        ));
    }

    let mut matching_keys = project
        .actors
        .iter()
        .filter(|actor| actor.public_key.eq_ignore_ascii_case(signer_pubkey_hex));
    let actor_id = matching_keys
        .next()
        .map(|actor| actor.id.clone())
        .ok_or_else(|| {
            format!("signer pubkey {signer_pubkey_hex} is not a registered actor on this frontier")
        })?;
    if matching_keys.next().is_some() {
        return Err(format!(
            "signer pubkey {signer_pubkey_hex} resolves ambiguously on this frontier"
        ));
    }
    let actor = validate_human_reviewer_authority_at(project, &actor_id, now)?;

    let parent_event_log_hash = crate::events::event_log_hash(&project.events);
    let preimage = accept_preimage_bytes_v1(
        vfr_id,
        &proposal.id,
        &actor.id,
        reason,
        &parent_event_log_hash,
        decision_root,
    )?;
    let verified =
        crate::sign::verify_action_signature(&preimage, signature_hex, &actor.public_key)?;
    if !verified {
        return Err(format!(
            "decision-bound accept signature does not verify for actor '{}' over the canonical v1 accept preimage",
            actor.id
        ));
    }

    Ok(AcceptAuthorization {
        actor,
        frontier_id: vfr_id.to_string(),
        proposal_id: proposal.id.clone(),
        reason: reason.to_string(),
        parent_event_log_hash,
        decision_root: Some(decision_root.to_string()),
    })
}

pub fn summary(frontier: &Project) -> ProposalSummary {
    let mut out = ProposalSummary::default();
    let mut seen = BTreeSet::new();
    let finding_ids = frontier
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<BTreeSet<_>>();
    let artifact_ids = frontier
        .artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<BTreeSet<_>>();
    for proposal in &frontier.proposals {
        out.total += 1;
        *out.by_kind.entry(proposal.kind.clone()).or_default() += 1;
        match proposal.status.as_str() {
            "pending_review" => out.pending_review += 1,
            "accepted" => out.accepted += 1,
            "rejected" => out.rejected += 1,
            "applied" => out.applied += 1,
            _ => {}
        }
        if !seen.insert(proposal.id.clone()) {
            out.duplicate_ids.push(proposal.id.clone());
        }
        let target_known = match proposal.target.r#type.as_str() {
            "finding" => {
                proposal.kind == "finding.add" || finding_ids.contains(proposal.target.id.as_str())
            }
            "artifact" => {
                proposal.kind == "artifact.assert"
                    || artifact_ids.contains(proposal.target.id.as_str())
            }
            _ => true,
        };
        if !target_known {
            out.invalid_targets.push(proposal.target.id.clone());
        }
    }
    out.duplicate_ids.sort();
    out.duplicate_ids.dedup();
    out.invalid_targets.sort();
    out.invalid_targets.dedup();
    out
}

pub fn proposals_for_finding<'a>(
    frontier: &'a Project,
    finding_id: &str,
) -> Vec<&'a StateProposal> {
    frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.target.r#type == "finding" && proposal.target.id == finding_id)
        .collect()
}

/// Opaque proof that one exact signed proposal write was authorized against
/// one exact frontier registry snapshot. The fields are private so a caller
/// cannot turn a Boolean into authority; construction and use both verify the
/// registered Ed25519 signature and the bounded auto-apply tier.
#[derive(Debug, Clone)]
pub struct VerifiedProposalWrite {
    frontier_id: String,
    proposal_id: String,
    actor_id: String,
    actor_pubkey: String,
    proposal_signature: String,
    disposition: VerifiedProposalDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifiedProposalDisposition {
    Pending,
    AutoApplyNote,
}

/// Verify a detached proposal signature and mint a proposal-scoped write
/// capability. Auto-apply is deliberately limited to the historical
/// `auto-notes` process lane; a truth-bearing proposal can only be inserted as
/// pending and must later cross the human Decision Plan boundary.
pub fn authorize_signed_proposal_write(
    frontier: &Project,
    proposal: &StateProposal,
    signature_hex: &str,
    request_auto_apply: bool,
    observed_at: &str,
) -> Result<VerifiedProposalWrite, String> {
    chrono::DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("proposal-write observation time is invalid: {error}"))?;
    let signature = hex::decode(signature_hex)
        .map_err(|error| format!("invalid proposal signature hex: {error}"))?;
    if signature.len() != 64 {
        return Err("proposal signature must be 64 bytes".to_string());
    }
    let mut actors = frontier
        .actors
        .iter()
        .filter(|actor| actor.id == proposal.actor.id);
    let actor = actors.next().ok_or_else(|| {
        format!(
            "actor '{}' is not registered in this frontier; register it before writing",
            proposal.actor.id
        )
    })?;
    if actors.next().is_some() {
        return Err(format!(
            "actor '{}' is registered ambiguously (duplicate actor ids)",
            proposal.actor.id
        ));
    }
    if actor.algorithm != "ed25519"
        || hex::decode(&actor.public_key).map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(format!(
            "actor '{}' must have one registered Ed25519 write key",
            actor.id
        ));
    }
    if actor.is_revoked_at(observed_at) {
        return Err(format!(
            "actor '{}' is revoked as of {observed_at} and may not create a proposal",
            actor.id
        ));
    }
    let canonical_signature = hex::encode(signature);
    if !crate::sign::verify_proposal_signature(proposal, &canonical_signature, &actor.public_key)? {
        return Err(format!(
            "signature does not verify for actor '{}' on proposal {}",
            actor.id, proposal.id
        ));
    }
    let disposition = if request_auto_apply {
        if !crate::sign::actor_can_auto_apply(actor, &proposal.kind) {
            let tier = actor.tier.as_deref().unwrap_or("none");
            return Err(format!(
                "actor '{}' tier '{tier}' does not permit auto-applying {}",
                actor.id, proposal.kind
            ));
        }
        if proposal.kind != "finding.note" {
            return Err(format!(
                "signed proposal auto-apply is process-only; '{}' must remain pending for `vela sign`",
                proposal.kind
            ));
        }
        VerifiedProposalDisposition::AutoApplyNote
    } else {
        VerifiedProposalDisposition::Pending
    };
    Ok(VerifiedProposalWrite {
        frontier_id: frontier.frontier_id(),
        proposal_id: proposal.id.clone(),
        actor_id: actor.id.clone(),
        actor_pubkey: actor.public_key.clone(),
        proposal_signature: canonical_signature,
        disposition,
    })
}

fn revalidate_signed_proposal_write(
    frontier: &Project,
    proposal: &StateProposal,
    authority: &VerifiedProposalWrite,
) -> Result<(), String> {
    if frontier.frontier_id() != authority.frontier_id
        || proposal.id != authority.proposal_id
        || proposal.actor.id != authority.actor_id
    {
        return Err("verified proposal-write authority is bound to different bytes".to_string());
    }
    let mut actors = frontier
        .actors
        .iter()
        .filter(|actor| actor.id == authority.actor_id);
    let actor = actors
        .next()
        .ok_or_else(|| "verified proposal-write actor is no longer registered".to_string())?;
    if actors.next().is_some()
        || !actor
            .public_key
            .eq_ignore_ascii_case(&authority.actor_pubkey)
    {
        return Err("verified proposal-write actor registry binding changed".to_string());
    }
    if actor.revoked_at.is_some() {
        return Err("verified proposal-write actor is revoked".to_string());
    }
    if !crate::sign::verify_proposal_signature(
        proposal,
        &authority.proposal_signature,
        &actor.public_key,
    )? {
        return Err("verified proposal-write signature no longer matches".to_string());
    }
    if authority.disposition == VerifiedProposalDisposition::AutoApplyNote
        && (!crate::sign::actor_can_auto_apply(actor, &proposal.kind)
            || proposal.kind != "finding.note")
    {
        return Err("verified proposal-write auto-apply scope changed".to_string());
    }
    Ok(())
}

/// Phase P (v0.5): upsert by content address. If a proposal with the same
/// `vpr_…` already exists in the frontier, return the existing record instead
/// of inserting a duplicate. Combined with the `created_at`-free preimage,
/// this makes agent retries idempotent at the substrate level.
///
/// Decision creation is intentionally absent from this insertion primitive.
pub fn insert_pending_in_frontier(
    frontier: &mut Project,
    proposal: StateProposal,
) -> Result<CreateProposalResult, String> {
    let finding_id = proposal.target.id.clone();
    let proposal_id = proposal.id.clone();
    if let Some(existing) = frontier
        .proposals
        .iter()
        .find(|existing| existing.id == proposal_id)
    {
        return Ok(CreateProposalResult {
            proposal_id,
            finding_id,
            status: existing.status.clone(),
            applied_event_id: existing.applied_event_id.clone(),
        });
    }

    validate_new_proposal(frontier, &proposal)?;
    frontier.proposals.push(proposal);
    project::recompute_stats(frontier);
    Ok(CreateProposalResult {
        proposal_id,
        finding_id,
        status: "pending_review".to_string(),
        applied_event_id: None,
    })
}

pub fn create_or_apply(
    path: &Path,
    proposal: StateProposal,
    apply: bool,
) -> Result<CreateProposalResult, String> {
    if apply {
        return Err(
            "legacy create_or_apply(apply=true) is retired; insert the pending proposal, then use a Decision Plan or verified signed authority"
                .to_string(),
        );
    }
    let mut frontier = repo::load_from_path(path)?;
    let result = create_or_apply_in_frontier(&mut frontier, proposal, apply)?;
    repo::save_to_path(path, &frontier)?;
    Ok(result)
}

/// Apply the proposal mutation to an already loaded frontier without
/// persisting it. Transactional callers use this pure in-memory edge while
/// holding their repository-wide write barrier, then install the rendered
/// bytes through their durable transaction mechanism. The path wrapper above
/// remains for legacy single-writer CLI surfaces.
pub fn create_or_apply_in_frontier(
    frontier: &mut Project,
    proposal: StateProposal,
    apply: bool,
) -> Result<CreateProposalResult, String> {
    if apply {
        return Err(
            "legacy create_or_apply_in_frontier(apply=true) is retired; use a typed verified write authority or a Decision Plan"
                .to_string(),
        );
    }
    insert_pending_in_frontier(frontier, proposal)
}

/// Insert one cryptographically verified proposal and, only when the opaque
/// authority carries the bounded `auto-notes` capability, apply that
/// non-truth-bearing note. No Boolean crosses this trust edge.
pub fn create_with_verified_proposal_write_in_frontier(
    frontier: &mut Project,
    proposal: StateProposal,
    authority: &VerifiedProposalWrite,
) -> Result<CreateProposalResult, String> {
    revalidate_signed_proposal_write(frontier, &proposal, authority)?;
    let finding_id = proposal.target.id.clone();
    let proposal_id = proposal.id.clone();
    let existing_idx = frontier
        .proposals
        .iter()
        .position(|existing| existing.id == proposal_id);
    if existing_idx.is_none() {
        insert_pending_in_frontier(frontier, proposal)?;
    }
    if authority.disposition == VerifiedProposalDisposition::Pending {
        return Ok(CreateProposalResult {
            proposal_id,
            finding_id,
            status: frontier
                .proposals
                .iter()
                .find(|proposal| proposal.id == authority.proposal_id)
                .map(|proposal| proposal.status.clone())
                .unwrap_or_else(|| "pending_review".to_string()),
            applied_event_id: existing_idx
                .and_then(|index| frontier.proposals[index].applied_event_id.clone()),
        });
    }
    if let Some(index) = existing_idx
        && let Some(event_id) = frontier.proposals[index].applied_event_id.clone()
    {
        return Ok(CreateProposalResult {
            proposal_id,
            finding_id,
            status: "applied".to_string(),
            applied_event_id: Some(event_id),
        });
    }
    let event_id = apply_verified_process_proposal_in_frontier(frontier, authority)?;
    crate::sources::materialize_project(frontier);
    Ok(CreateProposalResult {
        proposal_id,
        finding_id,
        status: "applied".to_string(),
        applied_event_id: Some(event_id),
    })
}

fn apply_verified_process_proposal_in_frontier(
    frontier: &mut Project,
    authority: &VerifiedProposalWrite,
) -> Result<String, String> {
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == authority.proposal_id)
        .cloned()
        .ok_or_else(|| format!("Proposal not found: {}", authority.proposal_id))?;
    accept_proposal_in_frontier_with_authority_at(
        frontier,
        &proposal.id,
        &authority.actor_id,
        &proposal.reason,
        DecisionAuthority::VerifiedProcess(authority),
        None,
        None,
        false,
    )
}

pub fn list(frontier: &Project, status: Option<&str>) -> Vec<StateProposal> {
    let mut proposals = frontier
        .proposals
        .iter()
        .filter(|proposal| status.is_none_or(|wanted| proposal.status == wanted))
        .cloned()
        .collect::<Vec<_>>();
    proposals.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    proposals
}

pub fn show<'a>(frontier: &'a Project, proposal_id: &str) -> Result<&'a StateProposal, String> {
    frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))
}

pub fn preview_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
) -> Result<ProposalPreview, String> {
    validate_reviewer_identity(reviewer)?;
    let frontier = repo::load_from_path(path)?;
    preview_in_frontier(&frontier, proposal_id, reviewer)
}

pub fn preview_in_frontier(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
) -> Result<ProposalPreview, String> {
    validate_reviewer_identity(reviewer)?;
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?
        .clone();
    if proposal.status == "applied" {
        let applied_event_id = proposal
            .applied_event_id
            .clone()
            .ok_or_else(|| format!("Proposal {} is applied but has no event id", proposal.id))?;
        return Ok(ProposalPreview {
            proposal_id: proposal.id,
            kind: proposal.kind,
            changed_findings: changed_targets_for_type(frontier, &proposal.target, "finding"),
            changed_finding_details: Vec::new(),
            changed_artifacts: changed_targets_for_type(frontier, &proposal.target, "artifact"),
            new_event_ids: vec![applied_event_id.clone()],
            event_kinds: frontier
                .events
                .iter()
                .find(|event| event.id == applied_event_id)
                .map(|event| vec![event.kind.to_string()])
                .unwrap_or_default(),
            target: proposal.target,
            reviewer: reviewer.to_string(),
            findings_before: frontier.findings.len(),
            findings_after: frontier.findings.len(),
            findings_delta: 0,
            artifacts_before: frontier.artifacts.len(),
            artifacts_after: frontier.artifacts.len(),
            artifacts_delta: 0,
            events_before: frontier.events.len(),
            events_after: frontier.events.len(),
            events_delta: 0,
            proof_would_be_stale: false,
            applied_event_id,
        });
    }
    if !matches!(proposal.status.as_str(), "pending_review" | "accepted") {
        return Err(format!(
            "Proposal {} cannot be previewed from status {}",
            proposal.id, proposal.status
        ));
    }
    let mut preview_state: Project = serde_json::from_value(
        serde_json::to_value(frontier).map_err(|e| format!("serialize frontier preview: {e}"))?,
    )
    .map_err(|e| format!("clone frontier preview: {e}"))?;
    let finding_ids_before = preview_state
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    let artifact_ids_before = preview_state
        .artifacts
        .iter()
        .map(|artifact| artifact.id.clone())
        .collect::<BTreeSet<_>>();
    let findings_before = preview_state.findings.len();
    let artifacts_before = preview_state.artifacts.len();
    let events_before = preview_state.events.len();
    let event_id = apply_proposal(
        &mut preview_state,
        &proposal,
        reviewer,
        "Preview proposal application",
        None,
    )?;
    let findings_after = preview_state.findings.len();
    let artifacts_after = preview_state.artifacts.len();
    let events_after = preview_state.events.len();
    let new_events = preview_state
        .events
        .iter()
        .skip(events_before)
        .cloned()
        .collect::<Vec<_>>();
    let changed_findings = changed_finding_ids(&preview_state, &finding_ids_before, &new_events);
    let changed_finding_details =
        build_changed_finding_details(frontier, &preview_state, &changed_findings);
    Ok(ProposalPreview {
        proposal_id: proposal.id,
        kind: proposal.kind,
        target: proposal.target,
        reviewer: reviewer.to_string(),
        changed_findings,
        changed_finding_details,
        changed_artifacts: changed_artifact_ids(&preview_state, &artifact_ids_before, &new_events),
        new_event_ids: new_events.iter().map(|event| event.id.clone()).collect(),
        event_kinds: new_events
            .iter()
            .map(|event| event.kind.to_string())
            .collect(),
        findings_before,
        findings_after,
        findings_delta: findings_after as isize - findings_before as isize,
        artifacts_before,
        artifacts_after,
        artifacts_delta: artifacts_after as isize - artifacts_before as isize,
        events_before,
        events_after,
        events_delta: events_after as isize - events_before as isize,
        proof_would_be_stale: true,
        applied_event_id: event_id,
    })
}

fn changed_targets_for_type(
    frontier: &Project,
    target: &StateTarget,
    target_type: &str,
) -> Vec<String> {
    let known = match target_type {
        "finding" => frontier
            .findings
            .iter()
            .any(|finding| finding.id == target.id),
        "artifact" => frontier
            .artifacts
            .iter()
            .any(|artifact| artifact.id == target.id),
        _ => false,
    };
    if target.r#type == target_type && known {
        vec![target.id.clone()]
    } else {
        Vec::new()
    }
}

fn changed_finding_ids(
    preview_state: &Project,
    finding_ids_before: &BTreeSet<String>,
    new_events: &[StateEvent],
) -> Vec<String> {
    let mut ids = preview_state
        .findings
        .iter()
        .filter(|finding| !finding_ids_before.contains(&finding.id))
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    for event in new_events {
        if event.target.r#type == "finding" {
            ids.insert(event.target.id.clone());
        }
    }
    ids.into_iter().collect()
}

fn changed_artifact_ids(
    preview_state: &Project,
    artifact_ids_before: &BTreeSet<String>,
    new_events: &[StateEvent],
) -> Vec<String> {
    let mut ids = preview_state
        .artifacts
        .iter()
        .filter(|artifact| !artifact_ids_before.contains(&artifact.id))
        .map(|artifact| artifact.id.clone())
        .collect::<BTreeSet<_>>();
    for event in new_events {
        if event.target.r#type == "artifact" {
            ids.insert(event.target.id.clone());
        }
    }
    ids.into_iter().collect()
}

pub fn import_from_path(path: &Path, source: &Path) -> Result<ImportProposalReport, String> {
    let proposals = load_proposals(source)?;
    // A proposal record is not a decision certificate. In particular,
    // `status`, `reviewed_by`, and `applied_event_id` are ordinary imported
    // bytes and cannot substitute for a verified signed event or policy
    // authority. This importer therefore accepts only pristine pending
    // records. A future decided-state importer must verify and replay the
    // corresponding authority/event chain as a separate protocol operation.
    for proposal in &proposals {
        let carries_decision_state = proposal.status != "pending_review"
            || proposal.reviewed_by.is_some()
            || proposal.reviewed_at.is_some()
            || proposal.decision_reason.is_some()
            || proposal.applied_event_id.is_some();
        if carries_decision_state {
            return Err(format!(
                "Proposal {} cannot be imported: `vela proposals import` accepts pristine pending_review records only; decided records require a separately verified signed-authority/event import",
                proposal.id
            ));
        }
    }

    let mut frontier = repo::load_from_path(path)?;
    let wrote_to = path.display().to_string();
    let mut report = ImportProposalReport {
        wrote_to,
        ..ImportProposalReport::default()
    };
    for proposal in proposals {
        if frontier
            .proposals
            .iter()
            .any(|existing| existing.id == proposal.id)
        {
            report.duplicates += 1;
            continue;
        }
        validate_new_proposal(&frontier, &proposal)?;
        frontier.proposals.push(proposal);
        report.imported += 1;
    }
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(report)
}

pub fn validate_source(source: &Path) -> Result<ProposalValidationReport, String> {
    let proposals = load_proposals(source)?;
    let mut report = ProposalValidationReport {
        checked: proposals.len(),
        ..ProposalValidationReport::default()
    };
    let scratch = project::assemble("proposal-validation", Vec::new(), 0, 0, "validate");
    let mut seen = BTreeSet::new();
    for proposal in proposals {
        if !seen.insert(proposal.id.clone()) {
            report.invalid += 1;
            report
                .errors
                .push(format!("Duplicate proposal id {}", proposal.id));
            continue;
        }
        report.proposal_ids.push(proposal.id.clone());
        match validate_standalone_proposal(&scratch, &proposal) {
            Ok(()) => report.valid += 1,
            Err(err) => {
                report.invalid += 1;
                report.errors.push(format!("{}: {}", proposal.id, err));
            }
        }
    }
    report.ok = report.invalid == 0;
    Ok(report)
}

pub fn export_to_path(
    frontier_path: &Path,
    output: &Path,
    status: Option<&str>,
) -> Result<usize, String> {
    let frontier = repo::load_from_path(frontier_path)?;
    let proposals = list(&frontier, status);
    let json = serde_json::to_string_pretty(&proposals)
        .map_err(|e| format!("Failed to serialize proposals for export: {e}"))?;
    std::fs::write(output, json).map_err(|e| {
        format!(
            "Failed to write proposal export '{}': {e}",
            output.display()
        )
    })?;
    Ok(proposals.len())
}

fn ensure_legacy_decision_creation_disabled(surface: &str) -> Result<(), String> {
    Err(format!(
        "{surface} is retired for new decisions; use the decision-root-bound Decision Plan ceremony (`vela sign`)"
    ))
}

pub fn accept_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
) -> Result<String, String> {
    accept_at_path_signed(path, proposal_id, reviewer, reason, None)
}

pub fn accept_at_path_signed(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
) -> Result<String, String> {
    ensure_legacy_decision_creation_disabled("accept_at_path_signed")?;
    let mut frontier = repo::load_from_path(path)?;
    let event_id = accept_proposal_in_frontier_signed(
        &mut frontier,
        proposal_id,
        reviewer,
        reason,
        signing_key,
    )?;
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(event_id)
}

/// How the Engine treats the Evidence CI delta a candidate acceptance
/// introduces.
#[derive(Debug, Clone, Default)]
pub struct AcceptOptions {
    /// Also block on *new* review warnings, not just new release-blocking
    /// failures. Off by default — Evidence CI is review-readiness, not a
    /// truth oracle, so warnings inform by default and gate only on demand.
    pub strict: bool,
    /// Override the gate. The override is recorded in the proposal's
    /// decision reason so the forced acceptance is auditable.
    pub force: bool,
    /// The reviewer's Ed25519 private key. REQUIRED when the reviewer is
    /// registered with a public key (key custody is the accept
    /// authority); the accept event is signed with it.
    pub signing_key: Option<ed25519_dalek::SigningKey>,
    /// Opaque, exact-head authority minted by
    /// [`authorize_proposal_accept`]. A detached boundary may use this
    /// instead of passing a private key; callers cannot construct it from a
    /// Boolean or reviewer string.
    pub verified_authority: Option<AcceptAuthorization>,
    /// Co-authorship attribution for the signed decision event: the non-human
    /// (AI / CI) that contributed. The reviewer remains the accountable signer;
    /// this is signed-over data with no authority. `None` keeps the event
    /// byte-identical to the pre-redesign shape.
    pub provenance: Option<crate::provenance::Provenance>,
}

/// The Engine's read on an acceptance: what Evidence CI says about the
/// state the change would produce. Recomputable at any time from
/// `evidence_ci::run_project`; this captures the *delta* a single
/// acceptance introduces, which is what a reviewer (or the gate) acts on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineVerdict {
    /// `pass` (clean), `warn` (new review warnings), `blocked` (would be
    /// gated; only seen on the preview/error path), or `forced` (gated but
    /// overridden with --force and persisted).
    pub status: String,
    /// Release-blocking checks newly failing because of this change.
    pub new_blocking: Vec<String>,
    /// Review-readiness warnings this change introduces.
    pub new_warnings: Vec<String>,
    /// Whether a gate was overridden with --force.
    pub forced: bool,
    /// Whether warnings were treated as blocking (--strict).
    pub strict: bool,
    /// Post-accept Evidence CI counts, for context in the readout.
    pub release_blocking_failed: usize,
    pub warnings: usize,
}

/// The acceptance result plus the Engine verdict that gated it.
#[derive(Debug, Clone)]
pub struct AcceptOutcome {
    pub event_id: String,
    pub verdict: EngineVerdict,
}

fn authority_from_accept_options(
    opts: &AcceptOptions,
    preview_only: bool,
) -> Result<DecisionAuthority<'_>, String> {
    match (&opts.signing_key, &opts.verified_authority) {
        (Some(_), Some(_)) => Err(
            "accept options must carry exactly one authority: local key or verified detached proof"
                .to_string(),
        ),
        (Some(key), None) => Ok(DecisionAuthority::LocalKey(key)),
        (None, Some(verified)) => Ok(DecisionAuthority::DetachedAccept(verified)),
        (None, None) if preview_only => Ok(DecisionAuthority::Preview),
        (None, None) => Err(
            "accept requires a registered human key, verified detached authority, or the Decision Plan ceremony"
                .to_string(),
        ),
    }
}

/// A proposal kind is truth-bearing when accepting it changes what the
/// frontier asserts about the world. Process/provenance records and
/// mechanical repairs are not — this mirrors the bounded safe set the
/// agent self-accept policy already trusts, keeping the Engine gate and
/// that policy consistent.
pub fn is_truth_bearing_kind(kind: &str) -> bool {
    !(PROCESS_PROVENANCE_KINDS.contains(&kind) || MECHANICAL_REPAIR_KINDS.contains(&kind))
}

/// Evaluate one already-prepared candidate transaction under the strict Engine
/// gate without applying proposals again.
///
/// Evidence CI is computed exactly once before and once after. New release
/// failures or review warnings block only when at least one accepted proposal
/// kind is truth-bearing; rejects and mechanical/process accepts remain visible
/// in the counts but cannot turn Evidence CI into a truth oracle. The function
/// mutates nothing and performs no path access beyond Evidence CI's bounded
/// verifier observations.
pub fn strict_engine_verdict_for_candidate(
    original: &Project,
    candidate: &Project,
    frontier_path: &Path,
    accepted_kinds: &[String],
) -> EngineVerdict {
    let before = crate::evidence_ci::run_project(original, frontier_path);
    let after = crate::evidence_ci::run_project(candidate, frontier_path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warnings = crate::evidence_ci::review_warnings(&before);
    let new_blocking = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect::<Vec<_>>();
    let new_warnings = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warnings)
        .cloned()
        .collect::<Vec<_>>();
    let gates = accepted_kinds
        .iter()
        .any(|kind| is_truth_bearing_kind(kind));
    let status = if gates && (!new_blocking.is_empty() || !new_warnings.is_empty()) {
        "blocked"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced: false,
        strict: true,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    }
}

/// Accept a proposal under the Engine: run Evidence CI on the current and
/// the post-accept state, and gate the acceptance on the *regression* the
/// change introduces. A truth-bearing claim that adds a new release-
/// blocking failure (or, under `--strict`, a new review warning) is
/// blocked unless `--force` is given. The accepted event is itself the
/// record that the gate passed — Evidence CI is a recomputable projection,
/// so the verdict is surfaced, not persisted as a separate canonical object.
pub fn accept_at_path_engine(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    opts: AcceptOptions,
) -> Result<AcceptOutcome, String> {
    ensure_legacy_decision_creation_disabled("accept_at_path_engine")?;
    let mut frontier = repo::load_from_path(path)?;

    // The kind decides whether the gate applies; read it before the apply
    // mutates proposal status.
    let kind = frontier
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .map(|p| p.kind.clone())
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;

    let before = crate::evidence_ci::run_project(&frontier, path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);

    // Apply via the sole canonical path; this mutates `frontier` in memory.
    let authority = authority_from_accept_options(&opts, false)?;
    let event_id = accept_proposal_in_frontier_with_authority_at(
        &mut frontier,
        proposal_id,
        reviewer,
        reason,
        authority,
        opts.provenance.as_ref(),
        None,
        false,
    )?;

    let after = crate::evidence_ci::run_project(&frontier, path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();

    let truth_bearing = is_truth_bearing_kind(&kind);
    let would_block =
        truth_bearing && (!new_blocking.is_empty() || (opts.strict && !new_warnings.is_empty()));

    if would_block && !opts.force {
        // Gate: return without saving. The in-memory mutation is discarded,
        // so no canonical state changes on a blocked accept.
        let why = if !new_blocking.is_empty() {
            format!(
                "introduces {} new release-blocking failure(s): {}",
                new_blocking.len(),
                new_blocking.join(", ")
            )
        } else {
            format!(
                "--strict: introduces {} new review warning(s): {}",
                new_warnings.len(),
                new_warnings.join(", ")
            )
        };
        return Err(format!(
            "Engine gate blocked accept of {proposal_id}: {why}. Re-run with --force to \
             override (the override is recorded in the decision reason), or resolve the checks first."
        ));
    }

    let forced = would_block && opts.force;
    if forced {
        // Make the override auditable in the persisted proposal record.
        if let Some(p) = frontier.proposals.iter_mut().find(|p| p.id == proposal_id) {
            let note = format!(
                " [engine: --force past {} new blocking / {} new warning(s)]",
                new_blocking.len(),
                new_warnings.len()
            );
            p.decision_reason = Some(match p.decision_reason.take() {
                Some(r) => format!("{r}{note}"),
                None => note.trim_start().to_string(),
            });
        }
    }

    let status = if forced {
        "forced"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    let verdict = EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced,
        strict: opts.strict,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    };

    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(AcceptOutcome { event_id, verdict })
}

/// What a batch acceptance did. Counts are over the whole batch; the
/// single `verdict` is the Engine's read on the *aggregate* delta the
/// batch introduces (one CI pair for the whole batch, not one per
/// proposal — that is the entire scale point). `event_ids` is parallel to
/// `accepted_proposal_ids`.
#[derive(Debug, Clone)]
pub struct BatchAcceptReport {
    /// Proposals applied this run (excludes ones that were already
    /// applied — those are counted in `already_applied`).
    pub accepted_proposal_ids: Vec<String>,
    /// The canonical event id each accepted proposal produced.
    pub event_ids: Vec<String>,
    /// Proposals that `accept_proposal_in_frontier` short-circuited
    /// because they were already applied (idempotent re-accept).
    pub already_applied: usize,
    /// Proposals that failed to apply (id, error) — e.g. not found, or a
    /// reducer/validation error. A per-proposal failure does NOT abort the
    /// batch; the failures are reported and the rest proceed.
    pub failed: Vec<(String, String)>,
    /// The aggregate Engine verdict over the whole batch.
    pub verdict: EngineVerdict,
    /// True when the batch was gated (would_block && !force): nothing was
    /// persisted, the in-memory mutation discarded. The accepted/event
    /// lists reflect what *would* have applied so the caller can report it.
    pub gated: bool,
    /// True when this was a preview (`dry_run`): the batch applied + gated
    /// in memory, the verdict is real, but nothing was written to disk.
    pub dry_run: bool,
}

/// Accept a *batch* of proposals against an on-disk frontier in a single
/// load → apply-all → save pass. This is the scale-capable accept path.
///
/// The single-proposal [`accept_at_path_engine`] does load-whole →
/// accept-one → run Evidence CI (whole frontier) → save-whole on every
/// call. Accepting `N` proposals that way is `N` loads, `N` CI runs, and
/// `N` whole-frontier re-serializations — O(N²) work for an O(N) logical
/// change, the exact failure the scale-architecture plan names. This runs
/// the frontier once: load once, CI once before, apply every proposal in
/// memory (the sole canonical [`accept_proposal_in_frontier`] path, no
/// per-accept CI), CI once after, gate on the *aggregate* delta, and
/// `save_to_path` once.
///
/// Gate semantics mirror the single accept but at batch granularity: the
/// batch is all-or-nothing at the Engine gate. If the batch as a whole
/// introduces a new release-blocking failure (or, under `--strict`, a new
/// review warning) and `force` is not set, NOTHING is persisted — the
/// in-memory mutation is discarded exactly as a blocked single accept
/// discards its. `force` overrides and records the override note on each
/// forced proposal. Per-proposal *apply* failures (not-found, reducer
/// error) are collected in the report and do not abort the rest.
///
/// `dry_run` does everything except the final save: a real preview of the
/// aggregate verdict and the accept/skip/fail breakdown with zero
/// on-disk effect.
///
/// Note on persistence cost: this still calls `repo::save_to_path` once,
/// which re-serializes the frontier a single time — correct and O(frontier)
/// for the bulk case (the whole spine into a small frontier). Driving the
/// persist through `incremental_ingest::append_batch` (write only the new
/// finding/event files, touch only the flipped proposal files) is the
/// next layer, for appending a small batch into an already-large frontier;
/// it is intentionally not coupled here because `save_to_path` also
/// reconciles proposal-status, proof-state, and stats that the append
/// primitive deliberately leaves alone.
pub fn accept_batch_at_path(
    path: &Path,
    proposal_ids: &[String],
    reviewer: &str,
    reason: &str,
    opts: AcceptOptions,
    dry_run: bool,
) -> Result<BatchAcceptReport, String> {
    if !dry_run {
        ensure_legacy_decision_creation_disabled("accept_batch_at_path")?;
    }
    let mut frontier = repo::load_from_path(path)?;

    // CI once, before any apply.
    let before = crate::evidence_ci::run_project(&frontier, path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);

    // Apply every proposal in memory via the sole canonical path. No
    // per-accept CI and no save here — that is what makes the batch O(N).
    let mut accepted_proposal_ids = Vec::new();
    let mut event_ids = Vec::new();
    let mut failed = Vec::new();
    let mut already_applied = 0usize;
    let mut any_truth_bearing = false;
    for pid in proposal_ids {
        // Read the kind + prior applied state before the apply mutates it.
        let (kind, was_applied) = match frontier.proposals.iter().find(|p| &p.id == pid) {
            Some(p) => (p.kind.clone(), p.applied_event_id.is_some()),
            None => {
                failed.push((pid.clone(), format!("Proposal not found: {pid}")));
                continue;
            }
        };
        // The batch honors the session's single key read: each accept is
        // signed with `opts.signing_key` (the interactive `vela sign`
        // resolves the reviewer's identity key once and threads it here).
        // A key-registered reviewer whose key is absent still fails the
        // custody check inside — keyless bulk acceptance under a typed name
        // is exactly what key custody exists to prevent.
        let authority = match authority_from_accept_options(&opts, dry_run) {
            Ok(authority) => authority,
            Err(error) => {
                failed.push((pid.clone(), error));
                continue;
            }
        };
        match accept_proposal_in_frontier_with_authority_at(
            &mut frontier,
            pid,
            reviewer,
            reason,
            authority,
            opts.provenance.as_ref(),
            None,
            false,
        ) {
            Ok(event_id) => {
                if was_applied {
                    already_applied += 1;
                } else {
                    if is_truth_bearing_kind(&kind) {
                        any_truth_bearing = true;
                    }
                    accepted_proposal_ids.push(pid.clone());
                    event_ids.push(event_id);
                }
            }
            Err(e) => failed.push((pid.clone(), e)),
        }
    }

    // CI once, after all applies. The gate acts on the aggregate delta.
    let after = crate::evidence_ci::run_project(&frontier, path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();

    let would_block = any_truth_bearing
        && (!new_blocking.is_empty() || (opts.strict && !new_warnings.is_empty()));

    if would_block && !opts.force {
        // Discard: never save. Report what would have applied + why blocked.
        let verdict = EngineVerdict {
            status: "blocked".to_string(),
            new_blocking,
            new_warnings,
            forced: false,
            strict: opts.strict,
            release_blocking_failed: after.summary.release_blocking_failed,
            warnings: after.summary.warnings,
        };
        return Ok(BatchAcceptReport {
            accepted_proposal_ids,
            event_ids,
            already_applied,
            failed,
            verdict,
            gated: true,
            dry_run,
        });
    }

    let forced = would_block && opts.force;
    if forced {
        // Record the override on each forced proposal so it stays auditable.
        let note = format!(
            " [engine: batch --force past {} new blocking / {} new warning(s)]",
            new_blocking.len(),
            new_warnings.len()
        );
        for pid in &accepted_proposal_ids {
            if let Some(p) = frontier.proposals.iter_mut().find(|p| &p.id == pid) {
                p.decision_reason = Some(match p.decision_reason.take() {
                    Some(r) => format!("{r}{note}"),
                    None => note.trim_start().to_string(),
                });
            }
        }
    }

    let status = if forced {
        "forced"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    let verdict = EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced,
        strict: opts.strict,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    };

    if !dry_run {
        project::recompute_stats(&mut frontier);
        repo::save_to_path(path, &frontier)?;
        // A batch of in-memory accepts leaves the visible snapshot + lock
        // computed from the *accumulated in-memory* order, which can diverge
        // from a fresh canonical load+materialize (the load-time
        // `materialize_*_from_events` re-derivation differs from incremental
        // accumulation). Reseal from disk so `frontier.json`/`vela.lock`
        // match the materialized state and `vela integrity` stays clean. The
        // single-accept path gets this for free by reloading per accept; the
        // batch pays one canonical reseal at the end. Only meaningful for a
        // split `.vela/` repo — a flat project-file has no lock to reseal.
        if path.join(".vela").is_dir() {
            crate::frontier_repo::materialize(path)?;
        }
    }

    Ok(BatchAcceptReport {
        accepted_proposal_ids,
        event_ids,
        already_applied,
        failed,
        verdict,
        gated: false,
        dry_run,
    })
}

/// v0.128: in-memory twin of [`accept_at_path_engine`] for callers that
/// hold a materialized `Project` rather than an on-disk frontier — the
/// public accept boundary on the hub being the motivating case. Same
/// strict-gate contract, byte-for-byte the same kernel calls
/// (`evidence_ci::run_project` before/after the sole canonical
/// `accept_proposal_in_frontier` path, gated on the *regression*), but it
/// mutates `frontier` in place and never touches the filesystem. The
/// caller persists the mutated project (and the emitted canonical event)
/// itself, under whatever transaction it owns.
///
/// On a blocked gate the function returns [`Err(EngineGateBlocked)`] and
/// the caller MUST discard the (now partially-mutated) `frontier` without
/// persisting it — exactly as `accept_at_path_engine` discards its
/// in-memory mutation by never calling `save_to_path`. `Project` is
/// deliberately not `Clone`, so the discard is the caller's
/// responsibility: the hub loads a throwaway `Project` for the accept and
/// only writes it back on `Ok`. The returned [`EngineGateBlocked`]
/// carries the full [`EngineVerdict`] so the boundary can surface it
/// (e.g. as a 422 body) without re-running CI.
///
/// `frontier_path` is read solely for static policy documents and to
/// label the Evidence CI report (see [`evidence_ci::run_project`]); the
/// hub passes a non-existent path, which surfaces a missing-policy check
/// rather than reading any frontier state from disk.
pub fn accept_in_frontier_engine(
    frontier: &mut Project,
    frontier_path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    opts: AcceptOptions,
) -> Result<AcceptOutcome, AcceptEngineError> {
    ensure_legacy_decision_creation_disabled("accept_in_frontier_engine")
        .map_err(AcceptEngineError::Failed)?;
    let kind = frontier
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .map(|p| p.kind.clone())
        .ok_or_else(|| AcceptEngineError::Failed(format!("Proposal not found: {proposal_id}")))?;

    let before = crate::evidence_ci::run_project(frontier, frontier_path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);

    // Apply via the sole canonical path; this mutates `frontier` in memory.
    // `accept_proposal_in_frontier` short-circuits to the existing
    // applied_event_id when the proposal is already applied, so a
    // re-accept is a no-op that returns the original event id (the
    // before/after Evidence CI delta is empty, the gate passes, and the
    // caller re-persists deterministically).
    let authority =
        authority_from_accept_options(&opts, false).map_err(AcceptEngineError::Failed)?;
    let event_id = accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        authority,
        opts.provenance.as_ref(),
        None,
        false,
    )
    .map_err(AcceptEngineError::Failed)?;

    let after = crate::evidence_ci::run_project(frontier, frontier_path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();

    let truth_bearing = is_truth_bearing_kind(&kind);
    let would_block =
        truth_bearing && (!new_blocking.is_empty() || (opts.strict && !new_warnings.is_empty()));

    if would_block && !opts.force {
        // Gate: the caller discards `frontier` (never persisted), so no
        // canonical state changes on a blocked accept. We surface the
        // verdict so the boundary can return it verbatim.
        let verdict = EngineVerdict {
            status: "blocked".to_string(),
            new_blocking,
            new_warnings,
            forced: false,
            strict: opts.strict,
            release_blocking_failed: after.summary.release_blocking_failed,
            warnings: after.summary.warnings,
        };
        return Err(AcceptEngineError::Blocked(Box::new(verdict)));
    }

    let forced = would_block && opts.force;
    if forced && let Some(p) = frontier.proposals.iter_mut().find(|p| p.id == proposal_id) {
        let note = format!(
            " [engine: --force past {} new blocking / {} new warning(s)]",
            new_blocking.len(),
            new_warnings.len()
        );
        p.decision_reason = Some(match p.decision_reason.take() {
            Some(r) => format!("{r}{note}"),
            None => note.trim_start().to_string(),
        });
    }

    let status = if forced {
        "forced"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    let verdict = EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced,
        strict: opts.strict,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    };

    project::recompute_stats(frontier);
    Ok(AcceptOutcome { event_id, verdict })
}

/// v0.128: error outcome of [`accept_in_frontier_engine`]. `Blocked`
/// distinguishes the strict-gate refusal (which carries the full
/// [`EngineVerdict`] for the boundary to surface as a 422) from any other
/// `Failed` accept error (proposal-not-found, validation, etc.) the
/// caller maps to a different status. Boxed so the `Ok` variant of the
/// surrounding `Result` stays small.
#[derive(Debug, Clone)]
pub enum AcceptEngineError {
    /// The strict Engine gate refused the accept; no state should be
    /// persisted. Carries the verdict that explains the regression.
    Blocked(Box<EngineVerdict>),
    /// Any other accept failure (not the gate).
    Failed(String),
}

impl std::fmt::Display for AcceptEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked(v) => write!(
                f,
                "Engine gate blocked accept: {} new blocking, {} new warning(s)",
                v.new_blocking.len(),
                v.new_warnings.len()
            ),
            Self::Failed(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AcceptEngineError {}

/// Compute the Engine verdict a candidate acceptance *would* produce,
/// without persisting anything. Drives the review-time preview ("what
/// would CI say if I accept this?") on the CLI and the Workbench.
pub fn preview_engine_verdict(path: &Path, proposal_id: &str) -> Result<EngineVerdict, String> {
    let frontier = repo::load_from_path(path)?;
    preview_engine_verdict_in_frontier(&frontier, path, proposal_id, false)
}

/// Pure Engine preview over an already staged frontier projection.
///
/// Submission uses this before policy routing so the exact gate result can be
/// committed to its private transaction plan even when the proposal has not
/// yet been installed on disk. The clone is serialized rather than relying on
/// a partial hand-maintained projection of [`Project`].
pub fn preview_engine_verdict_in_frontier(
    frontier: &Project,
    path: &Path,
    proposal_id: &str,
    strict: bool,
) -> Result<EngineVerdict, String> {
    let kind = frontier
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .map(|p| p.kind.clone())
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;

    let before = crate::evidence_ci::run_project(frontier, path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);

    // Apply on this in-memory copy under a synthetic reviewer; never saved.
    let encoded = serde_json::to_value(frontier).map_err(|error| error.to_string())?;
    let mut candidate: Project =
        serde_json::from_value(encoded).map_err(|error| error.to_string())?;
    accept_proposal_in_frontier_with_authority_at(
        &mut candidate,
        proposal_id,
        "reviewer:engine-preview",
        "engine ci preview",
        DecisionAuthority::Preview,
        None,
        None,
        false,
    )?;

    let after = crate::evidence_ci::run_project(&candidate, path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();

    let status = if is_truth_bearing_kind(&kind)
        && (!new_blocking.is_empty() || (strict && !new_warnings.is_empty()))
    {
        "blocked"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    Ok(EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced: false,
        strict,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    })
}

pub fn reject_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
) -> Result<(), String> {
    reject_at_path_signed(path, proposal_id, reviewer, reason, None)
}

/// Reject a proposal, signing the resulting `review.rejected` event under
/// the reviewer key when supplied. Mirrors `accept_at_path_signed`: if the
/// reviewer is registered with a pubkey, the key is required.
pub fn reject_at_path_signed(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
) -> Result<(), String> {
    ensure_legacy_decision_creation_disabled("reject_at_path_signed")?;
    let mut frontier = repo::load_from_path(path)?;
    reject_proposal_in_frontier_signed(&mut frontier, proposal_id, reviewer, reason, signing_key)?;
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(())
}

pub fn record_proof_export(frontier: &mut Project, record: ProofPacketRecord) {
    frontier.proof_state.latest_packet = ProofPacketState {
        generated_at: Some(record.generated_at),
        snapshot_hash: Some(record.snapshot_hash),
        event_log_hash: Some(record.event_log_hash),
        packet_manifest_hash: Some(record.packet_manifest_hash),
        status: "current".to_string(),
    };
    frontier.proof_state.last_event_at_export =
        frontier.events.last().map(|event| event.timestamp.clone());
    frontier.proof_state.stale_reason = None;
}

pub fn mark_proof_stale(frontier: &mut Project, reason: String) {
    if frontier.proof_state.latest_packet.status != "never_exported" {
        frontier.proof_state.latest_packet.status = "stale".to_string();
        frontier.proof_state.stale_reason = Some(reason);
    }
}

pub fn proof_state_json(proof_state: &ProofState) -> Value {
    serde_json::to_value(proof_state).unwrap_or_else(|_| json!({"status": "never_exported"}))
}

pub fn proposal_state_hash(proposals: &[StateProposal]) -> String {
    let bytes = canonical::to_canonical_bytes(proposals).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn load_proposals(source: &Path) -> Result<Vec<StateProposal>, String> {
    if source.is_file() {
        let data = std::fs::read_to_string(source)
            .map_err(|e| format!("Failed to read proposal file '{}': {e}", source.display()))?;
        if let Ok(proposals) = serde_json::from_str::<Vec<StateProposal>>(&data) {
            return Ok(proposals);
        }
        let proposal = serde_json::from_str::<StateProposal>(&data)
            .map_err(|e| format!("Failed to parse proposal JSON '{}': {e}", source.display()))?;
        return Ok(vec![proposal]);
    }
    if source.is_dir() {
        let mut entries = std::fs::read_dir(source)
            .map_err(|e| format!("Failed to read proposal dir '{}': {e}", source.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();
        entries.sort();
        let mut proposals = Vec::new();
        for path in entries {
            proposals.extend(load_proposals(&path)?);
        }
        return Ok(proposals);
    }
    Err(format!(
        "Proposal source does not exist: {}",
        source.display()
    ))
}

fn validate_new_proposal(frontier: &Project, proposal: &StateProposal) -> Result<(), String> {
    if proposal.schema != PROPOSAL_SCHEMA {
        return Err(format!("Unsupported proposal schema '{}'", proposal.schema));
    }
    if frontier
        .proposals
        .iter()
        .any(|existing| existing.id == proposal.id)
    {
        return Err(format!("Duplicate proposal id {}", proposal.id));
    }
    validate_proposal_shape(frontier, proposal)?;
    validate_decision_state(proposal)
}

fn validate_proposal_shape(frontier: &Project, proposal: &StateProposal) -> Result<(), String> {
    // v0.52: relax the finding-only constraint so the agent inbox
    // can deposit nulls and trajectories through the same review-
    // gated flow as findings. The proposal-kind dispatch below
    // enforces that target.type matches the kind family.
    if !matches!(
        proposal.target.r#type.as_str(),
        "finding" | "artifact" | "evidence_atom" | "frontier_observation" | "governance"
    ) {
        return Err(format!(
            "Unsupported proposal target type '{}'; valid: finding, artifact, evidence_atom, frontier_observation, governance",
            proposal.target.r#type
        ));
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    if !matches!(
        proposal.status.as_str(),
        "pending_review" | "accepted" | "rejected" | "applied"
    ) {
        return Err(format!("Unsupported proposal status '{}'", proposal.status));
    }
    match proposal.kind.as_str() {
        policy_accept::POLICY_HEAD_PROPOSAL_KIND => {
            policy_accept::validate_policy_head_proposal(frontier, proposal)?;
        }
        "finding.add" => {
            let finding_value = proposal
                .payload
                .get("finding")
                .ok_or("finding.add proposal missing payload.finding")?
                .clone();
            let finding: FindingBundle = serde_json::from_value(finding_value)
                .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
            if finding.id != proposal.target.id {
                return Err(format!(
                    "finding.add target {} does not match payload finding {}",
                    proposal.target.id, finding.id
                ));
            }
            if frontier
                .findings
                .iter()
                .any(|existing| existing.id == proposal.target.id)
            {
                return Err(format!(
                    "Refusing to add duplicate finding with existing finding ID {}",
                    proposal.target.id
                ));
            }
            if let Some(submission) = proposal.payload.get("vela_submission") {
                validate_submission_links(submission)?;
            }
        }
        "finding.review" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let status = proposal
                .payload
                .get("status")
                .and_then(Value::as_str)
                .ok_or("finding.review proposal missing payload.status")?;
            if !matches!(
                status,
                "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
            ) {
                return Err(format!("Unsupported review proposal status '{status}'"));
            }
        }
        "finding.caveat" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.caveat proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.caveat payload.text must be non-empty".to_string());
            }
        }
        "finding.note" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.note proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.note payload.text must be non-empty".to_string());
            }
        }
        "finding.confidence_revise" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let score = proposal
                .payload
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or("finding.confidence_revise proposal missing payload.confidence")?;
            if !(0.0..=1.0).contains(&score) {
                return Err(
                    "finding.confidence_revise confidence must be between 0.0 and 1.0".to_string(),
                );
            }
        }
        "finding.reject" => {
            require_existing_finding(frontier, &proposal.target.id)?;
        }
        "finding.contribution.recorded" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let contribution: crate::bundle::Contribution =
                serde_json::from_value(proposal.payload.get("contribution").cloned().ok_or(
                    "finding.contribution.recorded proposal missing payload.contribution",
                )?)
                .map_err(|e| format!("malformed contribution: {e}"))?;
            contribution.validate()?;
        }
        "finding.retract" => {
            let idx = require_existing_finding(frontier, &proposal.target.id)?;
            if frontier.findings[idx].flags.retracted {
                return Err(format!(
                    "Finding {} is already retracted",
                    proposal.target.id
                ));
            }
        }
        "finding.supersede" => {
            let idx = require_existing_finding(frontier, &proposal.target.id)?;
            if frontier.findings[idx].flags.superseded {
                return Err(format!(
                    "Finding {} is already superseded",
                    proposal.target.id
                ));
            }
            let new_finding_value = proposal
                .payload
                .get("new_finding")
                .ok_or("finding.supersede proposal missing payload.new_finding")?
                .clone();
            let new_finding: FindingBundle = serde_json::from_value(new_finding_value)
                .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;
            if new_finding.id == proposal.target.id {
                return Err(
                    "finding.supersede new_finding has same content address as the superseded target — change assertion text, type, or provenance to derive a distinct vf_…".to_string(),
                );
            }
            if frontier
                .findings
                .iter()
                .any(|existing| existing.id == new_finding.id)
            {
                return Err(format!(
                    "Refusing to add superseding finding with existing finding ID {}",
                    new_finding.id
                ));
            }
        }
        "artifact.assert" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.assert proposal target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
            let artifact_value = proposal
                .payload
                .get("artifact")
                .ok_or("artifact.assert proposal missing payload.artifact")?
                .clone();
            let artifact: Artifact = serde_json::from_value(artifact_value)
                .map_err(|e| format!("Invalid artifact.assert payload: {e}"))?;
            if artifact.id != proposal.target.id {
                return Err(format!(
                    "artifact.assert target {} does not match payload id {}",
                    proposal.target.id, artifact.id
                ));
            }
            if frontier.artifacts.iter().any(|a| a.id == artifact.id) {
                return Err(format!(
                    "Refusing to add duplicate artifact with existing id {}",
                    artifact.id
                ));
            }
        }
        "artifact.retract" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.retract proposal target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
            if proposal.reason.trim().is_empty() {
                return Err("artifact.retract proposal reason must be non-empty".to_string());
            }
            let artifact = frontier
                .artifacts
                .iter()
                .find(|artifact| artifact.id == proposal.target.id)
                .ok_or_else(|| format!("Artifact not found: {}", proposal.target.id))?;
            if artifact.retracted {
                return Err(format!("Artifact {} is already retracted", artifact.id));
            }
        }
        "verifier.attach" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "verifier.attach proposal target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            let value = proposal
                .payload
                .get("attachment")
                .ok_or("verifier.attach proposal missing payload.attachment")?
                .clone();
            let att: crate::verifier_attachment::VerifierAttachment = serde_json::from_value(value)
                .map_err(|e| format!("Invalid verifier.attach payload: {e}"))?;
            att.verify()
                .map_err(|e| format!("verifier.attach attachment malformed: {e}"))?;
            if att.target != proposal.target.id {
                return Err(format!(
                    "verifier.attach attachment.target {} does not match proposal target {}",
                    att.target, proposal.target.id
                ));
            }
        }
        // v0.57: Mechanical finding-level span repair. Appends a
        // `{section, text}` span to the finding's evidence_spans.
        "finding.span_repair" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "finding.span_repair target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            require_existing_finding(frontier, &proposal.target.id)?;
            let section = proposal
                .payload
                .get("section")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.section")?;
            if section.trim().is_empty() {
                return Err("finding.span_repair payload.section must be non-empty".to_string());
            }
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.span_repair payload.text must be non-empty".to_string());
            }
        }
        // v0.56: Mechanical evidence-atom locator repair. Targets one
        // evidence atom by id; payload carries the resolved locator
        // string and the parent source id it was derived from. The
        // proposal is mechanical: the locator is already present on
        // `frontier.sources[atom.source_id].locator`. A Decision Plan accepts
        // it and the canonical event lands the locator
        // on the atom while preserving the derivation in the payload.
        "evidence_atom.locator_repair" => {
            if proposal.target.r#type != "evidence_atom" {
                return Err(format!(
                    "evidence_atom.locator_repair target.type must be 'evidence_atom', got '{}'",
                    proposal.target.r#type
                ));
            }
            let atom_id = proposal.target.id.as_str();
            let atom = frontier
                .evidence_atoms
                .iter()
                .find(|atom| atom.id == atom_id)
                .ok_or_else(|| {
                    format!("evidence_atom.locator_repair targets unknown atom {atom_id}")
                })?;
            let locator = proposal
                .payload
                .get("locator")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?;
            if locator.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.locator must be non-empty".to_string(),
                );
            }
            let source_id = proposal
                .payload
                .get("source_id")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?;
            if source_id.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.source_id must be non-empty".to_string(),
                );
            }
            if atom.source_id != source_id {
                return Err(format!(
                    "evidence_atom.locator_repair payload.source_id '{source_id}' does not match atom.source_id '{}'",
                    atom.source_id
                ));
            }
            // Refuse a no-op repair so the curation pipeline doesn't
            // emit empty events. An atom that already carries the same
            // locator should be filtered upstream.
            if let Some(existing) = &atom.locator
                && existing == locator
            {
                return Err(format!(
                    "evidence_atom {atom_id} already carries locator '{existing}'"
                ));
            }
            // Refuse a divergent overwrite. A different existing
            // locator is a chain-integrity issue, not a repair.
            if let Some(existing) = &atom.locator
                && existing != locator
            {
                return Err(format!(
                    "evidence_atom {atom_id} already carries locator '{existing}'; refusing to overwrite with '{locator}'"
                ));
            }
        }
        "research_trace.review" => {
            validate_research_trace_review_payload(proposal)?;
        }
        "correction_return.review" => {
            validate_correction_return_review_payload(proposal)?;
        }
        other => {
            return Err(format!("Unsupported proposal kind '{other}'"));
        }
    }
    Ok(())
}

fn validate_submission_links(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("finding.add payload.vela_submission must be an object")?;
    if object.get("schema").and_then(Value::as_str) != Some("vela.submission-links.internal.v1") {
        return Err("finding.add payload.vela_submission has an unsupported schema".to_string());
    }
    let valid_prefixed_digest = |field: &str, prefix: &str| -> Result<(), String> {
        let value = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("vela_submission.{field} must be a string"))?;
        let digest = value
            .strip_prefix(prefix)
            .ok_or_else(|| format!("vela_submission.{field} must start with {prefix}"))?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "vela_submission.{field} must contain 64 lowercase hex characters"
            ));
        }
        Ok(())
    };
    valid_prefixed_digest("receipt_root", "sha256:")?;
    valid_prefixed_digest("operation_id", "vop_")?;
    let record_id = object
        .get("record_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !record_id.strip_prefix("vrc_").is_some_and(|id| {
        id.len() == 16
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err("vela_submission.record_id must be vrc_<16 lowercase hex>".to_string());
    }
    let receipt_path = object
        .get("receipt_path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = std::path::Path::new(receipt_path);
    if receipt_path.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("vela_submission.receipt_path must be frontier-relative".to_string());
    }
    if let Some(review_path) = object.get("review_material_path").and_then(Value::as_str) {
        let receipt_digest = object
            .get("receipt_root")
            .and_then(Value::as_str)
            .and_then(|root| root.strip_prefix("sha256:"))
            .unwrap_or_default();
        let expected = format!("records/review/sha256/{receipt_digest}.json");
        if review_path != expected {
            return Err(format!(
                "vela_submission.review_material_path must be {expected}"
            ));
        }
    }
    Ok(())
}

fn validate_decision_state(proposal: &StateProposal) -> Result<(), String> {
    match proposal.status.as_str() {
        "pending_review" => Ok(()),
        "accepted" | "applied" | "rejected" => {
            let reviewer = proposal
                .reviewed_by
                .as_deref()
                .ok_or_else(|| format!("Proposal {} missing reviewed_by", proposal.id))?;
            validate_reviewer_identity(reviewer)?;
            if proposal
                .decision_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(format!("Proposal {} missing decision_reason", proposal.id));
            }
            if proposal.status == "applied" && proposal.applied_event_id.is_none() {
                return Err(format!(
                    "Applied proposal {} missing applied_event_id",
                    proposal.id
                ));
            }
            Ok(())
        }
        other => Err(format!("Unsupported proposal status '{}'", other)),
    }
}

fn validate_standalone_proposal(
    _frontier: &Project,
    proposal: &StateProposal,
) -> Result<(), String> {
    if proposal.schema != PROPOSAL_SCHEMA {
        return Err(format!("Unsupported proposal schema '{}'", proposal.schema));
    }
    if !matches!(
        proposal.target.r#type.as_str(),
        "finding" | "artifact" | "evidence_atom" | "frontier_observation" | "governance"
    ) {
        return Err(
            "Only finding, artifact, evidence_atom, frontier_observation, and governance proposals are supported in v0"
                .to_string(),
        );
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    match proposal.kind.as_str() {
        policy_accept::POLICY_HEAD_PROPOSAL_KIND => {
            if proposal.target.r#type != "governance" {
                return Err("policy-head proposal target.type must be governance".to_string());
            }
            policy_accept::parse_policy_head_payload(proposal)?;
        }
        "finding.add" => {
            let finding_value = proposal
                .payload
                .get("finding")
                .ok_or("finding.add proposal missing payload.finding")?
                .clone();
            let finding: FindingBundle = serde_json::from_value(finding_value)
                .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
            if finding.id != proposal.target.id {
                return Err(format!(
                    "finding.add target {} does not match payload finding {}",
                    proposal.target.id, finding.id
                ));
            }
        }
        "finding.review" => {
            let status = proposal
                .payload
                .get("status")
                .and_then(Value::as_str)
                .ok_or("finding.review proposal missing payload.status")?;
            if !matches!(
                status,
                "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
            ) {
                return Err(format!("Unsupported review proposal status '{status}'"));
            }
        }
        "finding.caveat" => {
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.caveat proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.caveat payload.text must be non-empty".to_string());
            }
        }
        "finding.note" => {
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.note proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.note payload.text must be non-empty".to_string());
            }
        }
        "finding.confidence_revise" => {
            let score = proposal
                .payload
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or("finding.confidence_revise proposal missing payload.confidence")?;
            if !(0.0..=1.0).contains(&score) {
                return Err(
                    "finding.confidence_revise confidence must be between 0.0 and 1.0".to_string(),
                );
            }
        }
        "finding.reject" | "finding.retract" => {}
        "artifact.retract" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.retract target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
        }
        "finding.supersede" => {
            let new_finding_value = proposal
                .payload
                .get("new_finding")
                .ok_or("finding.supersede proposal missing payload.new_finding")?
                .clone();
            let new_finding: FindingBundle = serde_json::from_value(new_finding_value)
                .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;
            if new_finding.id == proposal.target.id {
                return Err(
                    "finding.supersede new_finding has same content address as the superseded target"
                        .to_string(),
                );
            }
        }
        // v0.57: standalone validation of finding span-repair.
        "finding.span_repair" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "finding.span_repair target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            let section = proposal
                .payload
                .get("section")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.section")?;
            if section.trim().is_empty() {
                return Err("finding.span_repair payload.section must be non-empty".to_string());
            }
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.span_repair payload.text must be non-empty".to_string());
            }
        }
        // v0.56: standalone validation of an evidence-atom locator
        // repair. Mirrors the contextual validator in
        // `validate_proposal_shape`, except without frontier-side
        // existence checks (the standalone validator runs over an
        // exported proposal before it is loaded into a frontier).
        "evidence_atom.locator_repair" => {
            if proposal.target.r#type != "evidence_atom" {
                return Err(format!(
                    "evidence_atom.locator_repair target.type must be 'evidence_atom', got '{}'",
                    proposal.target.r#type
                ));
            }
            let locator = proposal
                .payload
                .get("locator")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?;
            if locator.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.locator must be non-empty".to_string(),
                );
            }
            let source_id = proposal
                .payload
                .get("source_id")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?;
            if source_id.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.source_id must be non-empty".to_string(),
                );
            }
        }
        "finding.contribution.recorded" => {
            let contribution: crate::bundle::Contribution =
                serde_json::from_value(proposal.payload.get("contribution").cloned().ok_or(
                    "finding.contribution.recorded proposal missing payload.contribution",
                )?)
                .map_err(|e| format!("malformed contribution: {e}"))?;
            contribution.validate()?;
        }
        "research_trace.review" => {
            validate_research_trace_review_payload(proposal)?;
        }
        "correction_return.review" => {
            validate_correction_return_review_payload(proposal)?;
        }
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    }
    validate_decision_state(proposal)
}

fn validate_research_trace_review_payload(proposal: &StateProposal) -> Result<(), String> {
    if proposal.target.r#type != "frontier_observation" {
        return Err(format!(
            "research_trace.review target.type must be 'frontier_observation', got '{}'",
            proposal.target.r#type
        ));
    }
    let trace_id = proposal
        .payload
        .get("trace_id")
        .and_then(Value::as_str)
        .ok_or("research_trace.review proposal missing payload.trace_id")?;
    if !trace_id.starts_with("vrt_") {
        return Err("research_trace.review payload.trace_id must start with `vrt_`".to_string());
    }
    let output_kind = proposal
        .payload
        .get("output_kind")
        .and_then(Value::as_str)
        .ok_or("research_trace.review proposal missing payload.output_kind")?;
    if !matches!(output_kind, "candidate_finding" | "open_need") {
        return Err(format!(
            "research_trace.review payload.output_kind must be candidate_finding or open_need, got '{output_kind}'"
        ));
    }
    if output_kind == "candidate_finding" && proposal.payload.get("candidate").is_none() {
        return Err(
            "research_trace.review candidate_finding missing payload.candidate".to_string(),
        );
    }
    if output_kind == "open_need" && proposal.payload.get("open_need").is_none() {
        return Err("research_trace.review open_need missing payload.open_need".to_string());
    }
    if proposal.payload.get("authority_boundary").is_none() {
        return Err("research_trace.review missing payload.authority_boundary".to_string());
    }
    if proposal.payload.get("formalization_fidelity").is_none() {
        return Err("research_trace.review missing payload.formalization_fidelity".to_string());
    }
    if !proposal
        .source_refs
        .iter()
        .any(|source_ref| source_ref == trace_id)
    {
        return Err(format!(
            "research_trace.review source_refs must include trace_id {trace_id}"
        ));
    }
    Ok(())
}

fn validate_correction_return_review_payload(proposal: &StateProposal) -> Result<(), String> {
    if proposal.target.r#type != "frontier_observation" {
        return Err(format!(
            "correction_return.review target.type must be 'frontier_observation', got '{}'",
            proposal.target.r#type
        ));
    }
    let correction = proposal
        .payload
        .get("correction")
        .ok_or("correction_return.review proposal missing payload.correction")?;
    for field in [
        "target_id",
        "issue",
        "proposed_change",
        "source_locator",
        "evidence_span",
    ] {
        let value = correction
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("correction_return.review payload.correction.{field} is required")
            })?;
        if value.trim().is_empty() {
            return Err(format!(
                "correction_return.review payload.correction.{field} must be non-empty"
            ));
        }
    }
    let verification_runs = correction
        .get("verification_run")
        .and_then(Value::as_array)
        .ok_or("correction_return.review payload.correction.verification_run must be an array")?;
    if verification_runs.is_empty() {
        return Err(
            "correction_return.review payload.correction.verification_run must be non-empty"
                .to_string(),
        );
    }
    let boundary = proposal
        .payload
        .get("claim_boundary")
        .and_then(Value::as_object)
        .ok_or("correction_return.review missing payload.claim_boundary")?;
    for field in [
        "claims_clinical_validity",
        "claims_external_adoption",
        "claims_external_validation",
        "claims_lab_validation",
        "claims_scientific_discovery",
        "claims_target_validation",
        "claims_treatment_advice",
    ] {
        match boundary.get(field).and_then(Value::as_bool) {
            Some(false) => {}
            Some(true) => {
                return Err(format!(
                    "correction_return.review payload.claim_boundary.{field} must be false"
                ));
            }
            None => {
                return Err(format!(
                    "correction_return.review payload.claim_boundary.{field} is required"
                ));
            }
        }
    }
    if !proposal.source_refs.iter().any(|source_ref| {
        source_ref == "correction-return.template.json"
            || (source_ref.starts_with("correction-return.") && source_ref.ends_with(".json"))
    }) {
        return Err(
            "correction_return.review source_refs must include correction-return.template.json or a correction-return.*.json file"
                .to_string(),
        );
    }
    Ok(())
}

fn require_existing_finding(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

// Proposal-kind classification is deliberately separate from authority.
// These lists tell policy and presentation code which records are mechanical
// or process-only; they do not mint a decision capability. A caller still
// needs a Decision Plan or the narrow, signed `VerifiedProposalWrite` token.

/// Mechanical, truth-preserving repair kinds. This is classification only;
/// it does not let an agent apply the proposal without decision authority.
const MECHANICAL_REPAIR_KINDS: &[&str] = &["finding.span_repair", "evidence_atom.locator_repair"];

/// Non-truth-bearing provenance kinds: content-addressed artifact registration
/// and claim-granularity attribution
/// (`finding.contribution.recorded`). These assert no scientific claim about the
/// world — an artifact stores bytes, and a contribution records *who produced
/// what*, disclosed and never conferring trust (the reducer ignores it, the gate
/// never reads it, and the credit view keeps a machine out of `author_of_record`
/// regardless). They fall outside the human-gated truth boundary, so a fleet need
/// not block on a human. Anything truth-bearing (a claim about the world,
/// including a null result) stays gated.
const PROCESS_PROVENANCE_KINDS: &[&str] = &[
    "artifact.assert",
    "artifact.add",
    "finding.contribution.recorded",
];

/// Proposal-level guards for exact-lane auto-admission (Phase 1A, the
/// de-human-gate). Returns `(admit, reasons)`; `reasons` is non-empty exactly
/// when refused. Pure and deterministic, so two implementations agree.
///
/// IMPORTANT — this is NOT the whole gate. The un-forgeable floor (a fresh
/// `vela reproduce` over the witness AND `vela_verify::claim_witness_faithful`
/// binding the parsed assertion to the witness structure) is applied by the
/// CLI command BEFORE this is called, because it needs the `vela-verify`
/// binary and the witness file, which the protocol crate does not see. This
/// function adds the protocol-level guards a human reviewer applies and then
/// delegates to the attachment corroboration predicate
/// [`crate::verifier_attachment::exact_lane_attachment_admit`]. See
/// `docs/VERIFICATION.md` for why the corroboration predicate alone is
/// insufficient (a `VerifierAttachment` is unsigned self-asserted data the
/// producing agent can author).
///
/// Fail-closed guard order:
///   1. kind allowlist: `finding.add` only.
///   2. target binding: target is this finding.
///   3. content-address drift-pin: the loaded finding body must content-address
///      to its own id (closes assertion-text edits after the id was minted).
///   4. lifecycle: the finding is neither retracted nor superseded.
///   5. synthetic: no `synthetic_source_requires_review` signal (caller-derived).
///   6. contradiction: no live open contradiction names this finding
///      (caller-derived, including freshly derived candidates).
///   7. producer != verifier: the proposing actor differs from every matched
///      attachment's `verifier_actor` (the producer cannot be its own
///      corroborator at the actor level).
///   8. delegate to the attachment predicate over the matched attachments.
pub fn exact_lane_auto_admit(
    proposal: &StateProposal,
    finding: &crate::bundle::FindingBundle,
    attachments: &[crate::verifier_attachment::VerifierAttachment],
    open_contradiction_finding_ids: &BTreeSet<String>,
    synthetic_unreviewed_finding_ids: &BTreeSet<String>,
    floor_sufficient: bool,
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();

    // 1. kind allowlist.
    if proposal.kind != "finding.add" {
        reasons.push(format!(
            "exact-lane: proposal kind '{}' is not 'finding.add'",
            proposal.kind
        ));
        return (false, reasons);
    }

    // 2. target binding.
    if proposal.target.r#type != "finding" || proposal.target.id != finding.id {
        reasons.push("exact-lane: proposal target does not bind to this finding".to_string());
        return (false, reasons);
    }

    // 3. content-address drift-pin: the body must hash to its own id.
    let recomputed =
        crate::bundle::FindingBundle::content_address(&finding.assertion, &finding.provenance);
    if recomputed != finding.id {
        reasons.push(format!(
            "exact-lane: finding body does not content-address to its id (drift): {} != {}",
            recomputed, finding.id
        ));
        return (false, reasons);
    }

    // 4. lifecycle.
    if finding.flags.retracted || finding.flags.superseded {
        reasons.push("exact-lane: finding is retracted or superseded".to_string());
        return (false, reasons);
    }

    // 5. synthetic-source signal.
    if synthetic_unreviewed_finding_ids.contains(&finding.id) {
        reasons.push(
            "exact-lane: finding carries a synthetic_source_requires_review signal".to_string(),
        );
        return (false, reasons);
    }

    // 6. live open contradiction.
    if open_contradiction_finding_ids.contains(&finding.id) {
        reasons.push("exact-lane: a live open contradiction names this finding".to_string());
        return (false, reasons);
    }

    // The matched attachments (those bound to this finding).
    let matched: Vec<crate::verifier_attachment::VerifierAttachment> = attachments
        .iter()
        .filter(|a| a.target == finding.id)
        .cloned()
        .collect();

    // 7. producer != verifier: the proposing actor cannot also be a corroborator.
    let producer = proposal.actor.id.trim();
    if !producer.is_empty()
        && let Some(bad) = matched.iter().find(|a| a.verifier_actor.trim() == producer)
    {
        reasons.push(format!(
            "exact-lane: the proposing actor '{}' is also a verifier_actor on attachment '{}' \
             (producer cannot corroborate itself)",
            producer, bad.id
        ));
        return (false, reasons);
    }

    // 8. corroboration. When `floor_sufficient` (the caller established the
    // un-forgeable floor: a fresh frozen `vela reproduce` over the witness AND
    // `claim_witness_faithful` binding the parsed assertion to it), the FLOOR is
    // itself the proof of an exact lower-bound / size claim, so the
    // >=2-independent-attachment requirement (the GENERAL gate's bar, for claims
    // with no single frozen verifier) is waived — attachments become optional
    // corroboration. Otherwise the attachment predicate must derive Verified.
    if !floor_sufficient {
        let digest = crate::verifier_attachment::claim_digest(&finding.assertion.text);
        let (admit, att_reasons) =
            crate::verifier_attachment::exact_lane_attachment_admit(&digest, &matched);
        if !admit {
            reasons.extend(att_reasons);
            return (false, reasons);
        }
    }

    (true, reasons)
}

/// The verification trust tier of a finding (Phase 1A). An ordered ladder;
/// the machine advances the lower rungs, a human key-custody accept is the
/// only path to `Accepted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    Candidate,
    SchemaChecked,
    MachineVerified,
    Accepted,
}

impl TrustTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustTier::Candidate => "candidate",
            TrustTier::SchemaChecked => "schema_checked",
            TrustTier::MachineVerified => "machine_verified",
            TrustTier::Accepted => "accepted",
        }
    }
}

/// Project a finding's verification trust tier from canonical state + the
/// immutable log + live attachments (Phase 1A). A read-only projection, never
/// a stored field: recomputed fresh so a forged `policy.auto_admitted` event
/// cannot by itself raise the tier, and a later-weakened attachment set
/// silently lowers it.
///
/// - `Accepted`: the finding is landed in canonical state (`frontier.findings`)
///   and not retracted/superseded. Landing runs only through the key-custody
///   accept ceremony, so canonical membership IS the human-accept signal (no
///   reliance on which event kind the ceremony emitted). Strictly highest; the
///   machine never reaches it.
/// - `MachineVerified`: the finding is a PENDING `finding.add` proposal carrying
///   a `policy.auto_admitted` marker whose gate, recomputed LIVE from the
///   proposal's finding text + the current matched attachments, is `Verified`.
///   Machine-verified state is a separate queryable layer over pending
///   proposals; it is NEVER landed in `frontier.findings` (that is the human
///   tier), which preserves the charter boundary.
/// - `SchemaChecked`: at least one passing matched attachment, not yet Verified.
/// - `Candidate`: everything else, including retracted/superseded.
pub fn derive_trust_tier(frontier: &Project, finding_id: &str) -> TrustTier {
    use crate::verifier_attachment::{AttachmentOutcome, GateStatus, derive_gate_status};

    // Landed in canonical accepted state?
    if let Some(f) = frontier.findings.iter().find(|f| f.id == finding_id) {
        if f.flags.retracted || f.flags.superseded {
            return TrustTier::Candidate;
        }
        return TrustTier::Accepted;
    }

    let matched: Vec<crate::verifier_attachment::VerifierAttachment> = frontier
        .verifier_attachments
        .iter()
        .filter(|a| a.target == finding_id)
        .cloned()
        .collect();

    // A pending finding.add proposal for this finding, carrying an auto-admit.
    let pending = frontier.proposals.iter().find(|p| {
        p.kind == "finding.add"
            && p.applied_event_id.is_none()
            && (p.target.id == finding_id
                || p.payload
                    .get("finding")
                    .and_then(|f| f.get("id"))
                    .and_then(|i| i.as_str())
                    == Some(finding_id))
    });
    if let Some(p) = pending {
        let admitted = frontier.events.iter().any(|e| {
            e.kind.as_str() == "policy.auto_admitted"
                && e.payload.get("proposal_id").and_then(|v| v.as_str()) == Some(p.id.as_str())
        });
        if admitted
            && let Some(finding_val) = p.payload.get("finding")
            && let Ok(fb) =
                serde_json::from_value::<crate::bundle::FindingBundle>(finding_val.clone())
        {
            let digest = crate::verifier_attachment::claim_digest(&fb.assertion.text);
            if derive_gate_status(&digest, &matched).status == GateStatus::Verified {
                return TrustTier::MachineVerified;
            }
        }
    }

    if matched
        .iter()
        .any(|a| a.outcome == AttachmentOutcome::Passed)
    {
        return TrustTier::SchemaChecked;
    }
    TrustTier::Candidate
}

/// Emit the unsigned `policy.auto_admitted` audit event for an exact-lane
/// machine admission (Phase 1A). IDEMPOTENT: if one already targets this
/// proposal, returns its id with `false` and writes nothing, so re-running
/// `--apply` yields a byte-identical log (closes the duplicate-mint hole — the
/// event id embeds a timestamp, so the parity guarantee is replay-stable, not
/// mint-deterministic). The event is a no-op on every finding digest
/// (`before == after == NULL_HASH`), so reproduce/materialize stay
/// byte-identical. UNSIGNED and mechanically un-signable: the trust is the
/// frozen predicate + frozen verifier, never a key. The caller MUST have
/// already established the YES verdict (the un-forgeable floor + the
/// proposal-level guards); this only records it.
pub fn emit_policy_auto_admitted(
    path: &Path,
    proposal_id: &str,
    claim_digest: &str,
    attachment_ids: &[String],
    policy_version: &str,
    verifier_env_hash: &str,
) -> Result<(String, bool), String> {
    let mut frontier = repo::load_from_path(path)?;
    // Idempotency: an existing admit for this proposal is authoritative.
    if let Some(existing) = frontier.events.iter().find(|e| {
        e.kind.as_str() == events::EVENT_KIND_POLICY_AUTO_ADMITTED
            && e.payload.get("proposal_id").and_then(|v| v.as_str()) == Some(proposal_id)
    }) {
        return Ok((existing.id.clone(), false));
    }
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_POLICY_AUTO_ADMITTED.into(),
        target: StateTarget {
            r#type: "proposal".to_string(),
            id: proposal_id.to_string(),
        },
        actor: StateActor {
            id: "policy:exact-lane".to_string(),
            r#type: "agent".to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: format!("exact-lane auto-admit: frozen predicate {policy_version}"),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal_id,
            "claim_digest": claim_digest,
            "attachment_ids": attachment_ids,
            "policy_version": policy_version,
            "verifier_env_hash": verifier_env_hash,
        }),
        caveats: Vec::new(),
        signature: None,
        schema_artifact_id: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    let id = event.id.clone();
    // Replay through the reducer (a verified no-op) before recording, exactly
    // as the loader will on the next materialize.
    crate::reducer::apply_event(&mut frontier, &event)?;
    frontier.events.push(event);
    repo::save_to_path(path, &frontier)?;
    Ok((id, true))
}

/// Result of a fixed-time, in-memory decision preparation.
///
/// This is transaction plumbing, not a serialized protocol object. The
/// prepared events are unsigned. A human-key caller may pass this exact set to
/// [`sign_prepared_decision_events`] only after rederiving and confirming its
/// Decision Plan under the frontier lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDecisionMutation {
    /// The domain event produced by an accept, or the review event for a
    /// reject/policy-head accept.
    primary_event_id: String,
    /// Every event appended by this one decision, in canonical order.
    appended_event_ids: Vec<String>,
    /// The `review.accepted` / `review.rejected` event that binds the decision
    /// provenance, including the decision-root input reference when supplied.
    decision_event_id: String,
    proposal_id: String,
    reviewer: String,
    first_event: usize,
    event_count_after: usize,
    binding: PreparedDecisionBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparedDecisionBinding {
    Unbound,
    Bound { decision_root: String },
}

impl PreparedDecisionMutation {
    #[must_use]
    pub fn primary_event_id(&self) -> &str {
        &self.primary_event_id
    }

    #[must_use]
    pub fn appended_event_ids(&self) -> &[String] {
        &self.appended_event_ids
    }

    #[must_use]
    pub fn decision_event_id(&self) -> &str {
        &self.decision_event_id
    }

    #[must_use]
    pub fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    #[must_use]
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    #[must_use]
    pub fn decision_root(&self) -> Option<&str> {
        match &self.binding {
            PreparedDecisionBinding::Unbound => None,
            PreparedDecisionBinding::Bound { decision_root } => Some(decision_root),
        }
    }

    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.decision_root().is_some()
    }
}

/// Check decision-time reviewer facts without reading or receiving a private
/// key. New Decision Plan reviews require a registered, active Ed25519 actor;
/// agent and CI identities can never enter this human seam. Legacy decision
/// creation APIs fail closed; there is no keyless bootstrap decision path.
pub fn validate_human_reviewer_authority_at(
    frontier: &Project,
    reviewer: &str,
    decided_at: &str,
) -> Result<crate::sign::ActorRecord, String> {
    validate_reviewer_identity(reviewer)?;
    let decision_at = chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("decision time is invalid: {error}"))?;
    if reviewer.starts_with("agent:") || reviewer.starts_with("ci:") {
        return Err(format!(
            "reviewer '{reviewer}' may not enter a human Decision Plan"
        ));
    }
    let mut actors = frontier.actors.iter().filter(|actor| actor.id == reviewer);
    let actor = actors
        .next()
        .cloned()
        .ok_or_else(|| format!("reviewer '{reviewer}' is not registered on this frontier"))?;
    if actors.next().is_some() {
        return Err(format!(
            "reviewer '{reviewer}' is registered ambiguously (duplicate actor ids)"
        ));
    }
    if actor.algorithm != "ed25519"
        || hex::decode(&actor.public_key).map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(format!(
            "reviewer '{}' must have a registered Ed25519 decision key",
            actor.id
        ));
    }
    if !actor_has_reviewer_authority(&actor)
        && !(actor.id.starts_with("steward:") && !is_placeholder_reviewer(&actor.id))
    {
        return Err(format!(
            "actor '{}' does not carry reviewer or steward decision authority",
            actor.id
        ));
    }
    let created_at = chrono::DateTime::parse_from_rfc3339(&actor.created_at)
        .map_err(|error| format!("reviewer '{}' creation time is invalid: {error}", actor.id))?;
    if created_at > decision_at {
        return Err(format!(
            "reviewer '{}' is not yet registered at {decided_at}",
            actor.id
        ));
    }
    if let Some(revoked_at) = actor.revoked_at.as_deref() {
        let revoked_at = chrono::DateTime::parse_from_rfc3339(revoked_at).map_err(|error| {
            format!(
                "reviewer '{}' revocation time is invalid: {error}",
                actor.id
            )
        })?;
        if decision_at >= revoked_at {
            return Err(format!(
                "reviewer key for actor '{}' is revoked as of {decided_at}",
                actor.id
            ));
        }
    }
    Ok(actor)
}

enum DecisionAuthority<'a> {
    LocalKey(&'a ed25519_dalek::SigningKey),
    DetachedAccept(&'a AcceptAuthorization),
    PlanPreparation,
    Preview,
    VerifiedProcess(&'a VerifiedProposalWrite),
}

impl<'a> DecisionAuthority<'a> {
    fn signing_key(&self) -> Option<&'a ed25519_dalek::SigningKey> {
        match self {
            Self::LocalKey(key) => Some(*key),
            _ => None,
        }
    }
}

fn enforce_decision_authority(
    frontier: &Project,
    proposal: &StateProposal,
    reviewer: &str,
    reason: &str,
    decided_at: &str,
    authority: &DecisionAuthority<'_>,
) -> Result<(), String> {
    match authority {
        DecisionAuthority::LocalKey(key) => {
            let actor = validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
            let derived = hex::encode(key.verifying_key().to_bytes());
            if !derived.eq_ignore_ascii_case(&actor.public_key) {
                return Err(format!(
                    "the supplied key derives pubkey {}…, which does not match {reviewer}'s registered decision key {}…",
                    &derived[..12],
                    &actor.public_key[..actor.public_key.len().min(12)]
                ));
            }
            Ok(())
        }
        DecisionAuthority::DetachedAccept(verified) => {
            if verified.decision_root().is_some() {
                return Err(
                    "decision-bound detached authority must be executed through a Decision Plan"
                        .to_string(),
                );
            }
            verified.validate_binding(frontier, &proposal.id, reviewer, reason)
        }
        DecisionAuthority::PlanPreparation => {
            validate_human_reviewer_authority_at(frontier, reviewer, decided_at).map(|_| ())
        }
        DecisionAuthority::Preview => validate_reviewer_identity(reviewer),
        DecisionAuthority::VerifiedProcess(verified) => {
            if reviewer != verified.actor_id || proposal.kind != "finding.note" {
                return Err(
                    "verified process authority cannot create a truth-bearing decision".to_string(),
                );
            }
            revalidate_signed_proposal_write(frontier, proposal, verified)?;
            if verified.disposition != VerifiedProposalDisposition::AutoApplyNote {
                return Err("verified proposal write carries pending-only authority".to_string());
            }
            Ok(())
        }
    }
}

/// The one canonical accept path, with key-custody enforcement.
///
/// New accepts require a registered human reviewer and its matching private
/// key. `None` is retained only for source compatibility and always refuses;
/// there is no keyless bootstrap decision path.
pub fn accept_proposal_in_frontier_signed(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
) -> Result<String, String> {
    ensure_legacy_decision_creation_disabled("accept_proposal_in_frontier_signed")?;
    let key = signing_key.ok_or_else(|| {
        "accept requires a registered human decision key; use a Decision Plan or verified detached authority"
            .to_string()
    })?;
    accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::LocalKey(key),
        None,
        None,
        false,
    )
}

/// Apply one detached accept only after [`authorize_proposal_accept`] minted
/// this opaque, proposal/head/reason-bound authority proof.
pub fn accept_proposal_in_frontier_with_authority(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    authority: &AcceptAuthorization,
    provenance: Option<&crate::provenance::Provenance>,
) -> Result<String, String> {
    ensure_legacy_decision_creation_disabled("accept_proposal_in_frontier_with_authority")?;
    accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::DetachedAccept(authority),
        provenance,
        None,
        false,
    )
}

/// Accept exactly one policy-head proposal at a caller-bound instant.
///
/// This is the transaction planner seam used by the human policy ceremony:
/// the CLI reads the key once, fixes one timestamp, signs both the policy and
/// the existing `review.accepted` authority event, then journals their public
/// postimages together. It cannot be used for ordinary proposal kinds.
pub fn accept_policy_head_proposal_in_frontier_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: &ed25519_dalek::SigningKey,
    decided_at: &str,
) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("policy-head decision time is invalid: {error}"))?;
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    if proposal.kind != policy_accept::POLICY_HEAD_PROPOSAL_KIND {
        return Err("fixed-time policy-head acceptance cannot accept another proposal kind".into());
    }
    accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::LocalKey(signing_key),
        None,
        Some(decided_at),
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn accept_proposal_in_frontier_with_authority_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    authority: DecisionAuthority<'_>,
    provenance: Option<&crate::provenance::Provenance>,
    fixed_decided_at: Option<&str>,
    record_review_event: bool,
) -> Result<String, String> {
    validate_reviewer_identity(reviewer)?;
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }
    let index = frontier
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    let status = frontier.proposals[index].status.clone();
    if status == "rejected" {
        return Err(format!("Cannot accept rejected proposal {}", proposal_id));
    }
    if status == "applied" {
        let proposal = frontier.proposals[index].clone();
        let decided_at = fixed_decided_at
            .map(ToString::to_string)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        enforce_decision_authority(
            frontier,
            &proposal,
            reviewer,
            reason,
            &decided_at,
            &authority,
        )?;
        return frontier.proposals[index]
            .applied_event_id
            .clone()
            .ok_or_else(|| format!("Proposal {} is applied but has no event id", proposal_id));
    }
    let proposal = frontier.proposals[index].clone();
    validate_proposal_shape(frontier, &proposal)?;
    let decided_at = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    enforce_decision_authority(
        frontier,
        &proposal,
        reviewer,
        reason,
        &decided_at,
        &authority,
    )?;
    let signing_key = authority.signing_key();
    if proposal.kind == policy_accept::POLICY_HEAD_PROPOSAL_KIND {
        if signing_key.is_none() && !record_review_event {
            return Err(
                "policy-head acceptance requires a real human event signature; custody-only or keyless review cannot activate authority"
                    .to_string(),
            );
        }
        if !(reviewer.starts_with("reviewer:") || reviewer.starts_with("steward:")) {
            return Err("policy-head acceptance requires a reviewer:/steward: actor".to_string());
        }
        let decided_at = decided_at.clone();
        let actor = validate_human_reviewer_authority_at(frontier, reviewer, &decided_at)?;
        if signing_key.is_some_and(|key| {
            !actor
                .public_key
                .eq_ignore_ascii_case(&hex::encode(key.verifying_key().to_bytes()))
        }) {
            return Err(
                "policy-head signer key does not match the resolved frontier actor".to_string(),
            );
        }
        let review_time = chrono::DateTime::parse_from_rfc3339(&decided_at)
            .map_err(|error| format!("policy-head decision time is invalid: {error}"))?;
        for parent in &frontier.events {
            let parent_time =
                chrono::DateTime::parse_from_rfc3339(&parent.timestamp).map_err(|error| {
                    format!("policy-head parent {} time is invalid: {error}", parent.id)
                })?;
            if parent_time >= review_time {
                return Err(format!(
                    "policy-head review must occur after causal parent {}",
                    parent.id
                ));
            }
        }
        let mut event = events::new_review_decision_event(
            &proposal.id,
            &proposal.kind,
            "accepted",
            None,
            reviewer,
            reason,
            Some(&decided_at),
        )?;
        if let Some(provenance) = provenance
            && !provenance.is_empty()
        {
            crate::provenance::attach_to_payload(&mut event.payload, provenance)?;
            event.id = events::compute_event_id(&event);
        }
        if let Some(key) = signing_key {
            event.signature = Some(crate::sign::sign_event(&event, key)?);
        }
        let event_id = event.id.clone();
        frontier.events.push(event);
        frontier.proposals[index].status = "applied".to_string();
        frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
        frontier.proposals[index].reviewed_at = Some(decided_at);
        frontier.proposals[index].decision_reason = Some(reason.to_string());
        frontier.proposals[index].applied_event_id = Some(event_id.clone());
        mark_proof_stale(
            frontier,
            format!("Accepted policy-head proposal {}", proposal.id),
        );
        return Ok(event_id);
    }
    frontier.proposals[index].status = "accepted".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(decided_at.clone());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    let domain_provenance = if record_review_event {
        None
    } else {
        provenance
    };
    let event_id = apply_proposal_at(
        frontier,
        &proposal,
        reviewer,
        reason,
        domain_provenance,
        fixed_decided_at,
    )?;
    frontier.proposals[index].status = "applied".to_string();
    frontier.proposals[index].applied_event_id = Some(event_id.clone());
    // Sign the accept event under the reviewer's key: the signature is
    // over the canonical event bytes (signature field excluded), so the
    // content-addressed id is unchanged and the accept is attributable
    // by cryptography, not by string.
    if let Some(key) = signing_key
        && let Some(ev) = frontier.events.iter_mut().find(|e| e.id == event_id)
    {
        ev.signature = Some(crate::sign::sign_event(ev, key)?);
    }
    if record_review_event {
        let decided_at = fixed_decided_at.ok_or_else(|| {
            "prepared acceptance requires a caller-bound decision time".to_string()
        })?;
        push_signed_review_event(
            frontier,
            &proposal.id,
            &proposal.kind,
            "accepted",
            Some(event_id.clone()),
            reviewer,
            reason,
            decided_at,
            signing_key,
            provenance,
        )?;
    }
    Ok(event_id)
}

/// Build, sign, and append a `review.*` decision event to the log. The
/// event is the tamper-evident, replayable record of the decision — the
/// thing a reject previously lacked entirely. Signed under the reviewer
/// key when present (custody is enforced by the caller before this runs),
/// so the decision is non-repudiable; the content-addressed id is over the
/// unsigned shape, so signing never changes it. `decided_at` is reused for
/// both the event timestamp and the proposal's `reviewed_at`, so the two
/// never diverge by a second clock read.
#[allow(clippy::too_many_arguments)]
fn push_signed_review_event(
    frontier: &mut Project,
    proposal_id: &str,
    proposal_kind: &str,
    verdict: &str,
    applied_event_id: Option<String>,
    reviewer: &str,
    reason: &str,
    decided_at: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
    provenance: Option<&crate::provenance::Provenance>,
) -> Result<String, String> {
    let mut event = events::new_review_decision_event(
        proposal_id,
        proposal_kind,
        verdict,
        applied_event_id,
        reviewer,
        reason,
        Some(decided_at),
    )?;
    if let Some(provenance) = provenance
        && !provenance.is_empty()
    {
        crate::provenance::attach_to_payload(&mut event.payload, provenance)?;
        event.id = events::compute_event_id(&event);
    }
    if let Some(key) = signing_key {
        event.signature = Some(crate::sign::sign_event(&event, key)?);
    }
    let event_id = event.id.clone();
    frontier.events.push(event);
    mark_proof_stale(
        frontier,
        format!("Recorded review decision on proposal {proposal_id} after latest proof export"),
    );
    Ok(event_id)
}

/// Reject a proposal, recording a signed `review.rejected` event. This is
/// the half of the lifecycle that used to leave no trace: a reject is now
/// as accountable as an accept — same key custody, same append-only signed
/// event, same replayability.
pub fn reject_proposal_in_frontier_signed(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
) -> Result<(), String> {
    ensure_legacy_decision_creation_disabled("reject_proposal_in_frontier_signed")?;
    let key = signing_key.ok_or_else(|| {
        "reject requires a registered human decision key or the Decision Plan ceremony".to_string()
    })?;
    let decided_at = Utc::now().to_rfc3339();
    reject_proposal_in_frontier_signed_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::LocalKey(key),
        None,
        &decided_at,
    )
}

#[allow(clippy::too_many_arguments)]
fn reject_proposal_in_frontier_signed_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    authority: DecisionAuthority<'_>,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(), String> {
    validate_reviewer_identity(reviewer)?;
    // A reject is a truth-bearing review verdict with no process exception:
    // burying a proposal is as much a decision as applying one. It is
    // reserved for a registered human reviewer executing a Decision Plan.
    if reviewer.starts_with("agent:") || reviewer.starts_with("ci:") {
        return Err(format!(
            "reviewer '{reviewer}' may not reject proposals: review decisions are \
             reserved for named human reviewers (key custody). Agents may propose, \
             attach mechanical evidence, or draft — never decide."
        ));
    }
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("decision time is invalid: {error}"))?;
    let index = frontier
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    match frontier.proposals[index].status.as_str() {
        "pending_review" | "accepted" | "needs_revision" => {}
        "rejected" => {
            return Err(format!("Proposal {} is already rejected", proposal_id));
        }
        "applied" => {
            return Err(format!("Proposal {} is already applied", proposal_id));
        }
        other => {
            return Err(format!("Unsupported proposal status '{}'", other));
        }
    }
    let proposal_kind = frontier.proposals[index].kind.clone();
    let proposal = frontier.proposals[index].clone();
    enforce_decision_authority(
        frontier, &proposal, reviewer, reason, decided_at, &authority,
    )?;
    if matches!(authority, DecisionAuthority::DetachedAccept(_)) {
        return Err("accept authority cannot authorize a rejection".to_string());
    }
    if matches!(authority, DecisionAuthority::VerifiedProcess(_)) {
        return Err("process write authority cannot authorize a rejection".to_string());
    }
    let signing_key = authority.signing_key();
    frontier.proposals[index].status = "rejected".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(decided_at.to_string());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    push_signed_review_event(
        frontier,
        proposal_id,
        &proposal_kind,
        "rejected",
        None,
        reviewer,
        reason,
        decided_at,
        signing_key,
        provenance,
    )?;
    Ok(())
}

fn prepared_decision_mutation(
    frontier: &Project,
    first_event: usize,
    primary_event_id: String,
    proposal_id: &str,
    reviewer: &str,
) -> Result<PreparedDecisionMutation, String> {
    let appended = frontier
        .events
        .get(first_event..)
        .ok_or_else(|| "prepared decision event range is invalid".to_string())?;
    let appended_event_ids = appended
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let decision_event_id = appended
        .iter()
        .rev()
        .find(|event| {
            event.target.r#type == "proposal"
                && event.target.id == proposal_id
                && matches!(
                    event.kind.as_str(),
                    events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
                )
        })
        .map(|event| event.id.clone())
        .ok_or_else(|| "prepared decision did not append a review decision event".to_string())?;
    let prepared = PreparedDecisionMutation {
        primary_event_id,
        appended_event_ids,
        decision_event_id,
        proposal_id: proposal_id.to_string(),
        reviewer: reviewer.to_string(),
        first_event,
        event_count_after: frontier.events.len(),
        binding: PreparedDecisionBinding::Unbound,
    };
    validate_prepared_decision_invariants(frontier, &prepared, true)?;
    Ok(prepared)
}

fn clone_project(project: &Project) -> Result<Project, String> {
    serde_json::from_value(serde_json::to_value(project).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn decision_root_refs(event: &StateEvent) -> Result<Vec<String>, String> {
    let Some(provenance) = event.payload.get("provenance") else {
        return Ok(Vec::new());
    };
    let object = provenance
        .as_object()
        .ok_or_else(|| format!("event {} provenance must be an object", event.id))?;
    let Some(input_refs) = object.get("input_refs") else {
        return Ok(Vec::new());
    };
    let input_refs = input_refs
        .as_array()
        .ok_or_else(|| format!("event {} provenance.input_refs must be an array", event.id))?;
    let mut refs = Vec::new();
    for value in input_refs {
        let value = value.as_str().ok_or_else(|| {
            format!(
                "event {} provenance.input_refs must contain only strings",
                event.id
            )
        })?;
        if value.starts_with(crate::provenance::DECISION_ROOT_INPUT_REF_PREFIX) {
            refs.push(value.to_string());
        }
    }
    Ok(refs)
}

fn validate_prepared_decision_invariants(
    frontier: &Project,
    prepared: &PreparedDecisionMutation,
    require_unsigned: bool,
) -> Result<(), String> {
    if prepared.appended_event_ids.is_empty() {
        return Err("prepared decision has no appended events".to_string());
    }
    if prepared.first_event + prepared.appended_event_ids.len() != prepared.event_count_after
        || prepared.event_count_after > frontier.events.len()
    {
        return Err("prepared decision event range is not contiguous".to_string());
    }
    let events = &frontier.events[prepared.first_event..prepared.event_count_after];
    let exact_ids = events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<Vec<_>>();
    if exact_ids
        != prepared
            .appended_event_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    {
        return Err("prepared decision ids do not match their contiguous event range".to_string());
    }
    let unique_ids = prepared.appended_event_ids.iter().collect::<BTreeSet<_>>();
    if unique_ids.len() != prepared.appended_event_ids.len() {
        return Err("prepared decision contains duplicate event ids".to_string());
    }
    if !unique_ids.contains(&prepared.primary_event_id)
        || !unique_ids.contains(&prepared.decision_event_id)
    {
        return Err(
            "prepared primary and decision events must belong to the exact event set".to_string(),
        );
    }
    for event_id in &prepared.appended_event_ids {
        if frontier
            .events
            .iter()
            .filter(|event| event.id == *event_id)
            .count()
            != 1
        {
            return Err(format!(
                "prepared decision event id is not globally unique: {event_id}"
            ));
        }
    }

    let proposals = frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.id == prepared.proposal_id)
        .collect::<Vec<_>>();
    if proposals.len() != 1 {
        return Err(format!(
            "prepared proposal {} is missing or duplicated",
            prepared.proposal_id
        ));
    }
    let proposal = proposals[0];
    let mut decision_events = 0_usize;
    for event in events {
        if event.id != events::compute_event_id(event) {
            return Err(format!("prepared event {} id does not rederive", event.id));
        }
        if event.actor.id != prepared.reviewer
            || event.actor.r#type != events::actor_kind(&prepared.reviewer)
        {
            return Err(format!(
                "prepared event {} actor does not match reviewer {}",
                event.id, prepared.reviewer
            ));
        }
        if event.payload.get("proposal_id").and_then(Value::as_str)
            != Some(prepared.proposal_id.as_str())
        {
            return Err(format!(
                "prepared event {} does not bind proposal {}",
                event.id, prepared.proposal_id
            ));
        }
        if require_unsigned && event.signature.is_some() {
            return Err(format!("prepared event {} is already signed", event.id));
        }
        let refs = decision_root_refs(event)?;
        let is_review_decision = event.target.r#type == "proposal"
            && event.target.id == prepared.proposal_id
            && matches!(
                event.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
            );
        if is_review_decision {
            decision_events += 1;
            if event.id != prepared.decision_event_id {
                return Err("prepared set contains an untracked review decision event".to_string());
            }
        }
        if event.id == prepared.decision_event_id {
            if !is_review_decision {
                return Err("prepared decision event has inconsistent target or kind".to_string());
            }
            match &prepared.binding {
                PreparedDecisionBinding::Unbound if !refs.is_empty() => {
                    return Err(
                        "unbound prepared decision already carries a decision-root reference"
                            .to_string(),
                    );
                }
                PreparedDecisionBinding::Unbound => {}
                PreparedDecisionBinding::Bound { decision_root } => {
                    let expected = crate::provenance::decision_root_input_ref(decision_root)?;
                    if refs != [expected] {
                        return Err(
                            "bound prepared decision must carry exactly one canonical decision-root reference"
                                .to_string(),
                        );
                    }
                }
            }
        } else if !refs.is_empty() {
            return Err(format!(
                "non-decision event {} carries a decision-root reference",
                event.id
            ));
        }
    }
    if decision_events != 1 {
        return Err("prepared decision must contain exactly one review decision event".to_string());
    }
    let decision_event = events
        .iter()
        .find(|event| event.id == prepared.decision_event_id)
        .expect("membership checked above");
    if decision_event
        .payload
        .get("proposal_kind")
        .and_then(Value::as_str)
        != Some(proposal.kind.as_str())
    {
        return Err("prepared decision event proposal kind is inconsistent".to_string());
    }
    if proposal.reviewed_by.as_deref() != Some(prepared.reviewer.as_str())
        || proposal.reviewed_at.as_deref() != Some(decision_event.timestamp.as_str())
    {
        return Err(
            "prepared proposal review metadata does not match its decision event".to_string(),
        );
    }
    match proposal.status.as_str() {
        "applied"
            if proposal.applied_event_id.as_deref() == Some(prepared.primary_event_id.as_str())
                && decision_event.kind == events::EVENT_KIND_REVIEW_ACCEPTED => {}
        "rejected"
            if proposal.applied_event_id.is_none()
                && decision_event.kind == events::EVENT_KIND_REVIEW_REJECTED => {}
        _ => {
            return Err(
                "prepared proposal status/applied_event_id is inconsistent with its primary event"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Apply one exact accept to an in-memory project without receiving or reading
/// a private key. The caller supplies the sole timestamp used by the proposal
/// status, every semantic state timestamp, and every appended event.
///
/// This mutates only the provided `Project`. It always appends an unsigned
/// `review.accepted` event in addition to any domain events. After the final
/// plan root is known, bind it with [`bind_decision_root_to_prepared`]; then,
/// after a locked rederivation, sign exactly the returned event set with
/// [`sign_prepared_decision_events`].
pub fn prepare_proposal_accept_in_memory_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_accept_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    *frontier = candidate;
    Ok(prepared)
}

fn prepare_proposal_accept_on_candidate(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
    let first_event = frontier.events.len();
    let primary_event_id = accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::PlanPreparation,
        provenance,
        Some(decided_at),
        true,
    )?;
    crate::sources::materialize_project(frontier);
    prepared_decision_mutation(
        frontier,
        first_event,
        primary_event_id,
        proposal_id,
        reviewer,
    )
}

/// Clone-only convenience wrapper for the first item in a Decision Plan.
/// Callers can chain later items by applying the mutating in-memory seam to the
/// returned candidate; the input project is never changed.
pub fn prepare_proposal_accept_candidate_at(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(Project, PreparedDecisionMutation), String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_accept_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    Ok((candidate, prepared))
}

/// Apply one exact reject to an in-memory project without receiving or reading
/// a private key. The result is an unsigned, fixed-time `review.rejected`
/// event, optionally carrying a decision-root provenance input reference.
pub fn prepare_proposal_reject_in_memory_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_reject_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    *frontier = candidate;
    Ok(prepared)
}

fn prepare_proposal_reject_on_candidate(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
    let first_event = frontier.events.len();
    reject_proposal_in_frontier_signed_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::PlanPreparation,
        provenance,
        decided_at,
    )?;
    let primary_event_id = frontier
        .events
        .get(first_event)
        .map(|event| event.id.clone())
        .ok_or_else(|| "prepared reject appended no event".to_string())?;
    prepared_decision_mutation(
        frontier,
        first_event,
        primary_event_id,
        proposal_id,
        reviewer,
    )
}

/// Clone-only reject counterpart to
/// [`prepare_proposal_accept_candidate_at`].
pub fn prepare_proposal_reject_candidate_at(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(Project, PreparedDecisionMutation), String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_reject_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    Ok((candidate, prepared))
}

/// Insert the final decision root into the prepared review event and rederive
/// its content address before any signature is made.
///
/// Only the dedicated `review.*` event carries this input reference. Domain and
/// fanout event IDs therefore remain stable, including the
/// `review.accepted.payload.applied_event_id` pointer, and the Decision Plan has
/// no hash cycle. Policy-head acceptance uses its review event as the primary
/// event, so this helper also updates the proposal's cached applied-event id.
pub fn bind_decision_root_to_prepared(
    frontier: &mut Project,
    prepared: &mut PreparedDecisionMutation,
    decision_root: &str,
) -> Result<(), String> {
    crate::provenance::decision_root_input_ref(decision_root)?;
    match prepared.decision_root() {
        Some(existing) if existing == decision_root => {
            return validate_prepared_decision_invariants(frontier, prepared, true);
        }
        Some(_) => {
            return Err(
                "prepared decision is already bound; rebuild it before changing the root"
                    .to_string(),
            );
        }
        None => {}
    }
    let mut candidate = clone_project(frontier)?;
    let mut candidate_prepared = prepared.clone();
    bind_decision_root_to_prepared_inner(&mut candidate, &mut candidate_prepared, decision_root)?;
    validate_prepared_decision_invariants(&candidate, &candidate_prepared, true)?;
    *frontier = candidate;
    *prepared = candidate_prepared;
    Ok(())
}

fn bind_decision_root_to_prepared_inner(
    frontier: &mut Project,
    prepared: &mut PreparedDecisionMutation,
    decision_root: &str,
) -> Result<(), String> {
    validate_prepared_decision_invariants(frontier, prepared, true)?;
    let old_event_id = prepared.decision_event_id.clone();
    let index = frontier
        .events
        .iter()
        .position(|event| event.id == old_event_id)
        .ok_or_else(|| format!("prepared decision event not found: {old_event_id}"))?;
    let event = &frontier.events[index];
    if event.signature.is_some() {
        return Err("cannot bind a decision root after the decision event is signed".to_string());
    }
    if !matches!(
        event.kind.as_str(),
        events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
    ) {
        return Err(format!(
            "prepared decision event {} has non-review kind '{}'",
            event.id, event.kind
        ));
    }
    let mut candidate_event: StateEvent =
        serde_json::from_value(serde_json::to_value(event).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let mut provenance = candidate_event
        .payload
        .get("provenance")
        .cloned()
        .map(serde_json::from_value::<crate::provenance::Provenance>)
        .transpose()
        .map_err(|error| format!("prepared decision provenance is invalid: {error}"))?
        .unwrap_or_default();
    provenance.bind_decision_root(decision_root)?;
    crate::provenance::attach_to_payload(&mut candidate_event.payload, &provenance)?;
    candidate_event.id = events::compute_event_id(&candidate_event);
    let new_event_id = candidate_event.id.clone();

    if frontier
        .events
        .iter()
        .enumerate()
        .any(|(other_index, other)| other_index != index && other.id == new_event_id)
    {
        return Err(format!(
            "decision-root binding collides with existing event {new_event_id}"
        ));
    }
    frontier.events[index] = candidate_event;
    for proposal in &mut frontier.proposals {
        if proposal.applied_event_id.as_deref() == Some(old_event_id.as_str()) {
            proposal.applied_event_id = Some(new_event_id.clone());
        }
    }
    for event_id in &mut prepared.appended_event_ids {
        if *event_id == old_event_id {
            *event_id = new_event_id.clone();
        }
    }
    if prepared.primary_event_id == old_event_id {
        prepared.primary_event_id = new_event_id.clone();
    }
    prepared.decision_event_id = new_event_id;
    prepared.binding = PreparedDecisionBinding::Bound {
        decision_root: decision_root.to_string(),
    };
    Ok(())
}

/// Sign the exact unsigned events returned by a prepared decision mutation.
///
/// Every event is validated before any signature is written, so a malformed or
/// stale set produces zero signature delta. This function performs no I/O and
/// no clock read; key resolution and human confirmation remain CLI concerns.
pub fn sign_prepared_decision_events(
    frontier: &mut Project,
    prepared: &PreparedDecisionMutation,
    reviewer: &str,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<(), String> {
    if reviewer != prepared.reviewer {
        return Err(format!(
            "prepared reviewer '{}' does not match signing reviewer '{reviewer}'",
            prepared.reviewer
        ));
    }
    if !prepared.is_bound() {
        return Err("prepared decision must be decision-root bound before signing".to_string());
    }
    validate_prepared_decision_invariants(frontier, prepared, true)?;
    let decision_event = &frontier.events[prepared.first_event
        + prepared
            .appended_event_ids
            .iter()
            .position(|event_id| event_id == &prepared.decision_event_id)
            .expect("prepared invariant checks decision-event membership")];
    let actor =
        validate_human_reviewer_authority_at(frontier, reviewer, &decision_event.timestamp)?;
    let derived = hex::encode(signing_key.verifying_key().to_bytes());
    if !derived.eq_ignore_ascii_case(&actor.public_key) {
        return Err(format!(
            "the supplied key does not match {reviewer}'s registered decision key"
        ));
    }
    let indexes = (prepared.first_event..prepared.event_count_after).collect::<Vec<_>>();
    let signatures = indexes
        .iter()
        .map(|index| crate::sign::sign_event(&frontier.events[*index], signing_key))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, signature) in indexes.into_iter().zip(signatures) {
        frontier.events[index].signature = Some(signature);
    }
    Ok(())
}

pub(crate) fn apply_proposal(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
) -> Result<String, String> {
    apply_proposal_at(
        frontier,
        proposal,
        reviewer,
        decision_reason,
        provenance,
        None,
    )
}

fn apply_proposal_at(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    fixed_decided_at: Option<&str>,
) -> Result<String, String> {
    // Phase L: retraction emits a fan of events — one for the source
    // and one `finding.dependency_invalidated` per dependent in BFS
    // order. apply_retract is responsible for pushing all of them in
    // sequence; this branch only assigns the primary event ID.
    if proposal.kind.as_str() == "finding.retract" {
        let events = apply_retract(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?;
        let primary_id = events
            .first()
            .map(|event| event.id.clone())
            .ok_or_else(|| "apply_retract returned no events".to_string())?;
        for event in events {
            frontier.events.push(event);
        }
        mark_proof_stale(
            frontier,
            format!("Applied proposal {} after latest proof export", proposal.id),
        );
        return Ok(primary_id);
    }
    // v0.55: confidence_revise can also fan out a cascade when the new
    // score crosses below the 0.5 propagation threshold. Same fan-out
    // pattern as retract.
    if proposal.kind.as_str() == "finding.confidence_revise" {
        let events = apply_confidence_revise(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?;
        let primary_id = events
            .first()
            .map(|event| event.id.clone())
            .ok_or_else(|| "apply_confidence_revise returned no events".to_string())?;
        for event in events {
            frontier.events.push(event);
        }
        mark_proof_stale(
            frontier,
            format!("Applied proposal {} after latest proof export", proposal.id),
        );
        return Ok(primary_id);
    }
    let mut event = match proposal.kind.as_str() {
        "finding.add" => apply_add(frontier, proposal, reviewer, decision_reason)?,
        "finding.review" => apply_review(frontier, proposal, reviewer, decision_reason)?,
        "research_trace.review" | "correction_return.review" => {
            apply_frontier_observation_review(proposal, reviewer, decision_reason)?
        }
        "finding.caveat" => apply_caveat(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "finding.note" => apply_note(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "finding.reject" => apply_reject(frontier, proposal, reviewer, decision_reason)?,
        "finding.contribution.recorded" => {
            apply_contribution(frontier, proposal, reviewer, decision_reason)?
        }
        "finding.supersede" => apply_supersede(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "artifact.assert" => apply_artifact_assert(frontier, proposal, reviewer, decision_reason)?,
        "artifact.retract" => {
            apply_artifact_retract(frontier, proposal, reviewer, decision_reason)?
        }
        "verifier.attach" => apply_verifier_attach(frontier, proposal, reviewer, decision_reason)?,
        // v0.56: mechanical evidence-atom locator repair.
        "evidence_atom.locator_repair" => {
            apply_evidence_atom_locator_repair(frontier, proposal, reviewer, decision_reason)?
        }
        // v0.57: mechanical finding-level span repair.
        "finding.span_repair" => {
            apply_finding_span_repair(frontier, proposal, reviewer, decision_reason)?
        }
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    };
    if let Some(decided_at) = fixed_decided_at {
        event.timestamp = decided_at.to_string();
        event.id = events::compute_event_id(&event);
    }
    // Co-authorship: when a non-human (an AI that drafted, CI that attested)
    // contributed, record it as signed-over attribution on this single decision
    // event. The reviewer stays the accountable signer; the provenance carries
    // zero authority (validated non-human in `attach_to_payload`). Because the
    // block enters the signed payload, the content-addressed id is re-derived.
    // None leaves the event byte-identical, so existing frontiers are untouched.
    if let Some(prov) = provenance
        && !prov.is_empty()
    {
        crate::provenance::attach_to_payload(&mut event.payload, prov)?;
        event.id = events::event_id(&event);
    }
    let event_id = event.id.clone();
    frontier.events.push(event);
    mark_proof_stale(
        frontier,
        format!("Applied proposal {} after latest proof export", proposal.id),
    );
    Ok(event_id)
}

fn apply_frontier_observation_review(
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
) -> Result<StateEvent, String> {
    match proposal.kind.as_str() {
        "research_trace.review" => validate_research_trace_review_payload(proposal)?,
        "correction_return.review" => validate_correction_return_review_payload(proposal)?,
        other => {
            return Err(format!(
                "Unsupported frontier observation proposal kind '{other}'"
            ));
        }
    }
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_FRONTIER_OBSERVATION_REVIEWED.into(),
        target: proposal.target.clone(),
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: "human".to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
            "proposal_kind": proposal.kind,
            "status": "accepted",
            "decision_reason": decision_reason,
            "reviewed_payload": proposal.payload,
            "source_refs": proposal.source_refs,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
        schema_artifact_id: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

/// v0.14: `finding.supersede` — first-class flow for *changing a claim's text*.
///
/// Until v0.14 the only way to update a finding was to stack caveats/notes
/// on top, because the assertion text is part of the content address. The
/// substrate-correct path for a real correction is a *new* content-addressed
/// finding that explicitly supersedes the old one. This proposal kind:
///
/// 1. Validates the old finding exists and is not already superseded.
/// 2. Adds the new finding bundle (a fresh `vf_…` content address) to
///    `frontier.findings`.
/// 3. Auto-injects a `supersedes` link from the new finding's `links` to the
///    old finding's id (if not already present in the payload).
/// 4. Sets `flags.superseded = true` on the old finding.
/// 5. Emits a `finding.superseded` canonical event targeting the *old*
///    finding (since that's the state change). The new finding's existence
///    is recorded in the event payload as `new_finding_id`.
///
/// Both findings remain queryable; readers walk the supersedes chain via
/// the link or via the `flags.superseded` marker.
fn apply_supersede(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    use crate::bundle::Link;

    let old_id = proposal.target.id.clone();
    let new_finding_value = proposal
        .payload
        .get("new_finding")
        .ok_or("finding.supersede proposal missing payload.new_finding")?
        .clone();
    let mut new_finding: FindingBundle = serde_json::from_value(new_finding_value)
        .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;

    // Locate the old finding before mutating; capture before_hash for the event.
    let old_idx = find_finding_index(frontier, &old_id)?;
    if frontier.findings[old_idx].flags.superseded {
        return Err(format!(
            "Refusing to supersede already-superseded finding {old_id}"
        ));
    }
    if new_finding.id == old_id {
        return Err(
            "Refusing to supersede with a finding that has the same content address as the old finding (assertion / type / provenance_id are unchanged)".to_string(),
        );
    }
    if frontier
        .findings
        .iter()
        .any(|existing| existing.id == new_finding.id)
    {
        return Err(format!(
            "Refusing to add superseding finding with existing finding ID {}",
            new_finding.id
        ));
    }
    let before_hash = events::finding_hash(&frontier.findings[old_idx]);

    // Auto-inject the supersedes link if the caller didn't already include it.
    let already_links_old = new_finding
        .links
        .iter()
        .any(|l| l.target == old_id && l.link_type == "supersedes");
    if !already_links_old {
        new_finding.links.push(Link {
            target: old_id.clone(),
            link_type: "supersedes".to_string(),
            note: format!(
                "Supersedes {old_id} via finding.supersede proposal {}.",
                proposal.id
            ),
            inferred_by: "reviewer".to_string(),
            created_at: fixed_decided_at
                .map(ToString::to_string)
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
            mechanism: None,
        });
    }

    let new_finding_id = new_finding.id.clone();
    frontier.findings.push(new_finding);
    frontier.findings[old_idx].flags.superseded = true;
    let after_hash = events::finding_hash(&frontier.findings[old_idx]);

    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.superseded",
        finding_id: &old_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "new_finding_id": new_finding_id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: fixed_decided_at,
    }))
}

fn apply_add(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_value = proposal
        .payload
        .get("finding")
        .ok_or("finding.add proposal missing payload.finding")?
        .clone();
    let finding: FindingBundle = serde_json::from_value(finding_value)
        .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
    let finding_id = finding.id.clone();
    // Activity is not state: an accepted finding may not depend on an
    // activity-plane id (`vac_`/`vrr_`). A search/trace/retrieval is recorded in
    // the activity plane and referenced by content address, never admitted as
    // accepted lineage (the `activity::assert_not_in_lineage` law, at the write).
    if let Some(l) = finding
        .links
        .iter()
        .find(|l| crate::activity::is_activity_id(&l.target))
    {
        return Err(format!(
            "finding.add refused: link target `{}` is an activity-plane id; activity is non-authoritative and cannot enter lineage",
            l.target
        ));
    }
    if frontier
        .findings
        .iter()
        .any(|existing| existing.id == finding_id)
    {
        return Err(format!(
            "Refusing to add duplicate finding with existing finding ID {finding_id}"
        ));
    }
    // Prior-art collision: an EXACT duplicate of an accepted finding's
    // statement is refused unless the proposal names what it supersedes
    // (the Sakana rediscovery failure mode, made mechanical).
    {
        let new_hash = crate::canonical::normalized_statement_hash(&finding.assertion.text);
        let declares_supersession = proposal.payload.get("supersedes").is_some()
            || proposal.payload.get("improves_on").is_some();
        if !declares_supersession
            && let Some(dup) = frontier.findings.iter().find(|f| {
                crate::canonical::normalized_statement_hash(&f.assertion.text) == new_hash
            })
        {
            return Err(format!(
                "prior-art collision: statement duplicates accepted finding {} — name it via payload.supersedes/improves_on or change the claim",
                dup.id
            ));
        }
    }
    frontier.findings.push(finding);
    let after_hash = events::finding_hash_by_id(frontier, &finding_id);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.asserted",
        finding_id: &finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: NULL_HASH,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_artifact_assert(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let artifact_value = proposal
        .payload
        .get("artifact")
        .ok_or("artifact.assert proposal missing payload.artifact")?
        .clone();
    let artifact: Artifact = serde_json::from_value(artifact_value)
        .map_err(|e| format!("Invalid artifact.assert payload: {e}"))?;
    let artifact_id = artifact.id.clone();
    if frontier
        .artifacts
        .iter()
        .any(|existing| existing.id == artifact_id)
    {
        return Err(format!(
            "Refusing to add duplicate artifact with existing id {artifact_id}"
        ));
    }
    frontier.artifacts.push(artifact.clone());
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_ARTIFACT_ASSERTED.into(),
        target: StateTarget {
            r#type: "artifact".to_string(),
            id: artifact_id,
        },
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: if reviewer.starts_with("agent:") {
                "agent"
            } else {
                "human"
            }
            .to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
            "artifact": artifact,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
        schema_artifact_id: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

fn apply_artifact_retract(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let artifact_id = proposal.target.id.as_str();
    let artifact = frontier
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| format!("Artifact not found: {artifact_id}"))?;
    if artifact.retracted {
        return Err(format!("Artifact {artifact_id} is already retracted"));
    }
    artifact.retracted = true;
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_ARTIFACT_RETRACTED.into(),
        target: StateTarget {
            r#type: "artifact".to_string(),
            id: artifact_id.to_string(),
        },
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: events::actor_kind(reviewer).to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
        schema_artifact_id: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

/// Bind a verifier attachment to a finding (`target.type == "finding"`). Appends
/// to the sidecar `verifier_attachments` collection and emits
/// `verifier_attachment.added`. Per-finding trust-gate status is derived on read.
fn apply_verifier_attach(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    if proposal.target.r#type != "finding" {
        return Err(format!(
            "verifier.attach target.type must be 'finding', got '{}'",
            proposal.target.r#type
        ));
    }
    let value = proposal
        .payload
        .get("attachment")
        .ok_or("verifier.attach proposal missing payload.attachment")?
        .clone();
    let att: crate::verifier_attachment::VerifierAttachment = serde_json::from_value(value)
        .map_err(|e| format!("Invalid verifier.attach payload: {e}"))?;
    att.verify()
        .map_err(|e| format!("verifier.attach attachment malformed: {e}"))?;
    if att.target != proposal.target.id {
        return Err(format!(
            "verifier.attach attachment.target {} does not match proposal target {}",
            att.target, proposal.target.id
        ));
    }
    // Activity is not state: a verifier gate may not attach to, or claim
    // independence from, an activity-plane id (`vac_`/`vrr_`).
    if crate::activity::is_activity_id(&att.target) {
        return Err(format!(
            "verifier.attach refused: target `{}` is an activity-plane id (activity is not lineage)",
            att.target
        ));
    }
    if let Some(indep) = att
        .independent_of
        .iter()
        .find(|i| crate::activity::is_activity_id(i))
    {
        return Err(format!(
            "verifier.attach refused: independent_of `{indep}` is an activity-plane id"
        ));
    }
    if !frontier.verifier_attachments.iter().any(|a| a.id == att.id) {
        frontier.verifier_attachments.push(att.clone());
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: events::EVENT_KIND_VERIFIER_ATTACHMENT_ADDED,
        finding_id: &proposal.target.id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({ "proposal_id": proposal.id, "attachment": att }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_review(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let status = proposal
        .payload
        .get("status")
        .and_then(Value::as_str)
        .ok_or("finding.review proposal missing payload.status")?;
    use crate::bundle::ReviewState;
    let new_state = match status {
        "accepted" | "approved" => ReviewState::Accepted,
        "contested" => ReviewState::Contested,
        "needs_revision" => ReviewState::NeedsRevision,
        "rejected" => ReviewState::Rejected,
        other => return Err(format!("Unknown review proposal status '{other}'")),
    };
    frontier.findings[idx].flags.contested = new_state.implies_contested();
    frontier.findings[idx].flags.review_state = Some(new_state);
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.reviewed",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "status": status,
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

/// Append a claim-granularity attribution to a finding's provenance. Descriptive
/// only: no flag, no confidence, no review state. Idempotent — a contribution
/// already present (same unit + agent + role) is not re-added.
fn apply_contribution(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let contribution: crate::bundle::Contribution = serde_json::from_value(
        proposal
            .payload
            .get("contribution")
            .cloned()
            .ok_or("finding.contribution.recorded proposal missing payload.contribution")?,
    )
    .map_err(|e| format!("malformed contribution: {e}"))?;
    contribution.validate()?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let existing = &mut frontier.findings[idx].provenance.contributions;
    if !existing.iter().any(|c| {
        c.unit == contribution.unit
            && c.agent_id == contribution.agent_id
            && c.role == contribution.role
    }) {
        existing.push(contribution.clone());
    }
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let payload = json!({
        "contribution": serde_json::to_value(&contribution)
            .map_err(|e| format!("serialize contribution: {e}"))?,
        "proposal_id": proposal.id,
    });
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.contribution.recorded",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_caveat(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.caveat proposal missing payload.text")?;
    let provenance = extract_annotation_provenance(&proposal.payload);
    let annotation_id = annotation_id(finding_id, text, reviewer, &now);
    frontier.findings[idx].annotations.push(Annotation {
        id: annotation_id.clone(),
        text: text.to_string(),
        author: reviewer.to_string(),
        timestamp: now.clone(),
        provenance: provenance.clone(),
    });
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let mut payload = json!({
        "annotation_id": annotation_id,
        "text": text,
        "proposal_id": proposal.id,
    });
    if let Some(prov) = &provenance {
        payload["provenance"] = serde_json::to_value(prov).unwrap_or(Value::Null);
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.caveated",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    }))
}

fn apply_note(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.note proposal missing payload.text")?;
    let provenance = extract_annotation_provenance(&proposal.payload);
    let annotation_id = annotation_id(finding_id, text, reviewer, &now);
    frontier.findings[idx].annotations.push(Annotation {
        id: annotation_id.clone(),
        text: text.to_string(),
        author: reviewer.to_string(),
        timestamp: now.clone(),
        provenance: provenance.clone(),
    });
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let mut payload = json!({
        "annotation_id": annotation_id,
        "text": text,
        "proposal_id": proposal.id,
    });
    if let Some(prov) = &provenance {
        payload["provenance"] = serde_json::to_value(prov).unwrap_or(Value::Null);
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.noted",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    }))
}

/// v0.57: Apply a `finding.span_repair` proposal. Appends a
/// `{section, text}` span to `state.findings[i].evidence.evidence_spans`
/// and emits one signed `finding.span_repaired` event.
fn apply_finding_span_repair(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let section = proposal
        .payload
        .get("section")
        .and_then(Value::as_str)
        .ok_or("finding.span_repair proposal missing payload.section")?
        .to_string();
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.span_repair proposal missing payload.text")?
        .to_string();
    let idx = find_finding_index(frontier, finding_id)?;
    let already_present = frontier.findings[idx]
        .evidence
        .evidence_spans
        .iter()
        .any(|existing| {
            existing.get("section").and_then(Value::as_str) == Some(section.as_str())
                && existing.get("text").and_then(Value::as_str) == Some(text.as_str())
        });
    if already_present {
        return Err(format!(
            "finding {finding_id} already carries an identical (section, text) span"
        ));
    }
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let span_value = json!({"section": section, "text": text});
    frontier.findings[idx]
        .evidence
        .evidence_spans
        .push(span_value);
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let payload = json!({
        "proposal_id": proposal.id,
        "section": section,
        "text": text,
    });
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.span_repaired",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

/// v0.56: Apply an `evidence_atom.locator_repair` proposal. Sets
/// `locator` on the named evidence atom, removes the
/// "missing evidence locator" caveat, and emits one signed
/// `evidence_atom.locator_repaired` canonical event. The before/after
/// hashes are over the canonical bytes of the named atom only, so a
/// chain validator can confirm the exact atom changed and exactly the
/// named repair was applied.
fn apply_evidence_atom_locator_repair(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let atom_id = proposal.target.id.as_str();
    let locator = proposal
        .payload
        .get("locator")
        .and_then(Value::as_str)
        .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?
        .to_string();
    let source_id = proposal
        .payload
        .get("source_id")
        .and_then(Value::as_str)
        .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?
        .to_string();

    let idx = frontier
        .evidence_atoms
        .iter()
        .position(|atom| atom.id == atom_id)
        .ok_or_else(|| format!("evidence_atom.locator_repair targets unknown atom {atom_id}"))?;
    if frontier.evidence_atoms[idx].source_id != source_id {
        return Err(format!(
            "evidence_atom.locator_repair payload.source_id '{source_id}' does not match atom.source_id '{}'",
            frontier.evidence_atoms[idx].source_id
        ));
    }
    if let Some(existing) = &frontier.evidence_atoms[idx].locator {
        if existing == &locator {
            return Err(format!(
                "evidence_atom {atom_id} already carries locator '{existing}'"
            ));
        }
        return Err(format!(
            "evidence_atom {atom_id} already carries locator '{existing}'; refusing to overwrite with '{locator}'"
        ));
    }

    let before_hash = events::evidence_atom_hash(&frontier.evidence_atoms[idx]);
    frontier.evidence_atoms[idx].locator = Some(locator.clone());
    frontier.evidence_atoms[idx]
        .caveats
        .retain(|c| c != "missing evidence locator");
    let after_hash = events::evidence_atom_hash(&frontier.evidence_atoms[idx]);

    let payload = json!({
        "proposal_id": proposal.id,
        "locator": locator,
        "source_id": source_id,
    });

    Ok(events::new_evidence_atom_locator_repair_event(
        atom_id,
        reviewer,
        "human",
        &proposal.reason,
        &before_hash,
        &after_hash,
        payload,
        proposal.caveats.clone(),
    ))
}

/// Phase β (v0.6): pull optional structured provenance off a note/caveat
/// proposal payload. The propose-* tools accept it; the validator gates
/// it; this helper threads it through to the materialized annotation
/// and the canonical event payload.
fn extract_annotation_provenance(payload: &Value) -> Option<crate::bundle::ProvenanceRef> {
    let prov = payload.get("provenance")?;
    let parsed: crate::bundle::ProvenanceRef = serde_json::from_value(prov.clone()).ok()?;
    if parsed.has_identifier() {
        Some(parsed)
    } else {
        None
    }
}

fn apply_confidence_revise(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<Vec<StateEvent>, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let previous = frontier.findings[idx].confidence.score;
    let new_score = proposal
        .payload
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or("finding.confidence_revise proposal missing payload.confidence")?;

    // v0.55: when the revised confidence crosses the propagation threshold
    // (previous >= 0.5, new < 0.5), invoke the same cascade pattern that
    // `apply_retract` uses — emit `finding.dependency_invalidated` events for
    // each downstream supports/depends finding at depth ≤ MAX_DEPTH. Pre-v0.55
    // this path silently mutated confidence without firing the cascade, which
    // forced callers to chase a separate `vela propagate --reduce-confidence`
    // command for the substrate's signature feature.
    let cascade_threshold_crossed = previous >= 0.5 && new_score < 0.5;

    let pre_cascade_hashes: std::collections::HashMap<String, String> = if cascade_threshold_crossed
    {
        frontier
            .findings
            .iter()
            .map(|finding| (finding.id.clone(), events::finding_hash(finding)))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let before_hash = events::finding_hash(&frontier.findings[idx]);

    // Apply the local mutation first so propagate_correction sees the new
    // confidence on the source finding.
    frontier.findings[idx].confidence.score = new_score;
    frontier.findings[idx].confidence.basis = format!(
        "expert revision from {:.3} to {:.3}: {}",
        previous, new_score, proposal.reason
    );
    frontier.findings[idx].confidence.method = ConfidenceMethod::ExpertJudgment;
    frontier.findings[idx].updated = Some(now.clone());

    let cascade = if cascade_threshold_crossed {
        Some(propagate::propagate_correction(
            frontier,
            finding_id,
            PropagationAction::ConfidenceReduced { new_score },
        ))
    } else {
        None
    };

    let after_hash = events::finding_hash(&frontier.findings[idx]);

    let source_event = events::new_finding_event(events::FindingEventInput {
        kind: "finding.confidence_revised",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "previous_score": previous,
            "new_score": new_score,
            "updated_at": now,
            "proposal_id": proposal.id,
            "cascade_fired": cascade_threshold_crossed,
            "affected": cascade.as_ref().map(|c| c.affected).unwrap_or(0),
        }),
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    });

    let source_event_id = source_event.id.clone();
    let mut emitted = vec![source_event];

    if let Some(cascade) = cascade {
        // Mirror apply_retract's per-dependent dependency_invalidated emission:
        // each affected dep at each depth gets a canonical event with the
        // before/after hash boundary so chain validation works downstream.
        for (depth_idx, level) in cascade.cascade.iter().enumerate() {
            let depth = (depth_idx as u32) + 1;
            for dep_id in level {
                let before = pre_cascade_hashes
                    .get(dep_id)
                    .cloned()
                    .unwrap_or_else(|| events::NULL_HASH.to_string());
                let after = events::finding_hash_by_id(frontier, dep_id);
                emitted.push(events::new_finding_event(events::FindingEventInput {
                    kind: "finding.dependency_invalidated",
                    finding_id: dep_id,
                    actor_id: reviewer,
                    actor_type: events::actor_kind(reviewer),
                    reason: &format!(
                        "Upstream finding {finding_id} confidence reduced to {new_score:.2}; cascade depth {depth}"
                    ),
                    before_hash: &before,
                    after_hash: &after,
                    payload: json!({
                        "upstream_finding_id": finding_id,
                        "upstream_event_id": source_event_id,
                        "depth": depth,
                        "new_score": new_score,
                        "previous_score": previous,
                        "proposal_id": proposal.id,
                    }),
                    caveats: vec![],
                    timestamp: fixed_decided_at,
                }));
            }
        }
    }

    Ok(emitted)
}

fn apply_reject(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    frontier.findings[idx].flags.contested = true;
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.rejected",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "status": "rejected",
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_retract(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<Vec<StateEvent>, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    if frontier.findings[idx].flags.retracted {
        return Err(format!("Finding {finding_id} is already retracted"));
    }
    // Phase L: capture every finding's pre-cascade hash so each emitted
    // `finding.dependency_invalidated` event can name a real before_hash
    // that matches whatever event last touched that dep.
    let pre_cascade_hashes: std::collections::HashMap<String, String> = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.clone(), events::finding_hash(finding)))
        .collect();

    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let cascade =
        propagate::propagate_correction(frontier, finding_id, PropagationAction::Retracted);
    let after_hash = events::finding_hash_by_id(frontier, finding_id);

    let source_event = events::new_finding_event(events::FindingEventInput {
        kind: "finding.retracted",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "affected": cascade.affected,
            "cascade": cascade.cascade,
        }),
        caveats: vec!["Retraction impact is simulated over declared dependency links.".to_string()],
        timestamp: fixed_decided_at,
    });
    let source_event_id = source_event.id.clone();

    let mut emitted = vec![source_event];

    // Phase L: emit one canonical `finding.dependency_invalidated`
    // event per affected dependent, in BFS depth order. Each event
    // carries the before/after hash boundary for that specific dep so
    // chain validation works downstream.
    for (depth_idx, level) in cascade.cascade.iter().enumerate() {
        let depth = (depth_idx as u32) + 1;
        for dep_id in level {
            let before = pre_cascade_hashes
                .get(dep_id)
                .cloned()
                .unwrap_or_else(|| events::NULL_HASH.to_string());
            let after = events::finding_hash_by_id(frontier, dep_id);
            emitted.push(events::new_finding_event(events::FindingEventInput {
                kind: "finding.dependency_invalidated",
                finding_id: dep_id,
                actor_id: reviewer,
                actor_type: events::actor_kind(reviewer),
                reason: &format!("Upstream finding {finding_id} retracted; cascade depth {depth}"),
                before_hash: &before,
                after_hash: &after,
                payload: json!({
                    "upstream_finding_id": finding_id,
                    "upstream_event_id": source_event_id,
                    "depth": depth,
                    "proposal_id": proposal.id,
                }),
                caveats: vec![],
                timestamp: fixed_decided_at,
            }));
        }
    }

    Ok(emitted)
}

fn find_finding_index(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

fn annotation_id(finding_id: &str, text: &str, author: &str, timestamp: &str) -> String {
    let hash = Sha256::digest(format!("{finding_id}|{text}|{author}|{timestamp}").as_bytes());
    format!("ann_{}", &hex::encode(hash)[..16])
}

// ── Review-decision projection + parity (status derived from the log) ──
//
// A proposal's decision state is no longer a free-floating mutable field:
// it is a PROJECTION of the signed `review.*` events (and, for accepts,
// the domain event the accept produced). The stored `status` is a cache
// of that projection. `verify_proposal_decision_parity` is the gate that
// pins the cache to the log — if someone hand-edits a `status` field, or a
// decision exists with no signed event behind it, parity fails. That is
// the tamper-evidence the mutable field never had.

/// A decision reconstructed from the event log for one proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedDecision {
    /// `applied` | `rejected` | `needs_revision`.
    pub status: String,
    /// The reviewer that made the latest decision.
    pub reviewer: String,
    /// The latest decision event's timestamp.
    pub decided_at: String,
    /// The `review.*` event id that carried the decision, when one exists.
    /// `None` for an accept whose only trace is its domain event (the
    /// pre-`review.accepted` accept path; see module note).
    pub review_event_id: Option<String>,
}

/// Reduce the event log to the current decision for a single proposal.
///
/// Folds, in timestamp order:
///   - `review.rejected` → rejected
///   - `review.revision_requested` → needs_revision
///   - `review.accepted` → applied
///   - any domain event produced by an accept of this proposal
///     (matched via the proposal's `applied_event_id`) → applied
///
/// The latest decision wins. Returns `None` when no decision event exists
/// (the proposal is pending).
pub fn proposal_status_from_log(
    frontier: &Project,
    proposal_id: &str,
    applied_event_id: Option<&str>,
) -> Option<DerivedDecision> {
    let mut decisions: Vec<DerivedDecision> = Vec::new();
    for event in &frontier.events {
        let is_review_for_this = event.target.r#type == "proposal"
            && event.target.id == proposal_id
            && matches!(
                event.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED
                    | events::EVENT_KIND_REVIEW_REJECTED
                    | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
            );
        if is_review_for_this {
            let status = match event.kind.as_str() {
                events::EVENT_KIND_REVIEW_ACCEPTED => "applied",
                events::EVENT_KIND_REVIEW_REJECTED => "rejected",
                _ => "needs_revision",
            };
            decisions.push(DerivedDecision {
                status: status.to_string(),
                reviewer: event.actor.id.clone(),
                decided_at: event.timestamp.clone(),
                review_event_id: Some(event.id.clone()),
            });
            continue;
        }
        // An accept's domain event is its decision trace when no explicit
        // review.accepted exists (the historical accept path).
        if let Some(applied) = applied_event_id
            && event.id == applied
        {
            decisions.push(DerivedDecision {
                status: "applied".to_string(),
                reviewer: event.actor.id.clone(),
                decided_at: event.timestamp.clone(),
                review_event_id: None,
            });
        }
    }
    decisions.sort_by(|a, b| a.decided_at.cmp(&b.decided_at));
    decisions.pop()
}

/// Verify that every proposal's stored decision state is backed by the
/// event log, and vice versa. Returns a list of human-readable conflicts
/// (empty == parity holds). This is the invariant the conformance gate
/// runs: it makes the mutable `status` field a verifiable projection
/// rather than an unconstrained side-table.
///
/// Checks, per proposal:
///   - a decided status (`applied` / `rejected` / `needs_revision`) MUST
///     have a backing event in the log (a `review.*` event, or for
///     `applied` the referenced domain event);
///   - the stored status MUST equal the status derived from the log;
///   - `pending_review` MUST NOT have a decision event.
/// And globally:
///   - every `review.*` event MUST reference a proposal that exists.
pub fn verify_proposal_decision_parity(frontier: &Project) -> Vec<String> {
    let mut conflicts = Vec::new();
    let proposal_ids: BTreeSet<&str> = frontier.proposals.iter().map(|p| p.id.as_str()).collect();

    for proposal in &frontier.proposals {
        let derived =
            proposal_status_from_log(frontier, &proposal.id, proposal.applied_event_id.as_deref());
        match proposal.status.as_str() {
            "pending_review" => {
                if let Some(d) = derived {
                    conflicts.push(format!(
                        "proposal {} is stored pending_review but the log carries a {} decision ({})",
                        proposal.id,
                        d.status,
                        d.review_event_id.as_deref().unwrap_or("domain event")
                    ));
                }
            }
            "accepted" => {
                // Transient in-memory state only; never persisted.
                conflicts.push(format!(
                    "proposal {} is stored in transient 'accepted' state (should be 'applied')",
                    proposal.id
                ));
            }
            stored @ ("applied" | "rejected" | "needs_revision") => match derived {
                None => conflicts.push(format!(
                    "proposal {} is stored '{}' but NO decision event backs it in the log \
                     — a decision with no signed, replayable record (the silent-drop vector)",
                    proposal.id, stored
                )),
                Some(d) if d.status != stored => conflicts.push(format!(
                    "proposal {} is stored '{}' but the log's latest decision is '{}'",
                    proposal.id, stored, d.status
                )),
                Some(_) => {}
            },
            other => conflicts.push(format!(
                "proposal {} has unknown status '{}'",
                proposal.id, other
            )),
        }
    }

    for event in &frontier.events {
        if matches!(
            event.kind.as_str(),
            events::EVENT_KIND_REVIEW_ACCEPTED
                | events::EVENT_KIND_REVIEW_REJECTED
                | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
        ) && !proposal_ids.contains(event.target.id.as_str())
        {
            conflicts.push(format!(
                "review event {} targets proposal {} which does not exist in the frontier",
                event.id, event.target.id
            ));
        }
    }

    conflicts
}

fn build_changed_finding_details(
    before: &Project,
    after: &Project,
    ids: &[String],
) -> Vec<ChangedFindingDetail> {
    ids.iter()
        .map(|id| {
            let fa = before.findings.iter().find(|f| &f.id == id);
            let fb = after.findings.iter().find(|f| &f.id == id);
            ChangedFindingDetail {
                id: id.clone(),
                assertion_before: fa.map(|f| f.assertion.text.clone()),
                assertion_after: fb.map(|f| f.assertion.text.clone()),
                assertion_type_before: fa.map(|f| f.assertion.assertion_type.clone()),
                assertion_type_after: fb.map(|f| f.assertion.assertion_type.clone()),
                confidence_before: fa.map(|f| format!("{:.2}", f.confidence.score)),
                confidence_after: fb.map(|f| format!("{:.2}", f.confidence.score)),
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
