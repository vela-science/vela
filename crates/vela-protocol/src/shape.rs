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
pub fn is_lower_hex_64(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether `value` is a full Vela root: lowercase `sha256:` plus
/// [`is_lower_hex_64`].
pub fn is_full_sha256_root(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_lower_hex_64)
}

/// Whether `byte` is a lowercase hexadecimal digit.
pub fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// The standard that defines one current Repository's non-security identity.
pub const REPOSITORY_ID_CONTRACT: &str = "rfc9562-uuidv4";

/// Whether `value` is lowercase canonical RFC 9562 UUIDv4 text.
///
/// Repository identity is deliberately standard and opaque. It is not a
/// security root, timestamp, host identity, or substitute for the independently
/// obtained sequence-one authority root.
pub fn is_repository_id(value: &str) -> bool {
    let Ok(parsed) = uuid::Uuid::parse_str(value) else {
        return false;
    };
    parsed.get_version() == Some(uuid::Version::Random)
        && parsed.get_variant() == uuid::Variant::RFC4122
        && parsed.hyphenated().to_string() == value
}

/// Whether `value` is `prefix` followed by exactly `hex_len` lowercase
/// hexadecimal characters — the shape of every Vela identifier.
///
/// `vro_` takes 16, `vcl_` takes 64, and `sha256:` takes 64 through
/// [`is_full_sha256_root`]. Six implementations spelled the strip-and-measure
/// out in full across two crates and three different error sentences.
///
/// The two `require_prefixed_hex` helpers are deliberately not callers. They
/// distinguish a missing prefix from a malformed body and say so in two
/// different messages, which a predicate returning one boolean cannot do.
pub fn is_prefixed_lower_hex(value: &str, prefix: &str, hex_len: usize) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|hex| hex.len() == hex_len && hex.bytes().all(is_lower_hex))
}

/// The hexadecimal width of a derived display handle.
pub const HANDLE_HEX_LEN: usize = 16;

/// Derive the readable handle for an object from its full root.
///
/// A handle is a prefix of a root and nothing else. It is not stored inside
/// the object it names — an object cannot contain its own content address —
/// and where one appears in a reference it sits beside the root it came from,
/// so a reader re-derives it rather than trusting it. That is the rule that
/// makes a truncated identifier safe to print: it is a rendering of an exact
/// value, never the value itself, and it can always be checked.
///
/// The `vsb_`, `vvr_`, `vpr_`, `vpw_` and `vro_` handles were each stored in
/// their own object, over a preimage that had to be reconstructed by clearing
/// the very field being derived. Deriving them here removes both the stored
/// field and the clearing convention.
pub fn derive_handle(prefix: &str, root: &str) -> Result<String, String> {
    let Some(hex) = root.strip_prefix("sha256:") else {
        return Err(format!("cannot derive `{prefix}` handle from {root}"));
    };
    if !is_lower_hex_64(hex) {
        return Err(format!("cannot derive `{prefix}` handle from {root}"));
    }
    Ok(format!("{prefix}{}", &hex[..HANDLE_HEX_LEN]))
}

/// Require a stored reference handle to be the one its root derives.
///
/// A handle that disagrees with the root beside it is the ambiguity a
/// truncated identifier invites, and it fails here rather than resolving to
/// whichever object a scan happens to reach first.
pub(crate) fn require_derived_handle(
    name: &str,
    value: &str,
    prefix: &str,
    root: &str,
) -> Result<(), String> {
    let expected = derive_handle(prefix, root)
        .map_err(|_| format!("{name} cannot be checked against a malformed root"))?;
    if value != expected {
        return Err(format!(
            "{name} must be {expected}, the handle its root derives, not {value}"
        ));
    }
    Ok(())
}

/// A full `sha256:` root, named for the caller's field.
///
/// Three kernel modules carried this character for character — body and error
/// prose alike — so unlike the per-object helpers, which differ in prose on
/// purpose, these really were one function written three times.
pub(crate) fn require_sha256_root(name: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{name} must use a full sha256: digest"));
    };
    if !is_lower_hex_64(hex) {
        return Err(format!(
            "{name} must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
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
    fn repository_identity_is_canonical_rfc_9562_uuidv4() {
        assert!(is_repository_id("01234567-89ab-4def-8123-456789abcdef"));
        assert!(!is_repository_id("01234567-89AB-4DEF-8123-456789ABCDEF"));
        assert!(!is_repository_id("0123456789ab4def8123456789abcdef"));
        assert!(!is_repository_id("vrepo_0123456789abcdef0123456789abcdef"));
        assert!(!is_repository_id("01234567-89ab-7def-8123-456789abcdef"));
        assert!(!is_repository_id("01234567-89ab-4def-7123-456789abcdef"));
        assert!(!is_repository_id(
            "urn:uuid:01234567-89ab-4def-8123-456789abcdef"
        ));
    }

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

    /// A corpus wide enough to separate the rules below from each other.
    fn corpus() -> Vec<String> {
        let hex64 = "a".repeat(64);
        let mut values: Vec<String> = [
            "",
            " ",
            "  ",
            "a",
            "Alice",
            " leading",
            "trailing ",
            " both ",
            "inner space",
            "sha256:",
            "SHA256:",
            "0123456789abcdef",
            "vsb_0123456789abcdef",
            "agent:fixture",
            "ci:runner",
            "agent:",
            "artifacts/result.json",
            "../escape",
            "/absolute",
        ]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
        values.push(hex64.clone());
        values.push(hex64.to_uppercase());
        values.push("a".repeat(63));
        values.push("a".repeat(65));
        values.push("g".repeat(64));
        values.push(format!("sha256:{hex64}"));
        values.push(format!("sha256:{}", hex64.to_uppercase()));
        values.push(format!("sha256:{}", "a".repeat(63)));
        values.push(format!("SHA256:{hex64}"));
        values.push("a".repeat(128));
        values.push("a".repeat(127));
        values.push("Al\nice".to_string());
        values.push("tab\there".to_string());
        values
    }

    /// Hold each published wire pattern against the predicate it stands for.
    ///
    /// The patterns in `crate::wire_schema` are the only place a wire rule is
    /// written twice — once as a Rust predicate that runs, once as a regular
    /// expression that ships to implementers. Nothing in the generated schema
    /// can notice if the two stop agreeing, because the schema is generated
    /// from the regular expression. This is that check.
    #[test]
    fn wire_patterns_agree_with_predicates() {
        use crate::wire_schema::{
            ED25519_SIGNATURE_PATTERN, LOWER_HEX_64_PATTERN, SHA256_ROOT_PATTERN,
        };

        let signature_predicate = |value: &str| {
            hex::decode(value).is_ok_and(|bytes| bytes.len() == 64)
                && value.bytes().all(is_lower_hex)
        };
        /// A published pattern and the predicate it is the wire spelling of.
        type WirePattern<'a> = (&'a str, &'a dyn Fn(&str) -> bool);

        let cases: [WirePattern; 3] = [
            (SHA256_ROOT_PATTERN, &is_full_sha256_root),
            (LOWER_HEX_64_PATTERN, &is_lower_hex_64),
            (ED25519_SIGNATURE_PATTERN, &signature_predicate),
        ];
        for (pattern, predicate) in cases {
            let compiled = regex::Regex::new(pattern).expect("wire pattern compiles");
            for value in corpus() {
                assert_eq!(
                    compiled.is_match(&value),
                    predicate(&value),
                    "`{pattern}` and its predicate disagree about {value:?}"
                );
            }
        }
    }

    /// The text pattern is deliberately weaker than `require_text`, in exactly
    /// one direction.
    ///
    /// `^\S(?:[\s\S]*\S)?$` cannot express "no interior control character",
    /// so the wire schema admits a string the Rust reader then rejects. That
    /// asymmetry is safe — the reader is the authority, and it is stricter —
    /// but it must stay one-directional: anything the reader accepts, the
    /// published schema must also accept, or a valid object would fail
    /// validation somewhere in the ecosystem.
    #[test]
    fn text_pattern_never_rejects_what_the_reader_accepts() {
        let compiled =
            regex::Regex::new(crate::wire_schema::TRIMMED_TEXT_PATTERN).expect("pattern compiles");
        let mut saw_the_known_gap = false;
        for value in corpus() {
            let reader_accepts = require_bounded_text("field", &value, 16 * 1024).is_ok()
                && value == value.trim()
                && !value.is_empty();
            if reader_accepts {
                assert!(
                    compiled.is_match(&value),
                    "the reader accepts {value:?} but the published pattern rejects it"
                );
            } else if compiled.is_match(&value) {
                saw_the_known_gap = true;
            }
        }
        assert!(
            saw_the_known_gap,
            "the corpus no longer exercises the interior-control-character gap"
        );
    }
}
