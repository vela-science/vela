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
    // Custody gate before anything renders.
    let actor = crate::cli_identity::resolve_decision_actor(None);
    ceremony_binary_gate();

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

    let default_reason = "accepted via sign session";
    let session_reason = {
        let entered = read_line(&format!("session reason [Enter = \"{default_reason}\"]: "));
        if entered.is_empty() {
            default_reason.to_string()
        } else {
            entered
        }
    };

    // The judgment loop: per frontier, per item; resume-safe.
    let total_signable = total;
    let mut position = 0usize;
    for (dir, _project, items) in &queues {
        let mut state = load_session(dir);
        for item in items {
            if !item.signable || state.answers.contains_key(&item.id) {
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
            if let Some(pack) = &item.pack {
                println!(
                    "    {}",
                    style::dim(&format!("pack {pack} — deciding one decides the set"))
                );
            }
            println!("    {}", style::dim(&item.id));
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
    for (dir, _, items) in &queues {
        let mut state = load_session(dir);
        let mut accepted: Vec<String> = Vec::new();
        let mut event_ids: Vec<String> = Vec::new();
        for item in items {
            let Some(ans) = state.answers.get(&item.id).cloned() else {
                continue;
            };
            match (item.lane, ans.as_str()) {
                (SignLane::Decision, "accept") => accepted.push(item.id.clone()),
                (SignLane::Decision, a) if a.starts_with("reject:") => {
                    let reason = a.trim_start_matches("reject:");
                    // Reject emits its signed review event inside the
                    // engine; the publish below sweeps the store change.
                    if let Err(e) = proposals::reject_at_path(
                        dir,
                        &item.id,
                        &actor,
                        if reason.is_empty() {
                            "rejected via sign session"
                        } else {
                            reason
                        },
                    ) {
                        eprintln!("  reject {} failed: {e}", item.id);
                    } else {
                        event_ids.push(format!("reject:{}", item.id));
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
            let signing_key = crate::cli_identity::resolve_signing_key_opt(key.as_deref());
            match proposals::accept_batch_at_path(
                dir,
                &accepted,
                &actor,
                &session_reason,
                proposals::AcceptOptions {
                    strict: false,
                    force: false,
                    signing_key,
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
                        continue;
                    }
                    event_ids.extend(report.event_ids.clone());
                }
                Err(e) => {
                    eprintln!("  accept batch in {} failed: {e}", dir.display());
                    continue;
                }
            }
        }
        if !event_ids.is_empty() {
            let opts = crate::config::git_publish::PublishOptions::new(false, false);
            crate::config::git_publish::publish_decision(dir, "sign", &event_ids, &opts);
        }
        // The session is consumed.
        state.answers.clear();
        let _ = std::fs::remove_file(session_path(dir));
    }
    println!("\n  · signed. `vela log` shows the lane on every event.");
}

/// The clear-signing binary gate every ceremony runs first: a pinned
/// binary that no longer matches refuses; no pin renders a one-line
/// notice (pinning is opt-in but the ceremony never hides its state).
pub(crate) fn ceremony_binary_gate() {
    match crate::config::binary_pin::verify_for_ceremony() {
        Ok(Some(_)) => {}
        Ok(None) => eprintln!(
            "  {}",
            vela_protocol::cli_style::dim(
                "unpinned binary — `vela id pin-binary` anchors your ceremonies"
            )
        ),
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
    ceremony_binary_gate();
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
