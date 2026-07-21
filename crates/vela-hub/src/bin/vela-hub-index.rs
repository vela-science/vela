//! One-shot Git-to-SQL projection refresh.
//!
//! This is the deployment primitive for hosted readers: Git remains the
//! canonical byte source, Vela verifies and replays each configured frontier,
//! and Postgres receives only a disposable read projection. No HTTP service or
//! mutation surface is started.

use std::env;

use sqlx::postgres::PgPoolOptions;
use vela_hub::{
    db::{HubDb, ensure_postgres_event_first_schema},
    git_ingest::{GitIngestConfig, load_source_catalog, run_once},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::from_path(
        std::path::PathBuf::from(env::var("HOME").unwrap_or_default())
            .join(".vela")
            .join("hub.env"),
    );
    let database_url = env::var("VELA_HUB_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "set VELA_HUB_DATABASE_URL")?;
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        return Err("vela-hub-index requires Postgres".into());
    }

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    if let Err(error) = ensure_postgres_event_first_schema(&pool).await {
        eprintln!("schema migration skipped: {error}");
    }
    let db = HubDb::Postgres(pool);
    if !db.schema_present().await? {
        return Err("projection schema is absent; run once with its owner role".into());
    }

    let catalog = load_source_catalog()?;
    for source in &catalog.sources {
        db.upsert_git_source(&source.vfr_id, &source.git_remote, &source.git_ref)
            .await?;
    }
    let configured: Vec<String> = catalog
        .sources
        .iter()
        .map(|source| source.vfr_id.clone())
        .collect();
    let pruned = db.retain_git_sources(&configured).await?;
    let promoted = run_once(&db, &GitIngestConfig::from_env()).await?;
    let health = db.ingest_health().await?;
    let failures: Vec<&str> = health
        .iter()
        .filter_map(|(vfr_id, failing, _)| failing.then_some(vfr_id.as_str()))
        .collect();
    if !failures.is_empty() {
        return Err(format!("strict projection failed for {}", failures.join(", ")).into());
    }

    println!(
        "projection refreshed: {} configured, {promoted} promoted, {pruned} pruned",
        configured.len()
    );
    Ok(())
}
