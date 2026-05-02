//! Read-only MCP/HTTP frontier server.

#![allow(clippy::too_many_lines)]

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use reqwest::Client;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::bundle::FindingBundle;
use crate::project::{self, ConfidenceDistribution, Project, ProjectStats};
use crate::{bridge, events, observer, repo, signals, sources, state, tool_registry};

pub enum ProjectSource {
    Single(PathBuf),
    Directory(PathBuf),
}

impl ProjectSource {
    pub fn from_args(single: Option<&Path>, dir: Option<&Path>) -> Self {
        if let Some(d) = dir {
            Self::Directory(d.to_path_buf())
        } else if let Some(s) = single {
            Self::Single(s.to_path_buf())
        } else {
            eprintln!(
                "{} provide either a frontier file or --frontiers <dir>",
                crate::cli_style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

#[derive(Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub file: String,
    pub findings_count: usize,
    pub links_count: usize,
    pub papers: usize,
}

pub fn load_projects(source: &ProjectSource) -> (Project, Vec<ProjectInfo>) {
    match source {
        ProjectSource::Single(path) => {
            let mut frontier = repo::load_from_path(path).unwrap_or_else(|e| {
                eprintln!(
                    "{} failed to load frontier: {e}",
                    crate::cli_style::err_prefix()
                );
                std::process::exit(1);
            });
            sources::materialize_project(&mut frontier);
            let info = ProjectInfo {
                name: frontier.project.name.clone(),
                file: path.display().to_string(),
                findings_count: frontier.findings.len(),
                links_count: frontier.stats.links,
                papers: frontier.project.papers_processed,
            };
            (frontier, vec![info])
        }
        ProjectSource::Directory(dir) => {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| {
                    eprintln!(
                        "{} failed to read directory: {e}",
                        crate::cli_style::err_prefix()
                    );
                    std::process::exit(1);
                })
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    (path.is_dir() && path.join(".vela").exists())
                        || path.extension().is_some_and(|ext| ext == "json")
                })
                .collect();
            entries.sort();
            if entries.is_empty() {
                eprintln!("no frontier files found in {}", dir.display());
                std::process::exit(1);
            }

            let mut named = Vec::new();
            for path in &entries {
                let mut frontier = repo::load_from_path(path).unwrap_or_else(|e| {
                    eprintln!(
                        "{} failed to load {}: {e}",
                        crate::cli_style::err_prefix(),
                        path.display()
                    );
                    std::process::exit(1);
                });
                sources::materialize_project(&mut frontier);
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                named.push((name, frontier));
            }
            let infos = named
                .iter()
                .map(|(name, frontier)| ProjectInfo {
                    name: frontier.project.name.clone(),
                    file: name.clone(),
                    findings_count: frontier.findings.len(),
                    links_count: frontier.stats.links,
                    papers: frontier.project.papers_processed,
                })
                .collect::<Vec<_>>();
            (merge_projects(named), infos)
        }
    }
}

fn merge_projects(frontiers: Vec<(String, Project)>) -> Project {
    let mut findings = Vec::<FindingBundle>::new();
    let mut categories = HashMap::<String, usize>::new();
    let mut link_types = HashMap::<String, usize>::new();
    let mut names = Vec::new();
    let mut papers_processed = 0usize;
    let mut errors = 0usize;
    // v0.36.2: preserve v0.32+ kernel objects across the merge.
    // Pre-v0.36.2, `replications`, `datasets`, `code_artifacts`,
    // `predictions`, and `resolutions` were dropped during merge,
    // leaving the merged stats reading the legacy `evidence.replicated`
    // scalar instead of the structured collection.
    let mut replications = Vec::new();
    let mut datasets = Vec::new();
    let mut code_artifacts = Vec::new();
    let mut predictions = Vec::new();
    let mut resolutions = Vec::new();

    for (name, frontier) in frontiers {
        names.push(name);
        papers_processed += frontier.project.papers_processed;
        errors += frontier.project.errors;
        for (category, count) in frontier.stats.categories {
            *categories.entry(category).or_default() += count;
        }
        for (link_type, count) in frontier.stats.link_types {
            *link_types.entry(link_type).or_default() += count;
        }
        findings.extend(frontier.findings);
        replications.extend(frontier.replications);
        datasets.extend(frontier.datasets);
        code_artifacts.extend(frontier.code_artifacts);
        predictions.extend(frontier.predictions);
        resolutions.extend(frontier.resolutions);
    }

    let mut deduped = Vec::<FindingBundle>::new();
    let mut seen = HashMap::<String, usize>::new();
    for finding in findings {
        if let Some(existing) = seen.get(&finding.id).copied() {
            if finding.confidence.score > deduped[existing].confidence.score {
                deduped[existing] = finding;
            }
        } else {
            seen.insert(finding.id.clone(), deduped.len());
            deduped.push(finding);
        }
    }

    let links = deduped.iter().map(|finding| finding.links.len()).sum();
    // v0.36.2: count from the merged `replications` collection, with
    // legacy `evidence.replicated` as fall-through for findings without
    // structured records.
    let mut targets_with_success: HashSet<&str> = HashSet::new();
    let mut targets_with_any_record: HashSet<&str> = HashSet::new();
    for r in &replications {
        targets_with_any_record.insert(r.target_finding.as_str());
        if r.outcome == "replicated" {
            targets_with_success.insert(r.target_finding.as_str());
        }
    }
    let replicated = deduped
        .iter()
        .filter(|finding| {
            if targets_with_any_record.contains(finding.id.as_str()) {
                targets_with_success.contains(finding.id.as_str())
            } else {
                finding.evidence.replicated
            }
        })
        .count();
    let avg_confidence = if deduped.is_empty() {
        0.0
    } else {
        (deduped
            .iter()
            .map(|finding| finding.confidence.score)
            .sum::<f64>()
            / deduped.len() as f64
            * 1000.0)
            .round()
            / 1000.0
    };
    let stats = ProjectStats {
        findings: deduped.len(),
        links,
        replicated,
        unreplicated: deduped.len().saturating_sub(replicated),
        avg_confidence,
        gaps: deduped.iter().filter(|finding| finding.flags.gap).count(),
        negative_space: deduped
            .iter()
            .filter(|finding| finding.flags.negative_space)
            .count(),
        contested: deduped
            .iter()
            .filter(|finding| finding.flags.contested)
            .count(),
        categories,
        link_types,
        human_reviewed: deduped
            .iter()
            .filter(|finding| {
                finding
                    .provenance
                    .review
                    .as_ref()
                    .is_some_and(|review| review.reviewed)
            })
            .count(),
        review_event_count: 0,
        confidence_update_count: 0,
        event_count: 0,
        source_count: 0,
        evidence_atom_count: 0,
        condition_record_count: 0,
        proposal_count: 0,
        confidence_distribution: ConfidenceDistribution {
            high_gt_80: deduped
                .iter()
                .filter(|finding| finding.confidence.score > 0.8)
                .count(),
            medium_60_80: deduped
                .iter()
                .filter(|finding| (0.6..=0.8).contains(&finding.confidence.score))
                .count(),
            low_lt_60: deduped
                .iter()
                .filter(|finding| finding.confidence.score < 0.6)
                .count(),
        },
    };

    let mut project = Project {
        vela_version: project::VELA_SCHEMA_VERSION.to_string(),
        schema: project::VELA_SCHEMA_URL.to_string(),
        frontier_id: None,
        project: project::ProjectMeta {
            name: format!("merged: {}", names.join(", ")),
            description: format!("Merged from {} frontiers", names.len()),
            compiled_at: chrono::Utc::now().to_rfc3339(),
            compiler: project::VELA_COMPILER_VERSION.to_string(),
            papers_processed,
            errors,
            dependencies: Vec::new(),
        },
        stats,
        findings: deduped,
        sources: Vec::new(),
        evidence_atoms: Vec::new(),
        condition_records: Vec::new(),
        review_events: Vec::new(),
        confidence_updates: Vec::new(),
        events: Vec::new(),
        proposals: Vec::new(),
        proof_state: Default::default(),
        signatures: Vec::new(),
        actors: Vec::new(),
        replications,
        datasets,
        code_artifacts,
        predictions,
        resolutions,
        peers: Vec::new(),
    };
    sources::materialize_project(&mut project);
    project
}

pub async fn run(source: ProjectSource, _backend: Option<&str>) {
    dotenvy::dotenv().ok();
    let (frontier, project_infos) = load_projects(&source);
    let source_path: Option<PathBuf> = match &source {
        ProjectSource::Single(path) => Some(path.clone()),
        ProjectSource::Directory(_) => None,
    };
    let frontier = Arc::new(Mutex::new(frontier));
    let client = Client::new();
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or_default();
        let response = match method {
            "initialize" => json_rpc_result(
                &id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "vela", "version": project::VELA_SCHEMA_VERSION}
                }),
            ),
            "notifications/initialized" => continue,
            "tools/list" => json_rpc_result(&id, json!({"tools": tool_registry::mcp_tools_json()})),
            "tools/call" => {
                let name = request["params"]["name"].as_str().unwrap_or_default();
                let args = request["params"]["arguments"].clone();
                handle_tool_call(
                    &id,
                    name,
                    &args,
                    &frontier,
                    &client,
                    &project_infos,
                    source_path.as_deref(),
                )
                .await
            }
            "ping" => json_rpc_result(&id, json!({})),
            _ => json_rpc_error(&id, -32601, "Method not found"),
        };
        let mut out = stdout.lock();
        let _ = serde_json::to_writer(&mut out, &response);
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }
}

pub async fn run_http(source: ProjectSource, backend: Option<&str>, port: u16, workbench: bool) {
    let _ = backend;
    dotenvy::dotenv().ok();
    let (frontier, project_infos) = load_projects(&source);
    let source_path = match &source {
        ProjectSource::Single(path) => Some(path.clone()),
        ProjectSource::Directory(_) => None,
    };
    let state = AppState {
        project: Arc::new(Mutex::new(frontier)),
        project_infos,
        client: Client::new(),
        source_path,
    };

    let mut app = Router::new()
        .route("/api/frontier", get(http_frontier))
        .route("/api/findings", get(http_findings))
        .route("/api/findings/{id}", get(http_finding_by_id))
        .route("/api/contradictions", get(http_contradictions))
        .route("/api/observer/{policy}", get(http_observer))
        .route("/api/propagate/{id}", get(http_propagate))
        .route("/api/hypotheses", get(http_bridges))
        .route("/api/stats", get(http_stats))
        .route("/api/frontiers", get(http_frontiers))
        .route("/api/pubmed", get(http_pubmed))
        // Phase Q-r (v0.5): cursor-paginated event-log read for agent
        // loops and public consumers. The canonical event log is
        // already ordered and content-addressed, so the cursor is just
        // the last seen `vev_…`.
        .route("/api/events", get(http_events))
        // Phase R (v0.5): Workbench draft queue. Browser POSTs unsigned
        // intents here; `vela queue sign` is the only path that turns
        // them into signed canonical state. The Ed25519 key never
        // enters the browser.
        .route("/api/queue", post(http_queue_append))
        .route("/api/tools", get(http_tools_list))
        .route("/api/tool", post(http_tool_call));

    // When --workbench, also serve the static `web/` directory at /
    // alongside the API. The canonical Workbench UI now lives in the
    // Astro site (vela-site.fly.dev/workbench) and proxies /api/* here;
    // --workbench remains for local development against any web/ tree.
    if workbench {
        let web_dir = workbench_web_dir();
        if web_dir.exists() {
            app = app.fallback_service(tower_http::services::ServeDir::new(web_dir));
        } else {
            eprintln!(
                "{} --workbench: web/ directory not found at expected location; serving API only",
                crate::cli_style::err_prefix()
            );
        }
    }

    let app = app.layer(CorsLayer::permissive()).with_state(state);

    let addr = format!("0.0.0.0:{port}");
    eprintln!(
        "  {}",
        if workbench {
            format!("VELA · WORKBENCH :{port}").to_uppercase()
        } else {
            format!("VELA · SERVE · HTTP :{port}").to_uppercase()
        }
        .as_str()
    );
    eprintln!("  {}", crate::cli_style::tick_row(60));
    eprintln!("  listening on http://{addr}");
    if workbench {
        // v0.29: print the deep link the researcher actually opens.
        // The deployed Astro page accepts ?api=… and bypasses the hub
        // — same UI, local data. This was the v0.28 friction-pass
        // forcing function (Friction #1: "researcher with a local
        // frontier should not need to publish before reviewing in a
        // browser"). Same banner works against `npm run dev` at
        // localhost:4321 too.
        eprintln!(
            "  workbench UI: https://vela-site.fly.dev/frontiers/view?api=http://{addr}"
        );
        eprintln!(
            "                (or  http://localhost:4321/frontiers/view?api=http://{addr}  for a local site)"
        );
    }
    eprintln!("  endpoints: /api/frontier, /api/findings, /api/events, /api/queue, /api/tool");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!(
                "{} failed to bind to {addr}: {e}",
                crate::cli_style::err_prefix()
            );
            std::process::exit(1);
        });
    axum::serve(listener, app).await.unwrap();
}

pub fn check_tools(source: ProjectSource) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let (frontier, _project_infos) = load_projects(&source);
    let first_id = frontier.findings.first().map(|finding| finding.id.clone());
    let mut checks = vec![
        check_tool_result("frontier_stats", tool_frontier_stats(&frontier), started),
        check_tool_result(
            "search_findings",
            tool_search_findings(&json!({"query": "amyloid", "limit": 3}), &frontier),
            started,
        ),
        check_tool_result("list_gaps", tool_list_gaps(&frontier), started),
        check_tool_result(
            "list_contradictions",
            tool_list_contradictions(&frontier),
            started,
        ),
        check_tool_result(
            "find_bridges",
            tool_find_bridges(&json!({"limit": 5, "min_categories": 2}), &frontier),
            started,
        ),
        check_tool_result(
            "apply_observer",
            tool_apply_observer(&json!({"policy": "academic", "limit": 5}), &frontier),
            started,
        ),
        check_tool_result(
            "propagate_retraction",
            tool_propagate_retraction(&json!({"finding_id": "vf_missing"}), &frontier),
            started,
        ),
    ];
    if let Some(id) = first_id {
        checks.push(check_tool_result(
            "get_finding",
            tool_get_finding(&json!({"id": id}), &frontier),
            started,
        ));
        checks.push(check_tool_result(
            "get_finding_history",
            tool_get_finding_history(&json!({"id": id}), &frontier),
            started,
        ));
        checks.push(check_tool_result(
            "trace_evidence_chain",
            tool_trace_evidence_chain(&json!({"finding_id": id}), &frontier),
            started,
        ));
    }
    let failures = checks
        .iter()
        .filter(|check| check.get("ok").and_then(Value::as_bool) != Some(true))
        .filter_map(|check| {
            check
                .get("tool")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let checked_tools = checks
        .iter()
        .filter_map(|check| check.get("tool").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let registered_tools = tool_registry::all_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    Ok(json!({
        "ok": failures.is_empty(),
        "command": "serve --check-tools",
        "schema": "vela.tool-check.v0",
        "frontier": {
            "name": frontier.project.name,
            "findings": frontier.stats.findings,
            "links": frontier.stats.links,
        },
        "summary": {
            "checks": checks.len(),
            "passed": checks.len().saturating_sub(failures.len()),
            "failed": failures.len(),
        },
        "tool_count": checked_tools.len(),
        "tools": checked_tools,
        "registered_tool_count": registered_tools.len(),
        "registered_tools": registered_tools,
        "checks": checks,
        "failures": failures,
    }))
}

#[derive(Clone)]
struct AppState {
    project: Arc<Mutex<Project>>,
    project_infos: Vec<ProjectInfo>,
    client: Client,
    /// Phase Q-w (v0.5): when serving a single frontier file, this is
    /// the path to write back to after a successful signed write. None
    /// when `--frontiers <dir>` is used; in that mode all writes are
    /// rejected.
    source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolResult {
    tool: String,
    ok: bool,
    data: Value,
    markdown: String,
    signals: Vec<signals::SignalItem>,
    caveats: Vec<String>,
    duration_ms: u128,
}

impl ToolResult {
    fn from_text(
        tool: &str,
        text: String,
        duration_ms: u128,
        is_error: bool,
        frontier: Option<&Project>,
    ) -> Self {
        let data = serde_json::from_str(&text).unwrap_or_else(|_| json!({"text": text}));
        let signal_items = frontier
            .map(|project| signals::analyze(project, &[]).signals)
            .unwrap_or_default();
        Self {
            tool: tool.to_string(),
            ok: !is_error,
            data,
            markdown: text,
            signals: signal_items,
            caveats: tool_registry::tool_caveats(tool),
            duration_ms,
        }
    }

    fn metadata(&self) -> Value {
        json!({
            "tool": self.tool,
            "ok": self.ok,
            "duration_ms": self.duration_ms,
            "signals": self.signals,
            "caveats": self.caveats,
            "definition": tool_registry::get_tool(&self.tool),
        })
    }

    fn to_json_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

async fn handle_tool_call(
    id: &Option<Value>,
    name: &str,
    args: &Value,
    frontier: &Arc<Mutex<Project>>,
    client: &Client,
    project_infos: &[ProjectInfo],
    source_path: Option<&Path>,
) -> Value {
    let started = std::time::Instant::now();
    let (result, snapshot) =
        execute_tool(name, args, frontier, client, project_infos, source_path).await;
    match result {
        Ok(text) => {
            let output = ToolResult::from_text(
                name,
                text,
                started.elapsed().as_millis(),
                false,
                snapshot.as_ref(),
            );
            json_rpc_result(
                id,
                json!({
                    "content": [{"type": "text", "text": output.to_json_text()}],
                    "isError": false,
                    "_meta": output.metadata()
                }),
            )
        }
        Err(error) => {
            let output = ToolResult::from_text(
                name,
                error,
                started.elapsed().as_millis(),
                true,
                snapshot.as_ref(),
            );
            json_rpc_result(
                id,
                json!({
                    "content": [{"type": "text", "text": output.to_json_text()}],
                    "isError": true,
                    "_meta": output.metadata()
                }),
            )
        }
    }
}

async fn execute_tool(
    name: &str,
    args: &Value,
    frontier: &Arc<Mutex<Project>>,
    client: &Client,
    _project_infos: &[ProjectInfo],
    source_path: Option<&Path>,
) -> (Result<String, String>, Option<Project>) {
    match name {
        "search_findings" => {
            let project = frontier.lock().await;
            (
                tool_search_findings(args, &project),
                Some(clone_project(&project)),
            )
        }
        "get_finding" => {
            let project = frontier.lock().await;
            (
                tool_get_finding(args, &project),
                Some(clone_project(&project)),
            )
        }
        "get_finding_history" => {
            let project = frontier.lock().await;
            (
                tool_get_finding_history(args, &project),
                Some(clone_project(&project)),
            )
        }
        "list_gaps" => {
            let project = frontier.lock().await;
            (tool_list_gaps(&project), Some(clone_project(&project)))
        }
        "list_contradictions" => {
            let project = frontier.lock().await;
            (
                tool_list_contradictions(&project),
                Some(clone_project(&project)),
            )
        }
        "frontier_stats" => {
            let project = frontier.lock().await;
            (tool_frontier_stats(&project), Some(clone_project(&project)))
        }
        "find_bridges" => {
            let project = frontier.lock().await;
            (
                tool_find_bridges(args, &project),
                Some(clone_project(&project)),
            )
        }
        "propagate_retraction" => {
            let project = frontier.lock().await;
            (
                tool_propagate_retraction(args, &project),
                Some(clone_project(&project)),
            )
        }
        "apply_observer" => {
            let project = frontier.lock().await;
            (
                tool_apply_observer(args, &project),
                Some(clone_project(&project)),
            )
        }
        "trace_evidence_chain" => {
            let project = frontier.lock().await;
            (
                tool_trace_evidence_chain(args, &project),
                Some(clone_project(&project)),
            )
        }
        "check_pubmed" => (tool_check_pubmed(args, client).await, None),
        "list_events_since" => {
            let project = frontier.lock().await;
            (
                tool_list_events_since(args, &project),
                Some(clone_project(&project)),
            )
        }
        // Phase Q-w (v0.5): write surface — propose-* and decision tools.
        // Each requires a registered actor and a verifying signature
        // over a canonical preimage. Idempotent under Phase P.
        "propose_review" => {
            let result = write_tool_propose(
                args,
                frontier,
                source_path,
                "finding.review",
                |args| {
                    let status = args
                        .get("status")
                        .and_then(Value::as_str)
                        .ok_or("propose_review requires `status`")?;
                    if !matches!(
                        status,
                        "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
                    ) {
                        return Err(format!("invalid review status '{status}'"));
                    }
                    Ok(json!({"status": status}))
                },
                false,
            )
            .await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        "propose_note" => {
            let result = write_tool_propose(
                args,
                frontier,
                source_path,
                "finding.note",
                |args| build_note_payload(args, "propose_note"),
                false,
            )
            .await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        // Phase α (v0.6): one-call propose-and-apply for `finding.note`.
        // Requires the actor to have `tier="auto-notes"` registered; the
        // `write_tool_propose` helper rejects with a clear error otherwise.
        // Doctrine: tiers permit review-context kinds only; never state-
        // changing kinds (no `propose_and_apply_review`/`_retract`/`_revise`).
        "propose_and_apply_note" => {
            let result = write_tool_propose(
                args,
                frontier,
                source_path,
                "finding.note",
                |args| build_note_payload(args, "propose_and_apply_note"),
                true,
            )
            .await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        "propose_revise_confidence" => {
            let result = write_tool_propose(
                args,
                frontier,
                source_path,
                "finding.confidence_revise",
                |args| {
                    let new_score = args
                        .get("new_score")
                        .and_then(Value::as_f64)
                        .ok_or("propose_revise_confidence requires `new_score`")?;
                    if !(0.0..=1.0).contains(&new_score) {
                        return Err(format!("new_score {new_score} out of [0.0, 1.0]"));
                    }
                    Ok(json!({"new_score": new_score}))
                },
                false,
            )
            .await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        "propose_retract" => {
            let result = write_tool_propose(
                args,
                frontier,
                source_path,
                "finding.retract",
                |_args| Ok(json!({})),
                false,
            )
            .await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        "accept_proposal" => {
            let result = write_tool_decision(args, frontier, source_path, "accept").await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        "reject_proposal" => {
            let result = write_tool_decision(args, frontier, source_path, "reject").await;
            let snapshot = Some(clone_project(&*frontier.lock().await));
            (result, snapshot)
        }
        _ => (Err(format!("Unknown tool: {name}")), None),
    }
}

/// Phase β (v0.6): build the `finding.note` proposal payload from
/// caller args. Accepts the required `text` plus an optional structured
/// `provenance` object whose at-least-one-identifier rule is enforced
/// here at the API boundary, so the same validation runs whether the
/// caller is `propose_note` or `propose_and_apply_note`.
fn build_note_payload(args: &Value, tool_name: &str) -> Result<Value, String> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{tool_name} requires `text`"))?;
    if text.trim().is_empty() {
        return Err("text must be non-empty".to_string());
    }
    let mut payload = json!({"text": text});
    if let Some(prov) = args.get("provenance") {
        let prov_obj = prov
            .as_object()
            .ok_or("provenance must be a JSON object when present")?;
        let has_id = ["doi", "pmid", "title"].iter().any(|k| {
            prov_obj
                .get(*k)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        });
        if !has_id {
            return Err("provenance must include at least one of doi/pmid/title".to_string());
        }
        payload["provenance"] = prov.clone();
    }
    Ok(payload)
}

/// Phase Q-w (v0.5) + Phase α (v0.6): shared body for the propose-* write
/// tools. `payload_builder` extracts the kind-specific payload from `args`.
/// `apply_if_tier_permits` (Phase α): when `true`, the function looks up the
/// actor's `tier`, requires `sign::actor_can_auto_apply(actor, kind)` to
/// return `true`, and applies the proposal in one canonical event;
/// otherwise rejects with a clear error. When `false` (the v0.5 default),
/// the proposal stays in `pending_review` regardless of tier.
async fn write_tool_propose<F>(
    args: &Value,
    frontier: &Arc<Mutex<Project>>,
    source_path: Option<&Path>,
    kind: &str,
    payload_builder: F,
    apply_if_tier_permits: bool,
) -> Result<String, String>
where
    F: Fn(&Value) -> Result<Value, String>,
{
    let path = source_path.ok_or_else(|| {
        "Write tools require a single-file frontier (--frontier <PATH>); rejected in --frontiers <DIR> mode".to_string()
    })?;
    let actor_id = args
        .get("actor_id")
        .and_then(Value::as_str)
        .ok_or("write tool requires `actor_id`")?;
    let target_finding_id = args
        .get("target_finding_id")
        .and_then(Value::as_str)
        .ok_or("write tool requires `target_finding_id`")?;
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .ok_or("write tool requires `reason`")?;
    let signature_hex = args
        .get("signature")
        .and_then(Value::as_str)
        .ok_or("write tool requires `signature` (Ed25519 over canonical proposal preimage)")?;
    let created_at = args
        .get("created_at")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let payload = payload_builder(args)?;

    // Look up the actor's registered pubkey AND tier (Phase α).
    let (pubkey, tier_permits_apply) = {
        let project = frontier.lock().await;
        let actor = project
            .actors
            .iter()
            .find(|actor| actor.id == actor_id)
            .ok_or_else(|| {
                format!(
                    "actor '{actor_id}' is not registered in this frontier; register via `vela actor add` before writing"
                )
            })?;
        let tier_permits = crate::sign::actor_can_auto_apply(actor, kind);
        // If the caller asked to auto-apply but the actor's tier doesn't
        // permit this kind, reject before signature verification — the
        // capability gate is independent of signing correctness.
        if apply_if_tier_permits && !tier_permits {
            let tier_label = actor.tier.as_deref().unwrap_or("none");
            return Err(format!(
                "actor '{actor_id}' tier '{tier_label}' does not permit auto-apply for {kind}"
            ));
        }
        (actor.public_key.clone(), tier_permits)
    };

    // Build the proposal exactly as the CLI would, then verify the signature
    // against the registered pubkey before persisting.
    let mut proposal = crate::proposals::new_proposal(
        kind,
        crate::events::StateTarget {
            r#type: "finding".to_string(),
            id: target_finding_id.to_string(),
        },
        actor_id,
        "human",
        reason,
        payload,
        Vec::new(),
        Vec::new(),
    );
    proposal.created_at = created_at;
    proposal.id = crate::proposals::proposal_id(&proposal);

    let valid = crate::sign::verify_proposal_signature(&proposal, signature_hex, &pubkey)?;
    if !valid {
        return Err(format!(
            "Signature does not verify for actor '{actor_id}' on this proposal"
        ));
    }

    // Persist. Phase α: apply iff caller asked AND tier permits (already
    // enforced above). Phase P guarantees `create_or_apply` is idempotent
    // either way.
    let apply = apply_if_tier_permits && tier_permits_apply;
    let result = crate::proposals::create_or_apply(path, proposal, apply)
        .map_err(|e| format!("create_or_apply failed: {e}"))?;

    // Refresh the in-memory state from disk so subsequent reads see the write.
    let fresh =
        crate::repo::load_from_path(path).map_err(|e| format!("reload after write failed: {e}"))?;
    let mut project = frontier.lock().await;
    *project = fresh;

    serde_json::to_string(&json!({
        "proposal_id": result.proposal_id,
        "finding_id": result.finding_id,
        "status": result.status,
        "applied_event_id": result.applied_event_id,
    }))
    .map_err(|e| format!("serialize write result: {e}"))
}

/// Phase Q-w (v0.5): shared body for `accept_proposal` and `reject_proposal`.
/// The signing preimage is `{action, proposal_id, reviewer_id, reason, timestamp}`
/// canonicalized; the reviewer must be a registered actor.
async fn write_tool_decision(
    args: &Value,
    frontier: &Arc<Mutex<Project>>,
    source_path: Option<&Path>,
    action: &str,
) -> Result<String, String> {
    let path = source_path.ok_or_else(|| {
        "Write tools require a single-file frontier (--frontier <PATH>); rejected in --frontiers <DIR> mode".to_string()
    })?;
    let proposal_id = args
        .get("proposal_id")
        .and_then(Value::as_str)
        .ok_or("decision tool requires `proposal_id`")?;
    let reviewer_id = args
        .get("reviewer_id")
        .and_then(Value::as_str)
        .ok_or("decision tool requires `reviewer_id`")?;
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .ok_or("decision tool requires `reason`")?;
    let signature_hex = args
        .get("signature")
        .and_then(Value::as_str)
        .ok_or("decision tool requires `signature`")?;
    let timestamp = args
        .get("timestamp")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    // Canonical preimage for the decision action.
    let preimage = json!({
        "action": action,
        "proposal_id": proposal_id,
        "reviewer_id": reviewer_id,
        "reason": reason,
        "timestamp": timestamp,
    });
    let signing_bytes = crate::canonical::to_canonical_bytes(&preimage)?;

    // Look up the reviewer's registered pubkey.
    let pubkey = {
        let project = frontier.lock().await;
        project
            .actors
            .iter()
            .find(|actor| actor.id == reviewer_id)
            .map(|actor| actor.public_key.clone())
            .ok_or_else(|| format!("reviewer '{reviewer_id}' is not registered"))?
    };

    let valid = crate::sign::verify_action_signature(&signing_bytes, signature_hex, &pubkey)?;
    if !valid {
        return Err(format!(
            "Signature does not verify for reviewer '{reviewer_id}' on {action} of {proposal_id}"
        ));
    }

    let outcome = match action {
        "accept" => {
            let event_id = crate::proposals::accept_at_path(path, proposal_id, reviewer_id, reason)
                .map_err(|e| format!("accept failed: {e}"))?;
            json!({
                "proposal_id": proposal_id,
                "applied_event_id": event_id,
                "status": "applied",
            })
        }
        "reject" => {
            crate::proposals::reject_at_path(path, proposal_id, reviewer_id, reason)
                .map_err(|e| format!("reject failed: {e}"))?;
            json!({
                "proposal_id": proposal_id,
                "applied_event_id": Value::Null,
                "status": "rejected",
            })
        }
        other => return Err(format!("unsupported decision action '{other}'")),
    };

    // Refresh in-memory state.
    let fresh =
        crate::repo::load_from_path(path).map_err(|e| format!("reload after write failed: {e}"))?;
    let mut project = frontier.lock().await;
    *project = fresh;

    serde_json::to_string(&outcome).map_err(|e| format!("serialize decision: {e}"))
}

/// Phase Q-r (v0.5): MCP-tool form of the cursor-paginated event read.
/// Mirrors `GET /api/events`. Same cursor semantics: events strictly
/// after `cursor` (a `vev_…` id), or from genesis if cursor is omitted.
fn tool_list_events_since(args: &Value, project: &Project) -> Result<String, String> {
    let cursor = args.get("cursor").and_then(Value::as_str);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(100usize, |n| (n as usize).min(500));
    let start_idx: usize = match cursor {
        None => 0,
        Some(c) => match project.events.iter().position(|event| event.id == c) {
            Some(idx) => idx + 1,
            None => {
                return Err(format!(
                    "cursor '{c}' not found in event log; client is out of sync"
                ));
            }
        },
    };
    let end_idx = (start_idx + limit).min(project.events.len());
    let slice = &project.events[start_idx..end_idx];
    let next_cursor = if end_idx < project.events.len() {
        slice.last().map(|event| event.id.clone())
    } else {
        None
    };
    let payload = json!({
        "events": slice,
        "count": slice.len(),
        "next_cursor": next_cursor,
        "log_total": project.events.len(),
    });
    serde_json::to_string(&payload).map_err(|e| format!("serialize list_events_since: {e}"))
}

fn check_tool_result(
    name: &str,
    result: Result<String, String>,
    started: std::time::Instant,
) -> Value {
    let output = ToolResult::from_text(
        name,
        result.unwrap_or_else(|e| e),
        started.elapsed().as_millis(),
        false,
        None,
    );
    let has_data = !output.data.is_null();
    let has_markdown = !output.markdown.trim().is_empty();
    let has_signals = true;
    let has_caveats = true;
    json!({
        "tool": name,
        "ok": has_data && has_markdown && has_signals && has_caveats,
        "data": output.data,
        "markdown": output.markdown,
        "has_data": has_data,
        "has_markdown": has_markdown,
        "has_signals": has_signals,
        "has_caveats": has_caveats,
        "signals": output.signals,
        "caveats": output.caveats,
        "duration_ms": output.duration_ms,
    })
}

/// Phase Q-r (v0.5): cursor-paginated read over the canonical event log.
///
/// Query params:
///   - `since` (optional): a `vev_…` event id; events strictly after this id
///     are returned. Omit to start from the genesis event.
///   - `limit` (optional, default 100, max 500): cap the response size.
///
/// Returns `{events: [...], next_cursor: "vev_..." | null, count: usize}`.
/// `next_cursor` is null when the response includes the tail of the log.
///
/// 400 if `since` is provided but does not exist in the log (the client is
/// out of sync with the log it's reading; better to fail loudly than to
/// silently skip).
async fn http_events(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let project = state.project.lock().await;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100)
        .min(500);
    let start_idx: usize = match params.get("since") {
        None => 0,
        Some(cursor) => match project.events.iter().position(|event| &event.id == cursor) {
            Some(idx) => idx + 1,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": format!(
                            "cursor '{cursor}' not found in event log; client is out of sync"
                        ),
                    })),
                );
            }
        },
    };
    // v0.17: server-side `?kind=` and `?target=` filters. Agents watching
    // for specific event kinds (e.g. polling for new finding.superseded
    // events) shouldn't need to fetch the whole log to locate one match.
    // Filters apply BEFORE the limit/cursor so pagination works on the
    // filtered view.
    let kind_filter = params.get("kind").map(String::as_str);
    let target_filter = params.get("target").map(String::as_str);
    let filtered: Vec<&crate::events::StateEvent> = project
        .events
        .iter()
        .skip(start_idx)
        .filter(|e| kind_filter.is_none_or(|k| e.kind == k))
        .filter(|e| target_filter.is_none_or(|t| e.target.id == t))
        .collect();
    let total_filtered = filtered.len();
    let take_n = limit.min(total_filtered);
    let slice: Vec<&crate::events::StateEvent> = filtered.into_iter().take(take_n).collect();
    let next_cursor = if take_n < total_filtered {
        slice.last().map(|event| event.id.clone())
    } else {
        None
    };
    (
        StatusCode::OK,
        Json(json!({
            "events": slice,
            "count": slice.len(),
            "next_cursor": next_cursor,
            "log_total": project.events.len(),
            "filtered_total": total_filtered,
        })),
    )
}

/// Phase R (v0.5): append a draft Workbench action to the local queue.
/// The browser POSTs `{kind, args}` (no signature, no actor key — the
/// browser is identity-blind under the v0.5 doctrine). The Workbench
/// host process appends to the configured queue file; `vela queue sign`
/// is the only path that produces a signed write.
///
/// Body:
///   `{"kind": "<tool_name>", "args": { ... }}`
///
/// Returns `{ok: true, queued_at: "<rfc3339>"}` on success.
async fn http_queue_append(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let path = match &state.source_path {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"error": "Workbench queue requires a single-file frontier (--frontier <PATH>)"}),
                ),
            );
        }
    };
    let kind = match body.get("kind").and_then(Value::as_str) {
        Some(k) => k.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "POST /api/queue requires `kind`"})),
            );
        }
    };
    let valid_kinds = [
        "propose_review",
        "propose_note",
        "propose_revise_confidence",
        "propose_retract",
        "accept_proposal",
        "reject_proposal",
    ];
    if !valid_kinds.contains(&kind.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("unsupported queue kind '{kind}'")})),
        );
    }
    let args = body.get("args").cloned().unwrap_or(Value::Null);
    let queued_at = chrono::Utc::now().to_rfc3339();
    let action = crate::queue::QueuedAction {
        kind,
        frontier: path,
        args,
        queued_at: queued_at.clone(),
    };
    let queue_path = crate::queue::default_queue_path();
    if let Err(error) = crate::queue::append(&queue_path, action) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("append to queue: {error}")})),
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "queue_file": queue_path.display().to_string(),
            "queued_at": queued_at,
            "next_step": "run `vela queue sign` to apply queued drafts",
        })),
    )
}

/// Phase R (v0.5): resolve the location of the `web/` directory for the
/// Workbench static assets. Tries common paths: workspace root relative
/// to the running binary, then current working directory, then a
/// `VELA_WEB_DIR` env override.
fn workbench_web_dir() -> PathBuf {
    if let Ok(path) = std::env::var("VELA_WEB_DIR") {
        return PathBuf::from(path);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        cwd.join("web"),
        PathBuf::from("./web"),
        PathBuf::from("web"),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }
    cwd.join("web")
}

async fn http_frontier(State(state): State<AppState>) -> Json<Value> {
    let project = state.project.lock().await;
    Json(
        serde_json::to_value(&*project)
            .unwrap_or_else(|_| json!({"error": "serialization failed"})),
    )
}

async fn http_findings(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let project = state.project.lock().await;
    let args = json!({
        "query": params.get("query"),
        "entity": params.get("entity"),
        "entity_type": params.get("entity_type"),
        "assertion_type": params.get("type"),
        "limit": params.get("limit").and_then(|v| v.parse::<u64>().ok()).unwrap_or(50),
    });
    match tool_search_findings(&args, &project) {
        Ok(text) => Json(json!({"result": text})),
        Err(error) => Json(json!({"error": error})),
    }
}

async fn http_finding_by_id(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> (StatusCode, Json<Value>) {
    let project = state.project.lock().await;
    match project
        .findings
        .iter()
        .find(|finding| finding.id == id || finding.id.starts_with(&id))
    {
        Some(finding) => (
            StatusCode::OK,
            Json(serde_json::to_value(finding).unwrap_or_default()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Finding not found: {id}")})),
        ),
    }
}

async fn http_contradictions(State(state): State<AppState>) -> Json<Value> {
    let project = state.project.lock().await;
    Json(
        serde_json::from_str(&tool_list_contradictions(&project).unwrap_or_default())
            .unwrap_or_else(
                |_| json!({"result": tool_list_contradictions(&project).unwrap_or_default()}),
            ),
    )
}

async fn http_observer(
    State(state): State<AppState>,
    axum::extract::Path(policy): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let project = state.project.lock().await;
    let args = json!({
        "policy": policy,
        "limit": params.get("limit").and_then(|v| v.parse::<u64>().ok()).unwrap_or(20),
    });
    match tool_apply_observer(&args, &project) {
        Ok(text) => Json(serde_json::from_str(&text).unwrap_or_else(|_| json!({"result": text}))),
        Err(error) => Json(json!({"error": error})),
    }
}

async fn http_propagate(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    let project = state.project.lock().await;
    let args = json!({"finding_id": id});
    match tool_propagate_retraction(&args, &project) {
        Ok(text) => Json(serde_json::from_str(&text).unwrap_or_else(|_| json!({"result": text}))),
        Err(error) => Json(json!({"error": error})),
    }
}

async fn http_bridges(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let project = state.project.lock().await;
    let args = json!({
        "min_categories": params.get("min_categories").and_then(|v| v.parse::<u64>().ok()).unwrap_or(2),
        "limit": params.get("limit").and_then(|v| v.parse::<u64>().ok()).unwrap_or(15),
    });
    match tool_find_bridges(&args, &project) {
        Ok(text) => Json(serde_json::from_str(&text).unwrap_or_else(|_| json!({"result": text}))),
        Err(error) => Json(json!({"error": error})),
    }
}

async fn http_stats(State(state): State<AppState>) -> Json<Value> {
    let project = state.project.lock().await;
    Json(json!({
        "frontier": {
            "name": project.project.name,
            "compiled_at": project.project.compiled_at,
            "compiler": project.project.compiler,
        },
        "stats": project.stats,
        "signals": signals::analyze(&project, &[]).signals,
    }))
}

async fn http_frontiers(State(state): State<AppState>) -> Json<Value> {
    Json(
        serde_json::from_str(&frontier_index_json(&state.project_infos).unwrap_or_default())
            .unwrap_or_else(|_| json!({"frontier_count": 0, "frontiers": []})),
    )
}

async fn http_pubmed(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let args = json!({"query": params.get("query").cloned().unwrap_or_default()});
    match tool_check_pubmed(&args, &state.client).await {
        Ok(text) => Json(serde_json::from_str(&text).unwrap_or_else(|_| json!({"result": text}))),
        Err(error) => Json(json!({"error": error})),
    }
}

async fn http_tools_list() -> Json<Value> {
    Json(tool_registry::mcp_tools_json())
}

async fn http_tool_call(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let name = body["name"].as_str().unwrap_or_default();
    let args = &body["arguments"];
    let started = std::time::Instant::now();
    let (result, snapshot) = execute_tool(
        name,
        args,
        &state.project,
        &state.client,
        &state.project_infos,
        state.source_path.as_deref(),
    )
    .await;
    match result {
        Ok(text) => {
            let output = ToolResult::from_text(
                name,
                text,
                started.elapsed().as_millis(),
                false,
                snapshot.as_ref(),
            );
            (
                StatusCode::OK,
                Json(json!({
                    "result": output.markdown,
                    "tool": output.tool,
                    "ok": output.ok,
                    "data": output.data,
                    "markdown": output.markdown,
                    "signals": output.signals,
                    "caveats": output.caveats,
                    "duration_ms": output.duration_ms,
                    "metadata": output.metadata(),
                })),
            )
        }
        Err(error) => {
            let output = ToolResult::from_text(
                name,
                error,
                started.elapsed().as_millis(),
                true,
                snapshot.as_ref(),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": output.markdown,
                    "tool": output.tool,
                    "ok": output.ok,
                    "data": output.data,
                    "markdown": output.markdown,
                    "signals": output.signals,
                    "caveats": output.caveats,
                    "duration_ms": output.duration_ms,
                    "metadata": output.metadata(),
                })),
            )
        }
    }
}

fn tool_search_findings(args: &Value, frontier: &Project) -> Result<String, String> {
    let query = args["query"].as_str().map(str::to_lowercase);
    let entity = args["entity"].as_str().map(str::to_lowercase);
    let entity_type = args["entity_type"].as_str().map(str::to_lowercase);
    let assertion_type = args["assertion_type"].as_str().map(str::to_lowercase);
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let results = frontier
        .findings
        .iter()
        .filter(|finding| {
            query.as_ref().is_none_or(|q| {
                finding.assertion.text.to_lowercase().contains(q)
                    || finding.conditions.text.to_lowercase().contains(q)
                    || finding
                        .assertion
                        .entities
                        .iter()
                        .any(|e| e.name.to_lowercase().contains(q))
            }) && entity.as_ref().is_none_or(|needle| {
                finding
                    .assertion
                    .entities
                    .iter()
                    .any(|e| e.name.to_lowercase().contains(needle))
            }) && entity_type.as_ref().is_none_or(|needle| {
                finding
                    .assertion
                    .entities
                    .iter()
                    .any(|e| e.entity_type.to_lowercase() == *needle)
            }) && assertion_type
                .as_ref()
                .is_none_or(|needle| finding.assertion.assertion_type.to_lowercase() == *needle)
        })
        .take(limit)
        .collect::<Vec<_>>();

    if results.is_empty() {
        return Ok("No findings matched the search criteria.".to_string());
    }
    let mut out = format!("{} findings matched:\n\n", results.len());
    for finding in results {
        let entities = finding
            .assertion
            .entities
            .iter()
            .map(|e| format!("{} ({})", e.name, e.entity_type))
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "**{}** [conf: {}, type: {}]\n{}\nEntities: {}\nReplicated: {} | Gap: {} | Contested: {}\nSource: {} ({})\n\n",
            finding.id,
            finding.confidence.score,
            finding.assertion.assertion_type,
            finding.assertion.text,
            entities.join(", "),
            finding.evidence.replicated,
            finding.flags.gap,
            finding.flags.contested,
            finding.provenance.title,
            finding.provenance.year.map(|y| y.to_string()).unwrap_or_else(|| "?".to_string()),
        ));
    }
    Ok(out)
}

fn tool_get_finding(args: &Value, frontier: &Project) -> Result<String, String> {
    let id = args["id"].as_str().ok_or("Missing 'id' argument")?;
    let finding = frontier
        .findings
        .iter()
        .find(|finding| finding.id == id || finding.id.starts_with(id))
        .ok_or_else(|| format!("Finding '{id}' not found"))?;
    let mut context = state::finding_context(frontier, &finding.id)?;
    if let Value::Object(map) = &mut context {
        map.insert(
            "caveats".to_string(),
            json!([
            "Finding-local events are canonical state transitions; review_events are projection artifacts.",
            "Sources identify artifacts; evidence atoms identify source-grounded units that bear on the finding."
            ]),
        );
    }
    serde_json::to_string_pretty(&context).map_err(|e| format!("Serialization error: {e}"))
}

/// v0.17: chronological event log for one finding. The full canonical event
/// log filtered to events whose `target.id` matches the requested finding,
/// sorted ascending by timestamp. Useful for agents walking the supersedes
/// chain or auditing corrections.
fn tool_get_finding_history(args: &Value, frontier: &Project) -> Result<String, String> {
    let id = args["id"].as_str().ok_or("Missing 'id' argument")?;
    let mut events: Vec<&crate::events::StateEvent> = frontier
        .events
        .iter()
        .filter(|e| {
            e.target.r#type == "finding" && (e.target.id == id || e.target.id.starts_with(id))
        })
        .collect();
    events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    let payload = json!({
        "finding_id": id,
        "event_count": events.len(),
        "events": events,
        "caveats": [
            "Events are the canonical state-transition log; events without a 'finding' target are excluded.",
            "Use payload.new_finding_id on finding.superseded events to walk forward in the supersedes chain."
        ],
    });
    serde_json::to_string_pretty(&payload).map_err(|e| format!("Serialization error: {e}"))
}

fn tool_list_gaps(frontier: &Project) -> Result<String, String> {
    let gaps = frontier
        .findings
        .iter()
        .filter(|finding| finding.flags.gap)
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        return Ok("No gap-flagged findings in this frontier.".to_string());
    }
    let mut out = format!(
        "{} candidate gap review leads:\nTreat these as navigation signals, not confirmed experiment targets.\n\n",
        gaps.len()
    );
    for finding in gaps {
        out.push_str(&format!(
            "**{}** [conf: {}]\n{}\nConditions: {}\n\n",
            finding.id, finding.confidence.score, finding.assertion.text, finding.conditions.text
        ));
    }
    Ok(out)
}

fn tool_list_contradictions(frontier: &Project) -> Result<String, String> {
    let lookup = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect::<HashMap<_, _>>();
    let mut contradictions = Vec::new();
    for finding in &frontier.findings {
        for link in &finding.links {
            if matches!(link.link_type.as_str(), "contradicts" | "disputes") {
                let target = lookup
                    .get(link.target.as_str())
                    .map(|f| f.assertion.text.as_str())
                    .unwrap_or("(unknown target)");
                contradictions.push(format!(
                    "**{}** {} **{}**\n  {} --[{}]--> {}\n  Note: {}\n",
                    finding.id,
                    link.link_type,
                    link.target,
                    trunc(&finding.assertion.text, 80),
                    link.link_type,
                    trunc(target, 80),
                    link.note,
                ));
            }
        }
    }
    if contradictions.is_empty() {
        return Ok("No candidate contradiction links in this frontier.".to_string());
    }
    Ok(format!(
        "{} candidate contradiction links:\n\n{}",
        contradictions.len(),
        contradictions.join("\n")
    ))
}

fn tool_frontier_stats(frontier: &Project) -> Result<String, String> {
    serde_json::to_string_pretty(&json!({
        "frontier": {
            "name": frontier.project.name,
            "description": frontier.project.description,
            "compiled_at": frontier.project.compiled_at,
            "compiler": frontier.project.compiler,
            "papers_processed": frontier.project.papers_processed,
            "errors": frontier.project.errors,
        },
        "stats": frontier.stats,
        "source_registry": sources::source_summary(frontier),
        "evidence_atoms": sources::evidence_summary(frontier),
        "conditions": sources::condition_summary(frontier),
        "proposals": crate::proposals::summary(frontier),
        "proof_state": frontier.proof_state,
        "events": {
            "count": frontier.events.len(),
            "summary": events::summarize(frontier),
            "replay": events::replay_report(frontier),
        },
        "signals": signals::analyze(frontier, &[]).signals,
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

fn tool_find_bridges(args: &Value, frontier: &Project) -> Result<String, String> {
    let min_categories = args["min_categories"].as_u64().unwrap_or(2) as usize;
    let limit = args["limit"].as_u64().unwrap_or(15) as usize;
    let mut entity_categories = HashMap::<String, HashSet<String>>::new();
    let mut entity_counts = HashMap::<String, usize>::new();
    for finding in &frontier.findings {
        for entity in &finding.assertion.entities {
            let key = entity.name.to_lowercase();
            entity_categories
                .entry(key.clone())
                .or_default()
                .insert(finding.assertion.assertion_type.clone());
            *entity_counts.entry(key).or_default() += 1;
        }
    }
    let mut bridges = entity_categories
        .iter()
        .filter(|(name, categories)| {
            categories.len() >= min_categories && !bridge::is_obvious(name)
        })
        .map(|(name, categories)| {
            json!({
                "entity": name,
                "categories": categories.iter().cloned().collect::<Vec<_>>(),
                "category_count": categories.len(),
                "finding_count": entity_counts.get(name).copied().unwrap_or(0),
            })
        })
        .collect::<Vec<_>>();
    bridges.sort_by(|a, b| {
        b["category_count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["category_count"].as_u64().unwrap_or(0))
    });
    bridges.truncate(limit);
    serde_json::to_string_pretty(&json!({"count": bridges.len(), "bridges": bridges}))
        .map_err(|e| format!("Serialization error: {e}"))
}

fn tool_propagate_retraction(args: &Value, frontier: &Project) -> Result<String, String> {
    let id = args["finding_id"]
        .as_str()
        .ok_or("Missing 'finding_id' argument")?;
    let target = frontier
        .findings
        .iter()
        .find(|finding| finding.id == id || finding.id.starts_with(id))
        .ok_or_else(|| format!("Finding '{id}' not found"))?;

    // v0.49.3: O(1) reverse-dep lookup via the denormalized index
    // instead of the prior O(N×L) scan over every finding × every
    // link. The index is built once per request — at this corridor's
    // size it costs microseconds; at 100K findings it stays under a
    // second. Filter on link_type after the lookup so "supports" /
    // "depends" semantics are preserved.
    let reverse_idx = frontier.build_reverse_dep_index();
    let dependent_ids = reverse_idx.dependents_of(&target.id);
    let id_to_finding: std::collections::HashMap<&str, &crate::bundle::FindingBundle> =
        frontier.findings.iter().map(|f| (f.id.as_str(), f)).collect();

    let mut affected = Vec::new();
    for dep_id in dependent_ids {
        let Some(dependent) = id_to_finding.get(dep_id.as_str()) else {
            continue;
        };
        for link in &dependent.links {
            if matches!(link.link_type.as_str(), "supports" | "depends")
                && link.target == target.id
            {
                affected.push(json!({
                    "id": dependent.id,
                    "assertion": trunc(&dependent.assertion.text, 100),
                    "link_type": link.link_type,
                }));
            }
        }
    }
    serde_json::to_string_pretty(&json!({
        "retracted": {"id": target.id, "assertion": trunc(&target.assertion.text, 120)},
        "directly_affected": affected.len(),
        "affected_findings": affected,
        "caveat": "Retraction impact is simulated over declared dependency links.",
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

fn tool_apply_observer(args: &Value, frontier: &Project) -> Result<String, String> {
    let policy_name = args["policy"].as_str().ok_or("Missing 'policy' argument")?;
    let limit = args["limit"].as_u64().unwrap_or(15) as usize;
    let policy = observer::policy_by_name(policy_name).unwrap_or_else(observer::academic);
    let view = observer::observe(&frontier.findings, &frontier.replications, &policy);
    let top = view
        .findings
        .iter()
        .take(limit)
        .map(|scored| {
            let finding = frontier
                .findings
                .iter()
                .find(|finding| finding.id == scored.finding_id);
            json!({
                "id": scored.finding_id,
                "original_confidence": scored.original_confidence,
                "observer_score": scored.observer_score,
                "rank": scored.rank,
                "assertion": finding.map(|f| trunc(&f.assertion.text, 100)).unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "policy": policy_name,
        "shown": top.len(),
        "hidden": view.hidden,
        "top_findings": top,
        "caveat": "Observer output is policy-weighted reranking, not definitive disagreement.",
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

async fn tool_check_pubmed(args: &Value, client: &Client) -> Result<String, String> {
    let query = args["query"].as_str().ok_or("Missing 'query' argument")?;
    let count = bridge::check_novelty(client, query).await?;
    serde_json::to_string_pretty(&json!({
        "query": query,
        "pubmed_results": count,
        "rough_prior_art_clear": count == 0,
        "caveat": "PubMed counts are rough prior-art signals, not proof of novelty.",
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

fn frontier_index_json(project_infos: &[ProjectInfo]) -> Result<String, String> {
    let frontiers = project_infos
        .iter()
        .map(|info| {
            json!({
                "name": info.name,
                "file": info.file,
                "findings": info.findings_count,
                "links": info.links_count,
                "papers": info.papers,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "frontier_count": frontiers.len(),
        "frontiers": frontiers,
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

fn tool_trace_evidence_chain(args: &Value, frontier: &Project) -> Result<String, String> {
    let id = args["finding_id"]
        .as_str()
        .ok_or("Missing 'finding_id' argument")?;
    let depth = args["depth"].as_u64().unwrap_or(2) as usize;
    let lookup = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect::<HashMap<_, _>>();
    let finding = lookup
        .get(id)
        .copied()
        .or_else(|| {
            frontier
                .findings
                .iter()
                .find(|finding| finding.id.starts_with(id))
        })
        .ok_or_else(|| format!("Finding '{id}' not found"))?;
    let links = finding
        .links
        .iter()
        .take(depth.saturating_mul(10).max(10))
        .map(|link| {
            let target = lookup.get(link.target.as_str());
            json!({
                "target": link.target,
                "type": link.link_type,
                "note": link.note,
                "target_assertion": target.map(|f| trunc(&f.assertion.text, 120)),
            })
        })
        .collect::<Vec<_>>();
    let evidence_span_count = finding.evidence.evidence_spans.len();
    let source_ref = finding
        .provenance
        .doi
        .as_deref()
        .or(finding.provenance.pmid.as_deref())
        .unwrap_or(&finding.provenance.title);
    let review_state = finding
        .provenance
        .review
        .as_ref()
        .map(|review| {
            if review.reviewed {
                "reviewed"
            } else {
                "pending_review"
            }
        })
        .unwrap_or("pending_review");
    let finding_events = events::events_for_finding(frontier, &finding.id);
    let linked_sources = sources::sources_for_finding(frontier, &finding.id);
    let linked_atoms = sources::evidence_atoms_for_finding(frontier, &finding.id);
    let linked_conditions = sources::condition_records_for_finding(frontier, &finding.id);
    let linked_proposals = crate::proposals::proposals_for_finding(frontier, &finding.id);
    serde_json::to_string_pretty(&json!({
        "finding": {"id": finding.id, "assertion": finding.assertion.text},
        "sources": linked_sources,
        "evidence_atoms": linked_atoms,
        "condition_records": linked_conditions,
        "proposals": linked_proposals,
        "source_to_state": [
            {"step": "source", "value": linked_sources, "fallback": source_ref},
            {"step": "evidence_atom", "value": linked_atoms},
            {"step": "condition_boundary", "value": linked_conditions},
            {"step": "proposal_lineage", "value": linked_proposals},
            {"step": "legacy_evidence", "value": {"type": finding.evidence.evidence_type, "spans": evidence_span_count, "method": finding.evidence.method}},
            {"step": "finding", "value": {"id": finding.id, "assertion_type": finding.assertion.assertion_type, "confidence": finding.confidence.score}},
            {"step": "event_history", "value": finding_events},
            {"step": "links", "value": {"declared": finding.links.len()}},
            {"step": "review_state", "value": review_state}
        ],
        "state_events": finding_events,
        "path_explanation": format!(
            "source -> evidence spans ({}) -> finding {} -> {} declared links -> {}",
            evidence_span_count,
            finding.id,
            finding.links.len(),
            review_state
        ),
        "depth": depth,
        "links": links,
        "caveat": "Evidence-chain strength is heuristic and depends on declared links.",
    }))
    .map_err(|e| format!("Serialization error: {e}"))
}

fn clone_project(project: &Project) -> Project {
    serde_json::from_value(serde_json::to_value(project).unwrap_or_default()).unwrap_or_else(|_| {
        project::assemble("unavailable", Vec::new(), 0, 1, "failed to clone frontier")
    })
}

fn json_rpc_result(id: &Option<Value>, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn json_rpc_error(id: &Option<Value>, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}
