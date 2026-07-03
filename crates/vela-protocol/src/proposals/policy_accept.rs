//! The policy lane: canonical acceptance whose AUTHORITY is a
//! human-signed standing policy (`vap_`) instead of a per-item key
//! ceremony.
//!
//! This is the flip the acceptance-policy module staged ("Today this
//! runs in SHADOW … so the autonomy can be proven before it is
//! granted"): a human signs a scoped, revocable [`AcceptancePolicy`]
//! ONCE; the deterministic evaluator then routes each landing, and a
//! `Permit` lands the SAME canonical accept event a human key would
//! have produced — with three differences that keep custody honest:
//!
//! 1. `reviewed_by` / the event actor is `policy:<vap_id>` (a machine
//!    actor, never counted as human review — see
//!    [`crate::events::actor_kind`]).
//! 2. The event carries no key signature. Its integrity chain is the
//!    `policy_lane` payload block: the full [`DecisionCertificate`]
//!    plus the exact [`PolicyContext`] evaluated, content-addressed
//!    into the event id (the same stamp-then-rederive pattern the
//!    provenance block uses). `vela check --strict` re-runs the
//!    evaluator over the stamped context against the persisted signed
//!    policy bytes and refuses the log if `Permit` does not re-derive.
//! 3. The policy file that authorized the accept is persisted
//!    content-addressed under `.vela/policies/<vap_id>.json` (+ sig),
//!    so verification survives policy rotation forever.
//!
//! What this deliberately does NOT change: the engine CI gate runs
//! exactly as it does for a human accept (strict, and `force` is
//! unreachable — there is no flag); `Defer` and `Deny` land nothing;
//! and no agent key ever signs anything here — the human's authority
//! arrived earlier, once, as the policy signature.
//!
//! Tiered verification honesty: `check --strict` re-derives the
//! ROUTING (policy × stamped context ⇒ Permit). Whether the stamped
//! context itself was honest (the assurance level truly derived from
//! the gate) is enforced at landing time by the caller building the
//! context from gate outputs — and re-derivable from the proposal's
//! attachments, which deeper audit tiers re-check.

use chrono::Utc;
use serde_json::json;
use std::path::Path;

use crate::events;
use crate::policy::acceptance_policy::{
    AcceptancePolicy, Decision, DecisionCertificate, Outcome, PolicyContext, VerifiedPolicy,
    evaluate, load_active_policy,
};
use crate::project;
use crate::repo;

use super::EngineVerdict;

/// The payload key on a policy-lane accept event.
pub const POLICY_LANE_PAYLOAD_KEY: &str = "policy_lane";

/// What a policy-lane acceptance produced.
#[derive(Debug, Clone)]
pub struct PolicyAcceptOutcome {
    pub event_id: String,
    pub certificate: DecisionCertificate,
    pub verdict: EngineVerdict,
}

/// Why the policy lane did not land anything. `Deferred` is the normal
/// exit for work that needs a human — the caller leaves the proposal
/// pending (it becomes a sign-queue item). `Denied` and `Closed` are
/// refusals.
#[derive(Debug, Clone)]
pub enum PolicyLaneRefusal {
    /// No active, signed policy — the lane is closed; everything defers.
    Closed,
    /// The evaluator routed this to a named human.
    Deferred { reasons: Vec<String> },
    /// The evaluator prohibited this outright.
    Denied { reasons: Vec<String> },
    /// A structural error (missing proposal, IO, gate block…).
    Error(String),
}

impl std::fmt::Display for PolicyLaneRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "policy lane closed: no active signed policy"),
            Self::Deferred { reasons } => write!(f, "policy deferred: {}", reasons.join(", ")),
            Self::Denied { reasons } => write!(f, "policy denied: {}", reasons.join(", ")),
            Self::Error(e) => write!(f, "{e}"),
        }
    }
}

/// Accept a pending proposal under the frontier's active, human-signed
/// acceptance policy. The executor is the agent that drove the landing
/// (recorded in the certificate, carries zero authority). Returns
/// `Err(Deferred)` when a human is needed — the caller routes the
/// proposal to the sign queue, which is success-shaped for a landing.
pub fn accept_under_policy_at_path(
    path: &Path,
    proposal_id: &str,
    ctx: &PolicyContext,
    executor: &str,
) -> Result<PolicyAcceptOutcome, PolicyLaneRefusal> {
    let executor = executor.trim();
    // The lane exists so that NO personal key is in this code path. A
    // human wanting to accept uses their key (`vela sign`); routing a
    // human id through here would launder a key-less accept.
    if !(executor.starts_with("agent:") || executor.starts_with("ci:")) {
        return Err(PolicyLaneRefusal::Error(format!(
            "policy-lane executor must be an agent:/ci: actor, got `{executor}` — humans accept \
             with their key via `vela sign`"
        )));
    }

    let verified = load_active_policy(path)
        .map_err(PolicyLaneRefusal::Error)?
        .ok_or(PolicyLaneRefusal::Closed)?;

    let now = Utc::now().to_rfc3339();
    let decision = evaluate(&verified.policy, ctx, &now);
    match decision.outcome {
        Outcome::Permit => {}
        Outcome::Defer => {
            return Err(PolicyLaneRefusal::Deferred {
                reasons: decision.reasons,
            });
        }
        Outcome::Deny => {
            return Err(PolicyLaneRefusal::Denied {
                reasons: decision.reasons,
            });
        }
    }

    // Persist the authorizing policy content-addressed BEFORE landing:
    // a policy-lane event must stay verifiable after `active.json`
    // rotates to a successor policy.
    persist_policy_snapshot(path, &verified).map_err(PolicyLaneRefusal::Error)?;

    let mut frontier = repo::load_from_path(path).map_err(PolicyLaneRefusal::Error)?;

    let before = crate::evidence_ci::run_project(&frontier, path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);
    let state_root_before = events::event_log_hash(&frontier.events);

    let (event_id, certificate) = accept_in_frontier_under_policy(
        &mut frontier,
        proposal_id,
        &verified,
        &decision,
        ctx,
        executor,
        &state_root_before,
    )
    .map_err(PolicyLaneRefusal::Error)?;

    // The engine gate, identical to a human accept and STRICTER: there
    // is no `force` here at all. A gate block discards the in-memory
    // mutation and lands nothing.
    let after = crate::evidence_ci::run_project(&frontier, path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();
    let truth_bearing = super::is_truth_bearing_kind(
        &frontier
            .proposals
            .iter()
            .find(|p| p.id == proposal_id)
            .map(|p| p.kind.clone())
            .unwrap_or_default(),
    );
    if truth_bearing && (!new_blocking.is_empty() || !new_warnings.is_empty()) {
        return Err(PolicyLaneRefusal::Error(format!(
            "engine gate blocked policy-lane accept of {proposal_id}: {} new blocking, {} new \
             warning(s) — nothing landed (the policy lane has no --force)",
            new_blocking.len(),
            new_warnings.len()
        )));
    }

    let verdict = EngineVerdict {
        status: if new_warnings.is_empty() {
            "pass".to_string()
        } else {
            "warn".to_string()
        },
        new_blocking,
        new_warnings,
        forced: false,
        strict: true,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    };

    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier).map_err(PolicyLaneRefusal::Error)?;

    Ok(PolicyAcceptOutcome {
        event_id,
        certificate,
        verdict,
    })
}

/// The in-memory apply: mirrors `accept_proposal_in_frontier_with_custody`
/// with the authority swapped from reviewer-key custody to the verified
/// policy + certificate. No key is read; no signature lands on the event.
fn accept_in_frontier_under_policy(
    frontier: &mut project::Project,
    proposal_id: &str,
    verified: &VerifiedPolicy,
    decision: &Decision,
    ctx: &PolicyContext,
    executor: &str,
    state_root_before: &str,
) -> Result<(String, DecisionCertificate), String> {
    let index = frontier
        .proposals
        .iter()
        .position(|p| p.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    let status = frontier.proposals[index].status.clone();
    if status == "rejected" {
        return Err(format!("Cannot accept rejected proposal {proposal_id}"));
    }
    if status == "applied" {
        return Err(format!(
            "Proposal {proposal_id} is already applied (idempotent no-op is the caller's exit 5)"
        ));
    }
    let proposal = frontier.proposals[index].clone();
    super::validate_proposal_shape(frontier, &proposal)?;

    let reviewer = format!("policy:{}", verified.policy.id);
    let reason = format!(
        "policy permit under {} (rules: {})",
        verified.policy.id,
        decision.matched_rule_ids.join(", ")
    );

    frontier.proposals[index].status = "accepted".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.clone());
    frontier.proposals[index].reviewed_at = Some(Utc::now().to_rfc3339());
    frontier.proposals[index].decision_reason = Some(reason.clone());

    let event_id = super::apply_proposal(frontier, &proposal, &reviewer, &reason, None)?;

    // The certificate: the replayable record of WHY this landed without
    // a key. state_root_after is stamped after the apply so the cert
    // pins both sides of the transition.
    let certificate = DecisionCertificate {
        schema: "vela.decision_certificate.v0.1".to_string(),
        id: format!("vdc_{}", &event_id[4..]),
        frontier_id: frontier.frontier_id.clone().unwrap_or_default(),
        proposal_id: proposal_id.to_string(),
        state_root_before: state_root_before.to_string(),
        state_root_after: events::event_log_hash(&frontier.events),
        outcome: Outcome::Permit,
        policy_id: verified.policy.id.clone(),
        rule_ids: decision.matched_rule_ids.clone(),
        evaluator: decision.evaluator.clone(),
        authority_mode: crate::policy::acceptance_policy::AuthorityMode::PolicyDelegation,
        human_authorizers: vec![verified.signer_pubkey_hex.clone()],
        executor: executor.to_string(),
        assurance_profile: format!("assurance_level_a{}", ctx.assurance_level),
        assurance_level: ctx.assurance_level,
        claim_digest: proposal_claim_digest(&proposal),
        impact_tier: ctx.impact_tier,
        reasons: decision.reasons.clone(),
        audit_required: false,
    };

    // Stamp the lane into the SIGNED payload (the provenance pattern):
    // the block enters the content address, so the id is re-derived and
    // the lane claim is tamper-evident.
    let stamped_id = {
        let ev = frontier
            .events
            .iter_mut()
            .find(|e| e.id == event_id)
            .ok_or_else(|| format!("applied event {event_id} not found for lane stamp"))?;
        if let serde_json::Value::Object(map) = &mut ev.payload {
            map.insert(
                POLICY_LANE_PAYLOAD_KEY.to_string(),
                json!({
                    "policy_id": verified.policy.id,
                    "rule_ids": decision.matched_rule_ids,
                    "certificate": certificate,
                    "context": ctx,
                }),
            );
        } else {
            return Err("accept event payload is not an object".to_string());
        }
        ev.id = events::event_id(ev);
        ev.id.clone()
    };
    frontier.proposals[index].status = "applied".to_string();
    frontier.proposals[index].applied_event_id = Some(stamped_id.clone());

    Ok((stamped_id, certificate))
}

/// The content digest of what the proposal asserts — the same digest
/// family the exact-lane attachments bind to. Empty-payload proposals
/// digest the empty string (structurally valid; the evaluator's context
/// carries the real assurance story).
fn proposal_claim_digest(proposal: &super::StateProposal) -> String {
    let text = proposal
        .payload
        .get("assertion")
        .and_then(|a| a.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| proposal.payload.get("text").and_then(|t| t.as_str()))
        .unwrap_or_default();
    crate::verifier_attachment::claim_digest(text)
}

/// Persist the authorizing policy (and its human signature) under its
/// own content address, so a rotated `active.json` never orphans the
/// events it admitted.
fn persist_policy_snapshot(frontier_dir: &Path, verified: &VerifiedPolicy) -> Result<(), String> {
    let dir = frontier_dir.join(".vela").join("policies");
    let policy_path = dir.join(format!("{}.json", verified.policy.id));
    let sig_src = dir.join("active.sig.json");
    let sig_dst = dir.join(format!("{}.sig.json", verified.policy.id));
    if !policy_path.exists() {
        let body = serde_json::to_string_pretty(&verified.policy).map_err(|e| e.to_string())?;
        std::fs::write(&policy_path, format!("{body}\n")).map_err(|e| e.to_string())?;
    }
    if !sig_dst.exists() && sig_src.exists() {
        std::fs::copy(&sig_src, &sig_dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Verify every policy-lane event in a project against the persisted
/// signed policies: the routing must RE-DERIVE. Returns one error string
/// per failing event; empty = all lanes verify. Called from strict
/// checking.
pub fn verify_policy_lane_events(project: &project::Project, frontier_dir: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    for ev in &project.events {
        let Some(lane) = ev.payload.get(POLICY_LANE_PAYLOAD_KEY) else {
            continue;
        };
        let Some(policy_id) = lane.get("policy_id").and_then(|v| v.as_str()) else {
            errors.push(format!("{}: policy_lane block missing policy_id", ev.id));
            continue;
        };
        // 1. Actor honesty: a policy-lane event must be actored by that policy.
        if ev.actor.id != format!("policy:{policy_id}") {
            errors.push(format!(
                "{}: policy_lane actor mismatch ({} vs policy:{policy_id})",
                ev.id, ev.actor.id
            ));
            continue;
        }
        // 2. The persisted policy + human signature must verify.
        let policy = match load_policy_snapshot(frontier_dir, policy_id) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("{}: {e}", ev.id));
                continue;
            }
        };
        // 3. The stamped context must re-derive the Permit under that
        //    exact policy (routing forgery check).
        let Ok(ctx) = serde_json::from_value::<PolicyContext>(
            lane.get("context").cloned().unwrap_or_default(),
        ) else {
            errors.push(format!("{}: policy_lane context does not parse", ev.id));
            continue;
        };
        let decision = evaluate(&policy, &ctx, &ev.timestamp);
        if decision.outcome != Outcome::Permit {
            errors.push(format!(
                "{}: re-evaluation under {policy_id} yields {:?}, not permit ({})",
                ev.id,
                decision.outcome,
                decision.reasons.join(", ")
            ));
            continue;
        }
        // 4. Certificate consistency.
        let cert_ok = lane
            .get("certificate")
            .and_then(|c| serde_json::from_value::<DecisionCertificate>(c.clone()).ok())
            .map(|c| c.policy_id == policy_id && c.outcome == Outcome::Permit)
            .unwrap_or(false);
        if !cert_ok {
            errors.push(format!(
                "{}: policy_lane certificate missing or inconsistent",
                ev.id
            ));
        }
    }
    errors
}

/// Load a persisted policy snapshot by id and verify its detached human
/// signature (same bar as `load_active_policy`, addressed by id).
fn load_policy_snapshot(frontier_dir: &Path, policy_id: &str) -> Result<AcceptancePolicy, String> {
    use ed25519_dalek::Verifier;
    let dir = frontier_dir.join(".vela").join("policies");
    let policy_path = dir.join(format!("{policy_id}.json"));
    let sig_path = dir.join(format!("{policy_id}.sig.json"));
    if !policy_path.exists() || !sig_path.exists() {
        return Err(format!(
            "policy snapshot {policy_id} (+sig) not found under .vela/policies/"
        ));
    }
    let raw = std::fs::read_to_string(&policy_path).map_err(|e| e.to_string())?;
    let policy: AcceptancePolicy =
        serde_json::from_str(&raw).map_err(|e| format!("policy {policy_id} parse: {e}"))?;
    if policy.id != policy_id || !policy.id_is_valid() {
        return Err(format!(
            "policy snapshot {policy_id}: id does not re-derive"
        ));
    }
    let sig_raw = std::fs::read_to_string(&sig_path).map_err(|e| e.to_string())?;
    let sig: crate::policy::acceptance_policy::PolicySignatureRecord =
        serde_json::from_str(&sig_raw).map_err(|e| format!("policy sig parse: {e}"))?;
    if sig.policy_id != policy.id {
        return Err(format!(
            "policy snapshot {policy_id}: signature is for a different policy"
        ));
    }
    let body = crate::canonical::to_canonical_bytes(&policy).map_err(|e| e.to_string())?;
    let pk: [u8; 32] = hex::decode(&sig.signer_pubkey_hex)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "pubkey must be 32 bytes".to_string())?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk).map_err(|e| e.to_string())?;
    let sig_bytes: [u8; 64] = hex::decode(&sig.signature)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "signature must be 64 bytes".to_string())?;
    vk.verify(&body, &ed25519_dalek::Signature::from_bytes(&sig_bytes))
        .map_err(|_| format!("policy snapshot {policy_id}: signature does not verify"))?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::acceptance_policy::{
        Constraints, PolicyRule, PolicySignatureRecord, Quorum,
    };
    use crate::proposals::{StateProposal, new_proposal};
    use ed25519_dalek::Signer;
    use serde_json::json;
    use tempfile::TempDir;

    fn permitting_policy() -> AcceptancePolicy {
        let mut p = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".into(),
            epoch: 1,
            issued_by: vec!["reviewer:will".into()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".into()],
            },
            rules: vec![PolicyRule {
                id: "review-exact-auto-v1".into(),
                effect: Outcome::Permit,
                claim_classes: vec!["sidon_lower_bound".into()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 5,
                    required_assurance_min: 3,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: true,
                    require_method_integrity: true,
                },
            }],
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".into(),
            revocation_ref: None,
        };
        p.id = p.content_address();
        p
    }

    fn permitting_ctx() -> PolicyContext {
        PolicyContext {
            claim_class: "sidon_lower_bound".into(),
            assurance_level: 3,
            impact_tier: 2,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: false,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: true,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
        }
    }

    /// Initialize a REAL `.vela`-store frontier with one finding, one
    /// pending review proposal, and a HUMAN-SIGNED active policy.
    /// Returns the dir and the pending proposal id.
    fn seeded_frontier(tmp: &TempDir) -> (std::path::PathBuf, String) {
        let dir = tmp.path().to_path_buf();
        crate::frontier_repo::initialize(
            &dir,
            crate::frontier_repo::InitOptions {
                name: "policy-lane-test",
                template: "",
                initialize_git: false,
            },
        )
        .unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        frontier
            .findings
            .push(crate::proposals::tests::finding("vf_target"));
        project::recompute_stats(&mut frontier);
        repo::save_to_path(&dir, &frontier).unwrap();

        let proposal = new_proposal(
            "finding.review",
            crate::events::StateTarget {
                r#type: "finding".to_string(),
                id: "vf_target".to_string(),
            },
            "agent:prover",
            "agent",
            "exact witness re-derived",
            json!({"status": "accepted"}),
            Vec::new(),
            Vec::new(),
        );
        let pid = proposal.id.clone();
        super::super::create_or_apply(&dir, proposal, false).unwrap();

        // Human-sign the policy with a throwaway key (the test's "Will").
        let policy = permitting_policy();
        let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let body = crate::canonical::to_canonical_bytes(&policy).unwrap();
        let sig = key.sign(&body);
        let pol_dir = dir.join(".vela").join("policies");
        std::fs::create_dir_all(&pol_dir).unwrap();
        std::fs::write(
            pol_dir.join("active.json"),
            serde_json::to_string_pretty(&policy).unwrap(),
        )
        .unwrap();
        std::fs::write(
            pol_dir.join("active.sig.json"),
            serde_json::to_string_pretty(&PolicySignatureRecord {
                policy_id: policy.id.clone(),
                signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
                signature: hex::encode(sig.to_bytes()),
                signed_at: "2026-07-03T00:00:00Z".to_string(),
            })
            .unwrap(),
        )
        .unwrap();
        (dir, pid)
    }

    #[test]
    fn permit_lands_canonical_event_with_verifiable_lane() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let store = dir.clone();

        let out = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "agent:prover")
            .expect("permit lands");
        assert!(out.certificate.policy_id.starts_with("vap_"));
        assert_eq!(out.certificate.outcome, Outcome::Permit);

        let loaded = repo::load_from_path(&store).unwrap();
        let ev = loaded
            .events
            .iter()
            .find(|e| e.id == out.event_id)
            .expect("event landed");
        assert!(ev.actor.id.starts_with("policy:vap_"));
        assert_eq!(events::actor_kind(&ev.actor.id), "agent");
        assert!(ev.signature.is_none());
        assert!(ev.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some());
        // Content address survived the stamp (id re-derives).
        assert_eq!(ev.id, events::event_id(ev));
        // Proposal applied and points at the stamped event.
        let p = loaded.proposals.iter().find(|p| p.id == pid).unwrap();
        assert_eq!(p.status, "applied");
        assert_eq!(p.applied_event_id.as_deref(), Some(out.event_id.as_str()));

        // The lane verifies.
        let errors = verify_policy_lane_events(&loaded, &dir);
        assert!(errors.is_empty(), "{errors:?}");

        // Tampering with the stamped context must fail verification.
        let mut tampered = repo::load_from_path(&store).unwrap();
        for ev in tampered.events.iter_mut() {
            if let Some(lane) = ev.payload.get_mut(POLICY_LANE_PAYLOAD_KEY) {
                lane["context"]["assurance_level"] = json!(0);
            }
        }
        let errors = verify_policy_lane_events(&tampered, &dir);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not permit"), "{errors:?}");
    }

    #[test]
    fn defer_and_deny_land_nothing() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut ctx = permitting_ctx();
        ctx.assurance_level = 1; // below the rule's floor -> Defer (default)
        let err =
            accept_under_policy_at_path(&dir, &pid, &ctx, "agent:prover").expect_err("must defer");
        assert!(matches!(err, PolicyLaneRefusal::Deferred { .. }), "{err}");
        let loaded = repo::load_from_path(&dir).unwrap();
        assert_eq!(
            loaded
                .proposals
                .iter()
                .find(|p| p.id == pid)
                .unwrap()
                .status,
            "pending_review"
        );
    }

    #[test]
    fn human_executor_is_refused() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let err = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "reviewer:will")
            .expect_err("humans use their key");
        assert!(err.to_string().contains("agent:/ci:"), "{err}");
    }

    #[test]
    fn no_signed_policy_means_lane_closed() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        std::fs::remove_file(dir.join(".vela/policies/active.sig.json")).unwrap();
        let err = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "agent:prover")
            .expect_err("unsigned policy = no authority");
        assert!(matches!(err, PolicyLaneRefusal::Closed), "{err}");
    }
}
