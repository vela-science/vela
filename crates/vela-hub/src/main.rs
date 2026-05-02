//! Vela hub: HTTP server over a Postgres-backed registry of signed
//! frontier manifests.
//!
//! Doctrine: the hub is dumb transport. It serves canonical JSON
//! manifests as published; clients verify signatures locally. The hub
//! does not re-canonicalize, does not index by content, does not
//! attempt to interpret findings. If the hub is compromised, signature
//! verification still catches tampering.
//!
//! Writes are accepted from anyone who can produce a valid signature
//! over their own manifest — the signature is the bind, not access
//! control. The hub verifies the signature against the manifest's
//! declared `owner_pubkey` and stores the canonical bytes verbatim.
//!
//! Endpoints:
//!   GET  /entries                   — full registry (latest-publish-wins per vfr_id)
//!   GET  /entries/{vfr_id}          — single entry for a `vfr_…`
//!   POST /entries                   — publish a signed manifest (open, signature-gated)
//!   GET  /healthz                   — liveness
//!   GET  /                          — banner + endpoint list

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tokio::sync::RwLock;

mod db;
use db::{HubDb, ensure_sqlite_schema};
use tower_http::cors::CorsLayer;
use vela_protocol::counterfactual::{
    CounterfactualQuery, answer_counterfactual,
};
use vela_protocol::project::Project;
use vela_protocol::registry::{RegistryEntry as ProtocolEntry, verify_entry};

const HUB_VERSION: &str = env!("CARGO_PKG_VERSION");
const REGISTRY_SCHEMA: &str = "vela.registry.v0.1";

const DEFAULT_PUBLIC_URL: &str = "https://vela-hub.fly.dev";
const DEFAULT_REPO_URL: &str = "https://github.com/vela-science/vela";
const DEFAULT_SITE_URL: &str = "https://vela-site.fly.dev";

/// Cache key: (vfr_id, signed_publish_at). A fresh publish gets a new
/// timestamp, so the key changes and the next read re-fetches.
type FrontierCache = Arc<RwLock<HashMap<(String, String), Arc<Project>>>>;

/// URL strings the hub renders into HTML. Sourced at startup from env
/// vars (`VELA_HUB_PUBLIC_URL`, `VELA_REPO_URL`, `VELA_SITE_URL`) with
/// hardcoded defaults that match the v0.7 deploy. Changing the deploy
/// target is one secret-set away.
#[derive(Clone)]
struct PublicUrls {
    hub: String,
    repo: String,
    site: String,
}

impl PublicUrls {
    fn from_env() -> Self {
        let strip = |s: String| s.trim_end_matches('/').to_string();
        Self {
            hub: strip(
                env::var("VELA_HUB_PUBLIC_URL").unwrap_or_else(|_| DEFAULT_PUBLIC_URL.into()),
            ),
            repo: strip(env::var("VELA_REPO_URL").unwrap_or_else(|_| DEFAULT_REPO_URL.into())),
            site: strip(env::var("VELA_SITE_URL").unwrap_or_else(|_| DEFAULT_SITE_URL.into())),
        }
    }
    fn hub_host(&self) -> &str {
        self.hub
            .trim_start_matches("https://")
            .trim_start_matches("http://")
    }
}

#[derive(Clone)]
struct AppState {
    /// v0.21: backend-agnostic DB handle. Postgres for production
    /// (vela-hub.fly.dev / vela-hub-2.fly.dev), SQLite for self-hosted
    /// laptop runs. Variant chosen at startup from URL prefix.
    db: HubDb,
    /// Frontier cache for the entry detail page. Keyed by
    /// `(vfr_id, signed_publish_at)` so a fresh publish forces a
    /// re-fetch automatically. Bounded loosely; in v0.7 we expect
    /// fewer than a dozen frontiers ever.
    frontier_cache: FrontierCache,
    /// v0.49: stale-on-read cache for DB reads. When the Postgres
    /// backend hiccups (Neon cold-start, network blip, restart), the
    /// hub serves the last-known-good response with an `X-Vela-Stale`
    /// header instead of 5xx-ing. The TTL is short (60 s) so a
    /// long-lived outage still surfaces; but a single failed query
    /// no longer takes down the registry.
    db_cache: DbCache,
    /// Shared reqwest client. Connection pool reuse matters for
    /// repeat fetches against the same locator host.
    http: reqwest::Client,
    /// Public-facing URLs the rendered HTML quotes back to readers.
    /// Configurable via env so the same binary serves any deployment.
    urls: PublicUrls,
}

/// v0.49: tiny stale-on-read cache for DB query results. Keyed by a
/// short string (route + arg). Each entry stores the JSON value, the
/// time it was fetched, and serves stale on any query failure within
/// `DB_CACHE_STALE_WINDOW`.
type DbCache = Arc<RwLock<HashMap<String, DbCacheEntry>>>;

#[derive(Clone)]
struct DbCacheEntry {
    value: Value,
    fetched_at: std::time::Instant,
}

const DB_CACHE_FRESH_TTL: std::time::Duration = std::time::Duration::from_secs(60);
const DB_CACHE_STALE_WINDOW: std::time::Duration = std::time::Duration::from_secs(30 * 60);

async fn db_cache_read(cache: &DbCache, key: &str) -> Option<DbCacheEntry> {
    cache.read().await.get(key).cloned()
}

async fn db_cache_write(cache: &DbCache, key: &str, value: Value) {
    cache.write().await.insert(
        key.to_string(),
        DbCacheEntry {
            value,
            fetched_at: std::time::Instant::now(),
        },
    );
}

// Local RegistryEntry struct removed in v0.21 — db.rs now uses
// vela_protocol::registry::RegistryEntry directly so the publish handler
// and the DB layer agree on the type.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("vela_hub=info,tower_http=info")
            }),
        )
        .init();

    // Load credentials. We read VELA_HUB_DATABASE_URL from env, with
    // ~/.vela/hub.env as a convenience fallback so the dev path "just works"
    // without exporting variables in every shell.
    let _ = dotenvy::from_path(
        std::path::PathBuf::from(env::var("HOME").unwrap_or_default())
            .join(".vela")
            .join("hub.env"),
    );
    let database_url = env::var("VELA_HUB_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "set VELA_HUB_DATABASE_URL (e.g. via ~/.vela/hub.env)")?;

    // v0.21: pick backend by URL prefix.
    //   postgres://… or postgresql://… → production Postgres path
    //   sqlite://…  or sqlite:./…      → self-hosted SQLite path
    //                                     (auto-creates schema if missing)
    let db = if database_url.starts_with("sqlite:") {
        let opts = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;
        ensure_sqlite_schema(&pool)
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        tracing::info!(url = %database_url, "vela-hub using SQLite backend (self-hosted)");
        HubDb::Sqlite(pool)
    } else {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await?;
        let h = HubDb::Postgres(pool);
        // Sanity-check schema presence so we fail fast on a misconfigured DB.
        let table_exists = h
            .schema_present()
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        if !table_exists {
            return Err(
                "registry_entries table not found; run the schema migration before starting the hub"
                    .into(),
            );
        }
        tracing::info!("vela-hub using Postgres backend");
        h
    };

    let http = reqwest::Client::builder()
        .user_agent(concat!("vela-hub/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(8))
        .build()?;
    let urls = PublicUrls::from_env();
    let state = AppState {
        db,
        frontier_cache: Arc::new(RwLock::new(HashMap::new())),
        db_cache: Arc::new(RwLock::new(HashMap::new())),
        http,
        urls,
    };

    let port: u16 = env::var("VELA_HUB_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3849);
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .route("/entries", get(list_entries).post(publish_entry))
        .route("/entries/{vfr_id}", get(get_entry))
        .route("/entries/{vfr_id}/depends-on", get(get_depends_on))
        .route("/entries/{vfr_id}/findings/{vf_id}", get(get_finding))
        .route("/entries/{vfr_id}/proof", get(get_proof_packet))
        .route("/entries/{vfr_id}/proof/download", get(get_proof_packet_download))
        .route("/api/counterfactual/{vfr_id}", axum::routing::post(api_counterfactual))
        .route("/static/tokens.css", get(static_tokens_css))
        .route("/static/workbench.css", get(static_workbench_css))
        .route("/static/site.css", get(static_site_css))
        .route("/static/favicon.svg", get(static_favicon_svg))
        .route("/static/vela-logo-mark.svg", get(static_logo_mark_svg))
        .route(
            "/static/vela-logo-wordmark.svg",
            get(static_logo_wordmark_svg),
        )
        .route("/static/rete.svg", get(static_rete_svg))
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("vela-hub {HUB_VERSION} listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Browsers send `Accept: text/html,…`; CLI clients (curl, reqwest, jq)
/// usually omit the header or send `*/*`. We render HTML only when the
/// client explicitly asks for it.
fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"))
}

fn root_json() -> Value {
    json!({
        "service": "vela-hub",
        "version": HUB_VERSION,
        "doctrine": "Dumb transport for signed registry manifests. The signature is the bind; clients verify locally.",
        "endpoints": [
            "GET  /              — this banner",
            "GET  /healthz       — liveness",
            "GET  /entries       — full registry (latest-publish-wins per vfr_id)",
            "GET  /entries/{vfr_id} — single entry",
            "POST /entries       — publish a signed manifest (open, signature-gated)",
            "POST /api/counterfactual/{vfr_id} — Pearl level 3 counterfactual over a registered frontier",
        ],
    })
}

async fn root(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if wants_html(&headers) {
        Html(render_root_html(&state.urls)).into_response()
    } else {
        Json(root_json()).into_response()
    }
}

async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match state.db.health().await {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true, "db": "reachable"}))),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok": false, "db": "unreachable", "error": e})),
        ),
    }
}

async fn list_entries(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let cache_key = "list_entries";
    let cached = db_cache_read(&state.db_cache, cache_key).await;
    let now = std::time::Instant::now();

    // Fresh cache window — serve straight from memory, skip DB.
    if let Some(entry) = cached.as_ref() {
        if now.duration_since(entry.fetched_at) < DB_CACHE_FRESH_TTL {
            return cached_list_response(&state.urls, &entry.value, &headers, false);
        }
    }

    match state.db.list_latest_entries().await {
        Ok(values) => {
            let payload = json!({"schema": REGISTRY_SCHEMA, "entries": values});
            db_cache_write(&state.db_cache, cache_key, payload.clone()).await;
            if wants_html(&headers) {
                Html(render_entries_html(&state.urls, &values)).into_response()
            } else {
                (StatusCode::OK, Json(payload)).into_response()
            }
        }
        Err(e) => {
            // v0.49: stale-on-read fallback. Serve the last good
            // payload (with X-Vela-Stale) instead of 5xx-ing on a
            // single DB hiccup. Inside the stale window only.
            if let Some(entry) = cached {
                if now.duration_since(entry.fetched_at) < DB_CACHE_STALE_WINDOW {
                    tracing::warn!(
                        "list_entries: db error '{e}', serving stale ({}s old)",
                        now.duration_since(entry.fetched_at).as_secs()
                    );
                    return cached_list_response(&state.urls, &entry.value, &headers, true);
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("query: {e}")})),
            )
                .into_response()
        }
    }
}

fn cached_list_response(
    urls: &PublicUrls,
    payload: &Value,
    headers: &HeaderMap,
    stale: bool,
) -> Response {
    let entries = payload
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut resp = if wants_html(headers) {
        Html(render_entries_html(urls, &entries)).into_response()
    } else {
        (StatusCode::OK, Json(payload.clone())).into_response()
    };
    if stale {
        resp.headers_mut().insert(
            axum::http::header::HeaderName::from_static("x-vela-stale"),
            axum::http::HeaderValue::from_static("1"),
        );
    }
    resp
}

/// Fetch the frontier referenced by an entry's `network_locator`,
/// caching by `(vfr_id, signed_publish_at)`. Returns `None` (logged) on
/// any failure so the entry page degrades to "frontier unavailable"
/// rather than 5xx-ing on a flaky locator.
async fn fetch_frontier_cached(
    state: &AppState,
    vfr_id: &str,
    signed_publish_at: &str,
    locator: &str,
) -> Option<Arc<Project>> {
    let key = (vfr_id.to_string(), signed_publish_at.to_string());
    if let Some(hit) = state.frontier_cache.read().await.get(&key).cloned() {
        return Some(hit);
    }
    if !locator.starts_with("http://") && !locator.starts_with("https://") {
        return None;
    }
    let resp = match state.http.get(locator).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(%vfr_id, %locator, error = %e, "fetch frontier failed");
            return None;
        }
    };
    if !resp.status().is_success() {
        tracing::warn!(%vfr_id, %locator, status = %resp.status(), "fetch frontier non-2xx");
        return None;
    }
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(%vfr_id, error = %e, "read frontier body failed");
            return None;
        }
    };
    let project: Project = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(%vfr_id, error = %e, "parse frontier failed");
            return None;
        }
    };
    let arc = Arc::new(project);
    state.frontier_cache.write().await.insert(key, arc.clone());
    Some(arc)
}

async fn get_entry(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let row = state.db.get_entry(&vfr_id).await;
    match row {
        Ok(Some(value)) => {
            if wants_html(&headers) {
                let signed_at = value
                    .get("signed_publish_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let locator = value
                    .get("network_locator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let frontier = if signed_at.is_empty() || locator.is_empty() {
                    None
                } else {
                    fetch_frontier_cached(&state, &vfr_id, signed_at, locator).await
                };
                Html(render_entry_html(
                    &state.urls,
                    &vfr_id,
                    &value,
                    frontier.as_deref(),
                ))
                .into_response()
            } else {
                (StatusCode::OK, Json(value)).into_response()
            }
        }
        Ok(None) => {
            if wants_html(&headers) {
                (
                    StatusCode::NOT_FOUND,
                    Html(render_not_found_html(&state.urls, &vfr_id)),
                )
                    .into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("{vfr_id} not found")})),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("query: {e}")})),
        )
            .into_response(),
    }
}

/// v0.15: hub-level reverse lookup. Returns the registry entries
/// (latest-publish-wins per vfr_id) whose frontier declares a
/// cross-frontier dependency on `{vfr_id}`. Surfaces "who in the world
/// is referencing my frontier" — closes the bidirectional gap in the
/// cross-frontier composition story.
///
/// Implementation is O(N) over current entries: the hub doesn't store
/// dependency lists in its registry rows; we walk the latest-per-vfr
/// view, fetch each frontier through the existing
/// `fetch_frontier_cached` LRU, and filter. For a hub of N entries this
/// is at most N HTTP fetches on a cold cache; warm cache makes
/// subsequent calls O(N) memory-only. A future optimization would
/// denormalize a `dependent_vfrs` JSONB column at POST time and back
/// this with a SQL `?` lookup.
async fn get_depends_on(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let _ = &headers; // reserved for future HTML rendering
    let rows = match state.db.list_latest_entries().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("query: {e}")})),
            )
                .into_response();
        }
    };

    let mut dependents: Vec<serde_json::Value> = Vec::new();
    for entry in &rows {
        let entry_vfr = entry.get("vfr_id").and_then(|v| v.as_str()).unwrap_or("");
        if entry_vfr == vfr_id {
            continue; // a frontier doesn't depend on itself
        }
        let signed_at = entry
            .get("signed_publish_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let locator = entry
            .get("network_locator")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let Some(project) = fetch_frontier_cached(&state, entry_vfr, signed_at, locator).await
        else {
            // Locator unreachable / parse-failed — entry can't be classified.
            // Skip silently; the per-entry page surfaces the same failure
            // when a user hits it directly.
            continue;
        };
        if project
            .project
            .dependencies
            .iter()
            .any(|d| d.vfr_id.as_deref() == Some(vfr_id.as_str()))
        {
            dependents.push(entry.clone());
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.depends-on.v0.1",
            "target_vfr_id": vfr_id,
            "dependents": dependents,
            "count": dependents.len(),
        })),
    )
        .into_response()
}

/// Single-finding detail page. Fetches the cached frontier (same one
/// the entry detail page uses), looks up the finding by id, renders
/// claim + conditions + evidence + history in workbench finding-pattern.
/// JSON path returns the finding bundle as-is.
async fn get_finding(
    State(state): State<AppState>,
    Path((vfr_id, vf_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    // Find the entry to get the locator.
    let entry = state.db.get_entry(&vfr_id).await;
    let entry = match entry {
        Ok(Some(v)) => v,
        Ok(None) => {
            if wants_html(&headers) {
                return (
                    StatusCode::NOT_FOUND,
                    Html(render_not_found_html(&state.urls, &vfr_id)),
                )
                    .into_response();
            }
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("{vfr_id} not found")})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("query: {e}")})),
            )
                .into_response();
        }
    };

    let signed_at = entry
        .get("signed_publish_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let locator = entry
        .get("network_locator")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let frontier = if signed_at.is_empty() || locator.is_empty() {
        None
    } else {
        fetch_frontier_cached(&state, &vfr_id, signed_at, locator).await
    };

    let Some(project) = frontier else {
        if wants_html(&headers) {
            return Html(render_finding_unavailable_html(
                &state.urls,
                &vfr_id,
                &vf_id,
            ))
            .into_response();
        }
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "frontier file unreachable; pull via the CLI to inspect"})),
        )
            .into_response();
    };

    let Some(bundle) = project.findings.iter().find(|b| b.id == vf_id) else {
        if wants_html(&headers) {
            return (
                StatusCode::NOT_FOUND,
                Html(render_finding_not_found_html(&state.urls, &vfr_id, &vf_id)),
            )
                .into_response();
        }
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("{vf_id} not in {vfr_id}")})),
        )
            .into_response();
    };

    if wants_html(&headers) {
        Html(render_finding_html(&state.urls, &vfr_id, &project, bundle)).into_response()
    } else {
        match serde_json::to_value(bundle) {
            Ok(v) => (StatusCode::OK, Json(v)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("serialize: {e}")})),
            )
                .into_response(),
        }
    }
}

// ─── Proof packet ─────────────────────────────────────────────────────
//
// `vela frontier export --packet` produces a directory of canonical
// proof artifacts (manifest.json + packet.lock.json + proof-trace.json
// + findings/full.json + sources/source-registry.json + ...). The hub
// surfaces that directory inline so a skeptic can see the seam: signer
// hashes, included-files sha256 table, replay status, schema version.
//
// Resolution: env VELA_PROOF_PACKET_DIR points at either
//   (a) a single packet directory containing manifest.json (single-
//       packet demo deploy — handler ignores vfr_id and serves it for
//       every entry), or
//   (b) a directory of packet directories named by vfr_id (multi-
//       packet deploy, future).
// If the env is unset OR the path doesn't resolve, the route renders
// an honest "no packet has been generated for this entry yet" page
// with the CLI invocation that would generate one.

fn resolve_packet_dir(vfr_id: &str) -> Option<std::path::PathBuf> {
    let base = std::env::var("VELA_PROOF_PACKET_DIR").ok()?;
    let base_path = std::path::PathBuf::from(&base);
    if !base_path.is_dir() {
        return None;
    }
    // Multi-packet deploy: prefer ${base}/${vfr_id}.
    let by_id = base_path.join(vfr_id);
    if by_id.join("manifest.json").is_file() {
        return Some(by_id);
    }
    // Single-packet deploy: serve ${base} itself if it has a manifest.
    if base_path.join("manifest.json").is_file() {
        return Some(base_path);
    }
    None
}

fn read_packet_json(dir: &std::path::Path, name: &str) -> Option<Value> {
    let raw = std::fs::read_to_string(dir.join(name)).ok()?;
    serde_json::from_str(&raw).ok()
}

async fn get_proof_packet(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
) -> Response {
    let dir = match resolve_packet_dir(&vfr_id) {
        Some(d) => d,
        None => {
            return Html(render_no_packet_html(&state.urls, &vfr_id)).into_response();
        }
    };
    let manifest = match read_packet_json(&dir, "manifest.json") {
        Some(v) => v,
        None => return Html(render_no_packet_html(&state.urls, &vfr_id)).into_response(),
    };
    let proof_trace = read_packet_json(&dir, "proof-trace.json");
    let lock = read_packet_json(&dir, "packet.lock.json");
    Html(render_proof_packet_html(
        &state.urls,
        &vfr_id,
        &dir,
        &manifest,
        proof_trace.as_ref(),
        lock.as_ref(),
    ))
    .into_response()
}

async fn get_proof_packet_download(
    State(_state): State<AppState>,
    Path(vfr_id): Path<String>,
) -> Response {
    let dir = match resolve_packet_dir(&vfr_id) {
        Some(d) => d,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "no proof packet available for this entry"})),
            )
                .into_response();
        }
    };
    // Build the tar.gz in memory. Packets are a few MB; this is fine.
    let mut buf: Vec<u8> = Vec::new();
    {
        let enc = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        let label = format!("{vfr_id}-proof-packet");
        if let Err(e) = tar.append_dir_all(&label, &dir) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("tar: {e}")})),
            )
                .into_response();
        }
        if let Err(e) = tar.into_inner().and_then(|enc| enc.finish()) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("gz: {e}")})),
            )
                .into_response();
        }
    }
    let filename = format!("{vfr_id}-proof-packet.tar.gz");
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/gzip".to_string()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        buf,
    )
        .into_response()
}

/// v0.45.1: hub-level counterfactual endpoint. Pearl level 3 over a
/// network frontier. Body is the same `CounterfactualQuery` shape the
/// CLI produces; the hub fetches the frontier (cached), runs
/// `answer_counterfactual` byte-for-byte against the in-memory
/// `Project`, and returns the verdict as JSON.
///
/// Doctrine: the hub does not invent answers. It runs the same kernel
/// algorithm a local CLI would run, against the frontier the registry
/// declares. Same input → same output, regardless of whether the query
/// originates from a local repo or a network client.
async fn api_counterfactual(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    Json(query): Json<CounterfactualQuery>,
) -> Response {
    let row = match state.db.get_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("{vfr_id} not found")})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("query: {e}")})),
            )
                .into_response();
        }
    };

    let signed_at = row
        .get("signed_publish_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let locator = row
        .get("network_locator")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if signed_at.is_empty() || locator.is_empty() {
        return (
            StatusCode::FAILED_DEPENDENCY,
            Json(json!({
                "error": "registry entry missing signed_publish_at or network_locator",
                "vfr_id": vfr_id,
            })),
        )
            .into_response();
    }

    let project = match fetch_frontier_cached(&state, &vfr_id, signed_at, locator).await {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "could not fetch frontier from network_locator",
                    "vfr_id": vfr_id,
                    "network_locator": locator,
                })),
            )
                .into_response();
        }
    };

    let verdict = answer_counterfactual(project.as_ref(), &query);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "vfr_id": vfr_id,
            "query": query,
            "verdict": verdict,
        })),
    )
        .into_response()
}

/// Publish a signed manifest. The doctrine here is the entire shape of
/// the hub: anyone can POST, the signature is the bind. We deserialize
/// the body, verify the signature against the declared `owner_pubkey`,
/// and persist the canonical bytes verbatim. The hub stores; clients
/// verify on read. Idempotent on `(vfr_id, signature)` — re-posting the
/// same signed manifest returns 200 without inserting again.
async fn publish_entry(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Two-step deserialize: keep the raw Value so we can store the
    // canonical bytes the publisher signed, and decode the structured
    // shape so we can index it.
    let entry: ProtocolEntry = match serde_json::from_value(body.clone()) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"ok": false, "error": format!("schema: {e}")})),
            );
        }
    };

    match verify_entry(&entry) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"ok": false, "error": "signature does not verify"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"ok": false, "error": format!("verify: {e}")})),
            );
        }
    }

    // Idempotent insert. The UNIQUE (vfr_id, signature) index ensures
    // re-posting an identical signed manifest is a no-op.
    match state.db.insert_entry(&entry, &body).await {
        Ok(true) => (
            StatusCode::CREATED,
            Json(json!({
                "ok": true,
                "duplicate": false,
                "vfr_id": entry.vfr_id,
                "signed_publish_at": entry.signed_publish_at,
            })),
        ),
        Ok(false) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "duplicate": true,
                "vfr_id": entry.vfr_id,
                "signed_publish_at": entry.signed_publish_at,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": format!("db: {e}")})),
        ),
    }
}

// ── HTML rendering ───────────────────────────────────────────────────
//
// The hub renders against the canonical Vela design system. The same
// `tokens.css` and `workbench.css` files that drive `web/index.html`
// are baked into the binary via `include_str!` and served at
// `/static/...` so the marketing site and the hub share one source of
// truth. Hub-specific page styles are kept in a small inline block.

const FONT_LINK: &str = r#"<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter+Tight:wght@400;500;600&family=Source+Serif+4:ital,wght@0,400;0,500;1,400&family=JetBrains+Mono:wght@400;500&display=swap">"#;

// Embedded design-system files. Compiled into the binary so the runtime
// has no external file dependency; touching any of these forces a rebuild.
const TOKENS_CSS: &str = include_str!("../../../web/styles/tokens.css");
const WORKBENCH_CSS: &str = include_str!("../../../web/styles/workbench.css");
const SITE_CSS: &str = include_str!("../../../web/styles/site.css");
const FAVICON_SVG: &str = include_str!("../../../assets/brand/favicon.svg");
const LOGO_MARK_SVG: &str = include_str!("../../../assets/brand/vela-logo-mark.svg");
const LOGO_WORDMARK_SVG: &str = include_str!("../../../assets/brand/vela-logo-wordmark.svg");
const RETE_SVG: &str = include_str!("../../../assets/brand/rete.svg");

// Hub-specific page styles. The frame and tokens come from the shared
// stylesheets above; this block adds the entries table, the manifest
// detail layout, the terminal-paper code block, and the endpoint list.
//
// Visual register: Borrowed Light. Inter Tight as the dominant face;
// EB Garamond reserved for true prose (evidence quotes, annotations);
// JetBrains Mono for IDs / kickers. Cream paper, gold accent, hairlines.
const HUB_STYLES: &str = r#"
/* Entries table — frontier registry */
.fr-table { width: 100%; border-collapse: collapse; margin-top: 8px; }
.fr-table thead th {
  font-family: var(--font-mono); font-size: 10px; font-weight: 500;
  text-transform: uppercase; letter-spacing: 0.18em;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
  text-align: left; padding: 14px 12px; border-bottom: 1px solid var(--rule-2);
}
.fr-table thead th.num { text-align: right; }
.fr-table tbody tr {
  border-bottom: 1px solid var(--rule-1);
  transition: background var(--dur-1) var(--ease);
}
.fr-table tbody tr:hover { background: var(--paper-1); cursor: pointer; }
.fr-table tbody td { padding: 16px 12px; vertical-align: top; font-size: 14px; }
.fr-table td.idx {
  font-family: var(--font-mono); font-size: 11px; color: var(--ink-3);
  white-space: nowrap; width: 200px;
}
.fr-table td.idx a { color: var(--ink-2); border: 0; }
.fr-table td.idx a:hover { color: var(--gold); }
.fr-table td.name {
  font-family: var(--font-sans); font-weight: 500; font-size: 15px;
  letter-spacing: -0.012em; color: var(--ink-0);
  line-height: 1.4; max-width: 360px;
}
.fr-table td.owner { font-family: var(--font-mono); font-size: 11px; color: var(--ink-2); white-space: nowrap; }
.fr-table td.state { width: 110px; }
.fr-table td.upd {
  width: 160px; color: var(--ink-3);
  font-family: var(--font-mono); font-size: 11px; text-align: right;
  letter-spacing: 0.04em;
}

/* Single-entry detail — finding page two-column layout */
.fd { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 56px; padding-top: 8px; }
@media (max-width: 1080px) { .fd { grid-template-columns: 1fr; gap: 32px; } }
.fd-claim {
  font-family: var(--font-sans); font-weight: 600;
  font-size: clamp(1.5rem, 3.2vw, 2rem);
  line-height: 1.08; letter-spacing: -0.022em;
  color: var(--ink-0); margin: 0 0 18px;
  text-wrap: balance;
}
.fd-note {
  font-family: var(--font-body); font-style: italic; font-size: 1.02rem;
  color: color-mix(in oklab, var(--ink-2) 92%, transparent);
  line-height: 1.55; max-width: 58ch;
  padding-left: 1.1rem;
  border-left: 1px solid color-mix(in oklab, var(--gold) 56%, transparent);
  margin: 0 0 32px;
  text-wrap: pretty;
}

.fd-conditions { border-top: 1px solid var(--rule-2); margin: 0; padding: 0; }
.fd-cond {
  display: grid; grid-template-columns: 180px 1fr;
  padding: 12px 0; border-bottom: 1px solid var(--rule-1); align-items: baseline;
  margin: 0;
}
.fd-cond dt {
  font-family: var(--font-mono); font-size: 10px;
  color: color-mix(in oklab, var(--ink-3) 88%, transparent);
  letter-spacing: 0.18em; text-transform: uppercase; margin: 0;
  font-weight: 500;
}
.fd-cond dd {
  font-family: var(--font-mono); font-size: 13px; color: var(--ink-1);
  word-break: break-all; margin: 0;
  letter-spacing: -0.005em;
}
.fd-cond dd.serif {
  font-family: var(--font-sans); font-weight: 400; font-size: 14px;
  letter-spacing: -0.005em; word-break: normal; color: var(--ink-1);
}
.fd-cond dd a {
  border-bottom: 1px solid color-mix(in oklab, var(--gold) 38%, transparent);
}
.fd-cond dd a:hover {
  color: var(--gold);
  border-bottom-color: var(--gold);
}

.fd-margin { padding-top: 4px; }
.fd-dial {
  border-top: 1px solid color-mix(in oklab, var(--gold) 32%, transparent);
  border-bottom: 1px solid var(--rule-2);
  padding: 14px 0 16px;
  margin-bottom: 22px;
  position: relative;
  background: transparent;
}
.fd-dial__k {
  position: relative; font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.18em; text-transform: uppercase;
  color: color-mix(in oklab, var(--gold) 72%, var(--ink-3));
  margin-bottom: 8px; font-weight: 500;
}
.fd-dial__v {
  position: relative; font-family: var(--font-sans); font-weight: 500;
  font-size: 1.15rem; letter-spacing: -0.015em; color: var(--ink-0);
}
.fd-dial__v.mono {
  font-family: var(--font-mono); font-weight: 400; font-size: 14px;
  word-break: break-all; letter-spacing: -0.005em;
}

/* Terminal-paper code block */
.tm-paper {
  background: var(--paper-1); border: 1px solid var(--rule-2);
  border-radius: var(--radius-sm); font-family: var(--font-mono);
  font-size: 13px; line-height: 1.65; color: var(--ink-1);
  overflow: hidden; margin: 16px 0 24px;
}
.tm-paper__bar {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 14px; border-bottom: 1px solid var(--rule-2);
  font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.18em; text-transform: uppercase;
  color: color-mix(in oklab, var(--gold) 60%, var(--ink-3));
  background: transparent;
}
.tm-paper__body { padding: 14px 18px 16px; white-space: pre; overflow-x: auto; }
.tm-ps { color: color-mix(in oklab, var(--gold) 60%, var(--ink-3)); }
.tm-cmd { color: var(--ink-0); }
.tm-flag { color: var(--ink-2); }

/* Endpoint list */
.hub-endpoints { list-style: none; padding: 0; margin: 0; }
.hub-endpoints li {
  display: flex; align-items: baseline; gap: 24px;
  padding: 12px 0; border-bottom: 1px solid var(--rule-1);
}
.hub-endpoints li:last-child { border-bottom: 0; }
.hub-endpoints li .verb {
  font-family: var(--font-mono); font-size: 12px;
  color: var(--ink-2); flex: 0 0 auto; white-space: nowrap;
  letter-spacing: 0.04em;
}
.hub-endpoints li .verb .v {
  color: color-mix(in oklab, var(--gold) 64%, var(--ink-3));
  letter-spacing: 0.06em; margin-right: 8px;
  text-transform: uppercase;
}
.hub-endpoints li .desc {
  color: var(--ink-2); font-family: var(--font-sans); font-size: 14px;
  line-height: 1.5; min-width: 0;
  letter-spacing: -0.005em;
}
.hub-endpoints li .desc a {
  border-bottom: 1px solid color-mix(in oklab, var(--gold) 38%, transparent);
}
.hub-endpoints li .desc a:hover {
  color: var(--gold);
  border-bottom-color: var(--gold);
}

/* Inline code */
code, .mono-inline {
  font-family: var(--font-mono); font-size: 0.88em;
  color: var(--ink-1); background: var(--paper-1);
  padding: 1px 5px; border: 1px solid var(--rule-2); border-radius: var(--radius-xs);
}

/* Lead paragraph */
.t-lead {
  font-family: var(--font-sans); font-weight: 400;
  font-size: 1.1rem; line-height: 1.5;
  letter-spacing: -0.012em;
  color: color-mix(in oklab, var(--ink-1) 92%, var(--ink-2));
  max-width: 60ch; margin: 0 0 24px;
  text-wrap: pretty;
}

/* Empty state — atmospheric, italic Garamond is appropriate here */
.empty {
  font-family: var(--font-body); font-style: italic;
  color: var(--ink-3); padding: 40px 0; text-align: center;
  font-size: 1.05rem;
}

/* Raw json block */
.raw-json {
  font-family: var(--font-mono); font-size: 12px;
  background: var(--paper-1); border: 1px solid var(--rule-2);
  padding: 14px 18px; overflow-x: auto;
  white-space: pre; color: var(--ink-1);
  border-radius: var(--radius-sm);
  margin: 12px 0 0;
}

/* Section heads */
.wb-section { margin: 32px 0 16px; }
.wb-section__head {
  display: flex; align-items: baseline; gap: 14px;
  padding-bottom: 10px;
  border-bottom: 1px solid color-mix(in oklab, var(--gold) 28%, transparent);
  margin-bottom: 14px;
}
.wb-section__num {
  font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.22em;
  color: color-mix(in oklab, var(--gold) 64%, var(--ink-3));
  font-weight: 500;
}
.wb-section__t {
  font-family: var(--font-sans); font-weight: 600; font-size: 1rem;
  color: var(--ink-0); letter-spacing: -0.018em;
}
.wb-section__aside {
  margin-left: auto; font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.18em; text-transform: uppercase; color: var(--ink-3);
}

/* Finding page — provenance, annotations, links */
.fd-prov-meta {
  margin-top: 6px;
  font-family: var(--font-sans); font-weight: 400; font-size: 13px;
  color: var(--ink-3); letter-spacing: -0.005em;
}
.fd-dial__gauge {
  margin-top: 10px;
  height: 3px;
  background: var(--rule-1);
  position: relative;
}
.fd-dial__gauge i {
  position: absolute; top: 0; left: 0; height: 100%;
  background: var(--gold);
}

.ann-list, .link-list {
  list-style: none;
  padding: 0;
  margin: 0;
  border-top: 1px solid var(--rule-2);
}
.ann-list li, .link-list li {
  padding: 12px 0;
  border-bottom: 1px solid var(--rule-1);
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: 18px;
  align-items: baseline;
}
.ann-author, .link-rel {
  font-family: var(--font-mono);
  font-size: 10px;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
  letter-spacing: 0.18em;
  text-transform: uppercase;
  font-weight: 500;
}
/* Annotation text — keep serif EB Garamond. These are quoted reviewer
   prose and serif reads as "this is a quote from a person." */
.ann-text {
  font-family: var(--font-body);
  font-size: 1rem;
  color: color-mix(in oklab, var(--ink-1) 92%, var(--ink-2));
  line-height: 1.55;
  text-wrap: pretty;
}
.link-list li a {
  font-family: var(--font-sans); font-weight: 500;
  font-size: 14px;
  color: var(--ink-1);
  border-bottom: 1px solid color-mix(in oklab, var(--gold) 38%, transparent);
  letter-spacing: -0.008em;
}
.link-list li a:hover {
  color: var(--gold);
  border-bottom-color: var(--gold);
}
.link-list li code {
  font-family: var(--font-mono);
  font-size: 12px;
  background: var(--paper-1);
  padding: 1px 5px;
  border: 1px solid var(--rule-2);
  border-radius: var(--radius-xs);
}
.link-list li .cross-vfr {
  font-family: var(--font-sans); font-weight: 400;
  font-size: 12px;
  color: var(--ink-3);
  letter-spacing: -0.005em;
}
.link-list li a:hover .cross-vfr { color: var(--gold); }
.link-list li .cross-vfr-bad {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--cinnabar);
  letter-spacing: 0.12em;
  text-transform: uppercase;
  margin-left: 6px;
}

/* Findings toolbar — search + state chips above the table */
.vf-toolbar {
  display: flex; gap: 18px; align-items: center;
  flex-wrap: wrap;
  padding: 12px 0 6px;
  border-bottom: 1px solid var(--rule-1);
  margin-bottom: 4px;
}
.vf-search {
  display: flex; align-items: center; gap: 8px;
  flex: 1 1 320px; min-width: 240px;
  padding: 6px 4px;
  border-bottom: 1px solid var(--rule-2);
  color: var(--ink-3);
  transition: border-bottom-color var(--dur-1) var(--ease);
}
.vf-search:focus-within { border-bottom-color: var(--gold); color: var(--ink-2); }
.vf-search input {
  flex: 1; border: 0; outline: 0; background: transparent;
  font-family: var(--font-sans); font-weight: 400; font-size: 14px;
  color: var(--ink-0); letter-spacing: -0.005em;
}
.vf-search input::placeholder { color: var(--ink-4); }
.vf-search__count {
  font-family: var(--font-mono); font-size: 11px;
  color: var(--ink-3); font-variant-numeric: tabular-nums;
  letter-spacing: 0.04em;
}
.vf-chips { display: flex; gap: 6px; flex-wrap: wrap; }
.vf-chip {
  font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.14em; text-transform: uppercase;
  color: var(--ink-3);
  border: 1px solid var(--rule-2);
  background: transparent;
  padding: 4px 9px; border-radius: var(--radius-sm);
  cursor: pointer;
  transition: border-color var(--dur-1) var(--ease), color var(--dur-1) var(--ease);
}
.vf-chip:hover {
  color: var(--ink-1);
  border-color: color-mix(in oklab, var(--gold) 56%, transparent);
}
.vf-chip--on {
  color: var(--ink-0);
  border-color: color-mix(in oklab, var(--gold) 64%, transparent);
  background: var(--paper-1);
}
.vf-chip span {
  margin-left: 6px; color: var(--ink-3);
  font-variant-numeric: tabular-nums;
}
.vf-chip--on span { color: var(--ink-2); }
.vf-empty {
  font-family: var(--font-body); font-style: italic;
  color: var(--ink-3); padding: 28px 0; text-align: center;
}

/* Findings table */
.vf-table { width: 100%; border-collapse: collapse; margin-top: 8px; }
.vf-table thead th {
  font-family: var(--font-mono); font-size: 10px; font-weight: 500;
  text-transform: uppercase; letter-spacing: 0.18em;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
  text-align: left; padding: 12px 10px; border-bottom: 1px solid var(--rule-2);
}
.vf-table thead th.num { text-align: right; }
.vf-table tbody tr {
  border-bottom: 1px solid var(--rule-1);
  cursor: pointer;
  transition: background var(--dur-1) var(--ease);
}
.vf-table tbody tr:hover { background: var(--paper-1); }
.vf-table tbody td a { color: inherit; border: 0; }
.vf-table tbody td a:hover { color: var(--gold); }
.vf-table tbody td {
  padding: 14px 10px; vertical-align: top; font-size: 14px;
}
.vf-table td.vf-id {
  font-family: var(--font-mono); font-size: 11px; color: var(--ink-3);
  white-space: nowrap; width: 130px; letter-spacing: 0.02em;
}
.vf-table td.vf-cls {
  font-family: var(--font-mono); font-size: 10px;
  text-transform: uppercase; letter-spacing: 0.14em;
  color: color-mix(in oklab, var(--gold) 60%, var(--ink-3));
  white-space: nowrap; width: 110px;
}
.vf-table td.vf-claim {
  font-family: var(--font-sans); font-weight: 500; font-size: 14px;
  letter-spacing: -0.012em; color: var(--ink-0);
  line-height: 1.45;
}
.vf-table td.vf-state { width: 130px; white-space: nowrap; }
.vf-table td.vf-conf {
  width: 92px; text-align: right;
  display: flex; align-items: center; justify-content: flex-end; gap: 8px;
  padding-top: 16px;
}
.vf-bar {
  display: inline-block; width: 36px; height: 3px;
  background: var(--rule-1); position: relative;
}
.vf-bar i {
  position: absolute; top: 0; left: 0; height: 100%;
  background: color-mix(in oklab, var(--gold) 72%, var(--ink-2));
}
.vf-num {
  font-family: var(--font-mono); font-size: 12px;
  color: var(--ink-2); font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

/* Constellation — findings as a star chart, sits above the table.
   Deterministic radial layout: each finding is a node colored by state
   and sized by confidence; links between findings render as faint gold
   arcs through the centre. Hover reads the claim; click opens the
   finding detail page. */
.vc-figure {
  margin: 0 0 28px;
  padding: 0;
  position: relative;
  background: var(--paper-1);
  border: 1px solid var(--rule-2);
  border-radius: var(--radius-md);
  overflow: hidden;
}
.vc {
  display: block;
  width: 100%;
  height: auto;
  max-height: 420px;
  background:
    radial-gradient(circle at 50% 50%,
      var(--star-glow) 0%,
      transparent 38%),
    var(--paper-1);
}
.vc-ring {
  fill: none;
  stroke: color-mix(in oklab, var(--gold) 22%, transparent);
  stroke-width: 0.6;
  stroke-dasharray: 1 5;
}
.vc-center {
  fill: var(--gold);
  filter: drop-shadow(0 0 6px var(--gold-glow));
}
.vc-edges {
  fill: none;
  stroke: color-mix(in oklab, var(--gold) 28%, transparent);
  stroke-width: 0.6;
  pointer-events: none;
}
.vc-edge {
  transition: stroke 200ms var(--ease), stroke-width 200ms var(--ease), opacity 200ms var(--ease);
}
.vc-edge--cross {
  stroke: color-mix(in oklab, var(--winter) 64%, transparent);
  stroke-width: 0.85;
  stroke-linecap: round;
}
.vc-node {
  cursor: pointer;
  outline: none;
  transition: opacity 200ms var(--ease);
}
.vc-glow {
  fill: var(--gold);
  opacity: 0;
  transition: opacity 200ms var(--ease);
  pointer-events: none;
}
.vc-node:hover .vc-glow,
.vc-node:focus .vc-glow {
  opacity: 0.32;
}
.vc-dot {
  transition: r 200ms var(--ease), stroke 200ms var(--ease), stroke-width 200ms var(--ease);
  stroke: color-mix(in oklab, var(--ink-1) 18%, transparent);
  stroke-width: 0.5;
}
.vc-node:hover .vc-dot,
.vc-node:focus .vc-dot {
  stroke: var(--ink-1);
  stroke-width: 1;
}
.vc-node--live .vc-dot {
  filter: drop-shadow(0 0 4px var(--gold-glow));
}
.vc-node--live .vc-glow {
  opacity: 0.18;
}

/* ─── Focus mode ─── click a node, fade everything but it and its
   incident edges + connected nodes. Click again or click background or
   press Esc to clear. */
.vc--focused .vc-node           { opacity: 0.22; }
.vc--focused .vc-node--focus    { opacity: 1; }
.vc--focused .vc-node--related  { opacity: 1; }
.vc--focused .vc-edge           { opacity: 0.16; }
.vc--focused .vc-edge--focus    { opacity: 1; stroke: var(--gold); stroke-width: 1.4; }
.vc--focused .vc-ring           { opacity: 0.4; }
.vc--focused .vc-center         { opacity: 0.5; }
.vc-node--focus .vc-glow        { opacity: 0.42; }
.vc-node--focus .vc-dot {
  stroke: var(--ink-0);
  stroke-width: 1.4;
}

.vc-tooltip {
  margin: 0;
  padding: 12px 18px 14px;
  border-top: 1px solid var(--rule-2);
  font-family: var(--font-sans);
  font-weight: 500;
  font-size: 14px;
  letter-spacing: -0.012em;
  line-height: 1.4;
  color: var(--ink-1);
  text-wrap: pretty;
  min-height: 1.4em;
  background: var(--paper-1);
  opacity: 1;
  transition: opacity 200ms var(--ease);
}
.vc-tooltip:empty::before {
  content: 'Hover a node to read the claim · click to focus · esc to clear.';
  color: var(--ink-3);
  font-weight: 400;
  font-style: italic;
}
.vc-tooltip__meta {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 400;
  letter-spacing: 0.04em;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
}
.vc-tooltip__open {
  margin-left: 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.04em;
  color: var(--gold);
  border-bottom: 1px solid color-mix(in oklab, var(--gold) 56%, transparent);
}
.vc-tooltip__open:hover {
  border-bottom-color: var(--gold);
}
.vc-legend {
  margin: 0;
  padding: 8px 18px 12px;
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  align-items: center;
  border-top: 1px solid var(--rule-1);
  background: transparent;
}
.vc-legend > span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.vc-legend__dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-right: 2px;
}
.vc-legend .vc-sep {
  color: var(--ink-4);
}

@media (max-width: 720px) {
  .vc { max-height: 280px; }
  .vc-tooltip { font-size: 13px; padding: 10px 14px 12px; }
  .vc-legend { padding: 8px 14px 12px; font-size: 9px; gap: 3px 8px; }
}

/* ─── Proof packet page ─── manifest + trace + lock + included-files
   table. The seam the skeptic wants to see. */
.pp-subhead {
  margin: 22px 0 10px;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 500;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: color-mix(in oklab, var(--gold) 64%, var(--ink-3));
}
.pp-checked {
  list-style: none;
  margin: 0 0 16px;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 4px 14px;
}
.pp-checked li code {
  display: inline-block;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--ink-1);
  background: transparent;
  border: 0;
  padding: 0;
  letter-spacing: 0.005em;
}
.pp-caveats {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.pp-caveats li {
  font-family: var(--font-body);
  font-size: 14px;
  line-height: 1.5;
  color: color-mix(in oklab, var(--ink-2) 92%, transparent);
  text-wrap: pretty;
}
.pp-table {
  width: 100%;
  border-collapse: collapse;
  margin-top: 4px;
}
.pp-table thead th {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 500;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: color-mix(in oklab, var(--ink-3) 92%, transparent);
  text-align: left;
  padding: 10px 10px;
  border-bottom: 1px solid var(--rule-2);
}
.pp-table thead th.num { text-align: right; }
.pp-table tbody tr {
  border-bottom: 1px solid var(--rule-1);
  transition: background var(--dur-1) var(--ease);
}
.pp-table tbody tr:hover { background: var(--paper-1); }
.pp-table td { padding: 8px 10px; vertical-align: baseline; }
.pp-path {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--ink-1);
  letter-spacing: -0.005em;
}
.pp-path code {
  font-family: inherit; font-size: inherit; color: inherit;
  background: transparent; border: 0; padding: 0;
}
.pp-bytes {
  width: 90px;
  text-align: right;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--ink-3);
  font-variant-numeric: tabular-nums;
}
.pp-sha {
  width: 180px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: color-mix(in oklab, var(--ink-2) 92%, transparent);
  letter-spacing: 0.02em;
  cursor: help;
}
.pp-sha code {
  font-family: inherit; font-size: inherit; color: inherit;
  background: transparent; border: 0; padding: 0;
}
.pp-row--canonical .pp-path,
.pp-row--canonical .pp-path code {
  color: color-mix(in oklab, var(--gold) 60%, var(--ink-0));
  font-weight: 500;
}
.pp-row--canonical .pp-sha {
  color: color-mix(in oklab, var(--gold) 36%, var(--ink-2));
}

/* Mobile fallback for the workbench rim */
@media (max-width: 720px) {
  .wb { grid-template-columns: 0 1fr 0 !important; }
  .wb-rim { display: none !important; }
  .wb-head, .wb-main, .wb-foot { padding-left: 20px !important; padding-right: 20px !important; }
  .vf-table td.vf-cls, .vf-table thead th:nth-child(2) { display: none; }
  .pp-table thead th, .pp-table td { padding: 6px 6px; }
  .pp-sha { width: 110px; }
  .pp-bytes { width: 60px; }
}
"#;

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Build the workbench frame around a page body. `active` controls which
/// rim link is marked with the alidade; `eyebrow` is the small mono label
/// above the title. URLs in the rim and foot come from `urls` so the
/// same render code works for any deploy.
#[allow(clippy::too_many_arguments)]
fn shell(
    urls: &PublicUrls,
    title: &str,
    active: &str,
    eyebrow_html: &str,
    title_html: &str,
    sub_html: &str,
    aside_html: &str,
    main_html: &str,
    foot_left_html: &str,
) -> String {
    let nav_link = |id: &str, href: &str, label: &str| -> String {
        let on = if id == active {
            " wb-rim__link--on"
        } else {
            ""
        };
        format!(r#"<a class="wb-rim__link{on}" href="{href}">{label}</a>"#)
    };
    let rim = format!(
        r#"<aside class="wb-rim">
  <div class="wb-rim__mark">
    <a href="/" aria-label="Vela">
      <img src="/static/vela-logo-mark.svg" width="26" height="26" alt="Vela">
    </a>
  </div>
  <nav class="wb-rim__nav" aria-label="Hub">
    {l1}
    {l2}
  </nav>
  <div class="wb-rim__index">v{HUB_VERSION}</div>
</aside>"#,
        l1 = nav_link("hub", "/", "01 · Hub"),
        l2 = nav_link("entries", "/entries", "02 · Entries"),
    );
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<link rel="icon" type="image/svg+xml" href="/static/favicon.svg">
{FONT_LINK}
<link rel="stylesheet" href="/static/tokens.css">
<link rel="stylesheet" href="/static/workbench.css">
<style>{HUB_STYLES}</style>
</head>
<body>
<div class="wb">
{rim}
<header class="wb-head">
  <div class="wb-head__row">
    <div>
      <div class="wb-head__eyebrow">{eyebrow_html}</div>
      <h1 class="wb-head__title">{title_html}</h1>
      <p class="wb-head__sub">{sub_html}</p>
    </div>
    <div class="wb-head__aside">{aside_html}</div>
  </div>
  <div class="wb-head__ticks"></div>
</header>
<main class="wb-main">
{main_html}
</main>
<footer class="wb-foot">
  <div class="wb-foot__left">
    <span><span class="wb-foot__star"></span> live · {hub_host}</span>
    <span>{foot_left_html}</span>
  </div>
  <div>Vela · <a href="{repo_url}" style="color:var(--ink-3);">{repo_short}</a></div>
</footer>
</div>
</body>
</html>
"#,
        title = escape_html(title),
        hub_host = escape_html(urls.hub_host()),
        repo_url = escape_html(&urls.repo),
        repo_short = escape_html(
            urls.repo
                .trim_start_matches("https://")
                .trim_start_matches("http://")
        ),
    )
}

// ── Static asset handlers ────────────────────────────────────────────
//
// The hub embeds the design-system stylesheets and brand SVGs at build
// time and serves them at /static/<name>. This keeps the binary
// self-contained (no Docker volume juggling) and ensures the marketing
// site (web/index.html) and the hub render against the same source
// files. Cache headers are conservative — the deploy is small enough
// that a redeploy is the cache-bust path.

fn css_response(body: &'static str) -> Response {
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    )
        .into_response()
}

fn svg_response(body: &'static str) -> Response {
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "image/svg+xml"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    )
        .into_response()
}

async fn static_tokens_css() -> Response {
    css_response(TOKENS_CSS)
}
async fn static_workbench_css() -> Response {
    css_response(WORKBENCH_CSS)
}
async fn static_site_css() -> Response {
    css_response(SITE_CSS)
}
async fn static_favicon_svg() -> Response {
    svg_response(FAVICON_SVG)
}
async fn static_logo_mark_svg() -> Response {
    svg_response(LOGO_MARK_SVG)
}
async fn static_logo_wordmark_svg() -> Response {
    svg_response(LOGO_WORDMARK_SVG)
}
async fn static_rete_svg() -> Response {
    svg_response(RETE_SVG)
}

fn render_root_html(urls: &PublicUrls) -> String {
    let hub_url = escape_html(&urls.hub);
    let site_host = escape_html(
        urls.site
            .trim_start_matches("https://")
            .trim_start_matches("http://"),
    );
    let main = format!(
        r#"<p class="t-lead">A signed-manifest registry for scientific frontiers. Anyone with an Ed25519 key can publish their own <code>vfr_id</code>; clients verify locally. The hub stores canonical bytes verbatim.</p>

<section class="wb-section">
  <div class="wb-section__head">
    <span class="wb-section__num">§1</span>
    <span class="wb-section__t">Endpoints</span>
    <span class="wb-section__aside">read · open · signature-gated</span>
  </div>
  <ul class="hub-endpoints">
    <li><span class="verb"><span class="v">GET</span>/</span><span class="desc">this banner</span></li>
    <li><span class="verb"><span class="v">GET</span>/healthz</span><span class="desc"><a href="/healthz">liveness</a></span></li>
    <li><span class="verb"><span class="v">GET</span>/entries</span><span class="desc"><a href="/entries">full registry, latest-publish-wins per <code>vfr_id</code></a></span></li>
    <li><span class="verb"><span class="v">GET</span>/entries/&#123;vfr_id&#125;</span><span class="desc">single entry</span></li>
    <li><span class="verb"><span class="v">POST</span>/entries</span><span class="desc">publish a signed manifest</span></li>
  </ul>
</section>

<section class="wb-section">
  <div class="wb-section__head">
    <span class="wb-section__num">§2</span>
    <span class="wb-section__t">Publish</span>
    <span class="wb-section__aside">the signature is the bind</span>
  </div>
  <div class="tm-paper">
    <div class="tm-paper__bar"><span>vela registry publish</span></div>
    <div class="tm-paper__body"><span class="tm-ps">$</span> <span class="tm-cmd">vela registry publish</span> frontier.json \
  <span class="tm-flag">--owner</span> reviewer:my-id \
  <span class="tm-flag">--key</span> ~/.vela/keys/private.key \
  <span class="tm-flag">--locator</span> https://example.com/frontier.json \
  <span class="tm-flag">--to</span> {hub_url}</div>
  </div>
</section>

<section class="wb-section">
  <div class="wb-section__head">
    <span class="wb-section__num">§3</span>
    <span class="wb-section__t">Pull and verify</span>
    <span class="wb-section__aside">byte-identical reconstruction</span>
  </div>
  <div class="tm-paper">
    <div class="tm-paper__bar"><span>vela registry list / pull</span></div>
    <div class="tm-paper__body"><span class="tm-ps">$</span> <span class="tm-cmd">vela registry list</span> <span class="tm-flag">--from</span> {hub_url}/entries
<span class="tm-ps">$</span> <span class="tm-cmd">vela registry pull</span> &lt;vfr_id&gt; <span class="tm-flag">--from</span> {hub_url}/entries <span class="tm-flag">--out</span> ./pulled.json</div>
  </div>
</section>"#,
    );
    shell(
        urls,
        "Vela Hub",
        "hub",
        "00 · Hub",
        "Vela Hub",
        "Signed registry manifests over HTTP. Open publishing, signature-gated; clients verify locally.",
        &format!(
            r#"<span>v{HUB_VERSION}</span><span>·</span><a class="wb-chip wb-chip--live"><span class="wb-chip__dot"></span>live</a>"#
        ),
        &main,
        &site_host,
    )
}

fn render_entries_html(urls: &PublicUrls, entries: &[Value]) -> String {
    let row = |entry: &Value| -> String {
        let s = |key: &str| -> String {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .map(escape_html)
                .unwrap_or_else(|| String::from("—"))
        };
        let vfr = s("vfr_id");
        let name = s("name");
        let owner = s("owner_actor_id");
        let signed_at = s("signed_publish_at");
        format!(
            r#"<tr onclick="location.href='/entries/{vfr}'">
  <td class="idx"><a href="/entries/{vfr}">{vfr}</a></td>
  <td class="name">{name}</td>
  <td class="owner">{owner}</td>
  <td class="state"><span class="wb-chip wb-chip--live"><span class="wb-chip__dot"></span>latest</span></td>
  <td class="upd">{signed_at}</td>
</tr>"#
        )
    };
    let body_rows: String = entries.iter().map(row).collect();
    let count = entries.len();
    let main = if entries.is_empty() {
        r#"<p class="empty">The registry is empty. Be the first to publish.</p>"#.to_string()
    } else {
        format!(
            r#"<table class="fr-table">
  <thead>
    <tr>
      <th>vfr_id</th>
      <th>name</th>
      <th>owner</th>
      <th>state</th>
      <th class="num">signed</th>
    </tr>
  </thead>
  <tbody>{body_rows}</tbody>
</table>"#
        )
    };
    shell(
        urls,
        "Vela Hub · Entries",
        "entries",
        &format!("01 · Entries · <span style=\"color:var(--ink-2);\">{count} signed</span>"),
        "Registry",
        "Latest-publish-wins per <code>vfr_id</code>. Click through for the manifest, the pull recipe, and the raw signed bytes.",
        &format!(
            r#"<span>{count} {plural}</span><span>·</span><a href="/entries">JSON</a>"#,
            plural = if count == 1 { "entry" } else { "entries" }
        ),
        &main,
        &format!("registry · v{HUB_VERSION}"),
    )
}

fn render_entry_html(
    urls: &PublicUrls,
    vfr_id: &str,
    entry: &Value,
    frontier: Option<&Project>,
) -> String {
    let hub_url = escape_html(&urls.hub);
    let s = |key: &str| -> String {
        entry
            .get(key)
            .and_then(|v| v.as_str())
            .map(escape_html)
            .unwrap_or_else(|| String::from("—"))
    };
    let name = s("name");
    let owner = s("owner_actor_id");
    let pubkey = s("owner_pubkey");
    let snapshot = s("latest_snapshot_hash");
    let event_log = s("latest_event_log_hash");
    let locator_raw = entry
        .get("network_locator")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let locator_safe = escape_html(locator_raw);
    let signed_at = s("signed_publish_at");
    let signature = s("signature");
    let schema = s("schema");
    let raw_json = serde_json::to_string_pretty(entry).unwrap_or_default();
    let vfr_safe = escape_html(vfr_id);

    // Note line varies by whether the frontier loaded.
    let note = if let Some(p) = frontier {
        format!(
            r#"{count} signed finding{plural} · {events} canonical event{events_plural}. Signed by <span class="t-mono">{owner}</span> at <span class="t-mono">{signed_at}</span>."#,
            count = p.findings.len(),
            plural = if p.findings.len() == 1 { "" } else { "s" },
            events = p.events.len(),
            events_plural = if p.events.len() == 1 { "" } else { "s" },
        )
    } else {
        format!(
            r#"Signed by <span class="t-mono">{owner}</span> at <span class="t-mono">{signed_at}</span>. The frontier file is fetched from the network locator on demand; verification happens on the client."#,
        )
    };

    let constellation = render_findings_constellation(vfr_id, frontier);
    let findings_section = render_findings_section(vfr_id, frontier);

    let main = format!(
        r#"<div class="fd">
  <article>
    <p class="fd-claim">{name}</p>
    <p class="fd-note">{note}</p>

    {constellation}
    {findings_section}

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§{pull_num}</span>
        <span class="wb-section__t">Pull and verify</span>
        <span class="wb-section__aside">byte-identical reconstruction</span>
      </div>
      <div class="tm-paper">
        <div class="tm-paper__bar"><span>vela registry pull · {vfr_safe}</span></div>
        <div class="tm-paper__body"><span class="tm-ps">$</span> <span class="tm-cmd">vela registry pull</span> {vfr_safe} \
  <span class="tm-flag">--from</span> {hub_url}/entries \
  <span class="tm-flag">--out</span> ./pulled.json</div>
      </div>
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§{manifest_num}</span>
        <span class="wb-section__t">Manifest</span>
        <span class="wb-section__aside">vela.registry-entry.v0.1</span>
      </div>
      <dl class="fd-conditions">
        <div class="fd-cond"><dt>vfr_id</dt><dd>{vfr_safe}</dd></div>
        <div class="fd-cond"><dt>schema</dt><dd>{schema}</dd></div>
        <div class="fd-cond"><dt>name</dt><dd class="serif">{name}</dd></div>
        <div class="fd-cond"><dt>owner_actor_id</dt><dd>{owner}</dd></div>
        <div class="fd-cond"><dt>network_locator</dt><dd><a href="{locator_safe}" rel="noopener">{locator_safe}</a></dd></div>
        <div class="fd-cond"><dt>signed_publish_at</dt><dd>{signed_at}</dd></div>
      </dl>
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§{hashes_num}</span>
        <span class="wb-section__t">Hashes</span>
        <span class="wb-section__aside">SHA-256 hex · canonical-JSON</span>
      </div>
      <dl class="fd-conditions">
        <div class="fd-cond"><dt>snapshot_hash</dt><dd>{snapshot}</dd></div>
        <div class="fd-cond"><dt>event_log_hash</dt><dd>{event_log}</dd></div>
      </dl>
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§{sig_num}</span>
        <span class="wb-section__t">Signature</span>
        <span class="wb-section__aside">Ed25519 over canonical preimage</span>
      </div>
      <dl class="fd-conditions">
        <div class="fd-cond"><dt>owner_pubkey</dt><dd>{pubkey}</dd></div>
        <div class="fd-cond"><dt>signature</dt><dd>{signature}</dd></div>
      </dl>
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§{raw_num}</span>
        <span class="wb-section__t">Raw manifest</span>
        <span class="wb-section__aside">canonical bytes the publisher signed</span>
      </div>
      <pre class="raw-json">{raw}</pre>
    </section>
  </article>

  <aside class="fd-margin">
    <div class="fd-dial">
      <div class="fd-dial__k">state</div>
      <div class="fd-dial__v" style="color:var(--signal);">Latest</div>
      <div class="fd-dial__k" style="margin-top:16px;">vfr_id</div>
      <div class="fd-dial__v mono">{vfr_safe}</div>
      <div class="fd-dial__k" style="margin-top:16px;">signed</div>
      <div class="fd-dial__v mono">{signed_at}</div>
    </div>

    {margin_extras}

    <div class="fd-dial">
      <div class="fd-dial__k">JSON</div>
      <div style="font-family:var(--font-mono);font-size:12px;line-height:1.6;color:var(--ink-1);margin-top:6px;">
        <a href="/entries/{vfr_safe}" style="border-bottom:1px solid var(--rule-3);">/entries/{vfr_safe}</a>
        <div style="color:var(--ink-3);margin-top:4px;">with <code>Accept: application/json</code></div>
      </div>
    </div>
  </aside>
</div>"#,
        raw = escape_html(&raw_json),
        // Section numbering: if findings rendered, they take §1; otherwise we
        // skip that slot.
        pull_num = if frontier.is_some() { "2" } else { "1" },
        manifest_num = if frontier.is_some() { "3" } else { "2" },
        hashes_num = if frontier.is_some() { "4" } else { "3" },
        sig_num = if frontier.is_some() { "5" } else { "4" },
        raw_num = if frontier.is_some() { "6" } else { "5" },
        margin_extras = render_margin_extras(frontier),
    );

    shell(
        urls,
        &format!("Vela Hub · {vfr_id}"),
        "entries",
        &format!("02 · Entry · <span style=\"color:var(--ink-2);\">{vfr_safe}</span>"),
        &name,
        "One signed manifest, read end-to-end. Pull the frontier from the network locator; verify hashes locally.",
        &format!(
            r#"<a href="/entries">← Entries</a><span>·</span><a href="/entries/{vfr_safe}">JSON</a><span>·</span><a href="/entries/{vfr_safe}/proof">Proof packet →</a>"#
        ),
        &main,
        &format!("{vfr_safe} · latest"),
    )
}

/// Map a finding's flags + review verdict to the chip variants the
/// design system carries: replicated, supported, contested, gap,
/// retracted. Order of precedence matches the substrate's own
/// derivation: explicit retraction > gap > review verdict > replication
/// status > default supported.
/// v0.36.2: derive whether a finding counts as "replicated" using the
/// v0.32 `Project.replications` collection as the source of truth.
/// Falls back to the legacy `evidence.replicated` scalar only when the
/// finding has no `Replication` records yet — same fall-through shape
/// as `Project::compute_confidence_for`.
fn is_replicated(
    b: &vela_protocol::bundle::FindingBundle,
    replications: &[vela_protocol::bundle::Replication],
) -> bool {
    let mut has_record = false;
    let mut has_success = false;
    for r in replications {
        if r.target_finding == b.id {
            has_record = true;
            if r.outcome == "replicated" {
                has_success = true;
            }
        }
    }
    if has_record { has_success } else { b.evidence.replicated }
}

fn finding_state(
    b: &vela_protocol::bundle::FindingBundle,
    replications: &[vela_protocol::bundle::Replication],
) -> (&'static str, &'static str) {
    use vela_protocol::bundle::ReviewState;
    if b.flags.retracted {
        return ("retracted", "lost");
    }
    if b.flags.gap || b.flags.negative_space {
        return ("gap", "stale");
    }
    if let Some(state) = &b.flags.review_state {
        match state {
            ReviewState::Contested => return ("contested", "warn"),
            ReviewState::NeedsRevision => return ("contested", "warn"),
            ReviewState::Rejected => return ("retracted", "lost"),
            ReviewState::Accepted => {
                if is_replicated(b, replications) {
                    return ("replicated", "ok");
                }
                return ("supported", "ok");
            }
        }
    }
    if b.flags.contested {
        return ("contested", "warn");
    }
    if is_replicated(b, replications) {
        return ("replicated", "ok");
    }
    ("supported", "ok")
}

/// Render the findings as a constellation — a deterministic radial layout
/// where each finding is a star colored by state and sized by confidence,
/// and the cross-finding `links` become faint gold dependency arcs through
/// the centre. Sits above the findings table as a navigable visual proof
/// that the substrate is a graph, not a list.
///
/// Layout: stable order = order in p.findings; a single ring at evenly
/// distributed angles. Hover a node to read the claim; click to navigate
/// to its detail page.
fn render_findings_constellation(vfr_id: &str, frontier: Option<&Project>) -> String {
    let Some(p) = frontier else { return String::new(); };
    if p.findings.is_empty() { return String::new(); }

    let n = p.findings.len();
    let view_w: i32 = 720;
    let view_h: i32 = 380;
    let cx = view_w as f64 / 2.0;
    let cy = view_h as f64 / 2.0;
    let ring_r = (cx.min(cy) - 60.0).max(80.0);

    // Stable position per finding id. Angle starts at top (-π/2) and runs
    // clockwise so the first finding is "12 o'clock".
    let pos: std::collections::HashMap<&str, (f64, f64)> = p
        .findings
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let angle = (i as f64 / n as f64) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
            let x = cx + ring_r * angle.cos();
            let y = cy + ring_r * angle.sin();
            (b.id.as_str(), (x, y))
        })
        .collect();

    // Per-finding link counts so the focused tooltip can show
    // "N dependencies · M dependents". We count edges incident to each
    // node; cross-frontier links count too.
    let mut deps_out: std::collections::HashMap<&str, u32> =
        std::collections::HashMap::new();
    let mut deps_in: std::collections::HashMap<&str, u32> =
        std::collections::HashMap::new();
    for b in &p.findings {
        let from = b.id.as_str();
        for link in &b.links {
            *deps_out.entry(from).or_default() += 1;
            // Only count inbound for resolvable targets.
            if pos.contains_key(link.target.as_str()) {
                *deps_in.entry(link.target.as_str()).or_default() += 1;
            }
        }
    }

    // Edges first so nodes render on top. Same-frontier links render as
    // quadratic-Bezier arcs through the centre (gold). Cross-frontier
    // links — those whose target id doesn't resolve to a local
    // finding — render as short outward strokes from the source node
    // toward the rim, in --winter (cool, distinct from --gold) so the
    // viewer sees external dependencies without a fetch chain.
    let mut edges = String::new();
    for b in &p.findings {
        let Some(&(x1, y1)) = pos.get(b.id.as_str()) else { continue };
        let from = escape_html(&b.id);
        for link in &b.links {
            if let Some(&(x2, y2)) = pos.get(link.target.as_str()) {
                let mx = (x1 + x2) / 2.0;
                let my = (y1 + y2) / 2.0;
                let pull = 0.45;
                let qx = cx + (mx - cx) * pull;
                let qy = cy + (my - cy) * pull;
                let to = escape_html(&link.target);
                edges.push_str(&format!(
                    r##"<path class="vc-edge" data-from="{from}" data-to="{to}" d="M {x1:.1} {y1:.1} Q {qx:.1} {qy:.1} {x2:.1} {y2:.1}"/>"##
                ));
            } else {
                // Cross-frontier link — draw a short outward stroke from
                // the source node toward the rim. Length tapers with the
                // source's confidence (so a high-confidence external
                // dependency reaches further). The length is bounded so
                // it stays inside the figure.
                let dx = x1 - cx;
                let dy = y1 - cy;
                let mag = (dx * dx + dy * dy).sqrt().max(1e-6);
                let conf = b.confidence.score.clamp(0.0, 1.0);
                let outward = 18.0 + conf * 22.0;
                let xt = x1 + (dx / mag) * outward;
                let yt = y1 + (dy / mag) * outward;
                edges.push_str(&format!(
                    r##"<path class="vc-edge vc-edge--cross" data-from="{from}" data-to="cross" d="M {x1:.1} {y1:.1} L {xt:.1} {yt:.1}"/>"##
                ));
            }
        }
    }

    // Nodes.
    let mut nodes = String::new();
    for b in &p.findings {
        let (x, y) = pos[b.id.as_str()];
        let (label, state_class) = finding_state(b, &p.replications);
        let r = 4.0 + b.confidence.score.clamp(0.0, 1.0) * 5.0;
        let live_class = if label == "replicated" { " vc-node--live" } else { "" };
        let vf = escape_html(&b.id);
        let claim = escape_html(&b.assertion.text);
        let n_out = deps_out.get(b.id.as_str()).copied().unwrap_or(0);
        let n_in = deps_in.get(b.id.as_str()).copied().unwrap_or(0);
        let href = format!(
            "/entries/{vfr}/findings/{vf}",
            vfr = escape_html(vfr_id)
        );
        nodes.push_str(&format!(
            r#"<a class="vc-node{live_class}" href="{href}" data-vf="{vf}" data-state="{label}" data-claim="{claim}" data-deps-out="{n_out}" data-deps-in="{n_in}">
              <circle class="vc-glow" cx="{x:.1}" cy="{y:.1}" r="{rg:.1}"/>
              <circle class="vc-dot" cx="{x:.1}" cy="{y:.1}" r="{r:.1}" style="fill:var(--state-{state_class});"/>
            </a>"#,
            rg = r * 2.6,
        ));
    }

    format!(
        r#"<figure class="vc-figure" data-vc-figure>
          <svg class="vc" viewBox="0 0 {w} {h}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Finding constellation — {n} findings as a star chart">
            <circle class="vc-ring" cx="{cx}" cy="{cy}" r="{rr}"/>
            <circle class="vc-center" cx="{cx}" cy="{cy}" r="2.5"/>
            <g class="vc-edges">{edges}</g>
            <g class="vc-nodes">{nodes}</g>
          </svg>
          <p class="vc-tooltip" data-vc-tooltip aria-hidden="true"></p>
          <p class="vc-legend">
            <span><span class="vc-legend__dot" style="background:var(--state-ok);"></span>replicated · supported</span>
            <span class="vc-sep">·</span>
            <span><span class="vc-legend__dot" style="background:var(--state-warn);"></span>contested</span>
            <span class="vc-sep">·</span>
            <span><span class="vc-legend__dot" style="background:var(--state-stale);"></span>gap · inferred</span>
            <span class="vc-sep">·</span>
            <span><span class="vc-legend__dot" style="background:var(--state-lost);"></span>retracted</span>
            <span class="vc-sep">·</span>
            <span><span class="vc-legend__dot" style="background:var(--winter);"></span>cross-frontier</span>
            <span class="vc-sep">·</span>
            <span>radius = confidence · click to focus · esc to clear</span>
          </p>
          <script>
          (function(){{
            var fig    = document.querySelector('[data-vc-figure]');
            var nodes  = document.querySelectorAll('.vc-node');
            var edges  = document.querySelectorAll('.vc-edge');
            var tip    = document.querySelector('[data-vc-tooltip]');
            if (!fig || !tip) return;
            var focused = null;
            var openHref = null;

            function clearTip() {{
              tip.innerHTML = '';
            }}
            function showTipFromNode(n) {{
              var claim = n.getAttribute('data-claim') || '';
              var nOut = parseInt(n.getAttribute('data-deps-out') || '0', 10);
              var nIn  = parseInt(n.getAttribute('data-deps-in')  || '0', 10);
              var href = n.getAttribute('href');
              var meta = nOut + ' dep' + (nOut === 1 ? '' : 's') + ' · ' + nIn + ' dependent' + (nIn === 1 ? '' : 's');
              if (focused) {{
                tip.innerHTML = claim + ' <span class="vc-tooltip__meta">· ' + meta + '</span> <a class="vc-tooltip__open" href="' + href + '">→ open</a>';
              }} else {{
                tip.innerHTML = claim + ' <span class="vc-tooltip__meta">· ' + meta + '</span>';
              }}
            }}

            function relatedSet(vf) {{
              var related = {{}};
              edges.forEach(function(e){{
                var from = e.getAttribute('data-from');
                var to   = e.getAttribute('data-to');
                if (from === vf) {{
                  related[to] = true;
                  e.classList.add('vc-edge--focus');
                }} else if (to === vf) {{
                  related[from] = true;
                  e.classList.add('vc-edge--focus');
                }} else {{
                  e.classList.remove('vc-edge--focus');
                }}
              }});
              return related;
            }}

            function applyFocus(node) {{
              var vf = node.getAttribute('data-vf');
              focused = vf;
              openHref = node.getAttribute('href');
              fig.classList.add('vc--focused');
              var related = relatedSet(vf);
              nodes.forEach(function(n){{
                var nv = n.getAttribute('data-vf');
                n.classList.remove('vc-node--focus','vc-node--related');
                if (nv === vf) n.classList.add('vc-node--focus');
                else if (related[nv]) n.classList.add('vc-node--related');
              }});
              showTipFromNode(node);
            }}

            function clearFocus() {{
              focused = null;
              openHref = null;
              fig.classList.remove('vc--focused');
              nodes.forEach(function(n){{ n.classList.remove('vc-node--focus','vc-node--related'); }});
              edges.forEach(function(e){{ e.classList.remove('vc-edge--focus'); }});
              clearTip();
            }}

            nodes.forEach(function(n){{
              n.addEventListener('mouseenter', function(){{ if (!focused) showTipFromNode(n); }});
              n.addEventListener('mouseleave', function(){{ if (!focused) clearTip(); }});
              n.addEventListener('focus',      function(){{ if (!focused) showTipFromNode(n); }});
              n.addEventListener('blur',       function(){{ if (!focused) clearTip(); }});
              n.addEventListener('click',      function(e){{
                var vf = n.getAttribute('data-vf');
                if (focused === vf) {{
                  // Second click on same node → navigate.
                  return;
                }}
                e.preventDefault();
                applyFocus(n);
              }});
              n.addEventListener('keydown', function(e){{
                if (e.key === 'Enter' && focused === n.getAttribute('data-vf')) {{
                  // Enter on a focused node → navigate.
                  return;
                }}
              }});
            }});

            // Click anywhere outside the SVG to clear focus.
            document.addEventListener('click', function(e){{
              if (!focused) return;
              if (!fig.contains(e.target)) clearFocus();
            }});
            // Escape clears focus.
            document.addEventListener('keydown', function(e){{
              if (e.key === 'Escape' && focused) {{ clearFocus(); }}
            }});
          }})();
          </script>
        </figure>"#,
        w = view_w,
        h = view_h,
        rr = ring_r,
    )
}

fn render_findings_section(vfr_id: &str, frontier: Option<&Project>) -> String {
    let Some(p) = frontier else {
        return String::from(
            r#"<section class="wb-section">
              <div class="wb-section__head">
                <span class="wb-section__num">§1</span>
                <span class="wb-section__t">Findings</span>
                <span class="wb-section__aside">frontier unavailable · pull to inspect</span>
              </div>
              <p class="empty">The frontier file at this manifest's <code>network_locator</code> could not be fetched. The manifest itself remains verifiable below; pull the frontier with the CLI to inspect findings.</p>
            </section>"#,
        );
    };
    if p.findings.is_empty() {
        return String::from(
            r#"<section class="wb-section">
              <div class="wb-section__head">
                <span class="wb-section__num">§1</span>
                <span class="wb-section__t">Findings</span>
                <span class="wb-section__aside">empty frontier</span>
              </div>
              <p class="empty">This frontier has no findings yet.</p>
            </section>"#,
        );
    }

    // Counts for the section aside.
    let by_state: std::collections::BTreeMap<&str, usize> =
        p.findings
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut acc, b| {
                *acc.entry(finding_state(b, &p.replications).0).or_default() += 1;
                acc
            });
    let counts = by_state
        .iter()
        .map(|(label, n)| format!("{n} {label}"))
        .collect::<Vec<_>>()
        .join(" · ");

    let mut rows = String::new();
    for b in &p.findings {
        let (label, state_class) = finding_state(b, &p.replications);
        let live_class = if label == "replicated" {
            " wb-chip--live"
        } else {
            ""
        };
        let pct = (b.confidence.score.clamp(0.0, 1.0) * 100.0).round() as u32;
        let assertion_type = b
            .assertion
            .assertion_type
            .split_whitespace()
            .next()
            .unwrap_or(&b.assertion.assertion_type);
        let vf_safe = escape_html(&b.id);
        let vfr_safe = escape_html(vfr_id);
        let href = format!("/entries/{vfr_safe}/findings/{vf_safe}");
        rows.push_str(&format!(
            r#"<tr onclick="location.href='{href}'">
              <td class="vf-id"><a href="{href}">{vf_safe}</a></td>
              <td class="vf-cls">{cls}</td>
              <td class="vf-claim"><a href="{href}">{claim}</a></td>
              <td class="vf-state"><span class="wb-chip{live_class}" style="--chip:var(--state-{state_class});"><span class="wb-chip__dot"></span>{label}</span></td>
              <td class="vf-conf"><span class="vf-bar"><i style="width:{pct}%;"></i></span><span class="vf-num">{score:.2}</span></td>
            </tr>"#,
            cls = escape_html(assertion_type),
            claim = escape_html(&b.assertion.text),
            score = b.confidence.score,
        ));
    }

    // State-filter chip row. data-state matches finding_state()'s label set.
    let mut chip_html =
        String::from(r#"<button class="vf-chip vf-chip--on" data-state="all">all</button>"#);
    for (label, n) in by_state.iter() {
        chip_html.push_str(&format!(
            r#"<button class="vf-chip" data-state="{label}">{label} <span>{n}</span></button>"#
        ));
    }

    format!(
        r#"<section class="wb-section">
          <div class="wb-section__head">
            <span class="wb-section__num">§1</span>
            <span class="wb-section__t">Findings · {count}</span>
            <span class="wb-section__aside">{counts}</span>
          </div>
          <div class="vf-toolbar">
            <label class="vf-search">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" aria-hidden="true"><circle cx="7" cy="7" r="5" stroke-width="1"/><line x1="11" y1="11" x2="15" y2="15" stroke-width="1"/></svg>
              <input type="search" placeholder="filter by claim, class, vf_id…" data-vf-search>
              <span class="vf-search__count" data-vf-count>{count} / {count}</span>
            </label>
            <div class="vf-chips" data-vf-chips>{chip_html}</div>
          </div>
          <table class="vf-table" data-vf-table>
            <thead>
              <tr>
                <th>vf_id</th>
                <th>class</th>
                <th>claim</th>
                <th>state</th>
                <th class="num">conf</th>
              </tr>
            </thead>
            <tbody>{rows}</tbody>
          </table>
          <p class="vf-empty" data-vf-empty hidden>No findings match the current filter.</p>
          <script>
          (function() {{
            var search = document.querySelector('[data-vf-search]');
            var chips  = document.querySelectorAll('[data-vf-chips] button');
            var rows   = document.querySelectorAll('[data-vf-table] tbody tr');
            var empty  = document.querySelector('[data-vf-empty]');
            var countEl = document.querySelector('[data-vf-count]');
            var total = rows.length;
            var activeState = 'all';
            var query = '';

            function rowMatches(r) {{
              if (activeState !== 'all') {{
                var st = r.querySelector('.vf-state .wb-chip');
                if (!st || !st.textContent.toLowerCase().includes(activeState)) return false;
              }}
              if (!query) return true;
              return (r.textContent || '').toLowerCase().includes(query);
            }}
            function apply() {{
              var shown = 0;
              rows.forEach(function(r) {{
                if (rowMatches(r)) {{ r.hidden = false; shown++; }} else {{ r.hidden = true; }}
              }});
              if (countEl) countEl.textContent = shown + ' / ' + total;
              if (empty)   empty.hidden = shown !== 0;
            }}
            if (search) search.addEventListener('input', function(e) {{
              query = (e.target.value || '').toLowerCase();
              apply();
            }});
            chips.forEach(function(c) {{
              c.addEventListener('click', function() {{
                activeState = c.getAttribute('data-state');
                chips.forEach(function(b) {{ b.classList.remove('vf-chip--on'); }});
                c.classList.add('vf-chip--on');
                apply();
              }});
            }});
          }})();
          </script>
        </section>"#,
        count = p.findings.len(),
    )
}

/// Extra margin-column dials when the frontier has loaded.
fn render_margin_extras(frontier: Option<&Project>) -> String {
    let Some(p) = frontier else {
        return String::new();
    };
    let actor_count = p.actors.len();
    let event_count = p.events.len();
    let proposal_count = p.proposals.len();
    format!(
        r#"<div class="fd-dial">
          <div class="fd-dial__k">findings</div>
          <div class="fd-dial__v" style="font-family:var(--font-mono);font-variant-numeric:tabular-nums;">{n}</div>
          <div class="fd-dial__k" style="margin-top:14px;">actors</div>
          <div class="fd-dial__v mono">{actors}</div>
          <div class="fd-dial__k" style="margin-top:14px;">events</div>
          <div class="fd-dial__v mono">{events}</div>
          <div class="fd-dial__k" style="margin-top:14px;">proposals</div>
          <div class="fd-dial__v mono">{proposals}</div>
        </div>"#,
        n = p.findings.len(),
        actors = actor_count,
        events = event_count,
        proposals = proposal_count,
    )
}

/// Single-finding detail. Workbench finding-pattern: serif claim,
/// italic note, conditions table, evidence atoms with stance, history
/// ledger in the margin. Reuses the cached frontier so navigation
/// from /entries/{vfr_id} adds zero round trips.
fn render_finding_html(
    urls: &PublicUrls,
    vfr_id: &str,
    project: &Project,
    bundle: &vela_protocol::bundle::FindingBundle,
) -> String {
    use vela_protocol::bundle::FindingBundle;

    let vfr_safe = escape_html(vfr_id);
    let vf_safe = escape_html(&bundle.id);
    let claim_html = escape_html(&bundle.assertion.text);
    let assertion_type = escape_html(&bundle.assertion.assertion_type);
    let (state_label, state_class) = finding_state(bundle, &project.replications);

    // Conditions row — surface only the fields that have content,
    // and present the structured ones as small chips alongside the
    // free-text description.
    let cond = &bundle.conditions;
    let mut cond_rows: Vec<String> = Vec::new();
    if !cond.text.is_empty() {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>scope</dt><dd class="serif">{}</dd></div>"#,
            escape_html(&cond.text),
        ));
    }
    if !cond.species_verified.is_empty() {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>species</dt><dd class="serif">{}</dd></div>"#,
            escape_html(&cond.species_verified.join(", ")),
        ));
    }
    let mut model_chips = Vec::new();
    if cond.in_vivo {
        model_chips.push("in vivo");
    }
    if cond.in_vitro {
        model_chips.push("in vitro");
    }
    if cond.human_data {
        model_chips.push("human data");
    }
    if cond.clinical_trial {
        model_chips.push("clinical trial");
    }
    if !model_chips.is_empty() {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>model</dt><dd>{}</dd></div>"#,
            escape_html(&model_chips.join(" · ")),
        ));
    }
    if let Some(c) = &cond.concentration_range {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>concentration</dt><dd>{}</dd></div>"#,
            escape_html(c),
        ));
    }
    if let Some(d) = &cond.duration {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>duration</dt><dd>{}</dd></div>"#,
            escape_html(d),
        ));
    }
    if let Some(c) = &cond.cell_type {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>cell_type</dt><dd>{}</dd></div>"#,
            escape_html(c),
        ));
    }
    if let Some(a) = &cond.age_group {
        cond_rows.push(format!(
            r#"<div class="fd-cond"><dt>age_group</dt><dd>{}</dd></div>"#,
            escape_html(a),
        ));
    }
    let conditions_dl = if cond_rows.is_empty() {
        String::from(r#"<p class="empty">No structured conditions declared.</p>"#)
    } else {
        format!(r#"<dl class="fd-conditions">{}</dl>"#, cond_rows.join(""),)
    };

    // Evidence row.
    let ev = &bundle.evidence;
    let mut ev_rows: Vec<String> = Vec::new();
    if !ev.evidence_type.is_empty() {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>type</dt><dd>{}</dd></div>"#,
            escape_html(&ev.evidence_type),
        ));
    }
    if !ev.method.is_empty() {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>method</dt><dd>{}</dd></div>"#,
            escape_html(&ev.method),
        ));
    }
    if !ev.model_system.is_empty() {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>model system</dt><dd>{}</dd></div>"#,
            escape_html(&ev.model_system),
        ));
    }
    if let Some(s) = &ev.species {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>species</dt><dd>{}</dd></div>"#,
            escape_html(s),
        ));
    }
    if let Some(s) = &ev.sample_size {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>sample size</dt><dd>{}</dd></div>"#,
            escape_html(s),
        ));
    }
    if let Some(es) = &ev.effect_size {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>effect size</dt><dd>{}</dd></div>"#,
            escape_html(es),
        ));
    }
    if let Some(p) = &ev.p_value {
        ev_rows.push(format!(
            r#"<div class="fd-cond"><dt>p-value</dt><dd>{}</dd></div>"#,
            escape_html(p),
        ));
    }
    let replicated_label = if ev.replicated {
        format!(
            "yes{}",
            ev.replication_count
                .map_or(String::new(), |n| format!(" · {n}×"))
        )
    } else {
        "no".to_string()
    };
    ev_rows.push(format!(
        r#"<div class="fd-cond"><dt>replicated</dt><dd>{}</dd></div>"#,
        escape_html(&replicated_label),
    ));
    let evidence_dl = format!(r#"<dl class="fd-conditions">{}</dl>"#, ev_rows.join(""));

    // Provenance — link to source paper.
    let prov = &bundle.provenance;
    let prov_link = if let Some(doi) = &prov.doi {
        format!(
            r#"<a href="https://doi.org/{}" rel="noopener">{}</a>"#,
            escape_html(doi),
            escape_html(&prov.title),
        )
    } else if let Some(pmid) = &prov.pmid {
        format!(
            r#"<a href="https://pubmed.ncbi.nlm.nih.gov/{}" rel="noopener">{}</a>"#,
            escape_html(pmid),
            escape_html(&prov.title),
        )
    } else {
        escape_html(&prov.title)
    };
    let mut prov_meta = Vec::new();
    if !prov.authors.is_empty() {
        let n = prov.authors.len();
        let first = prov.authors.first().map(|a| a.name.as_str()).unwrap_or("");
        prov_meta.push(if n > 1 {
            format!("{first} et al.")
        } else {
            first.to_string()
        });
    }
    if let Some(j) = &prov.journal {
        prov_meta.push(j.clone());
    }
    if let Some(y) = prov.year {
        prov_meta.push(y.to_string());
    }
    let prov_meta_html = if prov_meta.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="fd-prov-meta">{}</div>"#,
            escape_html(&prov_meta.join(" · ")),
        )
    };

    // Annotations (notes from reviewers).
    let mut annotations_html = String::new();
    if !bundle.annotations.is_empty() {
        let mut items = Vec::new();
        for a in &bundle.annotations {
            items.push(format!(
                r#"<li><span class="ann-author">{author}</span><span class="ann-text">{text}</span></li>"#,
                author = escape_html(&a.author),
                text = escape_html(&a.text),
            ));
        }
        annotations_html = format!(
            r#"<section class="wb-section">
              <div class="wb-section__head">
                <span class="wb-section__num">§3</span>
                <span class="wb-section__t">Annotations · {n}</span>
                <span class="wb-section__aside">notes from reviewers</span>
              </div>
              <ul class="ann-list">{items}</ul>
            </section>"#,
            n = bundle.annotations.len(),
            items = items.join(""),
        );
    }

    // Links — outgoing references. v0.8 splits these:
    //   · Local (vf_…)            → in-frontier click-through
    //   · Cross (vf_…@vfr_…)      → cross-frontier click-through when
    //                               the target's vfr_id is a declared
    //                               dep of this frontier; raw id otherwise.
    let mut links_html = String::new();
    if !bundle.links.is_empty() {
        let id_index: std::collections::HashMap<&str, &FindingBundle> = project
            .findings
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let mut items = Vec::new();
        for link in &bundle.links {
            use vela_protocol::bundle::LinkRef;
            let target_label = match LinkRef::parse(&link.target) {
                Ok(LinkRef::Local { vf_id }) => {
                    if let Some(target) = id_index.get(vf_id.as_str()) {
                        let claim = if target.assertion.text.len() > 60 {
                            format!("{}…", &target.assertion.text[..60])
                        } else {
                            target.assertion.text.clone()
                        };
                        format!(
                            r#"<a href="/entries/{vfr}/findings/{vf}">{claim}</a>"#,
                            vfr = vfr_safe,
                            vf = escape_html(&target.id),
                            claim = escape_html(&claim),
                        )
                    } else {
                        // Local target whose vf_id isn't in this frontier
                        // — usually a dangling reference. Render as code.
                        format!("<code>{}</code>", escape_html(&link.target))
                    }
                }
                Ok(LinkRef::Cross { vf_id, vfr_id }) => {
                    let dep = project.dep_for_vfr(&vfr_id);
                    let dep_name = dep
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| vfr_id.clone());
                    if dep.is_some() {
                        // Declared cross-frontier dep — link out to the
                        // hub's entry page for that frontier and finding.
                        format!(
                            r#"<a href="/entries/{vfr_id_e}/findings/{vf_id_e}">{vf_id_e}<span class="cross-vfr"> @ {dep_name_e}</span></a>"#,
                            vfr_id_e = escape_html(&vfr_id),
                            vf_id_e = escape_html(&vf_id),
                            dep_name_e = escape_html(&dep_name),
                        )
                    } else {
                        // Cross-frontier syntax but no declared dep —
                        // strict validation would have flagged this; on
                        // the hub we just render the raw id with a hint.
                        format!(
                            r#"<code>{}</code> <span class="cross-vfr-bad">(undeclared dep)</span>"#,
                            escape_html(&link.target),
                        )
                    }
                }
                Err(_) => {
                    // Malformed target. Surface raw bytes for debugging.
                    format!("<code>{}</code>", escape_html(&link.target))
                }
            };
            items.push(format!(
                r#"<li><span class="link-rel">{rel}</span> {target}</li>"#,
                rel = escape_html(&link.link_type),
                target = target_label,
            ));
        }
        links_html = format!(
            r#"<section class="wb-section">
              <div class="wb-section__head">
                <span class="wb-section__num">§4</span>
                <span class="wb-section__t">Links · {n}</span>
                <span class="wb-section__aside">references in this frontier</span>
              </div>
              <ul class="link-list">{items}</ul>
            </section>"#,
            n = bundle.links.len(),
            items = items.join(""),
        );
    }

    let live_class = if state_label == "replicated" {
        " wb-chip--live"
    } else {
        ""
    };
    let conf_pct = (bundle.confidence.score.clamp(0.0, 1.0) * 100.0).round() as u32;

    let main = format!(
        r#"<div class="fd">
  <article>
    <p class="fd-claim">{claim_html}</p>
    <p class="fd-note">{prov_link}{prov_meta_html}</p>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§1</span>
        <span class="wb-section__t">Conditions</span>
        <span class="wb-section__aside">declared on creation · immutable</span>
      </div>
      {conditions_dl}
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§2</span>
        <span class="wb-section__t">Evidence</span>
        <span class="wb-section__aside">{assertion_type}</span>
      </div>
      {evidence_dl}
    </section>

    {annotations_html}
    {links_html}
  </article>

  <aside class="fd-margin">
    <div class="fd-dial">
      <div class="fd-dial__k">state</div>
      <div class="fd-dial__v"><span class="wb-chip{live_class}" style="--chip:var(--state-{state_class});"><span class="wb-chip__dot"></span>{state_label}</span></div>
      <div class="fd-dial__k" style="margin-top:16px;">confidence</div>
      <div class="fd-dial__v" style="font-family:var(--font-mono);font-variant-numeric:tabular-nums;">{score:.2}</div>
      <div class="fd-dial__gauge"><i style="width:{conf_pct}%"></i></div>
    </div>

    <div class="fd-dial">
      <div class="fd-dial__k">vf_id</div>
      <div class="fd-dial__v mono">{vf_safe}</div>
      <div class="fd-dial__k" style="margin-top:14px;">version</div>
      <div class="fd-dial__v mono">{version}</div>
      <div class="fd-dial__k" style="margin-top:14px;">created</div>
      <div class="fd-dial__v mono">{created}</div>
    </div>

    <div class="fd-dial">
      <div class="fd-dial__k">JSON</div>
      <div style="font-family:var(--font-mono);font-size:12px;line-height:1.6;color:var(--ink-1);margin-top:6px;">
        <a href="/entries/{vfr_safe}/findings/{vf_safe}" style="border-bottom:1px solid var(--rule-3);">/entries/{vfr_safe}/findings/{vf_safe}</a>
        <div style="color:var(--ink-3);margin-top:4px;">with <code>Accept: application/json</code></div>
      </div>
    </div>
  </aside>
</div>"#,
        score = bundle.confidence.score,
        version = bundle.version,
        created = escape_html(&bundle.created),
    );

    shell(
        urls,
        &format!("Vela Hub · {}", &bundle.id),
        "entries",
        &format!("03 · Finding · <span style=\"color:var(--ink-2);\">{vf_safe}</span>"),
        "Finding",
        &claim_html,
        &format!(
            r#"<a href="/entries/{vfr_safe}">← {vfr_safe}</a><span>·</span><a href="/entries/{vfr_safe}/findings/{vf_safe}">JSON</a><span>·</span><a href="/entries/{vfr_safe}/proof">Proof packet →</a>"#
        ),
        &main,
        &format!("{vf_safe} @ {vfr_safe}"),
    )
}

fn render_finding_unavailable_html(urls: &PublicUrls, vfr_id: &str, vf_id: &str) -> String {
    let vfr_safe = escape_html(vfr_id);
    let vf_safe = escape_html(vf_id);
    let main = format!(
        r#"<p class="t-lead">The frontier file for <code>{vfr_safe}</code> is not currently reachable from its <code>network_locator</code>, so we cannot show finding <code>{vf_safe}</code>. The manifest is still verifiable; pull the frontier with the CLI to inspect.</p>
<p class="t-lead"><a href="/entries/{vfr_safe}" style="border-bottom:1px solid var(--rule-3);">← back to entry</a></p>"#
    );
    shell(
        urls,
        "Vela Hub · finding unavailable",
        "entries",
        "503 · Frontier unavailable",
        "Frontier unavailable",
        "The manifest is verifiable from the hub; the underlying frontier file lives at the publisher's locator.",
        "",
        &main,
        &vfr_safe,
    )
}

fn render_finding_not_found_html(urls: &PublicUrls, vfr_id: &str, vf_id: &str) -> String {
    let vfr_safe = escape_html(vfr_id);
    let vf_safe = escape_html(vf_id);
    let main = format!(
        r#"<p class="t-lead">No finding <code>{vf_safe}</code> in <code>{vfr_safe}</code>. The id may belong to a different frontier or an earlier publish.</p>
<p class="t-lead"><a href="/entries/{vfr_safe}" style="border-bottom:1px solid var(--rule-3);">← back to entry</a></p>"#
    );
    shell(
        urls,
        "Vela Hub · finding not found",
        "entries",
        "404 · Finding not found",
        "Not found",
        "Findings are content-addressed; their ids change with content.",
        "",
        &main,
        &vfr_safe,
    )
}

fn render_no_packet_html(urls: &PublicUrls, vfr_id: &str) -> String {
    let vfr_safe = escape_html(vfr_id);
    let main = format!(
        r#"<p class="t-lead">No proof packet has been generated for <code>{vfr_safe}</code> yet, or this hub instance was not started with <code>VELA_PROOF_PACKET_DIR</code> pointing at a packet directory.</p>
<p class="t-lead">Generate one locally with the CLI:</p>
<div class="tm-paper">
  <div class="tm-paper__bar"><span>vela frontier export · {vfr_safe}</span></div>
  <div class="tm-paper__body"><span class="tm-ps">$</span> <span class="tm-cmd">vela frontier export</span> <span class="tm-flag">--packet</span> &lt;path/to/frontier.json&gt; <span class="tm-flag">--out</span> ./packet</div>
</div>
<p class="t-lead">Then point this hub at the packet directory and reload:</p>
<div class="tm-paper">
  <div class="tm-paper__bar"><span>vela-hub serve</span></div>
  <div class="tm-paper__body"><span class="tm-ps">$</span> <span class="tm-cmd">VELA_PROOF_PACKET_DIR=./packet</span> vela-hub</div>
</div>
<p class="t-lead"><a href="/entries/{vfr_safe}" style="border-bottom:1px solid var(--rule-3);">← back to entry</a></p>"#
    );
    shell(
        urls,
        "Vela Hub · no proof packet",
        "entries",
        &format!("04 · Proof · <span style=\"color:var(--ink-2);\">{vfr_safe}</span>"),
        "No proof packet",
        "The hub has no packet for this entry. Generate one with the CLI and serve it via VELA_PROOF_PACKET_DIR.",
        &format!(r#"<a href="/entries/{vfr_safe}">← Entry</a>"#),
        &main,
        &vfr_safe,
    )
}

/// Render the proof packet inline: manifest summary, proof-trace
/// summary, lock summary, included-files table with sha256 hashes.
/// The skeptic-facing seam.
fn render_proof_packet_html(
    urls: &PublicUrls,
    vfr_id: &str,
    dir: &std::path::Path,
    manifest: &Value,
    proof_trace: Option<&Value>,
    lock: Option<&Value>,
) -> String {
    let vfr_safe = escape_html(vfr_id);
    let dir_safe = escape_html(&dir.display().to_string());

    // ─── Manifest summary ──────────────────────────────────────────
    let generated_at = manifest
        .get("generated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let packet_format = manifest
        .get("packet_format")
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let packet_version = manifest
        .get("packet_version")
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let source = manifest.get("source");
    let vela_version = source
        .and_then(|s| s.get("vela_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let compiler = source
        .and_then(|s| s.get("compiler"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let project_name = source
        .and_then(|s| s.get("project_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let description = source
        .and_then(|s| s.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");
    let schema = source
        .and_then(|s| s.get("schema"))
        .and_then(|v| v.as_str())
        .unwrap_or("—");

    let stats = manifest.get("stats");
    let stat = |k: &str| -> i64 {
        stats
            .and_then(|s| s.get(k))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
    };
    let stats_html = format!(
        r#"<dl class="fd-conditions">
  <div class="fd-cond"><dt>findings</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>contested</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>gaps</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>contradiction edges</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>evidence atoms</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>condition records</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>sources</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>bridge entities</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>proposals</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>review events</dt><dd>{}</dd></div>
</dl>"#,
        stat("findings"),
        stat("contested"),
        stat("gaps"),
        stat("contradiction_edges"),
        stat("evidence_atoms"),
        stat("condition_records"),
        stat("sources"),
        stat("bridge_entities"),
        stat("proposals"),
        stat("review_events"),
    );

    // ─── Proof-trace summary ───────────────────────────────────────
    let trace_html = if let Some(t) = proof_trace {
        let s = |k: &str| -> &str { t.get(k).and_then(|v| v.as_str()).unwrap_or("—") };
        let checked: Vec<String> = t
            .get("checked_artifacts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| escape_html(s)))
                    .collect()
            })
            .unwrap_or_default();
        let checked_list = if checked.is_empty() {
            String::from(r#"<p class="empty">no checked_artifacts in trace</p>"#)
        } else {
            format!(
                r#"<ul class="pp-checked">{}</ul>"#,
                checked
                    .iter()
                    .map(|c| format!(r#"<li><code>{c}</code></li>"#))
                    .collect::<String>()
            )
        };
        let caveats: Vec<String> = t
            .get("caveats")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| escape_html(s)))
                    .collect()
            })
            .unwrap_or_default();
        let caveats_html = if caveats.is_empty() {
            String::new()
        } else {
            format!(
                r#"<dl class="fd-conditions"><div class="fd-cond"><dt>caveats</dt><dd class="serif"><ul class="pp-caveats">{}</ul></dd></div></dl>"#,
                caveats
                    .iter()
                    .map(|c| format!(r#"<li>{c}</li>"#))
                    .collect::<String>()
            )
        };
        format!(
            r#"<dl class="fd-conditions">
  <div class="fd-cond"><dt>trace_version</dt><dd>{tv}</dd></div>
  <div class="fd-cond"><dt>schema_version</dt><dd>{sv}</dd></div>
  <div class="fd-cond"><dt>source_hash</dt><dd>{sh}</dd></div>
  <div class="fd-cond"><dt>snapshot_hash</dt><dd>{snh}</dd></div>
  <div class="fd-cond"><dt>event_log_hash</dt><dd>{eh}</dd></div>
  <div class="fd-cond"><dt>proposal_state_hash</dt><dd>{ph}</dd></div>
  <div class="fd-cond"><dt>replay_status</dt><dd>{rs}</dd></div>
  <div class="fd-cond"><dt>status</dt><dd>{st}</dd></div>
</dl>
<h4 class="pp-subhead">Checked artifacts ({n})</h4>
{checked_list}
{caveats_html}"#,
            tv = escape_html(s("trace_version")),
            sv = escape_html(s("schema_version")),
            sh = escape_html(s("source_hash")),
            snh = escape_html(s("snapshot_hash")),
            eh = escape_html(s("event_log_hash")),
            ph = escape_html(s("proposal_state_hash")),
            rs = escape_html(s("replay_status")),
            st = escape_html(s("status")),
            n = checked.len(),
        )
    } else {
        String::from(r#"<p class="empty">No proof-trace.json in this packet.</p>"#)
    };

    // ─── Lock summary ──────────────────────────────────────────────
    let lock_html = if let Some(l) = lock {
        let lock_format = l.get("lock_format").and_then(|v| v.as_str()).unwrap_or("—");
        let lock_generated = l.get("generated_at").and_then(|v| v.as_str()).unwrap_or("—");
        let n_files = l
            .get("files")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!(
            r#"<dl class="fd-conditions">
  <div class="fd-cond"><dt>lock_format</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>generated_at</dt><dd>{}</dd></div>
  <div class="fd-cond"><dt>locked file count</dt><dd>{}</dd></div>
</dl>"#,
            escape_html(lock_format),
            escape_html(lock_generated),
            n_files,
        )
    } else {
        String::from(r#"<p class="empty">No packet.lock.json in this packet.</p>"#)
    };

    // ─── Included files table ──────────────────────────────────────
    // Mark canonical proof-bearing files (the set listed by
    // proof-trace.json's checked_artifacts) in gold.
    let canonical: std::collections::HashSet<String> = proof_trace
        .and_then(|t| t.get("checked_artifacts"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let empty: Vec<Value> = Vec::new();
    let included = manifest
        .get("included_files")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let mut total_bytes: u64 = 0;
    let mut rows = String::new();
    for f in included {
        let path = f.get("path").and_then(|v| v.as_str()).unwrap_or("—");
        let bytes = f.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let sha = f.get("sha256").and_then(|v| v.as_str()).unwrap_or("—");
        total_bytes += bytes;
        let cls = if canonical.contains(path) {
            "pp-row pp-row--canonical"
        } else {
            "pp-row"
        };
        rows.push_str(&format!(
            r#"<tr class="{cls}"><td class="pp-path"><code>{}</code></td><td class="pp-bytes">{}</td><td class="pp-sha"><code title="{}">{}</code></td></tr>"#,
            escape_html(path),
            bytes,
            escape_html(sha),
            escape_html(&sha[..sha.len().min(16)]),
        ));
    }

    // ─── Page assembly ─────────────────────────────────────────────
    let main = format!(
        r#"<div class="fd">
  <article>
    <p class="fd-claim">Proof packet for {project}</p>
    <p class="fd-note">{description}</p>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§1</span>
        <span class="wb-section__t">Manifest</span>
        <span class="wb-section__aside">{packet_format} · {packet_version}</span>
      </div>
      <dl class="fd-conditions">
        <div class="fd-cond"><dt>generated_at</dt><dd>{generated_at}</dd></div>
        <div class="fd-cond"><dt>vela_version</dt><dd>{vela_version}</dd></div>
        <div class="fd-cond"><dt>compiler</dt><dd>{compiler}</dd></div>
        <div class="fd-cond"><dt>schema</dt><dd>{schema}</dd></div>
      </dl>
      <h4 class="pp-subhead">Stats</h4>
      {stats_html}
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§2</span>
        <span class="wb-section__t">Proof trace</span>
        <span class="wb-section__aside">canonical-JSON SHA-256 chain</span>
      </div>
      {trace_html}
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§3</span>
        <span class="wb-section__t">Lock</span>
        <span class="wb-section__aside">packet integrity</span>
      </div>
      {lock_html}
    </section>

    <section class="wb-section">
      <div class="wb-section__head">
        <span class="wb-section__num">§4</span>
        <span class="wb-section__t">Included files · {n_files}</span>
        <span class="wb-section__aside">canonical files in gold · sha256 truncated to 16</span>
      </div>
      <table class="pp-table">
        <thead><tr><th>path</th><th class="num">bytes</th><th>sha256</th></tr></thead>
        <tbody>{rows}</tbody>
      </table>
    </section>
  </article>

  <aside class="fd-margin">
    <div class="fd-dial">
      <div class="fd-dial__k">findings</div>
      <div class="fd-dial__v" style="font-family:var(--font-mono);font-variant-numeric:tabular-nums;">{n_findings}</div>
      <div class="fd-dial__k" style="margin-top:14px;">total bytes</div>
      <div class="fd-dial__v mono">{total_kb} KB</div>
      <div class="fd-dial__k" style="margin-top:14px;">files</div>
      <div class="fd-dial__v mono">{n_files}</div>
      <div class="fd-dial__k" style="margin-top:14px;">generated</div>
      <div class="fd-dial__v mono">{generated_at}</div>
    </div>

    <div class="fd-dial">
      <div class="fd-dial__k">download</div>
      <p style="margin:8px 0 0;font-family:var(--font-sans);font-size:13px;line-height:1.5;color:var(--ink-2);">
        <a href="/entries/{vfr_safe}/proof/download" style="border-bottom:1px solid color-mix(in oklab, var(--gold) 56%, transparent);">↓ {vfr_safe}-proof-packet.tar.gz</a>
      </p>
      <p style="margin:10px 0 0;font-family:var(--font-mono);font-size:11px;color:var(--ink-3);">
        verify locally with <code>shasum -a 256</code>
      </p>
    </div>

    <div class="fd-dial">
      <div class="fd-dial__k">source</div>
      <p style="margin:6px 0 0;font-family:var(--font-mono);font-size:11px;color:var(--ink-3);word-break:break-all;">{dir_safe}</p>
      <p style="margin:10px 0 0;font-family:var(--font-mono);font-size:11px;color:var(--ink-3);">
        <a href="/entries/{vfr_safe}" style="border-bottom:1px solid var(--rule-3);">← /entries/{vfr_safe}</a>
      </p>
    </div>
  </aside>
</div>"#,
        n_findings = stat("findings"),
        n_files = included.len(),
        total_kb = total_bytes / 1024,
        project = escape_html(project_name),
        description = escape_html(description),
        generated_at = escape_html(generated_at),
        vela_version = escape_html(vela_version),
        compiler = escape_html(compiler),
        schema = escape_html(schema),
        packet_format = escape_html(packet_format),
        packet_version = escape_html(packet_version),
    );

    shell(
        urls,
        &format!("Vela Hub · proof · {vfr_id}"),
        "entries",
        &format!("04 · Proof · <span style=\"color:var(--ink-2);\">{vfr_safe}</span>"),
        "Proof packet",
        "Manifest, signed-trace chain, integrity lock, and the file-by-file SHA-256 table the skeptic actually wants to see.",
        &format!(
            r#"<a href="/entries/{vfr_safe}">← Entry</a><span>·</span><a href="/entries/{vfr_safe}/proof/download">Download (.tar.gz)</a>"#
        ),
        &main,
        &format!("{vfr_safe} · proof packet"),
    )
}

fn render_not_found_html(urls: &PublicUrls, vfr_id: &str) -> String {
    let vfr_safe = escape_html(vfr_id);
    let main = format!(
        r#"<p class="t-lead">No entry for <code>{vfr_safe}</code> in this hub. The id may belong to a different registry, or the publisher may not have pushed yet.</p>
<p class="t-lead"><a href="/entries" style="border-bottom:1px solid var(--rule-3);">← back to entries</a></p>"#
    );
    shell(
        urls,
        "Vela Hub · not found",
        "entries",
        "404 · Not found",
        "Not found",
        "Anyone can publish a signed manifest at this id. Until then, there is nothing here.",
        "",
        &main,
        &vfr_safe,
    )
}
