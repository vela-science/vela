//! The authorization engine named in a policy bundle must be the one that ran.
//!
//! `CEDAR_ENGINE` and `CEDAR_ENGINE_VERSION` are written into every
//! `PolicyBundleV1` this workspace mints, and `PolicyBundleV1::validate`
//! rejects a bundle whose `engine_version` differs from the constant. So the
//! constant is not a label. It is the repository's statement, carried inside
//! signed authority history, about which evaluator produced a decision.
//!
//! Nothing derived it. `cedar-policy` is a dependency of `vela-authority`, not
//! of this crate, and Cargo exposes no dependency version to a dependent's
//! source, so the version was typed by hand beside a pin typed by hand. That is
//! the shape that already cost this ecosystem once: `physlib` was pinned in
//! both `sources.lock.json` and a Rust constant, the two drifted four commits
//! apart, and nothing noticed because nothing compared them. Bumping
//! `cedar-policy` without editing the constant compiles clean and mints bundles
//! that name an evaluator the binary is not running.
//!
//! This is the comparison. It is deliberately three assertions rather than one:
//! the declared pin, the resolved build, and the exactness that makes the other
//! two mean anything.

use std::path::{Path, PathBuf};

use vela_protocol::authority::{CEDAR_ENGINE, CEDAR_ENGINE_VERSION};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> toml::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    // `str::parse` reads a bare value in toml 1.x, not a document.
    toml::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

/// The version requirement the workspace declares for the engine.
fn declared_requirement() -> String {
    let manifest = read(&workspace_root().join("Cargo.toml"));
    let entry = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.get(CEDAR_ENGINE))
        .unwrap_or_else(|| {
            panic!("`{CEDAR_ENGINE}` is not a workspace dependency; CEDAR_ENGINE names nothing")
        });
    // Either `name = "req"` or `name = { version = "req", ... }`.
    let requirement = match entry {
        toml::Value::String(text) => Some(text.as_str()),
        other => other.get("version").and_then(toml::Value::as_str),
    };
    requirement
        .unwrap_or_else(|| panic!("`{CEDAR_ENGINE}` declares no version"))
        .to_string()
}

/// The version Cargo actually resolved for the engine.
fn resolved_version() -> String {
    let lock = read(&workspace_root().join("Cargo.lock"));
    lock.get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock lists packages")
        .iter()
        .find(|package| package.get("name").and_then(toml::Value::as_str) == Some(CEDAR_ENGINE))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("Cargo.lock holds no `{CEDAR_ENGINE}`"))
        .to_string()
}

/// An inexact pin would let the lock move under a constant that cannot follow.
///
/// Checked first, because it is the assumption the other two rest on. Under
/// `^4.11` a routine `cargo update` changes the evaluator and leaves both the
/// constant and the equality below untouched and wrong.
#[test]
fn the_engine_is_pinned_exactly() {
    let requirement = declared_requirement();
    assert!(
        requirement.starts_with('='),
        "`{CEDAR_ENGINE}` is declared as {requirement:?}. `CEDAR_ENGINE_VERSION` states which \
         evaluator ran, inside signed authority history, so the pin has to be exact. Restore \
         `=<version>`, or delete the constant and stop making the claim."
    );
}

#[test]
fn the_named_engine_version_is_the_declared_pin() {
    let requirement = declared_requirement();
    let pinned = requirement.trim_start_matches('=').trim();
    assert_eq!(
        pinned, CEDAR_ENGINE_VERSION,
        "Cargo.toml pins `{CEDAR_ENGINE}` at {pinned:?} and \
         `authority::CEDAR_ENGINE_VERSION` says {CEDAR_ENGINE_VERSION:?}. Every policy bundle \
         this workspace mints carries the constant, and `PolicyBundleV1::validate` rejects a \
         bundle that disagrees with it. Change both or neither."
    );
}

/// The lock is what builds, and an exact pin is only a promise until it is read.
#[test]
fn the_named_engine_version_is_the_resolved_build() {
    let resolved = resolved_version();
    assert_eq!(
        resolved, CEDAR_ENGINE_VERSION,
        "Cargo.lock resolved `{CEDAR_ENGINE}` to {resolved:?} and \
         `authority::CEDAR_ENGINE_VERSION` says {CEDAR_ENGINE_VERSION:?}. The bundles this \
         build mints would name an evaluator it is not running."
    );
}
