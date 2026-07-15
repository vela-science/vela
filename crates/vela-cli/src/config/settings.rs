//! Plain configuration — the fourth and weakest config layer, by
//! design. The doctrine line (from the git/gh/Claude Code/Codex
//! study): **plain config may change how Vela speaks to you and how
//! mechanical consequences execute; it may never change what enters
//! the record, who may decide, or where signatures go** — those are
//! identity (`vela id`) and signed policy (`vela policy`), and no
//! scope of `vela config` can reach them.
//!
//! Layering, nearest-wins for preferences:
//!   flag > VELA_* env > frontier .vela/config.toml (ALLOWLISTED keys
//!   only) > user ~/.vela/config.toml > built-in default
//!
//! Two hard rules keep a cloned frontier from configuring its
//! operator (git's "protected configuration", Codex's project-scope
//! key blocking):
//!   1. The frontier file is read for an explicit allowlist of keys;
//!      anything else warns and is ignored.
//!   2. Safety-adjacent keys allowed there may only NARROW: a frontier
//!      can turn publishing off, never on; hub routing is not readable
//!      from frontier scope at all.
//!
//! Closed key set (gh's lesson: a set you can `list` in full beats
//! git's unbounded sprawl). `set` validates; unknown keys are refused.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
        key: "hub.url",
        default: crate::cli_identity::DEFAULT_HUB,
        env: "VELA_HUB_URL",
        frontier: None, // routing is never frontier-configurable (Codex base_url rule)
        allowed: &[],
        help: "the hub this machine publishes to and verifies against",
    },
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
    KeySpec {
        key: "mcp.profile",
        default: "read-only",
        env: "VELA_MCP_PROFILE",
        frontier: Some(false),
        allowed: &["read-only", "draft"],
        help: "default serve/agents-sync MCP profile",
    },
];

pub(crate) fn spec(key: &str) -> Option<&'static KeySpec> {
    KEYS.iter().find(|s| s.key == key)
}

fn user_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".vela").join("config.toml"))
}

fn frontier_config_path(frontier: &Path) -> PathBuf {
    frontier.join(".vela").join("config.toml")
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

/// Resolve one key: env > frontier (allowlisted, narrowing honored) >
/// user > default. Flags stay at call sites.
pub(crate) fn resolve(key: &str, frontier: Option<&Path>) -> (String, Origin) {
    resolve_with(
        key,
        frontier,
        |name| std::env::var(name).ok(),
        user_config_path(),
    )
}

/// The injectable core (tests pass their own env/home).
fn resolve_with(
    key: &str,
    frontier: Option<&Path>,
    env_get: impl Fn(&str) -> Option<String>,
    user_path: Option<PathBuf>,
) -> (String, Origin) {
    let Some(spec) = spec(key) else {
        return (String::new(), Origin::Default);
    };
    if let Some(v) = env_get(spec.env)
        && !v.trim().is_empty()
    {
        return (v, Origin::Env);
    }
    let user_val = user_path
        .map(|p| load_flat(&p))
        .and_then(|m| m.get(key).cloned());
    if let Some(dir) = frontier
        && let Some(frontier_rule) = spec.frontier
    {
        let fmap = load_flat(&frontier_config_path(dir));
        if let Some(fv) = fmap.get(key) {
            if frontier_rule {
                // Narrowing-only: the frontier may force the restrictive
                // value; it can never widen past the user's choice.
                if fv == "off" {
                    return (fv.clone(), Origin::Frontier);
                }
            } else {
                // Plain frontier convention; user config still loses to it
                // only when the user has not set the key.
                if user_val.is_none() {
                    return (fv.clone(), Origin::Frontier);
                }
            }
        }
    }
    if let Some(v) = user_val {
        return (v, Origin::User);
    }
    (spec.default.to_string(), Origin::Default)
}

/// Warn once per invocation about frontier keys outside the allowlist —
/// silence is how config becomes a trust hole.
pub(crate) fn warn_unknown_frontier_keys(frontier: &Path) {
    let path = frontier_config_path(frontier);
    if !path.exists() {
        return;
    }
    for key in load_flat(&path).keys() {
        match spec(key) {
            Some(s) if s.frontier.is_some() => {}
            _ => eprintln!(
                "  warning: frontier config key `{key}` is not frontier-scoped; ignored \
                 (routing/identity/UI keys are user-scope only)"
            ),
        }
    }
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
            frontier_config_path(dir)
        }
        None => user_config_path().ok_or("no HOME")?,
    };
    set_at(&path, key, value)?;
    Ok(path)
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
    spec(key).ok_or_else(|| format!("unknown key `{key}`"))?;
    let path = match frontier {
        Some(dir) => frontier_config_path(dir),
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
    let (value, origin) = resolve(key, frontier);
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
        warn_unknown_frontier_keys(dir);
    }
    if json {
        let rows: Vec<_> = KEYS
            .iter()
            .map(|s| {
                let (value, origin) = resolve(s.key, frontier);
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
        let (value, origin) = resolve(s.key, frontier);
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
        let (v, o) = resolve_with("publish.git_push", None, |_| None, Some(user.clone()));
        assert_eq!((v.as_str(), o), ("off", Origin::User));
    }

    #[test]
    fn frontier_scope_is_protected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let frontier = tmp.path().join("f");
        std::fs::create_dir_all(frontier.join(".vela")).unwrap();
        // Routing keys refuse frontier scope outright; narrowing keys
        // refuse widening values.
        assert!(set("hub.url", "https://evil.example", Some(&frontier)).is_err());
        assert!(set("publish.git_push", "auto", Some(&frontier)).is_err());
        set("publish.git_push", "off", Some(&frontier)).unwrap();
        let (v, o) = resolve_with("publish.git_push", Some(&frontier), |_| None, None);
        assert_eq!((v.as_str(), o), ("off", Origin::Frontier));
        // A hand-written hub.url in frontier config is IGNORED even
        // though the file contains it.
        std::fs::write(
            frontier.join(".vela/config.toml"),
            "[hub]\nurl = \"https://evil.example\"\n[publish]\ngit_push = \"off\"\n",
        )
        .unwrap();
        let (v, o) = resolve_with("hub.url", Some(&frontier), |_| None, None);
        assert_ne!(v, "https://evil.example");
        assert_ne!(o, Origin::Frontier);
    }

    #[test]
    fn env_beats_files_and_default_falls_through() {
        let (v, o) = resolve_with(
            "ui.color",
            None,
            |name| (name == "VELA_UI_COLOR").then(|| "never".to_string()),
            None,
        );
        assert_eq!((v.as_str(), o), ("never", Origin::Env));
        let (v, o) = resolve_with("ui.color", None, |_| None, None);
        assert_eq!((v.as_str(), o), ("auto", Origin::Default));
    }
}
