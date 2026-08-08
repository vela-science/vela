//! The published Action must invoke a verb this binary actually has.
//!
//! `check` was renamed to `replay` and shipped without an alias. `action.yml`
//! kept calling `check`, and nothing caught it: the contract test in
//! vela-protocol asserted the literal string `"$vela_bin" check "$FRONTIER"
//! --json`, so it pinned the stale verb rather than checking it existed. Every
//! consumer repository pins an older Action tag, so all of them stayed green and
//! all of them would have broken together on the first pin bump.
//!
//! This test closes that gap the only way it can be closed — by asking the
//! binary. It parses the verb out of the Action's own script and runs it, so a
//! future rename fails here rather than in four downstream repositories.
//!
//! The input name is held here for the same reason. The Action takes one
//! `repository` path and nothing else, and every step that needs it reads the
//! resolved output rather than the input, so there is one place a rename has to
//! reach.

use std::path::Path;
use std::process::Command;

/// The Action's source, read from the workspace root.
fn action_source() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../action.yml"))
        .expect("action.yml must be readable from the workspace root")
}

/// The vela invocation inside the Action's verification step.
fn action_verb() -> String {
    let action = action_source();

    let line = action
        .lines()
        .map(str::trim)
        .find(|line| line.contains("\"$vela_bin\""))
        .expect("action.yml must invoke the pinned binary");

    let after = line
        .split_once("\"$vela_bin\"")
        .expect("the invocation must name the binary")
        .1
        .trim();
    let verb = after
        .split_whitespace()
        .next()
        .expect("the invocation must name a subcommand")
        .to_string();
    assert!(
        !verb.starts_with('-') && !verb.starts_with('"') && !verb.starts_with('$'),
        "expected a literal subcommand in the Action invocation, found {verb}"
    );
    verb
}

#[test]
fn the_action_invokes_a_subcommand_this_binary_has() {
    let verb = action_verb();
    let output = Command::new(env!("CARGO_BIN_EXE_vela"))
        .args([verb.as_str(), "--help"])
        .output()
        .expect("the vela binary must run");

    assert!(
        output.status.success(),
        "action.yml runs `vela {verb}`, which this binary rejects:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn the_action_verb_is_read_only() {
    /* The Action is a verification gate in a consumer's CI. A verb that could
    write would run with whatever credentials that workflow holds, which is
    the one thing the Action's own header promises it never does. */
    let verb = action_verb();
    const WRITING: &[&str] = &[
        "init",
        "submit",
        "verification",
        "review",
        "authority",
        "start",
    ];
    assert!(
        !WRITING.contains(&verb.as_str()),
        "action.yml invokes `{verb}`, which can write; the Action must stay read-only"
    );
}

/// The `inputs:` block, as written, without the rest of the document.
fn declared_inputs() -> String {
    let action = action_source();
    let (_, after) = action
        .split_once("\ninputs:\n")
        .expect("action.yml must declare inputs");
    let (block, _) = after
        .split_once("\nruns:\n")
        .expect("the inputs block must end where `runs:` begins");
    block.to_string()
}

#[test]
fn the_action_takes_one_repository_path_and_reads_it_once() {
    let inputs = declared_inputs();
    let declared: Vec<&str> = inputs
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':'))
        .collect();
    assert_eq!(
        declared,
        ["  repository:"],
        "the Action accepts a path and nothing else; a second key is a second \
         thing it can be told to do:\n{inputs}"
    );

    let action = action_source();
    /* One reader, one place to change. Every consuming step used to name the
    input itself, which made the key four couplings rather than one, and a
    fifth step would have been a fifth. */
    assert_eq!(
        action.matches("inputs.repository").count(),
        1,
        "`inputs.repository` must be read exactly once, by the step that \
         resolves it; every other step reads the resolved output"
    );

    let resolved = "steps.resolve.outputs.path";
    let consumers = action.matches(resolved).count();
    assert!(
        consumers >= 4,
        "the resolved path reaches only {consumers} steps; every step that \
         takes the repository path must read `{resolved}` rather than an input"
    );
}
