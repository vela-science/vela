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
use vela_protocol::frontier_settings::{FRONTIER_SETTINGS_SCHEMA, FrontierSettingsV1};

#[derive(Debug, Clone)]
pub(crate) struct CurrentInitOptions<'a> {
    pub(crate) name: &'a str,
    pub(crate) scope: &'a str,
    pub(crate) initialize_git: bool,
}

#[derive(Serialize)]
struct FrontierGenesisIdentity<'a> {
    schema: &'static str,
    name: &'a str,
    scope: &'a str,
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
        "vela authority init {} --reason 'Establish repository authority.' --json",
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
    let identity_bytes = vela_protocol::canonical::to_canonical_bytes(&FrontierGenesisIdentity {
        schema: "vela.frontier-genesis-identity.v1",
        name,
        scope,
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
        path.join("frontier.yaml"),
        serde_yaml::to_string(&profile)
            .map_err(|error| format!("serialize current Profile: {error}"))?,
    )
    .map_err(|error| format!("write frontier.yaml: {error}"))?;
    fs::create_dir_all(path.join(".vela")).map_err(|error| format!("create .vela: {error}"))?;
    fs::write(
        path.join(".vela/settings.toml"),
        FrontierSettingsV1 {
            schema: FRONTIER_SETTINGS_SCHEMA.into(),
            publish: None,
            work: None,
        }
        .to_toml()?,
    )
    .map_err(|error| format!("write .vela/settings.toml: {error}"))?;
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
            "SCOPE.md",
            "frontier.yaml",
            ".gitignore",
            ".gitattributes",
            "VELA.md",
            ".vela/settings.toml"
        ],
        "next_action": format!(
            "vela authority init {} --reason 'Establish repository authority.' --json",
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
            "# {name}\n\n{scope}\n\nThis is a Vela Frontier. Git stores exact Claims, Submissions, Verification Records, Decisions, and authority history. Derived views are rebuildable.\n\n```bash\nvela status . --json\nvela next . --limit 1 --json\nvela check . --strict --json\n```\n"
        ),
    )?;
    write(
        "SCOPE.md",
        &format!(
            "# Scope\n\n## Question\n\n{scope}\n\n## Includes\n\nNo additional inclusions are declared.\n\n## Excludes\n\nNo exclusions are declared.\n"
        ),
    )?;
    write(
        ".gitignore",
        "/.vela/keys/\n/.vela/operation-journals/\n/.vela/tmp/\n/.vela/work/\n/target/\nnode_modules/\n.DS_Store\n",
    )?;
    write(
        ".gitattributes",
        "* text=auto eol=lf\n.vela/** -filter -ident -working-tree-encoding -merge -text\nrecords/** -filter -ident -working-tree-encoding -merge diff text eol=lf\nartifacts/** -filter -ident -working-tree-encoding -merge -text\nfrontier.yaml -filter -ident -working-tree-encoding -merge diff text eol=lf\ntargets.json -filter -ident -working-tree-encoding -merge diff text eol=lf\n",
    )?;
    write(
        "VELA.md",
        &format!(
            "# {name} — agent charter\n\nCanonical state is Git history plus the current `.vela/repository.json` manifest. Producers may inspect work, start one bounded Attempt, submit evidence, and import scoped Verification Records. Only an authorized human Decision changes scientific standing.\n\nAgents must not invoke `vela review accept` or `vela review reject`, access repository-authority credentials, hand-edit canonical records, or describe Verification as acceptance.\n\n```bash\nvela status . --json\nvela next . --limit 1 --json\nvela start <target> --frontier . --as agent:<name> --json\nvela submit --frontier . --attempt <vat_id> --claim <bounded-claim> --type computational --replayability exact --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json\nvela check . --strict --json\n```\n"
        ),
    )
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
