//! Phase S (v0.5): registry primitive — verifiable distribution.
//!
//! A registry is a directory of `RegistryEntry`s, each one a signed
//! manifest pointing at a frontier publication. Pulling a frontier
//! through a registry verifies:
//!
//! 1. The manifest signature was produced by the owner's pubkey.
//! 2. The pulled frontier's snapshot_hash matches the registered value.
//! 3. The pulled frontier's event_log_hash matches the registered value.
//!
//! Cross-frontier *links* (`vf_…@vfr_…` references) are deferred to
//! v0.6. v0.5's registry is the npm-tarball-with-a-signature shape:
//! archival, reproducibility, integrity-checked transfer between
//! collaborating institutions.
//!
//! A registry is NOT a Vela frontier (deferred to v0.6 once
//! cross-frontier semantics exist). For now it's a flat
//! `entries.json` + `pubkeys.json` pair on disk or fetched over HTTP.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

pub const REGISTRY_SCHEMA: &str = "vela.registry.v0.1";
pub const ENTRY_SCHEMA: &str = "vela.registry-entry.v0.1";

/// A single signed publication of a frontier into a registry. The
/// `signature` is Ed25519 over the canonical preimage of the entry's
/// fields *minus* the signature itself. Two implementations agree on
/// the signing-bytes derivation by following the same canonical-JSON
/// rule already used for `vev_…`/`vpr_…`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryEntry {
    #[serde(default = "default_entry_schema")]
    pub schema: String,
    pub vfr_id: String,
    pub name: String,
    pub owner_actor_id: String,
    /// Hex-encoded Ed25519 public key (64 hex chars).
    pub owner_pubkey: String,
    /// SHA-256 hex of the canonical snapshot at publication time.
    pub latest_snapshot_hash: String,
    /// SHA-256 hex of the canonical event log at publication time.
    pub latest_event_log_hash: String,
    /// Where to fetch the frontier from (`file://`, `http://`, or
    /// `git+...`). v0.5 supports `file://` and bare paths; HTTP and git
    /// transports are scaffolded but unimplemented (v0.6).
    pub network_locator: String,
    /// RFC3339 timestamp of when the entry was signed.
    pub signed_publish_at: String,
    /// Hex-encoded Ed25519 signature over the canonical preimage of
    /// the entry's other fields.
    pub signature: String,
}

fn default_entry_schema() -> String {
    ENTRY_SCHEMA.to_string()
}

/// On-disk registry shape: a JSON file containing the schema marker
/// and an array of entries. Multiple publications of the same `vfr_id`
/// are appended; readers select the latest by `signed_publish_at`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "default_registry_schema")]
    pub schema: String,
    #[serde(default)]
    pub entries: Vec<RegistryEntry>,
}

fn default_registry_schema() -> String {
    REGISTRY_SCHEMA.to_string()
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            schema: REGISTRY_SCHEMA.to_string(),
            entries: Vec::new(),
        }
    }
}

/// Build the canonical preimage for an entry's signature.
///
/// Excludes the `signature` field itself. Same canonical-JSON rule as
/// `event_signing_bytes` and `proposal_signing_bytes`: a second
/// implementation following only the canonical-JSON spec produces
/// byte-identical signing bytes.
pub fn entry_signing_bytes(entry: &RegistryEntry) -> Result<Vec<u8>, String> {
    let preimage = json!({
        "schema": entry.schema,
        "vfr_id": entry.vfr_id,
        "name": entry.name,
        "owner_actor_id": entry.owner_actor_id,
        "owner_pubkey": entry.owner_pubkey,
        "latest_snapshot_hash": entry.latest_snapshot_hash,
        "latest_event_log_hash": entry.latest_event_log_hash,
        "network_locator": entry.network_locator,
        "signed_publish_at": entry.signed_publish_at,
    });
    crate::canonical::to_canonical_bytes(&preimage)
}

/// Sign an unsigned entry (with `signature` as empty string), returning
/// a hex-encoded Ed25519 signature.
pub fn sign_entry(
    entry: &RegistryEntry,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<String, String> {
    use ed25519_dalek::Signer;
    let bytes = entry_signing_bytes(entry)?;
    Ok(hex::encode(signing_key.sign(&bytes).to_bytes()))
}

/// Verify an entry's `signature` against `owner_pubkey`.
pub fn verify_entry(entry: &RegistryEntry) -> Result<bool, String> {
    let bytes = entry_signing_bytes(entry)?;
    crate::sign::verify_action_signature(&bytes, &entry.signature, &entry.owner_pubkey)
}

// ── Local file-backed registry ───────────────────────────────────────

/// Load a registry from a local file (JSON). Returns an empty registry
/// if the file does not exist.
pub fn load_local(path: &Path) -> Result<Registry, String> {
    if !path.exists() {
        return Ok(Registry::default());
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read registry {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse registry {}: {e}", path.display()))
}

pub fn save_local(path: &Path, registry: &Registry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(registry).map_err(|e| format!("serialize registry: {e}"))?;
    std::fs::write(path, raw).map_err(|e| format!("write registry {}: {e}", path.display()))?;
    Ok(())
}

/// Resolve a registry URL/path into a local *write* path. Used by
/// `vela registry publish` which can only target a local file.
/// v0.6 supports:
///   - bare path: `/path/to/registry.json`
///   - `file://` URL
///   - directory: appends `entries.json`
///
/// HTTP and git transports are rejected here (publish-side only); for
/// read-side fetches use [`load_any`] which handles HTTP.
pub fn resolve_local(locator: &str) -> Result<PathBuf, String> {
    if locator.starts_with("http://") || locator.starts_with("https://") {
        return Err(
            "HTTP transport for registry write (publish) is deferred to v0.8; for reads, use https:// with `vela registry list/pull`."
                .to_string(),
        );
    }
    if locator.starts_with("git+") {
        return Err("Git transport for registries is deferred to v0.8".to_string());
    }
    let stripped = locator.strip_prefix("file://").unwrap_or(locator);
    let path = PathBuf::from(stripped);
    if path.is_dir() {
        Ok(path.join("entries.json"))
    } else {
        Ok(path)
    }
}

/// Fetch a registry from anywhere it might live. v0.7 (this phase):
///   - bare path / `file://` — local file (delegates to `load_local`)
///   - `https://…/entries.json` — fetched via blocking HTTP, parsed
///     identically to a local file
///   - `https://…/` (no `entries.json` suffix) — appended automatically
///
/// HTTP fetch returns the same `Registry` shape; the hub serves the
/// canonical-JSON manifest verbatim, so signature verification works
/// byte-for-byte without re-canonicalization.
pub fn load_any(locator: &str) -> Result<Registry, String> {
    if locator.starts_with("http://") || locator.starts_with("https://") {
        // Convention: a URL whose path ends in `/` is treated as a hub
        // root; we append `entries`. Anything else (including `/entries`,
        // `/entries.json`, or paths with query strings) is used verbatim.
        let url = if locator.ends_with('/') {
            format!("{locator}entries")
        } else {
            locator.to_string()
        };
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("vela/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("build http client: {e}"))?;
        let resp = client
            .get(&url)
            .send()
            .map_err(|e| format!("GET {url}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", resp.status()));
        }
        let text = resp
            .text()
            .map_err(|e| format!("read response body: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parse remote registry {url}: {e}"))
    } else {
        let path = resolve_local(locator)?;
        load_local(&path)
    }
}

/// Fetch a frontier file from its locator (the `network_locator` field
/// on a registry entry) into a local destination path. Supports
/// `file://`, bare paths, and `https://`. Returns the destination path
/// on success.
pub fn fetch_frontier_to(locator: &str, dest: &Path) -> Result<(), String> {
    if locator.starts_with("http://") || locator.starts_with("https://") {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("vela/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| format!("build http client: {e}"))?;
        let resp = client
            .get(locator)
            .send()
            .map_err(|e| format!("GET {locator}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("GET {locator}: HTTP {}", resp.status()));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| format!("read frontier bytes: {e}"))?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        std::fs::write(dest, &bytes).map_err(|e| format!("write {}: {e}", dest.display()))?;
        Ok(())
    } else {
        let stripped = locator.strip_prefix("file://").unwrap_or(locator);
        let source = PathBuf::from(stripped);
        std::fs::copy(&source, dest)
            .map(|_| ())
            .map_err(|e| format!("copy {} → {}: {e}", source.display(), dest.display()))
    }
}

/// Server response shape from `POST <hub>/entries`.
#[derive(Debug, Clone, Deserialize)]
pub struct PublishResponse {
    pub ok: bool,
    #[serde(default)]
    pub duplicate: bool,
    #[serde(default)]
    pub vfr_id: String,
    #[serde(default)]
    pub signed_publish_at: String,
}

/// Push a signed entry to a remote hub. The transport is doctrine-light:
/// canonical JSON over HTTPS POST. The hub verifies the signature and
/// stores the bytes verbatim.
///
/// `hub_url` may be either the hub root (`https://vela-hub.fly.dev`) or
/// the entries endpoint (`https://vela-hub.fly.dev/entries`); we append
/// `/entries` if missing.
pub fn publish_remote(entry: &RegistryEntry, hub_url: &str) -> Result<PublishResponse, String> {
    if !hub_url.starts_with("http://") && !hub_url.starts_with("https://") {
        return Err(format!(
            "publish_remote requires http:// or https:// URL, got: {hub_url}"
        ));
    }
    let trimmed = hub_url.trim_end_matches('/');
    let url = if trimmed.ends_with("/entries") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/entries")
    };
    let bytes = crate::canonical::to_canonical_bytes(entry)
        .map_err(|e| format!("canonicalize entry: {e}"))?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("vela/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;
    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .body(bytes)
        .send()
        .map_err(|e| format!("POST {url}: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("read response body: {e}"))?;
    if !status.is_success() {
        // Try to extract a server-supplied message; otherwise surface the body.
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or(text);
        return Err(format!("POST {url}: HTTP {status}: {msg}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("parse publish response: {e}"))
}

/// Append a signed entry to a registry, replacing any prior entry
/// for the same `vfr_id` (latest-publish-wins).
///
/// Verifies the entry's signature against its declared `owner_pubkey`
/// before persisting; refuses to register an entry that fails
/// verification (callers must sign first).
pub fn publish_entry(registry_path: &Path, entry: RegistryEntry) -> Result<(), String> {
    if !verify_entry(&entry)? {
        return Err("registry entry signature does not verify".to_string());
    }
    let mut registry = load_local(registry_path)?;
    registry
        .entries
        .retain(|existing| existing.vfr_id != entry.vfr_id);
    registry.entries.push(entry);
    save_local(registry_path, &registry)
}

/// Find the latest entry for `vfr_id` in a local registry, by
/// `signed_publish_at`. Returns None if no entry exists.
pub fn find_latest(registry: &Registry, vfr_id: &str) -> Option<RegistryEntry> {
    registry
        .entries
        .iter()
        .filter(|entry| entry.vfr_id == vfr_id)
        .max_by_key(|entry| entry.signed_publish_at.clone())
        .cloned()
}

/// Pull verification: given a registry entry and the path to a
/// pulled-frontier file on disk, verify that:
///
/// 1. The entry's signature verifies against its declared pubkey.
/// 2. The frontier's `snapshot_hash` matches the entry's
///    `latest_snapshot_hash`.
/// 3. The frontier's `event_log_hash` matches the entry's
///    `latest_event_log_hash`.
///
/// Returns Ok(()) if all three hold; Err(reason) on any mismatch.
pub fn verify_pull(entry: &RegistryEntry, frontier_path: &Path) -> Result<(), String> {
    if !verify_entry(entry)? {
        return Err("registry entry signature does not verify".to_string());
    }
    let frontier = crate::repo::load_from_path(frontier_path)
        .map_err(|e| format!("load frontier {}: {e}", frontier_path.display()))?;
    let snapshot = crate::events::snapshot_hash(&frontier);
    if snapshot != entry.latest_snapshot_hash {
        return Err(format!(
            "snapshot_hash mismatch: registry={}, frontier={}",
            entry.latest_snapshot_hash, snapshot
        ));
    }
    let event_log = crate::events::event_log_hash(&frontier.events);
    if event_log != entry.latest_event_log_hash {
        return Err(format!(
            "event_log_hash mismatch: registry={}, frontier={}",
            entry.latest_event_log_hash, event_log
        ));
    }
    Ok(())
}

// ── v0.8: transitive pull-and-verify ─────────────────────────────────

/// Outcome of `pull_transitive`. The primary frontier and every
/// recursively-resolved cross-frontier dependency end up as files on
/// disk; `verified` lists the `vfr_id`s whose snapshot pin matched.
#[derive(Debug, Clone)]
pub struct PullResult {
    /// Path to the primary frontier file.
    pub primary_path: std::path::PathBuf,
    /// `vfr_id` → on-disk path for every dependency successfully pulled.
    pub deps: std::collections::HashMap<String, std::path::PathBuf>,
    /// Order in which `vfr_id`s were verified (primary first, then deps
    /// in walk order). Useful for stable output / logging.
    pub verified: Vec<String>,
}

/// Pull a frontier and recursively pull every cross-frontier
/// dependency it declares, verifying each pinned snapshot hash along
/// the way. The primary's manifest must live in `registry`.
///
/// Doctrine notes:
/// - Verification is total: any snapshot mismatch, missing locator, or
///   missing dep-manifest aborts the whole pull. Partial trust is not
///   a state v0.8 supports.
/// - Cycles are impossible by content-addressing (a vfr_id is a hash
///   of bytes that include the dependency list). A visited-set guards
///   anyway; revisiting the same vfr_id is a no-op.
/// - `max_depth` caps recursion. The primary is depth 0; its direct
///   deps are depth 1, and so on. Reaching `max_depth` without
///   exhausting deps is an error so the caller can decide to retry
///   with a higher cap.
pub fn pull_transitive(
    registry: &Registry,
    primary_vfr: &str,
    out_dir: &Path,
    max_depth: usize,
) -> Result<PullResult, String> {
    use std::collections::{HashMap, HashSet, VecDeque};

    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    // Look up the primary entry, fetch the frontier, verify total.
    let primary_entry = find_latest(registry, primary_vfr)
        .ok_or_else(|| format!("primary {primary_vfr} not found in registry"))?;
    let primary_path = out_dir.join(format!("{primary_vfr}.json"));
    fetch_frontier_to(&primary_entry.network_locator, &primary_path)
        .map_err(|e| format!("fetch primary {primary_vfr}: {e}"))?;
    verify_pull(&primary_entry, &primary_path)
        .map_err(|e| format!("verify primary {primary_vfr}: {e}"))?;

    let mut deps: HashMap<String, std::path::PathBuf> = HashMap::new();
    let mut verified: Vec<String> = vec![primary_vfr.to_string()];
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(primary_vfr.to_string());

    // BFS: each queue entry carries (vfr_id, frontier_path, depth).
    let mut queue: VecDeque<(String, std::path::PathBuf, usize)> = VecDeque::new();
    queue.push_back((primary_vfr.to_string(), primary_path.clone(), 0));

    while let Some((cur_vfr, cur_path, depth)) = queue.pop_front() {
        let frontier =
            crate::repo::load_from_path(&cur_path).map_err(|e| format!("reload {cur_vfr}: {e}"))?;

        for dep in frontier.cross_frontier_deps() {
            let Some(dep_vfr) = dep.vfr_id.clone() else {
                continue;
            };
            if visited.contains(&dep_vfr) {
                continue; // already pulled (deduped + cycle-safe)
            }
            if depth + 1 > max_depth {
                return Err(format!(
                    "transitive pull exceeded max depth {max_depth} at {dep_vfr} (declared by {cur_vfr})"
                ));
            }
            let dep_locator = dep
                .locator
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    format!(
                        "cross-frontier dep {dep_vfr} (declared by {cur_vfr}) has no locator; cannot fetch"
                    )
                })?;
            let dep_pinned = dep
                .pinned_snapshot_hash
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    format!(
                        "cross-frontier dep {dep_vfr} (declared by {cur_vfr}) has no pinned_snapshot_hash; cannot verify"
                    )
                })?;

            // Manifest must live in this same registry. (Hub-to-hub
            // federation — pulling deps from a different registry — is
            // deferred to v0.9.)
            let dep_entry = find_latest(registry, &dep_vfr).ok_or_else(|| {
                format!(
                    "cross-frontier dep {dep_vfr} (declared by {cur_vfr}) not present in registry"
                )
            })?;

            let dep_path = out_dir.join(format!("{dep_vfr}.json"));
            fetch_frontier_to(dep_locator, &dep_path)
                .map_err(|e| format!("fetch dep {dep_vfr}: {e}"))?;
            verify_pull(&dep_entry, &dep_path).map_err(|e| format!("verify dep {dep_vfr}: {e}"))?;

            // Heart of the pin: compare the *dependent's* declared
            // pinned snapshot to the *dependency's* actual snapshot.
            // The dep's manifest snapshot is what `verify_pull` already
            // checked against the file; equality there means the file's
            // canonical snapshot equals `dep_entry.latest_snapshot_hash`.
            // So the pin check is just: dep_pinned == dep_entry's hash.
            if dep_pinned != dep_entry.latest_snapshot_hash {
                return Err(format!(
                    "pinned_snapshot_hash mismatch for {dep_vfr}: dependent {cur_vfr} pinned {dep_pinned}, registry has {actual}",
                    actual = dep_entry.latest_snapshot_hash
                ));
            }

            visited.insert(dep_vfr.clone());
            verified.push(dep_vfr.clone());
            deps.insert(dep_vfr.clone(), dep_path.clone());
            queue.push_back((dep_vfr, dep_path, depth + 1));
        }
    }

    Ok(PullResult {
        primary_path,
        deps,
        verified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use tempfile::TempDir;

    fn keypair() -> (SigningKey, String) {
        let key = SigningKey::generate(&mut OsRng);
        let pubkey = hex::encode(key.verifying_key().to_bytes());
        (key, pubkey)
    }

    fn sample_entry(pubkey: &str) -> RegistryEntry {
        RegistryEntry {
            schema: ENTRY_SCHEMA.to_string(),
            vfr_id: "vfr_aaaaaaaaaaaaaaaa".to_string(),
            name: "Test Frontier".to_string(),
            owner_actor_id: "reviewer:test".to_string(),
            owner_pubkey: pubkey.to_string(),
            latest_snapshot_hash: "a".repeat(64),
            latest_event_log_hash: "b".repeat(64),
            network_locator: "/tmp/x.json".to_string(),
            signed_publish_at: "2026-04-25T00:00:00Z".to_string(),
            signature: String::new(),
        }
    }

    #[test]
    fn entry_sign_and_verify_round_trip() {
        let (key, pubkey) = keypair();
        let mut entry = sample_entry(&pubkey);
        entry.signature = sign_entry(&entry, &key).unwrap();
        assert!(verify_entry(&entry).unwrap(), "entry must self-verify");
    }

    #[test]
    fn tampered_entry_fails_verification() {
        let (key, pubkey) = keypair();
        let mut entry = sample_entry(&pubkey);
        entry.signature = sign_entry(&entry, &key).unwrap();
        entry.latest_snapshot_hash = "f".repeat(64);
        assert!(
            !verify_entry(&entry).unwrap(),
            "tampered entry must fail to verify"
        );
    }

    #[test]
    fn publish_entry_replaces_prior_for_same_vfr_id() {
        let (key, pubkey) = keypair();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("entries.json");
        let mut entry = sample_entry(&pubkey);
        entry.signature = sign_entry(&entry, &key).unwrap();
        publish_entry(&path, entry.clone()).unwrap();

        // Re-publish with newer timestamp + new snapshot.
        let mut entry2 = entry.clone();
        entry2.latest_snapshot_hash = "c".repeat(64);
        entry2.signed_publish_at = "2026-04-26T00:00:00Z".to_string();
        entry2.signature = sign_entry(&entry2, &key).unwrap();
        publish_entry(&path, entry2.clone()).unwrap();

        let registry = load_local(&path).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(
            registry.entries[0].latest_snapshot_hash,
            entry2.latest_snapshot_hash
        );
        let latest = find_latest(&registry, &entry.vfr_id).unwrap();
        assert_eq!(latest.signed_publish_at, "2026-04-26T00:00:00Z");
    }

    #[test]
    fn publish_rejects_unsigned_entry() {
        let (_key, pubkey) = keypair();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("entries.json");
        let entry = sample_entry(&pubkey); // signature is empty
        let result = publish_entry(&path, entry);
        assert!(result.is_err(), "unsigned entry must be rejected");
    }

    // ── v0.8: transitive pull-and-verify ──────────────────────────────

    /// Build a frontier file at `path`, return its (vfr_id,
    /// snapshot_hash, event_log_hash). Uses `project::assemble` to make
    /// a real frontier (real frontier_id, real hashes), so the
    /// resulting RegistryEntry is verifiable end-to-end.
    fn make_real_frontier(
        dir: &Path,
        name: &str,
        seed: &str,
        deps: Vec<crate::project::ProjectDependency>,
    ) -> (std::path::PathBuf, String, String, String) {
        use crate::bundle::{
            Assertion, Conditions, Confidence, ConfidenceMethod, Evidence, Extraction,
            FindingBundle, Flags, Provenance,
        };
        let assertion = Assertion {
            text: format!("Test assertion {seed}"),
            assertion_type: "mechanism".into(),
            entities: vec![],
            relation: None,
            direction: None,
            causal_claim: None,
            causal_evidence_grade: None,
        };
        let provenance = Provenance {
            source_type: "published_paper".into(),
            doi: Some(format!("10.0000/{seed}")),
            pmid: None,
            pmc: None,
            openalex_id: None,
            url: None,
            title: format!("Test {seed}"),
            authors: vec![],
            year: Some(2024),
            journal: None,
            license: None,
            publisher: None,
            funders: vec![],
            citation_count: None,
            extraction: Extraction {
                method: "llm_extraction".into(),
                model: None,
                model_version: None,
                extracted_at: "1970-01-01T00:00:00Z".into(),
                extractor_version: "vela/0.2.0".into(),
            },
            review: None,
        };
        let id = FindingBundle::content_address(&assertion, &provenance);
        let finding = FindingBundle {
            id,
            version: 1,
            previous_version: None,
            assertion,
            evidence: Evidence {
                evidence_type: "experimental".into(),
                model_system: String::new(),
                species: None,
                method: String::new(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: vec![],
            },
            conditions: Conditions {
                text: String::new(),
                species_verified: vec![],
                species_unverified: vec![],
                in_vitro: false,
                in_vivo: false,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            confidence: Confidence {
                kind: Default::default(),
                score: 0.5,
                basis: "test".into(),
                method: ConfidenceMethod::LlmInitial,
                components: None,
                extraction_confidence: 0.5,
            },
            provenance,
            flags: Flags {
                gap: false,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
                superseded: false,
            signature_threshold: None,
            jointly_accepted: false,
        },
            links: vec![],
            annotations: vec![],
            attachments: vec![],
            created: chrono::Utc::now().to_rfc3339(),
            updated: None,
        };
        let mut p = crate::project::assemble(name, vec![finding], 1, 0, "Test");
        p.project.dependencies = deps;
        let path = dir.join(format!("{name}.json"));
        let json = serde_json::to_string_pretty(&p).unwrap();
        std::fs::write(&path, json).unwrap();
        let vfr_id = p.frontier_id();
        let snapshot = crate::events::snapshot_hash(&p);
        let event_log = crate::events::event_log_hash(&p.events);
        (path, vfr_id, snapshot, event_log)
    }

    fn signed_entry(
        key: &SigningKey,
        pubkey: &str,
        vfr_id: &str,
        name: &str,
        path: &Path,
        snapshot: &str,
        event_log: &str,
    ) -> RegistryEntry {
        let mut entry = RegistryEntry {
            schema: ENTRY_SCHEMA.to_string(),
            vfr_id: vfr_id.to_string(),
            name: name.to_string(),
            owner_actor_id: "reviewer:test".to_string(),
            owner_pubkey: pubkey.to_string(),
            latest_snapshot_hash: snapshot.to_string(),
            latest_event_log_hash: event_log.to_string(),
            network_locator: format!("file://{}", path.display()),
            signed_publish_at: chrono::Utc::now().to_rfc3339(),
            signature: String::new(),
        };
        entry.signature = sign_entry(&entry, key).unwrap();
        entry
    }

    #[test]
    fn pull_transitive_resolves_one_level() {
        let (key, pubkey) = keypair();
        let tmp = TempDir::new().unwrap();
        let stage = tmp.path().join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        let out = tmp.path().join("out");

        // Frontier A — leaf, no deps.
        let (a_path, a_vfr, a_snap, a_eventlog) =
            make_real_frontier(&stage, "frontier-a", "aaa", vec![]);
        // Frontier B declares A as a dep with the right snapshot pin.
        let (b_path, b_vfr, b_snap, b_eventlog) = make_real_frontier(
            &stage,
            "frontier-b",
            "bbb",
            vec![crate::project::ProjectDependency {
                name: "frontier-a".into(),
                source: "vela.hub".into(),
                version: None,
                pinned_hash: None,
                vfr_id: Some(a_vfr.clone()),
                locator: Some(format!("file://{}", a_path.display())),
                pinned_snapshot_hash: Some(a_snap.clone()),
            }],
        );

        let mut registry = Registry::default();
        registry.entries.push(signed_entry(
            &key,
            &pubkey,
            &a_vfr,
            "frontier-a",
            &a_path,
            &a_snap,
            &a_eventlog,
        ));
        registry.entries.push(signed_entry(
            &key,
            &pubkey,
            &b_vfr,
            "frontier-b",
            &b_path,
            &b_snap,
            &b_eventlog,
        ));

        let result = pull_transitive(&registry, &b_vfr, &out, 4).unwrap();
        assert_eq!(result.verified.len(), 2, "both frontiers verified");
        assert!(result.verified.contains(&b_vfr));
        assert!(result.verified.contains(&a_vfr));
        assert!(result.deps.contains_key(&a_vfr));
        assert!(out.join(format!("{b_vfr}.json")).exists());
        assert!(out.join(format!("{a_vfr}.json")).exists());
    }

    #[test]
    fn pull_transitive_fails_on_pin_mismatch() {
        let (key, pubkey) = keypair();
        let tmp = TempDir::new().unwrap();
        let stage = tmp.path().join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        let out = tmp.path().join("out");

        let (a_path, a_vfr, a_snap, a_eventlog) =
            make_real_frontier(&stage, "frontier-a", "aaa", vec![]);
        // B pins a *different* snapshot than the registry has for A.
        let bad_pin = "f".repeat(64);
        let (b_path, b_vfr, b_snap, b_eventlog) = make_real_frontier(
            &stage,
            "frontier-b",
            "bbb",
            vec![crate::project::ProjectDependency {
                name: "frontier-a".into(),
                source: "vela.hub".into(),
                version: None,
                pinned_hash: None,
                vfr_id: Some(a_vfr.clone()),
                locator: Some(format!("file://{}", a_path.display())),
                pinned_snapshot_hash: Some(bad_pin),
            }],
        );

        let mut registry = Registry::default();
        registry.entries.push(signed_entry(
            &key,
            &pubkey,
            &a_vfr,
            "frontier-a",
            &a_path,
            &a_snap,
            &a_eventlog,
        ));
        registry.entries.push(signed_entry(
            &key,
            &pubkey,
            &b_vfr,
            "frontier-b",
            &b_path,
            &b_snap,
            &b_eventlog,
        ));

        let err = pull_transitive(&registry, &b_vfr, &out, 4).unwrap_err();
        assert!(
            err.contains("pinned_snapshot_hash mismatch"),
            "expected pin-mismatch error, got: {err}"
        );
    }

    #[test]
    fn pull_transitive_errors_when_dep_missing_from_registry() {
        let (key, pubkey) = keypair();
        let tmp = TempDir::new().unwrap();
        let stage = tmp.path().join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        let out = tmp.path().join("out");

        let (a_path, a_vfr, a_snap, _a_eventlog) =
            make_real_frontier(&stage, "frontier-a", "aaa", vec![]);
        let (b_path, b_vfr, b_snap, b_eventlog) = make_real_frontier(
            &stage,
            "frontier-b",
            "bbb",
            vec![crate::project::ProjectDependency {
                name: "frontier-a".into(),
                source: "vela.hub".into(),
                version: None,
                pinned_hash: None,
                vfr_id: Some(a_vfr.clone()),
                locator: Some(format!("file://{}", a_path.display())),
                pinned_snapshot_hash: Some(a_snap),
            }],
        );

        // Registry only has B; A is not registered.
        let mut registry = Registry::default();
        registry.entries.push(signed_entry(
            &key,
            &pubkey,
            &b_vfr,
            "frontier-b",
            &b_path,
            &b_snap,
            &b_eventlog,
        ));

        let err = pull_transitive(&registry, &b_vfr, &out, 4).unwrap_err();
        assert!(err.contains("not present in registry"));
    }
}
