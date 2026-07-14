//! Bounded terminal rendering for untrusted scientific and repository text.
//!
//! Canonical bytes remain untouched. This module is only a presentation
//! boundary: it makes terminal controls visible, prevents bidi or invisible
//! separators from changing what a reviewer sees, and gives truncated values a
//! stable digest reference to their complete input.

use sha2::{Digest, Sha256};

pub(crate) const DEFAULT_MAX_BYTES: usize = 4096;
pub(crate) const DEFAULT_MAX_SCALARS: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeText {
    pub text: String,
    pub truncated: bool,
    pub full_sha256: String,
    pub original_bytes: usize,
    pub original_scalars: usize,
}

pub(crate) fn inline(input: &str) -> String {
    render_with_limits(input, DEFAULT_MAX_BYTES, DEFAULT_MAX_SCALARS, false).text
}

pub(crate) fn multiline(input: &str) -> String {
    render_with_limits(input, DEFAULT_MAX_BYTES, DEFAULT_MAX_SCALARS, true).text
}

pub(crate) fn render_with_limits(
    input: &str,
    max_bytes: usize,
    max_scalars: usize,
    preserve_newlines: bool,
) -> SafeText {
    let full_sha256 = hex::encode(Sha256::digest(input.as_bytes()));
    let original_bytes = input.len();
    let original_scalars = input.chars().count();

    let escaped_bytes = input
        .chars()
        .map(|ch| escaped_segment(ch, preserve_newlines).len())
        .sum::<usize>();
    let escaped_scalars = input
        .chars()
        .map(|ch| escaped_segment(ch, preserve_newlines).chars().count())
        .sum::<usize>();

    if escaped_bytes <= max_bytes && escaped_scalars <= max_scalars {
        let text = input
            .chars()
            .map(|ch| escaped_segment(ch, preserve_newlines))
            .collect::<String>();
        return SafeText {
            text,
            truncated: false,
            full_sha256,
            original_bytes,
            original_scalars,
        };
    }

    let suffix = format!("… [truncated; sha256:{full_sha256}]");
    let suffix_bytes = suffix.len();
    let suffix_scalars = suffix.chars().count();
    let prefix_byte_budget = max_bytes.saturating_sub(suffix_bytes);
    let prefix_scalar_budget = max_scalars.saturating_sub(suffix_scalars);
    let suffix = fit_to_limits(&suffix, max_bytes, max_scalars);

    let mut text = String::new();
    let mut used_scalars = 0usize;
    for ch in input.chars() {
        let segment = escaped_segment(ch, preserve_newlines);
        let next_bytes = text.len() + segment.len();
        let segment_scalars = segment.chars().count();
        if next_bytes > prefix_byte_budget || used_scalars + segment_scalars > prefix_scalar_budget
        {
            break;
        }
        text.push_str(&segment);
        used_scalars += segment_scalars;
    }
    text.push_str(&suffix);

    SafeText {
        text,
        truncated: true,
        full_sha256,
        original_bytes,
        original_scalars,
    }
}

fn fit_to_limits(input: &str, max_bytes: usize, max_scalars: usize) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if out.len() + ch.len_utf8() > max_bytes || out.chars().count() + 1 > max_scalars {
            break;
        }
        out.push(ch);
    }
    out
}

fn escaped_segment(ch: char, preserve_newlines: bool) -> String {
    match ch {
        '\n' if preserve_newlines => "\n".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c if must_escape(c) => format!("\\u{{{:04X}}}", c as u32),
        c => c.to_string(),
    }
}

fn must_escape(ch: char) -> bool {
    let cp = ch as u32;
    matches!(
        cp,
        0x0000..=0x001F | 0x007F..=0x009F | 0x2028..=0x2029
    ) || is_default_ignorable(cp)
        || is_format_control(cp)
}

/// Unicode 17.0 General_Category=Cf controls not already guaranteed by the
/// default-ignorable property. Some are legitimate script controls, but they
/// remain invisible or reorder glyphs in a terminal review surface, so render
/// them visibly rather than interpreting them.
///
/// Source: <https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt>
fn is_format_control(cp: u32) -> bool {
    matches!(
        cp,
        0x0600..=0x0605
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
    )
}

/// Unicode 17.0 `Default_Ignorable_Code_Point`, frozen here so terminal
/// rendering does not change when the Rust toolchain updates its Unicode
/// tables. This is deliberately the derived property rather than all combining
/// marks: visible marks such as U+0301 COMBINING ACUTE ACCENT must survive.
///
/// Source: <https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt>
fn is_default_ignorable(cp: u32) -> bool {
    matches!(
        cp,
        0x00AD
            | 0x034F
            | 0x061C
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFF8
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_controls_and_bidi_are_visible() {
        let rendered =
            inline("claim\u{1b}]8;;https://bad.example\u{7}link\u{1b}]8;;\u{7}\u{202e}txt\r");
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\u{7}'));
        assert!(!rendered.contains('\u{202e}'));
        assert!(!rendered.contains('\r'));
        assert!(rendered.contains("\\u{001B}"), "{rendered}");
        assert!(rendered.contains("\\u{202E}"), "{rendered}");
        assert!(rendered.ends_with("\\r"), "{rendered}");
    }

    #[test]
    fn multiline_preserves_only_line_feed() {
        let rendered = multiline("a\nb\tc\r");
        assert_eq!(rendered, "a\nb\\tc\\r");
    }

    #[test]
    fn truncation_is_bounded_and_names_the_full_digest() {
        let input = "α".repeat(400);
        let rendered = render_with_limits(&input, 160, 120, false);
        assert!(rendered.truncated);
        assert!(rendered.text.len() <= 160, "{}", rendered.text.len());
        assert!(rendered.text.chars().count() <= 120);
        assert!(rendered.text.contains(&rendered.full_sha256));
        assert_eq!(rendered.original_bytes, 800);
        assert_eq!(rendered.original_scalars, 400);
    }

    #[test]
    fn ordinary_text_is_unchanged() {
        let rendered = render_with_limits("bounded scientific claim", 256, 256, false);
        assert!(!rendered.truncated);
        assert_eq!(rendered.text, "bounded scientific claim");
    }

    #[test]
    fn default_ignorables_are_visible() {
        assert_eq!(inline("a\u{00AD}b"), "a\\u{00AD}b");
        assert_eq!(inline("a\u{034F}b"), "a\\u{034F}b");
        assert_eq!(inline("a\u{FE0F}b"), "a\\u{FE0F}b");
        assert_eq!(inline("a\u{E0020}b"), "a\\u{E0020}b");
        assert_eq!(inline("a\u{FFF9}b"), "a\\u{FFF9}b");
        assert_eq!(inline("a\u{0600}b"), "a\\u{0600}b");
    }

    #[test]
    fn visible_combining_marks_are_preserved() {
        let input = "e\u{0301}";
        assert_eq!(inline(input), input);
    }
}
