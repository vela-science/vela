use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn vela_bin() -> PathBuf {
    if let Ok(env_path) = std::env::var("VELA_BIN") {
        return PathBuf::from(env_path);
    }
    if let Ok(env_path) = std::env::var("CARGO_BIN_EXE_vela") {
        return PathBuf::from(env_path);
    }
    // CI may have built only the release binary; check both locations.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let debug = manifest.join("../../target/debug/vela");
    if debug.is_file() {
        return debug;
    }
    let release = manifest.join("../../target/release/vela");
    if release.is_file() {
        return release;
    }
    debug
}

fn run_text(args: &[&str]) -> String {
    let output = Command::new(vela_bin())
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        output.status.success(),
        "vela command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command output was not UTF-8")
}

fn run_expect_failure(args: &[&str]) -> String {
    let output = Command::new(vela_bin())
        .args(args)
        .output()
        .expect("failed to run vela");
    assert!(
        !output.status.success(),
        "vela command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The README's install block advertises a release, so it is release contract.
///
/// It had been left on `v0.966.2` through the whole of `v0.966.3`, which is the
/// one drift a reader meets first: the quick start installs a binary older than
/// everything the page goes on to describe. Nothing checked it, because the tag
/// lived only in prose.
///
/// This asserts agreement rather than restating the tag. The workspace version
/// is the declaration; every `v…` the install block carries is read out of the
/// README and held to it, so bumping the version is the only edit that moves
/// this test, and forgetting the README is what fails.
#[test]
fn readme_install_block_advertises_the_released_version() {
    const README: &str = include_str!("../../../README.md");
    let expected = format!("v{}", env!("CARGO_PKG_VERSION"));

    let advertised: Vec<&str> = README
        .match_indices("https://raw.githubusercontent.com/vela-science/vela/")
        .map(|(at, prefix)| &README[at + prefix.len()..])
        .chain(
            README
                .match_indices("VELA_VERSION=")
                .map(|(at, prefix)| &README[at + prefix.len()..]),
        )
        .map(|rest| {
            rest.split(['/', ' ', '\n'])
                .next()
                .expect("split always yields one field")
        })
        .collect();

    assert!(
        advertised.len() >= 2,
        "the README no longer carries an install block to check"
    );
    for tag in advertised {
        assert_eq!(
            tag, expected,
            "the README install block advertises {tag}, not the released {expected}"
        );
    }
}

/// The documentation index covers the documentation directory.
///
/// `docs/README.md` and the roster `vela-web` publishes from were two lists of
/// one set, and they disagreed: the site published AGENT_QUICKSTART,
/// ARCHITECTURE and REPOSITORY_PROFILE while this index named none of
/// them. Neither list is the set — `docs/` is — so each is now held to it: the
/// web script reads the tree at its pinned commit, and this reads the working
/// tree. `docs/adr/` and `docs/history/` are linked as directories and keep
/// their own indexes, so only the top level is covered here — and those indexes
/// are held to their directories below, because that sentence was an assertion
/// about a file that did not exist: `docs/adr/` had no index at all, and the
/// exclusion justified itself with the thing it was excluding.
/// The two directories the index above excludes must actually index themselves.
///
/// Excluding a subtree because it "keeps its own index" is only sound if one
/// exists and covers it. Read from the directory, so adding an ADR without
/// listing it fails here rather than leaving a decision record that nothing
/// points at.
#[test]
fn every_subtree_index_lists_its_own_directory() {
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    for subtree in ["adr", "history"] {
        let directory = docs.join(subtree);
        let index = std::fs::read_to_string(directory.join("README.md"))
            .unwrap_or_else(|_| panic!("docs/{subtree}/ is excluded from the top-level index because it keeps its own, and has no README.md"));
        let mut present: Vec<String> = std::fs::read_dir(&directory)
            .unwrap_or_else(|_| panic!("read docs/{subtree}/"))
            .map(|entry| entry.expect("directory entry").file_name())
            .filter_map(|name| name.to_str().map(str::to_string))
            .filter(|name| name.ends_with(".md") && name != "README.md")
            .collect();
        present.sort();
        assert!(
            present.len() > 1,
            "docs/{subtree}/ holds {} markdown files; the test is reading the wrong directory",
            present.len()
        );
        let unlinked: Vec<&String> = present
            .iter()
            .filter(|name| !index.contains(&format!("]({name})")))
            .collect();
        assert!(
            unlinked.is_empty(),
            "docs/{subtree}/README.md does not link: {unlinked:?}"
        );
    }
}

#[test]
fn the_documentation_index_lists_every_current_document() {
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let index = std::fs::read_to_string(docs.join("README.md")).expect("read docs/README.md");

    let mut present: Vec<String> = std::fs::read_dir(&docs)
        .expect("read docs/")
        .map(|entry| entry.expect("docs/ entry").file_name())
        .filter_map(|name| name.to_str().map(str::to_string))
        .filter(|name| name.ends_with(".md") && name != "README.md")
        .collect();
    present.sort();
    assert!(
        present.len() > 10,
        "docs/ holds {} markdown files; the test is reading the wrong directory",
        present.len()
    );

    let unlinked: Vec<&String> = present
        .iter()
        .filter(|name| !index.contains(&format!("]({name})")))
        .collect();
    assert!(
        unlinked.is_empty(),
        "docs/README.md does not link {unlinked:?}"
    );

    let linked: Vec<String> = index
        .match_indices("](")
        .map(|(at, prefix)| &index[at + prefix.len()..])
        .filter_map(|rest| rest.split(')').next())
        .filter(|target| target.ends_with(".md") && !target.contains('/'))
        .map(str::to_string)
        .collect();
    let dangling: Vec<&String> = linked
        .iter()
        .filter(|target| !docs.join(target).is_file())
        .collect();
    assert!(
        dangling.is_empty(),
        "docs/README.md links documents docs/ does not hold: {dangling:?}"
    );
}

/// The Rust version is `rust-toolchain.toml`'s to declare, and Cargo's to agree
/// with.
///
/// It was written out in four places: this workspace's `rust-version`, the
/// toolchain file's `channel`, and a string in each of the two workflows. The
/// workflows now read the toolchain file. Cargo cannot — there is no way to
/// point `rust-version` at another file — so the fourth copy stays and is held
/// here instead. `rust-version` is the MSRV a consumer is promised and
/// `channel` is what CI builds with; they are different statements, and the one
/// this project makes is that they are the same version. A bump moves the
/// toolchain file, and forgetting Cargo is what fails.
#[test]
fn the_workspace_msrv_is_the_pinned_toolchain() {
    const TOOLCHAIN: &str = include_str!("../../../rust-toolchain.toml");
    const WORKSPACE: &str = include_str!("../../../Cargo.toml");

    fn quoted(document: &str, key: &str) -> String {
        let values: Vec<&str> = document
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix(key))
            .filter_map(|rest| rest.trim_start().strip_prefix('='))
            .filter_map(|rest| rest.trim().strip_prefix('"'))
            .filter_map(|rest| rest.strip_suffix('"'))
            .collect();
        assert_eq!(
            values.len(),
            1,
            "expected exactly one `{key}` declaration, found {}",
            values.len()
        );
        values[0].to_string()
    }

    assert_eq!(
        quoted(WORKSPACE, "rust-version"),
        quoted(TOOLCHAIN, "channel"),
        "Cargo.toml's rust-version and rust-toolchain.toml's channel name different compilers"
    );
}

#[test]
fn replay_missing_repository_reports_error_without_panic() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("missing-repository.json");

    // `replay` distinguishes a missing path from both a native bootstrap and a
    // retired repository. It must not fall through to a legacy loader.
    let error = run_expect_failure(&["replay", missing.to_str().unwrap()]);

    assert!(error.contains("repository directory does not exist"));
    assert!(error.contains("vela init <dir>"));
    assert!(!error.contains("panicked at"));
}

#[test]
fn advanced_help_uses_current_product_commands() {
    let help = run_text(&["help", "advanced"]);

    for command in [
        "init",
        "status",
        "next",
        "start",
        "submit",
        "show",
        "why",
        "review",
        "replay",
        "reproduce",
        "log",
    ] {
        assert!(
            help.contains(&format!("  {command}")),
            "advanced help omitted current product command: {command}"
        );
    }
    assert!(help.contains("replay        Replay, signatures, parity, and repository integrity"));
    assert!(help.contains("reproduce     Re-run stored witnesses with frozen verifiers"));
    assert!(help.contains("review        Inspect or perform one exact Proposal lifecycle action"));
    assert!(help.contains("verification  Retain non-authorizing scoped Verification Records"));
    assert!(!help.contains("  id "));

    assert!(!help.contains("bridges derive"));
    assert!(!help.contains("vela workbench"));
    // The help must advertise nothing the binary cannot run.
    for dead in [
        "scout",
        "compile-notes",
        "clinical-trial-import",
        "source-inbox",
        "constellation",
        "federation",
        "  bridge ",
        "  packet ",
        "  bench ",
        "Workbench",
    ] {
        assert!(
            !help.contains(dead),
            "help advanced still advertises removed surface: {dead}"
        );
    }
}

#[test]
fn verification_help_exposes_ordinary_authoring_without_key_flags() {
    let help = run_text(&["verification", "record", "--help"]);

    for flag in [
        "--profile",
        "--method",
        "--property",
        "--complementary",
        "--outcome",
        "--does-not-establish",
        "--independent-of",
        "--shared-dependency",
        "--as",
        "--json",
    ] {
        assert!(
            help.contains(flag),
            "verification record help omitted {flag}"
        );
    }
    // The repository is optional on every verb that acts on an existing one: it is
    // discovered upward when omitted, and may be given either as the leading positional
    // or as `--repo`. Help must show both spellings, and must show the positional as
    // optional -- `<REPO>` here would assert the retired required-positional surface.
    assert!(
        help.contains("[REPO]"),
        "verification record help omitted the optional repository positional"
    );
    assert!(
        help.contains("--repo"),
        "verification record help omitted the --repo spelling"
    );
    // The Proposal stays required, and the usage line is what states so.
    assert!(help.contains("<PROPOSAL>"));
    assert!(!help.contains("--key"));
}

/// The profile contract's example TOML must parse as a profile.
///
/// `docs/REPOSITORY_PROFILE.md` documented `frontier_id = "vfr_…"` against
/// `CurrentRepositoryProfileV1`, whose field is `repository_id` under
/// `#[serde(deny_unknown_fields)]`. A reader who copied the documented block
/// got a hard parse rejection from the schema the same document describes, and
/// nothing read the block, so it could say anything.
#[test]
fn the_profile_contract_documents_a_profile_that_parses() {
    let document = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/REPOSITORY_PROFILE.md"),
    )
    .expect("read docs/REPOSITORY_PROFILE.md");
    let block = document
        .split_once("```toml\n")
        .expect("the profile contract must show one example profile")
        .1
        .split_once("```")
        .expect("the example profile block is unclosed")
        .0;
    let profile =
        vela_protocol::current_repository::CurrentRepositoryProfileV1::from_toml_str(block)
            .expect("the documented profile must parse");
    profile
        .validate()
        .expect("the documented profile must validate");
}

/// One tagline, held to the one the binary prints.
///
/// "version control for living science" and "version control for scientific
/// state" shipped at once across README.md and four documents, and the binary
/// printed the second. A reader's first sentence about the product depended on
/// which file they opened.
#[test]
fn the_documented_tagline_is_the_one_the_binary_prints() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let printed = run_text(&["help"]);
    let tagline = printed
        .lines()
        .nth(1)
        .expect("`vela help` prints a tagline under the version");
    assert_eq!(tagline, "Version control for scientific state.");

    let phrase = tagline.trim_end_matches('.').to_ascii_lowercase();
    for name in [
        "README.md",
        "docs/ARCHITECTURE.md",
        "docs/TERMINOLOGY.md",
        "docs/ROADMAP.md",
        "docs/PROTOCOL.md",
        "docs/QUICKSTART.md",
    ] {
        let document = std::fs::read_to_string(root.join(name))
            .unwrap_or_else(|error| panic!("read {name}: {error}"));
        let lowered = document.to_ascii_lowercase();
        assert!(
            lowered.contains(&phrase),
            "{name} does not carry the tagline the binary prints"
        );
        assert!(
            !lowered.contains("version control for living science"),
            "{name} carries the retired tagline"
        );
    }
}
