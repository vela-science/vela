# Vela MCP

Vela serves compiled frontiers to AI agents through MCP. Agents can search structured findings, inspect evidence, compare candidate contradictions, review candidate gap leads, and cite finding IDs instead of rebuilding context from raw papers.

## Setup

```bash
cargo build --release
./target/release/vela serve frontier.json
```

Client command:

```text
/absolute/path/to/vela serve /absolute/path/to/frontier.normalized.json
```

For HTTP mode:

```bash
./target/release/vela serve frontiers/bbb-alzheimer.json --http 3001
```

To validate the read-only tool surface without starting a server:

```bash
./target/release/vela serve frontiers/bbb-alzheimer.json --check-tools
./target/release/vela serve frontiers/bbb-alzheimer.json --check-tools --json
```

## Investigation loop

Agents should use Vela as a structured frontier reader:

1. `frontier_stats` - map the frontier size, links, confidence distribution, gaps, and categories
2. `search_findings` - search the user's question across assertions, entities, and conditions
3. `get_finding` - inspect evidence, conditions, provenance, and links for important findings
4. `list_contradictions` - surface candidate disagreements
5. `list_gaps` - surface candidate gap review leads
6. `find_bridges` - surface candidate cross-domain connections
7. `check_pubmed` - run a rough prior-art check for a proposed hypothesis
8. `apply_observer` - rerank findings under a policy lens
9. `trace_evidence_chain` - inspect support, dependency, contradiction, and replication links
10. synthesize with finding IDs

## Tool reference

| Tool | Purpose |
|------|---------|
| `frontier_stats` | Return frontier metadata, aggregate stats, source/evidence/condition counts, proposal counts, and proof freshness state. |
| `search_findings` | Search finding text, entities, conditions, and assertion types. |
| `get_finding` | Return the full finding bundle, linked source records, linked evidence atoms, condition records, and finding-local proposals for a `vf_...` ID. |
| `list_gaps` | List findings flagged as candidate gap review leads. |
| `list_contradictions` | List candidate contradiction pairs. |
| `find_bridges` | List candidate entities spanning multiple assertion categories. |
| `propagate_retraction` | Simulate impact over declared dependency links. |
| `apply_observer` | Run a policy-weighted reranking of findings. |
| `check_pubmed` | Count matching PubMed publications for rough prior-art checking. |
| `trace_evidence_chain` | Return source -> evidence atom -> condition boundary -> proposal/event lineage -> finding -> link/review paths. |

Tool output includes structured `data`, human-readable `markdown`, derived
`signals`, and conservative `caveats`. Where relevant, `frontier_stats`,
`get_finding`, and `trace_evidence_chain` include source, evidence atom,
condition-record, proposal, and canonical event/replay context so agents can
distinguish current snapshot state from review history.

The HTTP endpoint `/api/frontiers` can list loaded frontier sources, but it is
not exposed as an MCP tool. Vela v0 does not expose extension, workspace,
runtime, or autonomous agent tools.

## Example prompts

Copy-paste investigator prompt:

```text
Use the Vela frontier tools as your source of structured scientific context.
First call frontier_stats. Then search_findings for the user's question.
Inspect important results with get_finding. Cite finding IDs like vf_xxx.
Review list_contradictions, list_gaps, and find_bridges only as candidate
surfaces. Preserve caveats, confidence, evidence type, provenance, and review
state. Explain through paths from source to evidence to finding to declared
links and canonical events. Do not turn candidate gaps, bridges, tensions,
prior-art checks, or observer rerankings into definitive scientific conclusions.
```

```text
Search this frontier for LRP1 and RAGE. Open the strongest findings and summarize the evidence.

List candidate contradictions about BBB transport and compare the evidence types on each side.

Find candidate gap review leads in this frontier and explain what evidence would reduce uncertainty.

Run a rough PubMed prior-art check for the top bridge, then cite the supporting finding IDs.
```

## Short transcripts

BBB frontier:

```text
frontier_stats -> 48 findings, 215 links, 1 candidate gap
search_findings {"query":"LRP1 RAGE amyloid","limit":3}
get_finding {"id":"vf_5021284e4155f141"}
list_gaps -> candidate gap review leads, not guaranteed experiments
find_bridges {"limit":5,"min_categories":2}
```

Paper-folder fixture:

```text
frontier_stats -> local corpus summary
search_findings {"query":"amyloid-beta LRP1","limit":3}
get_finding {"id":"vf_c4bf737129fe5c50"}
list_contradictions -> may return none for small curated corpora
```

The deterministic paper-folder transcript is stored at
`examples/paper-folder/expected/mcp-transcript.json`.

## Interpretation rules

- A bridge is a candidate connection, not a discovery.
- A gap ranking is a review-prioritization aid, not a guaranteed experiment target.
- A PubMed count is a rough prior-art signal, not proof of novelty.
- Observer policies expose weighting differences, not definitive stakeholder preferences.
- Simulated impact is computed over declared dependency links, not over the field itself.
- Evidence-chain explanations are path explanations over declared frontier
  state, not proof that the chain is scientifically complete.
