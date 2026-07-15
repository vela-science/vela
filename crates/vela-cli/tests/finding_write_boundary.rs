//! Regression tests for the prelaunch hard cut of legacy writer commands.

use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vela"));
    command
        .current_dir(dir)
        .args(args)
        .env("HOME", dir)
        .env("NO_COLOR", "1");
    for (key, _) in std::env::vars() {
        if key.starts_with("VELA_") {
            command.env_remove(key);
        }
    }
    command.output().expect("run vela")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut paths = std::fs::read_dir(dir)
            .expect("read frontier")
            .map(|entry| entry.expect("read entry").path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root)
                        .expect("frontier-relative path")
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(path).expect("read frontier file"),
                ));
            }
        }
    }

    let mut files = Vec::new();
    collect(root, root, &mut files);
    files
}

fn initialized_frontier() -> tempfile::TempDir {
    let frontier = tempfile::tempdir().expect("temp frontier");
    let output = run(
        frontier.path(),
        &["init", ".", "--name", "writer-boundary", "--json"],
    );
    assert!(output.status.success(), "{}", text(&output));
    frontier
}

fn assert_absent_without_delta(frontier: &Path, args: &[&str], retired: &str) {
    let before = snapshot(frontier);
    let output = run(frontier, args);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    let body = text(&output);
    assert!(
        body.contains("unrecognized subcommand") && body.contains(retired),
        "{body}"
    );
    assert_eq!(snapshot(frontier), before, "retired command changed bytes");
}

#[test]
fn direct_finding_writers_are_absent_and_have_zero_delta() {
    let frontier = initialized_frontier();
    for command in [
        "add",
        "supersede",
        "note",
        "caveat",
        "revise",
        "review",
        "reject",
        "retract",
        "contribution",
    ] {
        assert_absent_without_delta(frontier.path(), &["finding", command], command);
    }
}

#[test]
fn proposal_import_is_absent_and_has_zero_delta() {
    let frontier = initialized_frontier();
    assert_absent_without_delta(frontier.path(), &["proposals", "import"], "import");
}
