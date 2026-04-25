# First frontier

This is the recommended first serious Vela workflow for protocol and tooling
evaluators.

`compile` is a bootstrap path into candidate frontier state. It is not the trust
anchor. The v0 trust boundary is reviewed state: proposal -> canonical event ->
reducer -> replayable frontier.

## Choose a bounded question

Use a narrow corpus: 5-20 papers around one *claim family* — a small cluster
of related findings about one mechanism, target, intervention, or review
question (e.g. "does LRP1 shuttle amyloid-β across the blood-brain barrier",
not "Alzheimer's disease"). Good first frontiers are small enough that a
human can inspect every generated finding.

## Prepare a paper folder

Supported local sources:

- PDF files
- JATS XML or NXML
- Markdown or text notes
- CSV/TSV files with curated findings
- `.doi` or `.dois` files with one DOI per line

CSV rows should include an `assertion` column. Optional useful columns are
`type`, `evidence`, `confidence`, `entities`, `source`, `span`, and `direction`.
Entity cells use `name:type` pairs separated by semicolons.

## Run the bootstrap loop

```bash
cargo build --release -p vela-protocol
mkdir -p /tmp/vela-first-frontier
./target/release/vela compile examples/paper-folder/papers --output /tmp/vela-first-frontier/frontier.json
less /tmp/vela-first-frontier/compile-report.json
less /tmp/vela-first-frontier/quality-table.json
less /tmp/vela-first-frontier/frontier-quality.md
./target/release/vela check /tmp/vela-first-frontier/frontier.json --strict --json
./target/release/vela normalize /tmp/vela-first-frontier/frontier.json --out /tmp/vela-first-frontier/frontier.normalized.json
FINDING_ID=$(jq -r '.findings[0].id' /tmp/vela-first-frontier/frontier.json)
./target/release/vela review /tmp/vela-first-frontier/frontier.normalized.json "$FINDING_ID" --status contested --reason "First-pass fixture review: verify the source span before reuse." --reviewer reviewer:demo --apply
./target/release/vela caveat /tmp/vela-first-frontier/frontier.normalized.json "$FINDING_ID" --text "First-pass caveat: deterministic extraction should be manually verified." --author reviewer:demo --apply
./target/release/vela history /tmp/vela-first-frontier/frontier.normalized.json "$FINDING_ID"
./target/release/vela proof /tmp/vela-first-frontier/frontier.normalized.json --out /tmp/vela-first-frontier/proof-packet
./target/release/vela serve /tmp/vela-first-frontier/frontier.normalized.json --check-tools
```

`compile` writes three sidecars beside the frontier:

- `compile-report.json`: accepted/skipped/error sources, extraction modes,
  warnings, and source coverage
- `quality-table.json`: finding-level source, confidence, evidence spans,
  unresolved entities, flags, and caveats
- `frontier-quality.md`: a human-readable review queue for the same quality
  diagnostics

If no model key is configured, Vela uses deterministic fallback extraction for
text-like sources where possible. Treat those findings as review leads, not
scientific conclusions.

For a copy-paste local fixture run:

```bash
examples/paper-folder/run.sh
```

It writes all outputs to `/tmp/vela-first-frontier`.

## What good looks like

- `check --strict --json` reports `ok: true`.
- Warnings are explicit and source-specific.
- Every finding has provenance and a source span status.
- Candidate gaps, bridges, and tensions remain caveated review surfaces.
- At least one finding can be reviewed, caveated, revised, rejected, and traced
  as a state transition with `vela history`.
- An accepted correction changes inherited frontier state.
- The MCP investigator prompt cites `vf_*` IDs.
- `packet validate proof-packet` reports `status: ok`.

## Inspect the frontier

```bash
vela stats frontier.normalized.json
vela search "LRP1 amyloid" --source frontier.normalized.json
vela tensions frontier.normalized.json --both-high
vela gaps rank frontier.normalized.json --top 5
```

Review finding IDs, evidence spans, provenance, conditions, and confidence
components before using the frontier as agent context.

## Correct bad findings

Use [Frontier Review](FRONTIER_REVIEW.md) to reject unclear assertions, add
caveats, fix entities, or improve evidence. Then rerun:

```bash
vela check frontier.normalized.json --strict --json
vela proof frontier.normalized.json --out proof-packet
```

`vela proof` exports a review packet without changing `frontier.normalized.json`
unless you explicitly pass `--record-proof-state`.

Normalize deterministic source/evidence/condition projections before applying
reviewed state transitions. v0 refuses `normalize --out` and `normalize --write`
once canonical events exist, because normalization is not yet represented as a
reviewed event and can otherwise break replay hashes.
