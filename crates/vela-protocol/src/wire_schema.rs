//! The published wire JSON Schemas, generated from the types that sign.
//!
//! `schemas/*.schema.json` used to be written by hand beside the Rust. Both
//! documents described the same wire bytes, but only the Rust builds canonical
//! bytes and signs, so the JSON was a copy nothing could hold to its original:
//! a field added to the struct still validated against the stale schema, and a
//! field added to the schema alone was never contradicted by anything. These
//! builders make the Rust the single statement and the JSON its output.
//!
//! The derive alone would not be enough. Every constrained field on the wire is
//! a Rust `String`, so the pattern, bound, and vocabulary that each object's
//! `validate()` enforces are invisible to `#[derive(JsonSchema)]` and are
//! supplied here as attributes. They are not second copies: each vocabulary
//! fragment reads the same `const` slice the validator reads, and the tests in
//! [`crate::shape`] hold each pattern against the predicate it stands for, so
//! the two cannot drift apart in silence.
//!
//! What stays outside JSON Schema — cross-field ordering, signature
//! verification, derived identifiers, canonical bytes — is listed in
//! `schemas/README.md`. A schema that validates is a document worth parsing,
//! never a document worth trusting.

use schemars::{JsonSchema, Schema, SchemaGenerator, generate::SchemaSettings, json_schema};
use serde_json::{Map, Value};

use crate::authority::AUTHORITY_PAYLOAD_TYPE_V1;
use crate::execution_binding::EXECUTION_BINDING_SCHEMA;
use crate::identity::IDENTITY_BINDING_SCHEMA;
use crate::proposal_withdrawal_v1::{
    PROPOSAL_WITHDRAWAL_V1_AUTH_ALGORITHM, PROPOSAL_WITHDRAWAL_V1_SCHEMA,
};
use crate::submission_v1::{
    CLAIM_TYPES, PRODUCER_CHECK_OUTCOMES, PRODUCER_REPORTED_AUTHORITY, REPLAYABILITY_LEVELS,
    REQUESTED_CHANGE_KINDS, SUBMISSION_V1_AUTH_ALGORITHM, SUBMISSION_V1_SCHEMA,
};
use crate::verification_record::{
    VERIFICATION_OUTCOMES, VERIFICATION_RECORD_AUTH_ALGORITHM, VERIFICATION_RECORD_V1_SCHEMA,
};

/// The published base for every `$id` in `schemas/`.
const SCHEMA_BASE: &str = "https://vela.science/schemas";

// The regular expressions below are the wire spelling of the predicates in
// `crate::shape`. `shape::tests::wire_patterns_agree_with_predicates` runs both
// over the same corpus, so a change to either that is not made to the other
// fails rather than quietly widening what the published schema accepts.

/// A full Vela root: lowercase `sha256:` and exactly 64 lowercase hex digits.
pub const SHA256_ROOT_PATTERN: &str = "^sha256:[0-9a-f]{64}$";

/// Exactly 64 lowercase hex digits, with any prefix already stripped.
pub const LOWER_HEX_64_PATTERN: &str = "^[0-9a-f]{64}$";

/// A hex Ed25519 signature: 64 bytes, so 128 lowercase hex digits.
pub const ED25519_SIGNATURE_PATTERN: &str = "^[0-9a-f]{128}$";

/// Non-empty text with no leading or trailing whitespace.
///
/// This is the wire half of `require_text`. It cannot reach the interior
/// control characters that the Rust predicate also rejects; see
/// `schemas/README.md` for the constraints that stay behind in Rust.
pub const TRIMMED_TEXT_PATTERN: &str = r"^\S(?:[\s\S]*\S)?$";

/// A relative path that cannot climb out of the Submission tree.
pub const SAFE_RELATIVE_PATH_PATTERN: &str = r"^(?!/)(?!.*(?:^|/)\.\.(?:/|$))\S(?:[\s\S]*\S)?$";

/// A producer identity: an `agent:` or `ci:` namespace and a non-empty body.
pub const PRODUCER_ACTOR_PATTERN: &str = r"^(agent|ci):[^\s]+$";

/// The byte ceiling `require_text` applies to a single bounded field.
pub const TEXT_MAX_BYTES: usize = 16 * 1024;

/// Build a `{"type": "string", "enum": [...]}` fragment from the same slice the
/// object's validator passes to `require_member`.
fn vocabulary(members: &[&str]) -> Schema {
    json_schema!({ "type": "string", "enum": members })
}

/// Build a `{"type": "string", "const": "..."}` fragment from the same `const`
/// the object's validator compares against.
fn tag(value: &str) -> Schema {
    json_schema!({ "type": "string", "const": value })
}

fn text_fragment() -> Value {
    serde_json::json!({
        "type": "string",
        "minLength": 1,
        "maxLength": TEXT_MAX_BYTES,
        "pattern": TRIMMED_TEXT_PATTERN,
    })
}

// ---------------------------------------------------------------------------
// Field builders, named for the rule they carry rather than for the field.
// `schema_with` takes a fixed signature, so each distinct shape needs its own
// function; the bodies stay one expression each.
// ---------------------------------------------------------------------------

/// Text bounded at 16 KiB, as `require_text` bounds it.
pub fn text(_: &mut SchemaGenerator) -> Schema {
    Schema::try_from(text_fragment()).expect("text fragment is an object schema")
}

/// Text with no declared ceiling, matching the Proposal Withdrawal reader,
/// whose `require_text` omits the 16 KiB check its siblings apply.
pub fn unbounded_text(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "minLength": 1, "pattern": TRIMMED_TEXT_PATTERN })
}

/// An array of bounded text.
pub fn text_array(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "array", "items": text_fragment() })
}

/// An array of bounded text that must state at least one entry.
pub fn nonempty_text_array(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "array", "minItems": 1, "items": text_fragment() })
}

/// A full `sha256:` root.
pub fn sha256_root(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": SHA256_ROOT_PATTERN })
}

/// An array of bare content hashes.
pub fn artifact_reference_id_array(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "array",
        "items": { "type": "string", "pattern": LOWER_HEX_64_PATTERN },
    })
}

/// A hex Ed25519 public key.
pub fn public_key_hex(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": LOWER_HEX_64_PATTERN })
}

/// A hex Ed25519 signature.
pub fn ed25519_signature(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": ED25519_SIGNATURE_PATTERN })
}

/// An RFC 3339 timestamp. `format` is an assertion in Vela's conformance
/// check, not an annotation.
pub fn timestamp(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "format": "date-time" })
}

/// A relative Artifact path that cannot escape the Submission tree.
pub fn safe_relative_path(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "minLength": 1,
        "maxLength": TEXT_MAX_BYTES,
        "pattern": SAFE_RELATIVE_PATH_PATTERN,
    })
}

/// An `agent:` or `ci:` producer identity.
pub fn producer_actor(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "maxLength": TEXT_MAX_BYTES,
        "pattern": PRODUCER_ACTOR_PATTERN,
    })
}

// Readable routing handles. The `<prefix>_<n>hex` forms are content-addressed
// and fully determined; the `.+` forms name an object whose own reader derives
// the identifier, so this document checks only the namespace.

/// `vsb_` and 16 hex digits.
pub fn submission_id(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vsb_[0-9a-f]{16}$" })
}

/// `vvr_` and 16 hex digits.
pub fn verification_record_id(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vvr_[0-9a-f]{16}$" })
}

/// `vib_` and 16 hex digits.
pub fn identity_binding_id(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vib_[0-9a-f]{16}$" })
}

/// `vcl_` and 64 hex digits.
pub fn claim_id(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vcl_[0-9a-f]{64}$" })
}

/// `vat_` and 64 hex digits.
pub fn source_attempt_id(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vat_[0-9a-f]{64}$" })
}

/// A `vsb_` namespace only, for readers that do not re-derive the identifier.
pub fn submission_id_reference(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vsb_.+$" })
}

/// A `vpr_` namespace only.
pub fn proposal_id_reference(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vpr_.+$" })
}

/// A `vpw_` namespace only.
pub fn withdrawal_id_reference(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "pattern": "^vpw_.+$" })
}

/// A base64 body, permitting both the standard and URL alphabets.
pub fn base64_body(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "string", "minLength": 1, "pattern": "^[A-Za-z0-9+/_-]+={0,2}$" })
}

// Schema tags and closed vocabularies. Each reads the constant its validator
// reads, so the vocabulary is stated once in the object module and rendered
// here.

/// `vela.submission.v1`.
pub fn submission_schema_tag(_: &mut SchemaGenerator) -> Schema {
    tag(SUBMISSION_V1_SCHEMA)
}

/// `vela.verification-record.v1`.
pub fn verification_record_schema_tag(_: &mut SchemaGenerator) -> Schema {
    tag(VERIFICATION_RECORD_V1_SCHEMA)
}

/// `vela.proposal-withdrawal.v1`.
pub fn proposal_withdrawal_schema_tag(_: &mut SchemaGenerator) -> Schema {
    tag(PROPOSAL_WITHDRAWAL_V1_SCHEMA)
}

/// `vela.execution-binding.v1`.
pub fn execution_binding_schema_tag(_: &mut SchemaGenerator) -> Schema {
    tag(EXECUTION_BINDING_SCHEMA)
}

/// The one DSSE payload type a Vela authority envelope may carry.
pub fn authority_payload_type_tag(_: &mut SchemaGenerator) -> Schema {
    tag(AUTHORITY_PAYLOAD_TYPE_V1)
}

/// `vela.identity_binding.v0.1`.
pub fn identity_binding_schema_tag(_: &mut SchemaGenerator) -> Schema {
    tag(IDENTITY_BINDING_SCHEMA)
}

/// The one signature algorithm a Submission may declare.
pub fn submission_auth_algorithm(_: &mut SchemaGenerator) -> Schema {
    tag(SUBMISSION_V1_AUTH_ALGORITHM)
}

/// The one signature algorithm a Verification Record may declare.
pub fn verification_auth_algorithm(_: &mut SchemaGenerator) -> Schema {
    tag(VERIFICATION_RECORD_AUTH_ALGORITHM)
}

/// The one signature algorithm a Proposal Withdrawal may declare.
pub fn withdrawal_auth_algorithm(_: &mut SchemaGenerator) -> Schema {
    tag(PROPOSAL_WITHDRAWAL_V1_AUTH_ALGORITHM)
}

/// A producer check reports only its own authority; it is not a Verification.
pub fn producer_reported_authority(_: &mut SchemaGenerator) -> Schema {
    tag(PRODUCER_REPORTED_AUTHORITY)
}

/// The closed Claim-type vocabulary.
pub fn claim_type(_: &mut SchemaGenerator) -> Schema {
    vocabulary(CLAIM_TYPES)
}

/// The closed replayability vocabulary.
pub fn replayability(_: &mut SchemaGenerator) -> Schema {
    vocabulary(REPLAYABILITY_LEVELS)
}

/// The closed producer-check outcome vocabulary.
pub fn producer_check_outcome(_: &mut SchemaGenerator) -> Schema {
    vocabulary(PRODUCER_CHECK_OUTCOMES)
}

/// The closed Verification outcome vocabulary. It has no member that implies
/// acceptance; Standing moves by Decision and Event, never by this field.
pub fn verification_outcome(_: &mut SchemaGenerator) -> Schema {
    vocabulary(VERIFICATION_OUTCOMES)
}

/// The closed requested-change vocabulary.
pub fn requested_change_kind(_: &mut SchemaGenerator) -> Schema {
    vocabulary(REQUESTED_CHANGE_KINDS)
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// Drop every `description` the derive lifted out of a Rust doc comment.
///
/// Those comments address someone reading this crate, and they say so: they
/// carry rustdoc link syntax and refer to methods and preimages that a wire
/// reader has no access to. Publishing them would put a second, unreviewed
/// prose contract on `vela.science` and make every doc edit a change to a
/// published artifact. The schemas carry constraints; `schemas/README.md`
/// carries the prose.
fn strip_descriptions(node: &mut Value) {
    match node {
        Value::Object(map) => {
            map.remove("description");
            for value in map.values_mut() {
                strip_descriptions(value);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_descriptions),
        _ => {}
    }
}

/// Rebuild every object with its keys in sorted order.
///
/// `serde_json::Map` is a `BTreeMap` normally and an `IndexMap` when something
/// in the build enables `serde_json/preserve_order`, which Cedar does — so the
/// same types serialize in two different key orders depending on whether this
/// crate is built alone or alongside `vela-cli`. The generated file is compared
/// byte for byte against the checked-in one, so an order that depends on which
/// cargo invocation ran would make the drift gate report drift that is not
/// there. Sorting on the way out makes the output a function of the types only.
///
/// Arrays keep their order: `required` and `enum` carry declaration order,
/// which is meaningful.
fn sort_keys(node: Value) -> Value {
    match node {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, sort_keys(value)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

/// Render one object type as a published schema document.
///
/// Subschemas are inlined rather than collected into `$defs`. `$defs` keys
/// would be Rust type names, and these files are published under
/// `vela.science`: an implementer reading the wire contract should not have to
/// learn this crate's internal type names to follow a `$ref`.
fn document<T: JsonSchema>(file: &str, title: &str) -> Value {
    let mut settings = SchemaSettings::draft2020_12();
    settings.inline_subschemas = true;
    let generated = SchemaGenerator::new(settings).into_root_schema_for::<T>();

    let mut body = serde_json::to_value(generated).expect("schema serializes");
    strip_descriptions(&mut body);
    let Value::Object(body) = body else {
        unreachable!("a derived root schema is always an object");
    };
    let mut document = Map::new();
    document.insert(
        "$schema".into(),
        Value::String("https://json-schema.org/draft/2020-12/schema".into()),
    );
    document.insert("$id".into(), Value::String(format!("{SCHEMA_BASE}/{file}")));
    // The derive titles the root with the Rust type name. The published name is
    // the protocol's, not the implementation's.
    document.insert("title".into(), Value::String(title.into()));
    for (key, value) in body {
        if key != "$schema" && key != "title" {
            document.insert(key, value);
        }
    }
    sort_keys(Value::Object(document))
}

/// Every published wire schema, as `(file name, document)`.
///
/// The drift test in `tests/wire_schemas.rs` walks this list, so a schema added
/// here without a checked-in file — or a file left behind after its type is
/// removed — fails rather than going unnoticed.
pub fn published() -> Vec<(&'static str, Value)> {
    vec![
        (
            "submission-v1.schema.json",
            document::<crate::submission_v1::SubmissionV1>(
                "submission-v1.schema.json",
                "Vela Submission v1",
            ),
        ),
        (
            "verification-record-v1.schema.json",
            document::<crate::verification_record::VerificationRecordV1>(
                "verification-record-v1.schema.json",
                "Vela Verification Record v1",
            ),
        ),
        (
            "proposal-withdrawal-v1.schema.json",
            document::<crate::proposal_withdrawal_v1::ProposalWithdrawalV1>(
                "proposal-withdrawal-v1.schema.json",
                "Vela Proposal Withdrawal v1",
            ),
        ),
        (
            "authority-envelope-v1.schema.json",
            document::<crate::authority::AuthorityEnvelopeV1>(
                "authority-envelope-v1.schema.json",
                "Vela Authority DSSE Envelope v1",
            ),
        ),
    ]
}

/// Render a document the way the checked-in files are written: two-space
/// pretty JSON with a trailing newline.
pub fn render(document: &Value) -> String {
    let mut text = serde_json::to_string_pretty(document).expect("schema document serializes");
    text.push('\n');
    text
}
