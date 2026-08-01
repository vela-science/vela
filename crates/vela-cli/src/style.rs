//! CLI-only output styling.
//!
//! The protocol crate owns canonical scientific objects. Presentation belongs
//! here, at the application edge. All ANSI output is disabled for non-TTY
//! stdout and when `NO_COLOR` is set.

use colored::{ColoredString, Colorize};
use std::io::IsTerminal;
use std::sync::Once;

pub const MOSS: (u8, u8, u8) = (0x3F, 0x6B, 0x4E);
pub const MADDER: (u8, u8, u8) = (0x8A, 0x3A, 0x3A);

static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        if std::env::var_os("NO_COLOR").is_some() || !std::io::stdout().is_terminal() {
            colored::control::set_override(false);
        }
    });
}

#[must_use]
pub fn dim(value: &str) -> ColoredString {
    value.dimmed()
}

#[must_use]
pub fn tick_row(width: usize) -> String {
    format!("{}", "·".repeat(width).dimmed())
}

#[must_use]
pub fn moss(value: impl AsRef<str>) -> ColoredString {
    let (r, g, b) = MOSS;
    value.as_ref().truecolor(r, g, b)
}

#[must_use]
pub fn chip(label: &str, rgb: (u8, u8, u8)) -> String {
    let dot = "·".truecolor(rgb.0, rgb.1, rgb.2);
    let text = label.truecolor(rgb.0, rgb.1, rgb.2);
    format!("{dot} {text}")
}

#[must_use]
pub fn ok(label: &str) -> String {
    chip(label, MOSS)
}

#[must_use]
pub fn err_prefix() -> String {
    let (r, g, b) = MADDER;
    format!("{}", "err ·".truecolor(r, g, b))
}
