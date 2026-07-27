//! Tool registry — tools defined as data, separate from execution.
//! Borrowed from Codex (MIT) tool-as-data pattern.
//!
//! The surface is exactly eight tools. Each one owns a concept (orientation,
//! one finding, search, the graph, verification, agent work, agent
//! objects, external services); the dispatch in
//! `vela-cli/src/server/serve.rs` maps each onto the underlying analysis
//! functions. Schemas are strict: closed sets are enums, actor ids carry
//! patterns, required text carries minLength, lists carry limit bounds and
//! opaque cursors.

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
            "orient",
            "One-call situational awareness for the served frontier: stats, verification \
             posture, ranked open targets, recent events, and gap-flagged findings. Pass \
             `problem` to also get the full task briefing for that problem (statement, gate \
             status, allowed output types, failed-route memory, attempt ledger, obligations, \
             staleness). Call this first in a session; for a single known finding use \
             `finding` instead. Example: {\"problem\": \"617\", \"limit\": 10}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "problem": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Problem number like \"617\", a vf_ finding id, or a statement substring. Omit for whole-frontier orientation only."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "Cap for open targets, gaps, and the recent-event tail (default 12)."
                    },
                    "review_cursor": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 16384,
                        "description": "Opaque continuation returned as pending_review.next_cursor. Pass it back unchanged with the same limit."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Open targets and rankings are advice, never authority; starting one goes through `attempt` with action=start.",
                "Campaign seeds require the server to know the frontier directory; hosted/merged serves list only review and verify lanes.",
            ],
        ),
        tool(
            "finding",
            "Fetch one finding by vf_ id: assertion, evidence, conditions, links, confidence, \
             and provenance. Add `include` entries to merge the finding's chronological event \
             history, its direct dependents, or its full graph neighborhood into the same \
             response. Use `search` when you do not know the id, and `graph` for multi-hop \
             traversal. Example: {\"id\": \"vf_3f9a\", \"include\": [\"history\", \"dependents\"]}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["id"],
                "properties": {
                    "id": {
                        "type": "string",
                        "minLength": 3,
                        "pattern": "^vf_",
                        "description": "The vf_ finding id (a unique prefix is accepted)."
                    },
                    "include": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["history", "dependents", "neighborhood"]},
                        "description": "Extra payloads to merge: history (event log for this finding), dependents (direct inbound links), neighborhood (rests-on / dependents / related / contradictions in one view)."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Neighborhood and dependent relations are declared links, not adjudicated truth.",
                "History event order reflects timestamps as recorded.",
            ],
        ),
        tool(
            "search",
            "Search the frontier by text over findings, sources, and evidence atoms. Returns \
             structured matches plus `next_cursor` when more remain — pass it back unchanged \
             to continue (it is an opaque cursor into the stable result order). Use `finding` \
             to fetch a known id, and `orient` for whole-frontier awareness. Example: \
             {\"query\": \"Sidon\", \"type\": \"finding\", \"limit\": 24}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Case-insensitive substring matched against assertions, conditions, entity names, source titles/DOIs, and evidence text."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["finding", "source", "evidence", "any"],
                        "description": "Restrict the object kind searched (default any)."
                    },
                    "entity": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Additionally require this substring among a finding's entity names."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Maximum matches per page (default 24)."
                    },
                    "cursor": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Opaque continuation cursor from a previous response's next_cursor."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec!["Matches are substring hits over declared content, not a relevance ranking."],
        ),
        tool(
            "graph",
            "Walk the typed claim graph. mode=traverse explores out from `root` layered by hop \
             distance plus the finding's evidence chain (omit root for a whole-graph summary); \
             mode=impact computes the dependency blast radius and retraction cascade for \
             `root`; mode=contradictions lists raw contradiction links and first-class \
             contradiction objects together, each row tagged `first_class`. For one node's \
             immediate neighborhood use `finding` with include=[\"neighborhood\"]. Example: \
             {\"root\": \"vf_3f9a\", \"mode\": \"impact\", \"direction\": \"down\"}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "root": {
                        "type": "string",
                        "minLength": 3,
                        "pattern": "^vf_",
                        "description": "The vf_ finding to start from. Required for mode=impact; omit in mode=traverse for a whole-frontier summary."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["up", "down", "both"],
                        "description": "Impact direction: up = what root rests on, down = what rests on root, both (default)."
                    },
                    "edge_kinds": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["supports", "contradicts", "depends_on", "derived_from", "replicates", "specializes"]},
                        "description": "Edge-kind filter. Applied to impact traversal and to whole-graph edge listings; traverse mode follows all declared kinds and notes when the filter is not applied."
                    },
                    "max_hops": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 6,
                        "description": "Traversal depth for mode=traverse (default 2)."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["traverse", "impact", "contradictions"],
                        "description": "What to compute (default traverse)."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 500,
                        "description": "Cap on returned nodes/edges/rows per section."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Edges are candidate relations over declared links, not adjudicated truth.",
                "Impact is structural: being in the blast radius is not a claim a result is wrong.",
                "Candidate contradictions are auto-detected signals pending expert review.",
            ],
        ),
        tool(
            "verify",
            "Run the frozen verifiers against the one frontier checkout bound to this MCP \
             server. mode=strict holds the frontier to the strict bar (content-address \
             validation, strict reducer replay, signature signals — the same bundle the hub's \
             git ingestor enforces); mode=witness re-verifies every stored \
             witnesses/*.witness.json from scratch with the frozen exact verifiers. Read-only \
             but path-bound, so it is not served on hosted endpoints. Example: \
             {\"frontier_path\": \".\", \"mode\": \"witness\"}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["frontier_path", "mode"],
                "properties": {
                    "frontier_path": {
                        "type": "string",
                        "minLength": 1,
                        "description": "The exact frontier checkout bound to this MCP server; direct files and other local paths are refused."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["strict", "witness"],
                        "description": "strict = validation + reducer replay + signature signals; witness = re-verify stored witnesses."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec!["Read-only: replays and verifies, writes nothing."],
        ),
        tool(
            "attempt",
            "The non-finalizing producer loop against one local Frontier checkout. action=start \
             leases an exact Target and opens an Attempt; action=submit registers one signed \
             vela.submission.v1 and pending Proposal; action=abandon releases the exact \
             same-owner lease and removes private Attempt state. Registration creates no \
             Verification Record, Decision, Event, or accepted-state change. Example: \
             {\"frontier_path\": \".\", \"action\": \"start\", \"obligation_id\": \
             \"vf_3f9a\", \"agent_actor\": \"agent:swarm-1\"}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["frontier_path", "action"],
                "properties": {
                    "frontier_path": {
                        "type": "string",
                        "minLength": 1,
                        "description": "The exact frontier checkout bound to this MCP server."
                    },
                    "action": {
                        "type": "string",
                        "enum": ["start", "submit", "abandon"],
                        "description": "start = lease a Target and open an Attempt; submit = register signed Submission v1; abandon = release the Attempt."
                    },
                    "obligation_id": {
                        "type": "string",
                        "minLength": 1,
                        "description": "start/abandon: the exact Target to lease or release."
                    },
                    "attempt_id": {
                        "type": "string",
                        "pattern": "^vat_[0-9a-f]{64}$",
                        "description": "submit: optional exact Attempt id when the actor owns more than one active Attempt."
                    },
                    "agent_actor": {
                        "type": "string",
                        "minLength": 4,
                        "pattern": "^(agent:|ci:)",
                        "description": "start/submit/abandon: the producer identity; abandon is allowed only for the exact lease owner."
                    },
                    "ttl_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "start: lease TTL (default 86400)."
                    },
                    "release_reason": {
                        "type": "string",
                        "minLength": 1,
                        "description": "abandon: why the exact current owner is releasing the lease without a Submission."
                    },
                    "submission": {
                        "type": "string",
                        "minLength": 2,
                        "description": "submit: raw vela.submission.v1 JSON text. Pass exact bytes so Vela can reject duplicate object names before normalization."
                    }
                }
            }),
            PermissionLevel::Write,
            true,
            vec![
                "Uses only the producer's agent identity, never a human authority key.",
                "An Attempt is coordination and Submission registration is intake; neither is Verification or acceptance.",
            ],
        ),
        tool(
            "objects",
            "Read the content-addressed agent objects on a frontier checkout's .vela/ tree: \
             diff packs (vsd_), attestations (vaa_), evaluations (ver_), verdict conflicts \
             (vdc_), tool descriptors (vtd_). Pass `id` to fetch one object; omit it to list \
             with `limit` and opaque `cursor` pagination — `target` filters evaluations by \
             descriptor id and conflicts by resolution mode, `only_pending` filters packs. \
             Path-bound, so not served on hosted endpoints. Example: {\"frontier_path\": \
             \".\", \"type\": \"pack\", \"only_pending\": true}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["frontier_path", "type"],
                "properties": {
                    "frontier_path": {
                        "type": "string",
                        "minLength": 1,
                        "description": "The exact frontier checkout bound to this MCP server."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["pack", "attestation", "evaluation", "conflict", "tool_descriptor"],
                        "description": "Which object family to read."
                    },
                    "id": {
                        "type": "string",
                        "minLength": 5,
                        "description": "Fetch one object by its typed id (vsd_/vaa_/ver_/vdc_/vtd_). Omit to list."
                    },
                    "target": {
                        "type": "string",
                        "minLength": 1,
                        "description": "List filter: for evaluations, a vtd_ descriptor id; for conflicts, a resolution mode (majority, owner_override, escalation)."
                    },
                    "only_pending": {
                        "type": "boolean",
                        "description": "List filter for packs: only those awaiting a reviewer verdict."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Maximum objects per page (default 50)."
                    },
                    "cursor": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Opaque continuation cursor from a previous response's next_cursor."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "Objects are read verbatim from the frontier's .vela/ tree; listing order is by id.",
            ],
        ),
        tool(
            "external",
            "Query an external service for a rough prior-art count on `query`: \
             service=pubmed (NCBI esearch, biomedical), service=arxiv (math/CS/physics, \
             exact-phrase), service=semantic_scholar (all fields); or service=nanopub exports \
             `finding_id` as a nanopublication (TriG/RDF) for the FAIR / semantic-web ecosystem. \
             Results are signals or interchange artifacts, never canonical state. Example: \
             {\"service\": \"arxiv\", \"query\": \"cap set problem\"}.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["service"],
                "properties": {
                    "service": {
                        "type": "string",
                        "enum": ["pubmed", "nanopub"],
                        "description": "Which external surface to hit."
                    },
                    "query": {
                        "type": "string",
                        "minLength": 1,
                        "description": "pubmed: the prior-art query."
                    },
                    "finding_id": {
                        "type": "string",
                        "minLength": 3,
                        "pattern": "^vf_",
                        "description": "nanopub: the vf_ finding to export."
                    }
                }
            }),
            PermissionLevel::ReadOnly,
            false,
            vec![
                "PubMed counts are rough prior-art signals, not proof of novelty.",
                "Nanopublication export is a derived interchange artifact; the canonical finding remains the vf_ object.",
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

/// MCP exposure profile (memo §9.1). A served frontier scopes which tools an
/// agent can see and call. `MCP exposes tools; Vela governs state`. Human
/// finalization is available only through one direct protected
/// `vela review accept` or `vela review reject` action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpProfile {
    /// Inspect state, graph, provenance, tasks, schemas. The default.
    ReadOnly,
    /// Read + the non-finalizing producer Attempt workflow (`attempt`).
    Draft,
}

impl McpProfile {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "read-only" => Ok(Self::ReadOnly),
            "draft" => Ok(Self::Draft),
            other => Err(format!(
                "unknown MCP profile `{other}`; valid: read-only (default), draft"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Draft => "draft",
        }
    }

    /// Whether this profile may expose AND execute `tool`. Read-only admits
    /// only non-mutating reads. Draft admits non-dangerous drafting writes.
    /// No MCP profile exposes human finalization.
    pub fn allows(self, tool: &ToolDefinition) -> bool {
        match self {
            Self::ReadOnly => matches!(tool.permission_level, PermissionLevel::ReadOnly),
            Self::Draft => !matches!(tool.permission_level, PermissionLevel::Dangerous),
        }
    }
}

pub fn tools_for_profile(profile: McpProfile) -> Vec<ToolDefinition> {
    all_tools()
        .into_iter()
        .filter(|tool| profile.allows(tool))
        .collect()
}

/// The MCP-standard tool annotations, derived from the tool's own
/// permission level and scope. Claude Code reads `readOnlyHint` to run
/// read tools concurrently, so exposing these speeds a swarm's inspection
/// calls. `destructiveHint` is true for `attempt` because action=abandon
/// removes the caller-owned private Attempt directory (never truth-bearing
/// state).
/// `openWorldHint` is true only for `external`, which
/// reaches outside the frontier (PubMed / nanopublications).
fn tool_annotations(tool: &ToolDefinition) -> Value {
    let read_only = matches!(tool.permission_level, PermissionLevel::ReadOnly);
    json!({
        "title": tool.name,
        "readOnlyHint": read_only,
        "destructiveHint": tool.name == "attempt",
        "idempotentHint": read_only,
        "openWorldHint": tool.name == "external",
    })
}

/// Output schema (JSON Schema for the result `data` payload) for the
/// high-traffic tools, so typed clients can validate `structuredContent`.
/// The envelope (ok/signals/caveats/duration) stays in the text block;
/// this describes the `data` a caller actually consumes. Tools without a
/// schema here return text only (still valid MCP).
pub fn tool_output_schema(name: &str) -> Option<Value> {
    let schema = match name {
        "orient" => json!({
            "type": "object",
            "description": "Situational awareness for the served frontier.",
            "properties": {
                "frontier": {"type": "object"},
                "pending_review": {"type": ["object", "null"]},
                "open_targets": {"type": "array"},
                "gaps": {"type": "array"},
                "recent_events": {"type": "array"},
                "agent_objects": {"type": ["array", "object", "null"]},
                "briefing": {"type": ["object", "null"]}
            }
        }),
        "finding" => json!({
            "type": "object",
            "description": "One finding's claim, evidence, gate status, and links.",
            "properties": {
                "id": {"type": "string"},
                "assertion": {"type": "object"},
                "gate_status": {"type": "string"},
                "evidence": {"type": "array"},
                "links": {"type": "array"}
            }
        }),
        "search" => json!({
            "type": "object",
            "description": "Cross-frontier matches for the query.",
            "properties": {
                "query": {"type": "string"},
                "results": {"type": "array"},
                "total": {"type": "integer"}
            }
        }),
        "attempt" => json!({
            "type": "object",
            "description": "The start, submit, or abandon outcome for one producer Attempt.",
            "properties": {
                "action": {"type": "string"},
                "operation_id": {"type": ["string", "null"]},
                "submission_id": {"type": ["string", "null"]},
                "submission_root": {"type": ["string", "null"]},
                "registration_record_id": {"type": ["string", "null"]},
                "proposal_id": {"type": ["string", "null"]},
                "accepted_event_delta": {"type": ["integer", "null"], "minimum": 0},
                "route": {"type": ["string", "null"], "description": "pending_review for Submission registration"},
                "publication": {"type": ["object", "null"]}
            }
        }),
        _ => return None,
    };
    Some(schema)
}

fn tool_to_mcp_json(tool: &ToolDefinition) -> Value {
    let mut obj = json!({
        "name": tool.name,
        "title": tool.name,
        "description": tool.description,
        "inputSchema": tool.parameters,
        "annotations": tool_annotations(tool),
        "metadata": {
            "permission_level": tool.permission_level,
            "mutating": tool.mutating,
            "caveats": tool.caveats,
        }
    });
    if let Some(out) = tool_output_schema(&tool.name) {
        obj["outputSchema"] = out;
    }
    obj
}

pub fn mcp_tools_json() -> Value {
    Value::Array(all_tools().iter().map(tool_to_mcp_json).collect())
}

/// `tools/list` payload scoped to a profile (memo §9.1).
pub fn mcp_tools_json_for_profile(profile: McpProfile) -> Value {
    Value::Array(
        tools_for_profile(profile)
            .iter()
            .map(tool_to_mcp_json)
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

#[cfg(test)]
mod profile_tests {
    use super::*;

    /// The whole MCP surface, by contract: exactly these eight names.
    const THE_EIGHT: [&str; 8] = [
        "orient", "finding", "search", "graph", "verify", "attempt", "objects", "external",
    ];
    const REMOVED_WRITERS: [&str; 2] = ["decide", "propose"];

    #[test]
    fn the_surface_is_exactly_eight_and_finalization_is_absent() {
        let names: Vec<String> = all_tools().into_iter().map(|t| t.name).collect();
        assert_eq!(names, THE_EIGHT.to_vec(), "the eight-tool contract");

        for removed in REMOVED_WRITERS {
            assert!(
                get_tool(removed).is_none(),
                "{removed} must not be registered"
            );
            for profile in [McpProfile::ReadOnly, McpProfile::Draft] {
                assert!(
                    !tools_for_profile(profile)
                        .iter()
                        .any(|tool| tool.name == removed),
                    "{removed} must be absent from {} discovery",
                    profile.as_str()
                );
                assert!(
                    !mcp_tools_json_for_profile(profile)
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|tool| tool["name"] == removed),
                    "{removed} must have no MCP schema in {}",
                    profile.as_str()
                );
            }
        }
    }

    #[test]
    fn profiles_nest_and_readonly_excludes_writes() {
        let ro = tools_for_profile(McpProfile::ReadOnly);
        let draft = tools_for_profile(McpProfile::Draft);
        // read-only is a strict subset of the nonfinalizing draft surface.
        assert!(
            ro.len() < draft.len(),
            "read-only must be a strict subset of draft"
        );
        let draft_names: Vec<&str> = draft.iter().map(|t| t.name.as_str()).collect();
        // read-only = the seven inspection tools, and no mutating tool.
        let ro_names: Vec<&str> = ro.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            ro_names,
            vec![
                "orient", "finding", "search", "graph", "verify", "objects", "external"
            ],
            "read-only is exactly the inspection surface"
        );
        assert!(
            ro.iter().all(|t| !t.mutating),
            "read-only must expose no mutating tool"
        );
        // draft adds the task-first Attempt tool, never a parallel Proposal or
        // finalization tier.
        assert_eq!(
            draft_names,
            vec![
                "orient", "finding", "search", "graph", "verify", "attempt", "objects", "external"
            ]
        );
        assert_eq!(
            draft
                .iter()
                .filter(|tool| tool.mutating)
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["attempt"],
            "MCP writes are limited to the non-finalizing Attempt tool"
        );
    }

    #[test]
    fn every_schema_is_strict() {
        for tool in all_tools() {
            let schema = &tool.parameters;
            assert_eq!(
                schema["additionalProperties"],
                Value::Bool(false),
                "{}: schemas are closed",
                tool.name
            );
            let props = schema["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("{}: object schema with properties", tool.name));
            for (pname, p) in props {
                assert!(
                    p.get("description").and_then(Value::as_str).is_some(),
                    "{}.{pname}: every param carries a description",
                    tool.name
                );
                if let Some(limits) = p.get("type").and_then(Value::as_str)
                    && limits == "integer"
                    && pname == "limit"
                {
                    assert!(
                        p.get("minimum").is_some() && p.get("maximum").is_some(),
                        "{}.{pname}: limit params carry minimum/maximum",
                        tool.name
                    );
                }
            }
            assert!(
                !tool.description.contains("v0."),
                "{}: descriptions carry no version tags",
                tool.name
            );
        }
    }

    #[test]
    fn attempt_submit_exposes_submission_as_raw_json_text() {
        let attempt = get_tool("attempt").unwrap();
        let submission = &attempt.parameters["properties"]["submission"];
        assert_eq!(submission["type"], "string");
        assert!(
            submission["description"]
                .as_str()
                .unwrap()
                .contains("exact bytes"),
            "the MCP contract must preserve producer wire text until SubmissionV1::parse"
        );
    }

    #[test]
    fn attempt_actions_are_exactly_task_first() {
        let attempt = get_tool("attempt").unwrap();
        assert_eq!(
            attempt.parameters["properties"]["action"]["enum"],
            json!(["start", "submit", "abandon"])
        );
        let properties = attempt.parameters["properties"].as_object().unwrap();
        for retired in [
            "problem",
            "kind",
            "claim",
            "detail",
            "claimed_status",
            "insight",
            "base_frontier_root",
            "target_obligation_id",
            "statement_variant_id",
            "method_families",
            "remaining_obligations",
            "named_obstructions",
            "verifier_attachments",
            "producer",
            "frontier",
        ] {
            assert!(
                !properties.contains_key(retired),
                "retired producer field `{retired}` remains in MCP attempt"
            );
        }
    }

    #[test]
    fn profile_parse_roundtrips() {
        assert_eq!(
            McpProfile::parse("read-only").unwrap(),
            McpProfile::ReadOnly
        );
        assert!(McpProfile::parse("read_only").is_err());
        assert!(McpProfile::parse("read").is_err());
        assert!(McpProfile::parse("readonly").is_err());
        assert_eq!(McpProfile::parse("draft").unwrap(), McpProfile::Draft);
        assert!(McpProfile::parse("maintainer").is_err());
        assert!(McpProfile::parse("god-mode").is_err());
    }

    #[test]
    fn tool_annotations_match_permission_and_scope() {
        let by_name: std::collections::HashMap<String, Value> = mcp_tools_json()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t["name"].as_str().unwrap().to_string(), t.clone()))
            .collect();

        // Read tools: readOnlyHint + idempotentHint true (Claude Code runs
        // these in parallel); every tool is non-destructive (append-only).
        for name in ["orient", "finding", "search", "graph", "verify", "objects"] {
            let a = &by_name[name]["annotations"];
            assert_eq!(a["readOnlyHint"], json!(true), "{name} should be read-only");
            assert_eq!(a["idempotentHint"], json!(true), "{name}");
            assert_eq!(a["destructiveHint"], json!(false), "{name}");
            assert_eq!(
                a["openWorldHint"],
                json!(false),
                "{name} is frontier-scoped"
            );
        }
        // Attempt is coarse-marked destructive because abandon removes a
        // private, actor-owned Attempt.
        let attempt = &by_name["attempt"]["annotations"];
        assert_eq!(attempt["readOnlyHint"], json!(false), "attempt writes");
        assert_eq!(attempt["destructiveHint"], json!(true), "attempt");
        // external reaches outside the frontier.
        assert_eq!(
            by_name["external"]["annotations"]["openWorldHint"],
            json!(true)
        );
        // Structured output on the high-traffic tools.
        for name in ["orient", "finding", "search", "attempt"] {
            assert!(
                by_name[name].get("outputSchema").is_some(),
                "{name} should declare an outputSchema"
            );
        }
        let attempt_properties = by_name["attempt"]["outputSchema"]["properties"]
            .as_object()
            .expect("attempt output schema properties");
        for field in [
            "operation_id",
            "submission_id",
            "submission_root",
            "registration_record_id",
            "proposal_id",
            "accepted_event_delta",
            "route",
            "publication",
        ] {
            assert!(
                attempt_properties.contains_key(field),
                "attempt output schema is missing {field}"
            );
        }
        assert_eq!(
            attempt_properties["accepted_event_delta"]["minimum"],
            json!(0)
        );
    }
}
