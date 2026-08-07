//! The one binding rule for the Frontier argument.
//!
//! The surface used to carry four shapes and four resolution behaviours for
//! the same concept: a required leading positional on `show`, `why`, `review`,
//! and `verification`; an optional one on `status`, `next`, and `log`; a
//! `default_value = "."` one on `authority trust pin`; and a `--repo` flag
//! on `start` and `submit`. A reader who learned `vela status` and then typed
//! `vela show vcl_…` had the object id silently bound to the Frontier slot and
//! was told the *object id* was missing.
//!
//! One rule now covers every verb that acts on an existing Frontier:
//!
//!   1. `--repo <path>` is accepted everywhere.
//!   2. The Frontier may also be the leading positional.
//!   3. Omitted entirely, it is discovered upward from the current directory.
//!
//! Positional binding is by count, not by guessing: the verb's object
//! arguments bind last, so the Frontier is whatever leading positional is left
//! over. `vela show vcl_…` and `vela show . vcl_…` therefore mean the same
//! thing. `log` is the single verb whose object is *also* optional, so count
//! cannot separate the two; there — and only there — a lone positional that is
//! a Vela object id is read as the object. [`looks_like_object_id`] is that
//! test, and it is deliberately checked after `is_dir`, so a directory always
//! wins over a name that merely looks like an id.

use std::path::{Path, PathBuf};

use crate::ui::{self, ErrorKind};

/// Does this token look like the id of a retained Vela object rather than a
/// path? The prefixed forms are minted in vela-protocol (`vcl_`, `vpr_`,
/// `vsb_`, `vvr_`, `vpw_`, `vev_`); Artifact ids carry no prefix at all and are
/// the bare lowercase sha256 of the content, which is why the bare-hex arm
/// exists. Used for the `log` tie-break and to turn "no such directory" into an
/// error that names the real mistake.
pub(crate) fn looks_like_object_id(token: &str) -> bool {
    let hex = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit());
    if let Some((prefix, digest)) = token.split_once('_') {
        return matches!(prefix, "vcl" | "vpr" | "vsb" | "vvr" | "vpw" | "vev") && hex(digest);
    }
    token.len() == 64 && hex(token)
}

fn given_twice(verb: &str) -> ! {
    ui::fail_with(
        ErrorKind::Usage,
        "the Frontier was given twice: once positionally and once with --repo",
        Some(&format!("drop one, e.g. `vela {verb} --repo <path>`")),
    );
}

/// Refuse a leading positional that is an object id where a Frontier belongs,
/// instead of letting it fall through to "Frontier directory does not exist"
/// with a hint pointing at `vela init` — a *writing* verb offered to repair an
/// argument-order mistake.
fn reject_object_id_as_repo(verb: &str, candidate: &Path) {
    let Some(token) = candidate.to_str() else {
        return;
    };
    if candidate.is_dir() || !looks_like_object_id(token) {
        return;
    }
    ui::fail_with(
        ErrorKind::Usage,
        &format!("`vela {verb}` reads a Frontier path here, and {token} is an object id"),
        Some(&format!(
            "the Frontier is optional and discovered upward: try `vela show {token}`"
        )),
    );
}

/// Verbs whose only Frontier-shaped argument is the Frontier itself:
/// `status`, `next`, `replay`, `review inbox`, `review list`,
/// `authority trust pin`.
pub(crate) fn bind_repo(
    verb: &str,
    positional: Option<PathBuf>,
    flag: Option<PathBuf>,
) -> PathBuf {
    match (positional, flag) {
        (Some(_), Some(_)) => given_twice(verb),
        (Some(path), None) => {
            reject_object_id_as_repo(verb, &path);
            path
        }
        (None, explicit) => ui::resolve_repo(explicit),
    }
}

/// Verbs that take the Frontier plus exactly one required object:
/// `show`, `why`, `review show|accept|reject|withdraw`,
/// `verification record|import`.
///
/// `object` names the missing argument in the usage error — `"a Claim id
/// (vcl_...)"` — and `value_name` is its slot in the usage line, so an omitted
/// object is reported as an omitted object rather than as a missing Frontier.
pub(crate) fn bind_repo_and_object(
    verb: &str,
    object: &str,
    value_name: &str,
    first: Option<String>,
    second: Option<String>,
    flag: Option<PathBuf>,
) -> (PathBuf, String) {
    let missing = || -> ! {
        ui::fail_with(
            ErrorKind::Usage,
            &format!("`vela {verb}` needs {object}"),
            Some(&format!(
                "`vela {verb} <{value_name}>`, or name the Frontier first: `vela {verb} <FRONTIER> <{value_name}>`"
            )),
        )
    };
    match (first, second) {
        (Some(_), Some(_)) if flag.is_some() => given_twice(verb),
        (Some(frontier), Some(object)) => {
            let frontier = PathBuf::from(frontier);
            reject_object_id_as_repo(verb, &frontier);
            (frontier, object)
        }
        (Some(lone), None) => {
            /* A lone positional is the object — except when it is a directory
            that sits on disk, which no object id is. That is the old habit
            (`vela show .`) with the object forgotten, and answering it with
            "no exact current object '.'" would name the wrong mistake. */
            if flag.is_none() && !looks_like_object_id(&lone) && Path::new(&lone).is_dir() {
                missing();
            }
            (ui::resolve_repo(flag), lone)
        }
        (None, _) => missing(),
    }
}

/// `log`, the one verb whose object filter is optional too. Count cannot
/// separate a lone positional, so shape does — after `is_dir`.
pub(crate) fn bind_repo_and_optional_object(
    verb: &str,
    first: Option<String>,
    second: Option<String>,
    flag: Option<PathBuf>,
) -> (PathBuf, Option<String>) {
    match (first, second) {
        (Some(_), Some(_)) if flag.is_some() => given_twice(verb),
        (Some(frontier), Some(object)) => {
            let frontier = PathBuf::from(frontier);
            reject_object_id_as_repo(verb, &frontier);
            (frontier, Some(object))
        }
        (Some(lone), None) => {
            if flag.is_some() {
                return (ui::resolve_repo(flag), Some(lone));
            }
            let candidate = PathBuf::from(&lone);
            if !candidate.is_dir() && looks_like_object_id(&lone) {
                return (ui::resolve_repo(None), Some(lone));
            }
            (candidate, None)
        }
        (None, _) => (ui::resolve_repo(flag), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixed_object_ids_are_recognised() {
        for id in [
            "vcl_002c4bd422f81bc4b9500c82b6918d4b767c6e70aa06ab10e15fa363c1b7bbb1",
            "vpr_a8f8cb7709e55c5f",
            "vsb_0123456789abcdef",
            "vvr_0123456789abcdef",
            "vpw_0123456789abcdef",
            "vev_dd9a35d858ed9b13",
        ] {
            assert!(looks_like_object_id(id), "{id}");
        }
    }

    /// Artifact ids carry no `v??_` prefix — they are the bare sha256 of the
    /// content, exactly as `.vela/repository.json` records them. A prefix-only
    /// test would have missed every Artifact on every live Frontier.
    #[test]
    fn bare_sha256_artifact_ids_are_recognised() {
        assert!(looks_like_object_id(
            "06c47cd7517759340c8695152b01e537d04560eb38f4874069a73429918acb82"
        ));
    }

    #[test]
    fn paths_are_not_object_ids() {
        for path in [
            ".",
            "..",
            "/Users/x/erdos-frontier",
            "erdos-frontier",
            "vcl_",
            "vcl_notatallhex",
            "vro_0123456789abcdef",
            "0123456789abcdef",
        ] {
            assert!(!looks_like_object_id(path), "{path}");
        }
    }
}
