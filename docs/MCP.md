# Vela MCP Server

`vela serve` exposes the Vela substrate as an MCP server (JSON-RPC 2.0,
spec v2024-11-05) and as an HTTP API. Same tools, two transports.

The server runs on stdio for MCP clients (Claude Desktop, Claude Code, any
spec-compliant client) and on HTTP when invoked with `--http <port>`.

## Connection

### MCP stdio
```bash
vela serve frontiers/bbb-alzheimer.json
```
Wire to a Claude Desktop config or any MCP-aware host.

### HTTP
```bash
vela serve frontiers/bbb-alzheimer.json --http 3848
```
Endpoints listed below.

## Tools

The server registers 18 tools. Read tools require no identity. Write tools
require the caller to be a registered actor (`vela actor add`) and to sign the
canonical preimage with the actor's Ed25519 key.

### Read tools (10)

| Tool | Purpose |
| --- | --- |
| `frontier_stats` | Counts, confidence distribution, gaps, categories. |
| `search_findings` | Free-text + entity/type filter over findings. |
| `get_finding` | Full finding bundle by id. |
| `list_gaps` | Findings flagged as gap review leads. |
| `list_contradictions` | Contradiction/dispute links. |
| `find_bridges` | Cross-domain entities (≥N categories). |
| `check_pubmed` | Rough PubMed prior-art count. |
| `apply_observer` | Rerank findings under a policy. |
| `propagate_retraction` | Simulate cascade impact (read-only). |
| `trace_evidence_chain` | Evidence lineage for a finding. |

### Phase Q-r read tool (1)

| Tool | Purpose |
| --- | --- |
| `list_events_since` | Cursor-paginated read over the canonical event log. Used by agent loops to learn outcomes; used by public consumers to track diffs. |

### Phase Q-w write tools (6)

Each requires `actor_id` + `target_finding_id` (or `proposal_id`) + `reason`
+ `signature`. The `signature` is hex-encoded Ed25519 over the canonical
preimage of the proposal (or decision action).

| Tool | Purpose |
| --- | --- |
| `propose_review` | Create `finding.review` proposal (`status` ∈ accepted/approved/contested/needs_revision/rejected). |
| `propose_note` | Attach a `finding.note` annotation. Optional `provenance: {doi?, pmid?, title?, span?}` (Phase β, v0.6). |
| `propose_and_apply_note` | One-call propose+apply for `finding.note`. Requires `actor.tier="auto-notes"` (Phase α, v0.6). |
| `propose_revise_confidence` | `finding.confidence_revise` with `new_score` ∈ [0,1]. |
| `propose_retract` | `finding.retract` (cascade-emitting on apply). |
| `accept_proposal` | Apply pending proposal as the registered reviewer. |
| `reject_proposal` | Reject pending proposal. |

Idempotency is a substrate property (Phase P): retrying a `propose_*` with
identical content returns the same `vpr_…` and the server returns the
existing record without duplicating state. Same property holds for
`propose_and_apply_note`: identical content yields the same `vpr_…` and
the same `applied_event_id`.

**Tier-gated auto-apply (Phase α, v0.6).** `propose_and_apply_note` is
the only `propose_and_apply_*` variant in v0.6, by design. Tiers permit
review-context kinds only; never state-changing kinds. See
[`docs/TIERS.md`](./TIERS.md) for the doctrine.

## HTTP endpoints

```
GET  /api/frontier            — full project view (findings, sources, events, ...)
GET  /api/findings?query=...  — markdown-formatted search results
GET  /api/findings/{id}       — single finding bundle
GET  /api/contradictions      — contradiction links
GET  /api/observer/{policy}   — reranked findings under a policy
GET  /api/propagate/{id}      — simulated retraction cascade
GET  /api/hypotheses          — cross-domain entity bridges
GET  /api/stats               — frontier stats summary
GET  /api/frontiers           — (multi-frontier mode) list all frontiers
GET  /api/pubmed?query=...    — PubMed prior-art lookup
GET  /api/events?since=…&limit=…  — cursor-paginated event log read (Phase Q-r)
POST /api/queue               — append unsigned draft action (Phase R)
GET  /api/tools               — tool registry (17 tools)
POST /api/tool                — RPC-style tool invocation (read or write)
```

Write semantics: `POST /api/tool` with `{"name": "<write_tool>", "arguments":
{...}}`. Each write tool's argument schema is in `tool_registry.rs`.

## Doctrine

- **Reads are open.** No auth on read tools/endpoints. Agents and public
  consumers use the same surface.
- **Writes are bound.** Every write requires a registered actor's signature
  over the canonical preimage; unsigned or wrong-signature requests are
  rejected.
- **Canonical JSON is normative.** Two implementations of the protocol must
  produce byte-identical signing bytes and content-addressed IDs; the
  conformance vectors at `tests/conformance/` and the Python validator at
  `scripts/cross_impl_conformance.py` pin this property.
