//! Shared interactive-input primitives — one place, so every prompt reads
//! stdin the same way and callers guard with [`crate::ui::ensure_can_prompt`]
//! before invoking (never a raw prompt that hangs on piped stdin).
//!
//! There is deliberately no TUI here: `docs/CLI.md` — "no interactive app
//! and no TUI: the interactivity of this era belongs to the agent, and the
//! pen belongs to you." If a picker is ever needed *outside* the signing
//! ceremony, it is a small numeric `select_one`, no raw mode, no dep.

use std::io::{BufRead, Write};

/// Print `prompt`, read one line, return it trimmed (case preserved so a
/// capital `A` reaches a match). Empty on EOF/error — callers that must
/// not default should `ensure_can_prompt` first.
pub(crate) fn read_line(prompt: &str) -> String {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

/// A yes/no confirm: only `y`/`yes` (any case) is yes; everything else,
/// including EOF, is no.
pub(crate) fn confirm(prompt: &str) -> bool {
    matches!(read_line(prompt).to_lowercase().as_str(), "y" | "yes")
}
