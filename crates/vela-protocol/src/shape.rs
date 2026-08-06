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

/// Parse an RFC3339 timestamp and require the one spelling Vela treats as
/// canonical: UTC, whole seconds, zero offset written `Z`.
///
/// One instant must have one byte sequence. The offset is checked on its own
/// rather than left to the round-trip, because `to_rfc3339_opts(Secs, true)`
/// substitutes `Z` only at zero offset and so re-serializes a non-zero offset
/// back to itself — `2026-08-05T08:00:00-04:00` would round-trip cleanly and
/// pass, giving the same instant as `2026-08-05T12:00:00Z` a second set of
/// canonical bytes. The round-trip that follows then catches everything else:
/// sub-second digits, and the zero-offset spelling `+00:00`.
pub(crate) fn parse_canonical_time(
    name: &str,
    value: &str,
) -> Result<DateTime<chrono::FixedOffset>, String> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|error| format!("{name} is not RFC3339: {error}"))?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(format!(
            "{name} must be UTC with a zero offset spelled Z, not offset {}",
            parsed.offset()
        ));
    }
    if parsed.to_rfc3339_opts(SecondsFormat::Secs, true) != value {
        return Err(format!(
            "{name} must be whole-second UTC RFC3339 spelled with Z"
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
    fn canonical_time_accepts_only_whole_second_z() {
        assert!(parse_canonical_time("recorded_at", "2026-08-05T12:00:00Z").is_ok());
        assert!(parse_canonical_time("recorded_at", "not a time").is_err());

        // Zero offset, but not the canonical spelling.
        let plus_zero = parse_canonical_time("recorded_at", "2026-08-05T12:00:00+00:00")
            .expect_err("+00:00 is not the canonical spelling of a zero offset");
        assert!(plus_zero.contains("spelled with Z"), "{plus_zero}");

        // Sub-second precision, in the otherwise canonical spelling.
        let subsecond = parse_canonical_time("recorded_at", "2026-08-05T12:00:00.500Z")
            .expect_err("sub-second precision is not whole-second");
        assert!(subsecond.contains("whole-second"), "{subsecond}");
    }

    #[test]
    fn canonical_time_rejects_a_non_zero_offset_that_round_trips() {
        // The instant of 2026-08-05T12:00:00Z, written at -04:00. This is a
        // whole-second value that re-serializes to exactly the bytes supplied,
        // so the round-trip alone admits it and one instant gets two canonical
        // spellings. The offset check is what rejects it.
        let shifted = "2026-08-05T08:00:00-04:00";
        assert_eq!(
            DateTime::parse_from_rfc3339(shifted)
                .unwrap()
                .to_rfc3339_opts(SecondsFormat::Secs, true),
            shifted,
            "the round-trip alone cannot distinguish this value"
        );
        assert_eq!(
            DateTime::parse_from_rfc3339(shifted).unwrap(),
            parse_canonical_time("recorded_at", "2026-08-05T12:00:00Z").unwrap(),
            "the two spellings name the same instant"
        );

        let error = parse_canonical_time("recorded_at", shifted)
            .expect_err("a non-zero offset is not canonical");
        assert!(error.contains("recorded_at"), "{error}");
        assert!(error.contains("-04:00"), "{error}");

        // Non-zero in the other direction, and a non-zero offset that also
        // carries sub-second digits.
        assert!(parse_canonical_time("recorded_at", "2026-08-05T17:30:00+05:30").is_err());
        assert!(parse_canonical_time("recorded_at", "2026-08-05T08:00:00.500-04:00").is_err());
    }

    #[test]
    fn bounded_text_rejects_empty_oversized_and_control() {
        assert!(require_bounded_text("name", "Alice", 32).is_ok());
        assert!(require_bounded_text("name", "", 32).is_err());
        assert!(require_bounded_text("name", "Alice", 4).is_err());
        assert!(require_bounded_text("name", "Al\nice", 32).is_err());
    }
}
