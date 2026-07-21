//! The hosted MCP lane: `hub.constellate.science/mcp`.
//!
//! The hub embeds the `vela serve` dispatcher in-process (`McpService`
//! from vela-cli) and hydrates it from the hub's own database: every
//! live frontier's materialized projection — the same rows
//! `/entries/{vfr}/snapshot` serves — is loaded and merged into one
//! read-only service. There are no git checkouts in this lane; the
//! ingest sweep already strictly replays and promotes each registered
//! repo, and duplicating that fetch tree here bought nothing but
//! minutes of first-build latency. A fresh machine is ready in seconds.
//!
//! Freshness: the refresher lists the live entries and compares each
//! one's `latest_snapshot_hash` (from the verified frontier projection)
//! against the set the current service was built from; an unchanged set
//! is a no-op. It runs on an interval, and immediately when the GitHub
//! webhook kicks it — after the webhook's ingest sweep completes, so
//! the rebuild reads the state the push produced.
//!
//! Two consequences of hydrating from promoted state, stated plainly:
//! the projection serves ONLY strictly-verified promoted frontiers (the
//! old checkout loader was lenient and could merge in a frontier that
//! had never passed the strict bar), and a newly registered remote
//! appears once its first ingest sweep promotes it (≤ the sweep
//! interval, or the webhook), not on the refresher's own fetch.
//!
//! Custody note: the service is READ-ONLY profile with the hosted
//! exclusions (the filesystem-path `vela_*` runtime family). There is no
//! configuration in which this endpoint mutates state; the hub stays an
//! index, and decisions stay key-custody human acts in the repos.
//!
//! Every machine runs its own refresher (unlike the DB ingest sweep,
//! which elects a leader): the merged projection is per-machine memory,
//! so each machine must maintain its own.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, RwLock};
use vela_cli::McpService;
use vela_protocol::project::Project;

use crate::db::HubDb;

/// The hot-swappable service handle shared with the HTTP layer. `None`
/// until the first successful refresh (the route answers 503 meanwhile).
pub type SharedMcp = Arc<RwLock<Option<McpService>>>;

/// Refresh cadence. Reuses the ingest interval unless overridden; the
/// webhook makes the interval mostly irrelevant (it kicks immediately).
fn refresh_interval_secs() -> u64 {
    std::env::var("VELA_HUB_MCP_REFRESH_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
        .max(30)
}

/// Spawn the per-machine refresher. `kick` is notified by the webhook
/// handler (after its ingest sweep lands) to refresh ahead of the
/// interval.
pub fn spawn(db: HubDb, shared: SharedMcp, kick: Arc<Notify>) {
    tokio::spawn(async move {
        // The (vfr, latest_snapshot_hash) set the current service was
        // built from; an unchanged set means the rebuild can be skipped.
        let mut built_from: HashMap<String, String> = HashMap::new();
        loop {
            match refresh_from_db(&db, &shared, &mut built_from).await {
                Ok(Some(n)) => tracing::info!(frontiers = n, "mcp-host: projection rebuilt"),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "mcp-host: refresh failed"),
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(refresh_interval_secs())) => {}
                _ = kick.notified() => {
                    tracing::info!("mcp-host: webhook kick, refreshing ahead of interval");
                }
            }
        }
    });
}

/// Rebuild the merged service from the live projection tables when any
/// frontier's promoted snapshot hash moved. `Ok(Some(n))` = rebuilt over
/// n frontiers, `Ok(None)` = nothing changed (or nothing is live).
pub(crate) async fn refresh_from_db(
    db: &HubDb,
    shared: &SharedMcp,
    built_from: &mut HashMap<String, String>,
) -> Result<Option<usize>, String> {
    let live = db.list_entries().await?;
    if live.is_empty() {
        return Ok(None);
    }
    // vfr_id → latest_snapshot_hash, straight from the verified Git index
    // rows. This replaces git-HEAD change detection: an ingest rewrites the
    // index row, so the hash set moves exactly when state moves.
    let mut hashes: HashMap<String, String> = HashMap::new();
    let mut ordered: Vec<String> = Vec::new();
    for entry in &live {
        let (Some(vfr_id), Some(hash)) = (
            entry.get("vfr_id").and_then(|v| v.as_str()),
            entry.get("latest_snapshot_hash").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        if hashes
            .insert(vfr_id.to_string(), hash.to_string())
            .is_none()
        {
            ordered.push(vfr_id.to_string());
        }
    }
    if hashes == *built_from && shared.read().await.is_some() {
        return Ok(None);
    }

    // Hydrate sequentially, one frontier at a time: each call's
    // intermediate snapshot JSON is dropped before the next begins, so
    // peak memory stays one-frontier-sized (the hub runs on 1GB
    // machines). A frontier that fails to hydrate is warned and skipped,
    // never fatal for the rest.
    let mut entries: Vec<(String, Project)> = Vec::new();
    for vfr_id in &ordered {
        match db.get_materialized_project(vfr_id).await {
            Ok(Some(project)) => entries.push((vfr_id.clone(), project)),
            Ok(None) => {
                tracing::warn!(%vfr_id, "mcp-host: live entry has no materialized projection; skipped");
            }
            Err(e) => {
                tracing::warn!(%vfr_id, error = %e, "mcp-host: projection hydrate failed; skipped");
            }
        }
    }
    if entries.is_empty() {
        return Err("no live frontier could be hydrated from the projection tables".to_string());
    }

    // The merge re-materializes every frontier (sync, CPU-bound) — off
    // the runtime.
    let count = entries.len();
    let exclude = McpService::hosted_exclusions();
    let loaded = tokio::task::spawn_blocking(move || {
        McpService::from_projects(entries, "read-only", &exclude)
    })
    .await
    .map_err(|e| format!("mcp build task: {e}"))?;
    match loaded {
        Ok((service, warnings)) => {
            for w in warnings {
                tracing::warn!("mcp-host: skipped {w}");
            }
            *shared.write().await = Some(service);
            *built_from = hashes;
            Ok(Some(count))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::VerifiedFrontierIndex;
    use std::path::Path;
    use vela_protocol::events::{event_log_hash, snapshot_hash};

    // Same scaffolding as git_ingest::tests: a throwaway SQLite HubDb and
    // a copy of the load-bearing example frontier (never modified in
    // place — the copy is what tests may touch).
    async fn sqlite_db() -> crate::db::HubDb {
        let file = tempfile::NamedTempFile::new().expect("temp sqlite");
        let url = format!("sqlite://{}", file.path().display());
        let opts = <sqlx::sqlite::SqliteConnectOptions as std::str::FromStr>::from_str(&url)
            .expect("sqlite opts")
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("sqlite connect");
        crate::db::ensure_sqlite_schema(&pool)
            .await
            .expect("schema");
        std::mem::forget(file);
        crate::db::HubDb::Sqlite(pool)
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
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/erdos-formalization");
        let tmp = tempfile::TempDir::new().unwrap();
        copy_tree(&src, tmp.path());
        tmp
    }

    /// The full DB-hydration loop: strict-verify the fixture, promote it
    /// the way ingest_one does, then refresh_from_db must publish a
    /// working service — and a second refresh with unchanged hashes must
    /// be a no-op.
    #[tokio::test]
    async fn refresh_from_db_builds_service_and_skips_unchanged() {
        let db = sqlite_db().await;
        let tmp = fixture_copy();
        let dir = tmp.path().to_path_buf();
        let (project, fid) =
            tokio::task::spawn_blocking(move || vela_edge::verify::verify_frontier_strict(&dir))
                .await
                .expect("verify task")
                .expect("clean fixture verifies");

        // The internal index cursor, exactly as ingest_one builds it.
        let owner_pubkey = project
            .actors
            .first()
            .map(|a| a.public_key.clone())
            .unwrap_or_else(|| "00".repeat(32));
        let entry = VerifiedFrontierIndex {
            vfr_id: fid.clone(),
            name: project.project.name.clone(),
            owner_actor_id: project
                .actors
                .iter()
                .find(|a| a.public_key == owner_pubkey)
                .map(|a| a.id.clone())
                .unwrap_or_else(|| "owner:unregistered-in-frontier".to_string()),
            owner_pubkey,
            latest_snapshot_hash: snapshot_hash(&project),
            latest_event_log_hash: event_log_hash(&project.events),
            source_commit_at: "2026-07-03T00:00:00Z".to_string(),
            projection_verification: serde_json::json!({
                "schema": "vela.read-projection-verification.v1",
                "integrity": "passed",
                "replay": "passed",
                "strict": "passed",
                "strict_blocker_count": 0,
                "strict_blockers_by_code": {},
                "owner_actor_registered": true,
            }),
        };
        db.upsert_git_source(&fid, "https://example.test/erdos-formalization.git", "main")
            .await
            .expect("register fixture Git source");
        db.promote_frontier_snapshot(&entry, &project, crate::git_ingest::AUTHORITY_GIT_INGESTED)
            .await
            .expect("promote");

        let shared: SharedMcp = Arc::new(RwLock::new(None));
        let mut built_from = HashMap::new();
        let rebuilt = refresh_from_db(&db, &shared, &mut built_from)
            .await
            .expect("first refresh succeeds");
        assert_eq!(rebuilt, Some(1), "one frontier hydrated and merged");
        assert_eq!(built_from.get(&fid), Some(&entry.latest_snapshot_hash));

        // The published service answers an orient tools/call.
        {
            let guard = shared.read().await;
            let service = guard.as_ref().expect("shared service published");
            let (status, body) = service
                .handle_http(
                    r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"orient","arguments":{}}}"#,
                )
                .await;
            assert_eq!(status, 200);
            let body = body.unwrap();
            assert_eq!(body["result"]["isError"], false, "orient succeeds: {body}");
        }

        // Unchanged hash set + a live service = no rebuild.
        let rebuilt = refresh_from_db(&db, &shared, &mut built_from)
            .await
            .expect("second refresh succeeds");
        assert_eq!(rebuilt, None, "unchanged snapshot hashes are a no-op");
    }
}
