//! The sign queue: everything in a frontier that awaits a HUMAN key,
//! as one derived projection (never a store). This is the data model
//! behind `vela sign` — the one ceremony verb. Four lanes, fixed order:
//!
//! 1. `judgment` — products only a human can produce (statement-
//!    fidelity verdicts owed, role attestations). These can never be
//!    policied away: they are the product. Domain surfaces (the CLI's
//!    sign session) inject these via [`SignQueue::push_judgment`] until
//!    the generic verdict-owed detection lands with the draft-class
//!    policy work.
//! 2. `decision` — pending proposals/packs the active policy DEFERRED
//!    (or everything pending when no policy is signed: a closed lane
//!    defers all). Items the policy would Permit never appear here —
//!    they auto-land at landing time. Deny items appear dimmed
//!    (`signable: false`): a human should SEE prohibitions, not sign
//!    around them.
//! 3. `hygiene` — the operator's own unsigned events under a
//!    now-registered key (the old resign-frontier.sh ceremony), with
//!    the before-count the script used to compute by hand.
//! 4. `detached` — sealed-but-unsigned governance artifacts (policies
//!    awaiting their signature).

#![allow(dead_code)] // the projection behind `vela sign` (W3); built and tested ahead of its consumer

use std::path::Path;

use serde::Serialize;
use vela_protocol::acceptance_policy::{Outcome, PolicyContext, evaluate, load_active_policy};
use vela_protocol::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignLane {
    Judgment,
    Decision,
    Hygiene,
    Detached,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignItem {
    pub lane: SignLane,
    /// Object id (vpr_/vsd_/vsa-request/actor id/file path).
    pub id: String,
    pub title: String,
    /// Why this needs a human — the deferring rule's reasons, the
    /// signal counts, or "judgment product".
    pub why_here: String,
    /// False for policy-Denied items: shown, never signable.
    pub signable: bool,
    /// Pack id when the item decides a whole changeset.
    pub pack: Option<String>,
    /// The content being decided, as renderable lines: the claim, its
    /// artifacts with hashes, its caveats. A judgment surface that
    /// hides the artifact is a rubber stamp with extra steps.
    #[serde(default)]
    pub preview: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SignQueue {
    pub items: Vec<SignItem>,
    /// True when a signed policy filtered this queue (autonomy is ON);
    /// false = lane closed, everything pending defers to the human.
    pub policy_active: bool,
    pub policy_id: Option<String>,
}

impl SignQueue {
    pub fn push_judgment(&mut self, id: &str, title: &str, why: &str, preview: Vec<String>) {
        self.items.insert(
            0,
            SignItem {
                lane: SignLane::Judgment,
                id: id.to_string(),
                title: title.to_string(),
                why_here: why.to_string(),
                signable: true,
                pack: None,
                preview,
            },
        );
    }
}

/// The headline of a proposal is its CLAIM, never its kind string.
fn proposal_headline(p: &vela_protocol::proposals::StateProposal) -> String {
    // finding.add nests the claim at payload.finding.assertion.text;
    // review/note kinds carry it flatter. Walk the known shapes.
    let flat = &p.payload;
    let nested = flat.get("finding").unwrap_or(flat);
    nested
        .get("assertion")
        .and_then(|a| a.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| flat.get("text").and_then(|t| t.as_str()))
        .or_else(|| flat.get("status").and_then(|s| s.as_str()))
        .unwrap_or(&p.reason)
        .to_string()
}

/// The content a human must SEE before deciding: type, artifacts with
/// hashes, verifier runs, caveats, author.
fn proposal_preview(p: &vela_protocol::proposals::StateProposal) -> Vec<String> {
    let mut out = Vec::new();
    let nested = p.payload.get("finding").unwrap_or(&p.payload);
    let atype = nested
        .get("assertion")
        .and_then(|a| a.get("type"))
        .and_then(|v| v.as_str())
        .or_else(|| p.payload.get("assertion_type").and_then(|v| v.as_str()));
    match atype {
        Some(t) => out.push(format!("type      {t} · {}", p.kind)),
        None => out.push(format!("kind      {}", p.kind)),
    }
    out.push(format!("author    {}", p.actor.id));
    if let Some(cond) = nested
        .get("conditions")
        .and_then(|c| c.get("text"))
        .and_then(|v| v.as_str())
    {
        out.push(format!("evidence  {}", &cond[..cond.len().min(90)]));
    }
    if let Some(score) = nested
        .get("confidence")
        .and_then(|c| c.get("score"))
        .and_then(|v| v.as_f64())
    {
        out.push(format!("prior     {score:.2} (review required)"));
    }
    if let Some(arts) = p
        .payload
        .get("evidence_atoms")
        .or_else(|| p.payload.get("artifacts"))
        .and_then(|a| a.as_array())
    {
        for a in arts.iter().take(4) {
            let loc = a
                .get("locator")
                .or_else(|| a.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let sha = a.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
            out.push(format!("artifact  {loc}  {}", &sha[..sha.len().min(12)]));
        }
    }
    if let Some(runs) = p.payload.get("verifier_runs").and_then(|a| a.as_array()) {
        for r in runs.iter().take(3) {
            out.push(format!(
                "verified  {} -> {}",
                r.get("method").and_then(|v| v.as_str()).unwrap_or("?"),
                r.get("outcome").and_then(|v| v.as_str()).unwrap_or("?"),
            ));
        }
    }
    if let Some(caveats) = p.payload.get("caveats").and_then(|c| c.as_array()) {
        for c in caveats.iter().filter_map(|v| v.as_str()).take(3) {
            out.push(format!("caveat    {c}"));
        }
    }
    for c in p.caveats.iter().take(2) {
        out.push(format!("caveat    {c}"));
    }
    out
}

/// Reason codes become sentences. A bare rule id at a judgment prompt
/// is developer debris.
fn humanize_reasons(reasons: &[String]) -> String {
    let mapped: Vec<String> = reasons
        .iter()
        .map(|r| match r.as_str() {
            "default_defer" => "no policy rule covers this — the default is to ask you".to_string(),
            "assurance_below_minimum" => {
                "the evidence hasn't cleared the rule's assurance bar".to_string()
            }
            "no_matching_rule" => "no policy rule matches this claim class".to_string(),
            other => other.replace('_', " "),
        })
        .collect();
    mapped.join("; ")
}

/// Build the queue for one frontier. `ctx_for` derives the policy
/// context for a pending proposal — the SAME derivation the landing
/// path uses (assurance from the gate, never self-asserted); pass the
/// conservative default when no richer derivation exists yet.
pub fn sign_queue(
    project: &Project,
    frontier_dir: &Path,
    ctx_for: impl Fn(&Project, &str) -> PolicyContext,
) -> Result<SignQueue, String> {
    let mut queue = SignQueue::default();

    let policy = load_active_policy(frontier_dir)?;
    queue.policy_active = policy.is_some();
    queue.policy_id = policy.as_ref().map(|p| p.policy.id.clone());
    let now = chrono::Utc::now().to_rfc3339();

    // Lane 2 — decisions: pending proposals, policy-filtered, packs first.
    let pack_of = |proposal_id: &str| -> Option<String> {
        project
            .released_diff_packs
            .iter()
            .find(|p| p.verdict.is_none() && p.member_proposals.iter().any(|m| m == proposal_id))
            .map(|p| p.pack_id.clone())
    };
    for proposal in project
        .proposals
        .iter()
        .filter(|p| p.status == "pending_review")
    {
        let (why, signable) = match &policy {
            None => (
                "no standing policy covers this frontier — every decision is yours".to_string(),
                true,
            ),
            Some(vp) => {
                let ctx = ctx_for(project, &proposal.id);
                let decision = evaluate(&vp.policy, &ctx, &now);
                match decision.outcome {
                    // Permit-able items are NOT the human's job: the
                    // landing path admits them. Showing them here would
                    // re-create the per-item ceremony the lane removed.
                    Outcome::Permit => continue,
                    Outcome::Defer => (humanize_reasons(&decision.reasons), true),
                    Outcome::Deny => (
                        format!(
                            "policy prohibits this: {}",
                            humanize_reasons(&decision.reasons)
                        ),
                        false,
                    ),
                }
            }
        };
        queue.items.push(SignItem {
            lane: SignLane::Decision,
            id: proposal.id.clone(),
            title: proposal_headline(proposal),
            why_here: why,
            signable,
            pack: pack_of(&proposal.id),
            preview: proposal_preview(proposal),
        });
    }

    // Lane 3 — hygiene: unsigned events by registered actors (the
    // re-sign ceremony), grouped per actor with the signal count.
    let registered: std::collections::BTreeSet<&str> =
        project.actors.iter().map(|a| a.id.as_str()).collect();
    let mut unsigned_by_actor: std::collections::BTreeMap<&str, usize> = Default::default();
    for ev in &project.events {
        if ev.signature.is_none() && registered.contains(ev.actor.id.as_str()) {
            *unsigned_by_actor.entry(ev.actor.id.as_str()).or_default() += 1;
        }
    }
    for (actor, count) in unsigned_by_actor {
        queue.items.push(SignItem {
            lane: SignLane::Hygiene,
            id: format!("resign:{actor}"),
            title: format!("re-sign {count} unsigned event(s) by {actor}"),
            why_here: "events predate signing; strict flags unsigned_registered_actor".to_string(),
            signable: true,
            pack: None,
            preview: Vec::new(),
        });
    }

    // Lane 4 — detached: sealed-but-unsigned policies.
    let pol_dir = frontier_dir.join(".vela").join("policies");
    if pol_dir.join("active.json").exists() && !pol_dir.join("active.sig.json").exists() {
        queue.items.push(SignItem {
            lane: SignLane::Detached,
            id: pol_dir.join("active.json").display().to_string(),
            title: "sealed policy awaits your signature (the lane is closed until then)".into(),
            why_here: "a policy without a human signature carries no authority".into(),
            signable: true,
            pack: None,
            preview: Vec::new(),
        });
    }

    Ok(queue)
}
