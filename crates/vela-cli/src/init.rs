//! Native current-repository bootstrap.
//!
//! Filesystem setup belongs to the CLI edge. The protocol crate supplies only
//! closed values and validators.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vela_protocol::repository::{
    REPOSITORY_PROFILE_SCHEMA_V1, RepositoryProfileLicenseV1, RepositoryProfileScopeV1,
    RepositoryProfileV1,
};

#[derive(Debug, Clone)]
pub(crate) struct InitOptions<'a> {
    pub(crate) name: &'a str,
    pub(crate) scope: &'a str,
}

pub(crate) fn initialize_minimal(path: &Path, options: InitOptions<'_>) -> Result<Value, String> {
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
            if fs::read_dir(path)
                .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?
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
                    "failed to create repository directory '{}': {error}",
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

    if let Err(error) = install_staged_entries(path, staging.path()) {
        drop(staging);
        if !target_existed {
            let _ = fs::remove_dir(path);
        }
        return Err(error);
    }
    drop(staging);

    payload["path"] = json!(path.display().to_string());
    payload["next_action"] = json!(format!(
        "vela init {} --json",
        shell_arg(&path.display().to_string())
    ));
    Ok(payload)
}

/// Resume exactly one crash-retained bootstrap staging directory.
///
/// The staging directory is CLI-owned private state, not a serialized write
/// permission. It contains only a retained Profile, its deterministic
/// scaffold, an empty `.vela` directory, and optionally the exact unborn Git
/// repository created by the isolated initializer. Every byte and filesystem
/// kind is revalidated before any remaining top-level entry is installed.
pub(crate) fn resume_staged_minimal(
    path: &Path,
    name: Option<&str>,
    scope: Option<&str>,
) -> Result<Option<Value>, String> {
    let Some(staging) = find_initialization_staging(path)? else {
        return Ok(None);
    };
    let profile = validate_staged_bootstrap(path, &staging, name, scope)?;
    let git = distributed_entry(path, &staging, ".git")?;
    match git {
        Some(git) => crate::config::git_publish::verify_empty_native_git_repository(
            git.parent()
                .ok_or_else(|| "staged Git repository has no parent".to_string())?,
        )?,
        None => crate::config::git_publish::initialize_native_git_repository(&staging)?,
    }
    // Re-run the complete proof after Git initialization, before moving a byte.
    validate_staged_bootstrap(path, &staging, name, scope)?;
    install_staged_entries(path, &staging)?;
    fs::remove_dir(&staging).map_err(|error| {
        format!(
            "remove completed initialization staging directory '{}': {error}",
            staging.display()
        )
    })?;
    let mut payload = bootstrap_payload(path, &profile)?;
    payload["resumed"] = json!(true);
    Ok(Some(payload))
}

fn find_initialization_staging(path: &Path) -> Result<Option<PathBuf>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect init target '{}': {error}", path.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(None);
    }
    let mut staging = None;
    for entry in fs::read_dir(path)
        .map_err(|error| format!("inspect init target '{}': {error}", path.display()))?
    {
        let entry = entry.map_err(|error| format!("inspect init target entry: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(".vela-init-") {
            continue;
        }
        let suffix = &name[".vela-init-".len()..];
        if suffix.len() != 6 || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(format!(
                "initialization target contains an invalid staging entry {name}"
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("inspect initialization staging entry {name}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "initialization staging entry {name} must be a real directory"
            ));
        }
        if staging.replace(entry.path()).is_some() {
            return Err("initialization target contains multiple staging directories".into());
        }
    }
    Ok(staging)
}

fn distributed_entry(root: &Path, staging: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let installed = root.join(name);
    let staged = staging.join(name);
    let exists = |path: &Path| match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "inspect distributed initialization entry '{}': {error}",
            path.display()
        )),
    };
    let installed_exists = exists(&installed)?;
    let staged_exists = exists(&staged)?;
    match (installed_exists, staged_exists) {
        (true, false) => Ok(Some(installed)),
        (false, true) => Ok(Some(staged)),
        (false, false) => Ok(None),
        (true, true) => Err(format!(
            "initialization entry {name} exists in both staging and target"
        )),
    }
}

fn validate_staged_bootstrap(
    root: &Path,
    staging: &Path,
    name: Option<&str>,
    scope: Option<&str>,
) -> Result<RepositoryProfileV1, String> {
    let known = [
        ".git",
        ".gitattributes",
        ".gitignore",
        ".vela",
        "AGENTS.md",
        "CLAUDE.md",
        "README.md",
        "vela.toml",
    ];
    for directory in [root, staging] {
        for entry in fs::read_dir(directory).map_err(|error| {
            format!(
                "read initialization directory '{}': {error}",
                directory.display()
            )
        })? {
            let entry = entry.map_err(|error| format!("read initialization entry: {error}"))?;
            if directory == root && entry.path() == staging {
                continue;
            }
            let entry_name = entry.file_name();
            let entry_name = entry_name.to_str().ok_or_else(|| {
                "initialization state contains a non-UTF-8 top-level entry".to_string()
            })?;
            if !known.contains(&entry_name) {
                return Err(format!(
                    "initialization state contains unexpected top-level entry {entry_name}"
                ));
            }
        }
    }
    let profile_path = distributed_entry(root, staging, "vela.toml")?
        .ok_or_else(|| "initialization staging has no retained vela.toml".to_string())?;
    let profile = RepositoryProfileV1::from_toml_str(
        &fs::read_to_string(&profile_path)
            .map_err(|error| format!("read retained staged Profile: {error}"))?,
    )?;
    if name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| value != profile.name)
        || scope
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some_and(|value| value != profile.scope.question)
    {
        return Err("retry inputs do not match the retained initialization Profile".into());
    }
    for (relative, expected) in expected_scaffold(&profile)? {
        let path = distributed_entry(root, staging, &relative)?.ok_or_else(|| {
            format!("initialization staging is missing deterministic scaffold {relative}")
        })?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect staged scaffold {relative}: {error}"))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || fs::read(&path).map_err(|error| format!("read staged scaffold: {error}"))?
                != expected
        {
            return Err(format!(
                "initialization scaffold {relative} differs from its retained Profile"
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 != 0 {
                return Err(format!(
                    "initialization scaffold {relative} must have Git mode 100644"
                ));
            }
        }
    }
    let private = distributed_entry(root, staging, ".vela")?.ok_or_else(|| {
        "initialization staging is missing its private bootstrap directory".to_string()
    })?;
    let metadata = fs::symlink_metadata(&private)
        .map_err(|error| format!("inspect staged private bootstrap: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("initialization private bootstrap must be a real directory".into());
    }
    if fs::read_dir(&private)
        .map_err(|error| format!("read staged private bootstrap: {error}"))?
        .next()
        .transpose()
        .map_err(|error| format!("read staged private bootstrap entry: {error}"))?
        .is_some()
    {
        return Err("initialization private bootstrap contains unexpected state".into());
    }
    if let Some(git) = distributed_entry(root, staging, ".git")? {
        crate::config::git_publish::verify_empty_native_git_repository(
            git.parent()
                .ok_or_else(|| "staged Git repository has no parent".to_string())?,
        )?;
    }
    Ok(profile)
}

fn install_staged_entries(root: &Path, staging: &Path) -> Result<(), String> {
    let mut entries = fs::read_dir(staging)
        .map_err(|error| format!("read initialization staging directory: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read initialization staging entry: {error}"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut installed = Vec::new();
    for entry in entries {
        let destination = root.join(entry.file_name());
        if fs::symlink_metadata(&destination).is_ok() {
            rollback_install(&installed);
            return Err(format!(
                "initialization target changed while staging: {}",
                destination.display()
            ));
        }
        if let Err(error) = fs::rename(entry.path(), &destination) {
            rollback_install(&installed);
            return Err(format!(
                "install initialized repository entry '{}': {error}",
                destination.display()
            ));
        }
        installed.push((entry.path(), destination));
    }
    Ok(())
}

fn rollback_install(installed: &[(std::path::PathBuf, std::path::PathBuf)]) {
    for (source, destination) in installed.iter().rev() {
        let _ = fs::rename(destination, source);
    }
}

fn initialize_in_place(path: &Path, options: &InitOptions<'_>) -> Result<Value, String> {
    let name = options.name.trim();
    let scope = options.scope.trim();
    let repository_id = uuid::Uuid::new_v4().hyphenated().to_string();
    let profile = RepositoryProfileV1 {
        schema: REPOSITORY_PROFILE_SCHEMA_V1.into(),
        repository_id: repository_id.clone(),
        name: name.into(),
        summary: scope.into(),
        scope: RepositoryProfileScopeV1 {
            question: scope.into(),
            includes: Vec::new(),
            excludes: Vec::new(),
        },
        maintainers: Vec::new(),
        license: RepositoryProfileLicenseV1 {
            content: "CC-BY-4.0".into(),
            code: "Apache-2.0".into(),
            data: "NOASSERTION".into(),
        },
    };
    profile.validate()?;

    fs::create_dir_all(path.join(".vela")).map_err(|error| format!("create .vela: {error}"))?;
    write_scaffold(path, &profile)?;
    #[cfg(feature = "test-support")]
    if std::env::var_os("VELA_TEST_INTERRUPT_INIT_BEFORE_GIT").is_some() {
        std::process::exit(86);
    }
    crate::config::git_publish::initialize_native_git_repository(path)?;

    bootstrap_payload(path, &profile)
}

fn bootstrap_payload(path: &Path, profile: &RepositoryProfileV1) -> Result<Value, String> {
    Ok(json!({
        "schema": "vela.repository-init-draft.v1",
        "ok": true,
        "layout": "vela.repository-bootstrap.v1",
        "path": path.display().to_string(),
        "name": profile.name,
        "scope": profile.scope.question,
        "repository_id": profile.repository_id,
        "profile_root": profile.profile_root()?,
        "authority": "uninitialized",
        "scientific_object_count": 0,
        "wrote": [
            "README.md",
            "vela.toml",
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

fn write_scaffold(path: &Path, profile: &RepositoryProfileV1) -> Result<(), String> {
    for (relative, bytes) in expected_scaffold(profile)? {
        fs::write(path.join(&relative), bytes)
            .map_err(|error| format!("write {relative}: {error}"))?;
    }
    Ok(())
}

pub(crate) fn expected_scaffold(
    profile: &RepositoryProfileV1,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    profile.validate()?;
    let name = &profile.name;
    let scope = &profile.scope.question;
    let mut files = BTreeMap::new();
    files.insert(
        "README.md".into(),
        format!(
            "# {name}\n\n{scope}\n\nThis is a Vela repository. Git stores exact Claims, Submissions, Verification Records, Decisions, and authority history. Derived views are rebuildable.\n\n## Operator loop\n\n```bash\nvela status . --json\nvela submit --repo . --claim \"<bounded result>\" --type computational --replayability exact --artifact <path>:<kind> --caveat \"<limit>\" --as agent:<name> --json\n\n# Verification binds method bytes already retained at the current Git commit.\ngit add -- verification/method.json\ngit commit -m \"Retain verification method\"\nvela verification record . <vpr_id> --profile <profile> --method verification/method.json --outcome pass --does-not-establish \"Scientific acceptance.\" --as verifier:<name> --json\n\nvela review inbox . --json\n# Only an authorized operator may make the exact accept or reject Decision.\nvela review accept . <vpr_id> --reason \"<reason>\" --if-entry-root sha256:... --json\nvela replay . --json\n```\n"
        )
        .into_bytes(),
    );
    /* No SCOPE.md. It restated the scope already in `vela.toml`, which
    `profile_root` commits to, and the scaffold could only fill its Includes and
    Excludes with "none are declared" — so the file arrived saying nothing and
    then drifted from the declaration it duplicated. Three published repositories
    carried byte-identical copies whose own text said scope lives in
    `vela.toml`; they have been deleted, and a fresh `vela init` must not
    recreate what they dropped. */
    /* The runtime creates more under `.vela` than this used to list, so a fresh
    repository staged its task leases, workspaces, source inbox, agent state and
    key material into Git. Three of the four published repositories hand-patched
    exactly these entries; the fourth patched them incompletely and still
    carries unignored runtime directories. */
    files.insert(
        ".gitignore".into(),
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
        )
        .as_bytes()
        .to_vec(),
    );
    files.insert(
        ".gitattributes".into(),
        // The record path family is `-text`, not `text eol=lf`.
        //
        // A record is content addressed: its root is sha256 over the exact bytes
        // Git holds. Any end-of-line normalization rewrites those bytes on
        // checkout, and replay then reads a file whose digest is not the one the
        // manifest binds. All four published repositories carry `-text` here and
        // had to hand-correct this scaffold to get it; erdos still carries a
        // comment explaining the fix. `vela.toml` keeps `text eol=lf` because
        // it is configuration a human edits and is not hashed by content.
        //
        // That profile line named `frontier.toml` until now, and `vela init`
        // has never written a file by that name — the profile is `vela.toml`,
        // as the paragraph above always said. The rule therefore matched
        // nothing, and the one file here a human is expected to edit was left
        // to `* text=auto`. Live repositories carry the dead line too.
        "* text=auto eol=lf\n.vela/** -filter -ident -working-tree-encoding -merge -text\nrecords/** -filter -ident -working-tree-encoding -merge -text\nartifacts/** -filter -ident -working-tree-encoding -merge -text\nvela.toml -filter -ident -working-tree-encoding -merge diff text eol=lf\n"
            .as_bytes()
            .to_vec(),
    );
    /* AGENTS.md, not VELA.md. REPOSITORY_PROFILE.md names README.md
    and AGENTS.md as the guidance set, and all four published repositories carry
    AGENTS.md; none has ever had a VELA.md. A scaffold that writes a filename no
    repository uses guarantees the first act after `vela init` is renaming it. */
    files.insert(
        "AGENTS.md".into(),
        format!(
            "# {name} — agent charter\n\nCanonical state is Git history plus the current `.vela/repository.json` manifest. Producers may submit signed evidence directly and record scoped Verification. Only an authorized, attributed Decision changes scientific standing; human and agent performers use the same exact-root and replay checks.\n\nAgents must not copy or export repository-authority credentials, hand-edit canonical records, or describe Verification as acceptance. An agent selected to decide uses `vela review accept|reject --as agent:<name>` and may bind a source-owned `--session-ref`; Repository authority signs the transaction. A Verification method manifest must be tracked, clean, and retained in the current Git commit before `vela verification record`.\n\n```bash\nvela status . --json\nvela submit --repo . --claim <bounded-claim> --type computational --replayability exact --artifact <path>:<kind> --caveat <limit> --as agent:<name> --json\nvela verification record . <vpr_id> --profile <profile> --method <committed-method> --outcome <outcome> --does-not-establish <limit> --as verifier:<name> --json\nvela review inbox . --json\nvela review accept . <vpr_id> --if-entry-root sha256:... --reason <reason> --as agent:<name> --session-ref <ref> --json\nvela replay . --json\n```\n"
        )
        .into_bytes(),
    );
    /* One line pointing at the charter. All four published repositories carry
    exactly this file with exactly this content, so the convention is
    unanimous and was simply never scaffolded. */
    files.insert("CLAUDE.md".into(), b"@AGENTS.md\n".to_vec());
    files.insert(
        "vela.toml".into(),
        toml::to_string_pretty(profile)
            .map_err(|error| format!("serialize current Profile: {error}"))?
            .into_bytes(),
    );
    Ok(files)
}

fn shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize(root: &Path, name: &str, scope: &str) -> Value {
        initialize_minimal(root, InitOptions { name, scope })
            .expect("initialize a fresh repository")
    }

    fn repository_id(payload: &Value) -> String {
        payload["repository_id"]
            .as_str()
            .expect("repository_id is a string")
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
            repository_id(&first),
            repository_id(&second),
            "two independently created repositories must not share one identity"
        );
        assert_ne!(first["profile_root"], second["profile_root"]);
    }

    #[test]
    fn genesis_identity_keeps_the_declared_repository_id_shape() {
        let parent = tempfile::tempdir().expect("staging parent");
        let payload = initialize(
            &parent.path().join("repository_path"),
            "Bounded name",
            "Does X hold?",
        );
        let id = repository_id(&payload);

        assert!(vela_protocol::is_repository_id(&id));

        let profile = vela_protocol::repository::RepositoryProfileV1::from_toml_str(
            &fs::read_to_string(parent.path().join("repository_path").join("vela.toml"))
                .expect("read retained vela.toml"),
        )
        .expect("retained profile validates");
        assert_eq!(profile.repository_id, id);
    }

    #[test]
    fn repository_id_is_a_fresh_uuidv4() {
        let first = uuid::Uuid::new_v4().hyphenated().to_string();
        let second = uuid::Uuid::new_v4().hyphenated().to_string();
        assert!(vela_protocol::is_repository_id(&first));
        assert!(vela_protocol::is_repository_id(&second));
        assert_ne!(first, second);
    }
}
