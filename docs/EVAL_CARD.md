# Eval card

Vela uses benchmarks as release discipline, not as proof of automated
scientific discovery.

## What is measured

- finding F1 against frozen gold assertions
- entity precision, recall, and type accuracy
- typed link F1
- confidence calibration against simple accepted ranges
- evidence span coverage
- provenance coverage
- source registry coverage
- evidence atom coverage
- condition-record coverage
- workflow readiness for check, proof, serve, and MCP usage

## Current suites

```bash
vela bench --suite benchmarks/suites/bbb-core.json --json
vela bench --suite benchmarks/suites/example-paper-folder.json --json
```

BBB/Alzheimer remains the only canonical protocol frontier. The paper-folder
suite is a small contributor fixture that regresses local corpus bootstrap
behavior; it is not the v0 proof claim.

## Current scores

| Suite | Tasks | Workflow | Span coverage | Provenance coverage |
|---|---:|---:|---:|---:|
| BBB Alzheimer | 4/4 | 1.000 | 1.000 | 1.000 |
| Example paper folder | 4/4 | 1.000 | 1.000 | 1.000 |

These are fixture regression scores. They mean the checked-in frontiers still
meet their frozen release contracts; they do not prove scientific superiority.

## What is not measured

- definitive novelty
- automated discovery
- clinical truth
- complete field coverage
- real-world adoption
- whether a candidate gap is the best experiment to run

## Baseline comparison

| Mode | What the user gets | Main weakness |
|---|---|---|
| Paper folder only | PDFs, notes, and filenames | Hard to inspect claims, provenance, disagreement, or reuse. |
| Naive RAG | Searchable chunks and summaries | Weak correction trail, no typed finding state, and fragile citation grounding. |
| Vela frontier | Finding bundles, evidence, confidence, links, quality diagnostics, proof packet, and MCP tools | Compile output can still be wrong; accepted state transitions are the trust boundary. |

## Reproduce

```bash
./scripts/release-check.sh
```

The release gate runs the BBB proof workflow, HTTP/MCP smoke tests, benchmark
regression, and the local paper-folder workflow.
