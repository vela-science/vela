//! Read-only compatibility for the historical workspace registry at
//! `~/.vela/frontiers.json`. Legacy batch signing can still inspect existing
//! entries, but ordinary initialization no longer mutates host-global state.
//! A missing or stale registry never affects the Frontier itself.

use std::path::PathBuf;

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

/// Registered frontiers that still exist on disk and still look like
/// frontiers (stale rows are skipped, not errors).
pub fn live_frontiers() -> Vec<PathBuf> {
    load()
        .frontiers
        .iter()
        .map(|f| PathBuf::from(&f.path))
        .filter(|p| p.join(".vela").is_dir())
        .collect()
}
