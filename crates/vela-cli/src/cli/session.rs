//! The no-argument `vela` session dashboard: locate the enclosing `.vela/`
//! repo, print a one-screen frontier summary, and route bare session verbs.

use super::*;

/// Walk up from `cwd` looking for a `.vela/` directory. Returns the
/// first parent that contains one, or `None` if none found.
/// A frontier's `.vela` (it has the event log), NOT the user config
/// dir — `~/.vela` holds keys/identity/hub.env, and a parent walk from
/// anywhere under $HOME would otherwise "find" it and load the config
/// dir as an empty frontier.
fn is_frontier_store(store: &Path) -> bool {
    store.is_dir()
        && (store.join("events").is_dir()
            || store.join("proposals").is_dir()
            || store.join("genesis.json").is_file())
}

fn find_vela_repo() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if is_frontier_store(&cur.join(".vela")) {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

pub(crate) fn print_session_help() {
    println!(
        "  Vela {} · Version control for scientific state.",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("  Agents land. Verifiers reproduce. Humans approve. Git publishes.");
    println!();
    println!("  USAGE");
    println!("    vela <command> [options]");
    println!();
    println!("  COMMANDS");
    println!("    init       status     next       work");
    println!("    land       review     check      reproduce");
    println!("    log        doctor     migrate");
    println!();
    println!("  Run `vela help advanced` for setup nouns and advanced verification.");
    println!();
}

pub(crate) fn print_session_dashboard(project: &vela_protocol::project::Project, repo_path: &Path) {
    let label = frontier_label(project);
    let vfr = project.frontier_id();
    let vfr_short = vfr.chars().take(16).collect::<String>();

    let mut pending = 0usize;
    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for p in &project.proposals {
        if p.status == "pending_review" {
            pending += 1;
            *by_kind.entry(p.kind.clone()).or_insert(0) += 1;
        }
    }

    println!();
    let version = vela_protocol::project::VELA_COMPILER_VERSION
        .strip_prefix("vela/")
        .unwrap_or(vela_protocol::project::VELA_COMPILER_VERSION);
    println!(
        "  {}",
        format!("VELA · {version} · {label}")
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    println!(
        "  vfr_id     {}…   repo  {}",
        vfr_short,
        repo_path.display()
    );
    println!(
        "  findings   {:>4}     events   {}     proposals pending  {}",
        project.findings.len(),
        project.events.len(),
        pending
    );

    if pending > 0 {
        let parts: Vec<String> = by_kind.iter().map(|(k, n)| format!("{n} {k}")).collect();
        println!("  {}   · {}", style::warn("pending"), parts.join("  "));
    }
    println!();
    println!("  the loop: vela next · vela work <target> · vela land · vela sign");
    println!();
}

pub(crate) fn run_session() {
    let repo_path = match find_vela_repo() {
        Some(p) => p,
        None => {
            println!();
            println!(
                "  {}",
                "VELA · NO FRONTIER FOUND IN CWD OR ANY PARENT".dimmed()
            );
            println!("  {}", style::tick_row(60));
            println!("  Run `vela init` here to create a frontier, or cd into one.");
            println!("  Or run `vela help` for the command list.");
            println!();
            return;
        }
    };

    let project = match repo::load_from_path(&repo_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} failed to load .vela/ repo: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };

    print_session_dashboard(&project, &repo_path);

    // The dashboard IS the session: one screen of state plus the ranked
    // next actions, then your shell prompt back. (The old REPL loop is
    // retired — a prompt that shadows the shell helps neither humans
    // nor agents; every quick verb is one `vela <verb>` away.)
    let unpublished = crate::cli_read::unpublished_store_files(&repo_path);
    if unpublished > 0 {
        println!(
            "  {}  {unpublished} store file(s) not committed — signed state only on this machine",
            style::warn("unpublished")
        );
    }
    // The sign queue consumes the same bounded Decision Brief projection as
    // diff, status, MCP, and the ceremony. This dashboard never re-evaluates
    // policy or receipt facts independently.
    if let Ok(page) = crate::review_material::ReviewProjection::page(
        &repo_path,
        crate::review_material::ReviewRequest {
            limit: Some(3),
            ..crate::review_material::ReviewRequest::default()
        },
    ) {
        let queue = vela_edge::sign_queue::sign_queue(vela_edge::sign_queue::SignQueueInput {
            decisions: page.items,
            ..vela_edge::sign_queue::SignQueueInput::default()
        });
        let answerable = queue
            .items
            .iter()
            .filter(|item| {
                item.accept_action()
                    .is_some_and(vela_edge::decision_brief::DecisionAction::is_available)
                    || item
                        .reject_action()
                        .is_some_and(vela_edge::decision_brief::DecisionAction::is_available)
            })
            .count();
        if answerable > 0 {
            println!(
                "  {}  {answerable} of {} pending item(s) shown — `vela sign`",
                style::warn("sign queue"),
                page.total,
            );
        }
    }
    let observed_at = chrono::Utc::now().to_rfc3339();
    let targets = match vela_edge::frontier_next::try_frontier_next(
        &project,
        Some(&repo_path),
        &observed_at,
        3,
    ) {
        Ok(targets) => targets,
        Err(error) => {
            println!(
                "  {}  next projection failed: {error}",
                style::lost("blocked")
            );
            Vec::new()
        }
    };
    if !targets.is_empty() {
        println!();
        println!("  {}", "next, ranked (vela next for more):".dimmed());
        for t in &targets {
            println!(
                "    {}  {}",
                t.id,
                t.title.chars().take(56).collect::<String>()
            );
            println!("      {}", t.next_command.dimmed());
        }
    }
    println!();
}

#[cfg(test)]
mod frontier_store_tests {
    use super::is_frontier_store;

    #[test]
    fn config_shaped_vela_dir_is_not_a_frontier() {
        let tmp = std::env::temp_dir().join(format!("vela-store-test-{}", std::process::id()));
        // The user config shape: keys + identity, no event log.
        let config = tmp.join("config/.vela");
        std::fs::create_dir_all(config.join("keys")).unwrap();
        std::fs::write(config.join("identity.json"), "{}").unwrap();
        assert!(!is_frontier_store(&config));

        // The frontier shape: an events directory.
        let frontier = tmp.join("frontier/.vela");
        std::fs::create_dir_all(frontier.join("events")).unwrap();
        assert!(is_frontier_store(&frontier));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
