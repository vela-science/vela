//! Plain configuration — the fourth and weakest config layer, by
//! design. The doctrine line (from the git/gh/Claude Code/Codex
//! study): **plain config may change how Vela speaks to you and how
//! mechanical consequences execute; it may never change what enters
//! the record, who may decide, or where signatures go** — those are
//! identity (`vela id`) and signed policy (`vela policy`), and no
//! scope of `vela config` can reach them.
//!
//! Layering is safety-aware rather than one unconditional nearest-wins rule:
//!   flag > VELA_* env > explicit user preference > allowlisted Frontier
//!   convention > built-in default. A Frontier's narrowing-only value is the
//!   exception: `publish.git_push = "off"` may override a wider user value.
//!
//! Two hard rules keep a cloned frontier from configuring its
//! operator (git's "protected configuration", Codex's project-scope
//! key blocking):
//!   1. The v1 Frontier file is a closed typed value; anything else fails
//!      validation. No predecessor configuration file is read.
//!   2. Safety-adjacent keys allowed there may only NARROW: a frontier
//!      can turn publishing off, never on.
//!
//! Closed key set (gh's lesson: a set you can `list` in full beats
//! git's unbounded sprawl). `set` validates; unknown keys are refused.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Component;
use std::path::{Path, PathBuf};

use vela_edge::repository_write::{
    PreparedRepositoryFileReplacement, RepositoryFileReplacementMode,
};
use vela_protocol::frontier_settings::{
    FRONTIER_SETTINGS_SCHEMA, FrontierGitPush, FrontierSettingsV1,
};

fn read_repository_control_text(
    repository: &Path,
    relative: &Path,
    label: &str,
) -> Result<Option<String>, String> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{label} must use a normalized repository-relative path"
        ));
    }

    let mut directories = Vec::with_capacity(components.len());
    let mut current = repository.to_path_buf();
    directories.push(current.clone());
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            unreachable!("closed component check above")
        };
        current.push(name);
        directories.push(current.clone());
    }

    let mut pinned = Vec::with_capacity(directories.len());
    for directory in &directories {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("inspect parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        pinned.push(
            same_file::Handle::from_path(directory)
                .map_err(|error| format!("identify parent of {label}: {error}"))?,
        );
    }

    let value = read_regular_nonsymlink_text(&repository.join(relative), label)?;
    for (directory, expected) in directories.iter().zip(&pinned) {
        let metadata = fs::symlink_metadata(directory)
            .map_err(|error| format!("reinspect parent of {label}: {error}"))?;
        let actual = same_file::Handle::from_path(directory)
            .map_err(|error| format!("reidentify parent of {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() || &actual != expected {
            return Err(format!(
                "repository parent of {label} changed while it was read"
            ));
        }
    }
    Ok(value)
}

fn read_regular_nonsymlink_text(path: &Path, label: &str) -> Result<Option<String>, String> {
    const MAX_CONTROL_FILE_BYTES: u64 = 16 * 1024 * 1024;

    let linked = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect {label}: {error}")),
    };
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    let inspected =
        same_file::Handle::from_path(path).map_err(|error| format!("identify {label}: {error}"))?;
    let mut file = fs::File::open(path).map_err(|error| format!("open {label}: {error}"))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone open {label} descriptor: {error}"))?,
    )
    .map_err(|error| format!("identify open {label}: {error}"))?;
    if inspected != opened {
        return Err(format!("{label} changed while it was opened"));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_CONTROL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label}: {error}"))?;
    if bytes.len() as u64 > MAX_CONTROL_FILE_BYTES {
        return Err(format!("{label} exceeds the 16 MiB control-file limit"));
    }
    let data =
        String::from_utf8(bytes).map_err(|error| format!("{label} is not UTF-8: {error}"))?;
    let final_link =
        fs::symlink_metadata(path).map_err(|error| format!("reinspect {label}: {error}"))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("reidentify {label}: {error}"))?;
    if final_link.file_type().is_symlink() || !final_link.is_file() || opened != final_identity {
        return Err(format!("{label} changed while it was read"));
    }
    Ok(Some(data))
}

/// Where a resolved value came from — `list --origins` renders this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Default,
    User,
    Frontier,
    Env,
}

impl Origin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::User => "user",
            Self::Frontier => "frontier",
            Self::Env => "env",
        }
    }
}

/// One known key: its default, its env alias, whether the frontier
/// scope may set it, and its validator.
#[derive(Debug)]
pub(crate) struct KeySpec {
    pub key: &'static str,
    pub default: &'static str,
    pub env: &'static str,
    /// None = user scope only. Some(narrowing) = frontier may set it;
    /// when `narrowing` is true the frontier value only applies if it
    /// is more restrictive than the user's (publish.* may force "off").
    pub frontier: Option<bool>,
    pub allowed: &'static [&'static str],
    pub help: &'static str,
}

/// The entire configurable universe, v1. Growing this list is a
/// deliberate release act, never a side effect.
pub(crate) const KEYS: &[KeySpec] = &[
    KeySpec {
        key: "publish.git_push",
        // Explicit publish, like git: a decision commits locally but does NOT
        // push. Publishing is a deliberate `git push` or `--push`. Set "auto"
        // (user config / env / CI) to opt back into auto-push.
        default: "off",
        env: "VELA_PUBLISH_GIT_PUSH",
        frontier: Some(true),
        allowed: &["auto", "off"],
        help: "push the decision commit automatically (default off; publish with git push or --push)",
    },
    KeySpec {
        key: "ui.color",
        default: "auto",
        env: "VELA_UI_COLOR",
        frontier: None,
        allowed: &["auto", "always", "never"],
        help: "terminal color (NO_COLOR is always respected)",
    },
    KeySpec {
        key: "ui.advice",
        default: "on",
        env: "VELA_ADVICE",
        frontier: None,
        allowed: &["on", "off", "0", "1"],
        help: "hint: lines naming the next command on errors",
    },
    KeySpec {
        key: "core.editor",
        default: "",
        env: "VELA_EDITOR",
        frontier: None,
        allowed: &[],
        help: "editor for sign-session notes (falls back to VISUAL/EDITOR)",
    },
    KeySpec {
        key: "work.lease_ttl_seconds",
        default: "86400",
        env: "VELA_WORK_LEASE_TTL_SECONDS",
        frontier: Some(false), // frontier may set its own convention outright
        allowed: &[],
        help: "default lease length for `work`/claim",
    },
];

pub(crate) fn spec(key: &str) -> Option<&'static KeySpec> {
    KEYS.iter().find(|s| s.key == key)
}

fn user_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".vela").join("config.toml"))
}

fn frontier_settings_path(frontier: &Path) -> PathBuf {
    frontier.join(".vela").join("settings.toml")
}

fn missing_v1_settings_error(frontier: &Path) -> String {
    format!(
        "current Frontier '{}' is missing required .vela/settings.toml; restore the exact typed settings file from its repository history",
        frontier.display()
    )
}

/// Flat dotted-key view of a config.toml.
fn load_flat(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return out;
    };
    let Ok(value) = raw.parse::<toml::Value>() else {
        eprintln!("  warning: {} is not valid TOML; ignored", path.display());
        return out;
    };
    if let toml::Value::Table(sections) = value {
        for (section, body) in sections {
            match body {
                toml::Value::Table(entries) => {
                    for (k, v) in entries {
                        let vs = match v {
                            toml::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        out.insert(format!("{section}.{k}"), vs);
                    }
                }
                other => {
                    out.insert(section, other.to_string());
                }
            }
        }
    }
    out
}

/// Load the single closed Frontier settings file. Missing, malformed, or
/// unknown content fails closed; predecessor configuration is never consulted.
fn load_frontier_flat(frontier: &Path) -> Result<BTreeMap<String, String>, String> {
    let settings_path = frontier_settings_path(frontier);
    let raw = read_repository_control_text(
        frontier,
        Path::new(".vela/settings.toml"),
        ".vela/settings.toml",
    )?;
    let Some(raw) = raw else {
        return Err(missing_v1_settings_error(frontier));
    };
    let settings = FrontierSettingsV1::from_toml(&raw).map_err(|error| {
        format!(
            "invalid Frontier settings '{}': {error}",
            settings_path.display()
        )
    })?;
    let mut out = BTreeMap::new();
    if let Some(publish) = settings.publish {
        let value = match publish.git_push {
            FrontierGitPush::Off => "off",
        };
        out.insert("publish.git_push".to_string(), value.to_string());
    }
    if let Some(work) = settings.work {
        out.insert(
            "work.lease_ttl_seconds".to_string(),
            work.lease_ttl_seconds.to_string(),
        );
    }
    Ok(out)
}

/// Resolve one key using safety-aware precedence. Flags stay at call sites.
pub(crate) fn resolve(key: &str, frontier: Option<&Path>) -> (String, Origin) {
    try_resolve(key, frontier).unwrap_or_else(|error| {
        eprintln!("  warning: {error}; using the safe built-in default");
        let default = spec(key).map_or("", |known| known.default);
        (default.to_string(), Origin::Default)
    })
}

/// Resolve one key while preserving v1 settings validation failures. Callers
/// whose behavior can write, publish, or expose tools must use this form and
/// stop rather than silently proceeding through an invalid checked-in file.
pub(crate) fn try_resolve(key: &str, frontier: Option<&Path>) -> Result<(String, Origin), String> {
    try_resolve_with(
        key,
        frontier,
        |name| std::env::var(name).ok(),
        user_config_path(),
    )
}

/// The injectable core (tests pass their own env/home).
fn try_resolve_with(
    key: &str,
    frontier: Option<&Path>,
    env_get: impl Fn(&str) -> Option<String>,
    user_path: Option<PathBuf>,
) -> Result<(String, Origin), String> {
    let Some(spec) = spec(key) else {
        return Ok((String::new(), Origin::Default));
    };
    if let Some(v) = env_get(spec.env)
        && !v.trim().is_empty()
    {
        validate_value(key, &v)?;
        return Ok((v, Origin::Env));
    }
    let user_val = user_path
        .map(|p| load_flat(&p))
        .and_then(|m| m.get(key).cloned());
    if let Some(value) = user_val.as_deref() {
        validate_value(key, value)
            .map_err(|error| format!("invalid user configuration for `{key}`: {error}"))?;
    }
    if let Some(dir) = frontier
        && let Some(frontier_rule) = spec.frontier
    {
        let fmap = load_frontier_flat(dir)?;
        if let Some(fv) = fmap.get(key) {
            validate_value(key, fv)
                .map_err(|error| format!("invalid Frontier configuration for `{key}`: {error}"))?;
            if frontier_rule {
                // Narrowing-only: the frontier may force the restrictive
                // value; it can never widen past the user's choice.
                if fv == "off" {
                    return Ok((fv.clone(), Origin::Frontier));
                }
            } else {
                // Plain frontier convention applies only when the operator has
                // not set an explicit user preference.
                if user_val.is_none() {
                    return Ok((fv.clone(), Origin::Frontier));
                }
            }
        }
    }
    if let Some(v) = user_val {
        return Ok((v, Origin::User));
    }
    Ok((spec.default.to_string(), Origin::Default))
}

/// Validate the one current Frontier settings file.
pub(crate) fn validate_frontier_settings(frontier: &Path) -> Result<(), String> {
    load_frontier_flat(frontier)?;
    Ok(())
}

/// Validate + write one key to the chosen scope's config.toml.
fn validate_value(key: &str, value: &str) -> Result<&'static KeySpec, String> {
    let spec = spec(key).ok_or_else(|| {
        format!("unknown key `{key}` — the set is closed; `vela config list` shows it in full")
    })?;
    if !spec.allowed.is_empty() && !spec.allowed.contains(&value) {
        return Err(format!(
            "`{value}` is not a valid {key} (allowed: {})",
            spec.allowed.join(", ")
        ));
    }
    if key == "work.lease_ttl_seconds" {
        let seconds = value
            .parse::<u64>()
            .map_err(|_| format!("`{value}` is not a positive integer for {key}"))?;
        if seconds == 0 {
            return Err(format!("`{value}` is not a positive integer for {key}"));
        }
        if seconds > vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS {
            return Err(format!(
                "`{value}` exceeds the maximum {} seconds for {key}",
                vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS
            ));
        }
    }
    Ok(spec)
}

pub(crate) fn set(key: &str, value: &str, frontier: Option<&Path>) -> Result<PathBuf, String> {
    let spec = validate_value(key, value)?;
    let path = match frontier {
        Some(dir) => {
            if spec.frontier.is_none() {
                return Err(format!(
                    "`{key}` is user-scope only (a cloned frontier must never control it); \
                     drop --frontier"
                ));
            }
            if spec.frontier == Some(true) && value != "off" {
                return Err(format!(
                    "frontier scope may only NARROW `{key}` (set `off`, or set it at user scope)"
                ));
            }
            let settings_path = frontier_settings_path(dir);
            if !settings_path.exists() {
                return Err(missing_v1_settings_error(dir));
            }
            set_frontier_v1_at(dir, key, Some(value))?;
            return Ok(settings_path);
        }
        None => user_config_path().ok_or("no HOME")?,
    };
    set_at(&path, key, value)?;
    Ok(path)
}

fn set_frontier_v1_at(frontier: &Path, key: &str, value: Option<&str>) -> Result<(), String> {
    set_frontier_v1_at_with_hook(frontier, key, value, || Ok(()))
}

fn set_frontier_v1_at_with_hook(
    frontier: &Path,
    key: &str,
    value: Option<&str>,
    before_replace: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let relative = Path::new(".vela/settings.toml");
    let path = frontier.join(relative);
    let raw = read_repository_control_text(frontier, relative, ".vela/settings.toml")?
        .ok_or_else(|| format!("Frontier settings '{}' disappeared", path.display()))?;
    let mut settings = FrontierSettingsV1::from_toml(&raw)
        .map_err(|error| format!("invalid Frontier settings '{}': {error}", path.display()))?;
    match (key, value) {
        ("publish.git_push", Some("off")) => {
            settings.publish = Some(vela_protocol::frontier_settings::PublishSettingsV1 {
                git_push: FrontierGitPush::Off,
            });
        }
        ("publish.git_push", None) => settings.publish = None,
        ("work.lease_ttl_seconds", Some(raw_seconds)) => {
            let lease_ttl_seconds = raw_seconds
                .parse::<u64>()
                .map_err(|_| format!("`{raw_seconds}` is not a positive lease duration"))?;
            settings.work =
                Some(vela_protocol::frontier_settings::WorkSettingsV1 { lease_ttl_seconds });
        }
        ("work.lease_ttl_seconds", None) => settings.work = None,
        _ => {
            return Err(format!("`{key}` is not part of {FRONTIER_SETTINGS_SCHEMA}"));
        }
    }
    let rendered = settings.to_toml()?;
    replace_frontier_settings(
        frontier,
        raw.as_bytes(),
        rendered.as_bytes(),
        before_replace,
    )
}

/// Install one current settings update without reopening the repository
/// path at the write edge. The settings file is read through
/// `read_repository_control_text`; the replacement is then created and
/// renamed relative to a pinned `.vela` directory descriptor. This prevents a
/// leaf or `.vela` symlink substitution from redirecting the write.
///
/// The shared edge helper additionally retains the exact file preimage and
/// uses no-clobber/exchange semantics. Platforms without the required
/// descriptor-relative primitive fail closed in that helper.
fn replace_frontier_settings(
    frontier: &Path,
    expected: &[u8],
    replacement: &[u8],
    before_replace: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    const MAX_SETTINGS_BYTES: u64 = 16 * 1024 * 1024;
    PreparedRepositoryFileReplacement::prepare_exact(
        frontier,
        Path::new(".vela/settings.toml"),
        Some(expected),
        replacement,
        RepositoryFileReplacementMode::PreserveExisting,
        MAX_SETTINGS_BYTES,
    )?
    .install_with_hook(before_replace)
    .map(|_| ())
}

/// Write one validated key into a specific config.toml.
fn set_at(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let mut root: toml::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(toml::Value::Table(Default::default()));
    let (section, name) = key.split_once('.').ok_or("keys are section.name")?;
    let table = root
        .as_table_mut()
        .ok_or("config root must be a table")?
        .entry(section.to_string())
        .or_insert(toml::Value::Table(Default::default()));
    table
        .as_table_mut()
        .ok_or_else(|| format!("[{section}] is not a table"))?
        .insert(name.to_string(), toml::Value::String(value.to_string()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        path,
        toml::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// Remove one key from the chosen scope.
pub(crate) fn unset(key: &str, frontier: Option<&Path>) -> Result<(), String> {
    let spec = spec(key).ok_or_else(|| format!("unknown key `{key}`"))?;
    let path = match frontier {
        Some(dir) => {
            if spec.frontier.is_none() {
                return Err(format!(
                    "`{key}` is user-scope only (a cloned frontier must never control it); \
                     drop --frontier"
                ));
            }
            if !frontier_settings_path(dir).exists() {
                return Err(missing_v1_settings_error(dir));
            }
            return set_frontier_v1_at(dir, key, None);
        }
        None => user_config_path().ok_or("no HOME")?,
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let mut root: toml::Value = raw.parse().map_err(|e| format!("parse: {e}"))?;
    let (section, name) = key.split_once('.').ok_or("keys are section.name")?;
    if let Some(table) = root
        .as_table_mut()
        .and_then(|t| t.get_mut(section))
        .and_then(|s| s.as_table_mut())
    {
        table.remove(name);
    }
    std::fs::write(
        &path,
        toml::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

// ── The `vela config` porcelain ─────────────────────────────────────

pub(crate) fn cmd_config_get(key: &str, frontier: Option<&Path>, json: bool) {
    if spec(key).is_none() {
        crate::ui::fail_with(
            crate::ui::ErrorKind::NotFound,
            &format!("unknown key `{key}`"),
            Some("`vela config list` shows the whole closed set"),
        );
    }
    let (value, origin) =
        try_resolve(key, frontier).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json {
        println!(
            "{}",
            serde_json::json!({"ok": true, "command": "config", "key": key, "value": value, "origin": origin.as_str()})
        );
    } else {
        println!("{value}");
    }
}

pub(crate) fn cmd_config_set(key: &str, value: &str, frontier: Option<&Path>, json: bool) {
    match set(key, value, frontier) {
        Ok(path) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "command": "config", "key": key, "value": value, "file": path.display().to_string()})
                );
            } else {
                println!("  · {key} = {value}  ({})", path.display());
            }
        }
        Err(e) => crate::ui::fail_with(crate::ui::ErrorKind::Usage, &e, None),
    }
}

pub(crate) fn cmd_config_unset(key: &str, frontier: Option<&Path>, json: bool) {
    match unset(key, frontier) {
        Ok(()) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "command": "config", "key": key, "unset": true})
                );
            } else {
                println!("  · {key} unset");
            }
        }
        Err(e) => crate::ui::fail_with(crate::ui::ErrorKind::Usage, &e, None),
    }
}

pub(crate) fn cmd_config_list(frontier: Option<&Path>, json: bool) {
    if let Some(dir) = frontier {
        validate_frontier_settings(dir).unwrap_or_else(|error| crate::cli::fail_return(&error));
    }
    if json {
        let rows: Vec<_> = KEYS
            .iter()
            .map(|s| {
                let (value, origin) = try_resolve(s.key, frontier)
                    .unwrap_or_else(|error| crate::cli::fail_return(&error));
                serde_json::json!({"key": s.key, "value": value, "origin": origin.as_str(), "help": s.help})
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({"ok": true, "command": "config", "keys": rows})
        );
        return;
    }
    crate::ui::header("CONFIG", "", Some("the whole closed set; origins shown"));
    for s in KEYS {
        let (value, origin) =
            try_resolve(s.key, frontier).unwrap_or_else(|error| crate::cli::fail_return(&error));
        println!("  {:<28} {:<12} [{}]", s.key, value, origin.as_str());
        println!("    {}", s.help);
    }
    println!("\n  identity + keys: `vela id` · what may enter the record: `vela policy`");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_set_and_validation() {
        let tmp = tempfile::TempDir::new().unwrap();
        let user = tmp.path().join("config.toml");
        assert!(spec("no.such_key").is_none());
        assert!(set_at(&user, "publish.git_push", "off").is_ok());
        // Validation happens in `set`, which needs a spec; probe it via
        // the public path against a frontier dir (scope errors) and the
        // allowed-values check.
        let err = validate_value("publish.git_push", "sometimes").unwrap_err();
        assert!(err.contains("not a valid"), "{err}");
        let (v, o) =
            try_resolve_with("publish.git_push", None, |_| None, Some(user.clone())).unwrap();
        assert_eq!((v.as_str(), o), ("off", Origin::User));
        assert!(validate_value("work.lease_ttl_seconds", "0").is_err());
        assert!(validate_value("work.lease_ttl_seconds", "not-a-number").is_err());
        assert!(
            validate_value(
                "work.lease_ttl_seconds",
                &(vela_protocol::events::MAX_ATTEMPT_LEASE_TTL_SECONDS + 1).to_string(),
            )
            .is_err()
        );
    }

    #[test]
    fn frontier_scope_is_protected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("f");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        std::fs::write(
            frontier.join(".vela/settings.toml"),
            "schema = \"vela.frontier-settings.v1\"\n",
        )
        .unwrap();
        // User-only keys refuse frontier scope outright; narrowing keys refuse
        // widening values.
        assert!(set("ui.color", "never", Some(&frontier)).is_err());
        assert!(set("publish.git_push", "auto", Some(&frontier)).is_err());
        set("publish.git_push", "off", Some(&frontier)).unwrap();
        let (v, o) = try_resolve_with("publish.git_push", Some(&frontier), |_| None, None).unwrap();
        assert_eq!((v.as_str(), o), ("off", Origin::Frontier));
        let (v, o) = try_resolve_with("ui.color", Some(&frontier), |_| None, None).unwrap();
        assert_ne!(v, "never");
        assert_ne!(o, Origin::Frontier);
        assert!(unset("ui.color", Some(&frontier)).is_err());
    }

    #[test]
    fn env_beats_files_and_default_falls_through() {
        let (v, o) = try_resolve_with(
            "ui.color",
            None,
            |name| (name == "VELA_UI_COLOR").then(|| "never".to_string()),
            None,
        )
        .unwrap();
        assert_eq!((v.as_str(), o), ("never", Origin::Env));
        let (v, o) = try_resolve_with("ui.color", None, |_| None, None).unwrap();
        assert_eq!((v.as_str(), o), ("auto", Origin::Default));

        assert!(
            try_resolve_with(
                "publish.git_push",
                None,
                |name| (name == "VELA_PUBLISH_GIT_PUSH").then(|| "unsafe".to_string()),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_user_preferences_beat_non_narrowing_frontier_conventions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        let user = tmp.path().join("user.toml");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        std::fs::write(
            frontier.join(".vela/settings.toml"),
            "schema = \"vela.frontier-settings.v1\"\n[work]\nlease_ttl_seconds = 7200\n",
        )
        .unwrap();
        std::fs::write(&user, "[work]\nlease_ttl_seconds = 3600\n").unwrap();

        let (ttl, ttl_origin) = try_resolve_with(
            "work.lease_ttl_seconds",
            Some(&frontier),
            |_| None,
            Some(user.clone()),
        )
        .unwrap();
        assert_eq!((ttl.as_str(), ttl_origin), ("3600", Origin::User));
    }

    #[test]
    fn v1_settings_are_closed_authoritative_and_safety_aware() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        let user = tmp.path().join("user.toml");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        std::fs::write(
            frontier.join(".vela/settings.toml"),
            r#"schema = "vela.frontier-settings.v1"

[publish]
git_push = "off"

[work]
lease_ttl_seconds = 7200

"#,
        )
        .unwrap();
        std::fs::write(
            &user,
            "[publish]\ngit_push = \"auto\"\n[work]\nlease_ttl_seconds = 3600\n",
        )
        .unwrap();

        let (push, push_origin) = try_resolve_with(
            "publish.git_push",
            Some(&frontier),
            |_| None,
            Some(user.clone()),
        )
        .unwrap();
        assert_eq!((push.as_str(), push_origin), ("off", Origin::Frontier));

        let (ttl, ttl_origin) = try_resolve_with(
            "work.lease_ttl_seconds",
            Some(&frontier),
            |_| None,
            Some(user.clone()),
        )
        .unwrap();
        assert_eq!((ttl.as_str(), ttl_origin), ("3600", Origin::User));
    }

    #[test]
    fn invalid_settings_fail_closed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        std::fs::write(
            frontier.join(".vela/settings.toml"),
            "schema = \"vela.frontier-settings.v1\"\n[policy]\nauto_accept = true\n",
        )
        .unwrap();
        let error =
            try_resolve_with("publish.git_push", Some(&frontier), |_| None, None).unwrap_err();
        assert!(error.contains("invalid Frontier settings"), "{error}");
    }

    #[test]
    fn missing_settings_fail_closed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();

        let error = try_resolve_with("work.lease_ttl_seconds", Some(&frontier), |_| None, None)
            .unwrap_err();
        assert!(
            error.contains("missing required .vela/settings.toml"),
            "{error}"
        );
        assert!(validate_frontier_settings(&frontier).is_err());
        assert!(set("work.lease_ttl_seconds", "7200", Some(&frontier)).is_err());
        assert!(unset("work.lease_ttl_seconds", Some(&frontier)).is_err());
    }

    #[test]
    fn settings_set_and_unset_keep_the_typed_schema() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        let settings_path = frontier.join(".vela/settings.toml");
        std::fs::write(&settings_path, "schema = \"vela.frontier-settings.v1\"\n").unwrap();

        assert_eq!(
            set("work.lease_ttl_seconds", "43200", Some(&frontier)).unwrap(),
            settings_path
        );
        set("publish.git_push", "off", Some(&frontier)).unwrap();
        let parsed =
            FrontierSettingsV1::from_toml(&std::fs::read_to_string(&settings_path).unwrap())
                .unwrap();
        assert_eq!(parsed.work.unwrap().lease_ttl_seconds, 43_200);
        assert_eq!(parsed.publish.unwrap().git_push, FrontierGitPush::Off);
    }

    #[cfg(unix)]
    #[test]
    fn v1_settings_reject_a_symlinked_parent_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&frontier).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let outside_settings = outside.join("settings.toml");
        let original = b"schema = \"vela.frontier-settings.v1\"\n";
        std::fs::write(&outside_settings, original).unwrap();
        symlink(&outside, frontier.join(".vela")).unwrap();

        let read_error =
            try_resolve_with("work.lease_ttl_seconds", Some(&frontier), |_| None, None)
                .unwrap_err();
        assert!(
            read_error.contains("real non-symlink repository directories"),
            "{read_error}"
        );
        let write_error = set("work.lease_ttl_seconds", "43200", Some(&frontier)).unwrap_err();
        assert!(
            write_error.contains("real non-symlink repository directories"),
            "{write_error}"
        );
        assert_eq!(std::fs::read(outside_settings).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn v1_settings_leaf_swap_before_replace_fails_closed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        let vela = frontier.join(".vela");
        std::fs::create_dir_all(&vela).unwrap();
        let settings = vela.join("settings.toml");
        let displaced = vela.join("settings.original.toml");
        let original = b"schema = \"vela.frontier-settings.v1\"\n";
        let substituted =
            b"schema = \"vela.frontier-settings.v1\"\n[work]\nlease_ttl_seconds = 7200\n";
        std::fs::write(&settings, original).unwrap();

        let error = set_frontier_v1_at_with_hook(
            &frontier,
            "work.lease_ttl_seconds",
            Some("43200"),
            || {
                std::fs::rename(&settings, &displaced).map_err(|error| error.to_string())?;
                std::fs::write(&settings, substituted).map_err(|error| error.to_string())
            },
        )
        .unwrap_err();
        assert!(error.contains("changed before replacement"), "{error}");
        assert_eq!(std::fs::read(&displaced).unwrap(), original);
        assert_eq!(std::fs::read(&settings).unwrap(), substituted);
        assert!(std::fs::read_dir(&vela).unwrap().flatten().all(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".vela-replace-")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn v1_settings_parent_swap_before_replace_fails_closed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("frontier");
        let vela = frontier.join(".vela");
        let displaced = frontier.join(".vela-original");
        std::fs::create_dir_all(&vela).unwrap();
        let original = b"schema = \"vela.frontier-settings.v1\"\n";
        std::fs::write(vela.join("settings.toml"), original).unwrap();

        let error = set_frontier_v1_at_with_hook(
            &frontier,
            "work.lease_ttl_seconds",
            Some("43200"),
            || {
                std::fs::rename(&vela, &displaced).map_err(|error| error.to_string())?;
                std::fs::create_dir(&vela).map_err(|error| error.to_string())?;
                std::fs::write(
                    vela.join("settings.toml"),
                    b"schema = \"vela.frontier-settings.v1\"\n[publish]\ngit_push = \"off\"\n",
                )
                .map_err(|error| error.to_string())
            },
        )
        .unwrap_err();
        assert!(
            error.contains("repository parent of .vela/settings.toml changed"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(displaced.join("settings.toml")).unwrap(),
            original
        );
        assert!(
            std::fs::read_dir(&displaced)
                .unwrap()
                .flatten()
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".vela-replace-"))
        );
        assert_eq!(
            std::fs::read(vela.join("settings.toml")).unwrap(),
            b"schema = \"vela.frontier-settings.v1\"\n[publish]\ngit_push = \"off\"\n"
        );
    }
}
