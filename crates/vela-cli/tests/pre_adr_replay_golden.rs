use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Golden {
    schema: String,
    source: Source,
    files: Vec<FrozenFile>,
    replay: Replay,
}

#[derive(Deserialize)]
struct Source {
    frontier: String,
    commit: String,
    vela_tree: String,
}

#[derive(Deserialize)]
struct FrozenFile {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Deserialize)]
struct Replay {
    event_count: u64,
    event_log_hash: String,
    state_hash: String,
    finding_count: u64,
    proposal_total: u64,
    proposal_applied: u64,
}

fn collect_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap();
        if relative.starts_with(".vela/operation-journals") {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            collect_files(root, &path, out);
        } else {
            out.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            if entry.file_name() != "operation-journals" {
                copy_tree(&from, &to);
            }
        } else {
            fs::copy(from, to).unwrap();
        }
    }
}

fn pointer<'a>(value: &'a Value, path: &str) -> &'a Value {
    value
        .pointer(path)
        .unwrap_or_else(|| panic!("missing JSON pointer {path}"))
}

#[test]
fn pre_adr_frontier_bytes_and_strict_replay_are_frozen() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Golden = serde_json::from_slice(
        &fs::read(workspace.join("conformance/pre-adr-0003-replay.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest.schema, "vela.pre-adr-0003-replay-golden.v0.1");
    assert_eq!(
        manifest.source.frontier, "examples/erdos-formalization",
        "the golden must remain bound to the reviewed frontier path"
    );
    assert_eq!(
        manifest.source.commit, "7f5aaf906eec1034a2367847bd2520f5b1c4fa9d",
        "the golden must remain attributed to the exact pre-ADR 0003 commit"
    );
    assert_eq!(
        manifest.source.vela_tree, "a64e4d9ce110e1454e4904fb36ff8ede1799df61",
        "the frozen authority tree must not be silently re-baselined"
    );

    let source = workspace.join(&manifest.source.frontier);
    let tracked_tree = Command::new("git")
        .args([
            "-C",
            workspace.to_str().unwrap(),
            "rev-parse",
            &format!("HEAD:{}/.vela", manifest.source.frontier),
        ])
        .output()
        .unwrap();
    assert!(
        tracked_tree.status.success(),
        "read tracked authority tree: {}",
        String::from_utf8_lossy(&tracked_tree.stderr)
    );
    assert_eq!(
        String::from_utf8(tracked_tree.stdout).unwrap().trim(),
        manifest.source.vela_tree,
        "current tracked fixture bytes must remain the exact reviewed pre-ADR authority tree"
    );
    let expected_paths = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut actual_paths = BTreeSet::new();
    collect_files(&source, &source.join(".vela"), &mut actual_paths);
    assert_eq!(
        actual_paths, expected_paths,
        "pre-ADR authority path set drifted"
    );

    for frozen in &manifest.files {
        let bytes = fs::read(source.join(&frozen.path)).unwrap();
        assert_eq!(bytes.len(), frozen.bytes, "{} byte length", frozen.path);
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            frozen.sha256,
            "{} content",
            frozen.path
        );
    }

    let temporary = tempfile::TempDir::new().unwrap();
    let frontier = temporary.path().join("frontier");
    copy_tree(&source, &frontier);
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .env_clear()
        .env("HOME", temporary.path())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("VELA_NO_PUBLISH", "1")
        .args(["check", frontier.to_str().unwrap(), "--strict", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "strict replay failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(pointer(&report, "/ok"), true);
    assert_eq!(pointer(&report, "/replay/status"), "ok");
    assert_eq!(
        pointer(&report, "/replay/event_log/count"),
        manifest.replay.event_count
    );
    assert_eq!(
        pointer(&report, "/replay/event_log_hash"),
        &manifest.replay.event_log_hash
    );
    for field in ["current_hash", "replayed_hash", "source_hash"] {
        assert_eq!(
            pointer(&report, &format!("/replay/{field}")),
            &manifest.replay.state_hash
        );
    }
    assert_eq!(
        pointer(&report, "/summary/checked_findings"),
        manifest.replay.finding_count
    );
    assert_eq!(
        pointer(&report, "/proposals/total"),
        manifest.replay.proposal_total
    );
    assert_eq!(
        pointer(&report, "/proposals/applied"),
        manifest.replay.proposal_applied
    );
}
