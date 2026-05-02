//! v0.48: local workbench — axum web app rendering the substrate
//! against the cwd's `.vela/` repo.
//!
//! Doctrine: the static site (vela-site.fly.dev) is a marketing surface
//! bundled against one frontier at build time. The workbench is a
//! single-binary, single-user, localhost UI that renders the *user's*
//! frontier, with read+write actions that hit the same on-disk
//! representation `vela <subcommand>` would.
//!
//! Architecture:
//! - Pure Rust + axum. No node, no bun, no static-build step.
//! - Each request reads from disk. Writes call back into the same
//!   modules `vela <cmd>` uses (e.g., bridge confirm rewrites the
//!   `.vela/bridges/<vbr_id>.json` file in place).
//! - Shared CSS with the hub (`web/styles/tokens.css`,
//!   `web/styles/workbench.css`) via `include_str!`.
//! - Auto-opens the default browser on start unless `--no-open`.

#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use tower_http::cors::CorsLayer;

use crate::bridge::{Bridge, BridgeStatus};
use crate::causal_reasoning::{Identifiability, audit_frontier, summarize_audit};
use crate::project::Project;
use crate::repo;

const TOKENS_CSS: &str = include_str!("../../../web/styles/tokens.css");
const WORKBENCH_CSS: &str = include_str!("../../../web/styles/workbench.css");

const FAVICON_SVG: &str = include_str!("../../../assets/brand/favicon.svg");

const WB_VERSION: &str = "0.48.0"; // tracks Cargo.toml workspace version

/// Workbench app state: the absolute path to the user's `.vela/` repo
/// (its parent — the path that `repo::load_from_path` accepts).
#[derive(Clone)]
struct AppState {
    repo_path: Arc<PathBuf>,
}

/// Start the workbench on `127.0.0.1:<port>`, against `repo_path`. If
/// `open_browser` is true, opens the default browser at the local URL.
pub async fn run(repo_path: PathBuf, port: u16, open_browser: bool) -> Result<(), String> {
    if !repo_path.join(".vela").is_dir() {
        return Err(format!(
            "no .vela/ found at {} — run `vela init` first",
            repo_path.display()
        ));
    }
    // Sanity-check loadability before binding the port.
    let _ =
        repo::load_from_path(&repo_path).map_err(|e| format!("failed to load .vela/ repo: {e}"))?;

    let state = AppState {
        repo_path: Arc::new(repo_path),
    };

    let app = Router::new()
        .route("/", get(page_dashboard))
        .route("/findings", get(page_findings))
        .route("/findings/{vf_id}", get(page_finding_detail))
        .route("/audit", get(page_audit))
        .route("/bridges", get(page_bridges))
        .route("/bridges/{vbr_id}/confirm", post(post_bridge_confirm))
        .route("/bridges/{vbr_id}/refute", post(post_bridge_refute))
        .route("/static/tokens.css", get(static_tokens_css))
        .route("/static/workbench.css", get(static_workbench_css))
        .route("/static/favicon.svg", get(static_favicon_svg))
        .route("/healthz", get(healthz))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;
    let actual_addr = listener.local_addr().unwrap_or(addr);
    let url = format!("http://{actual_addr}/");

    println!("vela workbench listening on {url}");
    if open_browser && let Err(e) = open_browser_at(&url) {
        eprintln!("(could not auto-open browser: {e})");
    }
    println!("Ctrl-C to stop.");

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("axum serve: {e}"))
}

fn open_browser_at(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";

    std::process::Command::new(cmd)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{cmd}: {e}"))
}

// ── HTML helpers ─────────────────────────────────────────────────────

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn shell(active: &str, title: &str, eyebrow: &str, page_title: &str, body: &str) -> String {
    let nav = |id: &str, href: &str, label: &str| -> String {
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
      <span style="display:inline-block;width:26px;height:26px;background:#1a1a1a;border-radius:3px;color:#fff;font-family:ui-monospace,Menlo,monospace;font-size:11px;line-height:26px;text-align:center;font-weight:700;">v</span>
    </a>
  </div>
  <nav class="wb-rim__nav" aria-label="Workbench">
    {l1}
    {l2}
    {l3}
    {l4}
  </nav>
  <div class="wb-rim__index">v{ver}</div>
</aside>"#,
        l1 = nav("dashboard", "/", "01 · Dashboard"),
        l2 = nav("findings", "/findings", "02 · Findings"),
        l3 = nav("audit", "/audit", "03 · Audit"),
        l4 = nav("bridges", "/bridges", "04 · Bridges"),
        ver = WB_VERSION,
    );
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title_safe}</title>
<link rel="icon" type="image/svg+xml" href="/static/favicon.svg">
<link rel="stylesheet" href="/static/tokens.css">
<link rel="stylesheet" href="/static/workbench.css">
<style>
  body {{ margin: 0; font-family: var(--font-text, system-ui, sans-serif); color: var(--ink-1, #1a1a1a); background: var(--bg-1, #fafaf6); }}
  .wb {{ display: grid; grid-template-columns: 200px 1fr; min-height: 100vh; }}
  .wb-rim {{ background: var(--bg-2, #f5f2ec); padding: 1rem 0.75rem; border-right: 1px solid var(--rule-2, #d8d4cc); }}
  .wb-rim__mark {{ margin-bottom: 1.5rem; }}
  .wb-rim__nav {{ display: flex; flex-direction: column; gap: 0.4rem; }}
  .wb-rim__link {{ font-size: 0.86rem; color: var(--ink-2, #6b665d); text-decoration: none; padding: 0.3rem 0.5rem; border-radius: 2px; }}
  .wb-rim__link--on {{ color: var(--ink-1, #1a1a1a); background: var(--bg-3, #ebe6dd); font-weight: 600; }}
  .wb-rim__index {{ margin-top: 2rem; color: var(--ink-3, #a09a8d); font-size: 0.74rem; font-family: ui-monospace, Menlo, monospace; }}
  .wb-content {{ padding: 1.5rem 2rem; max-width: 920px; }}
  .wb-eyebrow {{ font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--ink-2, #6b665d); margin-bottom: 0.4rem; }}
  .wb-title {{ font-size: 1.6rem; margin: 0 0 1rem 0; line-height: 1.2; }}
  .wb-stats {{ display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin: 1rem 0 1.5rem 0; padding: 0.85rem 1rem; border: 1px solid var(--rule-2, #d8d4cc); background: var(--bg-2, #f5f2ec); }}
  .wb-stat__num {{ font-family: ui-monospace, Menlo, monospace; font-size: 1.3rem; font-weight: 600; }}
  .wb-stat__label {{ font-size: 0.74rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-2, #6b665d); }}
  .wb-card {{ border: 1px solid var(--rule-2, #d8d4cc); padding: 0.85rem 1rem; margin: 0 0 0.85rem 0; }}
  .wb-card h3 {{ margin: 0 0 0.4rem 0; font-size: 1rem; }}
  .wb-card p {{ margin: 0.2rem 0; font-size: 0.92rem; line-height: 1.55; }}
  .wb-chip {{ display: inline-block; padding: 0.05em 0.5em; border-radius: 2px; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.06em; margin-right: 0.4em; }}
  .wb-chip--ok {{ background: #d6e4d3; color: #2f5d3a; }}
  .wb-chip--warn {{ background: #efe2c0; color: #8a6d1f; }}
  .wb-chip--lost {{ background: #efd1cf; color: #872c2c; }}
  .wb-table {{ width: 100%; border-collapse: collapse; font-size: 0.92rem; }}
  .wb-table th, .wb-table td {{ text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--rule-2, #d8d4cc); }}
  .wb-table th {{ font-size: 0.74rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--ink-2, #6b665d); }}
  .wb-actions form {{ display: inline-block; margin-right: 0.4em; }}
  .wb-actions button {{ font-family: inherit; font-size: 0.78rem; padding: 0.25em 0.6em; border: 1px solid var(--rule-2, #d8d4cc); background: var(--bg-1, #fafaf6); cursor: pointer; border-radius: 2px; }}
  .wb-actions button:hover {{ background: var(--bg-3, #ebe6dd); }}
  code {{ background: var(--bg-3, #ebe6dd); padding: 0.05em 0.3em; border-radius: 2px; font-size: 0.88em; }}
  a {{ color: var(--ink-1, #1a1a1a); }}
</style>
</head>
<body>
<div class="wb">
{rim}
<main class="wb-content">
  <div class="wb-eyebrow">{eyebrow}</div>
  <h1 class="wb-title">{page_title}</h1>
  {body}
</main>
</div>
</body>
</html>
"#,
        title_safe = escape_html(title),
    )
}

fn frontier_label(p: &Project) -> String {
    p.project.name.clone()
}

// ── Pages ────────────────────────────────────────────────────────────

async fn page_dashboard(State(state): State<AppState>) -> Response {
    let repo_path = state.repo_path.clone();
    let project = match repo::load_from_path(&repo_path) {
        Ok(p) => p,
        Err(e) => return error_page("dashboard", "Could not load frontier", &e),
    };
    let label = frontier_label(&project);

    let mut pending = 0usize;
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for p in &project.proposals {
        if p.status == "pending_review" {
            pending += 1;
            *by_kind.entry(p.kind.clone()).or_insert(0) += 1;
        }
    }

    let audit = audit_frontier(&project);
    let audit_summary = summarize_audit(&audit);

    let bridges = list_bridges(&repo_path);
    let bridge_total = bridges.len();
    let bridge_confirmed = bridges
        .iter()
        .filter(|b| b.status == BridgeStatus::Confirmed)
        .count();
    let bridge_derived = bridges
        .iter()
        .filter(|b| b.status == BridgeStatus::Derived)
        .count();

    let mut targets_with_success = std::collections::HashSet::new();
    let mut failed_replications = 0usize;
    for r in &project.replications {
        if r.outcome == "replicated" {
            targets_with_success.insert(r.target_finding.clone());
        } else if r.outcome == "failed" {
            failed_replications += 1;
        }
    }

    let stats_html = format!(
        r#"<div class="wb-stats">
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">findings</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">events</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">pending</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">bridges</div></div>
</div>"#,
        project.findings.len(),
        project.events.len(),
        pending,
        bridge_total
    );

    let mut cards = String::new();

    if pending > 0 {
        let parts: Vec<String> = by_kind
            .iter()
            .map(|(k, n)| format!("<code>{n}</code> {}", escape_html(k)))
            .collect();
        cards.push_str(&format!(
            r#"<div class="wb-card">
  <h3><span class="wb-chip wb-chip--warn">inbox</span>{} pending proposals</h3>
  <p>{}</p>
  <p><a href="/audit">Open audit →</a></p>
</div>"#,
            pending,
            parts.join(" · ")
        ));
    }

    if audit_summary.underidentified > 0 || audit_summary.conditional > 0 {
        let chip_kind = if audit_summary.underidentified > 0 {
            "lost"
        } else {
            "warn"
        };
        cards.push_str(&format!(
            r#"<div class="wb-card">
  <h3><span class="wb-chip wb-chip--{chip}">audit</span>identifiability</h3>
  <p><strong>{}</strong> underidentified · <strong>{}</strong> conditional · <strong>{}</strong> identified</p>
  <p><a href="/audit">Open audit →</a></p>
</div>"#,
            audit_summary.underidentified,
            audit_summary.conditional,
            audit_summary.identified,
            chip = chip_kind,
        ));
    }

    if bridge_total > 0 {
        cards.push_str(&format!(
            r#"<div class="wb-card">
  <h3><span class="wb-chip wb-chip--ok">bridges</span>cross-frontier composition</h3>
  <p><strong>{bridge_total}</strong> total · <strong>{bridge_confirmed}</strong> confirmed · <strong>{bridge_derived}</strong> awaiting review</p>
  <p><a href="/bridges">Open bridges →</a></p>
</div>"#
        ));
    }

    if !project.replications.is_empty() {
        cards.push_str(&format!(
            r#"<div class="wb-card">
  <h3><span class="wb-chip wb-chip--ok">replications</span>empirical bedrock</h3>
  <p><strong>{}</strong> records · <strong>{}</strong> findings replicated · <strong>{}</strong> failed</p>
</div>"#,
            project.replications.len(),
            targets_with_success.len(),
            failed_replications
        ));
    }

    let body = format!("{stats_html}{cards}");

    Html(shell(
        "dashboard",
        &format!("Vela workbench · {label}"),
        "Workbench",
        &escape_html(&label),
        &body,
    ))
    .into_response()
}

async fn page_findings(State(state): State<AppState>) -> Response {
    let project = match repo::load_from_path(&state.repo_path) {
        Ok(p) => p,
        Err(e) => return error_page("findings", "Could not load frontier", &e),
    };

    let mut rows = String::new();
    for f in project.findings.iter().take(500) {
        let conf_pct = (f.confidence.score * 100.0).round() as i64;
        let claim = f.assertion.causal_claim.map_or("—", |c| match c {
            crate::bundle::CausalClaim::Correlation => "correlation",
            crate::bundle::CausalClaim::Mediation => "mediation",
            crate::bundle::CausalClaim::Intervention => "intervention",
        });
        let assertion_short: String = f.assertion.text.chars().take(110).collect();
        rows.push_str(&format!(
            r#"<tr>
  <td><a href="/findings/{vf}"><code>{vf_short}</code></a></td>
  <td>{conf}%</td>
  <td>{claim}</td>
  <td>{text}</td>
</tr>"#,
            vf = escape_html(&f.id),
            vf_short = escape_html(&f.id),
            conf = conf_pct,
            claim = claim,
            text = escape_html(&assertion_short),
        ));
    }

    let body = format!(
        r#"<table class="wb-table">
  <thead>
    <tr><th>vf_id</th><th>conf</th><th>claim</th><th>assertion</th></tr>
  </thead>
  <tbody>
{rows}
  </tbody>
</table>"#
    );

    Html(shell(
        "findings",
        "Findings",
        "Workbench",
        &format!("{} findings", project.findings.len()),
        &body,
    ))
    .into_response()
}

async fn page_finding_detail(
    AxumPath(vf_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Response {
    let project = match repo::load_from_path(&state.repo_path) {
        Ok(p) => p,
        Err(e) => return error_page("findings", "Could not load frontier", &e),
    };
    let Some(f) = project.findings.iter().find(|f| f.id == vf_id) else {
        return error_page(
            "findings",
            "Finding not found",
            &format!("no finding with id {vf_id}"),
        );
    };

    let conf_pct = (f.confidence.score * 100.0).round() as i64;

    let mut links_html = String::new();
    if !f.links.is_empty() {
        links_html.push_str(r#"<table class="wb-table"><thead><tr><th>type</th><th>target</th><th>mechanism</th></tr></thead><tbody>"#);
        for l in &f.links {
            let mech = l.mechanism.map_or("—".to_string(), |m| {
                use crate::bundle::Mechanism;
                match m {
                    Mechanism::Linear { sign, slope } => {
                        format!("linear {sign:?} slope {slope:.2}")
                    }
                    Mechanism::Monotonic { sign } => format!("monotonic {sign:?}"),
                    Mechanism::Threshold { sign, threshold } => {
                        format!("threshold {sign:?} {threshold:.2}")
                    }
                    Mechanism::Saturating { sign, half_max } => {
                        format!("saturating {sign:?} half_max {half_max:.2}")
                    }
                    Mechanism::Unknown => "unknown".into(),
                }
            });
            links_html.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
                escape_html(&l.link_type),
                escape_html(&l.target),
                escape_html(&mech)
            ));
        }
        links_html.push_str("</tbody></table>");
    }

    let assertion = escape_html(&f.assertion.text);
    let body = format!(
        r#"<div class="wb-stats">
  <div><div class="wb-stat__num">{conf_pct}%</div><div class="wb-stat__label">confidence</div></div>
  <div><div class="wb-stat__num">{n_links}</div><div class="wb-stat__label">links</div></div>
  <div><div class="wb-stat__num">{atype}</div><div class="wb-stat__label">type</div></div>
  <div><div class="wb-stat__num">{ver}</div><div class="wb-stat__label">version</div></div>
</div>
<div class="wb-card">
  <h3>Assertion</h3>
  <p>{assertion}</p>
</div>
<div class="wb-card">
  <h3>Links</h3>
  {links_html}
</div>"#,
        n_links = f.links.len(),
        atype = escape_html(&f.assertion.assertion_type),
        ver = f.version,
    );

    Html(shell(
        "findings",
        &format!("{} · {}", vf_id, project.project.name),
        "Finding",
        &vf_id,
        &body,
    ))
    .into_response()
}

async fn page_audit(State(state): State<AppState>) -> Response {
    let project = match repo::load_from_path(&state.repo_path) {
        Ok(p) => p,
        Err(e) => return error_page("audit", "Could not load frontier", &e),
    };

    let mut entries = audit_frontier(&project);
    let summary = summarize_audit(&entries);
    entries.retain(|e| {
        matches!(
            e.verdict,
            Identifiability::Underidentified | Identifiability::Conditional
        )
    });

    let mut rows = String::new();
    for e in &entries {
        let chip = match e.verdict {
            Identifiability::Underidentified => "lost",
            Identifiability::Conditional => "warn",
            _ => continue,
        };
        let claim = e
            .causal_claim
            .map_or("—".to_string(), |c| format!("{c:?}").to_lowercase());
        let grade = e
            .causal_evidence_grade
            .map_or("—".to_string(), |g| format!("{g:?}").to_lowercase());
        let text: String = e.assertion_text.chars().take(120).collect();
        rows.push_str(&format!(
            r#"<tr>
  <td><span class="wb-chip wb-chip--{chip}">{verdict}</span></td>
  <td><a href="/findings/{vf}"><code>{vf_short}</code></a></td>
  <td>{claim} / {grade}</td>
  <td>{text}</td>
</tr>"#,
            chip = chip,
            verdict = match e.verdict {
                Identifiability::Underidentified => "underidentified",
                Identifiability::Conditional => "conditional",
                _ => "—",
            },
            vf = escape_html(&e.finding_id),
            vf_short = escape_html(&e.finding_id),
            claim = escape_html(&claim),
            grade = escape_html(&grade),
            text = escape_html(&text),
        ));
    }

    let stats_html = format!(
        r#"<div class="wb-stats">
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">identified</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">conditional</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">underidentified</div></div>
  <div><div class="wb-stat__num">{}</div><div class="wb-stat__label">underdetermined</div></div>
</div>"#,
        summary.identified, summary.conditional, summary.underidentified, summary.underdetermined,
    );

    let body = if entries.is_empty() {
        format!(
            "{stats_html}<div class=\"wb-card\"><p>No reviewer-attention items. Audit clean.</p></div>"
        )
    } else {
        format!(
            r#"{stats_html}
<table class="wb-table">
  <thead>
    <tr><th>verdict</th><th>finding</th><th>claim/grade</th><th>assertion</th></tr>
  </thead>
  <tbody>
{rows}
  </tbody>
</table>"#
        )
    };

    Html(shell(
        "audit",
        "Causal audit",
        "Workbench",
        "Identifiability audit",
        &body,
    ))
    .into_response()
}

async fn page_bridges(State(state): State<AppState>) -> Response {
    let bridges = list_bridges(&state.repo_path);

    if bridges.is_empty() {
        let body = r#"<div class="wb-card">
  <p>No bridges yet. Derive one with:</p>
  <p><code>vela bridges derive &lt;frontier_a&gt; &lt;frontier_b&gt;</code></p>
</div>"#;
        return Html(shell("bridges", "Bridges", "Workbench", "No bridges", body)).into_response();
    }

    let mut cards = String::new();
    for b in &bridges {
        let chip = match b.status {
            BridgeStatus::Confirmed => "ok",
            BridgeStatus::Refuted => "lost",
            BridgeStatus::Derived => "warn",
        };
        let chip_label = match b.status {
            BridgeStatus::Confirmed => "confirmed",
            BridgeStatus::Refuted => "refuted",
            BridgeStatus::Derived => "derived",
        };

        let mut refs_html = String::new();
        for r in b.finding_refs.iter().take(6) {
            let txt: String = r.assertion_text.chars().take(110).collect();
            refs_html.push_str(&format!(
                "<p>· <code>[{}]</code> <code>{}</code> conf {:.2} — {}</p>",
                escape_html(&r.frontier),
                escape_html(&r.finding_id),
                r.confidence,
                escape_html(&txt),
            ));
        }
        if b.finding_refs.len() > 6 {
            refs_html.push_str(&format!("<p>… and {} more</p>", b.finding_refs.len() - 6));
        }

        let actions_html = match b.status {
            BridgeStatus::Derived => format!(
                r#"<div class="wb-actions">
  <form method="post" action="/bridges/{id}/confirm"><button type="submit">Confirm</button></form>
  <form method="post" action="/bridges/{id}/refute"><button type="submit">Refute</button></form>
</div>"#,
                id = escape_html(&b.id),
            ),
            BridgeStatus::Confirmed => format!(
                r#"<div class="wb-actions">
  <form method="post" action="/bridges/{id}/refute"><button type="submit">Mark refuted</button></form>
</div>"#,
                id = escape_html(&b.id),
            ),
            BridgeStatus::Refuted => format!(
                r#"<div class="wb-actions">
  <form method="post" action="/bridges/{id}/confirm"><button type="submit">Re-confirm</button></form>
</div>"#,
                id = escape_html(&b.id),
            ),
        };

        let tension_html = b.tension.as_deref().map_or(String::new(), |t| {
            format!(
                r#"<p style="color:#872c2c;font-style:italic;">tension: {}</p>"#,
                escape_html(t)
            )
        });

        cards.push_str(&format!(
            r#"<div class="wb-card">
  <h3><span class="wb-chip wb-chip--{chip}">{chip_label}</span><code>{id}</code> · {entity}</h3>
  <p><strong>frontiers:</strong> {frontiers} · <strong>findings:</strong> {n_refs}</p>
  {tension_html}
  {refs_html}
  {actions_html}
</div>"#,
            chip = chip,
            chip_label = chip_label,
            id = escape_html(&b.id),
            entity = escape_html(&b.entity_name),
            frontiers = escape_html(&b.frontiers.join(" ↔ ")),
            n_refs = b.finding_refs.len(),
        ));
    }

    let body = cards;

    Html(shell(
        "bridges",
        "Bridges",
        "Workbench",
        &format!("{} cross-frontier bridge(s)", bridges.len()),
        &body,
    ))
    .into_response()
}

async fn post_bridge_confirm(
    AxumPath(vbr_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Response {
    set_bridge_status(&state.repo_path, &vbr_id, BridgeStatus::Confirmed);
    Redirect::to("/bridges").into_response()
}

async fn post_bridge_refute(
    AxumPath(vbr_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Response {
    set_bridge_status(&state.repo_path, &vbr_id, BridgeStatus::Refuted);
    Redirect::to("/bridges").into_response()
}

// ── Bridge persistence (mirrors cli.rs cmd_bridges) ─────────────────

fn bridges_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(".vela/bridges")
}

fn list_bridges(repo_path: &Path) -> Vec<Bridge> {
    let dir = bridges_dir(repo_path);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(data) = std::fs::read_to_string(&p)
                && let Ok(b) = serde_json::from_str::<Bridge>(&data)
            {
                out.push(b);
            }
        }
    }
    out.sort_by(|a, b| {
        b.finding_refs
            .len()
            .cmp(&a.finding_refs.len())
            .then(a.entity_name.cmp(&b.entity_name))
    });
    out
}

fn set_bridge_status(repo_path: &Path, vbr_id: &str, status: BridgeStatus) {
    let p = bridges_dir(repo_path).join(format!("{vbr_id}.json"));
    let Ok(data) = std::fs::read_to_string(&p) else {
        return;
    };
    let Ok(mut b) = serde_json::from_str::<Bridge>(&data) else {
        return;
    };
    b.status = status;
    if let Ok(out) = serde_json::to_string_pretty(&b) {
        let _ = std::fs::write(&p, format!("{out}\n"));
    }
}

// ── Static assets ───────────────────────────────────────────────────

async fn static_tokens_css() -> Response {
    css_response(TOKENS_CSS)
}
async fn static_workbench_css() -> Response {
    css_response(WORKBENCH_CSS)
}
async fn static_favicon_svg() -> Response {
    svg_response(FAVICON_SVG)
}
async fn healthz() -> Response {
    (StatusCode::OK, "ok").into_response()
}

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

fn error_page(active: &str, title: &str, message: &str) -> Response {
    let body = format!(
        r#"<div class="wb-card"><h3>{title}</h3><p>{msg}</p></div>"#,
        title = escape_html(title),
        msg = escape_html(message)
    );
    let html = shell(active, title, "Workbench", title, &body);
    (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
}
