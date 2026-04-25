# Vela

Vela v0 is portable, correctable frontier state for science. It can bootstrap
candidate state by compiling literature and local corpora, but the release
primitive is the frontier state humans and AI can inspect, correct, serve, and
export. v0 proves state, not extraction.

## Current product shape

Lead with the useful wedge:

- compile candidate state or open a bounded frontier
- check portable frontier state
- record finding additions, reviews, caveats, revisions, rejections, and history as state transitions
- inspect findings, evidence, entities, conditions, provenance, and confidence
- compare candidate contradictions and tensions
- surface candidate underexplored areas and cross-domain connections
- serve the frontier through MCP/HTTP
- export proof packets and reviewable artifacts

Do not present v0 as a full science operating system, federation network, lab runtime, autonomous agent loop, broad exchange-network, Hub, desktop app, or GitHub-for-science product. Those remain roadmap or thesis items only.

## Architecture

Architecture follows this layered shape:

- **Frontier state:** finding bundles, typed links, provenance, confidence, review events, state transitions, signatures
- **Signal layer:** proof readiness, review queues, candidate gaps, candidate bridges, candidate tensions, observer rerankings
- **Review loop:** bootstrap, check, review, search, inspect, proof, serve, benchmark
- **Network later:** compare, merge, institutional sharing, broader federation

Public-facing protocol terms must stay Git-native:

- Findings are versioned as frontier state artifacts (for example `frontier.json`).
- Frontier correction history is represented as signed, reviewable events.
- Shared work should happen with normal Git primitives (branches, commits, diffs).
- Internal Rust identifiers may still use `Project`, but public names must say `frontier`.

## Vocabulary

- **Frontier:** a bounded, reviewable body of structured scientific state
- **Finding bundle:** one assertion with evidence, conditions, entities, confidence, provenance, and links. The assertion is a field; the finding bundle is the durable object.
- **Source:** the paper, dataset, note, protocol, file, or record a finding came from
- **Evidence:** the specific span, row, table, measurement, or excerpt supporting a finding
- **Observation:** a conceptual distinction between what was reported and the
  finding bundle Vela stores; not a first-class v0 object
- **Candidate bridge:** a possible cross-domain connection requiring review
- **Candidate gap:** a possible underexplored area, not a guaranteed experiment target
- **Retraction impact:** simulated impact over declared dependency links
- **Prior-art check:** PubMed search as a rough signal, not proof of novelty
- **Observer policy:** policy-weighted reranking, not definitive disagreement

## CLI

Examples should use:

```bash
vela compile ./papers --output frontier.json
vela check frontier.json --strict --json
vela normalize frontier.json --out frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' frontier.json)
vela review frontier.normalized.json "$FINDING_ID" --status contested --reason "Mouse-only evidence" --reviewer reviewer:demo --apply
vela history frontier.normalized.json "$FINDING_ID"
vela proof frontier.normalized.json --out proof-packet
vela stats frontier.normalized.json
vela search "LRP1 RAGE amyloid" --source frontier.json
vela tensions frontier.json --both-high
vela gaps rank frontier.json --top 5
vela serve frontier.normalized.json
```

Legacy naming note: avoid pre-frontier command, file, route, and MCP-tool names.

## Conservative reasoning loop

When investigating a frontier, keep this loop:

1. Call `frontier_stats`.
2. Search relevant findings with `search_findings`.
3. Inspect important findings with `get_finding`.
4. Review candidate gaps with `list_gaps`.
5. Review candidate bridges with `find_bridges`.
6. Run `check_pubmed` only as a rough prior-art check.
7. Inspect contested claims with `list_contradictions`.
8. Use `propagate_retraction` only as simulated impact over dependency links.
9. Compare `apply_observer` rerankings as policy-weighted views.
10. Summarize conclusions with finding IDs (`vf_xxx`) and explicit caveats.

Evidence ranking is a heuristic: meta-analysis > RCT > cohort > case-control > case-report > in-vitro. Do not overstate automated contradiction, novelty, bridge, gap, or observer outputs.

## Repository map

- `crates/vela-protocol/` - core frontier protocol and runnable `vela` binary
- `frontiers/` - checked-in compiled sample frontier artifacts
- `examples/` - tiny first-use fixtures, including the paper-folder workflow
- `schema/` - finding-bundle JSON schema
- `demo/` - demo scripts
- `docs/` - current product, architecture, protocol, MCP, proof, and vocabulary docs

Inherited coding-agent code, Hub, desktop, archive docs, runtime scaffolds, and reference research are not part of the Vela v0 OSS release. Keep them outside the release repo unless they are redesigned around the core frontier workflow and intentionally reintroduced.

## Doctrine

- `docs/CORE_DOCTRINE.md` is the source of truth for v0 public claims.
- `docs/PROOF.md`, `docs/BENCHMARKS.md`, and `docs/MCP_SETUP.md` define the review, benchmark, and serving contracts.
- Keep public examples centered on candidate bootstrap, `check`, state-transition review, `proof`, and `serve`.
- Preserve the compounding loop: use should write better state back into the frontier rather than create sidecar memory.

## Environment

- Root CLI env: `.env`
- Do not commit real secrets.

## Verification

Use focused gates for this repo:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p vela-protocol
./target/release/vela stats frontiers/bbb-alzheimer.json
./target/release/vela check frontiers/bbb-alzheimer.json
./target/release/vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-proof-packet
./tests/test-local-corpus-workflow.sh
tests/test-http-server.sh
tests/test-mcp-server.sh
./demo/run-bbb-proof.sh
./scripts/release-check.sh
```
