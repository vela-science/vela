//! The acceptance rules that every current object states about its own
//! fields, written down once.
//!
//! These predicates were each restated in eleven or more sibling modules. The
//! published contract ("full Vela SHA-256 roots are lowercase `sha256:` plus
//! exactly 64 hexadecimal characters") admits no per-object variation, so a
//! copy that drifts would put the binary out of step with its documentation
//! without failing any test. Callers keep their own error prose; only the rule
//! lives here.

use chrono::{DateTime, SecondsFormat};

/// Whether `digest` is exactly 64 lowercase hexadecimal characters — the
/// payload half of a full Vela root or a full `vcl_` Claim id, with the
/// prefix already stripped.
pub(crate) fn is_lower_hex_64(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether `value` is a full Vela root: lowercase `sha256:` plus
/// [`is_lower_hex_64`].
pub(crate) fn is_full_sha256_root(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_lower_hex_64)
}

/// Whether `byte` is a lowercase hexadecimal digit.
pub(crate) fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// Parse an RFC3339 timestamp and require that it re-serializes to exactly the
/// bytes supplied, at whole-second resolution.
///
/// A zero offset must be spelled `Z`. A non-zero offset that is already in its
/// canonical spelling is accepted; the error text calls that UTC, which
/// overstates what is checked. Behavior is preserved verbatim from the two
/// modules this was lifted out of, because tightening it would reject already
/// accepted history.
pub(crate) fn parse_canonical_time(
    name: &str,
    value: &str,
) -> Result<DateTime<chrono::FixedOffset>, String> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|error| format!("{name} is not RFC3339: {error}"))?;
    if parsed.to_rfc3339_opts(SecondsFormat::Secs, true) != value {
        return Err(format!(
            "{name} must use canonical whole-second UTC RFC3339"
        ));
    }
    Ok(parsed)
}

/// Reject empty, oversized, or control-bearing text.
pub(crate) fn require_bounded_text(name: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(|character| character.is_control())
    {
        return Err(format!(
            "{name} is empty, oversized, or contains control text"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_root_shape_is_exact() {
        let hex = "a".repeat(64);
        assert!(is_full_sha256_root(&format!("sha256:{hex}")));
        assert!(!is_full_sha256_root(&hex));
        assert!(!is_full_sha256_root(&format!("sha256:{}", "A".repeat(64))));
        assert!(!is_full_sha256_root(&format!("sha256:{}", "a".repeat(63))));
        assert!(!is_full_sha256_root(&format!("sha256:{}", "a".repeat(65))));
        assert!(!is_full_sha256_root(&format!("SHA256:{hex}")));
        assert!(!is_full_sha256_root(&format!("sha256:{}", "g".repeat(64))));
    }

    #[test]
    fn canonical_time_rejects_offsets_and_subseconds() {
        assert!(parse_canonical_time("recorded_at", "2026-08-05T12:00:00Z").is_ok());
        assert!(parse_canonical_time("recorded_at", "2026-08-05T12:00:00.500Z").is_err());
        assert!(parse_canonical_time("recorded_at", "2026-08-05T12:00:00+00:00").is_err());
        assert!(parse_canonical_time("recorded_at", "not a time").is_err());
    }

    #[test]
    fn bounded_text_rejects_empty_oversized_and_control() {
        assert!(require_bounded_text("name", "Alice", 32).is_ok());
        assert!(require_bounded_text("name", "", 32).is_err());
        assert!(require_bounded_text("name", "Alice", 4).is_err());
        assert!(require_bounded_text("name", "Al\nice", 32).is_err());
    }
}
