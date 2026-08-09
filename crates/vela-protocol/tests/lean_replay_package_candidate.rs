//! Independent Rust reader for the source-local Lean replay package root.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// The root of the package as it stands in this tree.
///
/// It moved once, when three of the package's ten files lost a `.v1` from their
/// names and two more stopped naming the old spellings in their bodies. A
/// logical package root is a function of the exact paths and bytes under it, so
/// all five edits move it, and the superseded root is retained under
/// `predecessor` in `research/lean-replay-contract-evidence/qualification.json`.
///
/// Three readers now have to agree on it, and each is checked against this
/// constant rather than against one of the others.
/// `independent_reader_reproduces_frozen_package_root` walks the tree,
/// `.github/workflows/conformance.yml` reads this constant out of this file and
/// runs `build_root.py`, and `the_qualification_record_states_the_current_root`
/// holds the evidence record to it — because a package whose stated identity
/// has drifted from its measured one is the failure that record exists to make
/// impossible.
const EXPECTED_ROOT: &str =
    "sha256:a72d2e262785d4465e6d7b7fd7b8472107182ceaed79f17b77bb62d660a5e6f3";

#[derive(Serialize)]
struct Descriptor {
    files: Vec<FileDescriptor>,
    schema: &'static str,
}

#[derive(Serialize)]
struct FileDescriptor {
    media_type: &'static str,
    path: String,
    sha256: String,
    size: u64,
}

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("research/lean-replay-contract")
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .expect("read package directory")
        .map(|entry| entry.expect("read package entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            let name = path.file_name().and_then(|value| value.to_str());
            if matches!(name, Some("__pycache__" | ".pytest_cache")) {
                continue;
            }
            collect_files(&path, files);
        } else if path.file_name().and_then(|value| value.to_str()) != Some("root.json") {
            files.push(path);
        }
    }
}

/// The four extensions this reader and `build_root.py` are known to agree on.
///
/// Outside them the two disagree, and not in a way either side can see: the
/// Python builder falls back to `mimetypes.guess_type`, which is seeded from
/// the host's own mime database — `/etc/mime.types` and friends, one of which
/// exists on a developer Mac and a different one on the CI runner. A `.sh` in
/// this package is `application/x-sh` there and `application/octet-stream`
/// here, so the two readers would compute two roots from identical bytes, and
/// a `.lean` could compute two different roots on two machines from the same
/// checkout. Both surface as "the frozen root moved", which is the one
/// explanation that is not true.
const SHARED_EXTENSIONS: [&str; 4] = ["json", "py", "md", "txt"];

fn media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("json") => "application/json",
        Some("py") => "text/x-python",
        Some("md" | "txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

#[test]
fn independent_reader_reproduces_frozen_package_root() {
    let root = package_root().canonicalize().expect("resolve package root");
    let mut paths = Vec::new();
    collect_files(&root, &mut paths);
    paths.sort();

    // Before the extension check, so the usual local accident — an editor
    // scratch file or a .DS_Store landing in the directory — still reports
    // itself as drift in the file set rather than as a media-type argument.
    assert_eq!(paths.len(), 10, "package file set drifted");
    for path in &paths {
        let extension = path.extension().and_then(|value| value.to_str());
        assert!(
            extension.is_some_and(|value| SHARED_EXTENSIONS.contains(&value)),
            "{} is outside the extensions the two package-root readers agree on ({}); \
             adding it makes the root a function of the host's mime database, not of the bytes",
            path.display(),
            SHARED_EXTENSIONS.join(", "),
        );
    }

    let files = paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).expect("read package file");
            let relative = path
                .strip_prefix(&root)
                .expect("package-relative path")
                .to_str()
                .expect("UTF-8 package path")
                .replace(std::path::MAIN_SEPARATOR, "/");
            FileDescriptor {
                media_type: media_type(&path),
                path: relative,
                sha256: sha256(&bytes),
                size: bytes.len() as u64,
            }
        })
        .collect::<Vec<_>>();

    let descriptor = Descriptor {
        files,
        schema: "vela.logical-package-root.v1",
    };
    let canonical = serde_json_canonicalizer::to_vec(&descriptor)
        .expect("RFC 8785 canonical package descriptor");
    assert_eq!(sha256(&canonical), EXPECTED_ROOT);
}

/// The evidence record beside the package states the root the package has.
///
/// `research/lean-replay-contract-evidence/qualification.json` is what a reader
/// consults to learn this package's identity, and `conformance/repository_lint.py`
/// reads `package.root` out of it to decide whether a repository's dependency on
/// this unreleased path is one somebody qualified. A record naming a root the
/// package no longer has qualifies nothing and denies everything, silently, and
/// the drift that produces it is the ordinary one: an edit under the package
/// that nobody thought moved its identity.
#[test]
fn the_qualification_record_states_the_current_root() {
    const RECORD: &[u8] =
        include_bytes!("../../../research/lean-replay-contract-evidence/qualification.json");

    let record: serde_json::Value =
        serde_json::from_slice(RECORD).expect("qualification record JSON");
    assert_eq!(
        record["package"]["root"].as_str(),
        Some(EXPECTED_ROOT),
        "the qualification record names a different root than the package has"
    );
    assert_eq!(
        record["package"]["source_path"].as_str(),
        Some("research/lean-replay-contract"),
        "the qualification record is about a different path than this reader walks"
    );
}
