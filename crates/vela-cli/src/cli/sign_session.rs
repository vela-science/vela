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

use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vela_edge::sign_queue::{SignItem, SignLane, sign_queue};
use vela_protocol::acceptance_policy::PolicyContext;
use vela_protocol::{detached, proposals, repo};

use colored::Colorize;
use vela_protocol::cli_style as style;

use crate::ui::{self, ErrorKind};

/// Saved-as-you-go session state: answers survive `q` and crashes.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SessionState {
    /// item id -> decision ("accept" | "reject" | reason-tagged forms)
    answers: std::collections::BTreeMap<String, String>,
}

/// The pack-level semantic preview, rendered inside a hard budget so the
/// ceremony stays scannable: 1 scope line, <=4 (already-capped) state ops,
/// 1 polarity line, <=5 gate rows, 1 blast line, <=1 missing note —
/// display only, no new prompts, the same one confirm and one key read.
fn render_pack_preview(pp: &vela_edge::sign_preview::PackSignPreview) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!(
        "pack      {} — deciding one decides the set",
        pp.pack_id
    ));
    out.push(format!("scope     {}", pp.scope));
    for op in &pp.state_ops {
        out.push(format!("op        {op}"));
    }
    if !pp.polarity.is_empty() {
        let line: Vec<String> = pp
            .polarity
            .iter()
            .map(|(name, n)| format!("{name} {n}"))
            .collect();
        out.push(format!("polarity  {}", line.join(" · ")));
    }
    for row in pp.gate_matrix.iter().take(5) {
        out.push(format!(
            "gate      {} {} ({} reason{})",
            row.finding_id,
            row.status,
            row.reason_count,
            if row.reason_count == 1 { "" } else { "s" }
        ));
    }
    if let Some(b) = &pp.blast {
        out.push(format!(
            "blast     {} weakened, {} support-killed of {} downstream",
            b.weakened, b.support_killed, b.downstream_candidates
        ));
    }
    if let Some(first) = pp.missing.first() {
        out.push(format!("unshown   {first}"));
    }
    out
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

/// The conservative context used to filter the queue: nothing proven.
/// Richer per-proposal derivation (assurance from the gate) lands with
/// `vela land`; until then the queue errs toward showing the human
/// MORE, never less.
fn conservative_ctx(_project: &vela_protocol::project::Project, _id: &str) -> PolicyContext {
    PolicyContext::default()
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
    use std::io::Write;
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

/// The interactive session (the default form of `vela sign`).
pub(crate) fn cmd_sign_session(frontier: Option<PathBuf>, key: Option<PathBuf>, json: bool) {
    let frontiers = session_frontiers(frontier);
    if frontiers.is_empty() {
        ui::fail_with(
            ErrorKind::NotFound,
            "no frontier here and none registered",
            Some("run inside a frontier, or `vela init` one (init registers it)"),
        );
    }

    // Build every queue up front so the header can tell the whole story.
    let mut queues: Vec<(PathBuf, vela_protocol::project::Project, Vec<SignItem>)> = Vec::new();
    let mut total = 0usize;
    for dir in &frontiers {
        let project = match repo::load_from_path(dir) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  skipping {}: {e}", dir.display());
                continue;
            }
        };
        match sign_queue(&project, dir, conservative_ctx) {
            Ok(q) => {
                total += q.items.iter().filter(|i| i.signable).count();
                queues.push((dir.clone(), project, q.items));
            }
            Err(e) => eprintln!("  skipping {}: {e}", dir.display()),
        }
    }

    if json {
        // `sign --list --json` shape: the queue, no session.
        let out = serde_json::json!({
            "ok": true,
            "command": "sign",
            "frontiers": queues.iter().map(|(d, _, items)| serde_json::json!({
                "frontier": d.display().to_string(),
                "items": items,
            })).collect::<Vec<_>>(),
            "signable_total": total,
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

    // Accepts carry this bookkeeping note; the judgment IS the a/r
    // answer, so nothing prompts for it. (Rejects still ask for their
    // reason — that one is content.)
    let session_reason = "accepted via sign session".to_string();

    // The judgment loop: per frontier, per item; resume-safe.
    let total_signable = total;
    let mut position = 0usize;
    for (dir, _project, items) in &queues {
        let mut state = load_session(dir);
        // Set once the human chooses "accept all remaining" (capital A) on a
        // Decision item: the rest of this frontier's decision items are marked
        // accept without a per-item keystroke. The final summary still lists
        // every planned verdict and the single confirm still gates, so the
        // human sees the whole set before one key read — a batch decision over
        // a shown set, not a blind one.
        let mut bulk_accept = false;
        for item in items {
            if !item.signable || state.answers.contains_key(&item.id) {
                continue;
            }
            if bulk_accept && matches!(item.lane, SignLane::Decision) {
                state.answers.insert(item.id.clone(), "accept".into());
                save_session(dir, &state);
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
            println!("  {}", item.title.bold());
            for line in &item.preview {
                println!("    {}", style::dim(line));
            }
            println!("    {}", style::dim(&format!("why you: {}", item.why_here)));
            match (&item.pack_preview, &item.pack) {
                (Some(pp), _) => {
                    for line in render_pack_preview(pp) {
                        println!("    {}", style::dim(&line));
                    }
                }
                (None, Some(pack)) => {
                    println!(
                        "    {}",
                        style::dim(&format!("pack {pack} — deciding one decides the set"))
                    );
                }
                (None, None) => {}
            }
            println!("    {}", style::dim(&item.id));
            let keys = match item.lane {
                SignLane::Decision => "a/A/r/s/q",
                _ => "y/s/q",
            };
            loop {
                let legend = match item.lane {
                    SignLane::Decision => format!(
                        "{}ccept · {}ll-remaining · {}eject · {}kip · {}uit",
                        style::moss("a"),
                        style::moss("A"),
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
                match (ans.as_str(), item.lane) {
                    ("q", _) => {
                        save_session(dir, &state);
                        println!("  saved; re-run `vela sign` to resume.");
                        return;
                    }
                    ("s", _) => break,
                    ("a", SignLane::Decision) => {
                        state.answers.insert(item.id.clone(), "accept".into());
                        break;
                    }
                    ("A", SignLane::Decision) => {
                        // Accept this item and every remaining decision item in
                        // this frontier; the summary + one confirm still gate.
                        state.answers.insert(item.id.clone(), "accept".into());
                        bulk_accept = true;
                        break;
                    }
                    ("r", SignLane::Decision) => {
                        let why = read_line("  reject reason: ");
                        state
                            .answers
                            .insert(item.id.clone(), format!("reject:{why}"));
                        break;
                    }
                    ("y", SignLane::Hygiene) | ("y", SignLane::Detached) => {
                        state.answers.insert(item.id.clone(), "yes".into());
                        break;
                    }
                    ("y", SignLane::Judgment) => {
                        state.answers.insert(item.id.clone(), "yes".into());
                        break;
                    }
                    _ => println!("    keys: {keys}"),
                }
            }
            save_session(dir, &state);
        }
    }

    // Summary + the ONE confirm.
    let mut planned = 0usize;
    println!("\n  ══════════════════════════════════════════════════");
    for (dir, _, items) in &queues {
        let state = load_session(dir);
        for item in items {
            if let Some(ans) = state.answers.get(&item.id) {
                println!(
                    "  {:>9}  {}  {}",
                    ans.split(':').next().unwrap_or(ans),
                    item.id,
                    item.title
                );
                planned += 1;
            }
        }
    }
    if planned == 0 {
        println!("  nothing answered; nothing to sign.");
        return;
    }
    let yn = read_line(&format!(
        "\nSign {planned} item(s) as {actor} — one key read, self-publishes? [y/N] "
    ));
    if yn != "y" {
        println!("not signed; answers saved. Re-run `vela sign` to finish.");
        return;
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
    for (dir, _, items) in &queues {
        let mut state = load_session(dir);
        let mut accepted: Vec<String> = Vec::new();
        let mut event_ids: Vec<String> = Vec::new();
        let mut frontier_failed = false;
        for item in items {
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
                            dir.display()
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
            let opts = crate::config::git_publish::PublishOptions::new(false, false);
            crate::config::git_publish::publish_decision(dir, "sign", &event_ids, &opts);
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
            eprintln!("  {} {id}: {err}", style::madder("failed"));
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
    if !failures.is_empty() {
        for (id, err) in &failures {
            eprintln!("  {} {id}: {err}", style::madder("failed"));
        }
        eprintln!(
            "  {} some items did not sign — re-run `vela sign` to retry them",
            style::warn("partial")
        );
    }

    // The self-shrinking step: if what you just signed keeps recurring,
    // say so once — the rule that absorbs the class is one command away.
    for (dir, _, _) in &queues {
        if let Some(hint) = crate::config::cli_policy::suggest_hint(dir) {
            println!("  {}", vela_protocol::cli_style::dim(&hint));
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
                pinned.version,
                &pinned.pinned_at[..pinned.pinned_at.len().min(10)],
                &current_sha[..16],
                current_version,
                current_path.display()
            );
            eprintln!("  {}", vela_protocol::cli_style::warn("binary changed"));
            for line in render.lines() {
                eprintln!("  {line}");
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
                pin.version
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
            let opts = crate::config::git_publish::PublishOptions::new(false, false);
            crate::config::git_publish::publish_decision(
                &dir,
                "sign",
                &[outcome.event_id.clone()],
                &opts,
            );
            if json {
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "command": "sign", "event_id": outcome.event_id})
                );
            } else {
                println!("  · signed {id} -> {}", outcome.event_id);
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
        path.display(),
        sig_path.display(),
        &record.subject_sha256[..16]
    );
}
