//! Native current-repository bootstrap.
//!
//! Filesystem setup belongs to the CLI edge. The protocol crate supplies only
//! closed values and validators.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::current_repository::{
    CURRENT_FRONTIER_PROFILE_SCHEMA_V2, CurrentFrontierProfileV2, FrontierProfileLicenseV2,
    FrontierProfileScopeV2,
};

#[derive(Debug, Clone)]
pub(crate) struct CurrentInitOptions<'a> {
    pub(crate) name: &'a str,
    pub(crate) scope: &'a str,
    pub(crate) initialize_git: bool,
}

/* A Frontier is one independently clonable repository with a bounded scope, so
its identity must distinguish repositories rather than questions. The v1
preimage was exactly the declared name and scope, which handed two unrelated
repositories that chose the same wording one identity; the user-local trust
store keys on `vfr_`, so the second repository's authority anchor then refused
to install over the first. The repository origin would be the natural
distinguishing commitment, but `vro_` is derived from the frontier_id, and it
is written by `vela authority init` after this point, so it cannot enter this
preimage. A fresh 256-bit draw from the OS CSPRNG is what makes this repository
this one. It is not retained: nothing recomputes a frontier_id, and a retained
nonce would not make the identity checkable, because the creator chooses it. */
#[derive(Serialize)]
struct FrontierGenesisIdentity<'a> {
    schema: &'static str,
    name: &'a str,
    scope: &'a str,
    genesis_entropy: &'a str,
}

fn draw_genesis_entropy() -> Result<String, String> {
    use rand_core::RngCore;

    let mut bytes = [0u8; 32];
    rand_core::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|error| format!("draw Frontier genesis entropy: {error}"))?;
    Ok(hex::encode(bytes))
}

pub(crate) fn initialize_current_minimal(
    path: &Path,
    options: CurrentInitOptions<'_>,
) -> Result<Value, String> {
    let name = options.name.trim();
    let scope = options.scope.trim();
    if name.is_empty() {
        return Err("current initialization requires a non-empty name".into());
    }
    if scope.is_empty() {
        return Err("current initialization requires a non-empty bounded scope".into());
    }

    let target_existed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "{} must be a real directory or an absent path",
                path.display()
            ));
        }
        Ok(_) => {
            let mut entries = fs::read_dir(path)
                .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?;
            if entries
                .next()
                .transpose()
                .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?
                .is_some()
            {
                return Err(format!(
                    "refusing to initialize non-empty directory {}",
                    path.display()
                ));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|error| {
                format!(
                    "failed to create frontier directory '{}': {error}",
                    path.display()
                )
            })?;
            false
        }
        Err(error) => return Err(format!("inspect init target '{}': {error}", path.display())),
    };

    let staging = tempfile::Builder::new()
        .prefix(".vela-init-")
        .tempdir_in(path)
        .map_err(|error| format!("create initialization staging directory: {error}"))?;
    let mut payload = match initialize_in_place(staging.path(), &options) {
        Ok(payload) => payload,
        Err(error) => {
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(error);
        }
    };

    let mut entries = fs::read_dir(staging.path())
        .map_err(|error| format!("read initialization staging directory: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read initialization staging entry: {error}"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut installed = Vec::new();
    for entry in entries {
        let destination = path.join(entry.file_name());
        if fs::symlink_metadata(&destination).is_ok() {
            rollback_install(&installed);
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(format!(
                "initialization target changed while staging: {}",
                destination.display()
            ));
        }
        if let Err(error) = fs::rename(entry.path(), &destination) {
            rollback_install(&installed);
            drop(staging);
            if !target_existed {
                let _ = fs::remove_dir(path);
            }
            return Err(format!(
                "install initialized repository entry '{}': {error}",
                destination.display()
            ));
        }
        installed.push((entry.path(), destination));
    }
    drop(staging);

    payload["path"] = json!(path.display().to_string());
    payload["next_action"] = json!(format!(
        "vela init {} --json",
        shell_arg(&path.display().to_string())
    ));
    Ok(payload)
}

fn rollback_install(installed: &[(std::path::PathBuf, std::path::PathBuf)]) {
    for (source, destination) in installed.iter().rev() {
        let _ = fs::rename(destination, source);
    }
}

fn initialize_in_place(path: &Path, options: &CurrentInitOptions<'_>) -> Result<Value, String> {
    let name = options.name.trim();
    let scope = options.scope.trim();
    let genesis_entropy = draw_genesis_entropy()?;
    let identity_bytes = vela_protocol::canonical::to_canonical_bytes(&FrontierGenesisIdentity {
        schema: "vela.frontier-genesis-identity.v2",
        name,
        scope,
        genesis_entropy: &genesis_entropy,
    })?;
    let frontier_id = format!("vfr_{}", &hex::encode(Sha256::digest(identity_bytes))[..16]);
    let profile = CurrentFrontierProfileV2 {
        schema: CURRENT_FRONTIER_PROFILE_SCHEMA_V2.into(),
        frontier_id: frontier_id.clone(),
        name: name.into(),
        summary: scope.into(),
        scope: FrontierProfileScopeV2 {
            question: scope.into(),
            includes: Vec::new(),
            excludes: Vec::new(),
        },
        maintainers: Vec::new(),
        license: FrontierProfileLicenseV2 {
            content: "CC-BY-4.0".into(),
            code: "Apache-2.0".into(),
            data: "varies".into(),
        },
    };
    profile.validate()?;
    let profile_root = profile.profile_root()?;

    fs::write(
        path.join("frontier.toml"),
        toml::to_string_pretty(&profile)
            .map_err(|error| format!("serialize current Profile: {error}"))?,
    )
    .map_err(|error| format!("write frontier.toml: {error}"))?;
    fs::create_dir_all(path.join(".vela")).map_err(|error| format!("create .vela: {error}"))?;
    write_scaffold(path, name, scope)?;
    initialize_git(path, options.initialize_git)?;

    Ok(json!({
        "schema": "vela.frontier-init.v2",
        "ok": true,
        "layout": "vela.repository-bootstrap.v1",
        "path": path.display().to_string(),
        "name": name,
        "scope": scope,
        "frontier_id": frontier_id,
        "profile_root": profile_root,
        "authority": "uninitialized",
        "scientific_object_count": 0,
        "wrote": [
            "README.md",
            "frontier.toml",
            ".gitignore",
            ".gitattributes",
            "AGENTS.md",
            "CLAUDE.md"
        ],
        "next_action": format!(
            "vela init {} --json",
            shell_arg(&path.display().to_string())
        )
    }))
}

fn write_scaffold(path: &Path, name: &str, scope: &str) -> Result<(), String> {
    let write = |relative: &str, contents: &str| -> Result<(), String> {
        fs::write(path.join(relative), contents)
            .map_err(|error| format!("write {relative}: {error}"))
    };
    write(
        "README.md",
        &format!(
            "# {name}\n\n{scope}\n\nThis is a Vela Frontier. Git stores exact Claims, Submissions, Verification Records, Decisions, and authority history. Derived views are rebuildable.\n\n## Operator loop\n\n```bash\nvela status . --json\nvela next . --limit 1 --json\nvela submit --frontier . --claim \"<bounded result>\" --type computational --replayability exact --artifact <path>:<kind> --caveat \"<limit>\" --as agent:<name> --json\n\n# Verification binds method bytes already retained at the current Git commit.\ngit add -- verification/method.json\ngit commit -m \"Retain verification method\"\nvela verification record . <vpr_id> --profile <profile> --method verification/method.json --outcome pass --does-not-establish \"Scientific acceptance.\" --as verifier:<name> --json\n\nvela review inbox . --json\n# Only an authorized operator may make the exact accept or reject Decision.\nvela review accept . <vpr_id> --reason \"<reason>\" --if-entry-root sha256:... --json\nvela replay . --json\n```\n"
        ),
    )?;
    /* No SCOPE.md. It restated the scope already in `frontier.toml`, which
    `profile_root` commits to, and the scaffold could only fill its Includes and
    Excludes with "none are declared" — so the file arrived saying nothing and
    then drifted from the declaration it duplicated. Three published Frontiers
    carried byte-identical copies whose own text said scope lives in
    `frontier.toml`; they have been deleted, and a fresh `vela init` must not
    recreate what they dropped. */
    /* The runtime creates more under `.vela` than this used to list, so a fresh
    Frontier staged its task leases, workspaces, source inbox, agent state and
    key material into Git. Three of the four published Frontiers hand-patched
    exactly these entries; the fourth patched them incompletely and still
    carries unignored runtime directories. */
    write(
        ".gitignore",
        concat!(
            "/.vela/operation-journals/\n",
            "/.vela/tmp/\n",
            "/.vela/work/\n",
            "/.vela/tasks/\n",
            "/.vela/workspaces/\n",
            "/.vela/source-inbox/\n",
            "/.vela/agents/\n",
            "/.vela/keys/\n",
            "/.vela/artifact-blobs/\n",
            "/target/\n",
            "node_modules/\n",
            ".DS_Store\n",
        ),
    )?;
    write(
        ".gitattributes",
        "* text=auto eol=lf\n.vela/** -filter -ident -working-tree-encoding -merge -text\nrecords/** -filter -ident -working-tree-encoding -merge diff text eol=lf\nartifacts/** -filter -ident -working-tree-encoding -merge -text\nfrontier.toml -filter -ident -working-tree-encoding -merge diff text eol=lf\ntargets.json -filter -ident -working-tree-encoding -merge diff text eol=lf\n",
    )?;
    /* AGENTS.md, not VELA.md. FRONTIER_REPOSITORY_PROFILE.md names README.md
    and AGENTS.md as the guidance set, and all four published Frontiers carry
    AGENTS.md; none has ever had a VELA.md. A scaffold that writes a filename no
    repository uses guarantees the first act after `vela init` is renaming it. */
    write(
        "AGENTS.md",
        &format!(
            "# {name} — agent charter\n\nCanonical state is Git history plus the current `.vela/repository.json` manifest. Producers may inspect exact Target briefings, submit signed evidence directly, and record scoped Verification. Only an authorized human Decision changes scientific standing.\n\nAgents must not invoke `vela review accept` or `vela review reject`, access repository-authority credentials, hand-edit canonical records, or describe Verification as acceptance. A Verification method manifest must be tracked, clean, and retained in the current Git commit before `vela verification record`.\n\n```bash\nvela status . --json\nvela next . --limit 1 --json\nvela start <target> --json\nvela submit --frontier . --claim <bounded-claim> --type computational --replayability exact --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json\nvela verification record . <vpr_id> --profile <profile> --method <committed-method> --outcome <outcome> --does-not-establish <limit> --as verifier:<name> --json\nvela review inbox . --json\nvela replay . --json\n```\n\nHand the rooted Decision Inbox entry to the authorized operator; do not decide it yourself.\n"
        ),
    )?;
    /* One line pointing at the charter. All four published Frontiers carry
    exactly this file with exactly this content, so the convention is
    unanimous and was simply never scaffolded. */
    write("CLAUDE.md", "@AGENTS.md\n")
}

fn initialize_git(path: &Path, requested: bool) -> Result<(), String> {
    if !requested || path.join(".git").exists() {
        return Ok(());
    }
    let output = Command::new("git")
        .args(["init", "--quiet", "-b", "main"])
        .arg(path)
        .output()
        .map_err(|error| format!("run git init: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize(root: &Path, name: &str, scope: &str) -> Value {
        initialize_current_minimal(
            root,
            CurrentInitOptions {
                name,
                scope,
                initialize_git: false,
            },
        )
        .expect("initialize a fresh Frontier")
    }

    fn frontier_id(payload: &Value) -> String {
        payload["frontier_id"]
            .as_str()
            .expect("frontier_id is a string")
            .to_string()
    }

    #[test]
    fn identical_name_and_scope_do_not_collide_across_repositories() {
        let parent = tempfile::tempdir().expect("staging parent");
        let first = initialize(
            &parent.path().join("first"),
            "Sidon sets",
            "Do B_h[g] sets grow?",
        );
        let second = initialize(
            &parent.path().join("second"),
            "Sidon sets",
            "Do B_h[g] sets grow?",
        );

        assert_ne!(
            frontier_id(&first),
            frontier_id(&second),
            "two independently created repositories must not share one identity"
        );
        assert_ne!(first["profile_root"], second["profile_root"]);
    }

    #[test]
    fn genesis_identity_keeps_the_declared_frontier_id_shape() {
        let parent = tempfile::tempdir().expect("staging parent");
        let payload = initialize(
            &parent.path().join("frontier"),
            "Bounded name",
            "Does X hold?",
        );
        let id = frontier_id(&payload);

        let suffix = id.strip_prefix("vfr_").expect("vfr_ prefix");
        assert_eq!(suffix.len(), 16);
        assert!(
            suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );

        let profile = vela_protocol::current_repository::CurrentFrontierProfileV2::from_toml_str(
            &fs::read_to_string(parent.path().join("frontier").join("frontier.toml"))
                .expect("read retained frontier.toml"),
        )
        .expect("retained profile validates");
        assert_eq!(profile.frontier_id, id);
    }

    #[test]
    fn genesis_entropy_is_a_fresh_full_width_draw() {
        let first = draw_genesis_entropy().expect("draw entropy");
        let second = draw_genesis_entropy().expect("draw entropy");
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }
}
