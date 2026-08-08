//! The one output contract.
//!
//! Every porcelain verb speaks through this module, so the surface can
//! only be coherent: one header shape, one aligned key-value block, one
//! error grammar with an exit-code contract, and one guarantee — under
//! `--json`, EVERY outcome (including every failure) is a single JSON
//! object with `{ok, command, ...}` and the process exit code tells the
//! truth. Dispatch arms call [`set_mode`] once; everything downstream
//! (including deep `fail_*` sites) inherits the right behavior without
//! threading flags.
//!
//! Exit-code contract (research: gh/clig.dev structured-error pattern —
//! an agent that knows WHY a call failed can self-correct):
//!   0 ok · 1 domain failure (gate red, verify fail) · 2 usage ·
//!   3 not found · 4 custody/permission refused · 5 already exists
//!   (idempotent no-op).
//!
//! Advice (the `hint:` line naming the next command) is a first-class
//! part of every error, and togglable: `VELA_ADVICE=0` or `set_quiet`
//! silences hints without touching the message (git's advice.* pattern).

use std::sync::Mutex;

use crate::style;
use colored::Colorize;
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Wrong invocation: missing/invalid arguments.
    Usage,
    /// The named object does not exist here.
    NotFound,
    /// Refused by the custody engine or a permission profile.
    Custody,
    /// Idempotent no-op: the thing already exists.
    Exists,
    /// The domain said no: gate red, verification failed, replay broken.
    Domain,
    /// Our fault: unexpected internal failure.
    #[allow(dead_code)] // part of the published contract; no CLI path is honestly internal yet
    Internal,
}

impl ErrorKind {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Domain | Self::Internal => 1,
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::Custody => 4,
            Self::Exists => 5,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::NotFound => "not_found",
            Self::Custody => "custody_refused",
            Self::Exists => "already_exists",
            Self::Domain => "domain",
            Self::Internal => "internal",
        }
    }
}

/// Every stable code this binary may put in `error.code`, and the whole list.
///
/// `kind` above is the six-value class that chooses the exit code. `message` is
/// prose, and `tests/wording_contract.rs` says outright what its contract is:
/// "the replacement is a matter of taste and will change; that these words stay
/// out of it is the contract". Between the two there was nothing a caller could
/// key on, so the only thing distinguishing one failure from another was the
/// one field this project had already promised to churn. Something downstream
/// was already paying for it — the Observatory derived an HTTP status by
/// substring-matching a thrown sentence, and got it wrong.
///
/// A code says what the kind cannot. It is present when this binary knows
/// something more specific than the class, and null otherwise; a null code is
/// not an unclassified failure, it is one where the class is the whole story.
///
/// The list is short and should stay short. Adding a code is additive and needs
/// no coordination. Removing or renaming one breaks every caller that branched
/// on it, which is precisely the promise `message` does not make, so the two
/// fields are governed differently on purpose. `wording_contract.rs` holds the
/// emitted set to this list.
pub const ERROR_CODES: &[&str] = &[
    // A retained file a command was told to read. `bounded_file` distinguishes
    // twelve causes; the submit preflight chose an `ErrorKind` from one of them
    // and discarded the rest, so a caller could not learn whether the artifact
    // it named was absent, oversized, or a symlink.
    "file_missing",
    "file_not_regular",
    "file_symlink",
    "file_oversized",
    "file_path_escape",
    "file_path_invalid",
    "file_path_changed",
    "file_unreadable",
    // The four distinct refusals of the current-repository gate, which reported
    // three of them as one `ErrorKind::Domain` and one sentence apiece.
    "repository_missing",
    "repository_authority_uninitialized",
    "repository_incomplete",
    "repository_predecessor_layout",
];

/// The per-invocation output mode, set once by the dispatch arm. A CLI
/// process runs exactly one command, so a process-global is the honest
/// scope (and lets `fail_*` sites deep in call stacks emit correctly
/// without threading `json` through every signature).
struct Mode {
    command: String,
    json: bool,
}

static MODE: Mutex<Option<Mode>> = Mutex::new(None);
/// Quiet is set at parse time, before any command mode exists.
static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_mode(command: &str, json: bool) {
    *MODE.lock().unwrap() = Some(Mode {
        command: command.to_string(),
        json,
    });
}

pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, std::sync::atomic::Ordering::Relaxed);
}

fn mode() -> (String, bool) {
    MODE.lock()
        .unwrap()
        .as_ref()
        .map(|m| (m.command.clone(), m.json))
        .unwrap_or_else(|| (String::new(), false))
}

fn advice_enabled() -> bool {
    if QUIET.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    /* The `Err` arm is `true` on purpose, and it is why this is not
    `is_ok_and`: an unset VELA_ADVICE is `Err` and must mean advice on, so the
    shorter spelling would turn every environment that never set the variable
    into advice off — the whole hint surface disappearing silently. */
    std::env::var("VELA_ADVICE")
        .map(|value| !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "off"))
        .unwrap_or(true)
}

/// Report best-effort maintenance that failed after the semantic operation
/// already succeeded. Diagnostics use stderr in both human and JSON modes so
/// the command's single structured stdout result remains valid.
pub(crate) fn warn_nonfatal(message: &str) {
    eprintln!("warn · {}", crate::cli::safe_text::multiline(message));
}

/// Terminate with the one error grammar. Human mode:
/// `err · <message>` + optional `hint: <next command>`; JSON mode: a
/// single `{ok:false, command, error:{kind,code,message,hint}}` object on
/// stdout. Exit code from the kind, always.
pub fn fail_with(kind: ErrorKind, message: &str, hint: Option<&str>) -> ! {
    fail_coded(kind, None, message, hint)
}

/// [`fail_with`], naming which failure this is. The code is one of
/// [`ERROR_CODES`]; see that list for what the field promises and does not.
pub fn fail_coded(
    kind: ErrorKind,
    code: Option<&'static str>,
    message: &str,
    hint: Option<&str>,
) -> ! {
    debug_assert!(
        code.is_none_or(|code| ERROR_CODES.contains(&code)),
        "error code {code:?} is not declared in ERROR_CODES"
    );
    let (command, json) = mode();
    if json {
        let payload = json!({
            "ok": false,
            "command": if command.is_empty() { serde_json::Value::Null } else { json!(command) },
            "error": {
                "kind": kind.as_str(),
                "code": code,
                "message": message,
                "hint": hint,
            },
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
        );
    } else {
        let message = crate::cli::safe_text::multiline(message);
        eprintln!("{} {message}", style::err_prefix());
        if let Some(hint) = hint
            && advice_enabled()
        {
            eprintln!("  hint: {}", crate::cli::safe_text::inline(hint));
        }
    }
    std::process::exit(kind.exit_code());
}

/// Terminate a mutating request that was rejected before its first durable
/// write. Unlike [`fail_with`], this contract may state zero delta because the
/// caller has already proved that no canonical or Git mutation was attempted.
/// The retained operation id lets a human correlate the repaired retry without
/// implying that a transaction marker or scientific result exists.
pub fn fail_unchanged(kind: ErrorKind, message: &str, operation_id: &str, next_command: &str) -> ! {
    fail_unchanged_coded(kind, None, message, operation_id, next_command)
}

/// [`fail_unchanged`], naming which failure this is. The code is one of
/// [`ERROR_CODES`].
pub fn fail_unchanged_coded(
    kind: ErrorKind,
    code: Option<&'static str>,
    message: &str,
    operation_id: &str,
    next_command: &str,
) -> ! {
    debug_assert!(
        code.is_none_or(|code| ERROR_CODES.contains(&code)),
        "error code {code:?} is not declared in ERROR_CODES"
    );
    let (command, json) = mode();
    if json {
        let payload = json!({
            "ok": false,
            "command": if command.is_empty() { serde_json::Value::Null } else { json!(command) },
            "request_id": operation_id,
            "operation_id": operation_id,
            "changed": false,
            "retained": {
                "request_id": operation_id,
                "transaction_marker": false,
            },
            "next": next_command,
            "error": {
                "kind": kind.as_str(),
                "code": code,
                "message": message,
                "hint": next_command,
            },
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
        );
    } else {
        let message = crate::cli::safe_text::multiline(message);
        eprintln!("{} {message}", style::err_prefix());
        eprintln!(
            "  {:<16} canonical Vela state, Git refs, index, and worktree",
            style::dim("unchanged")
        );
        eprintln!(
            "  {:<16} {} (request only; no transaction marker)",
            style::dim("retained"),
            crate::cli::safe_text::inline(operation_id)
        );
        eprintln!(
            "  {:<16} {}",
            style::dim("next"),
            crate::cli::safe_text::inline(next_command)
        );
    }
    std::process::exit(kind.exit_code());
}

/// The house header: `VELA · CMD · subject  (note)` over a tick row.
/// Status set the style; every verb renders through here so no command
/// can drift into its own dialect again.
pub fn header(command: &str, subject: &str, note: Option<&str>) {
    println!();
    let command = crate::cli::safe_text::inline(command);
    let subject = crate::cli::safe_text::inline(subject);
    let mut line = format!("VELA · {command}");
    if !subject.is_empty() {
        line.push_str(&format!(" · {subject}"));
    }
    if let Some(note) = note {
        line.push_str(&format!("  ({})", crate::cli::safe_text::inline(note)));
    }
    println!("  {}", line.to_uppercase().dimmed());
    println!("  {}", style::tick_row(60));
}

/// Resolve the repository argument: an explicit path wins; otherwise walk
/// upward from cwd for a repository-shaped `.vela` (the git discovery
/// pattern — `vela status` from anywhere inside a repository just works).
/// Discover a current repository or its native pre-authority bootstrap;
/// predecessor layouts require their pinned historical Vela release.
pub fn resolve_repo(explicit: Option<std::path::PathBuf>) -> std::path::PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    let Ok(mut cur) = std::env::current_dir() else {
        fail_with(
            ErrorKind::Usage,
            "cannot resolve the current directory",
            None,
        );
    };
    let started = cur.clone();
    loop {
        let store = cur.join(".vela");
        let current =
            store.join("origin.json").is_file() && store.join("repository.json").is_file();
        let bootstrap = store.is_dir()
            && cur.join("vela.toml").is_file()
            && !store.join("origin.json").exists()
            && !store.join("repository.json").exists();
        if current || bootstrap {
            return cur;
        }
        if !cur.pop() {
            fail_with(
                ErrorKind::NotFound,
                &format!(
                    "no repository found from {} up to the filesystem root",
                    started.display()
                ),
                Some("run `vela init` to create one here, or pass a path: `vela status <dir>`"),
            );
        }
    }
}

/// Canonicalize a repository path the user typed, keeping the kind the failure
/// actually has. A path that is not there is a not-found, which the exit-code
/// contract distinguishes from a domain failure; without this the same missing
/// directory exits 3 through [`require_initialized_repo`] and 1 through the
/// verbs that canonicalize first. Every other io failure stays Domain because
/// this call cannot tell a permission refusal from a broken repository.
pub fn canonicalize_repo(repository_path: &std::path::Path) -> std::path::PathBuf {
    repository_path.canonicalize().unwrap_or_else(|error| {
        let kind = if error.kind() == std::io::ErrorKind::NotFound {
            ErrorKind::NotFound
        } else {
            ErrorKind::Domain
        };
        fail_with(
            kind,
            &format!(
                "resolve current repository {}: {error}",
                repository_path.display()
            ),
            None,
        )
    })
}

/// Refuse commands that require an initialized current repository with one
/// phase-aware, actionable error. `status` and resumable `init` deliberately do
/// not call this helper because they are the two valid bootstrap operations.
///
/// Four refusals, three of which shared one `ErrorKind::Domain` and were told
/// apart only by their sentence. Resuming an interrupted `vela init`, restoring
/// a file someone deleted, and reaching for a predecessor layout this release
/// does not read are three different situations with three different next
/// moves, so each names itself.
pub fn require_initialized_repo(repository_path: &std::path::Path) {
    let store = repository_path.join(".vela");
    let origin = store.join("origin.json");
    let repository = store.join("repository.json");
    if origin.is_file() && repository.is_file() {
        return;
    }
    if !repository_path.is_dir() {
        fail_coded(
            ErrorKind::NotFound,
            Some("repository_missing"),
            &format!(
                "repository directory does not exist: {}",
                repository_path.display()
            ),
            Some("pass an existing repository path, or run `vela init <dir>`"),
        );
    }
    if store.is_dir()
        && repository_path.join("vela.toml").is_file()
        && !origin.exists()
        && !repository.exists()
    {
        let next = format!("vela init '{}' --json", repository_path.display());
        fail_coded(
            ErrorKind::Domain,
            Some("repository_authority_uninitialized"),
            "repository authority is not initialized",
            Some(&next),
        );
    }
    if origin.exists() || repository.exists() {
        fail_coded(
            ErrorKind::Domain,
            Some("repository_incomplete"),
            "repository is incomplete: expected both `.vela/origin.json` and `.vela/repository.json`",
            Some("restore the exact missing tracked file, then run `vela replay`"),
        );
    }
    fail_coded(
        ErrorKind::Domain,
        Some("repository_predecessor_layout"),
        "this Vela release verifies only current repository origins",
        Some(
            "inspect a predecessor with its pinned historical Vela release; create new work with `vela init <dir>`",
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_contract() {
        assert_eq!(ErrorKind::Domain.exit_code(), 1);
        assert_eq!(ErrorKind::Usage.exit_code(), 2);
        assert_eq!(ErrorKind::NotFound.exit_code(), 3);
        assert_eq!(ErrorKind::Custody.exit_code(), 4);
        assert_eq!(ErrorKind::Exists.exit_code(), 5);
    }

    #[test]
    fn mode_roundtrip() {
        set_mode("status", true);
        assert!(mode().1);
        set_mode("status", false);
        assert!(!mode().1);
    }
}
