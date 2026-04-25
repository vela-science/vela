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
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use vela_protocol::project::Project;
use vela_protocol::registry::{RegistryEntry as ProtocolEntry, verify_entry};

const HUB_VERSION: &str = env!("CARGO_PKG_VERSION");
const REGISTRY_SCHEMA: &str = "vela.registry.v0.1";

const DEFAULT_PUBLIC_URL: &str = "https://vela-hub.fly.dev";
const DEFAULT_REPO_URL: &str = "https://github.com/vela-science/vela";
const DEFAULT_SITE_URL: &str = "https://vela.science";

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
    pool: Pool<Postgres>,
    /// Frontier cache for the entry detail page. Keyed by
    /// `(vfr_id, signed_publish_at)` so a fresh publish forces a
    /// re-fetch automatically. Bounded loosely; in v0.7 we expect
    /// fewer than a dozen frontiers ever.
    frontier_cache: FrontierCache,
    /// Shared reqwest client. Connection pool reuse matters for
    /// repeat fetches against the same locator host.
    http: reqwest::Client,
    /// Public-facing URLs the rendered HTML quotes back to readers.
    /// Configurable via env so the same binary serves any deployment.
    urls: PublicUrls,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistryEntry {
    schema: String,
    vfr_id: String,
    name: String,
    owner_actor_id: String,
    owner_pubkey: String,
    latest_snapshot_hash: String,
    latest_event_log_hash: String,
    network_locator: String,
    signed_publish_at: String,
    signature: String,
}

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

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;

    // Sanity-check schema presence so we fail fast on a misconfigured DB.
    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'registry_entries')",
    )
    .fetch_one(&pool)
    .await?;
    if !table_exists {
        return Err(
            "registry_entries table not found; run the schema migration before starting the hub"
                .into(),
        );
    }

    let http = reqwest::Client::builder()
        .user_agent(concat!("vela-hub/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(8))
        .build()?;
    let urls = PublicUrls::from_env();
    let state = AppState {
        pool,
        frontier_cache: Arc::new(RwLock::new(HashMap::new())),
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
            "POST /entries       — publish a signed manifest (open, signature-gated)"
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
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"ok": true, "db": "reachable"}))),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok": false, "db": "unreachable", "error": e.to_string()})),
        ),
    }
}

/// Latest-publish-wins per vfr_id, newest first.
const LATEST_PER_VFR_SQL: &str = r#"
SELECT raw_json FROM registry_entries r
WHERE r.signed_publish_at = (
    SELECT MAX(signed_publish_at) FROM registry_entries
    WHERE vfr_id = r.vfr_id
)
ORDER BY r.signed_publish_at DESC
"#;

async fn list_entries(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let rows = sqlx::query_scalar::<_, serde_json::Value>(LATEST_PER_VFR_SQL)
        .fetch_all(&state.pool)
        .await;
    match rows {
        Ok(values) => {
            if wants_html(&headers) {
                Html(render_entries_html(&state.urls, &values)).into_response()
            } else {
                (
                    StatusCode::OK,
                    Json(json!({"schema": REGISTRY_SCHEMA, "entries": values})),
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
    let row = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT raw_json FROM registry_entries
        WHERE vfr_id = $1
        ORDER BY signed_publish_at DESC
        LIMIT 1
        "#,
    )
    .bind(&vfr_id)
    .fetch_optional(&state.pool)
    .await;
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
    .bind(&body)
    .fetch_optional(&state.pool)
    .await;

    match inserted {
        Ok(Some(_)) => (
            StatusCode::CREATED,
            Json(json!({
                "ok": true,
                "duplicate": false,
                "vfr_id": entry.vfr_id,
                "signed_publish_at": entry.signed_publish_at,
            })),
        ),
        Ok(None) => (
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
const HUB_STYLES: &str = r#"
/* Entries table — adapted from the workbench frontier pattern */
.fr-table { width: 100%; border-collapse: collapse; margin-top: 8px; }
.fr-table thead th {
  font-family: var(--font-mono); font-size: 10px; font-weight: 400;
  text-transform: uppercase; letter-spacing: 0.14em; color: var(--ink-3);
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
.fr-table td.idx a:hover { color: var(--ink-0); }
.fr-table td.name {
  font-family: var(--font-serif); font-size: 17px; color: var(--ink-0);
  line-height: 1.35; max-width: 320px;
}
.fr-table td.owner { font-family: var(--font-mono); font-size: 11px; color: var(--ink-2); white-space: nowrap; }
.fr-table td.state { width: 110px; }
.fr-table td.upd {
  width: 160px; color: var(--ink-3);
  font-family: var(--font-mono); font-size: 11px; text-align: right;
}

/* Single-entry detail — adapted from the workbench finding pattern */
.fd { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 56px; padding-top: 8px; }
@media (max-width: 1080px) { .fd { grid-template-columns: 1fr; gap: 32px; } }
.fd-claim {
  font-family: var(--font-serif); font-size: 30px; line-height: 1.25;
  letter-spacing: -0.015em; color: var(--ink-0); margin: 0 0 14px;
}
.fd-note {
  font-family: var(--font-serif); font-style: italic; font-size: 17px;
  color: var(--ink-2); line-height: 1.55; max-width: 56ch;
  padding-left: 18px; border-left: 1px solid var(--rule-ink); margin: 0 0 32px;
}

.fd-conditions { border-top: 1px solid var(--rule-2); margin: 0; padding: 0; }
.fd-cond {
  display: grid; grid-template-columns: 180px 1fr;
  padding: 12px 0; border-bottom: 1px solid var(--rule-1); align-items: baseline;
  margin: 0;
}
.fd-cond dt {
  font-family: var(--font-mono); font-size: 11px; color: var(--ink-3);
  letter-spacing: 0.1em; text-transform: uppercase; margin: 0;
}
.fd-cond dd {
  font-family: var(--font-mono); font-size: 13px; color: var(--ink-0);
  word-break: break-all; margin: 0;
}
.fd-cond dd.serif { font-family: var(--font-serif); font-size: 15px; word-break: normal; }
.fd-cond dd a { border-bottom: 1px solid var(--rule-3); }
.fd-cond dd a:hover { border-bottom-color: var(--rule-ink); }

.fd-margin { padding-top: 4px; }
.fd-dial {
  border: 1px solid var(--rule-2); padding: 18px 18px 16px;
  margin-bottom: 22px; position: relative; background: var(--paper-0);
}
.fd-dial::before {
  content: ""; position: absolute; inset: 0;
  background-image: linear-gradient(to bottom, var(--rule-1) 0 1px, transparent 1px 100%);
  background-size: 100% 12px; background-position: left 18px; opacity: 0.5; pointer-events: none;
}
.fd-dial__k {
  position: relative; font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.14em; text-transform: uppercase; color: var(--ink-3); margin-bottom: 8px;
}
.fd-dial__v {
  position: relative; font-family: var(--font-serif);
  font-size: 22px; letter-spacing: -0.01em; color: var(--ink-0);
}
.fd-dial__v.mono { font-family: var(--font-mono); font-size: 16px; word-break: break-all; }

/* Terminal-paper code block — adapted from the workbench terminal pattern */
.tm-paper {
  background: var(--paper-1); border: 1px solid var(--rule-2);
  border-radius: var(--radius-1); font-family: var(--font-mono);
  font-size: 13px; line-height: 1.65; color: var(--ink-1);
  overflow: hidden; margin: 16px 0 24px;
}
.tm-paper__bar {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 14px; border-bottom: 1px solid var(--rule-2);
  font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.14em; text-transform: uppercase;
  color: var(--ink-3); background: var(--paper-0);
}
.tm-paper__body { padding: 14px 18px 16px; white-space: pre; overflow-x: auto; }
.tm-ps { color: var(--ink-3); }
.tm-cmd { color: var(--ink-0); }
.tm-flag { color: var(--ink-2); }

/* Endpoint list */
.hub-endpoints { list-style: none; padding: 0; margin: 0; }
.hub-endpoints li {
  display: flex; align-items: baseline; gap: 24px;
  padding: 10px 0; border-bottom: 1px dashed var(--rule-1);
}
.hub-endpoints li:last-child { border-bottom: 0; }
.hub-endpoints li .verb {
  font-family: var(--font-mono); font-size: 13px;
  color: var(--ink-2); flex: 0 0 auto; white-space: nowrap;
}
.hub-endpoints li .verb .v {
  color: var(--ink-3); letter-spacing: 0.06em; margin-right: 8px;
}
.hub-endpoints li .desc {
  color: var(--ink-2); font-family: var(--font-sans); font-size: 14px;
  min-width: 0;
}
.hub-endpoints li .desc a { border-bottom: 1px solid var(--rule-3); }
.hub-endpoints li .desc a:hover { border-bottom-color: var(--rule-ink); }

/* Inline code */
code, .mono-inline {
  font-family: var(--font-mono); font-size: 0.88em;
  color: var(--ink-1); background: var(--paper-1);
  padding: 1px 5px; border: 1px solid var(--rule-1); border-radius: var(--radius-1);
}

/* Lead paragraph */
.t-lead {
  font-family: var(--font-serif); font-size: 18px;
  line-height: var(--leading-read); color: var(--ink-1);
  max-width: 64ch; margin: 0 0 24px;
}

/* Empty state */
.empty {
  font-family: var(--font-serif); font-style: italic;
  color: var(--ink-3); padding: 40px 0; text-align: center;
}

/* Raw json block */
.raw-json {
  font-family: var(--font-mono); font-size: 12px;
  background: var(--paper-1); border: 1px solid var(--rule-2);
  padding: 14px 18px; overflow-x: auto;
  white-space: pre; color: var(--ink-1);
  border-radius: var(--radius-1);
  margin: 12px 0 0;
}

/* Section heads */
.wb-section { margin: 32px 0 16px; }
.wb-section__head {
  display: flex; align-items: baseline; gap: 14px;
  padding-bottom: 10px; border-bottom: 1px solid var(--rule-2); margin-bottom: 14px;
}
.wb-section__num { font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.2em; color: var(--ink-3); }
.wb-section__t { font-family: var(--font-sans); font-weight: 500; font-size: 18px; color: var(--ink-0); letter-spacing: -0.005em; }
.wb-section__aside {
  margin-left: auto; font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.14em; text-transform: uppercase; color: var(--ink-3);
}

/* Findings table — workbench frontier-pattern, embedded in entry detail */
.vf-table { width: 100%; border-collapse: collapse; margin-top: 8px; }
.vf-table thead th {
  font-family: var(--font-mono); font-size: 10px; font-weight: 400;
  text-transform: uppercase; letter-spacing: 0.14em; color: var(--ink-3);
  text-align: left; padding: 12px 10px; border-bottom: 1px solid var(--rule-2);
}
.vf-table thead th.num { text-align: right; }
.vf-table tbody tr {
  border-bottom: 1px solid var(--rule-1);
}
.vf-table tbody tr:hover { background: var(--paper-1); }
.vf-table tbody td {
  padding: 14px 10px; vertical-align: top; font-size: 14px;
}
.vf-table td.vf-id {
  font-family: var(--font-mono); font-size: 11px; color: var(--ink-3);
  white-space: nowrap; width: 130px;
}
.vf-table td.vf-cls {
  font-family: var(--font-sans); font-size: 11px;
  text-transform: uppercase; letter-spacing: 0.1em;
  color: var(--ink-3); white-space: nowrap; width: 110px;
}
.vf-table td.vf-claim {
  font-family: var(--font-serif); font-size: 15px; color: var(--ink-0);
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
  background: var(--ink-2);
}
.vf-num {
  font-family: var(--font-mono); font-size: 12px;
  color: var(--ink-2); font-variant-numeric: tabular-nums;
}

/* Mobile fallback for the workbench rim */
@media (max-width: 720px) {
  .wb { grid-template-columns: 0 1fr 0 !important; }
  .wb-rim { display: none !important; }
  .wb-head, .wb-main, .wb-foot { padding-left: 20px !important; padding-right: 20px !important; }
  .vf-table td.vf-cls, .vf-table thead th:nth-child(2) { display: none; }
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

    let findings_section = render_findings_section(frontier);

    let main = format!(
        r#"<div class="fd">
  <article>
    <p class="fd-claim">{name}</p>
    <p class="fd-note">{note}</p>

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
            r#"<a href="/entries">← Entries</a><span>·</span><a href="/entries/{vfr_safe}">JSON</a>"#
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
fn finding_state(b: &vela_protocol::bundle::FindingBundle) -> (&'static str, &'static str) {
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
                if b.evidence.replicated {
                    return ("replicated", "ok");
                }
                return ("supported", "ok");
            }
        }
    }
    if b.flags.contested {
        return ("contested", "warn");
    }
    if b.evidence.replicated {
        return ("replicated", "ok");
    }
    ("supported", "ok")
}

fn render_findings_section(frontier: Option<&Project>) -> String {
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
                *acc.entry(finding_state(b).0).or_default() += 1;
                acc
            });
    let counts = by_state
        .iter()
        .map(|(label, n)| format!("{n} {label}"))
        .collect::<Vec<_>>()
        .join(" · ");

    let mut rows = String::new();
    for b in &p.findings {
        let (label, state_class) = finding_state(b);
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
        rows.push_str(&format!(
            r#"<tr>
              <td class="vf-id">{vf_id}</td>
              <td class="vf-cls">{cls}</td>
              <td class="vf-claim">{claim}</td>
              <td class="vf-state"><span class="wb-chip{live_class}" style="--chip:var(--state-{state_class});"><span class="wb-chip__dot"></span>{label}</span></td>
              <td class="vf-conf"><span class="vf-bar"><i style="width:{pct}%;"></i></span><span class="vf-num">{score:.2}</span></td>
            </tr>"#,
            vf_id = escape_html(&b.id),
            cls = escape_html(assertion_type),
            claim = escape_html(&b.assertion.text),
            score = b.confidence.score,
        ));
    }

    format!(
        r#"<section class="wb-section">
          <div class="wb-section__head">
            <span class="wb-section__num">§1</span>
            <span class="wb-section__t">Findings · {count}</span>
            <span class="wb-section__aside">{counts}</span>
          </div>
          <table class="vf-table">
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
