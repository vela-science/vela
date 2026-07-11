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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vela_protocol::acceptance_policy::PolicyContext;
use vela_protocol::proposals::policy_accept::{self, PolicyAcceptOutcome, PolicyLaneRefusal};
use vela_protocol::repo;

/// The portable receipt (see module doc). Field names deliberately
/// match the vrc_ draft body (and `vela land`'s flag spellings).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct Receipt {
    #[serde(default)]
    pub schema: String,
    pub claim: String,
    #[serde(default = "default_type")]
    pub r#type: String,
    /// How faithfully this run can be re-executed (docs/RECEIPTS.md): `exact`
    /// (same bytes, same frozen verifier) | `bounded` (deterministic code, only
    /// partly-pinned external service) | `approximate` (same prompt/model label,
    /// provider may vary) | `unavailable` (cannot rerun, but payloads are
    /// hash-bound) | `unknown`. Optional; pre-v0.748 receipts default to
    /// `unknown`. The signed policy reads it via `PolicyContext.replayability`,
    /// so a non-`exact` receipt need not auto-admit a serious claim on its own.
    #[serde(default = "default_replayability")]
    pub replayability: String,
    #[serde(default)]
    pub artifacts: Vec<ReceiptArtifact>,
    #[serde(default)]
    pub caveats: Vec<String>,
    #[serde(default)]
    pub verifier_runs: Vec<ReceiptVerifierRun>,
    /// Part of the published schema; the stable extension points for external-run
    /// provenance (docs/RECEIPTS.md §"extension points"): `source`
    /// {system,run_id,source_uri}, `trace_refs` {otel/shepherd/…}, `lineage_refs`
    /// {swhid/dvc/datalad/ro_crate}, `independence_basis`. Carried for provenance
    /// consumers (hub, export, adapters); the landing path itself does not branch
    /// on them.
    #[serde(default)]
    pub environment: Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub provenance: Value,
    /// The Receipt-v1 `lineage` layer ({parents, derived_from, source_refs,
    /// …}). Read-only input to the derived independence predicate; absent on
    /// minimal receipts, and absence never counts as clean lineage.
    #[serde(default)]
    pub lineage: Value,
    /// Preserve extension fields so the review digest binds the complete
    /// logical receipt rather than only the fields this CLI currently reads.
    #[serde(default, flatten)]
    pub extensions: BTreeMap<String, Value>,
}

fn default_type() -> String {
    "computational".to_string()
}

fn default_replayability() -> String {
    "unknown".to_string()
}

/// The receipt-level replay classes (docs/RECEIPTS.md). A landing with a value
/// outside this set is rejected — an honest classification, or none.
pub(crate) const REPLAYABILITY_CLASSES: &[&str] =
    &["exact", "bounded", "approximate", "unavailable", "unknown"];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ReceiptArtifact {
    pub path: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default, flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ReceiptVerifierRun {
    pub method: String,
    pub outcome: String,
    #[serde(default)]
    pub log: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub solver: String,
    /// The axioms a Lean kernel re-derivation observed (e.g. `propext`,
    /// `Classical.choice`, `Quot.sound`). Present on `verifier.lean_*` runs;
    /// absent (empty) on non-Lean runs and pre-axiom-audit receipts. Read to
    /// decide kernel-cleanliness for the Lean delegation lane.
    #[serde(default)]
    pub axioms: Vec<String>,
    #[serde(default, flatten)]
    pub extensions: BTreeMap<String, Value>,
}

/// True when this run is a Lean kernel re-derivation (its method names the
/// Lean external-declaration / kernel verifier). The axiom audit only
/// applies to these; a non-Lean run is judged by its own lane.
pub(crate) fn is_lean_run(method: &str) -> bool {
    let m = method.to_ascii_lowercase();
    m.contains("lean_external") || m.contains("lean_kernel") || m.starts_with("verifier.lean")
}

/// Whether a receipt carries a passing, kernel-clean Lean re-derivation:
/// at least one Lean run passed and every passing Lean run's axioms are
/// `KernelClean` under the frozen TCB policy (no `sorryAx`, no
/// compiler-trust axiom, nothing outside the allowlist). A Lean run that
/// passed with a forbidden or unlisted axiom makes this false — the
/// overclaim the gate exists to catch.
pub(crate) fn receipt_lean_kernel_clean(receipt: &Receipt) -> bool {
    use vela_protocol::tcb_policy::{AxiomVerdict, DEFAULT_ALLOWED_AXIOMS, FORBIDDEN_AXIOMS};
    let classify = |axioms: &[String]| -> AxiomVerdict {
        if axioms
            .iter()
            .any(|a| FORBIDDEN_AXIOMS.contains(&a.as_str()))
        {
            return AxiomVerdict::ForbiddenAxiom;
        }
        if axioms
            .iter()
            .any(|a| !DEFAULT_ALLOWED_AXIOMS.contains(&a.as_str()))
        {
            return AxiomVerdict::UnlistedAxiom;
        }
        AxiomVerdict::KernelClean
    };
    let lean_runs: Vec<&ReceiptVerifierRun> = receipt
        .verifier_runs
        .iter()
        .filter(|r| is_lean_run(&r.method) && r.outcome.eq_ignore_ascii_case("pass"))
        .collect();
    !lean_runs.is_empty()
        && lean_runs
            .iter()
            .all(|r| classify(&r.axioms) == AxiomVerdict::KernelClean)
}

pub(crate) const RECEIPT_SCHEMA: &str = "vela.receipt.v1";

/// Where a landing ended up.
#[derive(Debug)]
pub(crate) enum LandRoute {
    /// Admitted canonically under the signed policy (the autonomy lane).
    PolicyAdmitted(Box<PolicyAcceptOutcome>),
    /// Pending — a human's `vela sign` queue holds it now. Success-shaped.
    Deferred { reasons: Vec<String> },
    /// This exact claim is already in the frontier — a retry, not a new
    /// finding. The activity is timestamped (each land is a distinct act),
    /// but the CLAIM is content: re-landing must not fork a duplicate for a
    /// human to sign twice. Idempotent; the caller's exit 5.
    AlreadyLanded { finding_id: String },
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
            LandRoute::AlreadyLanded { finding_id } => (
                "already_landed",
                format!("this claim is already {finding_id}"),
            ),
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
    if !REPLAYABILITY_CLASSES.contains(&receipt.replayability.as_str()) {
        return Err(format!(
            "unknown replayability `{}` (expected one of: {})",
            receipt.replayability,
            REPLAYABILITY_CLASSES.join(", ")
        ));
    }

    // 0. Idempotency: landing a byte-identical claim is a retry (a crashed
    //    session, a network blip), not a new finding. The activity record
    //    is timestamped so each land IS a distinct act, but the CLAIM is
    //    content — forking a duplicate finding for a human to sign twice
    //    (or a policy to auto-land twice) is the failure. If this exact
    //    live claim already exists, return it; the CLI maps this to exit 5.
    let project = repo::load_from_path(frontier)?;
    let claim = receipt.claim.trim();
    // Accepted findings carry the claim on `assertion.text`.
    if let Some(existing) = project.findings.iter().find(|f| {
        !f.flags.retracted
            && f.assertion.text.trim() == claim
            && f.assertion.assertion_type == receipt.r#type
    }) {
        return Ok(LandOutcome {
            proposal_id: existing.id.clone(),
            route: LandRoute::AlreadyLanded {
                finding_id: existing.id.clone(),
            },
        });
    }
    // A PENDING proposal from a prior land holds the claim nested in its
    // payload (finding.assertion.text) and hasn't become a finding yet —
    // a fast retry must dedup against it too, or the queue gets twins.
    if let Some(pending) = project.proposals.iter().find(|p| {
        p.status == "pending_review" && {
            let nested = p.payload.get("finding").unwrap_or(&p.payload);
            nested
                .get("assertion")
                .and_then(|a| a.get("text"))
                .and_then(|t| t.as_str())
                .map(|t| t.trim() == claim)
                .unwrap_or(false)
        }
    }) {
        return Ok(LandOutcome {
            proposal_id: pending.id.clone(),
            route: LandRoute::AlreadyLanded {
                finding_id: pending.id.clone(),
            },
        });
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
    // Independence is DERIVED from what the receipt carries (its verifier
    // runs, its declared independence_basis, its lineage layer), never
    // asserted. A minimal receipt derives false — the honest default this
    // context used to hard-code.
    let runs: Vec<vela_protocol::independence::ReceiptRunView> = receipt
        .verifier_runs
        .iter()
        .map(|r| vela_protocol::independence::ReceiptRunView {
            method: &r.method,
            solver: &r.solver,
            outcome: &r.outcome,
        })
        .collect();
    let basis =
        vela_protocol::receipt_v1::independence_basis_from_environment(&receipt.environment);
    let lineage = vela_protocol::receipt_v1::lineage_from_layer(&receipt.lineage);
    let independence = vela_protocol::independence::independence_from_receipt(
        &runs,
        basis.as_ref(),
        lineage.as_ref(),
    );
    // A Lean receipt is method-integrity-sound only when its Lean runs are
    // kernel-clean. A passing Lean run that used a forbidden axiom (sorryAx,
    // compiler trust) must NOT read as sound — tightening only, so no
    // non-Lean receipt changes. A distinct claim_class lets a signed policy
    // scope a delegation lane to kernel-clean Lean precisely.
    let has_lean_run = receipt.verifier_runs.iter().any(|r| is_lean_run(&r.method));
    let lean_kernel_clean = receipt_lean_kernel_clean(receipt);
    let method_integrity_sound = has_pass && (!has_lean_run || lean_kernel_clean);
    let claim_class = if lean_kernel_clean {
        "receipt_lean_kernel_clean".to_string()
    } else {
        format!("receipt_{}", receipt.r#type)
    };
    let ctx = PolicyContext {
        claim_class,
        assurance_level: if has_pass { 2 } else { 0 },
        impact_tier: 1,
        changed_findings: 1,
        downstream_dependents: 0,
        assertion_text_mutated: true, // a new claim IS new text
        target_contested: false,
        governance_mutation: false,
        independence_satisfied: independence.satisfied,
        method_integrity_sound,
        credential_valid: true,
        has_unknown_fields: false,
        replayability: receipt.replayability.clone(),
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

#[cfg(test)]
mod lean_lane_tests {
    use super::*;

    fn run(method: &str, outcome: &str, axioms: &[&str]) -> ReceiptVerifierRun {
        ReceiptVerifierRun {
            method: method.to_string(),
            outcome: outcome.to_string(),
            log: String::new(),
            solver: String::new(),
            axioms: axioms.iter().map(|s| s.to_string()).collect(),
            extensions: BTreeMap::new(),
        }
    }

    fn receipt_with(runs: Vec<ReceiptVerifierRun>) -> Receipt {
        Receipt {
            schema: RECEIPT_SCHEMA.to_string(),
            claim: "c".to_string(),
            r#type: "theoretical".to_string(),
            replayability: "exact".to_string(),
            artifacts: vec![],
            caveats: vec![],
            verifier_runs: runs,
            environment: serde_json::Value::Null,
            provenance: serde_json::Value::Null,
            lineage: serde_json::Value::Null,
            extensions: BTreeMap::new(),
        }
    }

    #[test]
    fn kernel_clean_lean_run_is_detected() {
        let r = receipt_with(vec![run(
            "verifier.lean_external_declaration.v1",
            "pass",
            &["propext", "Classical.choice", "Quot.sound"],
        )]);
        assert!(receipt_lean_kernel_clean(&r));
    }

    #[test]
    fn sorry_axiom_is_not_kernel_clean() {
        let r = receipt_with(vec![run(
            "verifier.lean_external_declaration.v1",
            "pass",
            &["propext", "sorryAx"],
        )]);
        assert!(
            !receipt_lean_kernel_clean(&r),
            "a sorryAx proof is not clean"
        );
    }

    #[test]
    fn native_decide_compiler_trust_is_not_kernel_clean() {
        let r = receipt_with(vec![run(
            "verifier.lean_external_declaration.v1",
            "pass",
            &["propext", "Lean.ofReduceBool"],
        )]);
        assert!(!receipt_lean_kernel_clean(&r));
    }

    #[test]
    fn unlisted_axiom_is_not_kernel_clean() {
        let r = receipt_with(vec![run(
            "verifier.lean_external_declaration.v1",
            "pass",
            &["propext", "MyCustomAxiom"],
        )]);
        assert!(!receipt_lean_kernel_clean(&r));
    }

    #[test]
    fn failing_lean_run_is_not_kernel_clean() {
        let r = receipt_with(vec![run(
            "verifier.lean_external_declaration.v1",
            "fail",
            &["propext"],
        )]);
        assert!(!receipt_lean_kernel_clean(&r));
    }

    #[test]
    fn non_lean_receipt_is_not_a_lean_lane() {
        let r = receipt_with(vec![run("sidon_binary_vector_exact", "pass", &[])]);
        assert!(!receipt_lean_kernel_clean(&r));
        assert!(!is_lean_run("sidon_binary_vector_exact"));
    }
}
