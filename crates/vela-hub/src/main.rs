//! Vela hub: read-only HTTP index over verified Git frontiers, canonical
//! event logs, and materialized frontier projections.
//!
//! Doctrine: Git stores the bytes; the hub verifies each configured frontier
//! and indexes its event/projection tables. It does not mirror substrate bytes
//! or accept frontier state over an HTTP publication transport.
//!
//! Endpoints:
//!   GET  /entries                   - configured, verified frontier indexes
//!   GET  /entries/{vfr_id}          - one verified frontier index
//!   GET  /entries/{vfr_id}/events   - cursor-paginated event log
//!   GET  /entries/{vfr_id}/snapshot - derived materialized snapshot
//!   (publication is `git push`; the ingest loop re-derives the index from
//!   the operator's versioned Git source catalog)
//!   GET  /healthz                   - liveness
//!   GET  /                          - banner + endpoint list

use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tokio::sync::RwLock;

// DB and ingest modules are exposed via the lib so the read surface and
// verified Git ingestor share one projection implementation.
use db::{HubDb, ensure_postgres_event_first_schema, ensure_sqlite_schema};
use tower_http::cors::CorsLayer;
mod review;
use review::*;

use vela_hub::db;
use vela_protocol::project::Project;

const HUB_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_REQUEST_BODY_BYTES: usize = 128 * 1024 * 1024;
const FRONTIER_INDEX_LIST_SCHEMA: &str = "vela.frontier-index-list.v1";

const DEFAULT_PUBLIC_URL: &str = "https://hub.constellate.science";
const DEFAULT_REPO_URL: &str = "https://github.com/vela-science/vela";
const DEFAULT_SITE_URL: &str = "https://app.constellate.science";

/// Cache key: (vfr_id, snapshot_hash). A changed verified projection gets a
/// new content hash, so the next read re-fetches.
type FrontierCache = Arc<RwLock<HashMap<(String, String), Arc<Project>>>>;

/// Public URL strings the hub quotes back to clients: the JSON banners,
/// the `/.well-known/vela` manifest, and the 301 redirects into the app
/// (`site`). Sourced at startup from env vars (`VELA_HUB_PUBLIC_URL`,
/// `VELA_REPO_URL`, `VELA_SITE_URL`) with hardcoded defaults that match
/// the production deploy. Changing the deploy target is one secret-set
/// away.
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
}

#[derive(Clone)]
struct AppState {
    /// v0.21: backend-agnostic DB handle. Postgres for production
    /// (vela-hub.fly.dev / vela-hub-2.fly.dev), SQLite for self-hosted
    /// laptop runs. Variant chosen at startup from URL prefix.
    db: HubDb,
    /// Frontier cache for the entry detail page. Keyed by
    /// `(vfr_id, snapshot_hash)` so a changed projection forces a
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
    /// v0.49.1: hit/miss/stale counters for the DB cache. Surfaced at
    /// `/healthz` so an operator can monitor degradation.
    db_cache_metrics: Arc<DbCacheMetrics>,
    /// Per-route request metrics (count by status class + latency
    /// histogram), recorded by the router-level middleware and rendered
    /// at `/metrics` alongside the db-cache series. Keyed by the matched
    /// route TEMPLATE (`/entries/{vfr_id}`), never the raw path — raw
    /// paths would explode label cardinality.
    http_metrics: Arc<HttpMetrics>,
    /// Public-facing URLs the rendered HTML quotes back to readers.
    /// Configurable via env so the same binary serves any deployment.
    urls: PublicUrls,
    /// v0.727: the hosted MCP service, hot-swapped by the per-machine
    /// refresher (`mcp_host`). `None` until the first refresh lands.
    mcp: vela_hub::mcp_host::SharedMcp,
    /// Kicks the MCP refresher ahead of its interval (webhook lane).
    mcp_kick: Arc<tokio::sync::Notify>,
    /// v0.727: shared secret for `POST /webhook/github` (HMAC-SHA256 over
    /// the raw body, GitHub's `X-Hub-Signature-256`). Absent ⇒ the
    /// webhook lane answers 503 and the interval sweeps remain the only
    /// refresh path.
    webhook_secret: Option<Arc<String>>,
    /// Per-IP sliding-window rate limiter (protocol-node hygiene).
    /// `VELA_HUB_RATE_LIMIT_PER_MIN` configures the default GET budget;
    /// 0 disables the limiter entirely.
    rate_limiter: Arc<RateLimiter>,
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

/// v0.49.1: counters for the DB-cache fast/slow paths so an operator
/// can see at a glance whether the registry is healthy or limping.
/// `hits` are fresh-window cache hits (served without touching the
/// DB). `misses` are misses that fell through to the DB and the DB
/// answered. `stale_hits` are misses where the DB errored *and* we
/// served the last-known-good payload with `X-Vela-Stale: 1`.
///
/// The crucial signal for production: a sustained rise in `stale_hits`
/// means Postgres is failing repeatedly and the registry is degrading.
/// The cache is buying time, not papering over a healthy backend.
///
/// v0.49.2: per-bucket histogram of stale-age in seconds so an
/// operator can distinguish "we served stale 30 s ago" from "we've
/// been serving 28-min-stale data" — both increment `stale_hits`,
/// but only the second is reason to page someone.
#[derive(Default)]
struct DbCacheMetrics {
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,
    stale_hits: std::sync::atomic::AtomicU64,
    db_errors: std::sync::atomic::AtomicU64,
    /// Histogram buckets for stale-age in seconds. Indexes correspond
    /// to STALE_AGE_BUCKETS upper bounds (final bucket is "+Inf").
    stale_age_buckets: [std::sync::atomic::AtomicU64; STALE_AGE_BUCKETS.len() + 1],
}

/// Stale-age histogram bucket upper bounds, in seconds. Chosen to
/// straddle the fresh window (60 s), short outage (5 min), and the
/// stale window itself (30 min).
const STALE_AGE_BUCKETS: [u64; 6] = [60, 120, 300, 600, 1200, 1800];

impl DbCacheMetrics {
    fn record_hit(&self) {
        self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn record_miss(&self) {
        self.misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn record_stale_hit(&self, age_secs: u64) {
        self.stale_hits
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bucket_idx = STALE_AGE_BUCKETS
            .iter()
            .position(|&b| age_secs <= b)
            .unwrap_or(STALE_AGE_BUCKETS.len());
        self.stale_age_buckets[bucket_idx].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn record_db_error(&self) {
        self.db_errors
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn snapshot(&self) -> Value {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let stale_hits = self.stale_hits.load(std::sync::atomic::Ordering::Relaxed);
        let db_errors = self.db_errors.load(std::sync::atomic::Ordering::Relaxed);
        let total_serves = hits + misses + stale_hits;
        let stale_hit_rate = if total_serves == 0 {
            0.0
        } else {
            stale_hits as f64 / total_serves as f64
        };

        // Histogram snapshot: cumulative buckets in Prometheus style
        // (each bucket counts every observation ≤ its upper bound).
        let raw: Vec<u64> = self
            .stale_age_buckets
            .iter()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .collect();
        let mut cumulative = 0u64;
        let mut buckets_obj = serde_json::Map::new();
        for (i, &bound) in STALE_AGE_BUCKETS.iter().enumerate() {
            cumulative += raw[i];
            buckets_obj.insert(format!("le_{bound}s"), json!(cumulative));
        }
        cumulative += raw[STALE_AGE_BUCKETS.len()];
        buckets_obj.insert("le_inf".to_string(), json!(cumulative));

        json!({
            "hits": hits,
            "misses": misses,
            "stale_hits": stale_hits,
            "db_errors": db_errors,
            "total_serves": total_serves,
            "stale_hit_rate": stale_hit_rate,
            "stale_age_seconds": buckets_obj,
        })
    }

    /// Render the cache metrics as Prometheus 0.0.4 text format. The
    /// shape `vela_hub_db_cache_*` is namespaced so a multi-hub
    /// scrape can pull this hub alongside others without collision.
    fn render_prometheus(&self) -> String {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let stale_hits = self.stale_hits.load(std::sync::atomic::Ordering::Relaxed);
        let db_errors = self.db_errors.load(std::sync::atomic::Ordering::Relaxed);
        let total_serves = hits + misses + stale_hits;
        let stale_hit_rate = if total_serves == 0 {
            0.0
        } else {
            stale_hits as f64 / total_serves as f64
        };
        let mut out = String::new();
        out.push_str("# HELP vela_hub_db_cache_hits_total Cache fresh-window hits served without touching the DB.\n");
        out.push_str("# TYPE vela_hub_db_cache_hits_total counter\n");
        out.push_str(&format!("vela_hub_db_cache_hits_total {hits}\n"));
        out.push_str("# HELP vela_hub_db_cache_misses_total Cache misses that fell through to the DB and the DB answered.\n");
        out.push_str("# TYPE vela_hub_db_cache_misses_total counter\n");
        out.push_str(&format!("vela_hub_db_cache_misses_total {misses}\n"));
        out.push_str("# HELP vela_hub_db_cache_stale_hits_total Cache misses served stale because the DB errored within the stale window.\n");
        out.push_str("# TYPE vela_hub_db_cache_stale_hits_total counter\n");
        out.push_str(&format!(
            "vela_hub_db_cache_stale_hits_total {stale_hits}\n"
        ));
        out.push_str("# HELP vela_hub_db_errors_total Distinct DB query errors observed by the cache layer.\n");
        out.push_str("# TYPE vela_hub_db_errors_total counter\n");
        out.push_str(&format!("vela_hub_db_errors_total {db_errors}\n"));
        out.push_str("# HELP vela_hub_db_cache_stale_hit_rate Stale hits as a fraction of total cache serves.\n");
        out.push_str("# TYPE vela_hub_db_cache_stale_hit_rate gauge\n");
        out.push_str(&format!(
            "vela_hub_db_cache_stale_hit_rate {stale_hit_rate}\n"
        ));

        // Stale-age histogram, cumulative buckets per Prometheus convention.
        out.push_str("# HELP vela_hub_db_cache_stale_age_seconds Stale-age distribution (seconds since last good fetch) for stale serves.\n");
        out.push_str("# TYPE vela_hub_db_cache_stale_age_seconds histogram\n");
        let raw: Vec<u64> = self
            .stale_age_buckets
            .iter()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .collect();
        let mut cumulative = 0u64;
        for (i, &bound) in STALE_AGE_BUCKETS.iter().enumerate() {
            cumulative += raw[i];
            out.push_str(&format!(
                "vela_hub_db_cache_stale_age_seconds_bucket{{le=\"{bound}\"}} {cumulative}\n"
            ));
        }
        cumulative += raw[STALE_AGE_BUCKETS.len()];
        out.push_str(&format!(
            "vela_hub_db_cache_stale_age_seconds_bucket{{le=\"+Inf\"}} {cumulative}\n"
        ));
        out.push_str(&format!(
            "vela_hub_db_cache_stale_age_seconds_count {cumulative}\n"
        ));
        out
    }
}

/// Latency histogram bucket upper bounds, in seconds. Chosen to straddle
/// the cache fast path (single-digit ms), a healthy Postgres read
/// (tens of ms), and a cold projection hydrate (seconds).
const HTTP_LATENCY_BUCKETS_SECS: [f64; 9] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0];

/// One route template + method's counters: requests by status class and
/// the latency histogram (raw per-bucket counts; render cumulates them
/// in Prometheus style).
#[derive(Default)]
struct HttpRouteStat {
    /// Indexes 0..=4 ⇔ status classes 1xx..5xx.
    by_class: [u64; 5],
    latency_buckets: [u64; HTTP_LATENCY_BUCKETS_SECS.len() + 1],
    latency_sum_secs: f64,
    latency_count: u64,
}

/// Hand-rolled per-route request metrics, same register as
/// `DbCacheMetrics`: no metrics crate, Prometheus 0.0.4 text on render.
/// A `Mutex<HashMap>` is deliberate — the hub's request volume is far
/// below where lock contention matters, and the route-template key set
/// is small and bounded by the router itself.
#[derive(Default)]
struct HttpMetrics {
    routes: std::sync::Mutex<HashMap<(String, String), HttpRouteStat>>,
}

impl HttpMetrics {
    fn record(&self, method: &str, route: &str, status: u16, latency_secs: f64) {
        let mut routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        let stat = routes
            .entry((method.to_string(), route.to_string()))
            .or_default();
        let class_idx = (usize::from(status) / 100).clamp(1, 5) - 1;
        stat.by_class[class_idx] += 1;
        let bucket_idx = HTTP_LATENCY_BUCKETS_SECS
            .iter()
            .position(|&bound| latency_secs <= bound)
            .unwrap_or(HTTP_LATENCY_BUCKETS_SECS.len());
        stat.latency_buckets[bucket_idx] += 1;
        stat.latency_sum_secs += latency_secs;
        stat.latency_count += 1;
    }

    /// Render as Prometheus 0.0.4 text, `vela_hub_http_*` namespaced.
    /// Keys are sorted so consecutive scrapes diff cleanly.
    fn render_prometheus(&self) -> String {
        let routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        let mut keys: Vec<&(String, String)> = routes.keys().collect();
        keys.sort();

        let mut out = String::new();
        out.push_str("# HELP vela_hub_http_requests_total Requests served, by route template, method, and status class.\n");
        out.push_str("# TYPE vela_hub_http_requests_total counter\n");
        for key in &keys {
            let (method, route) = (key.0.as_str(), key.1.as_str());
            let stat = &routes[*key];
            for (idx, &count) in stat.by_class.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                out.push_str(&format!(
                    "vela_hub_http_requests_total{{method=\"{method}\",route=\"{route}\",class=\"{}xx\"}} {count}\n",
                    idx + 1,
                ));
            }
        }

        out.push_str("# HELP vela_hub_http_request_duration_seconds Request latency by route template and method.\n");
        out.push_str("# TYPE vela_hub_http_request_duration_seconds histogram\n");
        for key in &keys {
            let (method, route) = (key.0.as_str(), key.1.as_str());
            let stat = &routes[*key];
            let mut cumulative = 0u64;
            for (idx, &bound) in HTTP_LATENCY_BUCKETS_SECS.iter().enumerate() {
                cumulative += stat.latency_buckets[idx];
                out.push_str(&format!(
                    "vela_hub_http_request_duration_seconds_bucket{{method=\"{method}\",route=\"{route}\",le=\"{bound}\"}} {cumulative}\n",
                ));
            }
            cumulative += stat.latency_buckets[HTTP_LATENCY_BUCKETS_SECS.len()];
            out.push_str(&format!(
                "vela_hub_http_request_duration_seconds_bucket{{method=\"{method}\",route=\"{route}\",le=\"+Inf\"}} {cumulative}\n",
            ));
            out.push_str(&format!(
                "vela_hub_http_request_duration_seconds_sum{{method=\"{method}\",route=\"{route}\"}} {}\n",
                stat.latency_sum_secs,
            ));
            out.push_str(&format!(
                "vela_hub_http_request_duration_seconds_count{{method=\"{method}\",route=\"{route}\"}} {}\n",
                stat.latency_count,
            ));
        }
        out
    }
}

#[cfg(test)]
mod http_metrics_tests {
    use super::*;

    #[test]
    fn record_and_render_by_route_template() {
        let metrics = HttpMetrics::default();
        metrics.record("GET", "/entries/{vfr_id}", 200, 0.003);
        metrics.record("GET", "/entries/{vfr_id}", 200, 0.040);
        metrics.record("GET", "/entries/{vfr_id}", 404, 0.002);
        metrics.record("POST", "/mcp", 503, 0.001);

        let out = metrics.render_prometheus();
        // Counters carry the route TEMPLATE and the status class.
        assert!(out.contains(
            r#"vela_hub_http_requests_total{method="GET",route="/entries/{vfr_id}",class="2xx"} 2"#
        ));
        assert!(out.contains(
            r#"vela_hub_http_requests_total{method="GET",route="/entries/{vfr_id}",class="4xx"} 1"#
        ));
        assert!(
            out.contains(
                r#"vela_hub_http_requests_total{method="POST",route="/mcp",class="5xx"} 1"#
            )
        );
        // Histogram buckets are cumulative and end at +Inf = count.
        assert!(out.contains(
            r#"vela_hub_http_request_duration_seconds_bucket{method="GET",route="/entries/{vfr_id}",le="0.005"} 2"#
        ));
        assert!(out.contains(
            r#"vela_hub_http_request_duration_seconds_bucket{method="GET",route="/entries/{vfr_id}",le="+Inf"} 3"#
        ));
        assert!(out.contains(
            r#"vela_hub_http_request_duration_seconds_count{method="GET",route="/entries/{vfr_id}"} 3"#
        ));
    }
}

/// Router-level metrics middleware. Added via `route_layer`, so it runs
/// AFTER routing and can read the `MatchedPath` extension — the route
/// TEMPLATE — instead of the raw path. Unmatched requests (the 404
/// fallback) never reach a route layer and are deliberately unrecorded:
/// arbitrary probe paths must not mint label values.
async fn http_metrics_mw(
    State(metrics): State<Arc<HttpMetrics>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let route = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "(unmatched)".to_string());
    let method = req.method().as_str().to_string();
    let started = std::time::Instant::now();
    let response = next.run(req).await;
    metrics.record(
        &method,
        &route,
        response.status().as_u16(),
        started.elapsed().as_secs_f64(),
    );
    response
}

// ─── Rate limiting ────────────────────────────────────────────────────
//
// Hand-rolled per-IP sliding window, same register as `HttpMetrics`: no
// middleware crate, a `Mutex<HashMap>` because the hub's request volume
// is far below where lock contention matters. Budgets are per (client
// IP, route class) so a GET flood cannot starve the POST lanes.

const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RATE_LIMIT_DEFAULT_PER_MIN: u32 = 120;
const RATE_LIMIT_MCP_PER_MIN: u32 = 20;
const RATE_LIMIT_WEBHOOK_PER_MIN: u32 = 60;
/// Operational endpoints are exempt entirely: health checks and the
/// Prometheus scraper must never be starved by an abusive client
/// sharing a NAT'd IP with them.
const RATE_LIMIT_EXEMPT_ROUTES: [&str; 3] = ["/healthz", "/readyz", "/metrics"];

struct RateLimiter {
    /// The default budget per window (GETs and any route without its own
    /// class). 0 ⇒ the limiter is disabled.
    default_per_min: u32,
    window: Duration,
    hits: std::sync::Mutex<
        HashMap<(IpAddr, &'static str), std::collections::VecDeque<std::time::Instant>>,
    >,
    /// Requests refused with 429, exposed at `/metrics`.
    rate_limited_total: std::sync::atomic::AtomicU64,
}

impl RateLimiter {
    fn new(default_per_min: u32) -> Self {
        Self {
            default_per_min,
            window: RATE_LIMIT_WINDOW,
            hits: std::sync::Mutex::new(HashMap::new()),
            rate_limited_total: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// `VELA_HUB_RATE_LIMIT_PER_MIN`: 0 disables; unset/unparsable ⇒ 120.
    fn from_env() -> Self {
        let per_min = env::var("VELA_HUB_RATE_LIMIT_PER_MIN")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(RATE_LIMIT_DEFAULT_PER_MIN);
        Self::new(per_min)
    }

    fn disabled(&self) -> bool {
        self.default_per_min == 0
    }

    /// The (class, budget) for a matched route template, or `None` when
    /// the route is exempt from rate limiting.
    fn class_limit(&self, method: &str, route: &str) -> Option<(&'static str, u32)> {
        if RATE_LIMIT_EXEMPT_ROUTES.contains(&route) {
            return None;
        }
        match (method, route) {
            ("POST", "/mcp") => Some(("mcp", RATE_LIMIT_MCP_PER_MIN)),
            // HMAC'd, but still bounded: a stolen secret must not buy
            // an unmetered ingest-kick lane.
            ("POST", "/webhook/github") => Some(("webhook", RATE_LIMIT_WEBHOOK_PER_MIN)),
            _ => Some(("default", self.default_per_min)),
        }
    }

    /// Admit or refuse one request at `now`. `Err(retry_after_secs)`
    /// means over budget. Window math is sliding: a hit expires exactly
    /// `window` after it was recorded.
    fn check_at(
        &self,
        ip: IpAddr,
        class: &'static str,
        limit: u32,
        now: std::time::Instant,
    ) -> Result<(), u64> {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        // Periodic pruning: when the map grows past a bound, sweep out
        // every expired hit and drop empty keys so one-shot scanners
        // cannot grow the map without bound.
        if hits.len() > 4096 {
            hits.retain(|_, q| {
                while q
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= self.window)
                {
                    q.pop_front();
                }
                !q.is_empty()
            });
        }
        let q = hits.entry((ip, class)).or_default();
        while q
            .front()
            .is_some_and(|t| now.duration_since(*t) >= self.window)
        {
            q.pop_front();
        }
        if (q.len() as u32) < limit {
            q.push_back(now);
            return Ok(());
        }
        let oldest = *q.front().expect("deque non-empty when at limit");
        let remaining = self.window.saturating_sub(now.duration_since(oldest));
        self.rate_limited_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Err(remaining.as_secs().max(1))
    }

    fn render_prometheus(&self) -> String {
        let total = self
            .rate_limited_total
            .load(std::sync::atomic::Ordering::Relaxed);
        format!(
            "# HELP vela_hub_rate_limited_total Requests refused with 429 by the per-IP sliding-window rate limiter.\n\
             # TYPE vela_hub_rate_limited_total counter\n\
             vela_hub_rate_limited_total {total}\n"
        )
    }
}

/// The client IP for rate limiting. Fly terminates TLS and sets
/// `Fly-Client-IP`; prefer it, then the first `X-Forwarded-For` hop,
/// then the socket peer address.
fn client_ip(req: &axum::extract::Request) -> Option<IpAddr> {
    let headers = req.headers();
    if let Some(v) = headers.get("fly-client-ip").and_then(|v| v.to_str().ok())
        && let Ok(ip) = v.trim().parse()
    {
        return Some(ip);
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(first) = v.split(',').next()
        && let Ok(ip) = first.trim().parse()
    {
        return Some(ip);
    }
    req.extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
}

/// Router-level rate-limit middleware. Added via `route_layer` INSIDE the
/// metrics layer (metrics added last ⇒ outermost), so a 429 is still
/// recorded in the per-route request counters.
async fn rate_limit_mw(
    State(limiter): State<Arc<RateLimiter>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if limiter.disabled() {
        return next.run(req).await;
    }
    let route = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let Some((class, limit)) = limiter.class_limit(req.method().as_str(), &route) else {
        return next.run(req).await;
    };
    // No resolvable client IP (shouldn't happen behind Fly or the local
    // listener) fails open: rate limiting is hygiene, not custody.
    let Some(ip) = client_ip(&req) else {
        return next.run(req).await;
    };
    match limiter.check_at(ip, class, limit, std::time::Instant::now()) {
        Ok(()) => next.run(req).await,
        Err(retry_after_secs) => {
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(error_body(
                    "RATE_LIMITED",
                    format!(
                        "rate limit exceeded ({limit} requests per {}s for this endpoint class); retry in {retry_after_secs}s",
                        RATE_LIMIT_WINDOW.as_secs()
                    ),
                )),
            )
                .into_response();
            if let Ok(v) = axum::http::HeaderValue::from_str(&retry_after_secs.to_string()) {
                resp.headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, v);
            }
            resp
        }
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    use super::*;
    use std::time::Instant;

    fn ip() -> IpAddr {
        "203.0.113.7".parse().unwrap()
    }

    #[test]
    fn window_math_admits_then_refuses_then_recovers() {
        let rl = RateLimiter::new(3);
        let t0 = Instant::now();
        for _ in 0..3 {
            assert!(rl.check_at(ip(), "default", 3, t0).is_ok());
        }
        // Fourth hit inside the window is refused, with a sane Retry-After.
        let retry = rl.check_at(ip(), "default", 3, t0).unwrap_err();
        assert!((1..=60).contains(&retry), "retry-after in (0, 60]: {retry}");
        assert_eq!(
            rl.rate_limited_total
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        // Mid-window: still refused, Retry-After shrinks with time left.
        let retry_mid = rl
            .check_at(ip(), "default", 3, t0 + Duration::from_secs(30))
            .unwrap_err();
        assert!(retry_mid <= 30, "half the window elapsed: {retry_mid}");
        // After the window slides past the oldest hit, admitted again.
        assert!(
            rl.check_at(ip(), "default", 3, t0 + Duration::from_secs(61))
                .is_ok()
        );
    }

    #[test]
    fn classes_have_independent_budgets() {
        let rl = RateLimiter::new(2);
        let t0 = Instant::now();
        assert!(rl.check_at(ip(), "default", 2, t0).is_ok());
        assert!(rl.check_at(ip(), "default", 2, t0).is_ok());
        assert!(rl.check_at(ip(), "default", 2, t0).is_err());
        // The mcp class for the same IP is untouched.
        assert!(rl.check_at(ip(), "mcp", 20, t0).is_ok());
    }

    #[test]
    fn exemptions_and_route_classes() {
        let rl = RateLimiter::new(120);
        assert!(rl.class_limit("GET", "/healthz").is_none());
        assert!(rl.class_limit("GET", "/readyz").is_none());
        assert!(rl.class_limit("GET", "/metrics").is_none());
        assert_eq!(rl.class_limit("POST", "/mcp"), Some(("mcp", 20)));
        assert_eq!(
            rl.class_limit("POST", "/webhook/github"),
            Some(("webhook", 60))
        );
        assert_eq!(rl.class_limit("GET", "/entries"), Some(("default", 120)));
        assert_eq!(
            rl.class_limit("GET", "/entries/{vfr_id}"),
            Some(("default", 120))
        );
    }

    #[test]
    fn zero_disables() {
        let rl = RateLimiter::new(0);
        assert!(rl.disabled());
    }
}

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
        // v0.230: opportunistic schema migration. If the connected role
        // has DDL privileges, apply the event-first schema (idempotent
        // — every CREATE is IF NOT EXISTS). If it lacks DDL perms
        // (production Neon hub uses a least-privilege role; schema is
        // applied separately by a privileged migration job), log a
        // warning and continue. The schema_present() check below still
        // enforces that the core tables exist.
        if let Err(e) = ensure_postgres_event_first_schema(&pool).await {
            tracing::warn!(error = %e, "skipping auto-migration; ensure DDL has been applied via privileged role");
        }
        let h = HubDb::Postgres(pool);
        // Sanity-check schema presence so we fail fast on a misconfigured DB.
        let table_exists = h
            .schema_present()
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        if !table_exists {
            return Err(
                "frontiers table not found; run the schema migration before starting the hub"
                    .into(),
            );
        }
        tracing::info!("vela-hub using Postgres backend");
        h
    };

    let source_catalog = vela_hub::git_ingest::load_source_catalog()
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    for source in &source_catalog.sources {
        db.upsert_git_source(&source.vfr_id, &source.git_remote, &source.git_ref)
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    }
    let configured_ids: Vec<String> = source_catalog
        .sources
        .iter()
        .map(|source| source.vfr_id.clone())
        .collect();
    let pruned = db
        .retain_git_sources(&configured_ids)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    tracing::info!(
        sources = source_catalog.sources.len(),
        pruned,
        schema = %source_catalog.schema,
        "Hub Git source catalog loaded"
    );

    let urls = PublicUrls::from_env();

    let state = AppState {
        db,
        frontier_cache: Arc::new(RwLock::new(HashMap::new())),
        db_cache: Arc::new(RwLock::new(HashMap::new())),
        db_cache_metrics: Arc::new(DbCacheMetrics::default()),
        http_metrics: Arc::new(HttpMetrics::default()),
        urls,
        mcp: Arc::new(tokio::sync::RwLock::new(None)),
        mcp_kick: Arc::new(tokio::sync::Notify::new()),
        webhook_secret: env::var("VELA_HUB_WEBHOOK_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::new),
        rate_limiter: Arc::new(RateLimiter::from_env()),
    };
    if state.rate_limiter.disabled() {
        tracing::warn!("VELA_HUB_RATE_LIMIT_PER_MIN=0; per-IP rate limiting disabled");
    }
    if state.webhook_secret.is_none() {
        tracing::info!(
            "no VELA_HUB_WEBHOOK_SECRET set; /webhook/github disabled (interval sweeps only)"
        );
    }

    // Git ingestion (ADR 0001 / HUB.md): re-derive the index from catalogued
    // frontier Git repos on an interval. The repo is the authority; this
    // loop only refreshes the projection.
    vela_hub::git_ingest::spawn(
        state.db.clone(),
        vela_hub::git_ingest::GitIngestConfig::from_env(),
    );

    // The hosted MCP lane: per-machine refresher hydrating the in-process
    // serve dispatcher behind /mcp from the live projection tables.
    // Read-only by construction.
    vela_hub::mcp_host::spawn(state.db.clone(), state.mcp.clone(), state.mcp_kick.clone());

    let port: u16 = env::var("VELA_HUB_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3849);
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();

    let app = build_router(state);

    tracing::info!("vela-hub {HUB_VERSION} listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // ConnectInfo gives the rate limiter its socket-address fallback when
    // no proxy header names the client.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

/// The whole HTTP surface. Factored out of `main` so the in-process
/// tests exercise the same router (routes + middleware) the binary runs.
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics_prometheus))
        .route("/.well-known/vela", get(well_known_vela))
        .route("/entries", get(list_entries))
        .route("/entries/{vfr_id}", get(get_entry))
        .route("/entries/{vfr_id}/git-remote", get(get_git_remote))
        .route("/entries/{vfr_id}/snapshot", get(get_entry_snapshot))
        .route(
            "/entries/{vfr_id}/sidon-frontier-map",
            get(get_sidon_frontier_map),
        )
        .route(
            "/entries/{vfr_id}/sidon-observation",
            get(get_sidon_observation),
        )
        .route("/entries/{vfr_id}/summary", get(get_entry_summary))
        .route("/entries/{vfr_id}/manifest", get(get_entry_manifest))
        .route("/producers/{pubkey}", get(get_producer))
        .route("/search", get(search_endpoint))
        .route("/entries/{vfr_id}/objects/{otype}", get(get_entry_objects))
        .route(
            "/entries/{vfr_id}/objects/{otype}/{object_id}",
            get(get_entry_object),
        )
        .route("/entries/{vfr_id}/events", get(get_entry_events))
        // Read-only Evidence Diff: a pending proposal's before/after effect
        // on its target claim plus downstream impact. Pure projection over
        // the materialized state. Truth-bearing writes (propose / accept /
        // append) are no longer served here: the hub is a read-only index,
        // and acceptance is a signed review event landed via a git-native
        // frontier PR, not an HTTP endpoint.
        .route(
            "/entries/{vfr_id}/proposals/{proposal_id}/evidence-diff",
            get(get_proposal_evidence_diff),
        )
        .route(
            "/entries/{vfr_id}/events/stream",
            get(get_entry_events_stream),
        )
        .route("/frontier/{vfr_id}/inbox", get(get_entry_events_stream))
        .route("/entries/{vfr_id}/depends-on", get(get_depends_on))
        .route("/entries/{vfr_id}/graph", get(get_entry_graph))
        .route("/entries/{vfr_id}/frontier", get(get_entry_frontier))
        .route("/entries/{vfr_id}/boundary", get(get_entry_boundary))
        .route("/diff-packs/{pack_id}", get(get_diff_pack))
        .route("/entries/{vfr_id}/packs/{pack_id}", get(get_pack_review))
        .route("/entries/{vfr_id}/reproduce", get(get_reproduce))
        .route("/entries/{vfr_id}/review", get(get_entry_review))
        .route("/entries/{vfr_id}/findings/{vf_id}", get(get_finding))
        .route(
            "/entries/{vfr_id}/findings/{vf_id}/context",
            get(get_finding_context),
        )
        .route(
            "/entries/{vfr_id}/findings/{vf_id}/gate-status",
            get(get_finding_gate_status),
        )
        .route(
            "/entries/{vfr_id}/gate-status",
            get(get_frontier_gate_status),
        )
        // v0.727: the hosted MCP endpoint (streamable HTTP, stateless
        // JSON, read-only profile) and the GitHub webhook that kicks
        // ingest + MCP refresh ahead of the interval sweeps.
        .route("/mcp", post(post_mcp).get(get_mcp))
        .route("/webhook/github", post(post_webhook_github))
        // Per-IP rate limiting. route_layer so MatchedPath is visible for
        // the exemption/class table; added BEFORE the metrics layer so
        // metrics wraps it and a 429 still lands in the request counters.
        .route_layer(axum::middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            rate_limit_mw,
        ))
        // Per-route request metrics. route_layer (not layer) so the
        // middleware runs after routing and sees MatchedPath.
        .route_layer(axum::middleware::from_fn_with_state(
            state.http_metrics.clone(),
            http_metrics_mw,
        ))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// POST /mcp — the hosted MCP endpoint: streamable HTTP with stateless
/// JSON responses, read-only profile, over this machine's frontier
/// checkouts. 503 until the first refresh lands.
async fn post_mcp(State(state): State<AppState>, body: String) -> Response {
    let guard = state.mcp.read().await;
    let Some(service) = guard.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "jsonrpc": "2.0", "id": null,
                "error": {"code": -32000, "message": "MCP projection not built yet; retry shortly (or no frontier is registered)"}
            })),
        )
            .into_response();
    };
    let (status, response) = service.handle_http(&body).await;
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    match response {
        Some(value) => (status, Json(value)).into_response(),
        None => status.into_response(),
    }
}

/// GET /mcp — no server-initiated SSE stream is offered.
async fn get_mcp() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({
            "error": {"kind": "INVALID_ARG", "message": "stateless MCP endpoint: POST a JSON-RPC message; no server-initiated stream is offered"}
        })),
    )
}

/// Verify GitHub's `X-Hub-Signature-256` header (`sha256=<hex>`) over the
/// raw request body. Constant-time comparison via the Mac verifier.
fn github_signature_ok(secret: &str, body: &[u8], header: &str) -> bool {
    use hmac::{Hmac, Mac};
    let Some(hex_sig) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(sig) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

/// POST /webhook/github — push events kick the MCP refresher and a DB
/// ingest sweep ahead of the interval, so `git push` reflects in seconds.
/// The webhook is a LATENCY lane only: authenticity of state still comes
/// from strict replay of the signed event log, never from this header.
async fn post_webhook_github(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(secret) = state.webhook_secret.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "webhook lane not configured (set VELA_HUB_WEBHOOK_SECRET)",
            )),
        );
    };
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !github_signature_ok(&secret, &body, signature) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "PERMISSION_DENIED",
                "invalid or missing X-Hub-Signature-256",
            )),
        );
    }
    let event = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("push")
        .to_string();
    if event == "ping" {
        return (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "pong": true})),
        );
    }
    let db = state.db.clone();
    let mcp_kick = state.mcp_kick.clone();
    tokio::spawn(async move {
        match vela_hub::git_ingest::run_once(
            &db,
            &vela_hub::git_ingest::GitIngestConfig::from_env(),
        )
        .await
        {
            Ok(n) if n > 0 => tracing::info!(promoted = n, "webhook-triggered ingest complete"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "webhook-triggered ingest failed"),
        }
        // Kick the MCP refresher only AFTER the ingest sweep finishes:
        // the refresher hydrates from the DB, so kicking before the push
        // was promoted would rebuild yesterday's projection. Kicked even
        // on a sweep error — the refresh no-ops on unchanged hashes.
        mcp_kick.notify_one();
    });
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"accepted": true, "event": event})),
    )
}

/// usually omit the header or send `*/*`. We redirect to the app only
/// when the client explicitly asks for HTML.
fn wants_html(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.contains("text/html"))
}

/// The one browser exit (protocol-only hub): a permanent redirect into
/// the app at `{VELA_SITE_URL}{path}`. The hub serves protocol JSON;
/// any client that renders it is a viewer, and app.constellate.science
/// is the reference viewer. The body names the target for curl users
/// who sent `Accept: text/html` without following redirects.
fn redirect_to_site(urls: &PublicUrls, path: &str) -> Response {
    let target = format!("{}{}", urls.site, path);
    (
        StatusCode::MOVED_PERMANENTLY,
        [
            (axum::http::header::LOCATION, target.clone()),
            (
                axum::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8".to_string(),
            ),
        ],
        format!("moved permanently: this page now lives at {target}\n"),
    )
        .into_response()
}

/// Percent-encode one query-string value (RFC 3986 unreserved set kept).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The one JSON error body shape, shared with `vela serve`'s HTTP surface
/// and the MCP envelope's kind vocabulary:
/// `{"error": {"kind": "...", "message": "..."}}`.
fn error_body(kind: &str, message: impl Into<String>) -> Value {
    json!({"error": {"kind": kind, "message": message.into()}})
}

/// A configured Git source that failed its latest verification is unavailable,
/// not unknown. Ingest owns this status directly; there is no parallel publish
/// audit table to reconcile.
async fn ingest_failure_response(state: &AppState, vfr_id: &str) -> Option<Response> {
    let source = state.db.get_git_remote(vfr_id).await.ok().flatten()?;
    let message = source.get("ingest_error")?.as_str()?;
    Some(
        (
            StatusCode::FAILED_DEPENDENCY,
            Json(json!({
                "ok": false,
                "status": "unavailable",
                "vfr_id": vfr_id,
                "error": {"kind": "UNAVAILABLE", "message": message},
                "authority_mode": vela_hub::git_ingest::AUTHORITY_GIT_INGESTED,
            })),
        )
            .into_response(),
    )
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    /// The last-seen `vev_…` event id; events strictly after it are
    /// returned. Omit to start from the genesis event.
    cursor: Option<String>,
    limit: Option<usize>,
    kind: Option<String>,
    target: Option<String>,
}

/// Strict query parsing for the event endpoints: an unknown parameter is
/// a 400, not a silent no-op. A client still sending the retired
/// `?since=` gets told, instead of silently receiving page one.
fn parse_event_query(
    params: &HashMap<String, String>,
    allowed: &[&str],
) -> Result<EventQuery, Box<Response>> {
    if let Some(unknown) = params.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(error_body(
                "INVALID_ARG",
                format!(
                    "unknown query parameter `{unknown}` (allowed: {})",
                    allowed.join(", ")
                ),
            )),
        )
            .into_response()
            .into());
    }
    let limit = match params.get("limit") {
        Some(v) => match v.parse::<usize>() {
            Ok(n) => Some(n),
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(error_body(
                        "INVALID_ARG",
                        format!("limit `{v}` is not a number"),
                    )),
                )
                    .into_response()
                    .into());
            }
        },
        None => None,
    };
    Ok(EventQuery {
        cursor: params.get("cursor").cloned(),
        limit,
        kind: params.get("kind").cloned(),
        target: params.get("target").cloned(),
    })
}

fn root_json() -> Value {
    json!({
        "service": "vela-hub",
        "version": HUB_VERSION,
        "doctrine": "Git stores frontier bytes. The hub verifies configured repositories and serves a read-only event/projection index.",
        "endpoints": [
            "GET  /              - this banner",
            "GET  /healthz       - liveness + db-cache metrics",
            "GET  /readyz        - readiness (MCP projection built)",
            "GET  /entries       - configured, verified frontier indexes",
            "GET  /entries/{vfr_id} - single entry",
            "GET  /entries/{vfr_id}/events - cursor-paginated canonical event log",
            "GET  /entries/{vfr_id}/events/stream - server-sent event inbox",
            "GET  /entries/{vfr_id}/review - the review queue + the autonomy ledger (HTML or JSON; ?format=json)",
            "GET  /entries/{vfr_id}/git-remote - verified Git source and ingest status",
        ]
    })
}

/// The one page the hub still serves itself: a minimal self-describing
/// banner. System font stack, no external assets, no design system —
/// the app owns presentation; this exists so a human landing on the
/// bare node learns what it is and where the doors are. The endpoint
/// list is the JSON banner's own data, rendered once.
fn render_root_banner(urls: &PublicUrls) -> String {
    let endpoints: String = root_json()["endpoints"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|e| format!("<li><code>{e}</code></li>\n"))
                .collect()
        })
        .unwrap_or_default();
    let site = &urls.site;
    let repo = &urls.repo;
    let hub = &urls.hub;
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>vela-hub {HUB_VERSION}</title>
<style>
  body {{ font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
         max-width: 44rem; margin: 3rem auto; padding: 0 1.25rem;
         line-height: 1.55; color: #1a1a1a; background: #fdfdfc; }}
  h1 {{ font-size: 1.3rem; font-weight: 600; }}
  h1 small {{ font-weight: 400; color: #777; }}
  code {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
          font-size: 0.85em; }}
  ul {{ padding-left: 0; }}
  li {{ list-style: none; margin: 0.3rem 0; }}
  .doctrine {{ color: #555; font-style: italic; }}
  a {{ color: #1a1a1a; }}
</style>
</head>
<body>
<h1>vela-hub <small>{HUB_VERSION}</small></h1>
<p class="doctrine">a Vela protocol node — the log proves itself; viewers are replaceable</p>
<p>Every endpoint below speaks JSON. Browsers are redirected to the
reference viewer at <a href="{site}">{site}</a>.</p>
<ul>
{endpoints}</ul>
<p>Protocol docs: <a href="{repo}/blob/main/docs/HUB.md">docs/HUB.md</a>
· try <code>curl -s {hub}/entries</code></p>
</body>
</html>
"#
    )
}

async fn root(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if wants_html(&headers) {
        Html(render_root_banner(&state.urls)).into_response()
    } else {
        Json(root_json()).into_response()
    }
}

/// v0.49.2: Prometheus 0.0.4 text format metrics endpoint. Exposes
/// the DbCacheMetrics counters and stale-age histogram an operator
/// would otherwise have to scrape out of `/healthz` JSON, plus the
/// per-route request counters and latency histograms.
async fn metrics_prometheus(State(state): State<AppState>) -> impl axum::response::IntoResponse {
    let mut body = state.db_cache_metrics.render_prometheus();
    body.push_str(&state.http_metrics.render_prometheus());
    body.push_str(&state.rate_limiter.render_prometheus());
    // Ingest health, queried at scrape time. Born of a real incident: a
    // missing UPDATE grant left every re-ingest failing for days while
    // the error sat in a JSON field nobody scraped. A failing ingest is
    // a stale public record — it must be a first-class signal.
    if let Ok(rows) = state.db.ingest_health().await {
        body.push_str(
            "# HELP vela_hub_ingest_failing 1 when the frontier's last ingest sweep recorded an error.\n\
             # TYPE vela_hub_ingest_failing gauge\n",
        );
        for (vfr_id, failing, _age) in &rows {
            body.push_str(&format!(
                "vela_hub_ingest_failing{{vfr_id=\"{vfr_id}\"}} {}\n",
                if *failing { 1 } else { 0 }
            ));
        }
        body.push_str(
            "# HELP vela_hub_ingest_age_seconds Seconds since the frontier's last completed ingest.\n\
             # TYPE vela_hub_ingest_age_seconds gauge\n",
        );
        for (vfr_id, _failing, age) in &rows {
            if let Some(age) = age {
                body.push_str(&format!(
                    "vela_hub_ingest_age_seconds{{vfr_id=\"{vfr_id}\"}} {age}\n"
                ));
            }
        }
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Schema discoverability endpoint. Returns the canonical list of versioned
/// protocol schemas this hub knows about. Lets a
/// client bootstrap without scraping HTML or guessing URLs.
async fn well_known_vela(State(state): State<AppState>) -> Json<Value> {
    let manifest = json!({
        "name": "vela-hub",
        "version": HUB_VERSION,
        "protocol_version": HUB_VERSION,
        "site": state.urls.site.clone(),
        "endpoints": {
            "index": format!("{}/entries", state.urls.hub),
            "git_remote": format!("{}/entries/{{vfr_id}}/git-remote", state.urls.hub),
            "events": format!("{}/entries/{{vfr_id}}/events", state.urls.hub),
            "events_stream": format!("{}/entries/{{vfr_id}}/events/stream", state.urls.hub),
            "frontier_inbox": format!("{}/frontier/{{vfr_id}}/inbox", state.urls.hub),
            "snapshot": format!("{}/entries/{{vfr_id}}/snapshot", state.urls.hub),
            "metrics":  format!("{}/metrics", state.urls.hub),
            "healthz":  format!("{}/healthz", state.urls.hub),
        },
        "agent_sla": {
            "mode": "best_effort",
            "max_events_per_request": 500,
            "max_bytes_per_event": 1048576,
            "retry_after_seconds": 15,
            "writes": "frontier publication is git push; the hub index does not accept frontier state bytes"
        },
        "schemas": {
            "frontier-index":         "vela.frontier-index.v1",
            "frontier-index-list":    "vela.frontier-index-list.v1",
            "finding-bundle":         "https://vela.science/schema/finding-bundle/v0.10.0",
            "frontier-packet":        "https://vela.science/schema/frontier-packet/v1",
            "event":                  "https://vela.science/schema/event/v1",
            "agent-run":              "https://vela.science/schema/agent-run/v0.22",
            "key-revoke":             "https://vela.science/schema/event/key-revoke/v0.49",
            "cross-impl-reducer-fixture": "https://vela.science/schema/cross-impl-reducer-fixture/v1",
            "canonical-json":         "https://vela.science/schema/canonical-json/v1",
        },
        "canonical_json_v1": {
            "summary": "RFC-8785-shaped canonical JSON used as the preimage for every Vela signature.",
            "rules": [
                "object keys sorted lexicographically by UTF-8 byte order, recursively",
                "no insignificant whitespace between tokens",
                "strings are UTF-8 with JSON-standard escaping",
                "numbers in shortest round-trip form; NaN and Infinity rejected",
                "no trailing commas; arrays preserve source order"
            ],
            "reference_impl": "vela_protocol::canonical::to_canonical_bytes (Rust)"
        },
        "second_implementations": {
            "packet_verifier": "https://vela.science/vela_verify.py",
            "reducer":         "https://vela.science/vela_reducer.py",
            "reducer_typescript": "https://vela.science/vela_reducer.ts"
        },
    });

    Json(manifest)
}

/// Readiness, as distinct from liveness: a machine is READY only once
/// its hosted-MCP projection is built (or there is genuinely nothing to
/// build). Wired as a fly http check so a rolling deploy keeps the old
/// machine serving `/mcp` until the new one has finished its first
/// projection build — a deploy never blanks the public endpoint.
async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    if state.mcp.read().await.is_some() {
        return (
            StatusCode::OK,
            Json(json!({"ready": true, "mcp": "projection built", "version": HUB_VERSION})),
        );
    }
    match state.db.list_entries().await {
        Ok(entries) if entries.is_empty() => (
            StatusCode::OK,
            Json(json!({"ready": true, "mcp": "no frontiers registered", "version": HUB_VERSION})),
        ),
        Ok(entries) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ready": false,
                "mcp": format!("projection building ({} frontiers)", entries.len()),
                "version": HUB_VERSION,
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ready": false,
                "error": {"kind": "UNAVAILABLE", "message": e},
                "version": HUB_VERSION,
            })),
        ),
    }
}

async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let cache = state.db_cache_metrics.snapshot();
    match state.db.health().await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "db": "reachable",
                "version": HUB_VERSION,
                "cache": cache,
            })),
        ),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ok": false,
                "db": "unreachable",
                "error": {"kind": "UNAVAILABLE", "message": e},
                "version": HUB_VERSION,
                "cache": cache,
            })),
        ),
    }
}

/// The `/entries` response body. Default (no `limit`) is the full list —
/// byte-identical to the pre-pagination contract. When `limit` is
/// present the page is sliced and `{total, next_offset}` are added
/// ADDITIVELY (`next_offset` only when more rows remain).
fn entries_payload(values: &[Value], limit: Option<i64>, offset: i64) -> Value {
    let Some(limit) = limit else {
        return json!({"schema": FRONTIER_INDEX_LIST_SCHEMA, "entries": values});
    };
    let total = values.len() as i64;
    let start = offset.min(total) as usize;
    let end = offset.saturating_add(limit).min(total) as usize;
    let mut body = json!({
        "schema": FRONTIER_INDEX_LIST_SCHEMA,
        "entries": &values[start..end],
        "total": total,
    });
    if (end as i64) < total {
        body["next_offset"] = json!(end as i64);
    }
    body
}

/// Parse `/entries` pagination params: `limit` clamped to 1..=500 (and
/// only active when present), `offset` clamped >= 0.
fn entries_page_params(params: &HashMap<String, String>) -> (Option<i64>, i64) {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|l| l.clamp(1, 500));
    let offset = params
        .get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    (limit, offset)
}

async fn list_entries(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Protocol-only hub: the browsable frontier list lives in the app.
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, "/frontiers");
    }
    let (limit, offset) = entries_page_params(&params);
    let cache_key = "list_entries";
    let cached = db_cache_read(&state.db_cache, cache_key).await;
    let now = std::time::Instant::now();

    // Fresh cache window — serve straight from memory, skip DB.
    if let Some(entry) = cached.as_ref()
        && now.duration_since(entry.fetched_at) < DB_CACHE_FRESH_TTL
    {
        state.db_cache_metrics.record_hit();
        return cached_list_response(&entry.value, limit, offset, false);
    }

    match state.db.list_entries().await {
        Ok(values) => {
            state.db_cache_metrics.record_miss();
            // The cache holds the FULL list; pagination is a read-time
            // slice so every limit/offset shares one cache entry.
            let payload = json!({"schema": FRONTIER_INDEX_LIST_SCHEMA, "entries": values});
            db_cache_write(&state.db_cache, cache_key, payload.clone()).await;
            (
                StatusCode::OK,
                Json(entries_payload(&values, limit, offset)),
            )
                .into_response()
        }
        Err(e) => {
            state.db_cache_metrics.record_db_error();
            // v0.49: stale-on-read fallback. Serve the last good
            // payload (with X-Vela-Stale) instead of 5xx-ing on a
            // single DB hiccup. Inside the stale window only.
            if let Some(entry) = cached {
                let age = now.duration_since(entry.fetched_at);
                if age < DB_CACHE_STALE_WINDOW {
                    state.db_cache_metrics.record_stale_hit(age.as_secs());
                    tracing::warn!(
                        "list_entries: db error '{e}', serving stale ({}s old)",
                        age.as_secs()
                    );
                    return cached_list_response(&entry.value, limit, offset, true);
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response()
        }
    }
}

fn cached_list_response(payload: &Value, limit: Option<i64>, offset: i64, stale: bool) -> Response {
    let entries = payload
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut resp = (
        StatusCode::OK,
        Json(entries_payload(&entries, limit, offset)),
    )
        .into_response();
    if stale {
        resp.headers_mut().insert(
            axum::http::header::HeaderName::from_static("x-vela-stale"),
            axum::http::HeaderValue::from_static("1"),
        );
    }
    resp
}

/// Read a promoted frontier from event/projection tables and cache the
/// reconstructed `Project` by `(vfr_id, snapshot_hash)`.
///
/// This is intentionally strict after the event-first cutover: if a
/// frontier has not been promoted to `frontiers`, live routes surface an
/// unavailable state instead of fetching an alternate byte source.
async fn load_substrate(
    state: &AppState,
    vfr_id: &str,
    snapshot_hash: &str,
) -> Option<Arc<Project>> {
    let cache_key = (vfr_id.to_string(), snapshot_hash.to_string());
    if let Some(hit) = state.frontier_cache.read().await.get(&cache_key).cloned() {
        return Some(hit);
    }

    match state.db.get_materialized_project(vfr_id).await {
        Ok(Some(project)) => {
            let arc = Arc::new(project);
            state
                .frontier_cache
                .write()
                .await
                .insert(cache_key, arc.clone());
            return Some(arc);
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(%vfr_id, error = %e, "event-first materialized project read failed");
        }
    }
    None
}

/// The live Sidon open-frontier over HTTP: the next bound to beat at each n,
/// compiled from the frontier's accepted record so a producer reads what to
/// attempt without cloning. Keyless (a planning view, not accepted state) and
/// additive. Sidon-specific; a non-Sidon frontier returns 422.
async fn get_sidon_frontier_map(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
) -> Response {
    use std::collections::BTreeSet;
    use vela_protocol::sidon_profile::{
        build_frontier_map, live_presentation, next_bound_obligations,
    };
    let project = match state.db.get_materialized_project(&vfr_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body(
                    "NOT_FOUND",
                    format!("frontier not found: {vfr_id}"),
                )),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", e)),
            )
                .into_response();
        }
    };
    let pres = match live_presentation(&project) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(error_body(
                    "INVALID_ARG",
                    format!("not a live Sidon frontier ({vfr_id}): {e}"),
                )),
            )
                .into_response();
        }
    };
    let disabled = BTreeSet::new();
    let map =
        next_bound_obligations(&pres).and_then(|obls| build_frontier_map(&pres, &obls, &disabled));
    match map {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", e)),
        )
            .into_response(),
    }
}

/// The live authoritative Sidon bounds over HTTP: the best lower bound at each n,
/// compiled from the frontier's accepted record, with the presentation root so a
/// consumer can independently replay it. The read half of the loop, paired with
/// sidon-frontier-map. Keyless and replayable; the SIGNED ObservationPacket is the
/// producer's own read (`vela sidon export`). Sidon-specific; non-Sidon → 422.
async fn get_sidon_observation(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
) -> Response {
    use std::collections::BTreeSet;
    use vela_protocol::sidon_profile::{best_bounds, live_presentation};
    let project = match state.db.get_materialized_project(&vfr_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body(
                    "NOT_FOUND",
                    format!("frontier not found: {vfr_id}"),
                )),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", e)),
            )
                .into_response();
        }
    };
    let pres = match live_presentation(&project) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(error_body(
                    "INVALID_ARG",
                    format!("not a live Sidon frontier ({vfr_id}): {e}"),
                )),
            )
                .into_response();
        }
    };
    let disabled = BTreeSet::new();
    let bounds = match best_bounds(&pres, &disabled) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", e)),
            )
                .into_response();
        }
    };
    let root = pres.presentation_root().unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.sidon-bounds.v1",
            "vfr_id": vfr_id,
            "presentation_root": root,
            "bounds": bounds,
            "replay": "vela sidon export --frontier <dir> reproduces this as a signed observation",
        })),
    )
        .into_response()
}

async fn get_entry(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    // Protocol-only hub: browsers land on the app's record page, which
    // renders live state (and its own not-found / unavailable views).
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}"));
    }
    let row = state.db.get_index_entry(&vfr_id).await;
    match row {
        Ok(Some(value)) => (StatusCode::OK, Json(value)).into_response(),
        Ok(None) => {
            if let Some(response) = ingest_failure_response(&state, &vfr_id).await {
                return response;
            }
            (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("query: {e}"))),
        )
            .into_response(),
    }
}

/// GET /entries/{vfr_id}/proposals/{proposal_id}/evidence-diff —
/// the read-only Evidence Diff for a pending proposal: its before/after
/// effect on the target claim plus the downstream claims whose status
/// flips. A pure projection over the materialized state (never writes,
/// never accepts); the strict accept gate still runs at accept time and
/// is the only thing that mutates state. The Engine verdict is rendered
/// absent here because `evidence_ci::run_project` needs a frontier path
/// (policy docs, artifact files) the Postgres-materialized project lacks.
async fn get_proposal_evidence_diff(
    State(state): State<AppState>,
    Path((vfr_id, proposal_id)): Path<(String, String)>,
) -> Response {
    let project = match state.db.get_materialized_project(&vfr_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body(
                    "NOT_FOUND",
                    format!("{vfr_id} not found on this hub"),
                )),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("project query: {e}"))),
            )
                .into_response();
        }
    };
    match vela_protocol::evidence_diff::claim_state_delta(
        &project,
        &proposal_id,
        "reviewer:evidence-diff-preview",
    ) {
        Ok(delta) => Json(delta).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(error_body("NOT_FOUND", e))).into_response(),
    }
}

/// Lightweight per-frontier counts for list/dashboard views. Computed by cheap
/// projection-table aggregates (never the multi-MB snapshot), so the catalogue
/// can render real numbers without downloading whole frontiers. JSON only.
async fn get_entry_summary(State(state): State<AppState>, Path(vfr_id): Path<String>) -> Response {
    match state.db.frontier_summary(&vfr_id).await {
        Ok(Some(value)) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
            Json(value),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("summary: {e}"))),
        )
            .into_response(),
    }
}

/// The configured Git source + ingest cursor for a frontier.
async fn get_git_remote(State(state): State<AppState>, Path(vfr_id): Path<String>) -> Response {
    match state.db.get_git_remote(&vfr_id).await {
        Ok(Some(rec)) => Json(json!({"vfr_id": vfr_id, "git": rec})).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(error_body(
                "NOT_FOUND",
                format!("{vfr_id} has no configured Git source"),
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("query: {e}"))),
        )
            .into_response(),
    }
}

/// The producer view: cross-frontier objects signed by one key — the
/// fundable CV, queryable in one call.
async fn get_producer(
    State(state): State<AppState>,
    Path(pubkey): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/producer/{pubkey}"));
    }
    // Bounded read with an HONEST cap: default 500, caller-tunable to 2000, and
    // the response says when it truncated so a large producer is never silently
    // clipped (the "no silent caps" rule).
    const PRODUCER_DEFAULT_CAP: i64 = 500;
    const PRODUCER_MAX_CAP: i64 = 2000;
    let cap = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(PRODUCER_DEFAULT_CAP)
        .min(PRODUCER_MAX_CAP);
    match state.db.producer_objects(&pubkey, cap).await {
        Ok(rows) => {
            let truncated = rows.len() as i64 >= cap;
            let mut by_frontier: std::collections::BTreeMap<String, Vec<Value>> =
                std::collections::BTreeMap::new();
            for (vfr, otype, oid, raw) in rows {
                by_frontier.entry(vfr).or_default().push(json!({
                    "type": otype,
                    "id": oid,
                    "summary": raw.get("claim").or_else(|| raw.get("assertion").and_then(|a| a.get("text"))).cloned().unwrap_or(Value::Null),
                }));
            }
            Json(json!({
                "pubkey": pubkey,
                "frontiers": by_frontier,
                "limit": cap,
                "truncated": truncated,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("query: {e}"))),
        )
            .into_response(),
    }
}

async fn get_entry_manifest(State(state): State<AppState>, Path(vfr_id): Path<String>) -> Response {
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(e)) => e,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let counts = match state.db.frontier_summary(&vfr_id).await {
        Ok(Some(s)) => s,
        _ => json!({}),
    };
    let objects = match state.db.frontier_object_index(&vfr_id).await {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("index: {e}"))),
            )
                .into_response();
        }
    };
    let manifest = json!({
        "vfr_id": vfr_id,
        "name": entry.get("name").cloned().unwrap_or(Value::Null),
        "log_head": entry.get("latest_event_log_hash").cloned().unwrap_or(Value::Null),
        "snapshot_hash": entry.get("latest_snapshot_hash").cloned().unwrap_or(Value::Null),
        "counts": counts,
        "objects": objects,
    });
    (
        StatusCode::OK,
        [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
        Json(manifest),
    )
        .into_response()
}

/// Cross-frontier object text search. One hub query over
/// frontier_objects instead of downloading every frontier's snapshot.
/// Params: `q` (text), `type` (finding|source|evidence_atom|…, default
/// finding), `limit` (default 24, max 200), `offset` (default 0).
/// Postgres backends rank with full-text search
/// (`websearch_to_tsquery` + `ts_rank`); SQLite keeps substring LIKE.
/// `{total, next_offset}` are additive on the pre-FTS response shape.
async fn search_endpoint(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let q = params
        .get("q")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if wants_html(&headers) {
        let path = if q.is_empty() {
            "/search".to_string()
        } else {
            format!("/search?q={}", urlencode(&q))
        };
        return redirect_to_site(&state.urls, &path);
    }
    let object_type = params
        .get("type")
        .cloned()
        .unwrap_or_else(|| "finding".to_string());
    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(24)
        .clamp(1, 200);
    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .max(0);
    if q.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({"results": [], "q": q, "type": object_type})),
        )
            .into_response();
    }
    match state
        .db
        .search_objects(&q, &object_type, limit, offset)
        .await
    {
        Ok((results, total)) => {
            let consumed = offset + results.len() as i64;
            let mut body = json!({
                "results": results,
                "q": q,
                "type": object_type,
                "total": total,
            });
            if consumed < total {
                body["next_offset"] = json!(consumed);
            }
            (
                StatusCode::OK,
                [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
                Json(body),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("search: {e}"))),
        )
            .into_response(),
    }
}

/// One page of a frontier's objects of a given type — lets detail surfaces
/// (sources, proposals, …) render without pulling the whole snapshot. Params:
/// limit (default 100, max 500), offset (default 0). Returns {objects, total}.
async fn get_entry_objects(
    State(state): State<AppState>,
    Path((vfr_id, otype)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .max(0);
    match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    }
    match state
        .db
        .frontier_objects_page(&vfr_id, &otype, limit, offset)
        .await
    {
        Ok((objects, total)) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
            Json(json!({
                "vfr_id": vfr_id, "type": otype,
                "limit": limit, "offset": offset, "total": total,
                "objects": objects,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("objects: {e}"))),
        )
            .into_response(),
    }
}

/// A single frontier object by (type, object_id) — a primary-key point lookup.
async fn get_entry_object(
    State(state): State<AppState>,
    Path((vfr_id, otype, object_id)): Path<(String, String, String)>,
) -> Response {
    match state.db.frontier_object(&vfr_id, &otype, &object_id).await {
        Ok(Some(obj)) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
            Json(obj),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(error_body(
                "NOT_FOUND",
                format!("{object_id} not found in {vfr_id}"),
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("object: {e}"))),
        )
            .into_response(),
    }
}

/// Hub lookup for a replayed Scientific Diff Pack record (`vsd_*`). The record
/// comes from the verified frontier projection; the Hub has no independent
/// pack registration or blob transport.
async fn get_diff_pack(State(state): State<AppState>, Path(pack_id): Path<String>) -> Response {
    if !pack_id.starts_with("vsd_") {
        return (
            StatusCode::BAD_REQUEST,
            Json(error_body("INVALID_ARG", "pack_id must start with `vsd_`")),
        )
            .into_response();
    }
    match state.db.get_diff_pack(&pack_id).await {
        Ok(Some(value)) => (StatusCode::OK, Json(value)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(error_body(
                "NOT_FOUND",
                format!("{pack_id} not found in any verified Git frontier indexed by this hub"),
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("query: {e}"))),
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
/// Implementation is O(N) over current live entries: dependency lists
/// are materialized from promoted frontier state and cached by
/// `(vfr_id, snapshot_hash)`. Failed or unpromoted source rows do
/// not participate.
async fn get_depends_on(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let _ = &headers; // reserved for future HTML rendering
    let rows = match state.db.list_entries().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
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
        let snapshot_hash = entry
            .get("latest_snapshot_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let Some(project) = load_substrate(&state, entry_vfr, snapshot_hash).await else {
            // Projection unavailable means the frontier is not live for
            // composition. Skip it; direct entry routes surface the
            // unavailable state.
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
/// `GET /entries/{vfr_id}/graph` — the frontier's typed finding-link graph
/// (nodes + edges), the hosted counterpart of the MCP `graph` tool's traverse
/// mode. Reads the reconstructed `Project` and derives a [`FrontierGraph`], so
/// the same 12-type link vocabulary the CLI walks is answerable over HTTP: a
/// client can pull the whole Erdős dependency graph in one call. The graph is a
/// pure derived view of the declared links (candidates, not adjudicated truth) —
/// the `claim_boundary` says so on the wire.
async fn get_entry_graph(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/graph"));
    }
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(frontier) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    let graph = vela_protocol::frontier_graph::FrontierGraph::from_project(&frontier);
    let nodes: Vec<_> = graph.nodes().collect();
    let edges = graph.all_edges();
    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.frontier-graph.v0.1",
            "vfr_id": vfr_id,
            "claim_boundary": {
                "graph_is_derived": true,
                "edges_are_declared_links": true,
                "relations_are_candidates_not_adjudicated": true,
            },
            "counts": {
                "nodes": nodes.len(),
                "edges": edges.len(),
                "by_edge_kind": graph.edge_kind_counts(),
            },
            "nodes": nodes,
            "edges": edges,
        })),
    )
        .into_response()
}

/// `GET /entries/{vfr_id}/frontier` — the frontier-identification ranking:
/// OPEN findings ordered by accumulating structural support (which is closest to
/// a verifier-run from done), each with the popularity baseline and inspectable
/// evidence. A derived projection (advice, never adjudication), served with the
/// same claim-boundary discipline as `/graph`.
async fn get_entry_frontier(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/frontier"));
    }
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .min(500);
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(frontier) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };
    let ranked = vela_protocol::frontier_identification::frontier_identification(&frontier);
    let open_total = ranked.len();
    let candidates: Vec<_> = ranked.into_iter().take(limit).collect();
    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.frontier_rank.v0.1",
            "vfr_id": vfr_id,
            "claim_boundary": {
                "ranking_is_derived": true,
                "advice_not_authority": true,
                "target_is_solvability_not_truth": true,
            },
            "open_total": open_total,
            "candidates": candidates,
        })),
    )
        .into_response()
}

/// `GET /entries/{vfr_id}/boundary` — the frontier's dark-matter boundary: the
/// productive edges (one_premise_away, fragile, brittle, contested, stale_open),
/// a pure projection over the typed graph + review state. A health/triage view;
/// classifies, never adjudicates.
async fn get_entry_boundary(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/boundary"));
    }
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(frontier) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };
    let boundary = vela_protocol::boundary::Boundary::derive(&frontier);
    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.boundary.v0.1",
            "vfr_id": vfr_id,
            "claim_boundary": {
                "derived_projection": true,
                "classifies_not_adjudicates": true,
            },
            "counts": {
                "one_premise_away": boundary.one_premise_away.len(),
                "fragile": boundary.fragile.len(),
                "brittle": boundary.brittle.len(),
                "contested": boundary.contested.len(),
                "stale_open": boundary.stale_open.len(),
                "total": boundary.total(),
            },
            "boundary": boundary,
        })),
    )
        .into_response()
}

async fn get_finding(
    State(state): State<AppState>,
    Path((vfr_id, vf_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    // Protocol-only hub: the finding record page lives in the app.
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/findings/{vf_id}"));
    }
    // Find the entry to get the locator.
    let entry = state.db.get_index_entry(&vfr_id).await;
    let entry = match entry {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };

    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let frontier = load_substrate(&state, &vfr_id, snapshot_hash).await;

    let Some(project) = frontier else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    let Some(bundle) = project.findings.iter().find(|b| b.id == vf_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(error_body("NOT_FOUND", format!("{vf_id} not in {vfr_id}"))),
        )
            .into_response();
    };

    match serde_json::to_value(bundle) {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("serialize: {e}"))),
        )
            .into_response(),
    }
}

/// Pack review page: one released Scientific Diff Pack (`vsd_*`) on one
/// frontier, read end-to-end — release metadata, the human verdict when
/// present, and the member proposals with their Evidence Diff links.
/// HTML for browsers; `Accept: application/json` returns the replayed
/// `ReleasedDiffPackRecord` as-is (same dual-mode contract as the
/// finding page). The record is pure replay state from the canonical
/// event log — this page renders it, it never adjudicates it.
async fn get_pack_review(
    State(state): State<AppState>,
    Path((vfr_id, pack_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    // Protocol-only hub: the pack review page lives in the app.
    if wants_html(&headers) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/packs/{pack_id}"));
    }
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };

    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(project) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    let Some(rec) = project
        .released_diff_packs
        .iter()
        .find(|r| r.pack_id == pack_id)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(error_body(
                "NOT_FOUND",
                format!("{pack_id} not released on {vfr_id}"),
            )),
        )
            .into_response();
    };

    match serde_json::to_value(rec) {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("serialize: {e}"))),
        )
            .into_response(),
    }
}

/// The "verify this yourself" page was HTML-only presentation; the app
/// owns it now. The whole route is the redirect — the reproduce data
/// (registered remote, ingest cursor) stays queryable at
/// `GET /entries/{vfr_id}/git-remote`.
async fn get_reproduce(State(state): State<AppState>, Path(vfr_id): Path<String>) -> Response {
    redirect_to_site(&state.urls, &format!("/r/{vfr_id}/reproduce"))
}

/// `GET /entries/{vfr_id}/review` — the review queue + the autonomy
/// ledger for one frontier. Read-only JSON; browsers (or
/// `?format=html`) are redirected to the app's review page.
///
/// Three ledgers, one page: what awaits a human key (the sign-queue
/// Judgment + Decision lanes), what landed under a human-signed policy
/// (every event carrying a `policy_lane` block — machine-admitted,
/// human-authorized, replay-verified by strict check), and what a human
/// key decided directly. The hub renders custody; it never exercises it.
async fn get_entry_review(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Protocol-only hub: the review page lives in the app. Browsers
    // (or an explicit `?format=html`) are redirected; `?format=json`
    // still forces JSON for a client that cannot drop its Accept header.
    let format = params.get("format").map(String::as_str);
    if format == Some("html") || (format != Some("json") && wants_html(&headers)) {
        return redirect_to_site(&state.urls, &format!("/r/{vfr_id}/review"));
    }
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(project) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    let queue = build_review_queue(&state, &vfr_id, &project).await;

    let admissions = policy_admissions(&project);
    let decisions = human_decisions(&project);
    let autonomy = vela_edge::frontier_health::compounding_metrics(&project).autonomy_ratio;
    let mut by_policy: std::collections::BTreeMap<&str, usize> = Default::default();
    for a in &admissions {
        *by_policy.entry(a.policy_id.as_str()).or_default() += 1;
    }
    (
        StatusCode::OK,
        Json(json!({
            "schema": "vela.hub.review.v0.1",
            "vfr_id": vfr_id,
            "stats": {
                "awaiting": queue.rows.len(),
                "policy_admitted": admissions.len(),
                "human_decided": decisions.len(),
                "autonomy_ratio": autonomy,
            },
            "policy": {
                "active": queue.policy_active,
                "policy_id": queue.policy_id,
                "filtered": queue.policy_filtered,
            },
            "awaiting": queue.rows.iter().map(|r| json!({
                "lane": r.lane,
                "id": r.id,
                "title": r.title,
                "why_here": r.why_here,
                "actions": {
                    "accept": r.accept_eligibility,
                    "reject": r.reject_eligibility,
                },
                "pack_memberships": r.pack_memberships,
            })).collect::<Vec<_>>(),
            "policy_admitted": {
                "by_policy": by_policy,
                "events": admissions.iter().map(|a| json!({
                    "event_id": a.event_id,
                    "policy_id": a.policy_id,
                    "rule_ids": a.rule_ids,
                    "proposal_id": a.proposal_id,
                    "timestamp": a.timestamp,
                    "target": {"type": a.target_type, "id": a.target_id},
                })).collect::<Vec<_>>(),
            },
            "human_decisions": decisions.iter().rev().take(HUMAN_DECISIONS_SHOWN).map(|d| json!({
                "event_id": d.event_id,
                "kind": d.kind,
                "reviewer": d.reviewer,
                "timestamp": d.timestamp,
                "target": {"type": d.target_type, "id": d.target_id},
            })).collect::<Vec<_>>(),
        })),
    )
        .into_response()
}

/// Build the hub's replay-only awaiting view. The filesystem-aware Decision
/// Brief transaction lives in the CLI; reproducing only part of it here would
/// create a second trust interpretation. The hub therefore exposes every
/// pending proposal and labels action eligibility as not evaluated.
async fn build_review_queue(
    _state: &AppState,
    _vfr_id: &str,
    project: &Project,
) -> ReviewQueueView {
    pending_review_fallback(project)
}

/// `GET /entries/{vfr_id}/findings/{vf_id}/context`
///
/// Returns a *project-shaped slice* scoped to one finding: the target finding
/// plus the source findings that link into it (so the web's incoming-link scan
/// resolves), with evidence atoms / events / proposals / verifier attachments /
/// statement attestations filtered to the target, and the small shared metadata
/// (sources, actors, frontier meta, proof_state) carried whole. The finding page
/// consumes this in hub mode instead of pulling the whole multi-MB snapshot per
/// request (the erdos snapshot is ~15 MB; a finding page needs a few KB of it).
/// The shape is a strict subset of the snapshot `Project`, so the same web-side
/// normalizer applies unchanged. Filtering is done on the serialized JSON using
/// the exact field names the web consumes, so this never couples to the Rust
/// struct layout.
async fn get_finding_context(
    State(state): State<AppState>,
    Path((vfr_id, vf_id)): Path<(String, String)>,
) -> Response {
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(project) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    let full = match serde_json::to_value(&*project) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("serialize: {e}"))),
            )
                .into_response();
        }
    };
    let obj = full.as_object().cloned().unwrap_or_default();
    let arr = |k: &str| -> Vec<serde_json::Value> {
        obj.get(k)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };

    let findings = arr("findings");
    let Some(target) = findings
        .iter()
        .find(|f| f.get("id").and_then(|v| v.as_str()) == Some(vf_id.as_str()))
        .cloned()
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(error_body("NOT_FOUND", format!("{vf_id} not in {vfr_id}"))),
        )
            .into_response();
    };

    // Target first, then the source findings whose links point at it, so the
    // web's `bundle.findings.flatMap(... link.target === id)` incoming-link scan
    // resolves against the slice without shipping every finding.
    let mut sliced_findings = vec![target];
    for f in &findings {
        if f.get("id").and_then(|v| v.as_str()) == Some(vf_id.as_str()) {
            continue;
        }
        let links_in = f
            .get("links")
            .and_then(|v| v.as_array())
            .map(|ls| {
                ls.iter()
                    .any(|l| l.get("target").and_then(|v| v.as_str()) == Some(vf_id.as_str()))
            })
            .unwrap_or(false);
        if links_in {
            sliced_findings.push(f.clone());
        }
    }

    let by_finding_id = |k: &str| -> Vec<serde_json::Value> {
        arr(k)
            .into_iter()
            .filter(|a| a.get("finding_id").and_then(|v| v.as_str()) == Some(vf_id.as_str()))
            .collect()
    };
    let by_target_id = |k: &str| -> Vec<serde_json::Value> {
        arr(k)
            .into_iter()
            .filter(|a| {
                a.get("target")
                    .and_then(|t| t.get("id"))
                    .and_then(|v| v.as_str())
                    == Some(vf_id.as_str())
            })
            .collect()
    };
    let by_target_str = |k: &str| -> Vec<serde_json::Value> {
        arr(k)
            .into_iter()
            .filter(|a| a.get("target").and_then(|v| v.as_str()) == Some(vf_id.as_str()))
            .collect()
    };

    let mut slice = serde_json::Map::new();
    // Envelope fields the web normalizer reads (frontier meta + proof state).
    for k in [
        "vela_version",
        "schema",
        "frontier_id",
        "frontier",
        "stats",
        "proof_state",
    ] {
        if let Some(v) = obj.get(k) {
            slice.insert(k.to_string(), v.clone());
        }
    }
    slice.insert("findings".into(), serde_json::Value::Array(sliced_findings));
    slice.insert(
        "evidence_atoms".into(),
        serde_json::Value::Array(by_finding_id("evidence_atoms")),
    );
    slice.insert(
        "events".into(),
        serde_json::Value::Array(by_target_id("events")),
    );
    slice.insert(
        "proposals".into(),
        serde_json::Value::Array(by_target_id("proposals")),
    );
    slice.insert(
        "verifier_attachments".into(),
        serde_json::Value::Array(by_target_str("verifier_attachments")),
    );
    slice.insert(
        "statement_attestations".into(),
        serde_json::Value::Array(by_target_str("statement_attestations")),
    );
    // Small shared metadata, carried whole (bibliography + actor key map).
    slice.insert("sources".into(), serde_json::Value::Array(arr("sources")));
    slice.insert("actors".into(), serde_json::Value::Array(arr("actors")));

    (StatusCode::OK, Json(serde_json::Value::Object(slice))).into_response()
}

/// `GET /entries/{vfr_id}/findings/{vf_id}/gate-status`
///
/// Returns the **derived** trust-gate status for one finding — never stored,
/// always recomputed from the finding's current claim and its verifier
/// attachments (doctrine: status is a read-time projection). The UI uses this
/// to render verification as a material state without re-deriving the gate.
///
/// The response separates two things the campaign deliberately keeps apart:
///   - `machine_sealed` — the gate says `verified` (G1–G4: ≥2 independent,
///     matched, adversarially-probed attachments). This is the gold seam.
///   - `reviewer_accepted` — a human review verdict of `accepted`. A finding
///     can be reviewer-accepted yet NOT machine-sealed. `reviewer-accepted ≠
///     machine-sealed`; the UI must not conflate them.
/// `distinct_verifier_actors` / `distinct_methods` expose the independence
/// truth directly (independence is by distinct method/solver, not by count of
/// attachments), so the UI can be honest about thin evidence.
async fn get_finding_gate_status(
    State(state): State<AppState>,
    Path((vfr_id, vf_id)): Path<(String, String)>,
) -> Response {
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };

    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(project) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    match finding_gate_status_body(
        &project.findings,
        &project.verifier_attachments,
        &vfr_id,
        &vf_id,
    ) {
        Some(body) => (StatusCode::OK, Json(body)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(error_body("NOT_FOUND", format!("{vf_id} not in {vfr_id}"))),
        )
            .into_response(),
    }
}

/// `GET /entries/{vfr_id}/gate-status`
///
/// The frontier-wide projection: one gate-status row per finding, so a list
/// view renders the whole frontier's seal state in a single request instead
/// of N. Same derivation as the per-finding endpoint (status never stored).
async fn get_frontier_gate_status(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
) -> Response {
    let entry = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };
    let snapshot_hash = entry
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let Some(project) = load_substrate(&state, &vfr_id, snapshot_hash).await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "UNAVAILABLE",
                "frontier projection unavailable; pull via the CLI to inspect",
            )),
        )
            .into_response();
    };

    // Group attachments by target ONCE (O(attachments)), so each finding's
    // derivation is an O(1) lookup. The earlier per-finding re-scan was
    // O(findings × attachments) + O(findings²) on the bundle lookup — quadratic
    // on large frontiers (e.g. 5.5k findings).
    use std::collections::HashMap;
    type Att = vela_protocol::verifier_attachment::VerifierAttachment;
    let mut by_target: HashMap<&str, Vec<Att>> = HashMap::new();
    for a in &project.verifier_attachments {
        by_target
            .entry(a.target.as_str())
            .or_default()
            .push(a.clone());
    }
    let empty: Vec<Att> = Vec::new();
    let rows: Vec<Value> = project
        .findings
        .iter()
        .map(|b| gate_status_value(b, by_target.get(b.id.as_str()).unwrap_or(&empty), &vfr_id))
        .collect();
    let sealed = rows
        .iter()
        .filter(|r| r["machine_sealed"] == json!(true))
        .count();
    let body = json!({
        "schema": "vela.gate-status-page.v0.1",
        "vfr_id": vfr_id,
        "count": rows.len(),
        "machine_sealed_count": sealed,
        "findings": rows,
    });
    (StatusCode::OK, Json(body)).into_response()
}

/// Pure projection: the gate-status response body for one finding, or `None`
/// if the finding is absent. Takes just the slices it reads so the
/// seal-vs-review distinction is unit-testable without a server, DB, or a
/// fully-constructed `Project`.
fn finding_gate_status_body(
    findings: &[vela_protocol::bundle::FindingBundle],
    attachments_all: &[vela_protocol::verifier_attachment::VerifierAttachment],
    vfr_id: &str,
    vf_id: &str,
) -> Option<Value> {
    let bundle = findings.iter().find(|b| b.id == vf_id)?;
    // Single finding: filtering the attachments once is O(attachments). The
    // frontier-wide path must NOT call this in a loop (that is O(findings ×
    // attachments)); it groups attachments by target once and uses
    // `gate_status_value` directly.
    let attachments: Vec<_> = attachments_all
        .iter()
        .filter(|a| a.target == vf_id)
        .cloned()
        .collect();
    Some(gate_status_value(bundle, &attachments, vfr_id))
}

/// Core projection: the gate-status body for one finding given its bundle and
/// the attachments ALREADY filtered to it. No lookups or scans here, so the
/// caller controls the cost — the frontier-wide endpoint resolves attachments
/// once via a by-target map and calls this O(1) per finding.
fn gate_status_value(
    bundle: &vela_protocol::bundle::FindingBundle,
    attachments: &[vela_protocol::verifier_attachment::VerifierAttachment],
    vfr_id: &str,
) -> Value {
    use std::collections::BTreeSet;
    use vela_protocol::bundle::ReviewState;
    use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};

    let digest = claim_digest(&bundle.assertion.text);
    let outcome = derive_gate_status(&digest, attachments);

    let distinct_actors: BTreeSet<&str> = attachments
        .iter()
        .map(|a| a.verifier_actor.as_str())
        .collect();
    let distinct_methods: BTreeSet<&str> = attachments
        .iter()
        .map(|a| a.verifier_method.as_str())
        .collect();
    let reviewer_accepted = matches!(bundle.flags.review_state, Some(ReviewState::Accepted));
    let machine_sealed = outcome.status == GateStatus::Verified;

    json!({
        "schema": "vela.gate-status.v0.1",
        "vfr_id": vfr_id,
        "vf_id": bundle.id,
        "claim_digest": digest,
        // Machine seal (the gold seam): derived, fail-closed.
        "gate_status": outcome.status,
        "machine_sealed": machine_sealed,
        "reasons": outcome.reasons,
        // Human review verdict — distinct from the machine seal.
        "reviewer_accepted": reviewer_accepted,
        "review_state": bundle.flags.review_state,
        // Independence truth, exposed so the UI cannot overstate thin evidence.
        "attachment_count": attachments.len(),
        "distinct_verifier_actors": distinct_actors.len(),
        "distinct_methods": distinct_methods.len(),
        // Stone seam: superseded by a newer content-addressed finding.
        "superseded": bundle.flags.superseded,
    })
}

/// Return the materialized frontier state for `vfr_id`.
///
/// The event/projection tables are the only live read path. Snapshot bytes are
/// reconstructed from the verified Git-derived projection; the hub carries no
/// alternate object-store transport.
async fn get_entry_snapshot(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let row = match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            if let Some(response) = ingest_failure_response(&state, &vfr_id).await {
                return response;
            }
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    };

    let snap_hash = row
        .get("latest_snapshot_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if snap_hash.is_empty() {
        return (
            StatusCode::FAILED_DEPENDENCY,
            Json(error_body(
                "UNAVAILABLE",
                format!("frontier index row for {vfr_id} is missing latest_snapshot_hash"),
            )),
        )
            .into_response();
    }

    // Conditional GET: the snapshot is content-addressed, so its hash IS
    // the ETag. A polling swarm that sends `If-None-Match: "<hash>"` gets a
    // 304 here — before we materialize and serialize a multi-MB project —
    // whenever the frontier has not moved. The natural HTTP expression of
    // content addressing.
    if let Some(inm) = headers
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        && inm
            .trim_matches('"')
            .trim_start_matches("W/")
            .trim_matches('"')
            == snap_hash
    {
        let mut resp = StatusCode::NOT_MODIFIED.into_response();
        if let Ok(etag) = axum::http::HeaderValue::from_str(&format!("\"{snap_hash}\"")) {
            resp.headers_mut().insert(axum::http::header::ETAG, etag);
        }
        resp.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("public, max-age=60, stale-while-revalidate=300"),
        );
        return resp;
    }

    match state.db.get_materialized_project(&vfr_id).await {
        Ok(Some(project)) => {
            let value = serde_json::to_value(&project).unwrap_or(Value::Null);
            let mut resp = (StatusCode::OK, Json(value)).into_response();
            resp.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static(
                    "public, max-age=60, stale-while-revalidate=300",
                ),
            );
            if let Ok(etag) = axum::http::HeaderValue::from_str(&format!("\"{snap_hash}\"")) {
                resp.headers_mut().insert(axum::http::header::ETAG, etag);
            }
            return resp;
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body(
                    "INTERNAL",
                    format!("event-first snapshot read: {e}"),
                )),
            )
                .into_response();
        }
    }

    (
        StatusCode::FAILED_DEPENDENCY,
        Json(json!({
            "ok": false,
            "status": "unavailable",
            "vfr_id": vfr_id,
            "snapshot_hash": snap_hash,
            "error": {"kind": "UNAVAILABLE", "message": "frontier projection unavailable"},
        })),
    )
        .into_response()
}

async fn get_entry_events(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    Query(raw): Query<HashMap<String, String>>,
) -> Response {
    let params = match parse_event_query(&raw, &["cursor", "limit", "kind", "target"]) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Some(response) = ingest_failure_response(&state, &vfr_id).await {
                return response;
            }
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    }

    let limit = params.limit.unwrap_or(100);
    match state
        .db
        .event_page(
            &vfr_id,
            params.cursor.as_deref(),
            limit,
            params.kind.as_deref(),
            params.target.as_deref(),
        )
        .await
    {
        Ok(page) => (
            StatusCode::OK,
            Json(json!({
                "schema": "vela.events-page.v0.1",
                "vfr_id": vfr_id,
                "events": page.events,
                "count": page.events.len(),
                "next_cursor": page.next_cursor,
                "log_total": page.log_total,
            })),
        )
            .into_response(),
        Err(e) if e.starts_with("cursor_not_found:") => (
            StatusCode::BAD_REQUEST,
            Json(error_body(
                "INVALID_ARG",
                e.trim_start_matches("cursor_not_found: "),
            )),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("INTERNAL", format!("events query: {e}"))),
        )
            .into_response(),
    }
}

// ─── Read-only live event stream ──────────────────────────────────────────
//
// The hub deliberately has no proposal-create or proposal-accept route.
// Truth-bearing decisions are signed and landed through the git-native
// frontier workflow; this endpoint only streams the resulting verified log.

async fn get_entry_events_stream(
    State(state): State<AppState>,
    Path(vfr_id): Path<String>,
    Query(raw): Query<HashMap<String, String>>,
) -> Response {
    let params = match parse_event_query(&raw, &["cursor", "kind", "target"]) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    match state.db.get_index_entry(&vfr_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Some(response) = ingest_failure_response(&state, &vfr_id).await {
                return response;
            }
            return (
                StatusCode::NOT_FOUND,
                Json(error_body("NOT_FOUND", format!("{vfr_id} not found"))),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("INTERNAL", format!("query: {e}"))),
            )
                .into_response();
        }
    }

    let stream_state = state.clone();
    let stream_vfr = vfr_id.clone();
    let kind = params.kind.clone();
    let target = params.target.clone();
    let mut cursor = params.cursor.clone();
    let stream = async_stream::stream! {
        loop {
            match stream_state
                .db
                .event_page(
                    &stream_vfr,
                    cursor.as_deref(),
                    100,
                    kind.as_deref(),
                    target.as_deref(),
                )
                .await
            {
                Ok(page) if !page.events.is_empty() => {
                    for raw in page.events {
                        let id = raw
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("event")
                            .to_string();
                        cursor = Some(id.clone());
                        yield Ok::<Event, std::convert::Infallible>(
                            Event::default()
                                .event("event")
                                .id(id)
                                .data(raw.to_string())
                        );
                    }
                }
                Ok(_) => {
                    let heartbeat = json!({
                        "vfr_id": stream_vfr,
                        "cursor": cursor,
                        "status": "idle",
                    });
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default()
                            .event("heartbeat")
                            .data(heartbeat.to_string())
                    );
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
                Err(e) => {
                    let payload = json!({
                        "vfr_id": stream_vfr,
                        "error": {"kind": "INTERNAL", "message": e},
                    });
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default()
                            .event("error")
                            .data(payload.to_string())
                    );
                    break;
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ── Presentation ──────────────────────────────────────────────────────
//
// There is none. The hub is a protocol node: every surface is JSON (or
// a content-addressed artifact), and `Accept: text/html` is a 301 into
// the app (`redirect_to_site`). The root banner above is the single
// hand-written HTML page — system fonts, no assets, no design system.
// The app is the sole owner of the design system.

#[cfg(test)]
mod gate_status_tests {
    use super::finding_gate_status_body;
    use vela_protocol::bundle::FindingBundle;

    // A complete-but-minimal finding, reviewer-accepted (flags.review_state),
    // carrying ZERO verifier attachments. This is the exact shape the Lane C
    // design hinges on: a human said "accepted" but no machine seal exists.
    const REVIEWER_ACCEPTED_FINDING: &str = r#"{
        "id": "vf_test0000000001",
        "version": 1,
        "assertion": {
            "text": "a Sidon set of size 33 in {0,1}^8",
            "type": "mechanism",
            "entities": [],
            "relation": null,
            "direction": null
        },
        "evidence": {
            "type": "computational",
            "model_system": "search",
            "species": null,
            "method": "exhaustive enumeration",
            "sample_size": null,
            "effect_size": null,
            "p_value": null,
            "replicated": false,
            "replication_count": null,
            "evidence_spans": []
        },
        "conditions": {
            "text": "n/a",
            "species_verified": [],
            "species_unverified": [],
            "in_vitro": false,
            "in_vivo": false,
            "human_data": false,
            "clinical_trial": false
        },
        "confidence": {
            "kind": "frontier_epistemic",
            "score": 0.7,
            "method": "llm_initial",
            "basis": "test",
            "extraction_confidence": 0.9
        },
        "provenance": {
            "source_type": "computation",
            "title": "test"
        },
        "flags": {
            "review_state": "accepted"
        },
        "created": "2026-06-07T00:00:00Z"
    }"#;

    #[test]
    fn reviewer_accepted_is_not_machine_sealed() {
        let f: FindingBundle =
            serde_json::from_str(REVIEWER_ACCEPTED_FINDING).expect("deserialize test finding");
        let findings = vec![f];
        let body = finding_gate_status_body(&findings, &[], "vfr_test", "vf_test0000000001")
            .expect("finding present");

        // The keystone distinction: reviewer-accepted, but NO machine seal.
        assert_eq!(body["reviewer_accepted"], serde_json::json!(true));
        assert_eq!(body["machine_sealed"], serde_json::json!(false));
        assert_eq!(body["gate_status"], serde_json::json!("needs_verification"));
        // Zero attachments -> no independence to overstate.
        assert_eq!(body["attachment_count"], serde_json::json!(0));
        assert_eq!(body["distinct_verifier_actors"], serde_json::json!(0));
        assert_eq!(body["distinct_methods"], serde_json::json!(0));
        assert_eq!(body["superseded"], serde_json::json!(false));
        assert_eq!(body["schema"], serde_json::json!("vela.gate-status.v0.1"));
    }

    #[test]
    fn absent_finding_yields_none() {
        let f: FindingBundle =
            serde_json::from_str(REVIEWER_ACCEPTED_FINDING).expect("deserialize test finding");
        let findings = vec![f];
        assert!(
            finding_gate_status_body(&findings, &[], "vfr_test", "vf_does_not_exist").is_none(),
            "absent finding must return None (404), not a body"
        );
    }
}

#[cfg(test)]
mod protocol_surface_tests {
    use super::*;

    const SITE: &str = "https://app.constellate.example";

    fn test_urls() -> PublicUrls {
        PublicUrls {
            hub: "http://127.0.0.1".to_string(),
            repo: "https://github.com/vela-science/vela".to_string(),
            site: SITE.to_string(),
        }
    }

    async fn test_state() -> AppState {
        let file = tempfile::NamedTempFile::new().expect("temp sqlite");
        let url = format!("sqlite://{}", file.path().display());
        let opts = SqliteConnectOptions::from_str(&url)
            .expect("sqlite opts")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .expect("sqlite connect");
        ensure_sqlite_schema(&pool).await.expect("schema");
        std::mem::forget(file); // keep the db file for the process lifetime
        AppState {
            db: HubDb::Sqlite(pool),
            frontier_cache: Arc::new(RwLock::new(HashMap::new())),
            db_cache: Arc::new(RwLock::new(HashMap::new())),
            db_cache_metrics: Arc::new(DbCacheMetrics::default()),
            http_metrics: Arc::new(HttpMetrics::default()),
            urls: test_urls(),
            mcp: Arc::new(tokio::sync::RwLock::new(None)),
            mcp_kick: Arc::new(tokio::sync::Notify::new()),
            webhook_secret: None,
            // High budget: these tests exercise routing, not the limiter.
            rate_limiter: Arc::new(RateLimiter::new(10_000)),
        }
    }

    /// One indexed frontier: a synthesized Git-derived index row plus its
    /// promoted projection. Returns the exact index JSON served by `/entries`
    /// and `/entries/{vfr}`.
    async fn seed_entry(state: &AppState, vfr_id: &str) -> Value {
        let HubDb::Sqlite(pool) = &state.db else {
            unreachable!("test state is sqlite")
        };
        let index_row = json!({
            "schema": vela_hub::db::FRONTIER_INDEX_SCHEMA,
            "vfr_id": vfr_id,
            "name": "Fixture frontier",
            "owner_actor_id": "reviewer:test",
            "owner_pubkey": "00".repeat(32),
            "latest_snapshot_hash": "hash_snapshot",
            "latest_event_log_hash": "hash_log",
            "git_remote": "https://example.com/frontier",
            "source_commit_at": "2026-07-01T00:00:00Z",
        });
        let project = vela_protocol::project::assemble("fixture", vec![], 10, 0, "Fixture project");
        sqlx::query(
            "INSERT INTO frontiers (vfr_id, name, owner_actor_id, \
             owner_pubkey, latest_snapshot_hash, latest_event_log_hash, schema_version, \
             source_commit_at, materialized_snapshot_json, authority_mode) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'git_ingested')",
        )
        .bind(vfr_id)
        .bind("Fixture frontier")
        .bind("reviewer:test")
        .bind("00".repeat(32))
        .bind("hash_snapshot")
        .bind("hash_log")
        .bind("v1")
        .bind("2026-07-01T00:00:00Z")
        .bind(serde_json::to_string(&project).expect("project json"))
        .execute(pool)
        .await
        .expect("insert frontier");
        state
            .db
            .upsert_git_source(vfr_id, "https://example.com/frontier", "main")
            .await
            .expect("register fixture Git source");
        index_row
    }

    /// Serve the REAL router (routes + middleware) on an ephemeral port.
    async fn serve(state: AppState) -> (String, reqwest::Client) {
        let router = build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .expect("serve");
        });
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client");
        (format!("http://{addr}"), client)
    }

    /// THE 301 matrix: every HTML arm answers `301 Location: {site}…`,
    /// and the same requests with `Accept: application/json` serve the
    /// unchanged protocol JSON (fixture equality below).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn html_arms_301_into_the_app() {
        let state = test_state().await;
        seed_entry(&state, "vfr_fix1").await;
        let (base, client) = serve(state).await;

        let matrix = [
            ("/entries", "/frontiers"),
            ("/entries/vfr_fix1", "/r/vfr_fix1"),
            (
                "/entries/vfr_fix1/findings/vf_abc",
                "/r/vfr_fix1/findings/vf_abc",
            ),
            ("/entries/vfr_fix1/review", "/r/vfr_fix1/review"),
            (
                "/entries/vfr_fix1/packs/vsd_pack1",
                "/r/vfr_fix1/packs/vsd_pack1",
            ),
            ("/entries/vfr_fix1/reproduce", "/r/vfr_fix1/reproduce"),
            ("/producers/aabbccdd", "/producer/aabbccdd"),
            ("/search?q=sidon sets", "/search?q=sidon%20sets"),
        ];
        for (path, target) in matrix {
            let resp = client
                .get(format!("{base}{path}"))
                .header("accept", "text/html")
                .send()
                .await
                .expect(path);
            assert_eq!(resp.status(), 301, "{path} must 301 for text/html");
            assert_eq!(
                resp.headers()["location"].to_str().unwrap(),
                format!("{SITE}{target}"),
                "{path} Location"
            );
            // The plain-text body names the target for curl users.
            assert!(resp.text().await.unwrap().contains(target), "{path} body");
        }

        // /reproduce is redirect-only: no Accept header still 301s.
        let resp = client
            .get(format!("{base}/entries/vfr_fix1/reproduce"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 301);

        // /review?format=html forces the redirect without an Accept header.
        let resp = client
            .get(format!("{base}/entries/vfr_fix1/review?format=html"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 301);
        assert_eq!(
            resp.headers()["location"].to_str().unwrap(),
            format!("{SITE}/r/vfr_fix1/review")
        );

        // An unknown vfr_id still redirects — the app owns not-found UX.
        let resp = client
            .get(format!("{base}/entries/vfr_nope"))
            .header("accept", "text/html")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 301);
    }

    /// Pin the current JSON shapes for /entries, /entries/{vfr},
    /// /entries/{vfr}/review, and /search, plus pagination.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn json_branches_serve_protocol_shapes() {
        let state = test_state().await;
        let index_row = seed_entry(&state, "vfr_fix1").await;
        let (base, client) = serve(state).await;
        let get_json = |path: String| {
            let client = client.clone();
            let base = base.clone();
            async move {
                let resp = client
                    .get(format!("{base}{path}"))
                    .header("accept", "application/json")
                    .send()
                    .await
                    .expect("request");
                (
                    resp.status().as_u16(),
                    resp.json::<Value>().await.expect("json body"),
                )
            }
        };

        // /entries — the frontier-index collection.
        let (status, body) = get_json("/entries".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(
            body,
            json!({"schema": FRONTIER_INDEX_LIST_SCHEMA, "entries": [index_row]})
        );

        // /entries/{vfr} — the synthesized Git index row, nothing added.
        let (status, body) = get_json("/entries/vfr_fix1".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(body, index_row);

        // /entries?limit= — ADDITIVE pagination: same schema + entries,
        // plus total (and next_offset only when more remain).
        let (status, body) = get_json("/entries?limit=1".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(
            body,
            json!({"schema": FRONTIER_INDEX_LIST_SCHEMA, "entries": [index_row], "total": 1})
        );

        // /search — bounded results plus a total when queried.
        let (status, body) = get_json("/search?q=sidon".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(
            body,
            json!({"results": [], "q": "sidon", "type": "finding", "total": 0})
        );
        // Empty q stays exactly as before (no additive fields).
        let (status, body) = get_json("/search".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(body, json!({"results": [], "q": "", "type": "finding"}));

        // /entries/{vfr}/review — derived review queue and autonomy ledger.
        let (status, body) = get_json("/entries/vfr_fix1/review".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(
            body,
            json!({
                "schema": "vela.hub.review.v0.1",
                "vfr_id": "vfr_fix1",
                "stats": {
                    "awaiting": 0,
                    "policy_admitted": 0,
                    "human_decided": 0,
                    "autonomy_ratio": 0.0,
                },
                "policy": {"active": false, "policy_id": null, "filtered": false},
                "awaiting": [],
                "policy_admitted": {"by_policy": {}, "events": []},
                "human_decisions": [],
            })
        );

        // Root JSON banner is unchanged.
        let (status, body) = get_json("/".to_string()).await;
        assert_eq!(status, 200);
        assert_eq!(body, root_json());
    }

    /// The hub is a read-only index. Human decisions are landed through the
    /// git-native Decision Plan workflow, so the hub must expose no accept
    /// route.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn truth_bearing_accept_route_remains_absent() {
        let state = test_state().await;
        seed_entry(&state, "vfr_fix1").await;
        let (base, client) = serve(state).await;
        let endpoint = format!("{base}/entries/vfr_fix1/proposals/vpr_fixture/accept");
        let response = client
            .post(&endpoint)
            .json(&json!({"reason": "Evidence and caveats checked"}))
            .send()
            .await
            .expect("accept probe");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn removed_state_transport_routes_remain_absent() {
        let state = test_state().await;
        let (base, client) = serve(state).await;

        let publish = client
            .post(format!("{base}/entries"))
            .json(&json!({"frontier": "bytes"}))
            .send()
            .await
            .expect("publish probe");
        assert_eq!(publish.status(), StatusCode::METHOD_NOT_ALLOWED);

        let source_registration = client
            .post(format!("{base}/entries/vfr_001f148c07eebecb/git-remote"))
            .json(&json!({"git_remote": "https://example.com/untrusted"}))
            .send()
            .await
            .expect("source registration probe");
        assert_eq!(source_registration.status(), StatusCode::METHOD_NOT_ALLOWED);

        let deprecate = client
            .post(format!("{base}/entries/vfr_001f148c07eebecb/deprecate"))
            .json(&json!({"reason": "retire"}))
            .send()
            .await
            .expect("deprecate probe");
        assert_eq!(deprecate.status(), StatusCode::NOT_FOUND);

        let lifecycle_status = client
            .get(format!("{base}/entries/vfr_001f148c07eebecb/status"))
            .send()
            .await
            .expect("lifecycle status probe");
        assert_eq!(lifecycle_status.status(), StatusCode::NOT_FOUND);

        for path in [
            "/entries/vfr_001f148c07eebecb/proof",
            "/entries/vfr_001f148c07eebecb/proof/download",
            "/entries/vfr_001f148c07eebecb/log/sth",
            "/entries/vfr_001f148c07eebecb/log/proof/vev_deadbeef",
            "/entries/vfr_001f148c07eebecb/log/consistency?first=1",
        ] {
            let response = client
                .get(format!("{base}{path}"))
                .send()
                .await
                .expect("retired local-artifact or signed-log probe");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }

        let blob = client
            .get(format!("{base}/blobs/{}", "a".repeat(64)))
            .send()
            .await
            .expect("blob probe");
        assert_eq!(blob.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failed_git_ingest_is_reported_without_a_parallel_audit_table() {
        let state = test_state().await;
        state
            .db
            .upsert_git_source("vfr_broken", "https://example.com/broken", "main")
            .await
            .expect("register source");
        state
            .db
            .record_git_ingest("vfr_broken", None, Some("strict replay failed"))
            .await
            .expect("record ingest failure");
        let (base, client) = serve(state).await;

        let response = client
            .get(format!("{base}/entries/vfr_broken"))
            .send()
            .await
            .expect("failure probe");
        assert_eq!(response.status(), StatusCode::FAILED_DEPENDENCY);
        let body: Value = response.json().await.expect("json failure body");
        assert_eq!(body["error"]["message"], "strict replay failed");
        assert_eq!(body["authority_mode"], "git_ingested");
    }

    /// `/.well-known/vela` carries discovery data only: no Hub signature or
    /// trust key is introduced.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn well_known_manifest_is_unsigned_discovery_only() {
        let state = test_state().await;
        let (base, client) = serve(state).await;
        let body: Value = client
            .get(format!("{base}/.well-known/vela"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(body.get("peers").is_none());
        assert!(body["endpoints"].get("publish").is_none());
        assert!(body["endpoints"].get("counterfactual").is_none());
        assert!(body["schemas"].get("counterfactual-query").is_none());
        assert!(body.get("signature").is_none());
        assert!(body.get("mode").is_none());
        assert_eq!(
            body["agent_sla"]["writes"],
            "frontier publication is git push; the hub index does not accept frontier state bytes"
        );
    }

    #[test]
    fn urlencode_preserves_unreserved_and_encodes_the_rest() {
        assert_eq!(urlencode("sidon sets"), "sidon%20sets");
        assert_eq!(urlencode("a+b&c=d"), "a%2Bb%26c%3Dd");
        assert_eq!(urlencode("Ab0-_.~"), "Ab0-_.~");
    }

    #[test]
    fn entries_payload_is_additive_only_when_limit_present() {
        let values: Vec<Value> = (0..3)
            .map(|i| json!({"vfr_id": format!("vfr_{i}")}))
            .collect();
        // No limit: the unpaginated collection shape.
        assert_eq!(
            entries_payload(&values, None, 0),
            json!({"schema": FRONTIER_INDEX_LIST_SCHEMA, "entries": values})
        );
        // Page 1 of 2: total + next_offset appear.
        assert_eq!(
            entries_payload(&values, Some(2), 0),
            json!({
                "schema": FRONTIER_INDEX_LIST_SCHEMA,
                "entries": [values[0], values[1]],
                "total": 3,
                "next_offset": 2,
            })
        );
        // Final page: no next_offset.
        assert_eq!(
            entries_payload(&values, Some(2), 2),
            json!({
                "schema": FRONTIER_INDEX_LIST_SCHEMA,
                "entries": [values[2]],
                "total": 3,
            })
        );
        // Offset past the end: empty page, still additive shape.
        assert_eq!(
            entries_payload(&values, Some(2), 99),
            json!({
                "schema": FRONTIER_INDEX_LIST_SCHEMA,
                "entries": [],
                "total": 3,
            })
        );
    }
}

#[cfg(test)]
mod webhook_signature_tests {
    use super::github_signature_ok;

    #[test]
    fn valid_signature_verifies_and_wrong_ones_do_not() {
        // hmac-sha256("secret", "payload") — precomputable with any HMAC
        // implementation; pinned here so the header format is exercised
        // end-to-end (sha256= prefix + lowercase hex).
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(b"payload");
        let good = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        assert!(github_signature_ok("secret", b"payload", &good));
        assert!(!github_signature_ok("secret", b"tampered", &good));
        assert!(!github_signature_ok("wrong-secret", b"payload", &good));
        assert!(!github_signature_ok(
            "secret",
            b"payload",
            "sha256=deadbeef"
        ));
        assert!(!github_signature_ok("secret", b"payload", "no-prefix"));
        assert!(!github_signature_ok("secret", b"payload", ""));
    }
}
