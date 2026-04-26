//! v0.21: backend abstraction so the hub can run on Postgres (production)
//! or SQLite (self-hosted, no external dependencies). The enum stays
//! small — five methods cover everything the route handlers need. Each
//! backend handles its own placeholder syntax (`$1` vs `?`) and raw_json
//! storage (`JSONB` vs `TEXT`).
//!
//! Doctrine: the SQL surface stays minimal. If the enum grows past ~10
//! methods, the right move is to re-think whether the hub should be a
//! sqlx-direct service or move to an ORM.

use serde_json::Value;
use sqlx::{PgPool, SqlitePool};
use vela_protocol::registry::RegistryEntry;

const LATEST_PER_VFR_SQL: &str = r#"
SELECT raw_json FROM registry_entries r
WHERE r.signed_publish_at = (
    SELECT MAX(signed_publish_at) FROM registry_entries
    WHERE vfr_id = r.vfr_id
)
ORDER BY r.signed_publish_at DESC
"#;

/// Backend-agnostic hub database handle. Variant is picked at startup
/// based on the `VELA_HUB_DATABASE_URL` prefix.
#[derive(Clone)]
pub enum HubDb {
    Postgres(PgPool),
    Sqlite(SqlitePool),
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
                "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'registry_entries')",
            )
            .fetch_one(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='registry_entries'",
            )
            .fetch_one(p)
            .await
            .map(|n| n > 0)
            .map_err(|e| e.to_string()),
        }
    }

    pub async fn list_latest_entries(&self) -> Result<Vec<Value>, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, Value>(LATEST_PER_VFR_SQL)
                .fetch_all(p)
                .await
                .map_err(|e| e.to_string()),
            Self::Sqlite(p) => {
                let rows: Vec<String> = sqlx::query_scalar(LATEST_PER_VFR_SQL)
                    .fetch_all(p)
                    .await
                    .map_err(|e| e.to_string())?;
                rows.into_iter()
                    .map(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
                    .collect()
            }
        }
    }

    pub async fn get_entry(&self, vfr_id: &str) -> Result<Option<Value>, String> {
        match self {
            Self::Postgres(p) => sqlx::query_scalar::<_, Value>(
                r#"
                SELECT raw_json FROM registry_entries
                WHERE vfr_id = $1
                ORDER BY signed_publish_at DESC
                LIMIT 1
                "#,
            )
            .bind(vfr_id)
            .fetch_optional(p)
            .await
            .map_err(|e| e.to_string()),
            Self::Sqlite(p) => {
                let row: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT raw_json FROM registry_entries
                    WHERE vfr_id = ?
                    ORDER BY signed_publish_at DESC
                    LIMIT 1
                    "#,
                )
                .bind(vfr_id)
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

    /// Returns true on fresh insert, false on duplicate.
    pub async fn insert_entry(
        &self,
        entry: &RegistryEntry,
        raw_json: &Value,
    ) -> Result<bool, String> {
        match self {
            Self::Postgres(p) => {
                let inserted = sqlx::query_scalar::<_, String>(
                    r#"
                    INSERT INTO registry_entries (
                      vfr_id, schema, name, owner_actor_id, owner_pubkey,
                      latest_snapshot_hash, latest_event_log_hash, network_locator,
                      signed_publish_at, signature, raw_json
                    )
                    VALUES (
                      $1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10, $11
                    )
                    ON CONFLICT (vfr_id, signature) DO NOTHING
                    RETURNING vfr_id
                    "#,
                )
                .bind(&entry.vfr_id)
                .bind(&entry.schema)
                .bind(&entry.name)
                .bind(&entry.owner_actor_id)
                .bind(&entry.owner_pubkey)
                .bind(&entry.latest_snapshot_hash)
                .bind(&entry.latest_event_log_hash)
                .bind(&entry.network_locator)
                .bind(&entry.signed_publish_at)
                .bind(&entry.signature)
                .bind(raw_json)
                .fetch_optional(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(inserted.is_some())
            }
            Self::Sqlite(p) => {
                let raw_json_str = serde_json::to_string(raw_json)
                    .map_err(|e| format!("serialize raw_json: {e}"))?;
                let result = sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO registry_entries (
                      vfr_id, schema, name, owner_actor_id, owner_pubkey,
                      latest_snapshot_hash, latest_event_log_hash, network_locator,
                      signed_publish_at, signature, raw_json
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&entry.vfr_id)
                .bind(&entry.schema)
                .bind(&entry.name)
                .bind(&entry.owner_actor_id)
                .bind(&entry.owner_pubkey)
                .bind(&entry.latest_snapshot_hash)
                .bind(&entry.latest_event_log_hash)
                .bind(&entry.network_locator)
                .bind(&entry.signed_publish_at)
                .bind(&entry.signature)
                .bind(&raw_json_str)
                .execute(p)
                .await
                .map_err(|e| e.to_string())?;
                Ok(result.rows_affected() > 0)
            }
        }
    }
}

/// SQLite hub schema. Auto-applied at startup; safe to call repeatedly
/// (`IF NOT EXISTS` everywhere). The shape mirrors the Postgres schema
/// in `docs/HUB.md`: BIGSERIAL → INTEGER PRIMARY KEY AUTOINCREMENT,
/// TIMESTAMPTZ → TEXT (RFC3339), JSONB → TEXT.
pub async fn ensure_sqlite_schema(pool: &SqlitePool) -> Result<(), String> {
    for stmt in [
        r#"CREATE TABLE IF NOT EXISTS registry_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            vfr_id TEXT NOT NULL,
            schema TEXT NOT NULL,
            name TEXT NOT NULL,
            owner_actor_id TEXT NOT NULL,
            owner_pubkey TEXT NOT NULL,
            latest_snapshot_hash TEXT NOT NULL,
            latest_event_log_hash TEXT NOT NULL,
            network_locator TEXT NOT NULL,
            signed_publish_at TEXT NOT NULL,
            signature TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"#,
        "CREATE INDEX IF NOT EXISTS idx_entries_vfr_id ON registry_entries (vfr_id)",
        "CREATE INDEX IF NOT EXISTS idx_entries_signed_publish_at ON registry_entries (signed_publish_at DESC)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_entries_vfr_signature ON registry_entries (vfr_id, signature)",
    ] {
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| format!("sqlite schema migration: {e}"))?;
    }
    Ok(())
}
