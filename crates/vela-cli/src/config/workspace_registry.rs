//! The workspace registry: `~/.vela/frontiers.json`, the list of
//! frontiers on this machine. It exists for ONE ritual: `vela sign`
//! outside any frontier walks every registered frontier and builds one
//! aggregated queue — one session, one key read, all frontiers.
//! Registration is best-effort convenience state (a missing or stale
//! entry breaks nothing; the frontier itself is always the truth).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRegistry {
    #[serde(default)]
    pub frontiers: Vec<RegisteredFrontier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredFrontier {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub registered_at: String,
}

fn registry_path() -> Option<PathBuf> {
    dirs_home().map(|h| h.join(".vela").join("frontiers.json"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn load() -> WorkspaceRegistry {
    let Some(path) = registry_path() else {
        return WorkspaceRegistry::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Register a frontier (idempotent on canonical path). Called by init
/// and by any verb that touches a frontier not yet listed — silent
/// best-effort: registry failure never fails the verb.
pub fn register(frontier_dir: &Path, name: Option<&str>) {
    let Some(reg_path) = registry_path() else {
        return;
    };
    let canonical = frontier_dir
        .canonicalize()
        .unwrap_or_else(|_| frontier_dir.to_path_buf());
    let mut reg = load();
    if let Some(existing) = reg
        .frontiers
        .iter_mut()
        .find(|f| Path::new(&f.path) == canonical)
    {
        if existing.name.is_none() && name.is_some() {
            existing.name = name.map(str::to_string);
        }
    } else {
        reg.frontiers.push(RegisteredFrontier {
            path: canonical.display().to_string(),
            name: name.map(str::to_string),
            registered_at: chrono::Utc::now().to_rfc3339(),
        });
    }
    let _ = std::fs::create_dir_all(reg_path.parent().unwrap());
    if let Ok(body) = serde_json::to_string_pretty(&reg) {
        let _ = std::fs::write(reg_path, format!("{body}\n"));
    }
}

/// Registered frontiers that still exist on disk and still look like
/// frontiers (stale rows are skipped, not errors).
#[allow(dead_code)] // the cross-frontier `vela sign` walk (W3); built ahead of its consumer
pub fn live_frontiers() -> Vec<PathBuf> {
    load()
        .frontiers
        .iter()
        .map(|f| PathBuf::from(&f.path))
        .filter(|p| p.join(".vela").is_dir())
        .collect()
}
