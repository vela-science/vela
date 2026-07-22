//! Git ingestion: the hub as an index over git-replayed state (ADR 0001,
//! docs/HUB.md). For each repository in the operator's versioned source
//! catalog, the ingestor fetches the repo, replays the committed `.vela/events` log with
//! the protocol library, holds it to the one canonical strict bar
//! (`vela_edge::verify::verify_frontier_strict`), and promotes the result
//! into the read projection (`HubDb::promote_frontier_snapshot`).
//!
//! Authority model, stated plainly: the projection carries no owner-signed
//! publication manifest. Its authority is the repo's signed events, verified
//! on replay — the hub derives the index; it never owns the truth. Source
//! selection is explicit operator configuration and carries no scientific
//! authority.
//!
//! Anti-replay: after the first indexed commit, a new tip must be its Git
//! descendant. A rewritten or rewound source is refused even if its timestamp
//! is newer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::db::{HubDb, VerifiedFrontierIndex};
use vela_protocol::events::{event_log_hash, snapshot_hash};

/// Authority mode recorded on frontiers whose index rows derive from a
/// verified Git remote rather than an HTTP-delivered manifest.
pub const AUTHORITY_GIT_INGESTED: &str = "git_ingested";
pub const SOURCE_CATALOG_SCHEMA: &str = "vela.hub-source-catalog.v1";

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GitSource {
    pub vfr_id: String,
    pub git_remote: String,
    #[serde(default = "default_git_ref")]
    pub git_ref: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SourceCatalog {
    pub schema: String,
    pub sources: Vec<GitSource>,
}

fn default_git_ref() -> String {
    "main".to_string()
}

/// Collapse harmless clone-URL spelling differences in the operator catalog.
pub(crate) fn canonical_git_remote(remote: &str) -> String {
    let mut value = remote.trim();
    value = value.strip_suffix('/').unwrap_or(value);
    value = value.strip_suffix(".git").unwrap_or(value);
    value = value.strip_suffix('/').unwrap_or(value);
    value.to_string()
}

/// Load the versioned operator source catalog. Deployments may point at a
/// different checked-in catalog with `VELA_HUB_SOURCES_FILE`; the bundled
/// public catalog is the default.
pub fn load_source_catalog() -> Result<SourceCatalog, String> {
    let raw = match std::env::var("VELA_HUB_SOURCES_FILE") {
        Ok(path) => std::fs::read_to_string(&path)
            .map_err(|e| format!("read VELA_HUB_SOURCES_FILE {path}: {e}"))?,
        Err(_) => include_str!("../sources.json").to_string(),
    };
    parse_source_catalog(&raw)
}

fn parse_source_catalog(raw: &str) -> Result<SourceCatalog, String> {
    let mut catalog: SourceCatalog =
        serde_json::from_str(raw).map_err(|e| format!("parse Hub source catalog: {e}"))?;
    if catalog.schema != SOURCE_CATALOG_SCHEMA {
        return Err(format!(
            "Hub source catalog schema must be {SOURCE_CATALOG_SCHEMA}, got {}",
            catalog.schema
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for source in &mut catalog.sources {
        let frontier_suffix = source.vfr_id.strip_prefix("vfr_");
        if !frontier_suffix.is_some_and(|suffix| {
            suffix.len() == 16
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) || !seen.insert(source.vfr_id.clone())
        {
            return Err(format!(
                "Hub source catalog has invalid or duplicate frontier id {}",
                source.vfr_id
            ));
        }
        source.git_remote = canonical_git_remote(&source.git_remote);
        let remote: axum::http::Uri = source
            .git_remote
            .parse()
            .map_err(|e| format!("Hub source {} has invalid Git remote: {e}", source.vfr_id))?;
        if remote.scheme_str() != Some("https")
            || remote.host().is_none()
            || remote
                .authority()
                .is_some_and(|authority| authority.as_str().contains('@'))
            || remote.query().is_some()
            || remote.path().is_empty()
            || remote.path() == "/"
        {
            return Err(format!(
                "Hub source {} must use a credential-free https Git remote without query or fragment",
                source.vfr_id
            ));
        }
        if source.git_ref.is_empty()
            || source.git_ref.starts_with('-')
            || source.git_ref.chars().any(char::is_whitespace)
            || !source
                .git_ref
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
            || source.git_ref.contains("..")
            || source.git_ref.contains("//")
            || source.git_ref.ends_with('.')
            || source.git_ref.ends_with('/')
        {
            return Err(format!(
                "Hub source {} has invalid Git ref {:?}",
                source.vfr_id, source.git_ref
            ));
        }
    }
    Ok(catalog)
}

pub struct GitIngestConfig {
    /// Seconds between ingest sweeps. 0 disables the loop.
    pub interval_secs: u64,
    /// Scratch directory for clones (persisted between ticks so ingests
    /// after the first are incremental fetches).
    pub scratch_dir: PathBuf,
}

impl GitIngestConfig {
    pub fn from_env() -> Self {
        let interval_secs = std::env::var("VELA_HUB_GIT_INGEST_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        let scratch_dir = std::env::var("VELA_HUB_GIT_INGEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("vela-hub-git-ingest"));
        Self {
            interval_secs,
            scratch_dir,
        }
    }
}

/// Spawn the recurring ingest loop (no-op when interval is 0).
pub fn spawn(db: HubDb, cfg: GitIngestConfig) {
    if cfg.interval_secs == 0 {
        eprintln!("git-ingest: disabled (VELA_HUB_GIT_INGEST_INTERVAL_SECS=0)");
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(cfg.interval_secs));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(err) = run_once(&db, &cfg).await {
                eprintln!("git-ingest: sweep error: {err}");
            }
        }
    });
}

/// One sweep over every registered target. Errors on one target are recorded
/// on its row and do not stop the sweep.
pub async fn run_once(db: &HubDb, cfg: &GitIngestConfig) -> Result<usize, String> {
    // One sweeper at a time: with more than one hub machine on the same
    // database, concurrent sweeps duplicate fetch work and race the
    // receipt insert. A session advisory lock elects a leader per sweep;
    // the loser skips this tick (the state converges next tick).
    let _guard = match db.try_ingest_lock().await? {
        Some(g) => Some(g),
        None => {
            return Ok(0);
        }
    };
    let targets = db.git_ingest_targets().await?;
    let mut ingested = 0;
    for (vfr_id, remote, git_ref, last_commit) in targets {
        match ingest_one(db, cfg, &vfr_id, &remote, &git_ref, last_commit.as_deref()).await {
            Ok(Some(commit)) => {
                db.record_git_ingest(&vfr_id, Some(&commit), None).await?;
                eprintln!("git-ingest: {vfr_id} promoted at {commit}");
                ingested += 1;
            }
            Ok(None) => {
                // up to date — touch the timestamp, keep the cursor
                db.record_git_ingest(&vfr_id, None, None).await?;
            }
            Err(err) => {
                eprintln!("git-ingest: {vfr_id}: {err}");
                db.record_git_ingest(&vfr_id, None, Some(&err)).await?;
            }
        }
    }
    Ok(ingested)
}

/// Ingest a single frontier. Returns Ok(Some(commit)) on promotion,
/// Ok(None) when already at the tip.
async fn ingest_one(
    db: &HubDb,
    cfg: &GitIngestConfig,
    vfr_id: &str,
    remote: &str,
    git_ref: &str,
    last_commit: Option<&str>,
) -> Result<Option<String>, String> {
    let dir = cfg.scratch_dir.join(vfr_id);
    fetch_repo(remote, git_ref, &dir).await?;
    let commit = rev_parse_head(&dir).await?;
    if Some(commit.as_str()) == last_commit {
        return Ok(None);
    }
    if let Some(previous) = last_commit
        && !is_ancestor(&dir, previous, &commit).await?
    {
        return Err(format!(
            "non-fast-forward Git update: previous indexed commit {previous} is not an ancestor of {commit}"
        ));
    }
    let commit_time = commit_timestamp(&dir).await?;

    // Replay + verify off the async runtime (the protocol code is sync).
    // The strict bar is defined ONCE, in `vela_edge::verify` — the same
    // bundle any indexer must hold a frontier to.
    let dir_cloned = dir.clone();
    let (project, fid) =
        tokio::task::spawn_blocking(move || vela_edge::verify::verify_frontier_strict(&dir_cloned))
            .await
            .map_err(|e| format!("verify task: {e}"))??;

    // The repo must BE the catalogued frontier: a remote that replays to a
    // different frontier_id is a source error (or a swap attack), not an
    // update.
    if fid != vfr_id {
        return Err(format!(
            "frontier_id mismatch: the repo replays to {fid}, catalog source is for {vfr_id}"
        ));
    }

    let owner_actor_id = project
        .events
        .iter()
        .find(|event| event.kind == "frontier.created")
        .or_else(|| project.events.first())
        .map(|event| event.actor.id.clone())
        .ok_or_else(|| "verified frontier has no genesis actor".to_string())?;
    let owner_pubkey = project
        .actors
        .iter()
        .find(|actor| actor.id == owner_actor_id)
        .map(|actor| actor.public_key.clone())
        .ok_or_else(|| format!("genesis actor {owner_actor_id} has no actor record"))?;

    // Internal projection cursor. The promoted state was verified
    // event-by-event above; no second manifest signature is created. The owner
    // fields carry the verified genesis identity for index display.
    let entry = VerifiedFrontierIndex {
        vfr_id: vfr_id.to_string(),
        name: project.project.name.clone(),
        owner_actor_id,
        owner_pubkey,
        latest_snapshot_hash: snapshot_hash(&project),
        latest_event_log_hash: event_log_hash(&project.events),
        source_commit_at: commit_time,
    };
    db.promote_frontier_snapshot(&entry, &project, AUTHORITY_GIT_INGESTED)
        .await?;
    Ok(Some(commit))
}

// ── git plumbing (process git: the borrow-logistics choice — git is the
//    transport everywhere else in the doctrine, so the ingestor speaks the
//    same tool rather than reimplementing it) ─────────────────────────────

async fn git(args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("git");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .args(args)
        .output()
        .await
        .map_err(|e| format!("git {:?}: {e}", args.first().unwrap_or(&"")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub(crate) async fn fetch_repo(remote: &str, git_ref: &str, dir: &Path) -> Result<(), String> {
    if dir.join(".git").exists() {
        // A catalog edit may have re-pointed the remote: the scratch
        // clone must always fetch the CURRENTLY configured URL, never a
        // stale origin.
        git(&["remote", "set-url", "origin", remote], Some(dir)).await?;
        if dir.join(".git/shallow").exists() {
            git(&["fetch", "--unshallow", "origin", git_ref], Some(dir)).await?;
        } else {
            git(&["fetch", "origin", git_ref], Some(dir)).await?;
        }
        git(&["reset", "--hard", "FETCH_HEAD"], Some(dir)).await?;
        git(&["clean", "-fdq"], Some(dir)).await?;
    } else {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("scratch dir: {e}"))?;
        }
        git(
            &["clone", "--branch", git_ref, remote, &dir.to_string_lossy()],
            None,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn rev_parse_head(dir: &Path) -> Result<String, String> {
    git(&["rev-parse", "HEAD"], Some(dir)).await
}

async fn is_ancestor(dir: &Path, previous: &str, candidate: &str) -> Result<bool, String> {
    let out = tokio::process::Command::new("git")
        .current_dir(dir)
        .args(["merge-base", "--is-ancestor", previous, candidate])
        .output()
        .await
        .map_err(|e| format!("git merge-base: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Committer timestamp of the tip, RFC3339. This is display/ordering metadata;
/// rollback protection uses Git ancestry, not timestamps.
async fn commit_timestamp(dir: &Path) -> Result<String, String> {
    git(&["show", "-s", "--format=%cI", "HEAD"], Some(dir)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_source_url_strips_trailing_git_and_slash() {
        let expected = "https://github.com/vela-science/sidon-frontier";
        assert_eq!(canonical_git_remote(expected), expected);
        assert_eq!(
            canonical_git_remote("https://github.com/vela-science/sidon-frontier.git/"),
            expected
        );
    }

    #[test]
    fn bundled_source_catalog_is_valid_and_complete() {
        let catalog = parse_source_catalog(include_str!("../sources.json")).expect("catalog");
        assert_eq!(catalog.schema, SOURCE_CATALOG_SCHEMA);
        assert_eq!(catalog.sources.len(), 4);
        assert!(
            catalog
                .sources
                .iter()
                .all(|source| source.git_ref == "main")
        );
        assert!(catalog.sources.iter().all(|source| {
            source
                .git_remote
                .starts_with("https://github.com/vela-science/")
        }));
    }

    #[test]
    fn source_catalog_rejects_path_ids_credentials_and_option_refs() {
        let invalid = [
            serde_json::json!({
                "schema": SOURCE_CATALOG_SCHEMA,
                "sources": [{
                    "vfr_id": "vfr_../../outside",
                    "git_remote": "https://github.com/example/frontier",
                    "git_ref": "main"
                }]
            }),
            serde_json::json!({
                "schema": SOURCE_CATALOG_SCHEMA,
                "sources": [{
                    "vfr_id": "vfr_001f148c07eebecb",
                    "git_remote": "https://token@github.com/example/frontier",
                    "git_ref": "main"
                }]
            }),
            serde_json::json!({
                "schema": SOURCE_CATALOG_SCHEMA,
                "sources": [{
                    "vfr_id": "vfr_001f148c07eebecb",
                    "git_remote": "https://github.com/example/frontier",
                    "git_ref": "--upload-pack=oops"
                }]
            }),
        ];
        for catalog in invalid {
            assert!(
                parse_source_catalog(&catalog.to_string()).is_err(),
                "{catalog}"
            );
        }
    }

    #[test]
    fn source_catalog_rejects_duplicate_frontier_ids() {
        let catalog = serde_json::json!({
            "schema": SOURCE_CATALOG_SCHEMA,
            "sources": [
                {
                    "vfr_id": "vfr_001f148c07eebecb",
                    "git_remote": "https://github.com/example/one",
                    "git_ref": "main"
                },
                {
                    "vfr_id": "vfr_001f148c07eebecb",
                    "git_remote": "https://github.com/example/two",
                    "git_ref": "main"
                }
            ]
        });
        assert!(parse_source_catalog(&catalog.to_string()).is_err());
    }

    #[tokio::test]
    async fn git_ancestry_accepts_fast_forward_and_rejects_reverse() {
        let repo = tempfile::TempDir::new().expect("repo");
        git(&["init"], Some(repo.path())).await.expect("git init");
        git(
            &["config", "user.email", "vela@example.test"],
            Some(repo.path()),
        )
        .await
        .expect("git email");
        git(&["config", "user.name", "Vela test"], Some(repo.path()))
            .await
            .expect("git name");
        std::fs::write(repo.path().join("state"), "one").expect("write first");
        git(&["add", "state"], Some(repo.path()))
            .await
            .expect("add first");
        git(&["commit", "-m", "first"], Some(repo.path()))
            .await
            .expect("commit first");
        let first = rev_parse_head(repo.path()).await.expect("first head");

        std::fs::write(repo.path().join("state"), "two").expect("write second");
        git(&["commit", "-am", "second"], Some(repo.path()))
            .await
            .expect("commit second");
        let second = rev_parse_head(repo.path()).await.expect("second head");

        assert!(is_ancestor(repo.path(), &first, &second).await.unwrap());
        assert!(!is_ancestor(repo.path(), &second, &first).await.unwrap());
    }

    fn copy_tree(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let to = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &to);
            } else {
                std::fs::copy(entry.path(), to).unwrap();
            }
        }
    }

    fn fixture_copy() -> tempfile::TempDir {
        // The in-repo example frontier is a real signed substrate — the same
        // fixture class the live hub ingests.
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/erdos-formalization");
        let tmp = tempfile::TempDir::new().unwrap();
        copy_tree(&src, tmp.path());
        tmp
    }

    #[test]
    fn verify_passes_on_clean_frontier() {
        let tmp = fixture_copy();
        let (project, _fid) = vela_edge::verify::verify_frontier_strict(tmp.path())
            .expect("clean frontier must verify");
        assert!(!project.events.is_empty());
    }

    #[test]
    fn verify_refuses_tampered_signed_event() {
        // Live red-test regression (2026-07-01): flipping a verdict inside a
        // SIGNED statement.attested event slipped past replay+signals alone;
        // the validation pass (content-address re-derivation) must refuse it.
        let tmp = fixture_copy();
        let events_dir = tmp.path().join(".vela/events");
        let mut tampered = false;
        for entry in std::fs::read_dir(&events_dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
            if v.get("kind").and_then(|k| k.as_str()) == Some("statement.attested")
                && v.get("signature").is_some_and(|s| !s.is_null())
            {
                let att = v
                    .pointer_mut("/payload/attestation/verdict")
                    .expect("attestation verdict");
                *att = serde_json::Value::String("faithful".into());
                std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
                tampered = true;
                break;
            }
        }
        assert!(
            tampered,
            "fixture must contain a signed statement.attested event"
        );
        let err = vela_edge::verify::verify_frontier_strict(tmp.path())
            .expect_err("tampered event must refuse");
        assert!(
            err.contains("validation failed") || err.contains("re-derive"),
            "expected an integrity refusal, got: {err}"
        );
    }
}
