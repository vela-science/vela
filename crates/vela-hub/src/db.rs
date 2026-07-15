//! Backend abstraction for the Hub's verified Git projection. Postgres serves
//! hosted deployments; SQLite keeps self-hosting dependency-free. Each backend
//! handles its placeholder syntax (`$1` vs `?`) and JSON representation
//! (`JSONB` vs `TEXT`) while exposing one projection model.

use serde_json::{Value, json};
use sqlx::{PgPool, Row, SqlitePool};
use vela_protocol::events::{StateEvent, event_log_hash, snapshot_hash};
use vela_protocol::project::Project;

/// Backend-agnostic hub database handle. Variant is picked at startup
/// based on the `VELA_HUB_DATABASE_URL` prefix.
#[derive(Clone)]
pub enum HubDb {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

/// Metadata computed by the verified Git ingestor and stored with one
/// materialized frontier projection. This is an internal index cursor, not a
/// signed publication manifest.
#[derive(Debug, Clone)]
pub struct VerifiedFrontierIndex {
    pub vfr_id: String,
    pub name: String,
    pub owner_actor_id: String,
    pub owner_pubkey: String,
    pub latest_snapshot_hash: String,
    pub latest_event_log_hash: String,
    pub source_commit_at: String,
}

#[derive(Debug, Clone)]
pub struct EventFirstPromotionReport {
    pub vfr_id: String,
    pub findings_count: i64,
    pub events_count: i64,
    pub sources_count: i64,
    pub evidence_atoms_count: i64,
    pub condition_records_count: i64,
    pub objects_count: i64,
    pub authority_mode: String,
}

#[derive(Debug, Clone)]
pub struct EventPage {
    pub events: Vec<Value>,
    pub next_cursor: Option<String>,
    pub log_total: i64,
}

struct FrontierObjectRow {
    object_type: String,
    object_id: String,
    seq: i64,
    target_id: Option<String>,
    raw_json: Value,
    /// Producer index: the key that signed/authored the object, when
    /// derivable (finding provenance actor pubkey, attempt signer, …).
    signer_pubkey: Option<String>,
}

type IndexedFrontierRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

pub const FRONTIER_INDEX_SCHEMA: &str = "vela.frontier-index.v1";

fn indexed_entry_json(
    (
        vfr_id,
        name,
        owner_actor_id,
        owner_pubkey,
        latest_snapshot_hash,
        latest_event_log_hash,
        source_commit_at,
        git_remote,
    ): IndexedFrontierRow,
) -> Value {
    json!({
        "schema": FRONTIER_INDEX_SCHEMA,
        "vfr_id": vfr_id,
        "name": name,
        "owner_actor_id": owner_actor_id,
        "owner_pubkey": owner_pubkey,
        "latest_snapshot_hash": latest_snapshot_hash,
        "latest_event_log_hash": latest_event_log_hash,
        "git_remote": crate::git_ingest::canonical_git_remote(&git_remote),
        "source_commit_at": source_commit_at,
    })
}

impl HubDb {
    pub async fn health(&self) -> Result<(), String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(p)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(p)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }

    pub async fn schema_present(&self) -> Result<bool, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'frontiers')",
            )
            .fetch_one(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='frontiers'",
            )
            .fetch_one(p)
            .await
            .map(|n| n > 0)
            .map_err(|e| e.to_string()),
        }
    }

    /// Frontier-index rows synthesized directly from the verified projection
    /// and its configured Git source. There is no second publication record.
    pub async fn list_entries(&self) -> Result<Vec<Value>, String> {
        let rows: Vec<IndexedFrontierRow> = match self {
            Self::Postgres(p) => sqlx::query_as(
                r#"
                SELECT f.vfr_id, f.name, f.owner_actor_id, f.owner_pubkey,
                       f.latest_snapshot_hash, f.latest_event_log_hash,
                       to_char(f.source_commit_at AT TIME ZONE 'UTC',
                               'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                       g.git_remote
                FROM frontiers f
                JOIN frontier_git_remotes g ON g.vfr_id = f.vfr_id
                ORDER BY f.source_commit_at DESC
                "#,
            )
            .fetch_all(p)
            .await
            .map_err(|e| e.to_string())?,
            Self::Sqlite(p) => sqlx::query_as(
                r#"
                SELECT f.vfr_id, f.name, f.owner_actor_id, f.owner_pubkey,
                       f.latest_snapshot_hash, f.latest_event_log_hash,
                       f.source_commit_at, g.git_remote
                FROM frontiers f
                JOIN frontier_git_remotes g ON g.vfr_id = f.vfr_id
                ORDER BY f.source_commit_at DESC
                "#,
            )
            .fetch_all(p)
            .await
            .map_err(|e| e.to_string())?,
        };
        Ok(rows.into_iter().map(indexed_entry_json).collect())
    }

    pub async fn get_index_entry(&self, vfr_id: &str) -> Result<Option<Value>, String> {
        let row: Option<IndexedFrontierRow> = match self {
            Self::Postgres(p) => sqlx::query_as(
                r#"
                SELECT f.vfr_id, f.name, f.owner_actor_id, f.owner_pubkey,
                       f.latest_snapshot_hash, f.latest_event_log_hash,
                       to_char(f.source_commit_at AT TIME ZONE 'UTC',
                               'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                       g.git_remote
                FROM frontiers f
                JOIN frontier_git_remotes g ON g.vfr_id = f.vfr_id
                WHERE f.vfr_id = $1
                LIMIT 1
                "#,
            )
            .bind(vfr_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string())?,
            Self::Sqlite(p) => sqlx::query_as(
                r#"
                SELECT f.vfr_id, f.name, f.owner_actor_id, f.owner_pubkey,
                       f.latest_snapshot_hash, f.latest_event_log_hash,
                       f.source_commit_at, g.git_remote
                FROM frontiers f
                JOIN frontier_git_remotes g ON g.vfr_id = f.vfr_id
                WHERE f.vfr_id = ?
                LIMIT 1
                "#,
            )
            .bind(vfr_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string())?,
        };
        Ok(row.map(indexed_entry_json))
    }

    /// Lightweight per-frontier counts for list/dashboard views, computed by
    /// cheap aggregates over the projection tables — never by reading the full
    /// (multi-MB) snapshot. object_type counts come from `frontier_objects`
    /// (indexed on `(vfr_id, object_type)`); events from `frontier_events`;
    /// contested/human_reviewed/avg_confidence from finding `review_state` flags
    /// and confidence scores. Returns None when the frontier is not indexed.
    pub async fn frontier_summary(&self, vfr_id: &str) -> Result<Option<Value>, String> {
        if self.get_index_entry(vfr_id).await?.is_none() {
            return Ok(None);
        }
        type FlagAgg = (i64, i64, Option<f64>);
        let (obj_counts, events, flags): (Vec<(String, i64)>, i64, FlagAgg) = match self {
            Self::Postgres(p) => {
                let rows: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT object_type, COUNT(*)::bigint FROM frontier_objects \
                     WHERE vfr_id = $1 GROUP BY object_type",
                )
                .bind(vfr_id)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let events: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*)::bigint FROM frontier_events WHERE vfr_id = $1",
                )
                .bind(vfr_id)
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
                let flags: FlagAgg = sqlx::query_as(
                    "SELECT \
                       COUNT(CASE WHEN raw_json #>> '{flags,review_state}' = 'contested' THEN 1 END)::bigint, \
                       COUNT(CASE WHEN raw_json #>> '{flags,review_state}' = 'accepted'  THEN 1 END)::bigint, \
                       AVG((raw_json #>> '{confidence,score}')::double precision) \
                     FROM frontier_objects WHERE vfr_id = $1 AND object_type = 'finding'",
                )
                .bind(vfr_id)
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
                (rows, events, flags)
            }
            Self::Sqlite(p) => {
                let rows: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT object_type, COUNT(*) FROM frontier_objects \
                     WHERE vfr_id = ? GROUP BY object_type",
                )
                .bind(vfr_id)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let events: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM frontier_events WHERE vfr_id = ?")
                        .bind(vfr_id)
                        .fetch_one(p)
                        .await
                        .map_err(|e| e.to_string())?;
                let flags: FlagAgg = sqlx::query_as(
                    "SELECT \
                       COUNT(CASE WHEN json_extract(raw_json,'$.flags.review_state') = 'contested' THEN 1 END), \
                       COUNT(CASE WHEN json_extract(raw_json,'$.flags.review_state') = 'accepted'  THEN 1 END), \
                       AVG(json_extract(raw_json,'$.confidence.score')) \
                     FROM frontier_objects WHERE vfr_id = ? AND object_type = 'finding'",
                )
                .bind(vfr_id)
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
                (rows, events, flags)
            }
        };
        let map: std::collections::BTreeMap<String, i64> = obj_counts.into_iter().collect();
        let g = |k: &str| map.get(k).copied().unwrap_or(0);
        let (contested, human_reviewed, avg_confidence) = flags;
        Ok(Some(json!({
            "vfr_id": vfr_id,
            "findings": g("finding"),
            "sources": g("source"),
            "evidence_atoms": g("evidence_atom"),
            "links": g("link"),
            "proposals": g("proposal"),
            "events": events,
            "contested": contested,
            "human_reviewed": human_reviewed,
            "avg_confidence": avg_confidence.unwrap_or(0.0),
        })))
    }

    /// Lightweight object index for the frontier manifest: `(type, id, target_id,
    /// seq)` for every object, WITHOUT the bulk raw_json. Lets a client list a
    /// frontier and then fetch only the objects it opens (sparse / partial clone),
    /// instead of pulling the whole multi-MB snapshot.
    pub async fn frontier_object_index(&self, vfr_id: &str) -> Result<Vec<Value>, String> {
        type Row = (String, String, Option<String>, i64);
        let rows: Vec<Row> = match self {
            Self::Postgres(p) => sqlx::query_as(
                "SELECT object_type, object_id, target_id, seq FROM frontier_objects \
                 WHERE vfr_id = $1 ORDER BY object_type, seq",
            )
            .bind(vfr_id)
            .fetch_all(p)
            .await
            .map_err(|e| e.to_string())?,
            Self::Sqlite(p) => sqlx::query_as(
                "SELECT object_type, object_id, target_id, seq FROM frontier_objects \
                 WHERE vfr_id = ? ORDER BY object_type, seq",
            )
            .bind(vfr_id)
            .fetch_all(p)
            .await
            .map_err(|e| e.to_string())?,
        };
        Ok(rows
            .into_iter()
            .map(|(t, id, tgt, seq)| json!({"type": t, "id": id, "target_id": tgt, "seq": seq}))
            .collect())
    }

    /// Cross-frontier object text search (the /search backend) — one query
    /// over `frontier_objects` instead of downloading every frontier's
    /// multi-MB snapshot and scanning client-side. Restricted to one
    /// `object_type`, across indexed frontiers. Returns
    /// `({vfr_id, object} rows, total)` where `total` counts every match
    /// under the same predicate so callers can paginate additively.
    ///
    /// Postgres: real FTS — `websearch_to_tsquery('english', q)` against
    /// the stored generated `search_text` tsvector, ranked by `ts_rank`.
    /// `websearch_to_tsquery` is total (never errors on user syntax).
    /// SQLite (self-hosted): the original substring LIKE, same shape.
    pub async fn search_objects(
        &self,
        q: &str,
        object_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        type Row = (String, String);
        let (rows, total): (Vec<Row>, i64) = match self {
            Self::Postgres(p) => {
                let rows: Vec<Row> = sqlx::query_as(PG_FTS_SEARCH_SQL)
                    .bind(object_type)
                    .bind(q)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(p)
                    .await
                    .map_err(|e| e.to_string())?;
                let total: i64 = sqlx::query_scalar(PG_FTS_COUNT_SQL)
                    .bind(object_type)
                    .bind(q)
                    .fetch_one(p)
                    .await
                    .map_err(|e| e.to_string())?;
                (rows, total)
            }
            Self::Sqlite(p) => {
                let pattern = format!(
                    "%{}%",
                    q.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                );
                let rows: Vec<Row> = sqlx::query_as(
                    "SELECT o.vfr_id, o.raw_json \
                     FROM frontier_objects o \
                     WHERE o.object_type = ? AND o.raw_json LIKE ? ESCAPE '\\' \
                     ORDER BY o.vfr_id, o.seq LIMIT ? OFFSET ?",
                )
                .bind(object_type)
                .bind(&pattern)
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let total: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) \
                     FROM frontier_objects o \
                     WHERE o.object_type = ? AND o.raw_json LIKE ? ESCAPE '\\'",
                )
                .bind(object_type)
                .bind(&pattern)
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
                (rows, total)
            }
        };
        let values = rows
            .into_iter()
            .filter_map(|(vfr, raw)| {
                serde_json::from_str::<Value>(&raw)
                    .ok()
                    .map(|obj| json!({"vfr_id": vfr, "object": obj}))
            })
            .collect();
        Ok((values, total))
    }

    /// One page of a frontier's objects of a given type (raw_json), ordered by
    /// seq, with the total count — so the site renders a detail surface (sources,
    /// proposals, …) without pulling the whole multi-MB snapshot. Returns
    /// `(objects, total)`.
    pub async fn frontier_objects_page(
        &self,
        vfr_id: &str,
        object_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        let (rows, total): (Vec<String>, i64) = match self {
            Self::Postgres(p) => {
                let rows: Vec<String> = sqlx::query_scalar(
                    "SELECT raw_json::text FROM frontier_objects \
                     WHERE vfr_id = $1 AND object_type = $2 ORDER BY seq LIMIT $3 OFFSET $4",
                )
                .bind(vfr_id)
                .bind(object_type)
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let total: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*)::bigint FROM frontier_objects WHERE vfr_id = $1 AND object_type = $2",
                )
                .bind(vfr_id).bind(object_type).fetch_one(p).await.map_err(|e| e.to_string())?;
                (rows, total)
            }
            Self::Sqlite(p) => {
                let rows: Vec<String> = sqlx::query_scalar(
                    "SELECT raw_json FROM frontier_objects \
                     WHERE vfr_id = ? AND object_type = ? ORDER BY seq LIMIT ? OFFSET ?",
                )
                .bind(vfr_id)
                .bind(object_type)
                .bind(limit)
                .bind(offset)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let total: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM frontier_objects WHERE vfr_id = ? AND object_type = ?",
                )
                .bind(vfr_id)
                .bind(object_type)
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
                (rows, total)
            }
        };
        let objects = rows
            .into_iter()
            .filter_map(|s| serde_json::from_str::<Value>(&s).ok())
            .collect();
        Ok((objects, total))
    }

    /// A single frontier object by `(type, object_id)` — a primary-key point
    /// lookup. Returns the raw_json, or None if absent.
    pub async fn frontier_object(
        &self,
        vfr_id: &str,
        object_type: &str,
        object_id: &str,
    ) -> Result<Option<Value>, String> {
        let row: Option<String> = match self {
            Self::Postgres(p) => sqlx::query_scalar(
                "SELECT raw_json::text FROM frontier_objects \
                 WHERE vfr_id = $1 AND object_type = $2 AND object_id = $3 LIMIT 1",
            )
            .bind(vfr_id)
            .bind(object_type)
            .bind(object_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string())?,
            Self::Sqlite(p) => sqlx::query_scalar(
                "SELECT raw_json FROM frontier_objects \
                 WHERE vfr_id = ? AND object_type = ? AND object_id = ? LIMIT 1",
            )
            .bind(vfr_id)
            .bind(object_type)
            .bind(object_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string())?,
        };
        match row {
            Some(s) => serde_json::from_str::<Value>(&s)
                .map(Some)
                .map_err(|e| e.to_string()),
            None => Ok(None),
        }
    }

    /// Look up a replayed Scientific Diff Pack record by its `vsd_*` id.
    /// Packs are projected from each verified Git frontier's
    /// `released_diff_packs` array; there is no independent Hub write path.
    pub async fn get_diff_pack(&self, pack_id: &str) -> Result<Option<Value>, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, Value>(
                r#"
                SELECT raw_json
                FROM frontier_objects
                WHERE object_type = 'diff_pack'
                  AND object_id = $1
                LIMIT 1
                "#,
            )
            .bind(pack_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => {
                let row: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT raw_json
                    FROM frontier_objects
                    WHERE object_type = 'diff_pack'
                      AND object_id = ?
                    LIMIT 1
                    "#,
                )
                .bind(pack_id)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string())?;
                match row {
                    Some(s) => serde_json::from_str::<Value>(&s)
                        .map(Some)
                        .map_err(|e| e.to_string()),
                    None => Ok(None),
                }
            }
        }
    }

    /// Cross-frontier producer view: verified-frontier objects signed by
    /// one key (the fundable CV / 48-hour due-diligence query).
    pub async fn producer_objects(
        &self,
        pubkey: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String, Value)>, String> {
        match self {
            Self::Postgres(p) => {
                let rows: Vec<(String, String, String, Value)> = sqlx::query_as(
                    "SELECT vfr_id, object_type, object_id, raw_json FROM frontier_objects WHERE signer_pubkey = $1 ORDER BY vfr_id, object_type, object_id LIMIT $2",
                )
                .bind(pubkey).bind(limit).fetch_all(p).await.map_err(|e| e.to_string())?;
                Ok(rows)
            }
            Self::Sqlite(p) => {
                let rows: Vec<(String, String, String, String)> = sqlx::query_as(
                    "SELECT vfr_id, object_type, object_id, raw_json FROM frontier_objects WHERE signer_pubkey = ?1 ORDER BY vfr_id, object_type, object_id LIMIT ?2",
                )
                .bind(pubkey).bind(limit).fetch_all(p).await.map_err(|e| e.to_string())?;
                rows.into_iter()
                    .map(|(v, t, i, r)| {
                        serde_json::from_str(&r)
                            .map(|j| (v, t, i, j))
                            .map_err(|e| e.to_string())
                    })
                    .collect()
            }
        }
    }

    /// Upsert one operator-configured Git source. An unchanged source keeps its
    /// ingest cursor; changing the remote or ref clears the cursor so the next
    /// sweep verifies it from scratch.
    pub async fn upsert_git_source(
        &self,
        vfr_id: &str,
        git_remote: &str,
        git_ref: &str,
    ) -> Result<(), String> {
        match self {
            Self::Postgres(p) => {
                sqlx::query(
                    r#"
                    INSERT INTO frontier_git_remotes
                        (vfr_id, git_remote, git_ref)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (vfr_id) DO UPDATE SET
                        git_remote = EXCLUDED.git_remote,
                        git_ref = EXCLUDED.git_ref,
                        last_ingested_commit = NULL,
                        ingest_error = NULL
                    WHERE frontier_git_remotes.git_remote IS DISTINCT FROM EXCLUDED.git_remote
                       OR frontier_git_remotes.git_ref IS DISTINCT FROM EXCLUDED.git_ref
                    "#,
                )
                .bind(vfr_id)
                .bind(git_remote)
                .bind(git_ref)
                .execute(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            }
            Self::Sqlite(p) => {
                sqlx::query(
                    r#"
                    INSERT INTO frontier_git_remotes
                        (vfr_id, git_remote, git_ref)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT (vfr_id) DO UPDATE SET
                        git_remote = excluded.git_remote,
                        git_ref = excluded.git_ref,
                        last_ingested_commit = NULL,
                        ingest_error = NULL
                    WHERE frontier_git_remotes.git_remote <> excluded.git_remote
                       OR frontier_git_remotes.git_ref <> excluded.git_ref
                    "#,
                )
                .bind(vfr_id)
                .bind(git_remote)
                .bind(git_ref)
                .execute(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    /// Remove projections for sources absent from the active catalog. Hub rows
    /// are disposable derived state, so catalog removal must not leave a stale
    /// frontier discoverable through search or point reads.
    pub async fn retain_git_sources(&self, keep: &[String]) -> Result<usize, String> {
        match self {
            Self::Postgres(pool) => {
                let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
                let existing: Vec<String> =
                    sqlx::query_scalar("SELECT vfr_id FROM frontier_git_remotes")
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                let removed: Vec<String> = existing
                    .into_iter()
                    .filter(|vfr_id| !keep.contains(vfr_id))
                    .collect();
                for vfr_id in &removed {
                    sqlx::query("DELETE FROM frontiers WHERE vfr_id = $1")
                        .bind(vfr_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                    sqlx::query("DELETE FROM frontier_git_remotes WHERE vfr_id = $1")
                        .bind(vfr_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tx.commit().await.map_err(|e| e.to_string())?;
                Ok(removed.len())
            }
            Self::Sqlite(pool) => {
                let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
                let existing: Vec<String> =
                    sqlx::query_scalar("SELECT vfr_id FROM frontier_git_remotes")
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                let removed: Vec<String> = existing
                    .into_iter()
                    .filter(|vfr_id| !keep.contains(vfr_id))
                    .collect();
                for vfr_id in &removed {
                    for table in ["frontier_events", "frontier_objects", "frontiers"] {
                        let statement = format!("DELETE FROM {table} WHERE vfr_id = ?1");
                        sqlx::query(&statement)
                            .bind(vfr_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    sqlx::query("DELETE FROM frontier_git_remotes WHERE vfr_id = ?1")
                        .bind(vfr_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tx.commit().await.map_err(|e| e.to_string())?;
                Ok(removed.len())
            }
        }
    }

    /// Try to become THE ingest sweeper for this tick. Postgres: a
    /// TRANSACTION-scoped advisory lock (`pg_try_advisory_xact_lock`) held
    /// by the returned guard's open transaction — dropping the guard ends
    /// the transaction and releases the lock, so a pooled connection can
    /// never smuggle a stale session lock. None when another machine holds
    /// it. SQLite is single-node: always the leader.
    pub async fn try_ingest_lock(&self) -> Result<Option<IngestLockGuard>, String> {
        const INGEST_LOCK_KEY: i64 = 0x0076_656c_6169_6e67; // "vela"+"ing"
        match self {
            Self::Postgres(p) => {
                let mut tx = p.begin().await.map_err(|e| e.to_string())?;
                let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
                    .bind(INGEST_LOCK_KEY)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                if got {
                    Ok(Some(IngestLockGuard { _tx: Some(tx) }))
                } else {
                    Ok(None)
                }
            }
            Self::Sqlite(_) => Ok(Some(IngestLockGuard { _tx: None })),
        }
    }

    /// Every configured Git-ingestion target with its cursor, for the
    /// ingestor's tick. Row shape: (vfr_id, git_remote, git_ref,
    /// last_ingested_commit).
    pub async fn git_ingest_targets(
        &self,
    ) -> Result<Vec<(String, String, String, Option<String>)>, String> {
        const Q: &str = "SELECT vfr_id, git_remote, git_ref, last_ingested_commit \
                         FROM frontier_git_remotes";
        match self {
            Self::Postgres(p) => sqlx::query_as(Q)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_as(Q)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    /// Record the outcome of one ingest attempt (the cursor on success, the
    /// error text on failure — surfaced by the Git-source read endpoint).
    pub async fn record_git_ingest(
        &self,
        vfr_id: &str,
        commit: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), String> {
        match self {
            Self::Postgres(p) => {
                sqlx::query(
                    "UPDATE frontier_git_remotes SET \
                       last_ingested_commit = COALESCE($2, last_ingested_commit), \
                       last_ingested_at = now(), ingest_error = $3 \
                     WHERE vfr_id = $1",
                )
                .bind(vfr_id)
                .bind(commit)
                .bind(error)
                .execute(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            }
            Self::Sqlite(p) => {
                sqlx::query(
                    "UPDATE frontier_git_remotes SET \
                       last_ingested_commit = COALESCE(?2, last_ingested_commit), \
                       last_ingested_at = datetime('now'), ingest_error = ?3 \
                     WHERE vfr_id = ?1",
                )
                .bind(vfr_id)
                .bind(commit)
                .bind(error)
                .execute(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    /// The configured Git source and ingest cursor for one frontier.
    pub async fn get_git_remote(&self, vfr_id: &str) -> Result<Option<Value>, String> {
        const Q_PG: &str = "SELECT json_build_object(\
            'git_remote', git_remote, 'git_ref', git_ref, \
            'last_ingested_commit', last_ingested_commit, \
            'last_ingested_at', last_ingested_at::text, 'ingest_error', ingest_error) \
            FROM frontier_git_remotes WHERE vfr_id = $1";
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, Value>(Q_PG)
                .bind(vfr_id)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string()),
            Self::Sqlite(p) => {
                type GitRemoteRow = (
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                );
                let row: Option<GitRemoteRow> = sqlx::query_as(
                    "SELECT git_remote, git_ref, last_ingested_commit, \
                         last_ingested_at, ingest_error \
                         FROM frontier_git_remotes WHERE vfr_id = ?1",
                )
                .bind(vfr_id)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(row.map(|(remote, r#ref, commit, ing_at, err)| {
                    serde_json::json!({
                        "git_remote": remote, "git_ref": r#ref,
                        "last_ingested_commit": commit,
                        "last_ingested_at": ing_at, "ingest_error": err,
                    })
                }))
            }
        }
    }

    /// Ingest health per registered remote: (vfr_id, failing, seconds
    /// since last completed ingest). Feeds the /metrics gauges.
    pub async fn ingest_health(&self) -> Result<Vec<(String, bool, Option<i64>)>, String> {
        match self {
            Self::Postgres(p) => {
                let rows: Vec<(String, bool, Option<f64>)> = sqlx::query_as(
                    "SELECT vfr_id, ingest_error IS NOT NULL, \
                     EXTRACT(EPOCH FROM now() - last_ingested_at)::float8 \
                     FROM frontier_git_remotes",
                )
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(rows
                    .into_iter()
                    .map(|(v, f, age)| (v, f, age.map(|a| a as i64)))
                    .collect())
            }
            Self::Sqlite(p) => {
                let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT vfr_id, ingest_error, last_ingested_at \
                     FROM frontier_git_remotes",
                )
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let now = chrono::Utc::now();
                Ok(rows
                    .into_iter()
                    .map(|(v, err, at)| {
                        let age = at
                            .as_deref()
                            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                            .map(|t| (now - t.with_timezone(&chrono::Utc)).num_seconds());
                        (v, err.is_some(), age)
                    })
                    .collect())
            }
        }
    }

    /// Replace the read projection from one fully verified Git checkout.
    /// Callers must verify the repository and replay before invoking this
    /// method; the Hub accepts no independent manifest or snapshot bytes.
    pub async fn promote_frontier_snapshot(
        &self,
        entry: &VerifiedFrontierIndex,
        project: &Project,
        authority_mode: &str,
    ) -> Result<EventFirstPromotionReport, String> {
        let computed_snapshot = snapshot_hash(project);
        if computed_snapshot != entry.latest_snapshot_hash {
            return Err(format!(
                "snapshot_hash mismatch: index declares {}, verified project hashes to {}",
                entry.latest_snapshot_hash, computed_snapshot
            ));
        }
        let computed_event_log = event_log_hash(&project.events);
        if computed_event_log != entry.latest_event_log_hash {
            return Err(format!(
                "event_log_hash mismatch: index declares {}, verified events hash to {}",
                entry.latest_event_log_hash, computed_event_log
            ));
        }

        let snapshot_value =
            serde_json::to_value(project).map_err(|e| format!("serialize project: {e}"))?;
        let snapshot_skeleton = frontier_skeleton(&snapshot_value);
        let snapshot_skeleton_json =
            serde_json::to_string(&snapshot_skeleton).map_err(|e| format!("project json: {e}"))?;
        let schema_version = snapshot_value
            .get("schema")
            .and_then(Value::as_str)
            .or_else(|| snapshot_value.get("vela_version").and_then(Value::as_str))
            .unwrap_or("unknown");
        let findings_count = project.findings.len() as i64;
        let events_count = project.events.len() as i64;
        let sources_count = project.sources.len() as i64;
        let evidence_atoms_count = project.evidence_atoms.len() as i64;
        let condition_records_count = project.condition_records.len() as i64;
        let objects = collect_frontier_objects(&snapshot_value);
        let objects_count = objects.len() as i64;

        match self {
            Self::Postgres(p) => {
                let mut tx = p.begin().await.map_err(|e| e.to_string())?;
                sqlx::query("DELETE FROM frontier_events WHERE vfr_id = $1")
                    .bind(&entry.vfr_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                sqlx::query("DELETE FROM frontier_objects WHERE vfr_id = $1")
                    .bind(&entry.vfr_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                sqlx::query(
                    r#"
                    INSERT INTO frontiers (
                      vfr_id, name, owner_actor_id, owner_pubkey,
                      latest_snapshot_hash, latest_event_log_hash, schema_version,
                      source_commit_at,
                      findings_count, events_count, sources_count, evidence_atoms_count,
                      condition_records_count, materialized_snapshot_json, authority_mode
                    )
                    VALUES (
                      $1, $2, $3, $4,
                      $5, $6, $7,
                      $8::timestamptz,
                      $9, $10, $11, $12,
                      $13, $14::jsonb, $15
                    )
                    ON CONFLICT (vfr_id) DO UPDATE SET
                      name = EXCLUDED.name,
                      owner_actor_id = EXCLUDED.owner_actor_id,
                      owner_pubkey = EXCLUDED.owner_pubkey,
                      latest_snapshot_hash = EXCLUDED.latest_snapshot_hash,
                      latest_event_log_hash = EXCLUDED.latest_event_log_hash,
                      schema_version = EXCLUDED.schema_version,
                      source_commit_at = EXCLUDED.source_commit_at,
                      findings_count = EXCLUDED.findings_count,
                      events_count = EXCLUDED.events_count,
                      sources_count = EXCLUDED.sources_count,
                      evidence_atoms_count = EXCLUDED.evidence_atoms_count,
                      condition_records_count = EXCLUDED.condition_records_count,
                      materialized_snapshot_json = EXCLUDED.materialized_snapshot_json,
                      authority_mode = EXCLUDED.authority_mode,
                      updated_at = now()
                    "#,
                )
                .bind(&entry.vfr_id)
                .bind(&entry.name)
                .bind(&entry.owner_actor_id)
                .bind(&entry.owner_pubkey)
                .bind(&entry.latest_snapshot_hash)
                .bind(&entry.latest_event_log_hash)
                .bind(schema_version)
                .bind(&entry.source_commit_at)
                .bind(findings_count)
                .bind(events_count)
                .bind(sources_count)
                .bind(evidence_atoms_count)
                .bind(condition_records_count)
                .bind(&snapshot_skeleton_json)
                .bind(authority_mode)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

                let mut event_rows = Vec::with_capacity(project.events.len());
                for (idx, event) in project.events.iter().enumerate() {
                    let raw = serde_json::to_value(event)
                        .map_err(|e| format!("serialize event {}: {e}", event.id))?;
                    event_rows.push(json!({
                        "seq": idx as i64,
                        "event_id": event.id,
                        "kind": event.kind,
                        "target_type": event.target.r#type,
                        "target_id": event.target.id,
                        "actor_id": event.actor.id,
                        "event_timestamp": event.timestamp,
                        "raw_json": raw,
                    }));
                }
                for chunk in event_rows.chunks(4_000) {
                    let batch = Value::Array(chunk.to_vec());
                    sqlx::query(
                        r#"
                        INSERT INTO frontier_events (
                          vfr_id, seq, event_id, kind, target_type, target_id,
                          actor_id, event_timestamp, raw_json
                        )
                        SELECT
                          $1,
                          (item->>'seq')::bigint,
                          item->>'event_id',
                          item->>'kind',
                          item->>'target_type',
                          item->>'target_id',
                          item->>'actor_id',
                          (item->>'event_timestamp')::timestamptz,
                          item->'raw_json'
                        FROM jsonb_array_elements($2::jsonb) AS item
                        "#,
                    )
                    .bind(&entry.vfr_id)
                    .bind(&batch)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }

                for chunk in objects.chunks(1_000) {
                    let batch = Value::Array(
                        chunk
                            .iter()
                            .map(|object| {
                                json!({
                                    "object_type": object.object_type,
                                    "object_id": object.object_id,
                                    "seq": object.seq,
                                    "target_id": object.target_id,
                                    "raw_json": object.raw_json,
                                    "signer_pubkey": object.signer_pubkey,
                                })
                            })
                            .collect(),
                    );
                    sqlx::query(
                        r#"
                        INSERT INTO frontier_objects (
                          vfr_id, object_type, object_id, seq, target_id, raw_json, signer_pubkey
                        )
                        SELECT
                          $1,
                          item->>'object_type',
                          item->>'object_id',
                          (item->>'seq')::bigint,
                          item->>'target_id',
                          item->'raw_json',
                          item->>'signer_pubkey'
                        FROM jsonb_array_elements($2::jsonb) AS item
                        "#,
                    )
                    .bind(&entry.vfr_id)
                    .bind(&batch)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }

                tx.commit().await.map_err(|e| e.to_string())?;
            }
            Self::Sqlite(p) => {
                let mut tx = p.begin().await.map_err(|e| e.to_string())?;
                sqlx::query("DELETE FROM frontier_events WHERE vfr_id = ?")
                    .bind(&entry.vfr_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                sqlx::query("DELETE FROM frontier_objects WHERE vfr_id = ?")
                    .bind(&entry.vfr_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                sqlx::query(
                    r#"
                    INSERT INTO frontiers (
                      vfr_id, name, owner_actor_id, owner_pubkey,
                      latest_snapshot_hash, latest_event_log_hash, schema_version,
                      source_commit_at,
                      findings_count, events_count, sources_count, evidence_atoms_count,
                      condition_records_count, materialized_snapshot_json, authority_mode
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(vfr_id) DO UPDATE SET
                      name = excluded.name,
                      owner_actor_id = excluded.owner_actor_id,
                      owner_pubkey = excluded.owner_pubkey,
                      latest_snapshot_hash = excluded.latest_snapshot_hash,
                      latest_event_log_hash = excluded.latest_event_log_hash,
                      schema_version = excluded.schema_version,
                      source_commit_at = excluded.source_commit_at,
                      findings_count = excluded.findings_count,
                      events_count = excluded.events_count,
                      sources_count = excluded.sources_count,
                      evidence_atoms_count = excluded.evidence_atoms_count,
                      condition_records_count = excluded.condition_records_count,
                      materialized_snapshot_json = excluded.materialized_snapshot_json,
                      authority_mode = excluded.authority_mode,
                      updated_at = datetime('now')
                    "#,
                )
                .bind(&entry.vfr_id)
                .bind(&entry.name)
                .bind(&entry.owner_actor_id)
                .bind(&entry.owner_pubkey)
                .bind(&entry.latest_snapshot_hash)
                .bind(&entry.latest_event_log_hash)
                .bind(schema_version)
                .bind(&entry.source_commit_at)
                .bind(findings_count)
                .bind(events_count)
                .bind(sources_count)
                .bind(evidence_atoms_count)
                .bind(condition_records_count)
                .bind(&snapshot_skeleton_json)
                .bind(authority_mode)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

                for (idx, event) in project.events.iter().enumerate() {
                    let raw = serde_json::to_string(event)
                        .map_err(|e| format!("serialize event {}: {e}", event.id))?;
                    sqlx::query(
                        r#"
                        INSERT INTO frontier_events (
                          vfr_id, seq, event_id, kind, target_type, target_id,
                          actor_id, event_timestamp, raw_json
                        )
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&entry.vfr_id)
                    .bind(idx as i64)
                    .bind(&event.id)
                    .bind(event.kind.as_str())
                    .bind(&event.target.r#type)
                    .bind(&event.target.id)
                    .bind(&event.actor.id)
                    .bind(&event.timestamp)
                    .bind(&raw)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }

                for object in &objects {
                    let raw = serde_json::to_string(&object.raw_json)
                        .map_err(|e| format!("serialize object {}: {e}", object.object_id))?;
                    sqlx::query(
                        r#"
                        INSERT INTO frontier_objects (
                          vfr_id, object_type, object_id, seq, target_id, raw_json, signer_pubkey
                        )
                        VALUES (?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
                    .bind(&entry.vfr_id)
                    .bind(&object.object_type)
                    .bind(&object.object_id)
                    .bind(object.seq)
                    .bind(&object.target_id)
                    .bind(&raw)
                    .bind(&object.signer_pubkey)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                }

                tx.commit().await.map_err(|e| e.to_string())?;
            }
        }

        Ok(EventFirstPromotionReport {
            vfr_id: entry.vfr_id.clone(),
            findings_count,
            events_count,
            sources_count,
            evidence_atoms_count,
            condition_records_count,
            objects_count,
            authority_mode: authority_mode.to_string(),
        })
    }

    pub async fn get_materialized_project(&self, vfr_id: &str) -> Result<Option<Project>, String> {
        match self {
            Self::Postgres(p) => {
                let mut value: Option<Value> = sqlx::query_scalar(
                    "SELECT materialized_snapshot_json FROM frontiers WHERE vfr_id = $1",
                )
                .bind(vfr_id)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string())?;
                if let Some(snapshot) = value.as_mut() {
                    let rows = sqlx::query(
                        r#"
                        SELECT object_type, seq, raw_json
                        FROM frontier_objects
                        WHERE vfr_id = $1
                        ORDER BY object_type, seq
                        "#,
                    )
                    .bind(vfr_id)
                    .fetch_all(p)
                    .await
                    .map_err(|e| e.to_string())?;
                    let objects = rows
                        .into_iter()
                        .map(|row| {
                            Ok((
                                row.try_get::<String, _>("object_type")?,
                                row.try_get::<i64, _>("seq")?,
                                row.try_get::<Value, _>("raw_json")?,
                            ))
                        })
                        .collect::<Result<Vec<_>, sqlx::Error>>()
                        .map_err(|e| e.to_string())?;
                    merge_projected_objects(snapshot, objects);
                }
                value
                    .map(serde_json::from_value::<Project>)
                    .transpose()
                    .map_err(|e| e.to_string())
            }
            Self::Sqlite(p) => {
                let value: Option<String> = sqlx::query_scalar(
                    "SELECT materialized_snapshot_json FROM frontiers WHERE vfr_id = ?",
                )
                .bind(vfr_id)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string())?;
                let Some(raw) = value else {
                    return Ok(None);
                };
                let mut snapshot =
                    serde_json::from_str::<Value>(&raw).map_err(|e| e.to_string())?;
                let rows: Vec<(String, i64, String)> = sqlx::query_as(
                    r#"
                    SELECT object_type, seq, raw_json
                    FROM frontier_objects
                    WHERE vfr_id = ?
                    ORDER BY object_type, seq
                    "#,
                )
                .bind(vfr_id)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                let objects = rows
                    .into_iter()
                    .map(|(object_type, seq, raw)| {
                        serde_json::from_str::<Value>(&raw)
                            .map(|value| (object_type, seq, value))
                            .map_err(|e| e.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                merge_projected_objects(&mut snapshot, objects);
                serde_json::from_value::<Project>(snapshot)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
        }
    }

    pub async fn event_log_hash_from_db(&self, vfr_id: &str) -> Result<String, String> {
        let values = self
            .event_values_after(vfr_id, None, None, None, i64::MAX)
            .await?;
        let mut events: Vec<StateEvent> = values
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("parse event log: {e}"))?;
        // Hash in the loader's canonical id-sorted order (rows come back in
        // projection sequence), so this recompute equals the stored hash and
        // what a cold Git clone reconstructs.
        events.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(event_log_hash(&events))
    }

    pub async fn event_page(
        &self,
        vfr_id: &str,
        cursor: Option<&str>,
        limit: usize,
        kind: Option<&str>,
        target: Option<&str>,
    ) -> Result<EventPage, String> {
        let cursor_seq = match cursor {
            Some(cursor) => Some(self.event_seq(vfr_id, cursor).await?.ok_or_else(|| {
                format!("cursor_not_found: cursor '{cursor}' not found in event log")
            })?),
            None => None,
        };
        let take = limit.clamp(1, 500) as i64;
        let rows = self
            .event_values_after(vfr_id, cursor_seq, kind, target, take + 1)
            .await?;
        let log_total = self.event_log_total(vfr_id).await?;
        let has_more = rows.len() as i64 > take;
        let events: Vec<Value> = rows.into_iter().take(take as usize).collect();
        let next_cursor = if has_more {
            events
                .last()
                .and_then(|v| v.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            None
        };
        Ok(EventPage {
            events,
            next_cursor,
            log_total,
        })
    }

    async fn event_seq(&self, vfr_id: &str, event_id: &str) -> Result<Option<i64>, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, i64>(
                "SELECT seq FROM frontier_events WHERE vfr_id = $1 AND event_id = $2",
            )
            .bind(vfr_id)
            .bind(event_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_scalar::<_, i64>(
                "SELECT seq FROM frontier_events WHERE vfr_id = ? AND event_id = ?",
            )
            .bind(vfr_id)
            .bind(event_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string()),
        }
    }

    async fn event_log_total(&self, vfr_id: &str) -> Result<i64, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM frontier_events WHERE vfr_id = $1",
            )
            .bind(vfr_id)
            .fetch_one(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM frontier_events WHERE vfr_id = ?",
            )
            .bind(vfr_id)
            .fetch_one(p)
            .await
            .map_err(|e| e.to_string()),
        }
    }

    async fn event_values_after(
        &self,
        vfr_id: &str,
        cursor_seq: Option<i64>,
        kind: Option<&str>,
        target: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Value>, String> {
        let start_seq = cursor_seq.unwrap_or(-1);
        match self {
            Self::Postgres(p) => sqlx::query(
                r#"
                SELECT raw_json
                FROM frontier_events
                WHERE vfr_id = $1
                  AND seq > $2
                  AND ($3::text IS NULL OR kind = $3)
                  AND ($4::text IS NULL OR target_id = $4)
                ORDER BY seq ASC
                LIMIT $5
                "#,
            )
            .bind(vfr_id)
            .bind(start_seq)
            .bind(kind)
            .bind(target)
            .bind(limit)
            .fetch_all(p)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|row| {
                row.try_get::<Value, _>("raw_json")
                    .map_err(|e| e.to_string())
            })
            .collect(),
            Self::Sqlite(p) => {
                let rows: Vec<String> = sqlx::query_scalar(
                    r#"
                    SELECT raw_json
                    FROM frontier_events
                    WHERE vfr_id = ?
                      AND seq > ?
                      AND (? IS NULL OR kind = ?)
                      AND (? IS NULL OR target_id = ?)
                    ORDER BY seq ASC
                    LIMIT ?
                    "#,
                )
                .bind(vfr_id)
                .bind(start_seq)
                .bind(kind)
                .bind(kind)
                .bind(target)
                .bind(target)
                .bind(limit)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string())?;
                rows.into_iter()
                    .map(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
                    .collect()
            }
        }
    }
}

pub const POSTGRES_EVENT_FIRST_SCHEMA: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS frontiers (
        vfr_id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        owner_actor_id TEXT NOT NULL,
        owner_pubkey TEXT NOT NULL,
        latest_snapshot_hash TEXT NOT NULL,
        latest_event_log_hash TEXT NOT NULL,
        schema_version TEXT NOT NULL,
        source_commit_at TIMESTAMPTZ NOT NULL,
        findings_count BIGINT NOT NULL DEFAULT 0,
        events_count BIGINT NOT NULL DEFAULT 0,
        sources_count BIGINT NOT NULL DEFAULT 0,
        evidence_atoms_count BIGINT NOT NULL DEFAULT 0,
        condition_records_count BIGINT NOT NULL DEFAULT 0,
        materialized_snapshot_json JSONB NOT NULL,
        authority_mode TEXT NOT NULL,
        inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
        updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )"#,
    "CREATE INDEX IF NOT EXISTS idx_frontiers_source_commit_at ON frontiers (source_commit_at DESC)",
    r#"CREATE TABLE IF NOT EXISTS frontier_events (
        vfr_id TEXT NOT NULL REFERENCES frontiers(vfr_id) ON DELETE CASCADE,
        seq BIGINT NOT NULL,
        event_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        target_type TEXT NOT NULL,
        target_id TEXT NOT NULL,
        actor_id TEXT NOT NULL,
        event_timestamp TIMESTAMPTZ NOT NULL,
        raw_json JSONB NOT NULL,
        inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
        PRIMARY KEY (vfr_id, seq),
        UNIQUE (vfr_id, event_id)
    )"#,
    "CREATE INDEX IF NOT EXISTS idx_frontier_events_cursor ON frontier_events (vfr_id, seq)",
    "CREATE INDEX IF NOT EXISTS idx_frontier_events_kind ON frontier_events (vfr_id, kind, seq)",
    "CREATE INDEX IF NOT EXISTS idx_frontier_events_target ON frontier_events (vfr_id, target_id, seq)",
    r#"CREATE TABLE IF NOT EXISTS frontier_objects (
        vfr_id TEXT NOT NULL REFERENCES frontiers(vfr_id) ON DELETE CASCADE,
        object_type TEXT NOT NULL,
        object_id TEXT NOT NULL,
        seq BIGINT NOT NULL DEFAULT 0,
        target_id TEXT,
        raw_json JSONB NOT NULL,
        inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
        PRIMARY KEY (vfr_id, object_type, object_id)
    )"#,
    "CREATE INDEX IF NOT EXISTS idx_frontier_objects_type ON frontier_objects (vfr_id, object_type)",
    "CREATE INDEX IF NOT EXISTS idx_frontier_objects_target ON frontier_objects (vfr_id, target_id)",
    // Producer index: signer extracted at promote for cross-frontier
    // per-key queries.
    "ALTER TABLE frontier_objects ADD COLUMN IF NOT EXISTS signer_pubkey TEXT",
    // Git ingestion (ADR 0001 / HUB.md: the hub is an index over git-replayed
    // state). One row per frontier whose index is derived from a git remote.
    // Source identity is operator configuration; the remaining columns are the
    // ingestor's cursor + last error for status surfaces.
    r#"CREATE TABLE IF NOT EXISTS frontier_git_remotes (
        vfr_id TEXT PRIMARY KEY,
        git_remote TEXT NOT NULL,
        git_ref TEXT NOT NULL DEFAULT 'main',
        last_ingested_commit TEXT,
        last_ingested_at TIMESTAMPTZ,
        ingest_error TEXT,
        inserted_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )"#,
    // Full-text search over frontier objects (the /search backend). A
    // stored generated tsvector over the first 8 KiB of the raw JSON —
    // enough to cover ids, assertion text, DOIs — plus a GIN index so
    // websearch_to_tsquery ranking stays sub-linear as the corpus grows.
    // Applied through the same opportunistic privileged-DDL path as the
    // rest of this schema (least-privilege roles skip it; the privileged
    // migration job applies it).
    "ALTER TABLE frontier_objects ADD COLUMN IF NOT EXISTS search_text tsvector GENERATED ALWAYS AS (to_tsvector('english', left(raw_json::text, 8192))) STORED",
    "CREATE INDEX IF NOT EXISTS idx_frontier_objects_fts ON frontier_objects USING GIN (search_text)",
];

pub async fn ensure_postgres_event_first_schema(pool: &PgPool) -> Result<(), String> {
    for stmt in POSTGRES_EVENT_FIRST_SCHEMA {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| format!("postgres event-first schema migration: {e}"))?;
    }
    Ok(())
}

/// SQLite hub schema. Auto-applied at startup; safe to call repeatedly
/// (`IF NOT EXISTS` everywhere). The shape mirrors the Postgres schema
/// in `docs/HUB.md`: BIGSERIAL → INTEGER PRIMARY KEY AUTOINCREMENT,
/// TIMESTAMPTZ → TEXT (RFC3339), JSONB → TEXT.
pub async fn ensure_sqlite_schema(pool: &SqlitePool) -> Result<(), String> {
    for stmt in [
        r#"CREATE TABLE IF NOT EXISTS frontiers (
            vfr_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            owner_actor_id TEXT NOT NULL,
            owner_pubkey TEXT NOT NULL,
            latest_snapshot_hash TEXT NOT NULL,
            latest_event_log_hash TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            source_commit_at TEXT NOT NULL,
            findings_count INTEGER NOT NULL DEFAULT 0,
            events_count INTEGER NOT NULL DEFAULT 0,
            sources_count INTEGER NOT NULL DEFAULT 0,
            evidence_atoms_count INTEGER NOT NULL DEFAULT 0,
            condition_records_count INTEGER NOT NULL DEFAULT 0,
            materialized_snapshot_json TEXT NOT NULL,
            authority_mode TEXT NOT NULL,
            inserted_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_frontiers_source_commit_at ON frontiers (source_commit_at DESC)",
        r#"CREATE TABLE IF NOT EXISTS frontier_events (
            vfr_id TEXT NOT NULL,
            seq INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            event_timestamp TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            inserted_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (vfr_id, seq),
            UNIQUE (vfr_id, event_id)
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_frontier_events_cursor ON frontier_events (vfr_id, seq)",
        "CREATE INDEX IF NOT EXISTS idx_frontier_events_kind ON frontier_events (vfr_id, kind, seq)",
        "CREATE INDEX IF NOT EXISTS idx_frontier_events_target ON frontier_events (vfr_id, target_id, seq)",
        r#"CREATE TABLE IF NOT EXISTS frontier_objects (
            vfr_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            seq INTEGER NOT NULL DEFAULT 0,
            target_id TEXT,
            raw_json TEXT NOT NULL,
            signer_pubkey TEXT,
            inserted_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (vfr_id, object_type, object_id)
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_frontier_objects_type ON frontier_objects (vfr_id, object_type)",
        "CREATE INDEX IF NOT EXISTS idx_frontier_objects_target ON frontier_objects (vfr_id, target_id)",
        // Git ingestion source cursor (mirror of the Postgres table).
        r#"CREATE TABLE IF NOT EXISTS frontier_git_remotes (
            vfr_id TEXT PRIMARY KEY,
            git_remote TEXT NOT NULL,
            git_ref TEXT NOT NULL DEFAULT 'main',
            last_ingested_commit TEXT,
            last_ingested_at TEXT,
            ingest_error TEXT,
            inserted_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"#,
    ] {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| format!("sqlite schema migration: {e}"))?;
    }
    Ok(())
}

/// The array keys whose contents live in the `frontier_objects` projection,
/// not the stored skeleton. `frontier_skeleton` empties them and
/// `merge_projected_objects` rebuilds them on read.
const PROJECTED_ARRAY_KEYS: [&str; 11] = [
    "findings",
    "sources",
    "evidence_atoms",
    "condition_records",
    "actors",
    "artifacts",
    "proposals",
    "released_diff_packs",
    // The trust arrays: projected so the record pages can fetch them granularly
    // instead of pulling the whole snapshot to render the verification web.
    "verifier_attachments",
    "statement_attestations",
    "statement_registrations",
];

fn frontier_skeleton(snapshot: &Value) -> Value {
    let mut skeleton = snapshot.clone();
    if let Value::Object(map) = &mut skeleton {
        for array_key in PROJECTED_ARRAY_KEYS {
            map.insert(array_key.to_string(), Value::Array(Vec::new()));
        }
    }
    skeleton
}

fn projection_array_key(object_type: &str) -> Option<&'static str> {
    match object_type {
        "finding" => Some("findings"),
        "source" => Some("sources"),
        "evidence_atom" => Some("evidence_atoms"),
        "condition_record" => Some("condition_records"),
        "actor" => Some("actors"),
        "artifact" => Some("artifacts"),
        "proposal" => Some("proposals"),
        "diff_pack" => Some("released_diff_packs"),
        "verifier_attachment" => Some("verifier_attachments"),
        "statement_attestation" => Some("statement_attestations"),
        "statement_registration" => Some("statement_registrations"),
        _ => None,
    }
}

fn merge_projected_objects(snapshot: &mut Value, objects: Vec<(String, i64, Value)>) {
    let Some(map) = snapshot.as_object_mut() else {
        return;
    };
    // Only rebuild a type's array when the projection actually has rows for it.
    // A frontier promoted before a type was projected has no such rows, so its
    // skeleton-held array is left intact — which makes adding a newly-projected
    // type a single safe deploy with no re-projection ordering dependency.
    let mut present: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    for (object_type, _seq, _) in &objects {
        if let Some(key) = projection_array_key(object_type) {
            present.insert(key);
        }
    }
    for array_key in &present {
        map.insert((*array_key).to_string(), Value::Array(Vec::new()));
    }
    for (object_type, _seq, raw_json) in objects {
        let Some(array_key) = projection_array_key(&object_type) else {
            continue;
        };
        if let Some(Value::Array(values)) = map.get_mut(array_key) {
            values.push(raw_json);
        }
    }
}

fn collect_frontier_objects(snapshot: &Value) -> Vec<FrontierObjectRow> {
    let mut out = Vec::new();
    collect_array_objects(snapshot, "findings", "finding", &mut out);
    collect_array_objects(snapshot, "sources", "source", &mut out);
    collect_array_objects(snapshot, "evidence_atoms", "evidence_atom", &mut out);
    collect_array_objects(snapshot, "condition_records", "condition_record", &mut out);
    collect_array_objects(snapshot, "actors", "actor", &mut out);
    collect_array_objects(snapshot, "artifacts", "artifact", &mut out);
    collect_array_objects(snapshot, "proposals", "proposal", &mut out);
    collect_array_objects(snapshot, "released_diff_packs", "diff_pack", &mut out);
    // The trust arrays the record pages render (verification web, attestation
    // cards). Projecting them lets a page fetch GET /objects/{type} instead of
    // the whole snapshot. See PROJECTED_ARRAY_KEYS / merge_projected_objects.
    collect_array_objects(
        snapshot,
        "verifier_attachments",
        "verifier_attachment",
        &mut out,
    );
    collect_array_objects(
        snapshot,
        "statement_attestations",
        "statement_attestation",
        &mut out,
    );
    collect_array_objects(
        snapshot,
        "statement_registrations",
        "statement_registration",
        &mut out,
    );

    if let Some(findings) = snapshot.get("findings").and_then(Value::as_array) {
        for finding in findings {
            let source_id = finding
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            if let Some(links) = finding.get("links").and_then(Value::as_array) {
                for (idx, link) in links.iter().enumerate() {
                    let target_id = link
                        .get("target")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    out.push(FrontierObjectRow {
                        object_type: "link".to_string(),
                        object_id: format!("{source_id}:link:{idx}"),
                        seq: idx as i64,
                        target_id,
                        raw_json: json!({
                            "source": source_id,
                            "link": link,
                        }),
                        signer_pubkey: None,
                    });
                }
            }
        }
    }
    // Producer index: derive the signing/authoring key. Attempts,
    // transfers, and endorsements carry signer_pubkey_hex directly.
    // Findings carry no pubkey — signatures live on EVENTS, keyed by
    // actor id, with pubkeys in the snapshot's actor table — so resolve
    // finding -> asserting/accepting event actor -> registered pubkey.
    let actor_pubkeys: std::collections::HashMap<String, String> = snapshot
        .get("actors")
        .and_then(Value::as_array)
        .map(|actors| {
            actors
                .iter()
                .filter_map(|a| {
                    Some((
                        a.get("id")?.as_str()?.to_string(),
                        a.get("public_key")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let finding_actor: std::collections::HashMap<String, String> = snapshot
        .get("events")
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .filter(|e| {
                    matches!(
                        e.get("kind").and_then(Value::as_str),
                        Some("finding.asserted") | Some("finding.reviewed")
                    )
                })
                .filter_map(|e| {
                    Some((
                        e.get("target")?.get("id")?.as_str()?.to_string(),
                        e.get("actor")?.get("id")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    for row in &mut out {
        row.signer_pubkey = match row.object_type.as_str() {
            "attempt" | "transfer" | "endorsement" => row
                .raw_json
                .get("signer_pubkey_hex")
                .and_then(Value::as_str)
                .map(str::to_string),
            "finding" => finding_actor
                .get(&row.object_id)
                .and_then(|actor| actor_pubkeys.get(actor))
                .cloned(),
            _ => None,
        };
    }
    out
}
fn collect_array_objects(
    snapshot: &Value,
    array_key: &str,
    object_type: &str,
    out: &mut Vec<FrontierObjectRow>,
) {
    if let Some(items) = snapshot.get(array_key).and_then(Value::as_array) {
        for (idx, item) in items.iter().enumerate() {
            let object_id = item
                .get("id")
                .or_else(|| item.get("pack_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{object_type}:{idx}"));
            let target_id = item
                .get("target")
                .and_then(|v| v.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            out.push(FrontierObjectRow {
                object_type: object_type.to_string(),
                object_id,
                seq: idx as i64,
                target_id,
                raw_json: item.clone(),
                signer_pubkey: None,
            });
        }
    }
}

/// Holds the ingest advisory lock for the sweep's lifetime. The lock is
/// transaction-scoped: dropping the guard rolls the transaction back,
/// which releases the lock — pooled-connection session state can never
/// leak it.
pub struct IngestLockGuard {
    _tx: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
}

/// The Postgres FTS search query (`search_objects`). Bind order:
/// $1 = object_type, $2 = the raw user query (websearch grammar — total,
/// never a syntax error), $3 = limit, $4 = offset. Rank-ordered with a
/// stable `(vfr_id, seq)` tiebreak so pagination never shuffles.
pub(crate) const PG_FTS_SEARCH_SQL: &str = "SELECT o.vfr_id, o.raw_json::text \
     FROM frontier_objects o \
     WHERE o.object_type = $1 AND o.search_text @@ websearch_to_tsquery('english', $2) \
     ORDER BY ts_rank(o.search_text, websearch_to_tsquery('english', $2)) DESC, o.vfr_id, o.seq \
     LIMIT $3 OFFSET $4";

/// The matching total, counted over the SAME predicate as the page query.
pub(crate) const PG_FTS_COUNT_SQL: &str = "SELECT COUNT(*)::bigint \
     FROM frontier_objects o \
     WHERE o.object_type = $1 AND o.search_text @@ websearch_to_tsquery('english', $2)";

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use tempfile::NamedTempFile;

    async fn sqlite_db() -> HubDb {
        let file = NamedTempFile::new().expect("temp sqlite");
        let url = format!("sqlite://{}", file.path().display());
        let opts = SqliteConnectOptions::from_str(&url)
            .expect("sqlite opts")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("sqlite connect");
        ensure_sqlite_schema(&pool).await.expect("schema");
        // Keep the temp file alive for the duration of this process by
        // intentionally leaking it inside the test helper.
        std::mem::forget(file);
        HubDb::Sqlite(pool)
    }

    #[test]
    fn trust_arrays_round_trip_through_projection() {
        // A snapshot carrying the trust arrays: after promote (the skeleton
        // holds none of the projected types) and read (merge the projected
        // rows back), the reconstruction must equal the original on those
        // arrays — the property the record pages now depend on for granular
        // /objects fetches.
        let snapshot = json!({
            "frontier_id": "vfr_demo",
            "findings": [{"id": "vf_1"}],
            "verifier_attachments": [
                {"id": "vva_1", "target": {"id": "vf_1"}, "outcome": "pass"},
                {"id": "vva_2", "target": {"id": "vf_1"}, "outcome": "pass"}
            ],
            "statement_attestations": [{"id": "vsa_1", "target": "vf_1", "verdict": "faithful"}],
            "statement_registrations": [{"statement_hash": "sha256:abc", "informal_ref": "erdos:1"}],
            "released_diff_packs": [{
                "pack_id": "vsd_1234",
                "frontier_id": "vfr_demo",
                "summary": "one replayed pack",
                "aggregate_kind": "finding_set",
                "released_at": "2026-07-14T00:00:00Z",
                "released_event_id": "vev_pack"
            }]
        });

        let objects = collect_frontier_objects(&snapshot);
        for t in [
            "verifier_attachment",
            "statement_attestation",
            "statement_registration",
            "diff_pack",
        ] {
            assert!(
                objects.iter().any(|o| o.object_type == t),
                "expected projected object_type {t}"
            );
        }

        let mut reconstructed = frontier_skeleton(&snapshot);
        let rows: Vec<(String, i64, Value)> = objects
            .iter()
            .map(|o| (o.object_type.clone(), o.seq, o.raw_json.clone()))
            .collect();
        merge_projected_objects(&mut reconstructed, rows);

        assert_eq!(
            reconstructed["verifier_attachments"],
            snapshot["verifier_attachments"]
        );
        assert_eq!(
            reconstructed["statement_attestations"],
            snapshot["statement_attestations"]
        );
        assert_eq!(
            reconstructed["statement_registrations"],
            snapshot["statement_registrations"]
        );
        assert_eq!(
            reconstructed["released_diff_packs"],
            snapshot["released_diff_packs"]
        );
        assert_eq!(reconstructed["findings"], snapshot["findings"]);
    }

    #[tokio::test]
    async fn diff_pack_lookup_uses_git_projection_without_transport_tables() {
        let db = sqlite_db().await;
        let HubDb::Sqlite(pool) = &db else {
            unreachable!()
        };
        sqlx::query(
            "INSERT INTO frontiers (vfr_id, name, owner_actor_id, owner_pubkey, \
             latest_snapshot_hash, latest_event_log_hash, schema_version, \
             source_commit_at, materialized_snapshot_json, authority_mode) \
             VALUES ('vfr_pack', 'pack frontier', 'reviewer:test', '00', 'h', 'h', 'v1', \
             '2026-07-14T00:00:00Z', '{}', 'git_ingested')",
        )
        .execute(pool)
        .await
        .expect("insert frontier");
        let record = json!({
            "pack_id": "vsd_projected",
            "frontier_id": "vfr_pack",
            "summary": "projected from replay",
            "aggregate_kind": "finding_set",
            "released_at": "2026-07-14T00:00:00Z",
            "released_event_id": "vev_pack"
        });
        sqlx::query(
            "INSERT INTO frontier_objects \
             (vfr_id, object_type, object_id, seq, raw_json) \
             VALUES ('vfr_pack', 'diff_pack', 'vsd_projected', 0, ?)",
        )
        .bind(record.to_string())
        .execute(pool)
        .await
        .expect("insert projected pack");

        assert_eq!(
            db.get_diff_pack("vsd_projected").await.expect("lookup"),
            Some(record)
        );
        let obsolete_tables: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' \
             AND name IN ('registry_entries', 'registry_diff_packs', 'frontier_snapshots', \
                          'frontier_publish_audit', 'frontier_owner_rotations', \
                          'frontier_maintainers', 'frontier_deprecations', \
                          'frontier_revocations')",
        )
        .fetch_one(pool)
        .await
        .expect("schema query");
        assert_eq!(obsolete_tables, 0);
    }

    #[tokio::test]
    async fn source_catalog_prunes_removed_projection_rows() {
        let db = sqlite_db().await;
        let keep = "vfr_001f148c07eebecb";
        let drop = "vfr_496956067dc5ad79";
        db.upsert_git_source(keep, "https://example.com/keep", "main")
            .await
            .expect("keep source");
        db.upsert_git_source(drop, "https://example.com/drop", "main")
            .await
            .expect("drop source");
        let HubDb::Sqlite(pool) = &db else {
            unreachable!()
        };
        sqlx::query(
            "INSERT INTO frontiers (vfr_id, name, owner_actor_id, owner_pubkey, \
             latest_snapshot_hash, latest_event_log_hash, schema_version, \
             source_commit_at, materialized_snapshot_json, authority_mode) \
             VALUES (?, 'drop', 'reviewer:test', '00', 'h', 'h', 'v1', \
             '2026-07-14T00:00:00Z', '{}', 'git_ingested')",
        )
        .bind(drop)
        .execute(pool)
        .await
        .expect("drop projection");
        sqlx::query(
            "INSERT INTO frontier_objects (vfr_id, object_type, object_id, seq, raw_json) \
             VALUES (?, 'finding', 'vf_drop', 0, '{}')",
        )
        .bind(drop)
        .execute(pool)
        .await
        .expect("drop object");

        assert_eq!(db.retain_git_sources(&[keep.to_string()]).await.unwrap(), 1);
        assert_eq!(db.git_ingest_targets().await.unwrap().len(), 1);
        assert!(db.get_index_entry(drop).await.unwrap().is_none());
        let dropped_objects: i64 =
            sqlx::query_scalar("SELECT count(*) FROM frontier_objects WHERE vfr_id = ?")
                .bind(drop)
                .fetch_one(pool)
                .await
                .expect("object count");
        assert_eq!(dropped_objects, 0);
    }

    #[test]
    fn merge_keeps_skeleton_arrays_when_a_type_has_no_projected_rows() {
        // Deploy safety: a frontier promoted before the trust arrays were
        // projected holds them in its stored skeleton and has no projected
        // rows for them. The conditional merge must leave them intact rather
        // than blanking them — otherwise the live trust web would vanish the
        // instant this change deploys, before any re-ingest.
        let mut stored_skeleton = json!({
            "findings": [],
            "verifier_attachments": [{"id": "vva_old", "outcome": "pass"}],
            "statement_attestations": [{"id": "vsa_old", "verdict": "faithful"}]
        });
        let rows = vec![("finding".to_string(), 0i64, json!({"id": "vf_1"}))];
        merge_projected_objects(&mut stored_skeleton, rows);

        assert_eq!(stored_skeleton["findings"], json!([{"id": "vf_1"}]));
        assert_eq!(
            stored_skeleton["verifier_attachments"],
            json!([{"id": "vva_old", "outcome": "pass"}])
        );
        assert_eq!(
            stored_skeleton["statement_attestations"],
            json!([{"id": "vsa_old", "verdict": "faithful"}])
        );
    }

    /// The Postgres search lane is env-gated out of unit tests (no PG in
    /// the harness), so pin the query-builder strings instead: real FTS
    /// (websearch grammar + rank ordering), the stable pagination tiebreak,
    /// and a count over the SAME predicate.
    #[test]
    fn postgres_fts_query_shape() {
        assert!(PG_FTS_SEARCH_SQL.contains("websearch_to_tsquery('english', $2)"));
        assert!(PG_FTS_SEARCH_SQL.contains("o.search_text @@"));
        assert!(PG_FTS_SEARCH_SQL.contains("ts_rank"));
        assert!(
            PG_FTS_SEARCH_SQL.contains("o.vfr_id, o.seq"),
            "stable tiebreak"
        );
        assert!(PG_FTS_SEARCH_SQL.contains("LIMIT $3 OFFSET $4"));
        assert!(PG_FTS_COUNT_SQL.starts_with("SELECT COUNT(*)::bigint"));
        assert!(PG_FTS_COUNT_SQL.contains("o.search_text @@ websearch_to_tsquery('english', $2)"));
        // The migration that backs the query, pinned from the schema DDL.
        let ddl = POSTGRES_EVENT_FIRST_SCHEMA.join("\n");
        assert!(ddl.contains(
            "ALTER TABLE frontier_objects ADD COLUMN IF NOT EXISTS search_text tsvector"
        ));
        assert!(ddl.contains("to_tsvector('english', left(raw_json::text, 8192))"));
        assert!(ddl.contains(
            "CREATE INDEX IF NOT EXISTS idx_frontier_objects_fts ON frontier_objects USING GIN (search_text)"
        ));
    }

    #[test]
    fn postgres_schema_bootstraps_projection_without_registry_transport() {
        let frontiers = POSTGRES_EVENT_FIRST_SCHEMA
            .iter()
            .position(|stmt| stmt.contains("CREATE TABLE IF NOT EXISTS frontiers"))
            .expect("frontier projection table DDL");

        assert_eq!(frontiers, 0, "the verified projection is the base table");
        for retired in [
            "registry_entries",
            "frontier_publish_audit",
            "registry_diff_packs",
            "frontier_snapshots",
            "frontier_owner_rotations",
            "frontier_maintainers",
            "frontier_deprecations",
            "frontier_revocations",
        ] {
            assert!(
                POSTGRES_EVENT_FIRST_SCHEMA
                    .iter()
                    .all(|stmt| !stmt.contains(retired)),
                "fresh Postgres must not recreate retired table {retired}"
            );
        }
    }

    /// SQLite search keeps the LIKE lane but gains the additive
    /// pagination contract: `(rows, total)` with total counted over the
    /// same predicate, honoring limit/offset.
    #[tokio::test]
    async fn sqlite_search_returns_rows_and_total() {
        let db = sqlite_db().await;
        let HubDb::Sqlite(pool) = &db else {
            unreachable!()
        };
        sqlx::query(
            "INSERT INTO frontiers (vfr_id, name, owner_actor_id, owner_pubkey, \
             latest_snapshot_hash, latest_event_log_hash, schema_version, \
             source_commit_at, materialized_snapshot_json, authority_mode) \
             VALUES ('vfr_s1', 'sidon', 'reviewer:test', '00', 'h', 'h', 'v1', \
             '2026-07-01T00:00:00Z', '{}', 'git_ingested')",
        )
        .execute(pool)
        .await
        .expect("insert frontier");
        for (i, text) in [
            "a sidon set of size 33",
            "a sidon set of size 34",
            "unrelated claim",
        ]
        .iter()
        .enumerate()
        {
            sqlx::query(
                "INSERT INTO frontier_objects (vfr_id, object_type, object_id, seq, raw_json) \
                 VALUES ('vfr_s1', 'finding', ?, ?, ?)",
            )
            .bind(format!("vf_{i}"))
            .bind(i as i64)
            .bind(json!({"id": format!("vf_{i}"), "assertion": {"text": text}}).to_string())
            .execute(pool)
            .await
            .expect("insert object");
        }

        // Page 1 of 2: one row back, total counts both matches.
        let (rows, total) = db
            .search_objects("sidon", "finding", 1, 0)
            .await
            .expect("search");
        assert_eq!(rows.len(), 1);
        assert_eq!(total, 2);
        assert_eq!(rows[0]["vfr_id"], json!("vfr_s1"));
        assert!(
            rows[0]["object"]["assertion"]["text"]
                .as_str()
                .unwrap()
                .contains("sidon")
        );
        // Page 2: the offset walks forward under the same total.
        let (rows2, total2) = db
            .search_objects("sidon", "finding", 1, 1)
            .await
            .expect("search page 2");
        assert_eq!(rows2.len(), 1);
        assert_eq!(total2, 2);
        assert_ne!(rows[0]["object"]["id"], rows2[0]["object"]["id"]);
        // No matches: empty page, zero total.
        let (none, zero) = db
            .search_objects("nomatch", "finding", 10, 0)
            .await
            .expect("search none");
        assert!(none.is_empty());
        assert_eq!(zero, 0);
    }
}
