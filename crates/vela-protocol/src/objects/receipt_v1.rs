//! Lossless Receipt v1 parsing and typed read-only views.
//!
//! [`ReceiptV1`] owns the producer's raw [`serde_json::Value`]. It never
//! round-trips a rich receipt through a Rust mirror of the published schema,
//! so unknown top-level and nested extensions survive unchanged. Canonical
//! bytes and the receipt root bind that complete raw value.
//!
//! Receipt JSON is a descriptor, not a payload transport. The parser rejects
//! representations above these hard ceilings before they reach the write edge:
//!
//! - 8 MiB encoded JSON;
//! - 64 JSON container levels, counting the root as level one;
//! - 1 MiB for any single string or object key;
//! - 10,000 top-level artifact descriptors;
//! - 16 KiB for a path, URI, URL, locator, or reference string; and
//! - 65,536 object fields in total. Core fields consume this conservative
//!   structural budget, so unknown extension fields are necessarily below it.
//!
//! The parser does not open archives, follow locators, read artifacts, infer a
//! verifier result, or treat a receipt as acceptance.

use std::collections::HashSet;
use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};

const RECEIPT_V1_SCHEMA: &str = "vela.receipt.v1";
const MAX_RECEIPT_V1_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECEIPT_V1_DEPTH: usize = 64;
const MAX_RECEIPT_V1_STRING_BYTES: usize = 1024 * 1024;
const MAX_RECEIPT_V1_ARTIFACTS: usize = 10_000;
const MAX_RECEIPT_V1_LOCATOR_BYTES: usize = 16 * 1024;
const MAX_RECEIPT_V1_OBJECT_FIELDS: usize = 65_536;
const MAX_RECEIPT_V1_ARRAY_ELEMENTS: usize = 65_536;
const MAX_RECEIPT_V1_NODES: usize = 131_072;
const MAX_RECEIPT_V1_DSSE_BYTES: usize = 1024 * 1024;
/// Largest integer represented exactly by every IEEE-754 binary64 consumer.
///
/// Receipt v1's frozen open-extension contract permits finite decimal and
/// exponent numbers. Those values are retained and serialized with RFC 8785
/// JCS. Integral values outside this interoperable set are rejected instead of
/// being rounded differently by Rust, Python, and JavaScript readers.
const MAX_PORTABLE_JSON_INTEGER: u64 = (1_u64 << 53) - 1;
const MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES: usize = 512;
const MAX_RESTRICTED_ARTIFACT_KIND_BYTES: usize = 128;
const MAX_RESTRICTED_ARTIFACT_MEDIA_TYPE_BYTES: usize = 255;
const RECEIPT_BODY_BINDING_FIELD: &str = "vela:receipt_body";
const RECEIPT_PREDICATE_SCHEMA: &str = "vela.receipt.predicate.v1";
const NEUTRAL_RECEIPT_GENERATOR: &str = "vela-protocol/neutral-receipt-v1";
const NO_ACTIVE_POLICY_REF: &str = "urn:vela:policy:none";

/// Required fields remain byte-for-byte aligned with the published v1 schema.
const RECEIPT_V1_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "claim",
    "type",
    "replayability",
    "artifacts",
    "caveats",
    "verifier_runs",
    "environment",
    "provenance",
    "status",
    "machine",
    "acceptance",
    "distillation",
    "lineage",
    "contributors",
    "signature_identities",
    "attestation",
];

const CLAIM_TYPES: &[&str] = &[
    "computational",
    "theoretical",
    "empirical",
    "negative",
    "contradiction",
];
const REPLAYABILITY: &[&str] = &["exact", "bounded", "approximate", "unavailable", "unknown"];
const VERIFIER_OUTCOMES: &[&str] = &["pass", "fail", "error", "skipped", "unknown"];
const STATUS_KINDS: &[&str] = &[
    "draft",
    "emitted",
    "proposed",
    "runs",
    "minimal_sanity_check",
    "reported_metric_rederived",
    "full_reproduction",
    "landed_pending",
    "accepted",
    "rejected",
    "superseded",
    "retracted",
    "contested",
    "failed_reproduction",
];
const STATUS_AUTHORITIES: &[&str] = &["producer", "vela_landing", "human_key", "signed_policy"];
const CONTRIBUTOR_ROLES: &[&str] = &[
    "conceptualization",
    "data_curation",
    "formal_analysis",
    "funding_acquisition",
    "investigation",
    "methodology",
    "project_administration",
    "resources",
    "software",
    "supervision",
    "validation",
    "visualization",
    "writing_original_draft",
    "writing_review_editing",
    "machine_producer",
    "human_formalizer",
    "human_distiller",
    "reviewer",
    "acceptor",
    "profile_maintainer",
];
const IDENTITY_MECHANISMS: &[&str] = &[
    "sigstore_keyless_oidc",
    "sigstore_keyless_oidc_fixture",
    "ed25519_key_custody_ceremony",
    "ed25519_key_custody_ceremony_fixture",
];
const ACCEPTANCE_MECHANISM: &str = "accountable_scientific_steward_signoff";
const INTOTO_STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
const VELA_PREDICATE_TYPE: &str = "https://vela.science/receipt/v1";
const INTOTO_PAYLOAD_TYPE: &str = "application/vnd.in-toto+json";

#[derive(Debug, Clone, Copy)]
struct ReceiptLimits {
    bytes: usize,
    depth: usize,
    string_bytes: usize,
    artifacts: usize,
    locator_bytes: usize,
    object_fields: usize,
    array_elements: usize,
    nodes: usize,
    dsse_bytes: usize,
}

const LIMITS: ReceiptLimits = ReceiptLimits {
    bytes: MAX_RECEIPT_V1_BYTES,
    depth: MAX_RECEIPT_V1_DEPTH,
    string_bytes: MAX_RECEIPT_V1_STRING_BYTES,
    artifacts: MAX_RECEIPT_V1_ARTIFACTS,
    locator_bytes: MAX_RECEIPT_V1_LOCATOR_BYTES,
    object_fields: MAX_RECEIPT_V1_OBJECT_FIELDS,
    array_elements: MAX_RECEIPT_V1_ARRAY_ELEMENTS,
    nodes: MAX_RECEIPT_V1_NODES,
    dsse_bytes: MAX_RECEIPT_V1_DSSE_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptV1Error {
    path: String,
    message: String,
}

impl ReceiptV1Error {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ReceiptV1Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "receipt v1 {}: {}", self.path, self.message)
    }
}

impl std::error::Error for ReceiptV1Error {}

/// A validated Receipt v1 retaining the complete producer JSON value.
///
/// This type intentionally does not implement `Deserialize`: a generic Serde
/// entry point cannot enforce the encoded-byte limit. Use [`Self::parse`] for
/// untrusted bytes; protocol-owned builders use a crate-private value path.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiptV1 {
    value: Value,
}

impl ReceiptV1 {
    pub fn parse(bytes: &[u8]) -> Result<Self, ReceiptV1Error> {
        Self::parse_with_limits(bytes, LIMITS)
    }

    fn parse_with_limits(bytes: &[u8], limits: ReceiptLimits) -> Result<Self, ReceiptV1Error> {
        if bytes.len() > limits.bytes {
            return Err(error(
                "$",
                format!(
                    "encoded JSON is {} bytes; limit is {} bytes",
                    bytes.len(),
                    limits.bytes
                ),
            ));
        }
        let value = decode_bounded_json(bytes, limits, "$", false)?;
        Self::from_value_with_limits(value, limits)
    }

    /// Construct from a value that was created inside the protocol crate.
    /// Untrusted callers must use [`Self::parse`] so duplicate object names
    /// cannot be erased before validation.
    pub(crate) fn from_trusted_value(value: Value) -> Result<Self, ReceiptV1Error> {
        Self::from_value_with_limits(value, LIMITS)
    }

    fn from_value_with_limits(value: Value, limits: ReceiptLimits) -> Result<Self, ReceiptV1Error> {
        validate_limits(&value, limits)?;
        validate_portable_numeric_domain(&value, "$")?;
        validate_schema_exact(&value)?;
        validate_semantics(&value, limits)?;
        let canonical = canonical_receipt_bytes(&value)?;
        if canonical.len() > limits.bytes {
            return Err(error(
                "$",
                format!(
                    "canonical JSON is {} bytes; limit is {} bytes",
                    canonical.len(),
                    limits.bytes
                ),
            ));
        }
        Ok(Self { value })
    }

    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ReceiptV1Error> {
        canonical_receipt_bytes(&self.value)
    }

    /// `sha256:<hex>` over the complete canonical raw receipt.
    pub fn canonical_root(&self) -> Result<String, ReceiptV1Error> {
        use sha2::{Digest as _, Sha256};

        canonical_receipt_bytes(&self.value)
            .map(|bytes| format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }

    /// Recheck the fail-closed public descriptor policy for restricted
    /// artifacts. Parsing already performs this check; landing and other
    /// write edges may call it again to make the disclosure boundary explicit.
    ///
    /// A restricted descriptor is deliberately a very small allowlist. It may
    /// publish only an opaque `custodian:` or `opaque:` identifier plus the
    /// artifact kind and the typed `media_type`, `locator_integrity`, and
    /// `availability` metadata. It may not publish payloads, openings,
    /// resolvable locations, equality digests, byte sizes, or unreviewed
    /// extensions. Adding another public field requires an explicit protocol
    /// change instead of relying on Receipt v1's open-object schema.
    pub fn validate_safe_public_artifact_descriptors(&self) -> Result<(), ReceiptV1Error> {
        validate_safe_public_artifact_descriptors(object(&self.value, "$")?)
    }
}

impl AsRef<Value> for ReceiptV1 {
    fn as_ref(&self) -> &Value {
        self.as_value()
    }
}

impl Serialize for ReceiptV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(serializer)
    }
}

fn error(path: impl Into<String>, message: impl Into<String>) -> ReceiptV1Error {
    ReceiptV1Error::new(path, message)
}

#[derive(Debug)]
struct DecodeState {
    limits: ReceiptLimits,
    nodes: usize,
    object_fields: usize,
    array_elements: usize,
}

impl DecodeState {
    fn new(limits: ReceiptLimits) -> Self {
        Self {
            limits,
            nodes: 0,
            object_fields: 0,
            array_elements: 0,
        }
    }

    fn bump_node<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > self.limits.nodes {
            return Err(E::custom(format!(
                "{path}: JSON node budget exceeds {}",
                self.limits.nodes
            )));
        }
        Ok(())
    }

    fn bump_object_field<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.object_fields = self.object_fields.saturating_add(1);
        if self.object_fields > self.limits.object_fields {
            return Err(E::custom(format!(
                "{path}: object-field budget exceeds {}",
                self.limits.object_fields
            )));
        }
        Ok(())
    }

    fn bump_array_element<E: de::Error>(&mut self, path: &str) -> Result<(), E> {
        self.array_elements = self.array_elements.saturating_add(1);
        if self.array_elements > self.limits.array_elements {
            return Err(E::custom(format!(
                "{path}: array-element budget exceeds {}",
                self.limits.array_elements
            )));
        }
        Ok(())
    }

    fn check_container_depth<E: de::Error>(&self, path: &str, depth: usize) -> Result<(), E> {
        if depth > self.limits.depth {
            return Err(E::custom(format!(
                "{path}: JSON depth is {depth}; limit is {}",
                self.limits.depth
            )));
        }
        Ok(())
    }

    fn check_string<E: de::Error>(&self, path: &str, value: &str) -> Result<(), E> {
        if value.len() > self.limits.string_bytes {
            return Err(E::custom(format!(
                "{path}: string is {} bytes; limit is {} bytes",
                value.len(),
                self.limits.string_bytes
            )));
        }
        Ok(())
    }
}

struct BoundedValueSeed<'a> {
    state: &'a mut DecodeState,
    parent_depth: usize,
    path: String,
}

impl<'de> DeserializeSeed<'de> for BoundedValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        self.state.bump_node::<D::Error>(&self.path)?;
        deserializer.deserialize_any(BoundedValueVisitor {
            state: self.state,
            parent_depth: self.parent_depth,
            path: self.path,
        })
    }
}

struct BoundedValueVisitor<'a> {
    state: &'a mut DecodeState,
    parent_depth: usize,
    path: String,
}

impl<'de> Visitor<'de> for BoundedValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON value without duplicate object names")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.unsigned_abs() > MAX_PORTABLE_JSON_INTEGER {
            return Err(E::custom(format!(
                "{}: integer {value} is outside the portable JSON range -{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}",
                self.path
            )));
        }
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value > MAX_PORTABLE_JSON_INTEGER {
            return Err(E::custom(format!(
                "{}: integer {value} is outside the portable JSON range -{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}",
                self.path
            )));
        }
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !value.is_finite() {
            return Err(E::custom(format!("{}: non-finite JSON number", self.path)));
        }
        if value.fract() == 0.0 && value.abs() > MAX_PORTABLE_JSON_INTEGER as f64 {
            return Err(E::custom(format!(
                "{}: integral number {value} is outside the portable JSON range -{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}",
                self.path
            )));
        }
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom(format!("{}: non-finite JSON number", self.path)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string::<E>(&self.path, value)?;
        Ok(Value::String(value.to_string()))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string::<E>(&self.path, &value)?;
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state
            .check_container_depth::<A::Error>(&self.path, depth)?;
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(1024));
        let mut index = 0usize;
        loop {
            let child_path = format!("{}[{index}]", self.path);
            let seed = BoundedValueSeed {
                state: self.state,
                parent_depth: depth,
                path: child_path.clone(),
            };
            let Some(value) = sequence.next_element_seed(seed)? else {
                break;
            };
            self.state.bump_array_element::<A::Error>(&child_path)?;
            values.push(value);
            index = index.saturating_add(1);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut entries: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state
            .check_container_depth::<A::Error>(&self.path, depth)?;
        let mut value = Map::new();
        let mut names = HashSet::new();
        while let Some(key) = entries.next_key::<String>()? {
            self.state.check_string::<A::Error>(&self.path, &key)?;
            let child_path = format!("{}.{}", self.path, key);
            self.state.bump_object_field::<A::Error>(&child_path)?;
            if !names.insert(key.clone()) {
                return Err(<A::Error as de::Error>::custom(format!(
                    "{child_path}: duplicate object name `{key}`"
                )));
            }
            let item = entries.next_value_seed(BoundedValueSeed {
                state: self.state,
                parent_depth: depth,
                path: child_path.clone(),
            })?;
            if is_locator_key(&key) {
                validate_locator_value(&item, &child_path, self.state.limits.locator_bytes)
                    .map_err(<A::Error as de::Error>::custom)?;
            }
            value.insert(key, item);
        }
        Ok(Value::Object(value))
    }
}

fn decode_bounded_json(
    bytes: &[u8],
    limits: ReceiptLimits,
    root_path: &str,
    dsse_payload: bool,
) -> Result<Value, ReceiptV1Error> {
    let byte_limit = if dsse_payload {
        limits.dsse_bytes
    } else {
        limits.bytes
    };
    if bytes.len() > byte_limit {
        return Err(error(
            root_path,
            format!(
                "encoded JSON is {} bytes; limit is {} bytes",
                bytes.len(),
                byte_limit
            ),
        ));
    }
    let mut state = DecodeState::new(limits);
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = BoundedValueSeed {
        state: &mut state,
        parent_depth: 0,
        path: root_path.to_string(),
    }
    .deserialize(&mut deserializer)
    .map_err(|cause| error(root_path, format!("invalid JSON: {cause}")))?;
    deserializer
        .end()
        .map_err(|cause| error(root_path, format!("invalid JSON: {cause}")))?;
    Ok(value)
}

fn is_locator_key(key: &str) -> bool {
    let local = key
        .rsplit([':', '#', '/'])
        .next()
        .unwrap_or(key)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        local.as_str(),
        "parents" | "supersedes" | "derivedfrom" | "sourcerefs"
    ) || [
        "path",
        "uri",
        "url",
        "locator",
        "ref",
        "refs",
        "reference",
        "references",
    ]
    .iter()
    .any(|suffix| local == *suffix || local.ends_with(suffix))
}

fn validate_locator_value(value: &Value, path: &str, limit: usize) -> Result<(), String> {
    let locators: Vec<&str> = match value {
        Value::String(text) => vec![text],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    for locator in locators {
        if locator.len() > limit {
            return Err(format!(
                "{path}: locator is {} bytes; limit is {limit} bytes",
                locator.len()
            ));
        }
    }
    Ok(())
}

fn validate_portable_numeric_domain(value: &Value, path: &str) -> Result<(), ReceiptV1Error> {
    match value {
        Value::Number(number) => {
            let portable_integer = number
                .as_i64()
                .is_some_and(|value| value.unsigned_abs() <= MAX_PORTABLE_JSON_INTEGER)
                || number
                    .as_u64()
                    .is_some_and(|value| value <= MAX_PORTABLE_JSON_INTEGER);
            let portable_float = number.as_f64().is_some_and(|value| {
                value.is_finite()
                    && (value.fract() != 0.0 || value.abs() <= MAX_PORTABLE_JSON_INTEGER as f64)
            });
            let portable = portable_integer || portable_float;
            if !portable {
                return Err(error(
                    path,
                    format!(
                        "integral JSON numbers must be in -{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}; encode larger exact quantities as strings"
                    ),
                ));
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                validate_portable_numeric_domain(item, &format!("{path}[{index}]"))?;
            }
        }
        Value::Object(entries) => {
            for (key, item) in entries {
                validate_portable_numeric_domain(item, &format!("{path}.{key}"))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
    Ok(())
}

/// Receipt v1 canonical JSON is RFC 8785 JCS. This is receipt-specific so the
/// released hashing rules for older Vela primitives remain unchanged.
fn canonical_receipt_bytes(value: &Value) -> Result<Vec<u8>, ReceiptV1Error> {
    validate_portable_numeric_domain(value, "$")?;
    serde_json_canonicalizer::to_vec(value)
        .map_err(|cause| error("$", format!("canonical Receipt JSON failed: {cause}")))
}

fn validate_limits(value: &Value, limits: ReceiptLimits) -> Result<(), ReceiptV1Error> {
    let artifacts = value
        .get("artifacts")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if artifacts > limits.artifacts {
        return Err(error(
            "$.artifacts",
            format!(
                "contains {artifacts} descriptors; limit is {}",
                limits.artifacts
            ),
        ));
    }
    let mut counts = LimitCounts::default();
    let root_depth = usize::from(value.is_object() || value.is_array());
    walk_limits(value, "$", root_depth, limits, &mut counts)
}

#[derive(Debug, Default)]
struct LimitCounts {
    nodes: usize,
    object_fields: usize,
    array_elements: usize,
}

fn walk_limits(
    value: &Value,
    path: &str,
    depth: usize,
    limits: ReceiptLimits,
    counts: &mut LimitCounts,
) -> Result<(), ReceiptV1Error> {
    counts.nodes = counts.nodes.saturating_add(1);
    if counts.nodes > limits.nodes {
        return Err(error(
            path,
            format!("JSON node budget exceeds {}", limits.nodes),
        ));
    }
    if depth > limits.depth {
        return Err(error(
            path,
            format!("JSON depth is {depth}; limit is {}", limits.depth),
        ));
    }
    match value {
        Value::String(text) if text.len() > limits.string_bytes => Err(error(
            path,
            format!(
                "string is {} bytes; limit is {} bytes",
                text.len(),
                limits.string_bytes
            ),
        )),
        Value::Array(items) => {
            counts.array_elements = counts.array_elements.saturating_add(items.len());
            if counts.array_elements > limits.array_elements {
                return Err(error(
                    path,
                    format!("array-element budget exceeds {}", limits.array_elements),
                ));
            }
            for (index, item) in items.iter().enumerate() {
                let child_depth = depth + usize::from(item.is_object() || item.is_array());
                walk_limits(
                    item,
                    &format!("{path}[{index}]"),
                    child_depth,
                    limits,
                    counts,
                )?;
            }
            Ok(())
        }
        Value::Object(object) => {
            counts.object_fields = counts.object_fields.saturating_add(object.len());
            if counts.object_fields > limits.object_fields {
                return Err(error(
                    path,
                    format!("object-field budget exceeds {}", limits.object_fields),
                ));
            }
            for (key, item) in object {
                if key.len() > limits.string_bytes {
                    return Err(error(path, "object key exceeds the string-byte limit"));
                }
                let child_path = format!("{path}.{key}");
                if is_locator_key(key) {
                    validate_locator_value(item, &child_path, limits.locator_bytes)
                        .map_err(|message| error(&child_path, message))?;
                }
                let child_depth = depth + usize::from(item.is_object() || item.is_array());
                walk_limits(item, &child_path, child_depth, limits, counts)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
    }
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ReceiptV1Error> {
    value
        .as_object()
        .ok_or_else(|| error(path, "must be an object"))
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, ReceiptV1Error> {
    value
        .as_array()
        .ok_or_else(|| error(path, "must be an array"))
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, ReceiptV1Error> {
    object
        .get(key)
        .ok_or_else(|| error(format!("{path}.{key}"), "is required"))
}

fn text<'a>(value: &'a Value, path: &str, nonempty: bool) -> Result<&'a str, ReceiptV1Error> {
    let text = value
        .as_str()
        .ok_or_else(|| error(path, "must be a string"))?;
    if nonempty && text.trim().is_empty() {
        return Err(error(path, "must be a non-empty string"));
    }
    Ok(text)
}

fn enum_text<'a>(
    value: &'a Value,
    path: &str,
    allowed: &[&str],
) -> Result<&'a str, ReceiptV1Error> {
    let text = text(value, path, false)?;
    if !allowed.contains(&text) {
        return Err(error(
            path,
            format!("must be one of {}", allowed.join(", ")),
        ));
    }
    Ok(text)
}

/// A validation-only Serde mirror of the frozen Receipt v1 JSON Schema.
///
/// The raw [`Value`] remains authoritative and is never serialized through
/// these types. Open objects intentionally ignore unknown properties; the
/// contributor object is the sole `additionalProperties: false` definition.
/// `Optional<T>` distinguishes an absent optional field from a present `null`,
/// so optional-but-non-null schema properties retain their exact type rule.
#[allow(dead_code)]
mod schema_exact {
    use serde::Deserialize;
    use serde_json::{Map, Value};

    struct Optional<T>(Option<T>);

    impl<T> Default for Optional<T> {
        fn default() -> Self {
            Self(None)
        }
    }

    impl<'de, T> Deserialize<'de> for Optional<T>
    where
        T: Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            T::deserialize(deserializer).map(|value| Self(Some(value)))
        }
    }

    struct Nullable<T>(Option<T>);

    impl<'de, T> Deserialize<'de> for Nullable<T>
    where
        T: Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Option::<T>::deserialize(deserializer).map(Self)
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ClaimType {
        Computational,
        Theoretical,
        Empirical,
        Negative,
        Contradiction,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Replayability {
        Exact,
        Bounded,
        Approximate,
        Unavailable,
        Unknown,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum VerifierOutcome {
        Pass,
        Fail,
        Error,
        Skipped,
        Unknown,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum StatusKind {
        Draft,
        Emitted,
        Proposed,
        Runs,
        MinimalSanityCheck,
        ReportedMetricRederived,
        FullReproduction,
        LandedPending,
        Accepted,
        Rejected,
        Superseded,
        Retracted,
        Contested,
        FailedReproduction,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum StatusAuthority {
        Producer,
        VelaLanding,
        HumanKey,
        SignedPolicy,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum EvidenceStatus {
        Proposed,
        Runs,
        MinimalSanityCheck,
        ReportedMetricRederived,
        FullReproduction,
        Accepted,
        Superseded,
        Retracted,
        Contested,
        FailedReproduction,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum AcceptanceScope {
        MachineVerified,
        HumanSeen,
        LocallyAccepted,
        FrontierAccepted,
        CanonAccepted,
        HypothesisOnly,
        Retracted,
        Superseded,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum EvidenceLevel {
        LocalSignoff,
        ConsortiumReviewed,
        JournalAccepted,
        Replicated,
        RegulatorGrade,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum DistillationStatus {
        Missing,
        NotRequired,
        Draft,
        Accepted,
        Rejected,
        Superseded,
        Retracted,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ContributorRole {
        Conceptualization,
        DataCuration,
        FormalAnalysis,
        FundingAcquisition,
        Investigation,
        Methodology,
        ProjectAdministration,
        Resources,
        Software,
        Supervision,
        Validation,
        Visualization,
        WritingOriginalDraft,
        WritingReviewEditing,
        MachineProducer,
        HumanFormalizer,
        HumanDistiller,
        Reviewer,
        Acceptor,
        ProfileMaintainer,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum IdentityRole {
        Producer,
        Acceptor,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum IdentityMechanism {
        SigstoreKeylessOidc,
        SigstoreKeylessOidcFixture,
        Ed25519KeyCustodyCeremony,
        Ed25519KeyCustodyCeremonyFixture,
    }

    #[derive(Deserialize)]
    struct Artifact {
        path: String,
        kind: String,
        #[serde(default)]
        sha256: Optional<String>,
        #[serde(default)]
        media_type: Optional<String>,
        #[serde(default)]
        uri: Optional<String>,
    }

    #[derive(Deserialize)]
    struct VerifierRun {
        method: String,
        outcome: VerifierOutcome,
        #[serde(default)]
        log: Optional<String>,
        #[serde(default)]
        solver: Optional<String>,
        #[serde(default)]
        replay_command: Optional<String>,
    }

    #[derive(Deserialize)]
    struct Provenance {
        generated_by: String,
        emitted_at: String,
        #[serde(default)]
        submitter: Optional<Map<String, Value>>,
    }

    #[derive(Deserialize)]
    struct Status {
        kind: StatusKind,
        authority: StatusAuthority,
        #[serde(default)]
        evidence_status: Optional<EvidenceStatus>,
        #[serde(default)]
        scope: Optional<Map<String, Value>>,
    }

    #[derive(Deserialize)]
    struct MachineClaim {
        text: Value,
        #[serde(rename = "type")]
        claim_type: Value,
    }

    #[derive(Deserialize)]
    struct MachineVerification {
        status: Value,
        verifier_runs: Value,
        trust_base: Value,
    }

    #[derive(Deserialize)]
    struct MachineLayer {
        subject: Vec<Value>,
        claim: MachineClaim,
        verification: MachineVerification,
    }

    #[derive(Deserialize)]
    struct AcceptanceLayer {
        profile: String,
        mechanism: String,
        acceptor: Nullable<String>,
        #[serde(rename = "policyRef")]
        policy_ref: String,
        #[serde(rename = "evidenceRefs")]
        evidence_refs: Vec<String>,
        #[serde(default, rename = "evidenceLevel")]
        evidence_level: Optional<Nullable<EvidenceLevel>>,
        artifact_verification: Map<String, Value>,
        claim_acceptance: Map<String, Value>,
        distillation_acceptance: Map<String, Value>,
        acceptance_scope: AcceptanceScope,
    }

    #[derive(Deserialize)]
    struct DistillationLayer {
        status: DistillationStatus,
        audience: String,
        rubric: String,
        #[serde(default)]
        uri: Optional<Nullable<String>>,
        #[serde(default)]
        digest: Optional<Nullable<Map<String, Value>>>,
        #[serde(default)]
        level: Optional<String>,
        #[serde(default)]
        accepted_by: Optional<Nullable<String>>,
        #[serde(default)]
        comprehension_budget: Optional<String>,
        #[serde(default)]
        inheritance_note: Optional<String>,
        #[serde(default)]
        known_gaps: Optional<Vec<Value>>,
        #[serde(default)]
        signature_refs: Optional<Vec<Value>>,
    }

    #[derive(Deserialize)]
    struct LineageLayer {
        parents: Value,
        derived_from: Value,
        source_refs: Value,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Contributor {
        id: String,
        roles: Vec<ContributorRole>,
        #[serde(default)]
        credit_taxonomy: Optional<String>,
        #[serde(default)]
        author: Optional<bool>,
        #[serde(default)]
        note: Optional<String>,
    }

    #[derive(Deserialize)]
    struct SignatureIdentity {
        role: IdentityRole,
        mechanism: IdentityMechanism,
        #[serde(default, rename = "signatureRef")]
        signature_ref: Optional<Nullable<String>>,
        #[serde(default, rename = "oidcIssuer")]
        oidc_issuer: Optional<String>,
        #[serde(default)]
        orcid: Optional<String>,
        #[serde(default)]
        subject: Optional<String>,
    }

    #[derive(Deserialize)]
    struct SignatureIdentities {
        producer: SignatureIdentity,
        #[serde(default)]
        acceptor: Optional<SignatureIdentity>,
    }

    #[derive(Deserialize)]
    struct StatementPredicate {
        machine: Value,
        acceptance: Value,
        distillation: Value,
        lineage: Value,
        provenance: Value,
    }

    #[derive(Deserialize)]
    struct Statement {
        #[serde(rename = "_type")]
        statement_type: String,
        subject: Vec<Value>,
        #[serde(rename = "predicateType")]
        predicate_type: String,
        predicate: StatementPredicate,
    }

    #[derive(Deserialize)]
    struct DsseEnvelope {
        #[serde(rename = "payloadType")]
        payload_type: String,
        payload: String,
        signatures: Vec<Value>,
    }

    #[derive(Deserialize)]
    struct Attestation {
        format: String,
        statement: Statement,
        dsse_envelope: DsseEnvelope,
    }

    #[derive(Deserialize)]
    struct Receipt {
        schema: String,
        #[serde(default)]
        claim_id: Optional<String>,
        claim: String,
        #[serde(rename = "type")]
        claim_type: ClaimType,
        replayability: Replayability,
        artifacts: Vec<Artifact>,
        caveats: Vec<String>,
        verifier_runs: Vec<VerifierRun>,
        #[serde(default)]
        conditions: Optional<Vec<String>>,
        #[serde(default)]
        verification_requirements: Optional<Vec<String>>,
        #[serde(default)]
        state_diff: Optional<Map<String, Value>>,
        environment: Map<String, Value>,
        provenance: Provenance,
        status: Status,
        machine: MachineLayer,
        acceptance: AcceptanceLayer,
        distillation: DistillationLayer,
        lineage: LineageLayer,
        contributors: Vec<Contributor>,
        signature_identities: SignatureIdentities,
        attestation: Attestation,
    }

    pub(super) fn validate(value: &Value) -> Result<(), String> {
        serde_json::from_value::<Receipt>(value.clone())
            .map(|_| ())
            .map_err(|cause| cause.to_string())
    }
}

fn validate_schema_exact(value: &Value) -> Result<(), ReceiptV1Error> {
    schema_exact::validate(value).map_err(|cause| {
        error(
            "$",
            format!("does not match frozen Receipt v1 schema: {cause}"),
        )
    })
}

/// Shared semantic checks intentionally cover the stable trust-bearing waist,
/// while the shipped JSON Schema and schema-sync gate own every open extension.
fn validate_semantics(value: &Value, limits: ReceiptLimits) -> Result<(), ReceiptV1Error> {
    let receipt = object(value, "$")?;
    for field in RECEIPT_V1_REQUIRED_FIELDS {
        required(receipt, field, "$")?;
    }
    if text(required(receipt, "schema", "$")?, "$.schema", false)? != RECEIPT_V1_SCHEMA {
        return Err(error("$.schema", format!("must be {RECEIPT_V1_SCHEMA}")));
    }
    text(required(receipt, "claim", "$")?, "$.claim", true)?;
    enum_text(required(receipt, "type", "$")?, "$.type", CLAIM_TYPES)?;
    enum_text(
        required(receipt, "replayability", "$")?,
        "$.replayability",
        REPLAYABILITY,
    )?;

    let artifacts = array(required(receipt, "artifacts", "$")?, "$.artifacts")?;
    for (index, item) in artifacts.iter().enumerate() {
        let path = format!("$.artifacts[{index}]");
        let item = object(item, &path)?;
        text(
            required(item, "path", &path)?,
            &format!("{path}.path"),
            true,
        )?;
        text(
            required(item, "kind", &path)?,
            &format!("{path}.kind"),
            true,
        )?;
        if let Some(hash) = item.get("sha256") {
            let hash = text(hash, &format!("{path}.sha256"), false)?;
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(error(
                    format!("{path}.sha256"),
                    "must be 64 lowercase hexadecimal characters",
                ));
            }
        }
    }
    validate_safe_public_artifact_descriptors(receipt)?;

    let caveats = array(required(receipt, "caveats", "$")?, "$.caveats")?;
    if caveats.is_empty() {
        return Err(error("$.caveats", "must contain at least one caveat"));
    }
    for (index, caveat) in caveats.iter().enumerate() {
        text(caveat, &format!("$.caveats[{index}]"), true)?;
    }

    let runs = array(required(receipt, "verifier_runs", "$")?, "$.verifier_runs")?;
    for (index, run) in runs.iter().enumerate() {
        let path = format!("$.verifier_runs[{index}]");
        let run = object(run, &path)?;
        text(
            required(run, "method", &path)?,
            &format!("{path}.method"),
            true,
        )?;
        enum_text(
            required(run, "outcome", &path)?,
            &format!("{path}.outcome"),
            VERIFIER_OUTCOMES,
        )?;
    }

    for field in [
        "environment",
        "provenance",
        "status",
        "machine",
        "acceptance",
        "distillation",
        "lineage",
        "signature_identities",
        "attestation",
    ] {
        object(required(receipt, field, "$")?, &format!("$.{field}"))?;
    }
    validate_provenance(receipt)?;
    validate_status(receipt)?;
    validate_machine(receipt)?;
    validate_acceptance(receipt)?;
    validate_distillation_and_lineage(receipt)?;
    validate_contributors(receipt)?;
    validate_identities(receipt)?;
    validate_attestation(receipt, limits)
}

/// Receipt v1 intentionally keeps artifact objects open for compatible public
/// extensions. That openness cannot cross the restricted-data boundary: once
/// an artifact is marked restricted, only a reviewed safe-public descriptor is
/// accepted. This is semantic validation, not a new wire schema.
fn validate_safe_public_artifact_descriptors(
    receipt: &Map<String, Value>,
) -> Result<(), ReceiptV1Error> {
    let artifacts = array(required(receipt, "artifacts", "$")?, "$.artifacts")?;
    for (index, artifact) in artifacts.iter().enumerate() {
        let artifact_path = format!("$.artifacts[{index}]");
        let artifact = object(artifact, &artifact_path)?;
        let disclosure = match artifact.get("disclosure") {
            None => None,
            Some(value) => Some(text(value, &format!("{artifact_path}.disclosure"), false)?),
        };
        if let Some(value) = disclosure
            && !matches!(value, "public" | "restricted")
        {
            return Err(error(
                format!("{artifact_path}.disclosure"),
                "must be public or restricted",
            ));
        }

        // Do not let an alternate open-schema spelling declare sensitive
        // material while the canonical disclosure field remains absent or
        // public. Landing understands `disclosure`; aliases would create an
        // unsafe split interpretation between producers and the write edge.
        for alias in ["visibility", "access_tier", "accessTier"] {
            let Some(value) = artifact.get(alias).and_then(Value::as_str) else {
                continue;
            };
            if matches!(value, "restricted" | "classified" | "private" | "sealed")
                && disclosure != Some("restricted")
            {
                return Err(error(
                    format!("{artifact_path}.{alias}"),
                    "sensitive artifacts must use disclosure: restricted",
                ));
            }
        }

        if disclosure != Some("restricted") {
            continue;
        }

        for field in artifact.keys() {
            if !matches!(
                field.as_str(),
                "path"
                    | "kind"
                    | "disclosure"
                    | "media_type"
                    | "locator_integrity"
                    | "availability"
            ) {
                return Err(error(
                    format!("{artifact_path}.{field}"),
                    "is not permitted in a restricted artifact's safe-public descriptor",
                ));
            }
        }

        let locator = text(
            required(artifact, "path", &artifact_path)?,
            &format!("{artifact_path}.path"),
            true,
        )?;
        validate_restricted_opaque_locator(locator, &format!("{artifact_path}.path"))?;

        let kind_path = format!("{artifact_path}.kind");
        let kind = text(
            required(artifact, "kind", &artifact_path)?,
            &kind_path,
            true,
        )?;
        validate_safe_public_metadata_text(kind, &kind_path, MAX_RESTRICTED_ARTIFACT_KIND_BYTES)?;

        if let Some(media_type) = artifact.get("media_type") {
            let media_type_path = format!("{artifact_path}.media_type");
            let media_type = text(media_type, &media_type_path, true)?;
            validate_safe_public_metadata_text(
                media_type,
                &media_type_path,
                MAX_RESTRICTED_ARTIFACT_MEDIA_TYPE_BYTES,
            )?;
        }
        if let Some(integrity) = artifact.get("locator_integrity") {
            enum_text(
                integrity,
                &format!("{artifact_path}.locator_integrity"),
                &["immutable", "mutable", "unknown"],
            )?;
        }
        if let Some(availability) = artifact.get("availability") {
            enum_text(
                availability,
                &format!("{artifact_path}.availability"),
                &["available", "unavailable", "unknown"],
            )?;
        }
    }
    validate_restricted_artifact_mirrors(receipt, artifacts)?;
    Ok(())
}

/// Restricted artifact descriptors have a single canonical public projection.
/// Every built-in receipt mirror must use it, so moving a digest, opening, or
/// resolvable locator from `artifacts` into in-toto/PROV metadata cannot bypass
/// the disclosure boundary.
fn validate_restricted_artifact_mirrors(
    receipt: &Map<String, Value>,
    artifacts: &[Value],
) -> Result<(), ReceiptV1Error> {
    let mut restricted = Vec::new();
    let mut expected_subjects = Vec::with_capacity(artifacts.len());
    for (index, artifact) in artifacts.iter().enumerate() {
        let artifact_path = format!("$.artifacts[{index}]");
        let artifact = object(artifact, &artifact_path)?;
        let locator = text(
            required(artifact, "path", &artifact_path)?,
            &format!("{artifact_path}.path"),
            true,
        )?;
        let is_restricted =
            artifact.get("disclosure").and_then(Value::as_str) == Some("restricted");
        let mut subject = Map::new();
        subject.insert("name".to_string(), Value::String(locator.to_string()));
        if is_restricted {
            restricted.push(locator.to_string());
        } else {
            if let Some(digest) = artifact.get("sha256") {
                subject.insert("digest".to_string(), json!({"sha256": digest}));
            }
            if let Some(uri) = artifact.get("uri") {
                subject.insert("uri".to_string(), uri.clone());
            }
        }
        expected_subjects.push(Value::Object(subject));
    }
    if restricted.is_empty() {
        return Ok(());
    }
    let expected_subjects = Value::Array(expected_subjects);

    let machine = object(required(receipt, "machine", "$")?, "$.machine")?;
    if required(machine, "subject", "$.machine")? != &expected_subjects {
        return Err(error(
            "$.machine.subject",
            "must be the artifact-derived public subject projection when restricted artifacts are present; restricted subjects may contain only their opaque name",
        ));
    }

    let attestation = object(required(receipt, "attestation", "$")?, "$.attestation")?;
    let statement = object(
        required(attestation, "statement", "$.attestation")?,
        "$.attestation.statement",
    )?;
    if required(statement, "subject", "$.attestation.statement")? != &expected_subjects {
        return Err(error(
            "$.attestation.statement.subject",
            "must not add a digest, opening, or resolvable locator for a restricted artifact",
        ));
    }
    let predicate = object(
        required(statement, "predicate", "$.attestation.statement")?,
        "$.attestation.statement.predicate",
    )?;
    let predicate_machine = object(
        required(predicate, "machine", "$.attestation.statement.predicate")?,
        "$.attestation.statement.predicate.machine",
    )?;
    if required(
        predicate_machine,
        "subject",
        "$.attestation.statement.predicate.machine",
    )? != &expected_subjects
    {
        return Err(error(
            "$.attestation.statement.predicate.machine.subject",
            "must not add a digest, opening, or resolvable locator for a restricted artifact",
        ));
    }

    if let Some(prov) = attestation.get("prov") {
        let prov = object(prov, "$.attestation.prov")?;
        if let Some(entities) = prov.get("entity") {
            let entities = object(entities, "$.attestation.prov.entity")?;
            for locator in &restricted {
                let entity_id = format!("artifact:{locator}");
                let Some(entity) = entities.get(&entity_id) else {
                    continue;
                };
                let entity_path = format!("$.attestation.prov.entity.{entity_id}");
                let entity = object(entity, &entity_path)?;
                for field in entity.keys() {
                    if !matches!(field.as_str(), "prov:type" | "vela:kind") {
                        return Err(error(
                            format!("{entity_path}.{field}"),
                            "is not permitted in the public PROV mirror of a restricted artifact",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_restricted_opaque_locator(locator: &str, path: &str) -> Result<(), ReceiptV1Error> {
    let opaque_id = locator
        .strip_prefix("custodian:")
        .or_else(|| locator.strip_prefix("opaque:"))
        .ok_or_else(|| {
            error(
                path,
                "restricted artifacts require a non-resolving custodian: or opaque: identifier",
            )
        })?;
    if opaque_id.is_empty()
        || opaque_id.len() > MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES
        || !opaque_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(error(
            path,
            format!(
                "opaque identifier must be 1..={} ASCII identifier bytes and contain no path, URL, query, fragment, whitespace, or control syntax",
                MAX_RESTRICTED_ARTIFACT_OPAQUE_ID_BYTES
            ),
        ));
    }
    Ok(())
}

fn validate_safe_public_metadata_text(
    value: &str,
    path: &str,
    limit: usize,
) -> Result<(), ReceiptV1Error> {
    if value != value.trim() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(error(
            path,
            format!(
                "safe-public metadata must be trimmed, control-free, and at most {limit} bytes"
            ),
        ));
    }
    Ok(())
}

fn validate_provenance(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let provenance = object(&receipt["provenance"], "$.provenance")?;
    text(
        required(provenance, "generated_by", "$.provenance")?,
        "$.provenance.generated_by",
        true,
    )?;
    let emitted_at = text(
        required(provenance, "emitted_at", "$.provenance")?,
        "$.provenance.emitted_at",
        true,
    )?;
    chrono::DateTime::parse_from_rfc3339(emitted_at)
        .map_err(|_| error("$.provenance.emitted_at", "must be an RFC 3339 date-time"))?;
    Ok(())
}

fn validate_status(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let status = object(&receipt["status"], "$.status")?;
    let kind = enum_text(
        required(status, "kind", "$.status")?,
        "$.status.kind",
        STATUS_KINDS,
    )?;
    let authority = enum_text(
        required(status, "authority", "$.status")?,
        "$.status.authority",
        STATUS_AUTHORITIES,
    )?;
    if authority == "producer" && !["draft", "emitted"].contains(&kind) {
        return Err(error(
            "$.status",
            "producer authority may emit only draft or emitted status",
        ));
    }
    Ok(())
}

fn validate_machine(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let machine = object(&receipt["machine"], "$.machine")?;
    if array(
        required(machine, "subject", "$.machine")?,
        "$.machine.subject",
    )?
    .is_empty()
    {
        return Err(error("$.machine.subject", "must not be empty"));
    }
    for (field, path) in [
        ("claim", "$.machine.claim"),
        ("verification", "$.machine.verification"),
    ] {
        object(required(machine, field, "$.machine")?, path)?;
    }
    let claim = object(&machine["claim"], "$.machine.claim")?;
    required(claim, "text", "$.machine.claim")?;
    required(claim, "type", "$.machine.claim")?;
    let verification = object(&machine["verification"], "$.machine.verification")?;
    for field in ["status", "verifier_runs", "trust_base"] {
        required(verification, field, "$.machine.verification")?;
    }
    Ok(())
}

fn validate_acceptance(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let acceptance = object(&receipt["acceptance"], "$.acceptance")?;
    for field in [
        "profile",
        "mechanism",
        "acceptor",
        "policyRef",
        "evidenceRefs",
        "artifact_verification",
        "claim_acceptance",
        "distillation_acceptance",
        "acceptance_scope",
    ] {
        required(acceptance, field, "$.acceptance")?;
    }
    text(&acceptance["profile"], "$.acceptance.profile", true)?;
    if acceptance["mechanism"] != ACCEPTANCE_MECHANISM {
        return Err(error(
            "$.acceptance.mechanism",
            format!("must be {ACCEPTANCE_MECHANISM}"),
        ));
    }
    if !acceptance["acceptor"].is_null() {
        text(&acceptance["acceptor"], "$.acceptance.acceptor", false)?;
    }
    text(&acceptance["policyRef"], "$.acceptance.policyRef", true)?;
    array(&acceptance["evidenceRefs"], "$.acceptance.evidenceRefs")?;
    for field in [
        "artifact_verification",
        "claim_acceptance",
        "distillation_acceptance",
    ] {
        object(&acceptance[field], &format!("$.acceptance.{field}"))?;
    }
    let scope = text(
        &acceptance["acceptance_scope"],
        "$.acceptance.acceptance_scope",
        false,
    )?;
    if AcceptanceScope::parse(scope).is_none() {
        return Err(error(
            "$.acceptance.acceptance_scope",
            "is not a published v1 scope",
        ));
    }
    Ok(())
}

fn validate_distillation_and_lineage(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let distillation = object(&receipt["distillation"], "$.distillation")?;
    for field in ["status", "audience", "rubric"] {
        required(distillation, field, "$.distillation")?;
    }
    let lineage = object(&receipt["lineage"], "$.lineage")?;
    for field in ["parents", "derived_from", "source_refs"] {
        required(lineage, field, "$.lineage")?;
    }
    Ok(())
}

fn validate_contributors(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let contributors = array(&receipt["contributors"], "$.contributors")?;
    if contributors.is_empty() {
        return Err(error("$.contributors", "must not be empty"));
    }
    for (index, contributor) in contributors.iter().enumerate() {
        let path = format!("$.contributors[{index}]");
        let contributor = object(contributor, &path)?;
        text(
            required(contributor, "id", &path)?,
            &format!("{path}.id"),
            true,
        )?;
        let roles = array(
            required(contributor, "roles", &path)?,
            &format!("{path}.roles"),
        )?;
        if roles.is_empty() {
            return Err(error(format!("{path}.roles"), "must not be empty"));
        }
        for (role_index, role) in roles.iter().enumerate() {
            enum_text(
                role,
                &format!("{path}.roles[{role_index}]"),
                CONTRIBUTOR_ROLES,
            )?;
        }
    }
    Ok(())
}

fn validate_identities(receipt: &Map<String, Value>) -> Result<(), ReceiptV1Error> {
    let identities = object(&receipt["signature_identities"], "$.signature_identities")?;
    validate_identity(
        required(identities, "producer", "$.signature_identities")?,
        "$.signature_identities.producer",
        "producer",
    )?;
    if let Some(acceptor) = identities.get("acceptor") {
        validate_identity(acceptor, "$.signature_identities.acceptor", "acceptor")?;
    }
    validate_embedded_producer_identity(receipt, identities)?;
    Ok(())
}

fn validate_identity(value: &Value, path: &str, role: &str) -> Result<(), ReceiptV1Error> {
    let identity = object(value, path)?;
    if text(
        required(identity, "role", path)?,
        &format!("{path}.role"),
        false,
    )? != role
    {
        return Err(error(format!("{path}.role"), format!("must be {role}")));
    }
    enum_text(
        required(identity, "mechanism", path)?,
        &format!("{path}.mechanism"),
        IDENTITY_MECHANISMS,
    )?;
    Ok(())
}

fn validate_embedded_producer_identity(
    receipt: &Map<String, Value>,
    identities: &Map<String, Value>,
) -> Result<(), ReceiptV1Error> {
    let environment = object(&receipt["environment"], "$.environment")?;
    let context = environment
        .get("vela:producer_context")
        .map(|value| object(value, "$.environment.vela:producer_context"))
        .transpose()?;
    let generated_by = receipt["provenance"]["generated_by"].as_str();
    let binding_value = context.and_then(|context| context.get("identity_binding"));
    if binding_value.is_none() {
        if generated_by == Some(NEUTRAL_RECEIPT_GENERATOR) {
            return Err(error(
                "$.environment.vela:producer_context.identity_binding",
                "neutral protocol emissions require the full self-signed producer binding",
            ));
        }
        return Ok(());
    }
    let binding: crate::identity::IdentityBinding =
        serde_json::from_value(binding_value.cloned().unwrap()).map_err(|cause| {
            error(
                "$.environment.vela:producer_context.identity_binding",
                format!("must be a complete IdentityBinding: {cause}"),
            )
        })?;
    binding.verify().map_err(|cause| {
        error(
            "$.environment.vela:producer_context.identity_binding",
            format!("does not verify proof of possession: {cause}"),
        )
    })?;
    if binding.actor_class != crate::identity::ActorClass::Agent {
        return Err(error(
            "$.environment.vela:producer_context.identity_binding.actor_class",
            "must be agent",
        ));
    }
    let context = context.expect("binding implies context");
    let actor_suffix = binding
        .actor_id
        .strip_prefix("agent:")
        .or_else(|| binding.actor_id.strip_prefix("ci:"))
        .filter(|suffix| !suffix.trim().is_empty())
        .ok_or_else(|| {
            error(
                "$.environment.vela:producer_context.identity_binding.actor_id",
                "must be a non-empty agent: or ci: identity",
            )
        })?;
    if actor_suffix != actor_suffix.trim() || actor_suffix.chars().any(char::is_control) {
        return Err(error(
            "$.environment.vela:producer_context.identity_binding.actor_id",
            "identity suffix must be trimmed and contain no controls",
        ));
    }
    let event_log_root = text(
        required(
            context,
            "event_log_root",
            "$.environment.vela:producer_context",
        )?,
        "$.environment.vela:producer_context.event_log_root",
        false,
    )?;
    if !event_log_root
        .strip_prefix("sha256:")
        .is_some_and(|hex| is_lower_hex_length(hex, 64))
    {
        return Err(error(
            "$.environment.vela:producer_context.event_log_root",
            "must be sha256: followed by 64 lowercase hexadecimal characters",
        ));
    }
    if let Some(task_contract_root) = context.get("task_contract_root") {
        let task_contract_root = text(
            task_contract_root,
            "$.environment.vela:producer_context.task_contract_root",
            false,
        )?;
        if !task_contract_root
            .strip_prefix("sha256:")
            .is_some_and(|hex| is_lower_hex_length(hex, 64))
        {
            return Err(error(
                "$.environment.vela:producer_context.task_contract_root",
                "must be sha256: followed by 64 lowercase hexadecimal characters",
            ));
        }
    }
    let operation_id = text(
        required(
            context,
            "operation_id",
            "$.environment.vela:producer_context",
        )?,
        "$.environment.vela:producer_context.operation_id",
        false,
    )?;
    if !operation_id
        .strip_prefix("vop_")
        .is_some_and(|hex| is_lower_hex_length(hex, 64))
    {
        return Err(error(
            "$.environment.vela:producer_context.operation_id",
            "must be vop_ followed by 64 lowercase hexadecimal characters",
        ));
    }
    let base_path = text(
        required(context, "base_path", "$.environment.vela:producer_context")?,
        "$.environment.vela:producer_context.base_path",
        true,
    )?;
    if base_path != base_path.trim() || base_path.chars().any(char::is_control) {
        return Err(error(
            "$.environment.vela:producer_context.base_path",
            "must be a trimmed path without control characters",
        ));
    }
    let policy_ref = text(
        required(context, "policy_ref", "$.environment.vela:producer_context")?,
        "$.environment.vela:producer_context.policy_ref",
        false,
    )?;
    if policy_ref != NO_ACTIVE_POLICY_REF
        && !policy_ref
            .strip_prefix("vap_")
            .is_some_and(|hex| is_lower_hex_length(hex, 32))
    {
        return Err(error(
            "$.environment.vela:producer_context.policy_ref",
            "must be a vap_ content address or urn:vela:policy:none",
        ));
    }
    if receipt["acceptance"]["policyRef"].as_str() != Some(policy_ref) {
        return Err(error(
            "$.acceptance.policyRef",
            "must match the mechanical producer context",
        ));
    }
    let producer = object(
        required(identities, "producer", "$.signature_identities")?,
        "$.signature_identities.producer",
    )?;
    for (path, actual, expected) in [
        (
            "$.environment.vela:producer_context.actor",
            context.get("actor"),
            binding.actor_id.as_str(),
        ),
        (
            "$.environment.vela:producer_context.identity_binding_ref",
            context.get("identity_binding_ref"),
            binding.binding_id.as_str(),
        ),
        (
            "$.signature_identities.producer.subject",
            producer.get("subject"),
            binding.actor_id.as_str(),
        ),
        (
            "$.signature_identities.producer.publicKey",
            producer.get("publicKey"),
            binding.public_key_hex.as_str(),
        ),
        (
            "$.signature_identities.producer.identityBindingRef",
            producer.get("identityBindingRef"),
            binding.binding_id.as_str(),
        ),
        (
            "$.signature_identities.producer.signatureRef",
            producer.get("signatureRef"),
            binding.binding_id.as_str(),
        ),
    ] {
        if actual.and_then(Value::as_str) != Some(expected) {
            return Err(error(path, "must match the embedded producer binding"));
        }
    }
    if producer.get("mechanism").and_then(Value::as_str) != Some("ed25519_key_custody_ceremony") {
        return Err(error(
            "$.signature_identities.producer.mechanism",
            "embedded producer bindings require ed25519_key_custody_ceremony",
        ));
    }
    let provenance = object(&receipt["provenance"], "$.provenance")?;
    let submitter = object(
        required(provenance, "submitter", "$.provenance")?,
        "$.provenance.submitter",
    )?;
    if submitter.get("actor").and_then(Value::as_str) != Some(binding.actor_id.as_str())
        || submitter
            .get("identity_binding_ref")
            .and_then(Value::as_str)
            != Some(binding.binding_id.as_str())
        || submitter.get("operation_id").and_then(Value::as_str) != Some(operation_id)
    {
        return Err(error(
            "$.provenance.submitter",
            "must reference the embedded producer binding",
        ));
    }
    Ok(())
}

fn is_lower_hex_length(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn receipt_body(receipt: &Map<String, Value>) -> Value {
    let mut body = receipt.clone();
    body.remove("attestation");
    Value::Object(body)
}

fn receipt_body_sha256(receipt: &Map<String, Value>) -> Result<String, ReceiptV1Error> {
    use sha2::{Digest as _, Sha256};

    canonical_receipt_bytes(&receipt_body(receipt))
        .map(|bytes| hex::encode(Sha256::digest(bytes)))
        .map_err(|cause| error("$", format!("canonical receipt body: {cause}")))
}

fn statement_projection(receipt: &Map<String, Value>) -> Result<Value, ReceiptV1Error> {
    let machine = required(receipt, "machine", "$")?;
    let machine_object = object(machine, "$.machine")?;
    let subject = required(machine_object, "subject", "$.machine")?;
    Ok(json!({
        "_type": INTOTO_STATEMENT_TYPE,
        "subject": subject,
        "predicateType": VELA_PREDICATE_TYPE,
        "predicate": {
            "schema": RECEIPT_PREDICATE_SCHEMA,
            "machine": machine,
            "acceptance": required(receipt, "acceptance", "$")?,
            "distillation": required(receipt, "distillation", "$")?,
            "lineage": required(receipt, "lineage", "$")?,
            "contributors": required(receipt, "contributors", "$")?,
            "signature_identities": required(receipt, "signature_identities", "$")?,
            "provenance": required(receipt, "provenance", "$")?,
            (RECEIPT_BODY_BINDING_FIELD): {
                "sha256": receipt_body_sha256(receipt)?,
            },
        }
    }))
}

fn validate_bound_statement_projection(
    receipt: &Map<String, Value>,
    statement: &Map<String, Value>,
) -> Result<(), ReceiptV1Error> {
    let expected = statement_projection(receipt)?;
    let expected = object(&expected, "$.attestation.statement")?;
    for field in ["_type", "subject", "predicateType"] {
        if statement.get(field) != expected.get(field) {
            return Err(error(
                format!("$.attestation.statement.{field}"),
                "does not match the canonical receipt-body projection",
            ));
        }
    }
    let predicate = object(
        required(statement, "predicate", "$.attestation.statement")?,
        "$.attestation.statement.predicate",
    )?;
    let expected_predicate = object(
        required(expected, "predicate", "$.attestation.statement")?,
        "$.attestation.statement.predicate",
    )?;
    for (field, expected_value) in expected_predicate {
        if predicate.get(field) != Some(expected_value) {
            return Err(error(
                format!("$.attestation.statement.predicate.{field}"),
                "does not match the canonical receipt-body projection",
            ));
        }
    }
    let binding = object(
        required(
            predicate,
            RECEIPT_BODY_BINDING_FIELD,
            "$.attestation.statement.predicate",
        )?,
        "$.attestation.statement.predicate.vela:receipt_body",
    )?;
    if binding.len() != 1 {
        return Err(error(
            "$.attestation.statement.predicate.vela:receipt_body",
            "must contain only the canonical sha256 body root",
        ));
    }
    let digest = text(
        required(
            binding,
            "sha256",
            "$.attestation.statement.predicate.vela:receipt_body",
        )?,
        "$.attestation.statement.predicate.vela:receipt_body.sha256",
        false,
    )?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(error(
            "$.attestation.statement.predicate.vela:receipt_body.sha256",
            "must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_attestation(
    receipt: &Map<String, Value>,
    limits: ReceiptLimits,
) -> Result<(), ReceiptV1Error> {
    let attestation = object(&receipt["attestation"], "$.attestation")?;
    if required(attestation, "format", "$.attestation")? != "in-toto-statement" {
        return Err(error("$.attestation.format", "must be in-toto-statement"));
    }
    let statement = object(
        required(attestation, "statement", "$.attestation")?,
        "$.attestation.statement",
    )?;
    if required(statement, "_type", "$.attestation.statement")? != INTOTO_STATEMENT_TYPE {
        return Err(error(
            "$.attestation.statement._type",
            "invalid in-toto type",
        ));
    }
    if required(statement, "predicateType", "$.attestation.statement")? != VELA_PREDICATE_TYPE {
        return Err(error(
            "$.attestation.statement.predicateType",
            "invalid Vela predicate type",
        ));
    }
    if array(
        required(statement, "subject", "$.attestation.statement")?,
        "$.attestation.statement.subject",
    )?
    .is_empty()
    {
        return Err(error(
            "$.attestation.statement.subject",
            "must not be empty",
        ));
    }
    let predicate = object(
        required(statement, "predicate", "$.attestation.statement")?,
        "$.attestation.statement.predicate",
    )?;
    for field in [
        "machine",
        "acceptance",
        "distillation",
        "lineage",
        "provenance",
    ] {
        required(predicate, field, "$.attestation.statement.predicate")?;
    }
    let envelope = object(
        required(attestation, "dsse_envelope", "$.attestation")?,
        "$.attestation.dsse_envelope",
    )?;
    if required(envelope, "payloadType", "$.attestation.dsse_envelope")? != INTOTO_PAYLOAD_TYPE {
        return Err(error(
            "$.attestation.dsse_envelope.payloadType",
            "invalid DSSE payload type",
        ));
    }
    let payload = text(
        required(envelope, "payload", "$.attestation.dsse_envelope")?,
        "$.attestation.dsse_envelope.payload",
        true,
    )?;
    array(
        required(envelope, "signatures", "$.attestation.dsse_envelope")?,
        "$.attestation.dsse_envelope.signatures",
    )?;
    let estimated_decoded = payload.len().saturating_add(3) / 4 * 3;
    if estimated_decoded > limits.dsse_bytes {
        return Err(error(
            "$.attestation.dsse_envelope.payload",
            format!(
                "decoded DSSE statement may exceed {} bytes",
                limits.dsse_bytes
            ),
        ));
    }
    let decoded = BASE64_STANDARD
        .decode(payload)
        .map_err(|_| error("$.attestation.dsse_envelope.payload", "must be base64"))?;
    let decoded = decode_bounded_json(
        &decoded,
        limits,
        "$.attestation.dsse_envelope.payload",
        true,
    )?;
    if decoded != Value::Object(statement.clone()) {
        return Err(error(
            "$.attestation.dsse_envelope.payload",
            "does not encode attestation.statement",
        ));
    }
    required(
        predicate,
        RECEIPT_BODY_BINDING_FIELD,
        "$.attestation.statement.predicate",
    )?;
    validate_bound_statement_projection(receipt, statement)
}

/// Graded acceptance scope from the receipt's acceptance layer. This differs
/// from verifier gate status and from the status-event ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceScope {
    MachineVerified,
    HumanSeen,
    LocallyAccepted,
    FrontierAccepted,
    CanonAccepted,
    HypothesisOnly,
    Retracted,
    Superseded,
}

impl AcceptanceScope {
    const ALL: [AcceptanceScope; 8] = [
        AcceptanceScope::MachineVerified,
        AcceptanceScope::HumanSeen,
        AcceptanceScope::LocallyAccepted,
        AcceptanceScope::FrontierAccepted,
        AcceptanceScope::CanonAccepted,
        AcceptanceScope::HypothesisOnly,
        AcceptanceScope::Retracted,
        AcceptanceScope::Superseded,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MachineVerified => "machine_verified",
            Self::HumanSeen => "human_seen",
            Self::LocallyAccepted => "locally_accepted",
            Self::FrontierAccepted => "frontier_accepted",
            Self::CanonAccepted => "canon_accepted",
            Self::HypothesisOnly => "hypothesis_only",
            Self::Retracted => "retracted",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|item| item.as_str() == value)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiptLineage {
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub derived_from: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub producer_run_id: Option<String>,
    #[serde(default)]
    pub frontier: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndependenceBasis {
    #[serde(default)]
    pub method_family: String,
    #[serde(default)]
    pub solver_identity: String,
    #[serde(default)]
    pub code_lineage: String,
    #[serde(default)]
    pub dataset_lineage: String,
    #[serde(default)]
    pub model_lineage: String,
    #[serde(default)]
    pub shared_dependencies: Vec<String>,
    #[serde(default)]
    pub declared_independent_of: Vec<String>,
    #[serde(default)]
    pub known_couplings: Vec<String>,
}

pub fn lineage_from_receipt(receipt: &Value) -> Option<ReceiptLineage> {
    lineage_from_layer(receipt.get("lineage")?)
}

pub fn lineage_from_layer(layer: &Value) -> Option<ReceiptLineage> {
    layer
        .is_object()
        .then(|| serde_json::from_value(layer.clone()).ok())
        .flatten()
}

pub fn independence_basis_from_environment(environment: &Value) -> Option<IndependenceBasis> {
    let basis = environment.get("independence_basis")?;
    basis
        .is_object()
        .then(|| serde_json::from_value(basis.clone()).ok())
        .flatten()
}

pub fn acceptance_scope_from_receipt(receipt: &Value) -> Option<AcceptanceScope> {
    receipt
        .get("acceptance")?
        .get("acceptance_scope")?
        .as_str()
        .and_then(AcceptanceScope::parse)
}

// Authoring facts are not a second wire format. Foreign producers import
// complete Receipt v1 JSON. The narrow public builder accepts only neutral
// producer evidence plus an independently verifiable agent identity binding.
mod authoring {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use serde_json::{Value, json};

    use super::{
        ACCEPTANCE_MECHANISM, INTOTO_PAYLOAD_TYPE, NEUTRAL_RECEIPT_GENERATOR, NO_ACTIVE_POLICY_REF,
        ReceiptV1, ReceiptV1Error, canonical_receipt_bytes, error, object, statement_projection,
    };
    use crate::identity::{ActorClass, IdentityBinding};

    #[derive(Debug, Clone)]
    pub(super) struct ProducerContext {
        actor: String,
        generated_by: String,
        producer_pubkey_hex: String,
        identity_binding_id: String,
        identity_binding: IdentityBinding,
        policy_ref: String,
        event_log_root: String,
        emitted_at: String,
        base_path: String,
        operation_id: String,
        task_contract_root: Option<String>,
    }

    impl ProducerContext {
        fn from_verified_identity(
            identity: &IdentityBinding,
            actor: &str,
            event_log_root: &str,
            emitted_at: &str,
            base_path: &str,
            operation_id: &str,
            policy_ref: &str,
        ) -> Result<Self, ReceiptV1Error> {
            identity.verify().map_err(|cause| {
                error(
                    "$.signature_identities.producer",
                    format!("identity binding does not verify: {cause}"),
                )
            })?;
            if identity.actor_class != ActorClass::Agent {
                return Err(error(
                    "$.provenance.submitter.actor",
                    "producer capability must bind an agent-class identity",
                ));
            }
            if identity.actor_id != actor {
                return Err(error(
                    "$.provenance.submitter.actor",
                    "mechanical actor must exactly match the verified identity binding",
                ));
            }
            validate_actor(actor)?;
            for (path, value) in [
                ("$.event_log_root", event_log_root),
                ("$.base_path", base_path),
                ("$.operation_id", operation_id),
            ] {
                if value.trim().is_empty() {
                    return Err(error(path, "must be supplied explicitly"));
                }
            }
            chrono::DateTime::parse_from_rfc3339(emitted_at)
                .map_err(|_| error("$.provenance.emitted_at", "must be an RFC 3339 date-time"))?;
            validate_policy_ref(policy_ref)?;
            Ok(Self {
                actor: actor.to_string(),
                generated_by: NEUTRAL_RECEIPT_GENERATOR.to_string(),
                producer_pubkey_hex: identity.public_key_hex.clone(),
                identity_binding_id: identity.binding_id.clone(),
                identity_binding: identity.clone(),
                policy_ref: policy_ref.to_string(),
                event_log_root: event_log_root.to_string(),
                emitted_at: emitted_at.to_string(),
                base_path: base_path.to_string(),
                operation_id: operation_id.to_string(),
                task_contract_root: None,
            })
        }
    }

    fn validate_actor(actor: &str) -> Result<(), ReceiptV1Error> {
        let suffix = actor
            .strip_prefix("agent:")
            .or_else(|| actor.strip_prefix("ci:"))
            .filter(|suffix| !suffix.trim().is_empty())
            .ok_or_else(|| {
                error(
                    "$.provenance.submitter.actor",
                    "must be a non-empty agent: or ci: identity",
                )
            })?;
        if suffix != suffix.trim() || suffix.chars().any(char::is_control) {
            return Err(error(
                "$.provenance.submitter.actor",
                "identity suffix must be trimmed and contain no controls",
            ));
        }
        Ok(())
    }

    fn validate_policy_ref(policy_ref: &str) -> Result<(), ReceiptV1Error> {
        if policy_ref == NO_ACTIVE_POLICY_REF
            || policy_ref
                .strip_prefix("vap_")
                .is_some_and(|hex| is_lower_hex(hex, 32))
        {
            return Ok(());
        }
        Err(error(
            "$.acceptance.policyRef",
            "must be a vap_ content address or urn:vela:policy:none",
        ))
    }

    #[derive(Debug, Clone)]
    pub struct ArtifactInput {
        path: String,
        kind: String,
        sha256: Option<String>,
        uri: Option<String>,
    }

    impl ArtifactInput {
        pub fn new(
            path: String,
            kind: String,
            sha256: Option<String>,
            uri: Option<String>,
        ) -> Result<Self, ReceiptV1Error> {
            if path.trim().is_empty() || kind.trim().is_empty() {
                return Err(error(
                    "$.artifacts",
                    "artifact path and kind must be explicit non-empty strings",
                ));
            }
            if let Some(hash) = &sha256
                && (hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            {
                return Err(error(
                    "$.artifacts.sha256",
                    "must be 64 lowercase hexadecimal characters",
                ));
            }
            Ok(Self {
                path,
                kind,
                sha256,
                uri,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct VerifierRunInput {
        method: String,
        outcome: String,
    }

    impl VerifierRunInput {
        /// Producer-reported verifier data is portable evidence only. This
        /// constructor deliberately cannot mint a frozen-verifier capability.
        pub fn producer_reported(method: String, outcome: String) -> Result<Self, ReceiptV1Error> {
            if method.trim().is_empty() {
                return Err(error("$.verifier_runs.method", "must not be empty"));
            }
            if !super::VERIFIER_OUTCOMES.contains(&outcome.as_str()) {
                return Err(error(
                    "$.verifier_runs.outcome",
                    "is not a published Receipt v1 outcome",
                ));
            }
            Ok(Self { method, outcome })
        }
    }

    /// Authority-neutral inputs for a first-party Receipt v1 emission.
    ///
    /// The fields deliberately contain no acceptance scope, status authority,
    /// verifier capability, key, or attestation data. The policy reference is
    /// only a validated content-address locator (or the explicit no-policy
    /// sentinel), never an authority knob. A separately supplied identity
    /// binding must verify and exactly match the mechanical actor.
    #[derive(Debug, Clone)]
    pub struct NeutralReceiptInput {
        claim: String,
        claim_type: String,
        replayability: String,
        artifacts: Vec<ArtifactInput>,
        caveats: Vec<String>,
        verifier_runs: Vec<VerifierRunInput>,
        actor: String,
        emitted_at: String,
        event_log_root: String,
        base_path: String,
        operation_id: String,
        policy_ref: String,
        task_contract_root: Option<String>,
    }

    impl NeutralReceiptInput {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            claim: String,
            claim_type: String,
            replayability: String,
            artifacts: Vec<ArtifactInput>,
            caveats: Vec<String>,
            verifier_runs: Vec<VerifierRunInput>,
            actor: String,
            emitted_at: String,
            event_log_root: String,
            base_path: String,
            operation_id: String,
            policy_ref: String,
        ) -> Result<Self, ReceiptV1Error> {
            let input = Self {
                claim,
                claim_type,
                replayability,
                artifacts,
                caveats,
                verifier_runs,
                actor,
                emitted_at,
                event_log_root,
                base_path,
                operation_id,
                policy_ref,
                task_contract_root: None,
            };
            validate_neutral_input(&input)?;
            Ok(input)
        }

        /// Bind the private work-session task contract into portable receipt
        /// provenance without promoting it to an authority or protocol object.
        pub fn with_task_contract_root(
            mut self,
            task_contract_root: String,
        ) -> Result<Self, ReceiptV1Error> {
            if !is_sha256_ref(&task_contract_root) {
                return Err(error(
                    "$.environment.vela:producer_context.task_contract_root",
                    "must be sha256: followed by 64 lowercase hexadecimal characters",
                ));
            }
            self.task_contract_root = Some(task_contract_root);
            Ok(self)
        }
    }

    #[derive(Debug, Clone)]
    pub(super) struct ReceiptInput {
        claim: String,
        claim_type: String,
        replayability: String,
        artifacts: Vec<ArtifactInput>,
        caveats: Vec<String>,
        verifier_runs: Vec<VerifierRunInput>,
        producer: ProducerContext,
    }

    impl ReceiptInput {
        #[allow(clippy::too_many_arguments)]
        pub(super) fn new(
            claim: String,
            claim_type: String,
            replayability: String,
            artifacts: Vec<ArtifactInput>,
            caveats: Vec<String>,
            verifier_runs: Vec<VerifierRunInput>,
            producer: ProducerContext,
        ) -> Result<Self, ReceiptV1Error> {
            let input = Self {
                claim,
                claim_type,
                replayability,
                artifacts,
                caveats,
                verifier_runs,
                producer,
            };
            validate_input(&input)?;
            Ok(input)
        }
    }

    pub struct ReceiptBuilder;

    impl ReceiptBuilder {
        /// Build a schema-compatible producer receipt without acceptance or
        /// verifier authority. The supplied binding is verified and embedded
        /// as portable proof of key possession; it does not sign the receipt.
        pub fn build(
            input: NeutralReceiptInput,
            identity: &IdentityBinding,
        ) -> Result<ReceiptV1, ReceiptV1Error> {
            validate_neutral_input(&input)?;
            let producer = ProducerContext::from_verified_identity(
                identity,
                &input.actor,
                &input.event_log_root,
                &input.emitted_at,
                &input.base_path,
                &input.operation_id,
                &input.policy_ref,
            )?;
            let task_contract_root = input.task_contract_root.clone();
            let NeutralReceiptInput {
                claim,
                claim_type,
                replayability,
                artifacts,
                caveats,
                verifier_runs,
                ..
            } = input;
            let mut validated = ReceiptInput::new(
                claim,
                claim_type,
                replayability,
                artifacts,
                caveats,
                verifier_runs,
                producer,
            )?;
            validated.producer.task_contract_root = task_contract_root;
            Self::build_validated(validated)
        }

        fn build_validated(input: ReceiptInput) -> Result<ReceiptV1, ReceiptV1Error> {
            validate_input(&input)?;
            let artifacts: Vec<Value> = input
                .artifacts
                .iter()
                .map(|artifact| {
                    let mut value = json!({"path": artifact.path, "kind": artifact.kind});
                    if let Some(hash) = &artifact.sha256 {
                        value["sha256"] = json!(hash);
                    }
                    if let Some(uri) = &artifact.uri {
                        value["uri"] = json!(uri);
                    }
                    value
                })
                .collect();
            let subjects: Vec<Value> = input
                .artifacts
                .iter()
                .map(|artifact| {
                    let mut value = json!({"name": artifact.path});
                    if let Some(hash) = &artifact.sha256 {
                        value["digest"] = json!({"sha256": hash});
                    }
                    if let Some(uri) = &artifact.uri {
                        value["uri"] = json!(uri);
                    }
                    value
                })
                .collect();
            let verifier_runs: Vec<Value> = input
                .verifier_runs
                .iter()
                .map(|run| json!({"method": run.method, "outcome": run.outcome}))
                .collect();
            let scope = "hypothesis_only";
            let machine_status = if input.verifier_runs.is_empty() {
                "not_assessed"
            } else {
                "producer_reported"
            };

            let provenance = json!({
                "generated_by": input.producer.generated_by,
                "emitted_at": input.producer.emitted_at,
                "submitter": {
                    "actor": input.producer.actor,
                    "operation_id": input.producer.operation_id,
                    "identity_binding_ref": input.producer.identity_binding_id,
                }
            });
            let machine = json!({
                "subject": subjects,
                "claim": {"text": input.claim, "type": input.claim_type},
                "verification": {
                    "status": machine_status,
                    "verifier_runs": verifier_runs,
                    "trust_base": {
                        "kind": "producer_reported",
                        "authority": "producer",
                    }
                }
            });
            let acceptance = json!({
                "profile": "producer.emission.v1",
                "mechanism": ACCEPTANCE_MECHANISM,
                "acceptor": null,
                "policyRef": input.producer.policy_ref,
                "evidenceRefs": [],
                "artifact_verification": {"status": "not_assessed", "authority": "producer"},
                "claim_acceptance": {
                    "status": "not_assessed",
                    "accepted_by": null,
                    "authority": "producer",
                },
                "distillation_acceptance": {"status": "not_assessed", "accepted_by": null},
                "acceptance_scope": scope,
            });
            let distillation = json!({
                "status": "missing",
                "audience": "unspecified",
                "rubric": "not_assessed",
            });
            let lineage = json!({"parents": [], "derived_from": [], "source_refs": []});
            let contributors = json!([{
                "id": input.producer.actor,
                "roles": ["machine_producer"],
                "author": false,
                "note": "Origin identity only; not scientific acceptance.",
            }]);
            let identities = json!({
                "producer": {
                    "role": "producer",
                    "signatureRef": input.producer.identity_binding_id,
                    "mechanism": "ed25519_key_custody_ceremony",
                    "subject": input.producer.actor,
                    "publicKey": input.producer.producer_pubkey_hex,
                    "identityBindingRef": input.producer.identity_binding_id,
                    "note": "Self-signed origin binding only; it does not sign this unsigned DSSE receipt or confer scientific acceptance.",
                }
            });
            let mut receipt = json!({
                "schema": "vela.receipt.v1",
                "claim": input.claim,
                "type": input.claim_type,
                "replayability": input.replayability,
                "artifacts": artifacts,
                "caveats": input.caveats,
                "verifier_runs": verifier_runs,
                "environment": {
                    "vela:producer_context": {
                        "actor": input.producer.actor,
                        "event_log_root": input.producer.event_log_root,
                        "base_path": input.producer.base_path,
                        "operation_id": input.producer.operation_id,
                        "policy_ref": input.producer.policy_ref,
                        "identity_binding_ref": input.producer.identity_binding_id,
                        "identity_binding": input.producer.identity_binding,
                    }
                },
                "provenance": provenance,
                "status": {
                    "kind": "emitted",
                    "authority": "producer",
                    "evidence_status": if input.verifier_runs.is_empty() {"proposed"} else {"runs"},
                    "scope": {"acceptance_scope": scope},
                    "note": "Producer emission only. Landing and acceptance are separate.",
                },
                "machine": machine,
                "acceptance": acceptance,
                "distillation": distillation,
                "lineage": lineage,
                "contributors": contributors,
                "signature_identities": identities,
            });
            if let Some(task_contract_root) = input.producer.task_contract_root {
                receipt["environment"]["vela:producer_context"]["task_contract_root"] =
                    json!(task_contract_root);
            }
            let statement = statement_projection(object(&receipt, "$")?)?;
            let payload = canonical_receipt_bytes(&statement)
                .map_err(|cause| error("$.attestation.statement", cause.to_string()))?;
            let attestation = json!({
                "format": "in-toto-statement",
                "statement": statement,
                "dsse_envelope": {
                    "payloadType": INTOTO_PAYLOAD_TYPE,
                    "payload": BASE64_STANDARD.encode(payload),
                    "signatures": [],
                }
            });
            receipt["attestation"] = attestation;
            ReceiptV1::from_trusted_value(receipt)
        }
    }

    fn validate_neutral_input(input: &NeutralReceiptInput) -> Result<(), ReceiptV1Error> {
        validate_receipt_fields(
            &input.claim,
            &input.claim_type,
            &input.replayability,
            &input.artifacts,
            &input.caveats,
        )?;
        validate_actor(&input.actor)?;
        chrono::DateTime::parse_from_rfc3339(&input.emitted_at)
            .map_err(|_| error("$.provenance.emitted_at", "must be an RFC 3339 date-time"))?;
        if input.base_path.trim().is_empty()
            || input.base_path != input.base_path.trim()
            || input.base_path.chars().any(char::is_control)
        {
            return Err(error(
                "$.environment.vela:producer_context.base_path",
                "must be an explicit trimmed path without control characters",
            ));
        }
        if !is_sha256_ref(&input.event_log_root) {
            return Err(error(
                "$.environment.vela:producer_context.event_log_root",
                "must be sha256: followed by 64 lowercase hexadecimal characters",
            ));
        }
        if !is_operation_id(&input.operation_id) {
            return Err(error(
                "$.environment.vela:producer_context.operation_id",
                "must be vop_ followed by 64 lowercase hexadecimal characters",
            ));
        }
        if input
            .task_contract_root
            .as_deref()
            .is_some_and(|root| !is_sha256_ref(root))
        {
            return Err(error(
                "$.environment.vela:producer_context.task_contract_root",
                "must be sha256: followed by 64 lowercase hexadecimal characters",
            ));
        }
        validate_policy_ref(&input.policy_ref)?;
        Ok(())
    }

    fn is_sha256_ref(value: &str) -> bool {
        value
            .strip_prefix("sha256:")
            .is_some_and(|hex| is_lower_hex(hex, 64))
    }

    fn is_operation_id(value: &str) -> bool {
        value
            .strip_prefix("vop_")
            .is_some_and(|hex| is_lower_hex(hex, 64))
    }

    fn is_lower_hex(value: &str, length: usize) -> bool {
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn validate_input(input: &ReceiptInput) -> Result<(), ReceiptV1Error> {
        validate_receipt_fields(
            &input.claim,
            &input.claim_type,
            &input.replayability,
            &input.artifacts,
            &input.caveats,
        )
    }

    fn validate_receipt_fields(
        claim: &str,
        claim_type: &str,
        replayability: &str,
        artifacts: &[ArtifactInput],
        caveats: &[String],
    ) -> Result<(), ReceiptV1Error> {
        if claim.trim().is_empty() {
            return Err(error("$.claim", "must be an explicit non-empty claim"));
        }
        if artifacts.is_empty() {
            return Err(error("$.artifacts", "at least one reference is required"));
        }
        if caveats.is_empty() || caveats.iter().any(|item| item.trim().is_empty()) {
            return Err(error(
                "$.caveats",
                "explicit non-empty caveats are required",
            ));
        }
        if !super::CLAIM_TYPES.contains(&claim_type) {
            return Err(error("$.type", "is not a published Receipt v1 claim type"));
        }
        if !super::REPLAYABILITY.contains(&replayability) {
            return Err(error(
                "$.replayability",
                "is not a published Receipt v1 replayability class",
            ));
        }
        Ok(())
    }
}

pub use authoring::{
    ArtifactInput, NeutralReceiptInput as ReceiptInput, ReceiptBuilder,
    VerifierRunInput as ProducerReportedRun,
};

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use crate::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};

    type ReceiptMutation = (&'static str, Box<dyn Fn(&mut Value)>);

    fn identity(actor: &str) -> IdentityBinding {
        IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: actor.into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-13T12:34:55Z".into(),
            },
            &SigningKey::from_bytes(&[0x42; 32]),
        )
        .unwrap()
    }

    fn input(runs: Vec<ProducerReportedRun>) -> ReceiptInput {
        input_for_actor(runs, "agent:receipt-test")
    }

    fn input_for_actor(runs: Vec<ProducerReportedRun>, actor: &str) -> ReceiptInput {
        try_input_for_actor(runs, actor).unwrap()
    }

    fn try_input_for_actor(
        runs: Vec<ProducerReportedRun>,
        actor: &str,
    ) -> Result<ReceiptInput, ReceiptV1Error> {
        try_input_with_mechanics(
            runs,
            actor,
            &format!("sha256:{}", "c".repeat(64)),
            &format!("vop_{}", "d".repeat(64)),
            NO_ACTIVE_POLICY_REF,
        )
    }

    fn try_input_with_mechanics(
        runs: Vec<ProducerReportedRun>,
        actor: &str,
        event_log_root: &str,
        operation_id: &str,
        policy_ref: &str,
    ) -> Result<ReceiptInput, ReceiptV1Error> {
        ReceiptInput::new(
            "The bounded witness has the declared checksum.".into(),
            "computational".into(),
            "exact".into(),
            vec![
                ArtifactInput::new(
                    "witnesses/result.json".into(),
                    "witness".into(),
                    Some("a".repeat(64)),
                    Some("https://example.test/result.json".into()),
                )
                .unwrap(),
            ],
            vec!["This does not establish the unbounded claim.".into()],
            runs,
            actor.into(),
            "2026-07-13T12:34:56Z".into(),
            event_log_root.into(),
            env!("CARGO_MANIFEST_DIR").into(),
            operation_id.into(),
            policy_ref.into(),
        )
    }

    fn build(runs: Vec<ProducerReportedRun>) -> ReceiptV1 {
        ReceiptBuilder::build(input(runs), &identity("agent:receipt-test")).unwrap()
    }

    fn schema() -> Value {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/vela.receipt.v1.schema.json"
        ))
        .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn refresh_payload_only(receipt: &mut Value) {
        let bytes = canonical_receipt_bytes(&receipt["attestation"]["statement"]).unwrap();
        receipt["attestation"]["dsse_envelope"]["payload"] = json!(BASE64_STANDARD.encode(bytes));
    }

    fn refresh_bound_attestation(receipt: &mut Value) {
        let predicate_extensions = receipt["attestation"]["statement"]["predicate"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with("x:"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let mut statement = statement_projection(object(receipt, "$").unwrap()).unwrap();
        let predicate = statement["predicate"].as_object_mut().unwrap();
        for (key, value) in predicate_extensions {
            predicate.insert(key, value);
        }
        receipt["attestation"]["statement"] = statement;
        refresh_payload_only(receipt);
    }

    fn restricted_receipt(locator: &str) -> Value {
        let mut receipt = build(Vec::new()).into_value();
        receipt["artifacts"][0] = json!({
            "path": locator,
            "kind": "restricted_witness",
            "disclosure": "restricted",
            "media_type": "application/octet-stream",
            "locator_integrity": "unknown",
            "availability": "available",
        });
        receipt["machine"]["subject"][0] = json!({"name": locator});
        refresh_bound_attestation(&mut receipt);
        receipt
    }

    fn reported_pass() -> ProducerReportedRun {
        ProducerReportedRun::producer_reported("producer.check".into(), "pass".into()).unwrap()
    }

    #[test]
    fn builder_emits_complete_frozen_shape_without_authority_invention() {
        let receipt = build(Vec::new());
        let value = receipt.as_value();
        let schema = schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect();
        assert_eq!(required, RECEIPT_V1_REQUIRED_FIELDS);
        assert!(required.iter().all(|field| value.get(field).is_some()));
        assert_eq!(value["schema"], schema["properties"]["schema"]["const"]);
        assert_eq!(value["status"]["authority"], "producer");
        assert_eq!(value["machine"]["verification"]["status"], "not_assessed");
        assert_eq!(value["acceptance"]["acceptor"], Value::Null);
        assert_eq!(value["acceptance"]["evidenceRefs"], json!([]));
        assert_eq!(
            value["acceptance"]["claim_acceptance"]["status"],
            "not_assessed"
        );
        assert_eq!(value["acceptance"]["acceptance_scope"], "hypothesis_only");
        assert_eq!(value["acceptance"]["policyRef"], NO_ACTIVE_POLICY_REF);
        assert_eq!(
            value["signature_identities"]["producer"]["mechanism"],
            "ed25519_key_custody_ceremony"
        );
        assert_eq!(
            value["signature_identities"]["producer"]["publicKey"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert!(
            value["signature_identities"]["producer"]["identityBindingRef"]
                .as_str()
                .unwrap()
                .starts_with("vib_")
        );
        assert_eq!(
            value["signature_identities"]["producer"]["signatureRef"],
            value["signature_identities"]["producer"]["identityBindingRef"]
        );
        let embedded: IdentityBinding = serde_json::from_value(
            value["environment"]["vela:producer_context"]["identity_binding"].clone(),
        )
        .unwrap();
        embedded.verify().unwrap();
        assert_eq!(
            value["attestation"]["dsse_envelope"]["signatures"],
            json!([])
        );
        assert!(value["environment"].get("source").is_none());
        assert!(value["environment"].get("independence_basis").is_none());
        assert_eq!(
            value["attestation"]["statement"]["predicate"][RECEIPT_BODY_BINDING_FIELD]["sha256"],
            receipt_body_sha256(object(value, "$").unwrap()).unwrap()
        );
        validate_schema_exact(value).unwrap();
        assert!(
            !String::from_utf8(receipt.canonical_bytes().unwrap())
                .unwrap()
                .contains("sigstore")
        );
        assert!(
            !String::from_utf8(receipt.canonical_bytes().unwrap())
                .unwrap()
                .contains("_fixture")
        );
    }

    #[test]
    fn builder_never_infers_authority_from_reported_runs_or_raw_identity_strings() {
        let receipt = build(vec![reported_pass()]);
        assert_eq!(
            receipt.as_value()["acceptance"]["acceptance_scope"],
            "hypothesis_only"
        );
        assert_eq!(
            receipt.as_value()["machine"]["verification"]["status"],
            "producer_reported"
        );
        assert_eq!(
            receipt.as_value()["acceptance"]["claim_acceptance"]["status"],
            "not_assessed"
        );
        assert!(
            !String::from_utf8(receipt.canonical_bytes().unwrap())
                .unwrap()
                .contains("machine_verified")
        );

        assert_eq!(
            try_input_for_actor(Vec::new(), "agent:")
                .unwrap_err()
                .path(),
            "$.provenance.submitter.actor"
        );
        let mut forged_identity = identity("agent:receipt-test");
        forged_identity.public_key_hex = "b".repeat(64);
        assert!(ReceiptBuilder::build(input(Vec::new()), &forged_identity).is_err());
        assert_eq!(
            ReceiptBuilder::build(
                input_for_actor(Vec::new(), "agent:receipt-test"),
                &identity("agent:other"),
            )
            .unwrap_err()
            .path(),
            "$.provenance.submitter.actor"
        );
    }

    #[test]
    fn authoring_api_rejects_untyped_mechanical_and_policy_references() {
        assert_eq!(
            try_input_with_mechanics(
                Vec::new(),
                "agent:receipt-test",
                "sha256:not-a-root",
                &format!("vop_{}", "d".repeat(64)),
                NO_ACTIVE_POLICY_REF,
            )
            .unwrap_err()
            .path(),
            "$.environment.vela:producer_context.event_log_root"
        );
        assert_eq!(
            try_input_with_mechanics(
                Vec::new(),
                "agent:receipt-test",
                &format!("sha256:{}", "c".repeat(64)),
                "vop_short",
                NO_ACTIVE_POLICY_REF,
            )
            .unwrap_err()
            .path(),
            "$.environment.vela:producer_context.operation_id"
        );
        assert_eq!(
            try_input_with_mechanics(
                Vec::new(),
                "agent:receipt-test",
                &format!("sha256:{}", "c".repeat(64)),
                &format!("vop_{}", "d".repeat(64)),
                "vap_invented",
            )
            .unwrap_err()
            .path(),
            "$.acceptance.policyRef"
        );

        let policy_ref = format!("vap_{}", "e".repeat(32));
        let receipt = ReceiptBuilder::build(
            try_input_with_mechanics(
                Vec::new(),
                "agent:receipt-test",
                &format!("sha256:{}", "c".repeat(64)),
                &format!("vop_{}", "d".repeat(64)),
                &policy_ref,
            )
            .unwrap(),
            &identity("agent:receipt-test"),
        )
        .unwrap();
        assert_eq!(receipt.as_value()["acceptance"]["policyRef"], policy_ref);
        assert_eq!(
            receipt.as_value()["acceptance"]["acceptance_scope"],
            "hypothesis_only"
        );
    }

    #[test]
    fn task_contract_root_extension_is_validated_and_bound_on_import() {
        let root = format!("sha256:{}", "e".repeat(64));
        let receipt = ReceiptBuilder::build(
            input(Vec::new())
                .with_task_contract_root(root.clone())
                .unwrap(),
            &identity("agent:receipt-test"),
        )
        .unwrap();
        assert_eq!(
            receipt.as_value()["environment"]["vela:producer_context"]["task_contract_root"],
            root
        );
        ReceiptV1::parse(&receipt.canonical_bytes().unwrap()).unwrap();

        assert_eq!(
            input(Vec::new())
                .with_task_contract_root("sha256:not-a-root".to_string())
                .unwrap_err()
                .path(),
            "$.environment.vela:producer_context.task_contract_root"
        );

        let mut malformed = receipt.into_value();
        malformed["environment"]["vela:producer_context"]["task_contract_root"] =
            json!("sha256:NOT-CANONICAL");
        refresh_bound_attestation(&mut malformed);
        let failure = ReceiptV1::parse(&serde_json::to_vec(&malformed).unwrap()).unwrap_err();
        assert_eq!(
            failure.path(),
            "$.environment.vela:producer_context.task_contract_root"
        );
    }

    #[test]
    fn embedded_identity_binding_is_verified_after_clean_clone_import() {
        let receipt = build(Vec::new());
        let reparsed = ReceiptV1::parse(&receipt.canonical_bytes().unwrap()).unwrap();
        let embedded: IdentityBinding = serde_json::from_value(
            reparsed.as_value()["environment"]["vela:producer_context"]["identity_binding"].clone(),
        )
        .unwrap();
        embedded.verify().unwrap();

        let mut tampered = receipt.into_value();
        tampered["environment"]["vela:producer_context"]["identity_binding"]["signature"] =
            json!("00".repeat(64));
        refresh_bound_attestation(&mut tampered);
        let failure = ReceiptV1::parse(&serde_json::to_vec(&tampered).unwrap()).unwrap_err();
        assert_eq!(
            failure.path(),
            "$.environment.vela:producer_context.identity_binding"
        );
        assert!(failure.message().contains("proof of possession"));
    }

    #[test]
    fn current_schema_accepts_the_protocol_builder() {
        let schema = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/schemas/vela.receipt.v1.schema.json"
        ))
        .unwrap();
        assert!(!schema.is_empty());
        let receipt = build(Vec::new());
        validate_schema_exact(receipt.as_value()).unwrap();
    }

    #[test]
    fn rich_unknown_extensions_round_trip_losslessly_and_bind_root() {
        let initial = build(Vec::new());
        let initial_body = receipt_body_sha256(object(initial.as_value(), "$").unwrap()).unwrap();
        let mut raw = initial.into_value();
        raw["x:hep-belief"] =
            json!({"asserted_by": "collaboration:test", "confidence": ["0.7", "0.9"]});
        raw["environment"]["x:codegraph"] =
            json!({"root": "sha256:graph", "symbols": [{"name": "α", "span": [7, 11]}]});
        raw["acceptance"]["x:external-certificates"] =
            json!([{"profile": "independent-review.v2", "certificate": {"opaque": true}}]);
        raw["lineage"]["derived_from"] = json!(["arxiv:2607.09195"]);
        raw["attestation"]["statement"]["predicate"]["x:article-machine"] = json!({"document": "producer:article-machine", "sections": ["definition", "verification"]});
        refresh_bound_attestation(&mut raw);

        let parsed = ReceiptV1::parse(&serde_json::to_vec_pretty(&raw).unwrap()).unwrap();
        assert_ne!(
            initial_body,
            receipt_body_sha256(object(parsed.as_value(), "$").unwrap()).unwrap()
        );
        assert_eq!(parsed.as_value(), &raw);
        assert_eq!(serde_json::to_value(&parsed).unwrap(), raw);
        let bytes = canonical_receipt_bytes(&raw).unwrap();
        assert_eq!(parsed.canonical_bytes().unwrap(), bytes);
        assert_eq!(
            parsed.canonical_root().unwrap(),
            format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
        );
        assert_eq!(ReceiptV1::parse(&bytes).unwrap().into_value(), raw);
    }

    #[test]
    fn restricted_artifacts_allow_only_the_reviewed_safe_public_descriptor() {
        for locator in ["opaque:custodian-fixture-7", "custodian:lab-a:artifact-17"] {
            let raw = restricted_receipt(locator);
            let parsed = ReceiptV1::parse(&serde_json::to_vec_pretty(&raw).unwrap()).unwrap();
            parsed.validate_safe_public_artifact_descriptors().unwrap();
            assert_eq!(
                parsed.as_value()["machine"]["subject"],
                json!([{"name": locator}])
            );
            assert_eq!(parsed.as_value(), &raw);
        }
    }

    #[test]
    fn restricted_artifact_subject_and_prov_mirrors_cannot_carry_secret_material() {
        let secret_digest = "7".repeat(64);

        let mut machine_digest = restricted_receipt("opaque:custodian-fixture-7");
        machine_digest["machine"]["subject"][0]["digest"] = json!({"sha256": secret_digest});
        refresh_bound_attestation(&mut machine_digest);
        let failure = ReceiptV1::parse(&serde_json::to_vec(&machine_digest).unwrap()).unwrap_err();
        assert_eq!(failure.path(), "$.machine.subject");

        let mut machine_opening = restricted_receipt("opaque:custodian-fixture-7");
        machine_opening["machine"]["subject"][0]["opening"] = json!("do-not-publish");
        refresh_bound_attestation(&mut machine_opening);
        let failure = ReceiptV1::parse(&serde_json::to_vec(&machine_opening).unwrap()).unwrap_err();
        assert_eq!(failure.path(), "$.machine.subject");

        let mut machine_location = restricted_receipt("opaque:custodian-fixture-7");
        machine_location["machine"]["subject"][0]["uri"] = json!("file:///private/secret.bin");
        refresh_bound_attestation(&mut machine_location);
        let failure =
            ReceiptV1::parse(&serde_json::to_vec(&machine_location).unwrap()).unwrap_err();
        assert_eq!(failure.path(), "$.machine.subject");

        let mut statement_location = restricted_receipt("opaque:custodian-fixture-7");
        statement_location["attestation"]["statement"]["subject"][0]["uri"] =
            json!("https://internal.invalid/secret.bin");
        refresh_payload_only(&mut statement_location);
        let failure =
            ReceiptV1::parse(&serde_json::to_vec(&statement_location).unwrap()).unwrap_err();
        assert_eq!(failure.path(), "$.attestation.statement.subject");

        let mut predicate_digest = restricted_receipt("opaque:custodian-fixture-7");
        predicate_digest["attestation"]["statement"]["predicate"]["machine"]["subject"][0]["digest"] =
            json!({"sha256": "8".repeat(64)});
        refresh_payload_only(&mut predicate_digest);
        let failure =
            ReceiptV1::parse(&serde_json::to_vec(&predicate_digest).unwrap()).unwrap_err();
        assert_eq!(
            failure.path(),
            "$.attestation.statement.predicate.machine.subject"
        );

        for (field, value) in [
            ("vela:sha256", json!("9".repeat(64))),
            ("opening", json!("do-not-publish")),
            ("location", json!("/private/secret.bin")),
        ] {
            let mut prov = restricted_receipt("opaque:custodian-fixture-7");
            prov["attestation"]["prov"] = json!({
                "entity": {
                    "artifact:opaque:custodian-fixture-7": {
                        "prov:type": "vela:artifact",
                        "vela:kind": "restricted_witness",
                        (field): value,
                    }
                }
            });
            let failure = ReceiptV1::parse(&serde_json::to_vec(&prov).unwrap()).unwrap_err();
            assert_eq!(
                failure.path(),
                format!("$.attestation.prov.entity.artifact:opaque:custodian-fixture-7.{field}")
            );
        }
    }

    #[test]
    fn mixed_public_and_restricted_subjects_are_derived_without_losing_public_digests() {
        let mut raw = restricted_receipt("opaque:custodian-fixture-7");
        raw["artifacts"].as_array_mut().unwrap().push(json!({
            "path": "artifacts/public.json",
            "kind": "witness",
            "sha256": "a".repeat(64),
        }));
        raw["machine"]["subject"] = json!([
            {"name": "opaque:custodian-fixture-7"},
            {"name": "artifacts/public.json", "digest": {"sha256": "a".repeat(64)}},
        ]);
        refresh_bound_attestation(&mut raw);
        let parsed = ReceiptV1::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(parsed.as_value(), &raw);
    }

    #[test]
    fn restricted_artifacts_reject_payload_opening_location_digest_size_and_extensions() {
        let cases = [
            ("opening", json!("do-not-publish")),
            ("content", json!("do-not-publish")),
            ("data", json!({"value": "do-not-publish"})),
            ("payload", json!("do-not-publish")),
            ("bytes", json!([115, 101, 99, 114, 101, 116])),
            ("plaintext", json!("do-not-publish")),
            ("secret", json!("do-not-publish")),
            ("inline", json!("do-not-publish")),
            ("uri", json!("file:///private/secret.bin")),
            ("url", json!("https://internal.invalid/secret.bin")),
            ("location", json!("/private/secret.bin")),
            ("sha256", json!("a".repeat(64))),
            ("digest", json!({"sha256": "a".repeat(64)})),
            ("size", json!(17)),
            ("size_bytes", json!(17)),
            ("x:apparently-safe", json!({"reviewed": false})),
            ("x:opening", json!("do-not-publish")),
        ];
        for (field, value) in cases {
            let mut raw = restricted_receipt("opaque:custodian-fixture-7");
            raw["artifacts"][0][field] = value;
            refresh_bound_attestation(&mut raw);
            let failure = ReceiptV1::parse(&serde_json::to_vec(&raw).unwrap()).unwrap_err();
            assert_eq!(failure.path(), format!("$.artifacts[0].{field}"));
            assert!(
                failure.message().contains("not permitted"),
                "{field}: {failure}"
            );
        }
    }

    #[test]
    fn restricted_artifact_locator_is_opaque_and_non_resolving() {
        for locator in [
            "witnesses/secret.bin",
            "https://example.invalid/secret.bin",
            "file:///private/secret.bin",
            "opaque:",
            "opaque:https://example.invalid/secret.bin",
            "custodian:../secret",
            "opaque:has whitespace",
        ] {
            let raw = restricted_receipt(locator);
            let failure = ReceiptV1::parse(&serde_json::to_vec(&raw).unwrap()).unwrap_err();
            assert_eq!(failure.path(), "$.artifacts[0].path", "{locator}");
        }
    }

    #[test]
    fn alternate_sensitive_visibility_cannot_bypass_canonical_disclosure() {
        for (field, value) in [
            ("visibility", "restricted"),
            ("access_tier", "classified"),
            ("accessTier", "private"),
        ] {
            let mut raw = build(Vec::new()).into_value();
            raw["artifacts"][0][field] = json!(value);
            refresh_bound_attestation(&mut raw);
            let failure = ReceiptV1::parse(&serde_json::to_vec(&raw).unwrap()).unwrap_err();
            assert_eq!(failure.path(), format!("$.artifacts[0].{field}"));
            assert!(failure.message().contains("disclosure: restricted"));
        }
    }

    #[test]
    fn public_artifact_extensions_remain_lossless_and_compatible() {
        let mut raw = build(Vec::new()).into_value();
        raw["artifacts"][0]["x:producer-metadata"] =
            json!({"tool": "outside-producer", "result_class": "candidate"});
        refresh_bound_attestation(&mut raw);
        let parsed = ReceiptV1::parse(&serde_json::to_vec_pretty(&raw).unwrap()).unwrap();
        parsed.validate_safe_public_artifact_descriptors().unwrap();
        assert_eq!(parsed.into_value(), raw);
    }

    #[test]
    fn numeric_extensions_preserve_frozen_floats_and_bound_safe_integers() {
        let mut safe = build(Vec::new()).into_value();
        safe["x:numeric-domain"] = json!({
            "minimum": -(MAX_PORTABLE_JSON_INTEGER as i64),
            "maximum": MAX_PORTABLE_JSON_INTEGER,
            "zero": 0,
            "pre_adr_decimal": 0.61,
            "pre_adr_exponent": 0.000000125,
            "exponent_integer": 1000,
        });
        refresh_bound_attestation(&mut safe);
        let canonical = String::from_utf8(canonical_receipt_bytes(&safe).unwrap()).unwrap();
        assert!(canonical.contains(r#""pre_adr_decimal":0.61"#));
        assert!(canonical.contains(r#""pre_adr_exponent":1.25e-7"#));
        let pre_adr_wire = canonical
            .replace(
                r#""pre_adr_exponent":1.25e-7"#,
                r#""pre_adr_exponent":1.25E-7"#,
            )
            .replace(r#""exponent_integer":1000"#, r#""exponent_integer":1e3"#);
        let parsed = ReceiptV1::parse(pre_adr_wire.as_bytes()).unwrap();
        assert_eq!(
            parsed.as_value()["x:numeric-domain"]["pre_adr_decimal"],
            json!(0.61)
        );
        assert_eq!(
            parsed.as_value()["x:numeric-domain"]["pre_adr_exponent"],
            json!(0.000000125)
        );
        assert_eq!(
            parsed.as_value()["x:numeric-domain"]["exponent_integer"].as_f64(),
            Some(1000.0)
        );
        assert_eq!(parsed.canonical_bytes().unwrap(), canonical.as_bytes());
        assert_eq!(
            ReceiptV1::parse(canonical.as_bytes())
                .unwrap()
                .canonical_bytes()
                .unwrap(),
            canonical.as_bytes()
        );

        let canonical = String::from_utf8(build(Vec::new()).canonical_bytes().unwrap()).unwrap();
        for token in [
            "9007199254740992",
            "-9007199254740992",
            "9223372036854775807",
            "18446744073709551615",
            "18446744073709551616",
            "9007199254740992.0",
            "9.007199254740992e15",
        ] {
            let malicious = canonical.replacen('{', &format!("{{\"x:numeric\":{token},"), 1);
            let failure = ReceiptV1::parse(malicious.as_bytes()).unwrap_err();
            assert!(
                failure.message().contains("portable JSON range")
                    || failure.message().contains("integral JSON numbers"),
                "{token}: {failure}"
            );
        }

        let mut dsse = build(Vec::new()).into_value();
        let payload =
            String::from_utf8(canonical_receipt_bytes(&dsse["attestation"]["statement"]).unwrap())
                .unwrap();
        let payload = payload.replacen('{', "{\"x:numeric\":9.007199254740992e15,", 1);
        dsse["attestation"]["dsse_envelope"]["payload"] = json!(BASE64_STANDARD.encode(payload));
        let failure = ReceiptV1::parse(&serde_json::to_vec(&dsse).unwrap()).unwrap_err();
        assert_eq!(failure.path(), "$.attestation.dsse_envelope.payload");
        assert!(failure.message().contains("portable JSON range"));
    }

    #[test]
    fn duplicate_object_names_are_rejected_at_every_boundary() {
        let receipt = build(Vec::new());
        let canonical = String::from_utf8(receipt.canonical_bytes().unwrap()).unwrap();
        let claim = serde_json::to_string(&receipt.as_value()["claim"]).unwrap();
        let needle = format!("\"claim\":{claim}");
        for duplicate in [
            format!("{needle},\"claim\":\"shadow\""),
            format!("{needle},\"cl\\u0061im\":\"shadow\""),
        ] {
            let malicious = canonical.replacen(&needle, &duplicate, 1);
            let failure = ReceiptV1::parse(malicious.as_bytes()).unwrap_err();
            assert!(failure.message().contains("duplicate object name"));
        }

        let needle = "\"acceptance_scope\":\"hypothesis_only\"";
        let malicious = canonical.replacen(
            needle,
            &format!("{needle},\"acceptance_scope\":\"machine_verified\""),
            1,
        );
        assert!(
            ReceiptV1::parse(malicious.as_bytes())
                .unwrap_err()
                .message()
                .contains("duplicate object name")
        );

        let mut extension = receipt.into_value();
        extension["x:test"] = json!({"name": "first"});
        refresh_bound_attestation(&mut extension);
        let canonical = String::from_utf8(canonical_receipt_bytes(&extension).unwrap()).unwrap();
        let malicious = canonical.replacen(
            "\"name\":\"first\"",
            "\"name\":\"first\",\"name\":\"second\"",
            1,
        );
        assert!(
            ReceiptV1::parse(malicious.as_bytes())
                .unwrap_err()
                .message()
                .contains("duplicate object name")
        );

        let mut dsse = build(Vec::new()).into_value();
        let statement =
            String::from_utf8(canonical_receipt_bytes(&dsse["attestation"]["statement"]).unwrap())
                .unwrap();
        let duplicate_statement = statement.replacen(
            "\"_type\":",
            "\"_type\":\"https://in-toto.io/Statement/v1\",\"_type\":",
            1,
        );
        dsse["attestation"]["dsse_envelope"]["payload"] =
            json!(BASE64_STANDARD.encode(duplicate_statement.as_bytes()));
        let failure = ReceiptV1::parse(&serde_json::to_vec(&dsse).unwrap()).unwrap_err();
        assert!(failure.message().contains("duplicate object name"));
    }

    #[test]
    fn schema_exact_view_rejects_types_enums_and_closed_contributor_extensions() {
        let cases: Vec<ReceiptMutation> = vec![
            (
                "conditions object",
                Box::new(|raw| raw["conditions"] = json!(7)),
            ),
            (
                "conditions item",
                Box::new(|raw| raw["conditions"] = json!([7])),
            ),
            (
                "numeric evidence ref",
                Box::new(|raw| raw["acceptance"]["evidenceRefs"] = json!([7])),
            ),
            (
                "contributor extension",
                Box::new(|raw| raw["contributors"][0]["weight"] = json!(1)),
            ),
            (
                "nullable claim id",
                Box::new(|raw| raw["claim_id"] = Value::Null),
            ),
            (
                "bad status enum",
                Box::new(|raw| raw["status"]["evidence_status"] = json!("green")),
            ),
            (
                "numeric signature ref",
                Box::new(|raw| raw["signature_identities"]["producer"]["signatureRef"] = json!(7)),
            ),
            (
                "known gaps object",
                Box::new(|raw| raw["distillation"]["known_gaps"] = json!({})),
            ),
        ];
        for (name, mutate) in cases {
            let mut raw = build(Vec::new()).into_value();
            mutate(&mut raw);
            let failure = ReceiptV1::from_trusted_value(raw).unwrap_err();
            assert!(
                failure.message().contains("frozen Receipt v1 schema"),
                "{name}: {failure}"
            );
        }
    }

    #[test]
    fn every_trust_relevant_body_field_is_bound_by_one_root() {
        let mutators: Vec<ReceiptMutation> = vec![
            (
                "claim",
                Box::new(|raw| raw["claim"] = json!("changed claim")),
            ),
            ("type", Box::new(|raw| raw["type"] = json!("empirical"))),
            (
                "artifacts",
                Box::new(|raw| raw["artifacts"][0]["x:changed"] = json!(true)),
            ),
            (
                "verifier_runs",
                Box::new(|raw| raw["verifier_runs"][0]["method"] = json!("changed.check")),
            ),
            (
                "machine",
                Box::new(|raw| raw["machine"]["x:changed"] = json!(true)),
            ),
            (
                "acceptance",
                Box::new(|raw| raw["acceptance"]["x:changed"] = json!(true)),
            ),
            (
                "distillation",
                Box::new(|raw| raw["distillation"]["x:changed"] = json!(true)),
            ),
            (
                "lineage",
                Box::new(|raw| raw["lineage"]["x:changed"] = json!(true)),
            ),
            (
                "provenance",
                Box::new(|raw| raw["provenance"]["x:changed"] = json!(true)),
            ),
            (
                "contributors",
                Box::new(|raw| raw["contributors"][0]["id"] = json!("agent:changed")),
            ),
            (
                "signature identities",
                Box::new(|raw| raw["signature_identities"]["x:changed"] = json!(true)),
            ),
            (
                "environment",
                Box::new(|raw| raw["environment"]["x:changed"] = json!(true)),
            ),
            (
                "status",
                Box::new(|raw| raw["status"]["x:changed"] = json!(true)),
            ),
        ];
        for (name, mutate) in mutators {
            let mut raw = build(vec![reported_pass()]).into_value();
            mutate(&mut raw);
            let failure = ReceiptV1::from_trusted_value(raw).unwrap_err();
            assert!(
                failure
                    .message()
                    .contains("canonical receipt-body projection"),
                "{name}: {failure}"
            );
        }
    }

    #[test]
    fn recomputed_body_digest_cannot_hide_divergent_duplicated_layers() {
        let mut raw = build(Vec::new()).into_value();
        raw["machine"]["x:changed"] = json!(true);
        let body_root = receipt_body_sha256(object(&raw, "$").unwrap()).unwrap();
        raw["attestation"]["statement"]["predicate"][RECEIPT_BODY_BINDING_FIELD]["sha256"] =
            json!(body_root);
        refresh_payload_only(&mut raw);
        let failure = ReceiptV1::from_trusted_value(raw).unwrap_err();
        assert_eq!(failure.path(), "$.attestation.statement.predicate.machine");
    }

    #[test]
    fn statement_only_receipts_are_rejected() {
        let mut raw = build(Vec::new()).into_value();
        raw["attestation"]["statement"]["predicate"]
            .as_object_mut()
            .unwrap()
            .remove(RECEIPT_BODY_BINDING_FIELD);
        refresh_payload_only(&mut raw);
        let failure = ReceiptV1::parse(&serde_json::to_vec(&raw).unwrap()).unwrap_err();
        assert_eq!(
            failure.path(),
            "$.attestation.statement.predicate.vela:receipt_body"
        );
    }

    #[test]
    fn parser_enforces_every_documented_limit() {
        let base = ReceiptLimits {
            bytes: 4_096,
            depth: 8,
            string_bytes: 128,
            artifacts: 4,
            locator_bytes: 64,
            object_fields: 16,
            array_elements: 16,
            nodes: 32,
            dsse_bytes: 1_024,
        };
        let mut limits = base;
        limits.bytes = 2;
        assert!(
            ReceiptV1::parse_with_limits(br#"{  }"#, limits)
                .unwrap_err()
                .message()
                .contains("encoded JSON")
        );
        let mut limits = base;
        limits.depth = 2;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"x":{"a":{"b":1}}}"#, limits)
                .unwrap_err()
                .message()
                .contains("JSON depth")
        );
        let mut limits = base;
        limits.string_bytes = 4;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"x":"12345"}"#, limits)
                .unwrap_err()
                .message()
                .contains("string is")
        );
        let mut limits = base;
        limits.artifacts = 1;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"artifacts":[{},{}]}"#, limits)
                .unwrap_err()
                .message()
                .contains("descriptors")
        );
        let mut limits = base;
        limits.locator_bytes = 4;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"x:artifactURL":"12345"}"#, limits)
                .unwrap_err()
                .message()
                .contains("locator is")
        );
        let mut limits = base;
        limits.object_fields = 2;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"x":{"a":1,"b":2}}"#, limits)
                .unwrap_err()
                .message()
                .contains("object-field budget")
        );
        let mut limits = base;
        limits.array_elements = 1;
        assert!(
            ReceiptV1::parse_with_limits(br#"[1,2]"#, limits)
                .unwrap_err()
                .message()
                .contains("array-element budget")
        );
        let mut limits = base;
        limits.nodes = 2;
        assert!(
            ReceiptV1::parse_with_limits(br#"{"x":[1,2]}"#, limits)
                .unwrap_err()
                .message()
                .contains("JSON node budget")
        );

        let receipt = build(Vec::new());
        let mut limits = LIMITS;
        limits.dsse_bytes = 8;
        assert!(
            ReceiptV1::parse_with_limits(&receipt.canonical_bytes().unwrap(), limits)
                .unwrap_err()
                .message()
                .contains("decoded DSSE")
        );
    }

    #[test]
    fn default_parser_rejects_depth_65_and_one_hundred_thousand_artifacts() {
        let depth_65 = format!("{}0{}", "[".repeat(65), "]".repeat(65));
        let failure = ReceiptV1::parse(depth_65.as_bytes()).unwrap_err();
        assert!(failure.message().contains("JSON depth"), "{failure}");

        // Receipt v1 carries descriptors, never archive bodies. This input is
        // intentionally below the encoded-byte ceiling but far above both the
        // 10,000-artifact and bounded-array ceilings. The streaming parser
        // must reject it before schema or landing logic sees the document.
        let mut artifacts = String::with_capacity(300_032);
        artifacts.push_str("{\"artifacts\":[");
        for index in 0..100_000 {
            if index > 0 {
                artifacts.push(',');
            }
            artifacts.push_str("{}");
        }
        artifacts.push_str("]}");
        let failure = ReceiptV1::parse(artifacts.as_bytes()).unwrap_err();
        assert!(
            failure.message().contains("array-element budget")
                || failure.message().contains("JSON node budget")
                || failure.message().contains("descriptors"),
            "{failure}"
        );
    }

    #[test]
    fn malformed_dsse_payload_is_rejected_semantically() {
        let mut raw = build(Vec::new()).into_value();
        raw["attestation"]["dsse_envelope"]["payload"] = json!("e30=");
        assert!(
            ReceiptV1::from_trusted_value(raw)
                .unwrap_err()
                .message()
                .contains("does not encode")
        );
    }

    #[test]
    fn acceptance_scope_round_trips_every_variant() {
        for value in AcceptanceScope::ALL {
            assert_eq!(AcceptanceScope::parse(value.as_str()), Some(value));
            assert_eq!(serde_json::to_value(value).unwrap(), json!(value.as_str()));
        }
        assert_eq!(AcceptanceScope::parse("verified"), None);
    }

    #[test]
    fn acceptance_scope_matches_shipped_schema() {
        let values: Vec<String> = schema()["$defs"]["acceptance_scope"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            AcceptanceScope::ALL
                .iter()
                .map(|item| item.as_str().to_string())
                .collect::<Vec<_>>(),
            values
        );
    }

    #[test]
    fn typed_read_views_remain_parse_only() {
        let receipt = json!({
            "lineage": {
                "parents": ["vrc_aaa"],
                "derived_from": ["arxiv:2406.00001"],
                "source_refs": ["https://example.org/run/1"],
                "producer_run_id": "run-17",
                "unknown_extra": {"kept": "loose"}
            },
            "acceptance": {"acceptance_scope": "machine_verified"},
            "environment": {"independence_basis": {
                "method_family": "sat",
                "known_couplings": ["model:test"]
            }}
        });
        let lineage = lineage_from_receipt(&receipt).unwrap();
        assert_eq!(lineage.parents, vec!["vrc_aaa"]);
        assert_eq!(lineage.producer_run_id.as_deref(), Some("run-17"));
        assert_eq!(
            acceptance_scope_from_receipt(&receipt),
            Some(AcceptanceScope::MachineVerified)
        );
        assert_eq!(
            independence_basis_from_environment(&receipt["environment"])
                .unwrap()
                .known_couplings,
            vec!["model:test"]
        );
        assert_eq!(lineage_from_receipt(&json!({})), None);
    }
}
