//! The pinned JCS implementation, held to RFC 8785.
//!
//! `canonical_hashing_conformance.rs` pins *Vela's* canonical layer to the
//! portable vector file. This pins the dependency underneath it: that
//! `serde_json_canonicalizer` produces the exact strings RFC 8785 specifies,
//! that it orders property names by UTF-16 code unit, and that the strict
//! parser refuses the inputs a canonical form has no answer for.
//!
//! This file also carried a shadow audit — a frozen copy of the pre-JCS Vela
//! canonicalizer, re-run over four retained repository checkouts to measure
//! where a naive encoder and RFC 8785 disagree. That audit ran and its result
//! is recorded: `conformance/jcs-shadow-audit.json` holds the result root, the
//! canonicalizer version, and the per-repository file and match counts, and
//! ADR 0035 cites it. The harness required all four repositories present at
//! exact commits and trees, and ADR 0039 archived all four read-only, so it
//! could no longer run for anyone. A recorded measurement does not need its
//! instrument kept in the test suite.

use serde::Serialize;
use serde_json::Value;
use vela_protocol::canonical::parse_json_value_strict;

fn parse_unique_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    parse_json_value_strict(bytes)
}

fn jcs_string<T: Serialize>(value: &T) -> String {
    String::from_utf8(serde_json_canonicalizer::to_vec(value).unwrap()).unwrap()
}

#[test]
fn pinned_jcs_implementation_matches_rfc_8785_samples() {
    let input = br#"{
        "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
        "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
        "literals": [null, true, false]
    }"#;
    let value = parse_unique_json(input).unwrap();
    assert_eq!(
        jcs_string(&value),
        r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
    );

    let number_cases = [
        (0x0000_0000_0000_0000, "0"),
        (0x8000_0000_0000_0000, "0"),
        (0x0000_0000_0000_0001, "5e-324"),
        (0x8000_0000_0000_0001, "-5e-324"),
        (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
        (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
        (0x4340_0000_0000_0000, "9007199254740992"),
        (0xc340_0000_0000_0000, "-9007199254740992"),
        (0x4430_0000_0000_0000, "295147905179352830000"),
        (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
        (0x44b5_2d02_c7e1_4af6, "1e+23"),
        (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
        (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
        (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
        (0x444b_1ae4_d6e2_ef50, "1e+21"),
        (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
        (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
        (0x41b3_de43_5555_5553, "333333333.3333332"),
        (0x41b3_de43_5555_5554, "333333333.33333325"),
        (0x41b3_de43_5555_5555, "333333333.3333333"),
        (0x41b3_de43_5555_5556, "333333333.3333334"),
        (0x41b3_de43_5555_5557, "333333333.33333343"),
        (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
    ];
    for (bits, expected) in number_cases {
        assert_eq!(
            jcs_string(&f64::from_bits(bits)),
            expected,
            "bits={bits:016x}"
        );
    }
    assert!(serde_json_canonicalizer::to_vec(&f64::NAN).is_err());
    assert!(serde_json_canonicalizer::to_vec(&f64::INFINITY).is_err());
    assert!(serde_json_canonicalizer::to_vec(&f64::NEG_INFINITY).is_err());
}

#[test]
fn pinned_jcs_implementation_sorts_property_names_by_utf16() {
    let value = serde_json::json!({
        "\u{20ac}": "Euro Sign",
        "\r": "Carriage Return",
        "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "\u{1f600}": "Emoji: Grinning Face",
        "\u{0080}": "Control",
        "\u{00f6}": "Latin Small Letter O With Diaeresis"
    });
    let canonical = jcs_string(&value);
    let values = [
        "Carriage Return",
        "One",
        "Control",
        "Latin Small Letter O With Diaeresis",
        "Euro Sign",
        "Emoji: Grinning Face",
        "Hebrew Letter Dalet With Dagesh",
    ];
    let positions = values.map(|value| canonical.find(value).expect("RFC 8785 sample value"));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn raw_json_rejects_duplicate_properties_and_invalid_inputs() {
    assert!(parse_unique_json(br#"{"a":1,"a":2}"#).is_err());
    assert!(parse_unique_json(br#"{"outer":{"a":1,"a":2}}"#).is_err());
    assert!(parse_unique_json(br#"{"value":"\ud800"}"#).is_err());
    assert!(parse_unique_json(br#"{"value":NaN}"#).is_err());
    assert!(parse_unique_json(br#"{"value":Infinity}"#).is_err());
    assert!(parse_unique_json(br#"{"value":-Infinity}"#).is_err());
    assert!(parse_unique_json(br#"{"value":1e400}"#).is_err());
}
