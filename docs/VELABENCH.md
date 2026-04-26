# VelaBench (v0.26)

A reproducible scoring harness for AI-agent **state-update
quality**: how well does an agent-generated frontier approximate
a curator-validated gold frontier?

VelaBench is the first artifact that lets a future agent's
shipping bar be empirical. The forcing function for any new
extractor — Literature Scout, Notes Compiler, Code Analyst,
Datasets, or whatever lands in v0.27+ — becomes:

> *How does it score on `bbb-scout-bench-001`?*

## Doctrine

1. **Pure data comparison.** No LLM call, no network, no agent
   invocation at bench time. The scorer reads two `Project`
   structs and produces numbers.
2. **Deterministic.** Sort by `vf_id`. No wall-clock. No RNG.
   Same inputs → same numbers.
3. **Substrate-level.** Lives in `vela-protocol::agent_bench`;
   the bench has no LLM dependency.
4. **Pre-review and post-review both score.** Candidate findings
   are the union of `frontier.findings` (signed, accepted) and
   the payloads of `finding.add` proposals (unsigned, agent
   output). Same scoring either way.

## Usage

```bash
# Bare comparison — composite reported, no exit gate.
vela bench --gold gold.json --candidate candidate.json

# With evidence_fidelity (substring-match candidate evidence
# spans against the actual source files).
vela bench --gold gold.json --candidate candidate.json \
  --sources ./paper-text-files

# CI gate — non-zero exit if composite < threshold.
vela bench --gold gold.json --candidate candidate.json \
  --threshold 0.55

# Write report to JSON for archival.
vela bench --gold gold.json --candidate candidate.json \
  --report ./run-2026-04-26.json
```

## Metrics

All metrics normalized to `[0, 1]`. `composite` is the headline
number; individual metrics are the breakdown.

| Metric | Formula | Target |
|---|---|---|
| `claim_match_rate` | F1 over claim-text match: `2·|M| / (|G| + |C|)` | ≥ 0.70 |
| `scope_accuracy` | mean over matched pairs of `0.5·organism_eq + 0.5·intervention_overlap` | ≥ 0.80 |
| `evidence_fidelity` | fraction of candidate evidence spans that substring-match a source file (requires `--sources`) | ≥ 0.95 |
| `duplicate_rate` (reported as `1 − duplicate_rate`) | `1 − unique(vf_id)/|C|` | ≤ 0.02 |
| `novelty_rate` | `|C ∖ M| / |C|` (informational, not pass/fail) | report only |
| `contradiction_recall` | gold contradictions detected by candidate / total gold contradictions | ≥ 0.60 |
| `downstream_link_rate` | novel candidate findings that link to ≥1 gold `vf_id` / total novel | ≥ 0.75 |

Matching: greedy on (a) shared content-address `vf_…` and then
(b) claim-text Jaccard ≥ 0.4 on remaining unmatched. Stable
because the inputs are sorted by id.

### Composite

```
composite = 0.25·claim_match
          + 0.20·scope_accuracy
          + 0.20·evidence_fidelity   (when --sources provided)
          + 0.15·contradiction_recall
          + 0.10·downstream_link_rate
          + 0.10·(1 − duplicate_rate)
```

When `--sources` isn't provided, `evidence_fidelity` is dropped
from the composite and the remaining weights rebalance
proportionally so the headline number stays meaningful.

## Vectors

A *bench vector* is a checked-in directory with a frozen
candidate, a gold frontier, optional source files, and an
`expected.json` regression band:

```
benchmarks/bbb-scout-bench-001/
  candidate.json               ← frozen Scout output (1 paper, 2 findings)
  expected.json                ← regression bands per metric
  inputs/
    papers/
      focused-ultrasound.pdf
      focused-ultrasound.txt   ← extracted for evidence_fidelity
```

`gold.json` is referenced from outside the vector (typically
`frontiers/bbb-alzheimer.json`); the vector itself is the
candidate snapshot + the inputs that produced it.

### bbb-scout-bench-001

The first vector. One BBB-adjacent paper (focused ultrasound
review) → two scout-extracted findings. Composite expected band:
`[0.30, 0.55]` — *low by design*, since BBB's gold focuses on
TfR-shuttle / amyloid antibody delivery and the candidate's two
findings are about focused ultrasound. The bench's job here is
to detect drift, not to certify quality.

Specifically:
- `evidence_fidelity` is expected ≥ 0.90 (the scout's evidence
  snippets must continue to substring-match the source paper).
- `novelty_rate` is expected ≥ 0.80 (the candidate is largely
  new claims relative to gold).
- `claim_match_rate` is expected near zero — but if it suddenly
  spikes, that's worth investigating (did scout start
  hallucinating BBB-style claims?).

The composite band is wider than any single metric to absorb
small model-output variance across runs.

## Adding a vector

1. Pick a gold frontier (curator-validated). For most v0.26
   work, that's `frontiers/bbb-alzheimer.json`.
2. Put inputs under `benchmarks/<vector-id>/inputs/{papers,notes,code,data}/`.
3. Run an agent (`vela scout` / `compile-notes` / `compile-code`
   / `compile-data`) to produce the candidate frontier; check
   in the resulting JSON as `benchmarks/<vector-id>/candidate.json`.
4. Run `vela bench --gold <gold> --candidate <candidate>` once
   to capture the metric values; write a `expected.json`
   regression band that brackets each metric loosely (±0.10 is
   usually right).
5. Add the vector to CI: `vela bench --gold <…> --candidate <…>
   --threshold <composite_band[0]>` exits non-zero on regression.

## What's not in v0.26

- A Hungarian matcher (greedy is sufficient for the F1 use case
  and stays deterministic).
- Reviewer-acceptance scoring (would require checked-in
  per-proposal accept/reject decisions; a v0.27 add-on if
  manual review data accumulates).
- A `Bench` link in the Workbench sidebar that fetches a recent
  bench-report.json and renders bar charts. Polish for v0.27.
- Cross-vector aggregation (running every vector in
  `benchmarks/` and rolling up). One-line bash loop today.
