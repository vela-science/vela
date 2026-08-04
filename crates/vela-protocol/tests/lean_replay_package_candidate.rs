//! Independent Rust reader for the source-local Lean replay package root.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const EXPECTED_ROOT: &str =
    "sha256:5653a31b6b42a77cff91905ffa3086730e21eb6cc4105963d9d98cbcc2b2baae";

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

    assert_eq!(files.len(), 10, "package file set drifted");
    let descriptor = Descriptor {
        files,
        schema: "vela.logical-package-root.v1",
    };
    let canonical = serde_json_canonicalizer::to_vec(&descriptor)
        .expect("RFC 8785 canonical package descriptor");
    assert_eq!(sha256(&canonical), EXPECTED_ROOT);
}

