//! Canonical JSON serialization for content-addressed Vela primitives.
//!
//! Every content-addressed ID in the protocol — `vf_…` (finding),
//! `vev_…` (canonical event), `vpr_…` (proposal), the snapshot hash, the
//! event-log hash — is derived by hashing the canonical JSON bytes of a
//! value. For two implementations to produce identical IDs from the same
//! logical content, the serialization MUST be deterministic.
//!
//! Vela uses RFC 8785 JSON Canonicalization Scheme (JCS) rather than a local
//! approximation. Protocol numbers are additionally limited to the I-JSON
//! interoperable integer range before hashing. Conformance vectors at
//! `conformance/canonical-hashing.json` pin the exact bytes and SHA-256 roots.

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};

/// Parse a JSON byte slice without allowing duplicate object properties at
/// any depth, then deserialize the unique parsed value into `T`.
///
/// `serde_json` normally retains the last value for duplicate properties
/// inside open `Value` fields. Protocol objects cannot allow two different raw
/// inputs to collapse to the same parsed value before hashing or verification.
pub fn from_json_slice_strict<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_value(parse_json_value_strict(bytes)?)
}

/// Parse strict JSON into a `Value`, rejecting duplicate properties
/// recursively before callers canonicalize or hash it.
pub fn parse_json_value_strict(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueJson::deserialize(&mut deserializer)?.0;
    deserializer.end()?;
    Ok(value)
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object properties")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| UniqueJson(Value::Number(number)))
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = BTreeSet::new();
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "duplicate JSON property `{key}`"
                )));
            }
            values.insert(key, object.next_value::<UniqueJson>()?.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

/// Serialize `value` to canonical JSON bytes.
///
/// Returns the canonical UTF-8 byte representation of the value, suitable
/// for hashing into a content-addressed ID. Two callers serializing the
/// same logical value produce byte-identical output.
///
/// # Errors
///
/// Returns an error if `value` cannot be serialized as JSON, contains
/// non-finite floats (NaN, ±Inf), or is otherwise invalid JSON.
pub fn to_canonical_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("canonical: serialize to value failed: {e}"))?;
    validate_i_json(&value, "$")?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|e| format!("canonical: RFC 8785 serialization failed: {e}"))
}

/// String version of `to_canonical_bytes` for callers that want UTF-8 text.
///
/// Always valid UTF-8 because the bytes are JSON.
pub fn to_canonical_string<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let bytes = to_canonical_bytes(value)?;
    String::from_utf8(bytes).map_err(|e| format!("canonical: invalid utf-8: {e}"))
}

const MAX_I_JSON_INTEGER: u64 = 9_007_199_254_740_991;

fn validate_i_json(value: &serde_json::Value, path: &str) -> Result<(), String> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                validate_i_json(nested, &format!("{path}.{key}"))?;
            }
        }
        serde_json::Value::Array(values) => {
            for (index, nested) in values.iter().enumerate() {
                validate_i_json(nested, &format!("{path}[{index}]"))?;
            }
        }
        serde_json::Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                if integer.unsigned_abs() > MAX_I_JSON_INTEGER {
                    return Err(format!(
                        "canonical: integer at {path} exceeds the RFC 8785 I-JSON range"
                    ));
                }
            } else if let Some(integer) = number.as_u64() {
                if integer > MAX_I_JSON_INTEGER {
                    return Err(format!(
                        "canonical: integer at {path} exceeds the RFC 8785 I-JSON range"
                    ));
                }
            } else if number.as_f64().is_some_and(|value| !value.is_finite()) {
                return Err(format!("canonical: non-finite number at {path}"));
            }
        }
        _ => {}
    }
    Ok(())
}

/// SHA-256 of the canonical bytes, returned as lowercase hex.
///
/// The single function used everywhere the protocol derives a
/// content-addressed ID. Replaces every ad-hoc
/// `serde_json::to_vec(...) + Sha256::digest(...)` pattern in the kernel.
pub fn sha256_canonical<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = to_canonical_bytes(value)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, PartialEq)]
    struct OpenProtocolObject {
        schema: String,
        extension: Value,
    }

    #[test]
    fn strict_json_rejects_duplicate_properties_recursively() {
        let top_level =
            from_json_slice_strict::<Value>(br#"{"schema":"one","schema":"two"}"#).unwrap_err();
        assert!(
            top_level
                .to_string()
                .contains("duplicate JSON property `schema`")
        );

        let nested = from_json_slice_strict::<OpenProtocolObject>(
            br#"{"schema":"fixture","extension":{"nested":{"value":1,"value":2}}}"#,
        )
        .unwrap_err();
        assert!(
            nested
                .to_string()
                .contains("duplicate JSON property `value`")
        );
    }

    #[test]
    fn strict_json_preserves_open_value_fields_without_duplicates() {
        let value = from_json_slice_strict::<OpenProtocolObject>(
            br#"{"schema":"fixture","extension":{"nested":{"value":1}}}"#,
        )
        .unwrap();
        assert_eq!(value.schema, "fixture");
        assert_eq!(value.extension["nested"]["value"], 1);
    }

    #[test]
    fn object_keys_sort_at_every_depth() {
        let unordered = json!({
            "z": 1,
            "a": {
                "y": 2,
                "b": 3,
            },
            "m": [{"q": 4, "p": 5}],
        });
        let canon = to_canonical_string(&unordered).unwrap();
        // Keys at every depth must appear in lexicographic order.
        assert_eq!(canon, r#"{"a":{"b":3,"y":2},"m":[{"p":5,"q":4}],"z":1}"#);
    }

    #[test]
    fn whitespace_is_stripped() {
        let v = json!({"key": "value"});
        let canon = to_canonical_string(&v).unwrap();
        assert!(!canon.contains(' '));
        assert!(!canon.contains('\n'));
    }

    #[test]
    fn array_order_is_preserved() {
        let v = json!([3, 1, 2]);
        let canon = to_canonical_string(&v).unwrap();
        assert_eq!(canon, "[3,1,2]");
    }

    #[test]
    fn unicode_strings_pass_through() {
        let v = json!({"text": "amyloid-β"});
        let canon = to_canonical_string(&v).unwrap();
        assert!(canon.contains("amyloid-β"));
    }

    #[test]
    fn property_names_use_rfc_8785_utf16_order() {
        let value = json!({"\u{e000}": 1, "\u{1f600}": 2});
        assert_eq!(to_canonical_string(&value).unwrap(), "{\"😀\":2,\"\":1}");
    }

    #[test]
    fn numbers_use_ecmascript_form_and_unsafe_integers_fail() {
        assert_eq!(
            to_canonical_string(&json!({"score": 1.0})).unwrap(),
            "{\"score\":1}"
        );
        assert!(to_canonical_bytes(&9_007_199_254_740_992_u64).is_err());
        assert!(to_canonical_bytes(&-9_007_199_254_740_992_i64).is_err());
    }

    #[test]
    fn same_logical_content_produces_same_bytes() {
        let a = json!({"x": 1, "y": 2});
        let b = json!({"y": 2, "x": 1});
        let bytes_a = to_canonical_bytes(&a).unwrap();
        let bytes_b = to_canonical_bytes(&b).unwrap();
        assert_eq!(bytes_a, bytes_b);
    }

    #[test]
    fn sha256_canonical_is_stable() {
        let a = json!({"hello": "world"});
        let h1 = sha256_canonical(&a).unwrap();
        let h2 = sha256_canonical(&a).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
