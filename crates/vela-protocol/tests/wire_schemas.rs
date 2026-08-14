//! The drift gate for `schemas/*.schema.json`.
//!
//! These files are output, not source. Nothing else in the tree can notice when
//! they stop describing the types that sign: `conformance/verify_wire_schemas.py`
//! holds each schema against a frozen fixture, which is a different and still
//! useful question, but a fixture and a schema can agree while both have fallen
//! behind the struct. This test asks the only question that catches that — does
//! the checked-in file still equal what the current types generate.
//!
//! Regenerate with:
//!
//! ```text
//! VELA_BLESS_WIRE_SCHEMAS=1 cargo test -p vela-protocol --test wire_schemas
//! ```
//!
//! Blessing is deliberately a separate, explicit run. A generator that rewrote
//! the file whenever it disagreed would report success on every change and gate
//! nothing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use vela_protocol::wire_schema;

fn schemas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

fn blessing() -> bool {
    std::env::var_os("VELA_BLESS_WIRE_SCHEMAS").is_some_and(|value| value == "1")
}

#[test]
fn checked_in_schemas_match_the_types_that_sign() {
    let directory = schemas_dir();
    let mut stale = Vec::new();

    for (file, document) in wire_schema::published() {
        let path = directory.join(file);
        let generated = wire_schema::render(&document);
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == generated {
            continue;
        }
        if blessing() {
            std::fs::write(&path, &generated).expect("write regenerated schema");
        }
        stale.push(file);
    }

    if stale.is_empty() {
        return;
    }
    if blessing() {
        panic!(
            "regenerated {} schema(s): {}. Review the diff and commit it.",
            stale.len(),
            stale.join(", ")
        );
    }
    panic!(
        "{} checked-in schema(s) no longer match the Rust types: {}.\n\
         The types are normative. Regenerate with \
         `VELA_BLESS_WIRE_SCHEMAS=1 cargo test -p vela-protocol --test wire_schemas`, \
         then read the diff before committing it.",
        stale.len(),
        stale.join(", ")
    );
}

/// A schema file with no type behind it describes nothing, and would keep
/// validating documents long after the object it named was removed.
#[test]
fn every_schema_file_is_generated_by_a_live_type() {
    let generated: BTreeSet<&str> = wire_schema::published()
        .into_iter()
        .map(|(file, _)| file)
        .collect();
    let mut checked_in = BTreeSet::new();
    for entry in std::fs::read_dir(schemas_dir()).expect("read schemas directory") {
        let name = entry.expect("read schema entry").file_name();
        let name = name.to_string_lossy().into_owned();
        if name.ends_with(".schema.json") {
            checked_in.insert(name);
        }
    }
    let orphans: Vec<&String> = checked_in
        .iter()
        .filter(|name| !generated.contains(name.as_str()))
        .collect();
    assert!(
        orphans.is_empty(),
        "schemas/ holds {orphans:?}, which no type in `wire_schema::published` generates"
    );
    assert_eq!(
        checked_in.len(),
        generated.len(),
        "every generated schema must be checked in"
    );
}

/// The rendered bytes must be a function of the types alone.
///
/// `serde_json::Map` preserves insertion order when anything in the build turns
/// on `serde_json/preserve_order`, so different feature-unified cargo
/// invocations may otherwise render the same schema in two key orders. The drift gate compares
/// bytes, so it would then fail in one invocation and pass in the other.
/// Sorted keys at every depth is the property that makes it invocation-blind.
#[test]
fn rendered_keys_are_sorted_at_every_depth() {
    fn assert_sorted(node: &serde_json::Value, path: &str) {
        match node {
            serde_json::Value::Object(map) => {
                let keys: Vec<&String> = map.keys().collect();
                let mut sorted = keys.clone();
                sorted.sort();
                assert_eq!(keys, sorted, "keys out of order at {path}");
                for (key, value) in map {
                    assert_sorted(value, &format!("{path}.{key}"));
                }
            }
            serde_json::Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    assert_sorted(item, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    for (file, document) in wire_schema::published() {
        assert_sorted(&document, file);
    }
}

/// The `$id` a reader resolves must be the path the file actually lives at.
#[test]
fn each_document_declares_the_id_it_is_published_under() {
    for (file, document) in wire_schema::published() {
        let id = document["$id"].as_str().expect("$id is a string");
        assert_eq!(
            id,
            format!("https://vela.science/schemas/{file}"),
            "{file} declares an $id it is not served at"
        );
    }
}

/// A stable read export cannot make a consumer guess whether a missing field
/// means `null`, empty, unsupported, or a producer bug. The projection stays
/// open to additive fields, but every field v1 already names is always present.
#[test]
fn repository_projection_v1_requires_every_declared_field() {
    fn visit(node: &serde_json::Value, path: &str) {
        if let Some(properties) = node
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            let required = node
                .get("required")
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("{path} declares properties without required fields"));
            let required: BTreeSet<&str> = required
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .unwrap_or_else(|| panic!("{path}.required contains non-text"))
                })
                .collect();
            for key in properties.keys() {
                assert!(
                    required.contains(key.as_str()),
                    "{path}.{key} is optional; v1 nullable fields must be explicit nulls"
                );
            }
        }
        match node {
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    visit(value, &format!("{path}.{key}"));
                }
            }
            serde_json::Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    visit(value, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }

    let (_, projection) = wire_schema::published()
        .into_iter()
        .find(|(file, _)| *file == "repository-projection.schema.json")
        .expect("repository projection schema is published");
    visit(&projection, "repository-projection");
}
