//! The compounding loop's engine: claim → briefing → land → drop.
//! One implementation behind the CLI verbs (`work`, `land`) and the
//! MCP `work` tool, so agents and humans drive the same machinery.
//!
//! `land` is the loop's write edge and the home of the **Vela Receipt**
//! (`vela.receipt.v1`) — the portable JSON any external tool (Claude
//! Science exports, notebooks, Codex runs, foundry searches) hands
//! over to cross from activity into state:
//!
//! ```json
//! {
//!   "schema": "vela.receipt.v1",
//!   "claim": "what is now known / bounded / refuted",
//!   "type": "computational | theoretical | empirical | negative",
//!   "artifacts": [{"path": "…", "kind": "witness"}],
//!   "caveats": ["what this does NOT establish"],
//!   "verifier_runs": [{"method": "…", "outcome": "pass", "log": "…"}],
//!   "environment": {"…": "optional, carried into provenance"},
//!   "provenance": {"generated_by": "…", "co_author": "agent:…"}
//! }
//! ```
//!
//! Landing routes by the frontier's signed policy: **Permit** admits
//! canonically through the policy lane (no key ceremony — the human's
//! authority arrived earlier, once, as the policy signature); **Defer**
//! leaves the proposal pending, where it becomes a `vela sign` item;
//! **Deny** or a gate block lands nothing. Landing is idempotent:
//! content addressing collapses byte-identical records, and an
//! already-applied proposal is the caller's exit 5.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};
use vela_protocol::acceptance_policy::PolicyContext;
use vela_protocol::proposals::policy_accept::{self, PolicyAcceptOutcome, PolicyLaneRefusal};
use vela_protocol::repo;

/// The portable receipt (see module doc). Field names deliberately
/// match the vrc_ draft body (and `vela land`'s flag spellings).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Receipt {
    #[serde(default)]
    pub schema: String,
    pub claim: String,
    #[serde(default = "default_type")]
    pub r#type: String,
    #[serde(default)]
    pub artifacts: Vec<ReceiptArtifact>,
    #[serde(default)]
    pub caveats: Vec<String>,
    #[serde(default)]
    pub verifier_runs: Vec<ReceiptVerifierRun>,
    /// Part of the published schema; carried for provenance consumers
    /// (not yet read by the landing path itself).
    #[serde(default)]
    #[allow(dead_code)]
    pub environment: Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub provenance: Value,
}

fn default_type() -> String {
    "computational".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReceiptArtifact {
    pub path: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReceiptVerifierRun {
    pub method: String,
    pub outcome: String,
    #[serde(default)]
    pub log: String,
    #[serde(default)]
    pub solver: String,
}

pub(crate) const RECEIPT_SCHEMA: &str = "vela.receipt.v1";

/// Where a landing ended up.
#[derive(Debug)]
pub(crate) enum LandRoute {
    /// Admitted canonically under the signed policy (the autonomy lane).
    PolicyAdmitted(Box<PolicyAcceptOutcome>),
    /// Pending — a human's `vela sign` queue holds it now. Success-shaped.
    Deferred { reasons: Vec<String> },
}

#[derive(Debug)]
pub(crate) struct LandOutcome {
    pub proposal_id: String,
    pub route: LandRoute,
}

impl LandRoute {
    /// The `(route, detail)` pair every landing surface reports — the CLI
    /// verb and the MCP `work` tool speak the same contract.
    pub(crate) fn summary(&self) -> (&'static str, String) {
        match self {
            LandRoute::PolicyAdmitted(o) => (
                "policy_admitted",
                format!("event {} under {}", o.event_id, o.certificate.policy_id),
            ),
            LandRoute::Deferred { reasons } => ("deferred", reasons.join(", ")),
        }
    }
}

/// Claim a lease on a target (the same engine the MCP work tool uses).
pub(crate) fn claim(
    frontier: &Path,
    target: &str,
    actor: &str,
    ttl_seconds: Option<u64>,
) -> Result<Value, String> {
    let args = json!({
        "frontier_path": frontier.display().to_string(),
        "obligation_id": target,
        "agent_actor": actor,
        "ttl_seconds": ttl_seconds,
    });
    let raw = vela_edge::vela_agent_mcp::claim_task(&args)?;
    serde_json::from_str(&raw).map_err(|e| format!("claim response: {e}"))
}

/// The pre-loaded briefing for a target — the compounding payload the
/// session starts from. Problem-shaped targets get the full task
/// packet; everything else gets the frontier-level slice.
pub(crate) fn briefing(frontier: &Path, target: &str) -> Result<Value, String> {
    let project = repo::load_from_path(frontier)?;
    let head = vela_protocol::events::event_log_hash(&project.events);
    let packet = crate::server::tools::briefing_for_target(&project, frontier, target);
    Ok(json!({
        "schema": "vela.next_offer.v0.1",
        "target": target,
        "pinned_state": {
            "frontier_id": project.frontier_id().to_string(),
            "event_log_hash": head,
        },
        "briefing": packet,
    }))
}

/// The session directory for a target within a frontier.
pub(crate) fn session_dir(frontier: &Path, target: &str) -> PathBuf {
    let safe: String = target
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    frontier.join(".vela").join("work").join(safe)
}

/// Land a receipt: record → propose (always pending first) → route by
/// the signed policy. Executor must be agent:/ci: for the policy lane;
/// a human landing simply defers to their own sign queue.
pub(crate) fn land(
    frontier: &Path,
    receipt: &Receipt,
    executor: &str,
) -> Result<LandOutcome, String> {
    if !receipt.schema.is_empty() && receipt.schema != RECEIPT_SCHEMA {
        return Err(format!(
            "unknown receipt schema `{}` (this build speaks {RECEIPT_SCHEMA})",
            receipt.schema
        ));
    }
    if receipt.claim.trim().is_empty() {
        return Err("a receipt needs a claim".to_string());
    }
    if receipt.caveats.is_empty() {
        return Err(
            "a receipt needs at least one caveat — what does this NOT establish?".to_string(),
        );
    }

    // 1. The activity record (vrc_) via the existing record engine:
    //    hashes artifacts at land time, pins the head.
    let record_json = crate::cli::records::mint_record_for_land(frontier, receipt, executor)?;

    // 2. Propose: lands PENDING, never applies (the record engine's rule).
    let proposal_id = crate::cli::records::propose_record_for_land(frontier, &record_json)?;

    // 3. Route by policy. Context derivation, stated honestly: the
    //    claim class comes from the receipt type; the assurance level
    //    is 2 only when the receipt carries at least one passing
    //    verifier run (the landing-time honesty duty policy_accept's
    //    doc describes — audit tiers re-derive from attachments).
    let has_pass = receipt
        .verifier_runs
        .iter()
        .any(|r| r.outcome.eq_ignore_ascii_case("pass"));
    let ctx = PolicyContext {
        claim_class: format!("receipt_{}", receipt.r#type),
        assurance_level: if has_pass { 2 } else { 0 },
        impact_tier: 1,
        changed_findings: 1,
        downstream_dependents: 0,
        assertion_text_mutated: true, // a new claim IS new text
        target_contested: false,
        governance_mutation: false,
        independence_satisfied: false,
        method_integrity_sound: has_pass,
        credential_valid: true,
        has_unknown_fields: false,
    };
    match policy_accept::accept_under_policy_at_path(frontier, &proposal_id, &ctx, executor) {
        Ok(outcome) => Ok(LandOutcome {
            proposal_id,
            route: LandRoute::PolicyAdmitted(Box::new(outcome)),
        }),
        Err(PolicyLaneRefusal::Closed) => Ok(LandOutcome {
            proposal_id,
            route: LandRoute::Deferred {
                reasons: vec!["no signed policy: every decision is the human's".to_string()],
            },
        }),
        Err(PolicyLaneRefusal::Deferred { reasons }) => Ok(LandOutcome {
            proposal_id,
            route: LandRoute::Deferred { reasons },
        }),
        Err(PolicyLaneRefusal::Denied { reasons }) => Err(format!(
            "policy denies this landing: {}",
            reasons.join(", ")
        )),
        Err(PolicyLaneRefusal::Error(e)) if e.contains("must be an agent:/ci:") => {
            // A human landed it: pending is exactly right — their own
            // sign queue picks it up.
            Ok(LandOutcome {
                proposal_id,
                route: LandRoute::Deferred {
                    reasons: vec!["human landing: decide it in `vela sign`".to_string()],
                },
            })
        }
        Err(PolicyLaneRefusal::Error(e)) if e.contains("engine gate blocked policy-lane") => {
            // The proposal is already PENDING on disk (propose ran before
            // the accept); the policy would admit but the engine gate found
            // new review warnings, so a human must glance. That is a
            // deferral to the sign queue, not a failed landing — the
            // fidelity discipline made automatic.
            Ok(LandOutcome {
                proposal_id,
                route: LandRoute::Deferred {
                    reasons: vec![
                        "the policy admits this, but the engine gate found review warnings — \
                         a human glances at it in `vela sign`"
                            .to_string(),
                    ],
                },
            })
        }
        Err(PolicyLaneRefusal::Error(e)) => Err(e),
    }
}
