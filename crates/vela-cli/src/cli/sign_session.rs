//! `vela sign` — THE human ceremony. One interactive session over
//! everything that awaits your key (the sign-queue projection), one
//! summary, one confirm, one key read, one publish per frontier.
//!
//! Custody: agent/ci actors exit 4 before anything renders. There is
//! deliberately no `--all --yes` for the interactive session — a
//! decision without eyes on the item is a rubber stamp. Scripted forms
//! exist for single items (`sign <id> --yes`), pre-filled verdict
//! batches (`sign --batch file.json`), and detached artifact bytes
//! (`sign <path>`).
//!
//! Run inside a frontier: that frontier's queue. Run outside: every
//! registered frontier's queue (the workspace registry), one session,
//! one key read total — the morning ritual.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vela_edge::sign_queue::{SignItem, SignLane, SignQueueInput, SupplementalSignItem, sign_queue};
use vela_protocol::{detached, proposals, repo};

use colored::Colorize;
use vela_protocol::cli_style as style;

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

/// Saved-as-you-go session state: answers survive `q` and crashes.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SessionState {
    /// item id -> decision ("accept" | "reject" | reason-tagged forms)
    answers: std::collections::BTreeMap<String, String>,
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

fn supplemental_sign_items(
    project: &vela_protocol::project::Project,
    frontier: &Path,
) -> (Vec<SupplementalSignItem>, Vec<SupplementalSignItem>) {
    let registered = project
        .actors
        .iter()
        .map(|actor| actor.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut unsigned_by_actor = std::collections::BTreeMap::<&str, usize>::new();
    for event in &project.events {
        if event.signature.is_none() && registered.contains(event.actor.id.as_str()) {
            *unsigned_by_actor
                .entry(event.actor.id.as_str())
                .or_default() += 1;
        }
    }
    let hygiene = unsigned_by_actor
        .into_iter()
        .map(|(actor, count)| {
            SupplementalSignItem::new(
                format!("resign:{actor}"),
                format!("re-sign {count} unsigned event(s) by {actor}"),
                "events predate signing; strict verification flags a registered actor without signatures",
                Vec::new(),
            )
        })
        .collect();

    let detached = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier)
        .ok()
        .filter(|snapshot| snapshot.policy_bytes.is_some() && snapshot.signature_bytes.is_none())
        .map(|_| {
            vec![SupplementalSignItem::new(
                frontier
                    .join(".vela/policies/active.json")
                    .display()
                    .to_string(),
                "sealed policy awaits your signature (the lane is closed until then)",
                "a policy without a human signature carries no authority",
                Vec::new(),
            )]
        })
        .unwrap_or_default();
    (hygiene, detached)
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

/// Which frontiers this session covers: cwd's frontier when inside one,
/// else every live registered frontier.
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

    // Build every queue up front so the header can tell the whole story.
    let mut queues = Vec::<FrontierQueue>::new();
    let mut total = 0usize;
    for dir in &frontiers {
        let project = match repo::load_from_path(dir) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "  skipping {}: {}",
                    safe_inline(dir.display().to_string()),
                    safe_inline(e)
                );
                continue;
            }
        };
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
        let (hygiene, detached) = supplemental_sign_items(&project, dir);
        let queue = sign_queue(SignQueueInput {
            judgments: Vec::new(),
            decisions: page.items,
            hygiene,
            detached,
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

    let fr = if queues.len() == 1 {
        "frontier"
    } else {
        "frontiers"
    };
    let it = if total == 1 {
        "item awaits"
    } else {
        "items await"
    };
    ui::header(
        "SIGN",
        &format!("{} {fr}", queues.len()),
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
    // through `sign <id> --yes` / `--batch` (separate dispatch arms) and
    // never reach here.
    ui::ensure_can_prompt(
        "the sign session",
        "decide one with `sign <vpr_id> --yes`, or many with `sign --batch <verdicts.json>`",
    );

    // Accepts carry this bookkeeping note; the judgment IS the a/r
    // answer, so nothing prompts for it. (Rejects still ask for their
    // reason — that one is content.)
    let session_reason = "accepted via sign session".to_string();

    // The judgment loop: per frontier, per item; resume-safe.
    let total_signable = total;
    let mut position = 0usize;
    for queue in &queues {
        let mut state = load_session(&queue.dir);
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
                            state.answers.insert(item.id.clone(), "accept".into());
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
                        let why = read_line("  reject reason: ");
                        state
                            .answers
                            .insert(item.id.clone(), format!("reject:{why}"));
                        break;
                    }
                    SessionCommand::Yes => {
                        state.answers.insert(item.id.clone(), "yes".into());
                        break;
                    }
                    SessionCommand::Invalid => println!("    keys: {keys}"),
                }
            }
            save_session(&queue.dir, &state);
        }
    }

    // Summary + the ONE confirm — editable, so a mistake here never traps
    // you (clig.dev: let users correct choices; show and change state). The
    // loop re-renders after every edit; only `y` falls through to the key
    // read, and `reject` verdicts show in red so an accidental accept stands
    // out before you sign.
    loop {
        let mut answered: Vec<(PathBuf, String)> = Vec::new();
        println!("\n  ══════════════════════════════════════════════════");
        for queue in &queues {
            let mut state = load_session(&queue.dir);
            let mut removed_blocked = false;
            for item in &queue.items {
                if let Some(ans) = state.answers.get(&item.id).cloned() {
                    let verdict = ans.split(':').next().unwrap_or(ans.as_str());
                    let action_allowed = match (item.lane, verdict) {
                        (SignLane::Decision, "accept") => action_available(item, "accept"),
                        (SignLane::Decision, "reject") => action_available(item, "reject"),
                        _ => true,
                    };
                    if !action_allowed {
                        println!(
                            "  {}  {}  {}",
                            style::madder("blocked"),
                            safe_inline(&item.id),
                            safe_inline(explain_blocked_action(item, verdict))
                        );
                        state.answers.remove(&item.id);
                        removed_blocked = true;
                        continue;
                    }
                    // Pad to width first, THEN color — ANSI codes would break
                    // right-alignment if we formatted the colored string.
                    let label = format!("{verdict:>9}");
                    let shown = match verdict {
                        "reject" => style::madder(&label).to_string(),
                        "accept" | "yes" => style::moss(&label).to_string(),
                        _ => label.clone(),
                    };
                    println!(
                        "  {shown}  {}  {}",
                        safe_inline(&item.id),
                        safe_inline(&item.title)
                    );
                    for line in render_item_review_lines(item) {
                        println!("             {}", style::dim(&line));
                    }
                    answered.push((queue.dir.clone(), item.id.clone()));
                }
            }
            if removed_blocked {
                save_session(&queue.dir, &state);
            }
        }
        let planned = answered.len();
        if planned == 0 {
            println!("  nothing answered; nothing to sign.");
            return;
        }
        let choice = read_line(&format!(
            "\nSign {planned} item(s) as {}?  [{}es · {}dit one · {}eset all · {}o] > ",
            safe_inline(&actor),
            style::moss("y"),
            style::brass("e"),
            style::madder("r"),
            style::dim("n"),
        ));
        match choice.as_str() {
            "y" => break,
            "e" => {
                let id = read_line("  item id (or a prefix) to change: ");
                match answered
                    .iter()
                    .find(|(_, iid)| *iid == id || iid.starts_with(&id))
                {
                    Some((dir, iid)) => {
                        let v = read_line(
                            "  new verdict — [a]ccept · [r]eject · [s]kip (leave pending): ",
                        );
                        let mut st = load_session(dir);
                        match v.as_str() {
                            "a" => {
                                st.answers.insert(iid.clone(), "accept".into());
                            }
                            "r" => {
                                let why = read_line("  reject reason: ");
                                st.answers.insert(iid.clone(), format!("reject:{why}"));
                            }
                            "s" => {
                                st.answers.remove(iid);
                            }
                            _ => println!("  unchanged (need a, r, or s)"),
                        }
                        save_session(dir, &st);
                    }
                    None => println!("  no answered item matches `{}`", safe_inline(id)),
                }
            }
            "r" => {
                for dir in &frontiers {
                    clear_session(dir);
                }
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
    }

    // Apply, one publish per frontier.
    let apply_spin = crate::cli::progress::Spinner::start("applying your decisions");
    // One key read for the whole session: the reviewer's identity key,
    // resolved once and threaded into every accept/reject. A key-registered
    // reviewer whose key is absent fails the custody check in the engine —
    // and that failure is surfaced below, never swallowed.
    let signing_key = crate::cli_identity::resolve_signing_key_opt(key.as_deref());
    let mut total_persisted = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut publication_reports: Vec<(
        String,
        String,
        crate::config::git_publish::PublicationOutcome,
    )> = Vec::new();
    for queue in &queues {
        let dir = &queue.dir;
        let publish_opts = crate::config::git_publish::PublishOptions::new(false, false);
        let publication_preflight =
            crate::config::git_publish::publication_preflight(dir, &publish_opts);
        if let Err(outcome) = &publication_preflight
            && crate::config::git_publish::publication_is_busy(outcome)
        {
            failures.push((
                dir.display().to_string(),
                "another Vela write/publication owns this repository; no decision was persisted here — retry after it completes"
                    .to_string(),
            ));
            continue;
        }
        let mut state = load_session(dir);
        let mut accepted: Vec<String> = Vec::new();
        let mut event_ids: Vec<String> = Vec::new();
        let mut frontier_failed = false;
        for item in &queue.items {
            let Some(ans) = state.answers.get(&item.id).cloned() else {
                continue;
            };
            match (item.lane, ans.as_str()) {
                (SignLane::Decision, "accept") => accepted.push(item.id.clone()),
                (SignLane::Decision, a) if a.starts_with("reject:") => {
                    let reason = a.trim_start_matches("reject:");
                    // Reject emits its signed review event inside the engine,
                    // under the same session key; the publish below sweeps the
                    // store change. A custody/validation failure is recorded,
                    // not swallowed.
                    match proposals::reject_at_path_signed(
                        dir,
                        &item.id,
                        &actor,
                        if reason.is_empty() {
                            "rejected via sign session"
                        } else {
                            reason
                        },
                        signing_key.as_ref(),
                    ) {
                        Ok(()) => event_ids.push(format!("reject:{}", item.id)),
                        Err(e) => {
                            failures.push((item.id.clone(), e));
                            frontier_failed = true;
                        }
                    }
                }
                (SignLane::Hygiene, "yes") => {
                    // The re-sign ceremony, absorbed: same machinery as
                    // the old `vela id sign` / resign-frontier.sh.
                    crate::cli::identity::cmd_id_sign(dir.clone(), key.clone(), false);
                }
                (SignLane::Detached, "yes") => {
                    apply_detached_sign(Path::new(&item.id), key.as_deref());
                }
                _ => {}
            }
        }
        if !accepted.is_empty() {
            match proposals::accept_batch_at_path(
                dir,
                &accepted,
                &actor,
                &session_reason,
                proposals::AcceptOptions {
                    strict: false,
                    force: false,
                    signing_key: signing_key.clone(),
                    custody_verified: false,
                    provenance: crate::cli_identity::resolve_co_author_provenance(None, None),
                },
                false,
            ) {
                Ok(report) => {
                    if report.gated {
                        eprintln!(
                            "  engine gate blocked the batch in {}: nothing persisted there",
                            safe_inline(dir.display().to_string())
                        );
                        frontier_failed = true;
                    }
                    // A per-proposal custody/validation error is reported by
                    // the engine but does NOT abort the batch. Surface every
                    // one — a failed accept must never masquerade as signed.
                    for (pid, err) in &report.failed {
                        failures.push((pid.clone(), err.clone()));
                        frontier_failed = true;
                    }
                    event_ids.extend(report.event_ids.clone());
                }
                Err(e) => {
                    failures.push((dir.display().to_string(), e));
                    frontier_failed = true;
                }
            }
        }
        if !event_ids.is_empty() {
            let publication = match publication_preflight {
                Ok(preflight) => {
                    let publish_opts = publish_opts.with_preflight(preflight);
                    crate::config::git_publish::publish_decision(
                        dir,
                        "sign",
                        &event_ids,
                        &publish_opts,
                    )
                }
                Err(outcome) => outcome,
            };
            let planning_identity = event_ids.join("\0");
            let operation_id = crate::operation_journal::operation_id(
                "sign-session-publication",
                planning_identity.as_bytes(),
            );
            publication_reports.push((dir.display().to_string(), operation_id, publication));
            total_persisted += event_ids.len();
        }
        // Consume the session only when every decision applied. A failure
        // leaves the answers on disk so `vela sign` resumes and retries them.
        if !frontier_failed {
            state.answers.clear();
            let _ = std::fs::remove_file(session_path(dir));
        }
    }
    // Honesty gate: the ceremony reports "signed" only when a decision
    // durably persisted. If nothing did, say so and exit non-zero — a false
    // "signed" is the one failure a trust ceremony cannot have.
    if total_persisted == 0 {
        apply_spin.finish("nothing signed");
        println!();
        for (id, err) in &failures {
            eprintln!(
                "  {} {}: {}",
                style::madder("failed"),
                crate::cli::safe_text::inline(id),
                crate::cli::safe_text::inline(err)
            );
        }
        ui::fail_with(
            ErrorKind::Custody,
            "no decision was signed — nothing changed. Fix the error above and re-run `vela sign`.",
            Some(
                "a reviewer registered with a key signs with that key; run `vela sign` at a \
                 terminal (or pass --key) so the identity key is read",
            ),
        );
    }
    apply_spin.finish("applied");
    println!("\n  · signed. `vela log` shows the lane on every event.");
    for (frontier, operation_id, publication) in &publication_reports {
        println!(
            "  · publication {} · {} · retained {}",
            crate::cli::safe_text::inline(frontier),
            crate::cli::safe_text::inline(
                &serde_json::to_string(publication).unwrap_or_else(|_| "unknown".to_string())
            ),
            crate::cli::safe_text::inline(operation_id)
        );
        println!(
            "    next: {}",
            crate::cli::safe_text::inline(
                publication
                    .recovery_command
                    .as_deref()
                    .unwrap_or("vela status --json")
            )
        );
    }
    if !failures.is_empty() {
        for (id, err) in &failures {
            eprintln!(
                "  {} {}: {}",
                style::madder("failed"),
                crate::cli::safe_text::inline(id),
                crate::cli::safe_text::inline(err)
            );
        }
        eprintln!(
            "  {} some items did not sign — re-run `vela sign` to retry them",
            style::warn("partial")
        );
    }

    // The self-shrinking step: if what you just signed keeps recurring,
    // say so once — the rule that absorbs the class is one command away.
    for queue in &queues {
        if let Some(hint) = crate::config::cli_policy::suggest_hint(&queue.dir) {
            println!(
                "  {}",
                vela_protocol::cli_style::dim(&crate::cli::safe_text::inline(&hint))
            );
            break;
        }
    }
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

/// `vela sign <vpr_id> --yes` — the scripted single accept.
pub(crate) fn cmd_sign_one(
    frontier: Option<PathBuf>,
    id: &str,
    reason: Option<String>,
    key: Option<PathBuf>,
    json: bool,
) {
    let actor = crate::cli_identity::resolve_decision_actor(None);
    ceremony_binary_gate(false);
    let dir = crate::ui::resolve_frontier(frontier);
    let signing_key = crate::cli_identity::resolve_signing_key_opt(key.as_deref());
    let reason = reason.unwrap_or_else(|| "accepted via sign".to_string());
    let publish_opts = crate::config::git_publish::PublishOptions::new(false, false);
    let publication_preflight =
        crate::config::git_publish::publication_preflight(&dir, &publish_opts);
    if let Err(outcome) = &publication_preflight
        && crate::config::git_publish::publication_is_busy(outcome)
    {
        ui::fail_with(
            ErrorKind::Domain,
            "another Vela write/publication owns this repository; the decision was not persisted",
            Some("retry the same `vela sign` command after the active operation completes"),
        );
    }
    match proposals::accept_at_path_engine(
        &dir,
        id,
        &actor,
        &reason,
        proposals::AcceptOptions {
            strict: false,
            force: false,
            signing_key,
            custody_verified: false,
            provenance: crate::cli_identity::resolve_co_author_provenance(None, None),
        },
    ) {
        Ok(outcome) => {
            let publication = match publication_preflight {
                Ok(preflight) => {
                    let publish_opts = publish_opts.with_preflight(preflight);
                    crate::config::git_publish::publish_decision(
                        &dir,
                        "sign",
                        &[outcome.event_id.clone()],
                        &publish_opts,
                    )
                }
                Err(publication) => publication,
            };
            let operation_id =
                crate::operation_journal::operation_id("sign-command", outcome.event_id.as_bytes());
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "command": "sign",
                        "operation_id": operation_id,
                        "event_id": outcome.event_id,
                        "publication": publication,
                    })
                );
            } else {
                println!(
                    "  · signed {} -> {}",
                    crate::cli::safe_text::inline(id),
                    crate::cli::safe_text::inline(&outcome.event_id)
                );
                println!(
                    "  · publication {}",
                    crate::cli::safe_text::inline(
                        &serde_json::to_string(&publication)
                            .unwrap_or_else(|_| "unknown".to_string())
                    )
                );
                println!(
                    "  · retained {}",
                    crate::cli::safe_text::inline(&operation_id)
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
        Err(e) if e.contains("already applied") || e.contains("is applied") => ui::fail_with(
            ErrorKind::Exists,
            &format!("{id} is already decided"),
            Some("`vela log` shows the decision"),
        ),
        Err(e) => ui::fail_with(ErrorKind::Domain, &e, None),
    }
}

/// `vela sign <path>` — detached bytes under your key.
pub(crate) fn cmd_sign_detached(path: &Path, key: Option<&Path>, json: bool) {
    let _actor = crate::cli_identity::resolve_decision_actor(None);
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
}
