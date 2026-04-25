# Vela demo

Automated walkthrough of the proof-ready BBB/Alzheimer frontier workflow. This
is a protocol demo, not an authoritative Alzheimer's BBB science map.

## Prerequisites

- Vela release binary: `cargo build --release -p vela-protocol`
- `jq` installed: `brew install jq`
- canonical sample frontier at `frontiers/bbb-alzheimer.json`
- port 3001 available

## Run

```bash
cd ~/personal/vela
./demo/run-bbb-proof.sh
```

For a shorter reviewer-facing pass:

```bash
./demo/v0-state-proof-demo.sh
```

`v0-state-proof-demo.sh` is the concise narrative demo: correction -> event ->
history -> stale proof -> refreshed proof. `run-bbb-proof.sh` is the full
integration gate.

The script runs a deterministic walk over `frontiers/bbb-alzheimer.json`:

- CLI stats, search, tensions, and gap ranking
- HTTP/MCP checks for stats, findings, tools, and tool calls
- canonical packet export and `packet validate`
- one accepted correction that makes the prior proof stale
- reproducible generated output in ignored `demo/bbb-proof-run-.../`

## What it demonstrates

1. BBB/Alzheimer frontier stats, links, gaps, and confidence distribution
2. structured finding retrieval around LRP1/RAGE/BBB transport evidence
3. candidate bridges, gaps, and tensions as review surfaces
4. simulated retraction impact over declared dependency links
5. proof packet assembly and validation
6. proof freshness becoming stale after an accepted correction

The demo is an integration gate for the bounded proof wedge. It is not a claim
of complete field coverage, trusted automated extraction, or automated
scientific judgment.
