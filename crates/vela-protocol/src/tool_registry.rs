//! Tool registry — tools defined as data, separate from execution.
//! Borrowed from Codex (MIT) tool-as-data pattern.

use crate::permission::PermissionLevel;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub permission_level: PermissionLevel,
    pub mutating: bool,
    pub caveats: Vec<String>,
}

/// All MCP tools registered in Vela
pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        tool(
            "frontier_stats",
            "Return frontier metadata and statistics: finding count, links, confidence distribution, gaps, categories, and review state.",
            json!({"type": "object", "properties": {}}),
            PermissionLevel::ReadOnly,
            false,
            vec![],
        ),
        tool(
            "search_findings",
            "Search findings by text content, entity name, entity type, or assertion type. Returns matching findings.",
            json!({"type": "object", "properties": {
                "query": {"type": "string"}, "entity": {"type": "string"},
                "entity_type": {"type": "string"}, "assertion_type": {"type": "string"},
                "limit": {"type": "integer"}
            }}),
            PermissionLevel::ReadOnly,
            false,
            vec![],
        ),
        tool(
            "get_finding",
            "Get a single finding by ID, including evidence, conditions, links, confidence, and provenance.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
            PermissionLevel::ReadOnly,
            false,
            vec![],
        ),
        tool(
            "get_finding_history",
            "v0.17: Return the chronological event log for one finding (asserted, reviewed, caveated, noted, confidence-revised, superseded, retracted). Use this to walk the supersedes chain, audit corrections, or detect that a target has been refined since you last linked to it.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Event order reflects timestamps as recorded; sort client-side if you need a different ordering.",
            ],
        ),
        tool(
            "list_gaps",
            "List findings flagged as candidate gap review leads.",
            json!({"type": "object", "properties": {}}),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Candidate gap rankings are review leads, not guaranteed underexplored areas or experiment targets.",
            ],
        ),
        tool(
            "list_contradictions",
            "List contradiction and dispute links between findings.",
            json!({"type": "object", "properties": {}}),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Automated contradiction links are candidates for review, not definitive disagreements.",
            ],
        ),
        tool(
            "find_bridges",
            "Find entities spanning multiple assertion categories, suggesting candidate cross-domain connections.",
            json!({"type": "object", "properties": {
                "min_categories": {"type": "integer"}, "limit": {"type": "integer"}
            }}),
            PermissionLevel::ReadOnly,
            false,
            vec!["Candidate bridges require review before being treated as domain knowledge."],
        ),
        tool(
            "check_pubmed",
            "Run a rough PubMed prior-art check for a hypothesis.",
            json!({"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}),
            PermissionLevel::ReadOnly,
            false,
            vec!["PubMed counts are rough prior-art signals, not proof of novelty."],
        ),
        tool(
            "apply_observer",
            "Rerank findings under an observer policy such as pharma, academic, regulatory, clinical, or exploration.",
            json!({"type": "object", "properties": {
                "policy": {"type": "string"}, "limit": {"type": "integer"}
            }, "required": ["policy"]}),
            PermissionLevel::ReadOnly,
            false,
            vec!["Observer policy output is a weighted view, not definitive disagreement."],
        ),
        tool(
            "propagate_retraction",
            "Simulate retraction cascade impact over declared dependency/support links.",
            json!({"type": "object", "properties": {"finding_id": {"type": "string"}}, "required": ["finding_id"]}),
            PermissionLevel::Dangerous,
            false,
            vec!["Retraction impact is simulated over declared links only."],
        ),
        tool(
            "trace_evidence_chain",
            "Trace evidence lineage for a finding, including support, dependency, contradiction, and chain strength.",
            json!({"type": "object", "properties": {
                "finding_id": {"type": "string"}, "depth": {"type": "integer"}
            }, "required": ["finding_id"]}),
            PermissionLevel::ReadOnly,
            false,
            vec!["Evidence-chain strength is heuristic and depends on declared links."],
        ),
        // Phase Q-r (v0.5): cursor-paginated read over the canonical
        // event log. Agent loops use this to learn when their proposals
        // were accepted, rejected, or had cascade events emitted on
        // their behalf. Public consumers use it to track frontier state
        // changes without re-reading the full log.
        tool(
            "list_events_since",
            "List canonical events from the event log strictly after `cursor` (a `vev_…` id), ordered chronologically. Returns events plus a `next_cursor` for further pagination, or null when the tail is reached. Omit `cursor` to start from the genesis event.",
            json!({"type": "object", "properties": {
                "cursor": {"type": "string"},
                "limit": {"type": "integer"}
            }}),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Cursor must reference an event currently in the log; out-of-sync clients should restart from the beginning.",
            ],
        ),
        // Phase Q-w (v0.5): write surface — propose-* and decision tools.
        // Each requires a registered actor and a verifying Ed25519 signature
        // over the canonical preimage. Idempotent under Phase P:
        // identical logical proposals produce the same `vpr_…` and a retry
        // returns the existing record without duplicating state.
        tool(
            "propose_review",
            "Propose a `finding.review` decision on a finding (status: accepted/approved/contested/needs_revision/rejected). Requires the actor's Ed25519 signature over the canonical proposal preimage. Idempotent: identical logical proposals return the same `vpr_…`.",
            json!({"type": "object", "properties": {
                "actor_id": {"type": "string"},
                "target_finding_id": {"type": "string"},
                "status": {"type": "string"},
                "reason": {"type": "string"},
                "created_at": {"type": "string"},
                "signature": {"type": "string"}
            }, "required": ["actor_id", "target_finding_id", "status", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "actor_id must be registered in `frontier.actors` via `vela actor add` before writes verify.",
            ],
        ),
        tool(
            "propose_note",
            "Propose attaching a `finding.note` annotation to a finding. Requires a registered actor and signature. Optional structured `provenance` (Phase β, v0.6): `{doi?, pmid?, title?, span?}` with at least one identifier. Stays `pending_review` until accepted.",
            json!({"type": "object", "properties": {
                "actor_id": {"type": "string"},
                "target_finding_id": {"type": "string"},
                "text": {"type": "string"},
                "reason": {"type": "string"},
                "created_at": {"type": "string"},
                "signature": {"type": "string"},
                "provenance": {
                    "type": "object",
                    "properties": {
                        "doi": {"type": "string"},
                        "pmid": {"type": "string"},
                        "title": {"type": "string"},
                        "span": {"type": "string"}
                    }
                }
            }, "required": ["actor_id", "target_finding_id", "text", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec!["Notes do not change finding state; they accrete review context."],
        ),
        // Phase α (v0.6): one-call propose-and-apply for `finding.note`,
        // gated on actor `tier="auto-notes"`. Halves the signing surface
        // for trusted bulk-note extractors. Identical signing preimage and
        // arguments as `propose_note`; idempotent under Phase P.
        tool(
            "propose_and_apply_note",
            "Propose AND apply a `finding.note` annotation in one signed call. Requires the actor to have `tier=\"auto-notes\"` registered (`vela actor add --tier auto-notes`). Optional structured `provenance` (Phase β). Idempotent: a retry with identical content returns the same `applied_event_id`.",
            json!({"type": "object", "properties": {
                "actor_id": {"type": "string"},
                "target_finding_id": {"type": "string"},
                "text": {"type": "string"},
                "reason": {"type": "string"},
                "created_at": {"type": "string"},
                "signature": {"type": "string"},
                "provenance": {
                    "type": "object",
                    "properties": {
                        "doi": {"type": "string"},
                        "pmid": {"type": "string"},
                        "title": {"type": "string"},
                        "span": {"type": "string"}
                    }
                }
            }, "required": ["actor_id", "target_finding_id", "text", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "Requires actor.tier=auto-notes; calls from non-tiered actors are rejected.",
                "Notes still do not change finding state — they accrete review context.",
            ],
        ),
        tool(
            "propose_revise_confidence",
            "Propose a confidence revision (`finding.confidence_revise`) on a finding. `new_score` must be in [0.0, 1.0]. Requires a registered actor and signature.",
            json!({"type": "object", "properties": {
                "actor_id": {"type": "string"},
                "target_finding_id": {"type": "string"},
                "new_score": {"type": "number"},
                "reason": {"type": "string"},
                "created_at": {"type": "string"},
                "signature": {"type": "string"}
            }, "required": ["actor_id", "target_finding_id", "new_score", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "Confidence revisions update score and basis; they do not change scope or evidence.",
            ],
        ),
        tool(
            "propose_retract",
            "Propose retracting a finding (`finding.retract`). Applying triggers per-dependent `finding.dependency_invalidated` events through the propagation graph. Requires a registered actor and signature.",
            json!({"type": "object", "properties": {
                "actor_id": {"type": "string"},
                "target_finding_id": {"type": "string"},
                "reason": {"type": "string"},
                "created_at": {"type": "string"},
                "signature": {"type": "string"}
            }, "required": ["actor_id", "target_finding_id", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "Retraction propagates through declared dependency/support links; review impact before applying.",
            ],
        ),
        tool(
            "accept_proposal",
            "Apply a pending proposal as the named reviewer. The reviewer must be registered. Signature is over `{action: \"accept\", proposal_id, reviewer_id, reason, timestamp}` canonicalized. Idempotent: re-applying returns the same `applied_event_id`.",
            json!({"type": "object", "properties": {
                "proposal_id": {"type": "string"},
                "reviewer_id": {"type": "string"},
                "reason": {"type": "string"},
                "timestamp": {"type": "string"},
                "signature": {"type": "string"}
            }, "required": ["proposal_id", "reviewer_id", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "Accepting an applied proposal returns its existing event_id; no duplicate event is emitted.",
            ],
        ),
        tool(
            "reject_proposal",
            "Reject a pending proposal as the named reviewer. The reviewer must be registered. Signature is over `{action: \"reject\", proposal_id, reviewer_id, reason, timestamp}` canonicalized.",
            json!({"type": "object", "properties": {
                "proposal_id": {"type": "string"},
                "reviewer_id": {"type": "string"},
                "reason": {"type": "string"},
                "timestamp": {"type": "string"},
                "signature": {"type": "string"}
            }, "required": ["proposal_id", "reviewer_id", "reason", "signature"]}),
            PermissionLevel::Write,
            true,
            vec![
                "Rejection records the decision but emits no canonical event; rejected proposals stay on the proposal log.",
            ],
        ),
    ]
}

pub fn get_tool(name: &str) -> Option<ToolDefinition> {
    all_tools().into_iter().find(|tool| tool.name == name)
}

pub fn tool_caveats(name: &str) -> Vec<String> {
    get_tool(name).map(|tool| tool.caveats).unwrap_or_default()
}

pub fn mcp_tools_json() -> Value {
    Value::Array(
        all_tools()
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.parameters,
                    "metadata": {
                        "permission_level": tool.permission_level,
                        "mutating": tool.mutating,
                        "caveats": tool.caveats,
                    }
                })
            })
            .collect(),
    )
}

fn tool(
    name: &str,
    description: &str,
    parameters: Value,
    permission_level: PermissionLevel,
    mutating: bool,
    caveats: Vec<&str>,
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
        permission_level,
        mutating,
        caveats: caveats.into_iter().map(str::to_string).collect(),
    }
}
