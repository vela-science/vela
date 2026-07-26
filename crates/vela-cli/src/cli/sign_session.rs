//! `vela sign` — THE human proposal-decision ceremony. One frontier,
//! one bounded review set, one summary, one confirm, one key read, and
//! one recoverable frontier transaction. Detached bytes remain a separate
//! signing lane.
//!
//! Custody: agent/ci actors exit 4 before anything renders. There is
//! deliberately no `--all --yes` for the interactive session — a
//! decision without eyes on the item is a rubber stamp. Scripted forms
//! exist for single items (`sign <id>` preview, then exact root/time echo)
//! and detached artifact bytes (`sign <path>`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_edge::decision_brief::ReviewSnapshot;
use vela_edge::sign_queue::{SignItem, SignLane, SignQueueInput, sign_queue};
use vela_protocol::detached;

use colored::Colorize;
use vela_protocol::cli_style as style;

use crate::decision_plan::{
    DecisionAction, DecisionExecutionOutcome, DecisionPlanError, PreparedDecision,
    PreparedSignatureSet, SavedAnswer,
};
use crate::ui::{self, ErrorKind};

fn safe_inline(value: impl AsRef<str>) -> String {
    crate::cli::safe_text::inline(value.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionCommand {
    Quit,
    Skip,
    Accept,
    Reject,
    Yes,
    Invalid,
}

fn session_command(input: &str, lane: SignLane) -> SessionCommand {
    match (input, lane) {
        ("q", _) => SessionCommand::Quit,
        ("s", _) => SessionCommand::Skip,
        ("a", SignLane::Decision) => SessionCommand::Accept,
        ("r", SignLane::Decision) => SessionCommand::Reject,
        ("y", SignLane::Judgment | SignLane::Hygiene | SignLane::Detached) => SessionCommand::Yes,
        _ => SessionCommand::Invalid,
    }
}

/// A typed, resumable answer. Presentation roots invalidate the answer when
/// any decision-critical brief fact changes; they are never signing inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredDecisionAction {
    Accept,
    Reject,
}

impl From<StoredDecisionAction> for DecisionAction {
    fn from(action: StoredDecisionAction) -> Self {
        match action {
            StoredDecisionAction::Accept => Self::Accept,
            StoredDecisionAction::Reject => Self::Reject,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredAnswer {
    proposal_id: String,
    proposal_root: String,
    seen_decision_facts_root: String,
    action: StoredDecisionAction,
    reason: String,
}

impl StoredAnswer {
    fn from_item(item: &SignItem, action: StoredDecisionAction, reason: String) -> Option<Self> {
        let review = item.review.as_ref()?;
        Some(Self {
            proposal_id: review.brief.audit.proposal_id.clone(),
            proposal_root: review.decision_bindings.proposal_root.clone(),
            seen_decision_facts_root: review.brief.audit.decision_facts_root.clone(),
            action,
            reason,
        })
    }

    fn is_current_for(&self, item: &SignItem) -> bool {
        let Some(review) = &item.review else {
            return false;
        };
        self.proposal_id == item.id
            && self.proposal_id == review.brief.audit.proposal_id
            && self.proposal_root == review.decision_bindings.proposal_root
            && self.seen_decision_facts_root == review.brief.audit.decision_facts_root
    }

    fn to_saved_answer(&self) -> SavedAnswer {
        SavedAnswer {
            proposal_id: self.proposal_id.clone(),
            proposal_root: self.proposal_root.clone(),
            seen_decision_facts_root: self.seen_decision_facts_root.clone(),
            action: self.action.into(),
            reason: self.reason.clone(),
        }
    }
}

/// Saved-as-you-go session state: exact answers survive `q` and crashes.
/// Legacy string-only session files fail closed to an empty state.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SessionState {
    #[serde(default)]
    answers: std::collections::BTreeMap<String, StoredAnswer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_decision_root: Option<String>,
}

struct FrontierQueue {
    dir: PathBuf,
    items: Vec<SignItem>,
    review_total: usize,
    next_cursor: Option<String>,
    snapshot_root: String,
}

/// Shared bounded human rendering for diff, preview, and the ceremony.
pub(crate) fn render_decision_brief_lines(
    brief: &vela_edge::decision_brief::DecisionBrief,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "change    {} {} · {}",
        brief.change.subject.subject_type, brief.change.subject.id, brief.change.requested_action
    ));
    lines.push(format!(
        "base      {}",
        brief.change.fixed_base.event_log_root
    ));
    if let Some(before) = &brief.change.before {
        lines.push(format!("before    {}", before.text));
    }
    if let Some(after) = &brief.change.after {
        lines.push(format!("after     {}", after.text));
    }
    let evidence = brief
        .basis
        .primary_evidence_roots
        .iter()
        .map(|root| format!("{} {}", root.kind, root.root))
        .collect::<Vec<_>>()
        .join(" · ");
    lines.push(format!(
        "basis     {} · {}",
        brief.basis.check_state.gate_status, evidence
    ));
    if let Some(caveat) = &brief.basis.main_caveat {
        lines.push(format!("caveat    {caveat}"));
    }
    lines.push(format!(
        "impact    {} changed · {} downstream · tier {}",
        brief.impact.downstream_effect.changed_findings,
        brief.impact.downstream_effect.downstream_dependents,
        brief.impact.downstream_effect.impact_tier
    ));
    for warning in &brief.impact.critical_warnings {
        lines.push(format!(
            "warning   {}{}",
            warning.code,
            warning
                .reference
                .as_deref()
                .map(|reference| format!(" · {reference}"))
                .unwrap_or_default()
        ));
    }
    lines.push(format!(
        "authority {} · {}",
        brief.authority.route, brief.authority.scope
    ));
    for action in &brief.authority.actions {
        let reasons = if action.reasons.is_empty() {
            String::new()
        } else {
            format!(" · {}", action.reasons.join("; "))
        };
        lines.push(format!(
            "{} {:<7} {}{}",
            "action", action.action, action.eligibility, reasons
        ));
    }
    for (name, facet) in brief.facets.iter().filter(|(_, facet)| facet.critical) {
        lines.push(format!(
            "facet     {} · {} · {}",
            name,
            facet.full_root,
            serde_json::to_string(&facet.data).unwrap_or_else(|_| "unavailable".to_string())
        ));
    }
    if !brief.missing.is_empty() {
        lines.push(format!(
            "missing   {}",
            brief
                .missing
                .iter()
                .map(|fact| format!("{} ({})", fact.field, fact.reason))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    lines.push(format!(
        "audit     {} · facts {}",
        brief.audit.proposal_root, brief.audit.decision_facts_root
    ));
    lines.into_iter().map(safe_inline).collect()
}

/// The complete bounded material shown for one sign item both when it is
/// answered and again in the final summary. Re-rendering it immediately before
/// the sole confirmation makes resumed sessions reviewable and prevents an
/// accepted item from reaching the key through an id/title-only summary.
fn render_item_review_lines(item: &SignItem) -> Vec<String> {
    const PREVIEW_LINE_BUDGET: usize = 8;

    let mut lines = if let Some(review) = &item.review {
        render_decision_brief_lines(&review.brief)
    } else {
        let mut lines = Vec::new();
        lines.extend(
            item.preview
                .iter()
                .take(PREVIEW_LINE_BUDGET)
                .map(safe_inline),
        );
        lines
    };
    if item.preview.len() > PREVIEW_LINE_BUDGET {
        let mut hasher = Sha256::new();
        for line in &item.preview {
            hasher.update((line.len() as u64).to_be_bytes());
            hasher.update(line.as_bytes());
        }
        lines.push(format!(
            "… {} preview line(s) omitted; sha256:{}",
            item.preview.len() - PREVIEW_LINE_BUDGET,
            hex::encode(hasher.finalize())
        ));
    }
    lines.push(format!("why you: {}", safe_inline(&item.why_here)));
    lines.push(safe_inline(&item.id));
    lines.into_iter().map(safe_inline).collect()
}

fn action_available(item: &SignItem, action: &str) -> bool {
    item.review
        .as_ref()
        .and_then(|review| review.brief.action(action))
        .is_some_and(vela_edge::decision_brief::DecisionAction::is_available)
}

fn item_answerable(item: &SignItem) -> bool {
    match item.lane {
        SignLane::Decision => action_available(item, "accept") || action_available(item, "reject"),
        SignLane::Judgment | SignLane::Hygiene | SignLane::Detached => true,
    }
}

fn explain_blocked_action(item: &SignItem, action: &str) -> String {
    item.review
        .as_ref()
        .and_then(|review| review.brief.action(action))
        .map(|entry| entry.reasons.join("; "))
        .filter(|reasons| !reasons.is_empty())
        .unwrap_or_else(|| format!("{action} is unavailable for this item"))
}

fn reconcile_session(state: &mut SessionState, items: &[SignItem]) -> usize {
    let before = state.answers.len();
    state.answers.retain(|proposal_id, answer| {
        items
            .iter()
            .find(|item| item.id == *proposal_id)
            .is_some_and(|item| answer.is_current_for(item))
    });
    let removed = before - state.answers.len();
    if removed > 0 {
        state.latest_decision_root = None;
    }
    removed
}

pub(crate) fn saved_answer_from_review(
    review: &ReviewSnapshot,
    action: DecisionAction,
    reason: impl Into<String>,
) -> SavedAnswer {
    SavedAnswer {
        proposal_id: review.brief.audit.proposal_id.clone(),
        proposal_root: review.decision_bindings.proposal_root.clone(),
        seen_decision_facts_root: review.brief.audit.decision_facts_root.clone(),
        action,
        reason: reason.into(),
    }
}

fn action_label(action: DecisionAction) -> &'static str {
    match action {
        DecisionAction::Accept => "accept",
        DecisionAction::Reject => "reject",
    }
}

/// Stable semantic summary used by both human output and JSON callers. Git
/// publication controls are deliberately absent: they are operational state,
/// not scientific authorization.
pub(crate) fn final_decision_summary(
    reviews: &[&ReviewSnapshot],
    answers: &[SavedAnswer],
    prepared: &PreparedDecision,
) -> serde_json::Value {
    let items = answers
        .iter()
        .map(|answer| {
            let review = reviews
                .iter()
                .find(|review| review.brief.audit.proposal_id == answer.proposal_id)
                .expect("every planned answer has a rendered review");
            let warnings = review
                .brief
                .impact
                .critical_warnings
                .iter()
                .map(|warning| {
                    warning.reference.as_ref().map_or_else(
                        || warning.code.clone(),
                        |reference| format!("{} · {reference}", warning.code),
                    )
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "proposal_id": answer.proposal_id,
                "action": action_label(answer.action),
                "reason": answer.reason,
                "claim": review.brief.change.claim,
                "semantic_effect": {
                    "requested_action": review.brief.change.requested_action,
                    "root": review.decision_bindings.semantic_effect_root,
                },
                "critical_warnings": warnings,
                "decision_facts_root": review.brief.audit.decision_facts_root,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "decision_root": prepared.plan.decision_root,
        "expected_event_log_root": prepared.plan.expected_event_log_root,
        "items": items,
    })
}

pub(crate) fn render_final_decision_set(
    reviews: &[&ReviewSnapshot],
    answers: &[SavedAnswer],
    prepared: &PreparedDecision,
) {
    println!();
    println!("  {}", style::signal("FINAL DECISION SET"));
    for answer in answers {
        let review = reviews
            .iter()
            .find(|review| review.brief.audit.proposal_id == answer.proposal_id)
            .expect("every planned answer has a rendered review");
        println!();
        println!(
            "  {}  {}",
            style::moss(action_label(answer.action)),
            safe_inline(&answer.proposal_id)
        );
        println!("    reason    {}", safe_inline(&answer.reason));
        println!("    claim     {}", safe_inline(&review.brief.change.claim));
        println!(
            "    effect    {} · {}",
            safe_inline(&review.brief.change.requested_action),
            safe_inline(&review.decision_bindings.semantic_effect_root)
        );
        if review.brief.impact.critical_warnings.is_empty() {
            println!("    warning   none");
        } else {
            for warning in &review.brief.impact.critical_warnings {
                println!(
                    "    warning   {}{}",
                    safe_inline(&warning.code),
                    warning
                        .reference
                        .as_deref()
                        .map(|reference| format!(" · {}", safe_inline(reference)))
                        .unwrap_or_default()
                );
            }
        }
        println!(
            "    facts     {}",
            safe_inline(&review.brief.audit.decision_facts_root)
        );
    }
    println!();
    println!(
        "  transaction  {}",
        safe_inline(&prepared.plan.decision_root)
    );
    println!(
        "  fixed base   {}",
        safe_inline(&prepared.plan.expected_event_log_root)
    );
}

pub(crate) fn execute_confirmed_decision(
    frontier: &Path,
    prepared: &PreparedDecision,
    key: Option<&Path>,
) -> Result<DecisionExecutionOutcome, DecisionPlanError> {
    crate::decision_plan::execute_with_key_loader(frontier, prepared, || {
        crate::cli_identity::resolve_signing_key_opt(key).ok_or_else(|| {
            "no human signing key is configured; run `vela id create` or pass --key".to_string()
        })
    })
}

fn execute_confirmed_protected_decision(
    frontier: &Path,
    prepared: &PreparedDecision,
    review: &ReviewSnapshot,
    action: &str,
    proposal_id: &str,
    proposal_root: &str,
    reason: &str,
    observed_at: &str,
    gate_state: &str,
) -> Result<DecisionExecutionOutcome, DecisionPlanError> {
    let profile = crate::cli_identity::protected_signer_profile()
        .map_err(|error| DecisionPlanError::new_external("key_unavailable", error))?;
    crate::decision_plan::execute_with_signature_loader(frontier, prepared, |material| {
        request_helper_signatures(
            frontier,
            material,
            review,
            action,
            proposal_id,
            proposal_root,
            reason,
            observed_at,
            gate_state,
            &profile,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn request_helper_signatures(
    frontier: &Path,
    material: &PreparedSignatureSet,
    review: &ReviewSnapshot,
    action: &str,
    proposal_id: &str,
    proposal_root: &str,
    reason: &str,
    observed_at: &str,
    gate_state: &str,
    profile: &crate::cli_identity::ProtectedSignerProfile,
) -> Result<Vec<String>, String> {
    use rand::RngCore;

    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    if helper_sha256 != profile.helper_sha256 {
        return Err(format!(
            "installed signer helper {} does not match the protected identity pin {}; run `vela id protect --user-presence --remove-source-key` to authorize this exact helper update",
            helper_sha256, profile.helper_sha256
        ));
    }
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::SignerRequest {
        schema: vela_signer::contract::REQUEST_SCHEMA.to_string(),
        nonce: hex::encode(nonce),
        expires_at: (now + chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: vela_signer::contract::file_sha256(&vela_binary)?,
        helper_sha256,
        frontier_id: material.frontier_id.clone(),
        frontier_path: std::fs::canonicalize(frontier)
            .unwrap_or_else(|_| frontier.to_path_buf())
            .display()
            .to_string(),
        proposal_id: proposal_id.to_string(),
        proposal_root: proposal_root.to_string(),
        action: action.to_string(),
        reason: reason.to_string(),
        reviewer_actor: material.reviewer_id.clone(),
        reviewer_public_key: material.reviewer_public_key.clone(),
        observed_at: observed_at.to_string(),
        decision_plan_root: material.decision_root.clone(),
        gate_state: gate_state.to_string(),
        provider: profile.provider.clone(),
        protection_grade: profile.protection_grade.clone(),
        protection_mode: profile.mode,
        display: protected_decision_display(review, action),
        events: material
            .events
            .iter()
            .cloned()
            .map(|event| vela_signer::SignerEvent { event })
            .collect(),
    };
    vela_signer::validate_request(&request, now)?;
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("serialize signer request: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("approve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start pinned signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write signer request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for signer helper: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "signer helper declined or failed: {}",
            safe_inline(detail.trim())
        ));
    }
    let response: vela_signer::SignerResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse closed signer response: {error}"))?;
    vela_signer::validate_response(&request, &response)?;
    Ok(response
        .signatures
        .into_iter()
        .map(|signed| signed.signature)
        .collect())
}

fn protected_decision_display(review: &ReviewSnapshot, action: &str) -> vela_signer::SignerDisplay {
    let brief = &review.brief;
    let mut facts = Vec::new();
    if action == "reject"
        && !brief.accept_ready()
        && let Some(accept) = brief.action("accept")
    {
        facts.extend(accept.reasons.iter().map(|reason| humanize_fact(reason)));
    }
    if facts.is_empty() {
        facts.extend(
            brief
                .basis
                .check_state
                .gate_reasons
                .iter()
                .map(|reason| humanize_fact(reason)),
        );
    }
    if brief.basis.check_state.durable_verifier_count == 0 {
        facts.push("No independent verifier attachments".to_string());
    }
    for warning in &brief.impact.critical_warnings {
        facts.push(humanize_fact(&warning.code));
    }
    facts.sort();
    facts.dedup();
    facts.truncate(4);
    if facts.is_empty() {
        facts.push("This proposal requires a human scientific judgment".to_string());
    }

    let consequence = if action == "accept" {
        let state = brief
            .change
            .after
            .as_ref()
            .map(|after| after.text.as_str())
            .unwrap_or(brief.change.requested_action.as_str());
        format!(
            "Change accepted state to {state}. {} finding(s) change; {} downstream dependent(s) are recorded.",
            brief.impact.downstream_effect.changed_findings,
            brief.impact.downstream_effect.downstream_dependents,
        )
    } else {
        "Keep accepted scientific state unchanged and close this proposal as rejected.".to_string()
    };

    vela_signer::SignerDisplay {
        frontier_name: brief.authority.frontier.name.clone(),
        claim: brief.change.claim.clone(),
        requester: review.proposal_actor.clone(),
        decisive_facts: facts,
        consequence,
    }
}

fn humanize_fact(value: &str) -> String {
    let text = value.trim().trim_end_matches('.').replace(['_', '-'], " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Decision evidence is incomplete".to_string(),
    }
}

pub(crate) fn fail_decision(error: DecisionPlanError) -> ! {
    let kind = match error.code {
        "key_unavailable" | "key_mismatch" | "reviewer_unauthorized" | "signing_failed" => {
            ErrorKind::Custody
        }
        _ => ErrorKind::Domain,
    };
    ui::fail_with(kind, &error.to_string(), None)
}

/// Publish only the immutable public delta committed by `FrontierTxn`.
/// Publication is an operational follow-up: every failure is reported after
/// the scientific decision remains durably installed, and never triggers a
/// second key read or a broad worktree sweep.
pub(crate) fn publish_exact_decision(
    frontier: &Path,
    summary: &str,
    outcome: &DecisionExecutionOutcome,
    opts: &crate::config::git_publish::PublishOptions,
) -> crate::config::git_publish::PublicationOutcome {
    use crate::config::git_publish::{PublicationOutcome, PublicationState};
    let Some(delta) = &outcome.publication_delta else {
        return PublicationOutcome {
            state: PublicationState::Uncommitted {
                candidate: None,
                reason: "frontier transaction had no public Git delta".to_string(),
            },
            recovery_command: Some("git status --short".to_string()),
        };
    };
    match crate::config::git_publish::exact_publication_resume_preflight(frontier, delta, opts) {
        Ok(preflight) => match crate::config::git_publish::publish_exact_delta(
            frontier,
            summary,
            &outcome.event_ids,
            delta,
            preflight,
            opts,
        ) {
            Ok(publication) => publication,
            Err(error) => PublicationOutcome {
                state: PublicationState::Unknown {
                    reason: error.to_string(),
                },
                recovery_command: Some("git status --short".to_string()),
            },
        },
        Err(publication) => publication,
    }
}

fn publication_recovery_command(
    publication: &crate::config::git_publish::PublicationOutcome,
) -> &str {
    publication
        .recovery_command
        .as_deref()
        .unwrap_or("vela status --json")
}

fn session_path(frontier: &Path) -> PathBuf {
    frontier.join(".vela").join("sign-session.json")
}

fn load_session(frontier: &Path) -> SessionState {
    std::fs::read_to_string(session_path(frontier))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_session(frontier: &Path, state: &SessionState) {
    if let Ok(body) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(session_path(frontier), format!("{body}\n"));
    }
}

fn clear_session(frontier: &Path) {
    let _ = std::fs::remove_file(session_path(frontier));
}

/// Discover candidate frontiers. The read-only preview may page several;
/// the mutating ceremony requires this to resolve to exactly one.
fn session_frontiers(explicit: Option<PathBuf>) -> Vec<PathBuf> {
    if let Some(f) = explicit {
        return vec![f];
    }
    // Inside a frontier? Just that one.
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur = cwd;
        loop {
            if cur.join(".vela").is_dir() {
                return vec![cur];
            }
            if !cur.pop() {
                break;
            }
        }
    }
    crate::config::workspace_registry::live_frontiers()
}

fn read_line(prompt: &str) -> String {
    crate::cli::prompt::read_line(prompt)
}

/// Read-only convenience over the same page and bytes used by ordinary sign.
/// This path never resolves an actor, opens a key, or creates session state.
pub(crate) fn cmd_sign_preview(
    frontier: Option<PathBuf>,
    cursor: Option<String>,
    limit: Option<usize>,
    json: bool,
) {
    if limit.is_some_and(|limit| !(1..=crate::review_material::REVIEW_PAGE_MAX).contains(&limit)) {
        ui::fail_with(
            ErrorKind::Usage,
            "sign preview limit must be between 1 and 100",
            None,
        );
    }
    let frontiers = session_frontiers(frontier);
    if frontiers.is_empty() {
        ui::fail_with(
            ErrorKind::NotFound,
            "no frontier here and none registered",
            Some("run inside a frontier, or pass --frontier"),
        );
    }
    if cursor.is_some() && frontiers.len() != 1 {
        ui::fail_with(
            ErrorKind::Usage,
            "a review cursor is scoped to exactly one frontier",
            Some("pass --frontier with the cursor returned for that frontier"),
        );
    }
    let mut pages = Vec::new();
    for dir in frontiers {
        let page = crate::review_material::ReviewProjection::page(
            &dir,
            crate::review_material::ReviewRequest {
                limit,
                cursor: cursor.clone(),
                proposal_id: None,
            },
        )
        .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error.to_string(), None));
        pages.push((dir, page));
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "command": "sign.preview",
                "frontiers": pages.iter().map(|(dir, page)| serde_json::json!({
                    "frontier": dir.display().to_string(),
                    "snapshot_root": page.snapshot_root,
                    "event_log_root": page.event_log_root,
                    "observed_at": page.observed_at,
                    "total": page.total,
                    "returned": page.returned,
                    "pressure": page.pressure,
                    "items": page.items,
                    "next_cursor": page.next_cursor,
                })).collect::<Vec<_>>(),
            }))
            .expect("serialize sign preview")
        );
        return;
    }
    for (dir, page) in pages {
        ui::header(
            "SIGN PREVIEW",
            &safe_inline(dir.display().to_string()),
            Some(&format!("{} of {} pending", page.returned, page.total)),
        );
        println!(
            "  review pressure: {}",
            crate::review_material::review_pressure_summary(&page.pressure)
        );
        for item in &page.items {
            println!();
            println!("  {}", safe_inline(&item.brief.change.claim).bold());
            for line in render_decision_brief_lines(&item.brief) {
                println!("    {}", style::dim(&line));
            }
        }
        if page.next_cursor.is_some() {
            println!();
            println!(
                "  · more pending; use `vela sign --preview --json` to continue with the opaque cursor"
            );
        }
        println!();
    }
}

/// The interactive session (the default form of `vela sign`).
pub(crate) fn cmd_sign_session(
    frontier: Option<PathBuf>,
    key: Option<PathBuf>,
    json: bool,
    reset: bool,
) {
    let frontiers = session_frontiers(frontier);
    if frontiers.is_empty() {
        ui::fail_with(
            ErrorKind::NotFound,
            "no frontier here and none registered",
            Some("run inside a frontier, or `vela init` one (init registers it)"),
        );
    }
    if frontiers.len() != 1 {
        ui::fail_with(
            ErrorKind::Usage,
            "a decision ceremony is scoped to exactly one frontier",
            Some(
                "run inside the frontier or pass `--frontier <path>`; each frontier commits independently",
            ),
        );
    }

    // `--reset`: discard saved verdicts and stop, so the next run starts
    // clean. The escape hatch for a resumed session showing choices you
    // want to redo (clig.dev: recover from interruption; never trap).
    if reset {
        let mut cleared = 0usize;
        for dir in &frontiers {
            if session_path(dir).exists() {
                clear_session(dir);
                cleared += 1;
            }
        }
        println!("  cleared {cleared} saved session(s); re-run `vela sign` to decide fresh.");
        return;
    }

    // Build the one frontier's bounded decision queue up front. Detached
    // bytes, fidelity attestations, and re-sign hygiene keep their own
    // ceremonies so none can move key access ahead of locked rederivation.
    let mut queues = Vec::<FrontierQueue>::new();
    let mut total = 0usize;
    for dir in &frontiers {
        let page = match crate::review_material::ReviewProjection::page(
            dir,
            crate::review_material::ReviewRequest::default(),
        ) {
            Ok(page) => page,
            Err(error) => {
                eprintln!(
                    "  skipping {}: {}",
                    safe_inline(dir.display().to_string()),
                    safe_inline(error.to_string())
                );
                continue;
            }
        };
        let review_total = page.total;
        let next_cursor = page.next_cursor.clone();
        let snapshot_root = page.snapshot_root.clone();
        let queue = sign_queue(SignQueueInput {
            judgments: Vec::new(),
            decisions: page.items,
            hygiene: Vec::new(),
            detached: Vec::new(),
        });
        total += queue
            .items
            .iter()
            .filter(|item| item_answerable(item))
            .count();
        queues.push(FrontierQueue {
            dir: dir.clone(),
            items: queue.items,
            review_total,
            next_cursor,
            snapshot_root,
        });
    }

    if json {
        // `sign --list --json` shape: the queue, no session.
        let out = serde_json::json!({
            "ok": true,
            "command": "sign",
            "frontiers": queues.iter().map(|queue| serde_json::json!({
                "frontier": queue.dir.display().to_string(),
                "review_snapshot_root": queue.snapshot_root,
                "pending_total": queue.review_total,
                "next_cursor": queue.next_cursor,
                "items": queue.items,
            })).collect::<Vec<_>>(),
            "signable_total": total,
            "answerable_total": total,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }

    // The ceremony begins HERE — the --json list above is a plain read
    // (agents and the plugin's status render depend on it). Custody
    // gate, then the clear-signing binary check, before anything
    // renders or prompts.
    let actor = crate::cli_identity::resolve_decision_actor(None);
    ceremony_binary_gate(true);

    let it = if total == 1 {
        "item awaits"
    } else {
        "items await"
    };
    ui::header(
        "SIGN",
        "1 frontier",
        Some(&format!("{total} {it} your key")),
    );
    if total == 0 {
        println!("  · nothing awaits you — the policy lane is doing its job");
        return;
    }
    for queue in &queues {
        let shown = queue
            .items
            .iter()
            .filter(|item| item.lane == SignLane::Decision)
            .count();
        if queue.next_cursor.is_some() {
            println!(
                "  · {} shows {shown} of {} pending decisions; finish this page, then rerun `vela sign`",
                safe_inline(queue.dir.display().to_string()),
                queue.review_total,
            );
        }
    }

    // The session prompts per item; refuse cleanly on piped/CI stdin
    // rather than spin the read loop on empty reads. Scripted decisions go
    // through `sign <id>`'s preview/root handshake; fidelity batches and detached signatures
    // have separate dispatch arms and never reach here.
    ui::ensure_can_prompt(
        "the sign session",
        "preview one with `sign <vpr_id>`, then echo its --confirm-root and --confirm-at values",
    );

    // The judgment loop is resume-safe. A saved answer is reused only while
    // both its proposal root and its seen decision-facts root still match.
    let total_signable = total;
    let mut position = 0usize;
    for queue in &queues {
        let mut state = load_session(&queue.dir);
        let invalidated = reconcile_session(&mut state, &queue.items);
        if invalidated > 0 {
            println!(
                "  · {invalidated} saved answer(s) invalidated by changed review facts; review them again"
            );
            save_session(&queue.dir, &state);
        }
        for item in &queue.items {
            if !item_answerable(item) || state.answers.contains_key(&item.id) {
                continue;
            }
            position += 1;
            let lane_chip = match item.lane {
                SignLane::Judgment => style::brass("judgment"),
                SignLane::Decision => style::signal("decision"),
                SignLane::Hygiene => style::moss("hygiene"),
                SignLane::Detached => style::moss("governance"),
            };
            println!();
            println!(
                "  {}  {}",
                style::dim(&format!("{position}/{total_signable}")),
                lane_chip,
            );
            // The CLAIM is the headline — bold, full width, the thing
            // being judged.
            println!("  {}", safe_inline(&item.title).bold());
            for line in render_item_review_lines(item) {
                println!("    {}", style::dim(&line));
            }
            let keys = match item.lane {
                SignLane::Decision => "a/r/s/q",
                _ => "y/s/q",
            };
            loop {
                let legend = match item.lane {
                    SignLane::Decision => format!(
                        "{}ccept · {}eject · {}kip · {}uit",
                        style::moss("a"),
                        style::madder("r"),
                        style::dim("s"),
                        style::dim("q")
                    ),
                    _ => format!(
                        "{}es · {}kip · {}uit",
                        style::moss("y"),
                        style::dim("s"),
                        style::dim("q")
                    ),
                };
                let ans = read_line(&format!("  {legend}  > "));
                match session_command(&ans, item.lane) {
                    SessionCommand::Quit => {
                        save_session(&queue.dir, &state);
                        println!("  saved; re-run `vela sign` to resume.");
                        return;
                    }
                    SessionCommand::Skip => break,
                    SessionCommand::Accept => {
                        if action_available(item, "accept") {
                            let answer = StoredAnswer::from_item(
                                item,
                                StoredDecisionAction::Accept,
                                "accepted via sign session".to_string(),
                            )
                            .expect("decision queue item has a review snapshot");
                            state.answers.insert(item.id.clone(), answer);
                            state.latest_decision_root = None;
                            break;
                        }
                        println!(
                            "    {}",
                            safe_inline(explain_blocked_action(item, "accept"))
                        );
                    }
                    SessionCommand::Reject => {
                        if !action_available(item, "reject") {
                            println!(
                                "    {}",
                                safe_inline(explain_blocked_action(item, "reject"))
                            );
                            continue;
                        }
                        let why = loop {
                            let why = read_line("  reject reason: ");
                            if !why.trim().is_empty() {
                                break why;
                            }
                            println!("    a reject requires a non-empty reason");
                        };
                        let answer =
                            StoredAnswer::from_item(item, StoredDecisionAction::Reject, why)
                                .expect("decision queue item has a review snapshot");
                        state.answers.insert(item.id.clone(), answer);
                        state.latest_decision_root = None;
                        break;
                    }
                    SessionCommand::Yes => unreachable!("proposal queue has no yes-only lane"),
                    SessionCommand::Invalid => println!("    keys: {keys}"),
                }
            }
            save_session(&queue.dir, &state);
        }
    }

    let queue = &queues[0];
    let decided_at = queue
        .items
        .iter()
        .find_map(|item| {
            item.review
                .as_ref()
                .map(|review| review.observed_at.clone())
        })
        .expect("non-empty decision queue has an observation time");
    let provenance = crate::cli_identity::resolve_co_author_provenance(None, None);

    // Build and render a complete semantic set before the sole confirmation.
    // Edits rebuild the plan; stale roots erase saved answers before any key
    // loader can be reached.
    let confirmed = loop {
        let mut state = load_session(&queue.dir);
        if reconcile_session(&mut state, &queue.items) > 0 {
            save_session(&queue.dir, &state);
            println!("  review facts changed; invalidated answers must be reviewed again.");
            return;
        }

        let mut removed_blocked = false;
        for item in &queue.items {
            let Some(answer) = state.answers.get(&item.id) else {
                continue;
            };
            let action = match answer.action {
                StoredDecisionAction::Accept => "accept",
                StoredDecisionAction::Reject => "reject",
            };
            if !action_available(item, action) {
                println!(
                    "  {}  {}  {}",
                    style::madder("blocked"),
                    safe_inline(&item.id),
                    safe_inline(explain_blocked_action(item, action))
                );
                state.answers.remove(&item.id);
                state.latest_decision_root = None;
                removed_blocked = true;
                break;
            }
        }
        if removed_blocked {
            save_session(&queue.dir, &state);
            continue;
        }

        let answers = queue
            .items
            .iter()
            .filter_map(|item| {
                state
                    .answers
                    .get(&item.id)
                    .map(StoredAnswer::to_saved_answer)
            })
            .collect::<Vec<_>>();
        if answers.is_empty() {
            println!("  nothing answered; nothing to sign.");
            return;
        }
        let reviews = queue
            .items
            .iter()
            .filter_map(|item| item.review.as_ref())
            .collect::<Vec<_>>();
        for item in queue
            .items
            .iter()
            .filter(|item| !state.answers.contains_key(&item.id))
        {
            println!(
                "  {}  {} retained pending",
                style::dim("skip"),
                safe_inline(&item.id)
            );
        }

        let prepared = match crate::decision_plan::build_unlocked(
            &queue.dir,
            &answers,
            &actor,
            &decided_at,
            provenance.clone(),
        ) {
            Ok(prepared) => Some(prepared),
            Err(error)
                if matches!(
                    error.code,
                    "decision_stale" | "answer_invalidated" | "action_unavailable"
                ) =>
            {
                state.answers.clear();
                state.latest_decision_root = None;
                save_session(&queue.dir, &state);
                println!(
                    "  {} review changed before confirmation: {}",
                    style::warn("stale"),
                    safe_inline(error.to_string())
                );
                println!("  no key was read; re-run `vela sign` to review the current facts.");
                return;
            }
            Err(error) => {
                println!(
                    "  {} cannot form one coherent decision transaction: {}",
                    style::warn("plan blocked"),
                    safe_inline(error.to_string())
                );
                None
            }
        };
        if let Some(prepared) = &prepared {
            state.latest_decision_root = Some(prepared.plan.decision_root.clone());
            save_session(&queue.dir, &state);
            render_final_decision_set(&reviews, &answers, prepared);
        }

        let choice = if prepared.is_some() {
            read_line(&format!(
                "\nSign {} decision(s) as {}?  [{}es · {}dit one · {}eset all · {}o] > ",
                answers.len(),
                safe_inline(&actor),
                style::moss("y"),
                style::brass("e"),
                style::madder("r"),
                style::dim("n"),
            ))
        } else {
            read_line(&format!(
                "\nChoose a coherent subset: [{}dit one · {}eset all · {}o] > ",
                style::brass("e"),
                style::madder("r"),
                style::dim("n"),
            ))
        };
        match choice.as_str() {
            "y" if prepared.is_some() => break prepared.expect("checked above"),
            "e" => {
                let id = read_line("  item id (or a prefix) to change: ");
                let Some(item) = queue
                    .items
                    .iter()
                    .find(|item| item.id == id || item.id.starts_with(&id))
                else {
                    println!("  no item matches `{}`", safe_inline(id));
                    continue;
                };
                let verdict =
                    read_line("  new verdict — [a]ccept · [r]eject · [s]kip (leave pending): ");
                match verdict.as_str() {
                    "a" if action_available(item, "accept") => {
                        let answer = StoredAnswer::from_item(
                            item,
                            StoredDecisionAction::Accept,
                            "accepted via sign session".to_string(),
                        )
                        .expect("decision queue item has a review snapshot");
                        state.answers.insert(item.id.clone(), answer);
                    }
                    "r" if action_available(item, "reject") => {
                        let reason = read_line("  reject reason: ");
                        if reason.trim().is_empty() {
                            println!("  unchanged (a reject requires a non-empty reason)");
                            continue;
                        }
                        let answer =
                            StoredAnswer::from_item(item, StoredDecisionAction::Reject, reason)
                                .expect("decision queue item has a review snapshot");
                        state.answers.insert(item.id.clone(), answer);
                    }
                    "s" => {
                        state.answers.remove(&item.id);
                    }
                    "a" | "r" => {
                        println!(
                            "  unchanged ({})",
                            safe_inline(explain_blocked_action(item, &verdict))
                        );
                        continue;
                    }
                    _ => {
                        println!("  unchanged (need a, r, or s)");
                        continue;
                    }
                }
                state.latest_decision_root = None;
                save_session(&queue.dir, &state);
            }
            "r" => {
                clear_session(&queue.dir);
                println!("  reset — all verdicts cleared. Re-run `vela sign` to decide fresh.");
                return;
            }
            _ => {
                println!(
                    "  not signed; answers saved. Re-run `vela sign` to finish, `e` to edit, or `vela sign --reset` to start over."
                );
                return;
            }
        }
    };

    let apply_spin = crate::cli::progress::Spinner::start("applying exact decision set");
    let outcome = execute_confirmed_decision(&queue.dir, &confirmed, key.as_deref())
        .unwrap_or_else(|error| fail_decision(error));
    clear_session(&queue.dir);
    let publish_opts = crate::config::git_publish::PublishOptions::new(false);
    let publication = publish_exact_decision(&queue.dir, "sign", &outcome, &publish_opts);
    apply_spin.finish("applied");
    println!(
        "\n  · signed {} event(s) · decision {}",
        outcome.event_ids.len(),
        safe_inline(&outcome.decision_root)
    );
    println!(
        "  · publication {} · retained {}",
        safe_inline(serde_json::to_string(&publication).unwrap_or_else(|_| "unknown".to_string())),
        safe_inline(&outcome.operation_id)
    );
    println!(
        "  · next {}",
        safe_inline(publication_recovery_command(&publication))
    );
}

/// The clear-signing binary gate every ceremony runs first.
///
/// `interactive` = a human is at the keyboard (the default `vela sign`
/// session). When it is, a changed binary does not abort to a separate
/// command: the ceremony renders old -> new and offers a one-key inline
/// re-pin, folding what used to be `vela id pin-binary` into the flow. A
/// scripted ceremony (`--yes`/`--batch`/`<path>`, `interactive = false`)
/// cannot vouch for a new binary, so a mismatch stays fatal there.
pub(crate) fn ceremony_binary_gate(interactive_form: bool) {
    use crate::config::binary_pin::{self, PinState};
    use std::io::IsTerminal;
    use vela_protocol::cli_style::dim;

    // Prompt ONLY when a real human is at a terminal. An interactive-FORM
    // ceremony with a piped or CI stdin still behaves non-interactively, so a
    // changed binary refuses (never silently re-pins) and an unpinned binary is
    // only noted — the classic clear-signing behavior the scripted paths rely on.
    let interactive = interactive_form && std::io::stdin().is_terminal();
    if !interactive {
        match binary_pin::verify_for_ceremony() {
            Ok(Some(_)) => {}
            Ok(None) => eprintln!(
                "  {}",
                dim("unpinned binary — run `vela sign` interactively once to anchor it")
            ),
            Err(e) => ui::fail_with(ErrorKind::Custody, &e, None),
        }
        return;
    }

    let state = match binary_pin::pin_state() {
        Ok(s) => s,
        Err(e) => ui::fail_with(ErrorKind::Custody, &e, None),
    };
    match state {
        PinState::Match(_) => {}
        PinState::Unpinned => {
            // First run: offer to anchor now, so pinning never needs a separate
            // trip to `vela id pin-binary`.
            let ans =
                read_line("  no binary pin yet. pin this binary as your ceremony anchor? [Y/n] > ");
            if ans.is_empty() || ans.eq_ignore_ascii_case("y") {
                record_pin_or_warn();
            } else {
                eprintln!("  {}", dim("continuing unpinned."));
            }
        }
        PinState::Mismatch {
            pinned,
            current_sha,
            current_version,
            current_path,
        } => {
            let render = format!(
                "the vela binary changed since you pinned it:\n    \
                 pinned  {}  (v{}, {})\n    now     {}  (v{})  {}",
                &pinned.sha256[..16],
                safe_inline(&pinned.version),
                safe_inline(&pinned.pinned_at[..pinned.pinned_at.len().min(10)]),
                &current_sha[..16],
                safe_inline(current_version),
                safe_inline(current_path.display().to_string())
            );
            eprintln!("  {}", vela_protocol::cli_style::warn("binary changed"));
            for line in render.lines() {
                eprintln!("  {}", safe_inline(line));
            }
            let ans = read_line(
                "  re-pin this binary and continue signing? [y/N]  (only if you upgraded it) > ",
            );
            if ans.eq_ignore_ascii_case("y") {
                record_pin_or_warn();
            } else {
                ui::fail_with(
                    ErrorKind::Custody,
                    "not re-pinned — ceremony stopped. Inspect the binary if you did not upgrade it.",
                    None,
                );
            }
        }
    }
}

/// Record the pin, surfacing the dev-build footgun: pinning a `target/…`
/// binary (what the `vela` -> `scripts/vela` wrapper resolves to) anchors a
/// hash that changes on the next `cargo build`. The pin still records — the
/// human asked — but the warning tells them to pin their installed release.
fn record_pin_or_warn() {
    use crate::config::binary_pin;
    match binary_pin::record_pin() {
        Ok(pin) => {
            println!(
                "  · pinned {} (v{}) — ceremonies verify the binary first",
                &pin.sha256[..16],
                safe_inline(pin.version)
            );
            if binary_pin::is_dev_build_path(std::path::Path::new(&pin.binary_path)) {
                eprintln!(
                    "  {}",
                    vela_protocol::cli_style::warn(
                        "note: this is a build-tree binary; its hash changes on every \
                         `cargo build`. Pin your installed release (e.g. ~/.cargo/bin/vela) instead."
                    )
                );
            }
        }
        Err(e) => ui::fail_with(ErrorKind::Custody, &e, None),
    }
}

/// `vela review decide` is the ordinary one-proposal authority path. Its
/// first phase is entirely read-only. Its second phase accepts only the exact
/// root/time pair emitted by that preview and loads the human key exclusively
/// through the user-presence protected signer.
pub(crate) fn cmd_review_decide(
    frontier: PathBuf,
    id: &str,
    action: DecisionAction,
    reason: String,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json: bool,
) {
    if reason.trim().is_empty() {
        ui::fail_with(ErrorKind::Usage, "--reason must not be empty", None);
    }
    match crate::repository_decision::is_repository_authority_frontier(&frontier) {
        Ok(true) => {
            if confirm_root.is_some() || confirm_at.is_some() {
                ui::fail_with(
                    ErrorKind::Usage,
                    "repository-authority decisions do not accept copied confirmation roots or timestamps",
                    Some(
                        "rerun the same exact proposal, action, and reason without --confirm-root or --confirm-at",
                    ),
                );
            }
            let observed_at =
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            let prepared = match action {
                DecisionAction::Accept => {
                    crate::repository_decision::prepare_accept(&frontier, id, &reason, &observed_at)
                }
                DecisionAction::Reject => {
                    crate::repository_decision::prepare_reject(&frontier, id, &reason, &observed_at)
                }
            }
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error, None));
            if !json {
                println!(
                    "review decision · {} · {}",
                    action.as_str(),
                    safe_inline(&prepared.plan.proposal_id)
                );
                println!("  frontier: {}", safe_inline(&prepared.plan.frontier_name));
                println!(
                    "  claim: {}",
                    safe_inline(&prepared.review.brief.change.claim)
                );
                println!("  reason: {}", safe_inline(&prepared.plan.reason));
                println!("  authority: platform user presence → repository authority");
                println!(
                    "  scientific state change: {}",
                    if action == DecisionAction::Accept {
                        "exact accepted transition"
                    } else {
                        "none"
                    }
                );
                println!("  requesting one exact operating-system approval");
            }
            let result = match action {
                DecisionAction::Accept => {
                    crate::repository_decision::execute_accept(&frontier, &prepared.plan)
                }
                DecisionAction::Reject => {
                    crate::repository_decision::execute_reject(&frontier, &prepared.plan)
                }
            }
            .unwrap_or_else(|error| ui::fail_with(ErrorKind::Custody, &error, None));
            let payload = serde_json::json!({
                "ok": true,
                "command": "review.decide",
                "schema": "vela.review-decision.v2",
                "frontier": frontier.display().to_string(),
                "proposal_id": prepared.plan.proposal_id,
                "proposal_root": prepared.plan.proposal_root,
                "principal_id": prepared.plan.principal_id,
                "action": action.as_str(),
                "reason": prepared.plan.reason,
                "decision_plan_root": prepared.plan.plan_root,
                "event_ids": result.event_ids,
                "authority_record_id": result.authority_record_id,
                "authority_record_root": result.authority_record_root,
                "before_event_log_root": result.before_event_log_root,
                "after_event_log_root": result.after_event_log_root,
                "scientific_state_changed": action == DecisionAction::Accept,
                "authentication": "platform_user_presence",
                "transaction_signer": "repository_authority",
                "human_key_read": false,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("repository decision result JSON")
                );
            } else {
                println!("  · {} {}", safe_inline(action.as_str()), safe_inline(id));
                println!(
                    "  · authority record {}",
                    safe_inline(
                        payload
                            .get("authority_record_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown")
                    )
                );
                println!("  · inspect and commit the exact canonical delta");
            }
            return;
        }
        Ok(false) => {}
        Err(error) => ui::fail_with(ErrorKind::Domain, &error, None),
    }
    if confirm_root.is_some() != confirm_at.is_some() {
        ui::fail_with(
            ErrorKind::Usage,
            "protected confirmation requires both --confirm-root and --confirm-at from the same preview",
            Some("rerun without either flag to render a fresh key-free preview"),
        );
    }
    if let Some(observed_at) = confirm_at {
        crate::decision_plan::validate_scripted_confirmation_time(observed_at)
            .unwrap_or_else(|error| fail_decision(error));
    }

    let actor = crate::cli_identity::resolve_decision_actor(None);
    let review = match confirm_at {
        Some(observed_at) => {
            crate::review_material::ReviewProjection::one_at(&frontier, id, observed_at)
        }
        None => crate::review_material::ReviewProjection::one_read_only(&frontier, id),
    }
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error.to_string(), None));
    let (action_name, available) = match action {
        DecisionAction::Accept => ("accept", review.brief.accept_ready()),
        DecisionAction::Reject => ("reject", review.brief.reject_ready()),
    };
    if !available {
        let explanation = review
            .brief
            .action(action_name)
            .map(|entry| entry.reasons.join("; "))
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| format!("{action_name} is unavailable for this proposal"));
        ui::fail_with(ErrorKind::Domain, &explanation, None);
    }

    let answer = saved_answer_from_review(&review, action, reason);
    let provenance = crate::cli_identity::resolve_co_author_provenance(None, None);
    let prepared = crate::decision_plan::build_read_only_preview(
        &frontier,
        std::slice::from_ref(&answer),
        &actor,
        &review.observed_at,
        provenance,
    )
    .unwrap_or_else(|error| fail_decision(error));
    let summary = final_decision_summary(&[&review], std::slice::from_ref(&answer), &prepared);

    let Some(confirmed_root) = confirm_root else {
        let payload = serde_json::json!({
            "ok": true,
            "command": "review.decide",
            "schema": "vela.review-decision.v1",
            "frontier": frontier.display().to_string(),
            "proposal_id": id,
            "reviewer": actor,
            "action": action_name,
            "reason": answer.reason,
            "decision_brief": review.brief,
            "decision": summary,
            "confirm_root": prepared.plan.decision_root,
            "confirm_at": review.observed_at,
            "signed": false,
            "key_read": false,
        });
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&payload).expect("decision preview JSON")
            );
        } else {
            render_final_decision_set(&[&review], std::slice::from_ref(&answer), &prepared);
            println!("  · preview only; no key was read and nothing changed");
            println!(
                "  · approve this exact {action_name} with --confirm-root {} --confirm-at {}",
                safe_inline(&prepared.plan.decision_root),
                safe_inline(&review.observed_at),
            );
        }
        return;
    };
    if confirmed_root != prepared.plan.decision_root {
        ui::fail_with(
            ErrorKind::Domain,
            &format!(
                "confirmed decision root {confirmed_root} does not match the current exact root {}; no protected key was requested",
                prepared.plan.decision_root
            ),
            Some("rerun without the confirmation pair to render a fresh preview"),
        );
    }

    // A protected decision is never allowed to repin or continue unpinned.
    ceremony_binary_gate(false);
    if crate::config::binary_pin::load_pin().is_none() {
        ui::fail_with(
            ErrorKind::Custody,
            "protected decisions require an exact binary pin",
            Some(
                "rerun `vela id protect --user-presence --remove-source-key` with the installed release",
            ),
        );
    }
    if !json {
        render_final_decision_set(&[&review], std::slice::from_ref(&answer), &prepared);
        println!(
            "  · requesting one exact decision card for {action_name} {} at root {}",
            safe_inline(id),
            safe_inline(&prepared.plan.decision_root[..prepared.plan.decision_root.len().min(23)]),
        );
    }

    let gate_state = format!(
        "accept_ready={};reject_ready={}",
        review.brief.accept_ready(),
        review.brief.reject_ready()
    );
    let outcome = execute_confirmed_protected_decision(
        &frontier,
        &prepared,
        &review,
        action_name,
        id,
        &answer.proposal_root,
        &answer.reason,
        &review.observed_at,
        &gate_state,
    )
    .unwrap_or_else(|error| fail_decision(error));
    let publication = publish_exact_decision(
        &frontier,
        "review decide",
        &outcome,
        &crate::config::git_publish::PublishOptions::new(false),
    );
    let payload = serde_json::json!({
        "ok": true,
        "command": "review.decide",
        "schema": "vela.review-decision.v1",
        "frontier": frontier.display().to_string(),
        "proposal_id": id,
        "reviewer": actor,
        "action": action_name,
        "decision": summary,
        "operation_id": outcome.operation_id,
        "event_id": outcome.event_ids.first(),
        "event_ids": outcome.event_ids,
        "publication": publication,
        "signed": true,
        "key_read_by_cli": false,
        "signer": "vela-signer",
    });
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("decision result JSON")
        );
    } else {
        println!("  · signed {action_name} for {id}");
        println!(
            "  · publication {}",
            safe_inline(
                serde_json::to_string(&publication).unwrap_or_else(|_| "unknown".to_string())
            )
        );
    }
}

/// `vela sign <vpr_id>` — scripted preview first, then an exact-root accept.
pub(crate) fn cmd_sign_one(
    frontier: Option<PathBuf>,
    id: &str,
    reason: Option<String>,
    key: Option<PathBuf>,
    yes: bool,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json: bool,
) {
    if confirm_root.is_some() != confirm_at.is_some() {
        ui::fail_with(
            ErrorKind::Usage,
            "scripted confirmation requires both --confirm-root and --confirm-at from the same preview",
            Some("rerun without either flag to render a fresh key-free preview"),
        );
    }
    if confirm_root.is_some() && !yes {
        ui::fail_with(
            ErrorKind::Usage,
            "--confirm-root does not mutate by itself; scripted decisions require --yes, --confirm-root, and --confirm-at",
            Some("rerun the same command with --yes after reviewing the rendered root"),
        );
    }
    let actor = crate::cli_identity::resolve_decision_actor(None);
    let dir = crate::ui::resolve_frontier(frontier);
    let reason = reason.unwrap_or_else(|| "accepted via sign".to_string());
    if let Some(confirm_at) = confirm_at {
        crate::decision_plan::validate_scripted_confirmation_time(confirm_at)
            .unwrap_or_else(|error| fail_decision(error));
    }
    let review = match confirm_at {
        Some(observed_at) => {
            crate::review_material::ReviewProjection::one_at(&dir, id, observed_at)
        }
        None => crate::review_material::ReviewProjection::one_read_only(&dir, id),
    }
    .unwrap_or_else(|error| ui::fail_with(ErrorKind::Domain, &error.to_string(), None));
    if !review.brief.accept_ready() {
        let explanation = review
            .brief
            .action("accept")
            .map(|action| action.reasons.join("; "))
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| "accept is unavailable for this proposal".to_string());
        ui::fail_with(ErrorKind::Domain, &explanation, None);
    }
    let answer = saved_answer_from_review(&review, DecisionAction::Accept, reason);
    let decided_at = review.observed_at.clone();
    let provenance = crate::cli_identity::resolve_co_author_provenance(None, None);
    let prepared = crate::decision_plan::build_read_only_preview(
        &dir,
        std::slice::from_ref(&answer),
        &actor,
        &decided_at,
        provenance,
    )
    .unwrap_or_else(|error| fail_decision(error));
    let summary = final_decision_summary(&[&review], std::slice::from_ref(&answer), &prepared);
    let Some(confirmed_root) = confirm_root else {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "command": "sign.preview",
                    "frontier": dir.display().to_string(),
                    "proposal_id": id,
                    "reviewer": actor,
                    "decision": summary,
                    "confirm_at": review.observed_at,
                    "confirmation": {
                        "root": prepared.plan.decision_root,
                        "at": review.observed_at,
                        "arguments": [
                            "--yes",
                            "--confirm-root",
                            prepared.plan.decision_root,
                            "--confirm-at",
                            review.observed_at,
                        ],
                    },
                    "signed": false,
                    "key_read": false,
                })
            );
        } else {
            render_final_decision_set(&[&review], std::slice::from_ref(&answer), &prepared);
            println!("  · preview only; no key was read and nothing changed");
            println!(
                "  · rerun with --yes --confirm-root {} --confirm-at {} after reviewing this exact set",
                safe_inline(&prepared.plan.decision_root),
                safe_inline(&review.observed_at),
            );
        }
        return;
    };
    if confirmed_root != prepared.plan.decision_root {
        ui::fail_with(
            ErrorKind::Domain,
            &format!(
                "confirmed decision root {confirmed_root} does not match the current exact root {}; review the current semantic set before signing",
                prepared.plan.decision_root
            ),
            Some("rerun without --confirm-root to render a fresh key-free preview"),
        );
    }
    ceremony_binary_gate(false);
    if !json {
        render_final_decision_set(&[&review], std::slice::from_ref(&answer), &prepared);
        println!("  · exact prior root confirmed; entering the one-key decision edge");
    }

    let outcome = execute_confirmed_decision(&dir, &prepared, key.as_deref())
        .unwrap_or_else(|error| fail_decision(error));
    let publish_opts = crate::config::git_publish::PublishOptions::new(false);
    let publication = publish_exact_decision(&dir, "sign", &outcome, &publish_opts);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "command": "sign",
                "operation_id": outcome.operation_id,
                "decision": summary,
                "event_id": outcome.event_ids.first(),
                "event_ids": outcome.event_ids,
                "aggregate_engine": outcome.aggregate_engine,
                "publication": publication,
            })
        );
    } else {
        println!(
            "  · signed {} -> {} event(s)",
            crate::cli::safe_text::inline(id),
            outcome.event_ids.len()
        );
        println!(
            "  · publication {}",
            crate::cli::safe_text::inline(
                &serde_json::to_string(&publication).unwrap_or_else(|_| "unknown".to_string())
            )
        );
        println!(
            "  · retained {}",
            crate::cli::safe_text::inline(&outcome.operation_id)
        );
        println!(
            "  · next {}",
            crate::cli::safe_text::inline(
                publication
                    .recovery_command
                    .as_deref()
                    .unwrap_or("vela status --json")
            )
        );
    }
}

/// `vela sign <path>` — detached bytes under your key.
pub(crate) fn cmd_sign_detached(path: &Path, key: Option<&Path>, json: bool) {
    let _actor = crate::cli_identity::resolve_decision_actor(None);
    ceremony_binary_gate(false);
    apply_detached_sign(path, key);
    if json {
        println!(
            "{}",
            serde_json::json!({"ok": true, "command": "sign", "signed": path.display().to_string()})
        );
    }
}

fn apply_detached_sign(path: &Path, key: Option<&Path>) {
    let Ok(bytes) = std::fs::read(path) else {
        ui::fail_with(
            ErrorKind::NotFound,
            &format!("{} not found", path.display()),
            None,
        );
    };
    let signing_key = crate::cli_identity::resolve_signing_key(key);
    let record = detached::sign_detached(
        &path.file_name().unwrap_or_default().to_string_lossy(),
        &bytes,
        &signing_key,
        &chrono::Utc::now().to_rfc3339(),
    );
    let sig_path = path.with_extension(format!(
        "{}sig.json",
        path.extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ));
    std::fs::write(
        &sig_path,
        format!("{}\n", serde_json::to_string_pretty(&record).unwrap()),
    )
    .unwrap_or_else(|e| ui::fail_with(ErrorKind::Domain, &format!("write sig: {e}"), None));
    println!(
        "  · signed {} -> {} (sha256 {})",
        safe_inline(path.display().to_string()),
        safe_inline(sig_path.display().to_string()),
        &record.subject_sha256[..16]
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision_item(id: &str, preview: Vec<String>) -> SignItem {
        SignItem {
            lane: SignLane::Decision,
            id: id.to_string(),
            title: format!("claim {id}"),
            why_here: "policy deferred this claim".to_string(),
            preview,
            review: None,
        }
    }

    #[test]
    fn uppercase_a_cannot_accept_unrendered_items() {
        assert_eq!(
            session_command("A", SignLane::Decision),
            SessionCommand::Invalid
        );
        assert_eq!(
            session_command("a", SignLane::Decision),
            SessionCommand::Accept
        );
    }

    #[test]
    fn typed_saved_answer_round_trips_exact_review_roots() {
        let state = SessionState {
            answers: std::collections::BTreeMap::from([(
                "vpr_exact".to_string(),
                StoredAnswer {
                    proposal_id: "vpr_exact".to_string(),
                    proposal_root: "sha256:proposal".to_string(),
                    seen_decision_facts_root: "sha256:facts".to_string(),
                    action: StoredDecisionAction::Reject,
                    reason: "evidence does not support the claim".to_string(),
                },
            )]),
            latest_decision_root: Some("sha256:decision".to_string()),
        };
        let bytes = serde_json::to_vec(&state).unwrap();
        let decoded: SessionState = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.answers, state.answers);
        assert_eq!(decoded.latest_decision_root, state.latest_decision_root);
        let saved = decoded.answers["vpr_exact"].to_saved_answer();
        assert_eq!(saved.proposal_root, "sha256:proposal");
        assert_eq!(saved.seen_decision_facts_root, "sha256:facts");
        assert_eq!(saved.action, DecisionAction::Reject);
    }

    #[test]
    fn legacy_string_answers_fail_closed_instead_of_becoming_verdicts() {
        let legacy = r#"{"answers":{"vpr_old":"accept"}}"#;
        assert!(serde_json::from_str::<SessionState>(legacy).is_err());
    }

    #[test]
    fn final_summary_material_is_complete_bounded_and_terminal_safe() {
        let item = decision_item(
            "vpr_review",
            (0..10)
                .map(|index| {
                    if index == 1 {
                        "artifact\u{1b}]8;;https://bad.example\u{7}link".to_string()
                    } else {
                        format!("preview {index}")
                    }
                })
                .collect(),
        );
        let lines = render_item_review_lines(&item);
        let transcript = lines.join("\n");
        assert!(transcript.contains("preview 0"));
        assert!(transcript.contains("\\u{001B}"));
        assert!(!transcript.contains('\u{1b}'));
        assert!(transcript.contains("2 preview line(s) omitted; sha256:"));
        assert!(transcript.contains("why you: policy deferred this claim"));
        assert!(transcript.contains("vpr_review"));
    }

    #[test]
    fn decision_fact_codes_are_presented_as_words() {
        assert_eq!(humanize_fact("engine_gate_blocked"), "Engine gate blocked");
        assert_eq!(
            humanize_fact("no-independent-evidence"),
            "No independent evidence"
        );
    }
}
