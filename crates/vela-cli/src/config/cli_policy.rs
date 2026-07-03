//! `vela policy` — standing rules: the ceremony that pays compound interest.
//!
//! A key-holding human signs ONE scoped [`AcceptancePolicy`]; from then on the
//! deterministic evaluator routes every matching landing (`permit` lands the
//! canonical event, `defer` waits for `vela sign`, `deny` refuses). The porcelain
//! here is six verbs shaped as a ladder:
//!
//!   show    the active policy, its signature state, what it admitted lately
//!   draft   seal a policy from a template (no authority until signed)
//!   test    dry-run the policy over every pending proposal (never mutates)
//!   sign    THE ceremony: review, one confirm, one key read — the lane opens
//!   revoke  the pointer loses authority; snapshots keep past events verifiable
//!   log     every policy-lane admission across all policies
//!
//! Custody doctrine is unchanged: agents draft, the evaluator routes, only a
//! human key opens a lane (`resolve_decision_actor` refuses `agent:`/`ci:` with
//! exit 4), and revocation is a file deletion a human performs — never a model
//! output. The signature verified here is byte-for-byte the one
//! [`load_active_policy`] checks before any policy-lane accept.

use std::path::{Path, PathBuf};

use chrono::Utc;
use ed25519_dalek::Signer;
use serde_json::json;
use vela_protocol::acceptance_policy::{
    AcceptancePolicy, Constraints, EVALUATOR_VERSION, Outcome, PolicyContext, PolicyRule,
    PolicySignatureRecord, Quorum, evaluate, load_active_policy,
};
use vela_protocol::canonical;
use vela_protocol::cli_style as style;
use vela_protocol::project::Project;
use vela_protocol::proposals::StateProposal;
use vela_protocol::proposals::policy_accept::POLICY_LANE_PAYLOAD_KEY;
use vela_protocol::repo;

use crate::cli::print_json;
use crate::ui::{self, ErrorKind, fail_with};

// ── Store layout ───────────────────────────────────────────────────────

fn policies_dir(frontier: &Path) -> PathBuf {
    frontier.join(".vela").join("policies")
}

fn active_path(frontier: &Path) -> PathBuf {
    policies_dir(frontier).join("active.json")
}

fn active_sig_path(frontier: &Path) -> PathBuf {
    policies_dir(frontier).join("active.sig.json")
}

fn revoked_marker_path(frontier: &Path, policy_id: &str) -> PathBuf {
    policies_dir(frontier).join(format!("revoked-{policy_id}.json"))
}

// ── Errors: testable cores return these; cmd_ wrappers fail_with them ──

#[derive(Debug)]
struct CmdError {
    kind: ErrorKind,
    message: String,
    hint: Option<String>,
}

impl CmdError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        CmdError {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    fn hinted(kind: ErrorKind, message: impl Into<String>, hint: impl Into<String>) -> Self {
        CmdError {
            kind,
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    fn fail(self) -> ! {
        fail_with(self.kind, &self.message, self.hint.as_deref())
    }
}

// ── The template ladder ────────────────────────────────────────────────

const TEMPLATES: &str = "witness-rederivation, statement-drafts, notes-threshold";

/// The hardcoded template ladder, ordered by how much a signature delegates.
/// Every template defaults to `Defer` (never a permit default) and expires in
/// 90 days — the compound interest comes from re-signing a proven policy, not
/// from an eternal one.
///
///   witness-rederivation  exact witnesses the frozen gate re-derived (A3,
///                         independent, method-sound, no claim-text change)
///   statement-drafts      theoretical receipts from `vela land` (statement
///                         drafts land as receipt_theoretical) — drafts ARE
///                         text, so semantic text change is allowed, bounded
///                         to one finding at A2; independence is a
///                         verdict-time property, not a draft-time one
///   notes-threshold       notes attach without ceremony (A0) but the impact
///                         constraints stay tight: one finding, no dependents,
///                         no claim-language mutation
fn template_policy(name: &str) -> Option<(Vec<PolicyRule>, &'static str)> {
    match name {
        "witness-rederivation" => Some((
            vec![PolicyRule {
                id: "witness-rederivation-v1".to_string(),
                effect: Outcome::Permit,
                claim_classes: vec![
                    "sidon_lower_bound".to_string(),
                    "witness_construction".to_string(),
                ],
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
            "exact witnesses the frozen gate re-derived land themselves",
        )),
        "statement-drafts" => Some((
            vec![PolicyRule {
                id: "statement-drafts-v1".to_string(),
                effect: Outcome::Permit,
                // The class `vela land` actually stamps for statement
                // drafts (receipt_<type>); a class nothing produces would
                // make this a ceremony that delegates nothing.
                claim_classes: vec!["receipt_theoretical".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 2,
                    // Drafts ARE text: this lane exists to land statement
                    // language, so the semantic-text guard is deliberately open
                    // while everything else stays bounded to one finding.
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    // A draft is one agent's work by nature; independence
                    // is judged at verdict time, and landing stamps it
                    // false — requiring it here would permit nothing.
                    require_independence: false,
                    require_method_integrity: true,
                },
            }],
            "statement drafts (theoretical receipts) land at A2 (drafts ARE text)",
        )),
        "notes-threshold" => Some((
            vec![PolicyRule {
                id: "notes-threshold-v1".to_string(),
                effect: Outcome::Permit,
                claim_classes: vec!["finding_note".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 0,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: false,
                    require_method_integrity: false,
                },
            }],
            "notes attach without ceremony; anything touching claims defers",
        )),
        _ => None,
    }
}

// ── Cores (testable; no process exits, no prompts) ─────────────────────

/// Read + integrity-check the sealed `active.json`. This is the shared
/// pre-step for `test` and `sign`: the file must exist, parse, and its
/// `vap_` id must re-derive from the body (a tampered draft never reaches
/// a signature or an evaluation).
fn read_sealed_active(frontier: &Path) -> Result<AcceptancePolicy, CmdError> {
    let path = active_path(frontier);
    if !path.exists() {
        return Err(CmdError::hinted(
            ErrorKind::NotFound,
            "no policy at .vela/policies/active.json — the lane is closed",
            format!("draft one: `vela policy draft <template>` (templates: {TEMPLATES})"),
        ));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("read {}: {e}", path.display())))?;
    let policy: AcceptancePolicy = serde_json::from_str(&raw)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("active policy parse: {e}")))?;
    if !policy.id_is_valid() {
        return Err(CmdError::hinted(
            ErrorKind::Domain,
            format!(
                "active policy id does not re-derive (stored {}, sealed {})",
                policy.id,
                policy.content_address()
            ),
            "re-draft it: `vela policy draft <template> --replace`",
        ));
    }
    Ok(policy)
}

/// Seal a policy from a template into `.vela/policies/active.json`.
///
/// The id is the content address of the body (tamper-evident), the epoch is
/// the prior active policy's epoch + 1, expiry is +90 days, and the default
/// outcome is `Defer`. Sealing grants NO authority — that is the signature's
/// job. Refuses to overwrite an existing SIGNED active policy unless
/// `replace` is set; replacing snapshots the outgoing pair content-addressed
/// (past admissions keep verifying) and closes the lane until the new draft
/// is signed. Returns the sealed policy and whether a signed policy was
/// rotated out.
fn draft_policy(
    frontier: &Path,
    template: &str,
    replace: bool,
) -> Result<(AcceptancePolicy, bool), CmdError> {
    let (rules, _) = template_policy(template).ok_or_else(|| {
        CmdError::hinted(
            ErrorKind::Usage,
            format!("unknown template `{template}`"),
            format!("templates: {TEMPLATES}"),
        )
    })?;
    let project =
        repo::load_from_path(frontier).map_err(|e| CmdError::new(ErrorKind::Domain, e))?;

    let dir = policies_dir(frontier);
    let active = active_path(frontier);
    let sig = active_sig_path(frontier);
    let prior: Option<AcceptancePolicy> = std::fs::read_to_string(&active)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());
    let prior_epoch = prior.as_ref().map(|p| p.epoch).unwrap_or(0);

    let mut replaced_signed = false;
    if sig.exists() {
        let prior_id = prior
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "unparseable".to_string());
        if !replace {
            return Err(CmdError::hinted(
                ErrorKind::Exists,
                format!(
                    "an active SIGNED policy exists ({prior_id}, epoch {prior_epoch}) — refusing \
                     to overwrite a live governance object"
                ),
                "pass --replace to rotate it (the lane closes until the new draft is signed)",
            ));
        }
        // Snapshot the outgoing signed pair under its content address so the
        // events it admitted stay verifiable forever, then drop the pointer's
        // authority.
        if let Some(old) = &prior {
            let snap = dir.join(format!("{}.json", old.id));
            let snap_sig = dir.join(format!("{}.sig.json", old.id));
            if !snap.exists() {
                std::fs::copy(&active, &snap).map_err(|e| {
                    CmdError::new(
                        ErrorKind::Domain,
                        format!("snapshot {}: {e}", snap.display()),
                    )
                })?;
            }
            if !snap_sig.exists() {
                std::fs::copy(&sig, &snap_sig).map_err(|e| {
                    CmdError::new(
                        ErrorKind::Domain,
                        format!("snapshot {}: {e}", snap_sig.display()),
                    )
                })?;
            }
        }
        std::fs::remove_file(&sig).map_err(|e| {
            CmdError::new(ErrorKind::Domain, format!("remove {}: {e}", sig.display()))
        })?;
        replaced_signed = true;
    }

    // A rotation carries the standing grants forward: the new epoch is
    // the prior authority PLUS the template's rule, not a reset — a
    // signed lane must never close as a side effect of opening another.
    // A template rule with the same id supersedes (that IS the edit).
    let mut rules = rules;
    if let Some(old) = prior.as_ref().filter(|_| replaced_signed) {
        let new_ids: std::collections::HashSet<&str> =
            rules.iter().map(|r| r.id.as_str()).collect();
        let mut carried: Vec<PolicyRule> = old
            .rules
            .iter()
            .filter(|r| !new_ids.contains(r.id.as_str()))
            .cloned()
            .collect();
        carried.append(&mut rules);
        rules = carried;
    }
    let quorum = prior
        .as_ref()
        .filter(|_| replaced_signed)
        .map(|p| p.quorum.clone())
        .unwrap_or(Quorum {
            threshold: 1,
            eligible_roles: vec!["steward".to_string()],
        });

    let now = Utc::now();
    let mut policy = AcceptancePolicy {
        schema: "vela.acceptance_policy.v0.1".to_string(),
        id: String::new(),
        frontier_id: project.frontier_id.clone().unwrap_or_default(),
        epoch: prior_epoch + 1,
        issued_by: crate::cli_identity::load_identity()
            .map(|i| vec![i.actor_id])
            .unwrap_or_default(),
        quorum,
        rules,
        default: Outcome::Defer,
        expires_at: (now + chrono::Duration::days(90)).to_rfc3339(),
        revocation_ref: None,
    };
    policy.id = policy.content_address();
    if !policy.id_is_valid() {
        return Err(CmdError::new(
            ErrorKind::Domain,
            "internal: sealed id failed self-check",
        ));
    }

    std::fs::create_dir_all(&dir)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("create {}: {e}", dir.display())))?;
    let body = serde_json::to_string_pretty(&policy)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("serialize policy: {e}")))?;
    std::fs::write(&active, format!("{body}\n")).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("write {}: {e}", active.display()),
        )
    })?;
    Ok((policy, replaced_signed))
}

/// The signing core: Ed25519 over the sealed policy's canonical bytes —
/// byte-for-byte the body [`load_active_policy`] verifies (it re-serializes
/// the parsed struct through `canonical::to_canonical_bytes`, so file
/// formatting never drifts the signature). Writes `active.sig.json` as a
/// [`PolicySignatureRecord`] plus the content-addressed snapshot pair
/// `<vap_id>.json` / `<vap_id>.sig.json`, then round-trips the loader to
/// prove the lane actually opened. Refuses a revoked policy (revocation is
/// not undone by re-signing) and is idempotent on an already-open lane.
fn sign_active_policy(
    frontier: &Path,
    key: &ed25519_dalek::SigningKey,
    signed_at: &str,
) -> Result<(AcceptancePolicy, PolicySignatureRecord), CmdError> {
    let policy = read_sealed_active(frontier)?;
    if revoked_marker_path(frontier, &policy.id).exists() {
        return Err(CmdError::hinted(
            ErrorKind::Custody,
            format!(
                "{} was revoked — a revoked policy is never re-signed",
                policy.id
            ),
            "draft a new epoch: `vela policy draft <template>`",
        ));
    }
    if let Ok(Some(_)) = load_active_policy(frontier) {
        return Err(CmdError::hinted(
            ErrorKind::Exists,
            format!("{} is already signed — the lane is open", policy.id),
            "rotate deliberately: `vela policy revoke --reason <why>` then draft + sign",
        ));
    }

    let body = canonical::to_canonical_bytes(&policy)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("canonical: {e}")))?;
    let sig = key.sign(&body);
    let record = PolicySignatureRecord {
        policy_id: policy.id.clone(),
        signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
        signature: hex::encode(sig.to_bytes()),
        signed_at: signed_at.to_string(),
    };
    let sig_path = active_sig_path(frontier);
    let record_body = serde_json::to_string_pretty(&record)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("serialize signature: {e}")))?;
    std::fs::write(&sig_path, format!("{record_body}\n")).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("write {}: {e}", sig_path.display()),
        )
    })?;

    // Content-addressed snapshots: the pair that keeps every event this
    // policy admits verifiable after `active.json` rotates.
    let dir = policies_dir(frontier);
    let snap = dir.join(format!("{}.json", policy.id));
    let snap_sig = dir.join(format!("{}.sig.json", policy.id));
    if !snap.exists() {
        std::fs::copy(active_path(frontier), &snap).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap.display()),
            )
        })?;
    }
    std::fs::copy(&sig_path, &snap_sig).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("snapshot {}: {e}", snap_sig.display()),
        )
    })?;

    // The proof of the ceremony: the exact loader the accept lane uses must
    // see an open lane, or the signature we just wrote is worthless.
    match load_active_policy(frontier) {
        Ok(Some(_)) => Ok((policy, record)),
        Ok(None) => Err(CmdError::new(
            ErrorKind::Domain,
            "signature written but the loader does not see an open lane",
        )),
        Err(e) => Err(CmdError::new(
            ErrorKind::Domain,
            format!("post-sign verification failed: {e}"),
        )),
    }
}

/// The revocation core: the `active.sig.json` pointer loses authority (the
/// lane closes) while the content-addressed snapshots STAY so every event the
/// policy admitted keeps verifying on replay. Writes a
/// `revoked-<vap_id>.json` marker that `sign` honors — re-signing never
/// resurrects a revoked policy. Returns the revoked `vap_` id.
fn revoke_active_policy(
    frontier: &Path,
    actor: &str,
    reason: &str,
    revoked_at: &str,
) -> Result<String, CmdError> {
    let policy = read_sealed_active(frontier)?;
    let sig_path = active_sig_path(frontier);
    if !sig_path.exists() {
        return Err(CmdError::new(
            ErrorKind::Exists,
            format!(
                "the lane is already closed — {} carries no signature",
                policy.id
            ),
        ));
    }

    // Replay insurance first: never delete the only copy of the signature
    // that past policy-lane events verify against.
    let dir = policies_dir(frontier);
    let snap = dir.join(format!("{}.json", policy.id));
    let snap_sig = dir.join(format!("{}.sig.json", policy.id));
    if !snap.exists() {
        std::fs::copy(active_path(frontier), &snap).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap.display()),
            )
        })?;
    }
    if !snap_sig.exists() {
        std::fs::copy(&sig_path, &snap_sig).map_err(|e| {
            CmdError::new(
                ErrorKind::Domain,
                format!("snapshot {}: {e}", snap_sig.display()),
            )
        })?;
    }
    std::fs::remove_file(&sig_path).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("remove {}: {e}", sig_path.display()),
        )
    })?;

    let marker = revoked_marker_path(frontier, &policy.id);
    let record = json!({
        "schema": "vela.policy_revocation.v0.1",
        "policy_id": policy.id,
        "revoked_at": revoked_at,
        "revoked_by": actor,
        "reason": reason,
    });
    let body = serde_json::to_string_pretty(&record)
        .map_err(|e| CmdError::new(ErrorKind::Domain, format!("serialize revocation: {e}")))?;
    std::fs::write(&marker, format!("{body}\n")).map_err(|e| {
        CmdError::new(
            ErrorKind::Domain,
            format!("write {}: {e}", marker.display()),
        )
    })?;
    Ok(policy.id)
}

/// One shadow decision from the dry-run.
struct ShadowRow {
    proposal: String,
    kind: String,
    claim_class: String,
    outcome: Outcome,
    matched_rule_ids: Vec<String>,
    reasons: Vec<String>,
}

/// Dry-run the policy over every pending proposal under the CONSERVATIVE
/// default [`PolicyContext`] (everything unproven, every escalation trigger
/// set) — so the preview can never promise a permit the landing path would
/// not grant. Only the claim class is derived (from the proposal's kind and
/// payload text); richer context derivation — gate-derived assurance,
/// impact tiers, dependency counts — lands with `vela land`, which builds
/// the context the accept lane actually evaluates. Pure: never mutates.
fn evaluate_pending(project: &Project, policy: &AcceptancePolicy, now: &str) -> Vec<ShadowRow> {
    project
        .proposals
        .iter()
        .filter(|p| p.status == "pending_review")
        .map(|p| {
            let ctx = PolicyContext {
                claim_class: proposal_claim_class(p),
                ..PolicyContext::default()
            };
            let d = evaluate(policy, &ctx, now);
            ShadowRow {
                proposal: p.id.clone(),
                kind: p.kind.clone(),
                claim_class: ctx.claim_class,
                outcome: d.outcome,
                matched_rule_ids: d.matched_rule_ids,
                reasons: d.reasons,
            }
        })
        .collect()
}

/// Classify a proposal into a structural claim class: the kind wins for
/// note-shaped work, then the assertion text. Conservative — an
/// unrecognized proposal is "unknown", and the engine then defers, never
/// permits.
fn proposal_claim_class(p: &StateProposal) -> String {
    if p.kind == "finding.note" {
        return "finding_note".to_string();
    }
    let text = p
        .payload
        .get("assertion")
        .and_then(|a| a.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| p.payload.get("text").and_then(|t| t.as_str()))
        .unwrap_or_default();
    classify(text).to_string()
}

/// Classify a claim into a structural class from its assertion text.
fn classify(text: &str) -> &'static str {
    let t = text.to_lowercase();
    if t.contains("a309370") || t.contains("sidon") {
        "sidon_lower_bound"
    } else if t.contains("lean") || t.contains("formaliz") || t.contains("theorem") {
        "formal_theorem"
    } else if t.contains("oeis ") || t.contains("oeis:") {
        "oeis_sequence"
    } else if t.contains("erdős problem") || t.contains("erdos problem") {
        "erdos_problem"
    } else {
        "unknown"
    }
}

/// One policy-lane admission read back out of the event log.
struct Admission {
    policy_id: String,
    event_id: String,
    proposal_id: String,
    rule_ids: Vec<String>,
    timestamp: String,
}

/// Every event the policy lane admitted, in log order: the `policy_lane`
/// payload block is the tamper-evident stamp `policy_accept` writes into
/// the content address, so reading it back requires no join.
fn lane_admissions(project: &Project) -> Vec<Admission> {
    project
        .events
        .iter()
        .filter_map(|ev| {
            let lane = ev.payload.get(POLICY_LANE_PAYLOAD_KEY)?;
            Some(Admission {
                policy_id: lane
                    .get("policy_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                event_id: ev.id.clone(),
                proposal_id: lane
                    .get("certificate")
                    .and_then(|c| c.get("proposal_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                rule_ids: lane
                    .get("rule_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|r| r.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                timestamp: ev.timestamp.clone(),
            })
        })
        .collect()
}

// ── Rendering ──────────────────────────────────────────────────────────

fn constraints_summary(c: &Constraints) -> String {
    let mut parts = vec![
        format!("A>={}", c.required_assurance_min),
        format!("changed<={}", c.max_changed_findings),
        format!("deps<={}", c.max_downstream_dependents),
    ];
    parts.push(if c.allow_semantic_text_change {
        "text change allowed".to_string()
    } else {
        "no text change".to_string()
    });
    if c.require_independence {
        parts.push("independence".to_string());
    }
    if c.require_method_integrity {
        parts.push("method integrity".to_string());
    }
    if c.allow_contested {
        parts.push("contested allowed".to_string());
    }
    if c.allow_governance_mutation {
        parts.push("governance allowed".to_string());
    }
    parts.join(" · ")
}

fn render_policy(p: &AcceptancePolicy) {
    println!("  policy    {} · epoch {}", p.id, p.epoch);
    if !p.frontier_id.is_empty() {
        println!("  frontier  {}", p.frontier_id);
    }
    if !p.issued_by.is_empty() {
        println!("  issued by {}", p.issued_by.join(", "));
    }
    println!(
        "  default   {} · expires {}",
        p.default.as_str(),
        p.expires_at
    );
    println!();
    println!("  rules");
    for r in &p.rules {
        let classes = if r.claim_classes.is_empty() {
            "any claim class".to_string()
        } else {
            r.claim_classes.join(", ")
        };
        println!("    {:<6} {}  →  {}", r.effect.as_str(), r.id, classes);
        if r.effect == Outcome::Permit {
            println!(
                "           {}",
                style::dim(&constraints_summary(&r.constraints))
            );
        }
    }
}

fn print_admission_rows(rows: &[&Admission]) {
    for a in rows {
        let rules = if a.rule_ids.is_empty() {
            String::new()
        } else {
            format!("  ← {}", a.rule_ids.join(", "))
        };
        println!(
            "    {}  {}  {}{rules}",
            a.timestamp, a.event_id, a.proposal_id
        );
    }
}

fn confirm(prompt: &str) -> bool {
    use std::io::{BufRead, Write};
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

// ── The six verbs ──────────────────────────────────────────────────────

/// `vela policy show` — the active policy: id, epoch, rules, expiry, its
/// signature state (a sealed-unsigned policy carries NO authority), and what
/// it admitted lately (the last policy-lane events stamped with its id).
pub(crate) fn cmd_policy_show(frontier: &Path, json: bool) {
    let active = active_path(frontier);
    if !active.exists() {
        if json {
            print_json(&json!({
                "ok": true,
                "command": "policy.show",
                "state": "absent",
                "policy": serde_json::Value::Null,
                "signature": serde_json::Value::Null,
                "admissions": { "count": 0, "last": [] },
            }));
        } else {
            ui::header("POLICY", &frontier.display().to_string(), None);
            println!("  no active policy — the lane is closed; every accept is a key ceremony");
            println!(
                "  open one: `vela policy draft <template>` (templates: {TEMPLATES}), then `vela policy sign`"
            );
        }
        return;
    }

    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let sig_present = active_sig_path(frontier).exists();
    let signed = if sig_present {
        // A present-but-broken governance pair must never render as healthy.
        match load_active_policy(frontier) {
            Ok(v) => v,
            Err(e) => fail_with(
                ErrorKind::Domain,
                &format!("active policy pair is broken: {e}"),
                Some(
                    "revoke and re-draft: `vela policy revoke --reason <why>`, then `vela policy draft`",
                ),
            ),
        }
    } else {
        None
    };
    let record: Option<PolicySignatureRecord> = if sig_present {
        std::fs::read_to_string(active_sig_path(frontier))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
    } else {
        None
    };
    let revoked = revoked_marker_path(frontier, &policy.id).exists();
    let now = Utc::now().to_rfc3339();
    let expired = policy.is_expired(&now);

    // What it admitted lately. A missing/partial event log is fine — the
    // policy files are still worth showing.
    let admissions: Vec<Admission> = repo::load_from_path(frontier)
        .map(|p| lane_admissions(&p))
        .unwrap_or_default();
    let mine: Vec<&Admission> = admissions
        .iter()
        .filter(|a| a.policy_id == policy.id)
        .collect();
    let last: Vec<&Admission> = mine.iter().rev().take(5).rev().copied().collect();

    let state = if signed.is_some() {
        "signed"
    } else if revoked {
        "revoked"
    } else {
        "sealed_unsigned"
    };

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.show",
            "state": state,
            "expired": expired,
            "policy": serde_json::to_value(&policy).unwrap_or_default(),
            "signature": record.as_ref().map(|r| json!({
                "signer_pubkey_hex": r.signer_pubkey_hex,
                "signed_at": r.signed_at,
            })),
            "admissions": {
                "count": mine.len(),
                "last": last.iter().map(|a| json!({
                    "event": a.event_id,
                    "proposal": a.proposal_id,
                    "rule_ids": a.rule_ids,
                    "timestamp": a.timestamp,
                })).collect::<Vec<_>>(),
            },
        }));
        return;
    }

    ui::header("POLICY", &frontier.display().to_string(), None);
    render_policy(&policy);
    println!();
    match (&signed, revoked) {
        (Some(v), _) => {
            let signed_at = record.as_ref().map(|r| r.signed_at.as_str()).unwrap_or("");
            println!(
                "  {} signed by {}… at {signed_at} — the lane is open",
                style::ok("state"),
                &v.signer_pubkey_hex[..16.min(v.signer_pubkey_hex.len())]
            );
        }
        (None, true) => {
            println!(
                "  {} REVOKED — the lane is closed; this policy is never re-signed",
                style::lost("state")
            );
        }
        (None, false) => {
            println!(
                "  {} SEALED-UNSIGNED — carries no authority; sign with `vela policy sign`",
                style::warn("state")
            );
        }
    }
    if expired {
        println!(
            "  {} expired at {} — the evaluator denies everything",
            style::lost("note"),
            policy.expires_at
        );
    }
    println!();
    if mine.is_empty() {
        println!("  admitted under this policy: nothing yet");
    } else {
        println!(
            "  admitted under this policy: {} event(s), last {}:",
            mine.len(),
            last.len()
        );
        print_admission_rows(&last);
    }
}

/// `vela policy draft <template>` — seal a policy from the template ladder.
/// Sealing fixes the content-addressed id so the exact rules a human reviews
/// are the exact rules the signature covers. No authority until `sign`.
pub(crate) fn cmd_policy_draft(frontier: &Path, template: &str, replace: bool, json: bool) {
    let (policy, replaced) = draft_policy(frontier, template, replace).unwrap_or_else(|e| e.fail());
    let summary = template_policy(template)
        .map(|(_, s)| s)
        .unwrap_or_default();

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.draft",
            "template": template,
            "policy_id": policy.id,
            "epoch": policy.epoch,
            "frontier_id": policy.frontier_id,
            "expires_at": policy.expires_at,
            "replaced_signed": replaced,
            "signed": false,
            "policy": serde_json::to_value(&policy).unwrap_or_default(),
            "next": "vela policy sign",
        }));
        return;
    }

    ui::header("POLICY", template, Some("sealed draft"));
    println!("  {summary}");
    println!();
    render_policy(&policy);
    println!();
    if replaced {
        println!(
            "  {} the outgoing signed policy was snapshotted; the lane is CLOSED until you sign",
            style::warn("rotated")
        );
    }
    println!("  sealed — carries no authority yet");
    println!("  review the rules above, then: `vela policy sign` (one confirm, one key read)");
}

/// `vela policy test` — dry-run the active (or still-sealed) policy over
/// every pending proposal. Uses the conservative default [`PolicyContext`]
/// (see [`evaluate_pending`]) so the preview under-promises; richer context
/// derivation lands with `vela land`. Never mutates anything.
pub(crate) fn cmd_policy_test(frontier: &Path, json: bool) {
    let spin = (!json).then(|| {
        crate::cli::progress::Spinner::start("dry-running the policy over every pending proposal")
    });
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let lane_open = matches!(load_active_policy(frontier), Ok(Some(_)));
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let now = Utc::now().to_rfc3339();
    let rows = evaluate_pending(&project, &policy, &now);

    let permit = rows.iter().filter(|r| r.outcome == Outcome::Permit).count();
    let defer = rows.iter().filter(|r| r.outcome == Outcome::Defer).count();
    let deny = rows.iter().filter(|r| r.outcome == Outcome::Deny).count();

    if let Some(s) = spin {
        s.finish("evaluated");
    }
    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.test",
            "mode": "dry_run",
            "policy_id": policy.id,
            "lane_open": lane_open,
            "evaluator": EVALUATOR_VERSION,
            "now": now,
            "summary": { "permit": permit, "defer": defer, "deny": deny, "total": rows.len() },
            "decisions": rows.iter().map(|r| json!({
                "proposal": r.proposal,
                "kind": r.kind,
                "claim_class": r.claim_class,
                "outcome": r.outcome.as_str(),
                "matched_rules": r.matched_rule_ids,
                "reasons": r.reasons,
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header("POLICY", &policy.id, Some("dry-run"));
    println!(
        "  {} pending proposal(s): {permit} permit, {defer} defer, {deny} deny",
        rows.len()
    );
    println!();
    for r in rows.iter().take(20) {
        let why = if r.outcome == Outcome::Permit {
            r.matched_rule_ids.first().cloned().unwrap_or_default()
        } else {
            r.reasons.first().cloned().unwrap_or_default()
        };
        println!(
            "  [{}] {} ({}, {})  ← {why}",
            r.outcome.as_str(),
            r.proposal,
            r.kind,
            r.claim_class
        );
    }
    if rows.len() > 20 {
        println!("  … {} more (use --json for all)", rows.len() - 20);
    }
    println!();
    println!("  dry-run under the maximally-cautious context — nothing applied.");
    println!(
        "  the landing path derives the real context (gate assurance, impact) at accept time."
    );
    if !lane_open {
        println!(
            "  {} this policy is not signed — even a permit would land nothing until `vela policy sign`",
            style::warn("note")
        );
    }
}

/// `vela policy sign` — THE ceremony. A human (never `agent:`/`ci:`) reviews
/// the sealed rules, confirms once, the key is read once, and the signature
/// over the canonical bytes opens the lane: every matching permit rule is now
/// live authority with no per-item key ceremony.
pub(crate) fn cmd_policy_sign(frontier: &Path, key: Option<&Path>, yes: bool) {
    crate::cli::sign_session::ceremony_binary_gate();
    // Humans only: the whole point of the lane is that a HUMAN signed once.
    let actor = crate::cli_identity::resolve_decision_actor(None);
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    if revoked_marker_path(frontier, &policy.id).exists() {
        fail_with(
            ErrorKind::Custody,
            &format!(
                "{} was revoked — a revoked policy is never re-signed",
                policy.id
            ),
            Some("draft a new epoch: `vela policy draft <template>`"),
        );
    }
    match load_active_policy(frontier) {
        Ok(Some(_)) => fail_with(
            ErrorKind::Exists,
            &format!("{} is already signed — the lane is open", policy.id),
            Some("rotate deliberately: `vela policy revoke --reason <why>` then draft + sign"),
        ),
        Err(e) => println!(
            "  {} existing signature is broken ({e}) — re-signing replaces it",
            style::warn("note")
        ),
        Ok(None) => {}
    }

    ui::header("POLICY", &policy.id, Some("sign — the lane opens"));
    render_policy(&policy);
    println!();
    println!("  signing as {actor}");
    println!("  a signature makes every permit rule above LIVE: agents land that class of");
    println!("  gated work with no per-item ceremony, until expiry or `vela policy revoke`.");
    println!();
    if !yes && !confirm(&format!("  sign {} and open the lane? [y/N] ", policy.id)) {
        fail_with(
            ErrorKind::Usage,
            "not signed — no confirmation",
            Some("re-run and answer y, or pass --yes"),
        );
    }

    // ONE key read, after the human has seen and confirmed the exact rules.
    let signing_key = crate::cli_identity::resolve_signing_key(key);
    let (policy, record) = sign_active_policy(frontier, &signing_key, &Utc::now().to_rfc3339())
        .unwrap_or_else(|e| e.fail());

    println!();
    println!("  {} policy live — the lane is open", style::ok("signed"));
    println!(
        "  {} · epoch {} · signer {}…",
        policy.id,
        policy.epoch,
        &record.signer_pubkey_hex[..16.min(record.signer_pubkey_hex.len())]
    );
    println!();
    for r in policy.rules.iter().filter(|r| r.effect == Outcome::Permit) {
        println!(
            "  auto-lands: {}  ({})",
            r.claim_classes.join(", "),
            constraints_summary(&r.constraints)
        );
    }
    println!("  everything else defers to `vela sign` — the queue, not silence.");
    println!("  close the lane anytime: `vela policy revoke --reason <why>`");
}

/// `vela policy revoke --reason <why>` — a human closes the lane. The
/// `active.sig.json` pointer is deleted (authority gone) while the
/// content-addressed snapshots stay, so every event the policy admitted
/// keeps verifying on replay forever.
pub(crate) fn cmd_policy_revoke(frontier: &Path, reason: &str, yes: bool) {
    let actor = crate::cli_identity::resolve_decision_actor(None);
    if reason.trim().is_empty() {
        fail_with(
            ErrorKind::Usage,
            "a revocation needs a reason — it is recorded next to the closed lane",
            Some("vela policy revoke --reason \"rotating to a tighter epoch\""),
        );
    }
    let policy = read_sealed_active(frontier).unwrap_or_else(|e| e.fail());
    let admitted = repo::load_from_path(frontier)
        .map(|p| {
            lane_admissions(&p)
                .iter()
                .filter(|a| a.policy_id == policy.id)
                .count()
        })
        .unwrap_or(0);

    ui::header("POLICY", &policy.id, Some("revoke — closing the lane"));
    println!("  epoch {} · admitted {admitted} event(s)", policy.epoch);
    println!("  reason: {reason}");
    println!("  snapshots stay under .vela/policies/ — past admissions keep verifying.");
    println!();
    if !yes && !confirm(&format!("  close the lane for {}? [y/N] ", policy.id)) {
        fail_with(
            ErrorKind::Usage,
            "not revoked — no confirmation",
            Some("re-run and answer y, or pass --yes"),
        );
    }

    let vap = revoke_active_policy(frontier, &actor, reason, &Utc::now().to_rfc3339())
        .unwrap_or_else(|e| e.fail());
    println!();
    println!(
        "  {} lane closed — {vap} no longer admits anything",
        style::ok("revoked")
    );
    println!("  reopen with a NEW policy: `vela policy draft <template>` then `vela policy sign`");
}

/// `vela policy log` — every policy-lane admission across ALL policies (the
/// active one and every rotated-out epoch), grouped by policy id.
pub(crate) fn cmd_policy_log(frontier: &Path, json: bool) {
    let project =
        repo::load_from_path(frontier).unwrap_or_else(|e| fail_with(ErrorKind::Domain, &e, None));
    let admissions = lane_admissions(&project);
    let mut by_policy: std::collections::BTreeMap<String, Vec<&Admission>> =
        std::collections::BTreeMap::new();
    for a in &admissions {
        by_policy.entry(a.policy_id.clone()).or_default().push(a);
    }
    let active_id = read_sealed_active(frontier).ok().map(|p| p.id);

    if json {
        print_json(&json!({
            "ok": true,
            "command": "policy.log",
            "total": admissions.len(),
            "policies": by_policy.iter().map(|(pid, rows)| json!({
                "policy_id": pid,
                "active": Some(pid) == active_id.as_ref(),
                "count": rows.len(),
                "admissions": rows.iter().map(|a| json!({
                    "event": a.event_id,
                    "proposal": a.proposal_id,
                    "rule_ids": a.rule_ids,
                    "timestamp": a.timestamp,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }));
        return;
    }

    ui::header(
        "POLICY",
        &frontier.display().to_string(),
        Some("admissions"),
    );
    if by_policy.is_empty() {
        println!("  no policy-lane admissions yet — every event in this log is key-signed");
        println!("  open a lane: `vela policy draft <template>` then `vela policy sign`");
        return;
    }
    println!(
        "  {} admission(s) under {} policy(ies)",
        admissions.len(),
        by_policy.len()
    );
    for (pid, rows) in &by_policy {
        let live = if Some(pid) == active_id.as_ref() {
            format!("  {}", style::live("active"))
        } else {
            String::new()
        };
        println!();
        println!("  {pid} — {} admission(s){live}", rows.len());
        print_admission_rows(rows);
    }
}

// ── Transitional argv shim ─────────────────────────────────────────────

/// TRANSITIONAL: the pre-clap `vela policy` intercept in
/// `cli/mod.rs::run_from_args` still routes raw argv here. The clap surface
/// (`Commands::Policy` → the typed `cmd_policy_*` functions above) is the
/// real dispatch; once the lead deletes that intercept, delete this shim —
/// it exists only so both paths speak the same six verbs meanwhile.
pub(crate) fn run(args: &[String]) {
    let verb = args.get(2).map(String::as_str).unwrap_or("");
    let json = args.iter().any(|a| a == "--json");
    let replace = args.iter().any(|a| a == "--replace");
    let yes = args.iter().any(|a| a == "--yes");
    let value_of = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let key = value_of("--key").map(PathBuf::from);
    let reason = value_of("--reason");

    // Positional operands after the verb, skipping flags and their values.
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 3;
    while i < args.len() {
        let a = &args[i];
        if a == "--key" || a == "--reason" {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        positionals.push(a.clone());
        i += 1;
    }

    let usage = "usage: vela policy <show|draft <template>|test|sign|revoke --reason <why>|log> \
                 [frontier] [--json] [--replace] [--yes] [--key <path>]";
    match verb {
        "show" | "test" | "log" | "sign" | "revoke" => {
            let interactive = matches!(verb, "sign" | "revoke");
            ui::set_mode("policy", json && !interactive);
            let dir = ui::resolve_frontier(positionals.first().map(PathBuf::from));
            match verb {
                "show" => cmd_policy_show(&dir, json),
                "test" => cmd_policy_test(&dir, json),
                "log" => cmd_policy_log(&dir, json),
                "sign" => cmd_policy_sign(&dir, key.as_deref(), yes),
                "revoke" => {
                    let reason = reason.unwrap_or_else(|| {
                        fail_with(
                            ErrorKind::Usage,
                            "revoke needs --reason <why>",
                            Some("vela policy revoke --reason \"rotating epochs\""),
                        )
                    });
                    cmd_policy_revoke(&dir, &reason, yes);
                }
                _ => unreachable!(),
            }
        }
        "draft" => {
            ui::set_mode("policy", json);
            let template = positionals.first().cloned().unwrap_or_else(|| {
                fail_with(
                    ErrorKind::Usage,
                    &format!("draft needs a template (templates: {TEMPLATES})"),
                    Some("vela policy draft witness-rederivation"),
                )
            });
            let dir = ui::resolve_frontier(positionals.get(1).map(PathBuf::from));
            cmd_policy_draft(&dir, &template, replace, json);
        }
        "seal" => {
            ui::set_mode("policy", json);
            fail_with(
                ErrorKind::Usage,
                "`seal` folded into `draft` — drafting from a template seals the policy",
                Some(format!("vela policy draft <template> (templates: {TEMPLATES})").as_str()),
            );
        }
        "evaluate" => {
            ui::set_mode("policy", json);
            fail_with(
                ErrorKind::Usage,
                "`evaluate` folded into `test` — a dry-run over every pending proposal",
                Some("vela policy test [frontier] [--json]"),
            );
        }
        _ => {
            ui::set_mode("policy", json);
            fail_with(ErrorKind::Usage, usage, None);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::events::StateTarget;
    use vela_protocol::proposals::new_proposal;

    fn init_frontier(tmp: &TempDir) -> PathBuf {
        let dir = tmp.path().to_path_buf();
        vela_protocol::frontier_repo::initialize(
            &dir,
            vela_protocol::frontier_repo::InitOptions {
                name: "policy-porcelain-test",
                template: "",
                initialize_git: false,
            },
        )
        .unwrap();
        dir
    }

    fn throwaway_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    const AT: &str = "2026-07-03T00:00:00Z";

    #[test]
    fn draft_seals_a_content_addressed_policy() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        let (policy, replaced) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        assert!(!replaced);
        assert!(policy.id.starts_with("vap_"));
        assert!(policy.id_is_valid());
        assert_eq!(policy.epoch, 1);
        assert_eq!(policy.default, Outcome::Defer);
        // The sealed file on disk re-derives its own id.
        let raw = std::fs::read_to_string(active_path(&dir)).unwrap();
        let on_disk: AcceptancePolicy = serde_json::from_str(&raw).unwrap();
        assert_eq!(on_disk.id, policy.id);
        assert!(on_disk.id_is_valid());
        // Sealed-but-unsigned carries no authority: the loader sees no lane.
        assert!(load_active_policy(&dir).unwrap().is_none());
    }

    #[test]
    fn templates_encode_the_ladder() {
        let (w, _) = template_policy("witness-rederivation").unwrap();
        assert_eq!(w[0].constraints.required_assurance_min, 3);
        assert!(!w[0].constraints.allow_semantic_text_change);
        assert!(w[0].constraints.require_independence);
        assert!(w[0].constraints.require_method_integrity);

        let (s, _) = template_policy("statement-drafts").unwrap();
        assert_eq!(s[0].constraints.required_assurance_min, 2);
        assert!(
            s[0].constraints.allow_semantic_text_change,
            "drafts ARE text"
        );
        assert_eq!(s[0].constraints.max_changed_findings, 1);

        let (n, _) = template_policy("notes-threshold").unwrap();
        assert_eq!(n[0].constraints.required_assurance_min, 0);
        assert_eq!(n[0].constraints.max_changed_findings, 1);
        assert_eq!(n[0].constraints.max_downstream_dependents, 0);

        assert!(template_policy("nonsense").is_none());
    }

    #[test]
    fn draft_refuses_to_overwrite_a_signed_policy() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        let (old, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        sign_active_policy(&dir, &throwaway_key(), AT).unwrap();

        let err = draft_policy(&dir, "notes-threshold", false).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists, "{}", err.message);

        // --replace rotates: snapshots the outgoing pair, closes the lane.
        let (newer, replaced) = draft_policy(&dir, "notes-threshold", true).unwrap();
        assert!(replaced);
        assert_eq!(newer.epoch, 2);
        assert_ne!(newer.id, old.id);
        assert!(
            load_active_policy(&dir).unwrap().is_none(),
            "lane closes until the new draft is signed"
        );
        assert!(policies_dir(&dir).join(format!("{}.json", old.id)).exists());
        assert!(
            policies_dir(&dir)
                .join(format!("{}.sig.json", old.id))
                .exists()
        );
    }

    #[test]
    fn sign_round_trip_opens_the_lane() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        draft_policy(&dir, "witness-rederivation", false).unwrap();

        let key = throwaway_key();
        let (policy, record) = sign_active_policy(&dir, &key, AT).unwrap();
        assert_eq!(record.policy_id, policy.id);
        assert_eq!(record.signed_at, AT);

        // The exact loader the accept lane uses sees an open lane.
        let verified = load_active_policy(&dir).unwrap().expect("lane open");
        assert_eq!(verified.policy.id, policy.id);
        assert_eq!(verified.signer_pubkey_hex, record.signer_pubkey_hex);

        // Content-addressed snapshots survive future rotation.
        assert!(
            policies_dir(&dir)
                .join(format!("{}.json", policy.id))
                .exists()
        );
        assert!(
            policies_dir(&dir)
                .join(format!("{}.sig.json", policy.id))
                .exists()
        );

        // Idempotent: signing an already-open lane is `Exists`, not a re-sign.
        let err = sign_active_policy(&dir, &key, AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists);
    }

    #[test]
    fn revoke_closes_the_lane_and_keeps_snapshots() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        draft_policy(&dir, "witness-rederivation", false).unwrap();
        let key = throwaway_key();
        let (policy, _) = sign_active_policy(&dir, &key, AT).unwrap();

        let vap = revoke_active_policy(&dir, "reviewer:test", "rotating the ladder", AT).unwrap();
        assert_eq!(vap, policy.id);
        // The lane is closed: the loader sees nothing to grant authority.
        assert!(load_active_policy(&dir).unwrap().is_none());
        // But the snapshots stay for replay verification of past events.
        assert!(policies_dir(&dir).join(format!("{vap}.json")).exists());
        assert!(policies_dir(&dir).join(format!("{vap}.sig.json")).exists());
        let marker = revoked_marker_path(&dir, &vap);
        assert!(marker.exists());
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(record["reason"], "rotating the ladder");
        assert_eq!(record["revoked_by"], "reviewer:test");

        // A revoked policy is never re-signed.
        let err = sign_active_policy(&dir, &key, AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Custody);

        // Revoking again is the idempotent no-op, not a crash.
        let err = revoke_active_policy(&dir, "reviewer:test", "again", AT).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Exists);
    }

    #[test]
    fn dry_run_is_conservative_and_never_permits() {
        // Pure in-memory: the dry-run must not need (or touch) a store.
        let mut project = vela_protocol::project::assemble("t", vec![], 0, 0, "test");
        project.proposals.push(new_proposal(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_x".to_string(),
            },
            "agent:scout",
            "agent",
            "observation",
            json!({"text": "a sidon observation"}),
            Vec::new(),
            Vec::new(),
        ));
        project.proposals.push(new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_y".to_string(),
            },
            "agent:prover",
            "agent",
            "exact witness",
            json!({"assertion": {"text": "sidon set lower bound"}}),
            Vec::new(),
            Vec::new(),
        ));

        let (rules, _) = template_policy("notes-threshold").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        let rows = evaluate_pending(&project, &policy, AT);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter().all(|r| r.outcome != Outcome::Permit),
            "the maximally-cautious context can never permit"
        );
        let note = rows.iter().find(|r| r.kind == "finding.note").unwrap();
        assert_eq!(note.claim_class, "finding_note");
        let review = rows.iter().find(|r| r.kind == "finding.review").unwrap();
        assert_eq!(review.claim_class, "sidon_lower_bound");
    }

    /// The template must permit what `vela land` actually produces — the
    /// exact PolicyContext a draft receipt with a passing verifier run is
    /// stamped with (workflow.rs): class receipt_theoretical, A2, text
    /// mutated, independence NOT satisfied. A template that never fires
    /// is a ceremony that delegates nothing.
    #[test]
    fn statement_drafts_template_permits_a_landed_receipt() {
        let (rules, _) = template_policy("statement-drafts").unwrap();
        let mut policy = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: "vfr_test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["steward".to_string()],
            },
            rules,
            default: Outcome::Defer,
            expires_at: "2099-12-31T23:59:59Z".to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();

        let ctx = vela_protocol::acceptance_policy::PolicyContext {
            claim_class: "receipt_theoretical".to_string(),
            assurance_level: 2,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: true,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: false,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
        };
        let d = vela_protocol::acceptance_policy::evaluate(&policy, &ctx, AT);
        assert_eq!(
            d.outcome,
            Outcome::Permit,
            "a landed statement draft must route through this lane: {:?}",
            d.reasons
        );
    }

    /// Rotating a signed policy must carry its rules into the new epoch:
    /// opening one lane may never close another as a side effect.
    #[test]
    fn replace_carries_the_prior_rules_forward() {
        let tmp = TempDir::new().unwrap();
        let dir = init_frontier(&tmp);
        let (first, _) = draft_policy(&dir, "witness-rederivation", false).unwrap();
        // Rotation semantics trigger only on a SIGNED prior; the sig's
        // content is irrelevant to draft's existence check.
        std::fs::write(active_sig_path(&dir), "{}\n").unwrap();

        let (second, replaced) = draft_policy(&dir, "statement-drafts", true).unwrap();
        assert!(replaced);
        assert_eq!(second.epoch, first.epoch + 1);
        let ids: Vec<&str> = second.rules.iter().map(|r| r.id.as_str()).collect();
        assert!(
            ids.contains(&"witness-rederivation-v1"),
            "the standing grant must survive the rotation: {ids:?}"
        );
        assert!(ids.contains(&"statement-drafts-v1"), "{ids:?}");
        // The outgoing signed pair is snapshotted under its content address.
        assert!(
            policies_dir(&dir)
                .join(format!("{}.json", first.id))
                .exists()
        );
        // Re-drafting the SAME template supersedes, not duplicates.
        std::fs::write(active_sig_path(&dir), "{}\n").unwrap();
        let (third, _) = draft_policy(&dir, "statement-drafts", true).unwrap();
        let dup = third
            .rules
            .iter()
            .filter(|r| r.id == "statement-drafts-v1")
            .count();
        assert_eq!(dup, 1, "same-id template rule supersedes the carried one");
    }

    #[test]
    fn log_finds_stamped_events() {
        let mut project = vela_protocol::project::assemble("t", vec![], 0, 0, "test");
        let mut lane_payload = serde_json::Map::new();
        lane_payload.insert(
            POLICY_LANE_PAYLOAD_KEY.to_string(),
            json!({
                "policy_id": "vap_abc",
                "rule_ids": ["witness-rederivation-v1"],
                "certificate": {"proposal_id": "vpr_9"},
                "context": {},
            }),
        );
        let lane_event: vela_protocol::events::StateEvent = serde_json::from_value(json!({
            "id": "vev_lane1",
            "kind": "finding.reviewed",
            "target": {"type": "finding", "id": "vf_x"},
            "actor": {"id": "policy:vap_abc", "type": "agent"},
            "timestamp": AT,
            "reason": "policy permit",
            "before_hash": "h0",
            "after_hash": "h1",
            "payload": serde_json::Value::Object(lane_payload),
        }))
        .unwrap();
        let plain_event: vela_protocol::events::StateEvent = serde_json::from_value(json!({
            "id": "vev_plain",
            "kind": "finding.reviewed",
            "target": {"type": "finding", "id": "vf_x"},
            "actor": {"id": "reviewer:will", "type": "human"},
            "timestamp": AT,
            "reason": "key-signed accept",
            "before_hash": "h1",
            "after_hash": "h2",
            "payload": {},
        }))
        .unwrap();
        project.events.push(lane_event);
        project.events.push(plain_event);

        let rows = lane_admissions(&project);
        assert_eq!(rows.len(), 1, "only the stamped event is an admission");
        assert_eq!(rows[0].policy_id, "vap_abc");
        assert_eq!(rows[0].event_id, "vev_lane1");
        assert_eq!(rows[0].proposal_id, "vpr_9");
        assert_eq!(rows[0].rule_ids, vec!["witness-rederivation-v1"]);
        assert_eq!(rows[0].timestamp, AT);
    }
}
