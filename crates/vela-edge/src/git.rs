//! Shared ambient-process isolation for Git callers.
//!
//! An ordinary Git subprocess is not a trust boundary: inherited `GIT_*`
//! variables and process configuration can redirect repository discovery,
//! object lookup, the index, replacement refs, shallow state, or hooks. The
//! write-neutral [`isolate_ambient`] helper removes that shared ambient state.
//!
//! Repository discovery, reads, initialization, and publication remain with
//! their owning callers. This module does not choose or execute an operation.

use std::ffi::OsString;
use std::process::Command;

const NULL_DEVICE: &str = "/dev/null";

/// Remove hostile ambient Git state and install the process settings shared
/// by read and write callers.
pub fn isolate_ambient(command: &mut Command) {
    isolate_ambient_from(command, std::env::vars_os().map(|(name, _)| name));
}

fn isolate_ambient_from(
    command: &mut Command,
    inherited_environment: impl IntoIterator<Item = OsString>,
) {
    for name in inherited_environment {
        if name.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(name);
        }
    }
    command
        .args([
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "-c",
            "core.bare=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "diff.external=",
            "-c",
            "submodule.recurse=false",
            "-c",
            "protocol.file.allow=never",
        ])
        .envs([
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_SYSTEM", NULL_DEVICE),
            ("GIT_CONFIG_GLOBAL", NULL_DEVICE),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_PAGER", "cat"),
            ("PAGER", "cat"),
            ("LC_ALL", "C"),
        ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};

    #[test]
    fn ambient_isolation_removes_the_git_namespace_and_sets_only_common_state() {
        let mut inherited = vec![
            OsString::from("GIT_DIR"),
            OsString::from("GIT_WORK_TREE"),
            OsString::from("GIT_INDEX_FILE"),
            OsString::from("GIT_CONFIG_KEY_0"),
            OsString::from("GIT_NO_LAZY_FETCH"),
            OsString::from("GIT_EDITOR"),
        ];
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;

            let name = OsString::from_vec(b"GIT_\xff".to_vec());
            inherited.push(name);
        }
        let mut command = Command::new("git");
        isolate_ambient_from(&mut command, inherited.iter().cloned());
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<BTreeMap<_, _>>();

        for name in inherited {
            if name == "GIT_NO_LAZY_FETCH" {
                assert_eq!(
                    environment.get(&name),
                    Some(&Some(OsString::from("1"))),
                    "lazy fetch must remain disabled after ambient scrubbing"
                );
                continue;
            }
            assert_eq!(
                environment.get(&name),
                Some(&None),
                "inherited Git environment was not removed: {name:?}"
            );
        }
        for (name, value) in [
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_PAGER", "cat"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("LC_ALL", "C"),
            ("PAGER", "cat"),
        ] {
            assert_eq!(
                environment.get(OsStr::new(name)),
                Some(&Some(OsString::from(value))),
                "missing common Git environment: {name}"
            );
        }
        for required in [
            "--no-pager",
            "--no-optional-locks",
            "--no-replace-objects",
            "core.bare=false",
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "core.attributesFile=/dev/null",
            "core.excludesFile=/dev/null",
            "diff.external=",
            "submodule.recurse=false",
            "protocol.file.allow=never",
        ] {
            assert!(
                command.get_args().any(|arg| arg == required),
                "missing common Git argument: {required}"
            );
        }
        assert!(!command.get_args().any(|arg| arg == "--literal-pathspecs"));
        assert!(!command.get_args().any(|arg| arg == "commit.gpgSign=false"));
    }
}
