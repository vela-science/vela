# Benchmarks

Vela v0 uses benchmarks as release discipline, not as a claim of automated
scientific discovery.

The BBB condition-loss demo contract lives in
`demo/condition-loss-transcript.md`: exposure is not therapeutic efficacy, mouse
evidence is not a human claim, missing comparators stay visible, synthetic or
agent-generated sources require review, and new state events can make proof
packets stale.

The current benchmark suite asks a narrow set of release-gate questions. These
benchmarks protect the state/proof workflow from drift; they do not make compile
quality the v0 proof.

- Does this frontier preserve useful, grounded finding structure against a
  small gold set?
- Are core BBB entities present with the expected frontier entity types?
- Are frozen source-target typed links still present?
- Does the frontier meet workflow minimums needed by proof/check/serve demos?
- Do workflow metrics preserve evidence span and provenance coverage?
- Do condition records keep exposure, efficacy, comparator, and translation
  scope visible enough to catch condition-loss regressions?

## Canonical command

```bash
vela bench frontiers/bbb-alzheimer.json --gold benchmarks/gold-50.json
```

Canonical suite gate:

```bash
vela bench --suite benchmarks/suites/bbb-core.json --json
```

Suite readiness helper:

```bash
vela bench --suite benchmarks/suites/bbb-core.json --suite-ready
```

Machine-readable form:

```bash
vela bench --suite benchmarks/suites/bbb-core.json --json
```

The default finding benchmark has a hard F1 threshold. The threshold is
conservative because the checked-in gold set is intentionally small and exists
to prevent drift, not to prove scientific superiority.

## JSON contract

`vela bench --json` returns:

- `ok`: whether the benchmark passed configured thresholds
- `benchmark_type`: `suite`, `finding`, `entity`, `link`, or `workflow`
- `mode`: task mode for single-task envelopes
- `suite_id` / `task_id`: present for suite task envelopes
- `schema_version`: Vela schema version used by the frontier
- `frontier`: source path and hash
- `gold`: gold-set path, hash, and item count
- `metrics`: mode-specific precision, recall, F1, type, confidence, link, or
  workflow counts, evidence span coverage, and provenance coverage
- `thresholds`: active pass/fail thresholds
- `failures`: empty when `ok` is true
- `match_details` or `details`: reviewable alignment examples

See [CLI JSON](CLI_JSON.md) for the stable machine contract.

## BBB suite fixtures

The canonical BBB suite is:

- `benchmarks/suites/bbb-core.json`
- `benchmarks/gold/findings/bbb-core-50.json`
- `benchmarks/gold/entities/bbb-entity-50.json`
- `benchmarks/gold/links/bbb-link-50.json`

The local paper-folder contributor fixture is:

- `benchmarks/suites/example-paper-folder.json`
- `benchmarks/gold/findings/example-paper-folder.json`
- `benchmarks/gold/entities/example-paper-folder.json`
- `benchmarks/gold/links/example-paper-folder.json`

This smaller suite checks the first-use local corpus path. It is not a second
canonical science frontier.

## Standard candles

Benchmark gold fixtures are Vela's standard candles: small reviewed anchors
whose expected state is known. They calibrate drift in the bootstrap/compiler
path, schema, entity resolution, typed links, and proof workflow.

They are deliberately narrow:

- finding gold items anchor exact `vf_*` IDs, assertion text, entity presence,
  assertion type, and confidence ranges
- entity gold items anchor entity names, types, and resolution expectations
- link gold items anchor declared source-target-type structure
- workflow checks anchor proof/serve readiness, provenance coverage, and span
  coverage

`vela bench --suite ... --json` exposes a `standard_candles` block so release
consumers can see which reviewed anchors were used. Passing this gate means the
release did not drift against the anchors. It does not mean the field is
complete or that Vela has proven scientific superiority.

The finding fixture uses explicit `vf_*` finding IDs and exact assertion text
from `frontiers/bbb-alzheimer.json`. The entity fixture checks name/type
presence and resolution confidence for core BBB concepts; empty
`expected_source` and `expected_id` mean no external canonical identifier is
required for that item. The link fixture checks exact source ID, target ID, and
link type. The workflow task checks frontier minimums for findings, typed links,
entity mentions, evidence spans, assertion-type diversity, gap flags, and
contested flags. It also reports evidence span coverage and provenance
coverage so first-user frontiers cannot silently become ungrounded.

Validate fixture drift before running the gate:

```bash
benchmarks/validate-benchmark-fixtures.py --suite benchmarks/suites/bbb-core.json
```

## Baselines

Vela should be compared against three simple modes:

- **Paper folder only:** raw papers, PDFs, and notes with no structured state.
- **Naive RAG:** embeddings over papers with no typed findings, confidence, or
  correction trail.
- **Vela frontier:** structured findings, evidence, confidence, typed links,
  packet export, and MCP/HTTP serving.

The v0 claim is limited: Vela should improve inspectability, repeatability,
reviewable correction, stale-proof detection, citation grounding, and agent
context. It does not claim definitive novelty, trusted automated extraction,
automated discovery, or real-world institutional adoption.

## Release gate

The release gate runs:

```bash
./tests/test-benchmark-regression.sh
```

Release assets also include a benchmark JSON report generated by:

```bash
./scripts/package-release-assets.sh
```
